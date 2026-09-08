use super::*;

#[test]
fn admitted_rust_vec_new_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = rust_vec_new_call_il();
    assert!(
        admitted_rust_vec_new_factory_at_call(&il, &interner, call).is_none(),
        "raw Rust Vec::new() call shape alone must not admit stdlib Vec semantics"
    );

    let contract =
        library_rust_vec_new_factory_contract(Lang::Rust, "Vec::new").expect("Vec::new contract");

    let (mut missing_dependency, interner, call, _callee) = rust_vec_new_call_il();
    missing_dependency.push_evidence(rust_stdlib_vec_record(
        0,
        missing_dependency.node(call).span,
        contract,
        0,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_rust_vec_new_factory_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Vec::new evidence without callee dependency is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = rust_vec_new_call_il();
    wrong_pack.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_pack.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Vec::new"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_VEC_PRODUCER_ID,
    ));
    assert!(
        admitted_rust_vec_new_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust Vec::new evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = rust_vec_new_call_il();
    wrong_producer.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_producer.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Vec::new"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
        RUST_STDLIB_VEC_PACK_ID,
        "wrong.rust.stdlib.vec-factory-api",
    ));
    assert!(
        admitted_rust_vec_new_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust Vec::new evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = rust_vec_new_call_il();
    admitted.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Vec::new"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    admitted.push_evidence(rust_stdlib_vec_record(
        1,
        admitted.node(call).span,
        contract,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence = admitted_rust_vec_new_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::RustVecNewFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 0);
}

