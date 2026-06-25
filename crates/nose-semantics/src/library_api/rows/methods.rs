use super::*;

pub fn library_map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    if arg_count != 0 {
        return None;
    }
    let result = match (lang, method) {
        (Lang::Python | Lang::Ruby, "keys") => MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::Java, "keySet") => MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "keys") => {
            MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            }
        }
        _ => return None,
    };
    Some(LibraryMapKeyViewContract {
        pack_id: MAP_KEY_VIEW_PROTOCOL_PACK_ID,
        id: LibraryApiContractId::MapKeyView(result.kind),
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: MethodReceiverContract::ExactMap,
        },
        result,
    })
}

pub fn library_map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

pub fn library_object_key_view_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    if !js_like_lang(lang) || receiver != "Object" || method != "keys" || arg_count != 1 {
        return None;
    }
    let result = MapKeyViewContract {
        method: "keys",
        kind: MapKeyViewKind::Collection,
    };
    Some(LibraryMapKeyViewContract {
        pack_id: MAP_KEY_VIEW_PROTOCOL_PACK_ID,
        id: LibraryApiContractId::MapKeyView(result.kind),
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: "Object",
            method: result.method,
            qualified_path: "Object.keys",
            requires_unshadowed_receiver: true,
        },
        result,
    })
}

pub fn library_object_key_view_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    (method_hash == stable_symbol_hash("keys"))
        .then(|| library_object_key_view_contract(lang, receiver, "keys", arg_count))
        .flatten()
}

pub fn library_map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "from" || arg_count != 1 {
        return None;
    }
    let result = MapKeyViewWrapperContract {
        receiver: "Array",
        method: "from",
        qualified_path: "Array.from",
    };
    Some(LibraryMapKeyViewWrapperContract {
        pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
        id: LibraryApiContractId::MapKeyViewWrapper,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: true,
        },
        result,
    })
}

pub fn library_map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| library_map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

pub fn library_map_get_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    if !matches!(
        lang,
        Lang::Java
            | Lang::Rust
            | Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
    ) || method != "get"
        || arg_count != 1
    {
        return None;
    }
    let result = MapGetContract {
        method: "get",
        receiver: MethodReceiverContract::ExactMap,
    };
    Some(LibraryMapGetContract {
        pack_id: MAP_GET_PROTOCOL_PACK_ID,
        id: LibraryApiContractId::MapGet,
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| library_map_get_contract(lang, "get", arg_count))
        .flatten()
}

pub fn library_js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalMethodContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "isArray" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalMethodContract {
        receiver: "Array",
        method: "isArray",
        qualified_path: "Array.isArray",
        requires_unshadowed_receiver: true,
    };
    Some(LibraryStaticGlobalMethodContract {
        pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
        id: LibraryApiContractId::JsArrayIsArray,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: result.requires_unshadowed_receiver,
        },
        result,
    })
}

pub fn library_js_boolean_coercion_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalFunctionContract> {
    if !js_like_lang(lang) || function != "Boolean" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalFunctionContract {
        function: "Boolean",
        requires_unshadowed_function: true,
    };
    Some(LibraryStaticGlobalFunctionContract {
        pack_id: JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
        id: LibraryApiContractId::JsBooleanCoercion,
        callee: LibraryApiCalleeContract::StaticGlobalFunction {
            function: result.function,
            requires_unshadowed_function: result.requires_unshadowed_function,
        },
        result,
    })
}

pub fn library_regex_test_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryRegexTestContract> {
    if !js_like_lang(lang) || method != "test" || arg_count != 1 {
        return None;
    }
    let result = RegexTestContract {
        method: "test",
        required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
    };
    Some(LibraryRegexTestContract {
        pack_id: JS_LIKE_BUILTIN_REGEX_PACK_ID,
        id: LibraryApiContractId::RegexTest,
        callee: LibraryApiCalleeContract::RegexLiteralMethod {
            method: result.method,
            required_receiver_fact: result.required_receiver_fact,
        },
        result,
    })
}

