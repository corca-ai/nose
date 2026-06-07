//! The fragment behavior oracle: lower a [`FragmentContract`] into a runnable wrapper and
//! run it through the existing unit interpreter.
//!
//! Issue #33 decision: fragments go through the *same* independent behavior check as
//! whole functions, via **wrapper synthesis** rather than a new `run_fragment` interpreter
//! path. A contract is lowered into a synthetic `Func` — its free inputs become parameters,
//! its body is a deep copy of the fragment subtree — and handed to
//! [`nose_normalize::run_unit`]. We reuse `run_unit`, its [`Behavior`] (return value +
//! ordered effects + final field state), and the caller's input battery unchanged.
//!
//! The forcing function: a contract that cannot be lowered into a runnable wrapper is
//! *underspecified*. [`synthesize_wrapper`] returning `None` is therefore a signal that the
//! recognizer described a fragment the oracle cannot vouch for — fail closed.
//!
//! proof-obligation: detect.fragment.free_inputs
//! proof-obligation: detect.fragment.wrapper_synthesis
//! proof-obligation: il.arena.deep_copy

use super::contract::{Effect, FragmentContract};
use nose_il::{
    Builtin, FileMeta, Il, IlBuilder, Interner, LoopKind, NodeId, NodeKind, Payload, Span, Unit,
    UnitKind,
};
use nose_normalize::{run_unit, Behavior, Value};
use nose_semantics::builder_append_call_args;

/// Run the fragment described by `contract` on `args` (bound to its inputs in order) and
/// return its observable [`Behavior`], or `None` if the wrapper cannot be synthesized or
/// the interpreter cannot model the fragment.
pub fn fragment_behavior(
    il: &Il,
    interner: &Interner,
    contract: &FragmentContract,
    args: &[Value],
) -> Option<Behavior> {
    let (synth, func) = synthesize_wrapper(il, interner, contract)?;
    run_unit(&synth, func, args)
}

/// Lower `contract` into a fresh single-`Func` [`Il`] and return that IL plus the func id.
///
/// Layout of the synthesized function: `Func[ Param(input₀) … Param(inputₙ) , Block[ <copy
/// of fragment subtree> ] ]`. Parameters carry the fragment's free canonical ids so the
/// deep-copied `Var` references resolve against them; the interpreter binds them
/// positionally from `args`.
pub fn synthesize_wrapper(
    il: &Il,
    interner: &Interner,
    contract: &FragmentContract,
) -> Option<(Il, NodeId)> {
    let mut b = IlBuilder::new(il.file);
    let syn = Span::synthetic(il.file);
    let policy = CopyPolicy {
        canonicalize_append_effects: contract
            .effects
            .iter()
            .any(|site| site.effect == Effect::Append),
    };

    // Parameters: one per free input, in the contract's canonical order.
    let mut children: Vec<NodeId> = contract
        .inputs
        .iter()
        .map(|&cid| b.add(NodeKind::Param, Payload::Cid(cid), syn, &[]))
        .collect();

    // Body: deep-copy the fragment into the wrapper's body block. A block-rooted fragment
    // (a conditional branch, a loop or ordered-effect body) is spliced statement-by-statement
    // so the wrapper body stays flat rather than a `Block` nested in a `Block`; a single
    // statement becomes the lone body statement. Either way the interpreter executes the same
    // statements in the same order.
    let body_stmts: Vec<NodeId> = if il.kind(contract.root) == NodeKind::Block {
        il.children(contract.root)
            .to_vec()
            .iter()
            .map(|&s| copy_subtree(il, interner, s, &mut b, policy))
            .collect::<Option<Vec<_>>>()?
    } else {
        vec![copy_subtree(il, interner, contract.root, &mut b, policy)?]
    };
    let body = b.add(NodeKind::Block, Payload::None, syn, &body_stmts);
    children.push(body);

    let func = b.add(NodeKind::Func, Payload::None, syn, &children);
    let meta = FileMeta {
        path: il.meta.path.clone(),
        lang: il.meta.lang,
    };
    let units = vec![Unit {
        root: func,
        kind: UnitKind::Function,
        name: None,
    }];
    let synth = b.finish(func, meta, units, Vec::new());
    debug_assert!(
        synth.validate().is_ok(),
        "synthesized fragment wrapper must be a valid arena"
    );
    Some((synth, func))
}

