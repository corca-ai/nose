use super::digest::ContentDigest;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"NOSECAS1";
const FORMAT_SCHEMA: u32 = 1;
const HEADER_LEN: usize = 8 + 4 + 1 + 3 + 4 + 32 + 8 + 32;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Stable layer ids. All six #873 stages share one envelope and address space;
/// later issues can activate layers without inventing another storage format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ArtifactStage {
    SourceSnapshot = 1,
    RawIl = 2,
    ExportDependencySummary = 3,
    ResolvedIl = 4,
    UnitsSyntax = 5,
    GlobalDetectionIndex = 6,
}

impl ArtifactStage {
    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::SourceSnapshot,
            2 => Self::RawIl,
            3 => Self::ExportDependencySummary,
            4 => Self::ResolvedIl,
            5 => Self::UnitsSyntax,
            6 => Self::GlobalDetectionIndex,
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
            Self::GlobalDetectionIndex => "global-detection-index",
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
}

impl LayeredCas {
    pub(super) fn new(root: &Path) -> Self {
        Self {
            root: root.join("cas-v1"),
        }
    }

    pub(super) fn load(&self, key: ArtifactKey) -> Option<CasRead> {
        let bytes = std::fs::read(self.path(key)).ok()?;
        let stored_bytes = bytes.len() as u64;
        let payload = validate_envelope(&bytes, key)?;
        Some(CasRead {
            payload: payload.to_vec(),
            stored_bytes,
        })
    }

    /// Atomically publish a complete checksummed envelope. An invalid existing
    /// entry is replaced; readers that race the rename see either complete file.
    pub(super) fn store(&self, key: ArtifactKey, payload: &[u8]) -> std::io::Result<u64> {
        if let Some(existing) = self.load(key) {
            let _ = existing;
            return Ok(0);
        }
        let target = self.path(key);
        let parent = target.parent().expect("CAS entries always have a parent");
        std::fs::create_dir_all(parent)?;
        let bytes = envelope(key, payload);
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!(
            ".{}.{}.{}.tmp",
            std::process::id(),
            sequence,
            key.digest.hex()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.flush()?;
        drop(file);
        match publish(&temp, &target) {
            Ok(()) => Ok(bytes.len() as u64),
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

#[cfg(unix)]
fn publish(temp: &Path, target: &Path) -> std::io::Result<()> {
    // POSIX rename replaces atomically: concurrent readers never observe a
    // partially written envelope, and concurrent writers of one key converge.
    std::fs::rename(temp, target)
}

#[cfg(not(unix))]
fn publish(temp: &Path, target: &Path) -> std::io::Result<()> {
    // `rename` cannot replace on every supported non-Unix filesystem. A racing
    // reader may conservatively miss during this repair window, but can never
    // consume partial bytes because `temp` was fully synced first.
    if target.exists() {
        std::fs::remove_file(target)?;
    }
    std::fs::rename(temp, target)
}

fn envelope(key: ArtifactKey, payload: &[u8]) -> Vec<u8> {
    let checksum = ContentDigest::sha256(payload);
    let mut bytes = Vec::with_capacity(HEADER_LEN + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_SCHEMA.to_be_bytes());
    bytes.push(key.stage as u8);
    bytes.extend_from_slice(&[0; 3]);
    bytes.extend_from_slice(&key.schema.to_be_bytes());
    bytes.extend_from_slice(key.digest.as_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(checksum.as_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn validate_envelope(bytes: &[u8], key: ArtifactKey) -> Option<&[u8]> {
    if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
        return None;
    }
    let format = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
    let stage = ArtifactStage::from_code(bytes[12])?;
    let schema = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let digest = ContentDigest::from_bytes(bytes[20..52].try_into().ok()?);
    let payload_len = u64::from_be_bytes(bytes[52..60].try_into().ok()?);
    let checksum = ContentDigest::from_bytes(bytes[60..92].try_into().ok()?);
    if format != FORMAT_SCHEMA
        || stage != key.stage
        || schema != key.schema
        || digest != key.digest
        || payload_len > usize::MAX as u64
        || HEADER_LEN.checked_add(payload_len as usize)? != bytes.len()
    {
        return None;
    }
    let payload = &bytes[HEADER_LEN..];
    (ContentDigest::sha256(payload) == checksum).then_some(payload)
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
            ArtifactStage::GlobalDetectionIndex,
        ];
        let keys = stages.map(|stage| ArtifactKey::derive(stage, 1, &[b"same content"]));
        for (index, key) in keys.iter().enumerate() {
            assert!(keys[..index].iter().all(|prior| prior.digest != key.digest));
        }
    }

    #[test]
    fn corruption_and_truncation_are_cache_misses() {
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
        let _ = std::fs::remove_dir_all(&root);
    }
}
