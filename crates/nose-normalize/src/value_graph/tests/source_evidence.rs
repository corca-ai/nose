use super::*;

fn pure_inline_caller_il(interner: &Interner, with_target_evidence: bool) -> (Il, NodeId) {
    let helper_name = interner.intern("base");
    let caller_name = interner.intern("price");
    let mut b = IlBuilder::new(FileId(0));

    let helper_param = b.add(NodeKind::Param, Payload::Cid(0), sp(1), &[]);
    let helper_arg = b.add(NodeKind::Var, Payload::Cid(0), sp(2), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(3), &[]);
    let add = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(4),
        &[helper_arg, one],
    );
    let helper_ret = b.add(NodeKind::Return, Payload::None, sp(5), &[add]);
    let helper_body = b.add(NodeKind::Block, Payload::None, sp(6), &[helper_ret]);
    let helper = b.add(
        NodeKind::Func,
        Payload::None,
        sp(7),
        &[helper_param, helper_body],
    );

    let caller_param = b.add(NodeKind::Param, Payload::Cid(0), sp(10), &[]);
    let callee = b.add(NodeKind::Var, Payload::Name(helper_name), sp(11), &[]);
    let caller_arg = b.add(NodeKind::Var, Payload::Cid(0), sp(12), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, caller_arg]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp(14), &[]);
    let mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp(15), &[call, two]);
    let caller_ret = b.add(NodeKind::Return, Payload::None, sp(16), &[mul]);
    let caller_body = b.add(NodeKind::Block, Payload::None, sp(17), &[caller_ret]);
    let caller = b.add(
        NodeKind::Func,
        Payload::None,
        sp(18),
        &[caller_param, caller_body],
    );
    let module = b.add(NodeKind::Module, Payload::None, sp(19), &[helper, caller]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        vec![
            Unit {
                root: helper,
                kind: UnitKind::Function,
                name: Some(helper_name),
            },
            Unit {
                root: caller,
                kind: UnitKind::Function,
                name: Some(caller_name),
            },
        ],
        Vec::new(),
    );
    if with_target_evidence {
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: il.node(helper).span,
                name_hash: interner.symbol_hash(helper_name),
            }),
        ));
    }
    (il, caller)
}

fn pure_inline_direct_il(interner: &Interner) -> (Il, NodeId) {
    let caller_name = interner.intern("price");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(20), &[]);
    let arg = b.add(NodeKind::Var, Payload::Cid(0), sp(21), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(22), &[]);
    let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp(23), &[arg, one]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp(24), &[]);
    let mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp(25), &[add, two]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(26), &[mul]);
    let body = b.add(NodeKind::Block, Payload::None, sp(27), &[ret]);
    let caller = b.add(NodeKind::Func, Payload::None, sp(28), &[param, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(29), &[caller]);
    let il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        vec![Unit {
            root: caller,
            kind: UnitKind::Function,
            name: Some(caller_name),
        }],
        Vec::new(),
    );
    (il, caller)
}

#[test]
fn pure_inline_consumes_call_target_evidence_not_raw_callee_name() {
    let interner = Interner::new();
    let (direct_il, direct_root) = pure_inline_direct_il(&interner);
    let direct = value_fingerprint(&direct_il, direct_root, &interner);

    let (raw_call_il, raw_call_root) = pure_inline_caller_il(&interner, false);
    assert_ne!(
        direct,
        value_fingerprint(&raw_call_il, raw_call_root, &interner),
        "a raw callee spelling must not prove a pure inline target"
    );

    let (proven_call_il, proven_call_root) = pure_inline_caller_il(&interner, true);
    assert_eq!(
        direct,
        value_fingerprint(&proven_call_il, proven_call_root, &interner),
        "explicit CallTarget evidence should admit pure helper beta-substitution"
    );
}

#[test]
fn c_unsigned_cast32_value_graph_requires_source_cast_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let base = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp(2), &[]);
    let index = b.add(NodeKind::Index, Payload::None, sp(2), &[base, zero]);
    let cast = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::UnsignedCast32),
        sp(3),
        &[index],
    );
    let mut il = finish_test_il(b, cast, Lang::C);

    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(cast, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Opaque(_)),
        "raw UnsignedCast32 payload must not prove a C unsigned cast"
    );

    push_source_cast(&mut il, 0, sp(3), SourceCastKind::CUnsigned32);
    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(cast, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Call(tag) if tag == builtin_tag(Builtin::UnsignedCast32)),
        "source-proven C unsigned 32-bit casts should retain the byte-pack cast value"
    );
}

#[test]
fn raw_library_builtin_payloads_do_not_fold_without_admission() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Lit, Payload::LitInt(-7), sp(1), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Abs),
        sp(2),
        &[arg],
    );
    let mut il = finish_test_il(b, call, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[raw as usize].op, ValOp::Opaque(_)),
        "raw canonical Abs payload must not imply Python abs semantics"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "abs", 1)
        .expect("Python abs contract");
    il.evidence.push(library_api_contract_evidence(
        0,
        il.node(call).span,
        contract.id,
        contract.callee,
        1,
        Vec::new(),
    ));
    let mut builder = Builder::new(&il, &interner);
    let admitted = builder.eval(call, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[admitted as usize].op, ValOp::Un(code) if code == ABS_CODE),
        "admitted Python abs payload should fold to the canonical absolute value"
    );
}

