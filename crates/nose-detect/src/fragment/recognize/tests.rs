use super::*;
use crate::fragment::{fragment_behavior, Place};
use crate::units::{build_parent_index, exact_statement_fragment_root};
use nose_il::{
    EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, Lang, Span,
};
use nose_normalize::{normalize, NormalizeOptions, Value};

/// Walk `il` exactly as the real fragment collector does (skipping `Lambda` subtrees),
/// applying `classify` to each node and collecting the accepted `(span, kind)` pairs.
fn index<F>(il: &Il, classify: &F) -> Vec<(Span, FragmentKind)>
where
    F: Fn(NodeId) -> Option<FragmentKind>,
{
    fn walk<F: Fn(NodeId) -> Option<FragmentKind>>(
        il: &Il,
        node: NodeId,
        classify: &F,
        out: &mut Vec<(Span, FragmentKind)>,
    ) {
        if il.kind(node) == NodeKind::Lambda {
            return;
        }
        if let Some(kind) = classify(node) {
            out.push((il.node(node).span, kind));
        }
        for &c in il.children(node) {
            walk(il, c, classify, out);
        }
    }
    let mut out = Vec::new();
    walk(il, il.root, classify, &mut out);
    out
}

fn sort_key(entry: &(Span, FragmentKind)) -> (u32, u32, &'static str) {
    (entry.0.start_byte, entry.0.end_byte, entry.1.reason_code())
}

/// The two paths must agree on the migrated kinds for `src`.
fn assert_paths_agree(src: &str, lang: Lang) {
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &interner)
        .expect("lowering should succeed");
    let il = normalize(&raw, &interner, &NormalizeOptions::default());
    let parents = build_parent_index(&il);

    let mut predicate: Vec<(Span, FragmentKind)> = index(&il, &|node| {
        exact_statement_fragment_root(&il, node, &parents, &interner)
    })
    .into_iter()
    .filter(|(_, kind)| MIGRATED.contains(kind))
    .collect();
    let mut contract: Vec<(Span, FragmentKind)> = index(&il, &|node| {
        recognize_contract(&il, node, &parents, &interner).map(|c| c.kind)
    });

    predicate.sort_by_key(sort_key);
    contract.sort_by_key(sort_key);
    assert_eq!(
        predicate, contract,
        "predicate and contract paths disagree on migrated fragments in `{src}`"
    );
}

fn contract_fragments(src: &str, lang: Lang) -> Vec<(Span, FragmentKind)> {
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &interner)
        .expect("lowering should succeed");
    let il = normalize(&raw, &interner, &NormalizeOptions::default());
    let parents = build_parent_index(&il);
    index(&il, &|node| {
        recognize_contract(&il, node, &parents, &interner).map(|c| c.kind)
    })
}

fn assert_contract_contains_kind(src: &str, lang: Lang, kind: FragmentKind) {
    let fragments = contract_fragments(src, lang);
    assert!(
        fragments.iter().any(|(_, actual)| *actual == kind),
        "expected {kind:?} in contract fragments for `{src}`, got {fragments:?}"
    );
}

fn assert_contract_excludes_kind(src: &str, lang: Lang, kind: FragmentKind) {
    let fragments = contract_fragments(src, lang);
    assert!(
        fragments.iter().all(|(_, actual)| *actual != kind),
        "did not expect {kind:?} in contract fragments for `{src}`, got {fragments:?}"
    );
}

fn index_assignment_node(il: &Il) -> NodeId {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            let id = NodeId(idx as u32);
            (node.kind == NodeKind::Assign
                && il
                    .children(id)
                    .first()
                    .is_some_and(|&target| il.kind(target) == NodeKind::Index))
            .then_some(id)
        })
        .expect("fixture should contain an index assignment")
}

