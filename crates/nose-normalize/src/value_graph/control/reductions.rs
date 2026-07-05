use super::super::*;

impl<'a> Builder<'a> {
    /// Recognize an accumulator update `acc = acc ⊕ contrib` (⊕ associative and
    /// commutative) where `acc` is the previous-iteration value `loopv`. Returns the
    /// operator code and the canonical per-element `contrib` (with `acc` removed), or
    /// `None` if the update is not a single clean reduction step.
    pub(in crate::value_graph) fn as_reduction(
        &mut self,
        val: ValueId,
        loopv: ValueId,
    ) -> Option<(u32, ValueId)> {
        let mut cache = ReductionCache::default();
        self.as_reduction_cached(val, loopv, &mut cache)
    }

    pub(in crate::value_graph) fn as_loop_reduction_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        loop_context: &[ValueId],
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        if let ValOp::Reduce(op) = self.nodes[val as usize].op {
            let args = self.nodes[val as usize].args.clone();
            if is_selection_code(op)
                && args.len() == 1
                && !self.references_cached(args[0], loopv, cache)
                && self.references_any_cached(args[0], loop_context, cache)
            {
                return Some((op, args[0]));
            }
            if args.len() == 2
                && args[0] == loopv
                && !self.references_cached(args[1], loopv, cache)
                && self.references_any_cached(args[1], loop_context, cache)
            {
                return Some((op, args[1]));
            }
        }
        self.as_reduction_cached(val, loopv, cache)
    }

    pub(in crate::value_graph) fn as_reduction_cached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let key = (val, loopv);
        if let Some(cached) = cache.reductions.get(&key).copied() {
            return cached;
        }
        let result = self.as_reduction_uncached(val, loopv, cache);
        cache.reductions.insert(key, result);
        result
    }

    pub(in crate::value_graph) fn as_reduction_uncached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        // Guarded (filtered) reduction: `if cond { acc = acc ⊕ contrib }` merges to
        // `Phi(cond, ⊕(acc, contrib), acc)`. Canonicalize to `Reduce(⊕, init, cond ?
        // contrib : identity)` so a filtered loop converges with `sum(c for x if cond)`
        // and the per-element contribution becomes 0/1 (the op identity) when filtered.
        if matches!(self.nodes[val as usize].op, ValOp::Phi) {
            return self.phi_reduction_step(val, loopv, cache);
        }
        // A min/max accumulator written as a Min/Max node (`minmax_pattern` turned the
        // conditional update `if x>acc { acc=x }` into `Max(acc, x)`): map the idiom code
        // to the selection-reduction code so the loop converges with the `max()`/`min()`
        // builtin (both → `Reduce(REDUCE_MAX/MIN, [contrib])`).
        if let Some(step) = self.minmax_selection_step(val, loopv, cache) {
            return Some(step);
        }
        let op = match self.nodes[val as usize].op {
            ValOp::Bin(o) if is_assoc_comm_code(o) => o,
            _ => return None,
        };
        let mut operands = Vec::new();
        self.flatten_into(val, op, &mut operands);
        // Exactly one top-level operand must be the previous accumulator, and it must
        // not reappear nested in the remaining contribution (`acc = acc + acc*x`).
        if operands.iter().filter(|&&o| o == loopv).count() != 1 {
            return None;
        }
        let pos = operands.iter().position(|&o| o == loopv)?;
        operands.remove(pos);
        if operands.is_empty() {
            return None;
        }
        for &operand in &operands {
            if self.references_cached(operand, loopv, cache) {
                return None;
            }
        }
        operands.sort_by_key(|&v| self.vhash[v as usize]);
        let mut acc = operands[0];
        for &o in &operands[1..] {
            acc = self.mk(ValOp::Bin(op), vec![acc, o]);
        }
        Some((op, acc))
    }

    fn phi_reduction_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let args = self.nodes[val as usize].args.clone();
        if let Some(step) = self.phi_guarded_reduction(&args, loopv, cache) {
            return Some(step);
        }
        if let Some(step) = self.phi_swapped_guard_reduction(&args, loopv, cache) {
            return Some(step);
        }
        self.phi_branch_contribution(&args, loopv, cache)
    }

    fn phi_guarded_reduction(
        &mut self,
        args: &[ValueId],
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        if args.len() == 3 && args[2] == loopv {
            // (a) guarded accumulation: `if cond { acc = acc ⊕ contrib }`.
            if let Some((op, contrib)) = self
                .as_reduction_cached(args[1], loopv, cache)
                .or_else(|| self.nested_reduce_step(args[1], loopv, cache))
            {
                if let Some(id) = identity_of(op) {
                    let ident = self.int_const(id);
                    let guarded = self.mk(ValOp::Phi, vec![args[0], contrib, ident]);
                    return Some((op, guarded));
                }
            }
            // (b) selection (min/max): `if cand {>,<} acc { acc = cand }` —
            // the new value does not reference the old accumulator and the guard
            // compares the two. `acc = max(acc, cand)` / `min`.
            let cand = args[1];
            if !self.references_cached(cand, loopv, cache) {
                if let Some(code) = self.selection_code(args[0], cand, loopv) {
                    return Some((code, cand));
                }
            }
        }
        None
    }

    fn phi_swapped_guard_reduction(
        &mut self,
        args: &[ValueId],
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        // Swapped polarity: `if cond { acc } else { acc ⊕ contrib }`. cfg_norm can
        // flip a two-branch ternary's orientation, so the accumulator lands in the
        // THEN branch with a negated guard — a `functools.reduce(lambda acc,v: acc+v
        // if v>0 else acc, …)` lowers to `if v<=0 { acc } else { acc+v }`. Recognize
        // it with the negated guard so it converges with the loop form `if v>0:
        // acc+=v` (whose single-branch guard stays positive).
        if args.len() == 3 && args[1] == loopv {
            if let Some((op, contrib)) = self
                .as_reduction_cached(args[2], loopv, cache)
                .or_else(|| self.nested_reduce_step(args[2], loopv, cache))
            {
                if let Some(id) = identity_of(op) {
                    let ident = self.int_const(id);
                    let ncond = self.negate_guard(args[0]);
                    let guarded = self.mk(ValOp::Phi, vec![ncond, contrib, ident]);
                    return Some((op, guarded));
                }
            }
        }
        None
    }

    fn phi_branch_contribution(
        &mut self,
        args: &[ValueId],
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        // Full conditional contribution: both branches update the accumulator once,
        // e.g. `if x < 0 { total += -x } else { total += x }`. This is one reduction
        // whose per-element contribution is itself a branch value:
        // `Reduce(⊕, init, cond ? then_contrib : else_contrib)`. The `Phi` builder
        // then canonicalizes idioms such as `x < 0 ? -x : x` to `Abs(x)`.
        if args.len() == 3 {
            if let (Some((then_op, then_contrib)), Some((else_op, else_contrib))) = (
                self.as_reduction_cached(args[1], loopv, cache),
                self.as_reduction_cached(args[2], loopv, cache),
            ) {
                if then_op == else_op {
                    let contrib = self.mk(ValOp::Phi, vec![args[0], then_contrib, else_contrib]);
                    return Some((then_op, contrib));
                }
            }
        }
        None
    }

    fn minmax_selection_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let (op, left, right) = self.bin_op_args(val)?;
        if op != MIN_CODE && op != MAX_CODE {
            return None;
        }
        let red = if op == MAX_CODE {
            REDUCE_MAX
        } else {
            REDUCE_MIN
        };
        if left == loopv && !self.references_cached(right, loopv, cache) {
            return Some((red, right));
        }
        if right == loopv && !self.references_cached(left, loopv, cache) {
            return Some((red, left));
        }
        None
    }

    pub(in crate::value_graph) fn nested_reduce_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let ValOp::Reduce(op) = self.nodes[val as usize].op else {
            return None;
        };
        let args = self.nodes[val as usize].args.clone();
        if args.len() == 2 && args[0] == loopv && !self.references_cached(args[1], loopv, cache) {
            return Some((op, args[1]));
        }
        None
    }

    /// The canonical negation of a guard value: a comparison flips to its complement
    /// (`a<=b` → `a>b`, `a==b` → `a!=b`, …) — same operands, so a negated guard
    /// converges with the positive guard a loop produces — and anything else is wrapped
    /// in logical `Not`.
    pub(in crate::value_graph) fn negate_guard(&mut self, v: ValueId) -> ValueId {
        if self.comparison_law_enabled(ComparisonLaw::Negation) {
            if let Some((opc, left, right)) = self.bin_op_args(v) {
                if let Some(flip) = negate_cmp_code(self.il.meta.lang, opc) {
                    return self.mk(ValOp::Bin(flip), vec![left, right]);
                }
            }
        }
        self.mk(ValOp::Un(Op::Not as u32), vec![v])
    }

    /// If `cond` compares `cand` against the accumulator `loopv` (`cand > loopv` etc.),
    /// classify the selection as max or min. Operand order is meaningful (comparisons
    /// are not commutative-canonicalized), so `cand > acc` and `acc < cand` both → max.
    pub(in crate::value_graph) fn selection_code(
        &self,
        cond: ValueId,
        cand: ValueId,
        loopv: ValueId,
    ) -> Option<u32> {
        if !self.comparison_law_enabled(ComparisonLaw::SelectionReductionGuard) {
            return None;
        }
        let (opc, left, right) = self.bin_op_args(cond)?;
        let cand_first = left == cand && right == loopv;
        let acc_first = left == loopv && right == cand;
        if !cand_first && !acc_first {
            return None;
        }
        // `cand > acc` / `acc < cand` → take the larger ⇒ max; the reverse ⇒ min.
        let greater = opc == Op::Gt as u32 || opc == Op::Ge as u32;
        let lesser = opc == Op::Lt as u32 || opc == Op::Le as u32;
        if (greater && cand_first) || (lesser && acc_first) {
            Some(REDUCE_MAX)
        } else if (lesser && cand_first) || (greater && acc_first) {
            Some(REDUCE_MIN)
        } else {
            None
        }
    }
}
