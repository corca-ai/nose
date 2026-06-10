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

fn unshadowed_global_source_dependency(
    id: u32,
    span: Span,
    name: &str,
    status: EvidenceStatus,
) -> EvidenceRecord {
    evidence(
        id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        }),
        status,
    )
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

fn js_record_guard_il_with_surface(interner: &Interner, subject: &str) -> (Il, NodeId) {
    let (mut il, guard) = js_record_guard_il(interner, subject);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(12)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    (il, guard)
}

fn js_own_property_guard_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("own_property_guard");
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(22),
        &[],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(22),
        &[],
    );
    let own = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("own")),
        sp(22),
        &[],
    );
    let present = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("present")),
        sp(22),
        &[],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(22),
        &[receiver, key, own, present],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(22), &[guard]);
    (finish_il(b, root, Lang::JavaScript), guard)
}

fn qualified_global_dependency(
    id: u32,
    span: Span,
    path: &str,
    status: EvidenceStatus,
    root_dependency: Option<u32>,
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash(path),
        }),
        status,
        root_dependency
            .into_iter()
            .map(EvidenceId)
            .collect::<Vec<_>>(),
    )
}

fn own_property_guard_record(
    id: u32,
    span: Span,
    path: &str,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash(path),
        }),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

#[test]
fn own_property_guard_requires_dedicated_guard_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_own_property_guard_il(&interner);

    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(22)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.evidence.push(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        None,
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.evidence.push(qualified_global_dependency(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        3,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(own_property_guard_record(
        4,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[2],
    ));
    assert!(own_property_guard_for_node(&il, &interner, guard));
    assert!(own_property_guard_evidence_at_span(&il, sp(22)));
}

#[test]
fn own_property_guard_validates_api_dependencies() {
    let interner = Interner::new();
    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(22)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(qualified_global_dependency(
        1,
        sp(22),
        "Array.from",
        EvidenceStatus::Asserted,
        None,
    ));
    il.evidence.push(own_property_guard_record(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(22)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Ambiguous,
        None,
    ));
    il.evidence.push(own_property_guard_record(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(22)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(qualified_global_dependency(
        1,
        sp(22),
        "value.hasOwnProperty",
        EvidenceStatus::Asserted,
        None,
    ));
    il.evidence.push(own_property_guard_record(
        2,
        sp(22),
        "value.hasOwnProperty",
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));
}

#[test]
fn own_property_guard_rejects_ambiguous_guard_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(22)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        Some(5),
    ));
    il.evidence.push(qualified_global_dependency(
        2,
        sp(22),
        "Object.prototype.hasOwnProperty.call",
        EvidenceStatus::Asserted,
        Some(6),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        5,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        6,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(own_property_guard_record(
        3,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    il.evidence.push(own_property_guard_record(
        4,
        sp(22),
        "Object.prototype.hasOwnProperty.call",
        EvidenceStatus::Asserted,
        &[2],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));
}

#[test]
fn record_shape_guard_requires_dedicated_guard_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_record_guard_il(&interner, "value");

    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(12)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(record_guard_record(
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
    il.evidence.push(record_guard_record(
        1,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(12)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(record_guard_record(
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
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Ambiguous,
        None,
    ));
    il.evidence.push(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.evidence.push(array_is_array_dependency(
        1,
        sp(14),
        EvidenceStatus::Asserted,
        None,
    ));
    il.evidence.push(record_guard_record(
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
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        None,
    ));
    il.evidence.push(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::BooleanGlobalTruthy,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    il.evidence
        .push(boolean_dependency(3, sp(12), EvidenceStatus::Asserted));
    il.evidence.push(record_guard_record(
        4,
        sp(12),
        "value",
        JsRecordGuardNullCheck::BooleanGlobalTruthy,
        &[1, 3],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il_with_surface(&interner, "value");
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(4),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        4,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence
        .push(boolean_dependency(2, sp(12), EvidenceStatus::Asserted));
    il.evidence.push(record_guard_record(
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
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(12)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(record_guard_record(
        2,
        sp(12),
        "other",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il(&interner, "value");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(12)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        3,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::sequence(sp(12)),
        record_guard_evidence_with_null_check("value", JsRecordGuardNullCheck::StrictNonNull),
        EvidenceStatus::Ambiguous,
        vec![EvidenceId(1)],
    ));
    assert!(!record_shape_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_record_guard_il(&interner, "value");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(12)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(array_is_array_dependency(
        1,
        sp(12),
        EvidenceStatus::Asserted,
        Some(4),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        4,
        sp(12),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(record_guard_record(
        2,
        sp(12),
        "value",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));
    il.evidence.push(record_guard_record(
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
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(13)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(array_is_array_dependency(
        1,
        sp(13),
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.evidence.push(unshadowed_global_source_dependency(
        3,
        sp(13),
        "Array",
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(record_guard_record(
        2,
        sp(13),
        "source_name",
        JsRecordGuardNullCheck::StrictNonNull,
        &[1],
    ));

    assert!(record_shape_guard_for_node(&il, &interner, guard));
}

#[test]
fn qualified_global_symbol_contracts_are_language_and_path_scoped() {
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Object.hasOwn"),
        Some(QualifiedGlobalSymbolContract {
            path: "Object.hasOwn",
            root: "Object",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::TypeScript, "Array.from"),
        Some(QualifiedGlobalSymbolContract {
            path: "Array.from",
            root: "Array",
            requires_unshadowed_root: true,
        })
    );
    assert!(qualified_global_symbol_contract(
        Lang::JavaScript,
        "Object.prototype.hasOwnProperty.call"
    )
    .is_some());
    assert!(qualified_global_symbol_contract(Lang::Python, "Array.from").is_none());
    assert!(qualified_global_symbol_contract(Lang::JavaScript, "value.hasOwnProperty").is_none());
    assert!(qualified_global_symbol_contract(Lang::JavaScript, "Array.fromAsync").is_none());
}

#[test]
fn qualified_global_symbol_requires_matching_node_evidence_and_root_dependency() {
    let interner = Interner::new();
    let build = || {
        let mut b = IlBuilder::new(FileId(0));
        let array = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Array")),
            sp(27),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("from")),
            sp(27),
            &[array],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(27), &[field]);
        (finish_il(b, root, Lang::JavaScript), field)
    };
    let (mut il, field) = build();

    assert!(!qualified_global_symbol(&il, field, "Array.from"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(
        !qualified_global_symbol(&il, field, "Array.from"),
        "qualified API identity must not stand without a root proof"
    );

    let (mut il, field) = build();
    il.evidence.push(evidence_with_dependencies(
        0,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(27)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(qualified_global_symbol(&il, field, "Array.from"));
    assert!(qualified_global_symbol_at_span(
        &il,
        Some(sp(27)),
        NodeKind::Field,
        "Array.from"
    ));
    assert!(!qualified_global_symbol(&il, field, "Array.fromAsync"));

    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(!qualified_global_symbol(&il, field, "Array.from"));
}
