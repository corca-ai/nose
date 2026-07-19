//! Kernel conformance receipts for the external-claim exact lane.

use super::{
    CompiledSemanticPackV1, SemanticPackV1Channel, SemanticPackV1Expectation,
    SemanticPackV1FixtureKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const SEMANTIC_PACK_RECEIPT_API_VERSION_V1: &str = "nose.semantic-pack-conformance-receipt.v1";
pub const SEMANTIC_PACK_EXACT_KERNEL_CAPABILITY_V1: &str =
    "nose.kernel.external-collection-factory-exact.v1";
pub const MAX_SEMANTIC_PACK_FIXTURE_FILES: usize = 64;
pub const MAX_SEMANTIC_PACK_FIXTURE_BYTES: usize = 1024 * 1024;
pub const MAX_SEMANTIC_PACK_DEPENDENCY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackConformanceReceiptV1 {
    pub api_version: String,
    pub nose_version: String,
    pub kernel_capability: String,
    pub pack_id: String,
    pub pack_version: String,
    pub semantic_digest: String,
    pub rows: Vec<SemanticPackConformanceReceiptRow>,
    pub fixtures: Vec<SemanticPackConformanceReceiptFixture>,
    pub passed: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackConformanceReceiptRow {
    pub row_id: String,
    pub row_digest: String,
    pub channel: SemanticPackV1Channel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ObservedExpectation {
    ExternalExactMatch,
    NoExternalExactMatch,
    ResourceLimit,
    AnalysisFailure,
}

impl SemanticPackV1ObservedExpectation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExternalExactMatch => "external-exact-match",
            Self::NoExternalExactMatch => "no-external-exact-match",
            Self::ResourceLimit => "resource-limit",
            Self::AnalysisFailure => "analysis-failure",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackConformanceReceiptFixture {
    pub id: String,
    pub row_id: String,
    pub kind: SemanticPackV1FixtureKind,
    pub path: String,
    pub dependency: String,
    pub fixture_digest: String,
    pub dependency_digest: String,
    pub expectation: SemanticPackV1Expectation,
    pub observed: SemanticPackV1ObservedExpectation,
    pub passed: bool,
}

pub fn read_semantic_pack_conformance_receipt(
    path: &Path,
) -> Result<SemanticPackConformanceReceiptV1, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "reading exact conformance receipt {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parsing exact conformance receipt {}: {error}",
            path.display()
        )
    })
}

pub fn validate_semantic_pack_conformance_receipt(
    receipt: &SemanticPackConformanceReceiptV1,
    pack: &CompiledSemanticPackV1,
    manifest_path: &Path,
    selected_exact_rows: &[String],
) -> Result<(), String> {
    if receipt.api_version != SEMANTIC_PACK_RECEIPT_API_VERSION_V1
        || receipt.nose_version != env!("CARGO_PKG_VERSION")
        || receipt.kernel_capability != SEMANTIC_PACK_EXACT_KERNEL_CAPABILITY_V1
        || receipt.pack_id != pack.pack_id()
        || receipt.pack_version != pack.pack_version()
        || receipt.semantic_digest != pack.semantic_digest()
        || !receipt.passed
    {
        return Err("receipt identity, kernel capability, or pass status is stale".to_string());
    }
    let mut expected_rows = selected_exact_rows.to_vec();
    expected_rows.sort();
    expected_rows.dedup();
    let mut actual_rows = receipt.rows.clone();
    actual_rows.sort_by(|left, right| left.row_id.cmp(&right.row_id));
    if actual_rows.len() != expected_rows.len() {
        return Err("receipt rows do not match the selected external-exact rows".to_string());
    }
    for (actual, row_id) in actual_rows.iter().zip(&expected_rows) {
        if actual.row_id != *row_id
            || actual.channel != SemanticPackV1Channel::ExternalExact
            || pack.row_digest(row_id) != Some(actual.row_digest.as_str())
        {
            return Err(format!("receipt row `{row_id}` is stale or mismatched"));
        }
    }
    validate_receipt_fixtures(receipt, pack, manifest_path, &expected_rows)
}

