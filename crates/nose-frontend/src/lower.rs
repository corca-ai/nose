//! Shared lowering context and helpers used by every per-language frontend.
//! Language-specific walks build IL through this, so the arena/span/intern
//! mechanics live in one place.

/// Surface tags that [`Lowering::protocol_boundary`] (and the `await`/`yield` helpers) emit.
/// These `Raw` nodes are a **deliberate fail-closed boundary** — async, channels, defer,
/// try-propagation, generators — preserving effect/protocol semantics until a contract proves
/// they can be erased safely; they are NOT unlowered constructs. Coverage reporting separates
/// them from genuine lowering gaps so the worklist isn't misled into "fixing" a boundary
/// (which would be unsound). `protocol_boundary` debug-asserts membership, so a new boundary
/// tag must be added here.
pub(crate) const PROTOCOL_BOUNDARY_TAGS: &[&str] = &[
    "async_block",
    "await",
    "channel_receive",
    "channel_receive_status",
    "channel_send",
    "defer",
    "go",
    "select",
    "select_case",
    "select_default",
    "try",
    "yield",
];

/// Raw surfaces that are intentionally retained as non-runtime syntax/preprocessor
/// boundaries. They are not source protocol/effect boundaries, but they also are
/// not actionable lowering gaps.
pub(crate) const INTENTIONAL_RAW_BOUNDARY_TAGS: &[&str] = &[
    "availability_condition",
    "fallthrough_statement",
    "macro_rule_body",
];

/// Whether a `Raw` node's surface tag is a deliberate protocol boundary (vs a lowering gap).
#[must_use]
pub(crate) fn is_protocol_boundary_tag(tag: &str) -> bool {
    PROTOCOL_BOUNDARY_TAGS.contains(&tag)
}

/// Whether a `Raw` surface is deliberately retained fail-closed, not a fixable
/// lowering gap.
#[must_use]
pub(crate) fn is_intentional_raw_boundary_tag(tag: &str) -> bool {
    is_protocol_boundary_tag(tag)
        || INTENTIONAL_RAW_BOUNDARY_TAGS.contains(&tag)
        || tag.starts_with("go_goto ")
        || tag.starts_with("go_label ")
        || tag.starts_with("swift_labeled_break ")
        || tag.starts_with("swift_labeled_continue ")
        || tag.starts_with("swift_statement_label ")
        || tag.starts_with("type_case ")
}

use crate::type_domain_aliases::{
    ResolvedTypeDomain, TypeDomainAliases, TypeDomainEvidenceProvenance,
};
use nose_il::{
    stable_symbol_hash, DomainEvidence, EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter,
    EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta,
    Il, IlBuilder, ImportEvidenceKind, Interner, Lang, LibraryApiEvidenceKind, LitClass, NodeId,
    NodeKind, Op, Payload, PlaceEvidenceKind, RegionKind, SequenceSurfaceKind, SourceCallKind,
    SourceFactKind, SourceGranularity, SourceProtocolKind, Span, Symbol, SymbolEvidenceKind, Unit,
    UnitBodyKind, UnitDomain, UnitDomains, UnitEvidenceFlag, UnitKind, UnitOrigin, UnitSubkind,
};
use nose_semantics::{
    js_object_key_view_argument_dependency_ids_for_call, library_api_callee_contract_hash,
    library_api_contract_id_hash, library_api_free_name_shadow_safe,
    library_api_materialized_result_domain_for_arity,
    library_api_property_dependencies_for_field_with_cache,
    library_collection_factory_result_domain_for_arity, library_free_function_builtin_contract,
    library_free_name_collection_factory_contract, library_free_name_map_factory_contract,
    library_imported_collection_factory_contracts, library_imported_namespace_function_contract,
    library_java_collection_constructor_contract, library_java_collection_factory_contract,
    library_java_map_entry_contract, library_java_map_factory_contract,
    library_js_array_is_array_contract, library_js_boolean_coercion_contract,
    library_js_like_map_constructor_contract, library_js_like_set_constructor_contract,
    library_map_factory_result_domain, library_map_key_view_wrapper_contract,
    library_map_key_view_wrapper_result_domain, library_object_key_view_contract,
    library_promise_resolve_contract, library_property_builtin_contract,
    library_regex_test_contract, library_ruby_set_factory_contract,
    library_rust_option_none_sentinel_contract, library_rust_option_some_constructor_contract,
    library_rust_result_err_constructor_contract, library_rust_result_ok_constructor_contract,
    library_rust_vec_macro_factory_contract, library_rust_vec_new_factory_contract,
    library_static_collection_adapter_contract, library_static_index_membership_contract,
    library_swift_map_factory_contract, module_binding_mutating_method_contract,
    proven_receiver_method_api_contract_for_call_with_cache, qualified_global_symbol_contract,
    sequence_surface_kind_for_tag, type_domain_from_source_text, ImportFactKind,
    LibraryApiCalleeContract, LibraryApiContractId, LibraryApiDependencyCache,
    MethodReceiverContract, StaticIndexMembershipReceiverContract,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
    JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID, JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID,
    JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID, JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID,
    JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID, JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID, JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
    JS_LIKE_BUILTIN_REGEX_PRODUCER_ID, JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
    MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID, PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID,
    PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID, PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
    PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, PYTHON_STDLIB_MATH_PRODUCER_ID,
    RUBY_STDLIB_SET_PACK_ID, RUBY_STDLIB_SET_PRODUCER_ID,
    RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
    RUST_STDLIB_OPTION_PRODUCER_ID, RUST_STDLIB_RESULT_PRODUCER_ID, RUST_STDLIB_VEC_PRODUCER_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
};
use tree_sitter::Node as TsNode;

