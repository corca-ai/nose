//! Fixed result-domain materialization for admitted library API occurrences.

use super::contracts::{
    library_collection_factory_result_domain_for_arity, library_map_factory_result_domain,
    library_map_key_view_wrapper_result_domain, library_receiver_method_api_result_domain,
    JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID, JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
    JAVA_STDLIB_MAP_FACTORY_PACK_ID, JS_LIKE_BUILTIN_ARRAY_PACK_ID,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID, PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
    PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID, RUBY_STDLIB_SET_PACK_ID,
    RUST_STDLIB_COLLECTION_FACTORY_PACK_ID, RUST_STDLIB_MAP_FACTORY_PACK_ID,
    RUST_STDLIB_VEC_PACK_ID,
};
use super::*;
use crate::SEQ_VALUE_COLLECTION;
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
        | LibraryApiContractId::RustVecMacroFactory
        | LibraryApiContractId::RustVecNewFactory
        | LibraryApiContractId::JavaCollectionFactory(_)
        | LibraryApiContractId::JavaCollectionConstructor(_)
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => {
            library_collection_factory_result_domain_for_arity(
                LibraryCollectionFactoryContract {
                    pack_id: match id {
                        LibraryApiContractId::PythonBuiltinCollectionFactory => {
                            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID
                        }
                        LibraryApiContractId::PythonImportedCollectionFactory => {
                            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID
                        }
                        LibraryApiContractId::RustVecMacroFactory
                        | LibraryApiContractId::RustVecNewFactory => RUST_STDLIB_VEC_PACK_ID,
                        LibraryApiContractId::RustStdCollectionFactory => {
                            RUST_STDLIB_COLLECTION_FACTORY_PACK_ID
                        }
                        LibraryApiContractId::JavaCollectionFactory(_) => {
                            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID
                        }
                        LibraryApiContractId::JavaCollectionConstructor(_) => {
                            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
                        }
                        LibraryApiContractId::RubySetFactory => RUBY_STDLIB_SET_PACK_ID,
                        LibraryApiContractId::JsLikeSetConstructor => {
                            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
                        }
                        _ => unreachable!("collection-factory contract has no builtin pack"),
                    },
                    id,
                    callee,
                    result: LibraryCollectionFactoryResult::SequenceArgument,
                },
                arity as usize,
            )
        }
        LibraryApiContractId::RustStdMapFactory
        | LibraryApiContractId::JavaMapFactory(_)
        | LibraryApiContractId::JsLikeMapConstructor => Some(library_map_factory_result_domain(
            LibraryMapFactoryContract {
                pack_id: match id {
                    LibraryApiContractId::RustStdMapFactory => RUST_STDLIB_MAP_FACTORY_PACK_ID,
                    LibraryApiContractId::JavaMapFactory(_) => JAVA_STDLIB_MAP_FACTORY_PACK_ID,
                    LibraryApiContractId::JsLikeMapConstructor => {
                        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
                    }
                    _ => unreachable!("map-factory contract has no builtin pack"),
                },
                id,
                callee,
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_COLLECTION,
                },
            },
        )),
        LibraryApiContractId::MapKeyViewWrapper => Some(
            library_map_key_view_wrapper_result_domain(LibraryMapKeyViewWrapperContract {
                pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
                id,
                callee,
                result: MapKeyViewWrapperContract {
                    receiver: "Array",
                    method: "from",
                    qualified_path: "Array.from",
                },
            }),
        ),
        LibraryApiContractId::RustOptionSomeConstructor => Some(DomainEvidence::Option),
        LibraryApiContractId::PromiseFactory(_) => Some(DomainEvidence::PromiseLike),
        id @ (LibraryApiContractId::RustOptionAndThen
        | LibraryApiContractId::ScalarIntegerMethod(_)
        | LibraryApiContractId::MapKeyView(_)
        | LibraryApiContractId::PromiseThen) => library_receiver_method_api_result_domain(id),
        _ => None,
    }
}
