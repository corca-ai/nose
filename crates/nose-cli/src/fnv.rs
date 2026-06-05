//! FNV-1a 64-bit hashing primitives, shared by the content-cache key (`cache.rs`) and the
//! baseline family key (`baseline.rs`). Centralizing the constants and the per-step mix keeps
//! the two stable on-disk hashes computed by exactly the same arithmetic.

/// FNV-1a 64-bit offset basis — the accumulator seed.
pub(crate) const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime — the per-step multiplier.
pub(crate) const PRIME: u64 = 0x0000_0100_0000_01b3;

/// One FNV-1a step: xor `x` into the accumulator `h`, then multiply by [`PRIME`].
pub(crate) fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(PRIME)
}
