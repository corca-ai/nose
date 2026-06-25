use super::super::support::*;

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
    assert_eq!(
        admitted_hof_demand_effect_profile_at_node(&il, hof, HoFKind::Map),
        None,
        "raw HOF payloads must not resolve demand/effect profiles"
    );
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
        admitted_hof_demand_effect_profile_at_node(&il, hof, HoFKind::Map)
            .is_some_and(|profile| profile.proves_eager_per_element_callback_demand()),
        "a source-proven Python list comprehension should resolve eager callback demand"
    );
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
    assert_eq!(
        admitted_hof_demand_effect_profile_at_node(&set_il, hof, HoFKind::Map),
        None,
        "unsupported source comprehension proofs must keep HOF demand profiles closed"
    );
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Opaque(_)),
        "set comprehension proof must not reuse list-like HOF value semantics"
    );
}

fn div_zero_map_len_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(1), &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp(1), &[item]);
    let lambda = div_zero_lambda(&mut b, 2, sp(2));
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(3),
        &[coll, lambda],
    );
    let count = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(4),
        &[hof],
    );
    (finish_test_il(b, count, Lang::Rust), hof)
}

fn push_map_contract_evidence(il: &mut Il, lang: Lang, hof: NodeId, expect_msg: &str) {
    let receiver = il.children(hof)[0];
    if matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    ) {
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
            EvidenceKind::Domain(DomainEvidence::Array),
        ));
    } else {
        il.evidence.push(language_core_evidence(
            0,
            lang,
            EvidenceAnchor::sequence(il.node(receiver).span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        ));
    }
    let contract = library_method_call_contract(lang, "map", 1).expect(expect_msg);
    let evidence = if lang == Lang::Rust {
        rust_sequence_hof_adapter_evidence(
            1,
            il.node(hof).span,
            contract.id,
            contract.callee,
            1,
            vec![EvidenceId(0)],
        )
    } else if matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    ) {
        js_like_builtin_array_evidence(
            1,
            il.node(hof).span,
            contract.id,
            contract.callee,
            1,
            vec![EvidenceId(0)],
        )
    } else {
        library_api_contract_evidence(
            1,
            il.node(hof).span,
            contract.id,
            contract.callee,
            1,
            vec![EvidenceId(0)],
        )
    };
    il.evidence.push(evidence);
}

#[test]
fn library_hof_static_callback_error_requires_explicit_eager_demand() {
    let interner = Interner::new();
    let (mut il, hof) = div_zero_map_len_il();
    push_map_contract_evidence(&mut il, Lang::Rust, hof, "Rust map contract");
    assert!(nose_semantics::admitted_hof_api_at_node(
        &il,
        hof,
        HoFKind::Map
    ));
    assert!(
        admitted_hof_demand_effect_profile_at_node(&il, hof, HoFKind::Map)
            .is_some_and(|profile| profile.callback_effects_delayed_until_pull()),
        "Rust iterator map resolves to pull-lazy callback demand"
    );

    let mut builder = Builder::new(&il, &interner);
    assert!(
        !builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "admitted library HOF payloads must not prove eager callback exception timing"
    );
}

#[test]
fn eager_js_map_demand_exposes_static_callback_error_unless_broken() {
    let interner = Interner::new();
    let (mut js_il, hof) = div_zero_map_len_il();
    js_il.meta.lang = Lang::JavaScript;
    push_map_contract_evidence(&mut js_il, Lang::JavaScript, hof, "JS map contract");
    assert!(nose_semantics::admitted_hof_api_at_node(
        &js_il,
        hof,
        HoFKind::Map
    ));
    assert!(
        admitted_hof_demand_effect_profile_at_node(&js_il, hof, HoFKind::Map)
            .is_some_and(|profile| profile.proves_eager_per_element_callback_demand()),
        "JS Array.map resolves to eager per-element callback demand"
    );
    let mut builder = Builder::new(&js_il, &interner);
    assert!(
        builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "an admitted eager JS Array.map demand profile exposes callback exception timing"
    );

    let mut broken_js_il = js_il.clone();
    broken_js_il.evidence[1].status = EvidenceStatus::Ambiguous;
    assert_eq!(
        admitted_hof_demand_effect_profile_at_node(&broken_js_il, hof, HoFKind::Map),
        None,
        "broken library API evidence must not resolve HOF demand profiles"
    );
    let mut builder = Builder::new(&broken_js_il, &interner);
    assert!(
        !builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "broken library API evidence must keep callback exception timing closed"
    );
}

