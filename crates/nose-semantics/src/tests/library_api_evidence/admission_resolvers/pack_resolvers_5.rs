use super::*;

#[test]
fn admitted_static_collection_adapter_resolver_requires_import_backed_api_occurrence_evidence() {
    let (il, interner, call, _receiver) = java_arrays_stream_call_il();
    assert!(
        admitted_static_collection_adapter_at_call(&il, &interner, call).is_none(),
        "raw Java Arrays.stream(...) call shape alone must not admit adapter semantics"
    );

    let contract = library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1)
        .expect("Java Arrays.stream contract");
    let (mut missing_dependency, interner, call, _receiver) = java_arrays_stream_call_il();
    missing_dependency.push_evidence(java_stdlib_static_collection_adapter_record(
        0,
        missing_dependency.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_static_collection_adapter_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Java static adapter evidence without import dependency is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = java_arrays_stream_call_il();
    let imported_binding = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("Arrays"),
    });
    wrong_pack.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("Arrays")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_pack.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_pack.node(receiver).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[1],
        BUILTIN_COMPAT_PACK_ID,
        JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID,
    ));
    assert!(
        admitted_static_collection_adapter_at_call(&wrong_pack, &interner, call).is_none(),
        "Java Arrays.stream evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = java_arrays_stream_call_il();
    wrong_producer.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("Arrays")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_producer.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_producer.node(receiver).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        2,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[1],
        JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
        "wrong.java.stdlib.static-collection-adapter-api",
    ));
    assert!(
        admitted_static_collection_adapter_at_call(&wrong_producer, &interner, call).is_none(),
        "Java Arrays.stream evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) = java_arrays_stream_call_il();
    wrong_emitter.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("Arrays")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_emitter.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_emitter.node(receiver).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    let mut external_record = java_stdlib_static_collection_adapter_record(
        2,
        wrong_emitter.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_static_collection_adapter_at_call(&wrong_emitter, &interner, call).is_none(),
        "Java Arrays.stream evidence from an external emitter is rejected"
    );

    let (mut wrong_arity, interner, call, receiver) = java_arrays_stream_call_il_with_arg_count(2);
    wrong_arity.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("Arrays")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_arity.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_arity.node(receiver).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    wrong_arity.push_evidence(java_stdlib_static_collection_adapter_record(
        2,
        wrong_arity.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(
        admitted_static_collection_adapter_at_call(&wrong_arity, &interner, call).is_none(),
        "Java Arrays.stream evidence with unsupported arity is rejected"
    );

    let (mut admitted, interner, call, receiver) = java_arrays_stream_call_il();
    let imported_binding = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("Arrays"),
    });
    admitted.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("Arrays")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    admitted.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(admitted.node(receiver).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    admitted.push_evidence(java_stdlib_static_collection_adapter_record(
        2,
        admitted.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence =
        admitted_static_collection_adapter_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(occurrence.contract.id, contract.id);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}
