use sha2::{Digest as _, Sha256};
use std::hash::Hasher;

/// A collision-resistant content identity used by every cache layer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ContentDigest([u8; 32]);

impl ContentDigest {
    pub(super) fn sha256(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Hash length-framed components under a domain separator. Framing prevents
    /// ambiguous concatenations (`["ab", "c"]` vs `["a", "bc"]`).
    pub(super) fn derive(domain: &[u8], components: &[&[u8]]) -> Self {
        let mut digest = Sha256::new();
        digest.update((domain.len() as u64).to_be_bytes());
        digest.update(domain);
        for component in components {
            digest.update((component.len() as u64).to_be_bytes());
            digest.update(component);
        }
        Self(digest.finalize().into())
    }

    pub(super) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(super) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(super) fn hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write as _;
            write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
        }
        out
    }
}

/// A deterministic [`Hasher`] backed by SHA-256. Rust's derived [`Hash`]
/// implementations can feed this without falling back to a 64-bit cache key.
pub(super) struct StableSha256(Sha256);

impl StableSha256 {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut inner = Sha256::new();
        inner.update((domain.len() as u64).to_be_bytes());
        inner.update(domain);
        Self(inner)
    }

    pub(super) fn finish_digest(self) -> ContentDigest {
        ContentDigest(self.0.finalize().into())
    }
}

impl Hasher for StableSha256 {
    fn finish(&self) -> u64 {
        let clone = self.0.clone().finalize();
        u64::from_be_bytes(
            clone[..8]
                .try_into()
                .expect("SHA-256 prefix is eight bytes"),
        )
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u64).to_be_bytes());
        self.0.update(bytes);
    }

    fn write_u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn write_u16(&mut self, value: u16) {
        self.0.update(value.to_be_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn write_i8(&mut self, value: i8) {
        self.0.update(value.to_be_bytes());
    }

    fn write_i16(&mut self, value: i16) {
        self.0.update(value.to_be_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.0.update(value.to_be_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.0.update(value.to_be_bytes());
    }

    fn write_i128(&mut self, value: i128) {
        self.0.update(value.to_be_bytes());
    }

    fn write_isize(&mut self, value: isize) {
        self.write_i64(value as i64);
    }
}