#[test]
fn pull_lazy_java_stream_map_keeps_static_callback_error_closed() {
    let interner = Interner::new();
    let (mut java_il, hof) = div_zero_map_len_il();
    java_il.meta.lang = Lang::Java;
    push_map_contract_evidence(&mut java_il, Lang::Java, hof, "Java stream map contract");
    assert!(nose_semantics::admitted_hof_api_at_node(
        &java_il,
        hof,
        HoFKind::Map
    ));
    assert!(
        admitted_hof_demand_effect_profile_at_node(&java_il, hof, HoFKind::Map)
            .is_some_and(|profile| profile.callback_effects_delayed_until_pull()),
        "Java Stream.map resolves to pull-lazy callback demand"
    );
    let mut builder = Builder::new(&java_il, &interner);
    assert!(
        !builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "an admitted pull-lazy Java Stream.map profile delays callback exception timing"
    );
}

#[test]
fn source_comprehension_timing_controls_static_callback_error() {
    let interner = Interner::new();
    let (mut list_il, hof) = div_zero_map_len_il();
    list_il.meta.lang = Lang::Python;
    push_source_comprehension(
        &mut list_il,
        0,
        sp(3),
        SourceComprehensionKind::PythonListComprehension,
    );
    let mut builder = Builder::new(&list_il, &interner);
    assert!(
        builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "a source-proven Python list comprehension keeps eager callback exception timing"
    );

    let mut gen_il = list_il.clone();
    gen_il.evidence.clear();
    push_source_comprehension(
        &mut gen_il,
        0,
        sp(3),
        SourceComprehensionKind::PythonGeneratorExpression,
    );
    let mut builder = Builder::new(&gen_il, &interner);
    assert!(
        !builder.expr_is_static_runtime_err(hof, &FxHashMap::default()),
        "a source-proven Python generator expression remains pull-lazy"
    );
}

#[test]
fn raw_builtin_payload_does_not_prove_static_error_demand() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(1), &[]);
    let lhs = b.add(NodeKind::Lit, Payload::LitInt(1), sp(2), &[]);
    let rhs = b.add(NodeKind::Lit, Payload::LitInt(0), sp(2), &[]);
    let fallback = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp(2), &[lhs, rhs]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::ValueOrDefault),
        sp(3),
        &[value, fallback],
    );
    let il = finish_test_il(b, call, Lang::JavaScript);
    let mut builder = Builder::new(&il, &interner);
    assert!(
        !builder.expr_is_static_runtime_err(call, &FxHashMap::default()),
        "raw builtin payloads do not prove fallback demand without admitted semantics"
    );
}

#[test]
fn len_of_library_hof_requires_materialized_demand_profile() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(1), &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp(1), &[item]);
    let lambda = identity_lambda(&mut b, 2, sp(2));
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(3),
        &[coll, lambda],
    );
    let count = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        sp(4),
        &[hof],
    );
    let mut il = finish_test_il(b, count, Lang::Rust);
    push_map_contract_evidence(&mut il, Lang::Rust, hof, "Rust map contract");
    let mut builder = Builder::new(&il, &interner);
    assert!(
        builder.terminal_reduction_arg_admitted(hof),
        "terminal reductions may force admitted pull-lazy iterator HOFs"
    );
    assert!(
        !builder.len_arg_admitted(hof),
        "len must not treat pull-lazy iterator HOFs as materialized collections"
    );
    assert_eq!(
        builder.eval_len_builtin(hof, &FxHashMap::default()),
        None,
        "len over pull-lazy iterator HOFs stays closed without an exact-size contract"
    );
    let contract =
        library_method_call_contract(Lang::Rust, "count", 0).expect("Rust count contract");
    il.evidence.push(rust_sequence_hof_adapter_evidence(
        2,
        il.node(count).span,
        contract.id,
        contract.callee,
        0,
        vec![EvidenceId(1)],
    ));
    assert!(admitted_terminal_count_reduction_at_call(&il, count));
    let mut builder = Builder::new(&il, &interner);
    let value = builder.eval(count, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Reduce(op) if op == Op::Add as u32),
        "Rust count() over pull-lazy iterator HOFs should use terminal reduction demand"
    );

    let mut js_il = il.clone();
    js_il.meta.lang = Lang::JavaScript;
    js_il.evidence.clear();
    push_map_contract_evidence(&mut js_il, Lang::JavaScript, hof, "JS map contract");
    let mut builder = Builder::new(&js_il, &interner);
    assert!(
        builder.len_arg_admitted(hof),
        "len may consume admitted eager/materialized library HOF profiles"
    );
    assert!(
        builder
            .eval_len_builtin(hof, &FxHashMap::default())
            .is_some(),
        "eager Array.map keeps enough materialized value semantics for len"
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
