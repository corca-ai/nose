use super::*;

#[derive(Clone, Copy)]
enum SwiftCallbackShape {
    Inline,
    MultiParam,
    Reference,
    EffectfulCall,
    MutatingAssign,
    Throwing,
}

impl SwiftCallbackShape {
    fn fixture(self) -> CallbackFixtureShape {
        match self {
            Self::Inline | Self::MultiParam => CallbackFixtureShape::InlineFunc { cid: 1 },
            Self::Reference => CallbackFixtureShape::Reference { name: "transform" },
            Self::EffectfulCall => CallbackFixtureShape::EffectfulCall {
                callee: "sideEffect",
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

fn swift_sequence_hof_call_il(
    method: &str,
    shape: SwiftCallbackShape,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(300), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(301),
        &[receiver],
    );
    let callback = match shape {
        SwiftCallbackShape::Inline => inline_lambda_with_params(&mut b, 1, 1, 302),
        SwiftCallbackShape::MultiParam => inline_lambda_with_params(&mut b, 1, 2, 302),
        _ => callback_fixture_node(&mut b, &interner, shape.fixture(), 302),
    };
    let call = b.add(NodeKind::Call, Payload::None, sp(310), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(311), &[call]);
    (finish_il(b, root, Lang::Swift), interner, call, receiver)
}

fn swift_lazy_sequence_hof_call_il(method: &str) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(320), &[]);
    let lazy = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("lazy")),
        sp(321),
        &[receiver],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(322),
        &[lazy],
    );
    let callback = inline_lambda_with_params(&mut b, 1, 1, 323);
    let call = b.add(NodeKind::Call, Payload::None, sp(324), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(325), &[call]);
    (
        finish_il(b, root, Lang::Swift),
        interner,
        call,
        receiver,
        lazy,
    )
}

fn swift_sequence_hof_chain_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(330), &[]);
    let map_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(331),
        &[receiver],
    );
    let map_fn = inline_lambda_with_params(&mut b, 1, 1, 332);
    let map_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(333),
        &[map_callee, map_fn],
    );
    let filter_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("filter")),
        sp(334),
        &[map_call],
    );
    let filter_fn = inline_lambda_with_params(&mut b, 2, 1, 335);
    let filter_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(336),
        &[filter_callee, filter_fn],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(337), &[filter_call]);
    (
        finish_il(b, root, Lang::Swift),
        interner,
        filter_call,
        map_call,
        receiver,
    )
}

#[test]
fn swift_sequence_hof_pack_rejects_lazy_adapter_receiver() {
    for (method, semantic) in [
        ("map", MethodSemanticContract::HoF(HoFKind::Map)),
        ("allSatisfy", MethodSemanticContract::Builtin(Builtin::All)),
    ] {
        let contract =
            library_method_call_contract(Lang::Swift, method, 1).expect("Swift sequence HOF row");
        assert_eq!(contract.id, LibraryApiContractId::MethodCall(semantic));

        let (mut base_proof, interner, call, receiver, _lazy) =
            swift_lazy_sequence_hof_call_il(method);
        push_receiver_domain_dependency(&mut base_proof, 0, receiver, DomainEvidence::Collection);
        base_proof.push_evidence(sequence_hof_record(1, &base_proof, call, contract, 1, &[0]));
        assert!(
            admitted_library_method_call_at_call(&base_proof, &interner, call).is_none(),
            "Swift .lazy.{method} does not inherit eager Array/Collection proof from the base receiver"
        );

        let (mut lazy_iterable, interner, call, _receiver, lazy) =
            swift_lazy_sequence_hof_call_il(method);
        push_receiver_domain_dependency(&mut lazy_iterable, 0, lazy, DomainEvidence::Iterable);
        lazy_iterable.push_evidence(sequence_hof_record(
            1,
            &lazy_iterable,
            call,
            contract,
            1,
            &[0],
        ));
        assert!(
            admitted_library_method_call_at_call(&lazy_iterable, &interner, call).is_none(),
            "Swift .lazy.{method} stays closed until lazy demand and one-shot semantics are modeled"
        );
    }
}

