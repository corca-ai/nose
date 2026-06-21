use super::*;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use super::compiled::is_compiled_builtin_pack_id;

pub(super) struct LoadedLocalManifest {
    pub summary: SemanticPackSummary,
    pub external_evidence_producer_rows: Vec<ExternalEvidenceProducerRow>,
    pub external_contract_rows: Vec<ExternalContractRow>,
    pub external_value_law_rows: Vec<ExternalValueLawRow>,
}

pub fn check_semantic_pack_conformance(
    paths: &[PathBuf],
) -> Result<SemanticPackConformanceReport, SemanticPackLoadError> {
    let manifest_paths = discover_manifest_paths(paths)?;
    let mut manifests: Vec<SemanticPackConformanceManifest> = Vec::new();
    for path in manifest_paths {
        let manifest = read_local_manifest(&path)?;
        let conformance_command = manifest.conformance.command.clone();
        let proof_links = manifest.conformance.proofs.clone();
        let fixtures = collect_fixture_checks(&path, &manifest);
        let executable = collect_executable_conformance_checks(&path, &manifest, &fixtures);
        let pack =
            SemanticPackSummary::from_manifest(path.clone(), manifest).map_err(|message| {
                SemanticPackLoadError::InvalidManifest {
                    path: path.clone(),
                    message,
                }
            })?;
        if is_compiled_builtin_pack_id(&pack.id) {
            return Err(SemanticPackLoadError::DuplicatePackId {
                id: pack.id,
                first_path: None,
                second_path: Some(path),
            });
        }
        if let Some(existing) = manifests
            .iter()
            .find(|existing| existing.pack.id == pack.id)
        {
            return Err(SemanticPackLoadError::DuplicatePackId {
                id: pack.id,
                first_path: Some(existing.manifest_path.clone()),
                second_path: Some(path),
            });
        }
        manifests.push(SemanticPackConformanceManifest {
            pack,
            manifest_path: path,
            conformance_command,
            proof_links,
            fixtures,
            executable,
        });
    }
    Ok(SemanticPackConformanceReport { manifests })
}

pub fn discover_manifest_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SemanticPackLoadError> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if path.is_file() {
            push_unique_manifest(path, &mut seen, &mut out)?;
        } else if path.is_dir() {
            discover_manifest_directory(path, &mut seen, &mut out)?;
        } else {
            return Err(SemanticPackLoadError::NotFound { path: path.clone() });
        }
    }
    Ok(out)
}

fn discover_manifest_directory(
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    out: &mut Vec<PathBuf>,
) -> Result<(), SemanticPackLoadError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|source| SemanticPackLoadError::Io {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SemanticPackLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        if entry_path.extension().is_some_and(|ext| ext == "json") {
            entries.push(entry_path);
        }
    }
    entries.sort();
    if entries.is_empty() {
        return Err(SemanticPackLoadError::DirectoryHasNoManifests {
            path: path.to_path_buf(),
        });
    }
    for entry in entries {
        push_unique_manifest(&entry, seen, out)?;
    }
    Ok(())
}

