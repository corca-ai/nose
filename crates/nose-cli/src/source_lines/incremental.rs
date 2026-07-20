use super::{
    is_trivial_line, shared_lines_of, varying_spots_of, FileLineCache, LineIdf, SharedLines,
};
use crate::cache::CachedLineContext;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STATE_SCHEMA: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Default, Deserialize, Serialize)]
struct FamilyLineState {
    schema: u32,
    families: BTreeMap<[u8; 32], StoredFamilyLines>,
}

#[derive(Clone, Deserialize, Serialize)]
struct StoredFamilyLines {
    shared: Option<SharedLines>,
    varying_spots: Vec<nose_detect::VaryingSpot>,
    shared_weight: f64,
    file_count: usize,
}

#[derive(Default, Serialize)]
pub(crate) struct FamilyLineStats {
    schema: &'static str,
    families_reused: usize,
    families_reweighted: usize,
    families_rebuilt: usize,
}

pub(crate) fn apply_cached_family_lines(
    families: &mut [nose_detect::RefactorFamily],
    idf: &LineIdf,
    lines: &mut FileLineCache,
    context: &CachedLineContext,
    changed_lines: &FxHashSet<String>,
    file_count: usize,
) -> FamilyLineStats {
    let path = state_path(&context.cache_dir, context.workspace_digest);
    let previous = load_state(&path);
    let digests = source_digests(context);
    let mut current = BTreeMap::new();
    let mut stats = FamilyLineStats {
        schema: "nose.family-line-state/v1",
        ..FamilyLineStats::default()
    };
    for family in families
        .iter_mut()
        .filter(|family| family.languages == 1 && family.locations.len() >= 2)
    {
        let key = family_key(family, &digests);
        let analysis = if let Some(stored) = previous.families.get(&key) {
            let needs_reweight = stored.file_count != file_count
                || stored.shared.as_ref().is_some_and(|shared| {
                    shared
                        .rank_lines
                        .iter()
                        .any(|line| changed_lines.contains(line))
                });
            let mut stored = stored.clone();
            if needs_reweight {
                stored.shared_weight = stored
                    .shared
                    .as_ref()
                    .map_or(0.0, |shared| shared_weight(shared, idf));
                stored.file_count = file_count;
                stats.families_reweighted += 1;
            } else {
                stats.families_reused += 1;
            }
            stored
        } else {
            stats.families_rebuilt += 1;
            analyze_family(family, idf, lines, file_count)
        };
        apply_analysis(family, &analysis);
        current.insert(key, analysis);
    }
    store_state(
        &path,
        &FamilyLineState {
            schema: STATE_SCHEMA,
            families: current,
        },
    );
    stats
}

fn analyze_family(
    family: &nose_detect::RefactorFamily,
    idf: &LineIdf,
    lines: &mut FileLineCache,
    file_count: usize,
) -> StoredFamilyLines {
    let varying_spots = family.locations[1..]
        .iter()
        .find_map(|other| varying_spots_of(&family.locations[0], other, lines))
        .unwrap_or_default();
    let shared = shared_lines_of(&family.locations, lines);
    let shared_weight = shared
        .as_ref()
        .map_or(0.0, |shared| shared_weight(shared, idf));
    StoredFamilyLines {
        shared,
        varying_spots,
        shared_weight,
        file_count,
    }
}

fn apply_analysis(family: &mut nose_detect::RefactorFamily, analysis: &StoredFamilyLines) {
    family.varying_spots.clone_from(&analysis.varying_spots);
    family.shared_weight = analysis.shared_weight;
    if let Some(shared) = &analysis.shared {
        family.shared_lines = shared.display;
        family.params = shared.params;
        family.display_params = Some(shared.display_params);
    }
}

fn shared_weight(shared: &SharedLines, idf: &LineIdf) -> f64 {
    let substantive = shared
        .rank_lines
        .iter()
        .filter(|line| !is_trivial_line(line))
        .map(|line| idf.weight(line))
        .sum::<f64>();
    let gate = (substantive / 2.0).clamp(0.0, 1.0);
    shared.rank_lines.len() as f64 * gate
}

fn family_key(
    family: &nose_detect::RefactorFamily,
    source_digests: &FxHashMap<String, [u8; 32]>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"nose.family-line-analysis.v1\0");
    hasher.update((family.locations.len() as u64).to_be_bytes());
    for location in &family.locations {
        hash_component(&mut hasher, location.file.as_bytes());
        hasher.update(location.start_line.to_be_bytes());
        hasher.update(location.end_line.to_be_bytes());
        let digest = source_digest(&location.file, source_digests);
        hasher.update(digest);
    }
    hasher.finalize().into()
}

fn source_digests(context: &CachedLineContext) -> FxHashMap<String, [u8; 32]> {
    let mut digests = FxHashMap::default();
    let cwd = std::env::current_dir().ok();
    for source in &context.source_files {
        digests.insert(source.path.clone(), source.digest);
        if let Ok(canonical) = std::fs::canonicalize(&source.path) {
            digests.insert(canonical.to_string_lossy().into_owned(), source.digest);
        }
        if let Some(cwd) = &cwd {
            digests.insert(
                crate::path_utils::relativize(&source.path, cwd),
                source.digest,
            );
        }
    }
    digests
}

fn source_digest(path: &str, digests: &FxHashMap<String, [u8; 32]>) -> [u8; 32] {
    if let Some(digest) = digests.get(path) {
        return *digest;
    }
    if let Ok(canonical) = std::fs::canonicalize(path) {
        if let Some(digest) = digests.get(canonical.to_string_lossy().as_ref()) {
            return *digest;
        }
    }
    std::fs::read(path)
        .map(|bytes| Sha256::digest(bytes).into())
        .unwrap_or([0; 32])
}

fn hash_component(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn state_path(cache_dir: &Path, workspace: [u8; 32]) -> PathBuf {
    cache_dir
        .join("family-line-state-v1")
        .join(format!("{}.msgpack", hex(&workspace)))
}

fn load_state(path: &Path) -> FamilyLineState {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| rmp_serde::from_slice::<FamilyLineState>(&bytes).ok())
        .filter(|state| state.schema == STATE_SCHEMA)
        .unwrap_or_default()
}

fn store_state(path: &Path, state: &FamilyLineState) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(bytes) = rmp_serde::to_vec_named(state) else {
        return;
    };
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
