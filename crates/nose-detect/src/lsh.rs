//! Locality-sensitive hashing over MinHash signatures. Signatures are split into
//! `bands` bands of `rows = k / bands` rows; units sharing any band's value are
//! emitted as a candidate pair. This avoids comparing unrelated
//! units; dense buckets still require O(k²) candidate pairs.
//!
//! Band entries are built and sorted in parallel; equal-hash runs form buckets.
//! Each left unit deduplicates its right neighbors before pair emission, so
//! overlapping buckets never create a large temporary array of repeated pairs.

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
#[cfg(test)]
pub(crate) fn candidates<'a>(
    n: usize,
    sig: impl Fn(usize) -> &'a [u64] + Sync,
    bands: usize,
) -> Vec<(usize, usize)> {
    let buckets = buckets(n, sig, bands);
    pairs(n, &buckets, &(0..n).collect::<Vec<_>>())
}

/// Emit the union of bucket pairs without materializing repeated channel pairs.
pub(crate) fn pairs(n: usize, buckets: &[Vec<u32>], groups: &[usize]) -> Vec<(usize, usize)> {
    let membership = membership(n, buckets);
    (0..n)
        .into_par_iter()
        .map_init(
            || vec![usize::MAX; n],
            |seen, left| {
                let mut neighbors = Vec::new();
                collect_neighbors(left, buckets, &membership, groups, seen, &mut neighbors);
                neighbors
                    .into_iter()
                    .map(|right| (left, right))
                    .collect::<Vec<_>>()
            },
        )
        .flat_map_iter(|pairs| pairs)
        .collect()
}

/// Visit every distinct pair in stable order while retaining at most one batch
/// and one endpoint's neighbors. The callback can discard rejected scores early.
pub(crate) fn visit_batches(
    n: usize,
    buckets: &[Vec<u32>],
    groups: &[usize],
    batch_size: usize,
    mut visit: impl FnMut(&[(usize, usize)]),
) {
    assert!(batch_size > 0);
    let membership = membership(n, buckets);
    let mut seen = vec![usize::MAX; n];
    let mut neighbors = Vec::new();
    let mut batch = Vec::with_capacity(batch_size);
    for left in 0..n {
        collect_neighbors(
            left,
            buckets,
            &membership,
            groups,
            &mut seen,
            &mut neighbors,
        );
        for &right in &neighbors {
            batch.push((left, right));
            if batch.len() == batch_size {
                visit(&batch);
                batch.clear();
            }
        }
    }
    if !batch.is_empty() {
        visit(&batch);
    }
}

fn collect_neighbors(
    left: usize,
    buckets: &[Vec<u32>],
    membership: &[Vec<usize>],
    groups: &[usize],
    seen: &mut [usize],
    neighbors: &mut Vec<usize>,
) {
    neighbors.clear();
    for &bucket in &membership[left] {
        let members = &buckets[bucket];
        let start = members.partition_point(|&right| right as usize <= left);
        for &right in &members[start..] {
            let right = right as usize;
            if groups[left] != groups[right] && seen[right] != left {
                seen[right] = left;
                neighbors.push(right);
            }
        }
    }
    neighbors.sort_unstable();
}

/// Count distinct pairs with O(units + band memberships) auxiliary memory.
/// Overlapping bands must not spend the budget repeatedly on the same pair.
pub(crate) fn candidate_count(
    n: usize,
    buckets: &[Vec<u32>],
    groups: &[usize],
    limit: usize,
) -> Option<usize> {
    if buckets.iter().any(|members| {
        let mut counts = rustc_hash::FxHashMap::<usize, usize>::default();
        for &member in members {
            *counts.entry(groups[member as usize]).or_default() += 1;
        }
        let pairs = |n: usize| n.saturating_mul(n.saturating_sub(1)) / 2;
        pairs(members.len()).saturating_sub(counts.into_values().map(pairs).sum()) > limit
    }) {
        return None;
    }
    let membership = membership(n, buckets);
    let mut seen = vec![usize::MAX; n];
    let mut count = 0;
    for (left, memberships) in membership.iter().enumerate() {
        for &bucket in memberships {
            let members = &buckets[bucket];
            let start = members.partition_point(|&right| right as usize <= left);
            for &right in &members[start..] {
                if groups[left] != groups[right as usize] && seen[right as usize] != left {
                    if count == limit {
                        return None;
                    }
                    seen[right as usize] = left;
                    count += 1;
                }
            }
        }
    }
    Some(count)
}

pub(crate) fn membership(n: usize, buckets: &[Vec<u32>]) -> Vec<Vec<usize>> {
    let mut membership = vec![Vec::new(); n];
    for (bucket, members) in buckets.iter().enumerate() {
        for &member in members {
            membership[member as usize].push(bucket);
        }
    }
    membership
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
    fn bounded_batches_keep_non_hub_edges_and_deduplicate_overlapping_channels() {
        let buckets = vec![(0..100).collect(), (25..150).collect(), vec![2, 4, 149]];
        let groups = (0..150).map(|i| i / 2).collect::<Vec<_>>();
        let expected = pairs(150, &buckets, &groups);
        for size in [1, 127, 4096] {
            let mut observed = Vec::new();
            visit_batches(150, &buckets, &groups, size, |batch| {
                assert!(batch.len() <= size);
                observed.extend_from_slice(batch);
            });
            assert_eq!(observed, expected);
        }
    }

    #[test]
    fn dense_equal_span_buckets_keep_every_cross_span_pair() {
        let buckets = vec![(0..1_000).collect()];
        let mut groups = vec![0; 1_000];
        groups[999] = 1;
        assert_eq!(candidate_count(1_000, &buckets, &groups, 999), Some(999));
        assert_eq!(candidate_count(1_000, &buckets, &groups, 998), None);
        assert_eq!(
            pairs(1_000, &buckets, &groups),
            (0..999).map(|left| (left, 999)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn overlapping_bands_spend_the_pair_budget_once() {
        let buckets = vec![vec![0, 1, 2], vec![0, 1, 3]];
        assert_eq!(candidate_count(4, &buckets, &[0, 1, 2, 3], 5), Some(5));
        assert_eq!(candidate_count(4, &buckets, &[0, 1, 2, 3], 4), None);
    }

    #[test]
    fn deduplicating_before_emission_preserves_all_band_pairs() {
        let signatures = (0..80)
            .map(|i| vec![i % 3, i % 5, i % 7, i % 11])
            .collect::<Vec<_>>();
        let buckets = buckets(signatures.len(), |i| &signatures[i], 4);
        let mut expected = buckets
            .iter()
            .flat_map(|b| bucket_pairs(b))
            .map(|(a, b)| (a as usize, b as usize))
            .collect::<Vec<_>>();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(
            candidate_count(
                signatures.len(),
                &buckets,
                &(0..signatures.len()).collect::<Vec<_>>(),
                expected.len()
            ),
            Some(expected.len())
        );
        assert_eq!(
            candidates(signatures.len(), |i| &signatures[i], 4),
            expected
        );
    }

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
