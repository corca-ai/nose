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

    for method in ["map", "filter", "flatMap"] {
        let contract =
            library_method_call_contract(Lang::Swift, method, 1).expect("Swift Sequence HOF row");
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
        library_method_call_contract(Lang::Rust, "find", 1).is_none(),
        "Rust find stays closed until optional-result semantics are represented"
    );
}
