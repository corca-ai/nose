use crate::{
    align,
    cluster::UnionFind,
    detectors::{env_or, exact_claim_eligible, EXACT_VALUE_MIN},
    locations::loc_of,
    lsh,
    model::{EnclosingUnit, EquivalenceWitness, Group, Loc},
    options::DetectOptions,
    units::{self, UnitFeat},
};
use nose_semantics::ValueLaw;
use std::collections::HashMap;

fn group_witness(members: &[usize], units: &[UnitFeat]) -> EquivalenceWitness {
    let first = &units[members[0]];
    if members.iter().all(|&m| {
        let u = &units[m];
        exact_claim_eligible(u) && u.value == first.value
    }) {
        return EquivalenceWitness {
            kind: "exact-value-graph",
            value_nodes: Some(first.value.len()),
            mean_value_jaccard: None,
            mean_shape_jaccard: None,
            graded: None,
            graded_pair: None,
        };
    }
    if members.len() >= 2 && shared_subdag_hash(members, units).is_some() {
        return EquivalenceWitness {
            kind: "shared-sub-dag",
            value_nodes: None,
            mean_value_jaccard: None,
            mean_shape_jaccard: None,
            graded: None,
            graded_pair: None,
        };
    }
    let (mut vj, mut sj) = (0.0, 0.0);
    for &m in &members[1..] {
        vj += align::multiset_jaccard(&first.value, &units[m].value);
        sj += align::multiset_jaccard(&first.shapes, &units[m].shapes);
    }
    let n = (members.len().saturating_sub(1)).max(1) as f64;
    EquivalenceWitness {
        kind: "structural-similarity",
        value_nodes: None,
        mean_value_jaccard: Some(round3(vj / n)),
        mean_shape_jaccard: Some(round3(sj / n)),
        graded: None,
        graded_pair: None,
    }
}

const EXACT_VALUE_BUCKET_ALL_PAIRS_CAP: usize = 48;

pub(crate) fn structural_candidates(
    units: &[UnitFeat],
    opts: &DetectOptions,
) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();
    if opts.value_candidates {
        candidates.extend(lsh::candidates(
            units.len(),
            |i| units[i].minhash.as_slice(),
            opts.bands,
        ));
        candidates.extend(exact_value_candidates(units));
    }
    if opts.shape_candidates {
        candidates.extend(lsh::candidates(
            units.len(),
            |i| units[i].shape_minhash.as_slice(),
            opts.bands,
        ));
        // Partial / sub-DAG clones: pair units that share a rare heavy anchor (an
        // extractable common sub-computation). They share no shape band, so shape-LSH
        // alone never proposes them — this is the candidate channel's sub-DAG path.
        candidates.extend(anchor_candidates(units));
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// Build the report's `groups` from the clustered components.
///
/// Group score = mean of the accepted-pair scores within the group. Accumulate it in
/// ONE pass over `accepted` instead of rescanning every accepted pair for every group
/// (which was O(groups × accepted) — ~1e9 iterations / ~0.9s on guava's 17.6k groups ×
/// 59k pairs, the detector's real hot spot). Each accepted pair was unioned, so its two
/// endpoints share a component; index its contribution by that component's root. The
/// per-group sum still walks `accepted` in order, so the float total — and the rounded
/// score — is byte-identical to the per-group rescan.
pub(crate) fn build_groups(
    units: &[UnitFeat],
    accepted: &[(usize, usize, f64)],
    uf: &mut UnionFind,
    raw_groups: &[Vec<usize>],
    enclosing: &[Option<EnclosingUnit>],
    opts: &DetectOptions,
) -> Vec<Group> {
    let mut by_root: rustc_hash::FxHashMap<usize, (f64, u32)> = rustc_hash::FxHashMap::default();
    for &(i, _j, s) in accepted {
        let e = by_root.entry(uf.find(i)).or_insert((0.0, 0));
        e.0 += s;
        e.1 += 1;
    }
    raw_groups
        .iter()
        .map(|members| {
            let root = uf.find(members[0]);
            let (sum, n) = by_root.get(&root).copied().unwrap_or((0.0, 0));
            let score = if n == 0 { 0.0 } else { sum / n as f64 };
            let mut locs: Vec<Loc> = members
                .iter()
                .map(|&m| loc_of(&units[m], enclosing[m].clone()))
                .collect();
            // If every member shares a heavy sub-DAG (a partial / sub-DAG clone), annotate each
            // site with its OWN source range for that shared computation — so the report can point
            // at where the shared logic lives in each copy, not just that one exists.
            if let Some(hash) = shared_subdag_hash(members, units) {
                for (&m, loc) in members.iter().zip(locs.iter_mut()) {
                    if let Some(a) = units[m].anchors.iter().find(|a| a.hash == hash) {
                        if a.line_start > 0 || a.line_end > 0 {
                            loc.shared_subdag = Some((a.line_start, a.line_end));
                        }
                    }
                }
            }
            Group {
                score: round3(score),
                members: locs,
                semantic_laws: semantic_laws_for_members(members, units),
                abstraction_witness: if opts.abstraction_witnesses {
                    units::abstraction_family_witness(members.iter().map(|&m| &units[m]))
                } else {
                    None
                },
                witness: Some(group_witness(members, units)),
            }
        })
        .collect()
}

fn semantic_laws_for_members(members: &[usize], units: &[UnitFeat]) -> Vec<ValueLaw> {
    let mut laws = members
        .iter()
        .flat_map(|&member| units[member].semantic_laws.iter().copied())
        .collect::<Vec<_>>();
    laws.sort_unstable();
    laws.dedup();
    laws
}

fn exact_value_candidates(units: &[UnitFeat]) -> Vec<(usize, usize)> {
    let mut buckets: HashMap<Vec<u64>, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        if unit.exact_safe && unit.value.len() >= EXACT_VALUE_MIN {
            buckets.entry(unit.value.clone()).or_default().push(idx);
        }
    }
    let mut out = Vec::new();
    for members in buckets.values() {
        if members.len() < 2 {
            continue;
        }
        if members.len() <= EXACT_VALUE_BUCKET_ALL_PAIRS_CAP {
            for a in 0..members.len() {
                for b in (a + 1)..members.len() {
                    out.push(ordered_pair(members[a], members[b]));
                }
            }
        } else {
            for w in members.windows(2) {
                out.push(ordered_pair(w[0], w[1]));
            }
            for &m in &members[1..] {
                out.push(ordered_pair(members[0], m));
            }
        }
    }
    out
}

