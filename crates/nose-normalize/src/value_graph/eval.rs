use super::*;

const LARGE_AC_EXPR_OPERANDS: usize = 64;

impl<'a> Builder<'a> {
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
            NodeKind::Var => match node.payload {
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
            },
            NodeKind::Lit => {
                // Behavior-defining constants must be distinct values: `0` ≠ `1`
                // (else `x % 2 == 0` and `x % 2 == 1` collapse). Retained small ints
                // key by value (offset clear of the class range); others by class.
                let key = match node.payload {
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
            NodeKind::BinOp => {
                let op = op_code(node.payload);
                let kids = self.il.children(expr).to_vec();
                if op == Op::In as u32 && kids.len() == 2 {
                    let element = self.eval(kids[0], env);
                    if self.is_js_like_lang() {
                        let collection = self.eval(kids[1], env);
                        return self
                            .mk(ValOp::Call(JS_PROTOTYPE_IN_CODE), vec![element, collection]);
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
                        return self.mk(ValOp::Bin(op), vec![element, map]);
                    }
                    let collection = self.eval_membership_collection(kids[1], env);
                    return self.mk(ValOp::Bin(op), vec![element, collection]);
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
                if let Payload::Op(op_kind) = node.payload {
                    if let Some(v) =
                        self.eval_static_index_membership_comparison(op_kind, &kids, env)
                    {
                        return v;
                    }
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
                    let a = self.eval(kids[0], env);
                    let b = self.eval(kids[1], env);
                    let neg_b = self.mk(ValOp::Un(Op::Neg as u32), vec![b]);
                    let mut operands = Vec::new();
                    self.flatten_into(a, Op::Add as u32, &mut operands);
                    self.flatten_into(neg_b, Op::Add as u32, &mut operands);
                    // Sort unless an operand is proven concat (string/list); otherwise the
                    // operands Err in the oracle regardless of order, so sorting is safe.
                    if self.add_values_not_concat(ValueLaw::AddAssociativity, &operands) {
                        operands.sort_by_key(|&v| self.vhash[v as usize]);
                    }
                    let mut acc = operands[0];
                    for &o in &operands[1..] {
                        acc = self.mk(ValOp::Bin(Op::Add as u32), vec![acc, o]);
                    }
                    return acc;
                }
                if is_assoc_comm_code(op) {
                    // Flatten the chain (resolving temps), sort by structural hash, and
                    // rebuild canonically — so groupings/temps converge. EXCEPT `+` is only
                    // commutative on numeric operands; on strings/lists it is concat, which
                    // is ordered, so we keep source order there (sorting would be unsound).
                    //
                    // Very large generated formulas can arrive as deeply nested binary ASTs.
                    // For those, collect same-op source operands first so one giant expression
                    // pays for flatten/sort/rebuild once instead of once per nested pair.
                    let mut expr_operands = Vec::new();
                    for &k in &kids {
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
                        let do_sort = op != Op::Add as u32
                            || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands);
                        if do_sort {
                            operands.sort_by_key(|&v| self.vhash[v as usize]);
                        }
                        return self.compact_formula(op, &operands);
                    }

                    let mut operands = Vec::new();
                    for k in kids {
                        let v = self.eval(k, env);
                        self.flatten_into(v, op, &mut operands);
                    }
                    if let Some(v) = self.c_u32_be_byte_pack_pattern(&operands) {
                        return v;
                    }
                    let do_sort = op != Op::Add as u32
                        || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands);
                    if do_sort {
                        operands.sort_by_key(|&v| self.vhash[v as usize]);
                    }
                    let mut acc = operands[0];
                    for &o in &operands[1..] {
                        acc = self.mk(ValOp::Bin(op), vec![acc, o]);
                    }
                    acc
                } else {
                    let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                    self.mk(ValOp::Bin(op), a)
                }
            }
            NodeKind::UnOp => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let op = op_code(node.payload);
                self.mk(ValOp::Un(op), a)
            }
            NodeKind::Field => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let name = match node.payload {
                    Payload::Name(s) => self.interner.symbol_hash(s),
                    _ => 0,
                };
                if a.len() == 1 {
                    if let Some(admitted) =
                        admitted_property_builtin_at_field(self.il, self.interner, expr)
                    {
                        if admitted.contract.result == Builtin::Len {
                            if let Some(len) = self.eval_len_value(a[0]) {
                                return len;
                            }
                            if self
                                .domain_evidence_of_expr(kids[0])
                                .is_some_and(DomainEvidence::is_array_or_collection)
                            {
                                return self
                                    .mk(ValOp::Call(builtin_tag(admitted.contract.result)), a);
                            }
                        }
                    }
                    if let Payload::Name(s) = node.payload {
                        let receiver = &self.nodes[a[0] as usize];
                        if let ValOp::ImportNamespace { module_hash } = receiver.op {
                            return self.mk(
                                ValOp::ImportBinding {
                                    module_hash,
                                    exported_hash: stable_symbol_hash(self.interner.resolve(s)),
                                },
                                vec![],
                            );
                        }
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
            NodeKind::Index => {
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
            NodeKind::Call => {
                let kids = self.il.children(expr).to_vec();
                // Promise continuation beta-reduction is exact only when a semantic pack can
                // prove the receiver is Promise-like; otherwise arbitrary `.then` methods stay
                // opaque.
                if let Some(v) = rules::promise_then::apply(self, expr, env) {
                    return v;
                }
                if let Payload::Builtin(builtin) = node.payload {
                    if !self.admitted_builtin_call(expr, builtin) {
                        return self.source_salted_opaque(expr, 0x4255_494C);
                    }
                }
                // Reduction builtins fold a collection to one value — canonicalize to
                // the same `Reduce(op, init, per-element contrib)` a loop produces, so
                // `sum(x*x for x in xs)` / `reduce(λa,x. a+x*x, xs, 0)` converge with
                // the explicit accumulator loop (§AI representation axis).
                if let Payload::Builtin(Builtin::Abs) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        return self.mk(ValOp::Un(ABS_CODE), vec![v]);
                    }
                }
                if let Payload::Builtin(Builtin::IsNull | Builtin::IsNotNull) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        let op = if matches!(node.payload, Payload::Builtin(Builtin::IsNull)) {
                            Op::Eq
                        } else {
                            Op::Ne
                        };
                        let nil = self.null_const();
                        return self.mk(ValOp::Bin(op as u32), vec![v, nil]);
                    }
                }
                if let Payload::Builtin(Builtin::IsEmpty) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        return self.is_empty_value(v);
                    }
                }
                if let Payload::Builtin(Builtin::Contains) = node.payload {
                    if let [element, collection] = kids.as_slice() {
                        let element = self.eval(*element, env);
                        if let Some(map) = self.proven_map_key_view_expr(*collection, env) {
                            return self.mk(ValOp::Bin(Op::In as u32), vec![element, map]);
                        }
                        let collection = self.eval_membership_collection(*collection, env);
                        return self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                    }
                }
                if let Payload::Builtin(Builtin::GetOrDefault) = node.payload {
                    if let [map, key, default] = kids.as_slice() {
                        let map = self.eval_map_lookup_collection(*map, env);
                        let key = self.eval(*key, env);
                        let default = self.eval(*default, env);
                        return self.mk(
                            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                            vec![map, key, default],
                        );
                    }
                }
                if let Payload::Builtin(Builtin::ValueOrDefault) = node.payload {
                    if let [value, default] = kids.as_slice() {
                        let value = self.eval(*value, env);
                        let default = self.eval(*default, env);
                        return self.mk_value_or_map_default(value, default);
                    }
                }
                if let Payload::Builtin(b) = node.payload {
                    if let Some(r) = self.eval_reduction_builtin(b, &kids, env) {
                        return r;
                    }
                }
                if matches!(node.payload, Payload::Builtin(Builtin::UnsignedCast32))
                    && !nose_semantics::source_fact_at_node(
                        self.il,
                        expr,
                        SourceFactKind::Cast(SourceCastKind::CUnsigned32),
                    )
                {
                    return self.source_salted_opaque(expr, 0x5543_3332);
                }
                if let Some(r) = self.eval_count_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_product_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_integer_method_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_rust_map_get_unwrap_or_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_rust_map_get_is_some_call(expr, &kids, env) {
                    return r;
                }
                if self.is_rust_vec_new_call(expr) {
                    return self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
                }
                if let Some(v) = self.eval_java_collection_constructor_expr(expr, &kids) {
                    return v;
                }
                if let Some(v) = self.eval_js_like_constructed_collection_or_map(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_java_map_factory_expr(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_iterator_identity_adapter(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_proven_free_minmax_call(expr, &kids, env) {
                    return v;
                }
                if let Some(r) = self.eval_proven_collection_membership_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_key_membership_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_get_default_call(expr, &kids, env) {
                    return r;
                }
                if self.is_unproven_membership_like_call(expr, &kids) {
                    let salt = self.source_salted_hash(expr, 0x4D45_4D42_4552);
                    return self.mk(ValOp::Opaque(salt), vec![]);
                }
                // Interprocedural pure inline: `f(args)` to a pure file-local function ≡ its body
                // with `args` substituted — converges with the same logic written inline / with
                // a different extracted helper. Sound (β-reduction of an effect-free function).
                if let Some(v) = self.eval_inlined_call(expr, &kids, env) {
                    return v;
                }
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let tag = match node.payload {
                    Payload::Builtin(b) => builtin_tag(b),
                    _ => 0,
                };
                self.mk(ValOp::Call(tag), a)
            }
            NodeKind::HoF => {
                let kind = match node.payload {
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
            NodeKind::Seq => {
                if let Some(value) = self.import_fact_value(expr) {
                    return value;
                }
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                if matches!(node.payload, Payload::Builtin(Builtin::DictEntry)) {
                    return self.dict_entry(a);
                }
                if let Some(map) = self.proven_go_literal_zero_map_seq(expr, &a) {
                    return map;
                }
                self.mk(ValOp::Seq(self.seq_tag(expr)), a)
            }
            NodeKind::If => {
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
            NodeKind::Lambda => {
                let hash = self.valued_subtree_hash(expr);
                self.mk(ValOp::Lambda(hash), vec![])
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
}
