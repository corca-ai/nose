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

type IndexedCandidates = (Vec<(usize, usize)>, Vec<u16>);

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
    let mut previous = previous.filter(IncrementalDetectionState::is_valid);
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

    let mut counts = previous
        .as_ref()
        .into_iter()
        .flat_map(|state| {
            state.scores.iter().filter_map(|score| {
                score
                    .pair(&state.units)
                    .map(|pair| (pair, score.bucket_count))
            })
        })
        .collect::<BTreeMap<_, _>>();
    let buckets = if let Some(state) = previous.as_mut() {
        update_candidate_buckets(units, &unit_keys, opts, state, &mut counts, stats)
    } else {
        let memberships = candidate_memberships(units, &unit_keys, opts);
        stats.buckets_rebuilt = memberships.len();
        for (&key, members) in &memberships {
            adjust_pair_counts(&mut counts, key, members, true);
        }
        let positions = unit_index(&unit_keys);
        memberships
            .into_iter()
            .map(|(key, members)| store_bucket(key, &members, &positions))
            .collect()
    };
    counts.retain(|_, count| *count > 0);
    let (candidates, candidate_counts) = index_candidates(&unit_keys, &counts);
    let (
        previous_unit_keys,
        previous_scores,
        previous_components,
        previous_connected,
        previous_same_unit,
        previous_contiguous,
    ) = previous
        .map(|state| {
            (
                state.units,
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
        previous_unit_keys,
        previous_scores_aligned: false,
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
    let candidates = state
        .scores
        .iter()
        .map(|score| (score.left as usize, score.right as usize))
        .collect();
    let candidate_counts = state
        .scores
        .iter()
        .map(|score| score.bucket_count)
        .collect();
    PreparedDetection {
        unit_keys,
        candidates,
        candidate_counts,
        buckets: state.buckets,
        previous_scores: state.scores,
        previous_unit_keys: state.units,
        previous_scores_aligned: true,
        previous_components: state.components,
        previous_connected: state.connected,
        previous_same_unit: state.same_unit,
        previous_contiguous: state.contiguous,
    }
}

fn update_candidate_buckets(
    units: &[UnitFeat],
    unit_keys: &[UnitKey],
    opts: &DetectOptions,
    state: &mut IncrementalDetectionState,
    counts: &mut BTreeMap<UnitPairKey, u16>,
    stats: &mut IncrementalDetectionStats,
) -> Vec<CandidateBucket> {
    let current = unit_keys.iter().copied().collect::<BTreeSet<_>>();
    let previous = state.units.iter().copied().collect::<BTreeSet<_>>();
    let added_indices = unit_keys
        .iter()
        .enumerate()
        .filter_map(|(index, key)| (!previous.contains(key)).then_some(index))
        .collect::<Vec<_>>();
    let mut additions =
        candidate_memberships_selected(units, unit_keys, opts, added_indices.iter().copied(), None);
    let old_keys = state
        .buckets
        .iter()
        .map(|bucket| bucket.key)
        .collect::<BTreeSet<_>>();
    let new_keys = additions
        .keys()
        .filter(|key| !old_keys.contains(key))
        .copied()
        .collect::<BTreeSet<_>>();
    let new_memberships =
        candidate_memberships_selected(units, unit_keys, opts, 0..units.len(), Some(&new_keys));
    let current_positions = unit_index(unit_keys);
    let mut buckets = Vec::with_capacity(state.buckets.len() + new_memberships.len());
    for old in std::mem::take(&mut state.buckets) {
        let old_members = old
            .members
            .iter()
            .map(|&member| state.units[member as usize])
            .collect::<Vec<_>>();
        let added = additions.remove(&old.key).unwrap_or_default();
        let removed = old_members.iter().any(|member| !current.contains(member));
        if !removed && added.is_empty() {
            stats.buckets_reused += 1;
            buckets.push(store_bucket(old.key, &old_members, &current_positions));
            continue;
        }

        let mut members = old_members
            .iter()
            .copied()
            .filter(|member| current.contains(member))
            .chain(added)
            .collect::<Vec<_>>();
        members.sort_unstable_by_key(|member| current_positions[member]);
        members.dedup();
        adjust_pair_counts(counts, old.key, &old_members, false);
        if members.len() >= 2 {
            adjust_pair_counts(counts, old.key, &members, true);
            buckets.push(store_bucket(old.key, &members, &current_positions));
        }
        stats.buckets_rebuilt += 1;
    }
    for (key, members) in new_memberships {
        if members.len() < 2 {
            continue;
        }
        adjust_pair_counts(counts, key, &members, true);
        buckets.push(store_bucket(key, &members, &current_positions));
        stats.buckets_rebuilt += 1;
    }
    buckets.sort_unstable_by_key(|bucket| bucket.key);
    buckets
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
    let by_key = unit_index(unit_keys);
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
    let candidate_counts = rows.into_iter().map(|(_, _, count)| count).collect();
    (candidates, candidate_counts)
}

fn unit_index(unit_keys: &[UnitKey]) -> HashMap<UnitKey, usize> {
    unit_keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect()
}

fn store_bucket(
    key: BucketKey,
    members: &[UnitKey],
    positions: &HashMap<UnitKey, usize>,
) -> CandidateBucket {
    CandidateBucket {
        key,
        members: members
            .iter()
            .map(|member| positions[member] as u32)
            .collect(),
    }
}

pub(crate) fn score(
    units: &[UnitFeat],
    prepared: &PreparedDetection,
    detector: &dyn Detector,
    threshold: f64,
    stats: &mut IncrementalDetectionStats,
) -> (Vec<ScoredCandidate>, Vec<(usize, usize, f64)>) {
    let previous = (!prepared.previous_scores_aligned).then(|| {
        prepared
            .previous_scores
            .iter()
            .filter_map(|score| {
                score
                    .pair(&prepared.previous_unit_keys)
                    .map(|pair| (pair, score.ordinary_score))
            })
            .collect::<HashMap<_, _>>()
    });
    let reused = AtomicUsize::new(0);
    let evaluated = AtomicUsize::new(0);
    let scored = prepared
        .candidates
        .par_iter()
        .enumerate()
        .map(|(index, &(left, right))| {
            let pair = UnitPairKey::new(prepared.unit_keys[left], prepared.unit_keys[right]);
            let cached = if prepared.previous_scores_aligned {
                debug_assert_eq!(
                    (
                        prepared.previous_scores[index].left as usize,
                        prepared.previous_scores[index].right as usize,
                    ),
                    (left, right)
                );
                Some(prepared.previous_scores[index].ordinary_score)
            } else {
                previous
                    .as_ref()
                    .and_then(|scores| scores.get(&pair).copied())
            };
            let ordinary_score = cached.unwrap_or_else(|| {
                evaluated.fetch_add(1, Ordering::Relaxed);
                (!is_nested(&units[left], &units[right]))
                    .then(|| detector.score(&units[left], &units[right]))
            });
            if cached.is_some() {
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
    let mut buckets = candidate_memberships_selected(units, unit_keys, opts, 0..units.len(), None);
    buckets.retain(|_, members| members.len() >= 2);
    buckets
}

fn candidate_memberships_selected(
    units: &[UnitFeat],
    unit_keys: &[UnitKey],
    opts: &DetectOptions,
    indices: impl IntoIterator<Item = usize>,
    target_keys: Option<&BTreeSet<BucketKey>>,
) -> BTreeMap<BucketKey, Vec<UnitKey>> {
    let mut buckets = BTreeMap::new();
    let value_rows = signature_rows(units, opts.bands, false);
    let shape_rows = signature_rows(units, opts.bands, true);
    for index in indices {
        let unit = &units[index];
        let key = unit_keys[index];
        for_each_bucket_key(unit, opts, value_rows, shape_rows, |bucket| {
            if target_keys.is_none_or(|targets| targets.contains(&bucket)) {
                buckets.entry(bucket).or_insert_with(Vec::new).push(key);
            }
        });
    }
    buckets
}

fn signature_rows(units: &[UnitFeat], bands: usize, shape: bool) -> Option<usize> {
    let first = units.first()?;
    let signature = if shape {
        &first.shape_minhash
    } else {
        &first.minhash
    };
    (!signature.is_empty() && bands > 0).then(|| (signature.len() / bands).max(1))
}

fn for_each_bucket_key(
    unit: &UnitFeat,
    opts: &DetectOptions,
    value_rows: Option<usize>,
    shape_rows: Option<usize>,
    mut visit: impl FnMut(BucketKey),
) {
    if opts.value_candidates {
        if let Some(rows) = value_rows {
            visit_lsh_buckets(&unit.minhash, opts.bands, rows, false, &mut visit);
        }
        if exact_claim_eligible(unit) {
            visit(BucketKey::ExactValue(digest_u64s(&unit.value)));
        }
    }
    if opts.shape_candidates {
        if let Some(rows) = shape_rows {
            visit_lsh_buckets(&unit.shape_minhash, opts.bands, rows, true, &mut visit);
        }
        let floor = nose_normalize::anchor_min_weight();
        for anchor in &unit.anchors {
            if anchor.weight >= floor {
                visit(BucketKey::Anchor(anchor.hash));
            }
        }
    }
}

fn visit_lsh_buckets(
    signature: &[u64],
    bands: usize,
    rows: usize,
    shape: bool,
    visit: &mut impl FnMut(BucketKey),
) {
    for band in 0..bands {
        let start = band * rows;
        if start >= signature.len() {
            continue;
        }
        let end = (start + rows).min(signature.len());
        let hash = band_hash(band, &signature[start..end]);
        visit(if shape {
            BucketKey::ShapeBand(hash)
        } else {
            BucketKey::ValueBand(hash)
        });
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
