use super::*;

#[test]
fn java_guava_immutable_factories_emit_api_and_result_domains() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
    import_binding(
        &mut lo,
        sp_at(1),
        "ImmutableList",
        "com.google.common.collect",
        "ImmutableList",
    );
    import_binding(
        &mut lo,
        sp_at(2),
        "ImmutableSet",
        "com.google.common.collect",
        "ImmutableSet",
    );
    import_binding(
        &mut lo,
        sp_at(3),
        "ImmutableMap",
        "com.google.common.collect",
        "ImmutableMap",
    );

    assert_guava_collection_factory_result_domain(
        &mut lo,
        &interner,
        "ImmutableList",
        DomainEvidence::Collection,
        sp_at(10),
    );
    assert_guava_collection_factory_result_domain(
        &mut lo,
        &interner,
        "ImmutableSet",
        DomainEvidence::Set,
        sp_at(20),
    );
    assert_guava_map_factory_result_domain(&mut lo, &interner, sp_at(30));
    assert_guava_copy_of_remains_closed(&mut lo, &interner, sp_at(40));
}

fn assert_guava_collection_factory_result_domain(
    lo: &mut Lowering,
    interner: &Interner,
    receiver: &str,
    domain: DomainEvidence,
    span: Span,
) {
    let receiver_node = lo.var(receiver, span);
    let callee = field_callee(lo, interner, receiver_node, "of", span);
    let item = lo.int_lit("1", span);
    lo.add(NodeKind::Call, Payload::None, span, &[callee, item]);

    let contract = library_java_collection_factory_contract(Lang::Java, receiver, "of").unwrap();
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(records.len(), 1);
    assert_java_guava_factory_record_provenance(records[0]);
    let api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        span,
        domain,
        &api,
    ));
}

fn assert_guava_map_factory_result_domain(lo: &mut Lowering, interner: &Interner, span: Span) {
    let map = lo.var("ImmutableMap", span);
    let callee = field_callee(lo, interner, map, "of", span);
    let key = lo.str_lit("\"red\"", span);
    let value = lo.int_lit("1", span);
    lo.add(NodeKind::Call, Payload::None, span, &[callee, key, value]);

    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of").unwrap();
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(records.len(), 1);
    assert_java_guava_factory_record_provenance(records[0]);
    let api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        span,
        DomainEvidence::Map,
        &api,
    ));
}

fn assert_guava_copy_of_remains_closed(lo: &mut Lowering, interner: &Interner, span: Span) {
    let before = lo.evidence.len();
    let list = lo.var("ImmutableList", span);
    let callee = field_callee(lo, interner, list, "copyOf", span);
    let values = lo.var("values", span);
    lo.add(NodeKind::Call, Payload::None, span, &[callee, values]);
    let list_contract =
        library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of").unwrap();
    assert_eq!(
        contract_api_records(&lo.evidence, list_contract.id, list_contract.callee).len(),
        1,
        "Guava copyOf remains closed until source-domain proof exists"
    );
    assert_eq!(result_domain_any_count_at(&lo.evidence, span), 0);
    assert!(lo.evidence.len() >= before);
}

fn assert_java_guava_factory_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );
}
