//! Semantic pack manifest loading and provenance summaries.
//!
//! Loading a manifest is an explicit opt-in metadata path. External packs do not
//! emit evidence or approve exact results from this module; consumers must still
//! require occurrence evidence and kernel-owned admission checks.

use crate::{
    pack_facing_value_laws, BuiltinTypeDomainAliasContract, PackTrust, BUILTIN_COMPAT_PACK_ID,
    BUILTIN_METHOD_CALL_CONTRACT_ID, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID, CSS_LANGUAGE_CORE_PRODUCER_ID, CSS_LANGUAGE_PACK_ID,
    CSS_SOURCE_FACT_PRODUCER_ID, C_LANGUAGE_CORE_PRODUCER_ID, C_LANGUAGE_PACK_ID,
    C_SOURCE_FACT_PRODUCER_ID, C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
    FREE_FUNCTION_BUILTIN_CONTRACT_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID, FREE_FUNCTION_HOF_CONTRACT_ID,
    GO_LANGUAGE_CORE_PRODUCER_ID, GO_LANGUAGE_PACK_ID, GO_SOURCE_FACT_PRODUCER_ID,
    GO_STDLIB_NAMESPACE_CALL_CONTRACT_ID, GO_STDLIB_NAMESPACE_CALL_PACK_ID,
    GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID, HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
    HTML_EMBEDDED_LANGUAGE_PACK_ID, HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
    ITERATOR_IDENTITY_ADAPTER_CONTRACT_ID, ITERATOR_IDENTITY_ADAPTER_PACK_ID,
    ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID, JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID, JAVA_GUAVA_IMMUTABLE_LIST_OF_CONTRACT_ID,
    JAVA_GUAVA_IMMUTABLE_MAP_OF_CONTRACT_ID, JAVA_GUAVA_IMMUTABLE_SET_OF_CONTRACT_ID,
    JAVA_LANGUAGE_CORE_PRODUCER_ID, JAVA_LANGUAGE_PACK_ID, JAVA_SOURCE_FACT_PRODUCER_ID,
    JAVA_STDLIB_COLLECTION_CONSTRUCTOR_EMPTY_LIST_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID, JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_ARRAYS_AS_LIST_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_LIST_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_SET_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_LIST_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_LIST_OF_CONTRACT_ID, JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, JAVA_STDLIB_COLLECTION_FACTORY_SET_OF_CONTRACT_ID,
    JAVA_STDLIB_MAP_ENTRY_CONTRACT_ID, JAVA_STDLIB_MAP_ENTRY_PACK_ID,
    JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID, JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_EMPTY_MAP_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_SINGLETON_MAP_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID, JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_PACK_ID, JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID, JAVA_STDLIB_MATH_PACK_ID,
    JAVA_STDLIB_MATH_PRODUCER_ID, JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_ID,
    JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
    JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID,
    JS_LIKE_BUILTIN_ARRAY_BOOL_REDUCTION_CONTRACT_ID, JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_HOF_CONTRACT_ID, JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_PACK_ID, JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
    JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_ID, JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
    JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID, JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    JS_LIKE_BUILTIN_MAP_CONSTRUCTOR_CONTRACT_ID, JS_LIKE_BUILTIN_PROMISE_PACK_ID,
    JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID, JS_LIKE_BUILTIN_PROMISE_RESOLVE_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID, JS_LIKE_BUILTIN_REGEX_PACK_ID,
    JS_LIKE_BUILTIN_REGEX_PRODUCER_ID, JS_LIKE_BUILTIN_REGEX_TEST_CONTRACT_ID,
    JS_LIKE_BUILTIN_SET_CONSTRUCTOR_CONTRACT_ID,
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_FIND_INDEX_CONTRACT_ID,
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_INDEX_OF_CONTRACT_ID,
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID, JS_TS_LANGUAGE_CORE_PRODUCER_ID,
    JS_TS_LANGUAGE_PACK_ID, JS_TS_SOURCE_FACT_PRODUCER_ID, MAP_GET_CONTRACT_ID,
    MAP_GET_DEFAULT_CONTRACT_ID, MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
    MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID, MAP_GET_PROTOCOL_PACK_ID, MAP_GET_PROTOCOL_PRODUCER_ID,
    MAP_KEY_VIEW_COLLECTION_CONTRACT_ID, MAP_KEY_VIEW_ITERATOR_CONTRACT_ID,
    MAP_KEY_VIEW_PROTOCOL_PACK_ID, MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
    PROPERTY_BUILTIN_IS_EMPTY_CONTRACT_ID, PROPERTY_BUILTIN_LEN_CONTRACT_ID,
    PROPERTY_BUILTIN_PROTOCOL_PACK_ID, PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID,
    PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_ID, PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
    PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID, PYTHON_ITERATOR_BUILTIN_CONTRACT_ID,
    PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID, PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
    PYTHON_LANGUAGE_CORE_PRODUCER_ID, PYTHON_LANGUAGE_PACK_ID, PYTHON_SOURCE_FACT_PRODUCER_ID,
    PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_ID, PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
    PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, PYTHON_STDLIB_MATH_PACK_ID,
    PYTHON_STDLIB_MATH_PRODUCER_ID, PYTHON_STDLIB_MATH_PROD_CONTRACT_ID,
    PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID,
    RECEIVER_MEMBERSHIP_CONTRACT_ID, RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
    RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID, RUBY_LANGUAGE_CORE_PRODUCER_ID,
    RUBY_LANGUAGE_PACK_ID, RUBY_SOURCE_FACT_PRODUCER_ID, RUBY_STDLIB_SET_CONTRACT_ID,
    RUBY_STDLIB_SET_PACK_ID, RUBY_STDLIB_SET_PRODUCER_ID, RUST_LANGUAGE_CORE_PRODUCER_ID,
    RUST_LANGUAGE_PACK_ID, RUST_SOURCE_FACT_PRODUCER_ID,
    RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID, RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
    RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, RUST_STDLIB_INTEGER_METHOD_PACK_ID,
    RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID, RUST_STDLIB_MAP_FACTORY_CONTRACT_ID,
    RUST_STDLIB_MAP_FACTORY_PACK_ID, RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
    RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID, RUST_STDLIB_OPTION_NONE_CONTRACT_ID,
    RUST_STDLIB_OPTION_PACK_ID, RUST_STDLIB_OPTION_PRODUCER_ID,
    RUST_STDLIB_OPTION_SOME_CONTRACT_ID, RUST_STDLIB_RESULT_ERR_CONTRACT_ID,
    RUST_STDLIB_RESULT_IS_ERR_CONTRACT_ID, RUST_STDLIB_RESULT_IS_OK_CONTRACT_ID,
    RUST_STDLIB_RESULT_OK_CONTRACT_ID, RUST_STDLIB_RESULT_PACK_ID, RUST_STDLIB_RESULT_PRODUCER_ID,
    RUST_STDLIB_VEC_MACRO_CONTRACT_ID, RUST_STDLIB_VEC_NEW_CONTRACT_ID, RUST_STDLIB_VEC_PACK_ID,
    RUST_STDLIB_VEC_PRODUCER_ID, SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID, SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID, SEQUENCE_HOF_ADAPTER_CONTRACT_ID,
    SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID, SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
    STRING_AFFIX_PREDICATE_CONTRACT_ID, STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID,
    STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID, SWIFT_LANGUAGE_CORE_PRODUCER_ID,
    SWIFT_LANGUAGE_PACK_ID, SWIFT_SOURCE_FACT_PRODUCER_ID, SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
    VALUE_GRAPH_LAW_PACK_ID,
};
#[cfg(test)]
use crate::{
    FIRST_PARTY_PACK_ID, FIRST_PARTY_VALUE_LAW_PACK_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_ARRAY_CONTRACT_ID, SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_SET_CONTRACT_ID,
    SWIFT_STDLIB_DICTIONARY_UNIQUE_KEYS_CONTRACT_ID,
};
use nose_il::stable_symbol_hash;
use serde::Deserialize;
use std::path::PathBuf;

