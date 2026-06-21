use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SemanticPackManifest {
    #[serde(rename = "$schema")]
    pub(super) _schema: Option<String>,
    pub(super) api_version: String,
    pub(super) pack: ManifestPack,
    pub(super) provenance: ManifestProvenance,
    pub(super) compatibility: ManifestCompatibility,
    pub(super) supported_languages: Vec<ManifestLanguage>,
    #[serde(default)]
    pub(super) packages: Vec<ManifestPackage>,
    #[serde(default)]
    pub(super) dependencies: Vec<ManifestDependency>,
    pub(super) declares: ManifestDeclares,
    pub(super) conformance: ManifestConformance,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestPack {
    pub(super) id: String,
    pub(super) kind: SemanticPackKind,
    pub(super) version: String,
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    pub(super) trust: PackTrust,
    pub(super) enabled_by_default: bool,
    // Documented v0 schema field the engine does not consume; listed (typed) so
    // `deny_unknown_fields` still accepts and validates conforming manifests.
    #[serde(default, rename = "status")]
    pub(super) _status: Option<SemanticPackStatus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestProvenance {
    pub(super) provider: ManifestProvider,
    pub(super) license: String,
    pub(super) repository: String,
    #[serde(default)]
    pub(super) source_revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestProvider {
    pub(super) name: String,
    #[serde(default)]
    pub(super) contact: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestCompatibility {
    pub(super) nose: String,
    #[serde(default, rename = "schema")]
    pub(super) _schema: Option<SemanticPackSchemaVersion>,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestLanguage {
    pub(super) id: String,
    #[serde(default)]
    pub(super) language_version: Option<String>,
    #[serde(default)]
    pub(super) runtime: Option<String>,
    #[serde(default)]
    pub(super) runtime_versions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestDependency {
    pub(super) id: String,
    pub(super) version: String,
    pub(super) required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestPackage {
    pub(super) ecosystem: String,
    pub(super) name: String,
    pub(super) versions: String,
    #[serde(default, rename = "stdlib")]
    pub(super) _stdlib: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestDeclares {
    pub(super) evidence_producers: Vec<ManifestEvidenceProducer>,
    pub(super) contracts: Vec<ManifestContract>,
    #[serde(default)]
    pub(super) value_laws: Vec<ManifestValueLaw>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestEvidenceProducer {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) anchors: Vec<SemanticPackAnchor>,
    pub(super) channel: SemanticPackChannel,
    #[serde(default)]
    pub(super) emits: Vec<String>,
    #[serde(default)]
    pub(super) requires: Vec<ManifestRequirement>,
    pub(super) stable_hash_inputs: Vec<String>,
    pub(super) conflict_policy: String,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestContract {
    pub(super) id: String,
    pub(super) surface: serde_json::Value,
    pub(super) requires: Vec<ManifestRequirement>,
    pub(super) semantics: serde_json::Value,
    pub(super) channel: SemanticPackChannel,
    pub(super) proof_status: SemanticPackProofStatus,
    pub(super) conformance_refs: Vec<String>,
    #[serde(default)]
    pub(super) known_unsupported: Vec<String>,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestValueLaw {
    pub(super) id: String,
    pub(super) requires: Vec<ManifestRequirement>,
    pub(super) semantics: serde_json::Value,
    pub(super) channel: SemanticPackChannel,
    pub(super) proof_status: SemanticPackProofStatus,
    pub(super) conformance_refs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestRequirement {
    #[serde(rename = "ref")]
    pub(super) ref_id: String,
    pub(super) subject: String,
    pub(super) required: bool,
    #[serde(default)]
    pub(super) same_anchor_as: Option<String>,
    #[serde(default)]
    pub(super) within_scope: Option<String>,
    #[serde(default)]
    pub(super) before: Option<String>,
    #[serde(default)]
    pub(super) after: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestConformance {
    pub(super) positive_fixtures: Vec<ManifestFixture>,
    pub(super) hard_negatives: Vec<ManifestFixture>,
    pub(super) known_unsupported: Vec<String>,
    #[serde(default)]
    pub(super) command: Option<String>,
    #[serde(default)]
    pub(super) proofs: Vec<String>,
    #[serde(default)]
    pub(super) executable: Vec<ManifestExecutableConformanceGate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestFixture {
    pub(super) id: String,
    pub(super) description: String,
    #[serde(default)]
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) expectation: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestExecutableConformanceGate {
    pub(super) id: String,
    pub(super) row_ref: String,
    pub(super) oracle: SemanticPackExecutableOracle,
    pub(super) positive_fixtures: Vec<String>,
    pub(super) hard_negatives: Vec<String>,
    pub(super) expected_positive: String,
    pub(super) expected_hard_negative: String,
}
