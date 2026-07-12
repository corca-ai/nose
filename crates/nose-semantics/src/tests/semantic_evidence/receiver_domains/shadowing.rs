use super::*;

#[test]
fn named_receiver_domain_stops_at_intervening_destructured_shadow() {
    let interner = Interner::new();
    let x = interner.intern("x");
    let mut b = IlBuilder::new(FileId(0));
    let outer_param = b.add(NodeKind::Param, Payload::Name(x), span(5, 6, 1), &[]);

    let shadow_name = b.add(NodeKind::Var, Payload::Name(x), span(30, 31, 3), &[]);
    let shadow_lhs = b.add(
        NodeKind::Seq,
        Payload::None,
        span(29, 32, 3),
        &[shadow_name],
    );
    let shadow_value = b.add(NodeKind::Lit, Payload::LitInt(1), span(34, 35, 3), &[]);
    let shadow = b.add(
        NodeKind::Assign,
        Payload::None,
        span(30, 35, 3),
        &[shadow_lhs, shadow_value],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(x), span(60, 61, 5), &[]);
    let callback_body = b.add(NodeKind::Block, Payload::None, span(55, 65, 5), &[receiver]);
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        span(50, 70, 4),
        &[callback_body],
    );
    let middle_body = b.add(
        NodeKind::Block,
        Payload::None,
        span(25, 75, 3),
        &[shadow, callback],
    );
    let middle = b.add(
        NodeKind::Func,
        Payload::None,
        span(20, 80, 2),
        &[middle_body],
    );
    let outer_body = b.add(NodeKind::Block, Payload::None, span(10, 90, 2), &[middle]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 100, 1),
        &[outer_param, outer_body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(5, 6, 1)),
        EvidenceKind::Domain(DomainEvidence::Number),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "the middle destructured x must shadow the outer parameter before the callback read"
    );
    assert!(
        !effect_closed_local_var_reference(&il, receiver),
        "pre-alpha callback purity must not borrow proof from the shadowed outer parameter"
    );
}

#[test]
fn cid_receiver_domain_stops_at_fresh_function_namespace() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let outer_param = b.add(NodeKind::Param, Payload::Cid(0), span(5, 6, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 61, 5), &[]);
    let callback_body = b.add(NodeKind::Block, Payload::None, span(55, 65, 5), &[receiver]);
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        span(50, 70, 4),
        &[callback_body],
    );
    let middle_body = b.add(NodeKind::Block, Payload::None, span(25, 75, 3), &[callback]);
    let middle = b.add(
        NodeKind::Func,
        Payload::None,
        span(20, 80, 2),
        &[middle_body],
    );
    let outer_body = b.add(NodeKind::Block, Payload::None, span(10, 90, 2), &[middle]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 100, 1),
        &[outer_param, outer_body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(5, 6, 1)),
        EvidenceKind::Domain(DomainEvidence::Number),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
    assert!(
        !effect_closed_local_var_reference(&il, receiver),
        "a cid from a fresh Func namespace must not borrow an outer Func parameter proof"
    );
}

#[test]
fn cid_receiver_domain_rejects_nested_param_shadow_in_same_function() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let function_param = b.add(NodeKind::Param, Payload::Cid(0), span(5, 6, 1), &[]);
    let nested_param = b.add(NodeKind::Param, Payload::Cid(0), span(30, 31, 3), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 61, 5), &[]);
    let callback_body = b.add(NodeKind::Block, Payload::None, span(55, 65, 5), &[receiver]);
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        span(50, 70, 4),
        &[callback_body],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(10, 90, 2),
        &[nested_param, callback],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 100, 1),
        &[function_param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(5, 6, 1)),
        EvidenceKind::Domain(DomainEvidence::Number),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
    assert!(
        !effect_closed_local_var_reference(&il, receiver),
        "a nested Param binder with the same cid must block the function parameter proof"
    );
}
