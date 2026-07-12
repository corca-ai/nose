use super::*;

fn nested_effectful_source_il(normalized: bool) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let outer_receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(900), &[]);
    let outer_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(901),
        &[outer_receiver],
    );
    let outer_param = b.add(NodeKind::Param, Payload::Cid(1), sp(902), &[]);
    let outer_value = b.add(NodeKind::Var, Payload::Cid(1), sp(903), &[]);
    let observe = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("observe")),
        sp(904),
        &[],
    );
    let observed = b.add(
        NodeKind::Call,
        Payload::None,
        sp(905),
        &[observe, outer_value],
    );
    let nested_source = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(906),
        &[observed],
    );
    let inner_param = b.add(NodeKind::Param, Payload::Cid(2), sp(907), &[]);
    let inner_value = b.add(NodeKind::Var, Payload::Cid(2), sp(908), &[]);
    let inner_return = b.add(NodeKind::Return, Payload::None, sp(909), &[inner_value]);
    let inner_body = b.add(NodeKind::Block, Payload::None, sp(910), &[inner_return]);
    let inner_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(911),
        &[inner_param, inner_body],
    );
    let inner = if normalized {
        b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::Map),
            sp(912),
            &[nested_source, inner_lambda],
        )
    } else {
        let inner_callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("map")),
            sp(912),
            &[nested_source],
        );
        b.add(
            NodeKind::Call,
            Payload::None,
            sp(913),
            &[inner_callee, inner_lambda],
        )
    };
    let outer_return = b.add(NodeKind::Return, Payload::None, sp(914), &[inner]);
    let outer_body = b.add(NodeKind::Block, Payload::None, sp(915), &[outer_return]);
    let outer_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(916),
        &[outer_param, outer_body],
    );
    let outer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(917),
        &[outer_callee, outer_lambda],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(918), &[outer_call]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(outer_receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Array),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(il.node(nested_source).span, NodeKind::Seq),
        EvidenceKind::Domain(DomainEvidence::Array),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(language_core_evidence(
        4,
        EvidenceAnchor::sequence(il.node(nested_source).span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    let map_contract = library_method_call_contract(Lang::JavaScript, "map", 1)
        .expect("JavaScript Array.map contract");
    il.evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            il.node(inner).span,
            map_contract.id,
            map_contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[1],
            map_contract.pack_id,
            map_contract.producer_id,
        ));
    il.evidence
        .push(library_api_record_with_provenance_and_arity(
            3,
            il.node(outer_call).span,
            map_contract.id,
            map_contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            map_contract.pack_id,
            map_contract.producer_id,
        ));
    (il, interner, outer_call, inner)
}

#[test]
fn nested_hof_admission_does_not_hide_effects_in_eager_sources() {
    for normalized in [false, true] {
        let (il, interner, outer_call, inner) = nested_effectful_source_il(normalized);
        if normalized {
            assert!(
                admitted_hof_api_at_node_with_interner(&il, Some(&interner), inner, HoFKind::Map,),
                "the inner normalized HOF should have valid API and receiver evidence"
            );
        } else {
            assert!(
                admitted_library_method_call_at_call(&il, &interner, inner).is_some(),
                "the inner source method call should have valid API and receiver evidence"
            );
        }
        assert!(
            admitted_library_method_call_at_call(&il, &interner, outer_call).is_none(),
            "nested HOF admission must still walk its eagerly evaluated source (normalized={normalized})"
        );
    }
}
