//! Semantic pack manifest loading and provenance summaries.
//!
//! Loading a manifest is an explicit opt-in metadata path. External packs do not
//! emit evidence or approve exact results from this module; consumers must still
//! require occurrence evidence and kernel-owned admission checks.

use crate::{PackTrust, FIRST_PARTY_PACK_ID};
use nose_il::stable_symbol_hash;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

pub const SEMANTIC_PACK_API_VERSION: &str = "nose.semantic-pack.v0";

const ALLOWED_REQUIREMENT_PREFIXES: &[&str] = &[
    "Source.",
    "Symbol.",
    "Import.",
    "Domain.",
    "Type.",
    "Guard.",
    "Place.",
    "Effect.",
    "LibraryApi.",
    "CallTarget.",
    "SequenceSurface.",
    "nose.",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackSource {
    CompiledFirstParty,
    LocalManifest,
}

impl SemanticPackSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackSource::CompiledFirstParty => "compiled-first-party",
            SemanticPackSource::LocalManifest => "local-manifest",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackInfluence {
    EvidenceAndContracts,
    MetadataOnly,
}

impl SemanticPackInfluence {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackInfluence::EvidenceAndContracts => "evidence-and-contracts",
            SemanticPackInfluence::MetadataOnly => "metadata-only",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum SemanticPackKind {
    LanguagePack,
    StdlibPack,
    LibraryPack,
    ProtocolPack,
    LawPack,
}

impl SemanticPackKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackKind::LanguagePack => "LanguagePack",
            SemanticPackKind::StdlibPack => "StdlibPack",
            SemanticPackKind::LibraryPack => "LibraryPack",
            SemanticPackKind::ProtocolPack => "ProtocolPack",
            SemanticPackKind::LawPack => "LawPack",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SemanticPackAnchor {
    SourceSpan,
    Node,
    Param,
    Binding,
    Sequence,
    Module,
    Package,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackChannel {
    SyntaxOnly,
    NearOnly,
    AbstractionWitness,
    ExactEmpirical,
    ExactProven,
}

impl SemanticPackChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackChannel::SyntaxOnly => "syntax-only",
            SemanticPackChannel::NearOnly => "near-only",
            SemanticPackChannel::AbstractionWitness => "abstraction-witness",
            SemanticPackChannel::ExactEmpirical => "exact-empirical",
            SemanticPackChannel::ExactProven => "exact-proven",
        }
    }

    pub const fn exact_capable(self) -> bool {
        matches!(
            self,
            SemanticPackChannel::ExactEmpirical | SemanticPackChannel::ExactProven
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackProofStatus {
    Proven,
    Covered,
    Missing,
    EmpiricalOnly,
    RejectedCounterexample,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SemanticPackStatus {
    DraftExample,
    Experimental,
    Stable,
    Deprecated,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SemanticPackSchemaVersion {
    V0,
}

impl PackTrust {
    pub const fn as_manifest_str(self) -> &'static str {
        match self {
            PackTrust::DefaultFirstParty => "default-first-party",
            PackTrust::FirstPartyOptional => "first-party-optional",
            PackTrust::ExternalOptIn => "external-opt-in",
        }
    }
}

impl<'de> Deserialize<'de> for PackTrust {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "default-first-party" => Ok(PackTrust::DefaultFirstParty),
            "first-party-optional" => Ok(PackTrust::FirstPartyOptional),
            "external-opt-in" => Ok(PackTrust::ExternalOptIn),
            other => Err(serde::de::Error::custom(format!(
                "unknown pack trust `{other}`"
            ))),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackCounts {
    pub evidence_producers: usize,
    pub contracts: usize,
    pub value_laws: usize,
    pub positive_fixtures: usize,
    pub hard_negatives: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackSummary {
    pub id: String,
    pub hash: u64,
    pub kind: SemanticPackKind,
    pub version: String,
    pub display_name: String,
    pub trust: PackTrust,
    pub enabled_by_default: bool,
    pub source: SemanticPackSource,
    pub influence: SemanticPackInfluence,
    pub manifest_path: Option<PathBuf>,
    pub provider: String,
    pub repository: String,
    pub license: String,
    pub supported_languages: Vec<String>,
    pub counts: SemanticPackCounts,
}

impl SemanticPackSummary {
    pub fn hash_hex(&self) -> String {
        format!("{:016x}", self.hash)
    }

    fn from_manifest(path: PathBuf, manifest: SemanticPackManifest) -> Result<Self, String> {
        validate_manifest(&manifest)?;
        let id = manifest.pack.id;
        let supported_languages = manifest
            .supported_languages
            .into_iter()
            .map(|language| language.id)
            .collect();
        let counts = SemanticPackCounts {
            evidence_producers: manifest.declares.evidence_producers.len(),
            contracts: manifest.declares.contracts.len(),
            value_laws: manifest.declares.value_laws.len(),
            positive_fixtures: manifest.conformance.positive_fixtures.len(),
            hard_negatives: manifest.conformance.hard_negatives.len(),
        };
        Ok(Self {
            hash: semantic_pack_hash(&id),
            id,
            kind: manifest.pack.kind,
            version: manifest.pack.version,
            display_name: manifest.pack.display_name,
            trust: manifest.pack.trust,
            enabled_by_default: manifest.pack.enabled_by_default,
            source: SemanticPackSource::LocalManifest,
            influence: SemanticPackInfluence::MetadataOnly,
            manifest_path: Some(path),
            provider: manifest.provenance.provider.name,
            repository: manifest.provenance.repository,
            license: manifest.provenance.license,
            supported_languages,
            counts,
        })
    }
}

pub fn semantic_pack_hash(pack_id: &str) -> u64 {
    stable_symbol_hash(pack_id)
}

pub fn first_party_semantic_pack() -> SemanticPackSummary {
    SemanticPackSummary {
        id: FIRST_PARTY_PACK_ID.to_string(),
        hash: semantic_pack_hash(FIRST_PARTY_PACK_ID),
        kind: SemanticPackKind::LanguagePack,
        version: env!("CARGO_PKG_VERSION").to_string(),
        display_name: "nose first-party semantic kernel".to_string(),
        trust: PackTrust::DefaultFirstParty,
        enabled_by_default: true,
        source: SemanticPackSource::CompiledFirstParty,
        influence: SemanticPackInfluence::EvidenceAndContracts,
        manifest_path: None,
        provider: "Corca, Inc.".to_string(),
        repository: "https://github.com/corca-ai/nose".to_string(),
        license: "MIT".to_string(),
        supported_languages: Vec::new(),
        counts: SemanticPackCounts {
            evidence_producers: 0,
            contracts: 0,
            value_laws: 0,
            positive_fixtures: 0,
            hard_negatives: 0,
        },
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackSet {
    packs: Vec<SemanticPackSummary>,
}

impl SemanticPackSet {
    pub fn new_local(paths: &[PathBuf]) -> Result<Self, SemanticPackLoadError> {
        let manifest_paths = discover_manifest_paths(paths)?;
        let mut packs = vec![first_party_semantic_pack()];
        for path in manifest_paths {
            let pack = load_local_manifest(&path)?;
            if let Some(existing) = packs.iter().find(|existing| existing.id == pack.id) {
                return Err(SemanticPackLoadError::DuplicatePackId {
                    id: pack.id,
                    first_path: existing.manifest_path.clone(),
                    second_path: Some(path),
                });
            }
            packs.push(pack);
        }
        Ok(Self { packs })
    }

    pub fn first_party_only() -> Self {
        Self {
            packs: vec![first_party_semantic_pack()],
        }
    }

    pub fn packs(&self) -> &[SemanticPackSummary] {
        &self.packs
    }
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
    let text = std::fs::read_to_string(path).map_err(|source| SemanticPackLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest = serde_json::from_str::<SemanticPackManifest>(&text).map_err(|source| {
        SemanticPackLoadError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    SemanticPackSummary::from_manifest(path.to_path_buf(), manifest).map_err(|message| {
        SemanticPackLoadError::InvalidManifest {
            path: path.to_path_buf(),
            message,
        }
    })
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
        .unwrap_or_else(|| "<compiled first-party>".to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticPackManifest {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    api_version: String,
    pack: ManifestPack,
    provenance: ManifestProvenance,
    compatibility: ManifestCompatibility,
    supported_languages: Vec<ManifestLanguage>,
    #[serde(default)]
    packages: Vec<ManifestPackage>,
    #[serde(default)]
    dependencies: Vec<ManifestDependency>,
    declares: ManifestDeclares,
    conformance: ManifestConformance,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestPack {
    id: String,
    kind: SemanticPackKind,
    version: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
    trust: PackTrust,
    enabled_by_default: bool,
    #[serde(default, rename = "status")]
    _status: Option<SemanticPackStatus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProvenance {
    provider: ManifestProvider,
    license: String,
    repository: String,
    #[serde(default)]
    source_revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProvider {
    name: String,
    #[serde(default)]
    contact: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCompatibility {
    nose: String,
    #[serde(default, rename = "schema")]
    _schema: Option<SemanticPackSchemaVersion>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestLanguage {
    id: String,
    #[serde(default)]
    language_version: Option<String>,
    #[serde(default)]
    runtime: Option<String>,
    #[serde(default)]
    runtime_versions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDependency {
    id: String,
    version: String,
    required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestPackage {
    ecosystem: String,
    name: String,
    versions: String,
    #[serde(default, rename = "stdlib")]
    _stdlib: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDeclares {
    evidence_producers: Vec<ManifestEvidenceProducer>,
    contracts: Vec<ManifestContract>,
    #[serde(default)]
    value_laws: Vec<ManifestValueLaw>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEvidenceProducer {
    id: String,
    kind: String,
    anchors: Vec<SemanticPackAnchor>,
    channel: SemanticPackChannel,
    #[serde(default)]
    emits: Vec<String>,
    #[serde(default)]
    requires: Vec<ManifestRequirement>,
    stable_hash_inputs: Vec<String>,
    conflict_policy: String,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestContract {
    id: String,
    surface: serde_json::Value,
    requires: Vec<ManifestRequirement>,
    semantics: serde_json::Value,
    channel: SemanticPackChannel,
    proof_status: SemanticPackProofStatus,
    conformance_refs: Vec<String>,
    #[serde(default)]
    known_unsupported: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestValueLaw {
    id: String,
    requires: Vec<ManifestRequirement>,
    semantics: serde_json::Value,
    channel: SemanticPackChannel,
    proof_status: SemanticPackProofStatus,
    conformance_refs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestRequirement {
    #[serde(rename = "ref")]
    ref_id: String,
    subject: String,
    required: bool,
    #[serde(default)]
    same_anchor_as: Option<String>,
    #[serde(default)]
    within_scope: Option<String>,
    #[serde(default)]
    before: Option<String>,
    #[serde(default)]
    after: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestConformance {
    positive_fixtures: Vec<ManifestFixture>,
    hard_negatives: Vec<ManifestFixture>,
    known_unsupported: Vec<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    proofs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFixture {
    id: String,
    description: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    expectation: Option<String>,
}

fn validate_manifest(manifest: &SemanticPackManifest) -> Result<(), String> {
    if manifest.api_version != SEMANTIC_PACK_API_VERSION {
        return Err(format!(
            "`api_version` must be {SEMANTIC_PACK_API_VERSION}, got `{}`",
            manifest.api_version
        ));
    }
    require_stable_id("pack.id", &manifest.pack.id)?;
    require_non_empty("pack.version", &manifest.pack.version)?;
    require_non_empty("pack.display_name", &manifest.pack.display_name)?;
    optional_non_empty("pack.description", manifest.pack.description.as_deref())?;
    require_non_empty(
        "provenance.provider.name",
        &manifest.provenance.provider.name,
    )?;
    optional_non_empty(
        "provenance.provider.contact",
        manifest.provenance.provider.contact.as_deref(),
    )?;
    require_non_empty("provenance.license", &manifest.provenance.license)?;
    require_non_empty("provenance.repository", &manifest.provenance.repository)?;
    optional_non_empty(
        "provenance.source_revision",
        manifest.provenance.source_revision.as_deref(),
    )?;
    require_non_empty("compatibility.nose", &manifest.compatibility.nose)?;
    optional_non_empty(
        "compatibility.notes",
        manifest.compatibility.notes.as_deref(),
    )?;
    if manifest.pack.trust != PackTrust::ExternalOptIn || manifest.pack.enabled_by_default {
        return Err(
            "local semantic pack manifests must be external-opt-in and disabled by default"
                .to_string(),
        );
    }
    if manifest.supported_languages.is_empty() {
        return Err("`supported_languages` must contain at least one language".to_string());
    }
    for language in &manifest.supported_languages {
        require_non_empty("supported_languages[].id", &language.id)?;
        optional_non_empty(
            "supported_languages[].language_version",
            language.language_version.as_deref(),
        )?;
        optional_non_empty("supported_languages[].runtime", language.runtime.as_deref())?;
        for version in &language.runtime_versions {
            require_non_empty("supported_languages[].runtime_versions[]", version)?;
        }
    }
    for package in &manifest.packages {
        require_non_empty("packages[].ecosystem", &package.ecosystem)?;
        require_non_empty("packages[].name", &package.name)?;
        require_non_empty("packages[].versions", &package.versions)?;
    }
    for dependency in &manifest.dependencies {
        require_stable_id("dependencies[].id", &dependency.id)?;
        require_non_empty("dependencies[].version", &dependency.version)?;
        let _required = dependency.required;
    }

    let mut known_refs = HashSet::new();
    collect_unique_refs(
        "dependencies",
        manifest.dependencies.iter().map(|dep| &dep.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.evidence_producers",
        manifest
            .declares
            .evidence_producers
            .iter()
            .map(|producer| &producer.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.contracts",
        manifest
            .declares
            .contracts
            .iter()
            .map(|contract| &contract.id),
        &mut known_refs,
    )?;
    collect_unique_refs(
        "declares.value_laws",
        manifest.declares.value_laws.iter().map(|law| &law.id),
        &mut known_refs,
    )?;

    if manifest.conformance.positive_fixtures.is_empty() {
        return Err("`conformance.positive_fixtures` must not be empty".to_string());
    }
    if manifest.conformance.hard_negatives.is_empty() {
        return Err("`conformance.hard_negatives` must not be empty".to_string());
    }
    for fixture in manifest
        .conformance
        .positive_fixtures
        .iter()
        .chain(&manifest.conformance.hard_negatives)
    {
        require_stable_id("conformance fixture id", &fixture.id)?;
        require_non_empty("conformance fixture description", &fixture.description)?;
        optional_non_empty("conformance fixture path", fixture.path.as_deref())?;
        optional_non_empty(
            "conformance fixture expectation",
            fixture.expectation.as_deref(),
        )?;
    }
    for unsupported in &manifest.conformance.known_unsupported {
        require_non_empty("conformance.known_unsupported[]", unsupported)?;
    }
    optional_non_empty(
        "conformance.command",
        manifest.conformance.command.as_deref(),
    )?;
    for proof in &manifest.conformance.proofs {
        require_non_empty("conformance.proofs[]", proof)?;
    }
    let mut conformance_refs = HashSet::new();
    collect_unique_refs(
        "conformance fixtures",
        manifest
            .conformance
            .positive_fixtures
            .iter()
            .chain(&manifest.conformance.hard_negatives)
            .map(|fixture| &fixture.id),
        &mut conformance_refs,
    )?;

    for producer in &manifest.declares.evidence_producers {
        validate_evidence_producer(producer, &known_refs)?;
    }
    for contract in &manifest.declares.contracts {
        if !contract.surface.is_object() {
            return Err(format!(
                "contract `{}` surface must be an object",
                contract.id
            ));
        }
        for unsupported in &contract.known_unsupported {
            require_non_empty("contract.known_unsupported[]", unsupported)?;
        }
        optional_non_empty("contract.notes", contract.notes.as_deref())?;
        validate_contract(
            &contract.id,
            &contract.requires,
            &contract.semantics,
            contract.channel,
            contract.proof_status,
            &contract.conformance_refs,
            &known_refs,
            &conformance_refs,
            true,
        )?;
    }
    for law in &manifest.declares.value_laws {
        validate_contract(
            &law.id,
            &law.requires,
            &law.semantics,
            law.channel,
            law.proof_status,
            &law.conformance_refs,
            &known_refs,
            &conformance_refs,
            false,
        )?;
    }
    Ok(())
}

fn validate_evidence_producer(
    producer: &ManifestEvidenceProducer,
    known_refs: &HashSet<String>,
) -> Result<(), String> {
    require_stable_id("declares.evidence_producers[].id", &producer.id)?;
    if !producer.kind.starts_with_evidence_prefix() {
        return Err(format!(
            "evidence producer `{}` has unknown kind `{}`",
            producer.id, producer.kind
        ));
    }
    if producer.anchors.is_empty() {
        return Err(format!(
            "evidence producer `{}` must declare at least one anchor",
            producer.id
        ));
    }
    if producer.stable_hash_inputs.is_empty()
        || !producer
            .stable_hash_inputs
            .iter()
            .any(|input| input == "pack.id")
        || !producer
            .stable_hash_inputs
            .iter()
            .any(|input| input == "producer.id")
    {
        return Err(format!(
            "evidence producer `{}` stable_hash_inputs must include pack.id and producer.id",
            producer.id
        ));
    }
    if producer.conflict_policy != "fail-closed" && producer.conflict_policy != "near-only" {
        return Err(format!(
            "evidence producer `{}` conflict_policy must be fail-closed or near-only",
            producer.id
        ));
    }
    if producer.channel.exact_capable() && producer.conflict_policy != "fail-closed" {
        return Err(format!(
            "exact-capable evidence producer `{}` must fail closed on conflicts",
            producer.id
        ));
    }
    for emitted in &producer.emits {
        if !emitted.starts_with_evidence_prefix() {
            return Err(format!(
                "evidence producer `{}` emits unknown evidence kind `{emitted}`",
                producer.id
            ));
        }
    }
    for requirement in &producer.requires {
        validate_requirement(&producer.id, requirement, known_refs)?;
    }
    optional_non_empty("producer.notes", producer.notes.as_deref())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_contract(
    id: &str,
    requirements: &[ManifestRequirement],
    semantics: &serde_json::Value,
    channel: SemanticPackChannel,
    _proof_status: SemanticPackProofStatus,
    conformance_refs: &[String],
    known_refs: &HashSet<String>,
    fixture_refs: &HashSet<String>,
    requires_surface: bool,
) -> Result<(), String> {
    require_stable_id("declares.contracts[].id", id)?;
    if requires_surface && !semantics.is_object() {
        return Err(format!("contract `{id}` semantics must be an object"));
    }
    for ref_id in conformance_refs {
        if !fixture_refs.contains(ref_id) {
            return Err(format!(
                "contract `{id}` references missing conformance fixture `{ref_id}`"
            ));
        }
    }
    if channel.exact_capable() {
        if requirements.is_empty() {
            return Err(format!(
                "exact-capable contract `{id}` must declare evidence requirements"
            ));
        }
        let semantics = semantics
            .as_object()
            .ok_or_else(|| format!("exact-capable contract `{id}` semantics must be an object"))?;
        if !semantics.contains_key("demand") || !semantics.contains_key("effects") {
            return Err(format!(
                "exact-capable contract `{id}` must declare demand and effects"
            ));
        }
    }
    for requirement in requirements {
        validate_requirement(id, requirement, known_refs)?;
    }
    Ok(())
}

fn validate_requirement(
    context: &str,
    requirement: &ManifestRequirement,
    known_refs: &HashSet<String>,
) -> Result<(), String> {
    require_non_empty("requirement.ref", &requirement.ref_id)?;
    require_non_empty("requirement.subject", &requirement.subject)?;
    let _required = requirement.required;
    optional_non_empty(
        "requirement.same_anchor_as",
        requirement.same_anchor_as.as_deref(),
    )?;
    optional_non_empty(
        "requirement.within_scope",
        requirement.within_scope.as_deref(),
    )?;
    optional_non_empty("requirement.before", requirement.before.as_deref())?;
    optional_non_empty("requirement.after", requirement.after.as_deref())?;
    if !known_refs.contains(&requirement.ref_id)
        && !ALLOWED_REQUIREMENT_PREFIXES
            .iter()
            .any(|prefix| requirement.ref_id.starts_with(prefix))
    {
        return Err(format!(
            "`{context}` requirement references unknown id `{}`",
            requirement.ref_id
        ));
    }
    Ok(())
}

fn collect_unique_refs<'a>(
    label: &str,
    ids: impl Iterator<Item = &'a String>,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    for id in ids {
        require_stable_id(label, id)?;
        if !out.insert(id.clone()) {
            return Err(format!("duplicate id `{id}` in `{label}`"));
        }
    }
    Ok(())
}

fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must be a non-empty string"));
    }
    Ok(())
}

fn optional_non_empty(label: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        return Err(format!("`{label}` must be a non-empty string when present"));
    }
    Ok(())
}

fn require_stable_id(label: &str, value: &str) -> Result<(), String> {
    require_non_empty(label, value)?;
    let mut chars = value.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
    {
        return Err(format!("`{label}` has invalid stable id `{value}`"));
    }
    Ok(())
}

trait EvidenceKindPrefix {
    fn starts_with_evidence_prefix(&self) -> bool;
}

impl EvidenceKindPrefix for str {
    fn starts_with_evidence_prefix(&self) -> bool {
        ALLOWED_REQUIREMENT_PREFIXES[..ALLOWED_REQUIREMENT_PREFIXES.len() - 1]
            .iter()
            .any(|prefix| self.starts_with(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nose_semantic_pack_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest(id: &str) -> String {
        format!(
            r#"{{
  "api_version": "nose.semantic-pack.v0",
  "pack": {{
    "id": "{id}",
    "kind": "LibraryPack",
    "version": "0.1.0",
    "display_name": "Example",
    "trust": "external-opt-in",
    "enabled_by_default": false
  }},
  "provenance": {{
    "provider": {{ "name": "Example" }},
    "license": "MIT",
    "repository": "https://example.invalid"
  }},
  "compatibility": {{ "nose": ">=0.5.0 <0.6.0" }},
  "supported_languages": [{{ "id": "python" }}],
  "declares": {{
    "evidence_producers": [{{
      "id": "python.library-api.example",
      "kind": "LibraryApi.Contract",
      "anchors": ["node"],
      "channel": "exact-empirical",
      "stable_hash_inputs": ["pack.id", "producer.id", "call_span"],
      "conflict_policy": "fail-closed"
    }}],
    "contracts": [{{
      "id": "python.example.contract",
      "surface": {{ "kind": "function" }},
      "requires": [{{
        "ref": "python.library-api.example",
        "subject": "call",
        "required": true
      }}],
      "semantics": {{
        "operation": "Example",
        "demand": {{ "arguments": "eager-left-to-right" }},
        "effects": ["argument-effects-in-order"]
      }},
      "channel": "exact-empirical",
      "proof_status": "covered",
      "conformance_refs": ["positive", "negative"]
    }}],
    "value_laws": []
  }},
  "conformance": {{
    "positive_fixtures": [{{ "id": "positive", "description": "positive" }}],
    "hard_negatives": [{{ "id": "negative", "description": "negative" }}],
    "known_unsupported": []
  }}
}}"#
        )
    }

    #[test]
    fn first_party_pack_hash_matches_evidence_provenance_hash_policy() {
        let pack = first_party_semantic_pack();
        assert_eq!(pack.id, FIRST_PARTY_PACK_ID);
        assert_eq!(pack.hash, stable_symbol_hash(FIRST_PARTY_PACK_ID));
        assert_eq!(pack.influence, SemanticPackInfluence::EvidenceAndContracts);
    }

    #[test]
    fn local_manifest_loads_as_metadata_only_opt_in() {
        let dir = unique_dir("load");
        let path = dir.join("pack.json");
        fs::write(&path, manifest("com.example.pack")).unwrap();
        let set = SemanticPackSet::new_local(&[path]).expect("pack loads");
        assert_eq!(set.packs().len(), 2);
        let external = &set.packs()[1];
        assert_eq!(external.id, "com.example.pack");
        assert_eq!(external.hash, stable_symbol_hash("com.example.pack"));
        assert_eq!(external.trust, PackTrust::ExternalOptIn);
        assert_eq!(external.source, SemanticPackSource::LocalManifest);
        assert_eq!(external.influence, SemanticPackInfluence::MetadataOnly);
        assert_eq!(external.counts.contracts, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn external_pack_enabled_by_default_is_rejected() {
        let dir = unique_dir("trust");
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""trust": "external-opt-in",
    "enabled_by_default": false"#,
                r#""trust": "external-opt-in",
    "enabled_by_default": true"#,
            ),
        )
        .unwrap();
        let err = load_local_manifest(&path).expect_err("must reject implicit external default");
        assert!(err
            .to_string()
            .contains("must be external-opt-in and disabled by default"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn local_manifest_claiming_first_party_trust_is_rejected() {
        let dir = unique_dir("first_party_trust");
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#""trust": "external-opt-in""#,
                r#""trust": "default-first-party""#,
            ),
        )
        .unwrap();
        let err =
            load_local_manifest(&path).expect_err("local manifest must not claim first-party");
        assert!(err
            .to_string()
            .contains("must be external-opt-in and disabled by default"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn package_entries_must_match_manifest_shape() {
        let dir = unique_dir("package");
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack").replace(
                r#"  "supported_languages": [{ "id": "python" }],
"#,
                r#"  "supported_languages": [{ "id": "python" }],
  "packages": [{ "ecosystem": "pypi", "name": "example" }],
"#,
            ),
        )
        .unwrap();
        let err = load_local_manifest(&path).expect_err("package versions are required");
        assert!(err.to_string().contains("missing field `versions`"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn evidence_producer_anchors_must_be_known_anchor_names() {
        let dir = unique_dir("anchor");
        let path = dir.join("pack.json");
        fs::write(
            &path,
            manifest("com.example.pack")
                .replace(r#""anchors": ["node"]"#, r#""anchors": ["raw-selector"]"#),
        )
        .unwrap();
        let err = load_local_manifest(&path).expect_err("unknown anchors must not load");
        assert!(err.to_string().contains("unknown variant"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_pack_ids_fail_closed() {
        let dir = unique_dir("dupe");
        let one = dir.join("one.json");
        let two = dir.join("two.json");
        fs::write(&one, manifest("com.example.pack")).unwrap();
        fs::write(&two, manifest("com.example.pack")).unwrap();
        let err = SemanticPackSet::new_local(&[one, two]).expect_err("duplicate id");
        assert!(err.to_string().contains("duplicate semantic pack id"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn directory_discovery_sorts_json_manifests() {
        let dir = unique_dir("dir");
        fs::write(dir.join("b.json"), manifest("com.example.b")).unwrap();
        fs::write(dir.join("a.json"), manifest("com.example.a")).unwrap();
        let paths = discover_manifest_paths(std::slice::from_ref(&dir)).expect("discover");
        let names = paths
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["a.json", "b.json"]);
        let _ = fs::remove_dir_all(dir);
    }
}
