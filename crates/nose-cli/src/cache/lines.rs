use super::CachedSourceFile;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STATE_SCHEMA: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Default, Deserialize, Serialize)]
struct LineIndexState {
    schema: u32,
    files: BTreeMap<String, StoredFileLines>,
    document_frequency: BTreeMap<String, u32>,
}

#[derive(Deserialize, Serialize)]
struct StoredFileLines {
    digest: [u8; 32],
    lines: Vec<String>,
    unique_substantive: Vec<String>,
}

#[derive(Eq, PartialEq, Deserialize, Serialize)]
struct LineIndexManifest {
    schema: u32,
    files: BTreeMap<String, [u8; 32]>,
}

pub(crate) struct CachedLineIndex {
    pub(crate) document_frequency: FxHashMap<String, u32>,
    pub(crate) files: FxHashMap<String, Option<Vec<String>>>,
    pub(crate) changed_lines: FxHashSet<String>,
    pub(crate) file_count: usize,
    pub(crate) complete: bool,
}

#[derive(Default, serde::Serialize)]
pub(crate) struct LineIndexStats {
    schema: &'static str,
    files_reused: usize,
    files_rebuilt: usize,
    files_removed: usize,
    changed_document_frequencies: usize,
}

pub(crate) fn build_line_index(
    cache_dir: &Path,
    workspace: [u8; 32],
    source_files: &[CachedSourceFile],
    force_full: bool,
) -> (CachedLineIndex, LineIndexStats) {
    let path = state_path(cache_dir, workspace);
    let manifest_path = manifest_path(cache_dir, workspace);
    let manifest = current_manifest(source_files);
    if !force_full {
        if let Some(reused) = reuse_unchanged_index(&manifest_path, &manifest) {
            return reused;
        }
    }
    let loaded = load_state(&path);
    let state_hit = loaded.is_some();
    let previous = loaded.unwrap_or_default();
    let mut document_frequency = previous.document_frequency.clone();
    let current_paths = source_files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut changed_lines = FxHashSet::default();
    let mut stats = LineIndexStats {
        schema: "nose.line-index/v1",
        ..LineIndexStats::default()
    };
    for (path, file) in &previous.files {
        if !current_paths.contains(path.as_str()) {
            apply_frequency_delta(
                &mut document_frequency,
                &file.unique_substantive,
                false,
                &mut changed_lines,
            );
            stats.files_removed += 1;
        }
    }

    let mut files = BTreeMap::new();
    for source in source_files {
        if let Some(previous_file) = previous
            .files
            .get(&source.path)
            .filter(|file| file.digest == source.digest)
        {
            stats.files_reused += 1;
            files.insert(
                source.path.clone(),
                StoredFileLines {
                    digest: previous_file.digest,
                    lines: previous_file.lines.clone(),
                    unique_substantive: previous_file.unique_substantive.clone(),
                },
            );
            continue;
        }
        if let Some(previous_file) = previous.files.get(&source.path) {
            apply_frequency_delta(
                &mut document_frequency,
                &previous_file.unique_substantive,
                false,
                &mut changed_lines,
            );
        }
        let Some(text) = std::fs::read_to_string(&source.path).ok() else {
            continue;
        };
        let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        let unique_substantive = unique_substantive_lines(&lines);
        apply_frequency_delta(
            &mut document_frequency,
            &unique_substantive,
            true,
            &mut changed_lines,
        );
        stats.files_rebuilt += 1;
        files.insert(
            source.path.clone(),
            StoredFileLines {
                digest: source.digest,
                lines,
                unique_substantive,
            },
        );
    }
    document_frequency.retain(|_, count| *count > 0);
    stats.changed_document_frequencies = changed_lines.len();
    let state = LineIndexState {
        schema: STATE_SCHEMA,
        files,
        document_frequency,
    };
    finish_index(
        &path,
        &manifest_path,
        &manifest,
        state,
        state_hit,
        stats,
        changed_lines,
    )
}

fn finish_index(
    path: &Path,
    manifest_path: &Path,
    manifest: &LineIndexManifest,
    state: LineIndexState,
    state_hit: bool,
    stats: LineIndexStats,
    changed_lines: FxHashSet<String>,
) -> (CachedLineIndex, LineIndexStats) {
    if !state_hit || stats.files_rebuilt > 0 || stats.files_removed > 0 {
        store_state(path, &state);
    }
    store_manifest(manifest_path, manifest);
    let index = CachedLineIndex {
        document_frequency: state
            .document_frequency
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        files: state
            .files
            .iter()
            .map(|(path, file)| (path.clone(), Some(file.lines.clone())))
            .collect(),
        changed_lines,
        file_count: state.files.len(),
        complete: true,
    };
    (index, stats)
}

fn current_manifest(source_files: &[CachedSourceFile]) -> LineIndexManifest {
    LineIndexManifest {
        schema: STATE_SCHEMA,
        files: source_files
            .iter()
            .map(|file| (file.path.clone(), file.digest))
            .collect(),
    }
}

fn reuse_unchanged_index(
    path: &Path,
    current: &LineIndexManifest,
) -> Option<(CachedLineIndex, LineIndexStats)> {
    (load_manifest(path).as_ref() == Some(current)).then(|| {
        (
            CachedLineIndex {
                document_frequency: FxHashMap::default(),
                files: FxHashMap::default(),
                changed_lines: FxHashSet::default(),
                file_count: current.files.len(),
                complete: false,
            },
            LineIndexStats {
                schema: "nose.line-index/v1",
                files_reused: current.files.len(),
                ..LineIndexStats::default()
            },
        )
    })
}

fn unique_substantive_lines(lines: &[String]) -> Vec<String> {
    let mut unique = lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !crate::source_lines::is_trivial_line(line))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unique.shrink_to_fit();
    unique
}

fn apply_frequency_delta(
    frequencies: &mut BTreeMap<String, u32>,
    lines: &[String],
    add: bool,
    changed: &mut FxHashSet<String>,
) {
    for line in lines {
        let value = frequencies.entry(line.clone()).or_default();
        if add {
            *value += 1;
        } else {
            *value = value.saturating_sub(1);
        }
        changed.insert(line.clone());
    }
}

fn state_path(cache_dir: &Path, workspace: [u8; 32]) -> PathBuf {
    cache_dir
        .join("line-index-state-v1")
        .join(format!("{}.msgpack", hex(&workspace)))
}

fn manifest_path(cache_dir: &Path, workspace: [u8; 32]) -> PathBuf {
    cache_dir
        .join("line-index-manifest-v1")
        .join(format!("{}.msgpack", hex(&workspace)))
}

fn load_state(path: &Path) -> Option<LineIndexState> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| rmp_serde::from_slice::<LineIndexState>(&bytes).ok())
        .filter(|state| state.schema == STATE_SCHEMA)
}

fn load_manifest(path: &Path) -> Option<LineIndexManifest> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| rmp_serde::from_slice(&bytes).ok())
        .filter(|manifest: &LineIndexManifest| manifest.schema == STATE_SCHEMA)
}

fn store_manifest(path: &Path, manifest: &LineIndexManifest) {
    store_bytes(path, rmp_serde::to_vec(manifest));
}

fn store_state(path: &Path, state: &LineIndexState) {
    store_bytes(path, rmp_serde::to_vec(state));
}

fn store_bytes(path: &Path, bytes: Result<Vec<u8>, rmp_serde::encode::Error>) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(bytes) = bytes else { return };
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&temp, bytes).is_ok() && std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}
