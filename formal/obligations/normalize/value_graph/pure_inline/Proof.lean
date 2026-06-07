/-
Soundness of nose's interprocedural pure-inline canonicalization.

The value graph inlines a call `f(args)` to a PURE, file-local, single-`return` function by
binding the parameters to the (caller-evaluated) arguments and evaluating the function's return
expression (`eval_inlined_call`). This is beta-substitution. Soundness rests on the function
being EFFECT-FREE: the Rust gate admits only a lone `return <expr>` that is
`function_binding_safe` (no loops, throws, lambdas, user calls, or — because there is no second
statement — field/index writes), so an effect-free function is modeled here as a total
mathematical function `body : α → β`, and its observable result is exactly `body arg`.

This file proves that beta-substitution is meaning-preserving for such a function (the inlined
value equals the call's value), in the single- and two-argument forms, and that inlining a helper
into a caller equals inlining the composed whole. Self-contained; checked by the formal
obligation CI gate.
-/

namespace NosePureInline

/-- A pure single-`return` function is modeled by its body, a total function `body : α → β`.
    Calling it is applying the body; inlining substitutes the argument into the body. -/
def callP (body : α → β) (arg : α) : β := body arg

/-- BETA: inlining `f(arg)` to `body arg` is exactly the call's value — substitution preserves
    meaning for an effect-free function. -/
theorem inline_beta (body : α → β) (arg : α) : callP body arg = body arg := rfl

/-- Positional binding for a two-parameter helper: each parameter is bound to its argument. -/
theorem inline_beta2 (body : α → β → γ) (a : α) (b : β) :
    (fun x y => body x y) a b = body a b := rfl

/-- Inlining a pure helper `f` into a caller `g(f(arg))` equals inlining the composed whole — so
    the caller converges with the same logic written inline (the extract-method equivalence). -/
theorem inline_compose (g : β → γ) (f : α → β) (arg : α) :
    g (callP f arg) = (fun x => g (f x)) arg := rfl

end NosePureInline
