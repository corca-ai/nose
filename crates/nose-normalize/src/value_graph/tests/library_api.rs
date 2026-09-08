use super::support::*;

#[test]
fn free_name_collection_factory_value_graph_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(20),
        &[],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(21), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(22),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, seq]);
    let root = b.add(NodeKind::Block, Payload::None, sp(19), &[call]);
    let mut il = finish_test_il(b, root, Lang::Python);
    il.push_evidence(collection_sequence_evidence(0, Lang::Python, sp(22)));
    il.push_evidence(language_core_symbol_evidence(
        1,
        Lang::Python,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
    ));
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "symbol proof alone must not prove the migrated free-name factory"
    );

    let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    il.push_evidence(python_builtin_collection_factory_evidence(
        2,
        sp(23),
        contract,
        1,
        vec![EvidenceId(1)],
    ));
    assert!(matches!(
        eval_proven_collection_op(&il, &interner, call),
        Some(ValOp::Seq(SEQ_VALUE_COLLECTION))
    ));
}

#[test]
fn free_name_minmax_value_graph_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("min")),
        sp(24),
        &[],
    );
    let left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(25), &[]);
    let right = b.add(NodeKind::Lit, Payload::LitInt(2), sp(26), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(27),
        &[callee, left, right],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(23), &[call]);
    let mut il = finish_test_il(b, root, Lang::Python);
    il.push_evidence(language_core_symbol_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::node(sp(24), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("min"),
        },
    ));
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == MIN_CODE),
        "symbol proof alone must not prove the migrated Python min builtin"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "min", 2).unwrap();
    il.push_evidence(library_api_contract_evidence(
        1,
        sp(27),
        contract.id,
        contract.callee,
        2,
        vec![EvidenceId(0)],
    ));
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Bin(op) if op == MIN_CODE
    ));
}

#[test]
fn scalar_integer_method_value_graph_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let x = interner.intern("x");
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(160), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(161), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("clamp")),
        sp(162),
        &[receiver],
    );
    let lo = b.add(NodeKind::Lit, Payload::LitInt(0), sp(163), &[]);
    let hi = b.add(NodeKind::Lit, Payload::LitInt(10), sp(164), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(165), &[callee, lo, hi]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(166), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(166), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp(160), &[param, body]);
    let root = b.add(NodeKind::Module, Payload::None, sp(159), &[func]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.rs".into(),
            lang: Lang::Rust,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(interner.intern("f")),
            origin: Default::default(),
        }],
        vec![x],
    );
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::param(sp(160)),
        EvidenceKind::Domain(DomainEvidence::Integer),
    ));
    il.push_evidence(evidence(
        1,
        EvidenceAnchor::node(sp(161), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Integer),
    ));

    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[raw as usize].op, ValOp::Clamp),
        "raw Rust clamp selector plus integer receiver is not enough"
    );

    let contract = library_scalar_integer_method_contract(Lang::Rust, "clamp", 2).unwrap();
    push_library_api_evidence_for_callee(
        &mut il,
        &interner,
        2,
        call,
        contract.id,
        contract.callee,
        2,
    );
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    let admitted = builder.eval(call, &FxHashMap::default());
    assert!(matches!(builder.nodes[admitted as usize].op, ValOp::Clamp));
}

#[test]
fn rust_some_wildcard_pattern_value_graph_requires_library_api_and_source_pattern_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(167), &[]);
    let some = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Some")),
        sp(168),
        &[],
    );
    let pattern = b.add(
        NodeKind::Raw,
        Payload::Name(interner.intern("tuple_struct_pattern")),
        sp(170),
        &[some],
    );
    let cond = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Eq),
        sp(171),
        &[value, pattern],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(166), &[cond]);
    let mut il = finish_test_il(b, root, Lang::Rust);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(167), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Option),
    ));

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(cond, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[raw as usize].op, ValOp::Bin(op) if op == Op::Ne as u32),
        "raw Some pattern selector must not become an Option presence predicate"
    );

    let contract = library_rust_option_some_constructor_contract(Lang::Rust, "Some", 1)
        .expect("Rust Some contract");
    il.push_evidence(language_core_symbol_evidence(
        1,
        Lang::Rust,
        EvidenceAnchor::node(sp(168), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Some"),
        },
    ));
    il.push_evidence(rust_option_evidence_with_dependencies(
        2,
        EvidenceAnchor::node(sp(168), NodeKind::Var),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 1,
        }),
        vec![EvidenceId(1)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let api_only = builder.eval(cond, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[api_only as usize].op, ValOp::Bin(op) if op == Op::Ne as u32),
        "admitted Some API proof without Rust wildcard pattern source proof must stay closed"
    );

    push_source_pattern(
        &mut il,
        3,
        sp(170),
        SourcePatternKind::RustTupleStructSingleWildcardPattern,
    );
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(cond, &FxHashMap::default());
    let node = &builder.nodes[proven as usize];
    assert!(matches!(node.op, ValOp::Bin(op) if op == Op::Ne as u32));
    assert!(
        node.args.iter().any(|&arg| matches!(
            builder.nodes[arg as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        )),
        "admitted Rust Some wildcard pattern should evaluate as non-null Option presence"
    );
}

#[test]
fn rust_option_none_pattern_value_graph_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp(171), &[]);
    let none = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("None")),
        sp(172),
        &[],
    );
    let cond = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Eq),
        sp(173),
        &[value, none],
    );
    let then_value = b.add(NodeKind::Lit, Payload::LitBool(true), sp(174), &[]);
    let else_value = b.add(NodeKind::Lit, Payload::LitBool(false), sp(175), &[]);
    let then_block = b.add(NodeKind::Block, Payload::None, sp(174), &[then_value]);
    let else_block = b.add(NodeKind::Block, Payload::None, sp(175), &[else_value]);
    let if_expr = b.add(
        NodeKind::If,
        Payload::None,
        sp(176),
        &[cond, then_block, else_block],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(170), &[if_expr]);
    let mut il = finish_test_il(b, root, Lang::Rust);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(if_expr, &FxHashMap::default());
    let raw_node = &builder.nodes[raw as usize];
    assert!(
        !raw_node.args.iter().any(|&arg| matches!(
            builder.nodes[arg as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        )),
        "raw None selector must not become a null predicate"
    );

    let contract = library_rust_option_none_sentinel_contract(Lang::Rust, "None").unwrap();
    il.push_evidence(language_core_symbol_evidence(
        0,
        Lang::Rust,
        EvidenceAnchor::node(sp(172), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("None"),
        },
    ));
    il.push_evidence(rust_option_evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(172), NodeKind::Var),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        vec![EvidenceId(0)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(if_expr, &FxHashMap::default());
    let node = &builder.nodes[proven as usize];
    assert!(matches!(node.op, ValOp::Bin(op) if op == Op::Eq as u32));
    assert!(
        node.args.iter().any(|&arg| matches!(
            builder.nodes[arg as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        )),
        "admitted Rust None occurrence should evaluate as the null sentinel"
    );
}
