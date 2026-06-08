use super::*;
use nose_il::{
    EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta, GuardEvidenceKind,
    IlBuilder, ImportEvidenceKind, JsRecordGuardComparison, JsRecordGuardNullCheck, Lang,
    LibraryApiEvidenceKind, LitClass, ParamSemantic, SequenceSurfaceKind, SourceCallKind,
    SourceComprehensionKind, SourceFactKind, Span, SymbolEvidenceKind, Unit, UnitKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_free_function_builtin_contract, library_free_name_collection_factory_contract,
    library_imported_collection_factory_contract, library_java_collection_constructor_contract,
    library_java_collection_factory_contract, library_java_map_factory_contract,
    library_js_like_map_constructor_contract, library_js_like_set_constructor_contract,
    library_method_call_contract, library_rust_option_none_sentinel_contract,
    library_rust_option_some_constructor_contract, library_scalar_integer_method_contract,
    library_static_index_membership_contract, LibraryApiContractId, FIRST_PARTY_PACK_ID,
};

fn sp(line: u32) -> Span {
    Span::new(FileId(0), line, line, line, line)
}

fn finish_test_il(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
    builder.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    )
}

fn evidence(id: u32, anchor: EvidenceAnchor, kind: EvidenceKind) -> EvidenceRecord {
    evidence_with_dependencies(id, anchor, kind, Vec::new())
}

fn evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::FirstParty,
            pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("test")),
        },
        dependencies,
        status: EvidenceStatus::Asserted,
    }
}

fn imported_binding_symbol(module: &str, exported: &str) -> EvidenceKind {
    EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    })
}

fn imported_namespace_symbol_kind(module: &str) -> EvidenceKind {
    EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    })
}

fn push_imported_binding_use(
    il: &mut Il,
    binding_id: u32,
    binding_span: Span,
    occurrence_id: u32,
    occurrence_span: Span,
    module: &str,
    exported: &str,
) {
    let symbol = imported_binding_symbol(module, exported);
    il.evidence.push(evidence(
        binding_id,
        EvidenceAnchor::binding(binding_span, stable_symbol_hash(exported)),
        symbol,
    ));
    il.evidence.push(evidence_with_dependencies(
        occurrence_id,
        EvidenceAnchor::node(occurrence_span, NodeKind::Var),
        symbol,
        vec![EvidenceId(binding_id)],
    ));
}

fn push_imported_namespace_use(
    il: &mut Il,
    binding_id: u32,
    binding_span: Span,
    occurrence_id: u32,
    occurrence_span: Span,
    module: &str,
) {
    let symbol = imported_namespace_symbol_kind(module);
    il.evidence.push(evidence(
        binding_id,
        EvidenceAnchor::binding(binding_span, stable_symbol_hash(module)),
        symbol,
    ));
    il.evidence.push(evidence_with_dependencies(
        occurrence_id,
        EvidenceAnchor::node(occurrence_span, NodeKind::Var),
        symbol,
        vec![EvidenceId(binding_id)],
    ));
}

fn collection_sequence_evidence(id: u32, span: Span) -> EvidenceRecord {
    evidence(
        id,
        EvidenceAnchor::sequence(span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
    )
}

fn identity_lambda(builder: &mut IlBuilder, param_cid: u32, span: Span) -> NodeId {
    let param = builder.add(NodeKind::Param, Payload::Cid(param_cid), span, &[]);
    let value = builder.add(NodeKind::Var, Payload::Cid(param_cid), span, &[]);
    let ret = builder.add(NodeKind::Return, Payload::None, span, &[value]);
    let block = builder.add(NodeKind::Block, Payload::None, span, &[ret]);
    builder.add(NodeKind::Lambda, Payload::None, span, &[param, block])
}

fn const_bool_lambda(builder: &mut IlBuilder, param_cid: u32, value: bool, span: Span) -> NodeId {
    let param = builder.add(NodeKind::Param, Payload::Cid(param_cid), span, &[]);
    let value = builder.add(NodeKind::Lit, Payload::LitBool(value), span, &[]);
    let ret = builder.add(NodeKind::Return, Payload::None, span, &[value]);
    let block = builder.add(NodeKind::Block, Payload::None, span, &[ret]);
    builder.add(NodeKind::Lambda, Payload::None, span, &[param, block])
}

fn push_source_comprehension(il: &mut Il, id: u32, span: Span, kind: SourceComprehensionKind) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Source(SourceFactKind::Comprehension(kind)),
    ));
}

