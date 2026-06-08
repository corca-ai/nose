//! Tail-recursion recognition and emission.
//!
//! proof-obligation: normalize.recursion.tail

use super::Rebuilder;
use nose_il::{NodeId, Span};

pub(super) struct Plan {
    pub(super) param_cids: Vec<u32>,
    pub(super) guards: Vec<(NodeId, NodeId)>, // (cond, returned value)
    pub(super) args: Vec<NodeId>,
}

pub(super) fn recognize(
    rb: &Rebuilder<'_>,
    fid: NodeId,
    param_cids: Vec<u32>,
    guards: Vec<(NodeId, NodeId)>,
    rexpr: NodeId,
) -> Option<Plan> {
    // Tail recursion: the recursive case IS the self-call.
    let args = rb.as_self_call(rexpr, fid)?;
    if args.len() != param_cids.len() {
        return None;
    }
    Some(Plan {
        param_cids,
        guards,
        args,
    })
}

pub(super) fn build_body(rb: &mut Rebuilder<'_>, plan: &Plan, span: Span) -> Option<NodeId> {
    let updates = rb.ordered_updates(&plan.param_cids, &plan.args)?;
    let cond = rb.not_any(plan.guards.iter().map(|&(c, _)| c).collect());
    let loop_body = rb.b.add(
        nose_il::NodeKind::Block,
        nose_il::Payload::None,
        span,
        &updates,
    );
    let wl = rb.while_loop(cond, loop_body, span);
    // Post-loop guard chain: exactly one guard holds on exit, so the final one
    // is an unconditional return.
    let mut stmts = vec![wl];
    for (i, &(cond, val)) in plan.guards.iter().enumerate() {
        let v = rb.go_val(val);
        let ret = rb.ret(v, span);
        if i + 1 == plan.guards.len() {
            stmts.push(ret);
        } else {
            let c = rb.go_val(cond);
            let then = rb.b.add(
                nose_il::NodeKind::Block,
                nose_il::Payload::None,
                span,
                &[ret],
            );
            stmts.push(rb.b.add(
                nose_il::NodeKind::If,
                nose_il::Payload::None,
                span,
                &[c, then],
            ));
        }
    }
    Some(rb.b.add(
        nose_il::NodeKind::Block,
        nose_il::Payload::None,
        span,
        &stmts,
    ))
}
