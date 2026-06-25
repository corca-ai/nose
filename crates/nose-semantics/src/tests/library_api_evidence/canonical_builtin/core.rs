use super::*;

#[test]
fn canonical_builtin_admission_requires_language_core_or_library_api_evidence() {
    let (mut il, call) = python_len_canonical_call_il();

    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Len));

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    push_canonical_unshadowed_symbol_dependency(&mut il, 9, call, "len");
    il.evidence.push(free_function_builtin_protocol_record(
        10,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));
    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Abs));
}

#[test]
fn canonical_builtin_admission_rejects_broad_unshadowed_symbol_dependency() {
    let (mut il, call) = python_len_canonical_call_il();
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(evidence(
        9,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(free_function_builtin_protocol_record(
        10,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));

    assert!(
        !admitted_builtin_semantics_at_call(&il, call, Builtin::Len),
        "canonical free-name builtin API evidence must reject broad symbol dependencies"
    );
}

#[test]
fn canonical_builtin_admission_requires_language_core_namespace_dependency() {
    let interner = Interner::new();
    let contract =
        library_method_call_contract(Lang::Go, "Println", 1).expect("Go fmt.Println contract");

    let (mut broad_namespace, call) = go_print_canonical_call_il();
    let namespace = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("fmt"),
    });
    broad_namespace.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(48), stable_symbol_hash("fmt")),
        namespace,
        EvidenceStatus::Asserted,
    ));
    broad_namespace.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(broad_namespace.node(call).span, NodeKind::Var),
        namespace,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    broad_namespace
        .evidence
        .push(builtin_method_call_protocol_record(
            2,
            broad_namespace.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[1],
        ));
    assert!(
        !admitted_builtin_semantics_at_call_with_interner(
            &broad_namespace,
            &interner,
            call,
            Builtin::Print
        ),
        "canonical namespace builtin API evidence must reject broad namespace dependencies"
    );

    let (mut wrong_pack, call) = go_print_canonical_call_il();
    push_canonical_imported_namespace_dependency(&mut wrong_pack, 0, 1, call, "fmt");
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
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        !admitted_builtin_semantics_at_call_with_interner(
            &wrong_pack,
            &interner,
            call,
            Builtin::Print
        ),
        "Go namespace builtins must require Go stdlib namespace-call pack provenance"
    );

    let (mut admitted, call) = go_print_canonical_call_il();
    push_canonical_imported_namespace_dependency(&mut admitted, 0, 1, call, "fmt");
    admitted.evidence.push(builtin_method_call_protocol_record(
        2,
        admitted.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert!(admitted_builtin_semantics_at_call_with_interner(
        &admitted,
        &interner,
        call,
        Builtin::Print
    ));
}

#[test]
fn canonical_builtin_admission_requires_import_backed_namespace_dependency() {
    let interner = Interner::new();
    let contract =
        library_method_call_contract(Lang::Go, "Println", 1).expect("Go fmt.Println contract");
    let symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("fmt"),
    };

    let (mut missing_binding, call) = go_print_canonical_call_il();
    missing_binding.evidence.push(language_core_symbol_record(
        0,
        EvidenceAnchor::node(missing_binding.node(call).span, NodeKind::Var),
        symbol,
        EvidenceStatus::Asserted,
        &[],
        Lang::Go,
    ));
    missing_binding
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            missing_binding.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        !admitted_builtin_semantics_at_call_with_interner(
            &missing_binding,
            &interner,
            call,
            Builtin::Print
        ),
        "canonical namespace builtin API evidence must reject occurrence symbols without import bindings"
    );
}

#[test]
fn go_strings_contains_canonical_builtin_requires_namespace_pack_provenance() {
    let interner = Interner::new();
    let contract = library_method_call_contracts(Lang::Go, "Contains", 2)
        .into_iter()
        .find(|contract| {
            contract.result.semantic == MethodSemanticContract::Builtin(Builtin::StringContains)
        })
        .expect("Go strings.Contains contract");
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(50), &[]);
    let needle = b.add(NodeKind::Var, Payload::Cid(1), sp(51), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::StringContains),
        sp(40),
        &[value, needle],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(41), &[call]);
    let mut il = finish_il(b, root, Lang::Go);
    push_canonical_imported_namespace_dependency(&mut il, 0, 1, call, "strings");
    il.evidence.push(builtin_method_call_protocol_record(
        2,
        il.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[1],
    ));

    assert!(admitted_builtin_semantics_at_call_with_interner(
        &il,
        &interner,
        call,
        Builtin::StringContains
    ));
    assert!(!admitted_builtin_semantics_at_call_with_interner(
        &il,
        &interner,
        call,
        Builtin::Contains
    ));
}

