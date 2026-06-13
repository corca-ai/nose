//! Value interning and proof-backed value canonicalization.
//!
//! proof-obligation: normalize.value_graph.algebra
//! proof-obligation: normalize.value_graph.clamp
//! proof-obligation: normalize.value_graph.compare
//! proof-obligation: normalize.value_graph.min_max

use super::*;

impl<'a> Builder<'a> {
    pub(super) fn mk(&mut self, mut op: ValOp, mut args: Vec<ValueId>) -> ValueId {
        self.commute_numeric_reduce_contrib(&op, &mut args);
        self.order_bin_operands(&mut op, &mut args);
        if let Some(v) = self.u16_byte_pack(&op, &args) {
            return v;
        }
        // Type-gated simplifications — now SOUND because the operand type is PROVEN (these
        // were the 17 false merges when applied untyped; they only hold on numbers/bools):
        //   -(-x) → x        when x : Num   (−(−x) = x; on a list it would Err ≠ x)
        //   x & x, x | x → x when x : Num   (idempotent integer bitwise)
        //   x && x, x || x → x when x : Bool (idempotent boolean)
        if let Some(v) = self.unary_canon(&op, &args) {
            return v;
        }
        if let Some(v) = self.bin_idempotence_and_factor(&op, &args) {
            return v;
        }
        if let Some(v) = self.bool_and_or_canon(&op, &mut args) {
            return v;
        }
        if let Some(v) = self.phi_select_idioms(&op, &args) {
            return v;
        }
        if let Some(v) = self.bool_chain_flatten(&op, &args) {
            return v;
        }
        if let Some(v) = self.ac_chain_canon(&op, &args) {
            return v;
        }
        let id = self.intern_node(op, args);
        rules::clamp::apply(self, id).unwrap_or(id)
    }

    fn order_bin_operands(&mut self, op: &mut ValOp, args: &mut [ValueId]) {
        let ValOp::Bin(opc) = *op else { return };
        // Canonicalize comparison DIRECTION: `a > b` ≡ `b < a`, `a >= b` ≡ `b <= a`.
        // Reduce the >/>= family to </<= with swapped operands so a guard converges
        // however it was written or negated (`0 < v`, `v > 0`, `!(v <= 0)` all become
        // one node). Language-agnostic and sound (total order). This is what lets a
        // `reduce(λa,v: a+v if v>0 else a, …)` fold converge with its loop, whose
        // guard may lower to the mirror comparison.
        if args.len() == 2 && self.comparison_law_enabled(ComparisonLaw::DirectionCanon) {
            if opc == Op::Gt as u32 {
                *op = ValOp::Bin(Op::Lt as u32);
                args.swap(0, 1);
            } else if opc == Op::Ge as u32 {
                *op = ValOp::Bin(Op::Le as u32);
                args.swap(0, 1);
            }
        }
        // Canonicalize commutative operands by structural hash. The commute is gated per op
        // (`ac_chain_commutes`): `+` is held ordered when an operand is PROVEN string/list
        // (concat, `s + x` ≠ `x + s`, #283-C) and `*` when it could be a Ruby string/list
        // repetition (`"ab" * 3` ≠ `3 * "ab"`, series 9) — both distinguished by the
        // free-monoid oracle. Unknown operands keep commuting (optimistic, oracle-checked),
        // so the common untyped numeric case is unaffected; `& | ^`, `==`/`!=`, min/max all
        // Err-or-symmetric regardless of order and stay safe.
        if let ValOp::Bin(o) = *op {
            if is_commutative(o)
                && args.len() == 2
                && self.ac_chain_commutes(o, args, ValueLaw::AddCommutativity)
                && self.reorder_safe(args[0])
                && self.reorder_safe(args[1])
                && self.vhash[args[0] as usize] > self.vhash[args[1] as usize]
            {
                args.swap(0, 1);
            }
        }
    }

