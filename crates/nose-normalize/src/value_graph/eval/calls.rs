use super::super::*;

impl<'a> Builder<'a> {
    pub(super) fn eval_call_expr(
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
        if let Some(v) = self.eval_builtin_predicate_call(expr, payload, &kids, env) {
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
        // The name-sort is sound only when EVERY keyword value is effect-free: Python
        // evaluates arguments in SOURCE order, so `f(a=g(), b=h())` and `f(b=h(), a=g())`
        // run `g`/`h` in different orders — observably different if they raise or have
        // side effects. Reordering them would false-merge the two (coevo series 7, S3;
        // the same `reorder_safe` discipline as effectful AC operands, §CE/#286). With an
        // effectful keyword value, keep source order, so the two stay distinct.
        if !keyword.is_empty() && keyword.iter().all(|&v| self.reorder_safe(v)) {
            keyword.sort_unstable_by_key(|&v| self.vhash[v as usize]);
        }
        positional.extend(keyword);
        let tag = match payload {
            Payload::Builtin(b) => builtin_tag(b),
            _ => 0,
        };
        self.mk(ValOp::Call(tag), positional)
    }
    fn eval_builtin_predicate_call(
        &mut self,
        expr: NodeId,
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
                let v = self.eval_proven_integer_expr(arg, env)?;
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
        if let Some(v) = self.eval_rust_result_predicate_call(expr, env) {
            return Some(v);
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
                return Some(self.mk_nullish_map_default(value, default));
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
        if let Some(v) = self.eval_swift_collection_factory_expr(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_js_like_constructed_collection_or_map(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_java_map_factory_expr(expr, kids, env) {
            return Some(v);
        }
        if let Some(v) = self.eval_swift_map_factory_expr(expr, kids, env) {
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
}