#[derive(Clone, Copy)]
struct CopyPolicy {
    canonicalize_append_effects: bool,
}

/// Deep-copy the subtree rooted at `node` from `src` into `b`, preserving kind, payload, and
/// span unless the accepted contract needs an append effect surface made executable. That
/// rewrite is deliberately local to wrapper synthesis: normal semantic normalization remains
/// proof-gated and does not infer collection semantics from a method name alone.
fn copy_subtree(
    src: &Il,
    interner: &Interner,
    node: NodeId,
    b: &mut IlBuilder,
    policy: CopyPolicy,
) -> Option<NodeId> {
    if policy.canonicalize_append_effects {
        if let Some((receiver, value)) = append_surface_parts(src, interner, node) {
            let receiver_tag = append_receiver_tag(src, receiver)?;
            let target = copy_subtree(src, interner, receiver, b, policy)?;
            let mut kids = Vec::with_capacity(2);
            kids.push(target);
            let value = copy_subtree(src, interner, value, b, policy)?;
            let tag = b.add(
                NodeKind::Lit,
                Payload::LitInt(receiver_tag),
                src.node(node).span,
                &[],
            );
            let tagged_value = b.add(
                NodeKind::Seq,
                Payload::None,
                src.node(node).span,
                &[tag, value],
            );
            kids.push(tagged_value);
            return Some(b.add(
                NodeKind::Call,
                Payload::Builtin(Builtin::Append),
                src.node(node).span,
                &kids,
            ));
        }
    }

    let kids: Vec<NodeId> = src
        .children(node)
        .to_vec()
        .iter()
        .map(|&c| copy_subtree(src, interner, c, b, policy))
        .collect::<Option<Vec<_>>>()?;
    let n = src.node(node);
    Some(b.add(n.kind, n.payload, n.span, &kids))
}

fn append_surface_parts(src: &Il, interner: &Interner, node: NodeId) -> Option<(NodeId, NodeId)> {
    builder_append_call_args(src, interner, node)
}

fn append_receiver_tag(src: &Il, receiver: NodeId) -> Option<i64> {
    match (src.kind(receiver), src.node(receiver).payload) {
        (NodeKind::Var, Payload::Cid(cid)) => Some(i64::from(cid)),
        _ => None,
    }
}

/// Collect the free canonical ids read in the subtree rooted at `node`, in ascending
/// (canonical) order — the cids the fragment reads from its enclosing scope. These become the
/// synthesized wrapper's parameters.
///
/// "Free" excludes cids *bound within* the fragment: a local assigned before use, a `for-each`
/// loop variable, a nested lambda parameter. The interpreter binds those as the wrapper runs
/// (assignment targets and loop patterns enter `env`), so making them parameters would inflate
/// the arity and feed a battery value the fragment immediately overwrites — the loop-variable
/// hazard that previously made loop/temp shapes unmodelable. The binding model mirrors the one
/// alpha-renaming uses (see `nose_normalize::alpha`): assignment targets and `for-each`
/// patterns — a `Var`, or each `Var` in a destructuring `Seq` — plus nested `Param`s.
///
/// Soundness: omitting a *genuine* outer input can only under-report, and an unbound `Var`
/// read makes the wrapper uninterpretable (`run_unit` returns `None`) — fail-closed, never a
/// false merge. Index/field stores mutate an existing receiver (which stays a free input) and
/// bind nothing, so they are deliberately not treated as bindings.
pub fn free_input_cids(il: &Il, node: NodeId) -> Vec<u32> {
    let mut reads = Vec::new();
    collect_var_cids(il, node, &mut reads);
    reads.sort_unstable();
    reads.dedup();

    let mut bound = Vec::new();
    collect_bound_cids(il, node, &mut bound);
    bound.sort_unstable();
    bound.dedup();

    reads.retain(|c| bound.binary_search(c).is_err());
    reads
}