#[test]
fn admitted_rust_vec_macro_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = rust_vec_macro_call_il();
    assert!(
        admitted_rust_vec_macro_factory_at_call(&il, &interner, call).is_none(),
        "raw Rust vec! macro call shape alone must not admit stdlib Vec semantics"
    );

    let contract =
        library_rust_vec_macro_factory_contract(Lang::Rust, "vec").expect("vec! contract");

    let (mut missing_dependency, interner, call, _callee) = rust_vec_macro_call_il();
    missing_dependency.push_evidence(rust_stdlib_vec_record(
        0,
        missing_dependency.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_rust_vec_macro_factory_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span vec! evidence without macro/source dependencies is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = rust_vec_macro_call_il();
    wrong_pack.push_evidence(evidence(
        0,
        EvidenceAnchor::source_span(wrong_pack.node(call).span),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
        EvidenceStatus::Asserted,
    ));
    wrong_pack.push_evidence(language_core_symbol_record(
        1,
        EvidenceAnchor::node(wrong_pack.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("vec"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_pack.push_evidence(library_api_record_with_provenance(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_VEC_PRODUCER_ID,
    ));
    assert!(
        admitted_rust_vec_macro_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust vec! evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = rust_vec_macro_call_il();
    wrong_producer.push_evidence(evidence(
        0,
        EvidenceAnchor::source_span(wrong_producer.node(call).span),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
        EvidenceStatus::Asserted,
    ));
    wrong_producer.push_evidence(language_core_symbol_record(
        1,
        EvidenceAnchor::node(wrong_producer.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("vec"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    wrong_producer.push_evidence(library_api_record_with_provenance(
        2,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
        RUST_STDLIB_VEC_PACK_ID,
        "wrong.rust.stdlib.vec-factory-api",
    ));
    assert!(
        admitted_rust_vec_macro_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust vec! evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = rust_vec_macro_call_il();
    admitted.push_evidence(evidence(
        0,
        EvidenceAnchor::source_span(admitted.node(call).span),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
        EvidenceStatus::Asserted,
    ));
    admitted.push_evidence(language_core_symbol_record(
        1,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("vec"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Rust,
    ));
    admitted.push_evidence(rust_stdlib_vec_record(
        2,
        admitted.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));

    let occurrence = admitted_rust_vec_macro_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::RustVecMacroFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_imported_collection_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _callee) = python_deque_factory_call_il();
    assert!(
        admitted_imported_collection_factory_at_call(&il, &interner, call).is_none(),
        "raw imported deque(...) call shape alone must not admit collection factory semantics"
    );

    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("Python collections.deque factory contract");
    let imported_binding = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("collections"),
        exported_hash: stable_symbol_hash("deque"),
    });

    let (mut missing_dependency, interner, call, _callee) = python_deque_factory_call_il();
    missing_dependency.push_evidence(python_stdlib_collection_factory_record(
        0,
        missing_dependency.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_imported_collection_factory_at_call(&missing_dependency, &interner, call)
            .is_none(),
        "same-span collections.deque evidence without import dependency is rejected"
    );

    let (mut wrong_pack, interner, call, callee) = python_deque_factory_call_il();
    wrong_pack.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(61), stable_symbol_hash("Values")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_pack.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_pack.node(callee).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    wrong_pack.push_evidence(library_api_record_with_provenance(
        2,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
        BUILTIN_COMPAT_PACK_ID,
        PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    assert!(
        admitted_imported_collection_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Python stdlib collection factory evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, callee) = python_deque_factory_call_il();
    wrong_producer.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(61), stable_symbol_hash("Values")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    wrong_producer.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(wrong_producer.node(callee).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    wrong_producer.push_evidence(library_api_record_with_provenance(
        2,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
        PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
        "wrong.python.stdlib.collection-factory-api",
    ));
    assert!(
        admitted_imported_collection_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Python stdlib collection factory evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, callee) = python_deque_factory_call_il();
    admitted.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(61), stable_symbol_hash("Values")),
        imported_binding,
        EvidenceStatus::Asserted,
    ));
    admitted.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        imported_binding,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    admitted.push_evidence(python_stdlib_collection_factory_record(
        2,
        admitted.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence =
        admitted_imported_collection_factory_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::PythonImportedCollectionFactory
    );
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_ruby_set_factory_resolver_requires_pack_provenance() {
    let (il, interner, call, _receiver) = ruby_set_factory_call_il();
    assert!(
        admitted_ruby_set_factory_at_call(&il, &interner, call).is_none(),
        "raw Ruby Set.new(...) call shape alone must not admit stdlib Set semantics"
    );

    let contract =
        library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).expect("Set.new contract");

    let (mut missing_dependency, interner, call, _receiver) = ruby_set_factory_call_il();
    missing_dependency.push_evidence(ruby_stdlib_set_record(
        0,
        missing_dependency.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_ruby_set_factory_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Ruby Set.new evidence without Set/require dependencies is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = ruby_set_factory_call_il();
    push_ruby_set_require_dependencies(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        3,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
        BUILTIN_COMPAT_PACK_ID,
        RUBY_STDLIB_SET_PRODUCER_ID,
    ));
    assert!(
        admitted_ruby_set_factory_at_call(&wrong_pack, &interner, call).is_none(),
        "Ruby Set.new evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = ruby_set_factory_call_il();
    push_ruby_set_require_dependencies(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        3,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
        RUBY_STDLIB_SET_PACK_ID,
        "wrong.ruby.stdlib.set-factory-api",
    ));
    assert!(
        admitted_ruby_set_factory_at_call(&wrong_producer, &interner, call).is_none(),
        "Ruby Set.new evidence with the wrong producer is rejected"
    );

    let (mut admitted, interner, call, receiver) = ruby_set_factory_call_il();
    push_ruby_set_require_dependencies(&mut admitted, receiver);
    admitted.push_evidence(ruby_stdlib_set_record(
        3,
        admitted.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));

    let occurrence = admitted_ruby_set_factory_at_call(&admitted, &interner, call).unwrap();
    let field_callee = admitted.children(call)[0];
    assert_eq!(occurrence.contract.id, LibraryApiContractId::RubySetFactory);
    assert_eq!(occurrence.callee, field_callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}
