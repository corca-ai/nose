use super::{CacheRun, CachedSourceFile};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const STATE_SCHEMA: u32 = 3;
const STATE_SLOT: &str = "line-index";
const MANIFEST_SLOT: &str = "line-manifest";
#[cfg(test)]
static TEST_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Default, Deserialize, Serialize)]
struct LineIndexState {
    schema: u32,
    lines: Vec<String>,
    document_frequency: Vec<u32>,
    files: BTreeMap<String, StoredFileLines>,
}

#[derive(Deserialize, Serialize)]
struct StoredFileLines {
    digest: [u8; 32],
    unique_substantive: Vec<u32>,
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
    run: &CacheRun,
    source_files: &[CachedSourceFile],
    force_full: bool,
) -> (CachedLineIndex, LineIndexStats) {
    let manifest = current_manifest(source_files);
    if !force_full {
        if let Some(reused) = reuse_unchanged_index(run, &manifest) {
            return reused;
        }
    }
    let loaded = load_state(run);
    let state_hit = loaded.is_some();
    let previous = loaded.unwrap_or_default();
    let mut lines = previous.lines;
    let mut registry = lines
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, line)| (line, index as u32))
        .collect::<FxHashMap<_, _>>();
    let mut document_frequency = previous.document_frequency;
    let mut previous_files = previous.files;
    let mut changed_ids = FxHashSet::default();
    let mut stats = LineIndexStats {
        schema: "nose.line-index/v1",
        ..LineIndexStats::default()
    };
    let mut files = BTreeMap::new();
    for source in source_files {
        if let Some(previous_file) = previous_files.remove(&source.path) {
            if previous_file.digest == source.digest {
                stats.files_reused += 1;
                files.insert(source.path.clone(), previous_file);
                continue;
            }
            apply_frequency_delta(
                &mut document_frequency,
                &previous_file.unique_substantive,
                false,
                &mut changed_ids,
            );
        }
        let Some(text) = std::fs::read_to_string(&source.path).ok() else {
            continue;
        };
        let unique_substantive = intern_lines(
            unique_substantive_lines(&text),
            &mut registry,
            &mut lines,
            &mut document_frequency,
        );
        apply_frequency_delta(
            &mut document_frequency,
            &unique_substantive,
            true,
            &mut changed_ids,
        );
        stats.files_rebuilt += 1;
        files.insert(
            source.path.clone(),
            StoredFileLines {
                digest: source.digest,
                unique_substantive,
            },
        );
    }
    for (_, removed) in previous_files {
        apply_frequency_delta(
            &mut document_frequency,
            &removed.unique_substantive,
            false,
            &mut changed_ids,
        );
        stats.files_removed += 1;
    }
    // Keep existing ids stable for ordinary edits. Compacting on every removed
    // line remapped every id in every file, turning a one-line leaf mutation into
    // an O(corpus) rewrite. Tombstones are harmless because zero-frequency rows
    // are omitted from the runtime IDF map; compact only when they become a
    // meaningful fraction of persistent state.
    let tombstones = document_frequency
        .iter()
        .filter(|frequency| **frequency == 0)
        .count();
    let should_compact =
        tombstones > 4096 && tombstones.saturating_mul(10) > document_frequency.len();
    let (lines, document_frequency, files, changed_lines) = if should_compact {
        compact_lines(registry, document_frequency, files, &changed_ids)
    } else {
        let changed_lines = changed_ids
            .iter()
            .filter_map(|id| lines.get(*id as usize).cloned())
            .collect();
        (lines, document_frequency, files, changed_lines)
    };
    stats.changed_document_frequencies = changed_lines.len();
    let state = LineIndexState {
        schema: STATE_SCHEMA,
        lines,
        document_frequency,
        files,
    };
    finish_index(run, &manifest, state, state_hit, stats, changed_lines)
}

