//! Promise `.then` continuation canonicalization.
//!
//! The surface contract exists for JS-like `.then(lambda)` calls, but exact beta-reduction is
//! admitted only when a pack-provided receiver proof establishes Promise/thenable semantics.
//! Current IL does not carry that proof, so this rule is fail-closed until the async protocol
//! extension point can validate the receiver.
//!
//! proof-obligation: normalize.value_graph.promise_then

use super::super::{Builder, ValueId};
use nose_il::{NodeId, NodeKind, Payload};
use nose_semantics::{library_promise_then_contract, AsyncReceiverContract};
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
    let contract =
        library_promise_then_contract(builder.il.meta.lang, builder.interner.resolve(s), 1)?.result;
    let cb = kids[1];
    if builder.il.kind(cb) != NodeKind::Lambda {
        return None;
    }
    let &recv = builder.il.children(callee).first()?;
    if !promise_receiver_proven(builder, contract.receiver, recv) {
        return None;
    }
    let recv_v = builder.eval(recv, env);
    builder.eval_lambda_body(cb, &[recv_v], env)
}

fn promise_receiver_proven(
    _builder: &Builder<'_>,
    receiver: AsyncReceiverContract,
    _recv: NodeId,
) -> bool {
    match receiver {
        // The current IL does not retain a resolved Promise/thenable type or constructor fact.
        // Until a language pack can provide that proof, exact `.then` beta-reduction is closed.
        AsyncReceiverContract::ExactPromiseLike => false,
    }
}