mod builder;
mod control_flow;
mod core_semantic_evidence;
mod evidence;
mod expr_helpers;
mod file;
mod imports;
mod library_api_evidence;
mod library_api_post_lower;
mod parse;
mod post_lower_evidence;
mod symbol_evidence;

pub(crate) use control_flow::{
    c_style_for, if_stmt, stmt_as_block, switch_to_if_chain, while_loop,
};
pub(crate) use expr_helpers::*;
pub(crate) use file::*;
pub(crate) use imports::*;
pub(crate) use parse::*;

pub(crate) struct Unsigned32Alias {
    pub alias: String,
    pub evidence: Option<EvidenceId>,
}

/// Mutable state threaded through a single file's lowering.
pub(crate) struct Lowering<'a> {
    pub b: IlBuilder,
    pub src: &'a [u8],
    pub lang: Lang,
    pub interner: &'a Interner,
    pub language_core_provenance: EvidenceProvenance,
    pub language_source_fact_provenance: EvidenceProvenance,
    pub units: Vec<Unit>,
    pub evidence: Vec<EvidenceRecord>,
    pub type_domain_aliases: TypeDomainAliases,
    pub unsigned_32_aliases: Vec<Unsigned32Alias>,
    /// Stack of `global`-declared names per enclosing function scope (Python). An
    /// assignment to a name on the top frame REBINDS the module binding, not a local —
    /// the frontend records that as a `ModuleRebind` source fact so the (otherwise
    /// information-losing) IL can distinguish it from a local declaration (#302).
    pub global_decls: Vec<rustc_hash::FxHashSet<Symbol>>,
}

impl<'a> Lowering<'a> {
    pub(crate) fn new(file: FileId, src: &'a [u8], lang: Lang, interner: &'a Interner) -> Self {
        let (core_pack_id, core_producer_id) =
            nose_semantics::language_core_evidence_provenance(lang);
        let (source_pack_id, source_producer_id) =
            nose_semantics::language_source_fact_provenance(lang);
        Lowering {
            b: IlBuilder::new(file),
            src,
            lang,
            interner,
            language_core_provenance: builtin_evidence_provenance(core_pack_id, core_producer_id),
            language_source_fact_provenance: builtin_evidence_provenance(
                source_pack_id,
                source_producer_id,
            ),
            units: Vec::new(),
            evidence: Vec::new(),
            type_domain_aliases: TypeDomainAliases::default(),
            unsigned_32_aliases: Vec::new(),
            global_decls: Vec::new(),
        }
    }

    /// Source text covered by a CST node.
    pub(crate) fn text(&self, n: TsNode) -> &'a str {
        n.utf8_text(self.src).unwrap_or("")
    }

    pub(crate) fn sym(&self, s: &str) -> Symbol {
        self.interner.intern(s)
    }

    /// Build a [`Span`] from a CST node (1-based inclusive lines).
    pub(crate) fn span(&self, n: TsNode) -> Span {
        let start_line = n.start_position().row as u32 + 1;
        // A node whose text ends in a newline "ends" at column 0 of the NEXT
        // row; counting that row would over-claim a line past the node's
        // content (and, for file-spanning units, past EOF — see #419).
        let end_pos = n.end_position();
        let mut end_line = end_pos.row as u32 + 1;
        if end_pos.column == 0 && end_line > start_line {
            end_line -= 1;
        }
        Span::new(
            self.b.file(),
            n.start_byte() as u32,
            n.end_byte() as u32,
            start_line,
            end_line,
        )
    }

    pub(crate) fn add(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        children: &[NodeId],
    ) -> NodeId {
        let id = self.b.add(kind, payload, span, children);
        self.record_core_semantic_evidence(kind, payload, span, children);
        if kind == NodeKind::Seq {
            let tag = match payload {
                Payload::None => None,
                Payload::Name(symbol) => Some(self.interner.resolve(symbol)),
                _ => return id,
            };
            if let Some(surface) = sequence_surface_kind_for_tag(self.lang, tag) {
                self.record_evidence(
                    EvidenceAnchor::sequence(span),
                    EvidenceKind::SequenceSurface(surface),
                    "sequence_surface",
                );
            }
        }
        id
    }
}

pub(crate) fn builtin_evidence_provenance(pack_id: &str, rule: &str) -> EvidenceProvenance {
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(rule)),
    }
}

#[allow(dead_code)]
pub(crate) fn first_party_evidence_provenance(pack_id: &str, rule: &str) -> EvidenceProvenance {
    builtin_evidence_provenance(pack_id, rule)
}

#[cfg(test)]
mod tests;