fn finish_index(
    run: &CacheRun,
    manifest: &LineIndexManifest,
    state: LineIndexState,
    state_hit: bool,
    stats: LineIndexStats,
    changed_lines: FxHashSet<String>,
) -> (CachedLineIndex, LineIndexStats) {
    if !state_hit || stats.files_rebuilt > 0 || stats.files_removed > 0 {
        store_state(run, &state);
    }
    store_manifest(run, manifest);
    let LineIndexState {
        lines,
        document_frequency,
        files,
        ..
    } = state;
    let file_count = files.len();
    let index = CachedLineIndex {
        document_frequency: lines
            .into_iter()
            .zip(document_frequency)
            .filter(|(_, frequency)| *frequency > 0)
            .collect(),
        // Source slices are needed only for families that cannot reuse their
        // analysis. FileLineCache reads those few files lazily; duplicating every
        // source line in both persistent state and the query heap dominated leaf RSS.
        files: FxHashMap::default(),
        changed_lines,
        file_count,
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
    run: &CacheRun,
    current: &LineIndexManifest,
) -> Option<(CachedLineIndex, LineIndexStats)> {
    (load_manifest(run).as_ref() == Some(current)).then(|| {
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

fn unique_substantive_lines(text: &str) -> Vec<String> {
    let mut unique = text
        .lines()
        .map(str::trim)
        .filter(|line| !crate::source_lines::is_trivial_line(line))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unique.shrink_to_fit();
    unique
}

fn intern_lines(
    lines: Vec<String>,
    registry: &mut FxHashMap<String, u32>,
    registered_lines: &mut Vec<String>,
    frequencies: &mut Vec<u32>,
) -> Vec<u32> {
    lines
        .into_iter()
        .map(|line| match registry.entry(line) {
            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let id = frequencies.len() as u32;
                frequencies.push(0);
                registered_lines.push(entry.key().clone());
                entry.insert(id);
                id
            }
        })
        .collect()
}

fn apply_frequency_delta(
    frequencies: &mut [u32],
    lines: &[u32],
    add: bool,
    changed: &mut FxHashSet<u32>,
) {
    for &line in lines {
        let Some(value) = frequencies.get_mut(line as usize) else {
            continue;
        };
        if add {
            *value += 1;
        } else {
            *value = value.saturating_sub(1);
        }
        changed.insert(line);
    }
}

type CompactedLines = (
    Vec<String>,
    Vec<u32>,
    BTreeMap<String, StoredFileLines>,
    FxHashSet<String>,
);

fn compact_lines(
    registry: FxHashMap<String, u32>,
    frequencies: Vec<u32>,
    mut files: BTreeMap<String, StoredFileLines>,
    changed_ids: &FxHashSet<u32>,
) -> CompactedLines {
    let mut by_id = (0..frequencies.len()).map(|_| None).collect::<Vec<_>>();
    for (line, id) in registry {
        if let Some(slot) = by_id.get_mut(id as usize) {
            *slot = Some(line);
        }
    }
    let changed_lines = changed_ids
        .iter()
        .filter_map(|id| by_id.get(*id as usize)?.as_ref().cloned())
        .collect();
    let mut remap = vec![u32::MAX; frequencies.len()];
    let mut lines = Vec::new();
    let mut compact_frequencies = Vec::new();
    for (old_id, (line, frequency)) in by_id.into_iter().zip(frequencies).enumerate() {
        if frequency == 0 {
            continue;
        }
        remap[old_id] = lines.len() as u32;
        lines.push(line.expect("every line id is registered"));
        compact_frequencies.push(frequency);
    }
    for file in files.values_mut() {
        for id in &mut file.unique_substantive {
            *id = remap[*id as usize];
        }
    }
    (lines, compact_frequencies, files, changed_lines)
}

fn load_state(run: &CacheRun) -> Option<LineIndexState> {
    run.load(STATE_SLOT, STATE_SCHEMA)
        .and_then(|bytes| rmp_serde::from_slice::<LineIndexState>(&bytes).ok())
        .filter(|state| {
            state.schema == STATE_SCHEMA
                && state.lines.len() == state.document_frequency.len()
                && state.files.values().all(|file| {
                    file.unique_substantive
                        .iter()
                        .all(|id| (*id as usize) < state.lines.len())
                })
        })
}

fn load_manifest(run: &CacheRun) -> Option<LineIndexManifest> {
    run.load(MANIFEST_SLOT, STATE_SCHEMA)
        .and_then(|bytes| rmp_serde::from_slice(&bytes).ok())
        .filter(|manifest: &LineIndexManifest| manifest.schema == STATE_SCHEMA)
}

fn store_manifest(run: &CacheRun, manifest: &LineIndexManifest) {
    store_bytes(run, MANIFEST_SLOT, rmp_serde::to_vec(manifest));
}

fn store_state(run: &CacheRun, state: &LineIndexState) {
    store_bytes(run, STATE_SLOT, rmp_serde::to_vec(state));
}

fn store_bytes(run: &CacheRun, slot: &str, bytes: Result<Vec<u8>, rmp_serde::encode::Error>) {
    let Ok(bytes) = bytes else { return };
    run.store(slot, STATE_SCHEMA, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_file_updates_compact_dictionary_without_stale_lines() {
        let root = std::env::temp_dir().join(format!(
            "nose_line_index_{}_{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let source = root.join("source");
        let cache = root.join("cache");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&source).unwrap();
        let a = source.join("a.py");
        let b = source.join("b.py");
        std::fs::write(&a, "shared\nold\n").unwrap();
        std::fs::write(&b, "shared\nother\n").unwrap();
        let run = CacheRun::new(&cache);
        run.set_workspace([7; 32]);
        let mut sources = vec![
            CachedSourceFile {
                path: a.to_string_lossy().into_owned(),
                logical_path: "0:a.py".to_owned(),
                digest: [1; 32],
                lang: nose_il::Lang::Python,
                source_kind: crate::cache::source::SourceIdentityKind::ContentSha256,
            },
            CachedSourceFile {
                path: b.to_string_lossy().into_owned(),
                logical_path: "0:b.py".to_owned(),
                digest: [2; 32],
                lang: nose_il::Lang::Python,
                source_kind: crate::cache::source::SourceIdentityKind::ContentSha256,
            },
        ];
        let (first, first_stats) = build_line_index(&run, &sources, true);
        assert_eq!(first_stats.files_rebuilt, 2);
        assert_eq!(first.document_frequency["shared"], 2);

        std::fs::write(&a, "shared\nnew\n").unwrap();
        sources[0].digest = [3; 32];
        let (second, second_stats) = build_line_index(&run, &sources, false);
        assert_eq!(second_stats.files_reused, 1);
        assert_eq!(second_stats.files_rebuilt, 1);
        assert_eq!(second.document_frequency["shared"], 2);
        assert_eq!(second.document_frequency["new"], 1);
        assert!(!second.document_frequency.contains_key("old"));
        assert!(second.changed_lines.contains("old"));
        assert!(second.changed_lines.contains("new"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
