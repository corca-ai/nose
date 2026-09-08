use super::*;

fn call_target_record(
    id: u32,
    span: Span,
    lang: Lang,
    target: CallTargetEvidenceKind,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Call),
        kind: EvidenceKind::CallTarget(target),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies: dependencies.iter().copied().map(EvidenceId).collect(),
        status,
    }
}

fn imported_function_call_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let local = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("prod")),
        sp(10),
        &[],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(11), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(12), &[local, arg]);
    (finish_il(b, call, Lang::Python), call)
}

fn field_call_il(interner: &Interner, method: &str) -> (Il, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("ns")),
        sp(20),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(21),
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(4), sp(22), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, arg]);
    (finish_il(b, call, Lang::Python), call, callee)
}

fn scoped_var_call_il(interner: &Interner, path: &str) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(path)),
        sp(40),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(41), &[callee]);
    (finish_il(b, call, Lang::Rust), call)
}

fn promise_settled_record(
    id: u32,
    call: NodeId,
    payload: NodeId,
    il: &Il,
    channel: PromiseSettlementChannel,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
            channel,
            payload_span: il.node(payload).span,
            payload_kind: il.kind(payload),
        }),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

fn push_imported_function_target(il: &mut Il, interner: &Interner, call: NodeId, id: u32) {
    il.push_evidence(call_target_record(
        id,
        il.node(call).span,
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));
}

#[test]
fn imported_function_call_target_requires_matching_local_selector() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(call_target_record(
        0,
        sp(12),
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        })
    );
    assert!(imported_function_call_target_at_call(&il, &interner, call));
}

#[test]
fn promise_settled_value_contract_admits_only_with_imported_call_target() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    let payload = il.children(call)[1];
    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Missing
    );

    il.push_evidence(promise_settled_record(
        1,
        call,
        payload,
        &il,
        PromiseSettlementChannel::Fulfilled,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Rejected
    );

    push_imported_function_target(&mut il, &interner, call, 2);
    assert_eq!(
        promise_settled_value_evidence_at_call(&il, &interner, call),
        Some(PromiseSettledValueAtCall {
            channel: PromiseSettlementChannel::Fulfilled,
            payload,
        })
    );
}

#[test]
fn promise_settled_value_contract_rejects_direct_targets_and_bad_payload_anchors() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    let payload = il.children(call)[1];
    il.push_evidence(call_target_record(
        0,
        il.node(call).span,
        Lang::Python,
        CallTargetEvidenceKind::DirectFunction {
            target_span: sp(99),
            name_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));
    il.push_evidence(promise_settled_record(
        1,
        call,
        payload,
        &il,
        PromiseSettlementChannel::Fulfilled,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Rejected
    );

    let (mut il, call) = imported_function_call_il(&interner);
    push_imported_function_target(&mut il, &interner, call, 10);
    let payload = il.children(call)[1];
    il.push_evidence(evidence(
        11,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
            channel: PromiseSettlementChannel::Fulfilled,
            payload_span: il.node(payload).span,
            payload_kind: NodeKind::Var,
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Rejected
    );
}

#[test]
fn promise_settled_value_contract_rejects_broken_or_conflicting_evidence() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    let payload = il.children(call)[1];
    push_imported_function_target(&mut il, &interner, call, 0);
    il.push_evidence(promise_settled_record(
        1,
        call,
        payload,
        &il,
        PromiseSettlementChannel::Fulfilled,
        EvidenceStatus::Ambiguous,
        &[],
    ));
    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Rejected
    );

    let (mut il, call) = imported_function_call_il(&interner);
    let payload = il.children(call)[1];
    push_imported_function_target(&mut il, &interner, call, 10);
    il.push_evidence(promise_settled_record(
        11,
        call,
        payload,
        &il,
        PromiseSettlementChannel::Fulfilled,
        EvidenceStatus::Asserted,
        &[],
    ));
    il.push_evidence(promise_settled_record(
        12,
        call,
        payload,
        &il,
        PromiseSettlementChannel::Rejected,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        promise_settled_value_evidence_status_at_call(&il, &interner, call),
        PromiseSettledValueEvidenceStatus::Rejected
    );
}

#[test]
fn legacy_first_party_call_target_does_not_admit_identity() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(12), NodeKind::Call),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Missing
    );
}

#[test]
fn wrong_language_core_call_target_does_not_admit_identity() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(call_target_record(
        0,
        sp(12),
        Lang::TypeScript,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Missing
    );
}

