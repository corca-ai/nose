use super::*;

fn iterator_identity_call_il(lang: Lang, method: &str) -> (Il, Interner, NodeId, NodeId) {
    iterator_identity_call_il_with_args(lang, method, 0)
}

fn iterator_identity_call_il_with_args(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(150), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(151),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Lit,
                Payload::LitInt(idx as i64),
                sp(152 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(160), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(153), &[call]);
    (finish_il(b, root, lang), interner, call, receiver)
}

fn push_protocol_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
}

#[test]
fn admitted_iterator_identity_adapter_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) = iterator_identity_call_il(Lang::Rust, "iter");
    push_protocol_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_iterator_identity_adapter_at_call(&raw_shape, &interner, call).is_none(),
        "raw iterator adapter shape plus protocol receiver proof is not enough"
    );

    let contract = library_iterator_identity_adapter_contract(Lang::Rust, "iter", 0)
        .expect("Rust iter contract");

    let (mut missing_dependency, interner, call, _receiver) =
        iterator_identity_call_il(Lang::Rust, "iter");
    missing_dependency.push_evidence(iterator_identity_adapter_record(
        1,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_iterator_identity_adapter_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span iterator adapter evidence without protocol receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = iterator_identity_call_il(Lang::Rust, "iter");
    push_protocol_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID,
    ));
    assert!(
        admitted_iterator_identity_adapter_at_call(&wrong_pack, &interner, call).is_none(),
        "iterator adapter evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) =
        iterator_identity_call_il(Lang::Rust, "iter");
    push_protocol_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
        ITERATOR_IDENTITY_ADAPTER_PACK_ID,
        "wrong.protocols.iterator-identity-adapter-api",
    ));
    assert!(
        admitted_iterator_identity_adapter_at_call(&wrong_producer, &interner, call).is_none(),
        "iterator adapter evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) =
        iterator_identity_call_il(Lang::Rust, "iter");
    push_protocol_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = iterator_identity_adapter_record(
        1,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_iterator_identity_adapter_at_call(&wrong_emitter, &interner, call).is_none(),
        "iterator adapter evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, receiver) = iterator_identity_call_il(Lang::Rust, "iter");
    push_protocol_receiver_dependency(&mut admitted, receiver);
    admitted.push_evidence(iterator_identity_adapter_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence =
        admitted_iterator_identity_adapter_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::IteratorIdentityAdapter
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 0);
}

#[test]
fn forged_iterator_identity_adapter_evidence_does_not_open_unsupported_shapes() {
    let contract = library_iterator_identity_adapter_contract(Lang::Rust, "iter", 0)
        .expect("Rust iter contract");

    let (mut unsupported_arity, interner, call, receiver) =
        iterator_identity_call_il_with_args(Lang::Rust, "iter", 1);
    push_protocol_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity.push_evidence(iterator_identity_adapter_record(
        1,
        unsupported_arity.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_iterator_identity_adapter_at_call(&unsupported_arity, &interner, call).is_none(),
        "protocol-pack evidence does not open unsupported iterator adapter arities"
    );

    let (mut unsupported_language, interner, call, receiver) =
        iterator_identity_call_il(Lang::JavaScript, "collect");
    push_protocol_receiver_dependency(&mut unsupported_language, receiver);
    unsupported_language.push_evidence(iterator_identity_adapter_record(
        1,
        unsupported_language.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_iterator_identity_adapter_at_call(&unsupported_language, &interner, call)
            .is_none(),
        "protocol-pack evidence does not open non-owned language/method shapes"
    );
}

#[test]
fn admitted_java_stream_identity_adapter_uses_same_protocol_pack() {
    let contract = library_iterator_identity_adapter_contract(Lang::Java, "stream", 0)
        .expect("Java stream contract");
    let (mut il, interner, call, receiver) = iterator_identity_call_il(Lang::Java, "stream");
    push_protocol_receiver_dependency(&mut il, receiver);
    il.push_evidence(iterator_identity_adapter_record(
        1,
        il.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_iterator_identity_adapter_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::IteratorIdentityAdapter
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 0);
}
