use super::*;

#[test]
fn library_non_factory_api_contracts_carry_identity_and_result_obligations() {
    assert_eq!(
        library_map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(LibraryMapKeyViewContract {
            pack_id: MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            id: LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator),
            callee: LibraryApiCalleeContract::Method {
                method: "keys",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            },
        })
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(LibraryMapKeyViewWrapperContract {
            pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            id: LibraryApiContractId::MapKeyViewWrapper,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
                requires_unshadowed_receiver: true,
            },
            result: MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
            },
        })
    );
    assert_eq!(
        library_map_get_contract(Lang::Rust, "get", 1),
        Some(LibraryMapGetContract {
            pack_id: MAP_GET_PROTOCOL_PACK_ID,
            id: LibraryApiContractId::MapGet,
            callee: LibraryApiCalleeContract::Method {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            },
        })
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(LibraryStaticGlobalMethodContract {
            pack_id: JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            id: LibraryApiContractId::JsArrayIsArray,
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
            result: StaticGlobalMethodContract {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            },
        })
    );
}

#[test]
fn library_coercion_regex_namespace_and_promise_contracts_carry_obligations() {
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(LibraryStaticGlobalFunctionContract {
            pack_id: JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
            id: LibraryApiContractId::JsBooleanCoercion,
            callee: LibraryApiCalleeContract::StaticGlobalFunction {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
            result: StaticGlobalFunctionContract {
                function: "Boolean",
                requires_unshadowed_function: true,
            },
        })
    );
    assert_eq!(
        library_regex_test_contract(Lang::JavaScript, "test", 1),
        Some(LibraryRegexTestContract {
            pack_id: JS_LIKE_BUILTIN_REGEX_PACK_ID,
            id: LibraryApiContractId::RegexTest,
            callee: LibraryApiCalleeContract::RegexLiteralMethod {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
            result: RegexTestContract {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            },
        })
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 2),
        Some(LibraryImportedNamespaceFunctionContract {
            pack_id: PYTHON_STDLIB_MATH_PACK_ID,
            id: LibraryApiContractId::ImportedNamespaceFunction(
                ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            ),
            callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
                module: "math",
                function: "prod",
            },
            result: ImportedNamespaceFunctionContract {
                module: "math",
                function: "prod",
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            },
        })
    );
    assert_eq!(
        library_promise_then_contract(Lang::Vue, "then", 1),
        Some(LibraryPromiseThenContract {
            pack_id: JS_LIKE_BUILTIN_PROMISE_PACK_ID,
            id: LibraryApiContractId::PromiseThen,
            callee: LibraryApiCalleeContract::AsyncMethod {
                method: "then",
                receiver: AsyncReceiverContract::ExactPromiseLike,
            },
            result: PromiseThenContract {
                receiver: AsyncReceiverContract::ExactPromiseLike,
                demand: promise_then_demand_effect_profile(),
            },
        })
    );
    assert_eq!(
        library_promise_resolve_contract(Lang::TypeScript, "Promise", "resolve", 1),
        Some(LibraryPromiseFactoryContract {
            pack_id: JS_LIKE_BUILTIN_PROMISE_PACK_ID,
            id: LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve),
            callee: LibraryApiCalleeContract::StaticGlobalMethod {
                receiver: "Promise",
                method: "resolve",
                qualified_path: "Promise.resolve",
                requires_unshadowed_receiver: true,
            },
            result: PromiseFactoryContract {
                receiver: "Promise",
                method: "resolve",
                qualified_path: "Promise.resolve",
                kind: PromiseFactoryKind::Resolve,
                result_domain: DomainEvidence::PromiseLike,
            },
        })
    );
}

