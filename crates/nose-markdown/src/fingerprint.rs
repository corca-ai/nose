//! Stage 1 — candidate generation.
//!
//! Incremental, cacheable, sub-quadratic candidate generation over the char-n-gram substrate:
//!   * **MinHash + LSH banding** — order-invariant resemblance candidates (robust to block
//!     reorder; the survey's reorder fault line).
//!   * **Winnowing (MOSS)** — positional local fingerprints in an inverted index; gives the
//!     small-in-large / shared-span candidates and a recall guarantee for spans ≥ w+k−1.
//!   * **Containment** — asymmetric overlap for "a small section pasted into a large document".
//!
//! Deterministic by construction: fixed hash (FNV-1a), fixed permutation seeds, sorted/dedup'd
//! sets, and candidate pairs accumulated into an ordered set.

use crate::unit::Unit;
use std::collections::{BTreeSet, HashMap};

pub const NUM_PERM: usize = 128;
const LSH_ROWS: usize = 4; // bands = NUM_PERM/ROWS = 32; S-curve threshold ~0.42 (recall-favoring)
const MERSENNE: u128 = (1 << 61) - 1;
pub const WINNOW_K: usize = 5;
pub const WINNOW_W: usize = 4;

/// Deterministic 64-bit FNV-1a over a string's bytes.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic LCG (no global RNG state) used to derive MinHash permutation coefficients.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn minhash_coeffs() -> Vec<(u128, u128)> {
    let mut rng = Lcg(0x00C0_FFEE);
    (0..NUM_PERM)
        .map(|_| {
            let a = (rng.next() as u128 % (MERSENNE - 1)) + 1;
            let b = rng.next() as u128 % MERSENNE;
            (a, b)
        })
        .collect()
}

thread_local! {
    static COEFFS: Vec<(u128, u128)> = minhash_coeffs();
}

/// Per-unit Stage-1 fingerprints.
#[derive(Clone, Debug)]
pub struct Fingerprint {
    /// Sorted, de-duplicated char-gram shingle hashes (for exact Jaccard / containment).
    pub shingles: Vec<u64>,
    /// MinHash signature (length NUM_PERM); unbiased Jaccard estimator + LSH key source.
    pub minhash: Vec<u64>,
    /// Winnowing fingerprint set (sorted, de-duplicated); positional local match candidates.
    pub winnow: Vec<u64>,
}

impl Fingerprint {
    pub fn of(unit: &Unit) -> Fingerprint {
        let mut shingles: Vec<u64> = unit.shingles().iter().map(|g| fnv1a(g)).collect();
        shingles.sort_unstable();
        shingles.dedup();
        let minhash = minhash_sig(&shingles);
        let winnow = winnow(&unit.norm);
        Fingerprint {
            shingles,
            minhash,
            winnow,
        }
    }
}

fn minhash_sig(shingles: &[u64]) -> Vec<u64> {
    if shingles.is_empty() {
        return vec![0; NUM_PERM];
    }
    COEFFS.with(|coeffs| {
        coeffs
            .iter()
            .map(|&(a, b)| {
                shingles
                    .iter()
                    .map(|&h| {
                        let h32 = (h & 0xffff_ffff) as u128;
                        ((a.wrapping_mul(h32) + b) % MERSENNE) as u64
                    })
                    .min()
                    .unwrap_or(0)
            })
            .collect()
    })
}

/// Winnowing (Schleimer/Wilkerson/Aiken): rightmost minimum in each window of `WINNOW_W`
/// consecutive char-`WINNOW_K`-gram hashes.
fn winnow(norm: &str) -> Vec<u64> {
    let chars: Vec<char> = norm.chars().collect();
    if chars.len() < WINNOW_K {
        return Vec::new();
    }
    let grams: Vec<u64> = (0..=chars.len() - WINNOW_K)
        .map(|i| {
            let g: String = chars[i..i + WINNOW_K].iter().collect();
            fnv1a(&g)
        })
        .collect();
    let mut fps = BTreeSet::new();
    if grams.len() < WINNOW_W {
        if let Some(&m) = grams.iter().min() {
            fps.insert(m);
        }
        return fps.into_iter().collect();
    }
    let mut last_pos: isize = -1;
    for i in 0..=grams.len() - WINNOW_W {
        let window = &grams[i..i + WINNOW_W];
        let m = *window.iter().min().unwrap();
        // rightmost occurrence of the minimum in this window
        let j = i + window.iter().rposition(|&x| x == m).unwrap();
        if j as isize != last_pos {
            fps.insert(grams[j]);
            last_pos = j as isize;
        }
    }
    fps.into_iter().collect()
}

/// Jaccard of two sorted, de-duplicated u64 sets.
pub fn jaccard(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = intersection_len(a, b);
    let union = a.len() + b.len() - inter;
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

/// Containment of the smaller set in the larger: |a∩b| / min(|a|,|b|).
pub fn containment(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    intersection_len(a, b) as f64 / a.len().min(b.len()) as f64
}

fn intersection_len(a: &[u64], b: &[u64]) -> usize {
    let (mut i, mut j, mut c) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                c += 1;
                i += 1;
                j += 1;
            }
        }
    }
    c
}

