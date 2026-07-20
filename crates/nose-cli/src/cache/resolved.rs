use super::digest::ContentDigest;
use super::portable_il;
use super::source::{RawCorpus, SourceIdentityKind};
use super::store::{ArtifactKey, ArtifactStage, LayeredCas};
use nose_frontend::ResolutionDependency;
use nose_il::Corpus;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const EXPORT_DEPENDENCY_SCHEMA: u32 = 1;
const RESOLVED_IL_SCHEMA: u32 = 3;
static STATE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct CachedCorpus {
    pub(crate) corpus: Corpus,
    pub(crate) report: InvalidationReport,
    pub(crate) unit_contexts: Vec<[u8; 32]>,
    pub(crate) workspace_digest: [u8; 32],
    pub(crate) semantic_pack_digest: [u8; 32],
    pub(crate) source_files: Vec<super::CachedSourceFile>,
}

#[derive(Debug, Serialize)]
pub(crate) struct InvalidationReport {
    schema: &'static str,
    discovery_membership_digest: String,
    corpus_global_line_statistics_digest: String,
    semantic_pack_digest: String,
    swift_global_digest: String,
    global_invalidations: Vec<&'static str>,
    source_identities: SourceIdentityCounts,
    source_snapshots: LayerStats,
    raw_il: LayerStats,
    resolved_il: LayerStats,
    invalidated: Vec<InvalidatedRegion>,
    over_invalidated: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LayerStats {
    hits: usize,
    misses: usize,
    passthrough: usize,
}

#[derive(Debug, Serialize)]
struct SourceIdentityCounts {
    git_blob: usize,
    content_sha256: usize,
}

#[derive(Debug, Serialize)]
struct InvalidatedRegion {
    path: String,
    language: String,
    reasons: Vec<&'static str>,
    dependency_providers: Vec<String>,
    source_identity: Option<SourceIdentityKind>,
}

#[derive(Default, Deserialize, Serialize)]
struct WorkspaceState {
    schema: u32,
    discovery_membership_digest: String,
    corpus_global_line_statistics_digest: String,
    semantic_pack_digest: String,
    swift_global_digest: String,
    regions: BTreeMap<String, RegionState>,
}

#[derive(Deserialize, Serialize)]
struct RegionState {
    path: String,
    language: String,
    raw_digest: String,
    export_digest: String,
    resolution_digest: String,
    over_invalidated: bool,
}

#[derive(Serialize)]
struct StoredDependencySummary<'a> {
    schema: u32,
    discovery_membership_digest: String,
    swift_global_digest: String,
    swift_global_active: bool,
    files: Vec<StoredFileSummary<'a>>,
}

#[derive(Serialize)]
struct StoredFileSummary<'a> {
    path: &'a str,
    export_digest: String,
    resolution_digest: String,
    dependencies: &'a [ResolutionDependency],
    over_invalidated: bool,
    requires_resolution: bool,
}

enum ResolvedReuse {
    Passthrough,
    Hit(Box<CorpusFile>),
    Miss,
}

type CorpusFile = nose_il::Il;

pub(super) fn build_resolved_corpus_cached(
    mut raw: RawCorpus,
    dir: &Path,
    semantic_pack_digest: ContentDigest,
) -> CachedCorpus {
    let workspace_digest = *raw.workspace_digest.as_bytes();
    let source_files = raw.source_files.clone();
    let cas = LayeredCas::new(dir);
    let summary =
        nose_frontend::resolution_dependency_summary(&raw.corpus.files, &raw.corpus.interner);
    debug_assert_eq!(raw.corpus.files.len(), raw.regions.len());
    debug_assert_eq!(summary.files.len(), raw.regions.len());
    let state_path = state_path(dir, raw.workspace_digest);
    let previous_state = load_state(&state_path);
    let current_state = workspace_state(&raw, &summary, semantic_pack_digest);
    store_dependency_summary(&cas, &raw, &summary);
    let resolved_keys = resolved_artifact_keys(&raw, &summary.files);
    let resolved_reuse = load_resolved_reuse(&cas, &raw, &summary.files, &resolved_keys);
    let affected = resolved_reuse
        .iter()
        .map(|reuse| matches!(reuse, ResolvedReuse::Miss))
        .collect::<Vec<_>>();
    nose_frontend::resolve_corpus_affected(&mut raw.corpus, &affected);
    apply_resolved_reuse(&cas, &mut raw, &resolved_keys, resolved_reuse);
    let report = build_invalidation_report(
        &raw,
        &summary,
        &affected,
        previous_state.as_ref(),
        &current_state,
        semantic_pack_digest,
    );
    store_state(&state_path, &current_state);
    CachedCorpus {
        unit_contexts: summary
            .files
            .iter()
            .map(|file| file.resolution_digest)
            .collect(),
        report,
        corpus: raw.corpus,
        workspace_digest,
        semantic_pack_digest: *semantic_pack_digest.as_bytes(),
        source_files,
    }
}

