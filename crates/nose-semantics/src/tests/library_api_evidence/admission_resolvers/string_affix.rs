use super::*;

fn string_affix_call_il(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let (il, interner, call, _callee, receiver) =
        receiver_method_call_il(lang, method, arg_count, 190);
    (il, interner, call, receiver)
}

fn go_namespace_string_affix_call_il(method: &str) -> (Il, Interner, NodeId, NodeId) {
    go_namespace_string_affix_call_il_with_arg_count(method, 2)
}

fn go_namespace_string_affix_call_il_with_arg_count(
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let strings = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("strings")),
        sp(210),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(211),
        &[strings],
    );
    let mut children = vec![callee];
    for idx in 0..arg_count {
        children.push(b.add(
            NodeKind::Var,
            Payload::Cid(idx as u32),
            sp(212 + idx as u32),
            &[],
        ));
    }
    let call = b.add(NodeKind::Call, Payload::None, sp(214), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(215), &[call]);
    (finish_il(b, root, Lang::Go), interner, call, strings)
}

fn push_receiver_domain_dependency(il: &mut Il, receiver: NodeId, domain: DomainEvidence) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
}

fn push_string_receiver_dependency(il: &mut Il, receiver: NodeId) {
    push_receiver_domain_dependency(il, receiver, DomainEvidence::String);
}

fn push_imported_namespace_dependency(
    il: &mut Il,
    receiver: NodeId,
    module: &str,
    dependency_id: u32,
    lang: Lang,
) -> u32 {
    let symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    let occurrence_id = dependency_id + 1;
    il.evidence.push(language_core_symbol_record(
        dependency_id,
        EvidenceAnchor::binding(
            sp(il.node(receiver).span.start_byte.saturating_sub(1)),
            stable_symbol_hash(module),
        ),
        symbol,
        EvidenceStatus::Asserted,
        &[],
        lang,
    ));
    il.evidence.push(language_core_symbol_record(
        occurrence_id,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        symbol,
        EvidenceStatus::Asserted,
        &[dependency_id],
        lang,
    ));
    occurrence_id
}

fn assert_admitted_string_affix(lang: Lang, method: &str, builtin: Builtin) {
    let (mut il, interner, call, receiver) = string_affix_call_il(lang, method, 1);
    push_string_receiver_dependency(&mut il, receiver);
    let contract =
        library_method_call_contract(lang, method, 1).expect("string affix method contract");
    il.evidence.push(builtin_method_call_protocol_record(
        1,
        il.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence =
        admitted_library_method_call_at_call(&il, &interner, call).expect("affix admitted");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(builtin))
    );
    assert_eq!(
        occurrence.contract.pack_id,
        STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

fn assert_admitted_go_namespace_string_affix(method: &str, builtin: Builtin) {
    let (mut il, interner, call, receiver) = go_namespace_string_affix_call_il(method);
    let namespace_dependency =
        push_imported_namespace_dependency(&mut il, receiver, "strings", 0, Lang::Go);
    let contract =
        library_method_call_contract(Lang::Go, method, 2).expect("Go string affix contract");
    il.evidence.push(builtin_method_call_protocol_record(
        1,
        il.node(call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[namespace_dependency],
    ));

    let occurrence =
        admitted_library_method_call_at_call(&il, &interner, call).expect("Go affix admitted");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(builtin))
    );
    assert_eq!(
        occurrence.contract.pack_id,
        STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 2);
}

#[test]
fn admitted_string_affix_requires_protocol_pack_and_string_receiver_proof() {
    let (mut raw_shape, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw startswith shape plus string receiver proof is not enough"
    );

    let contract = library_method_call_contract(Lang::Python, "startswith", 1)
        .expect("Python startswith contract");

    let (mut missing_dependency, interner, call, _receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    missing_dependency
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "affix evidence without exact string receiver proof is rejected"
    );

    let (mut wrong_domain, interner, call, receiver) =
        string_affix_call_il(Lang::JavaScript, "startsWith", 1);
    push_receiver_domain_dependency(&mut wrong_domain, receiver, DomainEvidence::Collection);
    let js_prefix_contract = library_method_call_contract(Lang::JavaScript, "startsWith", 1)
        .expect("JavaScript startsWith contract");
    wrong_domain
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            wrong_domain.node(call).span,
            js_prefix_contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_domain, &interner, call).is_none(),
        "affix evidence with a non-string receiver domain is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "string affix evidence under the broad builtin-method pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            STRING_AFFIX_PREDICATE_PROTOCOL_PACK_ID,
            "wrong.protocols.string-affix-predicate-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "string affix evidence with the wrong producer provenance is rejected"
    );

    let (mut wrong_direction, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 1);
    push_string_receiver_dependency(&mut wrong_direction, receiver);
    let suffix_contract = library_method_call_contract(Lang::Python, "endswith", 1)
        .expect("Python endswith contract");
    wrong_direction
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            wrong_direction.node(call).span,
            suffix_contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_direction, &interner, call).is_none(),
        "forged suffix evidence cannot admit a prefix source call"
    );

    let (mut unsupported_arity, interner, call, receiver) =
        string_affix_call_il(Lang::Python, "startswith", 2);
    push_string_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged affix evidence cannot open unsupported arity"
    );

    let (mut unsupported_offset, interner, call, receiver) =
        string_affix_call_il(Lang::JavaScript, "startsWith", 2);
    push_string_receiver_dependency(&mut unsupported_offset, receiver);
    unsupported_offset
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            unsupported_offset.node(call).span,
            js_prefix_contract,
            1,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_offset, &interner, call).is_none(),
        "forged affix evidence cannot open the JS offset argument form"
    );

    assert_admitted_string_affix(Lang::Python, "startswith", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Python, "endswith", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::Java, "startsWith", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Java, "endsWith", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::Rust, "starts_with", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Rust, "ends_with", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::Swift, "hasPrefix", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::Swift, "hasSuffix", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::TypeScript, "startsWith", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::TypeScript, "endsWith", Builtin::EndsWith);
    assert_admitted_string_affix(Lang::JavaScript, "startsWith", Builtin::StartsWith);
    assert_admitted_string_affix(Lang::JavaScript, "endsWith", Builtin::EndsWith);
}

