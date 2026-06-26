use super::*;

fn map_get_default_call_il(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let (il, interner, call, _callee, receiver) =
        receiver_method_call_il(lang, method, arg_count, 152);
    (il, interner, call, receiver)
}

fn push_map_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
}

fn assert_admitted_map_get_default(lang: Lang, method: &str, expected_args: MethodBuiltinArgs) {
    let (mut il, interner, call, receiver) = map_get_default_call_il(lang, method, 2);
    push_map_receiver_dependency(&mut il, receiver);
    let contract =
        library_map_get_default_contract(lang, method, 2).expect("map-get-default contract");
    il.evidence.push(map_get_default_protocol_record(
        1,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence = admitted_library_method_call_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::GetOrDefault))
    );
    assert_eq!(occurrence.contract.result.args, expected_args);
    assert_eq!(
        occurrence.contract.result.receiver,
        MethodReceiverContract::ExactMap
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

#[test]
fn admitted_map_get_default_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 2);
    push_map_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw map.getOrDefault(...) shape plus map receiver proof is not enough"
    );

    let contract = library_map_get_default_contract(Lang::Java, "getOrDefault", 2)
        .expect("Java map-get-default contract");

    let (mut missing_dependency, interner, call, _receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 2);
    missing_dependency
        .evidence
        .push(map_get_default_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "map-get-default evidence without map receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 2);
    push_map_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[0],
            BUILTIN_COMPAT_PACK_ID,
            MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "map-get-default evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 2);
    push_map_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[0],
            MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
            "wrong.protocols.map-get-default-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "map-get-default evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 2);
    push_map_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = map_get_default_protocol_record(
        1,
        wrong_emitter.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_library_method_call_at_call(&wrong_emitter, &interner, call).is_none(),
        "map-get-default evidence from an external emitter is rejected"
    );

    assert_admitted_map_get_default(Lang::Python, "get", MethodBuiltinArgs::MapGetDefault);
    assert_admitted_map_get_default(
        Lang::Ruby,
        "fetch",
        MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
    );
    assert_admitted_map_get_default(Lang::Java, "getOrDefault", MethodBuiltinArgs::MapGetDefault);
}

#[test]
fn forged_map_get_default_evidence_does_not_open_unsupported_arity() {
    let contract = library_map_get_default_contract(Lang::Java, "getOrDefault", 2)
        .expect("Java map-get-default contract");
    let (mut unsupported_arity, interner, call, receiver) =
        map_get_default_call_il(Lang::Java, "getOrDefault", 1);
    push_map_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity
        .evidence
        .push(map_get_default_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged map-get-default evidence cannot open unsupported source arity"
    );
}
