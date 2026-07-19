use anyhow::Result;

use super::inventory::{InventoryJsonPack, InventoryJsonReport};

pub(crate) const ADOPTION_GATES_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum AdoptionGateFormat {
    Human,
    Json,
}

#[derive(serde::Serialize)]
struct AdoptionGateReport {
    schema_version: u32,
    status: &'static str,
    totals: AdoptionGateTotals,
    policy: AdoptionGatePolicy,
    checklist: AdoptionGateChecklist,
    packs: Vec<AdoptionGatePack>,
}

#[derive(serde::Serialize)]
struct AdoptionGateTotals {
    builtin_packs: usize,
    builtin_default_packs: usize,
    builtin_optional_packs: usize,
    exact_capable_packs: usize,
    packs_needing_coverage: usize,
    blocked_packs: usize,
}

#[derive(serde::Serialize)]
struct AdoptionGatePolicy {
    scope: &'static str,
    default_lane: &'static str,
    optional_lane: &'static str,
    external_influence: &'static str,
    product_behavior_gate: &'static str,
    performance_gate: &'static str,
}

#[derive(serde::Serialize)]
struct AdoptionGateChecklist {
    builtin_optional: Vec<&'static str>,
    builtin_default: Vec<&'static str>,
    rollback: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct AdoptionGatePack {
    id: String,
    trust: &'static str,
    enabled_by_default: bool,
    exact_capable: bool,
    coverage_status: &'static str,
    adoption_status: &'static str,
    required_evidence: Vec<&'static str>,
    blockers: Vec<&'static str>,
    rollback_actions: Vec<&'static str>,
}

impl AdoptionGateReport {
    fn new() -> Self {
        let inventory = InventoryJsonReport::new();
        let packs = inventory
            .packs
            .iter()
            .map(AdoptionGatePack::new)
            .collect::<Vec<_>>();
        let blocked_packs = packs
            .iter()
            .filter(|pack| !pack.blockers.is_empty())
            .count();
        Self {
            schema_version: ADOPTION_GATES_SCHEMA_VERSION,
            status: if blocked_packs == 0 {
                inventory.status
            } else {
                "needs-evidence"
            },
            totals: AdoptionGateTotals {
                builtin_packs: inventory.totals.builtin_packs,
                builtin_default_packs: packs
                    .iter()
                    .filter(|pack| pack.trust == "builtin-default")
                    .count(),
                builtin_optional_packs: packs
                    .iter()
                    .filter(|pack| pack.trust == "builtin-optional")
                    .count(),
                exact_capable_packs: inventory.totals.exact_capable_packs,
                packs_needing_coverage: inventory.totals.packs_needing_coverage,
                blocked_packs,
            },
            policy: AdoptionGatePolicy {
                scope: "compiled-builtin",
                default_lane: "builtin-default",
                optional_lane: "builtin-optional",
                external_influence:
                    "unlocked-metadata-or-locked-near-or-receipt-backed-external-claim-exact",
                product_behavior_gate: "required-for-builtin-default-promotion",
                performance_gate: "required-for-builtin-default-promotion",
            },
            checklist: AdoptionGateChecklist {
                builtin_optional: builtin_optional_checklist(),
                builtin_default: builtin_default_checklist(),
                rollback: rollback_checklist(),
            },
            packs,
        }
    }
}

impl AdoptionGatePack {
    fn new(pack: &InventoryJsonPack) -> Self {
        let blockers = adoption_blockers(
            pack.trust,
            pack.enabled_by_default,
            pack.audit.exact_capable,
            pack.audit.coverage_status,
            &pack.audit.gaps,
        );
        Self {
            id: pack.id.clone(),
            trust: pack.trust,
            enabled_by_default: pack.enabled_by_default,
            exact_capable: pack.audit.exact_capable,
            coverage_status: pack.audit.coverage_status,
            adoption_status: adoption_status(pack.trust, pack.enabled_by_default, &blockers),
            required_evidence: required_evidence(pack.trust),
            blockers,
            rollback_actions: rollback_checklist(),
        }
    }
}

fn adoption_status(
    trust: &str,
    enabled_by_default: bool,
    blockers: &[&'static str],
) -> &'static str {
    if !blockers.is_empty() {
        "blocked"
    } else if trust == "builtin-default" && enabled_by_default {
        "default-gated"
    } else if trust == "builtin-optional" && !enabled_by_default {
        "optional-gated"
    } else {
        "tracked"
    }
}

fn adoption_blockers(
    trust: &str,
    enabled_by_default: bool,
    exact_capable: bool,
    coverage_status: &str,
    inventory_gaps: &[&'static str],
) -> Vec<&'static str> {
    let mut blockers = Vec::new();
    match trust {
        "builtin-default" => {
            if !enabled_by_default {
                blockers.push("builtin-default-not-enabled");
            }
        }
        "builtin-optional" => {
            if enabled_by_default {
                blockers.push("builtin-optional-enabled-by-default");
            }
        }
        _ => blockers.push("unexpected-trust-lane"),
    }
    if exact_capable && coverage_status != "covered" {
        blockers.push("exact-capable-coverage-gap");
    }
    if !inventory_gaps.is_empty() {
        blockers.push("inventory-audit-gap");
    }
    blockers
}

fn required_evidence(trust: &str) -> Vec<&'static str> {
    match trust {
        "builtin-default" => builtin_default_checklist(),
        "builtin-optional" => builtin_optional_checklist(),
        _ => vec!["ownership-and-trust-lane-decision"],
    }
}