fn push_source_cast(il: &mut Il, id: u32, span: Span, kind: SourceCastKind) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::source_span(span),
        EvidenceKind::Source(SourceFactKind::Cast(kind)),
    ));
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
fn builder_append_candidate_requires_contract_and_seed_context() {
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
    let il = finish_test_il(b, body, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let seed = builder.mk(ValOp::Input(0), vec![]);
    let mut env = FxHashMap::default();
    env.insert(1, seed);
    assert!(
        builder.builder_candidates(body, &env).is_empty(),
        "a builder-append method contract without a collection seed must not prove a list builder"
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

    il.evidence.push(evidence(
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
        !matches!(builder.nodes[raw as usize].op, ValOp::Const(k) if k == LitClass::Null as u32),
        "raw undefined spelling must not prove the nullish constant"
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(1), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("undefined"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(var, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[proven as usize].op, ValOp::Const(k) if k == LitClass::Null as u32),
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

    il.evidence.push(collection_sequence_evidence(0, sp(21)));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(seq, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::Seq(SEQ_VALUE_COLLECTION)
    ));
}

fn library_api_contract_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        dependencies,
    )
}

fn push_method_call_library_api_evidence(
    il: &mut Il,
    interner: &Interner,
    id: u32,
    call: NodeId,
    method: &str,
    arity: usize,
) {
    let contract =
        library_method_call_contract(il.meta.lang, method, arity).expect("method contract");
    let dependencies = nose_semantics::library_api_receiver_dependencies_for_call(
        il,
        interner,
        call,
        contract.callee,
    )
    .expect("method receiver dependencies");
    il.evidence.push(library_api_contract_evidence(
        id,
        il.node(call).span,
        contract.id,
        contract.callee,
        arity as u16,
        dependencies,
    ));
}

fn push_library_api_evidence_for_callee(
    il: &mut Il,
    interner: &Interner,
    id: u32,
    call: NodeId,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) {
    let dependencies =
        nose_semantics::library_api_receiver_dependencies_for_call(il, interner, call, callee)
            .expect("library api receiver dependencies");
    il.evidence.push(library_api_contract_evidence(
        id,
        il.node(call).span,
        contract_id,
        callee,
        arity,
        dependencies,
    ));
}

fn eval_proven_collection_op(il: &Il, interner: &Interner, call: NodeId) -> Option<ValOp> {
    let mut builder = Builder::new(il, interner);
    let raw = builder.eval(call, &FxHashMap::default());
    builder
        .proven_collection_value(raw)
        .map(|value| builder.nodes[value as usize].op.clone())
}

fn receiver_domain_contains_call_il() -> (Il, Interner, NodeId, Span) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver_span = sp(30);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        receiver_span,
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(31),
        &[receiver],
    );
    let item = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("item")),
        sp(32),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(33), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(29), &[call]);
    let il = finish_test_il(b, root, Lang::TypeScript);
    (il, interner, call, receiver_span)
}

fn eval_op(il: &Il, interner: &Interner, node: NodeId) -> ValOp {
    let mut builder = Builder::new(il, interner);
    let value = builder.eval(node, &FxHashMap::default());
    builder.nodes[value as usize].op.clone()
}

