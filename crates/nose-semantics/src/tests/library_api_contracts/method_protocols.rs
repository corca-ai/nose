use super::*;

#[test]
fn method_protocol_contracts_are_language_constrained() {
    assert!(method_fold_name(Lang::Ruby, "inject"));
    assert!(!method_fold_name(Lang::Python, "inject"));
    assert!(!method_fold_name(Lang::Ruby, "map"));
    assert_eq!(
        method_bool_reduction_builtin(Lang::Java, "anyMatch"),
        Some(Builtin::Any)
    );
    assert_eq!(
        method_bool_reduction_builtin(Lang::JavaScript, "every"),
        Some(Builtin::All)
    );
    assert_eq!(method_bool_reduction_builtin(Lang::Python, "every"), None);
    assert_eq!(
        method_hof_contract(Lang::Ruby, "collect"),
        Some(HoFKind::Map)
    );
    assert_eq!(
        method_hof_contract(Lang::Rust, "flat_map"),
        Some(HoFKind::FlatMap)
    );
    assert_eq!(
        method_hof_contract(Lang::Swift, "flatMap"),
        Some(HoFKind::FlatMap)
    );
    assert_eq!(
        method_hof_contract(Lang::Ruby, "select"),
        Some(HoFKind::Filter)
    );
    assert_eq!(
        method_hof_contract(Lang::Swift, "filter"),
        Some(HoFKind::Filter)
    );
    assert_eq!(method_hof_contract(Lang::Python, "select"), None);
    assert_eq!(
        method_collection_reduction_builtin(Lang::Rust, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::Java, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        method_collection_reduction_builtin(Lang::JavaScript, "count"),
        None
    );
    assert_eq!(
        property_builtin_contract(Lang::JavaScript, "length"),
        Some(Builtin::Len)
    );
    assert_eq!(
        library_property_builtin_contract(Lang::JavaScript, "length")
            .expect("JS length property contract")
            .pack_id,
        PROPERTY_BUILTIN_PROTOCOL_PACK_ID
    );
    assert_eq!(
        property_builtin_contract(Lang::Swift, "count"),
        Some(Builtin::Len)
    );
    assert_eq!(
        property_builtin_contract(Lang::Swift, "isEmpty"),
        Some(Builtin::IsEmpty)
    );
    assert_eq!(property_builtin_contract(Lang::Python, "length"), None);
}

#[test]
fn method_call_contracts_carry_receiver_and_resolution_obligations() {
    assert_eq!(
        method_call_contract(Lang::Python, "append", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Append),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverThenAll,
        })
    );
    assert_eq!(method_call_contract(Lang::Python, "append", 0), None);
    assert_eq!(
        method_call_contract(Lang::Swift, "append", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Append),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverThenAll,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "count", 0),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Len),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverOnly,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "isEmpty", 0),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::IsEmpty),
            receiver: MethodReceiverContract::ExactCollection,
            args: MethodBuiltinArgs::ReceiverOnly,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "hasPrefix", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::StartsWith),
            receiver: MethodReceiverContract::ExactString,
            args: MethodBuiltinArgs::ReceiverAndFirst,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "hasSuffix", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::EndsWith),
            receiver: MethodReceiverContract::ExactString,
            args: MethodBuiltinArgs::ReceiverAndFirst,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "map", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::HoF(HoFKind::Map),
            receiver: MethodReceiverContract::ExactProtocol,
            args: MethodBuiltinArgs::Hof,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "filter", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::HoF(HoFKind::Filter),
            receiver: MethodReceiverContract::ExactProtocol,
            args: MethodBuiltinArgs::Hof,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Swift, "flatMap", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::HoF(HoFKind::FlatMap),
            receiver: MethodReceiverContract::ExactProtocol,
            args: MethodBuiltinArgs::Hof,
        })
    );
    assert_eq!(
        method_call_contract(Lang::JavaScript, "log", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Print),
            receiver: MethodReceiverContract::UnshadowedGlobal("console"),
            args: MethodBuiltinArgs::All,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "min", 2), None);
    assert_eq!(method_call_contract(Lang::TypeScript, "max", 2), None);
    assert_eq!(method_call_contract(Lang::JavaScript, "min", 1), None);
    assert_eq!(method_call_contract(Lang::Python, "min", 2), None);
    assert_eq!(method_call_contract(Lang::Go, "Abs", 1), None);
    assert_eq!(
        method_call_contract(Lang::Go, "Contains", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ImportedNamespace("slices"),
            args: MethodBuiltinArgs::GoSliceContains,
        })
    );
    assert_eq!(method_call_contract(Lang::Java, "abs", 1), None);
    assert_eq!(method_call_contract(Lang::Java, "min", 2), None);
}

#[test]
fn method_call_contracts_cover_membership_and_map_default_lookups() {
    assert_eq!(
        method_call_contract(Lang::Python, "__contains__", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::TypeScript, "has", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactSetOrMap,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "member?", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "contains", 1), None);
    assert_eq!(
        method_call_contract(Lang::Swift, "contains", 1),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::Contains),
            receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
            args: MethodBuiltinArgs::FirstThenReceiver,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Java, "getOrDefault", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Python, "get", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefault,
        })
    );
    assert_eq!(
        method_call_contract(Lang::Ruby, "fetch", 2),
        Some(MethodCallContract {
            semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            receiver: MethodReceiverContract::ExactMap,
            args: MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
        })
    );
    assert_eq!(method_call_contract(Lang::JavaScript, "abs", 0), None);
}