pub fn library_static_index_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticIndexMembershipContract> {
    let result = static_index_membership_contract(lang, method, arg_count)?;
    Some(LibraryStaticIndexMembershipContract {
        pack_id: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
        id: LibraryApiContractId::JsLikeStaticIndexMembership(result.kind),
        callee: LibraryApiCalleeContract::StaticIndexMembershipMethod {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_imported_namespace_function_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryImportedNamespaceFunctionContract> {
    let result = match (lang, function, arg_count) {
        (Lang::Python, "prod", 1 | 2) => ImportedNamespaceFunctionContract {
            module: "math",
            function: "prod",
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        },
        _ => return None,
    };
    Some(LibraryImportedNamespaceFunctionContract {
        pack_id: PYTHON_STDLIB_MATH_PACK_ID,
        id: LibraryApiContractId::ImportedNamespaceFunction(result.semantic),
        callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
            module: result.module,
            function: result.function,
        },
        result,
    })
}

pub fn library_promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryPromiseThenContract> {
    if !js_like_lang(lang) || method != "then" || arg_count != 1 {
        return None;
    }
    let result = PromiseThenContract {
        receiver: AsyncReceiverContract::ExactPromiseLike,
        demand: promise_then_demand_effect_profile(),
    };
    Some(LibraryPromiseThenContract {
        pack_id: JS_LIKE_BUILTIN_PROMISE_PACK_ID,
        id: LibraryApiContractId::PromiseThen,
        callee: LibraryApiCalleeContract::AsyncMethod {
            method: "then",
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_promise_resolve_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryPromiseFactoryContract> {
    if !js_like_lang(lang) || receiver != "Promise" || method != "resolve" || arg_count != 1 {
        return None;
    }
    let result = PromiseFactoryContract {
        receiver: "Promise",
        method: "resolve",
        qualified_path: "Promise.resolve",
        kind: PromiseFactoryKind::Resolve,
        result_domain: DomainEvidence::PromiseLike,
    };
    Some(LibraryPromiseFactoryContract {
        pack_id: JS_LIKE_BUILTIN_PROMISE_PACK_ID,
        id: LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve),
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: true,
        },
        result,
    })
}

pub fn library_iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryIteratorIdentityAdapterContract> {
    let method = if lang == Lang::Rust && arg_count == 0 {
        match method {
            "iter" => "iter",
            "into_iter" => "into_iter",
            "iter_mut" => "iter_mut",
            "collect" => "collect",
            "to_vec" => "to_vec",
            "copied" => "copied",
            "cloned" => "cloned",
            _ => return None,
        }
    } else if lang == Lang::Java && method == "stream" && arg_count == 0 {
        "stream"
    } else {
        return None;
    };
    let result = IteratorIdentityAdapterContract {
        receiver: IteratorAdapterReceiverContract::ExactIterableValue,
    };
    Some(LibraryIteratorIdentityAdapterContract {
        pack_id: ITERATOR_IDENTITY_ADAPTER_PACK_ID,
        id: LibraryApiContractId::IteratorIdentityAdapter,
        callee: LibraryApiCalleeContract::IteratorAdapterMethod {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_static_collection_adapter_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticCollectionAdapterContract> {
    if lang != Lang::Java || receiver != "Arrays" || method != "stream" || arg_count != 1 {
        return None;
    }
    let result = StaticCollectionAdapterContract {
        module: "java.util",
        exported: "Arrays",
    };
    Some(LibraryStaticCollectionAdapterContract {
        pack_id: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
        id: LibraryApiContractId::StaticCollectionAdapter,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: result.exported,
            method: "stream",
        },
        result,
    })
}

pub fn library_method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryMethodCallContract> {
    library_method_call_contracts(lang, name, arg_count)
        .into_iter()
        .next()
}

pub fn library_method_call_contracts(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Vec<LibraryMethodCallContract> {
    let Some(method) = library_method_selector_name(name) else {
        return Vec::new();
    };
    method_call_contract_shapes(lang, name, arg_count)
        .into_iter()
        .map(|result| {
            let (pack_id, producer_id) = method_call_contract_provenance(lang, result);
            LibraryMethodCallContract {
                pack_id,
                producer_id,
                id: LibraryApiContractId::MethodCall(result.semantic),
                callee: LibraryApiCalleeContract::Method {
                    method,
                    receiver: result.receiver,
                },
                result,
            }
        })
        .collect()
}

fn method_call_contract_provenance(
    lang: Lang,
    contract: MethodCallContract,
) -> (&'static str, &'static str) {
    if lang == Lang::Go
        && matches!(
            (contract.semantic, contract.receiver, contract.args,),
            (
                MethodSemanticContract::Builtin(Builtin::Print),
                MethodReceiverContract::ImportedNamespace("fmt"),
                MethodBuiltinArgs::All,
            ) | (
                MethodSemanticContract::Builtin(Builtin::StartsWith | Builtin::EndsWith),
                MethodReceiverContract::ImportedNamespace("strings"),
                MethodBuiltinArgs::All,
            ) | (
                MethodSemanticContract::Builtin(Builtin::StringContains),
                MethodReceiverContract::ImportedNamespace("strings"),
                MethodBuiltinArgs::All,
            ) | (
                MethodSemanticContract::Builtin(Builtin::Contains),
                MethodReceiverContract::ImportedNamespace("slices"),
                MethodBuiltinArgs::GoSliceContains,
            )
        )
    {
        (
            GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
        )
    } else if matches!(
        (contract.semantic, contract.receiver, contract.args,),
        (
            MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            MethodReceiverContract::ExactMap,
            MethodBuiltinArgs::MapGetDefault | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
        )
    ) {
        (
            MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
            MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID,
        )
    } else if matches!(
        (contract.semantic, contract.receiver, contract.args,),
        (
            MethodSemanticContract::Builtin(Builtin::Contains),
            MethodReceiverContract::ExactMap
                | MethodReceiverContract::ExactCollectionOrMap
                | MethodReceiverContract::ExactCollectionOrJavaKeySet
                | MethodReceiverContract::ExactSetOrMap,
            MethodBuiltinArgs::FirstThenReceiver,
        )
    ) {
        (
            RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
        )
    } else {
        (
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        )
    }
}

pub fn library_map_get_default_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMethodCallContract> {
    library_method_call_contracts(lang, method, arg_count)
        .into_iter()
        .find(|contract| {
            contract.id
                == LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::GetOrDefault,
                ))
                && matches!(
                    contract.callee,
                    LibraryApiCalleeContract::Method {
                        receiver: MethodReceiverContract::ExactMap,
                        ..
                    }
                )
                && matches!(
                    contract.result.args,
                    MethodBuiltinArgs::MapGetDefault
                        | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
                )
        })
}

pub fn library_receiver_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMethodCallContract> {
    library_method_call_contracts(lang, method, arg_count)
        .into_iter()
        .find(|contract| {
            contract.id
                == LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::Contains,
                ))
                && matches!(
                    contract.callee,
                    LibraryApiCalleeContract::Method {
                        receiver: MethodReceiverContract::ExactMap
                            | MethodReceiverContract::ExactCollectionOrMap
                            | MethodReceiverContract::ExactCollectionOrJavaKeySet
                            | MethodReceiverContract::ExactSetOrMap,
                        ..
                    }
                )
                && contract.result.args == MethodBuiltinArgs::FirstThenReceiver
        })
}

mod receiver;
mod selectors;
pub use receiver::{library_receiver_method_api_contract, library_receiver_method_api_contracts};
pub(crate) use selectors::library_method_selector_name;
