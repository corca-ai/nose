use super::*;

fn map_get_call_il(lang: Lang, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(132), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("get")),
        sp(133),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(1 + idx as u32),
                sp(134 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(136), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(137), &[call]);
    (finish_il(b, root, lang), interner, call, receiver)
}

fn push_map_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
}

fn assert_admitted_map_get(lang: Lang) {
    let (mut il, interner, call, receiver) = map_get_call_il(lang, 1);
    push_map_receiver_dependency(&mut il, receiver);
    let contract = library_map_get_contract(lang, "get", 1).expect("map-get contract");
    il.push_evidence(map_get_protocol_record(
        1,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence = admitted_map_get_at_call(&il, &interner, call).unwrap();
    assert_eq!(occurrence.contract.id, LibraryApiContractId::MapGet);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_map_get_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) = map_get_call_il(Lang::Rust, 1);
    push_map_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_map_get_at_call(&raw_shape, &interner, call).is_none(),
        "raw map.get(...) shape plus map receiver proof is not enough"
    );

    let contract = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map-get contract");

    let (mut missing_dependency, interner, call, _receiver) = map_get_call_il(Lang::Rust, 1);
    missing_dependency.push_evidence(map_get_protocol_record(
        1,
        missing_dependency.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_map_get_at_call(&missing_dependency, &interner, call).is_none(),
        "map-get evidence without map receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = map_get_call_il(Lang::Rust, 1);
    push_map_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        MAP_GET_PROTOCOL_PRODUCER_ID,
    ));
    assert!(
        admitted_map_get_at_call(&wrong_pack, &interner, call).is_none(),
        "map-get evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = map_get_call_il(Lang::Rust, 1);
    push_map_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        MAP_GET_PROTOCOL_PACK_ID,
        "wrong.protocols.map-get-api",
    ));
    assert!(
        admitted_map_get_at_call(&wrong_producer, &interner, call).is_none(),
        "map-get evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) = map_get_call_il(Lang::Rust, 1);
    push_map_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = map_get_protocol_record(
        1,
        wrong_emitter.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_map_get_at_call(&wrong_emitter, &interner, call).is_none(),
        "map-get evidence from an external emitter is rejected"
    );

    assert_admitted_map_get(Lang::Rust);
    assert_admitted_map_get(Lang::Java);
    assert_admitted_map_get(Lang::TypeScript);
}

#[test]
fn forged_map_get_evidence_does_not_open_unsupported_arity() {
    let contract = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map-get contract");
    let (mut unsupported_arity, interner, call, receiver) = map_get_call_il(Lang::Rust, 2);
    push_map_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity.push_evidence(map_get_protocol_record(
        1,
        unsupported_arity.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_map_get_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged map-get evidence cannot open unsupported source arity"
    );
}
