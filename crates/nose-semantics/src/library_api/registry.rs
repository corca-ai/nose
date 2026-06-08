//! Contract registry helpers for resolving evidence hashes back to first-party rows.

use super::*;

pub(super) fn library_api_contract_result_domain_for_arity(
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
                id,
                callee,
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_COLLECTION,
                },
            },
        )),
        LibraryApiContractId::MapKeyViewWrapper => Some(
            library_map_key_view_wrapper_result_domain(LibraryMapKeyViewWrapperContract {
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
        LibraryApiContractId::ScalarIntegerMethod(_) => Some(DomainEvidence::Integer),
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(_)) => {
            Some(DomainEvidence::Collection)
        }
        _ => None,
    }
}

pub(super) fn library_api_contract_id_from_hash(hash: u64) -> Option<LibraryApiContractId> {
    library_api_contract_ids()
        .into_iter()
        .find(|id| library_api_contract_id_hash(*id) == hash)
}

fn library_api_contract_ids() -> Vec<LibraryApiContractId> {
    let mut ids = vec![
        LibraryApiContractId::PropertyBuiltin(Builtin::Len),
        LibraryApiContractId::PythonBuiltinCollectionFactory,
        LibraryApiContractId::PythonImportedCollectionFactory,
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Append),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Print),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Range),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Sum),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Min),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Max),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Abs),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Enumerate),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::All),
        LibraryApiContractId::RustOptionSomeConstructor,
        LibraryApiContractId::RustOptionNoneSentinel,
        LibraryApiContractId::RustOptionAndThen,
        LibraryApiContractId::RustStdCollectionFactory,
        LibraryApiContractId::RustStdMapFactory,
        LibraryApiContractId::RustVecMacroFactory,
        LibraryApiContractId::RustVecNewFactory,
        LibraryApiContractId::JavaMapEntryFactory,
        LibraryApiContractId::RubySetFactory,
        LibraryApiContractId::JsLikeSetConstructor,
        LibraryApiContractId::JsLikeMapConstructor,
        LibraryApiContractId::MapKeyViewWrapper,
        LibraryApiContractId::MapGet,
        LibraryApiContractId::JsArrayIsArray,
        LibraryApiContractId::JsBooleanCoercion,
        LibraryApiContractId::RegexTest,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::IndexOf),
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::FindIndex),
        LibraryApiContractId::PromiseThen,
        LibraryApiContractId::IteratorIdentityAdapter,
        LibraryApiContractId::StaticCollectionAdapter,
    ];
    ids.extend(
        [
            ScalarIntegerMethod::Abs,
            ScalarIntegerMethod::Min,
            ScalarIntegerMethod::Max,
            ScalarIntegerMethod::Clamp,
        ]
        .into_iter()
        .map(LibraryApiContractId::ScalarIntegerMethod),
    );
    ids.extend(
        [
            JavaCollectionFactoryKind::ListOf,
            JavaCollectionFactoryKind::SetOf,
            JavaCollectionFactoryKind::ArraysAsList,
        ]
        .into_iter()
        .map(LibraryApiContractId::JavaCollectionFactory),
    );
    ids.push(LibraryApiContractId::JavaCollectionConstructor(
        JavaCollectionConstructorKind::EmptyList,
    ));
    ids.extend(
        [JavaMapFactoryKind::Of, JavaMapFactoryKind::OfEntries]
            .into_iter()
            .map(LibraryApiContractId::JavaMapFactory),
    );
    ids.extend(
        [MapKeyViewKind::Collection, MapKeyViewKind::Iterator]
            .into_iter()
            .map(LibraryApiContractId::MapKeyView),
    );
    ids.extend(
        [ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        }]
        .into_iter()
        .map(LibraryApiContractId::ImportedNamespaceFunction),
    );
    ids.extend(
        [
            MethodSemanticContract::Builtin(Builtin::Append),
            MethodSemanticContract::Builtin(Builtin::Print),
            MethodSemanticContract::Builtin(Builtin::Len),
            MethodSemanticContract::Builtin(Builtin::IsEmpty),
            MethodSemanticContract::Builtin(Builtin::IsNull),
            MethodSemanticContract::Builtin(Builtin::IsNotNull),
            MethodSemanticContract::Builtin(Builtin::StartsWith),
            MethodSemanticContract::Builtin(Builtin::EndsWith),
            MethodSemanticContract::Builtin(Builtin::Contains),
            MethodSemanticContract::Builtin(Builtin::Join),
            MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            MethodSemanticContract::Builtin(Builtin::ValueOrDefault),
            MethodSemanticContract::Builtin(Builtin::Reduce),
            MethodSemanticContract::Builtin(Builtin::Sum),
            MethodSemanticContract::Builtin(Builtin::Abs),
            MethodSemanticContract::Builtin(Builtin::Min),
            MethodSemanticContract::Builtin(Builtin::Max),
            MethodSemanticContract::Builtin(Builtin::Zip),
            MethodSemanticContract::Builtin(Builtin::Any),
            MethodSemanticContract::Builtin(Builtin::All),
            MethodSemanticContract::HoF(HoFKind::Map),
            MethodSemanticContract::HoF(HoFKind::Filter),
            MethodSemanticContract::HoF(HoFKind::FlatMap),
            MethodSemanticContract::HoF(HoFKind::FilterMap),
        ]
        .into_iter()
        .map(LibraryApiContractId::MethodCall),
    );
    ids
}

