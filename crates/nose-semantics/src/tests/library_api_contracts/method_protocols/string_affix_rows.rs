use super::*;

#[test]
fn receiver_method_string_affix_rows_use_string_affix_protocol_pack() {
    for (lang, method, semantic) in [
        (Lang::Python, "startswith", Builtin::StartsWith),
        (Lang::Python, "endswith", Builtin::EndsWith),
        (Lang::Java, "startsWith", Builtin::StartsWith),
        (Lang::Java, "endsWith", Builtin::EndsWith),
        (Lang::Rust, "starts_with", Builtin::StartsWith),
        (Lang::Rust, "ends_with", Builtin::EndsWith),
        (Lang::Swift, "hasPrefix", Builtin::StartsWith),
        (Lang::Swift, "hasSuffix", Builtin::EndsWith),
        (Lang::JavaScript, "startsWith", Builtin::StartsWith),
        (Lang::JavaScript, "endsWith", Builtin::EndsWith),
        (Lang::TypeScript, "startsWith", Builtin::StartsWith),
        (Lang::TypeScript, "endsWith", Builtin::EndsWith),
        (Lang::Ruby, "start_with?", Builtin::StartsWith),
        (Lang::Ruby, "end_with?", Builtin::EndsWith),
    ] {
        let contract =
            library_method_call_contract(lang, method, 1).expect("string affix method contract");
        assert_eq!(contract.pack_id, STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID);
        assert_eq!(
            contract.producer_id,
            STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID
        );
        assert_eq!(
            contract.id,
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(semantic))
        );
        assert_eq!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactString,
            }
        );
        assert_eq!(
            contract.result.receiver,
            MethodReceiverContract::ExactString
        );
        assert_eq!(contract.result.args, MethodBuiltinArgs::ReceiverAndFirst);
    }

    assert!(library_method_call_contract(Lang::Python, "startswith", 2).is_none());
}

#[test]
fn go_namespace_string_affix_rows_use_string_affix_protocol_pack() {
    for (method, semantic) in [
        ("HasPrefix", Builtin::StartsWith),
        ("HasSuffix", Builtin::EndsWith),
    ] {
        let go_affix = library_method_call_contract(Lang::Go, method, 2)
            .expect("Go strings namespace affix contract");
        assert_eq!(go_affix.pack_id, STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID);
        assert_eq!(
            go_affix.producer_id,
            STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID
        );
        assert_eq!(
            go_affix.id,
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(semantic))
        );
        assert_eq!(
            go_affix.callee,
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ImportedNamespace("strings"),
            }
        );
        assert_eq!(
            go_affix.result.receiver,
            MethodReceiverContract::ImportedNamespace("strings")
        );
        assert_eq!(go_affix.result.args, MethodBuiltinArgs::All);

        assert!(library_method_call_contract(Lang::Go, method, 1).is_none());
        assert!(library_method_call_contract(Lang::Go, method, 3).is_none());
    }
}
