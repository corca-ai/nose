use super::digest::ContentDigest;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const MAGIC: &[u8; 8] = b"NOSECAS2";
const FORMAT_SCHEMA: u32 = 2;
const FLAG_ZSTD: u8 = 1;
const ZSTD_LEVEL: i32 = 7;
const HEADER_LEN: usize = 8 + 4 + 1 + 1 + 2 + 4 + 32 + 8 + 8 + 32;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024 * 1024;
const MAX_STORED_BYTES: u64 = MAX_PAYLOAD_BYTES as u64 + HEADER_LEN as u64;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Stable layer ids. The layer schema in each key evolves independently from
/// the shared envelope schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ArtifactStage {
    SourceSnapshot = 1,
    RawIl = 2,
    ExportDependencySummary = 3,
    ResolvedIl = 4,
    UnitsSyntax = 5,
    StateRecord = 6,
}

impl ArtifactStage {
    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::SourceSnapshot,
            2 => Self::RawIl,
            3 => Self::ExportDependencySummary,
            4 => Self::ResolvedIl,
            5 => Self::UnitsSyntax,
            6 => Self::StateRecord,
            _ => return None,
        })
    }

    fn directory(self) -> &'static str {
        match self {
            Self::SourceSnapshot => "source-snapshot",
            Self::RawIl => "raw-il",
            Self::ExportDependencySummary => "export-dependency-summary",
            Self::ResolvedIl => "resolved-il",
            Self::UnitsSyntax => "units-syntax",
            Self::StateRecord => "state-record",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ArtifactKey {
    pub(super) stage: ArtifactStage,
    pub(super) schema: u32,
    pub(super) digest: ContentDigest,
}

impl ArtifactKey {
    pub(super) fn derive(stage: ArtifactStage, schema: u32, components: &[&[u8]]) -> Self {
        let stage_code = [stage as u8];
        let schema_bytes = schema.to_be_bytes();
        let mut framed = Vec::with_capacity(components.len() + 2);
        framed.push(stage_code.as_slice());
        framed.push(schema_bytes.as_slice());
        framed.extend_from_slice(components);
        Self {
            stage,
            schema,
            digest: ContentDigest::derive(b"nose.stage-artifact-key.v1", &framed),
        }
    }
}

pub(super) struct CasRead {
    pub(super) payload: Vec<u8>,
    pub(super) stored_bytes: u64,
}

pub(super) struct LayeredCas {
    root: PathBuf,
    written_bytes: Option<Arc<AtomicU64>>,
}

impl LayeredCas {
    #[cfg(test)]
    pub(super) fn new(root: &Path) -> Self {
        Self::tracked(root, None)
    }

    pub(super) fn with_write_counter(root: &Path, written_bytes: Arc<AtomicU64>) -> Self {
        Self::tracked(root, Some(written_bytes))
    }

    fn tracked(root: &Path, written_bytes: Option<Arc<AtomicU64>>) -> Self {
        Self {
            root: root.join("cas-v2"),
            written_bytes,
        }
    }

    pub(super) fn load(&self, key: ArtifactKey) -> Option<CasRead> {
        let path = self.path(key);
        let (payload, stored_bytes) = read_envelope_file(&path, key)?;
        Some(CasRead {
            payload,
            stored_bytes,
        })
    }

    /// Publish a complete checksummed envelope. A successful return means the
    /// payload and its directory entry have been synced before readers can see it.
    pub(super) fn store(&self, key: ArtifactKey, payload: &[u8]) -> std::io::Result<u64> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cache payload exceeds the decoded-size limit",
            ));
        }
        if self.load(key).is_some() {
            return Ok(0);
        }
        let target = self.path(key);
        let parent = target.parent().expect("CAS entries always have a parent");
        std::fs::create_dir_all(parent)?;
        let bytes = encode_envelope(key, payload);
        let temp = temporary_path(parent, &key.digest.hex());
        write_synced(&temp, &bytes)?;
        match publish(&temp, &target) {
            Ok(()) => {
                sync_parent(parent)?;
                let stored = bytes.len() as u64;
                if let Some(counter) = &self.written_bytes {
                    counter.fetch_add(stored, Ordering::Relaxed);
                }
                Ok(stored)
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(error)
            }
        }
    }

    fn path(&self, key: ArtifactKey) -> PathBuf {
        let hex = key.digest.hex();
        self.root
            .join(key.stage.directory())
            .join(&hex[..2])
            .join(format!("{}.artifact", &hex[2..]))
    }
}

