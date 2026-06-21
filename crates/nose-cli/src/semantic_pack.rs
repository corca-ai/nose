use anyhow::Result;
use std::path::PathBuf;

pub(crate) const CONFORMANCE_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum CheckFormat {
    Human,
    Json,
}

#[derive(serde::Serialize)]
struct CheckJsonReport {
    schema_version: u32,
    status: &'static str,
    totals: CheckJsonTotals,
    executable_conformance: CheckJsonExecutableConformance,
    influence_preflight: CheckJsonInfluencePreflight,
    manifests: Vec<CheckJsonManifest>,
}

#[derive(serde::Serialize)]
struct CheckJsonTotals {
    manifests: usize,
    positive_fixtures: usize,
    hard_negatives: usize,
    fixture_issues: usize,
    executable_conformance_rows: usize,
    passed_executable_conformance_rows: usize,
    executable_conformance_issues: usize,
    influence_rows: usize,
    blocked_influence_rows: usize,
}

#[derive(serde::Serialize)]
struct CheckJsonManifest {
    id: String,
    version: String,
    display_name: String,
    trust: &'static str,
    source: &'static str,
    influence: &'static str,
    manifest_path: String,
    provider: String,
    repository: String,
    license: String,
    supported_languages: Vec<String>,
    counts: CheckJsonCounts,
    conformance_command: Option<String>,
    proof_links: Vec<String>,
    fixture_issues: usize,
    fixtures: Vec<CheckJsonFixture>,
}

#[derive(serde::Serialize)]
struct CheckJsonCounts {
    evidence_producers: usize,
    contracts: usize,
    value_laws: usize,
    positive_fixtures: usize,
    hard_negatives: usize,
}

