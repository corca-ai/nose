use super::super::*;

impl<'a> Builder<'a> {
    pub(super) fn bin_idempotence_and_factor(
        &mut self,
        op: &ValOp,
        args: &[ValueId],
    ) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        if args.len() == 2 && args[0] == args[1] {
            let bitwise = o == Op::BitAnd as u32 || o == Op::BitOr as u32;
            let logical = o == Op::And as u32 || o == Op::Or as u32;
            if (bitwise
                && self.value_law_satisfied(ValueLaw::NumericBitwiseIdempotence, &[args[0]])
                && self.proven_numeric(args[0]))
                || (logical && self.value_law_satisfied(ValueLaw::BooleanIdempotence, &[args[0]]))
            {
                return Some(args[0]);
            }
        }
        // NOTE: arithmetic identity elimination (`x+0→x`, `x*1→x`) is deliberately NOT
        // done — it is unsound for non-numeric `x` (`"a"+0` Errs; `self*1` on a
        // non-number need not equal `self`), and value-domain inference is optimistic (it infers
        // `x:Num` from `x*1` itself), so a Num gate would still merge `return self*1`
        // with an identity `return self`. The oracle's all-types battery sees the
        // difference. `x*1`/`x+0` keep their identity operand (algebra no longer drops
        // it) and stay distinct from a bare `x` — a tiny convergence cost for soundness.

