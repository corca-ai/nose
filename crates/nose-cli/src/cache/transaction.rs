//! Transactional mutable cache generations.
//!
//! Immutable records are written first. A generation manifest names a complete
//! set of records, and `CURRENT` is atomically replaced last. Readers therefore
//! see either the previous complete generation or the new complete generation.

use super::digest::ContentDigest;
use super::store::{
    encode_envelope, publish, read_envelope_file, temporary_path, write_complete, ArtifactKey,
    ArtifactStage, LayeredCas,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const GENERATION_SCHEMA: u32 = 1;
const POINTER_SCHEMA: u32 = 1;
const MAX_POINTER_BYTES: u64 = 64 * 1024;

#[derive(Clone)]
pub(crate) struct CacheRun {
    root: PathBuf,
    max_bytes: u64,
    written_bytes: Arc<AtomicU64>,
    write_portable_il: Arc<AtomicBool>,
    read_existing_cas: bool,
    managed_store_existed: bool,
    inner: Arc<Mutex<RunState>>,
    // Keep pruning and clearing out of the interval between publishing an
    // immutable record and committing the generation that references it.
    _store_lease: Option<Arc<File>>,
}

#[derive(Default)]
struct RunState {
    workspace: Option<[u8; 32]>,
    base: GenerationManifest,
    pending: BTreeMap<String, StateRecordRef>,
}

#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
struct GenerationManifest {
    schema: u32,
    workspace: [u8; 32],
    sequence: u64,
    records: BTreeMap<String, StateRecordRef>,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct StateRecordRef {
    schema: u32,
    digest: [u8; 32],
}

#[derive(Deserialize, Serialize)]
struct CurrentPointer {
    schema: u32,
    generation: String,
    manifest_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitFault {
    BeforeManifest,
    AfterManifest,
    BeforeCurrent,
}

impl CacheRun {
    #[cfg(test)]
    pub(crate) fn new(root: &Path) -> Self {
        Self::with_limit(root, super::admin::DEFAULT_MAX_BYTES)
    }

    pub(crate) fn with_limit(root: &Path, max_bytes: u64) -> Self {
        Self::with_policy(root, max_bytes, true)
    }

    fn with_policy(root: &Path, max_bytes: u64, write_portable_il: bool) -> Self {
        let read_existing_cas = root.join("cas-v2").is_dir();
        let managed_store_existed = [
            "cas-v2",
            "state-v2",
            "cas-v1",
            "state-v1",
            "detection-state-v1",
            "line-index-state-v1",
            "line-index-manifest-v1",
            "family-line-state-v1",
        ]
        .iter()
        .any(|name| root.join(name).exists());
        Self {
            root: root.to_path_buf(),
            max_bytes,
            written_bytes: Arc::new(AtomicU64::new(0)),
            write_portable_il: Arc::new(AtomicBool::new(write_portable_il)),
            read_existing_cas,
            managed_store_existed,
            inner: Arc::new(Mutex::new(RunState::default())),
            _store_lease: shared_store_lease(root).map(Arc::new),
        }
    }

    pub(super) fn cas(&self) -> LayeredCas {
        LayeredCas::with_write_counter(
            &self.root,
            Arc::clone(&self.written_bytes),
            self.write_portable_il.load(Ordering::Relaxed),
            self.read_existing_cas,
        )
    }

    pub(crate) fn set_workspace(&self, workspace: [u8; 32]) {
        let mut inner = self.inner.lock().expect("cache run mutex poisoned");
        if let Some(existing) = inner.workspace {
            debug_assert_eq!(existing, workspace);
            return;
        }
        inner.workspace = Some(workspace);
        inner.base = load_current(&self.root, workspace).unwrap_or_else(|| GenerationManifest {
            schema: GENERATION_SCHEMA,
            workspace,
            ..GenerationManifest::default()
        });
    }

    pub(crate) fn load(&self, slot: &str, schema: u32) -> Option<Vec<u8>> {
        let (workspace, record) = {
            let inner = self.inner.lock().expect("cache run mutex poisoned");
            let workspace = inner.workspace?;
            let record = inner
                .pending
                .get(slot)
                .or_else(|| inner.base.records.get(slot))?
                .clone();
            (workspace, record)
        };
        if record.schema != schema {
            return None;
        }
        let key = record_key(record.schema, record.digest);
        let path = record_path(&self.root, workspace, record.digest);
        read_envelope_file(&path, key).map(|(payload, _)| payload)
    }

    pub(crate) fn store(&self, slot: &str, schema: u32, payload: &[u8]) {
        let workspace = {
            let inner = self.inner.lock().expect("cache run mutex poisoned");
            let Some(workspace) = inner.workspace else {
                return;
            };
            workspace
        };
        let key = ArtifactKey::derive(
            ArtifactStage::StateRecord,
            schema,
            &[&workspace, slot.as_bytes(), payload],
        );
        let path = record_path(&self.root, workspace, *key.digest.as_bytes());
        if read_envelope_file(&path, key).is_none() {
            let Some(parent) = path.parent() else { return };
            if std::fs::create_dir_all(parent).is_err() {
                return;
            }
            let bytes = encode_envelope(key, payload);
            let temp = temporary_path(parent, &key.digest.hex());
            if write_complete(&temp, &bytes).is_err() {
                let _ = std::fs::remove_file(&temp);
                return;
            }
            if publish(&temp, &path).is_err() {
                let _ = std::fs::remove_file(&temp);
                return;
            }
            self.written_bytes
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        }
        let mut inner = self.inner.lock().expect("cache run mutex poisoned");
        let record = StateRecordRef {
            schema,
            digest: *key.digest.as_bytes(),
        };
        if inner
            .pending
            .get(slot)
            .or_else(|| inner.base.records.get(slot))
            == Some(&record)
        {
            return;
        }
        inner.pending.insert(slot.to_owned(), record);
    }

    pub(crate) fn commit(&self) -> std::io::Result<()> {
        self.commit_inner(None)
    }

    pub(crate) fn written_bytes(&self) -> u64 {
        self.written_bytes.load(Ordering::Relaxed)
    }

    pub(super) fn writes_portable_il(&self) -> bool {
        self.write_portable_il.load(Ordering::Relaxed)
    }

    pub(super) fn set_portable_il_enabled(&self, enabled: bool) {
        self.write_portable_il.store(enabled, Ordering::Relaxed);
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    pub(super) fn started_empty(&self) -> bool {
        !self.managed_store_existed
    }

    fn commit_inner(&self, fault: Option<CommitFault>) -> std::io::Result<()> {
        let (workspace, pending) = {
            let inner = self.inner.lock().expect("cache run mutex poisoned");
            let Some(workspace) = inner.workspace else {
                return Ok(());
            };
            if inner.pending.is_empty() {
                return Ok(());
            }
            (workspace, inner.pending.clone())
        };
        let workspace_dir = workspace_dir(&self.root, workspace);
        std::fs::create_dir_all(&workspace_dir)?;
        let lock = open_lock(&workspace_dir)?;
        FileExt::lock_exclusive(&lock)?;

        let mut next = load_current(&self.root, workspace).unwrap_or_else(|| GenerationManifest {
            schema: GENERATION_SCHEMA,
            workspace,
            ..GenerationManifest::default()
        });
        next.sequence = next.sequence.saturating_add(1);
        next.records.extend(pending);
        inject(fault, CommitFault::BeforeManifest)?;
        let (generation, manifest_digest) = write_manifest(&self.root, &next)?;
        inject(fault, CommitFault::AfterManifest)?;
        inject(fault, CommitFault::BeforeCurrent)?;
        write_current(&self.root, workspace, &generation, manifest_digest)?;

        let mut inner = self.inner.lock().expect("cache run mutex poisoned");
        inner.base = next;
        inner.pending.clear();
        Ok(())
    }
}

fn shared_store_lease(root: &Path) -> Option<File> {
    std::fs::create_dir_all(root).ok()?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".nose-cache.lock"))
        .ok()?;
    FileExt::lock_shared(&file).ok()?;
    Some(file)
}

fn inject(actual: Option<CommitFault>, expected: CommitFault) -> std::io::Result<()> {
    if actual == Some(expected) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            format!("injected cache commit interruption at {expected:?}"),
        ));
    }
    Ok(())
}

