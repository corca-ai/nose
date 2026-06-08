//! First-party library/API row constructors.

use super::*;

pub fn library_free_name_collection_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Python => LibraryApiContractId::PythonBuiltinCollectionFactory,
                Lang::Rust => LibraryApiContractId::RustStdCollectionFactory,
                _ => return None,
            };
            Some(LibraryCollectionFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, row.shadow_guard),
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        })
}

pub fn library_free_name_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_collection_factory_contract(lang, name))
        })
}

pub fn library_free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryFreeFunctionBuiltinContract> {
    let result = free_function_builtin_contract(lang, name, arg_count)?;
    Some(LibraryFreeFunctionBuiltinContract {
        id: LibraryApiContractId::FreeFunctionBuiltin(result.builtin),
        callee: LibraryApiCalleeContract::FreeName {
            name: result.name,
            shadow: library_free_name_shadow_policy(lang, result.requires_unshadowed),
        },
        result,
    })
}

pub fn library_imported_collection_factory_contract(
    lang: Lang,
    module: &str,
    exported: &str,
) -> Option<LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .find(|row| {
            row.lang.is_none_or(|row_lang| row_lang == lang)
                && row.module == module
                && row.exported == exported
        })
        .map(|row| LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: row.module,
                exported: row.exported,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
}

pub fn library_imported_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .filter_map(move |row| {
            library_imported_collection_factory_contract(lang, row.module, row.exported)
        })
}

pub fn library_free_name_map_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Rust => LibraryApiContractId::RustStdMapFactory,
                _ => return None,
            };
            Some(LibraryMapFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, false),
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: row.entry_seq_tag,
                },
            })
        })
}

pub fn library_free_name_map_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_map_factory_contract(lang, name))
        })
}

pub fn library_java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_factory_contract(lang, receiver, method)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: contract.single_arg_spreads_array,
        },
    })
}

pub fn library_java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_constructor_contract(lang, type_name, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionConstructor(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type: contract.simple_type,
            qualified_type: contract.qualified_type,
            module: contract.module,
            requires_import_for_simple_type: contract.requires_import_for_simple_type,
            requires_no_local_type_shadow: contract.requires_no_local_type_shadow,
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = java_map_factory_contract(lang, receiver, method)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JavaMapFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryMapFactoryResult::JavaFactory {
            kind: contract.kind,
        },
    })
}

pub fn library_java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_map_entry_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapEntryFactoryContract> {
    java_map_entry_contract(lang, receiver, method).then_some(LibraryMapEntryFactoryContract {
        id: LibraryApiContractId::JavaMapEntryFactory,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: "Map",
            method: "entry",
        },
    })
}

pub fn library_java_map_entry_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapEntryFactoryContract> {
    (method_hash == stable_symbol_hash("entry"))
        .then(|| library_java_map_entry_contract(lang, receiver, "entry"))
        .flatten()
}

pub fn library_ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = ruby_set_factory_contract(lang, receiver, method, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RubySetFactory,
        callee: LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver: contract.receiver,
            method: contract.method,
            required_module: contract.required_module,
            shadow_root: contract.shadow_root,
        },
        result: LibraryCollectionFactoryResult::SequenceArgument,
    })
}

pub fn library_ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| library_ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

pub fn library_js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = js_like_set_constructor_contract(lang, receiver)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JsLikeSetConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument,
    })
}

pub fn library_js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = js_like_map_constructor_contract(lang, receiver)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JsLikeMapConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryMapFactoryResult::EntrySequence {
            entry_seq_tag: contract.entry_seq_tag?,
        },
    })
}

pub fn library_rust_vec_macro_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    (lang == Lang::Rust && name == "vec").then_some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecMacroFactory,
        callee: LibraryApiCalleeContract::RustMacro {
            name: "vec",
            shadow: LibraryApiShadowPolicy::SameName,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: false,
        },
    })
}

