//! Pairwise similarity: a cheap multiset Jaccard over shape features (the bulk
//! signal) plus a RANSAC-style alignment over the linearized node-tag sequences
//! (the discriminative signal that token-set methods lack — it rewards units
//! whose structure lines up *in order*, not just in aggregate).

/// Weighted Jaccard for sorted `u64` feature multisets.
///
/// Inputs must be sorted in ascending order, with duplicates preserved as
/// multiplicity. The score is `Σ min(count) / Σ max(count)`, and two empty
/// multisets are treated as a full match (`1.0`).
pub fn multiset_jaccard(a: &[u64], b: &[u64]) -> f64 {
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

pub(crate) fn alignment_input(sequence: &[u64]) -> &[u64] {
    &sequence[..sequence.len().min(ALIGN_CAP)]
}

/// RANSAC-style geometric verification (computer vision): treat token matches as
/// point correspondences, find the dominant position-offset (a 1-D "translation"
/// consensus), and score by the fraction of `a` positions consistent with it.
/// Tolerant to a block being shifted, unlike an LCS alignment.
pub(crate) fn ransac_ratio(a: &[u64], b: &[u64]) -> f64 {
    use rustc_hash::FxHashMap;
    use std::cell::RefCell;
    // Only the first eight occurrences cast votes. Keep them inline while reusing
    // the token map's buckets across calls; later occurrences still count as inliers.
    #[derive(Default)]
    struct Positions {
        indices: [u16; 8],
        len: usize,
    }
    thread_local! {
        static POS: RefCell<FxHashMap<u64, Positions>> = RefCell::new(FxHashMap::default());
    }
    const OFFSET_ORIGIN: usize = ALIGN_CAP - 1;
    const {
        assert!(ALIGN_CAP <= u16::MAX as usize);
    }
    let a = alignment_input(a);
    let b = alignment_input(b);
    let maxlen = a.len().max(b.len());
    if maxlen == 0 {
        return 1.0;
    }
    POS.with(|pos_cell| {
        let mut pos = pos_cell.borrow_mut();
        pos.clear();
        for (j, &token) in b.iter().enumerate() {
            let positions = pos.entry(token).or_default();
            if positions.len < positions.indices.len() {
                positions.indices[positions.len] = j as u16;
                positions.len += 1;
            }
        }
        let mut votes = [0u32; 2 * ALIGN_CAP - 1];
        let mut best = (0u32, 0usize);
        for (i, token) in a.iter().enumerate() {
            if let Some(positions) = pos.get(token) {
                for &j in &positions.indices[..positions.len] {
                    let index = usize::from(j) + OFFSET_ORIGIN - i;
                    votes[index] += 1;
                    // Counts only increase, so the greatest (count, offset) seen
                    // so far is also the final winner. Larger offsets win ties.
                    best = best.max((votes[index], index));
                }
            }
        }
        if best.0 == 0 {
            return 0.0;
        }
        let offset = best.1 as i32 - OFFSET_ORIGIN as i32;
        let mut inliers = 0usize;
        for (i, &token) in a.iter().enumerate() {
            let j = i as i32 + offset;
            if j >= 0 && (j as usize) < b.len() && b[j as usize] == token {
                inliers += 1;
            }
        }
        inliers as f64 / maxlen as f64
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

    // Independent scalar oracle: keep every occurrence and select the winner only
    // after constructing an ordered offset map, matching the established contract.
    fn scalar_alignment(a: &[u64], b: &[u64]) -> f64 {
        use std::collections::BTreeMap;
        let a = alignment_input(a);
        let b = alignment_input(b);
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        let mut positions: BTreeMap<u64, Vec<i32>> = BTreeMap::new();
        for (j, &token) in b.iter().enumerate() {
            positions.entry(token).or_default().push(j as i32);
        }
        let mut votes: BTreeMap<i32, u32> = BTreeMap::new();
        for (i, token) in a.iter().enumerate() {
            if let Some(indices) = positions.get(token) {
                for &j in indices.iter().take(8) {
                    *votes.entry(j - i as i32).or_default() += 1;
                }
            }
        }
        let Some((&offset, _)) = votes.iter().max_by_key(|&(offset, count)| (count, offset)) else {
            return 0.0;
        };
        let inliers = a
            .iter()
            .enumerate()
            .filter(|&(i, token)| {
                let j = i as i32 + offset;
                j >= 0 && b.get(j as usize) == Some(token)
            })
            .count();
        inliers as f64 / a.len().max(b.len()) as f64
    }

    #[test]
    fn bounded_alignment_matches_scalar_contract() {
        let mut cases = vec![
            (vec![], vec![]),
            (vec![], vec![1]),
            (vec![1], vec![]),
            (vec![1; 601], vec![2; 601]),
            (vec![5, 9], vec![5, 5, 5, 5, 5, 5, 5, 5, 5, 9]),
            (vec![7; 601], vec![7; 601]),
            ((0..600).collect(), (599..1199).collect()),
            ((0..601).collect(), (600..1201).collect()),
        ];
        let mut seed = 0x5261_6e73_6163_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for alphabet in [1, 2, 8, 64, 1024] {
            for _ in 0..80 {
                let a_len = (next() % 620) as usize;
                let b_len = (next() % 620) as usize;
                let a = (0..a_len).map(|_| next() % alphabet).collect();
                let b = (0..b_len).map(|_| next() % alphabet).collect();
                cases.push((a, b));
            }
        }
        for (a, b) in cases {
            for (left, right) in [(&a, &b), (&b, &a)] {
                assert_eq!(
                    ransac_ratio(left, right).to_bits(),
                    scalar_alignment(left, right).to_bits()
                );
            }
        }
    }
}