#[test]
fn membership_call_consumes_receiver_domain_evidence() {
    let (mut il, interner, call, receiver_span) = receiver_domain_contains_call_il();
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "method selector alone must not prove collection membership"
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    push_method_call_library_api_evidence(&mut il, &interner, 1, call, "includes", 1);
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Bin(op) if op == Op::In as u32
    ));

    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
    ));
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "conflicting receiver-domain evidence must close the exact membership rewrite"
    );
}

#[test]
fn membership_call_consumes_library_api_result_domain_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let factory_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let seed = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(41),
        &[],
    );
    let receiver = b.add(
        NodeKind::Call,
        Payload::None,
        sp(42),
        &[factory_callee, seed],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(43),
        &[receiver],
    );
    let item = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("item")),
        sp(44),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(45), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "call-result receiver must not be collection-like without domain evidence"
    );

    let api = library_js_like_set_constructor_contract(Lang::TypeScript, "Set").unwrap();
    il.evidence.push(library_api_contract_evidence(
        0,
        sp(42),
        api.id,
        api.callee,
        1,
        Vec::new(),
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(42), NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Set),
        vec![EvidenceId(0)],
    ));
    push_method_call_library_api_evidence(&mut il, &interner, 2, call, "includes", 1);
    assert!(matches!(
        eval_op(&il, &interner, call),
        ValOp::Bin(op) if op == Op::In as u32
    ));

    il.evidence[0].status = EvidenceStatus::Ambiguous;
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == Op::In as u32),
        "ambiguous LibraryApi dependency must close the call-result receiver proof"
    );
}

fn node_with_span(il: &Il, kind: NodeKind, span: Span) -> NodeId {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == kind && node.span == span).then_some(NodeId(idx as u32))
        })
        .expect("node with requested span")
}

#[derive(Clone, Copy)]
enum BindingMembershipCase {
    Visible,
    Late,
    MutatedVisible,
}

fn binding_assignment(b: &mut IlBuilder, xs: Symbol, array: Symbol, line: u32) -> (NodeId, Span) {
    let lhs = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let seq_span = sp(line + 1);
    let seq = b.add(NodeKind::Seq, Payload::Name(array), seq_span, &[]);
    (
        b.add(NodeKind::Assign, Payload::None, sp(line), &[lhs, seq]),
        seq_span,
    )
}

fn binding_membership_call(
    b: &mut IlBuilder,
    xs: Symbol,
    item_name: Symbol,
    includes: Symbol,
    line: u32,
) -> (NodeId, Span) {
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(includes),
        sp(line + 1),
        &[receiver],
    );
    let item = b.add(NodeKind::Var, Payload::Name(item_name), sp(line + 2), &[]);
    let call_span = sp(line + 3);
    (
        b.add(NodeKind::Call, Payload::None, call_span, &[callee, item]),
        call_span,
    )
}

fn binding_append(b: &mut IlBuilder, xs: Symbol, line: u32) -> NodeId {
    let append_receiver = b.add(NodeKind::Var, Payload::Name(xs), sp(line), &[]);
    let appended = b.add(NodeKind::Lit, Payload::LitInt(1), sp(line), &[]);
    b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        sp(line),
        &[append_receiver, appended],
    )
}

fn normalized_binding_membership_op(case: BindingMembershipCase) -> ValOp {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let item_name = interner.intern("item");
    let includes = interner.intern("includes");
    let array = interner.intern("array");
    let mut b = IlBuilder::new(FileId(0));
    let ((root_children, seq_span), call_span) = match case {
        BindingMembershipCase::Visible => {
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 10);
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 12);
            ((vec![assign, call], seq_span), call_span)
        }
        BindingMembershipCase::Late => {
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 12);
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 20);
            ((vec![call, assign], seq_span), call_span)
        }
        BindingMembershipCase::MutatedVisible => {
            let (assign, seq_span) = binding_assignment(&mut b, xs, array, 20);
            let append = binding_append(&mut b, xs, 22);
            let (call, call_span) = binding_membership_call(&mut b, xs, item_name, includes, 23);
            ((vec![assign, append, call], seq_span), call_span)
        }
    };
    let body = b.add(NodeKind::Block, Payload::None, sp(9), &root_children);
    let root = b.add(NodeKind::Func, Payload::None, sp(8), &[body]);
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    il.evidence.push(collection_sequence_evidence(0, seq_span));
    let normalized = crate::normalize(
        &il,
        &interner,
        &crate::NormalizeOptions {
            cfg_norm: false,
            dataflow: false,
            dce: false,
            oracle: false,
        },
    );
    let normalized_call = node_with_span(&normalized, NodeKind::Call, call_span);
    eval_op(&normalized, &interner, normalized_call)
}

