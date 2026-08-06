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
    assert_java_collection_factory_requires_pack_provenance(JavaCollectionFactoryAdmission::Span);
}

#[test]
fn admitted_span_rust_std_map_factory_requires_pack_provenance() {
    assert_rust_std_map_factory_requires_pack_provenance(RustStdMapFactoryAdmission::Span);
}

#[test]
fn admitted_span_java_map_factory_requires_pack_provenance() {
    assert_java_map_factory_requires_pack_provenance(JavaMapFactoryAdmission::Span);
}
