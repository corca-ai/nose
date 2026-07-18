use super::*;
use nose_il::Lang;

mod constants;
mod counts;
mod descriptor_groups;

use constants::*;
use counts::*;
use descriptor_groups::BUILTIN_PACK_DESCRIPTORS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuiltinLanguageBinding {
    pub langs: &'static [Lang],
    pub file_extensions: &'static [&'static str],
    pub parser: &'static str,
    pub lowering_entrypoint: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinPackDescriptor {
    pub id: &'static str,
    pub kind: SemanticPackKind,
    pub display_name: &'static str,
    pub trust: PackTrust,
    pub enabled_by_default: bool,
    pub supported_languages: &'static [&'static str],
    pub supported_packages: &'static [&'static str],
    pub language: Option<BuiltinLanguageBinding>,
    pub evidence_producer_ids: &'static [&'static str],
    pub source_fact_producer_ids: &'static [&'static str],
    pub contract_ids: &'static [&'static str],
    pub type_domain_alias_contracts: &'static [BuiltinTypeDomainAliasContract],
    static_value_law_ids: &'static [&'static str],
    dynamic_value_law_ids: Option<fn() -> Vec<&'static str>>,
    static_conformance_refs: &'static [&'static str],
    dynamic_conformance_refs: Option<fn() -> Vec<&'static str>>,
    counts: fn() -> SemanticPackCounts,
}

impl BuiltinPackDescriptor {
    pub fn value_law_ids(self) -> Vec<&'static str> {
        let mut ids = self.static_value_law_ids.to_vec();
        if let Some(dynamic_ids) = self.dynamic_value_law_ids {
            ids.extend(dynamic_ids());
        }
        ids
    }

    pub fn conformance_refs(self) -> Vec<&'static str> {
        let mut refs = self.static_conformance_refs.to_vec();
        if let Some(dynamic_refs) = self.dynamic_conformance_refs {
            refs.extend(dynamic_refs());
        }
        refs
    }

    pub fn counts(self) -> SemanticPackCounts {
        (self.counts)()
    }

    fn summary(self) -> SemanticPackSummary {
        SemanticPackSummary {
            id: self.id.to_string(),
            hash: semantic_pack_hash(self.id),
            kind: self.kind,
            version: env!("CARGO_PKG_VERSION").to_string(),
            display_name: self.display_name.to_string(),
            trust: self.trust,
            enabled_by_default: self.enabled_by_default,
            source: SemanticPackSource::CompiledBuiltin,
            influence: SemanticPackInfluence::EvidenceAndContracts,
            manifest_path: None,
            provider: "Corca, Inc.".to_string(),
            repository: "https://github.com/corca-ai/nose".to_string(),
            license: "MIT".to_string(),
            supported_languages: self
                .supported_languages
                .iter()
                .map(|language| (*language).to_string())
                .collect(),
            counts: self.counts(),
            api_version: None,
            semantic_digest: None,
        }
    }
}

pub fn builtin_pack_descriptors() -> &'static [BuiltinPackDescriptor] {
    BUILTIN_PACK_DESCRIPTORS.as_slice()
}

pub fn builtin_pack_descriptor(pack_id: &str) -> Option<&'static BuiltinPackDescriptor> {
    builtin_pack_descriptors()
        .iter()
        .find(|descriptor| descriptor.id == pack_id)
}

pub fn builtin_compat_semantic_pack() -> SemanticPackSummary {
    builtin_pack_descriptor(BUILTIN_COMPAT_PACK_ID)
        .expect("builtin compatibility pack descriptor exists")
        .summary()
}

pub fn first_party_semantic_pack() -> SemanticPackSummary {
    builtin_compat_semantic_pack()
}

pub fn python_stdlib_type_domain_pack() -> SemanticPackSummary {
    builtin_pack_descriptor(PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID)
        .expect("Python stdlib type-domain descriptor exists")
        .summary()
}

pub fn value_graph_law_pack() -> SemanticPackSummary {
    builtin_pack_descriptor(VALUE_GRAPH_LAW_PACK_ID)
        .expect("value-graph law descriptor exists")
        .summary()
}

pub fn first_party_value_law_pack() -> SemanticPackSummary {
    value_graph_law_pack()
}

pub(super) fn compiled_builtin_packs() -> Vec<SemanticPackSummary> {
    builtin_pack_descriptors()
        .iter()
        .map(|descriptor| descriptor.summary())
        .collect()
}

pub(super) fn is_compiled_builtin_pack_id(pack_id: &str) -> bool {
    compiled_builtin_packs()
        .iter()
        .any(|pack| pack.id == pack_id)
}
