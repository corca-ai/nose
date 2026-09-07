//! Preserve the identity byte protocol while avoiding small-buffer allocations.
use nose_il::ContentDigest;
use serde::Serialize;
use std::io::{self, Write};

pub(crate) fn digest(domain: &[u8], value: &impl Serialize) -> ContentDigest {
    let mut bytes = IdentityBytes::default();
    rmp_serde::encode::write_named(&mut bytes, value).expect("identity records serialize");
    ContentDigest::derive(domain, &[bytes.as_slice()])
}

struct IdentityBytes {
    inline: [u8; 512],
    len: usize,
    overflow: Vec<u8>,
}

impl Default for IdentityBytes {
    fn default() -> Self {
        Self {
            inline: [0; 512],
            len: 0,
            overflow: Vec::new(),
        }
    }
}

impl IdentityBytes {
    fn as_slice(&self) -> &[u8] {
        if self.overflow.is_empty() {
            &self.inline[..self.len]
        } else {
            &self.overflow
        }
    }
}

impl Write for IdentityBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.overflow.is_empty() && bytes.len() <= self.inline.len() - self.len {
            self.inline[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        } else {
            if self.overflow.is_empty() {
                self.overflow.reserve(self.len + bytes.len());
                self.overflow.extend_from_slice(&self.inline[..self.len]);
            }
            self.overflow.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_and_overflow_writes_preserve_every_byte() {
        for size in [0, 1, 511, 512, 513, 4096] {
            let expected = (0..size).map(|i| (i % 251) as u8).collect::<Vec<_>>();
            for chunk in [1, 17, 512, 1024, 8192] {
                let mut bytes = IdentityBytes::default();
                for part in expected.chunks(chunk) {
                    bytes.write_all(part).unwrap();
                }
                bytes.write_all(&[]).unwrap();
                assert_eq!(bytes.as_slice(), expected);
            }
        }
    }

    #[test]
    fn identity_encoding_matches_named_messagepack_and_length_framing() {
        #[derive(Serialize)]
        struct Record {
            label: String,
            values: Vec<u64>,
            optional: Option<(bool, i64)>,
        }
        for size in [0, 1, 511, 512, 513, 4096] {
            let value = Record {
                label: "region λ".into(),
                values: (0..size).collect(),
                optional: Some((true, -7)),
            };
            let original = rmp_serde::to_vec_named(&value).unwrap();
            let mut bytes = IdentityBytes::default();
            rmp_serde::encode::write_named(&mut bytes, &value).unwrap();
            assert_eq!(bytes.as_slice(), original);
            for domain in [
                b"nose.region-analysis/v1".as_slice(),
                b"nose.review-content/v1",
            ] {
                assert_eq!(
                    digest(domain, &value),
                    ContentDigest::derive(domain, &[&original])
                );
            }
        }
    }
}
