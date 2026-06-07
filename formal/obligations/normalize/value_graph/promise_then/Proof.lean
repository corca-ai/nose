/-
Soundness of nose's Promise `.then` continuation canonicalization.

The value graph already strips `await e` to its operand `e`, i.e. it models a *settled* Promise
by its resolved value (an eager / identity-monad model — sound for clone detection, where the
two forms compute the same result and the scheduling difference is irrelevant). Under that model
`p.then(λr. body r)` is just the continuation applied to the resolved value, which is exactly
what `let r = await p; body r` computes. So the value graph beta-reduces the `.then` callback
over the receiver (`eval_promise_then`), and a `.then`-chain is the nested await composition.

This file proves those two equalities for the model. Self-contained; checked by the formal
obligation CI gate.
-/

namespace NosePromiseThen

/-- `await p` on a settled promise: the value graph models a settled `Promise α` by its resolved
    value `α`, so awaiting is the identity. -/
def awaitP (p : α) : α := p

/-- `p.then(f)` on a settled promise: run the continuation on the resolved value. -/
def thenP (p : α) (f : α → β) : β := f p

/-- The rewrite `eval_promise_then` performs: `p.then(λr. body r)` ≡ `let r = await p; body r`.
    Beta-reducing the callback over the receiver is meaning-preserving. -/
theorem then_eq_await_bind (p : α) (f : α → β) : thenP p f = f (awaitP p) := rfl

/-- A `.then`-chain is the nested sequential composition of the continuations — so
    `p.then(f).then(g)` converges with `let r = await p; let s = await f(r); g(s)`. This is why
    chains reduce recursively (evaluating the receiver triggers the inner `.then`). -/
theorem then_chain (p : α) (f : α → β) (g : β → γ) :
    thenP (thenP p f) g = thenP p (fun r => g (f r)) := rfl

end NosePromiseThen
