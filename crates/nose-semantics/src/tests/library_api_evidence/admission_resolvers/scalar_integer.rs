use super::*;

fn rust_integer_method_call_il(method: &str, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(120), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(121),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Lit,
                Payload::LitInt(idx as i64),
                sp(122 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(130), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(131), &[call]);
    (finish_il(b, root, Lang::Rust), interner, call, receiver)
}

fn push_integer_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Integer),
        EvidenceStatus::Asserted,
    ));
}

fn java_math_call_il(method: &str, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let math = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Math")),
        sp(140),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(141),
        &[math],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(idx as u32),
                sp(142 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(143), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(144), &[call]);
    (finish_il(b, root, Lang::Java), interner, call, math)
}

fn push_java_math_receiver_dependency(il: &mut Il, math: NodeId) {
    il.push_evidence(language_core_symbol_record(
        0,
        EvidenceAnchor::node(il.node(math).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Java,
    ));
}

fn push_java_math_arg_dependencies(il: &mut Il, call: NodeId, first_id: u32) -> Vec<u32> {
    let args = il.children(call)[1..].to_vec();
    args.into_iter()
        .enumerate()
        .map(|(idx, arg)| {
            let id = first_id + idx as u32;
            il.push_evidence(evidence(
                id,
                EvidenceAnchor::node(il.node(arg).span, il.kind(arg)),
                EvidenceKind::Domain(DomainEvidence::Integer),
                EvidenceStatus::Asserted,
            ));
            id
        })
        .collect()
}

fn push_java_math_dependencies(il: &mut Il, call: NodeId, math: NodeId) -> Vec<u32> {
    push_java_math_receiver_dependency(il, math);
    let mut dependencies = vec![0];
    dependencies.extend(push_java_math_arg_dependencies(il, call, 1));
    dependencies
}

#[test]
fn admitted_rust_scalar_integer_method_requires_integer_method_builtin_pack_provenance() {
    let (mut raw_shape, interner, call, receiver) = rust_integer_method_call_il("clamp", 2);
    push_integer_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_scalar_integer_method_at_call(&raw_shape, &interner, call).is_none(),
        "raw Rust integer method shape plus integer receiver proof is not enough"
    );

    let contract =
        library_scalar_integer_method_contract(Lang::Rust, "clamp", 2).expect("clamp contract");

    let (mut missing_dependency, interner, call, _receiver) =
        rust_integer_method_call_il("clamp", 2);
    missing_dependency.push_evidence(rust_stdlib_integer_method_record(
        1,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span Rust integer method evidence without integer receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = rust_integer_method_call_il("clamp", 2);
    push_integer_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_pack, &interner, call).is_none(),
        "Rust integer method evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = rust_integer_method_call_il("clamp", 2);
    push_integer_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        1,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[0],
        RUST_STDLIB_INTEGER_METHOD_PACK_ID,
        "wrong.rust.stdlib.integer-method-api",
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_producer, &interner, call).is_none(),
        "Rust integer method evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, receiver) = rust_integer_method_call_il("clamp", 2);
    push_integer_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = rust_stdlib_integer_method_record(
        1,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_emitter, &interner, call).is_none(),
        "Rust integer method evidence from an external emitter is rejected"
    );

    let (mut admitted, interner, call, receiver) = rust_integer_method_call_il("clamp", 2);
    push_integer_receiver_dependency(&mut admitted, receiver);
    admitted.push_evidence(rust_stdlib_integer_method_record(
        1,
        admitted.node(call).span,
        contract.id,
        contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = admitted_scalar_integer_method_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Clamp)
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

fn assert_admitted_java_math_method(method: &str, arg_count: usize, semantic: ScalarIntegerMethod) {
    let (mut il, interner, call, math) = java_math_call_il(method, arg_count);
    let dependencies = push_java_math_dependencies(&mut il, call, math);
    let contract = library_scalar_integer_method_contract(Lang::Java, method, arg_count)
        .expect("Java Math scalar integer contract");
    il.push_evidence(java_stdlib_math_record(
        10,
        il.node(call).span,
        contract.id,
        contract.callee,
        arg_count as u16,
        EvidenceStatus::Asserted,
        &dependencies,
    ));
    let occurrence = admitted_scalar_integer_method_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::ScalarIntegerMethod(semantic)
    );
    assert_eq!(occurrence.receiver, Some(math));
    assert_eq!(occurrence.arg_count, arg_count);
}

