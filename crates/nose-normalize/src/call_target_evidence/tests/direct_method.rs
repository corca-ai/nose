use super::support::*;

#[test]
fn emits_promise_like_domain_for_direct_method_returning_promise_like() {
    let DirectMethodReturnFixture {
        interner,
        mut il,
        target,
        call,
        return_expr,
        method,
    } = direct_method_return_call_fixture();
    let target_evidence_id = EvidenceId(400);
    let return_domain_id = EvidenceId(500);
    let target_evidence =
        direct_method_target_record(&il, &interner, target_evidence_id, call, target, method);
    let return_domain = promise_like_domain_record(&il, return_domain_id, return_expr);
    il.push_evidence(target_evidence);
    il.push_evidence(return_domain);

    run(&mut il, &interner);

    let domain = promise_like_domain_at_call(&il, call)
        .expect("PromiseLike domain evidence for direct method call result");
    assert_eq!(
        domain.dependencies,
        vec![target_evidence_id, return_domain_id]
    );
}

#[test]
fn emits_promise_like_domain_for_branching_direct_method_returns() {
    let BranchingDirectMethodReturnFixture {
        interner,
        mut il,
        target,
        call,
        then_expr,
        else_expr,
        method,
    } = branching_direct_method_return_call_fixture();
    let target_evidence_id = EvidenceId(400);
    let then_domain_id = EvidenceId(500);
    let else_domain_id = EvidenceId(501);
    let target_evidence =
        direct_method_target_record(&il, &interner, target_evidence_id, call, target, method);
    il.push_evidence(target_evidence);
    for (id, expr) in [(then_domain_id, then_expr), (else_domain_id, else_expr)] {
        let record = promise_like_domain_record(&il, id, expr);
        il.push_evidence(record);
    }

    run(&mut il, &interner);

    let domain = promise_like_domain_at_call(&il, call)
        .expect("PromiseLike domain evidence for branching direct method call result");
    assert_eq!(
        domain.dependencies,
        vec![target_evidence_id, then_domain_id, else_domain_id]
    );
}

#[test]
fn direct_method_return_domain_requires_return_expression_domain_proof() {
    let DirectMethodReturnFixture {
        interner,
        mut il,
        target,
        call,
        method,
        ..
    } = direct_method_return_call_fixture();
    let target_evidence =
        direct_method_target_record(&il, &interner, EvidenceId(400), call, target, method);
    il.push_evidence(target_evidence);

    run(&mut il, &interner);

    assert!(
        il.evidence
            .iter()
            .find(|record| {
                record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                    && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
            })
            .is_none(),
        "direct method result domain requires returned expression domain evidence"
    );
}

struct DirectMethodReturnFixture {
    interner: Interner,
    il: Il,
    target: NodeId,
    call: NodeId,
    return_expr: NodeId,
    method: Symbol,
}

struct BranchingDirectMethodReturnFixture {
    interner: Interner,
    il: Il,
    target: NodeId,
    call: NodeId,
    then_expr: NodeId,
    else_expr: NodeId,
    method: Symbol,
}

fn branching_direct_method_return_call_fixture() -> BranchingDirectMethodReturnFixture {
    let interner = Interner::new();
    let method = interner.intern("load");
    let mut b = IlBuilder::new(FileId(0));
    let cond = b.add(NodeKind::Var, Payload::Cid(0), sp(20), &[]);
    let then_expr = promise_call_expr(&mut b, &interner, 21);
    let then_ret = b.add(NodeKind::Return, Payload::None, sp(23), &[then_expr]);
    let then_block = b.add(NodeKind::Block, Payload::None, sp(24), &[then_ret]);
    let else_expr = promise_call_expr(&mut b, &interner, 25);
    let else_ret = b.add(NodeKind::Return, Payload::None, sp(27), &[else_expr]);
    let else_block = b.add(NodeKind::Block, Payload::None, sp(28), &[else_ret]);
    let branch = b.add(
        NodeKind::If,
        Payload::None,
        sp(29),
        &[cond, then_block, else_block],
    );
    let body = b.add(NodeKind::Block, Payload::None, sp(30), &[branch]);
    let target = b.add(NodeKind::Func, Payload::None, sp(31), &[body]);
    let call_arg = b.add(NodeKind::Lit, Payload::LitBool(true), sp(34), &[]);
    let (il, call) = finish_ts_method_call_fixture(b, &interner, target, method, 32, &[call_arg]);
    BranchingDirectMethodReturnFixture {
        interner,
        il,
        target,
        call,
        then_expr,
        else_expr,
        method,
    }
}

fn direct_method_return_call_fixture() -> DirectMethodReturnFixture {
    let interner = Interner::new();
    let method = interner.intern("load");
    let mut b = IlBuilder::new(FileId(0));
    let (target, return_expr) = promise_returning_method(&mut b, &interner);
    let (il, call) = finish_ts_method_call_fixture(b, &interner, target, method, 10, &[]);
    DirectMethodReturnFixture {
        interner,
        il,
        target,
        call,
        return_expr,
        method,
    }
}

fn promise_returning_method(b: &mut IlBuilder, interner: &Interner) -> (NodeId, NodeId) {
    let promise_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("producer")),
        sp(1),
        &[],
    );
    let return_expr = b.add(NodeKind::Call, Payload::None, sp(2), &[promise_callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(3), &[return_expr]);
    (
        b.add(NodeKind::Func, Payload::None, sp(4), &[ret]),
        return_expr,
    )
}

fn finish_ts_method_call_fixture(
    mut builder: IlBuilder,
    interner: &Interner,
    target: NodeId,
    method: Symbol,
    base_line: u32,
    args: &[NodeId],
) -> (Il, NodeId) {
    let worker = interner.intern("worker");
    let receiver = builder.add(NodeKind::Var, Payload::Name(worker), sp(base_line), &[]);
    let callee = builder.add(
        NodeKind::Field,
        Payload::Name(method),
        sp(base_line + 1),
        &[receiver],
    );
    let mut call_children = vec![callee];
    call_children.extend_from_slice(args);
    let call = builder.add(
        NodeKind::Call,
        Payload::None,
        sp(base_line + 2),
        &call_children,
    );
    let module = builder.add(
        NodeKind::Module,
        Payload::None,
        sp(base_line + 3),
        &[target, call],
    );
    (
        finish_ts_method_fixture(builder, module, target, method),
        call,
    )
}

fn finish_ts_method_fixture(
    builder: IlBuilder,
    root: NodeId,
    method_root: NodeId,
    method: Symbol,
) -> Il {
    builder.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::TypeScript,
        },
        vec![Unit {
            root: method_root,
            kind: UnitKind::Method,
            name: Some(method),
            origin: Default::default(),
        }],
        Vec::new(),
    )
}