pub fn library_rust_vec_new_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = rust_vec_new_factory_contract(lang, name)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecNewFactory,
        callee: LibraryApiCalleeContract::FreeName {
            name: match name {
                "Vec::new" => "Vec::new",
                "std::vec::Vec::new" => "std::vec::Vec::new",
                "alloc::vec::Vec::new" => "alloc::vec::Vec::new",
                _ => return None,
            },
            shadow: LibraryApiShadowPolicy::ExplicitRoot(contract.shadow_root),
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

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
    };
    Some(LibraryPromiseThenContract {
        id: LibraryApiContractId::PromiseThen,
        callee: LibraryApiCalleeContract::AsyncMethod {
            method: "then",
            receiver: result.receiver,
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
    let result = method_call_contract_shape(lang, name, arg_count)?;
    let method = library_method_selector_name(name)?;
    Some(LibraryMethodCallContract {
        id: LibraryApiContractId::MethodCall(result.semantic),
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_receiver_method_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_get_contract(lang, method, arg_count)
        .map(|contract| LibraryReceiverMethodApiContract {
            id: contract.id,
            callee: contract.callee,
            rule: "library_api_map_get",
        })
        .or_else(|| {
            library_map_key_view_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_map_key_view",
                }
            })
        })
        .or_else(|| {
            library_iterator_identity_adapter_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_iterator_identity_adapter",
                }
            })
        })
        .or_else(|| {
            library_scalar_integer_method_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_scalar_integer_method",
                }
            })
        })
        .or_else(|| {
            library_rust_option_and_then_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_rust_option_and_then",
                }
            })
        })
        .or_else(|| {
            library_method_call_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_method_call",
                }
            })
        })
}

pub(crate) fn library_method_selector_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "__contains__" => "__contains__",
        "Abs" => "Abs",
        "Contains" => "Contains",
        "HasPrefix" => "HasPrefix",
        "HasSuffix" => "HasSuffix",
        "Max" => "Max",
        "Min" => "Min",
        "Print" => "Print",
        "Printf" => "Printf",
        "Println" => "Println",
        "abs" => "abs",
        "add" => "add",
        "all" => "all",
        "all?" => "all?",
        "allMatch" => "allMatch",
        "any" => "any",
        "any?" => "any?",
        "anyMatch" => "anyMatch",
        "and_then" => "and_then",
        "append" => "append",
        "clamp" => "clamp",
        "collect" => "collect",
        "contains" => "contains",
        "containsKey" => "containsKey",
        "contains_key" => "contains_key",
        "count" => "count",
        "debug" => "debug",
        "empty?" => "empty?",
        "end_with?" => "end_with?",
        "endsWith" => "endsWith",
        "ends_with" => "ends_with",
        "endswith" => "endswith",
        "every" => "every",
        "fetch" => "fetch",
        "filter" => "filter",
        "filter_map" => "filter_map",
        "flatMap" => "flatMap",
        "flat_map" => "flat_map",
        "fold" => "fold",
        "get" => "get",
        "getOrDefault" => "getOrDefault",
        "has" => "has",
        "has_key?" => "has_key?",
        "include?" => "include?",
        "includes" => "includes",
        "info" => "info",
        "inject" => "inject",
        "isEmpty" => "isEmpty",
        "is_empty" => "is_empty",
        "is_none" => "is_none",
        "is_some" => "is_some",
        "join" => "join",
        "key?" => "key?",
        "len" => "len",
        "length" => "length",
        "log" => "log",
        "map" => "map",
        "map_or" => "map_or",
        "max" => "max",
        "member?" => "member?",
        "min" => "min",
        "nil?" => "nil?",
        "push" => "push",
        "reduce" => "reduce",
        "select" => "select",
        "size" => "size",
        "some" => "some",
        "start_with?" => "start_with?",
        "startsWith" => "startsWith",
        "starts_with" => "starts_with",
        "startswith" => "startswith",
        "sum" => "sum",
        "unwrap_or" => "unwrap_or",
        "unwrap_or_else" => "unwrap_or_else",
        "zip" => "zip",
        _ => return None,
    })
}

fn library_free_name_shadow_policy(lang: Lang, shadow_guard: bool) -> LibraryApiShadowPolicy {
    if shadow_guard {
        LibraryApiShadowPolicy::SameName
    } else if lang == Lang::Rust {
        LibraryApiShadowPolicy::RustStdRootForStdPath
    } else {
        LibraryApiShadowPolicy::None
    }
}
