use super::*;

mod admission_resolvers;

fn js_array_is_array_call_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let array = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Array")),
        sp(29),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("isArray")),
        sp(30),
        &[array],
    );
    let value = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(31),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(32), &[callee, value]);
    let root = b.add(NodeKind::Module, Payload::None, sp(29), &[call]);
    (finish_il(b, root, Lang::JavaScript), call, callee, array)
}

fn library_api_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_arity(id, span, contract_id, callee, 1, status, dependencies)
}

fn library_api_record_with_arity(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

fn is_array_contract() -> LibraryStaticGlobalMethodContract {
    library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1)
        .expect("test contract")
}

fn contract_status_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> LibraryApiEvidenceStatus {
    library_api_contract_evidence_for_call(il, interner, call, id, callee, 1)
}

fn admitted_js_array_is_array_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
    let (mut il, call, callee, array) = js_array_is_array_call_il(interner);
    let contract = is_array_contract();
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(il.node(callee).span),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(library_api_record(
        3,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));
    (il, call, callee, array)
}

fn canonical_builtin_call_il(
    lang: Lang,
    builtin: Builtin,
    args: &[NodeId],
    builder: IlBuilder,
    root: NodeId,
) -> (Il, NodeId) {
    let mut builder = builder;
    let call = builder.add(NodeKind::Call, Payload::Builtin(builtin), sp(40), args);
    let root = builder.add(NodeKind::Func, Payload::None, sp(41), &[root, call]);
    (finish_il(builder, root, lang), call)
}

fn python_len_canonical_call_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(39), &[]);
    canonical_builtin_call_il(Lang::Python, Builtin::Len, &[arg], b, arg)
}

