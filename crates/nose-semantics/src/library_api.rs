//! Library and standard-library API contracts plus occurrence-evidence admission.
//!
//! Contract rows describe builtin API semantics. Admission remains separate:
//! consumers only rely on a contract after matching `LibraryApi` evidence and its
//! source/import/symbol/domain dependencies.

use super::*;

mod admission;
mod callee_dependencies;
mod callee_shape;
mod contract_keys;
mod contracts;
mod dependency_facts;
mod imported_occurrences;
mod receiver_dependencies;
mod registry;
mod resolvers;
mod result_domains;
mod rows;

pub(in crate::library_api) use callee_dependencies::*;
pub(in crate::library_api) use callee_shape::*;
pub use contracts::*;
pub(in crate::library_api) use dependency_facts::*;
pub(in crate::library_api) use receiver_dependencies::{
    async_receiver_dependencies_at_span, iterator_adapter_receiver_dependencies_at_span,
    method_receiver_dependencies_at_span, static_index_membership_receiver_dependency_id,
    static_index_membership_receiver_dependency_id_at_span,
};
pub(crate) use receiver_dependencies::{
    language_core_builtin_at_call, library_api_dependency_id_for_normalized_hof,
};
pub use registry::admitted_library_api_result_domain_for_call_record;
pub(in crate::library_api) use registry::{
    library_api_callee_contract_for_hash, library_api_contract_id_from_hash,
    library_api_contract_result_domain_for_arity, library_api_record_admitted_for_current_shape,
};
pub use resolvers::*;
pub use result_domains::*;
pub use rows::*;

pub(in crate::library_api) use admission::library_api_record_provenance_matches_contract;
pub use admission::{
    library_api_contract_evidence_at_call_span, library_api_contract_evidence_for_call,
    library_api_contract_evidence_for_node,
};
pub use imported_occurrences::{
    imported_occurrence_symbol_dependencies_valid,
    imported_occurrence_symbol_dependencies_valid_with_cache, ImportedOccurrenceValidationCache,
};
pub use receiver_dependencies::{
    library_api_dependency_id_for_canonical_builtin_call,
    library_api_dependency_id_for_canonical_builtin_call_with_interner,
    library_api_dependency_id_for_canonical_builtin_method_call,
    library_api_dependency_id_for_canonical_builtin_method_call_with_interner,
    library_api_property_dependencies_for_field_with_cache,
    library_api_receiver_dependencies_for_call,
    library_api_receiver_dependencies_for_call_with_cache, LibraryApiDependencyCache,
};