#[test]
fn admitted_java_scalar_integer_method_requires_math_builtin_pack_provenance() {
    let (mut raw_shape, interner, call, math) = java_math_call_il("abs", 1);
    push_java_math_receiver_dependency(&mut raw_shape, math);
    assert!(
        admitted_scalar_integer_method_at_call(&raw_shape, &interner, call).is_none(),
        "raw Java Math method shape plus unshadowed Math proof is not enough"
    );

    let contract = library_scalar_integer_method_contract(Lang::Java, "abs", 1)
        .expect("Java Math.abs contract");

    let (mut missing_dependency, interner, call, _math) = java_math_call_il("abs", 1);
    missing_dependency.push_evidence(java_stdlib_math_record(
        1,
        missing_dependency.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&missing_dependency, &interner, call).is_none(),
        "Java Math evidence without unshadowed Math proof is rejected"
    );

    let (mut missing_arg_domain, interner, call, math) = java_math_call_il("abs", 1);
    push_java_math_receiver_dependency(&mut missing_arg_domain, math);
    missing_arg_domain.push_evidence(java_stdlib_math_record(
        1,
        missing_arg_domain.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&missing_arg_domain, &interner, call).is_none(),
        "Java Math evidence without integer-domain argument proof is rejected"
    );

    let (mut wrong_pack, interner, call, math) = java_math_call_il("abs", 1);
    let dependencies = push_java_math_dependencies(&mut wrong_pack, call, math);
    wrong_pack.push_evidence(library_api_record_with_provenance(
        10,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &dependencies,
        BUILTIN_COMPAT_PACK_ID,
        JAVA_STDLIB_MATH_PRODUCER_ID,
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_pack, &interner, call).is_none(),
        "Java Math evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, math) = java_math_call_il("abs", 1);
    let dependencies = push_java_math_dependencies(&mut wrong_producer, call, math);
    wrong_producer.push_evidence(library_api_record_with_provenance(
        10,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &dependencies,
        JAVA_STDLIB_MATH_PACK_ID,
        "wrong.java.stdlib.math-api",
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_producer, &interner, call).is_none(),
        "Java Math evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, math) = java_math_call_il("abs", 1);
    let dependencies = push_java_math_dependencies(&mut wrong_emitter, call, math);
    let mut external_record = java_stdlib_math_record(
        10,
        wrong_emitter.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &dependencies,
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.push_evidence(external_record);
    assert!(
        admitted_scalar_integer_method_at_call(&wrong_emitter, &interner, call).is_none(),
        "Java Math evidence from an external emitter is rejected"
    );

    assert_admitted_java_math_method("abs", 1, ScalarIntegerMethod::Abs);
    assert_admitted_java_math_method("min", 2, ScalarIntegerMethod::Min);
    assert_admitted_java_math_method("max", 2, ScalarIntegerMethod::Max);
}

#[test]
fn java_math_receiver_dependency_rejects_conflicting_symbol_identity() {
    let (mut il, interner, call, math) = java_math_call_il("abs", 1);
    push_java_math_receiver_dependency(&mut il, math);
    let arg_dependencies = push_java_math_arg_dependencies(&mut il, call, 1);
    let contract = library_scalar_integer_method_contract(Lang::Java, "abs", 1)
        .expect("Java Math.abs contract");
    assert_eq!(
        library_api_receiver_dependencies_for_call(&il, &interner, call, contract.callee),
        Some(
            std::iter::once(EvidenceId(0))
                .chain(arg_dependencies.iter().copied().map(EvidenceId))
                .collect()
        )
    );

    il.push_evidence(language_core_symbol_record(
        9,
        EvidenceAnchor::node(il.node(math).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("NotMath"),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Java,
    ));
    assert!(
        library_api_receiver_dependencies_for_call(&il, &interner, call, contract.callee).is_none(),
        "conflicting same-anchor language-core symbol evidence must close receiver dependency generation"
    );
}

#[test]
fn forged_java_scalar_integer_method_evidence_does_not_open_unsupported_arity() {
    let contract = library_scalar_integer_method_contract(Lang::Java, "abs", 1)
        .expect("Java Math.abs contract");
    let (mut unsupported_arity, interner, call, math) = java_math_call_il("abs", 2);
    let dependencies = push_java_math_dependencies(&mut unsupported_arity, call, math);
    unsupported_arity.push_evidence(java_stdlib_math_record(
        10,
        unsupported_arity.node(call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &dependencies,
    ));
    assert!(
        admitted_scalar_integer_method_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged Java Math evidence cannot open unsupported source arity"
    );
}
