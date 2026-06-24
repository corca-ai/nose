use super::*;

#[test]
fn import_lowering_emits_symbol_identity_evidence_for_aliases() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);

    import_binding(&mut lo, sp(), "deque", "collections", "deque");
    import_namespace(&mut lo, sp(), "math", "math");

    assert!(lo.evidence.iter().any(|record| matches!(
        record.kind,
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash,
            exported_hash,
        }) if module_hash == stable_symbol_hash("collections")
            && exported_hash == stable_symbol_hash("deque")
    )));
    assert!(lo.evidence.iter().any(|record| matches!(
        record.kind,
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace { module_hash })
            if module_hash == stable_symbol_hash("math")
    )));
}

#[test]
fn core_lowering_emits_import_backed_library_api_occurrences() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
    import_binding(&mut lo, sp_at(1), "deque", "collections", "deque");
    let callee = lo.var("deque", sp_at(2));
    let seq = lo.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp_at(3),
        &[],
    );
    lo.add(NodeKind::Call, Payload::None, sp_at(4), &[callee, seq]);

    let contract = nose_semantics::library_imported_collection_factory_contract(
        Lang::Python,
        "collections",
        "deque",
    )
    .expect("deque contract");
    assert_eq!(
        library_api_evidence_count(
            &lo,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        1
    );
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(
        records[0].provenance.pack_hash,
        Some(stable_symbol_hash(PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID))
    );
    assert_eq!(
        records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );

    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
    import_binding(&mut lo, sp_at(10), "Values", "collections", "deque");
    let callee = lo.var("Values", sp_at(11));
    let seq = lo.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp_at(12),
        &[],
    );
    lo.add(NodeKind::Call, Payload::None, sp_at(13), &[callee, seq]);
    assert_eq!(
        library_api_evidence_count(
            &lo,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        1
    );
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(
        records[0].provenance.pack_hash,
        Some(stable_symbol_hash(PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID))
    );
    assert_eq!(
        records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );

    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
    import_namespace(&mut lo, sp_at(30), "collections", "collections");
    let collections = lo.var("collections", sp_at(31));
    let callee = lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern("deque")),
        sp_at(32),
        &[collections],
    );
    let seq = lo.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp_at(33),
        &[],
    );
    lo.add(NodeKind::Call, Payload::None, sp_at(34), &[callee, seq]);
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].provenance.pack_hash,
        Some(stable_symbol_hash(PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID))
    );
    assert_eq!(
        records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );

    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
    import_namespace(&mut lo, sp_at(20), "math", "math");
    let math = lo.var("math", sp_at(21));
    let callee = lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern("prod")),
        sp_at(22),
        &[math],
    );
    let seq = lo.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp_at(23),
        &[],
    );
    lo.add(NodeKind::Call, Payload::None, sp_at(24), &[callee, seq]);
    let contract = library_imported_namespace_function_contract(Lang::Python, "prod", 1)
        .expect("math.prod contract");
    assert_eq!(
        library_api_evidence_count(
            &lo,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        1
    );
    let records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(
        records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::PYTHON_STDLIB_MATH_PACK_ID
        ))
    );
    assert_eq!(
        records[0].provenance.rule_hash,
        Some(stable_symbol_hash(PYTHON_STDLIB_MATH_PRODUCER_ID))
    );
}

#[test]
fn core_lowering_emits_result_domain_evidence_for_library_api_factories() {
    let interner = Interner::new();
    assert_python_deque_factory_result_domain(&interner);
    assert_js_constructor_result_domains(&interner);
}

fn assert_python_deque_factory_result_domain(interner: &Interner) {
    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, interner);
    import_binding(&mut lo, sp_at(1), "Values", "collections", "deque");
    let callee = lo.var("Values", sp_at(2));
    let seq = array_seq(&mut lo, interner, sp_at(3));
    lo.add(NodeKind::Call, Payload::None, sp_at(4), &[callee, seq]);
    let deque = nose_semantics::library_imported_collection_factory_contract(
        Lang::Python,
        "collections",
        "deque",
    )
    .expect("deque contract");
    let deque_api = contract_api_ids(&lo.evidence, deque.id, deque.callee);
    assert!(
        result_domain_depends_on_api(
            &lo.evidence,
            sp_at(4),
            DomainEvidence::Collection,
            &deque_api,
        ),
        "collections.deque result domain should depend on the admitted LibraryApi occurrence"
    );
}