#[test]
fn canonical_builtin_admission_rejects_namespace_dependency_from_other_call() {
    let interner = Interner::new();
    let contract =
        library_method_call_contract(Lang::Go, "Println", 1).expect("Go fmt.Println contract");
    let mut b = IlBuilder::new(FileId(0));
    let first_arg = b.add(NodeKind::Var, Payload::Cid(0), sp(49), &[]);
    let first_call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Print),
        sp(40),
        &[first_arg],
    );
    let second_arg = b.add(NodeKind::Var, Payload::Cid(1), sp(69), &[]);
    let second_call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Print),
        sp(60),
        &[second_arg],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(70),
        &[first_call, second_call],
    );
    let mut il = finish_il(b, root, Lang::Go);
    push_canonical_imported_namespace_dependency(&mut il, 0, 1, first_call, "fmt");
    il.evidence.push(builtin_method_call_protocol_record(
        2,
        il.node(second_call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));

    assert!(
        !admitted_builtin_semantics_at_call_with_interner(
            &il,
            &interner,
            second_call,
            Builtin::Print
        ),
        "canonical namespace builtin API evidence must depend on this call's namespace occurrence"
    );
}

#[test]
fn rust_integer_canonical_builtin_requires_integer_method_pack_provenance() {
    for (builtin, method, source_arity, canonical_arg_count) in [
        (Builtin::Abs, "abs", 0, 1),
        (Builtin::Min, "min", 1, 2),
        (Builtin::Max, "max", 1, 2),
    ] {
        let contract = library_scalar_integer_method_contract(Lang::Rust, method, source_arity)
            .expect("Rust integer method contract");

        let (mut wrong_pack, call) =
            rust_integer_canonical_builtin_call_il(builtin, canonical_arg_count);
        let receiver = wrong_pack.children(call)[0];
        wrong_pack.evidence.push(evidence(
            0,
            EvidenceAnchor::node(wrong_pack.node(receiver).span, wrong_pack.kind(receiver)),
            EvidenceKind::Domain(DomainEvidence::Integer),
            EvidenceStatus::Asserted,
        ));
        wrong_pack
            .evidence
            .push(library_api_record_with_provenance_and_arity(
                1,
                wrong_pack.node(call).span,
                contract.id,
                contract.callee,
                source_arity as u16,
                EvidenceStatus::Asserted,
                &[0],
                BUILTIN_COMPAT_PACK_ID,
                RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
            ));
        assert!(
            !admitted_builtin_semantics_at_call(&wrong_pack, call, builtin),
            "canonical Rust {method} builtin must reject compatibility-pack evidence"
        );

        let (mut admitted, call) =
            rust_integer_canonical_builtin_call_il(builtin, canonical_arg_count);
        let receiver = admitted.children(call)[0];
        admitted.evidence.push(evidence(
            0,
            EvidenceAnchor::node(admitted.node(receiver).span, admitted.kind(receiver)),
            EvidenceKind::Domain(DomainEvidence::Integer),
            EvidenceStatus::Asserted,
        ));
        admitted.evidence.push(rust_stdlib_integer_method_record(
            1,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            source_arity as u16,
            EvidenceStatus::Asserted,
            &[0],
        ));
        assert!(
            admitted_builtin_semantics_at_call(&admitted, call, builtin),
            "canonical Rust {method} builtin should admit the builtin-pack evidence"
        );
    }
}

