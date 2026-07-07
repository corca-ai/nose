use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn process_block(
        &mut self,
        block: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        let path_base = self.path.len();
        let bound_order_base = self.bound_order_facts.len();
        for s in self.il.children(block).to_vec() {
            self.process_stmt(s, env);
            // GUARD-CLAUSE normalization: an `if c { …terminates… }` with no else means
            // the REST of the block is reached only when `!c`. Narrow the path by `!c`
            // for the remaining statements, so a guard-clause (`if c {return a}; return b`)
            // produces the same guarded sinks as the if-else form (`if c {return a} else
            // {return b}`) — converging the two writings of the same function (e.g. sympy
            // `symmetric_residue` vs `gf_int`). Cascades for stacked guards.
            if let Some((cond_node, ncond)) = self.guard_clause_negation(s, env) {
                self.record_bound_order_fact_for_condition(
                    cond_node,
                    BoundOrderGuardActivation::WhenFalse,
                    env,
                );
                self.path.push(ncond);
                continue;
            }
            // Statements after an UNCONDITIONAL terminator (a `return`/`throw` at this
            // block level — only when no guard narrowing is in effect) are unreachable
            // dead code; the interpreter takes the first return, so the value graph must
            // too (else C `#if return 1 #else return 0`, both preproc branches lowered
            // live, emits two order-independent return sinks → a branch-swapped twin
            // collapses to the same multiset while behaving differently — a false merge).
            if self.path.len() == path_base
                && matches!(self.il.kind(s), NodeKind::Return | NodeKind::Throw)
            {
                break;
            }
        }
        self.bound_order_facts.truncate(bound_order_base);
        self.path.truncate(path_base);
    }

    /// If `s` is a guard clause — `if c { …unconditionally exits… }` with no else — return
    /// `!c` (the condition under which control falls through to the rest of the block).
    /// Used to narrow the path so guard-clause and if-else writings of a function converge.
    pub(in crate::value_graph) fn guard_clause_negation(
        &mut self,
        s: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<(NodeId, ValueId)> {
        if self.il.kind(s) != NodeKind::If {
            return None;
        }
        let kids = self.il.children(s).to_vec();
        if kids.len() != 2 || !self.branch_exits(kids[1]) {
            return None; // has an else, or the then-branch can fall through
        }
        let cond = self.eval(kids[0], env);
        let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
        Some((kids[0], ncond))
    }

    pub(in crate::value_graph) fn record_bound_order_fact_for_condition(
        &mut self,
        cond_node: NodeId,
        activation: BoundOrderGuardActivation,
        env: &FxHashMap<u32, ValueId>,
    ) {
        if let Some((lo_node, hi_node)) = bound_order_guard_for_node(self.il, cond_node, activation)
        {
            let lo = self.eval(lo_node, env);
            let hi = self.eval(hi_node, env);
            self.bound_order_facts.push((lo, hi));
        }
    }

    pub(in crate::value_graph) fn has_bound_order_fact(&self, lo: ValueId, hi: ValueId) -> bool {
        if let (Some(lo_value), Some(hi_value)) =
            (self.int_const_value(lo), self.int_const_value(hi))
        {
            return lo_value <= hi_value;
        }
        self.bound_order_facts
            .iter()
            .any(|&(fact_lo, fact_hi)| fact_lo == lo && fact_hi == hi)
    }

    pub(in crate::value_graph) fn is_safe_clamp_integer_value(&self, value: ValueId) -> bool {
        self.int_const_value(value).is_some() || self.is_param_value(value, DomainEvidence::Integer)
    }

    pub(in crate::value_graph) fn proof_backed_clamp_value(
        &mut self,
        x: ValueId,
        lo: ValueId,
        hi: ValueId,
    ) -> Option<ValueId> {
        if self.is_safe_clamp_integer_value(x)
            && self.is_safe_clamp_integer_value(lo)
            && self.is_safe_clamp_integer_value(hi)
            && self.has_bound_order_fact(lo, hi)
        {
            let value = self.mk(ValOp::Clamp, vec![x, lo, hi]);
            self.record_value_law(ValueLaw::IntegerClampOrderedMinMax);
            Some(value)
        } else {
            None
        }
    }

    pub(in crate::value_graph) fn clamp_minmax_candidates(
        &self,
        value: ValueId,
    ) -> Vec<(ValueId, ValueId, ValueId)> {
        let mut out = Vec::new();
        if let Some((outer_a, outer_b)) = self.bin_args(value, MIN_CODE) {
            for (inner, hi) in [(outer_a, outer_b), (outer_b, outer_a)] {
                if let Some((inner_a, inner_b)) = self.bin_args(inner, MAX_CODE) {
                    out.push((inner_a, inner_b, hi));
                    out.push((inner_b, inner_a, hi));
                }
            }
        }
        if let Some((outer_a, outer_b)) = self.bin_args(value, MAX_CODE) {
            for (inner, lo) in [(outer_a, outer_b), (outer_b, outer_a)] {
                if let Some((inner_a, inner_b)) = self.bin_args(inner, MIN_CODE) {
                    out.push((inner_a, lo, inner_b));
                    out.push((inner_b, lo, inner_a));
                }
            }
        }
        out
    }

    pub(in crate::value_graph) fn bin_args(
        &self,
        value: ValueId,
        want: u32,
    ) -> Option<(ValueId, ValueId)> {
        let (op, left, right) = self.bin_op_args(value)?;
        (op == want).then_some((left, right))
    }

    pub(in crate::value_graph) fn bin_op_args(
        &self,
        value: ValueId,
    ) -> Option<(u32, ValueId, ValueId)> {
        match &self.nodes[value as usize] {
            ValNode {
                op: ValOp::Bin(op),
                args,
            } if args.len() == 2 => Some((*op, args[0], args[1])),
            _ => None,
        }
    }

    pub(in crate::value_graph) fn bin_other_arg(
        &self,
        value: ValueId,
        want: u32,
        known: ValueId,
    ) -> Option<ValueId> {
        let (left, right) = self.bin_args(value, want)?;
        if left == known {
            Some(right)
        } else if right == known {
            Some(left)
        } else {
            None
        }
    }

    /// Does this branch unconditionally exit its enclosing block (return / throw / break /
    /// continue on every path)? Conservative: a block exits iff its last statement does;
    /// an `if` exits iff both arms do.
    pub(in crate::value_graph) fn branch_exits(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue => true,
            NodeKind::ExprStmt => self.il.children(node).first().is_some_and(|&expr| {
                matches!(
                    self.il.kind(expr),
                    NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
                )
            }),
            NodeKind::Block => self
                .il
                .children(node)
                .last()
                .is_some_and(|&c| self.branch_exits(c)),
            NodeKind::If => {
                let k = self.il.children(node);
                k.len() >= 3 && self.branch_exits(k[1]) && self.branch_exits(k[2])
            }
            _ => false,
        }
    }

    pub(in crate::value_graph) fn branch_returns(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Return => true,
            NodeKind::Block => self
                .il
                .children(node)
                .last()
                .is_some_and(|&c| self.branch_returns(c)),
            NodeKind::If => {
                let k = self.il.children(node);
                k.len() >= 3 && self.branch_returns(k[1]) && self.branch_returns(k[2])
            }
            _ => false,
        }
    }

    /// Does this returning branch provably evaluate WITHOUT raising? Conservative:
    /// only literal/variable returns qualify (`return 1`, `return x` — no language
    /// raises on reading a bound local), plus blocks/ifs composed of them. Any
    /// operation (`x+1` TypeErrors in Python, indexing can raise anywhere) keeps
    /// the try handler alive in the fingerprint.
    pub(in crate::value_graph) fn branch_returns_throw_free(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Return => self
                .il
                .children(node)
                .first()
                .is_none_or(|&e| matches!(self.il.kind(e), NodeKind::Lit | NodeKind::Var)),
            NodeKind::Block => {
                let kids = self.il.children(node);
                kids.len() == 1 && self.branch_returns_throw_free(kids[0])
            }
            NodeKind::If => {
                let k = self.il.children(node);
                k.len() >= 3
                    && matches!(self.il.kind(k[0]), NodeKind::Lit | NodeKind::Var)
                    && self.branch_returns_throw_free(k[1])
                    && self.branch_returns_throw_free(k[2])
            }
            _ => false,
        }
    }

    pub(in crate::value_graph) fn is_effect_free_throw_body(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Throw => true,
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.il.kind(expr) == NodeKind::Throw),
            NodeKind::Block => {
                let Some((&last, prefix)) = self.il.children(node).split_last() else {
                    return false;
                };
                self.is_effect_free_throw_body(last)
                    && prefix
                        .iter()
                        .all(|&stmt| self.is_effect_free_throw_prefix(stmt))
            }
            _ => false,
        }
    }

    pub(in crate::value_graph) fn is_effect_free_throw_prefix(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_none_or(|&expr| crate::is_pure(self.il, expr)),
            NodeKind::Block => self
                .il
                .children(node)
                .iter()
                .all(|&stmt| self.is_effect_free_throw_prefix(stmt)),
            NodeKind::Seq => self.il.children(node).is_empty(),
            _ => false,
        }
    }
}
