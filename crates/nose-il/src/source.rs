//! Immutable source provenance. Coordinates describe one snapshot; content
//! digests deliberately identify equal bytes, including distinct copies.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    pub fn sha256(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn derive(domain: &[u8], components: &[&[u8]]) -> Self {
        let mut digest = Sha256::new();
        digest.update((domain.len() as u64).to_be_bytes());
        digest.update(domain);
        for component in components {
            digest.update((component.len() as u64).to_be_bytes());
            digest.update(component);
        }
        Self(digest.finalize().into())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn hex(self) -> String {
        String::from_utf8(self.hex_bytes().to_vec()).expect("hex digits are ASCII")
    }

    fn hex_bytes(self) -> [u8; 64] {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0; 64];
        for (byte, digits) in self.0.into_iter().zip(out.chunks_exact_mut(2)) {
            digits[0] = HEX[(byte >> 4) as usize];
            digits[1] = HEX[(byte & 15) as usize];
        }
        out
    }
}

impl Serialize for ContentDigest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer
            .serialize_str(std::str::from_utf8(&self.hex_bytes()).expect("hex digits are ASCII"))
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(serde::de::Error::custom(
                "expected 64 lowercase SHA-256 hex digits",
            ));
        }
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Self(bytes))
    }
}

/// The buffer supplied to the frontend, shared by embedded regions. Derived
/// indexes are never persisted as independent sources of truth.
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceDocument {
    bytes: Vec<u8>,
    #[serde(skip)]
    digest: OnceLock<ContentDigest>,
    #[serde(skip)]
    lines: OnceLock<Vec<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRegion {
    pub source_digest: ContentDigest,
    pub start_byte: u32,
    pub end_byte: u32,
    pub content_digest: ContentDigest,
}

impl SourceDocument {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            digest: OnceLock::new(),
            lines: OnceLock::new(),
        }
    }

    pub fn digest(&self) -> ContentDigest {
        *self
            .digest
            .get_or_init(|| ContentDigest::sha256(&self.bytes))
    }

    pub fn region(&self, start_byte: u32, end_byte: u32) -> Option<SourceRegion> {
        if start_byte >= end_byte {
            return None;
        }
        let bytes = self.bytes.get(start_byte as usize..end_byte as usize)?;
        Some(SourceRegion {
            source_digest: self.digest(),
            start_byte,
            end_byte,
            content_digest: ContentDigest::sha256(bytes),
        })
    }

    /// A line-selected witness intentionally includes complete source lines,
    /// preserving CRLF and the final newline. Invalid selectors never clamp.
    pub fn line_region(&self, start: u32, end: u32) -> Option<SourceRegion> {
        if start == 0 || start > end {
            return None;
        }
        let lines = self.lines.get_or_init(|| {
            let mut lines = vec![0];
            for (index, byte) in self.bytes.iter().enumerate() {
                if *byte == b'\n' && index + 1 < self.bytes.len() {
                    lines.push(index + 1);
                }
            }
            lines
        });
        let start_byte = *lines.get(start as usize - 1)?;
        lines.get(end as usize - 1)?;
        let end_byte = lines.get(end as usize).copied().unwrap_or(self.bytes.len());
        self.region(
            u32::try_from(start_byte).ok()?,
            u32::try_from(end_byte).ok()?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bytes_and_invalid_ranges() {
        let source = SourceDocument::new("α();\r\nβ();\n".as_bytes().to_vec());
        let region = source.line_region(2, 2).unwrap();
        assert_eq!(region.start_byte, 7);
        assert_eq!(
            region.content_digest,
            ContentDigest::sha256("β();\n".as_bytes())
        );
        for (start, end) in [(0, 1), (2, 1), (1, 3)] {
            assert!(source.line_region(start, end).is_none());
        }
        assert!(source.region(3, 3).is_none());
        assert!(source.region(0, u32::MAX).is_none());
    }

    #[test]
    fn framed_hash_and_persistent_encoding() {
        assert_ne!(
            ContentDigest::derive(b"x", &[b"ab", b"c"]),
            ContentDigest::derive(b"x", &[b"a", b"bc"])
        );
        assert_ne!(
            ContentDigest::derive(b"x", &[b"a"]),
            ContentDigest::derive(b"y", &[b"a"])
        );
        let digest = ContentDigest::sha256(b"abc");
        assert_eq!(
            digest.hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            serde_json::from_str::<ContentDigest>(&serde_json::to_string(&digest).unwrap())
                .unwrap(),
            digest
        );
        assert!(serde_json::from_str::<ContentDigest>("\"xyz\"").is_err());
    }
}