pub(super) fn library_api_record_admitted_for_current_shape(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    record: &EvidenceRecord,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash,
        callee_hash,
        arity,
    }) = record.kind
    else {
        return false;
    };
    let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
        return false;
    };
    let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash) else {
        return false;
    };
    matches!(
        library_api_contract_evidence_for_call(il, interner, call, id, callee, arity as usize),
        LibraryApiEvidenceStatus::Admitted
    )
}

pub(super) fn library_api_callee_contract_for_hash(
    lang: Lang,
    id: LibraryApiContractId,
    hash: u64,
) -> Option<LibraryApiCalleeContract> {
    library_api_callee_contracts_for_id(lang, id)
        .into_iter()
        .find(|callee| library_api_callee_contract_hash(*callee) == hash)
}

fn library_api_callee_contracts_for_id(
    lang: Lang,
    id: LibraryApiContractId,
) -> Vec<LibraryApiCalleeContract> {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin) => ["length"]
            .into_iter()
            .filter_map(|property| library_property_builtin_contract(lang, property))
            .filter(|contract| contract.id == LibraryApiContractId::PropertyBuiltin(builtin))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory => {
            library_free_name_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            library_imported_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            library_free_function_builtin_callee_contracts_for_id(lang, builtin)
        }
        LibraryApiContractId::RustOptionSomeConstructor => [
            "Some",
            "Option::Some",
            "std::option::Option::Some",
            "core::option::Option::Some",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_some_constructor_contract(lang, name, 1))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionNoneSentinel => [
            "None",
            "Option::None",
            "std::option::Option::None",
            "core::option::Option::None",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_none_sentinel_contract(lang, name))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionAndThen => {
            library_rust_option_and_then_contract(lang, "and_then", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::ScalarIntegerMethod(method) => ["abs", "min", "max", "clamp"]
            .into_iter()
            .filter_map(|name| library_scalar_integer_method_contract(lang, name, 0))
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 1)),
            )
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 2)),
            )
            .filter(|contract| contract.id == LibraryApiContractId::ScalarIntegerMethod(method))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustStdMapFactory => library_free_name_map_factory_contracts(lang)
            .filter(|contract| contract.id == id)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustVecMacroFactory => {
            library_rust_vec_macro_factory_contract(lang, "vec")
                .filter(|contract| contract.id == id)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RustVecNewFactory => {
            ["Vec::new", "std::vec::Vec::new", "alloc::vec::Vec::new"]
                .into_iter()
                .filter_map(|name| library_rust_vec_new_factory_contract(lang, name))
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionFactory(kind) => {
            [("List", "of"), ("Set", "of"), ("Arrays", "asList")]
                .into_iter()
                .filter_map(|(receiver, method)| {
                    library_java_collection_factory_contract(lang, receiver, method)
                })
                .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionFactory(kind))
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionConstructor(kind) => [
            "ArrayList",
            "java.util.ArrayList",
            "LinkedList",
            "java.util.LinkedList",
        ]
        .into_iter()
        .filter_map(|type_name| library_java_collection_constructor_contract(lang, type_name, 0))
        .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionConstructor(kind))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::JavaMapFactory(kind) => ["of", "ofEntries"]
            .into_iter()
            .filter_map(|method| library_java_map_factory_contract(lang, "Map", method))
            .filter(|contract| contract.id == LibraryApiContractId::JavaMapFactory(kind))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::JavaMapEntryFactory => {
            library_java_map_entry_contract(lang, "Map", "entry")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RubySetFactory => {
            library_ruby_set_factory_contract(lang, "Set", "new", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeSetConstructor => {
            library_js_like_set_constructor_contract(lang, "Set")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeMapConstructor => {
            library_js_like_map_constructor_contract(lang, "Map")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MapKeyViewWrapper => {
            library_map_key_view_wrapper_contract(lang, "Array", "from", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeStaticIndexMembership(kind) => ["indexOf", "findIndex"]
            .into_iter()
            .filter_map(|method| library_static_index_membership_contract(lang, method, 1))
            .filter(|contract| {
                contract.id == LibraryApiContractId::JsLikeStaticIndexMembership(kind)
            })
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::MapGet => ["get"]
            .into_iter()
            .filter_map(|method| {
                library_map_get_contract(lang, method, 1).map(|contract| contract.callee)
            })
            .collect(),
        LibraryApiContractId::MapKeyView(kind) => ["keys", "keySet"]
            .into_iter()
            .filter_map(|method| library_map_key_view_contract(lang, method, 0))
            .filter(|contract| contract.result.kind == kind)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::IteratorIdentityAdapter => {
            let methods = [
                "iter",
                "into_iter",
                "iter_mut",
                "collect",
                "to_vec",
                "copied",
                "cloned",
                "stream",
            ];
            methods
                .into_iter()
                .filter_map(|method| {
                    library_iterator_identity_adapter_contract(lang, method, 0)
                        .map(|contract| contract.callee)
                })
                .collect()
        }
        LibraryApiContractId::StaticCollectionAdapter => {
            library_static_collection_adapter_contract(lang, "Arrays", "stream", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MethodCall(semantic) => {
            method_call_contract_callees_for_semantic(lang, semantic)
        }
        _ => Vec::new(),
    }
}

fn library_free_function_builtin_callee_contracts_for_id(
    lang: Lang,
    builtin: Builtin,
) -> Vec<LibraryApiCalleeContract> {
    let candidate = match (lang, builtin) {
        (Lang::Python, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Append) => Some(("append", 2)),
        (Lang::Python, Builtin::Print) => Some(("print", 0)),
        (Lang::Python, Builtin::Range) => Some(("range", 1)),
        (Lang::Python, Builtin::Sum) => Some(("sum", 1)),
        (Lang::Python, Builtin::Min) => Some(("min", 1)),
        (Lang::Python, Builtin::Max) => Some(("max", 1)),
        (Lang::Python, Builtin::Abs) => Some(("abs", 1)),
        (Lang::Python, Builtin::Zip) => Some(("zip", 2)),
        (Lang::Python, Builtin::Enumerate) => Some(("enumerate", 1)),
        (Lang::Python, Builtin::Any) => Some(("any", 1)),
        (Lang::Python, Builtin::All) => Some(("all", 1)),
        _ => None,
    };
    candidate
        .and_then(|(name, arg_count)| library_free_function_builtin_contract(lang, name, arg_count))
        .map(|contract| vec![contract.callee])
        .unwrap_or_default()
}

fn method_call_contract_callees_for_semantic(
    lang: Lang,
    semantic: MethodSemanticContract,
) -> Vec<LibraryApiCalleeContract> {
    let methods = [
        "append",
        "push",
        "log",
        "info",
        "debug",
        "Println",
        "Printf",
        "Print",
        "Abs",
        "HasPrefix",
        "HasSuffix",
        "Contains",
        "len",
        "size",
        "length",
        "is_empty",
        "isEmpty",
        "empty?",
        "nil?",
        "is_none",
        "is_some",
        "startsWith",
        "startswith",
        "starts_with",
        "start_with?",
        "endsWith",
        "endswith",
        "ends_with",
        "end_with?",
        "containsKey",
        "contains_key",
        "key?",
        "has_key?",
        "__contains__",
        "includes",
        "include?",
        "member?",
        "contains",
        "has",
        "join",
        "get",
        "fetch",
        "getOrDefault",
        "unwrap_or",
        "unwrap_or_else",
        "map_or",
        "reduce",
        "Min",
        "Max",
        "abs",
        "min",
        "max",
        "zip",
        "fold",
        "inject",
        "map",
        "collect",
        "filter",
        "select",
        "flatMap",
        "flat_map",
        "filter_map",
        "some",
        "every",
        "all",
        "any",
        "all?",
        "any?",
        "allMatch",
        "anyMatch",
        "sum",
        "count",
    ];
    methods
        .into_iter()
        .flat_map(|method| {
            (0..=3).filter_map(move |arity| library_method_call_contract(lang, method, arity))
        })
        .filter(|contract| contract.result.semantic == semantic)
        .map(|contract| contract.callee)
        .collect()
}
