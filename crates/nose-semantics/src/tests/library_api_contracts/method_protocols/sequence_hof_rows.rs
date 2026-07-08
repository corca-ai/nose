use super::*;

#[test]
fn rust_iterator_hof_rows_use_sequence_hof_protocol_pack() {
    for (method, arity) in [
        ("map", 1),
        ("filter", 1),
        ("filter_map", 1),
        ("flat_map", 1),
        ("any", 1),
        ("all", 1),
        ("count", 0),
    ] {
        let contract =
            library_method_call_contract(Lang::Rust, method, arity).expect("Rust method row");
        assert_eq!(contract.pack_id, SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID);
        assert_eq!(
            contract.producer_id,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
        );
        assert_eq!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactProtocol,
            }
        );
    }

    for (method, arity) in [
        ("map", 1),
        ("filter", 1),
        ("flatMap", 1),
        ("some", 1),
        ("every", 1),
    ] {
        let contract =
            library_method_call_contract(Lang::JavaScript, method, arity).expect("JS method row");
        assert_eq!(contract.pack_id, JS_LIKE_BUILTIN_ARRAY_PACK_ID);
        assert_eq!(contract.producer_id, JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID);
        assert_eq!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactArray,
            }
        );
    }
    assert!(
        library_method_call_contract(Lang::JavaScript, "map", 2).is_none(),
        "JS Array.map with thisArg remains closed until callback binding is modeled"
    );

    for (method, expected, args) in [
        (
            "map",
            MethodSemanticContract::HoF(HoFKind::Map),
            MethodBuiltinArgs::Hof,
        ),
        (
            "filter",
            MethodSemanticContract::HoF(HoFKind::Filter),
            MethodBuiltinArgs::Hof,
        ),
        (
            "flatMap",
            MethodSemanticContract::HoF(HoFKind::FlatMap),
            MethodBuiltinArgs::Hof,
        ),
        (
            "allSatisfy",
            MethodSemanticContract::Builtin(Builtin::All),
            MethodBuiltinArgs::BoolReduction,
        ),
    ] {
        let contract =
            library_method_call_contract(Lang::Swift, method, 1).expect("Swift Sequence HOF row");
        assert_eq!(contract.id, LibraryApiContractId::MethodCall(expected));
        assert_eq!(contract.result.args, args);
        assert_eq!(contract.pack_id, SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID);
        assert_eq!(
            contract.producer_id,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
        );
        assert_eq!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactArrayOrCollection,
            }
        );
    }
    assert!(
        library_method_call_contract(Lang::Swift, "compactMap", 1).is_none(),
        "Swift compactMap stays closed until optional-channel semantics are represented"
    );
    assert!(
        library_method_call_contract(Lang::Swift, "map", 2).is_none(),
        "Swift Sequence HOF rows stay closed outside the single-callback shape"
    );
    assert!(
        library_method_call_contract(Lang::Swift, "allSatisfy", 0).is_none(),
        "Swift allSatisfy requires an explicit predicate closure"
    );
    assert!(
        library_method_call_contract(Lang::Swift, "allSatisfy", 2).is_none(),
        "Swift allSatisfy stays closed outside the single-callback shape"
    );

    for method in [
        "map", "collect", "select", "filter", "reject", "any?", "all?",
    ] {
        let contract =
            library_method_call_contract(Lang::Ruby, method, 1).expect("Ruby Enumerable HOF row");
        assert_eq!(contract.pack_id, SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID);
        assert_eq!(
            contract.producer_id,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
        );
        assert_eq!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactArrayOrCollection,
            }
        );
    }
    assert!(
        library_method_call_contract(Lang::Ruby, "map", 0).is_none(),
        "Ruby Enumerable HOF rows require an explicit block"
    );
    assert!(
        library_method_call_contract(Lang::Ruby, "map", 2).is_none(),
        "Ruby Enumerable HOF rows stay closed for block-plus-argument shapes"
    );
    assert!(
        library_method_call_contract(Lang::Ruby, "any?", 0).is_none(),
        "Ruby Enumerable quantifier rows require an explicit block"
    );
    assert!(
        library_method_call_contract(Lang::Ruby, "all?", 2).is_none(),
        "Ruby Enumerable quantifier rows stay closed for block-plus-argument shapes"
    );
    assert!(
        library_method_call_contract(Lang::Ruby, "flat_map", 1).is_none(),
        "Ruby flat_map stays closed until nested flattening semantics are represented"
    );

    assert!(
        library_method_call_contract(Lang::Rust, "find", 1).is_none(),
        "Rust find stays closed until optional-result semantics are represented"
    );
}
