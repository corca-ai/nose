//! Strict exact-safety proof gates for semantic unit extraction.
//!
//! This module owns the fail-closed checks that decide whether a normalized IL
//! subtree can participate in the exact semantic channel. Unit extraction keeps
//! orchestration in `units.rs`; proof policy lives here.

use nose_il::{
    stable_symbol_hash, Builtin, CallTargetEvidenceKind, EvidenceAnchor, EvidenceKind, HoFKind, Il,
    Interner, Lang, LitClass, NodeId, NodeKind, Op, Payload, SourceBindingKind,
    SourceComprehensionKind, SourceFactKind, Span, Symbol,
};
use nose_normalize::module_facts::collect_module_mutations;
use nose_semantics::{
    admitted_builtin_semantics_at_call_with_interner,
    admitted_free_name_collection_factory_at_call, admitted_free_name_map_factory_at_call,
    admitted_hof_demand_effect_profile_at_node, admitted_imported_collection_factory_at_call,
    admitted_iterator_identity_adapter_at_call, admitted_java_collection_constructor_at_call,
    admitted_java_collection_factory_at_call, admitted_java_map_entry_at_call,
    admitted_java_map_factory_at_call, admitted_js_array_is_array_at_call,
    admitted_js_like_map_constructor_at_call, admitted_js_like_set_constructor_at_call,
    admitted_library_method_call_at_call, admitted_map_get_at_call, admitted_map_key_view_at_call,
    admitted_map_key_view_wrapper_at_call, admitted_regex_test_at_call,
    admitted_ruby_set_factory_at_call, admitted_rust_option_none_sentinel_at_node,
    admitted_rust_vec_macro_factory_at_call, admitted_rust_vec_new_factory_at_call,
    admitted_static_index_membership_at_call, admitted_terminal_count_reduction_at_call,
    asserted_unshadowed_global_symbol, call_target_evidence_status_at_call, construct_syntax_proof,
    direct_function_call_target_at_call, direct_method_call_target_at_call,
    exact_static_membership_predicate_operator, go_zero_map_default_kind,
    go_zero_map_entry_contract_for_node, go_zero_map_literal_contract_for_node,
    go_zero_map_lookup_contract, java_collection_factory_rejects_null_literal,
    java_map_factory_positional_arg_count_supported, java_map_factory_uses_positional_entries,
    nodes_contain_duplicate_static_literal_keys, nodes_contain_static_null_literal,
    nullish_global_contract, own_property_guard_for_node, record_shape_guard_for_node, semantics,
    seq_surface_contract_for_node, source_comprehension_at_node, source_fact_at_node,
    source_operator_at_node, typeof_operator_contract, CallTargetEvidenceStatus, DomainRequirement,
    IndexMembershipThreshold, JavaMapFactoryKind, LibraryApiContractId,
    LibraryCollectionFactoryResult, LibraryMapFactoryResult, LibraryMethodCallContract,
    MapKeyViewKind, MethodBuiltinArgs, MethodReceiverContract, MethodSemanticContract,
    ReceiverDomainEvidenceIndex, StaticIndexMembershipKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

mod calls;
mod collections;
mod factories;
mod facts;
mod hof;
mod identity;
mod primitives;
mod static_index;
mod tree;

#[cfg(test)]
mod tests;

use calls::{admitted_method_call_contract, field_receiver, strict_exact_safe_call};
pub(crate) use collections::{
    strict_exact_collection_contains_call_safe, strict_exact_membership_collection_safe,
};
use collections::{
    strict_exact_iterator_identity_adapter_call_safe, strict_exact_map_contains_call_safe,
    strict_exact_map_get_call_safe, strict_exact_map_get_default_call_safe,
    strict_exact_map_key_view_collection_safe, strict_exact_proven_collection_receiver_safe,
    strict_exact_proven_map_receiver_safe,
};
use factories::{
    strict_exact_go_literal_zero_map_index_safe, strict_exact_java_collection_constructor_safe,
    strict_exact_map_constructor_entries_safe, strict_exact_ruby_set_factory_safe,
    strict_exact_rust_std_collection_factory_safe, strict_exact_rust_std_map_factory_safe,
    strict_exact_rust_vec_macro_collection_safe, strict_exact_rust_vec_new_safe,
    strict_exact_swift_default_subscript_index_safe,
};
pub(crate) use factories::{
    strict_exact_java_collection_factory_safe, strict_exact_java_map_factory_safe,
    strict_exact_python_collection_factory_safe, strict_exact_set_constructor_collection_safe,
};
pub(crate) use facts::StrictFacts;
use hof::{
    strict_exact_in_membership_safe, strict_exact_len_arg_safe, strict_exact_safe_hof,
    strict_exact_terminal_reduction_arg_safe,
};
use identity::{strict_exact_call_args_safe, strict_exact_callee_identity};
use primitives::{
    exact_literal_safe, strict_exact_nullish_global_safe, strict_exact_rust_option_none_safe,
    strict_exact_safe_seq, strict_exact_safe_var,
};
use static_index::{
    strict_exact_static_index_membership_safe, strict_exact_static_non_float_collection,
};
pub(crate) use tree::{function_binding_safe, strict_exact_safe_tree};
