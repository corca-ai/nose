use super::*;

#[test]
fn admitted_collection_factory_resolver_requires_api_occurrence_evidence() {
    let (il, interner, call, _callee) = python_list_factory_call_il();
    assert!(
        admitted_free_name_collection_factory_at_call(&il, &interner, call).is_none(),
        "raw Python list(...) call shape alone must not admit collection factory semantics"
    );

    let contract = library_free_name_collection_factory_contract(Lang::Python, "list")
        .expect("Python list factory contract");
    let (mut missing_dependency, interner, call, _callee) = python_list_factory_call_il();
    missing_dependency
        .evidence
        .push(python_builtin_collection_factory_record(
            0,
            missing_dependency.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_free_name_collection_factory_at_call(&missing_dependency, &interner, call)
            .is_none(),
        "same-span collection factory evidence without callee dependency is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = python_list_factory_call_il();
    wrong_pack.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_pack.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    assert!(
        admitted_free_name_collection_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Python builtin collection factory evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = python_list_factory_call_il();
    wrong_producer.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_producer.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
            "wrong.python.builtin.collection-factory-api",
        ));
    assert!(
        admitted_free_name_collection_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Python builtin collection factory evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = python_list_factory_call_il();
    admitted.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    admitted
        .evidence
        .push(python_builtin_collection_factory_record(
            1,
            admitted.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[0],
        ));

    let occurrence =
        admitted_free_name_collection_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::PythonBuiltinCollectionFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_rust_std_collection_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = rust_std_collection_factory_call_il();
    assert!(
        admitted_free_name_collection_factory_at_call(&il, &interner, call).is_none(),
        "raw Rust std::collections factory shape alone must not admit stdlib semantics"
    );

    let contract = library_free_name_collection_factory_contract(
        Lang::Rust,
        "std::collections::HashSet::from",
    )
    .expect("Rust std::collections HashSet::from contract");

    let (mut missing_dependency, interner, call, _callee) = rust_std_collection_factory_call_il();
    missing_dependency
        .evidence
        .push(rust_stdlib_collection_factory_record(
            0,
            missing_dependency.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_free_name_collection_factory_at_call(&missing_dependency, &interner, call)
            .is_none(),
        "same-span Rust std::collections evidence without callee dependency is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = rust_std_collection_factory_call_il();
    wrong_pack.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_pack.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("std::collections::HashSet::from"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    assert!(
        admitted_free_name_collection_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust std::collections evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = rust_std_collection_factory_call_il();
    wrong_producer.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_producer.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("std::collections::HashSet::from"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
            RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
            "wrong.rust.stdlib.collection-factory-api",
        ));
    assert!(
        admitted_free_name_collection_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust std::collections evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = rust_std_collection_factory_call_il();
    admitted.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("std::collections::HashSet::from"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    admitted
        .evidence
        .push(rust_stdlib_collection_factory_record(
            1,
            admitted.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[0],
        ));

    let occurrence =
        admitted_free_name_collection_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::RustStdCollectionFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_rust_std_map_factory_resolver_requires_pack_provenance() {
    assert_rust_std_map_factory_requires_pack_provenance(RustStdMapFactoryAdmission::Call);
}

#[test]
fn admitted_java_collection_factory_resolver_requires_pack_provenance() {
    let interner = Interner::new();
    let (mut raw, call, _root, _local, _contract) =
        java_list_of_import_evidence_il(&interner, true);
    raw.evidence.clear();
    assert!(
        admitted_java_collection_factory_at_call(&raw, &interner, call).is_none(),
        "raw Java List.of(...) shape alone must not admit stdlib collection factory semantics"
    );

    let (mut missing_dependency, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
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
        admitted_java_collection_factory_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Java List.of evidence without import dependency is rejected"
    );

    let (mut wrong_pack, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
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
        admitted_java_collection_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Java List.of evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
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
        admitted_java_collection_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Java List.of evidence with the wrong producer is rejected"
    );

    let (admitted, call, _root, _local, contract) =
        java_list_of_import_evidence_il(&interner, true);
    let occurrence = admitted_java_collection_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ListOf)
    );
    assert_eq!(occurrence.contract.callee, contract.callee);
    assert_eq!(occurrence.arg_count, 1);
}
