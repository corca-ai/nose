//! MinHash signatures over a unit's distinct structural features, for cheap
//! near-linear candidate generation via LSH (see [`crate::lsh`]).

const MIX: u64 = 0x9E37_79B9_7F4A_7C15;

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(MIX);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `k` independent permutation seeds.
pub(crate) fn seeds(k: usize) -> Vec<u64> {
    (0..k)
        .map(|i| splitmix64(i as u64 ^ 0xD1B5_4A32_D192_ED03))
        .collect()
}

/// MinHash signature of a *distinct* feature set: `sig[i] = min_f h_i(f)`.
pub(crate) fn sign(distinct: &[u64], seeds: &[u64]) -> Vec<u64> {
    let mut sig = vec![u64::MAX; seeds.len()];
    for &f in distinct {
        for (i, &s) in seeds.iter().enumerate() {
            let h = splitmix64(f ^ s);
            if h < sig[i] {
                sig[i] = h;
            }
        }
    }
    sig
}

/// Estimated Jaccard similarity from two signatures (fraction of equal slots).
#[allow(dead_code)]
pub(crate) fn estimate(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    let eq = a.iter().zip(b).filter(|(x, y)| x == y).count();
    eq as f64 / a.len() as f64
}
