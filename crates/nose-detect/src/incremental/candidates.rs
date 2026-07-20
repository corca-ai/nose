use super::*;
use crate::candidates::{anchor_max_df, ANCHOR_PAIR_CAP, EXACT_VALUE_BUCKET_ALL_PAIRS_CAP};
use crate::detectors::Detector;
use crate::exact_policy::exact_claim_eligible;
use crate::locations::is_nested;
use crate::lsh::{band_hash, BUCKET_ALL_PAIRS_CAP};
use crate::{DetectOptions, UnitFeat};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};

type IndexedCandidates = (Vec<(usize, usize)>, Vec<(UnitPairKey, u16)>);

pub(crate) fn prepare(
    units: &[UnitFeat],
    stable_unit_keys: Option<&[[u8; 32]]>,
    opts: &DetectOptions,
    previous: Option<IncrementalDetectionState>,
    stats: &mut IncrementalDetectionStats,
) -> PreparedDetection {
    let unit_keys = if let Some(keys) = stable_unit_keys {
        assert_eq!(keys.len(), units.len());
        keys.iter().copied().map(UnitKey).collect::<Vec<_>>()
    } else {
        units.par_iter().map(unit_key).collect::<Vec<_>>()
    };
    let previous = previous.filter(|state| state.schema == STATE_SCHEMA);
    stats.state_hit = previous.is_some();
    record_unit_churn(previous.as_ref(), &unit_keys, stats);

    if previous
        .as_ref()
        .is_some_and(|state| state.units == unit_keys)
    {
        return reuse_unchanged(
            unit_keys,
            previous.expect("checked incremental state"),
            stats,
        );
    }

    let memberships = candidate_memberships(units, &unit_keys, opts);
    let previous_buckets = previous
        .as_ref()
        .map(|state| {
            state
                .buckets
                .iter()
                .map(|bucket| (bucket.key, bucket))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut counts = previous
        .as_ref()
        .into_iter()
        .flat_map(|state| &state.scores)
        .map(|score| (score.pair, score.bucket_count))
        .collect::<BTreeMap<_, _>>();
    let current_bucket_keys = memberships.keys().copied().collect::<BTreeSet<_>>();
    let mut buckets = Vec::with_capacity(memberships.len());
    for (key, members) in memberships {
        if previous_buckets
            .get(&key)
            .filter(|old| old.members == members)
            .is_some()
        {
            stats.buckets_reused += 1;
        } else {
            stats.buckets_rebuilt += 1;
            if let Some(old) = previous_buckets.get(&key) {
                adjust_pair_counts(&mut counts, key, &old.members, false);
            }
            adjust_pair_counts(&mut counts, key, &members, true);
        }
        buckets.push(CandidateBucket { key, members });
    }
    for (&key, old) in &previous_buckets {
        if !current_bucket_keys.contains(&key) {
            stats.buckets_rebuilt += 1;
            adjust_pair_counts(&mut counts, key, &old.members, false);
        }
    }
    counts.retain(|_, count| *count > 0);
    let (candidates, candidate_counts) = index_candidates(&unit_keys, &counts);
    let (
        previous_scores,
        previous_components,
        previous_connected,
        previous_same_unit,
        previous_contiguous,
    ) = previous
        .map(|state| {
            (
                state.scores,
                state.components,
                state.connected,
                state.same_unit,
                state.contiguous,
            )
        })
        .unwrap_or_default();
    PreparedDetection {
        unit_keys,
        candidates,
        candidate_counts,
        buckets,
        previous_scores,
        previous_components,
        previous_connected,
        previous_same_unit,
        previous_contiguous,
    }
}

fn reuse_unchanged(
    unit_keys: Vec<UnitKey>,
    state: IncrementalDetectionState,
    stats: &mut IncrementalDetectionStats,
) -> PreparedDetection {
    stats.buckets_reused = state.buckets.len();
    let counts = state
        .scores
        .iter()
        .map(|score| (score.pair, score.bucket_count))
        .collect::<BTreeMap<_, _>>();
    let (candidates, candidate_counts) = index_candidates(&unit_keys, &counts);
    PreparedDetection {
        unit_keys,
        candidates,
        candidate_counts,
        buckets: state.buckets,
        previous_scores: state.scores,
        previous_components: state.components,
        previous_connected: state.connected,
        previous_same_unit: state.same_unit,
        previous_contiguous: state.contiguous,
    }
}

fn adjust_pair_counts(
    counts: &mut BTreeMap<UnitPairKey, u16>,
    key: BucketKey,
    members: &[UnitKey],
    add: bool,
) {
    for pair in emit_bucket_pairs(key, members) {
        let count = counts.entry(pair).or_default();
        if add {
            *count = count.saturating_add(1);
        } else {
            *count = count.saturating_sub(1);
        }
    }
}

fn index_candidates(
    unit_keys: &[UnitKey],
    counts: &BTreeMap<UnitPairKey, u16>,
) -> IndexedCandidates {
    let by_key = unit_keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<HashMap<_, _>>();
    let mut rows = counts
        .iter()
        .filter_map(|(&pair, &count)| {
            let left = *by_key.get(&pair.left)?;
            let right = *by_key.get(&pair.right)?;
            let indexes = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            Some((indexes, pair, count))
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|row| row.0);
    let candidates = rows.iter().map(|row| row.0).collect();
    let candidate_counts = rows
        .into_iter()
        .map(|(_, pair, count)| (pair, count))
        .collect();
    (candidates, candidate_counts)
}

pub(crate) fn score(
    units: &[UnitFeat],
    prepared: &PreparedDetection,
    detector: &dyn Detector,
    threshold: f64,
    stats: &mut IncrementalDetectionStats,
) -> (Vec<ScoredCandidate>, Vec<(usize, usize, f64)>) {
    let previous = prepared
        .previous_scores
        .iter()
        .map(|score| (score.pair, score.ordinary_score))
        .collect::<HashMap<_, _>>();
    let reused = AtomicUsize::new(0);
    let evaluated = AtomicUsize::new(0);
    let scored = prepared
        .candidates
        .par_iter()
        .map(|&(left, right)| {
            let pair = UnitPairKey::new(prepared.unit_keys[left], prepared.unit_keys[right]);
            let ordinary_score = previous.get(&pair).copied().unwrap_or_else(|| {
                evaluated.fetch_add(1, Ordering::Relaxed);
                (!is_nested(&units[left], &units[right]))
                    .then(|| detector.score(&units[left], &units[right]))
            });
            if previous.contains_key(&pair) {
                reused.fetch_add(1, Ordering::Relaxed);
            }
            ScoredCandidate {
                left,
                right,
                ordinary_score,
            }
        })
        .collect::<Vec<_>>();
    stats.scores_reused = reused.load(Ordering::Relaxed);
    stats.scores_evaluated = evaluated.load(Ordering::Relaxed);
    let accepted = scored
        .iter()
        .filter_map(|candidate| {
            candidate
                .ordinary_score
                .filter(|&value| value >= threshold)
                .map(|value| (candidate.left, candidate.right, value))
        })
        .collect();
    (scored, accepted)
}

fn record_unit_churn(
    previous: Option<&IncrementalDetectionState>,
    current: &[UnitKey],
    stats: &mut IncrementalDetectionStats,
) {
    let before = previous
        .map(|state| state.units.iter().copied().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let after = current.iter().copied().collect::<BTreeSet<_>>();
    stats.units_reused = before.intersection(&after).count();
    stats.units_added = after.difference(&before).count();
    stats.units_removed = before.difference(&after).count();
}

fn candidate_memberships(
    units: &[UnitFeat],
    unit_keys: &[UnitKey],
    opts: &DetectOptions,
) -> BTreeMap<BucketKey, Vec<UnitKey>> {
    let mut buckets = BTreeMap::<BucketKey, Vec<UnitKey>>::new();
    if opts.value_candidates {
        append_lsh_memberships(&mut buckets, units, unit_keys, opts.bands, false);
        for (unit, &key) in units.iter().zip(unit_keys) {
            if exact_claim_eligible(unit) {
                buckets
                    .entry(BucketKey::ExactValue(digest_u64s(&unit.value)))
                    .or_default()
                    .push(key);
            }
        }
    }
    if opts.shape_candidates {
        append_lsh_memberships(&mut buckets, units, unit_keys, opts.bands, true);
        let floor = nose_normalize::anchor_min_weight();
        for (unit, &key) in units.iter().zip(unit_keys) {
            for anchor in &unit.anchors {
                if anchor.weight >= floor {
                    buckets
                        .entry(BucketKey::Anchor(anchor.hash))
                        .or_default()
                        .push(key);
                }
            }
        }
    }
    buckets.retain(|_, members| members.len() >= 2);
    buckets
}

fn append_lsh_memberships(
    buckets: &mut BTreeMap<BucketKey, Vec<UnitKey>>,
    units: &[UnitFeat],
    unit_keys: &[UnitKey],
    bands: usize,
    shape: bool,
) {
    let Some(first) = units.first() else { return };
    let first_signature = if shape {
        &first.shape_minhash
    } else {
        &first.minhash
    };
    if first_signature.is_empty() || bands == 0 {
        return;
    }
    let rows = (first_signature.len() / bands).max(1);
    for (unit, &key) in units.iter().zip(unit_keys) {
        let signature = if shape {
            &unit.shape_minhash
        } else {
            &unit.minhash
        };
        for band in 0..bands {
            let start = band * rows;
            if start >= signature.len() {
                continue;
            }
            let end = (start + rows).min(signature.len());
            let hash = band_hash(band, &signature[start..end]);
            let bucket = if shape {
                BucketKey::ShapeBand(hash)
            } else {
                BucketKey::ValueBand(hash)
            };
            buckets.entry(bucket).or_default().push(key);
        }
    }
}

fn emit_bucket_pairs(key: BucketKey, members: &[UnitKey]) -> Vec<UnitPairKey> {
    match key {
        BucketKey::Anchor(_) if members.len() > anchor_max_df() => Vec::new(),
        BucketKey::Anchor(_) => all_pairs_capped(members, ANCHOR_PAIR_CAP),
        BucketKey::ExactValue(_) => connected_pairs(members, EXACT_VALUE_BUCKET_ALL_PAIRS_CAP),
        BucketKey::ValueBand(_) | BucketKey::ShapeBand(_) => {
            connected_pairs(members, BUCKET_ALL_PAIRS_CAP)
        }
    }
}

fn connected_pairs(members: &[UnitKey], all_pairs_cap: usize) -> Vec<UnitPairKey> {
    if members.len() <= all_pairs_cap {
        return all_pairs_capped(members, usize::MAX);
    }
    let mut pairs = members
        .windows(2)
        .map(|window| UnitPairKey::new(window[0], window[1]))
        .collect::<Vec<_>>();
    pairs.extend(
        members[1..]
            .iter()
            .map(|&member| UnitPairKey::new(members[0], member)),
    );
    pairs
}

fn all_pairs_capped(members: &[UnitKey], cap: usize) -> Vec<UnitPairKey> {
    let mut pairs = Vec::new();
    'outer: for left in 0..members.len() {
        for right in (left + 1)..members.len() {
            pairs.push(UnitPairKey::new(members[left], members[right]));
            if pairs.len() >= cap {
                break 'outer;
            }
        }
    }
    pairs
}

fn unit_key(unit: &UnitFeat) -> UnitKey {
    let bytes = rmp_serde::to_vec(unit).expect("UnitFeat serialization cannot fail");
    UnitKey(digest(b"nose.incremental-unit.v1", &bytes))
}

fn digest_u64s(values: &[u64]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"nose.incremental-exact-value.v1\0");
    hasher.update((values.len() as u64).to_be_bytes());
    for value in values {
        hasher.update(value.to_be_bytes());
    }
    hasher.finalize().into()
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}