#[test]
fn java_math_canonical_builtin_requires_math_pack_provenance() {
    for (builtin, method, source_arity, canonical_arg_count) in [
        (Builtin::Abs, "abs", 1, 1),
        (Builtin::Min, "min", 2, 2),
        (Builtin::Max, "max", 2, 2),
    ] {
        let contract = library_scalar_integer_method_contract(Lang::Java, method, source_arity)
            .expect("Java Math integer method contract");

        let (mut missing_dependency, call) =
            java_math_canonical_builtin_call_il(builtin, canonical_arg_count);
        missing_dependency.evidence.push(java_stdlib_math_record(
            1,
            missing_dependency.node(call).span,
            contract.id,
            contract.callee,
            source_arity as u16,
            EvidenceStatus::Asserted,
            &[],
        ));
        assert!(
            !admitted_builtin_semantics_at_call(&missing_dependency, call, builtin),
            "canonical Java Math {method} builtin must reject evidence without Math/integer dependencies"
        );

        let (mut wrong_pack, call) =
            java_math_canonical_builtin_call_il(builtin, canonical_arg_count);
        let dependencies = push_java_math_canonical_dependencies(&mut wrong_pack, call);
        wrong_pack
            .evidence
            .push(library_api_record_with_provenance_and_arity(
                10,
                wrong_pack.node(call).span,
                contract.id,
                contract.callee,
                source_arity as u16,
                EvidenceStatus::Asserted,
                &dependencies,
                BUILTIN_COMPAT_PACK_ID,
                JAVA_STDLIB_MATH_PRODUCER_ID,
            ));
        assert!(
            !admitted_builtin_semantics_at_call(&wrong_pack, call, builtin),
            "canonical Java Math {method} builtin must reject compatibility-pack evidence"
        );

        let LibraryApiCalleeContract::Method {
            method: callee_method,
            ..
        } = contract.callee
        else {
            unreachable!("Java Math contract is a method contract");
        };
        let forged_callee = LibraryApiCalleeContract::Method {
            method: callee_method,
            receiver: MethodReceiverContract::ExactInteger,
        };
        let (mut unresolved_callee_il, call) =
            java_math_canonical_builtin_call_il(builtin, canonical_arg_count);
        let dependencies = push_java_math_canonical_dependencies(&mut unresolved_callee_il, call);
        unresolved_callee_il.evidence.push(java_stdlib_math_record(
            10,
            unresolved_callee_il.node(call).span,
            contract.id,
            forged_callee,
            source_arity as u16,
            EvidenceStatus::Asserted,
            &dependencies,
        ));
        assert!(
            !admitted_builtin_semantics_at_call(&unresolved_callee_il, call, builtin),
            "canonical Java Math {method} builtin must reject unresolved callee hashes"
        );

        let (mut admitted, call) =
            java_math_canonical_builtin_call_il(builtin, canonical_arg_count);
        let dependencies = push_java_math_canonical_dependencies(&mut admitted, call);
        admitted.evidence.push(java_stdlib_math_record(
            10,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            source_arity as u16,
            EvidenceStatus::Asserted,
            &dependencies,
        ));
        assert!(
            admitted_builtin_semantics_at_call(&admitted, call, builtin),
            "canonical Java Math {method} builtin should admit the math-pack evidence"
        );
    }
}

#[test]
fn rust_map_get_unwrap_or_canonical_builtin_uses_map_get_dependency() {
    let mut b = IlBuilder::new(FileId(0));
    let map = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(39), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(40), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[map, key, default],
        b,
        map,
    );
    let map_get = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map get contract");
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.evidence.push(evidence(
        9,
        EvidenceAnchor::node(il.node(map).span, il.kind(map)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(map_get_protocol_record(
        10,
        sp(39),
        map_get,
        EvidenceStatus::Asserted,
        &[9],
    ));
    il.evidence.push(builtin_method_call_protocol_record(
        11,
        il.node(call).span,
        unwrap_or,
        1,
        EvidenceStatus::Asserted,
        &[10],
    ));

    assert!(admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::GetOrDefault
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::ValueOrDefault
    ));
}

#[test]
fn rust_map_get_unwrap_or_canonical_builtin_rejects_nested_map_get_arity_drift() {
    let mut b = IlBuilder::new(FileId(0));
    let map = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(39), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(40), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[map, key, default],
        b,
        map,
    );
    let map_get = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map get contract");
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.evidence.push(evidence(
        9,
        EvidenceAnchor::node(il.node(map).span, il.kind(map)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(map_get_protocol_record_with_arity(
        10,
        sp(39),
        map_get,
        2,
        EvidenceStatus::Asserted,
        &[9],
    ));
    il.evidence.push(builtin_method_call_protocol_record(
        11,
        il.node(call).span,
        unwrap_or,
        1,
        EvidenceStatus::Asserted,
        &[10],
    ));

    assert!(
        !admitted_builtin_semantics_at_call(&il, call, Builtin::GetOrDefault),
        "canonical Rust map defaulting must reject nested MapGet evidence with unsupported arity"
    );
}

#[test]
fn rust_map_get_unwrap_or_canonical_builtin_rejects_unrelated_map_dependency() {
    let mut b = IlBuilder::new(FileId(0));
    let unrelated_map = b.add(NodeKind::Var, Payload::Cid(99), sp(37), &[]);
    let map = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(39), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(40), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[map, key, default],
        b,
        map,
    );
    let map_get = library_map_get_contract(Lang::Rust, "get", 1).expect("Rust map get contract");
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.evidence.push(evidence(
        9,
        EvidenceAnchor::node(il.node(unrelated_map).span, il.kind(unrelated_map)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(map_get_protocol_record(
        10,
        sp(39),
        map_get,
        EvidenceStatus::Asserted,
        &[9],
    ));
    il.evidence.push(builtin_method_call_protocol_record(
        11,
        il.node(call).span,
        unwrap_or,
        1,
        EvidenceStatus::Asserted,
        &[10],
    ));

    assert!(
        !admitted_builtin_semantics_at_call(&il, call, Builtin::GetOrDefault),
        "canonical Rust map defaulting must reject nested MapGet evidence whose map proof is for another receiver"
    );
}
