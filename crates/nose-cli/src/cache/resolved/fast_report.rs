use super::*;

pub(crate) fn fast_invalidation_report(
    snapshot: &CachedUnitSnapshot,
    current_sources: &[super::super::CachedSourceFile],
    changed_source: Option<&str>,
) -> InvalidationReport {
    let current_discovery = super::super::source::discovery_digest(current_sources);
    let current_lines = super::super::source::global_line_statistics_digest(current_sources);
    let current_identities = current_sources
        .iter()
        .map(|source| (source.path.as_str(), source.source_kind))
        .collect::<BTreeMap<_, _>>();
    let changed_regions = changed_source.map_or(0, |changed| {
        snapshot
            .contexts
            .iter()
            .filter(|context| context.source_path == changed)
            .count()
    });
    let mut global_invalidations = Vec::new();
    if current_discovery.as_bytes() != &snapshot.discovery_digest {
        global_invalidations.push("discovery-membership");
    }
    if current_lines.as_bytes() != &snapshot.global_line_statistics_digest {
        global_invalidations.push("corpus-global-line-statistics");
    }
    let invalidated = changed_source
        .into_iter()
        .flat_map(|changed| {
            let source = current_sources.iter().find(|source| source.path == changed);
            snapshot
                .contexts
                .iter()
                .filter(move |context| context.source_path == changed)
                .map(move |context| InvalidatedRegion {
                    path: source
                        .map(|source| source.logical_path.clone())
                        .unwrap_or_else(|| context.region_path.clone()),
                    language: context.lang.name().to_owned(),
                    reasons: vec!["source-content"],
                    dependency_providers: Vec::new(),
                    source_identity: source.map(|source| source.source_kind),
                })
        })
        .collect();
    InvalidationReport {
        schema: "nose.invalidation/v1",
        discovery_membership_digest: current_discovery.hex(),
        corpus_global_line_statistics_digest: current_lines.hex(),
        semantic_pack_digest: ContentDigest::from_bytes(snapshot.semantic_pack_digest).hex(),
        swift_global_digest: hex(snapshot.swift_global_digest),
        global_invalidations,
        source_identities: SourceIdentityCounts {
            git_blob: snapshot
                .contexts
                .iter()
                .filter(|context| {
                    current_identities
                        .get(context.source_path.as_str())
                        .copied()
                        .unwrap_or(context.source_kind)
                        == SourceIdentityKind::GitBlob
                })
                .count(),
            content_sha256: snapshot
                .contexts
                .iter()
                .filter(|context| {
                    current_identities
                        .get(context.source_path.as_str())
                        .copied()
                        .unwrap_or(context.source_kind)
                        == SourceIdentityKind::ContentSha256
                })
                .count(),
        },
        source_snapshots: LayerStats {
            hits: current_sources.len() - usize::from(changed_source.is_some()),
            misses: usize::from(changed_source.is_some()),
            passthrough: 0,
        },
        raw_il: LayerStats {
            hits: snapshot.contexts.len() - changed_regions,
            misses: changed_regions,
            passthrough: 0,
        },
        resolved_il: LayerStats {
            hits: snapshot.contexts.len(),
            misses: 0,
            passthrough: snapshot
                .contexts
                .iter()
                .filter(|context| !context.requires_resolution)
                .count(),
        },
        invalidated,
        over_invalidated: Vec::new(),
    }
}
