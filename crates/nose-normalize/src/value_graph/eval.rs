use super::*;

const LARGE_AC_EXPR_OPERANDS: usize = 64;

impl<'a> Builder<'a> {
    /// JS strict (in)equality. The value model conflates `null`/`undefined` into
    /// ONE constant, so the model's Eq-against-null means the LOOSE check
    /// (`x == null`, true for both) — which is also what `x ?? d` desugars to. A
    /// strict `x === null` (false for undefined) cannot be expressed in that
    /// model: keep it a distinct shape keyed by the spelled operand, so strict
    /// checks converge only with the same strict spelling and never feed the
    /// nullish-default fold (`null_condition` → `ValueOrDefault`).
    fn eval_strict_equality_comparison(
        &mut self,
        expr: NodeId,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if (op != Op::Eq as u32 && op != Op::Ne as u32) || kids.len() != 2 {
            return None;
        }
        let strict = matches!(
            source_operator_at_node(self.il, expr),
            Some(
                nose_il::SourceOperatorKind::StrictEquality
                    | nose_il::SourceOperatorKind::StrictInequality
            )
        );
        if !strict {
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
            // Exception: `=== undefined` against a value that provably cannot be
            // null IS the faithful absence check — a typed `Map.get` result is
            // `V | undefined`. Keep the model's Eq shape there so the map-default
            // fold still proves `m.get(k) ?? d` ≡ the `=== undefined` guarded form.
            let undefined_spelling = matches!(
                (self.il.kind(null_kid), self.il.node(null_kid).payload),
                (NodeKind::Var, Payload::Name(_))
            );
            let undefined_only_absence = self.proven_map_get_value(value_side).is_some();
            if !(undefined_spelling && undefined_only_absence) {
                let salt = combine(JS_STRICT_NULL_CMP_TAG, self.valued_subtree_hash(null_kid));
                let eq = self.mk(ValOp::Opaque(salt), vec![value_side]);
                return Some(if op == Op::Ne as u32 {
                    self.mk(ValOp::Un(Op::Not as u32), vec![eq])
                } else {
                    eq
                });
            }
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

    pub(super) fn eval(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
        // Track the enclosing source expression so EVERY node created while evaluating it (the top
        // node AND the intermediate nodes a reduce/map unfolds via `mk`) is stamped with its span
        // at creation — those intermediates are exactly what a heavy sub-DAG anchor points at.
        let prev = self.cur_span;
        self.cur_span = Some(self.il.node(expr).span);
        let v = self.eval_inner(expr, env);
        self.cur_span = prev;
        v
    }

    pub(super) fn eval_inner(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
        let node = *self.il.node(expr);
        match node.kind {
            NodeKind::Var => self.eval_var_expr(expr, node.payload, env),
            NodeKind::Lit => self.eval_lit_expr(node.payload),
            NodeKind::BinOp => self.eval_binop_expr(expr, node.payload, env),
            NodeKind::UnOp => {
                let kids = self.il.children(expr).to_vec();
                let mut a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let op = op_code(node.payload);
                // JS `~x` is `~ToInt32(x)`, an int32 — narrow the operand so it fingerprints
                // distinctly from arbitrary-precision `~` (#283-D).
                if op == Op::BitNot as u32 {
                    for v in a.iter_mut() {
                        *v = self.js_int32_narrow(*v);
                    }
                }
                self.mk(ValOp::Un(op), a)
            }
            NodeKind::Field => self.eval_field_expr(expr, node.payload, env),
            NodeKind::Index => self.eval_index_expr(expr, env),
            NodeKind::Call => self.eval_call_expr(expr, node.payload, env),
            NodeKind::HoF => self.eval_hof_expr(expr, node.payload, env),
            NodeKind::Seq => self.eval_seq_expr(expr, node.payload, env),
            NodeKind::If => self.eval_if_expr(expr, env),
            NodeKind::Lambda => {
                let hash = self.valued_subtree_hash(expr);
                self.mk(ValOp::Lambda(hash), vec![])
            }
            // A keyword call argument `name=value` evaluated outside a call's own
            // kwarg handling (e.g. a kwarg that survived into an opaque position): key
            // it by the keyword name so `a=p` ≠ `b=p` and ≠ positional `p`.
            NodeKind::KwArg => {
                let name_hash = match node.payload {
                    Payload::Name(s) => self.interner.symbol_hash(s),
                    _ => 0,
                };
                let value = self
                    .il
                    .children(expr)
                    .first()
                    .map(|&v| self.eval(v, env))
                    .unwrap_or_else(|| self.mk(ValOp::Opaque(0), vec![]));
                self.mk(ValOp::KwArg(name_hash), vec![value])
            }
            // Any unlowered / unhandled construct — notably `Raw`, which wraps a
            // macro, C compound literal, `#ifdef`, parse-ERROR, etc. Key it by its full
            // subtree hash (surface kind + lowered children), exactly like `Lambda`, so
            // behaviorally-different unlowered constructs produce DIFFERENT fingerprints.
            // A positional opaque counter collapsed them (e.g. two distinct C compound
            // literals → one fingerprint = an unsound false merge the interpreter oracle
            // can't catch, since `Raw` is uninterpretable). Identical constructs converge.
            _ => {
                let hash = self.subtree_hash(expr);
                self.mk(ValOp::Opaque(hash), vec![])
            }
        }
    }

    fn eval_var_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        match payload {
            Payload::Cid(c) => env
                .get(&c)
                .copied()
                .unwrap_or_else(|| self.mk(ValOp::Input(c), vec![])),
            // A free variable (global / un-canonicalized callee) kept its name in
            // alpha — give it a STABLE identity keyed by that name (high-bit range,
            // clear of positional cids), so `foo(x)` ≠ `bar(x)` while two uses of
            // `foo` agree. Without this, distinct globals collapsed to one cid.
            Payload::Name(s) => {
                if let Some(&v) = self.global_env.get(&s) {
                    return v;
                }
                let name = self.interner.resolve(s);
                if self.is_rust_option_none_node(expr) {
                    return self.null_const();
                }
                if let Some(contract) = nullish_global_contract(self.il.meta.lang, name) {
                    if !contract.requires_unshadowed
                        || asserted_unshadowed_global_symbol(self.il, expr, contract.name)
                    {
                        return self.null_const();
                    }
                }
                self.mk(ValOp::Input(self.free_name_key(s)), vec![])
            }
            _ => self.fresh_opaque(),
        }
    }

    fn eval_lit_expr(&mut self, payload: Payload) -> ValueId {
        // Behavior-defining constants must be distinct values: `0` ≠ `1`
        // (else `x % 2 == 0` and `x % 2 == 1` collapse). Retained small ints
        // key by value (offset clear of the class range); others by class.
        let key = match payload {
            Payload::LitInt(v) => 0x1000_0000u32.wrapping_add(v as u32),
            // strings in a separate key range from ints to avoid collision
            Payload::LitStr(h) => 0x2000_0000u32.wrapping_add(h as u32),
            // floats keyed by source-text hash, in their own range (so `3.14`≠`2.71`)
            Payload::LitFloat(h) => 0x4000_0000u32.wrapping_add(h as u32),
            // true/false are behavior-defining and must be distinct (own range)
            Payload::LitBool(b) => 0x3000_0001u32 + b as u32,
            Payload::Lit(c) => c as u32,
            _ => 0,
        };
        self.mk(ValOp::Const(key), vec![])
    }

    fn eval_binop_expr(
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
        if let Some(v) = self.eval_strict_equality_comparison(expr, op, &kids, env) {
            return v;
        }
        if op == Op::Add as u32 || op == Op::Sub as u32 {
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
            let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
            self.mk(ValOp::Bin(op), a)
        }
    }

    fn eval_sub_chain(&mut self, kids: &[NodeId], env: &FxHashMap<u32, ValueId>) -> ValueId {
        let a = self.eval(kids[0], env);
        let b = self.eval(kids[1], env);
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
            let do_sort = (op != Op::Add as u32
                || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands))
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
        self.narrow_js_bitwise_leaves(op, &mut operands);
        let do_sort = (op != Op::Add as u32
            || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands))
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