#[test]
fn membership_call_consumes_normalized_binding_domain_evidence() {
    assert!(matches!(
        normalized_binding_membership_op(BindingMembershipCase::Visible),
        ValOp::Bin(op) if op == Op::In as u32
    ));
}

#[test]
fn membership_call_rejects_binding_domain_after_receiver_use() {
    assert!(
        !matches!(
            normalized_binding_membership_op(BindingMembershipCase::Late),
            ValOp::Bin(op) if op == Op::In as u32
        ),
        "binding-domain evidence must not prove use-before-assignment receivers"
    );
}

#[test]
fn mutated_binding_domain_evidence_keeps_membership_rewrite_closed() {
    assert!(
        !matches!(
            normalized_binding_membership_op(BindingMembershipCase::MutatedVisible),
            ValOp::Bin(op) if op == Op::In as u32
        ),
        "mutated binding must not receive binding-domain evidence"
    );
}

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
    il.evidence.push(collection_sequence_evidence(0, sp(22)));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        }),
    ));
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "symbol proof alone must not prove the migrated free-name factory"
    );

    let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    il.evidence.push(library_api_contract_evidence(
        2,
        sp(23),
        contract.id,
        contract.callee,
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
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(24), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("min"),
        }),
    ));
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Bin(op) if op == MIN_CODE),
        "symbol proof alone must not prove the migrated Python min builtin"
    );

    let contract = library_free_function_builtin_contract(Lang::Python, "min", 2).unwrap();
    il.evidence.push(library_api_contract_evidence(
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
        }],
        vec![x],
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(160)),
        EvidenceKind::Domain(DomainEvidence::Integer),
    ));
    il.evidence.push(evidence(
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
fn rust_some_wildcard_pattern_value_graph_requires_library_api_evidence() {
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
    il.evidence.push(evidence(
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
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(168), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Some"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
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
    let proven = builder.eval(cond, &FxHashMap::default());
    let node = &builder.nodes[proven as usize];
    assert!(matches!(node.op, ValOp::Bin(op) if op == Op::Ne as u32));
    assert!(
        node.args
            .iter()
            .any(|&arg| matches!(builder.nodes[arg as usize].op, ValOp::Const(k) if k == LitClass::Null as u32)),
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
        !raw_node
            .args
            .iter()
            .any(|&arg| matches!(builder.nodes[arg as usize].op, ValOp::Const(k) if k == LitClass::Null as u32)),
        "raw None selector must not become a null predicate"
    );

    let contract = library_rust_option_none_sentinel_contract(Lang::Rust, "None").unwrap();
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(172), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("None"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
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
        node.args
            .iter()
            .any(|&arg| matches!(builder.nodes[arg as usize].op, ValOp::Const(k) if k == LitClass::Null as u32)),
        "admitted Rust None occurrence should evaluate as the null sentinel"
    );
}

#[derive(Clone, Copy)]
enum ClampShape {
    MinMax,
    SwappedBounds,
    WrongNesting,
}

#[derive(Clone, Copy)]
enum GuardShape {
    None,
    Exiting,
    NonExiting,
}

fn param(b: &mut IlBuilder, cid: u32, line: u32) -> NodeId {
    b.add(NodeKind::Param, Payload::Cid(cid), sp(line), &[])
}

fn var(b: &mut IlBuilder, cid: u32) -> NodeId {
    b.add(NodeKind::Var, Payload::Cid(cid), sp(10 + cid), &[])
}

fn int_lit(b: &mut IlBuilder, value: i64) -> NodeId {
    b.add(NodeKind::Lit, Payload::LitInt(value), sp(20), &[])
}

fn builtin(b: &mut IlBuilder, op: Builtin, args: &[NodeId]) -> NodeId {
    b.add(
        NodeKind::Call,
        Payload::Builtin(op),
        sp(30 + b.len() as u32),
        args,
    )
}

fn push_canonical_java_minmax_builtin_evidence(il: &mut Il, first_id: u32) {
    let mut next_id = first_id;
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        let (Payload::Builtin(builtin), arg_count) =
            (il.node(node).payload, il.children(node).len())
        else {
            continue;
        };
        let method = match builtin {
            Builtin::Min => "min",
            Builtin::Max => "max",
            _ => continue,
        };
        let contract = library_method_call_contract(il.meta.lang, method, arg_count)
            .expect("min/max contract");
        il.evidence.push(library_api_contract_evidence(
            next_id,
            il.node(node).span,
            contract.id,
            contract.callee,
            arg_count as u16,
            Vec::new(),
        ));
        next_id += 1;
    }
}

fn clamp_expr(b: &mut IlBuilder, shape: ClampShape, x: NodeId, lo: NodeId, hi: NodeId) -> NodeId {
    match shape {
        ClampShape::MinMax => {
            let inner = builtin(b, Builtin::Max, &[x, lo]);
            builtin(b, Builtin::Min, &[inner, hi])
        }
        ClampShape::SwappedBounds => {
            let inner = builtin(b, Builtin::Max, &[x, hi]);
            builtin(b, Builtin::Min, &[inner, lo])
        }
        ClampShape::WrongNesting => {
            let inner = builtin(b, Builtin::Min, &[x, lo]);
            builtin(b, Builtin::Max, &[inner, hi])
        }
    }
}

fn guarded_function(
    guard: GuardShape,
    shape: ClampShape,
    semantics: [Option<ParamSemantic>; 3],
) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let plo = param(&mut b, 1, 2);
    let phi = param(&mut b, 2, 3);
    let mut stmts = Vec::new();
    if !matches!(guard, GuardShape::None) {
        let hi_guard = var(&mut b, 2);
        let lo_guard = var(&mut b, 1);
        let cond = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Lt),
            sp(4),
            &[hi_guard, lo_guard],
        );
        let then_stmt = match guard {
            GuardShape::Exiting => {
                let err = int_lit(&mut b, 0);
                b.add(NodeKind::Throw, Payload::None, sp(5), &[err])
            }
            GuardShape::NonExiting => {
                let err = int_lit(&mut b, 0);
                b.add(NodeKind::ExprStmt, Payload::None, sp(5), &[err])
            }
            GuardShape::None => unreachable!(),
        };
        let then_block = b.add(NodeKind::Block, Payload::None, sp(5), &[then_stmt]);
        stmts.push(b.add(NodeKind::If, Payload::None, sp(4), &[cond, then_block]));
    }
    let x = var(&mut b, 0);
    let lo = var(&mut b, 1);
    let hi = var(&mut b, 2);
    let expr = clamp_expr(&mut b, shape, x, lo, hi);
    let ret = b.add(NodeKind::Return, Payload::None, sp(6), &[expr]);
    stmts.push(ret);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &stmts);
    let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, plo, phi, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t.java".to_string(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: None,
        }],
        Vec::new(),
    );
    for (idx, semantic) in semantics.into_iter().enumerate() {
        if let Some(semantic) = semantic {
            il.evidence.push(evidence(
                idx as u32,
                EvidenceAnchor::param(sp(idx as u32 + 1)),
                EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            ));
        }
    }
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

