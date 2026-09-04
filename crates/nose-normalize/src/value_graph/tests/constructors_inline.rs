use super::support::*;

fn js_new_set_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let set = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Set")),
        sp(70),
        &[],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(71), &[]);
    let array = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(72),
        &[one],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(73), &[set, array]);
    let root = b.add(NodeKind::Block, Payload::None, sp(73), &[call]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::source_span(sp(73)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
    ));
    il.push_evidence(language_core_symbol_evidence(
        1,
        Lang::JavaScript,
        EvidenceAnchor::node(sp(70), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        },
    ));
    il.push_evidence(collection_sequence_evidence(2, Lang::JavaScript, sp(72)));
    (il, call)
}

#[test]
fn js_constructor_value_graph_requires_library_api_evidence() {
    let interner = Interner::new();
    let (mut il, call) = js_new_set_il(&interner);

    let mut builder = Builder::new(&il, &interner);
    let missing = builder.eval(call, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[missing as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));

    let wrong = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
    il.push_evidence(library_api_contract_evidence(
        3,
        sp(73),
        wrong.id,
        wrong.callee,
        1,
        vec![EvidenceId(0), EvidenceId(1)],
    ));
    let mut builder = Builder::new(&il, &interner);
    let rejected = builder.eval(call, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[rejected as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));

    let (mut il, call) = js_new_set_il(&interner);
    let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    il.push_evidence(js_like_builtin_collection_constructor_evidence(
        3,
        sp(73),
        set.id,
        set.callee,
        1,
        vec![EvidenceId(0), EvidenceId(1)],
    ));
    let mut builder = Builder::new(&il, &interner);
    let admitted = builder.eval(call, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[admitted as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
}

#[test]
fn inline_capture_poisons_in_loop_returns() {
    // A `return` reached while the loop depth is above the capture frame's base has
    // first-match-wins iteration semantics no single value can express — the frame must
    // poison (failing the inline closed) instead of capturing a bogus return value.
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let module = b.add(NodeKind::Module, Payload::None, sp(1), &[]);
    let il = finish_test_il(b, module, Lang::Python);
    let mut builder = Builder::new(&il, &interner);

    builder.inline_capture.push(InlineCaptureFrame {
        path_base: 0,
        loop_depth_base: 0,
        poisoned: false,
        returns: Vec::new(),
    });
    let v = builder.mk(
        ValOp::Const {
            kind: ConstKind::Int,
            bits: 1,
        },
        vec![],
    );
    builder.loop_depth = 1;
    builder.emit_return(v);
    let frame = builder.inline_capture.last().expect("frame");
    assert!(
        frame.poisoned && frame.returns.is_empty(),
        "an in-loop return must poison the capture frame, not record a value"
    );

    // At the frame's own loop depth the same return is an ordinary capture.
    builder.loop_depth = 0;
    builder.inline_capture.last_mut().expect("frame").poisoned = false;
    builder.emit_return(v);
    let frame = builder.inline_capture.last().expect("frame");
    assert!(
        !frame.poisoned && frame.returns == vec![(None, v)],
        "a return outside any callee loop is captured with no guard"
    );
}
