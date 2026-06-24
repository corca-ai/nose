use super::*;

#[test]
fn java_guava_factory_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("ImmutableList");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(76), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(76), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(76), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(77), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(78),
        &[receiver],
    );
    let left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(79), &[]);
    let right = b.add(NodeKind::Lit, Payload::LitInt(2), sp(80), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(81),
        &[callee, left, right],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(76), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of")
        .expect("ImmutableList.of contract");
    push_imported_binding_use(
        &mut il,
        0,
        sp(76),
        1,
        sp(77),
        "com.google.common.collect",
        "ImmutableList",
    );
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "Guava import proof alone must not prove the factory"
    );
    il.evidence
        .push(java_guava_immutable_collection_factory_evidence(
            2,
            sp(81),
            contract.id,
            contract.callee,
            2,
            vec![EvidenceId(1)],
        ));
    let admitted = eval_proven_collection_op(&il, &interner, call)
        .expect("admitted Guava LibraryApi evidence should prove the factory");
    assert!(matches!(admitted, ValOp::Seq(SEQ_VALUE_COLLECTION)));
}

#[test]
fn java_guava_map_factory_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("ImmutableMap");
    let import = java_util_map_import(&mut b, Payload::Name(local), sp(110));
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(111), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(112),
        &[receiver],
    );
    let call = java_map_of_call(&mut b, callee, 113);
    let root = b.add(NodeKind::Module, Payload::None, sp(110), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    il.evidence.clear();
    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of")
        .expect("ImmutableMap.of contract");
    push_imported_binding_use(
        &mut il,
        0,
        sp(110),
        1,
        sp(111),
        "com.google.common.collect",
        "ImmutableMap",
    );
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "Guava import proof alone must not canonicalize the map factory"
    );
    il.evidence
        .push(java_guava_immutable_collection_factory_evidence(
            2,
            sp(117),
            contract.id,
            contract.callee,
            4,
            vec![EvidenceId(1)],
        ));
    assert!(
        matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "admitted Guava LibraryApi evidence should canonicalize ImmutableMap.of"
    );
}

#[test]
fn java_guava_map_factory_rejects_throwing_or_unsupported_shapes() {
    let duplicate = [
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(2),
    ];
    assert_guava_map_not_canonicalized(&duplicate, 130);

    let null_key = [
        Payload::Lit(LitClass::Null),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("blue")),
        Payload::LitInt(2),
    ];
    assert_guava_map_not_canonicalized(&null_key, 140);

    let unsupported_arity = eleven_entry_payloads();
    assert_guava_map_not_canonicalized(&unsupported_arity, 150);
}

#[test]
fn java_guava_collection_factory_rejects_static_null_elements() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("ImmutableList");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(180), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(180), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(180), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(181), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(182),
        &[receiver],
    );
    let null = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp(183), &[]);
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(184), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(185),
        &[callee, null, value],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(180), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "ImmutableList", "of")
        .expect("ImmutableList.of contract");
    push_imported_binding_use(
        &mut il,
        0,
        sp(180),
        1,
        sp(181),
        "com.google.common.collect",
        "ImmutableList",
    );
    il.evidence
        .push(java_guava_immutable_collection_factory_evidence(
            2,
            sp(185),
            contract.id,
            contract.callee,
            2,
            vec![EvidenceId(1)],
        ));

    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "Guava ImmutableList.of with a static null element throws and must stay unproven"
    );
}

fn assert_guava_map_not_canonicalized(args: &[Payload], base_line: u32) {
    let (il, interner, call) = java_guava_map_call_il(args, base_line);
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "Guava ImmutableMap.of shape must stay uncanonicalized"
    );
}

fn java_guava_map_call_il(args: &[Payload], base_line: u32) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("ImmutableMap");
    let import = java_util_map_import(&mut b, Payload::Name(local), sp(base_line));
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(base_line + 1), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(base_line + 2),
        &[receiver],
    );
    let arg_nodes: Vec<_> = args
        .iter()
        .enumerate()
        .map(|(idx, &payload)| b.add(NodeKind::Lit, payload, sp(base_line + 3 + idx as u32), &[]))
        .collect();
    let mut children = Vec::with_capacity(arg_nodes.len() + 1);
    children.push(callee);
    children.extend(arg_nodes);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(base_line + 3 + args.len() as u32),
        &children,
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(base_line),
        &[import, call],
    );
    let mut il = finish_test_il(b, root, Lang::Java);
    il.evidence.clear();
    let contract = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of")
        .expect("ImmutableMap.of contract");
    push_imported_binding_use(
        &mut il,
        0,
        sp(base_line),
        1,
        sp(base_line + 1),
        "com.google.common.collect",
        "ImmutableMap",
    );
    il.evidence
        .push(java_guava_immutable_collection_factory_evidence(
            2,
            sp(base_line + 3 + args.len() as u32),
            contract.id,
            contract.callee,
            args.len() as u16,
            vec![EvidenceId(1)],
        ));
    (il, interner, call)
}

fn eleven_entry_payloads() -> Vec<Payload> {
    (0..11)
        .flat_map(|idx| {
            [
                Payload::LitStr(stable_symbol_hash(&format!("k{idx}"))),
                Payload::LitInt(idx),
            ]
        })
        .collect()
}
