use super::*;
use std::collections::BTreeMap;

const WORKSPACE_STATE_SCHEMA: u32 = 1;

#[derive(Default, Deserialize, Serialize)]
pub(super) struct WorkspaceState {
    pub(super) schema: u32,
    pub(super) discovery_membership_digest: String,
    pub(super) corpus_global_line_statistics_digest: String,
    pub(super) semantic_pack_digest: String,
    pub(super) swift_global_digest: String,
    pub(super) regions: BTreeMap<String, RegionState>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct RegionState {
    pub(super) path: String,
    pub(super) language: String,
    pub(super) raw_digest: String,
    pub(super) export_digest: String,
    pub(super) resolution_digest: String,
    pub(super) over_invalidated: bool,
}

pub(super) fn workspace_state(
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
        schema: WORKSPACE_STATE_SCHEMA,
        discovery_membership_digest: raw.discovery_digest.hex(),
        corpus_global_line_statistics_digest: raw.global_line_statistics_digest.hex(),
        semantic_pack_digest: semantic_pack_digest.hex(),
        swift_global_digest: hex(summary.swift_global_digest),
        regions,
    }
}

pub(super) fn region_key(raw: &RawCorpus, index: usize) -> String {
    let region = &raw.regions[index];
    format!(
        "{}\0{}\0{}",
        region.logical_path,
        raw.corpus.files[index].meta.lang.name(),
        region.region_id
    )
}

pub(super) fn global_invalidations(
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

pub(super) fn load_state(run: &CacheRun) -> Option<WorkspaceState> {
    let bytes = run.load("resolved-workspace", WORKSPACE_STATE_SCHEMA)?;
    let state = rmp_serde::from_slice::<WorkspaceState>(&bytes).ok()?;
    (state.schema == WORKSPACE_STATE_SCHEMA).then_some(state)
}

pub(super) fn store_state(run: &CacheRun, state: &WorkspaceState) {
    let Ok(bytes) = rmp_serde::to_vec(state) else {
        return;
    };
    run.store("resolved-workspace", WORKSPACE_STATE_SCHEMA, &bytes);
}
