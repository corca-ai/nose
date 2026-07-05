use super::super::*;

impl<'a> Builder<'a> {
    // Recognize select idioms on EVERY branch merge — `Phi(cond, then, els)` is built
    // both by a ternary (`a if c else b`) and by an if/else that assigns a variable, so
    // doing this here (not just at the ternary) keeps the two forms convergent:
    //   integer-proven `x if x>=0 else -x` → Abs(x);
    //   `x if x<y else y` → Min(x,y) / Max(x,y).
    pub(super) fn phi_select_idioms(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
        if !matches!(*op, ValOp::Phi) || args.len() != 3 {
            return None;
        }
        if self.bool_const(args[1]) == Some(true) && self.bool_const(args[2]) == Some(false) {
            return Some(args[0]);
        }
        if self.bool_const(args[1]) == Some(false) && self.bool_const(args[2]) == Some(true) {
            return Some(self.mk(ValOp::Un(Op::Not as u32), vec![args[0]]));
        }
        if let Some(v) = self.phi_branch_orientation(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.boolean_guarded_identity_phi(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.abs_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.minmax_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.clamp_ternary_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.low_bit_toggle_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.map_default_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.value_default_pattern(args[0], args[1], args[2]) {
            return Some(v);
        }
        if let Some(v) = self.flatten_nested_guarded_identity_phi(args[0], args[1], args[2]) {
            return Some(v);
        }
        None
    }
    fn phi_branch_orientation(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.vhash[then_v as usize] <= self.vhash[else_v as usize] {
            return None;
        }
        let negated = self.negated_comparison(cond)?;
        Some(self.mk(ValOp::Phi, vec![negated, else_v, then_v]))
    }
    fn boolean_guarded_identity_phi(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.vty(cond) != ValueDomain::Boolean || self.vty(then_v) != ValueDomain::Boolean {
            return None;
        }
        match self.bool_const(else_v) {
            Some(false) => Some(self.mk(ValOp::Bin(Op::And as u32), vec![cond, then_v])),
            Some(true) => {
                let not_then = self.mk(ValOp::Un(Op::Not as u32), vec![then_v]);
                let failure = self.mk(ValOp::Bin(Op::And as u32), vec![cond, not_then]);
                Some(self.mk(ValOp::Un(Op::Not as u32), vec![failure]))
            }
            None => None,
        }
    }
    fn flatten_nested_guarded_identity_phi(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        let inner_args = {
            let inner = &self.nodes[then_v as usize];
            if !matches!(inner.op, ValOp::Phi) || inner.args.len() != 3 || inner.args[2] != else_v {
                return None;
            }
            inner.args.clone()
        };
        let both = self.mk(ValOp::Bin(Op::And as u32), vec![cond, inner_args[0]]);
        Some(self.mk(ValOp::Phi, vec![both, inner_args[1], else_v]))
    }
    /// Recognize integer absolute-value idioms `x if x>=0 else -x` (and mirrors)
    /// as `Un(ABS_CODE, [x])`, so they converge with integer-proven `abs(x)`.
    fn abs_pattern(&mut self, cond: ValueId, then: ValueId, els: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::AbsSignTernary) {
            return None;
        }
        let is_neg_of = |s: &Self, neg: ValueId, base: ValueId| {
            matches!(s.nodes[neg as usize].op, ValOp::Un(o) if o == Op::Neg as u32)
                && s.nodes[neg as usize].args == [base]
        };
        // (v, the positive branch is `then`)
        let (v, pos_is_then) = if is_neg_of(self, els, then) {
            (then, true) // then = v (the x>=0 branch), else = -v
        } else if is_neg_of(self, then, els) {
            (els, false) // else = v, then = -v
        } else {
            return None;
        };
        if !self.is_integer_domain_value(v) {
            return None;
        }
        let (opc, left, right) = self.bin_op_args(cond)?;
        let is_zero = |s: &Self, id: ValueId| {
            matches!(
                s.nodes[id as usize].op,
                ValOp::Const {
                    kind: ConstKind::Int,
                    bits: 0
                }
            )
        };
        let (nonneg, neg) = if left == v && is_zero(self, right) {
            (
                opc == Op::Ge as u32 || opc == Op::Gt as u32,
                opc == Op::Lt as u32 || opc == Op::Le as u32,
            )
        } else if right == v && is_zero(self, left) {
            (
                opc == Op::Le as u32 || opc == Op::Lt as u32,
                opc == Op::Gt as u32 || opc == Op::Ge as u32,
            )
        } else {
            return None;
        };
        // `then` is the value when the condition holds: positive branch must be `v`
        // exactly when the condition says v is non-negative.
        if (nonneg && pos_is_then) || (neg && !pos_is_then) {
            Some(self.mk(ValOp::Un(ABS_CODE), vec![v]))
        } else {
            None
        }
    }
    /// Recognize a 2-way min/max selection `x if x<y else y` (and its variants) as a
    /// canonical `Bin(MIN_CODE/MAX_CODE, [x, y])`, so the ternary idiom converges with a
    /// `min(x, y)` / `max(x, y)` call. The condition has already been canonicalized to the
    /// `</<= ` family by `mk` (`x>y` → `y<x`). Sound: it is the literal meaning of the
    /// ternary (and `MIN_CODE`/`MAX_CODE` are interpreted as exactly that).
    fn minmax_pattern(&mut self, cond: ValueId, then: ValueId, els: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::MinMaxTernary) {
            return None;
        }
        let (opc, x, y) = self.bin_op_args(cond)?;
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) {
            return None;
        }
        // cond is `x < y`: `x if x<y else y` → min(x,y); `y if x<y else x` → max(x,y).
        if then == x && els == y {
            Some(self.mk(ValOp::Bin(MIN_CODE), vec![x, y]))
        } else if then == y && els == x {
            Some(self.mk(ValOp::Bin(MAX_CODE), vec![x, y]))
        } else {
            None
        }
    }
    /// Recognize the two-comparison integer clamp surface after the inner ternary has already
    /// become a `Min`/`Max`: `lo if x < lo else min(x, hi)` or
    /// `hi if hi < x else max(x, lo)`. It still requires the same bound-order proof as nested
    /// min/max clamp composition, so unproven parameter bounds and float domains stay separate.
    fn clamp_ternary_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        let (opc, left, right) = self.bin_op_args(cond)?;
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) {
            return None;
        }

        if then == right {
            let hi = self.bin_other_arg(els, MIN_CODE, left)?;
            return self.proof_backed_clamp_value(left, right, hi);
        }
        if then == left {
            let lo = self.bin_other_arg(els, MAX_CODE, right)?;
            return self.proof_backed_clamp_value(right, lo, left);
        }
        None
    }
    /// Java integer low-bit toggle: `x % 2 == 0 ? x + 1 : x - 1` (and the
    /// equivalent `!= 0` branch order) is exactly `x ^ 1`. The branch split avoids
    /// overflow at both signed extremes: max values take the `-1` branch and min
    /// values take the `+1` branch. Keep this Java-only for now because the real
    /// frontier evidence is Java and other surfaces may expose overload/coercion
    /// semantics for these operators.
    fn low_bit_toggle_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        if !self.has_java_primitive_integer_ops() {
            return None;
        }
        let (base, even_when_true) = self.parity_zero_condition(cond)?;
        let then_delta = self.additive_one_delta(then, base)?;
        let else_delta = self.additive_one_delta(els, base)?;
        let is_toggle = (even_when_true && then_delta == 1 && else_delta == -1)
            || (!even_when_true && then_delta == -1 && else_delta == 1);
        if !is_toggle {
            return None;
        }
        let one = self.int_const(1);
        Some(self.mk(ValOp::Bin(Op::BitXor as u32), vec![base, one]))
    }
    fn has_java_primitive_integer_ops(&self) -> bool {
        semantics(self.il.meta.lang)
            .stdlib()
            .java_primitive_integer_ops()
    }
    fn parity_zero_condition(&self, cond: ValueId) -> Option<(ValueId, bool)> {
        let (op, left, right) = self.bin_op_args(cond)?;
        let even_when_true = match op {
            o if o == Op::Eq as u32 => true,
            o if o == Op::Ne as u32 => false,
            _ => return None,
        };
        for (candidate, zero) in [(left, right), (right, left)] {
            if self.int_const_eq(zero, 0) {
                if let Some(base) = self.mod_by_two_base(candidate) {
                    return Some((base, even_when_true));
                }
            }
        }
        None
    }
    fn mod_by_two_base(&self, value: ValueId) -> Option<ValueId> {
        let (op, left, right) = self.bin_op_args(value)?;
        if op != Op::Mod as u32 {
            return None;
        }
        if self.int_const_eq(right, 2) {
            Some(left)
        } else {
            None
        }
    }
    fn additive_one_delta(&mut self, value: ValueId, base: ValueId) -> Option<i8> {
        if !matches!(self.bin_op_args(value), Some((op, _, _)) if op == Op::Add as u32) {
            return None;
        }
        let mut leaves = Vec::new();
        self.flatten_into(value, Op::Add as u32, &mut leaves);
        if leaves.len() != 2 {
            return None;
        }
        if leaves[0] == base {
            self.signed_one_const(leaves[1])
        } else if leaves[1] == base {
            self.signed_one_const(leaves[0])
        } else {
            None
        }
    }
    fn signed_one_const(&self, value: ValueId) -> Option<i8> {
        if self.int_const_eq(value, 1) {
            return Some(1);
        }
        if self.int_const_eq(value, -1) {
            return Some(-1);
        }
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Un(o) if o == Op::Neg as u32)
            && node.args.len() == 1
            && self.int_const_eq(node.args[0], 1)
        {
            return Some(-1);
        }
        None
    }
}
