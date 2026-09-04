use super::*;

fn js_record_guard_il(interner: &Interner, subject: &str) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("record_guard");
    let subject = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(subject)),
        sp(12),
        &[],
    );
    let object = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("object")),
        sp(12),
        &[],
    );
    let non_null = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("non_null")),
        sp(12),
        &[],
    );
    let not_array = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("not_array")),
        sp(12),
        &[],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(12),
        &[subject, object, non_null, not_array],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(12), &[guard]);
    (finish_il(b, root, Lang::JavaScript), guard)
}

fn record_guard_evidence_with_null_check(
    subject: &str,
    null_check: JsRecordGuardNullCheck,
) -> EvidenceKind {
    EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
        subject_hash: stable_symbol_hash(subject),
        null_check,
        comparison: JsRecordGuardComparison::StrictOnly,
    })
}

fn array_is_array_dependency(
    id: u32,
    span: Span,
    status: EvidenceStatus,
    root_dependency: Option<u32>,
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        status,
        root_dependency
            .into_iter()
            .map(EvidenceId)
            .collect::<Vec<_>>(),
    )
}

fn boolean_dependency(id: u32, span: Span, status: EvidenceStatus) -> EvidenceRecord {
    unshadowed_global_source_dependency(id, span, "Boolean", status)
}

fn record_guard_record(
    id: u32,
    span: Span,
    subject: &str,
    null_check: JsRecordGuardNullCheck,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::sequence(span),
        record_guard_evidence_with_null_check(subject, null_check),
        EvidenceStatus::Asserted,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

fn record_guard_surface_evidence(id: u32, span: Span) -> EvidenceRecord {
    language_core_evidence(
        id,
        EvidenceAnchor::sequence(span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    )
}

fn js_record_guard_il_with_surface(interner: &Interner, subject: &str) -> (Il, NodeId) {
    let (mut il, guard) = js_record_guard_il(interner, subject);
    il.push_evidence(record_guard_surface_evidence(0, sp(12)));
    (il, guard)
}

#[test]
fn record_shape_guard_requires_dedicated_guard_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_record_guard_il(&interner, "value");

    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.push_evidence(record_guard_surface_evidence(0, sp(12)));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_validates_required_dependencies() {
    let interner = Interner::new();

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(record_guard_record(
        1,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(evidence(
        1,
        EvidenceAnchor::source_span(sp(12)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_rejects_ambiguous_or_mispositioned_dependencies() {
    let interner = Interner::new();

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Ambiguous,
        None,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(array_is_array_dependency(
        1,
        sp(14),
        EvidenceStatus::Asserted,
        None,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_boolean_truthy_null_check_requires_full_proofs() {
    let interner = Interner::new();

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        None,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::BooleanGlobalTruthy,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.push_evidence(boolean_dependency(3, sp(12), EvidenceStatus::Asserted));
    il.push_evidence(record_guard_record(
        4,
        sp(12),
        "value",
        JsRecordGuardNullCheck::BooleanGlobalTruthy,
        &[1, 3],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(4),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        4,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(boolean_dependency(2, sp(12), EvidenceStatus::Asserted));
    il.push_evidence(record_guard_record(
        3,
        sp(12),
        "value",
        JsRecordGuardNullCheck::BooleanGlobalTruthy,
        &[1, 2],
    ));
    assert!(record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_rejects_mismatched_or_ambiguous_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_record_guard_il(&interner, "value");
    il.push_evidence(record_guard_surface_evidence(0, sp(12)));
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "other",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il(&interner, "value");
    il.push_evidence(record_guard_surface_evidence(0, sp(12)));
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(evidence_with_dependencies(
        2,
        EvidenceAnchor::sequence(sp(12)),
        record_guard_evidence_with_null_check("value", JsRecordGuardNullCheck::StrictNonNull),
        EvidenceStatus::Ambiguous,
        vec![EvidenceId(1)],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il(&interner, "value");
    il.push_evidence(record_guard_surface_evidence(0, sp(12)));
    il.push_evidence(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(4),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        4,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    il.push_evidence(record_guard_record(
        3,
        sp(12),
        "candidate",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_keeps_source_subject_proof_after_alpha_rename() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("record_guard");
    let subject = b.add(NodeKind::Var, Payload::Cid(0), sp(13), &[]);
    let object = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("object")),
        sp(13),
        &[],
    );
    let non_null = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("non_null")),
        sp(13),
        &[],
    );
    let not_array = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("not_array")),
        sp(13),
        &[],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(13),
        &[subject, object, non_null, not_array],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(13), &[guard]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.push_evidence(record_guard_surface_evidence(0, sp(13)));
    il.push_evidence(array_is_array_dependency(
        1,
        sp(13),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        3,
        sp(13),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(record_guard_record(
        2,
        sp(13),
        "source_name",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));

    assert!(record_shape_guard_for_node(&il, &interner, guard));
}