/// An anchor present in more than this many units is boilerplate (a common idiom), not a
/// specific extractable sub-computation — skip it. Env-overridable.
fn anchor_max_df() -> usize {
    use std::sync::OnceLock;
    static D: OnceLock<usize> = OnceLock::new();
    *D.get_or_init(|| env_or("NOSE_ANCHOR_MAX_DF", 6.0) as usize)
}

const ANCHOR_PAIR_CAP: usize = 64;

/// Partial / sub-DAG clone candidates: units that share a RARE heavy anchor — an extractable
/// common sub-computation that whole-unit Jaccard misses. Index anchor → units; for anchors
/// present in `2..=anchor_max_df` units, emit their pairs (capped per bucket). Common anchors
/// (boilerplate above the df ceiling) are skipped so this stays specific.
fn anchor_candidates(units: &[UnitFeat]) -> Vec<(usize, usize)> {
    // Anchors are COLLECTED at the finer containment floor; the near channel keeps its
    // own coarser floor at every consumption point, so its behavior is unchanged.
    let floor = nose_normalize::anchor_min_weight();
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        for a in &unit.anchors {
            if a.weight < floor {
                continue;
            }
            buckets.entry(a.hash).or_default().push(idx);
        }
    }
    let max_df = anchor_max_df();
    let mut out = Vec::new();
    for members in buckets.values() {
        if members.len() < 2 || members.len() > max_df {
            continue;
        }
        let mut count = 0;
        'pairs: for a in 0..members.len() {
            for b in (a + 1)..members.len() {
                out.push(ordered_pair(members[a], members[b]));
                count += 1;
                if count >= ANCHOR_PAIR_CAP {
                    break 'pairs;
                }
            }
        }
    }
    out
}

/// The heaviest sub-DAG anchor present in EVERY member of a group (`None` if none is shared by
/// all) — the shared computation the report annotates each site with. For a 2-member partial clone
/// this is the shared anchor; for a larger family it is the common heavy computation.
pub(crate) fn shared_subdag_hash(members: &[usize], units: &[UnitFeat]) -> Option<u64> {
    if members.len() < 2 {
        return None;
    }
    let floor = nose_normalize::anchor_min_weight();
    units[members[0]]
        .anchors
        .iter()
        .filter(|a| {
            a.weight >= floor
                && members[1..]
                    .iter()
                    .all(|&m| units[m].anchors.iter().any(|b| b.hash == a.hash))
        })
        .max_by_key(|a| a.weight)
        .map(|a| a.hash)
}

/// The weight of the LARGEST sub-DAG the two units share (0 if none) — a shared anchor is a
/// shared extractable sub-computation, and a bigger one is a stronger partial-clone signal.
/// Both anchor lists are sorted by hash, so this is a linear merge.
pub(crate) fn shared_anchor_weight(
    a: &[nose_normalize::Anchor],
    b: &[nose_normalize::Anchor],
) -> u32 {
    // Near-channel floor (collection runs at the finer containment floor).
    let floor = nose_normalize::anchor_min_weight();
    let (mut i, mut j, mut best) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].hash.cmp(&b[j].hash) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                if a[i].weight >= floor && b[j].weight >= floor {
                    best = best.max(a[i].weight.min(b[j].weight));
                }
                i += 1;
                j += 1;
            }
        }
    }
    best
}

#[inline]
fn ordered_pair(i: usize, j: usize) -> (usize, usize) {
    if i < j {
        (i, j)
    } else {
        (j, i)
    }
}

pub(crate) fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}
