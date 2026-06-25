use super::*;

fn rust_result_predicate_call_il(method: &str) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(621), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(622),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(623), &[callee]);
    let root = b.add(NodeKind::Func, Payload::None, sp(624), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, receiver)
}

#[test]
fn admitted_rust_result_predicate_requires_result_receiver_and_pack_provenance() {
    let (il, interner, call, _receiver) = rust_result_predicate_call_il("is_ok");
    assert!(
        admitted_rust_result_predicate_at_call(&il, &interner, call).is_none(),
        "raw Rust is_ok call shape alone must not admit Result semantics"
    );

    let contract = library_rust_result_predicate_contract(Lang::Rust, "is_ok", 0)
        .expect("Rust Result is_ok contract");
    let (mut wrong_domain, interner, call, receiver) = rust_result_predicate_call_il("is_ok");
    wrong_domain.evidence.push(evidence(
        0,
        EvidenceAnchor::node(wrong_domain.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
        EvidenceStatus::Asserted,
    ));
    wrong_domain.evidence.push(rust_stdlib_result_record(
        1,
        wrong_domain.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_rust_result_predicate_at_call(&wrong_domain, &interner, call).is_none(),
        "Rust Result predicate must not admit an Option receiver dependency"
    );

    let (mut wrong_pack, interner, call, receiver) = rust_result_predicate_call_il("is_ok");
    wrong_pack.evidence.push(evidence(
        0,
        EvidenceAnchor::node(wrong_pack.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Result),
        EvidenceStatus::Asserted,
    ));
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_RESULT_PRODUCER_ID,
    ));
    assert!(
        admitted_rust_result_predicate_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust Result predicate evidence under the compatibility pack is rejected"
    );

    let (mut admitted, interner, call, receiver) = rust_result_predicate_call_il("is_ok");
    admitted.evidence.push(evidence(
        0,
        EvidenceAnchor::node(admitted.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Result),
        EvidenceStatus::Asserted,
    ));
    admitted.evidence.push(rust_stdlib_result_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_rust_result_predicate_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(occurrence.contract.id, LibraryApiContractId::RustResultIsOk);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 0);
}

