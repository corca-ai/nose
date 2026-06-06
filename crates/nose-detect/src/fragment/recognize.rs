//! The contract-path recognizer and its differential gate against the shape predicates.
//!
//! Issue #33 steps 4–5. As each fragment family migrates off the standalone shape
//! predicates in [`crate::units`], its recognition is re-expressed here as the
//! construction of a [`FragmentContract`]. [`recognize_contract`] is an *independent*
//! recognizer for the migrated shapes: it matches structure directly and builds a contract,
//! reusing only the shared invalidation-boundary gates (span containment + context safety),
//! which are substrate, not per-shape predicates.
//!
//! The differential test below is the acceptance gate the maintainer required: over a
//! representative corpus, the set of `(span, kind)` the predicate path accepts (restricted
//! to migrated kinds) must equal the set the contract path produces. A migration step that
//! changes which nodes are accepted fails this test. As [`MIGRATED`] grows, the gate keeps
//! the two paths in lockstep until every shape is contract-expressed.

use super::contract::{Effect, FragmentContract};
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind, Place};
use crate::units::{exact_java_this_field, exact_java_this_var};
use nose_il::{stable_symbol_hash, Builtin, Il, Interner, Lang, NodeId, NodeKind, Payload};

/// Fragment kinds that have been migrated onto the contract path. The differential gate
/// compares the predicate and contract paths over exactly this set; everything outside it
/// is still owned solely by the [`crate::units`] predicates.
pub(crate) const MIGRATED: &[FragmentKind] = &[
    FragmentKind::DirectReturn,
    FragmentKind::DirectThrow,
    FragmentKind::IndexAssignEffect,
    FragmentKind::SelfFieldAssign,
    FragmentKind::ExprEffect,
];

/// Recognize `node` as a migrated exact-fragment shape by building its contract directly,
/// independently of [`crate::units::exact_statement_fragment_root`]. Returns `None` for
/// non-fragments and for shapes not yet migrated.
pub(crate) fn recognize_contract(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
) -> Option<FragmentContract> {
    // Shared substrate gates — the invalidation-boundary model, reused (not duplicated).
    if !crate::units::subtree_spans_within(il, node, il.node(node).span) {
        return None;
    }
    if !crate::units::top_level_statement_fragment_context_safe(il, node, parents, interner) {
        return None;
    }
    let kids = il.children(node);
    let computed_unary =
        || kids.len() == 1 && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit);
    match il.kind(node) {
        NodeKind::Return if computed_unary() => Some(FragmentContract::value_sink(
            FragmentKind::DirectReturn,
            node,
            free_input_cids(il, node),
            Exit::Return,
        )),
        NodeKind::Throw if computed_unary() => Some(FragmentContract::value_sink(
            FragmentKind::DirectThrow,
            node,
            free_input_cids(il, node),
            Exit::Throw,
        )),
        NodeKind::Assign => recognize_assignment_effect(il, interner, node),
        NodeKind::ExprStmt if expr_effect_shape(il, kids) => {
            let effect = if is_append_call(il, kids[0]) {
                Effect::Append
            } else {
                Effect::Other
            };
            Some(effect_contract(
                FragmentKind::ExprEffect,
                il,
                node,
                effect,
                None,
            ))
        }
        _ => None,
    }
}

/// Classify an assignment-effect fragment: a non-overloadable index write (C/Go/Java) or a
/// Java fixed-receiver `this.field` write. The two shapes are structurally disjoint.
fn recognize_assignment_effect(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentContract> {
    let kids = il.children(node);
    if kids.len() != 2 {
        return None;
    }
    let target = kids[0];
    if matches!(il.meta.lang, Lang::C | Lang::Go | Lang::Java) && il.kind(target) == NodeKind::Index
    {
        let place = resolve_place(il, interner, target);
        return Some(effect_contract(
            FragmentKind::IndexAssignEffect,
            il,
            node,
            Effect::IndexWrite,
            Some(place),
        ));
    }
    if il.meta.lang == Lang::Java && exact_java_this_field(il, interner, target) {
        let place = resolve_place(il, interner, target);
        // Field writes do not observe their receiver in the oracle (the field-state map is
        // keyed by name only), so the write is exact-safe only with a proven receiver. The
        // `this.field` recognizer guarantees this; assert the invariant fail-closed.
        debug_assert!(
            place.is_exact_safe(),
            "self-field write must resolve to a proven place, got {place:?}"
        );
        return Some(effect_contract(
            FragmentKind::SelfFieldAssign,
            il,
            node,
            Effect::FieldWrite,
            Some(place),
        ));
    }
    None
}

/// An expression statement evaluated for its side effect: a single child that is not a
/// control sink, bare variable, or bare literal (those carry no observable effect).
fn expr_effect_shape(il: &Il, kids: &[NodeId]) -> bool {
    kids.len() == 1
        && !matches!(
            il.kind(kids[0]),
            NodeKind::Return
                | NodeKind::Throw
                | NodeKind::Break
                | NodeKind::Continue
                | NodeKind::Var
                | NodeKind::Lit
        )
}

fn is_append_call(il: &Il, node: NodeId) -> bool {
    il.kind(node) == NodeKind::Call
        && matches!(il.node(node).payload, Payload::Builtin(Builtin::Append))
}

fn effect_contract(
    kind: FragmentKind,
    il: &Il,
    node: NodeId,
    effect: Effect,
    place: Option<Place>,
) -> FragmentContract {
    FragmentContract {
        kind,
        root: node,
        inputs: free_input_cids(il, node),
        exit: Exit::Normal,
        effect: Some(effect),
        place,
    }
}

/// Resolve a write target's [`Place`] receiver identity, fail-closed to [`Place::Unknown`].
///
/// - `this` (Java) → [`Place::This`]
/// - a free variable → [`Place::Param`] (its canonical id)
/// - `base.field` → [`Place::Field`] over the resolved base, keyed by field-name hash
/// - `base[key]` → [`Place::Index`] over the resolved base, keyed by a coarse key hash
/// - anything else (a call result, an unresolved receiver) → [`Place::Unknown`]
fn resolve_place(il: &Il, interner: &Interner, node: NodeId) -> Place {
    match il.kind(node) {
        NodeKind::Var if exact_java_this_var(il, interner, node) => Place::This,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => Place::Param(c),
            _ => Place::Unknown,
        },
        NodeKind::Field => {
            let base = il.children(node).first().copied();
            let receiver = base.map_or(Place::Unknown, |b| resolve_place(il, interner, b));
            match il.node(node).payload {
                Payload::Name(sym) => Place::Field(
                    Box::new(receiver),
                    stable_symbol_hash(interner.resolve(sym)),
                ),
                _ => Place::Unknown,
            }
        }
        NodeKind::Index => {
            let kids = il.children(node);
            let receiver = kids
                .first()
                .map_or(Place::Unknown, |&b| resolve_place(il, interner, b));
            let key = kids.get(1).map_or(0, |&k| place_key_hash(il, interner, k));
            Place::Index(Box::new(receiver), key)
        }
        _ => Place::Unknown,
    }
}

