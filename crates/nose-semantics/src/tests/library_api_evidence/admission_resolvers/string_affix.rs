use super::*;

fn string_affix_call_il(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let (il, interner, call, _callee, receiver) =
        receiver_method_call_il(lang, method, arg_count, 190);
    (il, interner, call, receiver)
}

fn push_string_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::String),
        EvidenceStatus::Asserted,
    ));
}

fn assert_admitted_string_affix(lang: Lang, method: &str, builtin: Builtin) {
    let (mut il, interner, call, receiver) = string_affix_call_il(lang, method, 1);
    push_string_receiver_dependency(&mut il, receiver);
    let contract =
        library_method_call_contract(lang, method, 1).expect("string affix method contract");
    il.evidence.push(builtin_method_call_protocol_record(
        1,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence =
        admitted_library_method_call_at_call(&il, &interner, call).expect("affix admitted");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(builtin))
    );
    assert_eq!(
        occurrence.contract.pack_id,
        STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_string_affix_requires_protocol_pack_and_string_receiver_proof() {
    let (mut raw_shape, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw startswith shape plus string receiver proof is not enough"
    );

    let contract = library_method_call_contract(Lang::Python, "startswith", 1)
        .expect("Python startswith contract");

    let (mut missing_dependency, interner, call, _receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    missing_dependency
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "affix evidence without exact string receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "string affix evidence under the broad builtin-method pack is rejected"
    );

    let (mut wrong_direction, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut wrong_direction, receiver);
    let suffix_contract = library_method_call_contract(Lang::Python, "endswith", 1)
        .expect("Python endswith contract");
    wrong_direction
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            wrong_direction.node(call).span,
            suffix_contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_direction, &interner, call).is_none(),
        "forged suffix evidence cannot admit a prefix source call"
    );

    let (mut unsupported_arity, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 2);
    push_string_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged affix evidence cannot open unsupported arity"
    );

    assert_admitted_string_affix(Lang::Python, "startswith", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Python, "endswith", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::Rust, "starts_with", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Swift, "hasSuffix", Builtin::EndsWith);
}
