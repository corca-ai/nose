use super::*;

impl<'a> Interp<'a> {
    pub(super) fn eval_call(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let b = match self.il.node(node).payload {
            Payload::Builtin(b) => b,
            _ => return self.eval_user_call(node, env), // self-recursion, else opaque
        };
        let kids = self.il.children(node).to_vec();
        if !admitted_builtin_semantics_at_call_with_interner(self.il, self.interner, node, b) {
            // Unproven builtin spelling: an opaque, identified operation — not a bail.
            return self.opaque_call(sym_id(0xCA11_B170, &[hashed(&b)]), &kids, env);
        }
        let mut args = Vec::new();
        let profile = builtin_demand_profile(b);
        let demand = profile.demand_effect_profile();
        let eager_contract = match (demand.operation, profile) {
            (DemandOperation::FoldReduction, _) => return self.eval_reduce_call(&kids, env),
            (
                DemandOperation::ShortCircuitQuantifier,
                BuiltinDemandProfile::ShortCircuitQuantifier { all },
            ) => {
                return self.eval_any_all_call(all, &kids, env);
            }
            (DemandOperation::AppendMutation, _) => return self.eval_append(&kids, env),
            (DemandOperation::NullishDefault, _) => {
                return self.eval_value_or_default_call(&kids, env);
            }
            (_, BuiltinDemandProfile::Eager { contract }) => contract,
            // Admitted but unmodeled demand profile: opaque, identified by the builtin.
            _ => return self.opaque_call(sym_id(0xCA11_B170, &[hashed(&b)]), &kids, env),
        };
        for &k in &kids {
            let arg = self.eval(k, env)?;
            if matches!(arg, Value::Err) {
                return Ok(Value::Err);
            }
            args.push(arg);
        }
        self.eval_eager_builtin(eager_contract, args)
    }

