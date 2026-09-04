//! Locality-sensitive hashing over MinHash signatures. Signatures are split into
//! `bands` bands of `rows = k / bands` rows; units sharing any band's value are
//! emitted as a candidate pair. This avoids comparing unrelated
//! units; dense buckets still require O(k²) candidate pairs.
//!
//! The implementation is **sort-based** rather than hash-map-based: every
//! `(band-hash, unit)` entry is produced in parallel, sorted once (a parallel
//! radix-friendly sort), and equal-hash runs are the buckets. Sorting beats a
//! `HashMap<key, Vec>` here — contiguous memory, no per-bucket allocation, and the
//! sort + per-bucket pair emission both parallelize across cores.

use rayon::prelude::*;

const SEED: u64 = 0xA24B_AED4_963E_E407;

/// Hash one band's rows, folding the band index in so band *b*'s hash never aliases
/// band *b'*'s — a single `u64` key then identifies the whole `(band, value)` bucket.
#[inline]
pub(crate) fn band_hash(band: usize, slice: &[u64]) -> u64 {
    let mut h = SEED ^ (band as u64).wrapping_mul(0x100_0000_01B3);
    for &x in slice {
        h = (h ^ x).wrapping_mul(0x1000_0000_01B3);
    }
    h
}

/// Generate candidate `(i, j)` pairs (i < j) from `n` unit signatures, each
/// accessed by `sig(idx)`. Taking a borrowing accessor (rather than an owned
/// `&[Vec<u64>]`) lets the caller pass `|i| &units[i].minhash[..]` with no copy.
pub(crate) fn candidates<'a>(
    n: usize,
    sig: impl Fn(usize) -> &'a [u64] + Sync,
    bands: usize,
) -> Vec<(usize, usize)> {
    let buckets = buckets(n, sig, bands);
    let mut pairs: Vec<(u32, u32)> = buckets
        .par_iter()
        .flat_map_iter(|members| bucket_pairs(members))
        .collect();
    pairs.par_sort_unstable();
    pairs.dedup();
    pairs
        .into_iter()
        .map(|(i, j)| (i as usize, j as usize))
        .collect()
}

pub(crate) fn buckets<'a>(
    n: usize,
    sig: impl Fn(usize) -> &'a [u64] + Sync,
    bands: usize,
) -> Vec<Vec<u32>> {
    let k = if n == 0 { 0 } else { sig(0).len() };
    if k == 0 || bands == 0 {
        return Vec::new();
    }
    let rows = (k / bands).max(1);

    // 1. Every (band-hash, unit) entry, computed in parallel. `u32` units keep the
    //    entry 16 bytes so the sort streams through cache.
    let mut entries: Vec<(u64, u32)> = (0..n)
        .into_par_iter()
        .flat_map_iter(|idx| {
            let s = sig(idx);
            (0..bands).filter_map(move |b| {
                let start = b * rows;
                (start < s.len()).then(|| {
                    let end = (start + rows).min(s.len());
                    (band_hash(b, &s[start..end]), idx as u32)
                })
            })
        })
        .collect();

    // 2. Sort so equal-hash entries are contiguous — these runs are the buckets.
    entries.par_sort_unstable();

    // 3. Find bucket boundaries (cheap O(n) pass over contiguous memory)…
    let mut bounds = Vec::new();
    let mut start = 0;
    while start < entries.len() {
        let h = entries[start].0;
        let mut end = start + 1;
        while end < entries.len() && entries[end].0 == h {
            end += 1;
        }
        if end - start >= 2 {
            bounds.push((start, end)); // bucket with ≥2 members
        }
        start = end;
    }

    // Bands often contain the same dense family. Emit each distinct membership
    // only once, but never prune pairs before scoring: a connectivity skeleton
    // can disconnect accepted clones when its chosen hub fails the scorer.
    let mut buckets = bounds
        .iter()
        .map(|&(s, e)| {
            entries[s..e]
                .iter()
                .map(|entry| entry.1)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    buckets.par_sort_unstable();
    buckets.dedup();
    buckets
}

/// Enumerate every unordered pair once, in member order. Both clean and
/// incremental candidates share this rule; callers apply explicit anchor caps.
pub(crate) fn bucket_pairs<T: Copy>(members: &[T]) -> impl Iterator<Item = (T, T)> + '_ {
    members
        .iter()
        .enumerate()
        .flat_map(move |(left, &a)| members[left + 1..].iter().map(move |&b| (a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_a_dense_bucket_preserves_every_existing_candidate() {
        let signature = [7; 128];
        let small = candidates(48, |_| &signature, 32);
        let large = candidates(49, |_| &signature, 32);
        assert_eq!(small.len(), 48 * 47 / 2);
        assert_eq!(large.len(), 49 * 48 / 2);
        assert!(small.iter().all(|pair| large.binary_search(pair).is_ok()));
        assert!(large.contains(&(1, 3)));
    }
}