#[test]
fn raw_contains_payload_does_not_prove_membership() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let item = b.add(NodeKind::Var, Payload::Cid(0), sp(1), &[]);
    let collection = b.add(NodeKind::Var, Payload::Cid(1), sp(2), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Contains),
        sp(3),
        &[item, collection],
    );
    let py_il = finish_test_il(b, call, Lang::Python);

    let mut builder = Builder::new(&py_il, &interner);
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[raw as usize].op, ValOp::Opaque(_)),
        "raw Contains payload must not become a membership predicate"
    );

    let mut b = IlBuilder::new(FileId(0));
    let item = b.add(NodeKind::Var, Payload::Cid(0), sp(1), &[]);
    let collection = b.add(NodeKind::Var, Payload::Cid(1), sp(2), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Contains),
        sp(3),
        &[item, collection],
    );
    let go_il = finish_test_il(b, call, Lang::Go);
    let mut builder = Builder::new(&go_il, &interner);
    let admitted = builder.eval(call, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[admitted as usize].op, ValOp::Bin(op) if op == Op::In as u32),
        "Go map lookup-ok lowering remains a language-core membership predicate"
    );
}

#[test]
fn raw_hof_value_graph_requires_source_or_api_admission() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let coll = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let lambda = identity_lambda(&mut b, 2, sp(2));
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(3),
        &[coll, lambda],
    );
    let mut il = finish_test_il(b, hof, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(hof, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Opaque(_)),
        "raw HOF payloads must stay opaque without source or API proof"
    );

    push_source_comprehension(
        &mut il,
        0,
        sp(3),
        SourceComprehensionKind::PythonListComprehension,
    );
    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(hof, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Hof(k) if k == HoFKind::Map as u32),
        "a source-proven Python list comprehension should still enter HOF value semantics"
    );

    let mut set_il = il.clone();
    set_il.evidence.clear();
    push_source_comprehension(
        &mut set_il,
        0,
        sp(3),
        SourceComprehensionKind::PythonSetComprehension,
    );
    let mut builder = Builder::new(&set_il, &interner);
    let value = builder.eval(hof, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Opaque(_)),
        "set comprehension proof must not reuse list-like HOF value semantics"
    );
}

#[test]
fn source_comprehension_admits_internal_python_filter_hof_only_in_context() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let coll = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let pred = const_bool_lambda(&mut b, 2, true, sp(2));
    let filter = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Filter),
        sp(3),
        &[coll, pred],
    );
    let mapper = identity_lambda(&mut b, 3, sp(4));
    let map = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(5),
        &[filter, mapper],
    );
    let mut il = finish_test_il(b, map, Lang::Python);
    push_source_comprehension(
        &mut il,
        0,
        sp(5),
        SourceComprehensionKind::PythonListComprehension,
    );

    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(filter, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Opaque(_)),
        "an internal filter HOF remains closed when evaluated as its own unproven surface"
    );

    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(map, &FxHashMap::default());
    let node = &builder.nodes[value as usize];
    assert!(
        matches!(node.op, ValOp::Hof(k) if k == HoFKind::Map as u32) && node.args.len() == 2,
        "a proven Python comprehension should admit its internal filter and carry the predicate"
    );
}

#[test]
fn len_of_raw_filter_hof_requires_filter_admission() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let coll = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let pred = const_bool_lambda(&mut b, 2, true, sp(2));
    let filter = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Filter),
        sp(3),
        &[coll, pred],
    );
    let len = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(4),
        &[filter],
    );
    let mut il = finish_test_il(b, len, Lang::Python);
    let contract =
        library_free_function_builtin_contract(Lang::Python, "len", 1).expect("len contract");
    il.evidence.push(library_api_contract_evidence(
        0,
        il.node(len).span,
        contract.id,
        contract.callee,
        1,
        Vec::new(),
    ));

    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(len, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[value as usize].op, ValOp::Reduce(op) if op == Op::Add as u32),
        "admitted len must not turn an unadmitted raw filter HOF into a predicate count"
    );
}

#[test]
fn rust_full_range_seq_requires_source_range_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let coll = b.add(NodeKind::Var, Payload::Cid(0), sp(176), &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp(177), &[]);
    let len = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(178),
        &[coll],
    );
    let inclusive = b.add(NodeKind::Lit, Payload::LitInt(0), sp(179), &[]);
    let range = b.add(
        NodeKind::Seq,
        Payload::None,
        sp(180),
        &[zero, len, inclusive],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(175), &[range]);
    let mut il = finish_test_il(b, root, Lang::Python);
    let len_contract =
        library_free_function_builtin_contract(Lang::Python, "len", 1).expect("len contract");
    il.evidence.push(library_api_contract_evidence(
        0,
        il.node(len).span,
        len_contract.id,
        len_contract.callee,
        1,
        Vec::new(),
    ));

    let builder = Builder::new(&il, &interner);
    assert_eq!(
        builder.range_len_collection(range),
        None,
        "raw Seq(0, Len(C), 0) shape must not prove Rust full-index range semantics"
    );

    let mut inclusive_il = il.clone();
    push_source_range(
        &mut inclusive_il,
        1,
        sp(180),
        SourceRangeKind::RustInclusiveRangeExpression,
    );
    let builder = Builder::new(&inclusive_il, &interner);
    assert_eq!(
        builder.range_len_collection(range),
        None,
        "inclusive Rust range source evidence must not license half-open full-index rewrite"
    );

    push_source_range(
        &mut il,
        1,
        sp(180),
        SourceRangeKind::RustHalfOpenRangeExpression,
    );
    let builder = Builder::new(&il, &interner);
    assert_eq!(builder.range_len_collection(range), Some(coll));
}
