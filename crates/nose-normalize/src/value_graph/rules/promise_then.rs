//! Promise `.then` continuation canonicalization.
//!
//! A settled Promise is modeled by its resolved value (the value graph strips `await` to its
//! operand), so `p.then(λr. body)` is the continuation applied to that value — exactly what
//! `let r = await p; body` computes. Beta-reduce the single-argument `.then` callback over the
//! receiver, so a `.then`-chain converges with the equivalent await / sequential code. Chains
//! reduce recursively (evaluating the receiver triggers the inner `.then`). `.then(fnRef)` (no
//! lambda body), the two-argument `.then(onOk, onErr)`, and `.catch`/`.finally` (error/cleanup
//! continuations whose parameter is the error, not the resolved value) are left opaque.
//!
//! proof-obligation: normalize.value_graph.promise_then

use super::super::{Builder, ValueId};
use nose_il::{NodeId, NodeKind, Payload};
use rustc_hash::FxHashMap;

pub(in super::super) fn apply(
    builder: &mut Builder<'_>,
    expr: NodeId,
    env: &FxHashMap<u32, ValueId>,
) -> Option<ValueId> {
    let kids = builder.il.children(expr).to_vec();
    if kids.len() != 2 {
        return None;
    }
    let callee = kids[0];
    if builder.il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(s) = builder.il.node(callee).payload else {
        return None;
    };
    if builder.interner.resolve(s) != "then" {
        return None;
    }
    let cb = kids[1];
    if builder.il.kind(cb) != NodeKind::Lambda {
        return None;
    }
    let &recv = builder.il.children(callee).first()?;
    let recv_v = builder.eval(recv, env);
    builder.eval_lambda_body(cb, &[recv_v], env)
}
