use super::super::*;

impl<'a> Builder<'a> {
    pub(super) fn unary_canon(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
        let ValOp::Un(o) = *op else { return None };
        // ABS IDEMPOTENCE: `abs(abs x) → abs x` (#284). `ABS_CODE` is synthesized
        // only from integer-proven calls or sign ternaries, so the outer Abs is a no-op.
        if o == ABS_CODE && !args.is_empty() {
            if let ValOp::Un(io) = self.nodes[args[0] as usize].op {
                if io == ABS_CODE {
                    return Some(args[0]);
                }
            }
        }
        // BITWISE DE MORGAN: `~(a & b) -> ~a | ~b`, `~(a | b) -> ~a & ~b` (#284).
        // Two's-complement identity for all integers; on a non-integer operand
        // `~`/`&`/`|` Err on both the original and the distributed form, so it is
        // sound for ALL inputs. Pushing `~` inward gives a canonical form that
        // converges `~(a&b)` with an explicit `~a | ~b`.
        if o == Op::BitNot as u32 && !args.is_empty() {
            if let ValOp::Bin(bo) = self.nodes[args[0] as usize].op {
                let flip = if bo == Op::BitAnd as u32 {
                    Some(Op::BitOr as u32)
                } else if bo == Op::BitOr as u32 {
                    Some(Op::BitAnd as u32)
                } else {
                    None
                };
                if let Some(out_op) = flip {
                    let inner = self.nodes[args[0] as usize].args.clone();
                    let negated: Vec<ValueId> = inner
                        .iter()
                        .map(|&t| self.mk(ValOp::Un(Op::BitNot as u32), vec![t]))
                        .collect();
                    if negated.len() == 2 {
                        return Some(self.mk(ValOp::Bin(out_op), negated));
                    }
                }
            }
        }
        // LOGICAL DE MORGAN for value-graph-synthesized `Not`, such as the `all`
        // predicate produced by early-return loop recognition. Source-level logical
        // negation usually reaches the IL algebra pass first; synthesized values do not.
        if o == Op::Not as u32 && !args.is_empty() {
            if let Some(v) = self.logical_demorgan(args[0]) {
                return Some(v);
            }
        }
        // Boolean double negation: unlike truthiness `!!x`, `!!b == b` when the
        // inner value is already proven Boolean (comparisons, boolean params, etc.).
        if o == Op::Not as u32 && !args.is_empty() {
            if let Some(inner) = self.boolean_double_negation(args[0]) {
                return Some(inner);
            }
        }
        // NEGATED COMPARISON: `!(a==b) → a!=b` is value-independent. Order duals
        // such as `!(a<b) → a>=b` require integer-domain proof because NaN makes
        // the apparent dual false while the negated comparison is true.
        // This canonicalizes the residual `Not` the algebra pass leaves on a pushed
        // double-negation (`!!(a<b)` → `!(b<=a)` → `a<b`), converging it with the bare
        // comparison without the unsound untyped `!!x → x`.
        if o == Op::Not as u32
            && !args.is_empty()
            && self.comparison_law_enabled(ComparisonLaw::Negation)
        {
            if let Some(negated) = self.negated_comparison(args[0]) {
                return Some(negated);
            }
        }
        if o == Op::Neg as u32 && !args.is_empty() {
            if let ValOp::Un(io) = self.nodes[args[0] as usize].op {
                let inner = self.nodes[args[0] as usize].args[0];
                if io == Op::Neg as u32
                    && self.value_law_satisfied(ValueLaw::NumericNegationInvolution, &[inner])
                    && self.proven_numeric(inner)
                {
                    return Some(inner);
                }
            }
            // Distribute negation over addition: `-(x + y) → (-x) + (-y)`. Sound for
            // ALL types — `Neg` errors on non-numeric, and so does the distributed
            // form (`-list` is Err either way). Pushing Neg inward gives a canonical
            // form so `-(a+b)` converges with `-a - b` (= `-a + -b`).
            if let ValOp::Bin(bo) = self.nodes[args[0] as usize].op {
                if bo == Op::Add as u32 {
                    let inner = self.nodes[args[0] as usize].args.clone();
                    let mut leaves = Vec::new();
                    for &value in &inner {
                        self.flatten_into(value, Op::Add as u32, &mut leaves);
                    }
                    if !self.add_association_safe(&leaves) {
                        return None;
                    }
                    let negs: Vec<ValueId> = inner
                        .iter()
                        .map(|&t| self.mk(ValOp::Un(Op::Neg as u32), vec![t]))
                        .collect();
                    let mut acc = negs[0];
                    for &n in &negs[1..] {
                        acc = self.mk(ValOp::Bin(Op::Add as u32), vec![acc, n]);
                    }
                    return Some(acc);
                }
            }
        }
        None
    }
    fn logical_demorgan(&mut self, value: ValueId) -> Option<ValueId> {
        let ValOp::Bin(op) = self.nodes[value as usize].op else {
            return None;
        };
        let out_op = if op == Op::And as u32 {
            Op::Or as u32
        } else if op == Op::Or as u32 {
            Op::And as u32
        } else {
            return None;
        };
        let inner = self.nodes[value as usize].args.clone();
        if inner.len() != 2 || !self.value_law_satisfied(ValueLaw::BooleanCommutativity, &inner) {
            return None;
        }
        if !inner.iter().copied().all(|arg| self.reorder_safe(arg)) {
            return None;
        }
        let negated = inner
            .into_iter()
            .map(|arg| self.mk(ValOp::Un(Op::Not as u32), vec![arg]))
            .collect();
        Some(self.mk(ValOp::Bin(out_op), negated))
    }
    fn boolean_double_negation(&self, value: ValueId) -> Option<ValueId> {
        let ValOp::Un(op) = self.nodes[value as usize].op else {
            return None;
        };
        let inner = self.nodes[value as usize].args[0];
        (op == Op::Not as u32
            && (self.vty(inner) == ValueDomain::Boolean
                || self.opaque_comparison_result_is_boolean(inner)))
        .then_some(inner)
    }
    fn opaque_comparison_result_is_boolean(&self, value: ValueId) -> bool {
        let ValOp::Opaque(tag) = self.nodes[value as usize].op else {
            return false;
        };
        if matches!(
            tag,
            JS_LOOSE_EQ_CMP_TAG | JS_LOOSE_NE_CMP_TAG | JS_INSTANCEOF_CMP_TAG
        ) {
            return true;
        }
        [Op::Lt, Op::Le, Op::Gt, Op::Ge]
            .into_iter()
            .any(|op| tag == combine(JS_RELATIONAL_CMP_TAG, op as u64))
    }
    pub(super) fn negated_comparison(&mut self, comparison: ValueId) -> Option<ValueId> {
        let ValOp::Bin(op) = self.nodes[comparison as usize].op else {
            return None;
        };
        let negated = negate_cmp_code(self.il.meta.lang, op)?;
        let args = self.nodes[comparison as usize].args.clone();
        if matches!(op_from_code(op), Some(Op::Lt | Op::Le | Op::Gt | Op::Ge))
            && !args
                .iter()
                .copied()
                .all(|arg| self.is_integer_domain_value(arg))
        {
            return None;
        }
        Some(self.mk(ValOp::Bin(negated), args))
    }
}