fn add_effect_evidence(il: &mut Il, node: NodeId, kind: EffectEvidenceKind) {
    let id = EvidenceId(il.evidence.len() as u32);
    il.push_evidence(EvidenceRecord {
        id,
        anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        kind: EvidenceKind::Effect(kind),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::External,
            pack_hash: None,
            rule_hash: None,
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
}

#[test]
fn differential_direct_return_and_throw() {
    // Accepted: top-level computed return / throw.
    assert_paths_agree("function g(b){ return b*b + 1; }", Lang::JavaScript);
    assert_paths_agree("function f(a){ throw a + 1; }", Lang::JavaScript);
    assert_paths_agree("def h(a, c):\n    return a * a + c\n", Lang::Python);
}

#[test]
fn differential_rejects_match_for_non_fragments() {
    // `return x` (bare var) and `return 1` (bare lit) are not computed returns;
    // both paths must reject — yielding empty, equal sets.
    assert_paths_agree("function f(a){ return a; }", Lang::JavaScript);
    assert_paths_agree("function f(a){ return 1; }", Lang::JavaScript);
    // A preceding reassignment of the returned input invalidates context safety;
    // both paths must reject the return.
    assert_paths_agree(
        "function f(a){ a = a + 1; return a * a; }",
        Lang::JavaScript,
    );
}

#[test]
fn differential_index_self_field_and_expr_effects() {
    // Index-assignment effect (Go): a top-level `m[k] = v`.
    assert_paths_agree(
        "package p\nfunc f(m map[string]int, k string, v int) {\n\tm[k] = v\n}\n",
        Lang::Go,
    );
    // Java index-assignment and `this.field` write.
    assert_paths_agree(
        "class C { int[] a; void f(int i, int v){ a[i] = v; } }",
        Lang::Java,
    );
    assert_paths_agree(
        "class C { int x; void set(int v){ this.x = v + 1; } }",
        Lang::Java,
    );
    // Expression-statement effect: an append/push call.
    assert_paths_agree("function f(xs, v){ xs.push(v + 1); }", Lang::JavaScript);
    assert_paths_agree("def f(xs, v):\n    xs.append(v + 1)\n", Lang::Python);
}

#[test]
fn index_assignment_contract_uses_effect_evidence_gate() {
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(
        FileId(0),
        "t",
        b"class C { int[] a; void f(int i, int v){ a[i] = v; } }",
        Lang::Java,
        &interner,
    )
    .expect("lowering should succeed");
    let mut il = normalize(&raw, &interner, &NormalizeOptions::default());
    let parents = build_parent_index(&il);
    let assign = index_assignment_node(&il);
    assert_eq!(
        exact_statement_fragment_root(&il, assign, &parents, &interner),
        Some(FragmentKind::IndexAssignEffect)
    );
    assert_eq!(
        recognize_contract(&il, assign, &parents, &interner).map(|contract| contract.kind),
        Some(FragmentKind::IndexAssignEffect)
    );

    add_effect_evidence(
        &mut il,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash: 1 },
    );
    assert_eq!(
        exact_statement_fragment_root(&il, assign, &parents, &interner),
        None
    );
    assert!(recognize_contract(&il, assign, &parents, &interner).is_none());
}

#[test]
fn differential_conditional_guard() {
    // Accepted: direct return guards, bare returns, nested conditionals, branch-local
    // temp windows, and small ordered effect sequences. The contract recognizer
    // re-expresses the branch admissibility matrix independently of the predicate.
    assert_paths_agree(
        "function f(a){ if (a > 0) { return a * a; } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function f(a){ if (a > 0) { return; } else {} }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function f(a){ if (a > 0) { if (a > 10) { return a * 2; } } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function f(a){ if (a > 0) { const t = a * 2; return t + 1; } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function f(out, a){ if (a > 0) { out.push(1); out.push(2); } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "package p\nfunc f(out []int, a int) {\n\tif a > 0 {\n\t\tout[0] = a\n\t\tout[1] = a + 1\n\t}\n}\n",
        Lang::Go,
    );
    let ordered_self_fields =
        "class C { int x; int y; void f(boolean b, int a){ if (b) { this.x = a; this.y = a + 1; } } }";
    assert_paths_agree(ordered_self_fields, Lang::Java);
    assert_contract_contains_kind(
        ordered_self_fields,
        Lang::Java,
        FragmentKind::ConditionalGuard,
    );
    // Rejected by both paths: arbitrary multi-statement branch windows and wrong temp
    // consumption are still outside the exact fragment set.
    assert_paths_agree(
        "function f(a){ if (a > 0) { const t = a * 2; const u = a + 1; return t + u; } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function f(out, a){ if (a > 0) { const t = a * 2; out.push(a); } }",
        Lang::JavaScript,
    );
    let unproven_receiver =
        "class C { int x; int y; void f(C other, boolean b, int a){ if (b) { other.x = a; this.y = a + 1; } } }";
    assert_paths_agree(unproven_receiver, Lang::Java);
    assert_contract_excludes_kind(
        unproven_receiver,
        Lang::Java,
        FragmentKind::ConditionalGuard,
    );
}

#[test]
fn differential_self_field_body() {
    // Accepted: Java function body blocks composed of fixed-`this` field writes, nested
    // conditionals over those writes, and an optional terminal `return this`. The body
    // root bypasses the shared context gate only through the SelfFieldBody recognizer.
    assert_paths_agree(
        "class C { int x; int y; void set(int a, int b){ this.x = a; this.y = b; } }",
        Lang::Java,
    );
    assert_paths_agree(
        "class C { int x; C set(int a){ if (a > 0) { this.x = a; } return this; } }",
        Lang::Java,
    );
    // Rejected by both paths at the body root: return-this is only allowed terminally,
    // and non-`this` field writes do not have a proven receiver.
    assert_paths_agree(
        "class C { int x; C set(int a){ return this; this.x = a; } }",
        Lang::Java,
    );
    assert_paths_agree(
        "class C { int x; void set(C other, int a){ other.x = a; this.x = a; } }",
        Lang::Java,
    );
}

#[test]
fn differential_loop_effect() {
    // Accepted: for-each loops whose body is an iteration-dependent append/index effect,
    // including a local-temp variant — the predicate and the independent contract
    // recognizer must agree on the loop node (and every migrated leaf).
    assert_paths_agree(
        "function h(xs){ const out=[]; for(const x of xs){ out.push(x*2); } return out; }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "def k(xs):\n    out = []\n    for x in xs:\n        out.append(x + 1)\n    return out\n",
        Lang::Python,
    );
    assert_paths_agree(
        "function t(xs){ const out=[]; for(const x of xs){ const v = x*2; out.push(v); } return out; }",
        Lang::JavaScript,
    );
    // Go index-write loop: `out[x] = x*2`, key/value depend on the loop var, receiver does not.
    assert_paths_agree(
        "package p\nfunc f(xs []int, out []int) {\n\tfor _, x := range xs {\n\t\tout[x] = x * 2\n\t}\n}\n",
        Lang::Go,
    );
    // `if`-guarded effect body: both paths recurse into the branch identically. (The
    // condition is not effect-checked on either path — the contract recognizer is a
    // faithful mirror of the predicate here, which is exactly what output-preserving
    // migration requires; the differential gate locks the two together.)
    assert_paths_agree(
        "function g(xs){ const out=[]; for(const x of xs){ if (x > 0) { out.push(x); } } return out; }",
        Lang::JavaScript,
    );
    // Rejected by both paths: receiver depends on the loop var (not loop-invariant);
    // appended value is loop-invariant; the loop is not a for-each. Each leaves the
    // migrated set empty at the loop node on both sides.
    assert_contract_excludes_kind(
        "function r(xs, out){ for(const x of xs){ out.push(x * 2); } }",
        Lang::JavaScript,
        FragmentKind::LoopEffect,
    );
    assert_paths_agree(
        "function r(xs){ for(const x of xs){ x.push(1); } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function r(xs, out){ for(const x of xs){ out.push(1); } }",
        Lang::JavaScript,
    );
    assert_paths_agree(
        "function r(xs, out){ let i = 0; while (i < xs.length){ out.push(xs[i]); i = i + 1; } }",
        Lang::JavaScript,
    );
}

/// Lower + normalize `src`, returning the IL and its parent index.
fn norm(src: &str, lang: Lang) -> (Il, Vec<Option<NodeId>>, Interner) {
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &interner)
        .expect("lowering should succeed");
    let il = normalize(&raw, &interner, &NormalizeOptions::default());
    let parents = build_parent_index(&il);
    (il, parents, interner)
}

