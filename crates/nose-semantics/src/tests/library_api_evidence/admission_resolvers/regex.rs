use super::*;

fn js_regex_test_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let regex = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("/x/")),
        sp(92),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("test")),
        sp(93),
        &[regex],
    );
    let subject = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("subject")),
        sp(94),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(95), &[callee, subject]);
    let root = b.add(NodeKind::Func, Payload::None, sp(96), &[call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        call,
        callee,
        regex,
    )
}

fn push_regex_literal_dependency(il: &mut Il, regex: NodeId) {
    il.push_evidence(evidence_with_dependencies(
        0,
        EvidenceAnchor::source_span(il.node(regex).span),
        EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
        EvidenceStatus::Asserted,
        vec![],
    ));
}

#[test]
fn admitted_regex_test_resolver_requires_regex_builtin_pack_provenance() {
    let (il, interner, call, _callee, _regex) = js_regex_test_call_il();
    assert!(
        admitted_regex_test_at_call(&il, &interner, call).is_none(),
        "raw regex .test(...) shape alone must not admit builtin regex semantics"
    );

    let contract =
        library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex test contract");

    let (mut missing_dependency, interner, call, _callee, _regex) = js_regex_test_call_il();
    missing_dependency.push_evidence(js_like_builtin_regex_record(
        0,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_regex_test_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span regex .test evidence without regex-literal dependency is rejected"
    );

    let (mut wrong_pack, interner, call, _callee, regex) = js_regex_test_call_il();
    push_regex_literal_dependency(&mut wrong_pack, regex);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_REGEX_PRODUCER_ID,
    ));
    assert!(
        admitted_regex_test_at_call(&wrong_pack, &interner, call).is_none(),
        "regex .test evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, _callee, regex) = js_regex_test_call_il();
    push_regex_literal_dependency(&mut wrong_producer, regex);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        JS_LIKE_BUILTIN_REGEX_PACK_ID,
        "wrong.javascript.builtins.regex-api",
    ));
    assert!(
        admitted_regex_test_at_call(&wrong_producer, &interner, call).is_none(),
        "regex .test evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, _callee, regex) = js_regex_test_call_il();
    push_regex_literal_dependency(&mut wrong_emitter, regex);
    let mut external_record = js_like_builtin_regex_record(
        1,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_regex_test_at_call(&wrong_emitter, &interner, call).is_none(),
        "regex .test evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, callee, regex) = js_regex_test_call_il();
    push_regex_literal_dependency(&mut admitted, regex);
    admitted.push_evidence(js_like_builtin_regex_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_regex_test_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(occurrence.contract.id, LibraryApiContractId::RegexTest);
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, Some(regex));
    assert_eq!(occurrence.arg_count, 1);
}
