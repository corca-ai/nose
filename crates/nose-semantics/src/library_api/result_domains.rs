//! Fixed result-domain materialization for admitted library API occurrences.

use super::contracts::{
    library_collection_factory_result_domain_for_arity, library_map_factory_result_domain,
    library_map_key_view_wrapper_result_domain, library_receiver_method_api_result_domain,
    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID, JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID, JAVA_STDLIB_MAP_FACTORY_PACK_ID,
    JS_LIKE_BUILTIN_ARRAY_PACK_ID, JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
    PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID, PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
    RUBY_STDLIB_SET_PACK_ID, RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
    RUST_STDLIB_MAP_FACTORY_PACK_ID, RUST_STDLIB_VEC_PACK_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
};
use super::*;
use crate::{SEQ_VALUE_COLLECTION, SEQ_VALUE_TUPLE};
use nose_il::DomainEvidence;

pub fn library_api_materialized_result_domain_for_arity(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<DomainEvidence> {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::PythonImportedCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory
        | LibraryApiContractId::SwiftCollectionFactory(_)
        | LibraryApiContractId::RustVecMacroFactory
        | LibraryApiContractId::RustVecNewFactory
        | LibraryApiContractId::JavaCollectionFactory(_)
        | LibraryApiContractId::JavaCollectionConstructor(_)
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => {
            collection_factory_materialized_result_domain(id, callee, arity as usize)
        }
        LibraryApiContractId::RustStdMapFactory | LibraryApiContractId::JsLikeMapConstructor => {
            entry_sequence_map_factory_materialized_result_domain(id, callee, arity as usize)
        }
        LibraryApiContractId::SwiftMapFactory(_) => None,
        LibraryApiContractId::JavaMapFactory(kind) => {
            java_map_factory_materialized_result_domain(id, callee, kind, arity as usize)
        }
        LibraryApiContractId::MapKeyViewWrapper => {
            map_key_view_wrapper_materialized_result_domain(id, callee)
        }
        LibraryApiContractId::RustOptionSomeConstructor => Some(DomainEvidence::Option),
        LibraryApiContractId::PromiseFactory(_) => Some(DomainEvidence::PromiseLike),
        id @ (LibraryApiContractId::RustOptionAndThen
        | LibraryApiContractId::ScalarIntegerMethod(_)
        | LibraryApiContractId::MapKeyView(_)
        | LibraryApiContractId::PromiseThen) => library_receiver_method_api_result_domain(id),
        _ => None,
    }
}

fn collection_factory_materialized_result_domain(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: usize,
) -> Option<DomainEvidence> {
    library_collection_factory_result_domain_for_arity(
        LibraryCollectionFactoryContract {
            pack_id: collection_factory_materialized_pack_id(id),
            id,
            callee,
            result: collection_factory_materialized_result(id),
        },
        arity,
    )
}

fn collection_factory_materialized_pack_id(id: LibraryApiContractId) -> &'static str {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory => {
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID
        }
        LibraryApiContractId::RustVecMacroFactory | LibraryApiContractId::RustVecNewFactory => {
            RUST_STDLIB_VEC_PACK_ID
        }
        LibraryApiContractId::RustStdCollectionFactory => RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
        LibraryApiContractId::SwiftCollectionFactory(_) => SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
        LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::GuavaImmutableListOf
            | JavaCollectionFactoryKind::GuavaImmutableSetOf,
        ) => JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
        LibraryApiContractId::JavaCollectionFactory(_) => JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
        LibraryApiContractId::JavaCollectionConstructor(_) => {
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
        }
        LibraryApiContractId::RubySetFactory => RUBY_STDLIB_SET_PACK_ID,
        LibraryApiContractId::JsLikeSetConstructor => {
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
        }
        _ => unreachable!("collection-factory contract has no builtin pack"),
    }
}

fn collection_factory_materialized_result(
    id: LibraryApiContractId,
) -> LibraryCollectionFactoryResult {
    match id {
        LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsEmptyList
            | JavaCollectionFactoryKind::CollectionsEmptySet,
        ) => LibraryCollectionFactoryResult::EmptySequence,
        LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsSingleton
            | JavaCollectionFactoryKind::CollectionsSingletonList,
        ) => LibraryCollectionFactoryResult::ElementArguments,
        _ => LibraryCollectionFactoryResult::SequenceArgument,
    }
}

fn entry_sequence_map_factory_materialized_result_domain(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: usize,
) -> Option<DomainEvidence> {
    if matches!(id, LibraryApiContractId::SwiftMapFactory(_)) && arity != 1 {
        return None;
    }
    Some(library_map_factory_result_domain(
        LibraryMapFactoryContract {
            pack_id: match id {
                LibraryApiContractId::RustStdMapFactory => RUST_STDLIB_MAP_FACTORY_PACK_ID,
                LibraryApiContractId::SwiftMapFactory(_) => SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
                LibraryApiContractId::JsLikeMapConstructor => {
                    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
                }
                _ => unreachable!("map-factory contract has no builtin pack"),
            },
            id,
            callee,
            result: LibraryMapFactoryResult::EntrySequence {
                entry_seq_tag: match id {
                    LibraryApiContractId::SwiftMapFactory(_) => SEQ_VALUE_TUPLE,
                    _ => SEQ_VALUE_COLLECTION,
                },
            },
        },
    ))
}

fn java_map_factory_materialized_result_domain(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    kind: JavaMapFactoryKind,
    arity: usize,
) -> Option<DomainEvidence> {
    java_map_factory_result_domain_arg_count_supported(kind, arity).then(|| {
        library_map_factory_result_domain(LibraryMapFactoryContract {
            pack_id: match kind {
                JavaMapFactoryKind::GuavaImmutableMapOf => {
                    JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID
                }
                _ => JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            },
            id,
            callee,
            result: LibraryMapFactoryResult::JavaFactory { kind },
        })
    })
}

fn map_key_view_wrapper_materialized_result_domain(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<DomainEvidence> {
    Some(library_map_key_view_wrapper_result_domain(
        LibraryMapKeyViewWrapperContract {
            pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            id,
            callee,
            result: MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
            },
        },
    ))
}
