use super::support::*;

#[test]
fn membership_call_consumes_receiver_domain_evidence() {
    let (mut il, interner, call, receiver_span) = receiver_domain_contains_call_il();
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "method selector alone must not prove collection membership"
    );

    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    push_method_call_library_api_evidence(&mut il, &interner, 1, call, "includes", 1);
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Bin(op) if op == Op::In as u32
    ));

    il.push_evidence(evidence(
        2,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
    ));
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "conflicting receiver-domain evidence must close the exact membership rewrite"
    );
}

#[test]
fn membership_call_consumes_library_api_result_domain_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let factory_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let seed = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(41),
        &[],
    );
    let receiver = b.add(
        NodeKind::Call,
        Payload::None,
        sp(42),
        &[factory_callee, seed],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(43),
        &[receiver],
    );
    let item = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("item")),
        sp(44),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(45), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "call-result receiver must not be collection-like without domain evidence"
    );

    let api = library_js_like_set_constructor_contract(Lang::TypeScript, "Set").unwrap();
    il.push_evidence(js_like_builtin_collection_constructor_evidence(
        0,
        sp(42),
        api.id,
        api.callee,
        1,
        Vec::new(),
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(42), NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Set),
        vec![EvidenceId(0)],
    ));
    push_method_call_library_api_evidence(&mut il, &interner, 2, call, "includes", 1);
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Bin(op) if op == Op::In as u32
    ));

    il.evidence[0].status = EvidenceStatus::Ambiguous;
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "ambiguous LibraryApi dependency must close the call-result receiver proof"
    );
}

fn node_with_span(il: &Il, kind: NodeKind, span: Span) -> NodeId {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == kind && node.span == span).then_some(NodeId(idx as u32))
        })
        .expect("node with requested span")
}

#[derive(Clone, Copy)]
enum BindingMembershipCase {
    Visible,
    Late,
    MutatedVisible,
}

fn binding_assignment(b: &mut IlBuilder, xs: Symbol, array: Symbol, line: u32) -> (NodeId, Span) {
    let lhs = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let seq_span = sp(line + 1);
    let seq = b.add(NodeKind::Seq, Payload::Name(array), seq_span, &[]);
    (
        b.add(NodeKind::Assign, Payload::None, sp(line), &[lhs, seq]),
        seq_span,
    )
}

fn binding_membership_call(
    b: &mut IlBuilder,
    xs: Symbol,
    item_name: Symbol,
    includes: Symbol,
    line: u32,
) -> (NodeId, Span) {
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(includes),
        sp(line + 1),
        &[receiver],
    );
    let item = b.add(NodeKind::Var, Payload::Name(item_name), sp(line + 2), &[]);
    let call_span = sp(line + 3);
    (
        b.add(NodeKind::Call, Payload::None, call_span, &[callee, item]),
        call_span,
    )
}

fn binding_append(b: &mut IlBuilder, xs: Symbol, line: u32) -> NodeId {
    let append_receiver = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let appended = b.add(NodeKind::Lit, Payload::LitInt(1), sp(line), &[]);
    b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        sp(line),
        &[append_receiver, appended],
    )
}

fn normalized_binding_membership_op(case: BindingMembershipCase) -> ValOp {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let item_name = interner.intern("item");
    let includes = interner.intern("includes");
    let array = interner.intern("array");
    let mut b = IlBuilder::new(FileId(0));
    let ((root_children, seq_span), call_span, mutation_span) = match case {
        BindingMembershipCase::Visible => {
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 10);
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 12);
            ((vec![assign, call], seq_span), call_span, None)
        }
        BindingMembershipCase::Late => {
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 12);
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 20);
            ((vec![call, assign], seq_span), call_span, None)
        }
        BindingMembershipCase::MutatedVisible => {
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 20);
            let append = binding_append(&mut b, xs, 22);
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 23);
            (
                (vec![assign, append, call], seq_span),
                call_span,
                Some(sp(22)),
            )
        }
    };
    let body = b.add(NodeKind::Block, Payload::None, sp(9), &root_children);
    let root = b.add(NodeKind::Func, Payload::None, sp(8), &[body]);
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    il.push_evidence(collection_sequence_evidence(0, Lang::TypeScript, seq_span));
    if let Some(span) = mutation_span {
        il.push_evidence(evidence(
            1,
            EvidenceAnchor::node(span, NodeKind::Call),
            EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
        ));
    }
    let normalized = crate::normalize(
        &il,
        &interner,
        &crate::NormalizeOptions {
            cfg_norm: false,
            dataflow: false,
            dce: false,
            oracle: false,
        },
    );
    let normalized_call = node_with_span(&normalized, NodeKind::Call, call_span);
    eval_op(&normalized, &interner, normalized_call)
}

#[test]
fn membership_call_consumes_normalized_binding_domain_evidence() {
    assert!(matches!(
        normalized_binding_membership_op(BindingMembershipCase::Visible),
        ValOp::Bin(op) if op == Op::In as u32
    ));
}

#[test]
fn membership_call_rejects_binding_domain_after_receiver_use() {
    assert!(
        !matches!(
            normalized_binding_membership_op(BindingMembershipCase::Late),
            ValOp::Bin(op) if op == Op::In as u32
        ),
        "binding-domain evidence must not prove use-before-assignment receivers"
    );
}

#[test]
fn mutated_binding_domain_evidence_keeps_membership_rewrite_closed() {
    assert!(
        !matches!(
            normalized_binding_membership_op(BindingMembershipCase::MutatedVisible),
            ValOp::Bin(op) if op == Op::In as u32
        ),
        "mutated binding must not receive binding-domain evidence"
    );
}
