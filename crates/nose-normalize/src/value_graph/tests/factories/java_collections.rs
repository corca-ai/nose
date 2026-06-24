use super::*;

#[test]
fn java_collections_singleton_list_value_graph_uses_fixed_element_result() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Collections");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(140), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(140), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(140), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(141), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("singletonList")),
        sp(142),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(143), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(144), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(140), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract =
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
            .expect("Collections.singletonList contract");
    push_imported_binding_use(&mut il, 0, sp(140), 1, sp(141), "java.util", "Collections");
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "java.util import proof alone must not prove Collections.singletonList"
    );
    il.evidence.push(java_stdlib_collection_factory_evidence(
        2,
        sp(144),
        contract,
        1,
        vec![EvidenceId(1)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(call, &FxHashMap::default());
    let admitted = builder
        .proven_collection_value(raw)
        .expect("admitted LibraryApi evidence should prove singletonList");
    assert!(matches!(
        builder.nodes[admitted as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
    assert_eq!(builder.nodes[admitted as usize].args.len(), 1);
}

#[test]
fn java_collections_singleton_list_value_graph_rejects_wrong_arity() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Collections");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(145), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(145), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(145), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(146), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("singletonList")),
        sp(147),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(148), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(145), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract =
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
            .expect("Collections.singletonList contract");
    push_imported_binding_use(&mut il, 0, sp(145), 1, sp(146), "java.util", "Collections");
    il.evidence.push(java_stdlib_collection_factory_evidence(
        2,
        sp(148),
        contract,
        0,
        vec![EvidenceId(1)],
    ));

    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "fixed single-element Collections.singletonList must reject unsupported arity"
    );
}

#[test]
fn java_collections_empty_set_value_graph_uses_empty_sequence_result() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Collections");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(150), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(150), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(150), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(151), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("emptySet")),
        sp(152),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(153), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(150), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "Collections", "emptySet")
        .expect("Collections.emptySet contract");
    push_imported_binding_use(&mut il, 0, sp(150), 1, sp(151), "java.util", "Collections");
    il.evidence.push(java_stdlib_collection_factory_evidence(
        2,
        sp(153),
        contract,
        0,
        vec![EvidenceId(1)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(call, &FxHashMap::default());
    let admitted = builder
        .proven_collection_value(raw)
        .expect("admitted LibraryApi evidence should prove emptySet");
    assert!(matches!(
        builder.nodes[admitted as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
    assert!(builder.nodes[admitted as usize].args.is_empty());
}

#[test]
fn java_collections_empty_map_value_graph_uses_map_pack_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Collections");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(160), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(160), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(160), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(161), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("emptyMap")),
        sp(162),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(163), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(160), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_map_factory_contract(Lang::Java, "Collections", "emptyMap")
        .expect("Collections.emptyMap contract");
    push_imported_binding_use(&mut il, 0, sp(160), 1, sp(161), "java.util", "Collections");
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "java.util import proof alone must not prove Collections.emptyMap"
    );
    il.evidence.push(java_stdlib_map_factory_evidence(
        2,
        sp(163),
        contract,
        0,
        vec![EvidenceId(1)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let map_value = builder.eval(call, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[map_value as usize].op,
        ValOp::Seq(SEQ_VALUE_MAP)
    ));
    assert!(builder.nodes[map_value as usize].args.is_empty());
}

#[test]
fn java_collections_singleton_map_value_graph_uses_positional_entry_result() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("Collections");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(170), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(170), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(170), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(171), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("singletonMap")),
        sp(172),
        &[receiver],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(173),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(174), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(175),
        &[callee, key, value],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(170), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_map_factory_contract(Lang::Java, "Collections", "singletonMap")
        .expect("Collections.singletonMap contract");
    push_imported_binding_use(&mut il, 0, sp(170), 1, sp(171), "java.util", "Collections");
    il.evidence.push(java_stdlib_map_factory_evidence(
        2,
        sp(175),
        contract,
        2,
        vec![EvidenceId(1)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let map_value = builder.eval(call, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[map_value as usize].op,
        ValOp::Seq(SEQ_VALUE_MAP)
    ));
    assert_eq!(builder.nodes[map_value as usize].args.len(), 1);
}
