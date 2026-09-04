use super::*;

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

fn own_property_surface_evidence(id: u32, span: Span) -> EvidenceRecord {
    language_core_evidence(
        id,
        EvidenceAnchor::sequence(span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    )
}

#[test]
fn own_property_guard_requires_dedicated_guard_evidence() {
    let interner = Interner::new();
    let (mut il, guard) = js_own_property_guard_il(&interner);

    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.push_evidence(own_property_surface_evidence(0, sp(22)));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.push_evidence(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        None,
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    il.push_evidence(qualified_global_dependency(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        Some(3),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        3,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(own_property_guard_record(
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
    il.push_evidence(own_property_surface_evidence(0, sp(22)));
    il.push_evidence(qualified_global_dependency(
        1,
        sp(22),
        "Array.from",
        EvidenceStatus::Asserted,
        None,
    ));
    il.push_evidence(own_property_guard_record(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.push_evidence(own_property_surface_evidence(0, sp(22)));
    il.push_evidence(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Ambiguous,
        None,
    ));
    il.push_evidence(own_property_guard_record(
        2,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));

    let (mut il, guard) = js_own_property_guard_il(&interner);
    il.push_evidence(own_property_surface_evidence(0, sp(22)));
    il.push_evidence(qualified_global_dependency(
        1,
        sp(22),
        "value.hasOwnProperty",
        EvidenceStatus::Asserted,
        None,
    ));
    il.push_evidence(own_property_guard_record(
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
    il.push_evidence(own_property_surface_evidence(0, sp(22)));
    il.push_evidence(qualified_global_dependency(
        1,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        Some(5),
    ));
    il.push_evidence(qualified_global_dependency(
        2,
        sp(22),
        "Object.prototype.hasOwnProperty.call",
        EvidenceStatus::Asserted,
        Some(6),
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        5,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(unshadowed_global_source_dependency(
        6,
        sp(22),
        "Object",
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(own_property_guard_record(
        3,
        sp(22),
        "Object.hasOwn",
        EvidenceStatus::Asserted,
        &[1],
    ));
    il.push_evidence(own_property_guard_record(
        4,
        sp(22),
        "Object.prototype.hasOwnProperty.call",
        EvidenceStatus::Asserted,
        &[2],
    ));
    assert!(!own_property_guard_for_node(&il, &interner, guard));
}