    /// Wrap each LEAF operand of a JS-family `&`/`|`/`^` chain in `ToInt32` (a no-op for
    /// other ops and non-JS languages). Done after flattening so the wrap lands on the
    /// chain's leaves, not its intermediate (already int32) results — giving JS bitwise a
    /// fingerprint distinct from arbitrary-precision bitwise while keeping the op structure
    /// intact for the De Morgan / idempotence canons (#283-D). Shifts and `~` are narrowed
    /// at their own build sites.
    fn narrow_js_bitwise_leaves(&mut self, op: u32, operands: &mut [ValueId]) {
        if op == Op::BitAnd as u32 || op == Op::BitOr as u32 || op == Op::BitXor as u32 {
            for v in operands.iter_mut() {
                *v = self.js_int32_narrow(*v);
            }
        }
    }

    fn eval_field_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let kids = self.il.children(expr).to_vec();
        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        let name = match payload {
            Payload::Name(s) => self.interner.symbol_hash(s),
            _ => 0,
        };
        if a.len() == 1 {
            if let Some(v) = self.eval_field_builtin_or_import(expr, payload, &kids, &a) {
                return v;
            }
        }
        if a.len() == 1 {
            let Some(key) = self.exact_field_state_key(expr) else {
                return self.mk(ValOp::Field(name), a);
            };
            if let Some(&written) = self.field_env.get(&key) {
                return written;
            }
        }
        self.mk(ValOp::Field(name), a)
    }

    fn eval_field_builtin_or_import(
        &mut self,
        expr: NodeId,
        payload: Payload,
        kids: &[NodeId],
        a: &[ValueId],
    ) -> Option<ValueId> {
        if let Some(admitted) = admitted_property_builtin_at_field(self.il, self.interner, expr) {
            if admitted.contract.result == Builtin::Len {
                if let Some(len) = self.eval_len_value(a[0]) {
                    return Some(len);
                }
                if self
                    .domain_evidence_of_expr(kids[0])
                    .is_some_and(DomainEvidence::is_array_or_collection)
                {
                    return Some(self.mk(
                        ValOp::Call(builtin_tag(admitted.contract.result)),
                        a.to_vec(),
                    ));
                }
            }
        }
        if let Payload::Name(s) = payload {
            let receiver = &self.nodes[a[0] as usize];
            if let ValOp::ImportNamespace { module_hash } = receiver.op {
                return Some(self.mk(
                    ValOp::ImportBinding {
                        module_hash,
                        exported_hash: stable_symbol_hash(self.interner.resolve(s)),
                    },
                    vec![],
                ));
            }
        }
        None
    }

    fn eval_index_expr(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
        let kids = self.il.children(expr).to_vec();
        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        if a.len() == 2 {
            if let Some((map, default)) = self.proven_go_literal_zero_map_value(a[0]) {
                return self.mk(
                    ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                    vec![map, a[1], default],
                );
            }
        }
        self.mk(ValOp::Index, a)
    }

    fn eval_call_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let kids = self.il.children(expr).to_vec();
        // Promise continuation beta-reduction is exact only when a semantic pack can
        // prove the receiver is Promise-like; otherwise arbitrary `.then` methods stay
        // opaque.
        if let Some(v) = rules::promise_then::apply(self, expr, env) {
            return v;
        }
        if let Some(v) = rules::promise_then::promise_resolve_value(self, expr, env) {
            return v;
        }
        if let Payload::Builtin(builtin) = payload {
            if !self.admitted_builtin_call(expr, builtin) {
                return self.source_salted_opaque(expr, 0x4255_494C);
            }
        }
        if let Some(v) = self.eval_builtin_predicate_call(payload, &kids, env) {
            return v;
        }
        if let Some(v) = self.eval_builtin_collection_call(expr, payload, &kids, env) {
            return v;
        }
        if let Some(v) = self.eval_proven_call_pattern(expr, &kids, env) {
            return v;
        }
        // Interprocedural pure inline: `f(args)` to a pure file-local function ≡ its body
        // with `args` substituted — converges with the same logic written inline / with
        // a different extracted helper. Sound (β-reduction of an effect-free function).
        if let Some(v) = self.eval_inlined_call(expr, &kids, env) {
            return v;
        }
        // Keyword arguments are order-independent BY NAME, so evaluate positional args
        // in source order but the keyword args as a name-sorted suffix: `f(a=p, b=q)`
        // and `f(b=q, a=p)` converge, while `f(a=p, b=q)` and `f(a=q, b=p)` (different
        // mapping) stay distinct. The first child is the callee (or, for a builtin, the
        // first arg) and is never a keyword.
        let mut positional: Vec<ValueId> = Vec::new();
        let mut keyword: Vec<ValueId> = Vec::new();
        for &k in &kids {
            let v = self.eval(k, env);
            if self.il.kind(k) == NodeKind::KwArg {
                keyword.push(v);
            } else {
                positional.push(v);
            }
        }
        if !keyword.is_empty() {
            keyword.sort_unstable_by_key(|&v| self.vhash[v as usize]);
            positional.extend(keyword);
        }
        let tag = match payload {
            Payload::Builtin(b) => builtin_tag(b),
            _ => 0,
        };
        self.mk(ValOp::Call(tag), positional)
    }

    fn eval_builtin_predicate_call(
        &mut self,
        payload: Payload,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        // Reduction builtins fold a collection to one value — canonicalize to
        // the same `Reduce(op, init, per-element contrib)` a loop produces, so
        // `sum(x*x for x in xs)` / `reduce(λa,x. a+x*x, xs, 0)` converge with
        // the explicit accumulator loop (§AI representation axis).
        if let Payload::Builtin(Builtin::Abs) = payload {
            if let Some(&arg) = kids.first() {
                let v = self.eval(arg, env);
                return Some(self.mk(ValOp::Un(ABS_CODE), vec![v]));
            }
        }
        if let Payload::Builtin(Builtin::IsNull | Builtin::IsNotNull) = payload {
            if let Some(&arg) = kids.first() {
                let v = self.eval(arg, env);
                let op = if matches!(payload, Payload::Builtin(Builtin::IsNull)) {
                    Op::Eq
                } else {
                    Op::Ne
                };
                let nil = self.null_const();
                return Some(self.mk(ValOp::Bin(op as u32), vec![v, nil]));
            }
        }
        if let Payload::Builtin(Builtin::IsEmpty) = payload {
            if let Some(&arg) = kids.first() {
                let v = self.eval(arg, env);
                return Some(self.is_empty_value(v));
            }
        }
        None
    }

    fn eval_builtin_collection_call(
        &mut self,
        expr: NodeId,
        payload: Payload,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if let Payload::Builtin(Builtin::Contains) = payload {
            if let [element, collection] = kids {
                let element = self.eval(*element, env);
                if let Some(map) = self.proven_map_key_view_expr(*collection, env) {
                    return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, map]));
                }
                let collection = self.eval_membership_collection(*collection, env);
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
        }
        if let Payload::Builtin(Builtin::GetOrDefault) = payload {
            if let [map, key, default] = kids {
                let map = self.eval_map_lookup_collection(*map, env);
                let key = self.eval(*key, env);
                let default = self.eval(*default, env);
                return Some(self.mk(
                    ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                    vec![map, key, default],
                ));
            }
        }
        if let Payload::Builtin(Builtin::ValueOrDefault) = payload {
            if let [value, default] = kids {
                let value = self.eval(*value, env);
                let default = self.eval(*default, env);
                return Some(self.mk_value_or_map_default(value, default));
            }
        }
        if let Payload::Builtin(b) = payload {
            if let Some(r) = self.eval_reduction_builtin(expr, b, kids, env) {
                return Some(r);
            }
        }
        if matches!(payload, Payload::Builtin(Builtin::UnsignedCast32))
            && !nose_semantics::source_fact_at_node(
                self.il,
                expr,
                SourceFactKind::Cast(SourceCastKind::CUnsigned32),
            )
        {
            return Some(self.source_salted_opaque(expr, 0x5543_3332));
        }
        None
    }

    fn eval_proven_call_pattern(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if let Some(r) = self.eval_count_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_product_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_proven_integer_method_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_rust_map_get_unwrap_or_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_rust_map_get_is_some_call(expr, kids, env) {
            return Some(r);
        }
        if self.is_rust_vec_new_call(expr) {
            return Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]));
        }
        if let Some(v) = self.eval_java_collection_constructor_expr(expr, kids) {
            return Some(v);
        }
        if let Some(v) = self.eval_js_like_constructed_collection_or_map(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_java_map_factory_expr(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_iterator_identity_adapter(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_proven_free_minmax_call(expr, kids, env) {
            return Some(v);
        }
        if let Some(r) = self.eval_proven_collection_membership_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_proven_map_key_membership_call(expr, kids, env) {
            return Some(r);
        }
        if let Some(r) = self.eval_proven_map_get_default_call(expr, kids, env) {
            return Some(r);
        }
        if self.is_unproven_membership_like_call(expr, kids) {
            let salt = self.source_salted_hash(expr, 0x4D45_4D42_4552);
            return Some(self.mk(ValOp::Opaque(salt), vec![]));
        }
        None
    }

    fn eval_hof_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let kind = match payload {
            Payload::HoF(h) => h,
            _ => return self.source_salted_opaque(expr, 0x484F_465F),
        };
        let Some(admission) = self.hof_value_admission(expr, kind) else {
            return self.source_salted_opaque(expr, 0x484F_465F);
        };
        self.eval_hof_value(
            expr,
            kind,
            env,
            admission == HofAdmission::SourceComprehension,
        )
    }

    fn eval_seq_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        if let Some(value) = self.import_fact_value(expr) {
            return value;
        }
        let kids = self.il.children(expr).to_vec();
        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        if matches!(payload, Payload::Builtin(Builtin::DictEntry)) {
            return self.dict_entry(a);
        }
        if let Some(map) = self.proven_go_literal_zero_map_seq(expr, &a) {
            return map;
        }
        self.mk(ValOp::Seq(self.seq_tag(expr)), a)
    }

    fn eval_if_expr(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
        // Ternary / expression-if. Rust closures lower `if c { x } else { y }`
        // branches as Blocks whose trailing expression is the branch value, so
        // evaluate branches with the same implicit-return rule used for lambdas.
        let kids = self.il.children(expr).to_vec();
        let mut a = Vec::new();
        if let Some(&cond) = kids.first() {
            a.push(self.eval(cond, env));
        }
        for &branch in kids.iter().skip(1).take(2) {
            let mut branch_env = env.clone();
            let value = self
                .eval_block_return(branch, &mut branch_env)
                .unwrap_or_else(|| self.eval(branch, env));
            a.push(value);
        }
        // abs/min/max idiom recognition happens in `mk(Phi, …)` so it applies to
        // both the ternary and the equivalent if/else-assign form uniformly.
        self.mk(ValOp::Phi, a)
    }
}