fn literal_bound_function(shape: ClampShape, lo_value: i64, hi_value: i64) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let x = var(&mut b, 0);
    let lo = int_lit(&mut b, lo_value);
    let hi = int_lit(&mut b, hi_value);
    let expr = clamp_expr(&mut b, shape, x, lo, hi);
    let ret = b.add(NodeKind::Return, Payload::None, sp(1), &[expr]);
    let body = b.add(NodeKind::Block, Payload::None, sp(1), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t.java".to_string(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: None,
        }],
        Vec::new(),
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(1)),
        EvidenceKind::Domain(DomainEvidence::Integer),
    ));
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

#[test]
fn import_binding_value_requires_sequence_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(40),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(40),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(40), &[module, exported]);
    let root = b.add(NodeKind::Block, Payload::None, sp(40), &[binding]);
    let mut il = finish_test_il(b, root, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(binding, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[raw as usize].op,
        ValOp::Seq(SEQ_VALUE_UNTAGGED)
    ));
    assert!(!builder.is_import_binding_value(raw, "collections", "deque"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(40)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(binding, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::ImportBinding { .. }
    ));
    assert!(builder.is_import_binding_value(proven, "collections", "deque"));
}

#[test]
fn namespace_member_import_binding_requires_proven_namespace_value() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let prod = interner.intern("prod");
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(50),
        &[],
    );
    let namespace = b.add(NodeKind::Seq, Payload::None, sp(50), &[module]);
    let field = b.add(NodeKind::Field, Payload::Name(prod), sp(51), &[namespace]);
    let root = b.add(NodeKind::Block, Payload::None, sp(50), &[field]);
    let mut il = finish_test_il(b, root, Lang::Python);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(field, &FxHashMap::default());
    assert!(matches!(builder.nodes[raw as usize].op, ValOp::Field(_)));
    assert!(!builder.is_import_binding_value(raw, "math", "prod"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(50)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(field, &FxHashMap::default());
    assert!(builder.is_import_binding_value(proven, "math", "prod"));
}

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
    il.evidence.push(collection_sequence_evidence(2, sp(63)));
    assert!(
        eval_proven_collection_op(&il, &interner, call).is_none(),
        "import symbol proof alone must not prove the migrated stdlib factory"
    );
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(64),
        contract.id,
        contract.callee,
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
    il.evidence.push(library_api_contract_evidence(
        2,
        sp(75),
        contract.id,
        contract.callee,
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
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(81),
        contract.id,
        contract.callee,
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
    il.evidence.push(collection_sequence_evidence(0, sp(91)));
    assert!(
        !matches!(eval_op(&il, &interner, comparison), ValOp::Bin(op) if op == Op::In as u32),
        "static array receiver proof alone must not prove indexOf membership"
    );

    let contract =
        library_static_index_membership_contract(Lang::JavaScript, "indexOf", 1).unwrap();
    il.evidence.push(library_api_contract_evidence(
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

#[test]
fn java_map_factory_value_graph_uses_library_api_after_import_seed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let map = interner.intern("Map");
    let lookup = interner.intern("LOOKUP");
    let import_lhs = b.add(NodeKind::Var, Payload::Name(map), sp(100), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("java.util")),
        sp(100),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("Map")),
        sp(100),
        &[],
    );
    let import_rhs = b.add(NodeKind::Seq, Payload::None, sp(100), &[module, exported]);
    let import = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(100),
        &[import_lhs, import_rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(map), sp(101), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(102),
        &[receiver],
    );
    let red = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(103),
        &[],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(104), &[]);
    let blue = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("blue")),
        sp(105),
        &[],
    );
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp(106), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(107),
        &[callee, red, one, blue, two],
    );
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
    let contract =
        library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(100)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
        }),
    ));
    push_imported_binding_use(&mut il, 1, sp(100), 2, sp(101), "java.util", "Map");
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(107),
        contract.id,
        contract.callee,
        4,
        vec![EvidenceId(2)],
    ));
    il.evidence.push(evidence_with_dependencies(
        4,
        EvidenceAnchor::node(sp(107), NodeKind::Call),
        EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
            module_hash: stable_symbol_hash("LookupProvider"),
            exported_hash: stable_symbol_hash("LOOKUP"),
            root_kind: NodeKind::Call,
        }),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        5,
        EvidenceAnchor::node(sp(107), NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        6,
        EvidenceAnchor::binding(sp(108), stable_symbol_hash("LOOKUP")),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(5)],
    ));

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

    let import_lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(130), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("java.util")),
        sp(130),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("Map")),
        sp(130),
        &[],
    );
    let import_rhs = b.add(NodeKind::Seq, Payload::None, sp(130), &[module, exported]);
    let import = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(130),
        &[import_lhs, import_rhs],
    );

    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(131), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(132),
        &[receiver],
    );
    let red = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(133),
        &[],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(134), &[]);
    let blue = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("blue")),
        sp(135),
        &[],
    );
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp(136), &[]);
    let map_of = b.add(
        NodeKind::Call,
        Payload::None,
        sp(137),
        &[callee, red, one, blue, two],
    );
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
        }],
        vec![map, lookup],
    );
    let contract =
        library_java_map_factory_contract(Lang::Java, "Map", "of").expect("Map.of contract");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(130)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
        }),
    ));
    push_imported_binding_use(&mut il, 1, sp(130), 2, sp(131), "java.util", "Map");
    il.evidence.push(library_api_contract_evidence(
        3,
        sp(137),
        contract.id,
        contract.callee,
        4,
        vec![EvidenceId(2)],
    ));
    il.evidence.push(evidence_with_dependencies(
        4,
        EvidenceAnchor::node(sp(137), NodeKind::Call),
        EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
            module_hash: stable_symbol_hash("Tables"),
            exported_hash: stable_symbol_hash("LOOKUP"),
            root_kind: NodeKind::Call,
        }),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        5,
        EvidenceAnchor::node(sp(137), NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(3)],
    ));
    il.evidence.push(evidence_with_dependencies(
        6,
        EvidenceAnchor::binding(sp(138), stable_symbol_hash("LOOKUP")),
        EvidenceKind::Domain(DomainEvidence::Map),
        vec![EvidenceId(5)],
    ));
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
        "raw Name assignments need first-party import or imported-literal evidence"
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
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(80)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("collections"),
        }),
    ));
    push_imported_namespace_use(&mut il, 1, sp(80), 2, sp(81), "collections");
    il.evidence.push(collection_sequence_evidence(3, sp(84)));
    let mut builder = Builder::new(&il, &interner);
    builder.seed_module_value_bindings();
    let raw = builder.eval(call, &FxHashMap::default());
    assert!(
        builder.proven_collection_value(raw).is_none(),
        "namespace import proof alone must not prove the migrated stdlib factory"
    );
    il.evidence.push(library_api_contract_evidence(
        4,
        sp(85),
        contract.id,
        contract.callee,
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

#[test]
fn record_guard_value_tag_requires_guard_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("record_guard");
    let subject = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(60),
        &[],
    );
    let object = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("object")),
        sp(60),
        &[],
    );
    let non_null = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("non_null")),
        sp(60),
        &[],
    );
    let not_array = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("not_array")),
        sp(60),
        &[],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(60),
        &[subject, object, non_null, not_array],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(60), &[guard]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);

    let mut builder = Builder::new(&il, &interner);
    let raw = builder.eval(guard, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[raw as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(60)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
    ));
    let mut builder = Builder::new(&il, &interner);
    let surface_only = builder.eval(guard, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[surface_only as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(60)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(sp(60)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        vec![EvidenceId(1)],
    ));
    il.evidence.push(evidence_with_dependencies(
        3,
        EvidenceAnchor::sequence(sp(60)),
        EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
            subject_hash: stable_symbol_hash("value"),
            null_check: JsRecordGuardNullCheck::StrictNonNull,
            comparison: JsRecordGuardComparison::StrictOnly,
        }),
        vec![EvidenceId(2)],
    ));
    let mut builder = Builder::new(&il, &interner);
    let proven = builder.eval(guard, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven as usize].op,
        ValOp::Seq(SEQ_VALUE_RECORD_GUARD)
    ));
}

