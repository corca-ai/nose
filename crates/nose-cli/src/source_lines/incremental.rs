use super::{
    is_trivial_line, shared_lines_of, varying_spots_of, FileLineCache, LineIdf, SharedLines,
};
use crate::cache::CachedLineContext;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const STATE_SCHEMA: u32 = 1;
const STATE_SLOT: &str = "family-lines";

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
    line_index_complete: bool,
) -> Option<FamilyLineStats> {
    let mut previous = load_state(&context.run);
    let previous_len = previous.families.len();
    let digests = source_digests(context);
    if !line_index_complete
        && families
            .iter()
            .filter(|family| family.languages == 1 && family.locations.len() >= 2)
            .any(|family| {
                previous
                    .families
                    .get(&family_key(family, &digests))
                    .is_none_or(|stored| stored.file_count != file_count)
            })
    {
        return None;
    }
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
        let analysis = if let Some(mut stored) = previous.families.remove(&key) {
            let needs_reweight = stored.file_count != file_count
                || stored.shared.as_ref().is_some_and(|shared| {
                    shared
                        .rank_lines
                        .iter()
                        .any(|line| changed_lines.contains(line))
                });
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
    if stats.families_reweighted > 0 || stats.families_rebuilt > 0 || current.len() != previous_len
    {
        store_state(
            &context.run,
            &FamilyLineState {
                schema: STATE_SCHEMA,
                families: current,
            },
        );
    }
    Some(stats)
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

fn load_state(run: &crate::cache::CacheRun) -> FamilyLineState {
    run.load(STATE_SLOT, STATE_SCHEMA)
        .and_then(|bytes| rmp_serde::from_slice::<FamilyLineState>(&bytes).ok())
        .filter(|state| state.schema == STATE_SCHEMA)
        .unwrap_or_default()
}

fn store_state(run: &crate::cache::CacheRun, state: &FamilyLineState) {
    // `VaryingSpot` omits empty optional fields, which requires a named map: compact
    // tuple encoding would shift the remaining fields and fail closed on reload.
    let Ok(bytes) = rmp_serde::to_vec_named(state) else {
        return;
    };
    run.store(STATE_SLOT, STATE_SCHEMA, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_empty_varying_spot_fields_round_trip() {
        let state = FamilyLineState {
            schema: STATE_SCHEMA,
            families: BTreeMap::from([(
                [7; 32],
                StoredFamilyLines {
                    shared: None,
                    varying_spots: vec![nose_detect::VaryingSpot {
                        param: 1,
                        a_lines: None,
                        b_lines: None,
                        a_text: String::new(),
                        b_text: String::new(),
                    }],
                    shared_weight: 0.0,
                    file_count: 2,
                },
            )]),
        };
        let bytes = rmp_serde::to_vec_named(&state).expect("serialize family line state");
        let restored: FamilyLineState =
            rmp_serde::from_slice(&bytes).expect("deserialize family line state");
        let spot = &restored.families[&[7; 32]].varying_spots[0];
        assert_eq!(spot.param, 1);
        assert!(spot.a_lines.is_none());
        assert!(spot.a_text.is_empty());
    }
}
