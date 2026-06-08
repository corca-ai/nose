//! Track 1 Stage 3 — value-graph / global value numbering (GVN).
//!
//! Symbolically evaluates a function unit into a DAG of *values*, hash-consed by
//! `(op, operand-value-ids)`. Because a variable maps to the value it currently
//! holds (not its name), and identical computations intern to one node:
//!
//! - temporaries and intermediate names dissolve (`t=a+b; …t…` ≡ inline),
//! - common subexpressions share a node (CSE),
//! - the order of data-independent statements stops mattering,
//! - commutative operands are canonical.
//!
//! Branches merge variables with `Phi(cond, then, else)`. Loops are approximated:
//! variables written in the body become opaque loop values (no fixpoint — bounded
//! and deterministic). Calls/stores are treated as values too (fuzzy: identical
//! calls CSE). The per-unit **fingerprint** is the multiset of value-node hashes
//! reachable from the unit's sinks (returns, throws, branch conditions, effects).
//!
//! This is a *detection substrate*, not an IL rewrite: it returns a fingerprint
//! the detector can use instead of (or alongside) subtree shapes.
//!
//! CONVENTION: a meaning-preserving *canonicalization* (a value rewrite) needs Lean evidence.
//! Name it `canonicalize_*` and list it in an obligation's `[rust].symbols` — the formal
//! obligation gate (`scripts/check-formal-obligations.py`) ENFORCES that every `canonicalize_*`
//! fn is covered by some obligation (the name is the declaration; no separate marker needed). A
//! canon under another name must be registered in that script's REQUIRED_OBLIGATIONS, or it
//! skips the gate (the gap that let `.then`/`pure_inline` slip).
//!
mod api;
mod builders;
mod canonicalize;
mod collections;
mod context;
mod control;
mod eval;
mod field_state;
mod inline;
mod model;
mod ops;
mod output;
mod rules;
mod sinks;
mod state;
mod stdlib;

pub use api::{
    value_anchors, value_fingerprint, value_fingerprint_and_contracts, value_fingerprint_contracts,
    value_fingerprint_lits, value_fingerprint_lits_anchors,
    value_fingerprint_lits_anchors_with_context, value_fingerprint_lits_with_context, Anchor,
    Anchors, FingerprintBundle, ANCHOR_MIN_WEIGHT,
};
pub use context::ValueFingerprintContext;

use crate::combine;
use crate::module_facts::{
    assignment_name_in_scope, collect_all_node_symbols_in_scope,
    collect_module_mutations_in_scope_with_direct_definitions, local_scope_nodes,
    node_symbol_in_scope, shadowed_js_like_module_binding_nodes_for_symbol_in_scope,
    top_level_statements_for,
};
use field_state::FieldStateKey;
use model::{
    Builder, BuilderCandidate, BuilderKind, FilterMapResult, HofAdmission, InlineFunction,
    LoopRecurrenceScope, ReductionCache, SignedExprOperand, Sink, SinkKind, ValNode, ValOp,
    ValueId,
};
use nose_il::{
    stable_symbol_hash, Builtin, EffectEvidenceKind, HoFKind, Il, Interner, Lang, LoopKind, NodeId,
    NodeKind, Op, Payload, SourceCastKind, SourceComprehensionKind, SourceFactKind,
    SourcePatternKind, SourceRangeKind, Span, Symbol,
};
use nose_semantics::{
    admitted_builder_append_method_call_args, admitted_builtin_semantics_at_call,
    admitted_free_function_builtin_at_call, admitted_free_name_collection_factory_at_call_span,
    admitted_free_name_map_factory_at_call_span, admitted_imported_collection_factory_at_call_span,
    admitted_imported_namespace_function_at_call, admitted_iterator_identity_adapter_at_call,
    admitted_java_collection_constructor_at_call, admitted_java_collection_factory_at_call_span,
    admitted_java_map_entry_at_call, admitted_java_map_entry_at_call_span,
    admitted_java_map_factory_at_call, admitted_java_map_factory_at_call_span,
    admitted_js_like_map_constructor_at_call, admitted_js_like_set_constructor_at_call,
    admitted_library_method_call_at_call, admitted_map_get_at_call_span,
    admitted_map_key_view_at_call_span, admitted_map_key_view_wrapper_at_call_span,
    admitted_property_builtin_at_field, admitted_ruby_set_factory_at_call_span,
    admitted_rust_option_and_then_at_call, admitted_rust_option_none_sentinel_at_node,
    admitted_rust_option_some_constructor_at_call, admitted_rust_option_some_constructor_at_node,
    admitted_rust_vec_macro_factory_at_call_span, admitted_rust_vec_new_factory_at_call,
    admitted_scalar_integer_method_at_call, admitted_static_index_membership_at_call,
    asserted_unshadowed_global_symbol, binding_write_target, builder_append_call_args, builtin_tag,
    construct_syntax_proof, contracted_builder_append_method_call_args,
    domain_evidence_for_param as semantic_domain_evidence_for_param,
    exact_non_overloadable_index_assignment_parts, exact_static_membership_predicate_operator,
    go_zero_map_default_kind, go_zero_map_entry_contract_for_node,
    go_zero_map_literal_contract_for_node, go_zero_map_lookup_contract, import_fact_evidence_rhs,
    imported_literal_producer_evidence_for_node, imported_namespace_symbol,
    map_builder_index_write_contract, nullish_global_contract, opaque_argument_escape_args,
    own_property_guard_for_node, receiver_mutation_call_receiver, record_shape_guard_for_node,
    reduction_builtin_contract, semantics, seq_surface_contract_for_node,
    source_comprehension_at_node, source_operator_at_node, source_pattern_at_node,
    source_range_at_node, unproven_membership_like_method_contract, BuiltinArgContract,
    CBytePackWidth, CardinalityPredicate, CardinalityThreshold, ComparisonLaw, DomainEvidence,
    DomainRequirement, GoZeroMapDefaultKind, ImportFactKind, ImportedNamespaceFunctionSemantic,
    IndexMembershipThreshold, IndexWriteReceiverContract, IteratorAdapterReceiverContract,
    JavaMapFactoryKind, LibraryApiCalleeContract, LibraryApiSpanCall,
    LibraryCollectionFactoryResult, LibraryMapFactoryResult, MapKeyViewKind, MethodBuiltinArgs,
    MethodReceiverContract, MethodSemanticContract, ReductionBuiltinContract, ScalarIntegerMethod,
    SeqSurfaceContract, StaticIndexMembershipKind, ValueDomain, ValueLaw, SEQ_VALUE_COLLECTION,
    SEQ_VALUE_MAP, SEQ_VALUE_OWN_PROPERTY_GUARD, SEQ_VALUE_PAIR, SEQ_VALUE_RECORD_GUARD,
    SEQ_VALUE_UNTAGGED,
};
use ops::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;
