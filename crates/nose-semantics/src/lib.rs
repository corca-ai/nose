//! Semantic contracts for language and library facts used by exact matching.
//!
//! This crate is the builtin semantic-kernel facade. The initial migration is
//! deliberately behavior-preserving: it names the semantic assumptions that were
//! previously encoded as scattered `Lang` matches. Future pack loading should
//! extend this contract surface rather than letting packs mint fingerprints or
//! approve exact clone matches directly.

use nose_il::{
    contains_js_identifier, stable_symbol_hash, Builtin, CallTargetEvidenceKind,
    EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceRecord,
    EvidenceStatus, GuardEvidenceKind, HoFKind, Il, ImportEvidenceKind, Interner, Lang,
    LibraryApiEvidenceKind, LitClass, LoopKind, NodeId, NodeKind, Op, ParamSemantic, Payload,
    SequenceSurfaceKind, SourceBindingKind, SourceCallKind, SourceCastKind,
    SourceComprehensionKind, SourceFactKind, SourceLiteralKind, SourceOperatorKind,
    SourcePatternKind, SourceProtocolKind, SourceRangeKind, Span, Symbol, SymbolEvidenceKind,
    TypeEvidenceKind,
};
use rustc_hash::FxHashMap;

mod api_guards;
mod async_adapters;
mod collection_semantics;
mod constructor_contracts;
mod demand;
mod effects;
mod evidence;
mod free_builtins;
mod guard_evidence;
mod import_facts;
mod language_profile;
mod library_api;
mod map_statics;
mod method_contracts;
mod method_families;
mod module_exports;
mod module_semantics;
mod object_key_views;
mod operator_thresholds;
mod operators;
mod packs;
mod sequence_surface;
mod stdlib_semantics;
mod symbol_identity;
mod type_domain;

