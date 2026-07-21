//! Bounded cache administration and garbage collection.

use super::transaction::{live_state_paths, CacheRun};
use fs2::FileExt;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub(crate) const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;

const LEGACY_NAMES: &[&str] = &[
    "cas-v1",
    "state-v1",
    "detection-state-v1",
    "line-index-state-v1",
    "line-index-manifest-v1",
    "family-line-state-v1",
];
const ACTIVE_NAMES: &[&str] = &["cas-v2", "state-v2"];

#[derive(Debug, Serialize)]
pub(crate) struct CacheStatus {
    pub(crate) schema: &'static str,
    pub(crate) root: String,
    pub(crate) max_bytes: u64,
    pub(crate) bytes: u64,
    pub(crate) files: usize,
    pub(crate) generations: usize,
    pub(crate) active_generations: usize,
    pub(crate) reclaimable_bytes: u64,
    pub(crate) reclaimable_files: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct PruneReport {
    pub(crate) schema: &'static str,
    pub(crate) before_bytes: u64,
    pub(crate) after_bytes: u64,
    pub(crate) max_bytes: u64,
    pub(crate) removed_bytes: u64,
    pub(crate) removed_files: usize,
}

#[derive(Clone)]
struct FileInfo {
    path: PathBuf,
    bytes: u64,
    modified: u64,
}

pub(crate) fn status(root: &Path, max_bytes: u64) -> CacheStatus {
    let active = collect_named(root, ACTIVE_NAMES);
    let legacy = collect_named(root, LEGACY_NAMES);
    let live = live_state_paths(root);
    let orphaned = active
        .iter()
        .filter(|file| is_generation_file(&file.path) && !live.contains(&file.path))
        .collect::<Vec<_>>();
    CacheStatus {
        schema: "nose.cache-status/v1",
        root: root.display().to_string(),
        max_bytes,
        bytes: bytes(&active) + bytes(&legacy),
        files: active.len() + legacy.len(),
        generations: active
            .iter()
            .filter(|file| extension(&file.path) == Some("manifest"))
            .count(),
        active_generations: live
            .iter()
            .filter(|path| extension(path) == Some("manifest"))
            .count(),
        reclaimable_bytes: bytes(&legacy) + orphaned.iter().map(|file| file.bytes).sum::<u64>(),
        reclaimable_files: legacy.len() + orphaned.len(),
    }
}

pub(crate) fn prune(root: &Path, max_bytes: u64) -> std::io::Result<PruneReport> {
    let _lease = exclusive_store_lease(root)?;
    prune_locked(root, max_bytes)
}

fn prune_locked(root: &Path, max_bytes: u64) -> std::io::Result<PruneReport> {
    let before = status(root, max_bytes);
    let mut removed_files = 0;
    let mut removed_bytes = 0;
    for name in LEGACY_NAMES {
        let path = root.join(name);
        let files = collect_tree(&path);
        if remove_path(&path).is_ok() {
            removed_files += files.len();
            removed_bytes += bytes(&files);
        }
    }

    let live = live_state_paths(root);
    let active = collect_named(root, ACTIVE_NAMES);
    for file in active.iter().filter(|file| {
        is_temporary(&file.path) || (is_generation_file(&file.path) && !live.contains(&file.path))
    }) {
        if std::fs::remove_file(&file.path).is_ok() {
            removed_files += 1;
            removed_bytes += file.bytes;
        }
    }

    let mut remaining = collect_named(root, ACTIVE_NAMES);
    let mut total = bytes(&remaining);
    if total > max_bytes {
        remaining.sort_by_key(|file| (eviction_tier(&file.path, &live), file.modified));
        for file in remaining {
            if total <= max_bytes || eviction_tier(&file.path, &live) == u8::MAX {
                continue;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                total = total.saturating_sub(file.bytes);
                removed_files += 1;
                removed_bytes += file.bytes;
            }
        }
    }
    remove_empty_directories(&root.join("cas-v2"));
    remove_empty_directories(&root.join("state-v2"));
    let after = status(root, max_bytes).bytes;
    Ok(PruneReport {
        schema: "nose.cache-prune/v1",
        before_bytes: before.bytes,
        after_bytes: after,
        max_bytes,
        removed_bytes,
        removed_files,
    })
}

pub(crate) fn clear(root: &Path) -> std::io::Result<PruneReport> {
    let _lease = exclusive_store_lease(root)?;
    let before = status(root, DEFAULT_MAX_BYTES);
    for name in LEGACY_NAMES.iter().chain(ACTIVE_NAMES) {
        remove_path(&root.join(name))?;
    }
    Ok(PruneReport {
        schema: "nose.cache-clear/v1",
        before_bytes: before.bytes,
        after_bytes: 0,
        max_bytes: 0,
        removed_bytes: before.bytes,
        removed_files: before.files,
    })
}

pub(super) fn enforce_run_budget(run: CacheRun) {
    let written_bytes = run.written_bytes();
    let root = run.root().to_path_buf();
    let max_bytes = run.max_bytes();
    let started_empty = run.started_empty();
    // Releasing the run's shared lease before acquiring the exclusive prune
    // lease avoids self-deadlock and lets all concurrent writers finish first.
    drop(run);
    if written_bytes == 0 {
        return;
    }
    // A newly created managed store has no hidden prior bytes to account for.
    // Avoid immediately walking every object we just wrote when the run's own
    // exact byte counter proves it is still below budget.
    if started_empty && written_bytes <= max_bytes {
        return;
    }
    if let Err(error) = prune(&root, max_bytes) {
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [cache-prune] skipped: {error}");
        }
    }
}