#[test]
fn canonical_builtin_admission_requires_language_core_or_library_api_evidence() {
    let (mut il, call) = python_len_canonical_call_il();

    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Len));

    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(library_api_record(
        10,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));
    assert!(!admitted_builtin_semantics_at_call(&il, call, Builtin::Abs));
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

    il.evidence.push(library_api_record(
        10,
        sp(39),
        map_get.id,
        map_get.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    il.evidence.push(library_api_record(
        11,
        il.node(call).span,
        unwrap_or.id,
        unwrap_or.callee,
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

    il.evidence.push(library_api_record(
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
    broken.evidence.push(library_api_record(
        10,
        broken.node(broken_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[99],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &broken,
        broken_call,
        Builtin::Len
    ));

    let (mut ambiguous, ambiguous_call) = python_len_canonical_call_il();
    ambiguous.evidence.push(library_api_record(
        10,
        ambiguous.node(ambiguous_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Ambiguous,
        &[],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &ambiguous,
        ambiguous_call,
        Builtin::Len
    ));

    let (mut conflicting, conflicting_call) = python_len_canonical_call_il();
    let abs = library_free_function_builtin_contract(Lang::Python, "abs", 1)
        .expect("Python abs contract");
    conflicting.evidence.push(library_api_record(
        10,
        conflicting.node(conflicting_call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    conflicting.evidence.push(library_api_record(
        11,
        conflicting.node(conflicting_call).span,
        abs.id,
        abs.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert!(!admitted_builtin_semantics_at_call(
        &conflicting,
        conflicting_call,
        Builtin::Len
    ));
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
    il.evidence.push(evidence_with_dependencies(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Field),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));

    assert!(admitted_builtin_semantics_at_call(&il, call, Builtin::Len));
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

    il.evidence.push(evidence(
        10,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        EvidenceStatus::Asserted,
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
    il.evidence.push(library_api_record(
        10,
        il.node(len).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, il.root),
        vec![ValueDomain::Number, ValueDomain::Unknown],
        "admitted Python len can contribute its numeric result domain"
    );
}

fn java_list_of_import_evidence_il(
    interner: &Interner,
    import_in_root: bool,
) -> (Il, NodeId, NodeId, Symbol, LibraryCollectionFactoryContract) {
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("List");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(32),
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, arg]);
    let root = if import_in_root {
        b.add(NodeKind::Module, Payload::None, sp(29), &[import, call])
    } else {
        b.add(NodeKind::Func, Payload::None, sp(35), &[call])
    };
    let mut il = finish_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("List"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("List")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(31), NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(library_api_record(
        2,
        sp(34),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
    ));
    (il, call, root, local, contract)
}

#[test]
fn library_api_evidence_resolution_is_dependency_backed() {
    let interner = Interner::new();
    let (il, call, _callee, _array) = js_array_is_array_call_il(&interner);
    let contract = is_array_contract();

    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Missing
    );

    let (il, call, callee, array) = admitted_js_array_is_array_il(&interner);
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_span_queries_reject_mismatched_callee_and_receiver_spans() {
    let interner = Interner::new();
    let (il, call, callee, array) = admitted_js_array_is_array_il(&interner);
    let contract = is_array_contract();
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(sp(99)),
                receiver_span: Some(il.node(array).span),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(il.node(call).span),
                callee_span: Some(il.node(callee).span),
                receiver_span: Some(sp(99)),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_missing_or_ambiguous_dependencies() {
    let interner = Interner::new();
    let contract = is_array_contract();

    let (mut missing_dep, call, _callee, _array) = js_array_is_array_call_il(&interner);
    missing_dep.evidence.push(library_api_record(
        0,
        missing_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        contract_status_for_call(&missing_dep, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut ambiguous_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    ambiguous_dep.evidence.push(evidence(
        0,
        EvidenceAnchor::node(ambiguous_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Ambiguous,
    ));
    ambiguous_dep.evidence.push(evidence(
        1,
        EvidenceAnchor::node(ambiguous_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    ambiguous_dep.evidence.push(library_api_record(
        2,
        ambiguous_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        contract_status_for_call(
            &ambiguous_dep,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_conflicting_or_misanchored_records() {
    let interner = Interner::new();
    let contract = is_array_contract();

    let (mut conflicting_dep, call, callee, array) = js_array_is_array_call_il(&interner);
    conflicting_dep.evidence.push(evidence(
        0,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(evidence(
        1,
        EvidenceAnchor::node(conflicting_dep.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(evidence(
        2,
        EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Map"),
        }),
        EvidenceStatus::Asserted,
    ));
    conflicting_dep.evidence.push(library_api_record(
        3,
        conflicting_dep.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    assert_eq!(
        contract_status_for_call(
            &conflicting_dep,
            &interner,
            call,
            contract.id,
            contract.callee
        ),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut il, call, _callee, _array) = admitted_js_array_is_array_il(&interner);
    let boolean = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
    il.evidence.push(library_api_record(
        3,
        il.node(call).span,
        boolean.id,
        boolean.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Rejected
    );

    let (mut wrong_anchor, call, _callee, _array) = js_array_is_array_call_il(&interner);
    wrong_anchor.evidence.push(library_api_record(
        0,
        sp(99),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[],
    ));
    assert_eq!(
        contract_status_for_call(&wrong_anchor, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Missing
    );
}

#[test]
fn library_api_evidence_resolution_accepts_import_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Values");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(10), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(10), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(10), &[lhs, rhs]);
    let callee = b.add(NodeKind::Var, Payload::Name(local), sp(11), &[]);
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(12),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(9), &[import, call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("deque contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("collections"),
        exported_hash: stable_symbol_hash("deque"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(10), stable_symbol_hash("Values")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(11), NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(library_api_record(
        2,
        sp(13),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[1],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_source_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let regex = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("/x/")),
        sp(20),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("test")),
        sp(21),
        &[regex],
    );
    let arg = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("s")),
        sp(22),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(19), &[call]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    let contract =
        library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(20)),
        EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(23),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_free_name_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(41),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(42), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(39), &[call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract = library_free_name_collection_factory_contract(Lang::Python, "list")
        .expect("Python list contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(40), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(42),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(42)),
                callee_span: Some(sp(40)),
                receiver_span: None,
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_free_function_builtin_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("len")),
        sp(45),
        &[],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(46),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(47), &[callee, arg]);
    let root = b.add(NodeKind::Module, Payload::None, sp(44), &[call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
        .expect("Python len contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(45), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("len"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(library_api_record(
        1,
        sp(47),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_require_backed_callees() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let require_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("require")),
        sp(48),
        &[],
    );
    let require_arg = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("set")),
        sp(48),
        &[],
    );
    let require_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(48),
        &[require_callee, require_arg],
    );
    let set = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Set")),
        sp(50),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("new")),
        sp(51),
        &[set],
    );
    let arg = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(52),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, arg]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(49),
        &[require_call, call],
    );
    let mut il = finish_il(b, root, Lang::Ruby);
    let contract = library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).expect("Set.new");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(50), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(48), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(sp(48)),
        EvidenceKind::Import(ImportEvidenceKind::Require {
            module_hash: stable_symbol_hash("set"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(library_api_record(
        3,
        sp(53),
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0, 2],
    ));
    assert_eq!(
        contract_status_for_call(&il, &interner, call, contract.id, contract.callee),
        LibraryApiEvidenceStatus::Admitted
    );
}

#[test]
fn library_api_evidence_resolution_accepts_import_binding_outside_unit_root() {
    let interner = Interner::new();
    let (mut il, call, _root, _local, contract) = java_list_of_import_evidence_il(&interner, false);
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(34)),
                callee_span: Some(sp(32)),
                receiver_span: Some(sp(30)),
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Rejected
    );
    assert_eq!(
        library_api_contract_evidence_at_call_span(
            &il,
            &interner,
            LibraryApiSpanEvidenceQuery {
                call_span: Some(sp(34)),
                callee_span: Some(sp(32)),
                receiver_span: None,
                id: contract.id,
                callee: contract.callee,
                arg_count: 1,
            },
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    il.evidence.push(evidence(
        3,
        EvidenceAnchor::binding(sp(36), stable_symbol_hash("List")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("other.module"),
            exported_hash: stable_symbol_hash("List"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}

#[test]
fn library_api_evidence_resolution_rejects_shadowed_java_static_members() {
    let interner = Interner::new();
    let (mut il, call, root, local, contract) = java_list_of_import_evidence_il(&interner, true);
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    );

    il.units.push(Unit {
        root,
        kind: UnitKind::Class,
        name: Some(local),
    });
    assert_eq!(
        library_api_contract_evidence_for_call(
            &il,
            &interner,
            call,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Rejected
    );
}