#[derive(serde::Serialize)]
struct CheckJsonFixture {
    kind: &'static str,
    id: String,
    description: String,
    declared_path: Option<String>,
    resolved_path: Option<String>,
    expectation: Option<String>,
    issues: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct CheckJsonExecutableConformance {
    status: &'static str,
    rows: Vec<CheckJsonExecutableConformanceRow>,
}

#[derive(serde::Serialize)]
struct CheckJsonExecutableConformanceRow {
    gate_id: String,
    kind: &'static str,
    row_id: String,
    row_hash: String,
    pack_id: String,
    pack_hash: String,
    manifest_path: String,
    channel: &'static str,
    oracle: &'static str,
    passed: bool,
    positive_fixtures: Vec<String>,
    hard_negatives: Vec<String>,
    issues: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct CheckJsonInfluencePreflight {
    status: &'static str,
    rows: Vec<CheckJsonInfluenceRow>,
}

#[derive(serde::Serialize)]
struct CheckJsonInfluenceRow {
    kind: &'static str,
    row_id: String,
    row_hash: String,
    pack_id: String,
    pack_hash: String,
    manifest_path: String,
    channel: &'static str,
    passed: bool,
    blockers: Vec<&'static str>,
}

impl CheckJsonReport {
    fn new(
        report: &nose_semantics::SemanticPackConformanceReport,
        influence_preflight: &nose_semantics::ExternalInfluencePreflightReport,
    ) -> Self {
        Self {
            schema_version: CONFORMANCE_SCHEMA_VERSION,
            status: if report.passed() { "ok" } else { "failed" },
            totals: CheckJsonTotals {
                manifests: report.manifest_count(),
                positive_fixtures: report.positive_fixture_count(),
                hard_negatives: report.hard_negative_count(),
                fixture_issues: report.fixture_issue_count(),
                executable_conformance_rows: report.executable_conformance_count(),
                passed_executable_conformance_rows: report.passed_executable_conformance_count(),
                executable_conformance_issues: report.executable_conformance_issue_count(),
                influence_rows: influence_preflight.rows.len(),
                blocked_influence_rows: influence_preflight.blocked_count(),
            },
            executable_conformance: CheckJsonExecutableConformance::new(report),
            influence_preflight: CheckJsonInfluencePreflight::new(influence_preflight),
            manifests: report
                .manifests
                .iter()
                .map(|manifest| CheckJsonManifest {
                    id: manifest.pack.id.clone(),
                    version: manifest.pack.version.clone(),
                    display_name: manifest.pack.display_name.clone(),
                    trust: manifest.pack.trust.as_manifest_str(),
                    source: manifest.pack.source.as_str(),
                    influence: manifest.pack.influence.as_str(),
                    manifest_path: manifest.manifest_path.display().to_string(),
                    provider: manifest.pack.provider.clone(),
                    repository: manifest.pack.repository.clone(),
                    license: manifest.pack.license.clone(),
                    supported_languages: manifest.pack.supported_languages.clone(),
                    counts: CheckJsonCounts {
                        evidence_producers: manifest.pack.counts.evidence_producers,
                        contracts: manifest.pack.counts.contracts,
                        value_laws: manifest.pack.counts.value_laws,
                        positive_fixtures: manifest.pack.counts.positive_fixtures,
                        hard_negatives: manifest.pack.counts.hard_negatives,
                    },
                    conformance_command: manifest.conformance_command.clone(),
                    proof_links: manifest.proof_links.clone(),
                    fixture_issues: manifest.fixture_issue_count(),
                    fixtures: manifest
                        .fixtures
                        .iter()
                        .map(|fixture| CheckJsonFixture {
                            kind: fixture.kind.as_str(),
                            id: fixture.id.clone(),
                            description: fixture.description.clone(),
                            declared_path: fixture.declared_path.clone(),
                            resolved_path: fixture
                                .resolved_path
                                .as_ref()
                                .map(|path| path.display().to_string()),
                            expectation: fixture.expectation.clone(),
                            issues: fixture.issues.iter().map(|issue| issue.as_str()).collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl CheckJsonExecutableConformance {
    fn new(report: &nose_semantics::SemanticPackConformanceReport) -> Self {
        let rows = report
            .manifests
            .iter()
            .flat_map(|manifest| &manifest.executable)
            .map(|gate| CheckJsonExecutableConformanceRow {
                gate_id: gate.gate_id.clone(),
                kind: gate.kind.as_str(),
                row_id: gate.row_id.clone(),
                row_hash: format!("{:016x}", gate.row_hash),
                pack_id: gate.pack_id.clone(),
                pack_hash: format!("{:016x}", gate.pack_hash),
                manifest_path: gate.manifest_path.display().to_string(),
                channel: gate.channel.as_str(),
                oracle: gate.oracle.as_str(),
                passed: gate.passed(),
                positive_fixtures: gate.positive_fixtures.clone(),
                hard_negatives: gate.hard_negatives.clone(),
                issues: gate.issues.iter().map(|issue| issue.as_str()).collect(),
            })
            .collect::<Vec<_>>();
        let status = if rows.is_empty() {
            "unavailable"
        } else if report.executable_conformance_issue_count() == 0 {
            "ok"
        } else {
            "failed"
        };
        Self { status, rows }
    }
}

impl CheckJsonInfluencePreflight {
    fn new(report: &nose_semantics::ExternalInfluencePreflightReport) -> Self {
        Self {
            status: if report.passed() { "ok" } else { "blocked" },
            rows: report
                .rows
                .iter()
                .map(|row| CheckJsonInfluenceRow {
                    kind: row.kind.as_str(),
                    row_id: row.row_id.clone(),
                    row_hash: format!("{:016x}", row.row_hash),
                    pack_id: row.pack_id.clone(),
                    pack_hash: format!("{:016x}", row.pack_hash),
                    manifest_path: row.manifest_path.display().to_string(),
                    channel: row.channel.as_str(),
                    passed: row.passed(),
                    blockers: row
                        .blockers
                        .iter()
                        .map(|blocker| blocker.as_str())
                        .collect(),
                })
                .collect(),
        }
    }
}

pub(crate) fn cmd_check(paths: Vec<PathBuf>, format: CheckFormat) -> Result<()> {
    let report = nose_semantics::check_semantic_pack_conformance(&paths)?;
    match format {
        CheckFormat::Human => print_human(&report),
        CheckFormat::Json => {
            let influence_preflight = nose_semantics::SemanticPackSet::new_local(&paths)?
                .external_influence_preflight_with_conformance(&report);
            println!(
                "{}",
                serde_json::to_string_pretty(&CheckJsonReport::new(&report, &influence_preflight))?
            );
        }
    }
    if !report.passed() {
        anyhow::bail!(
            "semantic pack conformance failed: {} fixture issue(s), {} executable gate issue(s)",
            report.fixture_issue_count(),
            report.executable_conformance_issue_count()
        );
    }
    Ok(())
}

fn print_human(report: &nose_semantics::SemanticPackConformanceReport) {
    println!(
        "semantic pack conformance: {}",
        if report.passed() { "ok" } else { "failed" }
    );
    println!(
        "manifests: {}; fixtures: {} positive, {} hard-negative; fixture issues: {}",
        report.manifest_count(),
        report.positive_fixture_count(),
        report.hard_negative_count(),
        report.fixture_issue_count()
    );
    println!(
        "checks: schema/ref/trust/version ok; fixture files {}",
        if report.fixture_issue_count() == 0 {
            "ok"
        } else {
            "failed"
        }
    );
    println!(
        "executable gates: {} passed / {} declared; issues: {}",
        report.passed_executable_conformance_count(),
        report.executable_conformance_count(),
        report.executable_conformance_issue_count()
    );
    for manifest in &report.manifests {
        println!(
            "  {}@{}: {} fixture issue(s) ({})",
            manifest.pack.id,
            manifest.pack.version,
            manifest.fixture_issue_count(),
            manifest.manifest_path.display()
        );
        if let Some(command) = &manifest.conformance_command {
            println!("    command: {command}");
        }
        for fixture in &manifest.fixtures {
            let issue_text = if fixture.issues.is_empty() {
                "ok".to_string()
            } else {
                fixture
                    .issues
                    .iter()
                    .map(|issue| issue.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let path_text = fixture
                .declared_path
                .as_deref()
                .unwrap_or("<no fixture path>");
            println!(
                "    {} {}: {} ({})",
                fixture.kind.as_str(),
                fixture.id,
                issue_text,
                path_text
            );
        }
        for gate in &manifest.executable {
            let issue_text = if gate.issues.is_empty() {
                "ok".to_string()
            } else {
                gate.issues
                    .iter()
                    .map(|issue| issue.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!(
                "    executable {} {}: {} ({})",
                gate.kind.as_str(),
                gate.row_id,
                issue_text,
                gate.oracle.as_str()
            );
        }
    }
}