/// MinHash estimate of Jaccard: fraction of agreeing signature positions.
pub fn minhash_est(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let eq = a.iter().zip(b).filter(|(x, y)| x == y).count();
    eq as f64 / a.len() as f64
}

/// Generate candidate pairs sub-quadratically: union of MinHash-LSH buckets (resemblance) and
/// the winnowing inverted index (shared-span / small-in-large). Returns ordered unique pairs.
pub fn candidate_pairs(fps: &[Fingerprint]) -> Vec<(usize, usize)> {
    let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
    let bands = NUM_PERM / LSH_ROWS;

    // MinHash-LSH: bucket by (band index, hash of the band's rows).
    let mut buckets: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (idx, fp) in fps.iter().enumerate() {
        if fp.shingles.is_empty() {
            continue;
        }
        for band in 0..bands {
            let mut h: u64 = 0xcbf29ce484222325 ^ (band as u64).wrapping_mul(0x100000001b3);
            for r in 0..LSH_ROWS {
                h ^= fp.minhash[band * LSH_ROWS + r];
                h = h.wrapping_mul(0x100000001b3);
            }
            let entry = buckets.entry((band, h)).or_default();
            for &other in entry.iter() {
                pairs.insert(order(other, idx));
            }
            entry.push(idx);
        }
    }

    // Winnowing inverted index: any two units sharing a fingerprint are span candidates.
    // Stop-shingle guard: drop fingerprints whose document frequency exceeds a small fraction of
    // the corpus — these are ubiquitous boilerplate grams that otherwise flood candidates with
    // near-zero-similarity pairs (the survey's boilerplate failure mode).
    let stop_df = (fps.len() / 25).max(8);
    let mut winv: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, fp) in fps.iter().enumerate() {
        for &w in &fp.winnow {
            winv.entry(w).or_default().push(idx);
        }
    }
    // Require ≥WINNOW_MIN_SHARED shared fingerprints for a winnow candidate: a single shared
    // 5-gram is weak evidence and floods candidates with near-zero-similarity pairs; a real
    // partial/contained overlap shares many fingerprints.
    const WINNOW_MIN_SHARED: u32 = 3;
    let mut shared: HashMap<(usize, usize), u32> = HashMap::new();
    for members in winv.values() {
        if members.len() < 2 || members.len() > stop_df {
            continue;
        }
        for a in 0..members.len() {
            for b in a + 1..members.len() {
                *shared.entry(order(members[a], members[b])).or_default() += 1;
            }
        }
    }
    for (pair, c) in shared {
        if c >= WINNOW_MIN_SHARED {
            pairs.insert(pair);
        }
    }

    pairs.into_iter().collect()
}

fn order(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::split_units;

    fn fp(text: &str) -> Fingerprint {
        let u = split_units("t.md", text);
        Fingerprint::of(&u[0])
    }

    #[test]
    fn minhash_tracks_true_jaccard() {
        let a = fp("the quick brown fox jumps over the lazy dog in the morning light");
        let b = fp("the quick brown fox leaps over a lazy dog in the evening light");
        let exact = jaccard(&a.shingles, &b.shingles);
        let est = minhash_est(&a.minhash, &b.minhash);
        assert!((exact - est).abs() < 0.12, "exact={exact} est={est}");
    }

    #[test]
    fn identical_text_is_candidate_and_full_jaccard() {
        let t = "the quick brown fox jumps over the lazy dog in the morning";
        let fps = vec![fp(t), fp(t)];
        assert_eq!(jaccard(&fps[0].shingles, &fps[1].shingles), 1.0);
        let cands = candidate_pairs(&fps);
        assert!(cands.contains(&(0, 1)));
    }

    #[test]
    fn containment_catches_small_in_large() {
        let small = fp("error handling requires careful resource cleanup on every path");
        let large = fp(
            "this document is very long and discusses many topics at length. \
             error handling requires careful resource cleanup on every path. \
             it then continues for a while with much additional unrelated material.",
        );
        let c = containment(&small.shingles, &large.shingles);
        let j = jaccard(&small.shingles, &large.shingles);
        assert!(c > 0.8, "containment={c}");
        assert!(c > j, "containment {c} should exceed jaccard {j}");
    }

    #[test]
    fn unrelated_text_not_full_jaccard() {
        let a = fp("the quick brown fox jumps over the lazy dog");
        let b = fp("database indexes accelerate lookups across large tables");
        assert!(jaccard(&a.shingles, &b.shingles) < 0.2);
    }

    #[test]
    fn deterministic_minhash() {
        let a = fp("deterministic fingerprints must be byte identical across runs always");
        let b = fp("deterministic fingerprints must be byte identical across runs always");
        assert_eq!(a.minhash, b.minhash);
        assert_eq!(a.winnow, b.winnow);
    }
}
