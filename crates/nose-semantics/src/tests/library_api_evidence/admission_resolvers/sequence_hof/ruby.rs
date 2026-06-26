use super::*;

#[derive(Clone, Copy)]
enum RubyCallbackShape {
    Inline,
    Reference,
    EffectfulCall,
    MutatingAssign,
    Throwing,
}

impl RubyCallbackShape {
    fn fixture(self) -> CallbackFixtureShape {
        match self {
            Self::Inline => CallbackFixtureShape::InlineFunc { cid: 1 },
            Self::Reference => CallbackFixtureShape::Reference { name: "transform" },
            Self::EffectfulCall => CallbackFixtureShape::EffectfulCall {
                callee: "side_effect",
                arg_cid: 1,
            },
            Self::MutatingAssign => CallbackFixtureShape::MutatingAssign {
                lhs_cid: 2,
                rhs_cid: 1,
            },
            Self::Throwing => CallbackFixtureShape::Throwing { err_cid: 1 },
        }
    }
}

fn ruby_enumerable_hof_call_il(
    method: &str,
    shape: RubyCallbackShape,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(400), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(401),
        &[receiver],
    );
    let callback = callback_fixture_node(&mut b, &interner, shape.fixture(), 402);
    let call = b.add(NodeKind::Call, Payload::None, sp(410), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(411), &[call]);
    (finish_il(b, root, Lang::Ruby), interner, call, receiver)
}

fn ruby_enumerable_no_block_call_il(method: &str) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(420), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(421),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(422), &[callee]);
    let root = b.add(NodeKind::Func, Payload::None, sp(423), &[call]);
    (finish_il(b, root, Lang::Ruby), interner, call, receiver)
}

fn ruby_lazy_enumerator_hof_call_il(method: &str) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(430), &[]);
    let lazy = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("lazy")),
        sp(431),
        &[receiver],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(432),
        &[lazy],
    );
    let callback =
        callback_fixture_node(&mut b, &interner, RubyCallbackShape::Inline.fixture(), 433);
    let call = b.add(NodeKind::Call, Payload::None, sp(434), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(435), &[call]);
    (
        finish_il(b, root, Lang::Ruby),
        interner,
        call,
        receiver,
        lazy,
    )
}

fn ruby_enumerable_hof_chain_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(440), &[]);
    let reject_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("reject")),
        sp(441),
        &[receiver],
    );
    let reject_fn = b.add(NodeKind::Func, Payload::Cid(1), sp(442), &[]);
    let reject_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(443),
        &[reject_callee, reject_fn],
    );
    let map_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(444),
        &[reject_call],
    );
    let map_fn = b.add(NodeKind::Func, Payload::Cid(2), sp(445), &[]);
    let map_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(446),
        &[map_callee, map_fn],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(447), &[map_call]);
    (
        finish_il(b, root, Lang::Ruby),
        interner,
        map_call,
        reject_call,
        receiver,
    )
}

#[test]
fn admitted_ruby_enumerable_hof_requires_sequence_hof_pack_block_and_ordered_collection_receiver() {
    assert_sequence_hof_requires_pack_and_ordered_receiver(
        Lang::Ruby,
        "map",
        MethodSemanticContract::HoF(HoFKind::Map),
        || ruby_enumerable_hof_call_il("map", RubyCallbackShape::Inline),
        &[
            (
                DomainEvidence::Set,
                "Ruby Set receiver proof stays closed because order is not represented",
            ),
            (
                DomainEvidence::Map,
                "Ruby Hash receiver proof stays closed because key/value iteration shape is not represented",
            ),
            (
                DomainEvidence::Iterable,
                "Ruby lazy/framework Enumerable receiver proof stays closed",
            ),
        ],
        OrderedHofPackRequirementMessages {
            raw_shape: "raw Ruby map shape plus collection receiver proof is not enough",
            missing_dependency:
                "same-span Ruby Enumerable HOF evidence without collection proof is rejected",
            wrong_pack: "Ruby Enumerable HOF evidence under the generic method-call pack is rejected",
            wrong_producer: "Ruby Enumerable HOF evidence with the wrong producer is rejected",
            admitted: "Ruby map with collection proof and inline block admits",
        },
    );
}