#[test]
fn own_property_guard_value_tag_requires_node_shape_and_guard_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let tag = interner.intern("own_property_guard");
    let receiver = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("map")),
        sp(62),
        &[],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(62),
        &[],
    );
    let own = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("own")),
        sp(62),
        &[],
    );
    let present = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("present")),
        sp(62),
        &[],
    );
    let malformed_present = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("maybe")),
        sp(62),
        &[],
    );
    let malformed = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(62),
        &[receiver, key, own, malformed_present],
    );
    let guard = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        sp(63),
        &[receiver, key, own, present],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(62), &[malformed, guard]);
    let mut il = finish_test_il(b, root, Lang::JavaScript);
    for (id, span) in [(0, sp(62)), (4, sp(63))] {
        il.evidence.push(evidence(
            id,
            EvidenceAnchor::sequence(span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
        ));
    }
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(62)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Object"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::source_span(sp(62)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(1)],
    ));
    il.evidence.push(evidence_with_dependencies(
        3,
        EvidenceAnchor::sequence(sp(62)),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(2)],
    ));
    il.evidence.push(evidence(
        5,
        EvidenceAnchor::source_span(sp(63)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Object"),
        }),
    ));
    il.evidence.push(evidence_with_dependencies(
        6,
        EvidenceAnchor::source_span(sp(63)),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(5)],
    ));
    il.evidence.push(evidence_with_dependencies(
        7,
        EvidenceAnchor::sequence(sp(63)),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash("Object.hasOwn"),
        }),
        vec![EvidenceId(6)],
    ));

    let mut builder = Builder::new(&il, &interner);
    let malformed_value = builder.eval(malformed, &FxHashMap::default());
    assert!(!matches!(
        builder.nodes[malformed_value as usize].op,
        ValOp::Seq(SEQ_VALUE_OWN_PROPERTY_GUARD)
    ));

    let proven_value = builder.eval(guard, &FxHashMap::default());
    assert!(matches!(
        builder.nodes[proven_value as usize].op,
        ValOp::Seq(SEQ_VALUE_OWN_PROPERTY_GUARD)
    ));
}

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
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(73)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(70), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        }),
    ));
    il.evidence.push(collection_sequence_evidence(2, sp(72)));
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
    il.evidence.push(library_api_contract_evidence(
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
    il.evidence.push(library_api_contract_evidence(
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
fn clamp_literal_bound_order_is_proof_backed_only_when_ordered() {
    assert_eq!(literal_bound_function(ClampShape::MinMax, 1, 10), (1, 1));
    assert_eq!(literal_bound_function(ClampShape::MinMax, 10, 1), (1, 0));
}

#[test]
fn clamp_guarded_bound_order_requires_exiting_inverse_guard() {
    let integer = Some(ParamSemantic::Integer);
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::MinMax, [integer; 3]),
        (1, 1)
    );
    assert_eq!(
        guarded_function(GuardShape::NonExiting, ClampShape::MinMax, [integer; 3]),
        (1, 0)
    );
    assert_eq!(
        guarded_function(GuardShape::None, ClampShape::MinMax, [integer; 3]),
        (1, 0)
    );
}

#[test]
fn clamp_proof_rejects_floatish_number_and_wrong_shapes() {
    let integer = Some(ParamSemantic::Integer);
    let number = Some(ParamSemantic::Number);
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::MinMax, [number; 3]),
        (1, 0),
        "float-sensitive Number params need a separate NaN/domain proof"
    );
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::SwappedBounds, [integer; 3]),
        (1, 0)
    );
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::WrongNesting, [integer; 3]),
        (1, 0)
    );
}
