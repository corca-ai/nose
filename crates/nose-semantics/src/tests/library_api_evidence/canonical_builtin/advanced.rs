use super::*;

#[test]
fn rust_value_default_contract_alone_does_not_admit_map_default_builtin() {
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(38), &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(0), sp(39), &[]);
    let (mut il, call) = canonical_builtin_call_il(
        Lang::Rust,
        Builtin::GetOrDefault,
        &[value, default],
        b,
        value,
    );
    let unwrap_or =
        library_method_call_contract(Lang::Rust, "unwrap_or", 1).expect("Rust unwrap_or contract");

    il.push_evidence(library_api_record(
        10,
        il.node(call).span,
        unwrap_or.id,
        unwrap_or.callee,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::GetOrDefault
    ));
}

#[test]
fn canonical_builtin_admission_fails_closed_on_bad_library_api_evidence() {
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");

    let (mut broken, broken_call) = python_len_canonical_call_il();
    broken.push_evidence(free_function_builtin_protocol_record(
        10,
        broken.node(broken_call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[99],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &broken,
        broken_call,
        Builtin::Len
    ));

    let (mut wrong_arity, wrong_arity_call) = python_len_canonical_call_il();
    push_canonical_unshadowed_symbol_dependency(&mut wrong_arity, 9, wrong_arity_call, "len");
    wrong_arity.push_evidence(free_function_builtin_protocol_record(
        10,
        wrong_arity.node(wrong_arity_call).span,
        contract,
        2,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &wrong_arity,
        wrong_arity_call,
        Builtin::Len
    ));

    let (mut wrong_symbol_span, wrong_symbol_span_call) = python_len_canonical_call_il();
    let arg = wrong_symbol_span.children(wrong_symbol_span_call)[0];
    wrong_symbol_span.push_evidence(evidence(
        9,
        EvidenceAnchor::node(
            wrong_symbol_span.node(arg).span,
            wrong_symbol_span.kind(arg),
        ),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        }),
        EvidenceStatus::Asserted,
    ));
    wrong_symbol_span.push_evidence(free_function_builtin_protocol_record(
        10,
        wrong_symbol_span.node(wrong_symbol_span_call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &wrong_symbol_span,
        wrong_symbol_span_call,
        Builtin::Len
    ));

    let (mut ambiguous, ambiguous_call) = python_len_canonical_call_il();
    push_canonical_unshadowed_symbol_dependency(&mut ambiguous, 9, ambiguous_call, "len");
    ambiguous.push_evidence(free_function_builtin_protocol_record(
        10,
        ambiguous.node(ambiguous_call).span,
        contract,
        1,
        EvidenceStatus::Ambiguous,
        &[9],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &ambiguous,
        ambiguous_call,
        Builtin::Len
    ));

    let (mut conflicting, conflicting_call) = python_len_canonical_call_il();
    let abs = library_free_function_builtin_contract(Lang::Python, "abs", 1)
        .expect("Python abs contract");
    push_canonical_unshadowed_symbol_dependency(&mut conflicting, 8, conflicting_call, "len");
    push_canonical_unshadowed_symbol_dependency(&mut conflicting, 9, conflicting_call, "abs");
    conflicting.push_evidence(free_function_builtin_protocol_record(
        10,
        conflicting.node(conflicting_call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[8],
    ));
    conflicting.push_evidence(free_function_builtin_protocol_record(
        11,
        conflicting.node(conflicting_call).span,
        abs,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &conflicting,
        conflicting_call,
        Builtin::Len
    ));
}

#[test]
fn canonical_method_builtin_admission_requires_builtin_method_pack_contract_and_receiver_proof() {
    let mut b = IlBuilder::new(FileId(0));
    let collection = b.add(NodeKind::Var, Payload::Cid(0), sp(71), &[]);
    let (mut admitted, call) =
        canonical_builtin_call_il(Lang::Rust, Builtin::Len, &[collection], b, collection);
    let contract = library_method_call_contract(Lang::Rust, "len", 0).expect("Rust len contract");
    admitted.push_evidence(evidence(
        9,
        EvidenceAnchor::node(admitted.node(collection).span, admitted.kind(collection)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    admitted.push_evidence(builtin_method_call_protocol_record(
        10,
        admitted.node(call).span,
        contract,
        0,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(admitted_builtin_semantics_at_call(
        &admitted,
        call,
        Builtin::Len
    ));

    let mut missing_dependency = admitted.clone();
    missing_dependency.evidence.truncate(1);
    missing_dependency.push_evidence(builtin_method_call_protocol_record(
        10,
        missing_dependency.node(call).span,
        contract,
        0,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&missing_dependency, call, Builtin::Len),
        "generic method builtins must not admit without receiver-domain proof"
    );

    let mut wrong_receiver = admitted.clone();
    wrong_receiver.evidence.clear();
    wrong_receiver.push_evidence(evidence(
        9,
        EvidenceAnchor::node(wrong_receiver.node(call).span, wrong_receiver.kind(call)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    wrong_receiver.push_evidence(builtin_method_call_protocol_record(
        10,
        wrong_receiver.node(call).span,
        contract,
        0,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&wrong_receiver, call, Builtin::Len),
        "receiver-domain proof must belong to the canonical receiver"
    );

    let mut wrong_pack = admitted.clone();
    wrong_pack.evidence.truncate(1);
    wrong_pack.push_evidence(library_api_record_with_provenance_and_arity(
        10,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[9],
        BUILTIN_COMPAT_PACK_ID,
        BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&wrong_pack, call, Builtin::Len),
        "generic method builtins must require builtin-method pack provenance"
    );

    let mut wrong_producer = admitted.clone();
    wrong_producer.evidence.truncate(1);
    wrong_producer.push_evidence(library_api_record_with_provenance_and_arity(
        10,
        wrong_producer.node(call).span,
        contract.id,
        contract.callee,
        0,
        EvidenceStatus::Asserted,
        &[9],
        BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
        "wrong.builtin-method-call-api",
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&wrong_producer, call, Builtin::Len),
        "generic method builtins must require builtin-method producer provenance"
    );

    let mut wrong_arity = admitted.clone();
    wrong_arity.evidence.truncate(1);
    wrong_arity.push_evidence(builtin_method_call_protocol_record(
        10,
        wrong_arity.node(call).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&wrong_arity, call, Builtin::Len),
        "generic method builtins must reject unsupported arity drift"
    );
}

#[test]
fn canonical_property_builtin_admission_accepts_field_span_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let collection = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(40),
        &[collection],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(41), &[call]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    let contract =
        library_property_builtin_contract(Lang::JavaScript, "length").expect("length contract");
    il.push_evidence(property_builtin_record(
        10,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));

    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));

    let mut swift_len = IlBuilder::new(FileId(0));
    let collection = swift_len.add(NodeKind::Var, Payload::Cid(0), sp(50), &[]);
    let count = swift_len.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(51),
        &[collection],
    );
    let root = swift_len.add(NodeKind::Func, Payload::None, sp(52), &[count]);
    let mut swift_len = finish_il(swift_len, root, Lang::Swift);
    let contract = library_property_builtin_contract(Lang::Swift, "count").expect("count contract");
    swift_len.push_evidence(property_builtin_record(
        11,
        swift_len.node(count).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(admitted_builtin_semantics_at_call(
        &swift_len,
        count,
        Builtin::Len
    ));

    let mut swift_empty = IlBuilder::new(FileId(0));
    let collection = swift_empty.add(NodeKind::Var, Payload::Cid(0), sp(60), &[]);
    let is_empty = swift_empty.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::IsEmpty),
        sp(61),
        &[collection],
    );
    let root = swift_empty.add(NodeKind::Func, Payload::None, sp(62), &[is_empty]);
    let mut swift_empty = finish_il(swift_empty, root, Lang::Swift);
    let contract =
        library_property_builtin_contract(Lang::Swift, "isEmpty").expect("isEmpty contract");
    swift_empty.push_evidence(property_builtin_record(
        12,
        swift_empty.node(is_empty).span,
        contract,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(admitted_builtin_semantics_at_call(
        &swift_empty,
        is_empty,
        Builtin::IsEmpty
    ));
}

#[test]
fn canonical_builtin_admission_keeps_language_core_exceptions_narrow() {
    let mut go = IlBuilder::new(FileId(0));
    let key = go.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let map = go.add(NodeKind::Var, Payload::Cid(1), sp(40), &[]);
    let (go_il, contains) =
        canonical_builtin_call_il(Lang::Go, Builtin::Contains, &[key, map], go, map);
    assert!(admitted_builtin_semantics_at_call(
        &go_il,
        contains,
        Builtin::Contains
    ));

    let mut py = IlBuilder::new(FileId(0));
    let dict_key = py.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let dict_value = py.add(NodeKind::Var, Payload::Cid(1), sp(40), &[]);
    let (py_il, dict_entry) = canonical_builtin_call_il(
        Lang::Python,
        Builtin::DictEntry,
        &[dict_key, dict_value],
        py,
        dict_value,
    );
    assert!(admitted_builtin_semantics_at_call(
        &py_il,
        dict_entry,
        Builtin::DictEntry
    ));

    let mut raw_len = IlBuilder::new(FileId(0));
    let arg = raw_len.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let (go_len_il, go_len) =
        canonical_builtin_call_il(Lang::Go, Builtin::Len, &[arg], raw_len, arg);
    assert!(!admitted_builtin_semantics_at_call(
        &go_len_il,
        go_len,
        Builtin::Len
    ));
}

#[test]
fn c_unsigned_cast_builtin_admission_requires_source_cast_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let (mut il, call) =
        canonical_builtin_call_il(Lang::C, Builtin::UnsignedCast32, &[arg], b, arg);

    assert!(!admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::UnsignedCast32
    ));

    il.push_evidence(evidence(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        EvidenceStatus::Asserted,
    ));
    assert!(
        !admitted_builtin_semantics_at_call(&il, call, Builtin::UnsignedCast32),
        "wrong-pack source facts must not admit C unsigned-cast semantics"
    );

    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    let (mut il, call) =
        canonical_builtin_call_il(Lang::C, Builtin::UnsignedCast32, &[arg], b, arg);
    il.push_evidence(c_unsigned_32_source_cast_evidence(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(admitted_builtin_semantics_at_call(
        &il,
        call,
        Builtin::UnsignedCast32
    ));
}

fn add_with_len_rhs_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let px = b.add(NodeKind::Param, Payload::Cid(0), sp(50), &[]);
    let py = b.add(NodeKind::Param, Payload::Cid(1), sp(51), &[]);
    let x = b.add(NodeKind::Var, Payload::Cid(0), sp(52), &[]);
    let y = b.add(NodeKind::Var, Payload::Cid(1), sp(53), &[]);
    let len = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp(54), &[y]);
    let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp(55), &[x, len]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(56), &[add]);
    let root = b.add(NodeKind::Func, Payload::None, sp(57), &[px, py, ret]);
    (finish_il(b, root, Lang::Python), len)
}

#[test]
fn value_domain_inference_requires_admitted_builtin_result_domains() {
    let (mut il, len) = add_with_len_rhs_il();
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, il.root),
        vec![ValueDomain::Unknown, ValueDomain::Unknown],
        "raw canonical Len payload must not prove a numeric result domain"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    push_canonical_unshadowed_symbol_dependency(&mut il, 9, len, "len");
    il.push_evidence(free_function_builtin_protocol_record(
        10,
        il.node(len).span,
        contract,
        1,
        EvidenceStatus::Asserted,
        &[9],
    ));
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, il.root),
        vec![ValueDomain::Number, ValueDomain::Unknown],
        "admitted Python len can contribute its numeric result domain"
    );
}