#[test]
fn admitted_ruby_enumerable_hof_pack_covers_supported_eager_hofs() {
    for (method, semantic) in [
        ("map", MethodSemanticContract::HoF(HoFKind::Map)),
        ("collect", MethodSemanticContract::HoF(HoFKind::Map)),
        ("select", MethodSemanticContract::HoF(HoFKind::Filter)),
        ("filter", MethodSemanticContract::HoF(HoFKind::Filter)),
        ("reject", MethodSemanticContract::HoF(HoFKind::Reject)),
    ] {
        let (mut il, interner, call, receiver) =
            ruby_enumerable_hof_call_il(method, RubyCallbackShape::Inline);
        push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
        let contract = library_method_call_contract(Lang::Ruby, method, 1).expect("Ruby HOF row");
        il.evidence
            .push(sequence_hof_record(1, &il, call, contract, 1, &[0]));
        let occurrence = admitted_library_method_call_at_call(&il, &interner, call)
            .expect("supported Ruby Enumerable HOF admits");
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
fn ruby_enumerable_hof_rejects_no_block_lazy_custom_and_unsupported_surfaces() {
    let contract = library_method_call_contract(Lang::Ruby, "map", 1).expect("Ruby map row");

    let (mut no_block, interner, call, receiver) = ruby_enumerable_no_block_call_il("map");
    push_receiver_domain_dependency(&mut no_block, 0, receiver, DomainEvidence::Collection);
    no_block
        .evidence
        .push(sequence_hof_record(1, &no_block, call, contract, 0, &[0]));
    assert!(
        admitted_library_method_call_at_call(&no_block, &interner, call).is_none(),
        "Ruby Enumerable methods without blocks return Enumerator and stay closed"
    );

    let (mut base_proof, interner, call, receiver, _lazy) = ruby_lazy_enumerator_hof_call_il("map");
    push_receiver_domain_dependency(&mut base_proof, 0, receiver, DomainEvidence::Collection);
    base_proof
        .evidence
        .push(sequence_hof_record(1, &base_proof, call, contract, 1, &[0]));
    assert!(
        admitted_library_method_call_at_call(&base_proof, &interner, call).is_none(),
        "Ruby lazy.map does not inherit eager collection proof from the base receiver"
    );

    let (mut lazy_iterable, interner, call, _receiver, lazy) =
        ruby_lazy_enumerator_hof_call_il("map");
    push_receiver_domain_dependency(&mut lazy_iterable, 0, lazy, DomainEvidence::Iterable);
    lazy_iterable.evidence.push(sequence_hof_record(
        1,
        &lazy_iterable,
        call,
        contract,
        1,
        &[0],
    ));
    assert!(
        admitted_library_method_call_at_call(&lazy_iterable, &interner, call).is_none(),
        "Ruby Enumerator::Lazy stays closed until delayed demand is modeled"
    );

    let (mut custom_map, interner, call, receiver) =
        ruby_enumerable_hof_call_il("map", RubyCallbackShape::Inline);
    push_receiver_domain_dependency(&mut custom_map, 0, receiver, DomainEvidence::Collection);
    custom_map
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            custom_map.node(call).span,
            LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map)),
            LibraryApiCalleeContract::Method {
                method: "map",
                receiver: MethodReceiverContract::ExactProtocol,
            },
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&custom_map, &interner, call).is_none(),
        "Ruby same-name custom methods do not become Enumerable HOFs"
    );

    let (mut flat_map, interner, call, receiver) =
        ruby_enumerable_hof_call_il("flat_map", RubyCallbackShape::Inline);
    push_receiver_domain_dependency(&mut flat_map, 0, receiver, DomainEvidence::Collection);
    flat_map
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            flat_map.node(call).span,
            LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::FlatMap)),
            LibraryApiCalleeContract::Method {
                method: "flat_map",
                receiver: MethodReceiverContract::ExactArrayOrCollection,
            },
            1,
            EvidenceStatus::Asserted,
            &[0],
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&flat_map, &interner, call).is_none(),
        "Ruby flat_map stays closed until nested flattening semantics are represented"
    );
}

#[test]
fn ruby_enumerable_hof_rejects_non_inline_or_effectful_blocks() {
    let contract = library_method_call_contract(Lang::Ruby, "map", 1).expect("Ruby map row");
    for (shape, message) in [
        (
            RubyCallbackShape::Reference,
            "callback references stay closed until callable effects are proven",
        ),
        (
            RubyCallbackShape::EffectfulCall,
            "unknown calls inside Ruby blocks stay closed",
        ),
        (
            RubyCallbackShape::MutatingAssign,
            "receiver or captured mutation inside Ruby blocks stays closed",
        ),
        (
            RubyCallbackShape::Throwing,
            "raising Ruby blocks stay closed",
        ),
    ] {
        let (mut il, interner, call, receiver) = ruby_enumerable_hof_call_il("map", shape);
        push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
        il.evidence
            .push(sequence_hof_record(1, &il, call, contract, 1, &[0]));
        assert!(
            admitted_library_method_call_at_call(&il, &interner, call).is_none(),
            "{message}"
        );
    }
}

#[test]
fn ruby_enumerable_hof_result_domain_can_prove_follow_on_ordered_receiver() {
    let (mut il, interner, map_call, reject_call, receiver) = ruby_enumerable_hof_chain_il();
    push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
    let reject_contract =
        library_method_call_contract(Lang::Ruby, "reject", 1).expect("Ruby reject row");
    il.evidence.push(sequence_hof_record(
        1,
        &il,
        reject_call,
        reject_contract,
        1,
        &[0],
    ));
    let map_contract = library_method_call_contract(Lang::Ruby, "map", 1).expect("Ruby map row");
    il.evidence
        .push(sequence_hof_record(2, &il, map_call, map_contract, 1, &[1]));

    let occurrence = admitted_library_method_call_at_call(&il, &interner, map_call)
        .expect("Ruby reject result proves follow-on Ruby map receiver");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map))
    );
    assert_eq!(occurrence.receiver, Some(reject_call));
}