#[test]
fn direct_function_span_helper_requires_selector_shape() {
    let test_support::DirectFunctionCallTargetFixture {
        mut il,
        interner,
        target,
        call,
    } = test_support::direct_function_call_target_test_il(
        test_support::DirectFunctionFixtureScope::TopLevel,
        test_support::DirectFunctionFixtureSelector::DifferentName,
    );
    il.push_evidence(call_target_record(
        0,
        il.node(call).span,
        Lang::Python,
        CallTargetEvidenceKind::DirectFunction {
            target_span: il.node(target).span,
            name_hash: stable_symbol_hash("f"),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        direct_function_call_target_span_at_call(&il, &interner, call),
        None
    );
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, target
    ));
}

#[test]
fn wrong_imported_function_selector_is_rejected_not_missing() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(call_target_record(
        0,
        sp(12),
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: stable_symbol_hash("sum"),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Rejected
    );
    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
}

#[test]
fn imported_member_call_target_admits_scoped_var_suffix_shape() {
    let interner = Interner::new();
    let (mut il, call) = scoped_var_call_il(&interner, "Span::new");
    let target = CallTargetEvidenceKind::ImportedMember {
        module_hash: stable_symbol_hash("nose_il"),
        exported_hash: stable_symbol_hash("Span"),
        member_hash: stable_symbol_hash("new"),
    };
    il.push_evidence(call_target_record(
        0,
        sp(41),
        Lang::Rust,
        target,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(target)
    );
    assert!(imported_member_call_target_at_call(&il, &interner, call));
}

#[test]
fn imported_member_call_target_rejects_wrong_scoped_var_suffix() {
    let interner = Interner::new();
    let (mut il, call) = scoped_var_call_il(&interner, "Span::new");
    il.push_evidence(call_target_record(
        0,
        sp(41),
        Lang::Rust,
        CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("nose_il"),
            exported_hash: stable_symbol_hash("Span"),
            member_hash: stable_symbol_hash("with_file"),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Rejected
    );
}

#[test]
fn imported_member_call_target_requires_full_nested_scoped_suffix() {
    let interner = Interner::new();
    let (mut il, call) = scoped_var_call_il(&interner, "json::value::from_value");
    let target = CallTargetEvidenceKind::ImportedMember {
        module_hash: stable_symbol_hash("serde_json"),
        exported_hash: stable_symbol_hash("value::from_value"),
        member_hash: stable_symbol_hash("value::from_value"),
    };
    il.push_evidence(call_target_record(
        0,
        sp(41),
        Lang::Rust,
        target,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(target)
    );
}

#[test]
fn dependency_broken_call_target_is_rejected() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(10), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
        }),
        EvidenceStatus::Ambiguous,
    ));
    il.push_evidence(call_target_record(
        1,
        sp(12),
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[0],
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Rejected
    );
}

#[test]
fn conflicting_call_targets_stay_closed() {
    let interner = Interner::new();
    let (mut il, call) = imported_function_call_il(&interner);
    il.push_evidence(call_target_record(
        0,
        sp(12),
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));
    il.push_evidence(call_target_record(
        1,
        sp(12),
        Lang::Python,
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("statistics"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(interner.intern("prod")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert_eq!(
        call_target_evidence_status_at_call(&il, &interner, call),
        CallTargetEvidenceStatus::Rejected
    );
}

#[test]
fn direct_method_target_requires_matching_selector_and_target_span() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let target_body = b.add(NodeKind::Block, Payload::None, sp(30), &[]);
    let target = b.add(NodeKind::Func, Payload::None, sp(31), &[target_body]);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("worker")),
        sp(32),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("run")),
        sp(33),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee]);
    let root = b.add(NodeKind::Module, Payload::None, sp(35), &[target, call]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.push_evidence(call_target_record(
        0,
        sp(34),
        Lang::TypeScript,
        CallTargetEvidenceKind::DirectMethod {
            target_span: il.node(target).span,
            receiver_type_hash: stable_symbol_hash("Worker"),
            method_hash: interner.symbol_hash(interner.intern("run")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert!(direct_method_call_target_at_call(
        &il, &interner, call, target
    ));
}

#[test]
fn dynamic_dispatch_is_evidence_but_not_imported_member_identity() {
    let interner = Interner::new();
    let (mut il, call, _) = field_call_il(&interner, "next");
    il.push_evidence(call_target_record(
        0,
        sp(23),
        Lang::Python,
        CallTargetEvidenceKind::DynamicDispatch {
            protocol_hash: stable_symbol_hash("Iterator"),
            method_hash: interner.symbol_hash(interner.intern("next")),
        },
        EvidenceStatus::Asserted,
        &[],
    ));

    assert!(matches!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(CallTargetEvidenceKind::DynamicDispatch { .. })
    ));
    assert!(!imported_member_call_target_at_call(&il, &interner, call));
}