#[test]
fn library_iterator_adapter_and_method_call_contracts_carry_obligations() {
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(LibraryIteratorIdentityAdapterContract {
            pack_id: ITERATOR_IDENTITY_ADAPTER_PACK_ID,
            id: LibraryApiContractId::IteratorIdentityAdapter,
            callee: LibraryApiCalleeContract::IteratorAdapterMethod {
                method: "collect",
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
            result: IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            },
        })
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(LibraryStaticCollectionAdapterContract {
            pack_id: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
            id: LibraryApiContractId::StaticCollectionAdapter,
            callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                receiver: "Arrays",
                method: "stream",
            },
            result: StaticCollectionAdapterContract {
                module: "java.util",
                exported: "Arrays",
            },
        })
    );
    assert_eq!(
        library_receiver_membership_contract(Lang::Rust, "contains_key", 1),
        Some(LibraryMethodCallContract {
            pack_id: RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            producer_id: RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "contains_key",
                receiver: MethodReceiverContract::ExactMap,
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            },
        })
    );
    assert_eq!(
        library_receiver_membership_contract(Lang::TypeScript, "has", 1),
        Some(LibraryMethodCallContract {
            pack_id: RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            producer_id: RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "has",
                receiver: MethodReceiverContract::ExactSetOrMap,
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactSetOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            },
        })
    );
    assert_eq!(
        library_receiver_membership_contract(Lang::Python, "__contains__", 1),
        Some(LibraryMethodCallContract {
            pack_id: RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            producer_id: RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "__contains__",
                receiver: MethodReceiverContract::ExactCollectionOrMap,
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactCollectionOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            },
        })
    );
    assert_eq!(
        library_method_call_contract(Lang::Go, "Contains", 2),
        Some(LibraryMethodCallContract {
            pack_id: GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            producer_id: GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
            id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Contains,
            )),
            callee: LibraryApiCalleeContract::Method {
                method: "Contains",
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
            },
            result: MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
                args: MethodBuiltinArgs::GoSliceContains,
            },
        })
    );
    assert_eq!(
        library_method_call_contracts(Lang::Go, "Contains", 2),
        vec![
            LibraryMethodCallContract {
                pack_id: GO_STDLIB_NAMESPACE_CALL_PACK_ID,
                producer_id: GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
                id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::Contains,
                )),
                callee: LibraryApiCalleeContract::Method {
                    method: "Contains",
                    receiver: MethodReceiverContract::ImportedNamespace("slices"),
                },
                result: MethodCallContract {
                    semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                    receiver: MethodReceiverContract::ImportedNamespace("slices"),
                    args: MethodBuiltinArgs::GoSliceContains,
                },
            },
            LibraryMethodCallContract {
                pack_id: GO_STDLIB_NAMESPACE_CALL_PACK_ID,
                producer_id: GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
                id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::StringContains,
                )),
                callee: LibraryApiCalleeContract::Method {
                    method: "Contains",
                    receiver: MethodReceiverContract::ImportedNamespace("strings"),
                },
                result: MethodCallContract {
                    semantic: MethodSemanticContract::Builtin(Builtin::StringContains),
                    receiver: MethodReceiverContract::ImportedNamespace("strings"),
                    args: MethodBuiltinArgs::All,
                },
            },
        ]
    );
}

#[test]
fn library_non_factory_api_contracts_reject_raw_name_only_matches() {
    assert_eq!(
        library_map_key_view_contract(Lang::JavaScript, "keySet", 0),
        None
    );
    assert_eq!(library_map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        library_map_key_view_wrapper_contract(Lang::TypeScript, "Array", "from", 2),
        None
    );
    assert_eq!(library_map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(library_map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(
        library_js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        library_js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
}

#[test]
fn library_promise_adapter_and_method_contracts_reject_raw_name_only_matches() {
    assert_eq!(library_regex_test_contract(Lang::Ruby, "test", 1), None);
    assert_eq!(
        library_imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
        None
    );
    assert_eq!(
        library_imported_namespace_function_contract(Lang::Python, "prod", 3),
        None
    );
    assert_eq!(library_promise_then_contract(Lang::Python, "then", 1), None);
    assert_eq!(
        library_promise_then_contract(Lang::TypeScript, "then", 2),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
    assert_eq!(
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(
        library_receiver_membership_contract(Lang::Go, "Contains", 2),
        None
    );
    assert_eq!(
        library_receiver_membership_contract(Lang::JavaScript, "contains", 1),
        None
    );
    assert_eq!(library_method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "min", 1),
        None
    );
    assert_eq!(
        library_method_call_contract(Lang::JavaScript, "Contains", 2),
        None
    );
}
