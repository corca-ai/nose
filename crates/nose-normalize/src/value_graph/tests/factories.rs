use super::support::*;

mod guava_factories;
mod java_collections;
mod swift_factories;

#[test]
fn imported_collection_factory_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("deque");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(60), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(60), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(60), &[lhs, rhs]);
    let callee = b.add(NodeKind::Var, Payload::Name(local), sp(61), &[]);
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(62), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(63),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(64), &[callee, seq]);
    let root = b.add(NodeKind::Block, Payload::None, sp(60), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Python);
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("deque contract");
    push_imported_binding_use(&mut il, 0, sp(60), 1, sp(61), "collections", "deque");
    il.evidence
        .push(collection_sequence_evidence(2, Lang::Python, sp(63)));
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "import symbol proof alone must not prove the migrated stdlib factory"
    );
    il.evidence.push(python_stdlib_collection_factory_evidence(
        3,
        sp(64),
        contract,
        1,
        vec![EvidenceId(1)],
    ));
    let admitted = eval_proven_collection_op(&il, &interner, call)
        .expect("admitted LibraryApi evidence should prove the factory");
    assert!(matches!(admitted, ValOp::Seq(SEQ_VALUE_COLLECTION)));

    let wrong = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    il.evidence.pop();
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(64),
        wrong.id,
        wrong.callee,
        1,
        vec![EvidenceId(1)],
    ));
    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(builder.proven_collection_value(raw).is_none());
}

#[test]
fn java_collection_factory_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("List");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(70), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(70), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(70), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(71), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(72),
        &[receiver],
    );
    let left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(73), &[]);
    let right = b.add(NodeKind::Lit, Payload::LitInt(2), sp(74), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(75),
        &[callee, left, right],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(70), &[import, call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    push_imported_binding_use(&mut il, 0, sp(70), 1, sp(71), "java.util", "List");
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "java.util import proof alone must not prove the migrated Java factory"
    );
    il.evidence.push(java_stdlib_collection_factory_evidence(
        2,
        sp(75),
        contract,
        2,
        vec![EvidenceId(1)],
    ));
    let admitted = eval_proven_collection_op(&il, &interner, call)
        .expect("admitted LibraryApi evidence should prove the Java factory");
    assert!(matches!(admitted, ValOp::Seq(SEQ_VALUE_COLLECTION)));
}

#[test]
fn java_collection_constructor_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("ArrayList")),
        sp(80),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(81), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(79), &[call]);
    let mut il = finish_test_il(b, root, Lang::Java);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(81)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
    ));
    push_imported_binding_use(&mut il, 1, sp(70), 2, sp(80), "java.util", "ArrayList");
    assert!(
        !matches!(
            eval_op(&il, &interner, call),
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ),
        "source/import proof alone must not canonicalize a Java constructor"
    );

    let contract =
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0).unwrap();
    il.evidence
        .push(java_stdlib_collection_constructor_evidence(
            3,
            sp(81),
            contract,
            0,
            vec![EvidenceId(0), EvidenceId(2)],
        ));
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
}

#[test]
fn static_index_membership_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let red = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(90),
        &[],
    );
    let array = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(91),
        &[red],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("indexOf")),
        sp(92),
        &[array],
    );
    let value = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(93),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(94), &[callee, value]);
    let minus_one = b.add(NodeKind::Lit, Payload::LitInt(-1), sp(95), &[]);
    let comparison = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Ne),
        sp(96),
        &[call, minus_one],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(89), &[comparison]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);
    il.evidence
        .push(collection_sequence_evidence(0, Lang::JavaScript, sp(91)));
    assert!(
        !matches!(eval_op(&il, &interner, comparison), ValOp::Bin(op) if op == Op::In as u32),
        "static array receiver proof alone must not prove indexOf membership"
    );

    let contract =
        library_static_index_membership_contract(Lang::JavaScript, "indexOf", 1).unwrap();
    il.evidence
        .push(js_like_builtin_static_index_membership_evidence(
            1,
            sp(94),
            contract.id,
            contract.callee,
            1,
            vec![EvidenceId(0)],
        ));
    assert!(matches!(
        eval_op(&il, &interner, comparison),
        ValOp::Bin(op) if op == Op::In as u32
    ));
}

