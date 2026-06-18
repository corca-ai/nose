//! Structural recursion to accumulator-fold recognition and emission.
//!
//! proof-obligation: normalize.recursion.structural_fold

use super::Rebuilder;
use nose_il::{LitClass, NodeId, NodeKind, Op, Payload, Span};
use nose_semantics::{domain_evidence_for_param, semantics, ValueDomain, ValueLaw};
use rustc_hash::FxHashSet;

pub(super) struct Plan {
    pub(super) param_cids: Vec<u32>,
    pub(super) base_cond: NodeId,
    pub(super) op: Op,
    pub(super) head: NodeId,
    pub(super) args: Vec<NodeId>,
    pub(super) identity: NodeId,
}

pub(super) fn recognize(
    rb: &Rebuilder<'_>,
    fid: NodeId,
    param_cids: Vec<u32>,
    guards: Vec<(NodeId, NodeId)>,
    rexpr: NodeId,
) -> Option<Plan> {
    // Structural recursion: `HEAD ⊕ f(a…)` / `f(a…) ⊕ HEAD`, gated to a numeric monoid.
    if rb.old.kind(rexpr) != NodeKind::BinOp {
        return None;
    }
    let op = match rb.old.node(rexpr).payload {
        Payload::Op(o) => o,
        _ => return None,
    };
    if !matches!(op, Op::Add | Op::Mul) {
        return None;
    }
    let operands = rb.old.children(rexpr);
    if operands.len() != 2 {
        return None;
    }
    let (a, b) = (operands[0], operands[1]);
    let (head, args) = match (rb.as_self_call(a, fid), rb.as_self_call(b, fid)) {
        (Some(args), None) => (b, args),
        (None, Some(args)) => (a, args),
        _ => return None, // both or neither — not a linear fold
    };
    if args.len() != param_cids.len() || guards.len() != 1 {
        return None;
    }
    let (base_cond, identity) = guards[0];
    // Numeric monoid gate: ⊕ on proven `Number` domain operands (so commutative + associative)
    // and the base case returning ⊕'s identity literal (`0` for `+`, `1` for `·`).
    let ev = rb.param_value_domain_env(fid, &param_cids);
    let operators = semantics(rb.old.meta.lang).operators();
    let head_domain = operators.expression_value_domain(rb.old, head, &|cid| {
        ev.get(&cid).copied().unwrap_or(ValueDomain::Unknown)
    });
    if !operators
        .value_law(ValueLaw::StructuralNumericFold)
        .is_some_and(|contract| contract.requirement.accepts([head_domain]))
    {
        return None;
    }
    // `ValueDomain::Number` does not separate integer from float, but the fold's soundness
    // (right-fold == left-fold) holds only over an associative monoid — and float `+`/`*` is NOT
    // associative (`normalize.recursion.structural_fold` is proven over `Int` only; see
    // Counterexamples.lean). A `Number` HEAD that could carry a float at runtime must therefore be
    // rejected, exactly as the `algebra` IL pass holds float `+`/`*` chains via `possibly_float`.
    if head_possibly_float(rb, fid, head) {
        return None;
    }
    let want_identity = match op {
        Op::Add => 0,
        Op::Mul => 1,
        _ => return None,
    };
    if !rb.is_int_literal(identity, want_identity) {
        return None;
    }
    Some(Plan {
        param_cids,
        base_cond,
        op,
        head,
        args,
        identity,
    })
}

/// Whether the fold's `HEAD` could evaluate to a FLOAT, so `+`/`*` over it is non-associative and
/// the right-fold→left-fold rewrite would not preserve meaning. A truly-untyped dynamic-language
/// param cannot reach here — its `ValueDomain` is `Unknown`, which the `Number` head gate already
/// rejects — so the only float sources are a float literal, a true-division (`Op::TrueDiv`, which
/// is float-valued even over integer operands, unlike truncating `Op::Div`), or a statically
/// float-typed parameter (`f64`/`double`/…). This mirrors `algebra`'s `possibly_float` hold.
fn head_possibly_float(rb: &Rebuilder<'_>, fid: NodeId, head: NodeId) -> bool {
    let float_param_cids: FxHashSet<u32> = rb
        .old
        .children(fid)
        .iter()
        .filter(|&&c| rb.old.kind(c) == NodeKind::Param)
        .filter_map(|&c| match rb.old.node(c).payload {
            Payload::Cid(cid) => domain_evidence_for_param(rb.old, c)
                .is_some_and(|d| d.is_float())
                .then_some(cid),
            _ => None,
        })
        .collect();
    let mut stack = vec![head];
    while let Some(n) = stack.pop() {
        match rb.old.node(n).payload {
            Payload::LitFloat(_) | Payload::Lit(LitClass::Float) => return true,
            Payload::Op(Op::TrueDiv) => return true,
            Payload::Cid(cid)
                if rb.old.kind(n) == NodeKind::Var && float_param_cids.contains(&cid) =>
            {
                return true
            }
            _ => {}
        }
        stack.extend(rb.old.children(n).iter().copied());
    }
    false
}

pub(super) fn build_body(
    rb: &mut Rebuilder<'_>,
    fid: NodeId,
    plan: &Plan,
    span: Span,
) -> Option<NodeId> {
    let acc = rb.fresh_cid(fid);
    let init = {
        let lhs = rb.var(acc, span);
        let rhs = rb.go_val(plan.identity);
        rb.b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
    };
    // `acc = acc ⊕ HEAD` runs first (reads the current params/acc), then the
    // params advance to the next call's arguments.
    let acc_update = {
        let lhs = rb.var(acc, span);
        let cur = rb.var(acc, span);
        let h = rb.go_val(plan.head);
        let combined =
            rb.b.add(NodeKind::BinOp, Payload::Op(plan.op), span, &[cur, h]);
        rb.b.add(NodeKind::Assign, Payload::None, span, &[lhs, combined])
    };
    let mut loop_stmts = vec![acc_update];
    loop_stmts.extend(rb.ordered_updates(&plan.param_cids, &plan.args)?);
    let cond = rb.not_any(vec![plan.base_cond]);
    let loop_body = rb.b.add(NodeKind::Block, Payload::None, span, &loop_stmts);
    let wl = rb.while_loop(cond, loop_body, span);
    let ret = {
        let v = rb.var(acc, span);
        rb.ret(v, span)
    };
    Some(rb.b.add(NodeKind::Block, Payload::None, span, &[init, wl, ret]))
}