    pub(super) fn eval_eager_builtin(
        &mut self,
        eager_contract: EagerBuiltinContract,
        args: Vec<Value>,
    ) -> R<Value> {
        // A symbolic operand anywhere (including inside a list) makes the result
        // symbolic — the concrete arms below must never see one, or unknownness
        // would launder into a concrete `Err`.
        if args.iter().any(contains_sym) {
            let mut parts = vec![hashed(&format!("{eager_contract:?}"))];
            parts.extend(args.iter().map(vhash));
            return Ok(Value::Sym(sym_id(0x00EA_9E12, &parts)));
        }
        match eager_contract {
            EagerBuiltinContract::Len => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Int(xs.len() as i64)),
                // A string is the free monoid over opaque piece hashes; its character
                // length is unknown (piece count ≠ char count), so `len` stays `Err` —
                // matching the type doc and the `IsEmpty` sibling. Returning a constant
                // `Int(1)` falsely equated `len(any_string)` with the literal `1`.
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsEmpty => match args.first() {
                Some(Value::List(xs)) => Ok(Value::Bool(xs.is_empty())),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsNull => match args.first() {
                Some(value) => Ok(Value::Bool(matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::IsNotNull => match args.first() {
                Some(value) => Ok(Value::Bool(!matches!(value, Value::Null))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::StartsWith => Ok(string_affix(args.first(), args.get(1), true)),
            EagerBuiltinContract::EndsWith => Ok(string_affix(args.first(), args.get(1), false)),
            EagerBuiltinContract::Contains => match (args.first(), args.get(1)) {
                (Some(element), Some(Value::List(items))) => Ok(Value::Bool(
                    items.iter().any(|candidate| candidate == element),
                )),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::StringContains => string_contains(args.first(), args.get(1)),
            EagerBuiltinContract::Join => Ok(join_strings(args.first(), args.get(1))),
            EagerBuiltinContract::Abs => match args.first() {
                Some(Value::Int(v)) => Ok(Value::Int(v.abs())),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::UnsignedCast32 => match args.first() {
                Some(Value::Int(v)) => Ok(Value::Int(v.rem_euclid(1_i64 << 32))),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Sum => Ok(fold_ints(args.first(), 0, |a, x| a + x)),
            EagerBuiltinContract::Min => Ok(min_max_value(&args, |a, x| a.min(x))),
            EagerBuiltinContract::Max => Ok(min_max_value(&args, |a, x| a.max(x))),
            EagerBuiltinContract::Range => range_values(&args),
            EagerBuiltinContract::Zip => match (args.first(), args.get(1)) {
                (Some(Value::List(a)), Some(Value::List(b))) => Ok(Value::List(
                    a.iter()
                        .zip(b.iter())
                        .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                        .collect(),
                )),
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Enumerate => match args.first() {
                Some(Value::List(xs)) => Ok(Value::List(
                    xs.iter()
                        .enumerate()
                        .map(|(i, x)| Value::List(vec![Value::Int(i as i64), x.clone()]))
                        .collect(),
                )),
                _ => Ok(Value::Err),
            },
            // `for (x in list)` iterates the indices 0..n-1 (keys). Objects aren't
            // modeled → Err (so such loops are non-interpretable, not falsely merged).
            EagerBuiltinContract::Keys => match args.first() {
                Some(Value::List(xs)) => {
                    Ok(Value::List((0..xs.len() as i64).map(Value::Int).collect()))
                }
                _ => Ok(Value::Err),
            },
            EagerBuiltinContract::Print => {
                for a in &args {
                    self.effects.push(a.clone());
                }
                Ok(Value::Null)
            }
            // Dicts are not modeled — a `DictEntry` makes its unit non-interpretable (Err),
            // so dict-building units are excluded from the oracle rather than risk a false
            // merge. Their convergence rests on the DistinctEntry-vs-tuple representation.
            EagerBuiltinContract::DictEntry => Ok(Value::Err),
            EagerBuiltinContract::GetOrDefault => Ok(Value::Err),
        }
    }

    pub(super) fn eval_value_or_default_call(
        &mut self,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let value = self.eval(*kids.first().ok_or(Unsupported)?, env)?;
        if matches!(value, Value::Err) {
            return Ok(Value::Err);
        }
        // Null-ness of a top-level Sym is unknown: compose symbolically (the default
        // is evaluated eagerly under the same convention on both sides of a merge).
        if matches!(value, Value::Sym(_)) {
            let d = match kids.get(1) {
                Some(&default) => self.eval(default, env)?,
                None => Value::Null,
            };
            if matches!(d, Value::Err) {
                return Ok(Value::Err);
            }
            return Ok(Value::Sym(sym_id(0x9015_4E11, &[vhash(&value), vhash(&d)])));
        }
        if matches!(value, Value::Null) {
            return match kids.get(1) {
                Some(&default) => self.eval(default, env),
                None => Ok(Value::Null),
            };
        }
        Ok(value)
    }

    /// A non-builtin `callee(args…)`. Modeled only when call-target evidence resolves the
    /// occurrence to an in-file function root. The arguments are evaluated call-by-value in the
    /// caller, then bound to a fresh callee frame; effects, field state, and step budget are
    /// shared so recursion stays ordered and bounded. Every unproven or ambiguous call remains
    /// unsupported rather than guessed.
    /// An opaque call: the callee's semantics are unknown, but its IDENTITY and its
    /// argument values are not. Evaluate the arguments left-to-right (an `Err` argument
    /// propagates, as for every call), produce the symbolic application value, and
    /// RECORD it in the effect trace — an unknown callee may observably act, and call
    /// order is behavior. Both sides of a fingerprint merge see the same convention,
    /// so symbolic traces stay comparable; Sym-bearing disagreements are routed to the
    /// advisory lane by the verify report, never the hard SOUND gate.
    pub(super) fn opaque_call(
        &mut self,
        ident: u64,
        args: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let mut parts = vec![ident];
        for &a in args {
            let v = self.eval(a, env)?;
            if matches!(v, Value::Err) {
                return Ok(Value::Err);
            }
            parts.push(vhash(&v));
        }
        let sym = Value::Sym(sym_id(0x0CA1_1E55, &parts));
        self.effects.push(sym.clone());
        Ok(sym)
    }

    pub(super) fn eval_user_call(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let kids = self.il.children(node).to_vec();
        let &callee = kids.first().ok_or(Unsupported)?;
        let Some(target) = self.proven_call_target(node) else {
            // Unproven/ambiguous target: an opaque call identified by the callee's
            // structural signature (pre-canon syntax — fingerprint-equal units have
            // matching alpha-renamed cids, so signatures stay comparable).
            let ident = subtree_sig(self.il, self.interner, callee);
            return self.opaque_call(ident, &kids[1..], env);
        };
        // Bind arguments to the CALLEE's parameters in a fresh environment by the shared
        // plan (matching the value graph): positional left-to-right, keyword by name. An
        // unresolvable mapping bails the unit (Unsupported) so the oracle never mis-binds
        // a reordered keyword call (#301). Locals start empty; the effect trace, field
        // state, and step budget are shared.
        let params = self.il.children(target).to_vec();
        let param_cids: Vec<u32> = params
            .iter()
            .filter_map(|&p| match (self.il.kind(p), self.il.node(p).payload) {
                (NodeKind::Param, Payload::Cid(c)) => Some(c),
                _ => None,
            })
            .collect();
        let plan = crate::call_args::keyword_arg_binding_plan(self.il, &param_cids, &kids[1..])
            .ok_or(Unsupported)?;
        let mut fenv: FxHashMap<u32, Value> = FxHashMap::default();
        for (cid, value_node) in plan {
            let value = self.eval(value_node, env)?;
            if matches!(value, Value::Err) {
                return Ok(Value::Err);
            }
            fenv.insert(cid, value);
        }
        let body = *params.last().ok_or(Unsupported)?;
        let result = self.exec(body, &mut fenv);
        match result? {
            Flow::Ret(v) => Ok(v),
            Flow::Err => Ok(Value::Err),
            _ => Ok(Value::Null),
        }
    }

    pub(super) fn proven_call_target(&self, call: NodeId) -> Option<NodeId> {
        let mut found = None;
        for &root in &self.callable_roots {
            if !direct_function_call_target_at_call(self.il, self.interner, call, root) {
                continue;
            }
            if found.replace(root).is_some() {
                return None;
            }
        }
        found
    }

    /// `any`/`all` over a collection: short-circuit existential/universal truth. The method
    /// form `[coll, λ]` applies the predicate per element; the generator form `[mapped-list]`
    /// reads each element's truthiness directly. `all` of empty = true, `any` of empty =
    /// false (the AND/OR identities).
    /// `append(coll, items…)` as a VALUE (e.g. Go's `s = append(s, x...)`, which returns the
    /// extended slice and does NOT mutate in place): functional — return `coll ++ items`.
    /// The Python/JS *statement* form `r.append(x)` is handled in `exec` (in-place build for
    /// a local list, effect for a parameter), not here.
    pub(super) fn eval_append(
        &mut self,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let mut xs = match kids.first() {
            Some(&t) => match self.eval(t, env)? {
                Value::List(xs) => xs,
                Value::Err => return Ok(Value::Err),
                v if contains_sym(&v) => {
                    // append over an unknown collection: compose, do not launder to Err
                    let mut parts = vec![vhash(&v)];
                    for &k in kids.iter().skip(1) {
                        let item = self.eval(k, env)?;
                        if matches!(item, Value::Err) {
                            return Ok(Value::Err);
                        }
                        parts.push(vhash(&item));
                    }
                    return Ok(Value::Sym(sym_id(0x00A9_9E4D, &parts)));
                }
                _ => return Ok(Value::Err),
            },
            None => return Ok(Value::Err),
        };
        let mut items = Vec::with_capacity(kids.len().saturating_sub(1));
        for &k in kids.iter().skip(1) {
            let item = self.eval(k, env)?;
            if matches!(item, Value::Err) {
                return Ok(Value::Err);
            }
            items.push(item);
        }
        xs.extend(items);
        Ok(Value::List(xs))
    }

    /// A statement-level `r.append(x)` / `r.push(x)`: build `r` in place when it is a LOCAL
    /// list var (so `return r` yields the constructed list, converging with `[x for …]`);
    /// when `r` is a parameter (or non-list / non-var target) the append is a caller-visible
    /// mutation, recorded as an effect. Returns `Some` if `e` was an append handled here.
    pub(super) fn exec_stmt_append(
        &mut self,
        e: NodeId,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Option<Flow>> {
        if self.il.kind(e) != NodeKind::Call
            || !matches!(self.il.node(e).payload, Payload::Builtin(Builtin::Append))
            || !admitted_builtin_semantics_at_call_with_interner(
                self.il,
                self.interner,
                e,
                Builtin::Append,
            )
        {
            return Ok(None);
        }
        let kids = self.il.children(e).to_vec();
        let target_cid = kids.first().and_then(|&t| {
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(t), self.il.node(t).payload) {
                Some(c)
            } else {
                None
            }
        });
        if target_cid.is_some_and(|c| matches!(env.get(&c), Some(Value::Err))) {
            return Ok(Some(Flow::Err));
        }
        if target_cid.is_none() {
            if let Some(&target) = kids.first() {
                let target_value = if self.il.kind(target) == NodeKind::Field {
                    match self.il.children(target).first() {
                        Some(&receiver) => self.eval(receiver, env)?,
                        None => Value::Null,
                    }
                } else {
                    self.eval(target, env)?
                };
                if matches!(target_value, Value::Err) {
                    return Ok(Some(Flow::Err));
                }
            }
        }
        let mut items = Vec::with_capacity(kids.len().saturating_sub(1));
        for &k in kids.iter().skip(1) {
            let item = self.eval(k, env)?;
            if matches!(item, Value::Err) {
                return Ok(Some(Flow::Err));
            }
            items.push(item);
        }
        if let Some(c) = target_cid {
            if !self.params.contains(&c) {
                if let Some(Value::List(xs)) = env.get_mut(&c) {
                    xs.extend(items);
                    return Ok(Some(Flow::Normal));
                }
            }
        }
        for a in items {
            self.effects.push(a);
        }
        Ok(Some(Flow::Normal))
    }

    pub(super) fn eval_any_all_call(
        &mut self,
        all: bool,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        let coll = match self.eval(*kids.first().ok_or(Unsupported)?, env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![u64::from(all), vhash(&v)];
                parts.extend(
                    kids.iter()
                        .skip(1)
                        .map(|&k| subtree_sig(self.il, self.interner, k)),
                );
                return Ok(Value::Sym(sym_id(0x0A11_A4E0, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let pred = kids
            .get(1)
            .filter(|&&k| self.il.kind(k) == NodeKind::Lambda);
        for x in coll {
            let v = match pred {
                Some(&l) => self.apply(l, &[x], env)?,
                None => x,
            };
            if matches!(v, Value::Err) {
                return Ok(Value::Err);
            }
            let t = truthy(&v).ok_or(Unsupported)?;
            // short-circuit: `any` stops at the first truthy, `all` at the first falsy.
            if all != t {
                return Ok(Value::Bool(t));
            }
        }
        Ok(Value::Bool(all))
    }

    /// `reduce(f, xs[, init])`: fold `f` over `xs`.
    pub(super) fn eval_reduce_call(
        &mut self,
        kids: &[NodeId],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        if kids.len() < 2 {
            return Err(Unsupported);
        }
        let lambda = kids[0];
        let seq = match self.eval(kids[1], env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![subtree_sig(self.il, self.interner, lambda), vhash(&v)];
                if let Some(&i) = kids.get(2) {
                    let init = self.eval(i, env)?;
                    if matches!(init, Value::Err) {
                        return Ok(Value::Err);
                    }
                    parts.push(vhash(&init));
                }
                return Ok(Value::Sym(sym_id(0x04ED_0CE0, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let mut it = seq.into_iter();
        let mut acc = match kids.get(2) {
            Some(&i) => self.eval(i, env)?,
            None => match it.next() {
                Some(v) => v,
                None => return Ok(Value::Err),
            },
        };
        if matches!(acc, Value::Err) {
            return Ok(Value::Err);
        }
        for x in it {
            acc = self.apply(lambda, &[acc, x], env)?;
            if matches!(acc, Value::Err) {
                return Ok(Value::Err);
            }
        }
        Ok(acc)
    }
}
