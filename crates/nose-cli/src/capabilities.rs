use anyhow::Result;

const CAPABILITIES_SCHEMA_VERSION: u32 = 8;

#[derive(serde::Serialize)]
struct Report {
    schema_version: u32,
    tool: Tool,
    platform: Platform,
    interfaces: Interfaces,
    commands: Commands,
    schemas: Schemas,
    query: QuerySurface,
    semantic_packs: SemanticPacks,
    il: Il,
    stats: Stats,
}

#[derive(serde::Serialize)]
struct Tool {
    name: &'static str,
    version: &'static str,
}

#[derive(serde::Serialize)]
struct Platform {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
}

#[derive(serde::Serialize)]
struct Interfaces {
    capabilities_json: bool,
    version_json: bool,
    doctor_json: bool,
}

#[derive(serde::Serialize)]
struct Commands {
    stable: Vec<&'static str>,
    deprecated: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct Schemas {
    capabilities: Vec<u32>,
    cache_status: Vec<&'static str>,
    cache_prune: Vec<&'static str>,
    cache_clear: Vec<&'static str>,
    query_json: Vec<u32>,
    query_watch_jsonl: Vec<&'static str>,
    analysis: Vec<&'static str>,
    semantic_packs: Vec<&'static str>,
    semantic_pack_locks: Vec<&'static str>,
    semantic_pack_receipts: Vec<&'static str>,
    semantic_pack_lock_status: Vec<u32>,
    semantic_pack_conformance: Vec<u32>,
    semantic_pack_inventory: Vec<u32>,
    semantic_pack_adoption_gates: Vec<u32>,
    semantic_pack_compatibility: Vec<u32>,
}

#[derive(serde::Serialize)]
struct QuerySurface {
    modes: Vec<&'static str>,
    default_modes: Vec<&'static str>,
    output_formats: Vec<&'static str>,
    sort_keys: Vec<&'static str>,
    config_keys: Vec<&'static str>,
    capabilities: std::collections::BTreeMap<&'static str, bool>,
    analysis: serde_json::Value,
    member_navigation: serde_json::Value,
}

#[derive(serde::Serialize)]
struct SemanticPacks {
    api_versions: Vec<&'static str>,
    lock_api_versions: Vec<&'static str>,
    loading: Vec<&'static str>,
    project_lock: Vec<&'static str>,
    project_lock_output_formats: Vec<&'static str>,
    conformance: Vec<&'static str>,
    conformance_output_formats: Vec<&'static str>,
    inventory: Vec<&'static str>,
    inventory_output_formats: Vec<&'static str>,
    adoption_gates: Vec<&'static str>,
    adoption_gate_output_formats: Vec<&'static str>,
    compatibility: Vec<&'static str>,
    compatibility_output_formats: Vec<&'static str>,
    trust: Vec<&'static str>,
    external_packs_enabled_by_default: bool,
    external_pack_influence: &'static str,
    external_exact_operations: Vec<&'static str>,
    external_influence_blockers: Vec<&'static str>,
    external_pack_execution: &'static str,
}

#[derive(serde::Serialize)]
struct Il {
    output_formats: Vec<&'static str>,
    normalized: bool,
    cfg_norm_toggle: bool,
}

#[derive(serde::Serialize)]
struct Stats {
    output_formats: Vec<&'static str>,
}

impl Report {
    fn current() -> Self {
        Report {
            schema_version: CAPABILITIES_SCHEMA_VERSION,
            tool: Tool {
                name: "nose",
                version: env!("CARGO_PKG_VERSION"),
            },
            platform: Platform {
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                family: std::env::consts::FAMILY,
            },
            interfaces: Interfaces {
                capabilities_json: true,
                version_json: false,
                doctor_json: false,
            },
            commands: Commands {
                stable: vec![
                    "cache",
                    "capabilities",
                    "il",
                    "query",
                    "regions",
                    "semantic-pack",
                    "stats",
                ],
                deprecated: Vec::new(),
            },
            schemas: current_schemas(),
            query: QuerySurface {
                modes: vec!["syntax", "semantic", "near"],
                default_modes: vec!["syntax", "semantic", "near"],
                output_formats: vec!["human", "json", "jsonl", "markdown", "sarif"],
                sort_keys: vec!["extractability", "value", "sites", "hazard"],
                config_keys: vec![
                    "cache-max-bytes",
                    "exclude",
                    "generated-paths",
                    "ignore-file",
                    "min-lines",
                    "min-members",
                    "min-size",
                    "min-value",
                    "mode",
                    "semantic-packs",
                    "semantic-pack-lock",
                    "sort",
                ],
                capabilities: query_capability_flags(),
                analysis: crate::query_evolution::capabilities(),
                member_navigation: crate::query_members::capabilities(),
            },
            semantic_packs: current_semantic_packs(),
            il: Il {
                output_formats: vec!["sexpr", "json"],
                normalized: true,
                cfg_norm_toggle: true,
            },
            stats: Stats {
                output_formats: vec!["human", "json"],
            },
        }
    }
}

fn current_schemas() -> Schemas {
    Schemas {
        capabilities: vec![CAPABILITIES_SCHEMA_VERSION],
        cache_status: vec!["nose.cache-status/v1"],
        cache_prune: vec!["nose.cache-prune/v1"],
        cache_clear: vec!["nose.cache-clear/v1"],
        query_json: vec![
            crate::schema_versions::QUERY_BASE_JSON_SCHEMA_VERSION,
            crate::schema_versions::QUERY_JSON_SCHEMA_VERSION,
        ],
        query_watch_jsonl: vec![crate::schema_versions::QUERY_WATCH_JSONL_SCHEMA],
        analysis: vec![
            "nose.analysis/v1",
            "nose.analysis-capture/v1",
            "nose.analysis-changes/v1",
        ],
        semantic_packs: nose_semantics::SUPPORTED_SEMANTIC_PACK_API_VERSIONS.to_vec(),
        semantic_pack_locks: vec![nose_semantics::SEMANTIC_PACK_LOCK_API_VERSION_V1],
        semantic_pack_receipts: vec![nose_semantics::SEMANTIC_PACK_RECEIPT_API_VERSION_V1],
        semantic_pack_lock_status: vec![crate::semantic_pack::LOCK_STATUS_SCHEMA_VERSION],
        semantic_pack_conformance: vec![crate::semantic_pack::CONFORMANCE_SCHEMA_VERSION],
        semantic_pack_inventory: vec![crate::semantic_pack::INVENTORY_SCHEMA_VERSION],
        semantic_pack_adoption_gates: vec![crate::semantic_pack::ADOPTION_GATES_SCHEMA_VERSION],
        semantic_pack_compatibility: vec![crate::semantic_pack::COMPATIBILITY_SCHEMA_VERSION],
    }
}

fn current_semantic_packs() -> SemanticPacks {
    SemanticPacks {
        api_versions: nose_semantics::SUPPORTED_SEMANTIC_PACK_API_VERSIONS.to_vec(),
        lock_api_versions: vec![nose_semantics::SEMANTIC_PACK_LOCK_API_VERSION_V1],
        loading: vec![
            "compiled-builtin",
            "local-manifest-file",
            "local-manifest-directory",
            "local-project-lock",
        ],
        project_lock: vec!["create", "status"],
        project_lock_output_formats: vec!["human", "json"],
        conformance: vec![
            "local-manifest-file",
            "local-manifest-directory",
            "v0-fixture-metadata",
            "v1-kernel-source-analysis",
            "receipt-output",
        ],
        conformance_output_formats: vec!["human", "json"],
        inventory: vec!["compiled-builtin"],
        inventory_output_formats: vec!["human", "json"],
        adoption_gates: vec!["compiled-builtin"],
        adoption_gate_output_formats: vec!["human", "json"],
        compatibility: vec!["policy"],
        compatibility_output_formats: vec!["human", "json"],
        trust: vec!["builtin-default", "builtin-optional", "external-opt-in"],
        external_packs_enabled_by_default: false,
        external_pack_influence: "metadata-or-locked-near-or-receipt-backed-external-claim-exact",
        external_exact_operations: vec!["collection-factory"],
        external_influence_blockers: crate::semantic_pack::external_influence_blocker_labels(),
        external_pack_execution: "none",
    }
}

fn query_capability_flags() -> std::collections::BTreeMap<&'static str, bool> {
    [
        ("base_divergence", true),
        ("baseline", true),
        ("baseline_changed_detection", true),
        ("baseline_member_digest", true),
        ("cache", true),
        ("caller_generated_paths", true),
        ("ci_fail_gate", true),
        ("family_drilldown", true),
        ("inline_suppression", true),
        ("multi_root", true),
        ("query_base_gate_fail_default", true),
        ("query_base_json_v8", true),
        ("query_base_region_candidates_v1", true),
        ("query_region_identity_v1", true),
        ("query_review_key_v1", true),
        ("query_analysis_capture_v1", true),
        ("query_analysis_changes_v1", true),
        ("query_analysis_diagnostics_v1", true),
        ("query_analysis_navigation_v1", true),
        ("query_analysis_member_changes_v1", true),
        ("query_analysis_verified_source_v1", true),
        ("query_review_records_v1", true),
        ("query_extraction_assessment_v1", true),
        ("query_source_evidence_v1", true),
        ("query_member_navigation_v1", true),
        ("query_scope_evidence_v1", true),
        ("query_base_evidence_navigation_v1", true),
        ("region_snapshots_v1", true),
        ("region_correspondence_v1", true),
        ("query_base_sarif", true),
        ("query_base_structured_ignores", true),
        ("query_watch", true),
        ("query_watch_full_snapshot", true),
        ("query_watch_jsonl_v1", true),
        ("reinvented_view", true),
        ("semantic_pack_dependency_evidence", true),
        ("semantic_pack_locked_near_influence", true),
        ("semantic_pack_external_claim_exact", true),
        ("semantic_pack_kernel_conformance_receipt", true),
        ("semantic_pack_loading", true),
        ("semantic_pack_project_lock", true),
        ("structured_ignores", true),
    ]
    .into_iter()
    .collect()
}

pub(crate) fn print() -> Result<()> {
    let report = Report::current();
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
