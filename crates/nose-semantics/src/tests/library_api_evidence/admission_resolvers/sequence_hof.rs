use super::*;

mod ruby;
mod swift;

fn sequence_hof_call_il(method: &str, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(180), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(181),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(1 + idx as u32),
                sp(182 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(190), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(191), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, receiver)
}

fn push_protocol_receiver_dependency(il: &mut Il, receiver: NodeId) {
    push_receiver_domain_dependency(il, 0, receiver, DomainEvidence::Collection);
}

fn push_receiver_domain_dependency(il: &mut Il, id: u32, receiver: NodeId, domain: DomainEvidence) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
}

fn sequence_hof_record(
    id: u32,
    il: &Il,
    call: NodeId,
    contract: LibraryMethodCallContract,
    arity: u16,
    dependencies: &[u32],
) -> EvidenceRecord {
    asserted_library_api_node_record_with_provenance(
        id,
        il,
        call,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
        SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
    )
}

struct OrderedHofPackRequirementMessages {
    raw_shape: &'static str,
    missing_dependency: &'static str,
    wrong_pack: &'static str,
    wrong_producer: &'static str,
    admitted: &'static str,
}

fn assert_sequence_hof_requires_pack_and_ordered_receiver(
    lang: Lang,
    method: &'static str,
    expected: MethodSemanticContract,
    make_call: impl Fn() -> (Il, Interner, NodeId, NodeId),
    rejected_domains: &[(DomainEvidence, &'static str)],
    messages: OrderedHofPackRequirementMessages,
) {
    let (mut raw_shape, interner, call, receiver) = make_call();
    push_receiver_domain_dependency(&mut raw_shape, 0, receiver, DomainEvidence::Collection);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "{}",
        messages.raw_shape
    );

    let contract = library_method_call_contract(lang, method, 1).expect("sequence HOF row");
    assert_eq!(
        contract.callee,
        LibraryApiCalleeContract::Method {
            method,
            receiver: MethodReceiverContract::ExactArrayOrCollection,
        }
    );

    let (mut missing_dependency, interner, call, _receiver) = make_call();
    missing_dependency.evidence.push(sequence_hof_record(
        1,
        &missing_dependency,
        call,
        contract,
        1,
        &[],
    ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "{}",
        messages.missing_dependency
    );

    let (mut wrong_pack, interner, call, receiver) = make_call();
    push_receiver_domain_dependency(&mut wrong_pack, 0, receiver, DomainEvidence::Collection);
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
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "{}",
        messages.wrong_pack
    );

    let (mut wrong_producer, interner, call, receiver) = make_call();
    push_receiver_domain_dependency(&mut wrong_producer, 0, receiver, DomainEvidence::Collection);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            "wrong.protocols.sequence-hof-adapter-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "{}",
        messages.wrong_producer
    );

    for &(domain, message) in rejected_domains {
        let (mut il, interner, call, receiver) = make_call();
        push_receiver_domain_dependency(&mut il, 0, receiver, domain);
        il.evidence
            .push(sequence_hof_record(1, &il, call, contract, 1, &[0]));
        assert!(
            admitted_library_method_call_at_call(&il, &interner, call).is_none(),
            "{message}"
        );
    }

    let (mut admitted, interner, call, receiver) = make_call();
    push_receiver_domain_dependency(&mut admitted, 0, receiver, DomainEvidence::Collection);
    admitted
        .evidence
        .push(sequence_hof_record(1, &admitted, call, contract, 1, &[0]));
    let occurrence =
        admitted_library_method_call_at_call(&admitted, &interner, call).expect(messages.admitted);
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(expected)
    );
    assert_eq!(
        occurrence.contract.pack_id,
        SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_rust_iterator_hof_requires_sequence_hof_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) = sequence_hof_call_il("map", 1);
    push_protocol_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw iterator map shape plus receiver proof is not enough"
    );

    let contract = library_method_call_contract(Lang::Rust, "map", 1).expect("Rust map row");

    let (mut missing_dependency, interner, call, _receiver) = sequence_hof_call_il("map", 1);
    missing_dependency.evidence.push(sequence_hof_record(
        1,
        &missing_dependency,
        call,
        contract,
        1,
        &[],
    ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Rust iterator HOF evidence without receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = sequence_hof_call_il("map", 1);
    push_protocol_receiver_dependency(&mut wrong_pack, receiver);
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
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust iterator HOF evidence under the generic method-call pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = sequence_hof_call_il("map", 1);
    push_protocol_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            "wrong.protocols.sequence-hof-adapter-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust iterator HOF evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, receiver) = sequence_hof_call_il("map", 1);
    push_protocol_receiver_dependency(&mut admitted, receiver);
    admitted
        .evidence
        .push(sequence_hof_record(1, &admitted, call, contract, 1, &[0]));
    let occurrence = admitted_library_method_call_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map))
    );
    assert_eq!(
        occurrence.contract.pack_id,
        SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_rust_iterator_hof_pack_covers_lazy_adapters_and_terminals() {
    for (method, arity, semantic) in [
        ("filter", 1, MethodSemanticContract::HoF(HoFKind::Filter)),
        (
            "filter_map",
            1,
            MethodSemanticContract::HoF(HoFKind::FilterMap),
        ),
        ("flat_map", 1, MethodSemanticContract::HoF(HoFKind::FlatMap)),
        ("any", 1, MethodSemanticContract::Builtin(Builtin::Any)),
        ("all", 1, MethodSemanticContract::Builtin(Builtin::All)),
        ("count", 0, MethodSemanticContract::Builtin(Builtin::Len)),
    ] {
        let (mut il, interner, call, receiver) = sequence_hof_call_il(method, arity);
        push_protocol_receiver_dependency(&mut il, receiver);
        let contract =
            library_method_call_contract(Lang::Rust, method, arity).expect("Rust method row");
        il.evidence.push(sequence_hof_record(
            1,
            &il,
            call,
            contract,
            arity as u16,
            &[0],
        ));
        let occurrence = admitted_library_method_call_at_call(&il, &interner, call).unwrap();
        assert_eq!(
            occurrence.contract.id,
            LibraryApiContractId::MethodCall(semantic)
        );
        assert_eq!(
            occurrence.contract.pack_id,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID
        );
    }
}

#[test]
fn sequence_hof_pack_does_not_open_unsupported_find_shape() {
    let (mut il, interner, call, receiver) = sequence_hof_call_il("find", 1);
    push_protocol_receiver_dependency(&mut il, receiver);
    let forged_contract =
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains));
    il.evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            il.node(call).span,
            forged_contract,
            LibraryApiCalleeContract::Method {
                method: "find",
                receiver: MethodReceiverContract::ExactProtocol,
            },
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&il, &interner, call).is_none(),
        "sequence-HOF provenance does not mint unsupported Rust find semantics"
    );
}

#[test]
fn sequence_hof_pack_rejects_custom_methods_and_ecosystem_adapters() {
    let (mut custom_map, interner, call, receiver) = sequence_hof_call_il("map", 1);
    push_protocol_receiver_dependency(&mut custom_map, receiver);
    custom_map
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            custom_map.node(call).span,
            LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map)),
            LibraryApiCalleeContract::Method {
                method: "map",
                receiver: MethodReceiverContract::ExactMap,
            },
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&custom_map, &interner, call).is_none(),
        "sequence-HOF provenance does not turn a same-named custom map method into an iterator HOF"
    );

    let (mut collect_vec, interner, call, receiver) = sequence_hof_call_il("collect_vec", 0);
    push_protocol_receiver_dependency(&mut collect_vec, receiver);
    collect_vec
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            collect_vec.node(call).span,
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Len)),
            LibraryApiCalleeContract::Method {
                method: "collect_vec",
                receiver: MethodReceiverContract::ExactProtocol,
            },
            0,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&collect_vec, &interner, call).is_none(),
        "sequence-HOF provenance does not open ecosystem collect_vec as a std Iterator terminal"
    );
}
