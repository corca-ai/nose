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

use super::contract::FragmentContract;
use nose_il::{FileMeta, Il, IlBuilder, NodeId, NodeKind, Payload, Span, Unit, UnitKind};
use nose_normalize::{run_unit, Behavior, Value};

/// Run the fragment described by `contract` on `args` (bound to its inputs in order) and
/// return its observable [`Behavior`], or `None` if the wrapper cannot be synthesized or
/// the interpreter cannot model the fragment.
pub fn fragment_behavior(il: &Il, contract: &FragmentContract, args: &[Value]) -> Option<Behavior> {
    let (synth, func) = synthesize_wrapper(il, contract)?;
    run_unit(&synth, func, args)
}

/// Lower `contract` into a fresh single-`Func` [`Il`] and return that IL plus the func id.
///
/// Layout of the synthesized function: `Func[ Param(input₀) … Param(inputₙ) , Block[ <copy
/// of fragment subtree> ] ]`. Parameters carry the fragment's free canonical ids so the
/// deep-copied `Var` references resolve against them; the interpreter binds them
/// positionally from `args`.
pub fn synthesize_wrapper(il: &Il, contract: &FragmentContract) -> Option<(Il, NodeId)> {
    let mut b = IlBuilder::new(il.file);
    let syn = Span::synthetic(il.file);

    // Parameters: one per free input, in the contract's canonical order.
    let mut children: Vec<NodeId> = contract
        .inputs
        .iter()
        .map(|&cid| b.add(NodeKind::Param, Payload::Cid(cid), syn, &[]))
        .collect();

    // Body: a deep copy of the fragment statement, wrapped in a Block.
    let body_stmt = copy_subtree(il, contract.root, &mut b);
    let body = b.add(NodeKind::Block, Payload::None, syn, &[body_stmt]);
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

/// Deep-copy the subtree rooted at `node` from `src` into `b`, preserving kind, payload,
/// and span. Post-order: children are copied first so their fresh ids are known when the
/// parent is added.
fn copy_subtree(src: &Il, node: NodeId, b: &mut IlBuilder) -> NodeId {
    let kids: Vec<NodeId> = src
        .children(node)
        .to_vec()
        .iter()
        .map(|&c| copy_subtree(src, c, b))
        .collect();
    let n = src.node(node);
    b.add(n.kind, n.payload, n.span, &kids)
}

/// Collect the free canonical ids read in the subtree rooted at `node`, in ascending
/// (canonical) order. Only `Var` references count; this is correct for fragment shapes
/// with no internal bindings (e.g. a direct `return <expr>`). Shapes that introduce locals
/// (loop variables, temps) need binding-aware input inference, added when they migrate.
pub fn free_input_cids(il: &Il, node: NodeId) -> Vec<u32> {
    let mut out = Vec::new();
    collect_var_cids(il, node, &mut out);
    out.sort_unstable();
    out.dedup();
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{Exit, FragmentKind};
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

    fn behavior_vector(il: &Il, c: &FragmentContract, battery: &[Vec<Value>]) -> Vec<Behavior> {
        battery
            .iter()
            .map(|row| {
                fragment_behavior(il, c, row).expect("direct-return fragment must be interpretable")
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

        let (synth, func) = synthesize_wrapper(&il, &contract).expect("wrapper synthesizes");
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
            behavior_vector(&f, &cf, &battery),
            behavior_vector(&g, &cg, &battery),
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
            behavior_vector(&f, &cf, &battery),
            behavior_vector(&h, &ch, &battery),
            "behaviorally distinct fragments must diverge on the battery"
        );
    }
}
