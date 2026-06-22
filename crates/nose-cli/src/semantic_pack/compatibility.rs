use anyhow::Result;

use super::inventory::InventoryJsonReport;

pub(crate) const COMPATIBILITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum CompatibilityFormat {
    Human,
    Json,
}

#[derive(serde::Serialize)]
struct CompatibilityReport {
    schema_version: u32,
    status: &'static str,
    current_nose_version: &'static str,
    supported: CompatibilitySupported,
    policy: CompatibilityPolicy,
    requirements: CompatibilityRequirements,
    failure_modes: Vec<CompatibilityFailureMode>,
    checks: CompatibilityChecks,
}

#[derive(serde::Serialize)]
struct CompatibilitySupported {
    manifest_api_versions: Vec<&'static str>,
    trust_lanes: Vec<&'static str>,
    report_sources: Vec<&'static str>,
    output_formats: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct CompatibilityPolicy {
    manifest_nose_version: &'static str,
    manifest_schema_changes: &'static str,
    kernel_vocabulary_changes: &'static str,
    capabilities_changes: &'static str,
    external_pack_influence: &'static str,
    external_pack_execution: &'static str,
    external_packs_enabled_by_default: bool,
}

#[derive(serde::Serialize)]
struct CompatibilityRequirements {
    manifest: Vec<&'static str>,
    kernel: Vec<&'static str>,
    migration: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct CompatibilityFailureMode {
    code: &'static str,
    action: &'static str,
}

#[derive(serde::Serialize)]
struct CompatibilityChecks {
    builtin_inventory_status: &'static str,
    builtin_packs: usize,
    external_metadata_only: bool,
    external_influence_blockers: Vec<&'static str>,
}

impl CompatibilityReport {
    fn new() -> Self {
        let inventory = InventoryJsonReport::new();
        let blockers = external_influence_blocker_labels();
        Self {
            schema_version: COMPATIBILITY_SCHEMA_VERSION,
            status: if inventory.status == "ok" {
                "ok"
            } else {
                "blocked"
            },
            current_nose_version: env!("CARGO_PKG_VERSION"),
            supported: CompatibilitySupported {
                manifest_api_versions: vec![nose_semantics::SEMANTIC_PACK_API_VERSION],
                trust_lanes: vec!["builtin-default", "builtin-optional", "external-opt-in"],
                report_sources: vec!["policy"],
                output_formats: vec!["human", "json"],
            },
            policy: CompatibilityPolicy {
                manifest_nose_version: "must-include-installed-version",
                manifest_schema_changes: "breaking-change-requires-new-api-version",
                kernel_vocabulary_changes: "document-and-rehearse-with-builtin-packs-first",
                capabilities_changes: "additive-or-schema-versioned",
                external_pack_influence: "metadata-only",
                external_pack_execution: "none",
                external_packs_enabled_by_default: false,
            },
            requirements: CompatibilityRequirements {
                manifest: vec![
                    "api-version-supported",
                    "nose-version-range-includes-installed-version",
                    "stable-pack-id",
                    "local-manifest-trust-external-opt-in",
                    "local-manifest-disabled-by-default",
                    "no-duplicate-or-reserved-pack-id",
                ],
                kernel: vec![
                    "dependency-backed-evidence-required",
                    "unsupported-capability-fail-closed",
                    "unknown-fields-and-enums-rejected",
                    "external-execution-none",
                    "metadata-only-external-rows-do-not-influence-analysis",
                ],
                migration: vec![
                    "builtin-pack-migration-before-external-influence",
                    "breaking-manifest-change-requires-api-version",
                    "additive-capabilities-fields-without-schema-bump",
                    "vocabulary-change-requires-docs-conformance-and-compatibility-note",
                    "product-behavior-and-performance-gates-before-default-enable",
                ],
            },
            failure_modes: compatibility_failure_modes(),
            checks: CompatibilityChecks {
                builtin_inventory_status: inventory.status,
                builtin_packs: inventory.totals.builtin_packs,
                external_metadata_only: true,
                external_influence_blockers: blockers,
            },
        }
    }
}

fn compatibility_failure_modes() -> Vec<CompatibilityFailureMode> {
    vec![
        CompatibilityFailureMode {
            code: "unsupported-api-version",
            action: "reject-before-analysis",
        },
        CompatibilityFailureMode {
            code: "unsupported-nose-version",
            action: "reject-before-analysis",
        },
        CompatibilityFailureMode {
            code: "unsupported-trust-lane",
            action: "reject-before-analysis",
        },
        CompatibilityFailureMode {
            code: "default-enabled-external-pack",
            action: "reject-before-analysis",
        },
        CompatibilityFailureMode {
            code: "duplicate-or-reserved-pack-id",
            action: "reject-before-analysis",
        },
        CompatibilityFailureMode {
            code: "unsupported-influence",
            action: "block-external-influence",
        },
        CompatibilityFailureMode {
            code: "row-conflict",
            action: "block-external-influence",
        },
    ]
}

pub(crate) fn external_influence_blocker_labels() -> Vec<&'static str> {
    vec![
        nose_semantics::ExternalInfluenceBlocker::DataOnlyRegistration.as_str(),
        nose_semantics::ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable.as_str(),
        nose_semantics::ExternalInfluenceBlocker::ExplicitInfluenceTrustGateMissing.as_str(),
        nose_semantics::ExternalInfluenceBlocker::ExecutableConformanceUnavailable.as_str(),
        nose_semantics::ExternalInfluenceBlocker::RowConflict.as_str(),
    ]
}

pub(crate) fn cmd_compatibility(format: CompatibilityFormat) -> Result<()> {
    let report = CompatibilityReport::new();
    match format {
        CompatibilityFormat::Human => print_compatibility_human(&report),
        CompatibilityFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

fn print_compatibility_human(report: &CompatibilityReport) {
    println!("semantic-pack compatibility: {}", report.status);
    println!("nose version: {}", report.current_nose_version);
    println!(
        "manifest APIs: {}",
        report.supported.manifest_api_versions.join(", ")
    );
    println!(
        "external packs: influence {}, execution {}, enabled-by-default {}",
        report.policy.external_pack_influence,
        report.policy.external_pack_execution,
        report.policy.external_packs_enabled_by_default
    );
    println!("manifest requirements:");
    for item in &report.requirements.manifest {
        println!("  - {item}");
    }
    println!("failure modes:");
    for mode in &report.failure_modes {
        println!("  - {}: {}", mode.code, mode.action);
    }
}

#[cfg(test)]
mod tests {
    use super::{compatibility_failure_modes, external_influence_blocker_labels};

    #[test]
    fn failure_modes_cover_fail_closed_policy() {
        let codes = compatibility_failure_modes()
            .into_iter()
            .map(|mode| mode.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"unsupported-api-version"));
        assert!(codes.contains(&"unsupported-nose-version"));
        assert!(codes.contains(&"unsupported-influence"));
        assert!(codes.contains(&"row-conflict"));
    }

    #[test]
    fn external_blocker_labels_are_stable() {
        assert_eq!(
            external_influence_blocker_labels(),
            vec![
                "data-only-registration",
                "dependency-backed-evidence-unavailable",
                "explicit-influence-trust-gate-missing",
                "executable-conformance-unavailable",
                "row-conflict"
            ]
        );
    }
}
