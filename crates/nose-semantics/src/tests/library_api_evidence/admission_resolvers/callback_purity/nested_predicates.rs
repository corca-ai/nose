use super::*;

fn nested_predicate_il() -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let outer_receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(950), &[]);
    let outer_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(951),
        &[outer_receiver],
    );
    let outer_param = b.add(NodeKind::Param, Payload::Cid(2), sp(952), &[]);
    let captured_outer = b.add(NodeKind::Var, Payload::Cid(2), sp(953), &[]);
    let inner_source_param = b.add(NodeKind::Param, Payload::Cid(1), sp(954), &[]);
    let inner_source = b.add(NodeKind::Var, Payload::Cid(1), sp(955), &[]);
    let inner_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("filter")),
        sp(956),
        &[inner_source],
    );
    let inner_param = b.add(NodeKind::Param, Payload::Cid(3), sp(957), &[]);
    let inner_value = b.add(NodeKind::Var, Payload::Cid(3), sp(958), &[]);
    let comparison = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Gt),
        sp(959),
        &[inner_value, captured_outer],
    );
    let inner_return = b.add(NodeKind::Return, Payload::None, sp(960), &[comparison]);
    let inner_body = b.add(NodeKind::Block, Payload::None, sp(961), &[inner_return]);
    let inner_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(962),
        &[inner_param, inner_body],
    );
    let inner_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(963),
        &[inner_callee, inner_lambda],
    );
    let outer_return = b.add(NodeKind::Return, Payload::None, sp(964), &[inner_call]);
    let outer_body = b.add(NodeKind::Block, Payload::None, sp(965), &[outer_return]);
    let outer_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(966),
        &[outer_param, outer_body],
    );
    let outer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(967),
        &[outer_callee, outer_lambda],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(968),
        &[inner_source_param, outer_call],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    for (id, receiver) in [(0, outer_receiver), (1, inner_source)] {
        il.push_evidence(evidence(
            id,
            EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Array),
            EvidenceStatus::Asserted,
        ));
    }
    for (id, call, method, dependency) in [(2, inner_call, "filter", 1), (3, outer_call, "map", 0)]
    {
        let contract = library_method_call_contract(Lang::TypeScript, method, 1)
            .expect("TypeScript Array HOF contract");
        il.push_evidence(library_api_record_with_provenance_and_arity(
            id,
            il.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[dependency],
            contract.pack_id,
            contract.producer_id,
        ));
    }
    (il, interner, outer_call, inner_call)
}

#[test]
fn outer_transform_rechecks_nested_predicate_under_transform_effect_rules() {
    let (il, interner, outer_call, inner_call) = nested_predicate_il();
    assert!(
        admitted_library_method_call_at_call(&il, &interner, inner_call).is_some(),
        "the standalone predicate obligation preserves the existing untyped comparison perimeter"
    );
    assert!(
        admitted_library_method_call_at_call(&il, &interner, outer_call).is_none(),
        "an outer transform must not inherit a weaker nested predicate operator proof"
    );
}