fn store_dependency_summary(
    cas: &LayeredCas,
    raw: &RawCorpus,
    summary: &nose_frontend::ResolutionDependencySummary,
) {
    let key = ArtifactKey::derive(
        ArtifactStage::ExportDependencySummary,
        EXPORT_DEPENDENCY_SCHEMA,
        &[
            raw.discovery_digest.as_bytes(),
            raw.global_line_statistics_digest.as_bytes(),
        ],
    );
    if cas.load(key).is_some() {
        return;
    }
    let stored = StoredDependencySummary {
        schema: EXPORT_DEPENDENCY_SCHEMA,
        discovery_membership_digest: raw.discovery_digest.hex(),
        swift_global_digest: hex(summary.swift_global_digest),
        swift_global_active: summary.swift_global_active,
        files: summary
            .files
            .iter()
            .zip(&raw.regions)
            .map(|(file, region)| StoredFileSummary {
                path: &region.logical_path,
                export_digest: hex(file.export_digest),
                resolution_digest: hex(file.resolution_digest),
                dependencies: &file.dependencies,
                over_invalidated: file.over_invalidated,
                requires_resolution: file.requires_resolution,
            })
            .collect(),
    };
    if let Ok(payload) = rmp_serde::to_vec_named(&stored) {
        let _ = cas.store(key, &payload);
    }
}

fn resolved_artifact_keys(
    raw: &RawCorpus,
    summaries: &[nose_frontend::FileResolutionDependencySummary],
) -> Vec<ArtifactKey> {
    raw.regions
        .iter()
        .zip(summaries)
        .map(|(region, file)| {
            ArtifactKey::derive(
                ArtifactStage::ResolvedIl,
                RESOLVED_IL_SCHEMA,
                &[region.raw_digest.as_bytes(), &file.resolution_digest],
            )
        })
        .collect()
}

fn load_resolved_reuse(
    cas: &LayeredCas,
    raw: &RawCorpus,
    summaries: &[nose_frontend::FileResolutionDependencySummary],
    keys: &[ArtifactKey],
) -> Vec<ResolvedReuse> {
    keys.par_iter()
        .enumerate()
        .map(|(index, &key)| {
            if !summaries[index].requires_resolution {
                return ResolvedReuse::Passthrough;
            }
            let Some(entry) = cas.load(key) else {
                return ResolvedReuse::Miss;
            };
            portable_il::decode(
                &entry.payload,
                &raw.corpus.interner,
                raw.corpus.files[index].file,
                raw.corpus.files[index].meta.path.clone(),
            )
            .map_or(ResolvedReuse::Miss, |il| ResolvedReuse::Hit(Box::new(il)))
        })
        .collect()
}

fn apply_resolved_reuse(
    cas: &LayeredCas,
    raw: &mut RawCorpus,
    keys: &[ArtifactKey],
    reuse: Vec<ResolvedReuse>,
) {
    for (index, reuse) in reuse.into_iter().enumerate() {
        match reuse {
            ResolvedReuse::Passthrough => continue,
            ResolvedReuse::Hit(cached) => {
                raw.corpus.files[index] = *cached;
                continue;
            }
            ResolvedReuse::Miss => {}
        }
        if let Ok(payload) = portable_il::encode(&raw.corpus.files[index], &raw.corpus.interner) {
            let _ = cas.store(keys[index], &payload);
        }
    }
}