#[test]
fn admitted_rust_option_and_then_resolver_requires_pack_provenance() {
    let (il, interner, call, _receiver) = rust_option_and_then_call_il();
    assert!(
        admitted_rust_option_and_then_at_call(&il, &interner, call).is_none(),
        "raw Rust and_then call shape alone must not admit Option semantics"
    );

    let contract = library_rust_option_and_then_contract(Lang::Rust, "and_then", 1)
        .expect("Rust Option and_then contract");
    let (mut wrong_pack, interner, call, receiver) = rust_option_and_then_call_il();
    wrong_pack.evidence.push(evidence(
        0,
        EvidenceAnchor::node(wrong_pack.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
        EvidenceStatus::Asserted,
    ));
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_OPTION_PRODUCER_ID,
    ));
    assert!(
        admitted_rust_option_and_then_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust Option and_then evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = rust_option_and_then_call_il();
    wrong_producer.evidence.push(evidence(
        0,
        EvidenceAnchor::node(wrong_producer.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
        EvidenceStatus::Asserted,
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
            RUST_STDLIB_OPTION_PACK_ID,
            "wrong.rust.stdlib.option-api",
        ));
    assert!(
        admitted_rust_option_and_then_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust Option and_then evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) = rust_option_and_then_call_il();
    wrong_emitter.evidence.push(evidence(
        0,
        EvidenceAnchor::node(wrong_emitter.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
        EvidenceStatus::Asserted,
    ));
    let mut external_record = rust_stdlib_option_record(
        1,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_rust_option_and_then_at_call(&wrong_emitter, &interner, call).is_none(),
        "Rust Option and_then evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, receiver) = rust_option_and_then_call_il();
    admitted.evidence.push(evidence(
        0,
        EvidenceAnchor::node(admitted.node(receiver).span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
        EvidenceStatus::Asserted,
    ));
    admitted.evidence.push(rust_stdlib_option_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_rust_option_and_then_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::RustOptionAndThen
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_free_function_builtin_resolver_requires_api_occurrence_evidence() {
    let (il, interner, call, _callee) = python_len_builtin_call_il();
    assert!(
        admitted_free_function_builtin_at_call(&il, &interner, call).is_none(),
        "raw Python len(...) call shape alone must not admit builtin semantics"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    let (mut missing_dependency, interner, call, _callee) = python_len_builtin_call_il();
    missing_dependency
        .evidence
        .push(free_function_builtin_protocol_record(
            0,
            missing_dependency.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_free_function_builtin_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span builtin API occurrence without callee dependency is rejected"
    );

    let (mut admitted, interner, call, callee) = python_len_builtin_call_il();
    admitted.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(admitted.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    let (mut wrong_pack, wrong_interner, wrong_call, wrong_callee) = python_len_builtin_call_il();
    wrong_pack.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_pack.node(wrong_callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
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
        FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
    ));
    assert!(
        admitted_free_function_builtin_at_call(&wrong_pack, &wrong_interner, wrong_call).is_none(),
        "free-function builtin API occurrence rejects compatibility-pack evidence"
    );

    let (mut wrong_producer, wrong_interner, wrong_call, wrong_callee) =
        python_len_builtin_call_il();
    wrong_producer.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(wrong_producer.node(wrong_callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            1,
            wrong_producer.node(wrong_call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            "wrong-free-function-producer",
        ));
    assert!(
        admitted_free_function_builtin_at_call(&wrong_producer, &wrong_interner, wrong_call)
            .is_none(),
        "free-function builtin API occurrence rejects wrong producer evidence"
    );

    admitted
        .evidence
        .push(free_function_builtin_protocol_record(
            1,
            admitted.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));

    let occurrence = admitted_free_function_builtin_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(occurrence.contract.id, contract.id);
    assert_eq!(occurrence.contract.result.builtin, Builtin::Len);
    assert_eq!(occurrence.callee, callee);
    assert_eq!(occurrence.receiver, None);
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_free_function_builtin_rejects_broad_symbol_dependency() {
    let (mut il, interner, call, callee) = python_len_builtin_call_il();
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(free_function_builtin_protocol_record(
        1,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));

    assert!(
        admitted_free_function_builtin_at_call(&il, &interner, call).is_none(),
        "broad first-party unshadowed-global symbol evidence must not license builtin API evidence"
    );
}

#[test]
fn admitted_imported_namespace_function_resolver_requires_pack_provenance() {
    let (il, interner, call, _receiver) = python_math_prod_call_il();
    assert!(
        admitted_imported_namespace_function_at_call(&il, &interner, call).is_none(),
        "raw Python math.prod(...) call shape alone must not admit imported namespace semantics"
    );

    let contract = library_imported_namespace_function_contract(Lang::Python, "prod", 1)
        .expect("Python math.prod contract");
    let (mut missing_dependency, interner, call, _receiver) = python_math_prod_call_il();
    missing_dependency.evidence.push(python_stdlib_math_record(
        0,
        missing_dependency.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_imported_namespace_function_at_call(&missing_dependency, &interner, call)
            .is_none(),
        "same-span Python math.prod evidence without namespace dependency is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = python_math_prod_call_il();
    push_python_math_namespace_dependencies(&mut wrong_pack, receiver);
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
            PYTHON_STDLIB_MATH_PRODUCER_ID,
        ));
    assert!(
        admitted_imported_namespace_function_at_call(&wrong_pack, &interner, call).is_none(),
        "Python math.prod evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = python_math_prod_call_il();
    push_python_math_namespace_dependencies(&mut wrong_producer, receiver);
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
            PYTHON_STDLIB_MATH_PACK_ID,
            "wrong.python.stdlib.math-api",
        ));
    assert!(
        admitted_imported_namespace_function_at_call(&wrong_producer, &interner, call).is_none(),
        "Python math.prod evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) = python_math_prod_call_il();
    push_python_math_namespace_dependencies(&mut wrong_emitter, receiver);
    let mut external_record = python_stdlib_math_record(
        2,
        wrong_emitter.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_imported_namespace_function_at_call(&wrong_emitter, &interner, call).is_none(),
        "Python math.prod evidence from an external emitter is rejected"
    );

    let (mut wrong_arity, interner, call, receiver) = python_math_prod_call_il_with_arg_count(3);
    push_python_math_namespace_dependencies(&mut wrong_arity, receiver);
    wrong_arity.evidence.push(python_stdlib_math_record(
        2,
        wrong_arity.node(call).span,
        contract,
        3,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(
        admitted_imported_namespace_function_at_call(&wrong_arity, &interner, call).is_none(),
        "Python math.prod evidence with unsupported arity is rejected"
    );

    let (mut admitted, interner, call, receiver) = python_math_prod_call_il();
    push_python_math_namespace_dependencies(&mut admitted, receiver);
    admitted.evidence.push(python_stdlib_math_record(
        2,
        admitted.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));

    let occurrence =
        admitted_imported_namespace_function_at_call(&admitted, &interner, call).unwrap();
    let field_callee = admitted.children(call)[0];
    assert_eq!(occurrence.contract.id, contract.id);
    assert_eq!(occurrence.callee, field_callee);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_imported_namespace_function_rejects_broad_namespace_dependency() {
    let (mut il, interner, call, receiver) = python_math_prod_call_il();
    let contract = library_imported_namespace_function_contract(Lang::Python, "prod", 1)
        .expect("Python math.prod contract");
    let namespace_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("math"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(66), stable_symbol_hash("math")),
        namespace_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
        namespace_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(python_stdlib_math_record(
        2,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));

    assert!(
        admitted_imported_namespace_function_at_call(&il, &interner, call).is_none(),
        "broad first-party namespace occurrence evidence must not license imported API evidence"
    );
}
