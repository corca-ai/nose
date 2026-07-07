use super::*;

fn bound_order_fixture(op: Op) -> (Il, NodeId, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let lower = b.add(NodeKind::Var, Payload::Cid(1), sp(11), &[]);
    let upper = b.add(NodeKind::Var, Payload::Cid(2), sp(12), &[]);
    let cond = b.add(NodeKind::BinOp, Payload::Op(op), sp(13), &[lower, upper]);
    let root = b.add(NodeKind::Block, Payload::None, sp(13), &[cond]);
    (finish_il(b, root, Lang::Java), cond, lower, upper)
}

fn push_bound_order_evidence(
    il: &mut Il,
    id: u32,
    cond: NodeId,
    operands: (NodeId, NodeId),
    activation: BoundOrderGuardActivation,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) {
    let (lower, upper) = operands;
    il.evidence.push(language_core_evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(cond).span, NodeKind::BinOp),
        EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
            lower_span: il.node(lower).span,
            upper_span: il.node(upper).span,
            activation,
        }),
        status,
        dependencies,
        il.meta.lang,
    ));
}

#[test]
fn bound_order_guard_evidence_resolves_exact_activation_and_operands() {
    let (mut il, cond, lower, upper) = bound_order_fixture(Op::Le);
    push_bound_order_evidence(
        &mut il,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        Vec::new(),
    );

    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenTrue),
        Some((lower, upper))
    );
    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenFalse),
        None
    );
}

#[test]
fn bound_order_guard_false_branch_reverses_inverse_comparison() {
    let (mut il, cond, upper, lower) = bound_order_fixture(Op::Lt);
    push_bound_order_evidence(
        &mut il,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenFalse,
        EvidenceStatus::Asserted,
        Vec::new(),
    );

    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenFalse),
        Some((lower, upper))
    );
}

#[test]
fn bound_order_guard_evidence_allows_distinct_branch_facts() {
    let (mut il, cond, lower, upper) = bound_order_fixture(Op::Lt);
    push_bound_order_evidence(
        &mut il,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        Vec::new(),
    );
    push_bound_order_evidence(
        &mut il,
        1,
        cond,
        (upper, lower),
        BoundOrderGuardActivation::WhenFalse,
        EvidenceStatus::Asserted,
        Vec::new(),
    );

    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenTrue),
        Some((lower, upper))
    );
    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenFalse),
        Some((upper, lower))
    );
}

#[test]
fn bound_order_guard_evidence_fails_closed_on_conflict_or_dead_dependency() {
    let (mut ambiguous, cond, lower, upper) = bound_order_fixture(Op::Le);
    push_bound_order_evidence(
        &mut ambiguous,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        Vec::new(),
    );
    push_bound_order_evidence(
        &mut ambiguous,
        1,
        cond,
        (upper, lower),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        Vec::new(),
    );
    assert_eq!(
        bound_order_guard_for_node(&ambiguous, cond, BoundOrderGuardActivation::WhenTrue),
        None
    );

    let (mut dead_dep, cond, lower, upper) = bound_order_fixture(Op::Le);
    push_bound_order_evidence(
        &mut dead_dep,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    );
    assert_eq!(
        bound_order_guard_for_node(&dead_dep, cond, BoundOrderGuardActivation::WhenTrue),
        None
    );
}

#[test]
fn bound_order_guard_evidence_requires_language_core_provenance() {
    let (mut il, cond, lower, upper) = bound_order_fixture(Op::Le);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(cond).span, NodeKind::BinOp),
        EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
            lower_span: il.node(lower).span,
            upper_span: il.node(upper).span,
            activation: BoundOrderGuardActivation::WhenTrue,
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenTrue),
        None
    );
}

#[test]
fn bound_order_guard_evidence_rejects_float_literals() {
    let mut b = IlBuilder::new(FileId(0));
    let lower = b.add(
        NodeKind::Lit,
        Payload::LitFloat(0x3ff0_0000_0000_0000),
        sp(11),
        &[],
    );
    let upper = b.add(NodeKind::Lit, Payload::LitInt(10), sp(12), &[]);
    let cond = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Le),
        sp(13),
        &[lower, upper],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(13), &[cond]);
    let mut il = finish_il(b, root, Lang::Java);
    push_bound_order_evidence(
        &mut il,
        0,
        cond,
        (lower, upper),
        BoundOrderGuardActivation::WhenTrue,
        EvidenceStatus::Asserted,
        Vec::new(),
    );

    assert_eq!(
        bound_order_guard_for_node(&il, cond, BoundOrderGuardActivation::WhenTrue),
        None
    );
}
