use super::super::*;

const LARGE_AC_EXPR_OPERANDS: usize = 64;

impl<'a> Builder<'a> {
    /// JS equality with source-operator semantics. The value model conflates
    /// `null`/`undefined` into ONE constant, so the model's Eq-against-null means
    /// the LOOSE check (`x == null`, true for both) — which is also what `x ?? d`
    /// desugars to. A strict `x === null` (false for undefined) cannot be expressed
    /// in that model: keep it a distinct shape keyed by the spelled operand.
    ///
    /// Outside the nullish special case, JS loose equality uses coercions (`false == 0`,
    /// `"0" == 0`, `[] == 0`) and must not merge with strict equality.
    fn eval_js_equality_comparison(
        &mut self,
        expr: NodeId,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if (op != Op::Eq as u32 && op != Op::Ne as u32) || kids.len() != 2 {
            return None;
        }
        let source = source_operator_at_node(self.il, expr);
        if matches!(source, Some(nose_il::SourceOperatorKind::TypeMembership)) {
            let a = self.eval(kids[0], env);
            let b = self.eval(kids[1], env);
            let membership = self.mk(ValOp::Opaque(JS_INSTANCEOF_CMP_TAG), vec![a, b]);
            return Some(if op == Op::Ne as u32 {
                self.mk(ValOp::Un(Op::Not as u32), vec![membership])
            } else {
                membership
            });
        }
        let strict = matches!(
            source,
            Some(
                nose_il::SourceOperatorKind::StrictEquality
                    | nose_il::SourceOperatorKind::StrictInequality
            )
        );
        let loose = matches!(
            source,
            Some(
                nose_il::SourceOperatorKind::LooseEquality
                    | nose_il::SourceOperatorKind::LooseInequality
            )
        );
        if !strict && !loose {
            return None;
        }
        let a = self.eval(kids[0], env);
        let b = self.eval(kids[1], env);
        let null_kid = if self.is_null_value(a) {
            Some((kids[0], b))
        } else if self.is_null_value(b) {
            Some((kids[1], a))
        } else {
            None
        };
        if let Some((null_kid, value_side)) = null_kid {
            if !strict {
                return Some(self.mk(ValOp::Bin(op), vec![a, b]));
            }
            // Strict `=== null` / `=== undefined` is NOT the loose nullish check — keep it a
            // distinct opaque keyed by the spelled operand. Earlier this carved out
            // `=== undefined` over a map-get as a "faithful" absence `Eq` so it would fold to the
            // map default like `m.get(k) ?? d`. But the value model conflates `null`/`undefined`
            // into one constant, so that `Eq` was indistinguishable from `== null`/`?? ` and
            // false-merged the COALESCE form (default on absent OR present-null) with the
            // ABSENCE form (default on absent only) — they diverge on a present null-valued key
            // (#410, experiments §CT). Keeping it opaque splits the two; `=== undefined` no longer
            // claims equivalence with either `?? ` or the membership-guarded `GetOrDefault`.
            let salt = combine(JS_STRICT_NULL_CMP_TAG, self.valued_subtree_hash(null_kid));
            let eq = self.mk(ValOp::Opaque(salt), vec![value_side]);
            return Some(if op == Op::Ne as u32 {
                self.mk(ValOp::Un(Op::Not as u32), vec![eq])
            } else {
                eq
            });
        }
        if loose {
            let tag = if op == Op::Ne as u32 {
                JS_LOOSE_NE_CMP_TAG
            } else {
                JS_LOOSE_EQ_CMP_TAG
            };
            let mut args = vec![a, b];
            if self.reorder_safe(args[0])
                && self.reorder_safe(args[1])
                && self.vhash[args[0] as usize] > self.vhash[args[1] as usize]
            {
                args.swap(0, 1);
            }
            return Some(self.mk(ValOp::Opaque(tag), args));
        }
        // Strict comparison the model expresses faithfully: the model's Eq IS
        // strict. Operands are already evaluated — don't evaluate them twice.
        Some(self.mk(ValOp::Bin(op), vec![a, b]))
    }
    /// `a in b` — directional membership. JS `in` tests prototype-chain keys (its
    /// own shape); a language with no modeled membership operator stays opaque;
    /// otherwise membership over a proven map key-view or collection.
    fn eval_membership_binop(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let element = self.eval(kids[0], env);
        if self.is_js_like_lang() {
            let collection = self.eval(kids[1], env);
            return self.mk(ValOp::Call(JS_PROTOTYPE_IN_CODE), vec![element, collection]);
        }
        if semantics(self.il.meta.lang)
            .operators()
            .membership_operator(Op::In)
            .is_none()
        {
            let collection = self.eval(kids[1], env);
            let salt = self.source_salted_hash(expr, 0x494E_4F50);
            return self.mk(ValOp::Opaque(salt), vec![element, collection]);
        }
        if let Some(map) = self.proven_map_key_view_expr(kids[1], env) {
            return self.mk(ValOp::Bin(Op::In as u32), vec![element, map]);
        }
        let collection = self.eval_membership_collection(kids[1], env);
        self.mk(ValOp::Bin(Op::In as u32), vec![element, collection])
    }
    pub(super) fn eval_binop_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let op = op_code(payload);
        let kids = self.il.children(expr).to_vec();
        if op == Op::In as u32 && kids.len() == 2 {
            return self.eval_membership_binop(expr, &kids, env);
        }
        if let Some(v) = self.eval_rust_option_some_pattern_comparison(op, &kids, env) {
            return v;
        }
        if let Some(v) = self.eval_rust_result_wildcard_pattern_comparison(op, &kids, env) {
            return v;
        }
        if let Some(v) = self.eval_static_filter_membership_comparison(op, &kids, env) {
            return v;
        }
        if let Some(v) = self.eval_len_zero_comparison(op, &kids, env) {
            return v;
        }
        if let Payload::Op(op_kind) = payload {
            if let Some(v) = self.eval_static_index_membership_comparison(op_kind, &kids, env) {
                return v;
            }
        }
        if let Some(v) = self.eval_js_equality_comparison(expr, op, &kids, env) {
            return v;
        }
        if self.relational_has_string_ordering()
            && matches!(op_from_code(op), Some(Op::Lt | Op::Le | Op::Gt | Op::Ge))
            && kids.len() == 2
        {
            let args: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
            if args.iter().all(|&v| self.proven_numeric(v)) {
                return self.mk(ValOp::Bin(op), args);
            }
            return self.mk(
                ValOp::Opaque(combine(JS_RELATIONAL_CMP_TAG, op as u64)),
                args,
            );
        }
        if (op == Op::Add as u32 || op == Op::Sub as u32) && !self.plus_has_mixed_string_coercion()
        {
            let mut operands = Vec::new();
            self.collect_add_sub_expr_operands(expr, false, &mut operands);
            if operands.len() >= LARGE_AC_EXPR_OPERANDS {
                return self.compact_add_sub_formula(operands, env);
            }
        }
        // Canonicalize subtraction to addition-of-negation: `a - b ≡ a + (-b)`
        // (sound for the two's-complement Int model: a.wrapping_sub(b) ==
        // a.wrapping_add(-b)). Routing it through the AC `+` normalization unifies
        // `a - b`, `a + (-b)`, and `-b + a` to one fingerprint — converging the
        // many subtraction/negation algebraic variants (e.g. sympy `__sub__`
        // `self + (-a)` with a sibling `self - a`). `verify` is the soundness gate.
        if op == Op::Sub as u32 && kids.len() == 2 {
            return self.eval_sub_chain(&kids, env);
        }
        if is_assoc_comm_code(op) {
            self.eval_assoc_comm_chain(op, &kids, env)
        } else {
            let mut a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
            // JS `a << b` / `a >> b` shift `ToInt32(a)` (32-bit, sign-propagating), unlike
            // Python/Ruby's arbitrary-precision shifts — narrow the shifted operand so JS
            // shifts fingerprint distinctly and never false-merge cross-language. (`>>>` is
            // already kept distinct: js_bin_op leaves it unmapped.) #283-D, series 9.
            if (op == Op::Shl as u32 || op == Op::Shr as u32) && !a.is_empty() {
                a[0] = self.js_int32_narrow(a[0]);
            }
            self.mk(ValOp::Bin(op), a)
        }
    }
    fn eval_sub_chain(&mut self, kids: &[NodeId], env: &FxHashMap<u32, ValueId>) -> ValueId {
        let a = self.eval(kids[0], env);
        let b = self.eval(kids[1], env);
        // Routing `a - b` through the AC `+` chain (`a + (-b)`) reassociates it; that is
        // unsound for a string-coercion `+` (JS/TS/Java) and for float arithmetic
        // (`(x + a) - x != a` when x is a large float). Keep the literal `Sub` in those
        // cases so it is not flattened into a reassociated sum (#283 C-float).
        if (self.plus_has_mixed_string_coercion() && !self.proven_non_concat(a))
            || self.possibly_float(a)
            || self.possibly_float(b)
        {
            return self.mk(ValOp::Bin(Op::Sub as u32), vec![a, b]);
        }
        let neg_b = self.mk(ValOp::Un(Op::Neg as u32), vec![b]);
        let mut operands = Vec::new();
        self.flatten_into(a, Op::Add as u32, &mut operands);
        self.flatten_into(neg_b, Op::Add as u32, &mut operands);
        // Sort unless an operand is proven concat (string/list), OR carries an
        // observable effect — `f() - g()` reordered to `g() - f()` keeps the
        // value but swaps the effect trace, a false merge the interpreter
        // catches (coevo §CE / §AS). Effect-free numeric operands Err in the
        // oracle regardless of order, so sorting them is safe.
        if self.add_values_not_concat(ValueLaw::AddAssociativity, &operands)
            && operands.iter().all(|&v| self.reorder_safe(v))
        {
            operands.sort_by_key(|&v| self.vhash[v as usize]);
        }
        let mut acc = operands[0];
        for &o in &operands[1..] {
            acc = self.mk(ValOp::Bin(Op::Add as u32), vec![acc, o]);
        }
        acc
    }
    fn eval_assoc_comm_chain(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        // Flatten the chain (resolving temps), sort by structural hash, and
        // rebuild canonically — so groupings/temps converge. EXCEPT `+` is only
        // commutative on numeric operands; on strings/lists it is concat, which
        // is ordered, so we keep source order there (sorting would be unsound).
        //
        // Very large generated formulas can arrive as deeply nested binary ASTs.
        // For those, collect same-op source operands first so one giant expression
        // pays for flatten/sort/rebuild once instead of once per nested pair.
        if op == Op::Add as u32 && self.plus_has_mixed_string_coercion() {
            let direct: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
            let mut leaves = Vec::new();
            for &value in &direct {
                self.flatten_into(value, op, &mut leaves);
            }
            // Float `+` is NON-ASSOCIATIVE even in string-coercion langs (Java `double`,
            // TS `number`): a float-typed chain is provably non-concat, so the lines below
            // would happily flatten+sort it — but `(a+b)+c != a+(b+c)` in IEEE-754 (#283
            // C-float). Hold the SOURCE grouping by rebuilding from the original binop kids,
            // exactly as the general (non-coercion) path does. Each kid was recursively
            // eval'd, so any nested float subchain already preserved its own grouping.
            if leaves.iter().any(|&v| self.possibly_float(v)) {
                let mut acc = direct[0];
                for &v in &direct[1..] {
                    acc = self.mk(ValOp::Bin(op), vec![acc, v]);
                }
                return acc;
            }
            if !self.add_association_safe(&leaves) {
                return self.intern_ac_chain(op, &direct);
            }
            let mut operands = leaves;
            let do_sort = self.ac_chain_commutes(op, &operands, ValueLaw::AddAssociativity)
                && operands.iter().all(|&v| self.reorder_safe(v));
            if do_sort {
                operands.sort_by_key(|&v| self.vhash[v as usize]);
            }
            return self.intern_ac_chain(op, &operands);
        }

        let mut expr_operands = Vec::new();
        for &k in kids {
            self.collect_ac_expr_operands(k, op, &mut expr_operands);
        }
        if expr_operands.len() >= LARGE_AC_EXPR_OPERANDS {
            let mut operands = Vec::new();
            for k in expr_operands {
                let v = self.eval(k, env);
                self.flatten_into(v, op, &mut operands);
            }
            if let Some(v) = self.c_u32_be_byte_pack_pattern(&operands) {
                return v;
            }
            self.narrow_js_bitwise_leaves(op, &mut operands);
            let do_sort = self.ac_chain_commutes(op, &operands, ValueLaw::AddAssociativity)
                && operands.iter().all(|&v| self.reorder_safe(v));
            if do_sort {
                operands.sort_by_key(|&v| self.vhash[v as usize]);
            }
            return self.compact_formula(op, &operands);
        }

        let mut operands = Vec::new();
        for &k in kids {
            let v = self.eval(k, env);
            self.flatten_into(v, op, &mut operands);
        }
        if let Some(v) = self.c_u32_be_byte_pack_pattern(&operands) {
            return v;
        }
        // Float `+`/`*` is NON-ASSOCIATIVE (`(a+b)+c != a+(b+c)`, #283 C-float). When the chain
        // bears a POSSIBLY-float operand — proven float, OR (in a dynamically-typed language) a
        // truly-untyped param that could be float at runtime (#342) — do NOT flatten/reassociate
        // it; rebuild the SOURCE grouping so the two groupings fingerprint distinctly
        // (`ac_chain_canon` is gated to not re-flatten such a chain either). Commutativity is
        // preserved (the rebuild below sorts non-concat operands), and `: int`-typed chains stay
        // associative — only their float-possible siblings are held.
        if (op == Op::Add as u32 || op == Op::Mul as u32)
            && operands.iter().any(|&v| self.possibly_float(v))
        {
            // Rebuild the SOURCE grouping (no reassociation), but still COMMUTE operands at each
            // binary node when the whole chain is commutable — float `+`/`*` is commutative, so
            // `(a+b)+1` and `(b+a)+1` must still converge; only the GROUPING is held. Commutability
            // is the chain-wide non-concat verdict (a literal/non-concat leaf licenses sorting the
            // whole chain), so it must be computed over the flattened leaves and pushed DOWN —
            // the per-node `order_bin_operands` gate alone would miss it for an inner pair (#342).
            let commutable = self.ac_chain_commutes(op, &operands, ValueLaw::AddAssociativity)
                && operands.iter().all(|&v| self.reorder_safe(v));
            return self.rebuild_grouped_float_chain(op, kids, env, commutable);
        }
        self.narrow_js_bitwise_leaves(op, &mut operands);
        let do_sort = self.ac_chain_commutes(op, &operands, ValueLaw::AddAssociativity)
            && operands.iter().all(|&v| self.reorder_safe(v));
        if do_sort {
            operands.sort_by_key(|&v| self.vhash[v as usize]);
        }
        let mut acc = operands[0];
        for &o in &operands[1..] {
            acc = self.mk(ValOp::Bin(op), vec![acc, o]);
        }
        acc
    }
    /// Rebuild a possibly-float `+`/`*` chain PRESERVING its source grouping (no reassociation —
    /// float `+`/`*` is non-associative, #342), combining the source operands left-to-right.
    /// When `commutable` (the chain-wide non-concat verdict), the two operands at each binary
    /// node are sorted by structural hash, so commuted-but-same-grouping chains (`(a+b)+1` vs
    /// `(b+a)+1`) still CONVERGE while differently-grouped ones (`(a+b)+1` vs `(1+a)+b`) stay
    /// distinct. The commutability is pushed DOWN from the whole chain because an inner pair
    /// (`a+b`) lacks the literal that licenses sorting — the flatten that used to supply it is
    /// exactly what the grouping hold suppresses. `mk` will not re-flatten (its `ac_chain_canon`
    /// is `possibly_float`-gated), so the held grouping survives interning.
    fn rebuild_grouped_float_chain(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
        commutable: bool,
    ) -> ValueId {
        let mut acc: Option<ValueId> = None;
        for &k in kids {
            let v = if self.il.kind(k) == NodeKind::BinOp
                && matches!(self.il.node(k).payload, Payload::Op(o) if o as u32 == op)
            {
                let sub = self.il.children(k).to_vec();
                self.rebuild_grouped_float_chain(op, &sub, env, commutable)
            } else {
                self.eval(k, env)
            };
            acc = Some(match acc {
                None => v,
                Some(a) => {
                    let (l, r) = if commutable && self.vhash[a as usize] > self.vhash[v as usize] {
                        (v, a)
                    } else {
                        (a, v)
                    };
                    self.mk(ValOp::Bin(op), vec![l, r])
                }
            });
        }
        acc.unwrap_or_else(|| self.eval(kids[0], env))
    }
    /// Wrap each LEAF operand of a JS-family `&`/`|`/`^` chain in `ToInt32` (a no-op for
    /// other ops and non-JS languages). Done after flattening so the wrap lands on the
    /// chain's leaves, not its intermediate (already int32) results — giving JS bitwise a
    /// fingerprint distinct from arbitrary-precision bitwise while keeping the op structure
    /// intact for the De Morgan / idempotence canons (#283-D). Shifts (`eval_binop_expr`) and
    /// `~` (`eval_inner`) are narrowed at their own build sites.
    fn narrow_js_bitwise_leaves(&mut self, op: u32, operands: &mut [ValueId]) {
        if op == Op::BitAnd as u32 || op == Op::BitOr as u32 || op == Op::BitXor as u32 {
            for v in operands.iter_mut() {
                *v = self.js_int32_narrow(*v);
            }
        }
    }
}