fn java_util_map_import(b: &mut IlBuilder, lhs_payload: Payload, span: Span) -> NodeId {
    let import_lhs = b.add(NodeKind::Var, lhs_payload, span, &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("java.util")),
        span,
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("Map")),
        span,
        &[],
    );
    let import_rhs = b.add(NodeKind::Seq, Payload::None, span, &[module, exported]);
    b.add(
        NodeKind::Assign,
        Payload::None,
        span,
        &[import_lhs, import_rhs],
    )
}

fn java_map_of_call(b: &mut IlBuilder, callee: NodeId, base_line: u32) -> NodeId {
    let red = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(base_line),
        &[],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(base_line + 1), &[]);
    let blue = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("blue")),
        sp(base_line + 2),
        &[],
    );
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp(base_line + 3), &[]);
    b.add(
        NodeKind::Call,
        Payload::None,
        sp(base_line + 4),
        &[callee, red, one, blue, two],
    )
}

fn push_java_map_lookup_evidence(
    il: &mut Il,
    import_span: Span,
    receiver_span: Span,
    call_span: Span,
    binding_span: Span,
    snapshot_module: &str,
) {
    let contract =
        library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");
    il.evidence.push(language_core_evidence(
        0,
        Lang::Java,
        EvidenceAnchor::sequence(import_span),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
        }),
    ));
    push_imported_binding_use(il, 1, import_span, 2, receiver_span, "java.util", "Map");
    il.evidence.push(java_stdlib_map_factory_evidence(
        3,
        call_span,
        contract,
        4,
        vec![EvidenceId(2)],
    ));
    il.evidence.push(language_core_evidence_with_dependencies(
        4,
        Lang::Java,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
            module_hash: stable_symbol_hash(snapshot_module),
            exported_hash: stable_symbol_hash("LOOKUP"),
            root_kind: NodeKind::Call,
        }),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        5,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        6,
        EvidenceAnchor::binding(binding_span, stable_symbol_hash("LOOKUP")),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(5)],
    ));
}

#[test]
fn java_map_factory_value_graph_uses_library_api_after_import_seed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let map = interner.intern("Map");
    let lookup = interner.intern("LOOKUP");
    let import = java_util_map_import(&mut b, Payload::Name(map), sp(100));
    let receiver = b.add(NodeKind::Var, Payload::Name(map), sp(101), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(102),
        &[receiver],
    );
    let call = java_map_of_call(&mut b, callee, 103);
    let lookup_lhs = b.add(NodeKind::Var, Payload::Name(lookup), sp(108), &[]);
    let lookup_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(108),
        &[lookup_lhs, call],
    );
    let lookup_ref = b.add(NodeKind::Var, Payload::Name(lookup), sp(109), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(100),
        &[import, lookup_assign, lookup_ref],
    );
    let mut il = finish_test_il(b, root, Lang::Java);
    push_java_map_lookup_evidence(
        &mut il,
        sp(100),
        sp(101),
        sp(107),
        sp(108),
        "LookupProvider",
    );

    let mut builder = Builder::new(&il, &interner);
    assert!(!builder.unit_defines_symbol(lookup));
    assert!(
        !builder.module_binding_mutated(lookup),
        "read-only getOrDefault use must not mark LOOKUP as mutated"
    );
    builder.seed_module_value_bindings();
    let map_value = builder.eval(call, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[map_value as usize].op,
        ValOp::Seq(SEQ_VALUE_MAP)
    ));
    let proven = builder.eval(lookup_ref, &FxHashMap::default());
    assert!(
        builder.global_env.contains_key(&lookup),
        "LOOKUP should be seeded as an immutable module binding"
    );
    assert!(
        matches!(builder.nodes[proven as usize].op, ValOp::Seq(SEQ_VALUE_MAP)),
        "expected LOOKUP to seed as map"
    );
}