fn collect_var_cids(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            out.push(c);
        }
    }
    for &k in il.children(node) {
        collect_var_cids(il, k, out);
    }
}

/// Collect cids *bound within* the subtree: assignment targets, `for-each` loop patterns, and
/// nested `Param`s. Mirrors the binding model alpha-renaming uses, so "free" here means the
/// same thing it does after renaming.
fn collect_bound_cids(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    match il.kind(node) {
        NodeKind::Param => {
            if let Payload::Cid(c) = il.node(node).payload {
                out.push(c);
            }
        }
        NodeKind::Assign => {
            if let Some(&lhs) = il.children(node).first() {
                collect_binding_targets(il, lhs, out);
            }
        }
        NodeKind::Loop if matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) => {
            if let Some(&pat) = il.children(node).first() {
                collect_binding_targets(il, pat, out);
            }
        }
        _ => {}
    }
    for &k in il.children(node) {
        collect_bound_cids(il, k, out);
    }
}

/// Assignment / `for`-pattern binding targets: a `Var` cid, or each `Var` in a destructuring
/// `Seq`. Only plain `Var`/`Seq` targets bind a fresh cid; an `Index`/`Field` store target
/// mutates an existing receiver and binds nothing.
fn collect_binding_targets(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    match il.kind(node) {
        NodeKind::Var => {
            if let Payload::Cid(c) = il.node(node).payload {
                out.push(c);
            }
        }
        NodeKind::Seq => {
            for &c in il.children(node) {
                collect_binding_targets(il, c, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{Effect, EffectSite, Exit, FragmentKind};
    use nose_il::{FileId, Interner, Lang};
    use nose_normalize::{normalize, NormalizeOptions};

    /// Lower + normalize `src`, returning the normalized IL.
    fn norm(interner: &Interner, src: &str, lang: Lang) -> Il {
        let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner)
            .expect("lowering should succeed");
        normalize(&il, interner, &NormalizeOptions::default())
    }

    /// Find the first `Return` node with one computed (non-var/lit) child — a direct-return
    /// fragment root.
    fn first_direct_return(il: &Il, node: NodeId) -> Option<NodeId> {
        if il.kind(node) == NodeKind::Return {
            let kids = il.children(node);
            if kids.len() == 1 && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit) {
                return Some(node);
            }
        }
        for &c in il.children(node) {
            if let Some(found) = first_direct_return(il, c) {
                return Some(found);
            }
        }
        None
    }

    fn direct_return_contract(il: &Il, root: NodeId) -> FragmentContract {
        FragmentContract::value_sink(
            FragmentKind::DirectReturn,
            root,
            free_input_cids(il, root),
            Exit::Return,
        )
    }

    /// A small single-argument battery — enough to separate the spike's fragments.
    fn battery_1() -> Vec<Vec<Value>> {
        [-2i64, -1, 0, 1, 2, 3, 5]
            .into_iter()
            .map(|n| vec![Value::Int(n)])
            .collect()
    }

    fn behavior_vector(
        il: &Il,
        interner: &Interner,
        c: &FragmentContract,
        battery: &[Vec<Value>],
    ) -> Vec<Behavior> {
        battery
            .iter()
            .map(|row| {
                fragment_behavior(il, interner, c, row)
                    .expect("direct-return fragment must be interpretable")
            })
            .collect()
    }

    #[test]
    fn wrapper_synthesis_runs_a_direct_return_fragment() {
        let i = Interner::new();
        let il = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
        let root = first_direct_return(&il, il.root).expect("a direct-return fragment");
        let contract = direct_return_contract(&il, root);
        assert_eq!(contract.arity(), 1, "one free input (the parameter)");

        let (synth, func) = synthesize_wrapper(&il, &i, &contract).expect("wrapper synthesizes");
        assert_eq!(synth.kind(func), NodeKind::Func);
        let b = run_unit(&synth, func, &[Value::Int(4)]).expect("interpretable");
        assert_eq!(b.ret, Value::Int(17), "4*4 + 1 = 17");
    }

    #[test]
    fn equivalent_fragments_agree_on_the_battery() {
        let i = Interner::new();
        // Same spec, different surface: squared-plus-one.
        let f = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
        let g = norm(&i, "function g(b){ return 1 + b*b; }", Lang::JavaScript);
        let cf = direct_return_contract(&f, first_direct_return(&f, f.root).unwrap());
        let cg = direct_return_contract(&g, first_direct_return(&g, g.root).unwrap());

        let battery = battery_1();
        assert_eq!(
            behavior_vector(&f, &i, &cf, &battery),
            behavior_vector(&g, &i, &cg, &battery),
            "equivalent direct-return fragments must agree on every battery input"
        );
    }

    #[test]
    fn distinct_fragments_diverge_on_the_battery() {
        let i = Interner::new();
        let f = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
        let h = norm(&i, "function h(a){ return a*a - 1; }", Lang::JavaScript);
        let cf = direct_return_contract(&f, first_direct_return(&f, f.root).unwrap());
        let ch = direct_return_contract(&h, first_direct_return(&h, h.root).unwrap());

        let battery = battery_1();
        assert_ne!(
            behavior_vector(&f, &i, &cf, &battery),
            behavior_vector(&h, &i, &ch, &battery),
            "behaviorally distinct fragments must diverge on the battery"
        );
    }

    // ---- binding-aware free-input inference --------------------------------------------

    fn find<P: Fn(&Il, NodeId) -> bool>(il: &Il, node: NodeId, pred: &P) -> Option<NodeId> {
        if pred(il, node) {
            return Some(node);
        }
        il.children(node).iter().find_map(|&c| find(il, c, pred))
    }

    fn first_foreach(il: &Il) -> NodeId {
        find(il, il.root, &|il, n| {
            il.kind(n) == NodeKind::Loop
                && matches!(il.node(n).payload, Payload::Loop(LoopKind::ForEach))
        })
        .expect("a for-each loop")
    }

    /// The body `Block` of the first `Func` — the multi-statement fragment body.
    fn first_func_body(il: &Il) -> NodeId {
        let func = find(il, il.root, &|il, n| il.kind(n) == NodeKind::Func).expect("a func");
        *il.children(func).last().expect("func has a body block")
    }

    #[test]
    fn free_inputs_exclude_the_foreach_loop_variable() {
        // The loop variable `x` is bound by the for-each pattern, not read from outside; only
        // the appended-to list `out` and the iterable `xs` are genuine free inputs. Without
        // binding-aware inference this would be arity 3 and the wrapper would misbind `x`.
        let i = Interner::new();
        let il = norm(
            &i,
            "function f(out, xs){ for (const x of xs){ out.push(x); } }",
            Lang::JavaScript,
        );
        let loop_node = first_foreach(&il);
        let inputs = free_input_cids(&il, loop_node);
        assert_eq!(
            inputs.len(),
            2,
            "only `out` and `xs` are free; the loop variable `x` must be excluded, got {inputs:?}"
        );
    }

    #[test]
    fn free_inputs_exclude_a_local_temp() {
        // `t` is assigned then read inside the fragment, so it is a local, not a free input.
        let i = Interner::new();
        let il = norm(
            &i,
            "function f(a){ let t = a * a; return t + 1; }",
            Lang::JavaScript,
        );
        let body = first_func_body(&il);
        let inputs = free_input_cids(&il, body);
        assert_eq!(
            inputs.len(),
            1,
            "only `a` is free; the temp `t` must be excluded, got {inputs:?}"
        );
    }

    #[test]
    fn equivalent_foreach_loops_agree_through_the_oracle() {
        // Two for-each append loops with the same spec must agree; a different appended value
        // must diverge — exercising binding-aware inputs + multi-statement loop lowering.
        let battery = || {
            vec![vec![
                Value::List(vec![]),
                Value::List(vec![Value::Int(2), Value::Int(5)]),
            ]]
        };
        let run = |src: &str| -> Vec<Behavior> {
            let i = Interner::new();
            let il = norm(&i, src, Lang::TypeScript);
            let loop_node = first_foreach(&il);
            let c = FragmentContract::single_effect(
                FragmentKind::LoopEffect,
                loop_node,
                free_input_cids(&il, loop_node),
                EffectSite::observable(Effect::Append),
            );
            assert_eq!(c.arity(), 2, "loop var excluded → arity 2");
            battery()
                .iter()
                .map(|row| {
                    fragment_behavior(&il, &i, &c, row).expect("loop fragment interpretable")
                })
                .collect()
        };
        let f = run(
            "function f(out: number[], xs: number[]): void { for (const x of xs){ out.push(x); } }",
        );
        let g = run(
            "function g(acc: number[], ys: number[]): void { for (const y of ys){ acc.push(y); } }",
        );
        let h = run(
            "function h(out: number[], xs: number[]): void { for (const x of xs){ out.push(x * 2); } }",
        );
        assert!(
            f.iter().all(|b| !b.effects.is_empty()),
            "loop append surfaces as effects"
        );
        assert_eq!(f, g, "equivalent for-each append loops must agree");
        assert_ne!(f, h, "appending a different value must diverge");
    }

    // ---- ordered multi-effect, multi-statement body -----------------------------------

    #[test]
    fn ordered_multi_effect_body_observes_statement_order() {
        // A two-append body lowered as an ordered-effect contract: the effect order is
        // observable, so swapping the two appends diverges while an identical body agrees.
        let run = |src: &str| -> Behavior {
            let i = Interner::new();
            let il = norm(&i, src, Lang::TypeScript);
            let body = first_func_body(&il);
            let c = FragmentContract::ordered_effects(
                FragmentKind::ExprEffect,
                body,
                free_input_cids(&il, body),
                Exit::Normal,
                vec![
                    EffectSite::observable(Effect::Append),
                    EffectSite::observable(Effect::Append),
                ],
            );
            assert_eq!(c.arity(), 1, "only `out` is free (literals are not inputs)");
            fragment_behavior(&il, &i, &c, &[Value::List(vec![])]).expect("interpretable")
        };
        let fwd = run("function f(out: number[]): void { out.push(1); out.push(2); }");
        let fwd2 = run("function h(out: number[]): void { out.push(1); out.push(2); }");
        let rev = run("function g(out: number[]): void { out.push(2); out.push(1); }");
        assert_eq!(fwd.effects.len(), 2, "both appends are recorded in order");
        assert_eq!(fwd, fwd2, "identical ordered bodies must agree");
        assert_ne!(fwd, rev, "swapping the append order must be observable");
    }

    #[test]
    fn append_effect_wrapper_preserves_receiver_identity() {
        let run = |src: &str| -> Behavior {
            let i = Interner::new();
            let il = norm(&i, src, Lang::TypeScript);
            let body = first_func_body(&il);
            let c = FragmentContract::ordered_effects(
                FragmentKind::ExprEffect,
                body,
                free_input_cids(&il, body),
                Exit::Normal,
                vec![
                    EffectSite::observable(Effect::Append),
                    EffectSite::observable(Effect::Append),
                ],
            );
            fragment_behavior(&il, &i, &c, &[Value::List(vec![]), Value::List(vec![])])
                .expect("interpretable")
        };

        let same =
            run("function f(out: number[], other: number[]): void { out.push(1); other.push(2); }");
        let renamed =
            run("function g(dst: number[], aux: number[]): void { dst.push(1); aux.push(2); }");
        let swapped =
            run("function h(out: number[], other: number[]): void { other.push(1); out.push(2); }");

        assert_eq!(same, renamed, "alpha-renamed receiver roles should agree");
        assert_ne!(
            same, swapped,
            "append effects must preserve which receiver role was mutated"
        );
    }
}