/// The first contract the contract path produces for `src`, walking pre-order.
fn first_contract(il: &Il, parents: &[Option<NodeId>], interner: &Interner) -> FragmentContract {
    fn walk(
        il: &Il,
        node: NodeId,
        parents: &[Option<NodeId>],
        interner: &Interner,
    ) -> Option<FragmentContract> {
        if il.kind(node) == NodeKind::Lambda {
            return None;
        }
        if let Some(c) = recognize_contract(il, node, parents, interner) {
            return Some(c);
        }
        il.children(node)
            .iter()
            .find_map(|&c| walk(il, c, parents, interner))
    }
    walk(il, il.root, parents, interner).expect("a contract for the migrated shape")
}

fn assert_java_self_field_write_contract() {
    // Java `this.x = …` → FieldWrite over a proven This.field place (fail-closed safe).
    let (il, parents, interner) = norm(
        "class C { int x; void s(int v){ this.x = v + 1; } }",
        Lang::Java,
    );
    let c = first_contract(&il, &parents, &interner);
    assert_eq!(c.kind, FragmentKind::SelfFieldAssign);
    assert_eq!(c.effects.len(), 1);
    let site = &c.effects[0];
    assert_eq!(site.effect, Effect::FieldWrite);
    assert!(matches!(site.place, Some(Place::Field(ref base, _)) if **base == Place::This));
    assert!(site.place.as_ref().unwrap().is_exact_safe());
    assert!(site.effect.requires_proven_place());
    assert!(
        c.writes_proven(),
        "a proven self-field write must pass writes_proven"
    );
}