fn validate_receipt_fixtures(
    receipt: &SemanticPackConformanceReceiptV1,
    pack: &CompiledSemanticPackV1,
    manifest_path: &Path,
    selected_rows: &[String],
) -> Result<(), String> {
    let root = manifest_path
        .parent()
        .ok_or_else(|| "semantic-pack manifest has no parent directory".to_string())?;
    let expected = pack
        .conformance_fixtures()
        .iter()
        .filter(|fixture| selected_rows.binary_search(&fixture.row_id).is_ok())
        .collect::<Vec<_>>();
    if receipt.fixtures.len() != expected.len() {
        return Err("receipt fixtures do not match the manifest".to_string());
    }
    for fixture in expected {
        let actual = receipt
            .fixtures
            .iter()
            .find(|actual| actual.id == fixture.id)
            .ok_or_else(|| format!("receipt is missing fixture `{}`", fixture.id))?;
        let observed_matches = matches!(
            (actual.expectation, actual.observed),
            (
                SemanticPackV1Expectation::ExternalExactMatch,
                SemanticPackV1ObservedExpectation::ExternalExactMatch
            ) | (
                SemanticPackV1Expectation::NoExternalExactMatch,
                SemanticPackV1ObservedExpectation::NoExternalExactMatch
            )
        );
        if actual.row_id != fixture.row_id
            || actual.kind != fixture.kind
            || actual.path != fixture.path
            || actual.dependency != fixture.dependency
            || actual.expectation != fixture.expectation
            || !actual.passed
            || !observed_matches
        {
            return Err(format!(
                "receipt fixture `{}` is stale or mismatched",
                fixture.id
            ));
        }
        let fixture_path = resolve_fixture_path(root, &fixture.path)?;
        let dependency_path = resolve_fixture_path(root, &fixture.dependency)?;
        if semantic_pack_fixture_digest(&fixture_path)? != actual.fixture_digest
            || semantic_pack_file_digest(&dependency_path)? != actual.dependency_digest
        {
            return Err(format!("receipt fixture `{}` content changed", fixture.id));
        }
    }
    Ok(())
}

pub fn resolve_fixture_path(root: &Path, declared: &str) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("resolving semantic-pack root {}: {error}", root.display()))?;
    reject_symlink_components(&root, declared)?;
    let path = root.join(declared);
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("resolving conformance path {}: {error}", path.display()))?;
    if !resolved.starts_with(&root) {
        return Err(format!(
            "conformance path `{declared}` escapes the pack root"
        ));
    }
    Ok(resolved)
}

fn reject_symlink_components(root: &Path, declared: &str) -> Result<(), String> {
    let mut path = root.to_path_buf();
    for component in Path::new(declared).components() {
        use std::path::Component;
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => path.push(part),
            _ => return Err(format!("conformance path `{declared}` is not relative")),
        }
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("reading conformance path {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "conformance path `{declared}` must not contain symlinks"
            ));
        }
    }
    Ok(())
}

pub fn semantic_pack_file_digest(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("reading conformance file {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "conformance dependency {} is not a regular file",
            path.display()
        ));
    }
    if metadata.len() > MAX_SEMANTIC_PACK_DEPENDENCY_BYTES as u64 {
        return Err(format!(
            "dependency exceeds {MAX_SEMANTIC_PACK_DEPENDENCY_BYTES} bytes"
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("reading conformance file {}: {error}", path.display()))?;
    Ok(digest(&bytes))
}

pub fn semantic_pack_fixture_digest(path: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    if files.is_empty() || files.len() > MAX_SEMANTIC_PACK_FIXTURE_FILES {
        return Err(format!(
            "fixture must contain 1..={MAX_SEMANTIC_PACK_FIXTURE_FILES} regular files"
        ));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut total = 0usize;
    let mut hasher = Sha256::new();
    for (relative, file) in files {
        let bytes = std::fs::read(&file)
            .map_err(|error| format!("reading fixture file {}: {error}", file.display()))?;
        total = total.saturating_add(bytes.len());
        if total > MAX_SEMANTIC_PACK_FIXTURE_BYTES {
            return Err(format!(
                "fixture exceeds {MAX_SEMANTIC_PACK_FIXTURE_BYTES} bytes"
            ));
        }
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format_digest(hasher.finalize()))
}

fn collect_files(root: &Path, path: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reading fixture metadata {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "fixture path {} must not contain symlinks",
            path.display()
        ));
    }
    if metadata.is_file() {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((relative, path.to_path_buf()));
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!(
            "fixture path {} is not a file or directory",
            path.display()
        ));
    }
    let mut entries = std::fs::read_dir(path)
        .map_err(|error| format!("reading fixture directory {}: {error}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("reading fixture directory {}: {error}", path.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if out.len() >= MAX_SEMANTIC_PACK_FIXTURE_FILES {
            return Err(format!(
                "fixture exceeds {MAX_SEMANTIC_PACK_FIXTURE_FILES} files"
            ));
        }
        collect_files(root, &entry.path(), out)?;
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format_digest(Sha256::digest(bytes))
}

fn format_digest(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("sha256:{hex}")
}