        // DISTRIBUTION / FACTORING: `x*f + y*f → (x+y)*f`. Canonicalizes toward the
        // factored form so `a*c + b*c` converges with `(a+b)*c`. Sound ONLY on numbers —
        // string `*int` is repetition, where `"a"*2 + "b"*2` ("aabb") ≠ `("a"+"b")*2`
        // ("abab") — so every leaf must be PROVEN `Num`. Lean obligation:
        // `normalize.value_graph.factor_distribute`.
        if o == Op::Add as u32 && args.len() == 2 {
            if let Some(v) = rules::factor_distribute::apply(self, args[0], args[1]) {
                return Some(v);
            }
        }
        // DOUBLING (`x*k → x+…+x`, so `x*2` ≡ `x+x`) was TRIED and REJECTED: expansion is
        // sound only on numbers, so it must gate on a PROVEN `Num`; but then the canonical
        // form of `(a+b)*2` depends on whether the surrounding code happens to prove the
        // operands numeric, splitting two behaviorally-identical functions (`a+=b; a*=2`
        // diverged from `(a+b)*2`). It closed `x*2 vs x+x` but opened `compound assign` —
        // net-zero, plus fragility. The gap stays open; see experiments §BA. (`x+x` in
        // isolation cannot be proven `Num`, so the sound contraction direction never fires.)
        None
    }
    // `and`/`or` are TYPE-GATED on commutativity, exactly like `+` is gated on concat:
    //   • both operands PROVEN Bool → boolean-and/or, which IS commutative
    //     (`X && Y` = `Y && X` for booleans) — sort operands so `p∧q` converges with
    //     `q∧p` (e.g. `(a>b)∧(b>0)` vs `(0<b)∧(b<a)` after comparison-direction canon).
    //   • otherwise → short-circuit VALUE-and/or, which is NOT commutative and yields
    //     the deciding operand's VALUE: `a or b ≡ a if a else b`, `a and b ≡ b if a
    //     else a`. Canonicalize to the positional `Phi` the ternary builds, so a guard
    //     written `a or b` converges with its `a if a else b` twin — without ever
    //     merging `a or b` with `b or a` (the value-or false merge the oracle now sees).
    // (Idempotent `x∧x`/`x∨x` on Bool, handled just above, returns before this.)
    pub(super) fn bool_and_or_canon(
        &mut self,
        op: &ValOp,
        args: &mut [ValueId],
    ) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        let is_or = o == Op::Or as u32;
        let is_and = o == Op::And as u32;
        if (is_or || is_and) && args.len() == 2 {
            if self.value_law_satisfied(ValueLaw::BooleanCommutativity, args) {
                if is_or {
                    if let Some(v) = self.literal_equality_disjunction(args[0], args[1]) {
                        return Some(v);
                    }
                } else if let Some(v) = self.literal_inequality_conjunction(args[0], args[1]) {
                    return Some(v);
                }
                // LATTICE CANON on a total order — close the strict comparison from a
                // non-strict one plus an (in)equality, so a guard written as the
                // conjunction/disjunction converges with the strict comparison:
                //   (x ≤ y) ∧ (x ≠ y) → x < y     (dual of below)
                //   (x < y) ∨ (x = y) → x ≤ y
                // Sound for any total order (`normalize.value_graph.compare`); on a type
                // error every comparison Errs identically on both sides. It composes through
                // the recursive `mk` fixpoint, so `not (a>b or a==b)` reaches `a<b`.
                if is_and {
                    if let Some(v) = self.lattice_strict_absorbs_nonstrict(args[0], args[1]) {
                        return Some(v);
                    }
                    if let Some(v) = self.lattice_le_ne_to_lt(args[0], args[1]) {
                        return Some(v);
                    }
                } else if let Some(v) = self.lattice_lt_eq_to_le(args[0], args[1]) {
                    return Some(v);
                }
                if self.vhash[args[0] as usize] > self.vhash[args[1] as usize] {
                    args.swap(0, 1);
                }
            } else if is_or {
                return Some(self.mk(ValOp::Phi, vec![args[0], args[0], args[1]]));
            } else {
                return Some(self.mk(ValOp::Phi, vec![args[0], args[1], args[0]]));
            }
        }
        None
    }
    // Boolean logical `and`/`or` is associative and commutative only when both sides are
    // proven Bool. Flattening that narrow shape lets `guard && (p && q)` converge with
    // `(guard && p) && q` without reviving value-short-circuit false merges for unknowns.
    pub(super) fn bool_chain_flatten(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        if args.len() == 2 && (o == Op::And as u32 || o == Op::Or as u32) {
            let mut leaves = Vec::new();
            self.flatten_into(args[0], o, &mut leaves);
            self.flatten_into(args[1], o, &mut leaves);
            if leaves.len() > 2 && self.value_law_satisfied(ValueLaw::BooleanAssociativity, &leaves)
            {
                leaves.sort_unstable_by_key(|&v| self.vhash[v as usize]);
                return Some(self.intern_ac_chain(o, &leaves));
            }
        }
        None
    }
    // Full ASSOCIATIVE-COMMUTATIVE canonicalization: flatten a `+`/`*`/`&`/`|`/`^`
    // chain to its leaves, sort them by structural hash, and rebuild one canonical
    // left-leaning chain — so `(a+b)+c`, `a+(b+c)`, and a factored `(a+b)+d` (from
    // `factor_distribute`) all reach ONE node regardless of how they were grouped or
    // built. The value graph thus canonicalizes AC chains itself, not only via the
    // earlier `algebra` IL pass (which keyed by a different hash and did not see nodes
    // synthesized here). Sound: any operand permutation of an AC chain is denotation-
    // preserving (`normalize.value_graph.algebra`). String/list `+` is NOT reordered
    // (it is ordered concat); `* & | ^` Err on non-numeric regardless of order.
    /// A `Reduce(Add)` (a `sum`) forces its elements numeric — `sum` of non-numbers Errs
    /// in EVERY order — so the per-element contribution's top-level `+` is numeric here and
    /// may be commuted even when its operands are not context-free provable as non-concat:
    /// `sum(x+y for …)` equals `sum(y+x for …)` (numeric-equal, or both Err). Sorting the
    /// contribution's `+` operands at the point the reduction is built recovers the
    /// generator ≡ loop ≡ `reduce` cross-form convergence (a core Type-4 case) that the
    /// context-free `+` gate (#283-C) would otherwise split. Sound by the Reduce(Add)
    /// numeric context, not a context-free claim — so it lives here, not in `proven_non_concat`.
    pub(super) fn commute_numeric_reduce_contrib(&mut self, op: &ValOp, args: &mut [ValueId]) {
        let ValOp::Reduce(o) = *op else { return };
        if o != Op::Add as u32 || args.is_empty() {
            return;
        }
        // The element contribution is the last arg (`[init, contrib]` or `[contrib]`); the
        // optional seed in slot 0 is left alone.
        let idx = args.len() - 1;
        args[idx] = self.commute_add_in_numeric_context(args[idx]);
    }
    /// Sort the operands of a top-level `+` known numeric by its enclosing `Reduce(Add)`,
    /// recursing through the `Phi(cond, contrib, identity)` a FILTERED reduction builds
    /// (`sum(x+y for … if g)`) so the guarded `x+y` is reached too. The guard condition
    /// (slot 0) is untouched.
    fn commute_add_in_numeric_context(&mut self, v: ValueId) -> ValueId {
        match self.nodes[v as usize].op {
            ValOp::Bin(b) if b == Op::Add as u32 => {
                let mut leaves = Vec::new();
                self.flatten_into(v, b, &mut leaves);
                if leaves.len() >= 2 && leaves.iter().all(|&l| self.reorder_safe(l)) {
                    leaves.sort_unstable_by_key(|&l| self.vhash[l as usize]);
                    self.intern_ac_chain(b, &leaves)
                } else {
                    v
                }
            }
            ValOp::Phi => {
                let a = self.nodes[v as usize].args.clone();
                if a.len() == 3 {
                    let then_branch = self.commute_add_in_numeric_context(a[1]);
                    let else_branch = self.commute_add_in_numeric_context(a[2]);
                    if then_branch != a[1] || else_branch != a[2] {
                        return self.mk(ValOp::Phi, vec![a[0], then_branch, else_branch]);
                    }
                }
                v
            }
            _ => v,
        }
    }
    pub(super) fn ac_chain_canon(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        if is_assoc_comm_code(o) && args.len() == 2 {
            let mut leaves = Vec::new();
            for &a in args {
                self.flatten_into(a, o, &mut leaves);
            }
            if leaves.len() > 2 && leaves.iter().all(|&v| self.reorder_safe(v)) {
                // ASSOCIATIVITY — re-grouping a flat chain into one canonical left-leaning
                // shape — is sound for Python/Ruby string/list concat, but not for JS/TS/Java
                // mixed string coercion (`"a"+2+3` != `"a"+(2+3)`). Gate `+` association
                // in those languages on proof that every leaf is non-concat.
                if o == Op::Add as u32 && !self.add_association_safe(&leaves) {
                    return None;
                }
                // Float `+`/`*` is non-associative (`(a+b)+c != a+(b+c)`, #283 C-float): a
                // chain with a proven-float leaf keeps its source grouping (the value the
                // eval-time gate already built), so don't flatten it here either.
                if (o == Op::Add as u32 || o == Op::Mul as u32)
                    && leaves.iter().any(|&v| self.possibly_float(v))
                {
                    return None;
                }
                // COMMUTATIVITY — sorting the operands — is gated per op (`ac_chain_commutes`):
                // a concat-possible `+` (#283-C), mixed-coercion `+`, and a Ruby string/list
                // `*` (series 9) stay ordered, while numeric/typed sums and products still
                // fully canonicalize.
                let commute = self.ac_chain_commutes(o, &leaves, ValueLaw::AddCommutativity);
                if commute {
                    leaves.sort_unstable_by_key(|&v| self.vhash[v as usize]);
                }
                return Some(self.intern_ac_chain(o, &leaves));
            }
        }
        None
    }
    pub(in crate::value_graph) fn intern_ac_chain(
        &mut self,
        opc: u32,
        operands: &[ValueId],
    ) -> ValueId {
        debug_assert!(!operands.is_empty());
        let mut acc = operands[0];
        for &operand in &operands[1..] {
            acc = self.intern_node(ValOp::Bin(opc), vec![acc, operand]);
        }
        acc
    }
    pub(in crate::value_graph) fn compact_formula(
        &mut self,
        opc: u32,
        operands: &[ValueId],
    ) -> ValueId {
        let mut h = combine(0xF0A5_7A11, opc as u64);
        h = combine(h, operands.len() as u64);
        for &operand in operands {
            h = combine(h, self.vhash[operand as usize]);
        }
        self.mk(ValOp::Formula(h), vec![])
    }
    pub(in crate::value_graph) fn compact_add_sub_formula(
        &mut self,
        operands: Vec<SignedExprOperand>,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let mut values = Vec::new();
        for operand in operands {
            let mut value = self.eval(operand.expr, env);
            if operand.negated {
                value = self.mk(ValOp::Un(Op::Neg as u32), vec![value]);
            }
            self.flatten_into(value, Op::Add as u32, &mut values);
        }
        if self.add_values_not_concat(ValueLaw::AddAssociativity, &values) {
            values.sort_by_key(|&v| self.vhash[v as usize]);
        }
        self.compact_formula(Op::Add as u32, &values)
    }
    /// Flatten an associative-commutative chain of value nodes into `out`.
    pub(in crate::value_graph) fn flatten_into(
        &mut self,
        vid: ValueId,
        opc: u32,
        out: &mut Vec<ValueId>,
    ) {
        let mut stack = vec![vid];
        while let Some(value) = stack.pop() {
            if let ValOp::Bin(o) = self.nodes[value as usize].op {
                if o == opc {
                    for &arg in self.nodes[value as usize].args.iter().rev() {
                        stack.push(arg);
                    }
                    continue;
                }
            }
            out.push(value);
        }
    }
    pub(in crate::value_graph) fn collect_add_sub_expr_operands(
        &self,
        expr: NodeId,
        negated: bool,
        out: &mut Vec<SignedExprOperand>,
    ) {
        if self.il.kind(expr) != NodeKind::BinOp {
            out.push(SignedExprOperand { expr, negated });
            return;
        }
        match op_code(self.il.node(expr).payload) {
            op if op == Op::Add as u32 => {
                for &child in self.il.children(expr) {
                    self.collect_add_sub_expr_operands(child, negated, out);
                }
            }
            op if op == Op::Sub as u32 && self.il.children(expr).len() == 2 => {
                let kids = self.il.children(expr);
                self.collect_add_sub_expr_operands(kids[0], negated, out);
                self.collect_add_sub_expr_operands(kids[1], !negated, out);
            }
            _ => out.push(SignedExprOperand { expr, negated }),
        }
    }
    pub(in crate::value_graph) fn collect_ac_expr_operands(
        &self,
        expr: NodeId,
        opc: u32,
        out: &mut Vec<NodeId>,
    ) {
        if self.il.kind(expr) == NodeKind::BinOp && op_code(self.il.node(expr).payload) == opc {
            for &child in self.il.children(expr) {
                self.collect_ac_expr_operands(child, opc, out);
            }
        } else {
            out.push(expr);
        }
    }
}
