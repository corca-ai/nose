//! Locality-sensitive hashing over MinHash signatures. Signatures are split into
//! `bands` bands of `rows = k / bands` rows; units sharing any band's value are
//! emitted as a candidate pair. This keeps candidate generation near-linear
//! instead of comparing all O(n²) unit pairs.
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
fn band_hash(band: usize, slice: &[u64]) -> u64 {
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

    // 3. Find bucket boundaries (cheap O(n) scan over contiguous memory)…
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

    // 4. …then emit each bucket's pairs in parallel.
    let mut pairs: Vec<(u32, u32)> = bounds
        .par_iter()
        .flat_map_iter(|&(s, e)| {
            let members = &entries[s..e];
            let mut out = Vec::new();
            if members.len() <= BUCKET_ALL_PAIRS_CAP {
                // Small bucket: full all-pairs (keeps every clone pair for reporting).
                for a in 0..members.len() {
                    for b in (a + 1)..members.len() {
                        out.push(ordered(members[a].1, members[b].1));
                    }
                }
            } else {
                // Huge bucket (a dense near-duplicate family, e.g. 100s of locale
                // files): all-pairs is O(k²) and union-find collapses it anyway.
                // Emit a chain plus a star to the first member — O(k) edges that keep
                // the family connected.
                for w in members.windows(2) {
                    out.push(ordered(w[0].1, w[1].1));
                }
                for m in &members[1..] {
                    out.push(ordered(members[0].1, m.1));
                }
            }
            out
        })
        .collect();

    // 5. A pair can recur across bands; sort + dedup once (parallel sort).
    pairs.par_sort_unstable();
    pairs.dedup();
    pairs
        .into_iter()
        .map(|(i, j)| (i as usize, j as usize))
        .collect()
}

/// Above this bucket size, switch from O(k²) all-pairs to an O(k) connectivity
/// skeleton (chain + star) — bounds candidate-gen on dense corpora.
const BUCKET_ALL_PAIRS_CAP: usize = 48;

#[inline]
fn ordered(i: u32, j: u32) -> (u32, u32) {
    if i < j {
        (i, j)
    } else {
        (j, i)
    }
}
