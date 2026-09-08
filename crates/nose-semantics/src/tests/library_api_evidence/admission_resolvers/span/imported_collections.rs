use super::*;

#[test]
fn admitted_span_imported_collection_factory_rejects_namespace_dependency_for_bare_callee() {
    let (mut il, interner, call, callee) = python_deque_factory_call_il();
    let namespace_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("collections"),
    });
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(61), stable_symbol_hash("Values")),
        namespace_symbol,
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        namespace_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("Python collections.deque factory contract");
    il.push_evidence(python_stdlib_collection_factory_record(
        2,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence = LibraryApiSpanCall {
        call_span: Some(il.node(call).span),
        callee_span: Some(il.node(callee).span),
        receiver_span: None,
        arg_count: 1,
    };
    assert!(
        admitted_imported_collection_factory_at_call_span(&il, &interner, occurrence).is_none(),
        "bare imported collection factory calls require an imported-binding dependency, not a namespace dependency"
    );
}

#[test]
fn admitted_span_imported_collection_factory_rejects_receiver_span_without_field_callee() {
    let (mut il, interner, call, callee) = python_deque_factory_call_il();
    let namespace_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("collections"),
    });
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(61), stable_symbol_hash("Values")),
        namespace_symbol,
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        namespace_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("Python collections.deque factory contract");
    il.push_evidence(python_stdlib_collection_factory_record(
        2,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence = LibraryApiSpanCall {
        call_span: Some(il.node(call).span),
        callee_span: Some(il.node(callee).span),
        receiver_span: Some(il.node(callee).span),
        arg_count: 1,
    };
    assert!(
        admitted_imported_collection_factory_at_call_span(&il, &interner, occurrence).is_none(),
        "namespace dependency admission requires a field callee span for the exported member"
    );
}

#[test]
fn admitted_span_imported_collection_factory_rejects_unrelated_namespace_receiver_span() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let other = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("other")),
        sp(70),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("deque")),
        sp(71),
        &[other],
    );
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(72), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(73), &[callee, arg]);
    let unrelated_namespace = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("collections")),
        sp(74),
        &[],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(75),
        &[unrelated_namespace, call],
    );
    let mut il = finish_il(b, root, Lang::Python);

    let namespace_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("collections"),
    });
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(69), stable_symbol_hash("collections")),
        namespace_symbol,
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(unrelated_namespace).span, NodeKind::Var),
        namespace_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("Python collections.deque factory contract");
    il.push_evidence(python_stdlib_collection_factory_record(
        2,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence = LibraryApiSpanCall {
        call_span: Some(il.node(call).span),
        callee_span: Some(il.node(callee).span),
        receiver_span: Some(il.node(unrelated_namespace).span),
        arg_count: 1,
    };
    assert!(
        admitted_imported_collection_factory_at_call_span(&il, &interner, occurrence).is_none(),
        "namespace dependency admission requires the queried receiver span to be the field callee receiver"
    );
}