pub use api_guards::*;
pub use async_adapters::*;
pub use collection_semantics::*;
use collection_semantics::{
    FREE_NAME_COLLECTION_FACTORIES, FREE_NAME_MAP_FACTORIES, IMPORTED_COLLECTION_FACTORIES,
};
pub use constructor_contracts::*;
pub use demand::*;
pub(crate) use effects::asserted_effect_at_node;
pub use effects::*;
pub use evidence::*;
use evidence::{
    assignment_is_visible_at_reference, nearest_named_param_scope, nearest_scope,
    receiver_cid_param_span, strict_numeric_operand_operator, unique_asserted_evidence_at,
    unique_evidence_at, var_references_same_binding, EvidenceResolution,
};
pub use free_builtins::*;
pub use guard_evidence::*;
pub use import_facts::*;
pub use language_profile::{
    builtin_language_pack_id, is_builtin_language_pack_hash, language_core_evidence_provenance,
    language_source_fact_provenance, semantics, LanguageProfile, CSS_LANGUAGE_CORE_PRODUCER_ID,
    CSS_LANGUAGE_PACK_ID, CSS_SOURCE_FACT_PRODUCER_ID, C_LANGUAGE_CORE_PRODUCER_ID,
    C_LANGUAGE_PACK_ID, C_SOURCE_FACT_PRODUCER_ID, C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
    GO_LANGUAGE_CORE_PRODUCER_ID, GO_LANGUAGE_PACK_ID, GO_SOURCE_FACT_PRODUCER_ID,
    HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID, HTML_EMBEDDED_LANGUAGE_PACK_ID,
    HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID, JAVA_LANGUAGE_CORE_PRODUCER_ID, JAVA_LANGUAGE_PACK_ID,
    JAVA_SOURCE_FACT_PRODUCER_ID, JS_TS_LANGUAGE_CORE_PRODUCER_ID, JS_TS_LANGUAGE_PACK_ID,
    JS_TS_SOURCE_FACT_PRODUCER_ID, PYTHON_LANGUAGE_CORE_PRODUCER_ID, PYTHON_LANGUAGE_PACK_ID,
    PYTHON_SOURCE_FACT_PRODUCER_ID, RUBY_LANGUAGE_CORE_PRODUCER_ID, RUBY_LANGUAGE_PACK_ID,
    RUBY_SOURCE_FACT_PRODUCER_ID, RUST_LANGUAGE_CORE_PRODUCER_ID, RUST_LANGUAGE_PACK_ID,
    RUST_SOURCE_FACT_PRODUCER_ID, SWIFT_LANGUAGE_CORE_PRODUCER_ID, SWIFT_LANGUAGE_PACK_ID,
    SWIFT_SOURCE_FACT_PRODUCER_ID,
};
pub(crate) use language_profile::{js_like_lang, language_core_evidence_provenance_hashes};
pub use library_api::*;
use library_api::{
    language_core_builtin_at_call, library_api_dependency_id_for_normalized_hof,
    library_method_selector_name,
};
pub use map_statics::*;
use method_contracts::scalar_integer_method_contract_shape;
pub use method_contracts::*;
pub use method_families::*;
pub use module_exports::*;
pub use module_semantics::*;
pub use nose_il::DomainEvidence;
pub use object_key_views::{
    js_object_key_view_argument_dependency_ids_for_call,
    js_object_key_view_argument_map_node_at_call_span,
    js_object_key_view_argument_map_node_for_call,
};
pub use operator_thresholds::{index_membership_threshold_contract, IndexMembershipThreshold};
use operator_thresholds::{
    index_membership_threshold_matches, threshold_at_or_below_floor, threshold_below_floor,
    threshold_excludes_floor, threshold_reaches_floor,
};
pub use operators::*;
pub use packs::*;
pub use sequence_surface::*;
use sequence_surface::{
    sequence_surface_evidence_at_sequence_span, sequence_surface_evidence_matches_node,
    sequence_surface_evidence_record_at_sequence_span,
};
pub use stdlib_semantics::*;
pub use symbol_identity::{
    asserted_unshadowed_global_symbol, file_defines_name_visible_at, imported_binding_symbol,
    imported_member_symbol, imported_namespace_symbol, qualified_global_symbol,
    qualified_global_symbol_at_span,
};
use symbol_identity::{
    assignment_alias_hash, assignment_parts, binding_identity_matches,
    language_core_symbol_identity_at_anchor_matches, literal_string_hash, node_name,
    node_name_hash, qualified_global_dependency_valid, qualified_global_symbol_at_evidence_anchor,
    qualified_global_symbol_record_valid, symbol_evidence_at_node_anchor,
    symbol_record_has_admitted_provenance, top_level_statements, unit_defines_hash,
    unit_defines_hash_visible_at,
};
pub use type_domain::{
    python_stdlib_type_domain, python_stdlib_type_domain_contract, type_domain_from_source_text,
    BuiltinTypeDomainAliasContract, FirstPartyTypeDomainAliasContract,
    PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID,
    PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID,
};

/// Stable pack id for the legacy builtin compatibility facade.
///
/// The wire id remains `nose.first_party` while narrower builtin packs take
/// ownership of individual language, stdlib, protocol, and law surfaces.
pub const BUILTIN_COMPAT_PACK_ID: &str = "nose.first_party";

/// Compatibility alias for older callers and fixtures.
pub const FIRST_PARTY_PACK_ID: &str = BUILTIN_COMPAT_PACK_ID;

/// Stable pack id for builtin value-graph law provenance.
pub const VALUE_GRAPH_LAW_PACK_ID: &str = "nose.value_graph.laws";

/// Compatibility alias for older callers and fixtures.
pub const FIRST_PARTY_VALUE_LAW_PACK_ID: &str = VALUE_GRAPH_LAW_PACK_ID;

/// Channel a semantic fact or contract is safe to influence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelEligibility {
    SyntaxOnly,
    NearOnly,
    ExactEmpirical,
    ExactProven,
}

impl ChannelEligibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            ChannelEligibility::SyntaxOnly => "syntax-only",
            ChannelEligibility::NearOnly => "near-only",
            ChannelEligibility::ExactEmpirical => "exact-empirical",
            ChannelEligibility::ExactProven => "exact-proven",
        }
    }
}

/// Trust/provenance policy for a pack, separate from which analysis channel a fact may enter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackTrust {
    BuiltinDefault,
    BuiltinOptional,
    ExternalOptIn,
}

#[cfg(test)]
mod tests;
