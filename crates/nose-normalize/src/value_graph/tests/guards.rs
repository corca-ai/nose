use super::support::*;

#[test]
fn long_path_conditions_compact_to_formula() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let root = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
    let il = finish_test_il(b, root, Lang::Rust);
    let mut builder = Builder::new(&il, &interner);
    for idx in 0..40 {
        let value = builder.int_const(idx as u32);
        builder.path.push(value);
    }

    let condition = builder.path_cond().expect("path condition");
    assert!(matches!(
        builder.nodes[condition as usize].op,
        ValOp::Formula(_)
    ));
}

#[test]
fn short_path_conditions_keep_explicit_and_chain() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let root = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
    let il = finish_test_il(b, root, Lang::Rust);
    let mut builder = Builder::new(&il, &interner);
    for idx in 0..3 {
        let value = builder.int_const(idx as u32);
        builder.path.push(value);
    }

    let condition = builder.path_cond().expect("path condition");
    assert!(!matches!(
        builder.nodes[condition as usize].op,
        ValOp::Formula(_)
    ));
}

#[test]
fn record_guard_value_tag_requires_guard_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("record_guard");
    let subject = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(60),
        &[],
    );
    let object = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("object")),
        sp(60),
        &[],
    );
    let non_null = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("non_null")),
        sp(60),
        &[],
    );
    let not_array = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("not_array")),
        sp(60),
        &[],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(60),
        &[subject, object, non_null, not_array],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(60), &[guard]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(guard, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[raw as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));

    il.evidence.push(language_core_evidence(
        0,
        Lang::JavaScript,
        EvidenceAnchor::sequence(sp(60)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
    ));
    let mut builder = Builder::new(&il, &interner);
    let surface_only = builder.eval(guard, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[surface_only as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(60)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(sp(60)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        vec![EvidenceId(1)],
    ));
    il.evidence.push(evidence_with_dependencies(
        3,
        EvidenceAnchor::sequence(sp(60)),
        EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
            subject_hash: stable_symbol_hash("value"),
            null_check: JsRecordGuardNullCheck::StrictNonNull,
            comparison: JsRecordGuardComparison::StrictOnly,
        }),
        vec![EvidenceId(2)],
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(guard, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));
}

fn push_own_property_guard_evidence(il: &mut Il, base_id: u32, span: Span) {
    il.evidence.push(evidence(
        base_id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Object"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
        base_id + 1,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(base_id)],
    ));
    il.evidence.push(evidence_with_dependencies(
        base_id + 2,
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(base_id + 1)],
    ));
}

#[test]
fn own_property_guard_value_tag_requires_node_shape_and_guard_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("own_property_guard");
    let receiver = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("map")),
        sp(62),
        &[],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(62),
        &[],
    );
    let own = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("own")),
        sp(62),
        &[],
    );
    let present = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("present")),
        sp(62),
        &[],
    );
    let malformed_present = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("maybe")),
        sp(62),
        &[],
    );
    let malformed = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(62),
        &[receiver, key, own, malformed_present],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(63),
        &[receiver, key, own, present],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(62), &[malformed, guard]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);
    for (id, span) in [(0, sp(62)), (4, sp(63))] {
        il.evidence.push(language_core_evidence(
            id,
            Lang::JavaScript,
            EvidenceAnchor::sequence(span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        ));
    }
    push_own_property_guard_evidence(&mut il, 1, sp(62));
    push_own_property_guard_evidence(&mut il, 5, sp(63));

    let mut builder = Builder::new(&il, &interner);
    let malformed_value = builder.eval(malformed, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[malformed_value as usize].op,
        ValOp::Seq(SEQ_VALUE_OWN_PROPERTY_GUARD)
    ));

    let proven_value = builder.eval(guard, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven_value as usize].op,
        ValOp::Seq(SEQ_VALUE_OWN_PROPERTY_GUARD)
    ));
}