fn assert_js_constructor_result_domains(interner: &Interner) {
    let mut lo = Lowering::new(FileId(0), b"", Lang::JavaScript, interner);
    let set = lo.unshadowed_global_var("Set", sp_at(70));
    let seq = array_seq(&mut lo, interner, sp_at(71));
    lo.record_source_fact(sp_at(72), SourceFactKind::Call(SourceCallKind::Construct));
    lo.add(NodeKind::Call, Payload::None, sp_at(72), &[set, seq]);
    let set_contract =
        library_js_like_set_constructor_contract(Lang::JavaScript, "Set").expect("Set constructor");
    let set_records = contract_api_records(&lo.evidence, set_contract.id, set_contract.callee);
    assert_eq!(
        set_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
        ))
    );
    assert_eq!(
        set_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID
        ))
    );
    let set_api = contract_api_ids(&lo.evidence, set_contract.id, set_contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(72),
        DomainEvidence::Set,
        &set_api,
    ));

    let map = lo.unshadowed_global_var("Map", sp_at(80));
    let seq = array_seq(&mut lo, interner, sp_at(81));
    lo.record_source_fact(sp_at(82), SourceFactKind::Call(SourceCallKind::Construct));
    lo.add(NodeKind::Call, Payload::None, sp_at(82), &[map, seq]);
    let map_contract =
        library_js_like_map_constructor_contract(Lang::JavaScript, "Map").expect("Map constructor");
    let map_records = contract_api_records(&lo.evidence, map_contract.id, map_contract.callee);
    assert_eq!(
        map_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID
        ))
    );
    assert_eq!(
        map_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID
        ))
    );
    let map_api = contract_api_ids(&lo.evidence, map_contract.id, map_contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(82),
        DomainEvidence::Map,
        &map_api,
    ));

    let array = lo.unshadowed_global_var("Array", sp_at(90));
    let from_callee = field_callee(&mut lo, interner, array, "from", sp_at(91));
    lo.record_qualified_global_symbol(sp_at(91), NodeKind::Field, "Array.from");
    let iterable = lo.var("iterable", sp_at(92));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(93),
        &[from_callee, iterable],
    );
    let from_contract =
        library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap();
    let from_records = contract_api_records(&lo.evidence, from_contract.id, from_contract.callee);
    assert_eq!(
        from_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_ARRAY_PACK_ID
        ))
    );
    assert_eq!(
        from_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID))
    );
    let from_api = contract_api_ids(&lo.evidence, from_contract.id, from_contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(93),
        DomainEvidence::Array,
        &from_api,
    ));

    let array = lo.unshadowed_global_var("Array", sp_at(94));
    let is_array_callee = field_callee(&mut lo, interner, array, "isArray", sp_at(95));
    lo.record_qualified_global_symbol(sp_at(95), NodeKind::Field, "Array.isArray");
    let value = lo.var("value", sp_at(96));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(97),
        &[is_array_callee, value],
    );
    let is_array_contract =
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1).unwrap();
    let is_array_records =
        contract_api_records(&lo.evidence, is_array_contract.id, is_array_contract.callee);
    assert_eq!(
        is_array_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_ARRAY_PACK_ID
        ))
    );
    assert_eq!(
        is_array_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID))
    );

    let promise = lo.unshadowed_global_var("Promise", sp_at(98));
    let resolve_callee = field_callee(&mut lo, interner, promise, "resolve", sp_at(99));
    lo.record_qualified_global_symbol(sp_at(99), NodeKind::Field, "Promise.resolve");
    let value = lo.int_lit("1", sp_at(100));
    lo.add(
        NodeKind::Call,
        Payload::None,
        sp_at(101),
        &[resolve_callee, value],
    );
    let resolve_contract =
        library_promise_resolve_contract(Lang::JavaScript, "Promise", "resolve", 1).unwrap();
    let resolve_records =
        contract_api_records(&lo.evidence, resolve_contract.id, resolve_contract.callee);
    assert_eq!(
        resolve_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_PROMISE_PACK_ID
        ))
    );
    assert_eq!(
        resolve_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID))
    );
    let resolve_api = contract_api_ids(&lo.evidence, resolve_contract.id, resolve_contract.callee);
    assert!(result_domain_depends_on_api(
        &lo.evidence,
        sp_at(101),
        DomainEvidence::PromiseLike,
        &resolve_api,
    ));

    let boolean = lo.unshadowed_global_var("Boolean", sp_at(102));
    let value = lo.var("value", sp_at(103));
    lo.add(NodeKind::Call, Payload::None, sp_at(104), &[boolean, value]);
    let boolean_contract =
        library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
    let boolean_records =
        contract_api_records(&lo.evidence, boolean_contract.id, boolean_contract.callee);
    assert_eq!(
        boolean_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_BOOLEAN_PACK_ID
        ))
    );
    assert_eq!(
        boolean_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID))
    );
    assert_eq!(
        result_domain_any_count_at(&lo.evidence, sp_at(104)),
        0,
        "Boolean(...) has LibraryApi identity but no container result-domain"
    );
}

mod aliased_imports;
mod guava_factories;
mod java_factories;