#[test]
fn admitted_go_namespace_string_affix_requires_string_affix_pack_and_imported_namespace_proof() {
    let (mut raw_shape, interner, call, receiver) = go_namespace_string_affix_call_il("HasPrefix");
    push_imported_namespace_dependency(&mut raw_shape, receiver, "strings", 0, Lang::Go);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw Go strings.HasPrefix shape plus namespace proof is not enough"
    );

    let contract =
        library_method_call_contract(Lang::Go, "HasPrefix", 2).expect("Go HasPrefix contract");

    let (mut missing_dependency, interner, call, _receiver) =
        go_namespace_string_affix_call_il("HasPrefix");
    missing_dependency
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            2,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "Go affix evidence without imported strings namespace proof is rejected"
    );

    let (mut wrong_namespace, interner, call, receiver) =
        go_namespace_string_affix_call_il("HasPrefix");
    let namespace_dependency =
        push_imported_namespace_dependency(&mut wrong_namespace, receiver, "slices", 0, Lang::Go);
    wrong_namespace
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            wrong_namespace.node(call).span,
            contract,
            2,
            EvidenceStatus::Asserted,
            &[namespace_dependency],
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_namespace, &interner, call).is_none(),
        "Go affix evidence with a wrong imported namespace is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = go_namespace_string_affix_call_il("HasPrefix");
    let namespace_dependency =
        push_imported_namespace_dependency(&mut wrong_pack, receiver, "strings", 0, Lang::Go);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[namespace_dependency],
            GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "Go affix evidence under the old namespace-call pack is rejected"
    );

    let (mut wrong_direction, interner, call, receiver) =
        go_namespace_string_affix_call_il("HasPrefix");
    let namespace_dependency =
        push_imported_namespace_dependency(&mut wrong_direction, receiver, "strings", 0, Lang::Go);
    let suffix_contract =
        library_method_call_contract(Lang::Go, "HasSuffix", 2).expect("Go HasSuffix contract");
    wrong_direction
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            wrong_direction.node(call).span,
            suffix_contract,
            2,
            EvidenceStatus::Asserted,
            &[namespace_dependency],
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_direction, &interner, call).is_none(),
        "forged Go suffix evidence cannot admit a prefix source call"
    );

    let (mut unsupported_arity, interner, call, receiver) =
        go_namespace_string_affix_call_il_with_arg_count("HasPrefix", 1);
    let namespace_dependency = push_imported_namespace_dependency(
        &mut unsupported_arity,
        receiver,
        "strings",
        0,
        Lang::Go,
    );
    unsupported_arity
        .evidence
        .push(builtin_method_call_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            2,
            EvidenceStatus::Asserted,
            &[namespace_dependency],
        ));
    assert!(
        admitted_library_method_call_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged Go affix evidence cannot open unsupported source arity"
    );

    assert_admitted_go_namespace_string_affix("HasPrefix", Builtin::StartsWith);
    assert_admitted_go_namespace_string_affix("HasSuffix", Builtin::EndsWith);
}
