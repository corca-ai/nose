//! Path-conditioned sink emission for value-graph returns, throws, and effects.
//!
//! proof-obligation: normalize.control_flow.guard_returns

use super::*;

impl<'a> Builder<'a> {
    /// Push an effect sink, tagged with the current path condition — so a *conditional*
    /// effect (`if c { append(x) }`) carries `c`, the way a guarded return does.
    pub(super) fn push_effect(&mut self, v: ValueId) {
        let ord = self.next_effect_ordinal();
        let g = self.guarded(v);
        self.sinks.push(Sink::ordered_effect(g, ord));
    }

    pub(super) fn next_effect_ordinal(&mut self) -> u32 {
        let ord = self.effect_slot;
        self.effect_slot = self.effect_slot.saturating_add(1);
        ord
    }

    pub(super) fn emit_throw(&mut self, v: ValueId) {
        let g = self.guarded(v);
        self.sinks.push(Sink::new(SinkKind::Throw, g));
    }

    /// Tag a value with the current path condition: under branch conditions, the
    /// returned/thrown value is `Phi(path, v, ⊥)` (a sentinel for "not on this path"),
    /// so two branches that return swapped values no longer form the same multiset.
    /// Push a `Return` sink for value `v`, DECOMPOSING a ternary return into guarded
    /// returns. `return (a if c else b)` is behaviorally `if c {return a} else {return b}`,
    /// so we split a `Phi(c, a, b)` return into a `c`-guarded return of `a` and a
    /// `¬c`-guarded return of `b` — exactly the sink set the if-else / elif writing already
    /// produces via guard-clause path narrowing. Recursing on nested `Phi` makes a nested
    /// ternary converge with an `elif` cascade. Sound (behavior-preserving) and gated by the
    /// `verify` oracle; the abs/min/max idiom recognition runs first in `mk`, so a recognized
    /// `Abs`/`Min`/`Max` return is NOT a bare `Phi` here and stays atomic. Only genuine
    /// ternaries (both arms real values, not the `bot` placeholder) are decomposed.
    pub(super) fn emit_return(&mut self, v: ValueId) {
        // An active inline capture intercepts the callee's returns BEFORE ternary
        // decomposition: the raw value plus its inline-relative guard is exactly what
        // the post-body `Phi` fold needs (decomposition happens again, in caller
        // context, when the folded value reaches a real return). A return executing
        // inside a callee loop has first-match-wins semantics across iterations that a
        // single value cannot express — poison the frame so the inline fails closed.
        if let Some(frame) = self.inline_capture.last_mut() {
            if self.loop_depth > frame.loop_depth_base {
                frame.poisoned = true;
                return;
            }
            let base = frame.path_base;
            let guard = self.path_cond_from(base);
            // Re-borrow: `path_cond_from` needed `&mut self` to intern `And` nodes.
            if let Some(frame) = self.inline_capture.last_mut() {
                frame.returns.push((guard, v));
            }
            return;
        }
        if let ValOp::Phi = self.nodes[v as usize].op {
            let args = self.nodes[v as usize].args.clone();
            if args.len() == 3 {
                let bot = self.sentinel_const(sentinel::BOTTOM);
                let (cond, then_v, else_v) = (args[0], args[1], args[2]);
                if then_v != bot && else_v != bot {
                    self.path.push(cond);
                    self.emit_return(then_v);
                    self.path.pop();
                    let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
                    self.path.push(ncond);
                    self.emit_return(else_v);
                    self.path.pop();
                    return;
                }
            }
        }
        let g = self.guarded(v);
        self.sinks.push(Sink::new(SinkKind::Return, g));
    }

    pub(super) fn guarded(&mut self, v: ValueId) -> ValueId {
        match self.path_cond() {
            None => v,
            Some(pc) => {
                let bot = self.sentinel_const(sentinel::BOTTOM);
                self.mk(ValOp::Phi, vec![pc, v, bot])
            }
        }
    }
}
