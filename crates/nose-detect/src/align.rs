//! Pairwise similarity: a cheap multiset Jaccard over shape features (the bulk
//! signal) plus a RANSAC-style alignment over the linearized node-tag sequences
//! (the discriminative signal that token-set methods lack — it rewards units
//! whose structure lines up *in order*, not just in aggregate).

/// Weighted (multiset) Jaccard of two sorted feature multisets:
/// `Σ min(count) / Σ max(count)`.
pub(crate) fn multiset_jaccard(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (mut i, mut j) = (0, 0);
    let (mut inter, mut union) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                union += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                union += 1;
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                inter += 1;
                union += 1;
                i += 1;
                j += 1;
            }
        }
    }
    union += (a.len() - i) + (b.len() - j);
    if union == 0 {
        return 0.0;
    }
    inter as f64 / union as f64
}

/// Cap on linearization length for alignment scoring, bounding the per-pair cost
/// on pathological units.
const ALIGN_CAP: usize = 600;

/// RANSAC-style geometric verification (computer vision): treat token matches as
/// point correspondences, find the dominant position-offset (a 1-D "translation"
/// consensus), and score by the fraction of `a` positions consistent with it.
/// Tolerant to a block being shifted, unlike an LCS alignment.
pub(crate) fn ransac_ratio(a: &[u64], b: &[u64]) -> f64 {
    use rustc_hash::FxHashMap;
    use std::cell::RefCell;
    // Reusable per-thread scratch: this is the detector's hot path (~300k calls on
    // a large corpus), so we clear-and-reuse the maps instead of allocating two
    // HashMaps (and a Vec per token) on every call.
    thread_local! {
        static POS: RefCell<FxHashMap<u64, Vec<i32>>> = RefCell::new(FxHashMap::default());
        static VOTES: RefCell<FxHashMap<i32, u32>> = RefCell::new(FxHashMap::default());
    }
    let a = &a[..a.len().min(ALIGN_CAP)];
    let b = &b[..b.len().min(ALIGN_CAP)];
    let maxlen = a.len().max(b.len());
    if maxlen == 0 {
        return 1.0;
    }
    POS.with(|pos_cell| {
        VOTES.with(|votes_cell| {
            let mut pos = pos_cell.borrow_mut();
            let mut votes = votes_cell.borrow_mut();
            pos.clear();
            votes.clear();
            for (j, &t) in b.iter().enumerate() {
                pos.entry(t).or_default().push(j as i32);
            }
            // vote offsets (capped per token to bound cost)
            for (i, &t) in a.iter().enumerate() {
                if let Some(js) = pos.get(&t) {
                    for &j in js.iter().take(8) {
                        *votes.entry(j - i as i32).or_default() += 1;
                    }
                }
            }
            // Consensus offset = most-voted alignment shift. Break vote-count ties by
            // the offset value (a unique map key), NOT by `max_by_key`'s "last max
            // wins" — `votes` is a reused thread-local `FxHashMap` whose capacity (and
            // thus iteration order on ties) depends on how many prior pairs this worker
            // handled, which varies with the thread count. Without the tie-break, a tied
            // offset resolved differently across thread schedules, flipping marginal
            // pairs' scores and breaking byte-identical output (seen on clap/nushell/
            // h2database).
            let off = match votes.iter().max_by_key(|(&o, &c)| (c, o)).map(|(&o, _)| o) {
                Some(o) => o,
                None => return 0.0,
            };
            // inliers: a positions whose match exists at the consensus offset
            let mut inliers = 0usize;
            for (i, &t) in a.iter().enumerate() {
                let bj = i as i32 + off;
                if bj >= 0 && (bj as usize) < b.len() && b[bj as usize] == t {
                    inliers += 1;
                }
            }
            inliers as f64 / maxlen as f64
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the consensus-offset tie-break that keeps `ransac_ratio` deterministic.
    /// The scorer's reused thread-local vote map is cleared but not shrunk between calls,
    /// so its capacity — and thus its iteration order on ties — depends on how many prior
    /// pairs the worker handled, which varies with the thread count. Breaking a vote-count
    /// tie on the offset value (a unique key) instead of "whichever tied entry iterates
    /// last" makes the pick independent of that layout. Before this, clap/nushell/
    /// h2database produced thread-count-dependent output.
    #[test]
    fn ransac_ratio_breaks_offset_ties_deterministically() {
        // Token `5` appears 9× in `b`, but `a[0]` votes for only the first 8 of its
        // occurrences (the `.take(8)` cap). So the *best* shift — offset 8, which aligns
        // `a[0]==b[8]` and `a[1]==b[9]` for 2 inliers — gets just 1 vote, tying with the
        // 1-inlier offsets 0..7. The deterministic tie-break takes the largest offset (8),
        // giving 2 inliers / maxlen 10 = 0.2. A non-deterministic tie-break could land on
        // a 1-inlier offset (0.1), so this value also guards the inlier count.
        let a: Vec<u64> = vec![5, 9];
        let b: Vec<u64> = vec![5, 5, 5, 5, 5, 5, 5, 5, 5, 9];
        assert_eq!(ransac_ratio(&a, &b), 0.2);
        // And the result must not depend on the reused scratch map's capacity: growing it
        // with an unrelated call must not change the score.
        let big: Vec<u64> = (0..500u64).flat_map(|x| [x, x ^ 0x5a5a]).collect();
        let _ = ransac_ratio(&big, &big);
        assert_eq!(ransac_ratio(&a, &b), 0.2);
    }
}