mod compiled;
mod conformance;
mod external;
mod loading;
mod manifest;
mod result_domain_semantics;
mod validation;

pub use compiled::{
    builtin_compat_semantic_pack, builtin_pack_descriptor, builtin_pack_descriptors,
    first_party_semantic_pack, first_party_value_law_pack, python_stdlib_type_domain_pack,
    value_graph_law_pack, BuiltinLanguageBinding, BuiltinPackDescriptor,
};
pub use conformance::{
    SemanticPackConformanceManifest, SemanticPackConformanceReport,
    SemanticPackExecutableConformanceCheck, SemanticPackExecutableConformanceIssue,
    SemanticPackExecutableOracle, SemanticPackFixtureCheck, SemanticPackFixtureIssue,
    SemanticPackFixtureKind,
};
pub use external::{
    ExternalContractRow, ExternalEvidenceProducerRow, ExternalInfluenceBlocker,
    ExternalInfluencePreflightReport, ExternalRowConflict, ExternalRowConflictReport,
    ExternalRowInfluencePreflight, ExternalRowKind, ExternalValueLawRow,
    SemanticPackRequirementSummary,
};
pub use loading::{
    check_semantic_pack_conformance, discover_manifest_paths, load_local_manifest,
    SemanticPackLoadError,
};
use manifest::*;

