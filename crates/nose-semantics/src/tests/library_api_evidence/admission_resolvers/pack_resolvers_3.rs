use super::*;

#[test]
fn admitted_java_map_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee, _receiver) = java_map_factory_call_il();
    assert!(
        admitted_java_map_factory_at_call(&il, &interner, call).is_none(),
        "raw Java Map.of(...) shape alone must not admit stdlib map factory semantics"
    );

    let contract =
        library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");

    let (mut missing_dependency, interner, call, _callee, _receiver) = java_map_factory_call_il();
    missing_dependency
        .evidence
        .push(java_stdlib_map_factory_record(
            0,
            missing_dependency.node(call).span,
            contract,
            2,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_java_map_factory_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Java Map.of evidence without import dependency is rejected"
    );

    let (mut wrong_pack, interner, call, _callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            BUILTIN_COMPAT_PACK_ID,
            JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
        ));
    assert!(
        admitted_java_map_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Java Map.of evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, _callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            "wrong.java.stdlib.map-factory-api",
        ));
    assert!(
        admitted_java_map_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Java Map.of evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee, receiver) = java_map_factory_call_il();
    push_java_map_import_dependencies(&mut admitted, receiver);
    admitted.evidence.push(java_stdlib_map_factory_record(
        2,
        admitted.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence = admitted_java_map_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::Of)
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

#[test]
fn admitted_java_guava_map_factory_resolver_requires_guava_pack_provenance() {
    let (il, interner, call, _callee, _receiver) = java_guava_map_factory_call_il();
    assert!(
        admitted_java_map_factory_at_call(&il, &interner, call).is_none(),
        "raw Guava ImmutableMap.of(...) shape alone must not admit map factory semantics"
    );

    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of")
        .expect("ImmutableMap.of contract");

    let (mut wrong_pack, interner, call, _callee, receiver) = java_guava_map_factory_call_il();
    push_java_guava_import_dependencies(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
        ));
    assert!(
        admitted_java_map_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Guava ImmutableMap.of evidence under the stdlib map pack is rejected"
    );

    let (mut admitted, interner, call, callee, receiver) = java_guava_map_factory_call_il();
    push_java_guava_import_dependencies(&mut admitted, receiver);
    admitted
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
        ));

    let occurrence = admitted_java_map_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::GuavaImmutableMapOf)
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