fn builtin_optional_checklist() -> Vec<&'static str> {
    vec![
        "stable-pack-id-owner-version-policy",
        "structural-conformance",
        "positive-fixtures-and-hard-negatives",
        "dependency-backed-evidence-for-exact-rows",
        "unsupported-boundary-docs",
        "maintainer-owner",
        "rollback-plan",
    ]
}

fn builtin_default_checklist() -> Vec<&'static str> {
    vec![
        "inventory-covered",
        "query-regression-summary",
        "runtime-drift-measurement",
        "default-surface-noise-review",
        "docs-examples-capabilities",
        "release-note",
        "rollback-plan",
    ]
}

fn rollback_checklist() -> Vec<&'static str> {
    vec![
        "demote-pack-to-builtin-optional",
        "disable-risky-row-when-practical",
        "tighten-dependency-or-hard-negative-admission",
        "preserve-pack-provenance-in-reports",
    ]
}

pub(crate) fn cmd_adoption_gates(format: AdoptionGateFormat) -> Result<()> {
    let report = AdoptionGateReport::new();
    match format {
        AdoptionGateFormat::Human => print_adoption_gates_human(&report),
        AdoptionGateFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

fn print_adoption_gates_human(report: &AdoptionGateReport) {
    println!("builtin semantic-pack adoption gates: {}", report.status);
    println!(
        "packs: {}; default: {}; optional: {}; exact-capable: {}; blocked: {}",
        report.totals.builtin_packs,
        report.totals.builtin_default_packs,
        report.totals.builtin_optional_packs,
        report.totals.exact_capable_packs,
        report.totals.blocked_packs
    );
    println!("builtin-default promotion requires:");
    for item in &report.checklist.builtin_default {
        println!("  - {item}");
    }
    for pack in &report.packs {
        println!(
            "  {}: {} ({}, coverage {})",
            pack.id, pack.adoption_status, pack.trust, pack.coverage_status
        );
        if !pack.blockers.is_empty() {
            println!("    blockers: {}", pack.blockers.join(", "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{adoption_blockers, adoption_status, required_evidence};

    #[test]
    fn adoption_status_reports_supported_lanes() {
        assert_eq!(
            adoption_status("builtin-default", true, &[]),
            "default-gated"
        );
        assert_eq!(
            adoption_status("builtin-optional", false, &[]),
            "optional-gated"
        );
    }

    #[test]
    fn adoption_status_blocks_when_lane_or_coverage_is_invalid() {
        let optional_enabled = adoption_blockers(
            "builtin-optional",
            true,
            false,
            "tracked-no-exact-rows",
            &[],
        );
        assert_eq!(
            optional_enabled,
            vec!["builtin-optional-enabled-by-default"]
        );
        assert_eq!(
            adoption_status("builtin-optional", true, &optional_enabled),
            "blocked"
        );

        let coverage_gap = adoption_blockers(
            "builtin-default",
            true,
            true,
            "needs-coverage",
            &["exact-capable-missing-hard-negatives"],
        );
        assert_eq!(
            coverage_gap,
            vec!["exact-capable-coverage-gap", "inventory-audit-gap"]
        );
        assert_eq!(
            adoption_status("builtin-default", true, &coverage_gap),
            "blocked"
        );
    }

    #[test]
    fn required_evidence_differs_by_lane() {
        assert!(required_evidence("builtin-default").contains(&"query-regression-summary"));
        assert!(
            required_evidence("builtin-optional").contains(&"positive-fixtures-and-hard-negatives")
        );
        assert_eq!(
            required_evidence("external-opt-in"),
            vec!["ownership-and-trust-lane-decision"]
        );
    }
}
