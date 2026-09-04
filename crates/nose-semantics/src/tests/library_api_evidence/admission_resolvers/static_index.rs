use super::*;

fn js_static_index_membership_call_il(
    method: &str,
    lambda_arg: bool,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let red = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(97),
        &[],
    );
    let receiver = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(98),
        &[red],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(99),
        &[receiver],
    );
    let subject = if lambda_arg {
        b.add(NodeKind::Lambda, Payload::None, sp(100), &[])
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("subject")),
            sp(100),
            &[],
        )
    };
    let call = b.add(NodeKind::Call, Payload::None, sp(101), &[callee, subject]);
    let root = b.add(NodeKind::Func, Payload::None, sp(102), &[call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        call,
        callee,
        receiver,
    )
}

fn push_static_index_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.push_evidence(language_core_evidence_with_dependencies(
        0,
        EvidenceAnchor::sequence(il.node(receiver).span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        vec![],
        Lang::JavaScript,
    ));
}

#[test]
fn admitted_static_index_membership_resolver_requires_static_index_builtin_pack_provenance() {
    let (il, interner, call, _callee, _receiver) =
        js_static_index_membership_call_il("indexOf", false);
    assert!(
        admitted_static_index_membership_at_call(&il, &interner, call).is_none(),
        "raw indexOf(...) shape alone must not admit static membership semantics"
    );

    let contract = library_static_index_membership_contract(Lang::JavaScript, "indexOf", 1)
        .expect("indexOf contract");

    let (mut missing_dependency, interner, call, _callee, _receiver) =
        js_static_index_membership_call_il("indexOf", false);
    missing_dependency.push_evidence(js_like_builtin_static_index_membership_record(
        0,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_static_index_membership_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span static index evidence without collection receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, _callee, receiver) =
        js_static_index_membership_call_il("indexOf", false);
    push_static_index_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
    ));
    assert!(
        admitted_static_index_membership_at_call(&wrong_pack, &interner, call).is_none(),
        "static index evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, _callee, receiver) =
        js_static_index_membership_call_il("indexOf", false);
    push_static_index_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
        "wrong.javascript.builtins.static-index-membership-api",
    ));
    assert!(
        admitted_static_index_membership_at_call(&wrong_producer, &interner, call).is_none(),
        "static index evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, _callee, receiver) =
        js_static_index_membership_call_il("indexOf", false);
    push_static_index_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = js_like_builtin_static_index_membership_record(
        1,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_static_index_membership_at_call(&wrong_emitter, &interner, call).is_none(),
        "static index evidence from an external emitter is rejected"
    );

    let (mut broad_receiver_dependency, interner, call, _callee, receiver) =
        js_static_index_membership_call_il("indexOf", false);
    broad_receiver_dependency.push_evidence(evidence_with_dependencies(
        0,
        EvidenceAnchor::sequence(broad_receiver_dependency.node(receiver).span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    broad_receiver_dependency.push_evidence(js_like_builtin_static_index_membership_record(
        1,
        broad_receiver_dependency.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_static_index_membership_at_call(&broad_receiver_dependency, &interner, call)
            .is_none(),
        "static index evidence cannot depend on broad sequence-surface proof"
    );

    let (mut admitted, interner, call, callee, receiver) =
        js_static_index_membership_call_il("indexOf", false);
    push_static_index_receiver_dependency(&mut admitted, receiver);
    admitted.push_evidence(js_like_builtin_static_index_membership_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_static_index_membership_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::IndexOf)
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_static_index_membership_resolver_accepts_find_index_contract() {
    let contract = library_static_index_membership_contract(Lang::JavaScript, "findIndex", 1)
        .expect("findIndex contract");
    let (mut il, interner, call, _callee, receiver) =
        js_static_index_membership_call_il("findIndex", true);
    push_static_index_receiver_dependency(&mut il, receiver);
    il.push_evidence(js_like_builtin_static_index_membership_record(
        1,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_static_index_membership_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::FindIndex)
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}