fn build_invalidation_report(
    raw: &RawCorpus,
    summary: &nose_frontend::ResolutionDependencySummary,
    affected: &[bool],
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
    semantic_pack_digest: ContentDigest,
) -> InvalidationReport {
    let (invalidated, over_invalidated) =
        invalidated_regions(raw, summary, affected, previous, current);
    let raw_hits = raw.regions.iter().filter(|region| region.raw_hit).count();
    let resolved_misses = affected.iter().filter(|affected| **affected).count();
    let resolved_passthrough = summary
        .files
        .iter()
        .filter(|file| !file.requires_resolution)
        .count();
    InvalidationReport {
        schema: "nose.invalidation/v1",
        discovery_membership_digest: raw.discovery_digest.hex(),
        corpus_global_line_statistics_digest: raw.global_line_statistics_digest.hex(),
        semantic_pack_digest: semantic_pack_digest.hex(),
        swift_global_digest: hex(summary.swift_global_digest),
        global_invalidations: global_invalidations(previous, current),
        source_identities: SourceIdentityCounts {
            git_blob: raw
                .regions
                .iter()
                .filter(|region| region.source_kind == SourceIdentityKind::GitBlob)
                .count(),
            content_sha256: raw
                .regions
                .iter()
                .filter(|region| region.source_kind == SourceIdentityKind::ContentSha256)
                .count(),
        },
        source_snapshots: LayerStats {
            hits: raw.source_hits,
            misses: raw.source_misses,
            passthrough: 0,
        },
        raw_il: LayerStats {
            hits: raw_hits,
            misses: raw.regions.len() - raw_hits,
            passthrough: 0,
        },
        resolved_il: LayerStats {
            hits: affected.len() - resolved_misses,
            misses: resolved_misses,
            passthrough: resolved_passthrough,
        },
        invalidated,
        over_invalidated,
    }
}

fn invalidated_regions(
    raw: &RawCorpus,
    summary: &nose_frontend::ResolutionDependencySummary,
    affected: &[bool],
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
) -> (Vec<InvalidatedRegion>, Vec<String>) {
    if previous.is_none() {
        return (Vec::new(), Vec::new());
    }
    let indexes = affected
        .iter()
        .enumerate()
        .filter_map(|(index, affected)| {
            (*affected || region_state_changed(index, raw, previous, current)).then_some(index)
        })
        .collect::<BTreeSet<_>>();
    let mut invalidated = indexes
        .iter()
        .map(|&index| {
            invalidated_region(
                index,
                raw,
                &summary.files[index].dependencies,
                previous,
                current,
            )
        })
        .collect::<Vec<_>>();
    append_deleted_regions(&mut invalidated, previous, current);
    invalidated.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.language.cmp(&right.language))
    });
    let over_invalidated = indexes
        .iter()
        .filter(|&&index| summary.files[index].over_invalidated)
        .map(|&index| raw.regions[index].logical_path.clone())
        .collect();
    (invalidated, over_invalidated)
}

fn append_deleted_regions(
    invalidated: &mut Vec<InvalidatedRegion>,
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
) {
    let Some(previous) = previous else { return };
    let current_keys = current.regions.keys().collect::<BTreeSet<_>>();
    invalidated.extend(
        previous
            .regions
            .iter()
            .filter(|(key, _)| !current_keys.contains(key))
            .map(|(_, region)| InvalidatedRegion {
                path: region.path.clone(),
                language: region.language.clone(),
                reasons: vec!["deleted-source"],
                dependency_providers: Vec::new(),
                source_identity: None,
            }),
    );
}

fn region_state_changed(
    index: usize,
    raw: &RawCorpus,
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let key = region_key(raw, index);
    let Some(before) = previous.regions.get(&key) else {
        return true;
    };
    let after = &current.regions[&key];
    before.raw_digest != after.raw_digest || before.resolution_digest != after.resolution_digest
}

