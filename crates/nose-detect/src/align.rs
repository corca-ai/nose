//! Pairwise similarity: a cheap multiset Jaccard over shape features (the bulk
//! signal) plus an LCS-based alignment over the linearized node-tag sequences
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

/// RANSAC-style geometric verification (computer vision): treat token matches as
/// point correspondences, find the dominant position-offset (a 1-D "translation"
/// consensus), and score by the fraction of `a` positions consistent with it.
/// Tolerant to a block being shifted, unlike LCS.
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
    let a = &a[..a.len().min(LCS_CAP)];
    let b = &b[..b.len().min(LCS_CAP)];
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
            let off = match votes.iter().max_by_key(|(_, &c)| c).map(|(&o, _)| o) {
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

/// Cap on sequence length for LCS, to bound the O(n·m) DP on pathological units.
const LCS_CAP: usize = 600;

/// Longest-common-subsequence length / max(len), over linearized node tags.
/// Superseded by [`ransac_ratio`] in the default scorer (measured: RANSAC
/// generalizes better + is more precise), but kept as a tested alternative.
#[allow(dead_code)]
pub(crate) fn lcs_ratio(a: &[u64], b: &[u64]) -> f64 {
    let a = &a[..a.len().min(LCS_CAP)];
    let b = &b[..b.len().min(LCS_CAP)];
    let maxlen = a.len().max(b.len());
    if maxlen == 0 {
        return 1.0;
    }
    // Rolling 1-D DP.
    let mut prev = vec![0u32; b.len() + 1];
    let mut cur = vec![0u32; b.len() + 1];
    for &x in a {
        for j in 0..b.len() {
            cur[j + 1] = if x == b[j] {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()] as f64 / maxlen as f64
}