#[test]
fn admitted_swift_sequence_hof_requires_sequence_hof_pack_and_ordered_collection_receiver() {
    assert_sequence_hof_requires_pack_and_ordered_receiver(
        Lang::Swift,
        "map",
        MethodSemanticContract::HoF(HoFKind::Map),
        || swift_sequence_hof_call_il("map", SwiftCallbackShape::Inline),
        &[
            (
                DomainEvidence::Set,
                "Swift Set receiver proof stays closed because order is not represented",
            ),
            (
                DomainEvidence::Map,
                "Swift Dictionary receiver proof stays closed for Sequence HOF admission",
            ),
            (
                DomainEvidence::Iterable,
                "Swift AnySequence/one-shot receiver proof stays closed",
            ),
        ],
        OrderedHofPackRequirementMessages {
            raw_shape: "raw Swift map shape plus collection receiver proof is not enough",
            missing_dependency:
                "same-span Swift HOF evidence without ordered collection proof is rejected",
            wrong_pack: "Swift HOF evidence under the generic method-call pack is rejected",
            wrong_producer: "Swift HOF evidence with the wrong producer is rejected",
            admitted: "Swift map with ordered collection proof admits",
        },
    );

    assert_sequence_hof_requires_pack_and_ordered_receiver(
        Lang::Swift,
        "allSatisfy",
        MethodSemanticContract::Builtin(Builtin::All),
        || swift_sequence_hof_call_il("allSatisfy", SwiftCallbackShape::Inline),
        &[
            (
                DomainEvidence::Set,
                "Swift Set receiver proof stays closed because order is not represented",
            ),
            (
                DomainEvidence::Map,
                "Swift Dictionary receiver proof stays closed for Sequence allSatisfy admission",
            ),
            (
                DomainEvidence::Iterable,
                "Swift AnySequence/one-shot receiver proof stays closed",
            ),
        ],
        OrderedHofPackRequirementMessages {
            raw_shape: "raw Swift allSatisfy shape plus collection receiver proof is not enough",
            missing_dependency:
                "same-span Swift allSatisfy evidence without ordered collection proof is rejected",
            wrong_pack: "Swift allSatisfy evidence under the generic method-call pack is rejected",
            wrong_producer: "Swift allSatisfy evidence with the wrong producer is rejected",
            admitted: "Swift allSatisfy with ordered collection proof admits",
        },
    );
}

#[test]
fn admitted_swift_sequence_hof_pack_covers_supported_eager_non_flattening_hofs() {
    for (method, semantic) in [
        ("map", MethodSemanticContract::HoF(HoFKind::Map)),
        ("filter", MethodSemanticContract::HoF(HoFKind::Filter)),
        ("allSatisfy", MethodSemanticContract::Builtin(Builtin::All)),
    ] {
        let (mut il, interner, call, receiver) =
            swift_sequence_hof_call_il(method, SwiftCallbackShape::Inline);
        push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
        let contract = library_method_call_contract(Lang::Swift, method, 1).expect("Swift HOF row");
        il.push_evidence(sequence_hof_record(1, &il, call, contract, 1, &[0]));
        let occurrence = admitted_library_method_call_at_call(&il, &interner, call)
            .expect("supported Swift Sequence HOF admits");
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
fn swift_sequence_hof_pack_rejects_non_inline_or_effectful_callbacks() {
    for method in ["map", "allSatisfy"] {
        let contract =
            library_method_call_contract(Lang::Swift, method, 1).expect("Swift sequence HOF row");
        for (shape, message) in [
            (
                SwiftCallbackShape::Reference,
                "callback references stay closed until callable effects are proven",
            ),
            (
                SwiftCallbackShape::EffectfulCall,
                "unknown calls inside Swift HOF callbacks stay closed",
            ),
            (
                SwiftCallbackShape::MutatingAssign,
                "captured mutation inside Swift HOF callbacks stays closed",
            ),
            (
                SwiftCallbackShape::Throwing,
                "throwing Swift HOF callbacks stay closed",
            ),
        ] {
            let (mut il, interner, call, receiver) = swift_sequence_hof_call_il(method, shape);
            push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
            il.push_evidence(sequence_hof_record(1, &il, call, contract, 1, &[0]));
            assert!(
                admitted_library_method_call_at_call(&il, &interner, call).is_none(),
                "{method}: {message}"
            );
        }
    }
}

#[test]
fn swift_sequence_hof_pack_rejects_multi_param_callbacks() {
    for method in ["map", "allSatisfy"] {
        let contract =
            library_method_call_contract(Lang::Swift, method, 1).expect("Swift sequence HOF row");
        let (mut il, interner, call, receiver) =
            swift_sequence_hof_call_il(method, SwiftCallbackShape::MultiParam);
        push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
        il.push_evidence(sequence_hof_record(1, &il, call, contract, 1, &[0]));
        assert!(
            admitted_library_method_call_at_call(&il, &interner, call).is_none(),
            "Swift {method} admission must stay value-only so custom overloads cannot masquerade as Sequence HOFs"
        );
    }
}

#[test]
fn swift_sequence_hof_result_domain_can_prove_follow_on_ordered_receiver() {
    let (mut il, interner, filter_call, map_call, receiver) = swift_sequence_hof_chain_il();
    push_receiver_domain_dependency(&mut il, 0, receiver, DomainEvidence::Collection);
    let map_contract = library_method_call_contract(Lang::Swift, "map", 1).expect("Swift map row");
    il.push_evidence(sequence_hof_record(1, &il, map_call, map_contract, 1, &[0]));
    let filter_contract =
        library_method_call_contract(Lang::Swift, "filter", 1).expect("Swift filter row");
    il.push_evidence(sequence_hof_record(
        2,
        &il,
        filter_call,
        filter_contract,
        1,
        &[1],
    ));

    let occurrence = admitted_library_method_call_at_call(&il, &interner, filter_call)
        .expect("Swift map result proves follow-on Swift filter receiver");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Filter))
    );
    assert_eq!(occurrence.receiver, Some(map_call));
}
