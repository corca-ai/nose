use super::*;

#[test]
fn java_stdlib_factories_emit_result_domains() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
    import_binding(&mut lo, sp_at(10), "List", "java.util", "List");
    import_binding(&mut lo, sp_at(11), "Set", "java.util", "Set");
    import_binding(&mut lo, sp_at(12), "Map", "java.util", "Map");
    import_binding(&mut lo, sp_at(13), "Arrays", "java.util", "Arrays");
    import_binding(
        &mut lo,
        sp_at(14),
        "Collections",
        "java.util",
        "Collections",
    );
    assert_java_of_factory_result_domains(&mut lo, &interner);
    assert_java_arrays_and_map_entry_result_domains(&mut lo, &interner);
}

fn assert_java_collection_factory_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );
}

fn assert_java_of_factory_result_domains(lo: &mut Lowering, interner: &Interner) {
    let list = lo.var("List", sp_at(20));
    let list_callee = field_callee(lo, interner, list, "of", sp_at(21));
    let item = lo.int_lit("1", sp_at(22));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(23),
        &[list_callee, item],
    );
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of").unwrap();
    let list_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(list_records.len(), 1);
    assert_java_collection_factory_record_provenance(list_records[0]);
    let list_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(23),
        DomainEvidence::Collection,
        &list_api,
    ));

    let set = lo.var("Set", sp_at(30));
    let set_callee = field_callee(lo, interner, set, "of", sp_at(31));
    let item = lo.int_lit("1", sp_at(32));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(33),
        &[set_callee, item],
    );
    let contract = library_java_collection_factory_contract(Lang::Java, "Set", "of").unwrap();
    let set_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(set_records.len(), 1);
    assert_java_collection_factory_record_provenance(set_records[0]);
    let set_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(33),
        DomainEvidence::Set,
        &set_api,
    ));

    let collections = lo.var("Collections", sp_at(120));
    let empty_set_callee = field_callee(lo, interner, collections, "emptySet", sp_at(121));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(122),
        &[empty_set_callee],
    );
    let contract =
        library_java_collection_factory_contract(Lang::Java, "Collections", "emptySet").unwrap();
    let empty_set_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(empty_set_records.len(), 1);
    assert_java_collection_factory_record_provenance(empty_set_records[0]);
    let empty_set_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(122),
        DomainEvidence::Set,
        &empty_set_api,
    ));

    let collections = lo.var("Collections", sp_at(123));
    let singleton_list_callee =
        field_callee(lo, interner, collections, "singletonList", sp_at(124));
    let item = lo.int_lit("1", sp_at(125));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(126),
        &[singleton_list_callee, item],
    );
    let contract =
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
            .unwrap();
    let singleton_list_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(singleton_list_records.len(), 1);
    assert_java_collection_factory_record_provenance(singleton_list_records[0]);
    let singleton_list_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(126),
        DomainEvidence::Collection,
        &singleton_list_api,
    ));

    let map = lo.var("Map", sp_at(40));
    let map_callee = field_callee(lo, interner, map, "of", sp_at(41));
    let key = lo.str_lit("\"red\"", sp_at(42));
    let value = lo.int_lit("1", sp_at(43));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(44),
        &[map_callee, key, value],
    );
    let contract = library_java_map_factory_contract(Lang::Java, "Map", "of").unwrap();
    let map_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(map_records.len(), 1);
    assert_eq!(
        map_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_STDLIB_MAP_FACTORY_PACK_ID
        ))
    );
    assert_eq!(
        map_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID))
    );
    let map_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(44),
        DomainEvidence::Map,
        &map_api,
    ));

    let collections = lo.var("Collections", sp_at(130));
    let singleton_map_callee = field_callee(lo, interner, collections, "singletonMap", sp_at(131));
    let key = lo.str_lit("\"red\"", sp_at(132));
    let value = lo.int_lit("1", sp_at(133));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(134),
        &[singleton_map_callee, key, value],
    );
    let contract =
        library_java_map_factory_contract(Lang::Java, "Collections", "singletonMap").unwrap();
    let singleton_map_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(singleton_map_records.len(), 1);
    assert_eq!(
        singleton_map_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_STDLIB_MAP_FACTORY_PACK_ID
        ))
    );
    let singleton_map_api = contract_api_ids(&lo.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(134),
        DomainEvidence::Map,
        &singleton_map_api,
    ));
}