/// A coarse, stable identity for an index/key expression — enough to distinguish constant
/// keys and variable keys in a [`Place`], without modeling arbitrary key expressions.
fn place_key_hash(il: &Il, interner: &Interner, node: NodeId) -> u64 {
    match il.node(node).payload {
        Payload::Cid(c) => 0x01_0000_0000 | u64::from(c),
        Payload::Name(sym) => stable_symbol_hash(interner.resolve(sym)),
        Payload::LitInt(v) => 0x02_0000_0000 ^ (v as u64),
        Payload::LitStr(h) | Payload::LitFloat(h) => h,
        _ => u64::from(il.kind(node) as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{fragment_behavior, Place};
    use crate::units::{build_parent_index, exact_statement_fragment_root};
    use nose_il::{FileId, Lang, Span};
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
    fn differential_ignores_non_migrated_shapes() {
        // Loop/append and conditional effect shapes are accepted by the predicate path
        // under OTHER kinds and must be excluded from the contract path — the migrated
        // intersection stays empty and equal.
        assert_paths_agree(
            "function h(xs){ const out=[]; for(const x of xs){ out.push(x*2); } return out; }",
            Lang::JavaScript,
        );
        assert_paths_agree(
            "def k(xs):\n    out = []\n    for x in xs:\n        out.append(x + 1)\n    return out\n",
            Lang::Python,
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
    fn first_contract(
        il: &Il,
        parents: &[Option<NodeId>],
        interner: &Interner,
    ) -> FragmentContract {
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

    #[test]
    fn resolves_place_and_effect_for_write_shapes() {
        // Java `this.x = …` → FieldWrite over a proven This.field place (fail-closed safe).
        let (il, parents, interner) = norm(
            "class C { int x; void s(int v){ this.x = v + 1; } }",
            Lang::Java,
        );
        let c = first_contract(&il, &parents, &interner);
        assert_eq!(c.kind, FragmentKind::SelfFieldAssign);
        assert_eq!(c.effect, Some(Effect::FieldWrite));
        assert!(matches!(c.place, Some(Place::Field(ref base, _)) if **base == Place::This));
        assert!(c.place.as_ref().unwrap().is_exact_safe());
        assert!(c.effect.unwrap().requires_proven_place());

        // Java `a[i] = v` → IndexWrite over an index place. The base here is an instance
        // field accessed bare, so it resolves to a fail-closed `Unknown` receiver — yet the
        // write stays exact-safe because an index write is observable in the effect trace
        // and so does not require a proven receiver.
        let (il, parents, interner) = norm(
            "class C { int[] a; void f(int i, int v){ a[i] = v; } }",
            Lang::Java,
        );
        let c = first_contract(&il, &parents, &interner);
        assert_eq!(c.kind, FragmentKind::IndexAssignEffect);
        assert_eq!(c.effect, Some(Effect::IndexWrite));
        assert!(matches!(c.place, Some(Place::Index(_, _))));
        assert!(!c.effect.unwrap().requires_proven_place());

        // JS `xs.push(v)` → Append effect, no heap place.
        let (il, parents, interner) =
            norm("function f(xs, v){ xs.push(v + 1); }", Lang::JavaScript);
        let c = first_contract(&il, &parents, &interner);
        assert_eq!(c.kind, FragmentKind::ExprEffect);
        assert_eq!(c.effect, Some(Effect::Append));
        assert_eq!(c.place, None);
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
            let (il, parents, interner) = norm(src, Lang::JavaScript);
            let c = first_contract(&il, &parents, &interner);
            assert_eq!(c.effect, Some(Effect::Append));
            battery
                .iter()
                .map(|row| {
                    fragment_behavior(&il, &c, row).expect("append fragment is interpretable")
                })
                .collect()
        };

        let f = run("function f(xs){ xs.push(1); }");
        let g = run("function g(ys){ ys.push(1); }");
        let h = run("function h(zs){ zs.push(2); }");

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
}
