use super::*;

#[test]
fn admitted_span_resolver_requires_api_occurrence_evidence() {
    let (il, interner, call, callee, receiver) = rust_map_get_call_il();
    let occurrence = LibraryApiSpanCall {
        call_span: Some(il.node(call).span),
        callee_span: Some(il.node(callee).span),
        receiver_span: Some(il.node(receiver).span),
        arg_count: 1,
    };
    assert!(
        admitted_map_get_at_call_span(&il, &interner, occurrence, stable_symbol_hash("get"))
            .is_none(),
        "raw Rust map.get(...) value-level span shape alone must not admit map-get semantics"
    );

    let contract = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map get contract");
    let (mut missing_dependency, interner, call, callee, receiver) = rust_map_get_call_il();
    let occurrence = LibraryApiSpanCall {
        call_span: Some(missing_dependency.node(call).span),
        callee_span: Some(missing_dependency.node(callee).span),
        receiver_span: Some(missing_dependency.node(receiver).span),
        arg_count: 1,
    };
    missing_dependency.evidence.push(map_get_protocol_record(
        0,
        missing_dependency.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_map_get_at_call_span(
            &missing_dependency,
            &interner,
            occurrence,
            stable_symbol_hash("get")
        )
        .is_none(),
        "span-backed map-get API occurrence without receiver-domain dependency is rejected"
    );

    let (mut admitted, interner, call, callee, receiver) = rust_map_get_call_il();
    let occurrence = LibraryApiSpanCall {
        call_span: Some(admitted.node(call).span),
        callee_span: Some(admitted.node(callee).span),
        receiver_span: Some(admitted.node(receiver).span),
        arg_count: 1,
    };
    admitted.evidence.push(evidence(
        0,
        EvidenceAnchor::node(admitted.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    admitted.evidence.push(map_get_protocol_record(
        1,
        admitted.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let resolved =
        admitted_map_get_at_call_span(&admitted, &interner, occurrence, stable_symbol_hash("get"))
            .unwrap();
    assert_eq!(resolved.contract.id, LibraryApiContractId::MapGet);
    assert_eq!(resolved.call_span, Some(admitted.node(call).span));
    assert_eq!(resolved.callee_span, Some(admitted.node(callee).span));
    assert_eq!(resolved.receiver_span, Some(admitted.node(receiver).span));
    assert_eq!(resolved.arg_count, 1);
}

#[test]
fn admitted_span_factory_resolver_requires_import_backed_api_occurrence() {
    let interner = Interner::new();
    let (mut raw, call, _root, _local, _contract) =
        java_list_of_import_evidence_il(&interner, true);
    let callee = raw.children(call)[0];
    let receiver = raw.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(raw.node(call).span),
        callee_span: Some(raw.node(callee).span),
        receiver_span: Some(raw.node(receiver).span),
        arg_count: 1,
    };
    raw.evidence.clear();
    assert!(
        admitted_java_collection_factory_at_call_span(
            &raw,
            &interner,
            occurrence,
            stable_symbol_hash("of")
        )
        .is_none(),
        "raw Java List.of(...) value-level span shape alone must not admit factory semantics"
    );

    let (mut missing_dependency, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    let callee = missing_dependency.children(call)[0];
    let receiver = missing_dependency.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(missing_dependency.node(call).span),
        callee_span: Some(missing_dependency.node(callee).span),
        receiver_span: Some(missing_dependency.node(receiver).span),
        arg_count: 1,
    };
    missing_dependency.evidence.clear();
    missing_dependency
        .evidence
        .push(java_stdlib_collection_factory_record(
            0,
            missing_dependency.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_java_collection_factory_at_call_span(
            &missing_dependency,
            &interner,
            occurrence,
            stable_symbol_hash("of")
        )
        .is_none(),
        "span-backed Java List.of API occurrence without import dependency is rejected"
    );

    let (mut wrong_pack, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    let callee = wrong_pack.children(call)[0];
    let receiver = wrong_pack.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(wrong_pack.node(call).span),
        callee_span: Some(wrong_pack.node(callee).span),
        receiver_span: Some(wrong_pack.node(receiver).span),
        arg_count: 1,
    };
    wrong_pack
        .evidence
        .retain(|record| record.id != EvidenceId(2));
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[1],
            BUILTIN_COMPAT_PACK_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ));
    assert!(
        admitted_java_collection_factory_at_call_span(
            &wrong_pack,
            &interner,
            occurrence,
            stable_symbol_hash("of"),
        )
        .is_none(),
        "span-backed Java List.of evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    let callee = wrong_producer.children(call)[0];
    let receiver = wrong_producer.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(wrong_producer.node(call).span),
        callee_span: Some(wrong_producer.node(callee).span),
        receiver_span: Some(wrong_producer.node(receiver).span),
        arg_count: 1,
    };
    wrong_producer
        .evidence
        .retain(|record| record.id != EvidenceId(2));
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            "wrong.java.stdlib.collection-factory-api",
        ));
    assert!(
        admitted_java_collection_factory_at_call_span(
            &wrong_producer,
            &interner,
            occurrence,
            stable_symbol_hash("of"),
        )
        .is_none(),
        "span-backed Java List.of evidence with the wrong producer is rejected"
    );

    let (admitted, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    let callee = admitted.children(call)[0];
    let receiver = admitted.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(admitted.node(call).span),
        callee_span: Some(admitted.node(callee).span),
        receiver_span: Some(admitted.node(receiver).span),
        arg_count: 1,
    };
    let resolved = admitted_java_collection_factory_at_call_span(
        &admitted,
        &interner,
        occurrence,
        stable_symbol_hash("of"),
    )
    .unwrap();
    assert_eq!(resolved.contract.id, contract.id);
    assert_eq!(resolved.contract.callee, contract.callee);
    assert_eq!(resolved.call_span, Some(admitted.node(call).span));
    assert_eq!(resolved.callee_span, Some(admitted.node(callee).span));
    assert_eq!(resolved.receiver_span, Some(admitted.node(receiver).span));
    assert_eq!(resolved.arg_count, 1);
}

#[test]
fn admitted_span_rust_std_map_factory_requires_pack_provenance() {
    assert_rust_std_map_factory_requires_pack_provenance(RustStdMapFactoryAdmission::Span);
}

#[test]
fn admitted_span_java_map_factory_requires_pack_provenance() {
    assert_java_map_factory_requires_pack_provenance(JavaMapFactoryAdmission::Span);
}