fn assert_java_arrays_and_map_entry_result_domains(lo: &mut Lowering, interner: &Interner) {
    let arrays = lo.var("Arrays", sp_at(46));
    let as_list_callee = field_callee(lo, interner, arrays, "asList", sp_at(47));
    let maybe_array = lo.var("items", sp_at(48));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(49),
        &[as_list_callee, maybe_array],
    );
    let as_list_contract =
        library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
    let as_list_records =
        contract_api_records(&lo.evidence, as_list_contract.id, as_list_contract.callee);
    assert_eq!(as_list_records.len(), 1);
    assert_java_collection_factory_record_provenance(as_list_records[0]);
    assert_eq!(
        result_domain_any_count_at(&lo.evidence, sp_at(49)),
        0,
        "single-argument Arrays.asList must not emit any result-domain evidence"
    );

    let arrays = lo.var("Arrays", sp_at(55));
    let as_list_callee = field_callee(lo, interner, arrays, "asList", sp_at(56));
    let red = lo.str_lit("\"red\"", sp_at(57));
    let blue = lo.str_lit("\"blue\"", sp_at(58));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(59),
        &[as_list_callee, red, blue],
    );
    let as_list_records =
        contract_api_records(&lo.evidence, as_list_contract.id, as_list_contract.callee);
    assert_eq!(as_list_records.len(), 2);
    assert_java_collection_factory_record_provenance(as_list_records[1]);
    let as_list_api = library_api_evidence_ids_at(
        &lo.evidence,
        sp_at(59),
        library_api_contract_id_hash(as_list_contract.id),
        library_api_callee_contract_hash(as_list_contract.callee),
        2,
    );
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(59),
        DomainEvidence::Collection,
        &as_list_api,
    ));

    let map = lo.var("Map", sp_at(50));
    let entry_callee = field_callee(lo, interner, map, "entry", sp_at(51));
    let key = lo.str_lit("\"red\"", sp_at(52));
    let value = lo.int_lit("1", sp_at(53));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(54),
        &[entry_callee, key, value],
    );
    let entry_contract = library_java_map_entry_contract(Lang::Java, "Map", "entry").unwrap();
    let entry_records =
        contract_api_records(&lo.evidence, entry_contract.id, entry_contract.callee);
    assert_eq!(entry_records.len(), 1);
    assert_eq!(
        entry_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_STDLIB_MAP_ENTRY_PACK_ID
        ))
    );
    assert_eq!(
        entry_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID))
    );
    assert_eq!(
        result_domain_any_count_at(&lo.evidence, sp_at(54)),
        0,
        "Map.entry returns an entry value, not any receiver-domain container"
    );

    let arrays = lo.var("Arrays", sp_at(60));
    let stream_callee = field_callee(lo, interner, arrays, "stream", sp_at(61));
    let values = lo.var("values", sp_at(62));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(63),
        &[stream_callee, values],
    );
    let stream_contract =
        library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1).unwrap();
    let stream_records =
        contract_api_records(&lo.evidence, stream_contract.id, stream_contract.callee);
    assert_eq!(stream_records.len(), 1);
    assert_eq!(
        stream_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID
        ))
    );
    assert_eq!(
        stream_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID
        ))
    );
    assert_eq!(
        result_domain_any_count_at(&lo.evidence, sp_at(63)),
        0,
        "Arrays.stream produces a stream/protocol surface, not any receiver-domain container"
    );
}
