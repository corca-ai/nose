use super::*;
use crate::strict_exact::{
    strict_exact_promise_resolve_factory_safe, strict_exact_python_collection_factory_safe,
    strict_exact_safe_tree, strict_exact_set_constructor_collection_safe,
};
use nose_normalize::{normalize, NormalizeOptions};
use nose_semantics::{
    admitted_promise_resolve_at_call, library_free_name_collection_factory_contract,
    library_js_like_map_constructor_contract, library_js_like_set_constructor_contract,
};

#[test]
fn strict_exact_js_constructor_requires_library_api_evidence() {
    let interner = Interner::new();
    let (mut il, call) = js_new_set_il(&interner);
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));

    let wrong = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
    il.push_evidence(library_api_contract_evidence(
        3,
        sp(13),
        wrong.id,
        wrong.callee,
        1,
        vec![EvidenceId(0), EvidenceId(1)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));

    let (mut il, call) = js_new_set_il(&interner);
    let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    il.push_evidence(js_like_builtin_collection_constructor_evidence(
        3,
        sp(13),
        set.id,
        set.callee,
        1,
        vec![EvidenceId(0), EvidenceId(1)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_set_constructor_collection_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_python_builtin_factory_requires_library_api_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(40),
        &[],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(41), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(42),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(43), &[callee, seq]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.push_evidence(sequence_surface_evidence(
        0,
        Lang::Python,
        sp(42),
        SequenceSurfaceKind::Collection,
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_python_collection_factory_safe(
        &il, &interner, &facts, call
    ));

    let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    il.push_evidence(language_core_symbol_evidence(
        1,
        Lang::Python,
        EvidenceAnchor::node(sp(40), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
        Vec::new(),
    ));
    il.push_evidence(python_builtin_collection_factory_evidence(
        2,
        sp(43),
        contract,
        1,
        vec![EvidenceId(1)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_python_collection_factory_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_promise_resolve_requires_non_thenable_safe_arg() {
    let interner = Interner::new();
    let literal = normalized_source(
        "t.js",
        "function f() { return Promise.resolve(1); }\n",
        Lang::JavaScript,
        &interner,
    );
    let literal_call = promise_resolve_call(&literal, &interner);
    let facts = StrictFacts::collect(&literal, &interner);
    assert!(strict_exact_promise_resolve_factory_safe(
        &literal,
        &interner,
        &facts,
        literal_call
    ));
    assert!(strict_exact_safe_tree(
        &literal,
        &interner,
        &facts,
        literal_call
    ));

    let typed_scalar = normalized_source(
        "t.ts",
        "function f(x: number) { return Promise.resolve(x); }\n",
        Lang::TypeScript,
        &interner,
    );
    let typed_scalar_call = promise_resolve_call(&typed_scalar, &interner);
    let facts = StrictFacts::collect(&typed_scalar, &interner);
    assert!(strict_exact_promise_resolve_factory_safe(
        &typed_scalar,
        &interner,
        &facts,
        typed_scalar_call
    ));

    let untyped = normalized_source(
        "t.js",
        "function f(x) { return Promise.resolve(x); }\n",
        Lang::JavaScript,
        &interner,
    );
    let untyped_call = promise_resolve_call(&untyped, &interner);
    let facts = StrictFacts::collect(&untyped, &interner);
    assert!(!strict_exact_promise_resolve_factory_safe(
        &untyped,
        &interner,
        &facts,
        untyped_call
    ));

    let promise_like = normalized_source(
        "t.js",
        "function f() { return Promise.resolve(Promise.resolve(1)); }\n",
        Lang::JavaScript,
        &interner,
    );
    let promise_like_call =
        promise_resolve_call_with_arg_kind(&promise_like, &interner, NodeKind::Call);
    let facts = StrictFacts::collect(&promise_like, &interner);
    assert!(!strict_exact_promise_resolve_factory_safe(
        &promise_like,
        &interner,
        &facts,
        promise_like_call
    ));
}

#[test]
fn strict_exact_promise_continuations_remain_closed_for_reporting() {
    let interner = Interner::new();
    for (method, src) in [
        (
            "then",
            "function f(p, ok, err) { return p.then(ok, err); }\n",
        ),
        ("catch", "function f(p, h) { return p.catch(h); }\n"),
        ("finally", "function f(p, h) { return p.finally(h); }\n"),
        (
            "then",
            "function f(db, id, h) { return db.get(id).then(h); }\n",
        ),
    ] {
        let il = normalized_source("t.js", src, Lang::JavaScript, &interner);
        let call = method_call(&il, &interner, method);
        let facts = StrictFacts::collect(&il, &interner);

        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "Promise continuation selector {method} should remain closed until receiver, channel, and callback obligations are proven"
        );
        let root = il
            .units
            .iter()
            .find_map(|unit| (il.kind(unit.root) == NodeKind::Func).then_some(unit.root))
            .expect("function root");
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, root),
            "function containing Promise continuation selector {method} should produce recall-loss admission rejection"
        );
    }
}

fn normalized_source(path: &str, src: &str, lang: Lang, interner: &Interner) -> Il {
    let raw = nose_frontend::lower_source(FileId(0), path, src.as_bytes(), lang, interner)
        .expect("lower source");
    normalize(&raw, interner, &NormalizeOptions::default())
}

fn promise_resolve_call(il: &Il, interner: &Interner) -> NodeId {
    promise_resolve_call_matching(il, interner, |_| true)
}

fn method_call(il: &Il, interner: &Interner, method: &str) -> NodeId {
    il.nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Call).then_some(NodeId(idx as u32)))
        .find(|&call| {
            il.children(call).first().is_some_and(|&callee| {
                il.kind(callee) == NodeKind::Field
                    && matches!(
                        il.node(callee).payload,
                        Payload::Name(name) if interner.resolve(name) == method
                    )
            })
        })
        .unwrap_or_else(|| panic!("method call {method}"))
}

fn promise_resolve_call_with_arg_kind(il: &Il, interner: &Interner, arg_kind: NodeKind) -> NodeId {
    promise_resolve_call_matching(il, interner, |call| {
        il.children(call)
            .get(1)
            .is_some_and(|arg| il.kind(*arg) == arg_kind)
    })
}

fn promise_resolve_call_matching(
    il: &Il,
    interner: &Interner,
    matches_call: impl Fn(NodeId) -> bool,
) -> NodeId {
    let calls: Vec<_> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Call).then_some(NodeId(idx as u32)))
        .collect();
    calls
        .iter()
        .copied()
        .find(|&call| {
            admitted_promise_resolve_at_call(il, interner, call).is_some() && matches_call(call)
        })
        .unwrap_or_else(|| {
            let call_spans: Vec<_> = calls.iter().map(|&call| il.node(call).span).collect();
            let api_count = il
                .evidence
                .iter()
                .filter(|record| matches!(record.kind, EvidenceKind::LibraryApi(_)))
                .count();
            let qualified_count = il
                .evidence
                .iter()
                .filter(|record| {
                    matches!(
                        record.kind,
                        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { .. })
                    )
                })
                .count();
            panic!(
                "admitted Promise.resolve call; calls={call_spans:?}, library_api={api_count}, qualified_global={qualified_count}"
            )
        })
}