pub(super) fn read_envelope_file(path: &Path, key: ArtifactKey) -> Option<(Vec<u8>, u64)> {
    let stored_bytes = std::fs::metadata(path).ok()?.len();
    if stored_bytes > MAX_STORED_BYTES {
        warn_corrupt(path, "stored length exceeds the cache limit");
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    match decode_envelope(&bytes, key) {
        Some(payload) => Some((payload, stored_bytes)),
        None => {
            warn_corrupt(path, "invalid header, checksum, or compressed payload");
            None
        }
    }
}

pub(super) fn temporary_path(parent: &Path, suffix: &str) -> PathBuf {
    parent.join(format!(
        ".{}.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        suffix
    ))
}

pub(super) fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
pub(super) fn sync_parent(parent: &Path) -> std::io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
pub(super) fn sync_parent(_parent: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn publish(temp: &Path, target: &Path) -> std::io::Result<()> {
    // POSIX replacement is atomic. Concurrent writers of one content key
    // converge on identical bytes.
    std::fs::rename(temp, target)
}

#[cfg(not(unix))]
pub(super) fn publish(temp: &Path, target: &Path) -> std::io::Result<()> {
    if target.exists() {
        std::fs::remove_file(target)?;
    }
    std::fs::rename(temp, target)
}

pub(super) fn encode_envelope(key: ArtifactKey, payload: &[u8]) -> Vec<u8> {
    // Portable IL regions already carry independently bounded Zstandard frames.
    // Recompressing their bundle raises cold latency and warm peak memory without
    // materially shrinking it.
    let compressed = (!matches!(key.stage, ArtifactStage::RawIl | ArtifactStage::ResolvedIl))
        .then(|| zstd::bulk::compress(payload, ZSTD_LEVEL).ok())
        .flatten();
    let (flags, stored) = if compressed
        .as_ref()
        .is_some_and(|compressed| compressed.len() < payload.len())
    {
        (FLAG_ZSTD, compressed.as_deref().unwrap_or_default())
    } else {
        (0, payload)
    };
    let checksum = ContentDigest::sha256(payload);
    let mut bytes = Vec::with_capacity(HEADER_LEN + stored.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_SCHEMA.to_be_bytes());
    bytes.push(key.stage as u8);
    bytes.push(flags);
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&key.schema.to_be_bytes());
    bytes.extend_from_slice(key.digest.as_bytes());
    bytes.extend_from_slice(&(stored.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(checksum.as_bytes());
    bytes.extend_from_slice(stored);
    bytes
}

pub(super) fn decode_envelope(bytes: &[u8], key: ArtifactKey) -> Option<Vec<u8>> {
    if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
        return None;
    }
    let format = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
    let stage = ArtifactStage::from_code(bytes[12])?;
    let flags = bytes[13];
    let schema = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let digest = ContentDigest::from_bytes(bytes[20..52].try_into().ok()?);
    let stored_len = u64::from_be_bytes(bytes[52..60].try_into().ok()?);
    let payload_len = u64::from_be_bytes(bytes[60..68].try_into().ok()?);
    let checksum = ContentDigest::from_bytes(bytes[68..100].try_into().ok()?);
    if format != FORMAT_SCHEMA
        || stage != key.stage
        || schema != key.schema
        || digest != key.digest
        || flags & !FLAG_ZSTD != 0
        || stored_len > MAX_STORED_BYTES
        || payload_len > MAX_PAYLOAD_BYTES as u64
        || HEADER_LEN.checked_add(stored_len as usize)? != bytes.len()
    {
        return None;
    }
    let stored = &bytes[HEADER_LEN..];
    let payload = if flags & FLAG_ZSTD != 0 {
        zstd::bulk::decompress(stored, payload_len as usize).ok()?
    } else {
        if stored_len != payload_len {
            return None;
        }
        stored.to_vec()
    };
    (payload.len() == payload_len as usize && ContentDigest::sha256(&payload) == checksum)
        .then_some(payload)
}

fn warn_corrupt(path: &Path, reason: &str) {
    eprintln!(
        "warning: ignoring corrupt cache entry {}: {reason}; recomputing",
        path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nose_cas_{label}_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn all_stage_identities_are_distinct() {
        let stages = [
            ArtifactStage::SourceSnapshot,
            ArtifactStage::RawIl,
            ArtifactStage::ExportDependencySummary,
            ArtifactStage::ResolvedIl,
            ArtifactStage::UnitsSyntax,
            ArtifactStage::StateRecord,
        ];
        let keys = stages.map(|stage| ArtifactKey::derive(stage, 1, &[b"same content"]));
        for (index, key) in keys.iter().enumerate() {
            assert!(keys[..index].iter().all(|prior| prior.digest != key.digest));
        }
    }

    #[test]
    fn compression_and_round_trip_preserve_payload() {
        let key = ArtifactKey::derive(ArtifactStage::UnitsSyntax, 3, &[b"input"]);
        let payload = vec![7; 64 * 1024];
        let bytes = encode_envelope(key, &payload);
        assert!(bytes.len() < payload.len() / 4);
        assert_eq!(decode_envelope(&bytes, key), Some(payload));
    }

    #[test]
    fn corruption_truncation_and_oversized_lengths_are_cache_misses() {
        let root = temp_store("corruption");
        let _ = std::fs::remove_dir_all(&root);
        let cas = LayeredCas::new(&root);
        let key = ArtifactKey::derive(ArtifactStage::ResolvedIl, 3, &[b"input"]);
        cas.store(key, b"portable payload").unwrap();
        assert_eq!(cas.load(key).unwrap().payload, b"portable payload");

        let path = cas.path(key);
        let mut corrupt = std::fs::read(&path).unwrap();
        *corrupt.last_mut().unwrap() ^= 0xff;
        std::fs::write(&path, corrupt).unwrap();
        assert!(cas.load(key).is_none());

        cas.store(key, b"portable payload").unwrap();
        let mut truncated = std::fs::read(&path).unwrap();
        truncated.truncate(HEADER_LEN + 3);
        std::fs::write(&path, truncated).unwrap();
        assert!(cas.load(key).is_none());

        let mut oversized = encode_envelope(key, b"small");
        oversized[60..68].copy_from_slice(&((MAX_PAYLOAD_BYTES as u64) + 1).to_be_bytes());
        assert!(decode_envelope(&oversized, key).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