fn collect_named(root: &Path, names: &[&str]) -> Vec<FileInfo> {
    names
        .iter()
        .flat_map(|name| collect_tree(&root.join(name)))
        .collect()
}

fn collect_tree(root: &Path) -> Vec<FileInfo> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            files.push(FileInfo {
                path,
                bytes: metadata.len(),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_secs()),
            });
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    files
}

fn eviction_tier(path: &Path, live: &BTreeSet<PathBuf>) -> u8 {
    if is_temporary(path) || (is_generation_file(path) && !live.contains(path)) {
        0
    } else if extension(path) == Some("artifact") {
        1
    } else if extension(path) == Some("state") {
        2
    } else if extension(path) == Some("manifest") {
        3
    } else if path.file_name().and_then(|name| name.to_str()) == Some("CURRENT") {
        4
    } else if path.file_name().and_then(|name| name.to_str()) == Some("LOCK") {
        u8::MAX
    } else {
        5
    }
}

fn exclusive_store_lease(root: &Path) -> std::io::Result<File> {
    std::fs::create_dir_all(root)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".nose-cache.lock"))?;
    FileExt::lock_exclusive(&file)?;
    Ok(file)
}

fn is_state_object(path: &Path) -> bool {
    matches!(extension(path), Some("state" | "manifest"))
}

fn is_generation_file(path: &Path) -> bool {
    is_state_object(path) || path.file_name().and_then(|name| name.to_str()) == Some("CURRENT")
}

fn is_temporary(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.') && name.ends_with(".tmp"))
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|extension| extension.to_str())
}

fn bytes(files: &[FileInfo]) -> u64 {
    files.iter().map(|file| file.bytes).sum()
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            std::fs::remove_dir_all(path)
        }
        Ok(_) => std::fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_empty_directories(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    let children = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    for child in &children {
        if child.is_dir() {
            remove_empty_directories(child);
        }
    }
    std::fs::read_dir(root)
        .ok()
        .is_some_and(|mut entries| entries.next().is_none())
        && std::fs::remove_dir(root).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_store(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nose_admin_{label}_{}_{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn prune_removes_legacy_orphans_and_oldest_cas_without_touching_other_files() {
        let root = temp_store("prune");
        std::fs::create_dir_all(root.join("cas-v1")).unwrap();
        std::fs::write(root.join("cas-v1/legacy"), vec![1; 50]).unwrap();
        std::fs::create_dir_all(root.join("cas-v2/raw-il/aa")).unwrap();
        std::fs::write(root.join("cas-v2/raw-il/aa/old.artifact"), vec![2; 80]).unwrap();
        std::fs::write(root.join("keep.txt"), vec![3; 90]).unwrap();

        let report = prune(&root, 0).unwrap();
        assert!(report.removed_bytes >= 130);
        assert!(root.join("keep.txt").is_file());
        assert!(!root.join("cas-v1").exists());
        assert!(!root.join("cas-v2/raw-il/aa/old.artifact").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn status_counts_only_managed_cache_paths() {
        let root = temp_store("status");
        std::fs::create_dir_all(root.join("cas-v2/raw-il/aa")).unwrap();
        std::fs::write(root.join("cas-v2/raw-il/aa/entry.artifact"), vec![0; 11]).unwrap();
        std::fs::write(root.join("unrelated"), vec![0; 99]).unwrap();
        let report = status(&root, DEFAULT_MAX_BYTES);
        assert_eq!(report.bytes, 11);
        assert_eq!(report.files, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prune_reclaims_superseded_generations_and_keeps_current_state() {
        let root = temp_store("generations");
        let first = CacheRun::new(&root);
        first.set_workspace([9; 32]);
        first.store("line-index", 1, b"old");
        first.commit().unwrap();
        drop(first);

        let second = CacheRun::new(&root);
        second.set_workspace([9; 32]);
        second.store("line-index", 1, b"new");
        second.commit().unwrap();
        drop(second);

        let before = status(&root, DEFAULT_MAX_BYTES);
        assert_eq!(before.generations, 2);
        assert_eq!(before.active_generations, 1);
        assert!(before.reclaimable_files >= 2);
        prune(&root, DEFAULT_MAX_BYTES).unwrap();

        let reader = CacheRun::new(&root);
        reader.set_workspace([9; 32]);
        assert_eq!(
            reader.load("line-index", 1).as_deref(),
            Some(b"new".as_slice())
        );
        assert_eq!(status(&root, DEFAULT_MAX_BYTES).generations, 1);
        drop(reader);
        let _ = std::fs::remove_dir_all(root);
    }
}