#[test]
fn admitted_java_map_entry_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee, _receiver) = java_map_entry_call_il();
    assert!(
        admitted_java_map_entry_at_call(&il, &interner, call).is_none(),
        "raw Java Map.entry(...) shape alone must not admit stdlib map-entry semantics"
    );

    let contract =
        library_java_map_entry_contract(Lang::Java, "Map", "entry").expect("Map.entry contract");

    let (mut missing_dependency, interner, call, _callee, _receiver) = java_map_entry_call_il();
    missing_dependency
        .evidence
        .push(java_stdlib_map_entry_record(
            0,
            missing_dependency.node(call).span,
            contract,
            2,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_java_map_entry_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Java Map.entry evidence without import dependency is rejected"
    );

    let (mut wrong_pack, interner, call, _callee, receiver) = java_map_entry_call_il();
    push_java_map_import_dependencies(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            BUILTIN_COMPAT_PACK_ID,
            JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID,
        ));
    assert!(
        admitted_java_map_entry_at_call(&wrong_pack, &interner, call).is_none(),
        "Java Map.entry evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, _callee, receiver) = java_map_entry_call_il();
    push_java_map_import_dependencies(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[1],
            JAVA_STDLIB_MAP_ENTRY_PACK_ID,
            "wrong.java.stdlib.map-entry-api",
        ));
    assert!(
        admitted_java_map_entry_at_call(&wrong_producer, &interner, call).is_none(),
        "Java Map.entry evidence with the wrong producer is rejected"
    );

    let (mut wrong_arity, interner, call, _callee, receiver) =
        java_map_entry_call_il_with_arg_count(3);
    push_java_map_import_dependencies(&mut wrong_arity, receiver);
    wrong_arity.evidence.push(java_stdlib_map_entry_record(
        2,
        wrong_arity.node(call).span,
        contract,
        3,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(
        admitted_java_map_entry_at_call(&wrong_arity, &interner, call).is_none(),
        "Java Map.entry evidence with unsupported arity is rejected"
    );

    let (mut admitted, interner, call, callee, receiver) = java_map_entry_call_il();
    push_java_map_import_dependencies(&mut admitted, receiver);
    admitted.evidence.push(java_stdlib_map_entry_record(
        2,
        admitted.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence = admitted_java_map_entry_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JavaMapEntryFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

#[test]
fn admitted_js_set_constructor_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = js_global_constructor_call_il("Set");
    assert!(
        admitted_js_like_set_constructor_at_call(&il, &interner, call).is_none(),
        "raw JS new Set(...) shape alone must not admit builtin Set constructor semantics"
    );

    let contract =
        library_js_like_set_constructor_contract(Lang::JavaScript, "Set").expect("Set contract");

    let (mut missing_dependency, interner, call, _callee) = js_global_constructor_call_il("Set");
    missing_dependency
        .evidence
        .push(js_like_builtin_collection_constructor_record(
            0,
            missing_dependency.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_js_like_set_constructor_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Set evidence without construct/global dependencies is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = js_global_constructor_call_il("Set");
    push_js_global_constructor_dependencies(&mut wrong_pack, call, callee, "Set");
    wrong_pack.evidence.push(library_api_record_with_provenance(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    ));
    assert!(
        admitted_js_like_set_constructor_at_call(&wrong_pack, &interner, call).is_none(),
        "Set constructor evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = js_global_constructor_call_il("Set");
    push_js_global_constructor_dependencies(&mut wrong_producer, call, callee, "Set");
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
            "wrong.javascript.builtins.collection-constructor-api",
        ));
    assert!(
        admitted_js_like_set_constructor_at_call(&wrong_producer, &interner, call).is_none(),
        "Set constructor evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, callee) = js_global_constructor_call_il("Set");
    push_js_global_constructor_dependencies(&mut wrong_emitter, call, callee, "Set");
    let mut external_record = js_like_builtin_collection_constructor_record(
        2,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_js_like_set_constructor_at_call(&wrong_emitter, &interner, call).is_none(),
        "Set constructor evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, callee) = js_global_constructor_call_il("Set");
    push_js_global_constructor_dependencies(&mut admitted, call, callee, "Set");
    admitted
        .evidence
        .push(js_like_builtin_collection_constructor_record(
            2,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
    let occurrence = admitted_js_like_set_constructor_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JsLikeSetConstructor
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_js_map_constructor_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = js_global_constructor_call_il("Map");
    assert!(
        admitted_js_like_map_constructor_at_call(&il, &interner, call).is_none(),
        "raw JS new Map(...) shape alone must not admit builtin Map constructor semantics"
    );

    let contract =
        library_js_like_map_constructor_contract(Lang::JavaScript, "Map").expect("Map contract");

    let (mut missing_dependency, interner, call, _callee) = js_global_constructor_call_il("Map");
    missing_dependency
        .evidence
        .push(js_like_builtin_collection_constructor_record(
            0,
            missing_dependency.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_js_like_map_constructor_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Map evidence without construct/global dependencies is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = js_global_constructor_call_il("Map");
    push_js_global_constructor_dependencies(&mut wrong_pack, call, callee, "Map");
    wrong_pack.evidence.push(library_api_record_with_provenance(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
        BUILTIN_COMPAT_PACK_ID,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    ));
    assert!(
        admitted_js_like_map_constructor_at_call(&wrong_pack, &interner, call).is_none(),
        "Map constructor evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = js_global_constructor_call_il("Map");
    push_js_global_constructor_dependencies(&mut wrong_producer, call, callee, "Map");
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            2,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
            "wrong.javascript.builtins.collection-constructor-api",
        ));
    assert!(
        admitted_js_like_map_constructor_at_call(&wrong_producer, &interner, call).is_none(),
        "Map constructor evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, callee) = js_global_constructor_call_il("Map");
    push_js_global_constructor_dependencies(&mut wrong_emitter, call, callee, "Map");
    let mut external_record = js_like_builtin_collection_constructor_record(
        2,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_js_like_map_constructor_at_call(&wrong_emitter, &interner, call).is_none(),
        "Map constructor evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, callee) = js_global_constructor_call_il("Map");
    push_js_global_constructor_dependencies(&mut admitted, call, callee, "Map");
    admitted
        .evidence
        .push(js_like_builtin_collection_constructor_record(
            2,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
    let occurrence = admitted_js_like_map_constructor_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JsLikeMapConstructor
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_java_collection_constructor_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = java_collection_constructor_call_il();
    assert!(
        admitted_java_collection_constructor_at_call(&il, &interner, call).is_none(),
        "raw Java new ArrayList<>() call shape alone must not admit stdlib constructor semantics"
    );

    let contract = library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0)
        .expect("ArrayList constructor contract");

    let (mut missing_dependency, interner, call, _callee) = java_collection_constructor_call_il();
    missing_dependency
        .evidence
        .push(java_stdlib_collection_constructor_record(
            0,
            missing_dependency.node(call).span,
            contract,
            0,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_java_collection_constructor_at_call(&missing_dependency, &interner, call)
            .is_none(),
        "same-span Java constructor evidence without construct/import dependencies is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = java_collection_constructor_call_il();
    push_java_collection_constructor_dependencies(&mut wrong_pack, call, callee);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            3,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            0,
            EvidenceStatus::Asserted,
            &[0, 2],
            BUILTIN_COMPAT_PACK_ID,
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
        ));
    assert!(
        admitted_java_collection_constructor_at_call(&wrong_pack, &interner, call).is_none(),
        "Java constructor evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = java_collection_constructor_call_il();
    push_java_collection_constructor_dependencies(&mut wrong_producer, call, callee);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            3,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            0,
            EvidenceStatus::Asserted,
            &[0, 2],
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
            "wrong.java.stdlib.collection-constructor-api",
        ));
    assert!(
        admitted_java_collection_constructor_at_call(&wrong_producer, &interner, call).is_none(),
        "Java constructor evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = java_collection_constructor_call_il();
    push_java_collection_constructor_dependencies(&mut admitted, call, callee);
    admitted
        .evidence
        .push(java_stdlib_collection_constructor_record(
            3,
            admitted.node(call).span,
            contract,
            0,
            EvidenceStatus::Asserted,
            &[0, 2],
        ));

    let occurrence =
        admitted_java_collection_constructor_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JavaCollectionConstructor(JavaCollectionConstructorKind::EmptyList)
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 0);
}