fn assert_java_index_write_contract() {
    // Java `a[i] = v` → IndexWrite, observable in the effect trace, so it carries no
    // receiver-identity obligation and records no place on the contract (place is reserved
    // for receiver-bearing effects like field writes).
    let (il, parents, interner) = norm(
        "class C { int[] a; void f(int i, int v){ a[i] = v; } }",
        Lang::Java,
    );
    let c = first_contract(&il, &parents, &interner);
    assert_eq!(c.kind, FragmentKind::IndexAssignEffect);
    assert_eq!(c.effects.len(), 1);
    let site = &c.effects[0];
    assert_eq!(site.effect, Effect::IndexWrite);
    assert_eq!(
        site.place, None,
        "index writes carry no receiver-proof obligation"
    );
    assert!(!site.effect.requires_proven_place());
    assert!(c.writes_proven());
}

fn assert_typed_ts_push_contract() {
    // Typed TS `xs.push(v)` proves the receiver is an array, lowers to the canonical
    // append builtin, and then records an Append effect with no heap place.
    let (il, parents, interner) = norm(
        "function f(xs: number[], v: number): void { xs.push(v + 1); }",
        Lang::TypeScript,
    );
    let c = first_contract(&il, &parents, &interner);
    assert_eq!(c.kind, FragmentKind::ExprEffect);
    assert_eq!(c.effects.len(), 1);
    assert_eq!(c.effects[0].effect, Effect::Append);
    assert_eq!(c.effects[0].place, None);
}

fn assert_untyped_js_push_contract() {
    // The same raw selector without receiver proof is not append evidence. It may still
    // be accepted by the separate opaque-call policy as `Other`, but it must not claim
    // append semantics.
    let (il, parents, interner) = norm("function f(xs, v){ xs.push(v + 1); }", Lang::JavaScript);
    let c = first_contract(&il, &parents, &interner);
    assert_eq!(c.kind, FragmentKind::ExprEffect);
    assert_eq!(c.effects.len(), 1);
    assert_eq!(c.effects[0].effect, Effect::Other);
    assert_eq!(c.effects[0].place, None);
}

#[test]
fn resolves_place_and_effect_for_write_shapes() {
    assert_java_self_field_write_contract();
    assert_java_index_write_contract();
    assert_typed_ts_push_contract();
    assert_untyped_js_push_contract();
}

#[test]
fn effect_as_output_preserved_through_wrapper() {
    // An append effect must survive wrapper synthesis as observable behavior: appending
    // to a parameter list is a caller-visible mutation, recorded in the effect trace.
    let battery = [
        vec![Value::List(vec![])],
        vec![Value::List(vec![Value::Int(9)])],
    ];

    let run = |src: &str| -> Vec<nose_normalize::Behavior> {
        let (il, parents, interner) = norm(src, Lang::TypeScript);
        let c = first_contract(&il, &parents, &interner);
        assert_eq!(c.effects.first().map(|s| s.effect), Some(Effect::Append));
        battery
            .iter()
            .map(|row| {
                fragment_behavior(&il, &interner, &c, row)
                    .expect("append fragment is interpretable")
            })
            .collect()
    };

    let f = run("function f(xs: number[]): void { xs.push(1); }");
    let g = run("function g(ys: number[]): void { ys.push(1); }");
    let h = run("function h(zs: number[]): void { zs.push(2); }");

    // The effect is actually observed (not silently dropped).
    assert!(
        f.iter().all(|b| !b.effects.is_empty()),
        "append must surface as a non-empty effect trace"
    );
    // Equivalent effect fragments agree; a different appended value diverges.
    assert_eq!(f, g, "identical append effects must agree on the battery");
    assert_ne!(
        f, h,
        "appending a different value must diverge in observable behavior"
    );
}
