use super::*;

fn receiver_membership_call_il(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(162), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(163),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(1 + idx as u32),
                sp(164 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(168), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(169), &[call]);
    (finish_il(b, root, lang), interner, call, receiver)
}

fn push_receiver_domain_dependency(il: &mut Il, receiver: NodeId, domain: DomainEvidence) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
}

#[test]
fn receiver_membership_can_consume_safe_api_result_domain_record() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(180), &[]);
    let key_set_field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("keySet")),
        sp(181),
        &[receiver],
    );
    let key_set_call = b.add(NodeKind::Call, Payload::None, sp(182), &[key_set_field]);
    let contains_field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("contains")),
        sp(183),
        &[key_set_call],
    );
    let item = b.add(NodeKind::Var, Payload::Cid(1), sp(184), &[]);
    let contains_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(185),
        &[contains_field, item],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(186), &[contains_call]);
    let mut il = finish_il(b, root, Lang::Java);
    push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Map);

    let key_set =
        library_map_key_view_contract(Lang::Java, "keySet", 0).expect("Java Map.keySet contract");
    il.evidence.push(map_key_view_protocol_record(
        1,
        il.node(key_set_call).span,
        key_set,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let contains = library_receiver_membership_contract(Lang::Java, "contains", 1)
        .expect("Java collection membership contract");
    il.evidence.push(receiver_membership_protocol_record(
        2,
        il.node(contains_call).span,
        contains,
        EvidenceStatus::Asserted,
        &[1],
    ));

    assert!(
        il.evidence
            .iter()
            .all(|record| !matches!(record.kind, EvidenceKind::Domain(DomainEvidence::Collection))),
        "this test covers the LibraryApi result-domain fallback, not emitted call-node DomainEvidence"
    );
    assert!(
        admitted_map_key_view_at_call(&il, &interner, key_set_call).is_some(),
        "first API occurrence must be admitted before it can prove a result domain"
    );
    let occurrence = admitted_library_method_call_at_call(&il, &interner, contains_call)
        .expect("safe Map.keySet result-domain API record admits chained contains");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains))
    );
}

fn assert_admitted_receiver_membership(
    lang: Lang,
    method: &str,
    domain: DomainEvidence,
    receiver_contract: MethodReceiverContract,
) {
    let (mut il, interner, call, receiver) = receiver_membership_call_il(lang, method, 1);
    push_receiver_domain_dependency(&mut il, receiver, domain);
    let contract = library_receiver_membership_contract(lang, method, 1)
        .expect("receiver-membership contract");
    il.evidence.push(receiver_membership_protocol_record(
        1,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence = admitted_library_method_call_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains))
    );
    assert_eq!(
        occurrence.contract.result.args,
        MethodBuiltinArgs::FirstThenReceiver
    );
    assert_eq!(occurrence.contract.result.receiver, receiver_contract);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_receiver_membership_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 1);
    push_receiver_domain_dependency(&mut raw_shape, receiver, DomainEvidence::Map);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw membership shape plus receiver proof is not enough"
    );

    let contract = library_receiver_membership_contract(Lang::Java, "containsKey", 1)
        .expect("Java receiver-membership contract");

    let (mut missing_dependency, interner, call, _receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 1);
    missing_dependency
        .evidence
        .push(receiver_membership_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "receiver-membership evidence without receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 1);
    push_receiver_domain_dependency(&mut wrong_pack, receiver, DomainEvidence::Map);
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
    ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "receiver-membership evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 1);
    push_receiver_domain_dependency(&mut wrong_producer, receiver, DomainEvidence::Map);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
            RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            "wrong.protocols.receiver-membership-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "receiver-membership evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 1);
    push_receiver_domain_dependency(&mut wrong_emitter, receiver, DomainEvidence::Map);
    let mut external_record = receiver_membership_protocol_record(
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
        "receiver-membership evidence from an external emitter is rejected"
    );

    assert_admitted_receiver_membership(
        Lang::Java,
        "containsKey",
        DomainEvidence::Map,
        MethodReceiverContract::ExactMap,
    );
    assert_admitted_receiver_membership(
        Lang::Rust,
        "contains_key",
        DomainEvidence::Map,
        MethodReceiverContract::ExactMap,
    );
    assert_admitted_receiver_membership(
        Lang::Ruby,
        "key?",
        DomainEvidence::Map,
        MethodReceiverContract::ExactMap,
    );
    assert_admitted_receiver_membership(
        Lang::Ruby,
        "has_key?",
        DomainEvidence::Map,
        MethodReceiverContract::ExactMap,
    );
    assert_admitted_receiver_membership(
        Lang::Python,
        "__contains__",
        DomainEvidence::Collection,
        MethodReceiverContract::ExactCollectionOrMap,
    );
    assert_admitted_receiver_membership(
        Lang::TypeScript,
        "has",
        DomainEvidence::Set,
        MethodReceiverContract::ExactSetOrMap,
    );
    assert_admitted_receiver_membership(
        Lang::JavaScript,
        "includes",
        DomainEvidence::Collection,
        MethodReceiverContract::ExactCollectionOrJavaKeySet,
    );
    assert_admitted_receiver_membership(
        Lang::Java,
        "contains",
        DomainEvidence::Collection,
        MethodReceiverContract::ExactCollectionOrJavaKeySet,
    );
    assert_admitted_receiver_membership(
        Lang::Swift,
        "contains",
        DomainEvidence::Collection,
        MethodReceiverContract::ExactCollectionOrJavaKeySet,
    );
    assert_admitted_receiver_membership(
        Lang::Ruby,
        "member?",
        DomainEvidence::Collection,
        MethodReceiverContract::ExactCollectionOrJavaKeySet,
    );
}

#[test]
fn forged_receiver_membership_evidence_does_not_open_unsupported_arity() {
    let contract = library_receiver_membership_contract(Lang::Java, "containsKey", 1)
        .expect("Java receiver-membership contract");
    let (mut unsupported_arity, interner, call, receiver) =
        receiver_membership_call_il(Lang::Java, "containsKey", 2);
    push_receiver_domain_dependency(&mut unsupported_arity, receiver, DomainEvidence::Map);
    unsupported_arity
        .evidence
        .push(receiver_membership_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged receiver-membership evidence cannot open unsupported source arity"
    );
}

#[test]
fn receiver_membership_protocol_covers_receiver_surfaces_but_not_go_namespace_function() {
    assert!(library_receiver_membership_contract(Lang::Java, "containsKey", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Rust, "contains_key", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Ruby, "key?", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Ruby, "has_key?", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Python, "__contains__", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::TypeScript, "has", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::JavaScript, "has", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Java, "contains", 1).is_some());
    assert!(library_receiver_membership_contract(Lang::Ruby, "member?", 1).is_some());

    let python = library_receiver_membership_contract(Lang::Python, "__contains__", 1)
        .expect("Python collection-or-map membership is receiver membership");
    assert_eq!(
        python.result.receiver,
        MethodReceiverContract::ExactCollectionOrMap
    );
    let js_has = library_receiver_membership_contract(Lang::TypeScript, "has", 1)
        .expect("JS set-or-map membership is receiver membership");
    assert_eq!(
        js_has.result.receiver,
        MethodReceiverContract::ExactSetOrMap
    );
    let go_contains = library_method_call_contract(Lang::Go, "Contains", 2)
        .expect("Go slices.Contains remains a namespace-function method-call contract");
    assert_eq!(go_contains.result.args, MethodBuiltinArgs::GoSliceContains);
    assert!(library_receiver_membership_contract(Lang::Go, "Contains", 2).is_none());
}