#[test]
fn scalar_integer_methods_are_language_and_signature_constrained() {
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "clamp", 2),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Clamp,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::Rust, "min", 1),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Min,
            receiver: MethodReceiverContract::ExactInteger,
        })
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::Java, "abs", 1),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Abs,
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
        })
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::Java, "min", 2),
        Some(ScalarIntegerMethodContract {
            semantic: ScalarIntegerMethod::Min,
            receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
        })
    );
    assert_eq!(scalar_integer_method_contract(Lang::Rust, "clamp", 1), None);
    assert_eq!(scalar_integer_method_contract(Lang::Java, "abs", 0), None);
    assert_eq!(
        scalar_integer_method_contract(Lang::TypeScript, "clamp", 2),
        None
    );
    assert_eq!(
        scalar_integer_method_contract(Lang::JavaScript, "abs", 0),
        None
    );

    let rust_clamp = library_scalar_integer_method_contract(Lang::Rust, "clamp", 2)
        .expect("Rust clamp library contract");
    assert_eq!(rust_clamp.pack_id, RUST_STDLIB_INTEGER_METHOD_PACK_ID);

    let java_abs = library_scalar_integer_method_contract(Lang::Java, "abs", 1)
        .expect("Java Math.abs library contract");
    assert_eq!(java_abs.pack_id, JAVA_STDLIB_MATH_PACK_ID);
}

#[test]
fn promise_then_contract_requires_js_like_surface_and_receiver_proof() {
    assert_eq!(
        promise_then_contract(Lang::TypeScript, "then", 1),
        Some(PromiseThenContract {
            receiver: AsyncReceiverContract::ExactPromiseLike,
            demand: promise_then_demand_effect_profile(),
        })
    );
    assert_eq!(promise_then_contract(Lang::TypeScript, "then", 2), None);
    assert_eq!(promise_then_contract(Lang::Python, "then", 1), None);
}

#[test]
fn promise_resolve_contract_requires_js_like_static_global_surface() {
    assert_eq!(
        promise_resolve_contract(Lang::JavaScript, "Promise", "resolve", 1),
        Some(PromiseFactoryContract {
            receiver: "Promise",
            method: "resolve",
            qualified_path: "Promise.resolve",
            kind: PromiseFactoryKind::Resolve,
            result_domain: DomainEvidence::PromiseLike,
        })
    );
    assert_eq!(
        promise_resolve_contract(Lang::TypeScript, "Promise", "resolve", 2),
        None
    );
    assert_eq!(
        promise_resolve_contract(Lang::TypeScript, "Promise", "reject", 1),
        None
    );
    assert_eq!(
        promise_resolve_contract(Lang::Python, "Promise", "resolve", 1),
        None
    );
}

#[test]
fn iterator_identity_adapters_are_rust_and_receiver_proof_constrained() {
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "iter", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Java, "stream", 0),
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
        None
    );
    assert_eq!(
        iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
        None
    );
}

#[test]
fn static_collection_adapters_are_import_binding_constrained() {
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
        Some(StaticCollectionAdapterContract {
            module: "java.util",
            exported: "Arrays",
        })
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
        None
    );
    assert_eq!(
        static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
        None
    );
}

#[test]
fn receiver_method_api_rows_emit_only_safe_result_domains() {
    assert_eq!(
        library_receiver_method_api_contract(Lang::TypeScript, "keys", 0)
            .expect("JS-like Map.keys contract")
            .result_domain,
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::Java, "keySet", 0)
            .expect("Java Map.keySet contract")
            .result_domain,
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::Rust, "abs", 0)
            .expect("Rust integer abs contract")
            .result_domain,
        Some(DomainEvidence::Integer)
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::Java, "min", 2)
            .expect("Java Math.min contract")
            .result_domain,
        Some(DomainEvidence::Integer)
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::Rust, "and_then", 1)
            .expect("Rust Option::and_then contract")
            .result_domain,
        Some(DomainEvidence::Option)
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::TypeScript, "then", 1)
            .expect("JS-like Promise.then contract")
            .result_domain,
        Some(DomainEvidence::PromiseLike)
    );

    assert_eq!(
        library_receiver_method_api_contract(Lang::Rust, "collect", 0)
            .expect("Rust iterator adapter contract")
            .result_domain,
        None,
        "collect result type is caller-selected and must not be emitted as a domain"
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::TypeScript, "map", 1)
            .expect("JS-like HOF contract")
            .result_domain,
        None,
        "HOF result domains stay out of automatic emission until demand/effect boundaries are narrower"
    );
    assert_eq!(
        library_receiver_method_api_contract(Lang::Java, "get", 1)
            .expect("Java Map.get contract")
            .result_domain,
        None,
        "Map.get value domain depends on map value type and is not a receiver-method result-domain proof"
    );
}