fn open_lock(workspace_dir: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(workspace_dir.join("LOCK"))
}

fn load_current(root: &Path, workspace: [u8; 32]) -> Option<GenerationManifest> {
    load_current_named(root, workspace).map(|(manifest, _)| manifest)
}

fn load_current_named(root: &Path, workspace: [u8; 32]) -> Option<(GenerationManifest, String)> {
    let pointer_path = workspace_dir(root, workspace).join("CURRENT");
    if std::fs::metadata(&pointer_path).ok()?.len() > MAX_POINTER_BYTES {
        eprintln!(
            "warning: ignoring oversized cache generation pointer {}; recomputing",
            pointer_path.display()
        );
        return None;
    }
    let pointer_key = pointer_key(workspace);
    let (payload, _) = read_envelope_file(&pointer_path, pointer_key)?;
    let pointer = rmp_serde::from_slice::<CurrentPointer>(&payload).ok()?;
    if pointer.schema != POINTER_SCHEMA {
        return None;
    }
    let key = record_key(GENERATION_SCHEMA, pointer.manifest_digest);
    let path = workspace_dir(root, workspace)
        .join("generations")
        .join(&pointer.generation);
    let (payload, _) = read_envelope_file(&path, key)?;
    let manifest = rmp_serde::from_slice::<GenerationManifest>(&payload).ok()?;
    (manifest.schema == GENERATION_SCHEMA && manifest.workspace == workspace)
        .then_some((manifest, pointer.generation))
}

pub(super) fn live_state_paths(root: &Path) -> std::collections::BTreeSet<PathBuf> {
    let mut live = std::collections::BTreeSet::new();
    let state_root = root.join("state-v2");
    let Ok(workspaces) = std::fs::read_dir(&state_root) else {
        return live;
    };
    for workspace in workspaces.flatten() {
        let Ok(workspace_bytes) = parse_hex_32(&workspace.file_name().to_string_lossy()) else {
            continue;
        };
        let Some((manifest, generation)) = load_current_named(root, workspace_bytes) else {
            continue;
        };
        let dir = workspace.path();
        live.insert(dir.join("CURRENT"));
        live.insert(dir.join("generations").join(generation));
        for record in manifest.records.values() {
            live.insert(record_path(root, workspace_bytes, record.digest));
        }
    }
    live
}

