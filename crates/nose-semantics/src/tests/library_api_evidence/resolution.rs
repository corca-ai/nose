use super::*;

fn js_array_is_array_call_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let array = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Array")),
        sp(29),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("isArray")),
        sp(30),
        &[array],
    );
    let value = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(31),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(32), &[callee, value]);
    let root = b.add(NodeKind::Module, Payload::None, sp(29), &[call]);
    (finish_il(b, root, Lang::JavaScript), call, callee, array)
}

fn is_array_contract() -> LibraryStaticGlobalMethodContract {
    library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1)
        .expect("test contract")
}

fn admitted_js_array_is_array_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
    let (mut il, call, callee, array) = js_array_is_array_call_il(interner);
    let contract = is_array_contract();
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(array).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    il.push_evidence(language_core_symbol_record(
        1,
        EvidenceAnchor::source_span(il.node(callee).span),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    il.push_evidence(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.push_evidence(js_like_builtin_array_record(
        3,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));
    (il, call, callee, array)
}

fn js_boolean_call_il(interner: &Interner) -> (Il, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let boolean = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Boolean")),
        sp(40),
        &[],
    );
    let value = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(41),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(42), &[boolean, value]);
    let root = b.add(NodeKind::Module, Payload::None, sp(40), &[call]);
    (finish_il(b, root, Lang::JavaScript), call, boolean)
}

fn admitted_js_boolean_il(interner: &Interner) -> (Il, NodeId, NodeId) {
    let (mut il, call, boolean) = js_boolean_call_il(interner);
    let contract = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(boolean).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Boolean"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    il.push_evidence(js_like_builtin_boolean_record(
        1,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    (il, call, boolean)
}

#[test]
fn library_api_evidence_resolution_is_dependency_backed() {
    let interner = Interner::new();
    let (il, call, _callee, _array) = js_array_is_array_call_il(&interner);
    let contract = is_array_contract();

    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Missing
    );

    let (il, call, callee, array) = admitted_js_array_is_array_il(&interner);
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_requires_boolean_builtin_pack_provenance() {
    let interner = Interner::new();
    let contract = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();

    let (il, call, _boolean) = js_boolean_call_il(&interner);
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Missing
    );

    let (il, call, _boolean) = admitted_js_boolean_il(&interner);
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );

    let (mut missing_dependency, call, _boolean) = js_boolean_call_il(&interner);
    missing_dependency.push_evidence(js_like_builtin_boolean_record(
        0,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        contract_status_for_call(
            &missing_dependency,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_pack, call, _boolean) = admitted_js_boolean_il(&interner);
    wrong_pack
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(1))
        .expect("LibraryApi occurrence")
        .provenance
        .pack_hash = Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID));
    assert_eq!(
        contract_status_for_call(&wrong_pack, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_producer, call, _boolean) = admitted_js_boolean_il(&interner);
    wrong_producer
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(1))
        .expect("LibraryApi occurrence")
        .provenance
        .rule_hash = Some(stable_symbol_hash("wrong.javascript.builtins.boolean-api"));
    assert_eq!(
        contract_status_for_call(
            &wrong_producer,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_emitter, call, _boolean) = admitted_js_boolean_il(&interner);
    wrong_emitter
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(1))
        .expect("LibraryApi occurrence")
        .provenance
        .emitter = EvidenceEmitter::External;
    assert_eq!(
        contract_status_for_call(
            &wrong_emitter,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_span_queries_reject_mismatched_callee_and_receiver_spans() {
    let interner = Interner::new();
    let (il, call, callee, array) = admitted_js_array_is_array_il(&interner);
    let contract = is_array_contract();
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(sp(99)),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(sp(99)),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_requires_array_builtin_pack_provenance() {
    let interner = Interner::new();
    let contract = is_array_contract();

    let (mut wrong_pack, call, _callee, _array) = admitted_js_array_is_array_il(&interner);
    wrong_pack
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(3))
        .expect("LibraryApi occurrence")
        .provenance
        .pack_hash = Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID));
    assert_eq!(
        contract_status_for_call(&wrong_pack, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_producer, call, _callee, _array) = admitted_js_array_is_array_il(&interner);
    wrong_producer
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(3))
        .expect("LibraryApi occurrence")
        .provenance
        .rule_hash = Some(stable_symbol_hash("wrong.javascript.builtins.array-api"));
    assert_eq!(
        contract_status_for_call(
            &wrong_producer,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_emitter, call, _callee, _array) = admitted_js_array_is_array_il(&interner);
    wrong_emitter
        .evidence
        .iter_mut()
        .find(|record| record.id == EvidenceId(3))
        .expect("LibraryApi occurrence")
        .provenance
        .emitter = EvidenceEmitter::External;
    assert_eq!(
        contract_status_for_call(
            &wrong_emitter,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_missing_or_ambiguous_dependencies() {
    let interner = Interner::new();
    let contract = is_array_contract();

    let (mut missing_dep, call, _callee, _array) = js_array_is_array_call_il(&interner);
    missing_dep.push_evidence(js_like_builtin_array_record(
        0,
        missing_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        contract_status_for_call(&missing_dep, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut ambiguous_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    ambiguous_dep.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(ambiguous_dep.node(array).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Ambiguous,
        &[],
        Lang::JavaScript,
    ));
    ambiguous_dep.push_evidence(evidence(
        1,
        EvidenceAnchor::node(ambiguous_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    ambiguous_dep.push_evidence(js_like_builtin_array_record(
        2,
        ambiguous_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        contract_status_for_call(
            &ambiguous_dep,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_conflicting_or_misanchored_records() {
    let interner = Interner::new();
    let contract = is_array_contract();

    let (mut conflicting_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    conflicting_dep.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    conflicting_dep.push_evidence(evidence(
        1,
        EvidenceAnchor::node(conflicting_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.push_evidence(language_core_symbol_record(
        2,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Map"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    conflicting_dep.push_evidence(js_like_builtin_array_record(
        3,
        conflicting_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        contract_status_for_call(
            &conflicting_dep,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut il, call, _callee, _array) = admitted_js_array_is_array_il(&interner);
    let boolean = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
    il.push_evidence(library_api_record(
        3,
        il.node(call).span,
        boolean.id,
        boolean.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_anchor, call, _callee, _array) = js_array_is_array_call_il(&interner);
    wrong_anchor.push_evidence(js_like_builtin_array_record(
        0,
        sp(99),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        contract_status_for_call(&wrong_anchor, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Missing
    );
}
