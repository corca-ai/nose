//! Contract registry helpers for resolving evidence hashes back to builtin rows.

use super::*;

mod contract_ids;
use contract_ids::library_api_contract_ids;

pub(super) fn library_api_contract_result_domain_for_arity(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<DomainEvidence> {
    library_api_materialized_result_domain_for_arity(id, callee, arity).or(match id {
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)
        | LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate) => {
            Some(DomainEvidence::Iterator)
        }
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(_)) => {
            Some(DomainEvidence::Collection)
        }
        _ => None,
    })
}

pub fn admitted_library_api_result_domain_for_call_record(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    record: &EvidenceRecord,
) -> Option<DomainEvidence> {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash,
        callee_hash,
        arity,
    }) = record.kind
    else {
        return None;
    };
    if il.children(call).len().saturating_sub(1) != arity as usize {
        return None;
    }
    let id = library_api_contract_id_from_hash(contract_hash)?;
    let callee = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)?;
    if matches!(id, LibraryApiContractId::SwiftMapFactory(_)) {
        return None;
    }
    let domain = library_api_materialized_result_domain_for_arity(id, callee, arity)?;
    matches!(
        library_api_contract_evidence_for_call(il, interner, call, id, callee, arity as usize),
        LibraryApiEvidenceStatus::Admitted
    )
    .then_some(domain)
}

pub(super) fn library_api_contract_id_from_hash(hash: u64) -> Option<LibraryApiContractId> {
    library_api_contract_ids()
        .into_iter()
        .find(|id| library_api_contract_id_hash(*id) == hash)
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
        LibraryApiContractId::PropertyBuiltin(builtin) => ["length", "count", "isEmpty"]
            .into_iter()
            .filter_map(|property| library_property_builtin_contract(lang, property))
            .filter(|contract| contract.id == LibraryApiContractId::PropertyBuiltin(builtin))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory
        | LibraryApiContractId::SwiftCollectionFactory(_) => {
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
        LibraryApiContractId::FreeFunctionHof(kind) => {
            library_free_function_hof_callee_contracts_for_id(lang, kind)
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
        LibraryApiContractId::RustResultOkConstructor => [
            "Ok",
            "Result::Ok",
            "std::result::Result::Ok",
            "core::result::Result::Ok",
        ]
        .into_iter()
        .filter_map(|name| library_rust_result_ok_constructor_contract(lang, name, 1))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustResultErrConstructor => [
            "Err",
            "Result::Err",
            "std::result::Result::Err",
            "core::result::Result::Err",
        ]
        .into_iter()
        .filter_map(|name| library_rust_result_err_constructor_contract(lang, name, 1))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustResultIsOk => {
            library_rust_result_predicate_contract(lang, "is_ok", 0)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RustResultIsErr => {
            library_rust_result_predicate_contract(lang, "is_err", 0)
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
        _ => library_api_factory_callee_contracts_for_id(lang, id),
    }
}

fn library_api_factory_callee_contracts_for_id(
    lang: Lang,
    id: LibraryApiContractId,
) -> Vec<LibraryApiCalleeContract> {
    match id {
        LibraryApiContractId::RustStdMapFactory => library_free_name_map_factory_contracts(lang)
            .filter(|contract| contract.id == id)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::SwiftMapFactory(_) => library_swift_map_factory_contracts(lang)
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
        LibraryApiContractId::JavaCollectionFactory(kind) => [
            ("List", "of"),
            ("Set", "of"),
            ("Arrays", "asList"),
            ("Collections", "emptyList"),
            ("Collections", "emptySet"),
            ("Collections", "singleton"),
            ("Collections", "singletonList"),
            ("ImmutableList", "of"),
            ("ImmutableSet", "of"),
        ]
        .into_iter()
        .filter_map(|(receiver, method)| {
            library_java_collection_factory_contract(lang, receiver, method)
        })
        .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionFactory(kind))
        .map(|contract| contract.callee)
        .collect(),
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
        LibraryApiContractId::JavaMapFactory(kind) => [
            ("Map", "of"),
            ("Map", "ofEntries"),
            ("Collections", "emptyMap"),
            ("Collections", "singletonMap"),
            ("ImmutableMap", "of"),
        ]
        .into_iter()
        .filter_map(|(receiver, method)| library_java_map_factory_contract(lang, receiver, method))
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
        _ => library_api_member_callee_contracts_for_id(lang, id),
    }
}

fn library_api_member_callee_contracts_for_id(
    lang: Lang,
    id: LibraryApiContractId,
) -> Vec<LibraryApiCalleeContract> {
    match id {
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
            .chain(library_object_key_view_contract(lang, "Object", "keys", 1))
            .filter(|contract| contract.result.kind == kind)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve) => {
            library_promise_resolve_contract(lang, "Promise", "resolve", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::PromiseThen => library_promise_then_contract(lang, "then", 1)
            .map(|contract| vec![contract.callee])
            .unwrap_or_default(),
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
        (Lang::Swift, Builtin::Min) => Some(("min", 2)),
        (Lang::Swift, Builtin::Max) => Some(("max", 2)),
        (Lang::Swift, Builtin::Abs) => Some(("abs", 1)),
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

fn library_free_function_hof_callee_contracts_for_id(
    lang: Lang,
    kind: HoFKind,
) -> Vec<LibraryApiCalleeContract> {
    let candidate = match (lang, kind) {
        (Lang::Python, HoFKind::Map) => Some(("map", 2)),
        (Lang::Python, HoFKind::Filter) => Some(("filter", 2)),
        _ => None,
    };
    candidate
        .and_then(|(name, arg_count)| library_free_function_hof_contract(lang, name, arg_count))
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
        "hasPrefix",
        "hasSuffix",
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
            (0..=3).flat_map(move |arity| library_method_call_contracts(lang, method, arity))
        })
        .filter(|contract| contract.result.semantic == semantic)
        .map(|contract| contract.callee)
        .collect()
}