fn write_manifest(
    root: &Path,
    manifest: &GenerationManifest,
) -> std::io::Result<(String, [u8; 32])> {
    let payload = rmp_serde::to_vec(manifest).map_err(invalid_data)?;
    let key = ArtifactKey::derive(
        ArtifactStage::StateRecord,
        GENERATION_SCHEMA,
        &[&manifest.workspace, b"generation", &payload],
    );
    let generation = format!("{:020}-{}.manifest", manifest.sequence, key.digest.hex());
    let dir = workspace_dir(root, manifest.workspace).join("generations");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(&generation);
    if read_envelope_file(&target, key).is_none() {
        let bytes = encode_envelope(key, &payload);
        let temp = temporary_path(&dir, &key.digest.hex());
        write_complete(&temp, &bytes)?;
        publish(&temp, &target)?;
    }
    Ok((generation, *key.digest.as_bytes()))
}

fn write_current(
    root: &Path,
    workspace: [u8; 32],
    generation: &str,
    manifest_digest: [u8; 32],
) -> std::io::Result<()> {
    let pointer = CurrentPointer {
        schema: POINTER_SCHEMA,
        generation: generation.to_owned(),
        manifest_digest,
    };
    let payload = rmp_serde::to_vec(&pointer).map_err(invalid_data)?;
    let key = pointer_key(workspace);
    let bytes = encode_envelope(key, &payload);
    let dir = workspace_dir(root, workspace);
    let target = dir.join("CURRENT");
    let temp = temporary_path(&dir, "CURRENT");
    write_complete(&temp, &bytes)?;
    publish(&temp, &target)
}

fn pointer_key(workspace: [u8; 32]) -> ArtifactKey {
    ArtifactKey::derive(
        ArtifactStage::StateRecord,
        POINTER_SCHEMA,
        &[&workspace, b"CURRENT"],
    )
}

fn record_key(schema: u32, digest: [u8; 32]) -> ArtifactKey {
    ArtifactKey {
        stage: ArtifactStage::StateRecord,
        schema,
        digest: ContentDigest::from_bytes(digest),
    }
}

fn workspace_dir(root: &Path, workspace: [u8; 32]) -> PathBuf {
    root.join("state-v2").join(hex(&workspace))
}

fn record_path(root: &Path, workspace: [u8; 32], digest: [u8; 32]) -> PathBuf {
    let hex = hex(&digest);
    workspace_dir(root, workspace)
        .join("objects")
        .join(&hex[..2])
        .join(format!("{}.state", &hex[2..]))
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn parse_hex_32(text: &str) -> Result<[u8; 32], ()> {
    if text.len() != 64 {
        return Err(());
    }
    let mut out = [0; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| ())?;
        out[index] = u8::from_str_radix(pair, 16).map_err(|_| ())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_store(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nose_generation_{label}_{}_{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn interruption_at_every_commit_stage_keeps_the_previous_generation_visible() {
        for fault in [
            CommitFault::BeforeManifest,
            CommitFault::AfterManifest,
            CommitFault::BeforeCurrent,
        ] {
            let root = temp_store("fault");
            let _ = std::fs::remove_dir_all(&root);
            let first = CacheRun::new(&root);
            first.set_workspace([3; 32]);
            first.store("line-index", 1, b"old");
            first.commit().unwrap();

            let interrupted = CacheRun::new(&root);
            interrupted.set_workspace([3; 32]);
            interrupted.store("line-index", 1, b"new");
            assert!(interrupted.commit_inner(Some(fault)).is_err());

            let reader = CacheRun::new(&root);
            reader.set_workspace([3; 32]);
            assert_eq!(
                reader.load("line-index", 1).as_deref(),
                Some(b"old".as_slice())
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn concurrent_commits_merge_disjoint_slots() {
        let root = temp_store("merge");
        let _ = std::fs::remove_dir_all(&root);
        let first = CacheRun::new(&root);
        let second = CacheRun::new(&root);
        first.set_workspace([4; 32]);
        second.set_workspace([4; 32]);
        first.store("a", 1, b"alpha");
        second.store("b", 1, b"beta");
        first.commit().unwrap();
        second.commit().unwrap();

        let reader = CacheRun::new(&root);
        reader.set_workspace([4; 32]);
        assert_eq!(reader.load("a", 1).as_deref(), Some(b"alpha".as_slice()));
        assert_eq!(reader.load("b", 1).as_deref(), Some(b"beta".as_slice()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn state_schema_mismatch_is_a_cache_miss() {
        let root = temp_store("schema");
        let writer = CacheRun::new(&root);
        writer.set_workspace([5; 32]);
        writer.store("family-lines", 1, b"payload");
        writer.commit().unwrap();
        assert!(writer.load("family-lines", 2).is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
