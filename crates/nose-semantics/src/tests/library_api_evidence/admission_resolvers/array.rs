use super::*;

fn js_array_from_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Array")),
        sp(87),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("from")),
        sp(88),
        &[array],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(89), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(90), &[callee, value]);
    let root = b.add(NodeKind::Func, Payload::None, sp(91), &[call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        call,
        callee,
        array,
    )
}

fn push_array_from_dependencies(il: &mut Il, callee: NodeId, array: NodeId) {
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::source_span(il.node(callee).span),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.push_evidence(language_core_symbol_record(
        2,
        EvidenceAnchor::node(il.node(array).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::JavaScript,
    ));
}

#[test]
fn admitted_array_from_resolver_requires_array_builtin_pack_provenance() {
    let (il, interner, call, _callee, _array) = js_array_from_call_il();
    assert!(
        admitted_map_key_view_wrapper_at_call(&il, &interner, call).is_none(),
        "raw Array.from(...) shape alone must not admit map-key-view wrapper semantics"
    );

    let contract = library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1)
        .expect("Array.from contract");

    let (mut wrong_pack, interner, call, callee, array) = js_array_from_call_il();
    push_array_from_dependencies(&mut wrong_pack, callee, array);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        3,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1, 2],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
    ));
    assert!(
        admitted_map_key_view_wrapper_at_call(&wrong_pack, &interner, call).is_none(),
        "Array.from evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee, array) = js_array_from_call_il();
    push_array_from_dependencies(&mut wrong_producer, callee, array);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        3,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1, 2],
        JS_LIKE_BUILTIN_ARRAY_PACK_ID,
        "wrong.javascript.builtins.array-api",
    ));
    assert!(
        admitted_map_key_view_wrapper_at_call(&wrong_producer, &interner, call).is_none(),
        "Array.from evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, callee, array) = js_array_from_call_il();
    push_array_from_dependencies(&mut wrong_emitter, callee, array);
    let mut external_record = js_like_builtin_array_record(
        3,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1, 2],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_map_key_view_wrapper_at_call(&wrong_emitter, &interner, call).is_none(),
        "Array.from evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, callee, array) = js_array_from_call_il();
    push_array_from_dependencies(&mut admitted, callee, array);
    admitted.push_evidence(js_like_builtin_array_record(
        3,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1, 2],
    ));
    let resolved = admitted_map_key_view_wrapper_at_call(&admitted, &interner, call)
        .expect("qualified global and unshadowed receiver admit Array.from");
    assert_eq!(
        resolved.contract.id,
        LibraryApiContractId::MapKeyViewWrapper
    );
    assert_eq!(resolved.receiver, Some(array));
}
