use super::manifest::SemanticPackManifest;
use super::v1::{
    CompiledSemanticPackV1, SemanticPackManifestV1, SemanticPackV1FixtureKind,
    SemanticPackV1Language,
};
use super::{
    validation::validate_manifest, SEMANTIC_PACK_API_VERSION, SEMANTIC_PACK_API_VERSION_V1,
};
use crate::PackTrust;
use nose_il::stable_symbol_hash;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackSource {
    CompiledBuiltin,
    LocalManifest,
}

impl SemanticPackSource {
    #[allow(non_upper_case_globals)]
    #[deprecated(note = "use SemanticPackSource::CompiledBuiltin")]
    pub const CompiledFirstParty: Self = Self::CompiledBuiltin;

    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackSource::CompiledBuiltin => "compiled-builtin",
            SemanticPackSource::LocalManifest => "local-manifest",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackInfluence {
    EvidenceAndContracts,
    ExternalClaimExact,
    NearOnly,
    MetadataOnly,
}

impl SemanticPackInfluence {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackInfluence::EvidenceAndContracts => "evidence-and-contracts",
            SemanticPackInfluence::ExternalClaimExact => "external-claim-exact",
            SemanticPackInfluence::NearOnly => "near-only",
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
pub enum SemanticPackAnchor {
    SourceSpan,
    Node,
    Param,
    Binding,
    Sequence,
    Module,
    Package,
}

impl SemanticPackAnchor {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackAnchor::SourceSpan => "source-span",
            SemanticPackAnchor::Node => "node",
            SemanticPackAnchor::Param => "param",
            SemanticPackAnchor::Binding => "binding",
            SemanticPackAnchor::Sequence => "sequence",
            SemanticPackAnchor::Module => "module",
            SemanticPackAnchor::Package => "package",
        }
    }
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
pub(super) enum SemanticPackStatus {
    DraftExample,
    Experimental,
    Stable,
    Deprecated,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SemanticPackSchemaVersion {
    V0,
}

impl PackTrust {
    #[allow(non_upper_case_globals)]
    #[deprecated(note = "use PackTrust::BuiltinDefault")]
    pub const DefaultFirstParty: Self = Self::BuiltinDefault;

    #[allow(non_upper_case_globals)]
    #[deprecated(note = "use PackTrust::BuiltinOptional")]
    pub const FirstPartyOptional: Self = Self::BuiltinOptional;

    pub const fn as_manifest_str(self) -> &'static str {
        match self {
            PackTrust::BuiltinDefault => "builtin-default",
            PackTrust::BuiltinOptional => "builtin-optional",
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
            "builtin-default" | "default-first-party" => Ok(PackTrust::BuiltinDefault),
            "builtin-optional" | "first-party-optional" => Ok(PackTrust::BuiltinOptional),
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
    pub api_version: Option<&'static str>,
    pub semantic_digest: Option<String>,
}

impl SemanticPackSummary {
    pub fn hash_hex(&self) -> String {
        format!("{:016x}", self.hash)
    }

    pub(super) fn from_manifest_v0(
        path: PathBuf,
        manifest: SemanticPackManifest,
    ) -> Result<Self, String> {
        validate_manifest(&manifest).map_err(|err| err.to_string())?;
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
            api_version: Some(SEMANTIC_PACK_API_VERSION),
            semantic_digest: None,
        })
    }

    pub(super) fn from_manifest_v1(
        path: PathBuf,
        manifest: &SemanticPackManifestV1,
        compiled: &CompiledSemanticPackV1,
    ) -> Self {
        Self {
            id: manifest.pack.id.clone(),
            hash: semantic_pack_hash(&manifest.pack.id),
            kind: manifest.pack.kind,
            version: manifest.pack.version.clone(),
            display_name: manifest.pack.display_name.clone(),
            trust: PackTrust::ExternalOptIn,
            enabled_by_default: false,
            source: SemanticPackSource::LocalManifest,
            influence: SemanticPackInfluence::MetadataOnly,
            manifest_path: Some(path),
            provider: manifest.provenance.provider.name.clone(),
            repository: manifest.provenance.repository.clone(),
            license: manifest.provenance.license.clone(),
            supported_languages: manifest
                .supported_languages
                .iter()
                .map(|language| match language {
                    SemanticPackV1Language::Java => "java".to_string(),
                })
                .collect(),
            counts: SemanticPackCounts {
                evidence_producers: 0,
                contracts: manifest.declares.api_contracts.len(),
                value_laws: 0,
                positive_fixtures: compiled
                    .conformance_fixtures()
                    .iter()
                    .filter(|fixture| fixture.kind == SemanticPackV1FixtureKind::Positive)
                    .count(),
                hard_negatives: compiled
                    .conformance_fixtures()
                    .iter()
                    .filter(|fixture| fixture.kind == SemanticPackV1FixtureKind::HardNegative)
                    .count(),
            },
            api_version: Some(SEMANTIC_PACK_API_VERSION_V1),
            semantic_digest: Some(compiled.semantic_digest().to_string()),
        }
    }
}

pub fn semantic_pack_hash(pack_id: &str) -> u64 {
    stable_symbol_hash(pack_id)
}