    /// Effect-free ⇒ safe to reorder past a sibling operand. A subtree with a
    /// call/HOF/lambda/opaque node can carry an observable effect whose order the
    /// interpreter tracks, so it is held in place (coevo §CE / §AS).
    pub(super) fn reorder_safe(&self, v: ValueId) -> bool {
        let mut seen = FxHashSet::default();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            match self.nodes[n as usize].op {
                ValOp::Call(_)
                | ValOp::Hof(_)
                | ValOp::Lambda(_)
                | ValOp::Loop(_)
                | ValOp::Recurrence(_)
                | ValOp::Opaque(_) => return false,
                _ => {}
            }
            stack.extend(self.nodes[n as usize].args.iter().copied());
        }
        true
    }

    fn u16_byte_pack(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        if args.len() == 2 && (o == Op::Add as u32 || o == Op::BitOr as u32) {
            if let Some(v) = self.c_u16_be_byte_pack_pattern(args[0], args[1]) {
                return Some(v);
            }
        }
        None
    }

    fn unary_canon(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
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
        // BITWISE DE MORGAN: `~(a & b) → ~a | ~b`, `~(a | b) → ~a & ~b` (#284).
        // Two's-complement identity for all integers; on a non-integer operand
        // `~`/`&`/`|` Err on both the original and the distributed form, so it is
        // sound for ALL inputs. Pushing `~` inward gives a canonical form that
        // converges `~(a&b)` with an explicit `~a | ~b`. (The LOGICAL De Morgan,
        // over `Not`/`And`/`Or`, is handled by the `algebra` IL pass; this is its
        // bitwise twin, which that pass never reaches.)
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

    fn boolean_double_negation(&self, value: ValueId) -> Option<ValueId> {
        let ValOp::Un(op) = self.nodes[value as usize].op else {
            return None;
        };
        let inner = self.nodes[value as usize].args[0];
        (op == Op::Not as u32 && self.vty(inner) == ValueDomain::Boolean).then_some(inner)
    }

    fn negated_comparison(&mut self, comparison: ValueId) -> Option<ValueId> {
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

    fn bin_idempotence_and_factor(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
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
    fn bool_and_or_canon(&mut self, op: &ValOp, args: &mut [ValueId]) -> Option<ValueId> {
        let ValOp::Bin(o) = *op else { return None };
        let is_or = o == Op::Or as u32;
        let is_and = o == Op::And as u32;
        if (is_or || is_and) && args.len() == 2 {
            if self.value_law_satisfied(ValueLaw::BooleanCommutativity, args) {
                if is_or {
                    if let Some(v) = self.literal_equality_disjunction(args[0], args[1]) {
                        return Some(v);
                    }
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

    // Recognize select idioms on EVERY branch merge — `Phi(cond, then, els)` is built
    // both by a ternary (`a if c else b`) and by an if/else that assigns a variable, so
    // doing this here (not just at the ternary) keeps the two forms convergent:
    //   integer-proven `x if x>=0 else -x` → Abs(x);
    //   `x if x<y else y` → Min(x,y) / Max(x,y).
    fn phi_select_idioms(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
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

    // Boolean logical `and`/`or` is associative and commutative only when both sides are
    // proven Bool. Flattening that narrow shape lets `guard && (p && q)` converge with
    // `(guard && p) && q` without reviving value-short-circuit false merges for unknowns.
    fn bool_chain_flatten(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
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
    fn commute_numeric_reduce_contrib(&mut self, op: &ValOp, args: &mut [ValueId]) {
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

    fn ac_chain_canon(&mut self, op: &ValOp, args: &[ValueId]) -> Option<ValueId> {
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

    /// Intern a value node by `(op, args)` (hash-consing), computing its structural hash
    /// and kernel value domain. The raw constructor used by `mk` after canonicalization
    /// does not itself canonicalize, so callers must pass already-canonical operands.
    pub(super) fn intern_node(&mut self, op: ValOp, args: Vec<ValueId>) -> ValueId {
        let key = (op.clone(), args.clone());
        if let Some(&id) = self.intern.get(&key) {
            return id;
        }
        let id = self.nodes.len() as ValueId;
        let mut h = op_tag(&op);
        for &a in &args {
            h = combine(h, self.vhash[a as usize]);
        }
        let ty = self.value_domain_of(&op, &args);
        self.nodes.push(ValNode { op, args });
        self.vhash.push(h);
        self.vty.push(ty);
        self.node_span.push(self.cur_span);
        self.intern.insert(key, id);
        id
    }

    pub(super) fn boolean_guarded_identity_phi(
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

    pub(super) fn flatten_nested_guarded_identity_phi(
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

    pub(super) fn intern_ac_chain(&mut self, opc: u32, operands: &[ValueId]) -> ValueId {
        debug_assert!(!operands.is_empty());
        let mut acc = operands[0];
        for &operand in &operands[1..] {
            acc = self.intern_node(ValOp::Bin(opc), vec![acc, operand]);
        }
        acc
    }

    pub(super) fn compact_formula(&mut self, opc: u32, operands: &[ValueId]) -> ValueId {
        let mut h = combine(0xF0A5_7A11, opc as u64);
        h = combine(h, operands.len() as u64);
        for &operand in operands {
            h = combine(h, self.vhash[operand as usize]);
        }
        self.mk(ValOp::Formula(h), vec![])
    }

    pub(super) fn compact_add_sub_formula(
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
    pub(super) fn flatten_into(&mut self, vid: ValueId, opc: u32, out: &mut Vec<ValueId>) {
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

    pub(super) fn collect_add_sub_expr_operands(
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

    pub(super) fn collect_ac_expr_operands(&self, expr: NodeId, opc: u32, out: &mut Vec<NodeId>) {
        if self.il.kind(expr) == NodeKind::BinOp && op_code(self.il.node(expr).payload) == opc {
            for &child in self.il.children(expr) {
                self.collect_ac_expr_operands(child, opc, out);
            }
        } else {
            out.push(expr);
        }
    }

    pub(super) fn fresh_opaque(&mut self) -> ValueId {
        let c = self.opaque_ctr;
        self.opaque_ctr += 1;
        self.mk(ValOp::Opaque(c as u64), vec![])
    }

    /// The operands of a comparison node `cmp`, if it has opcode `want`. `Le`/`Lt` are
    /// ORDERED (operands kept in source order); `Eq`/`Ne` are COMMUTATIVE (operands
    /// vhash-sorted), so callers compare them as an unordered pair.
    pub(super) fn cmp_operands(&self, v: ValueId, want: u32) -> Option<(ValueId, ValueId)> {
        if let ValOp::Bin(o) = self.nodes[v as usize].op {
            if o == want && self.nodes[v as usize].args.len() == 2 {
                let a = &self.nodes[v as usize].args;
                return Some((a[0], a[1]));
            }
        }
        None
    }

    /// Lattice canon combining an ORDERED comparison (`ordered`, which fixes the operand
    /// pair `(x, y)` in source order) with a COMMUTATIVE (in)equality (`comm`, whose
    /// operands match `{x, y}` in either order) into a single `result` comparison. The two
    /// arguments may arrive in either slot (the conjunction/disjunction is itself sorted),
    /// so both assignments are tried. Sound on a total order; each instantiation cites its
    /// own Lean lemma at the call site.
    pub(super) fn lattice_pair_canon(
        &mut self,
        a: ValueId,
        b: ValueId,
        ordered: u32,
        comm: u32,
        result: u32,
    ) -> Option<ValueId> {
        for (ord_v, comm_v) in [(a, b), (b, a)] {
            if let Some((x, y)) = self.cmp_operands(ord_v, ordered) {
                if let Some((c0, c1)) = self.cmp_operands(comm_v, comm) {
                    if (c0 == x && c1 == y) || (c0 == y && c1 == x) {
                        return Some(self.mk(ValOp::Bin(result), vec![x, y]));
                    }
                }
            }
        }
        None
    }

    /// `(x ≤ y) ∧ (x ≠ y) → x < y`. Sound on a total order
    /// (`normalize.value_graph.compare`); the post-normalize oracle re-checks it.
    pub(super) fn lattice_le_ne_to_lt(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::LatticeLeNeToLt) {
            return None;
        }
        self.lattice_pair_canon(a, b, Op::Le as u32, Op::Ne as u32, Op::Lt as u32)
    }

    pub(super) fn comparison_law_enabled(&self, law: ComparisonLaw) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .comparison_law(law)
            .is_some()
    }

    /// `(x < y) ∧ (x ≤ y) → x < y`. Guard-clause lowering accumulates path conditions
    /// from earlier returns, so a comparator written as `if x<y return -1; if x>y return
    /// 1; return 0` otherwise leaves the second return guarded by `x≤y ∧ x<y` after
    /// comparison-direction canon. The non-strict half is implied by the strict half and
    /// can be absorbed only for source languages whose comparison operators are primitive
    /// rather than receiver-overloadable.
    pub(super) fn lattice_strict_absorbs_nonstrict(
        &mut self,
        a: ValueId,
        b: ValueId,
    ) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::LatticeStrictAbsorbsNonstrict) {
            return None;
        }
        for (lt_v, le_v) in [(a, b), (b, a)] {
            if let Some((x, y)) = self.cmp_operands(lt_v, Op::Lt as u32) {
                if self.cmp_operands(le_v, Op::Le as u32) == Some((x, y)) {
                    return Some(lt_v);
                }
            }
        }
        None
    }

    /// `(x < y) ∨ (x = y) → x ≤ y` — the dual of [`lattice_le_ne_to_lt`] over `∨`
    /// (`normalize.value_graph.compare`).
    pub(super) fn lattice_lt_eq_to_le(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::LatticeLtEqToLe) {
            return None;
        }
        self.lattice_pair_canon(a, b, Op::Lt as u32, Op::Eq as u32, Op::Le as u32)
    }

    /// Recognize integer absolute-value idioms `x if x>=0 else -x` (and mirrors)
    /// as `Un(ABS_CODE, [x])`, so they converge with integer-proven `abs(x)`.
    pub(super) fn abs_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
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
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if cn.args.len() != 2 {
            return None;
        }
        let is_zero = |s: &Self, id: ValueId| {
            matches!(
                s.nodes[id as usize].op,
                ValOp::Const {
                    kind: ConstKind::Int,
                    bits: 0
                }
            )
        };
        let (nonneg, neg) = if cn.args[0] == v && is_zero(self, cn.args[1]) {
            (
                opc == Op::Ge as u32 || opc == Op::Gt as u32,
                opc == Op::Lt as u32 || opc == Op::Le as u32,
            )
        } else if cn.args[1] == v && is_zero(self, cn.args[0]) {
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
    pub(super) fn minmax_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::MinMaxTernary) {
            return None;
        }
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) || cn.args.len() != 2 {
            return None;
        }
        let (x, y) = (cn.args[0], cn.args[1]); // cond is `x < y`
                                               // `x if x<y else y` → min(x,y);  `y if x<y else x` → max(x,y).
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
    pub(super) fn clamp_ternary_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) || cn.args.len() != 2 {
            return None;
        }
        let (left, right) = (cn.args[0], cn.args[1]);

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
    pub(super) fn low_bit_toggle_pattern(
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

    pub(super) fn has_java_primitive_integer_ops(&self) -> bool {
        semantics(self.il.meta.lang)
            .stdlib()
            .java_primitive_integer_ops()
    }

    pub(super) fn parity_zero_condition(&self, cond: ValueId) -> Option<(ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        let even_when_true = match node.op {
            ValOp::Bin(o) if o == Op::Eq as u32 => true,
            ValOp::Bin(o) if o == Op::Ne as u32 => false,
            _ => return None,
        };
        if node.args.len() != 2 {
            return None;
        }
        for (candidate, zero) in [(node.args[0], node.args[1]), (node.args[1], node.args[0])] {
            if self.int_const_eq(zero, 0) {
                if let Some(base) = self.mod_by_two_base(candidate) {
                    return Some((base, even_when_true));
                }
            }
        }
        None
    }

    pub(super) fn mod_by_two_base(&self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Mod as u32) || node.args.len() != 2 {
            return None;
        }
        if self.int_const_eq(node.args[1], 2) {
            Some(node.args[0])
        } else {
            None
        }
    }

    pub(super) fn additive_one_delta(&mut self, value: ValueId, base: ValueId) -> Option<i8> {
        if !matches!(self.nodes[value as usize].op, ValOp::Bin(o) if o == Op::Add as u32) {
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

    pub(super) fn signed_one_const(&self, value: ValueId) -> Option<i8> {
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

    pub(super) fn c_u16_be_byte_pack_pattern(
        &mut self,
        left: ValueId,
        right: ValueId,
    ) -> Option<ValueId> {
        let _contract = semantics(self.il.meta.lang)
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U16)?;
        for (shifted, low) in [(left, right), (right, left)] {
            // `else continue`, not `?`: the operands may sort either way by value-hash, so a
            // miss on the first ordering must fall through to the second, not abort the fn.
            let Some((base, high_index)) = self.shifted_byte_lane(shifted) else {
                continue;
            };
            let Some((low_base, low_index)) = self.byte_lane(low) else {
                continue;
            };
            if base == low_base
                && high_index == 0
                && low_index == 1
                && self.is_param_value(base, DomainEvidence::ByteArray)
            {
                let zero = self.int_const(0);
                let one = self.int_const(1);
                return Some(self.mk(ValOp::Call(C_U16_BE_BYTE_PACK_CODE), vec![base, zero, one]));
            }
        }
        None
    }

    pub(super) fn c_u32_be_byte_pack_pattern(&mut self, operands: &[ValueId]) -> Option<ValueId> {
        let contract = semantics(self.il.meta.lang)
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U32)?;
        if operands.len() != 4 {
            return None;
        }
        let mut base = None;
        let mut seen = [false; 4];
        for &operand in operands {
            let (lane_base, index, shift, unsigned_cast) = self.c_u32_byte_pack_lane(operand)?;
            if Some(lane_base) != base {
                if base.is_some() {
                    return None;
                }
                base = Some(lane_base);
            }
            let expected_shift = (3u8.checked_sub(index)? as i64) * 8;
            if shift != expected_shift {
                return None;
            }
            if index == 0 {
                match contract.required_high_lane_cast {
                    Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32)) if unsigned_cast => {}
                    Some(_) => return None,
                    None => {}
                }
            }
            if seen[index as usize] {
                return None;
            }
            seen[index as usize] = true;
        }
        if !seen.iter().all(|seen| *seen) {
            return None;
        }
        let base = base?;
        if !self.is_param_value(base, DomainEvidence::ByteArray) {
            return None;
        }
        let zero = self.int_const(0);
        let one = self.int_const(1);
        let two = self.int_const(2);
        let three = self.int_const(3);
        Some(self.mk(
            ValOp::Call(C_U32_BE_BYTE_PACK_CODE),
            vec![base, zero, one, two, three],
        ))
    }

    pub(super) fn c_u32_byte_pack_lane(&self, value: ValueId) -> Option<(ValueId, u8, i64, bool)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Bin(o) if o == Op::Shl as u32) && node.args.len() == 2 {
            let shift = self.int_const_value(node.args[1])?;
            let (base, index, unsigned_cast) = self.byte_lane_with_unsigned_cast(node.args[0])?;
            return Some((base, index, shift, unsigned_cast));
        }
        let (base, index, unsigned_cast) = self.byte_lane_with_unsigned_cast(value)?;
        Some((base, index, 0, unsigned_cast))
    }

    pub(super) fn shifted_byte_lane(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Shl as u32) || node.args.len() != 2 {
            return None;
        }
        if !self.int_const_eq(node.args[1], 8) {
            return None;
        }
        self.byte_lane(node.args[0])
    }

    pub(super) fn byte_lane(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let (base, index, _) = self.byte_lane_with_unsigned_cast(value)?;
        if index <= 1 {
            Some((base, index))
        } else {
            None
        }
    }

    pub(super) fn byte_lane_with_unsigned_cast(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, u8, bool)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == Builtin::UnsignedCast32 as u32 + 1)
            && node.args.len() == 1
        {
            let (base, index, _) = self.byte_lane_with_unsigned_cast(node.args[0])?;
            return Some((base, index, true));
        }
        self.byte_lane_any_index(value)
            .map(|(base, index)| (base, index, false))
    }

    pub(super) fn byte_lane_any_index(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Index) || node.args.len() != 2 {
            return None;
        }
        if self.int_const_eq(node.args[1], 0) {
            Some((node.args[0], 0))
        } else if self.int_const_eq(node.args[1], 1) {
            Some((node.args[0], 1))
        } else if self.int_const_eq(node.args[1], 2) {
            Some((node.args[0], 2))
        } else if self.int_const_eq(node.args[1], 3) {
            Some((node.args[0], 3))
        } else {
            None
        }
    }

    pub(super) fn int_const_eq(&self, value: ValueId, expected: i64) -> bool {
        self.int_const_value(value) == Some(expected)
    }

    pub(super) fn int_const_value(&self, value: ValueId) -> Option<i64> {
        match self.nodes[value as usize].op {
            ValOp::Const {
                kind: ConstKind::Int,
                bits,
            } => Some(bits as i64),
            _ => None,
        }
    }

    /// An integer-literal value, keyed identically to `eval`'s `LitInt` path so a
    /// builtin's implicit init (`sum` → 0) matches a loop's explicit `acc = 0`. `v` is a
    /// synthesized non-negative count; the FULL i64 is retained so it can never collide
    /// with a literal differing by a multiple of 2^32 (coevo series 8).
    pub(super) fn int_const(&mut self, v: u32) -> ValueId {
        self.mk_const(ConstKind::Int, v as u64)
    }

    pub(super) fn null_const(&mut self) -> ValueId {
        self.mk_const(ConstKind::Null, 0)
    }

    pub(super) fn bool_const_value(&mut self, b: bool) -> ValueId {
        self.mk_const(ConstKind::Bool, b as u64)
    }

    /// Construct a `Const` node of the given kind and full-width payload.
    pub(super) fn mk_const(&mut self, kind: ConstKind, bits: u64) -> ValueId {
        self.mk(ValOp::Const { kind, bits }, vec![])
    }

    pub(super) fn sentinel_const(&mut self, tag: u64) -> ValueId {
        self.mk_const(ConstKind::Sentinel, tag)
    }

    pub(super) fn bool_const(&self, id: ValueId) -> Option<bool> {
        match self.nodes[id as usize].op {
            ValOp::Const {
                kind: ConstKind::Bool,
                bits,
            } => Some(bits != 0),
            _ => None,
        }
    }

    pub(super) fn literal_equality_disjunction(
        &mut self,
        left: ValueId,
        right: ValueId,
    ) -> Option<ValueId> {
        let mut element = None;
        let mut items = Vec::new();
        self.collect_literal_membership_terms(left, &mut element, &mut items)?;
        self.collect_literal_membership_terms(right, &mut element, &mut items)?;
        if items.len() < 2 {
            return None;
        }
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        let collection = self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items);
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![element?, collection]))
    }

    pub(super) fn collect_literal_membership_terms(
        &self,
        value: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        let node = &self.nodes[value as usize];
        match node.op {
            ValOp::Bin(op) if op == Op::Or as u32 && node.args.len() == 2 => {
                self.collect_literal_membership_terms(node.args[0], element, items)?;
                self.collect_literal_membership_terms(node.args[1], element, items)
            }
            ValOp::Bin(op) if op == Op::Eq as u32 && node.args.len() == 2 => {
                let a = node.args[0];
                let b = node.args[1];
                let (candidate, literal) = if self.static_membership_literal_value(a) {
                    (b, a)
                } else if self.static_membership_literal_value(b) {
                    (a, b)
                } else {
                    return None;
                };
                self.record_literal_membership_term(candidate, literal, element, items)
            }
            ValOp::Bin(op) if op == Op::In as u32 && node.args.len() == 2 => {
                let candidate = node.args[0];
                let collection = &self.nodes[node.args[1] as usize];
                if !matches!(collection.op, ValOp::Seq(SEQ_VALUE_COLLECTION))
                    || !collection
                        .args
                        .iter()
                        .all(|&item| self.static_membership_literal_value(item))
                {
                    return None;
                }
                match *element {
                    Some(current) if current != candidate => None,
                    Some(_) => {
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                    None => {
                        *element = Some(candidate);
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                }
            }
            _ => None,
        }
    }

    pub(super) fn record_literal_membership_term(
        &self,
        candidate: ValueId,
        literal: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        match *element {
            Some(current) if current != candidate => None,
            Some(_) => {
                items.push(literal);
                Some(())
            }
            None => {
                *element = Some(candidate);
                items.push(literal);
                Some(())
            }
        }
    }

    pub(super) fn static_membership_literal_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const { kind, bits } if !(matches!(kind, ConstKind::Sentinel) && bits == sentinel::BOTTOM)
        )
    }

    pub(super) fn empty_string_value(&mut self) -> ValueId {
        self.mk_const(ConstKind::Str, stable_string_const_bits(""))
    }

    pub(super) fn is_empty_string_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const { kind: ConstKind::Str, bits } if bits == stable_string_const_bits("")
        )
    }

    /// Whether `target` appears anywhere in `v`'s value subgraph (DAG-safe).
    pub(super) fn references(&self, v: ValueId, target: ValueId) -> bool {
        let mut stack = vec![v];
        let mut seen = FxHashSet::default();
        while let Some(x) = stack.pop() {
            if x == target {
                return true;
            }
            if seen.insert(x) {
                stack.extend(self.nodes[x as usize].args.iter().copied());
            }
        }
        false
    }

    pub(super) fn references_cached(
        &self,
        v: ValueId,
        target: ValueId,
        cache: &mut ReductionCache,
    ) -> bool {
        let key = (v, target);
        if let Some(&cached) = cache.references.get(&key) {
            return cached;
        }
        let result = self.references(v, target);
        cache.references.insert(key, result);
        result
    }

    pub(super) fn references_any_cached(
        &self,
        v: ValueId,
        targets: &[ValueId],
        cache: &mut ReductionCache,
    ) -> bool {
        targets
            .iter()
            .copied()
            .any(|target| self.references_cached(v, target, cache))
    }
}