fn invalidated_region(
    index: usize,
    raw: &RawCorpus,
    dependencies: &[ResolutionDependency],
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
) -> InvalidatedRegion {
    let region = &raw.regions[index];
    let mut reasons = Vec::new();
    let key = region_key(raw, index);
    let current_region = &current.regions[&key];
    let previous_region = previous.and_then(|state| state.regions.get(&key));
    match (previous, previous_region) {
        (None, _) => reasons.push("cold-start"),
        (Some(_), None) => reasons.push("added-source"),
        (Some(previous), Some(before)) if before.raw_digest != current_region.raw_digest => {
            reasons.push(match region.source_kind {
                SourceIdentityKind::GitBlob | SourceIdentityKind::ContentSha256 => "source-content",
            });
            if before.export_digest != current_region.export_digest {
                reasons.push("export-surface");
            }
            if previous.discovery_membership_digest != current.discovery_membership_digest {
                reasons.push("discovery-membership");
            }
        }
        (Some(previous), Some(before))
            if before.resolution_digest != current_region.resolution_digest =>
        {
            if raw.corpus.files[index].meta.lang == nose_il::Lang::Swift
                && previous.swift_global_digest != current.swift_global_digest
            {
                reasons.push("swift-global-sentinel");
            } else if current_region.over_invalidated {
                reasons.push("unknown-dependency-over-invalidation");
            } else {
                reasons.push("dependency-export");
            }
        }
        _ => reasons.push("artifact-miss-or-corruption"),
    }
    let mut dependency_providers = dependencies
        .iter()
        .filter_map(|dependency| dependency.provider_file)
        .filter_map(|provider| raw.regions.get(provider))
        .map(|provider| provider.logical_path.clone())
        .collect::<Vec<_>>();
    dependency_providers.sort();
    dependency_providers.dedup();
    InvalidatedRegion {
        path: region.logical_path.clone(),
        language: raw.corpus.files[index].meta.lang.name().to_owned(),
        reasons,
        dependency_providers,
        source_identity: Some(region.source_kind),
    }
}

fn workspace_state(
    raw: &RawCorpus,
    summary: &nose_frontend::ResolutionDependencySummary,
    semantic_pack_digest: ContentDigest,
) -> WorkspaceState {
    let regions = raw
        .regions
        .iter()
        .zip(&raw.corpus.files)
        .zip(&summary.files)
        .enumerate()
        .map(|(index, ((region, il), summary))| {
            (
                region_key(raw, index),
                RegionState {
                    path: region.logical_path.clone(),
                    language: il.meta.lang.name().to_owned(),
                    raw_digest: region.raw_digest.hex(),
                    export_digest: hex(summary.export_digest),
                    resolution_digest: hex(summary.resolution_digest),
                    over_invalidated: summary.over_invalidated,
                },
            )
        })
        .collect();
    WorkspaceState {
        schema: 1,
        discovery_membership_digest: raw.discovery_digest.hex(),
        corpus_global_line_statistics_digest: raw.global_line_statistics_digest.hex(),
        semantic_pack_digest: semantic_pack_digest.hex(),
        swift_global_digest: hex(summary.swift_global_digest),
        regions,
    }
}

fn region_key(raw: &RawCorpus, index: usize) -> String {
    let region = &raw.regions[index];
    format!(
        "{}\0{}\0{}",
        region.logical_path,
        raw.corpus.files[index].meta.lang.name(),
        region.region_id
    )
}

fn global_invalidations(
    previous: Option<&WorkspaceState>,
    current: &WorkspaceState,
) -> Vec<&'static str> {
    let Some(previous) = previous else {
        return vec!["cold-start"];
    };
    let mut out = Vec::new();
    if previous.discovery_membership_digest != current.discovery_membership_digest {
        out.push("discovery-membership");
    }
    if previous.corpus_global_line_statistics_digest != current.corpus_global_line_statistics_digest
    {
        out.push("corpus-global-line-statistics");
    }
    if previous.semantic_pack_digest != current.semantic_pack_digest {
        out.push("semantic-pack-influence");
    }
    out
}

fn state_path(dir: &Path, workspace: ContentDigest) -> PathBuf {
    dir.join("state-v1")
        .join(format!("{}.json", workspace.hex()))
}

fn load_state(path: &Path) -> Option<WorkspaceState> {
    let bytes = std::fs::read(path).ok()?;
    let state = serde_json::from_slice::<WorkspaceState>(&bytes).ok()?;
    (state.schema == 1).then_some(state)
}

fn store_state(path: &Path, state: &WorkspaceState) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(state) else {
        return;
    };
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        STATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&temp, bytes).is_ok() && std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

fn hex(bytes: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}