#[test]
fn normalized_java_static_import_map_binding_feeds_get_or_default() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let map = interner.intern("Map");
    let lookup = interner.intern("LOOKUP");
    let lookup_method = interner.intern("lookup");

    let import = java_util_map_import(&mut b, Payload::Cid(0), sp(130));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(131), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(132),
        &[receiver],
    );
    let map_of = java_map_of_call(&mut b, callee, 133);
    let lookup_lhs = b.add(NodeKind::Var, Payload::Cid(1), sp(138), &[]);
    let lookup_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(138),
        &[lookup_lhs, map_of],
    );

    let key_param = b.add(NodeKind::Param, Payload::Cid(2), sp(139), &[]);
    let other_param = b.add(NodeKind::Param, Payload::Cid(3), sp(139), &[]);
    let lookup_receiver = b.add(NodeKind::Var, Payload::Name(lookup), sp(140), &[]);
    let get_or_default = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("getOrDefault")),
        sp(141),
        &[lookup_receiver],
    );
    let key_ref = b.add(NodeKind::Var, Payload::Cid(2), sp(142), &[]);
    let fallback = b.add(NodeKind::Lit, Payload::LitInt(0), sp(143), &[]);
    let get_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(144),
        &[get_or_default, key_ref, fallback],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp(144), &[get_call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(144), &[ret]);
    let func = b.add(
        NodeKind::Func,
        Payload::None,
        sp(139),
        &[key_param, other_param, body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(130),
        &[import, lookup_assign, func],
    );
    let mut il = b.finish(
        root,
        FileMeta {
            path: "JavaImported.java".into(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Method,
            name: Some(lookup_method),
            origin: Default::default(),
        }],
        vec![map, lookup],
    );
    push_java_map_lookup_evidence(&mut il, sp(130), sp(131), sp(137), sp(138), "Tables");
    push_method_call_library_api_evidence(&mut il, &interner, 7, get_call, "getOrDefault", 2);

    let mut builder = Builder::new(&il, &interner);
    builder.seed_module_value_bindings();
    assert!(
        builder.global_env.contains_key(&lookup),
        "normalized static import binding should seed the copied map value"
    );

    let mut env = FxHashMap::default();
    env.insert(2, builder.mk(ValOp::Input(0), vec![]));
    env.insert(3, builder.mk(ValOp::Input(1), vec![]));
    let value = builder.eval(get_call, &env);
    let node = &builder.nodes[value as usize];
    assert!(matches!(
        node.op,
        ValOp::Call(tag) if tag == builtin_tag(Builtin::GetOrDefault)
    ));
    assert!(matches!(
        builder.nodes[node.args[0] as usize].op,
        ValOp::Seq(SEQ_VALUE_MAP)
    ));
}

#[test]
fn raw_name_module_assignment_without_evidence_is_not_seeded() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let table = interner.intern("TABLE");
    let lhs = b.add(NodeKind::Var, Payload::Name(table), sp(120), &[]);
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(120), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(120), &[item]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(120), &[lhs, rhs]);
    let table_ref = b.add(NodeKind::Var, Payload::Name(table), sp(121), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(120),
        &[assign, table_ref],
    );
    let il = finish_test_il(b, root, Lang::JavaScript);
    let mut builder = Builder::new(&il, &interner);

    builder.seed_module_value_bindings();

    assert!(
        !builder.global_env.contains_key(&table),
        "raw Name assignments need matching language-core import or imported-literal evidence"
    );
}

#[test]
fn namespace_collection_factory_value_graph_uses_library_api_evidence_after_seed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("collections");
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(80), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(80),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(80), &[module]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(80), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(81), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("deque")),
        sp(82),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(83), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(84),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(85), &[callee, seq]);
    let root = b.add(NodeKind::Module, Payload::None, sp(80), &[import, call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        vec![local],
    );
    let contract =
        library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
            .expect("deque contract");
    il.evidence.push(language_core_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::sequence(sp(80)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("collections"),
        }),
    ));
    push_imported_namespace_use(&mut il, 1, sp(80), 2, sp(81), "collections");
    il.evidence
        .push(collection_sequence_evidence(3, Lang::Python, sp(84)));
    let mut builder = Builder::new(&il, &interner);
    builder.seed_module_value_bindings();
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(
        builder.proven_collection_value(raw).is_none(),
        "namespace import proof alone must not prove the migrated stdlib factory"
    );
    il.evidence.push(python_stdlib_collection_factory_evidence(
        4,
        sp(85),
        contract,
        1,
        vec![EvidenceId(2)],
    ));
    let mut builder = Builder::new(&il, &interner);
    builder.seed_module_value_bindings();
    let raw = builder.eval(call, &FxHashMap::default());
    let admitted = builder
        .proven_collection_value(raw)
        .expect("namespace LibraryApi evidence should survive seeded import values");
    assert!(matches!(
        builder.nodes[admitted as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
}
