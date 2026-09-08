use super::support::*;

#[test]
fn builder_append_candidate_requires_contract_or_effect_and_seed_context() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("push")),
        sp(2),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(3), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(4), &[field, item]);
    let stmt = b.add(NodeKind::ExprStmt, Payload::None, sp(4), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &[stmt]);
    let mut il = finish_test_il(b, body, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    let candidates = builder.builder_candidates(body, &env);
    assert!(
        candidates.is_empty(),
        "raw active-builder method selector must not prove append semantics"
    );

    let mut api_il = il.clone();
    api_il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(1), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    push_method_call_library_api_evidence(&mut api_il, &interner, 1, call, "push", 1);
    let mut builder = Builder::new(&api_il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    let candidates = builder.builder_candidates(body, &env);
    assert!(candidates
        .iter()
        .any(|candidate| candidate.cid == 1 && candidate.kind == BuilderKind::List));

    il.push_evidence(evidence(
        2,
        EvidenceAnchor::node(sp(4), NodeKind::Call),
        EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
    ));
    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Input(0), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "append-effect evidence without a collection seed must not prove a list builder"
    );

    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    let candidates = builder.builder_candidates(body, &env);
    assert!(candidates
        .iter()
        .any(|candidate| candidate.cid == 1 && candidate.kind == BuilderKind::List));

    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("add")),
        sp(2),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(3), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(4), &[field, item]);
    let stmt = b.add(NodeKind::ExprStmt, Payload::None, sp(4), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &[stmt]);
    let il = finish_test_il(b, body, Lang::JavaScript);
    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "a mutating method name that is not a list-builder append contract must stay closed"
    );
}

#[test]
fn map_builder_index_write_requires_write_evidence_and_map_seed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let base = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(2), &[base, key]);
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(3), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(4), &[target, value]);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &[assign]);
    let mut il = finish_test_il(b, body, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_MAP), vec![]);
    let mut env = FxHashMap::default();
    env.insert(2, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "raw index assignment shape must not prove a dict builder"
    );

    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(4), NodeKind::Assign),
        EvidenceKind::Effect(EffectEvidenceKind::BindingWrite),
    ));
    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_MAP), vec![]);
    let mut env = FxHashMap::default();
    env.insert(2, seed);
    let candidates = builder.builder_candidates(body, &env);
    assert!(candidates
        .iter()
        .any(|candidate| candidate.cid == 2 && candidate.kind == BuilderKind::Map));

    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
    let mut env = FxHashMap::default();
    env.insert(2, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "index writes require a proven map seed, not just any empty aggregate"
    );

    let mut unsupported = il.clone();
    unsupported.meta.lang = Lang::Ruby;
    let mut builder = Builder::new(&unsupported, &interner);
    let seed = builder.mk(ValOp::Seq(SEQ_VALUE_MAP), vec![]);
    let mut env = FxHashMap::default();
    env.insert(2, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "binding-write evidence plus map seed still needs a language index-write contract"
    );
}

#[test]
fn nullish_global_value_requires_symbol_evidence() {
    let interner = Interner::new();
    let undefined = interner.intern("undefined");
    let mut b = IlBuilder::new(FileId(0));
    let var = b.add(NodeKind::Var, Payload::Name(undefined), sp(1), &[]);
    let mut il = finish_test_il(b, var, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(var, &FxHashMap::default());
    assert!(
        !matches!(
            builder.nodes[raw as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        ),
        "raw undefined spelling must not prove the nullish constant"
    );

    il.push_evidence(language_core_evidence(
        0,
        Lang::JavaScript,
        EvidenceAnchor::node(sp(1), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("undefined"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(var, &FxHashMap::default());
    assert!(
        matches!(
            builder.nodes[proven as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        ),
        "symbol evidence should admit the nullish constant"
    );
}

#[test]
fn raw_sequence_tags_do_not_prove_value_graph_surfaces() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(20), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(21),
        &[one],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(19), &[seq]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(seq, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[raw as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));

    il.push_evidence(collection_sequence_evidence(0, Lang::JavaScript, sp(21)));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(seq, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
}