use compiled::compiled_builtin_packs;
use validation::validate_manifest;
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
            "builtin-default" => Ok(PackTrust::BuiltinDefault),
            "default-first-party" => Ok(PackTrust::BuiltinDefault),
            "builtin-optional" => Ok(PackTrust::BuiltinOptional),
            "first-party-optional" => Ok(PackTrust::BuiltinOptional),
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackSet {
    packs: Vec<SemanticPackSummary>,
    external_evidence_producer_rows: Vec<ExternalEvidenceProducerRow>,
    external_contract_rows: Vec<ExternalContractRow>,
    external_value_law_rows: Vec<ExternalValueLawRow>,
}

impl SemanticPackSet {
    pub fn new_local(paths: &[PathBuf]) -> Result<Self, SemanticPackLoadError> {
        let manifest_paths = discover_manifest_paths(paths)?;
        let mut packs = compiled_builtin_packs();
        let mut external_evidence_producer_rows = Vec::new();
        let mut external_contract_rows = Vec::new();
        let mut external_value_law_rows = Vec::new();
        for path in manifest_paths {
            let loaded = loading::load_local_manifest_with_rows(&path)?;
            if let Some(existing) = packs
                .iter()
                .find(|existing| existing.id == loaded.summary.id)
            {
                return Err(SemanticPackLoadError::DuplicatePackId {
                    id: loaded.summary.id,
                    first_path: existing.manifest_path.clone(),
                    second_path: Some(path),
                });
            }
            external_evidence_producer_rows.extend(loaded.external_evidence_producer_rows);
            external_contract_rows.extend(loaded.external_contract_rows);
            external_value_law_rows.extend(loaded.external_value_law_rows);
            packs.push(loaded.summary);
        }
        Ok(Self {
            packs,
            external_evidence_producer_rows,
            external_contract_rows,
            external_value_law_rows,
        })
    }

    pub fn builtin_only() -> Self {
        Self {
            packs: compiled_builtin_packs(),
            external_evidence_producer_rows: Vec::new(),
            external_contract_rows: Vec::new(),
            external_value_law_rows: Vec::new(),
        }
    }

    pub fn first_party_only() -> Self {
        Self::builtin_only()
    }

    pub fn packs(&self) -> &[SemanticPackSummary] {
        &self.packs
    }

    pub fn external_evidence_producer_rows(&self) -> &[ExternalEvidenceProducerRow] {
        &self.external_evidence_producer_rows
    }

    pub fn external_contract_rows(&self) -> &[ExternalContractRow] {
        &self.external_contract_rows
    }

    pub fn external_value_law_rows(&self) -> &[ExternalValueLawRow] {
        &self.external_value_law_rows
    }
}

#[cfg(test)]
mod tests;