fn push_unique_manifest(
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    out: &mut Vec<PathBuf>,
) -> Result<(), SemanticPackLoadError> {
    let canonical = path
        .canonicalize()
        .map_err(|source| SemanticPackLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if seen.insert(canonical.clone()) {
        out.push(canonical);
    }
    Ok(())
}

pub fn load_local_manifest(path: &Path) -> Result<SemanticPackSummary, SemanticPackLoadError> {
    let manifest = read_local_manifest(path)?;
    SemanticPackSummary::from_manifest(path.to_path_buf(), manifest).map_err(|message| {
        SemanticPackLoadError::InvalidManifest {
            path: path.to_path_buf(),
            message,
        }
    })
}

pub(super) fn load_local_manifest_with_rows(
    path: &Path,
) -> Result<LoadedLocalManifest, SemanticPackLoadError> {
    let manifest = read_local_manifest(path)?;
    let external_evidence_producer_rows = manifest
        .declares
        .evidence_producers
        .iter()
        .map(|producer| ExternalEvidenceProducerRow::from_manifest(path, &manifest, producer))
        .collect();
    let external_contract_rows = manifest
        .declares
        .contracts
        .iter()
        .map(|contract| ExternalContractRow::from_manifest(path, &manifest, contract))
        .collect();
    let external_value_law_rows = manifest
        .declares
        .value_laws
        .iter()
        .map(|law| ExternalValueLawRow::from_manifest(path, &manifest, law))
        .collect();
    let summary =
        SemanticPackSummary::from_manifest(path.to_path_buf(), manifest).map_err(|message| {
            SemanticPackLoadError::InvalidManifest {
                path: path.to_path_buf(),
                message,
            }
        })?;
    Ok(LoadedLocalManifest {
        summary,
        external_evidence_producer_rows,
        external_contract_rows,
        external_value_law_rows,
    })
}

fn read_local_manifest(path: &Path) -> Result<SemanticPackManifest, SemanticPackLoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| SemanticPackLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str::<SemanticPackManifest>(&text).map_err(|source| {
        SemanticPackLoadError::Json {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn collect_fixture_checks(
    manifest_path: &Path,
    manifest: &SemanticPackManifest,
) -> Vec<SemanticPackFixtureCheck> {
    let mut checks = Vec::new();
    for fixture in &manifest.conformance.positive_fixtures {
        checks.push(fixture_check(
            manifest_path,
            SemanticPackFixtureKind::Positive,
            fixture,
        ));
    }
    for fixture in &manifest.conformance.hard_negatives {
        checks.push(fixture_check(
            manifest_path,
            SemanticPackFixtureKind::HardNegative,
            fixture,
        ));
    }
    checks
}

fn fixture_check(
    manifest_path: &Path,
    kind: SemanticPackFixtureKind,
    fixture: &ManifestFixture,
) -> SemanticPackFixtureCheck {
    let resolved_path = fixture
        .path
        .as_deref()
        .map(|path| resolve_fixture_path(manifest_path, path));
    let mut issues = Vec::new();
    if fixture.path.is_none() {
        issues.push(SemanticPackFixtureIssue::MissingPath);
    } else if let Some(declared_path) = fixture.path.as_deref() {
        if Path::new(declared_path).is_absolute() {
            issues.push(SemanticPackFixtureIssue::AbsolutePath);
        }
        if !resolved_path.as_ref().is_some_and(|path| path.is_file()) {
            issues.push(SemanticPackFixtureIssue::MissingFile);
        }
    }
    if fixture.expectation.is_none() {
        issues.push(SemanticPackFixtureIssue::MissingExpectation);
    }
    SemanticPackFixtureCheck {
        kind,
        id: fixture.id.clone(),
        description: fixture.description.clone(),
        declared_path: fixture.path.clone(),
        resolved_path,
        expectation: fixture.expectation.clone(),
        issues,
    }
}

fn resolve_fixture_path(manifest_path: &Path, declared_path: &str) -> PathBuf {
    let path = Path::new(declared_path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

fn collect_executable_conformance_checks(
    manifest_path: &Path,
    manifest: &SemanticPackManifest,
    fixtures: &[SemanticPackFixtureCheck],
) -> Vec<SemanticPackExecutableConformanceCheck> {
    manifest
        .conformance
        .executable
        .iter()
        .filter_map(|gate| {
            executable_row(manifest, &gate.row_ref).map(|(kind, channel)| {
                executable_conformance_check(manifest_path, manifest, gate, kind, channel, fixtures)
            })
        })
        .collect()
}

fn executable_row(
    manifest: &SemanticPackManifest,
    row_ref: &str,
) -> Option<(ExternalRowKind, SemanticPackChannel)> {
    manifest
        .declares
        .evidence_producers
        .iter()
        .find(|producer| producer.id == row_ref)
        .map(|producer| (ExternalRowKind::EvidenceProducer, producer.channel))
        .or_else(|| {
            manifest
                .declares
                .contracts
                .iter()
                .find(|contract| contract.id == row_ref)
                .map(|contract| (ExternalRowKind::Contract, contract.channel))
        })
        .or_else(|| {
            manifest
                .declares
                .value_laws
                .iter()
                .find(|law| law.id == row_ref)
                .map(|law| (ExternalRowKind::ValueLaw, law.channel))
        })
}

fn executable_conformance_check(
    manifest_path: &Path,
    manifest: &SemanticPackManifest,
    gate: &ManifestExecutableConformanceGate,
    kind: ExternalRowKind,
    channel: SemanticPackChannel,
    fixtures: &[SemanticPackFixtureCheck],
) -> SemanticPackExecutableConformanceCheck {
    let mut issues = Vec::new();
    match gate.oracle {
        SemanticPackExecutableOracle::FixtureExpectations => {
            collect_fixture_expectation_issues(
                fixtures,
                &gate.positive_fixtures,
                SemanticPackFixtureKind::Positive,
                &gate.expected_positive,
                &mut issues,
            );
            collect_fixture_expectation_issues(
                fixtures,
                &gate.hard_negatives,
                SemanticPackFixtureKind::HardNegative,
                &gate.expected_hard_negative,
                &mut issues,
            );
        }
    }
    SemanticPackExecutableConformanceCheck {
        gate_id: gate.id.clone(),
        kind,
        row_id: gate.row_ref.clone(),
        row_hash: stable_symbol_hash(&gate.row_ref),
        pack_id: manifest.pack.id.clone(),
        pack_hash: semantic_pack_hash(&manifest.pack.id),
        manifest_path: manifest_path.to_path_buf(),
        channel,
        oracle: gate.oracle,
        positive_fixtures: gate.positive_fixtures.clone(),
        hard_negatives: gate.hard_negatives.clone(),
        issues,
    }
}

fn collect_fixture_expectation_issues(
    fixtures: &[SemanticPackFixtureCheck],
    ids: &[String],
    expected_kind: SemanticPackFixtureKind,
    expected_label: &str,
    issues: &mut Vec<SemanticPackExecutableConformanceIssue>,
) {
    for id in ids {
        let Some(fixture) = fixtures.iter().find(|fixture| fixture.id == *id) else {
            issues.push(SemanticPackExecutableConformanceIssue::UnknownFixture);
            continue;
        };
        if fixture.kind != expected_kind {
            issues.push(SemanticPackExecutableConformanceIssue::WrongFixtureKind);
            continue;
        }
        if !fixture.passed() {
            issues.push(SemanticPackExecutableConformanceIssue::FixtureIssue);
        }
        match fixture.expectation.as_deref() {
            Some(actual) if actual == expected_label => {}
            Some(_) => issues.push(SemanticPackExecutableConformanceIssue::ExpectationMismatch),
            None => issues.push(SemanticPackExecutableConformanceIssue::MissingExpectation),
        }
    }
}

#[derive(Debug)]
pub enum SemanticPackLoadError {
    NotFound {
        path: PathBuf,
    },
    DirectoryHasNoManifests {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidManifest {
        path: PathBuf,
        message: String,
    },
    DuplicatePackId {
        id: String,
        first_path: Option<PathBuf>,
        second_path: Option<PathBuf>,
    },
}

impl fmt::Display for SemanticPackLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemanticPackLoadError::NotFound { path } => {
                write!(f, "semantic pack path not found: {}", path.display())
            }
            SemanticPackLoadError::DirectoryHasNoManifests { path } => write!(
                f,
                "semantic pack directory contains no JSON manifests: {}",
                path.display()
            ),
            SemanticPackLoadError::Io { path, source } => {
                write!(f, "reading semantic pack {}: {source}", path.display())
            }
            SemanticPackLoadError::Json { path, source } => {
                write!(f, "parsing semantic pack {}: {source}", path.display())
            }
            SemanticPackLoadError::InvalidManifest { path, message } => {
                write!(f, "invalid semantic pack {}: {message}", path.display())
            }
            SemanticPackLoadError::DuplicatePackId {
                id,
                first_path,
                second_path,
            } => write!(
                f,
                "duplicate semantic pack id `{id}` between {} and {}",
                display_optional_path(first_path),
                display_optional_path(second_path)
            ),
        }
    }
}

impl std::error::Error for SemanticPackLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SemanticPackLoadError::Io { source, .. } => Some(source),
            SemanticPackLoadError::Json { source, .. } => Some(source),
            SemanticPackLoadError::NotFound { .. }
            | SemanticPackLoadError::DirectoryHasNoManifests { .. }
            | SemanticPackLoadError::InvalidManifest { .. }
            | SemanticPackLoadError::DuplicatePackId { .. } => None,
        }
    }
}

fn display_optional_path(path: &Option<PathBuf>) -> String {
    path.as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<compiled builtin>".to_string())
}
