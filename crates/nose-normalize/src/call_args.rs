//! Shared argument→parameter binding for calls with keyword arguments.
//!
//! Keyword arguments (`f(name=value)`) bind by NAME, not position. Both the value-graph
//! builder and the behavioral interpreter must resolve the same mapping, so the
//! IL-only analysis (which value-node feeds which parameter) lives here once: each
//! consumer then evaluates the chosen argument node in its own value domain.

use nose_il::{Il, NodeId, NodeKind, Payload, Symbol};

/// Resolve a call's positional + keyword arguments to a binding plan: a list of
/// `(param_cid, arg_value_node)` pairs covering every parameter exactly once.
///
/// `param_cids` are the callee's parameter canonical ids in declaration order;
/// `call_args` are the call's argument nodes (the children after the callee). Positional
/// args fill parameters left-to-right; a `KwArg` binds to the parameter whose original
/// name (via `il.cid_names`) matches its keyword. Returns `None` — the fail-closed signal
/// every caller routes to its opaque path — for a keyword naming no parameter, a
/// double-bind, an over-long positional run, or any parameter left unbound. The returned
/// node for a keyword argument is its VALUE child, so callers evaluate the value, not the
/// `KwArg` wrapper (#301).
pub(crate) fn keyword_arg_binding_plan(
    il: &Il,
    param_cids: &[u32],
    call_args: &[NodeId],
) -> Option<Vec<(u32, NodeId)>> {
    let param_name = |cid: u32| -> Option<Symbol> { il.cid_names.get(cid as usize).copied() };
    // A spread argument (`*args`/`**kwargs`) has dynamic arity — it cannot bind to fixed
    // parameters, so any call carrying one fails closed (no inline; the oracle bails).
    if call_args.iter().any(|&a| il.kind(a) == NodeKind::Splat) {
        return None;
    }
    let mut plan: Vec<(u32, NodeId)> = Vec::with_capacity(param_cids.len());
    let mut bound = vec![false; param_cids.len()];
    let mut pos = 0usize;
    for &arg in call_args {
        let (idx, value_node) = if il.kind(arg) == NodeKind::KwArg {
            let Payload::Name(kw) = il.node(arg).payload else {
                return None;
            };
            let idx = param_cids.iter().position(|&c| param_name(c) == Some(kw))?;
            (idx, *il.children(arg).first()?)
        } else {
            let idx = pos;
            if idx >= param_cids.len() {
                return None;
            }
            pos += 1;
            (idx, arg)
        };
        if bound[idx] {
            return None;
        }
        bound[idx] = true;
        plan.push((param_cids[idx], value_node));
    }
    bound.iter().all(|&b| b).then_some(plan)
}
