use super::*;

#[test]
fn method_receiver_contracts_expose_only_domain_backed_obligations() {
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactArray),
        Some(DomainRequirement::ARRAY)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollection),
        Some(DomainRequirement::ARRAY_COLLECTION_OR_SET)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactArrayOrCollection),
        Some(DomainRequirement::ARRAY_OR_COLLECTION)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactProtocol),
        Some(DomainRequirement::ARRAY_COLLECTION_OR_SET)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollectionOrMap),
        Some(DomainRequirement::COLLECTION_OR_MAP)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactSetOrMap),
        Some(DomainRequirement::SET_OR_MAP)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::RustMapGetOrExactOption),
        Some(DomainRequirement::OPTION)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactMapLiteral),
        None
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ImportedNamespace("math")),
        None
    );
}

#[test]
fn domain_evidence_records_drive_param_domain_proof() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(3), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(3), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn ambiguous_domain_evidence_stays_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(4), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(4), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_param(&il, param), None);
}

#[test]
fn receiver_domain_evidence_at_node_is_preferred_over_param_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(20, 22, 2),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Set)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
    assert!(receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::MAP
    ));
    assert!(!receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::SET
    ));
}

fn cid_param_receiver_fixture() -> (Il, Interner, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    (
        finish_il(b, root, Lang::TypeScript),
        Interner::new(),
        receiver,
    )
}

fn push_cid_param_domain(il: &mut Il, id: u32, domain: DomainEvidence) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
}

fn push_cid_receiver_domain(il: &mut Il, id: u32, domain: DomainEvidence, status: EvidenceStatus) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(domain),
        status,
    ));
}

#[test]
fn ambiguous_receiver_domain_evidence_blocks_param_fallback() {
    let (mut il, interner, receiver) = cid_param_receiver_fixture();
    push_cid_param_domain(&mut il, 0, DomainEvidence::Map);
    push_cid_receiver_domain(&mut il, 1, DomainEvidence::Set, EvidenceStatus::Asserted);
    push_cid_receiver_domain(&mut il, 2, DomainEvidence::Map, EvidenceStatus::Asserted);

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

fn binding_receiver_fixture(interner: &Interner, module_receiver: bool) -> (Il, NodeId, NodeId) {
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(15, 17, 1), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 17, 1),
        &[lhs, rhs],
    );
    let receiver_payload = if module_receiver {
        Payload::Name(xs)
    } else {
        Payload::Cid(0)
    };
    let receiver = b.add(NodeKind::Var, receiver_payload, span(40, 42, 3), &[]);
    let root = if module_receiver {
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(40, 42, 3),
            &[receiver],
        );
        let body = b.add(NodeKind::Block, Payload::None, span(38, 45, 3), &[stmt]);
        let func = b.add(NodeKind::Func, Payload::None, span(30, 50, 2), &[body]);
        b.add(
            NodeKind::Module,
            Payload::None,
            span(0, 60, 1),
            &[assign, func],
        )
    } else {
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(10, 44, 1),
            &[assign, receiver],
        );
        b.add(NodeKind::Func, Payload::None, span(0, 50, 1), &[body])
    };
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    (il, lhs, receiver)
}

#[test]
fn binding_domain_evidence_drives_receiver_domain_proof() {
    let interner = Interner::new();
    let (mut il, lhs, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "conflicting binding-domain evidence must close receiver proof"
    );
}

#[test]
fn binding_domain_evidence_validates_dependencies() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(span(15, 17, 1)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Ambiguous,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "dependency-broken binding-domain evidence must fail closed"
    );
}

#[test]
fn module_binding_domain_evidence_reaches_free_name_receiver() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, true);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );
}

#[test]
fn binding_domain_evidence_requires_matching_local_hash() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let ys = interner.intern("ys");
    let mut b = IlBuilder::new(FileId(0));
    let xs_lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let xs_rhs = b.add(NodeKind::Seq, Payload::None, span(14, 15, 1), &[]);
    let xs_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 15, 1),
        &[xs_lhs, xs_rhs],
    );
    let ys_lhs = b.add(NodeKind::Var, Payload::Cid(1), span(10, 12, 1), &[]);
    let ys_rhs = b.add(NodeKind::Seq, Payload::None, span(18, 19, 1), &[]);
    let ys_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(16, 19, 1),
        &[ys_lhs, ys_rhs],
    );
    let ys_receiver = b.add(NodeKind::Var, Payload::Cid(1), span(30, 32, 2), &[]);
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 34, 1),
        &[xs_assign, ys_assign, ys_receiver],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 40, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs, ys];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, xs_lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, ys_lhs),
        None,
        "same-span binding evidence must not cross local_hash boundaries"
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, ys_receiver),
        None
    );
}

#[test]
fn binding_domain_evidence_requires_assignment_before_receiver() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(24, 25, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(20, 25, 2),
        &[lhs, rhs],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 28, 1),
        &[receiver, assign],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 30, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(20, 22, 2), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "binding-domain evidence must not prove use-before-assignment receivers"
    );
}

#[test]
fn cid_receiver_domain_uses_nearest_function_scope() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let first_param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let first_body = b.add(NodeKind::Block, Payload::None, span(14, 20, 1), &[]);
    let first_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[first_param, first_body],
    );
    let second_param = b.add(NodeKind::Param, Payload::Cid(0), span(50, 52, 3), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 62, 4), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(60, 62, 4),
        &[receiver],
    );
    let second_body = b.add(NodeKind::Block, Payload::None, span(58, 66, 4), &[stmt]);
    let second_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(40, 80, 3),
        &[second_param, second_body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        span(0, 90, 1),
        &[first_func, second_func],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(span(50, 52, 3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn dependency_broken_receiver_domain_evidence_blocks_param_fallback() {
    let (mut il, interner, receiver) = cid_param_receiver_fixture();
    push_cid_param_domain(&mut il, 0, DomainEvidence::Set);
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

#[test]
fn receiver_domain_index_uses_kernel_fail_closed_policy() {
    let (mut il, interner, receiver) = cid_param_receiver_fixture();
    push_cid_param_domain(&mut il, 0, DomainEvidence::Collection);
    push_cid_receiver_domain(&mut il, 1, DomainEvidence::Map, EvidenceStatus::Ambiguous);

    let domains = ReceiverDomainEvidenceIndex::new(&il, &interner);
    assert_eq!(domains.domain_evidence_for_receiver(receiver), None);
    assert!(!domains.receiver_satisfies_domain(receiver, DomainRequirement::COLLECTION));
}

#[test]
fn named_receiver_domain_requires_unassigned_param_scope() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(20, 50, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Name(xs), span(24, 26, 2), &[]);
    let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), span(29, 30, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(24, 30, 2),
        &[lhs, rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(20, 50, 2),
        &[assign, stmt],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}
