//! Statement, block, loop, and local-reduction processing for value-graph construction.
//!
//! proof-obligation: normalize.control_flow.guard_returns
//! proof-obligation: normalize.value_graph.bool_reduce

use super::*;

impl<'a> Builder<'a> {
    /// Build the value graph for a `Func`/`Method`/class unit. The unit root may
    /// be a `Func` (params + body) or a `Block` (class body of methods); for a
    /// `Block` we process its statements directly.
    pub(super) fn build_unit(&mut self, root: NodeId) {
        self.build_unit_with_context(root, None);
    }

    pub(super) fn build_unit_with_context(
        &mut self,
        root: NodeId,
        context: Option<&ValueFingerprintContext>,
    ) {
        self.param_domain.clear();
        self.seed_param_domains(root);
        self.seed_param_value_domains(root);
        self.seed_immutable_bindings(root, context);
        self.build_inline_registry(root);
        let mut env: FxHashMap<u32, ValueId> = FxHashMap::default();
        match self.il.kind(root) {
            NodeKind::Func => {
                // Seed parameters as inputs *by position*, so duplicate-named params
                // (which alpha-rename collapses to one cid) stay distinct values — the
                // accessible one wins, as at runtime. For well-formed code param cid ==
                // position, so this is identical to keying by cid.
                let kids = self.il.children(root).to_vec();
                let mut pos = 0u32;
                for &k in &kids {
                    if self.il.kind(k) == NodeKind::Param {
                        if let Payload::Cid(c) = self.il.node(k).payload {
                            let v = self.mk(ValOp::Input(pos), vec![]);
                            env.insert(c, v);
                            pos += 1;
                        }
                    }
                }
                if let Some(&body) = kids.last() {
                    self.process_stmt(body, &mut env);
                }
                self.recognize_value_default_returns();
                self.recognize_existence_reduction();
            }
            NodeKind::Module | NodeKind::Block => {
                // Class/other container unit. Two things make its data visible:
                //  (1) attribute assignments (`name = value`) land in `env` but reach no
                //      sink — a class's attributes ARE its data, so expose them (two
                //      locale-table classes that differ only in values must differ);
                //  (2) a container's *behavior* is the aggregate of its methods. Plain
                //      `process_stmt` has no `Func` case, so a method definition fell to
                //      the opaque-effect branch and the class collapsed to a near-empty
                //      structural shell — a one-operator change deep inside a method left
                //      the class fingerprint identical, so two classes were "behavioral
                //      clones" on structure alone. Descend into each contained method and
                //      fold its returns/effects into the container, so the class differs
                //      exactly when its methods do.
                self.process_container(root, &mut env);
                let mut vals: Vec<ValueId> = env.values().copied().collect();
                vals.sort_unstable();
                vals.dedup();
                for v in vals {
                    self.sinks.push(Sink::new(SinkKind::Effect, v));
                }
            }
            _ => {
                self.process_stmt(root, &mut env);
            }
        }
        self.flush_fields();
    }

    /// Recognize an existence/universal loop written with an early return, and rewrite it
    /// to the same `Reduce(REDUCE_ANY/ALL, [predicate])` the functional `any`/`all` builds:
    ///   `for x in xs: if p(x): return True` ; `return False`        → `any(p(x) for x in xs)`
    ///   `for x in xs: if not p(x): return False` ; `return True`    → `all(p(x) for x in xs)`
    /// After lowering these are exactly two `Return` sinks (no effects): one guarded by a
    /// predicate over a loop ELEMENT returning a bool constant, plus the unguarded
    /// complementary bool constant. Behavior-preserving — `∃`/`∀` over a pure predicate
    /// is order- and short-circuit-insensitive — and gated by the oracle. Requiring an
    /// `Elem` in the guard ties it to genuine collection iteration (a first-element check
    /// `if xs[0]>0` keeps an `Index`, not `Elem`, so it is not mistaken for `any`).
    pub(super) fn recognize_existence_reduction(&mut self) {
        if self.sinks.len() != 2
            || self
                .sinks
                .iter()
                .any(|sink| !matches!(sink.kind, SinkKind::Return))
        {
            return;
        }
        let r0 = self.sinks[0].value;
        let r1 = self.sinks[1].value;
        let bot = self.mk(ValOp::Const(0x3000_0000), vec![]);
        let tru = self.mk(ValOp::Const(0x3000_0002), vec![]);
        let fls = self.mk(ValOp::Const(0x3000_0001), vec![]);
        let int_one = self.int_const(1);
        let int_zero = self.int_const(0);
        for &(guarded, plain) in &[(r0, r1), (r1, r0)] {
            let ValOp::Phi = self.nodes[guarded as usize].op else {
                continue;
            };
            let a = self.nodes[guarded as usize].args.clone();
            if a.len() != 3 || a[2] != bot || !self.refs_elem(a[0]) {
                continue;
            }
            let (guard, ret) = (a[0], a[1]);
            let ret_true = ret == tru || ret == int_one;
            let ret_false = ret == fls || ret == int_zero;
            let plain_true = plain == tru || plain == int_one;
            let plain_false = plain == fls || plain == int_zero;
            let (code, pred) = if ret_true && plain_false {
                (REDUCE_ANY, guard)
            } else if ret_false && plain_true {
                (REDUCE_ALL, self.mk(ValOp::Un(Op::Not as u32), vec![guard]))
            } else {
                continue;
            };
            let red = self.mk(ValOp::Reduce(code), vec![pred]);
            self.sinks = vec![Sink::new(SinkKind::Return, red)];
            return;
        }
    }

    pub(super) fn recognize_value_default_returns(&mut self) {
        if self.sinks.len() != 2
            || self
                .sinks
                .iter()
                .any(|sink| !matches!(sink.kind, SinkKind::Return))
        {
            return;
        }
        if let Some(defaulted) = self
            .map_default_from_partial_default_pair(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.map_default_from_partial_default_pair(self.sinks[1].value, self.sinks[0].value)
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) =
            self.map_default_from_guarded_pair(self.sinks[0].value, self.sinks[1].value)
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) = self
            .map_default_from_guarded_fallthrough(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.map_default_from_guarded_fallthrough(self.sinks[1].value, self.sinks[0].value)
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) =
            self.value_default_from_guarded_pair(self.sinks[0].value, self.sinks[1].value)
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) = self
            .value_default_from_guarded_fallthrough(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.value_default_from_guarded_fallthrough(
                    self.sinks[1].value,
                    self.sinks[0].value,
                )
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
        }
    }

    pub(super) fn map_default_from_partial_default_pair(
        &mut self,
        partial: ValueId,
        fallback: ValueId,
    ) -> Option<ValueId> {
        let (map, key) = self.map_default_bottom_call(partial)?;
        let (cond, fallback_ret) = self.guarded_return_parts(fallback)?;
        let (guard_key, guard_map, present) = self.map_presence_condition(cond)?;
        if present || guard_key != key || guard_map != map {
            return None;
        }
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, fallback_ret],
        ))
    }

    pub(super) fn map_default_from_guarded_pair(
        &mut self,
        first: ValueId,
        second: ValueId,
    ) -> Option<ValueId> {
        let (cond_a, ret_a) = self.guarded_return_parts(first)?;
        let (cond_b, ret_b) = self.guarded_return_parts(second)?;
        let (key_a, map_a, present_a) = self.map_presence_condition(cond_a)?;
        let (key_b, map_b, present_b) = self.map_presence_condition(cond_b)?;
        if key_a != key_b || map_a != map_b || present_a == present_b {
            return None;
        }
        let default = if present_a {
            if !self.map_lookup_value_matches(ret_a, map_a, key_a) {
                return None;
            }
            ret_b
        } else {
            if !self.map_lookup_value_matches(ret_b, map_a, key_a) {
                return None;
            }
            ret_a
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map_a, key_a, default],
        ))
    }

    pub(super) fn map_default_from_guarded_fallthrough(
        &mut self,
        guarded: ValueId,
        fallthrough: ValueId,
    ) -> Option<ValueId> {
        let (cond, guarded_ret) = self.guarded_return_parts(guarded)?;
        let (key, map, present) = self.map_presence_condition(cond)?;
        let default = if present {
            if !self.map_lookup_value_matches(guarded_ret, map, key) {
                return None;
            }
            fallthrough
        } else {
            if !self.map_lookup_value_matches(fallthrough, map, key) {
                return None;
            }
            guarded_ret
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    pub(super) fn value_default_from_guarded_pair(
        &mut self,
        first: ValueId,
        second: ValueId,
    ) -> Option<ValueId> {
        let (cond_a, ret_a) = self.guarded_return_parts(first)?;
        let (cond_b, ret_b) = self.guarded_return_parts(second)?;
        let (value_a, present_a) = self.null_condition(cond_a)?;
        let (value_b, present_b) = self.null_condition(cond_b)?;
        if value_a != value_b || present_a == present_b {
            return None;
        }
        let (value, default) = if present_a {
            if ret_a != value_a {
                return None;
            }
            (value_a, ret_b)
        } else {
            if ret_b != value_a {
                return None;
            }
            (value_a, ret_a)
        };
        Some(self.mk_value_or_map_default(value, default))
    }

    pub(super) fn value_default_from_guarded_fallthrough(
        &mut self,
        guarded: ValueId,
        fallthrough: ValueId,
    ) -> Option<ValueId> {
        let (cond, guarded_ret) = self.guarded_return_parts(guarded)?;
        let (value, present) = self.null_condition(cond)?;
        let default = if present {
            if guarded_ret != value {
                return None;
            }
            fallthrough
        } else {
            if fallthrough != value {
                return None;
            }
            guarded_ret
        };
        Some(self.mk_value_or_map_default(value, default))
    }

    pub(super) fn guarded_return_parts(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Phi)
            || node.args.len() != 3
            || !self.is_bottom_value(node.args[2])
        {
            return None;
        }
        Some((node.args[0], node.args[1]))
    }

    pub(super) fn map_default_bottom_call(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == builtin_tag(Builtin::GetOrDefault))
            && node.args.len() == 3
            && self.is_bottom_value(node.args[2])
        {
            return Some((node.args[0], node.args[1]));
        }
        None
    }

    pub(super) fn is_bottom_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Const(k) if k == 0x3000_0000)
    }

    /// The conjunction of the current branch path (`c₁ ∧ c₂ ∧ …`), or `None` at top level.
    pub(super) fn path_cond(&mut self) -> Option<ValueId> {
        let mut pc: Option<ValueId> = None;
        for &c in &self.path.clone() {
            pc = Some(match pc {
                None => c,
                Some(p) => self.mk(ValOp::Bin(Op::And as u32), vec![p, c]),
            });
        }
        pc
    }
    /// Does `v`'s value subgraph reference an `Elem` (a collection element)? Bounded DAG
    /// walk; used to confirm a guard is a per-element predicate of a loop.
    pub(super) fn refs_elem(&self, v: ValueId) -> bool {
        let mut seen = FxHashSet::default();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if matches!(self.nodes[n as usize].op, ValOp::Elem(_)) {
                return true;
            }
            stack.extend(self.nodes[n as usize].args.iter().copied());
        }
        false
    }

    pub(super) fn process_block(&mut self, block: NodeId, env: &mut FxHashMap<u32, ValueId>) {
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
            if let Some(ncond) = self.guard_clause_negation(s, env) {
                self.record_bound_order_fact(ncond);
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
    pub(super) fn guard_clause_negation(
        &mut self,
        s: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(s) != NodeKind::If {
            return None;
        }
        let kids = self.il.children(s).to_vec();
        if kids.len() != 2 || !self.branch_exits(kids[1]) {
            return None; // has an else, or the then-branch can fall through
        }
        let cond = self.eval(kids[0], env);
        Some(self.mk(ValOp::Un(Op::Not as u32), vec![cond]))
    }

    pub(super) fn record_bound_order_fact(&mut self, cond: ValueId) {
        if let Some((lo, hi)) = self.bound_order_from_condition(cond) {
            self.bound_order_facts.push((lo, hi));
        }
    }

    pub(super) fn bound_order_from_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId)> {
        match &self.nodes[cond as usize] {
            ValNode {
                op: ValOp::Bin(op),
                args,
            } if *op == Op::Le as u32 && args.len() == 2 => Some((args[0], args[1])),
            _ => None,
        }
    }

    pub(super) fn has_bound_order_fact(&self, lo: ValueId, hi: ValueId) -> bool {
        if let (Some(lo_value), Some(hi_value)) =
            (self.int_const_value(lo), self.int_const_value(hi))
        {
            return lo_value <= hi_value;
        }
        self.bound_order_facts
            .iter()
            .any(|&(fact_lo, fact_hi)| fact_lo == lo && fact_hi == hi)
    }

    pub(super) fn is_safe_clamp_integer_value(&self, value: ValueId) -> bool {
        self.int_const_value(value).is_some() || self.is_param_value(value, DomainEvidence::Integer)
    }

    pub(super) fn proof_backed_clamp_value(
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
            Some(self.mk(ValOp::Clamp, vec![x, lo, hi]))
        } else {
            None
        }
    }

    pub(super) fn clamp_minmax_candidates(
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

    pub(super) fn bin_args(&self, value: ValueId, want: u32) -> Option<(ValueId, ValueId)> {
        match &self.nodes[value as usize] {
            ValNode {
                op: ValOp::Bin(op),
                args,
            } if *op == want && args.len() == 2 => Some((args[0], args[1])),
            _ => None,
        }
    }

    pub(super) fn bin_other_arg(
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
    pub(super) fn branch_exits(&self, node: NodeId) -> bool {
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

    pub(super) fn branch_returns(&self, node: NodeId) -> bool {
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

    pub(super) fn is_effect_free_throw_body(&self, node: NodeId) -> bool {
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

    pub(super) fn is_effect_free_throw_prefix(&self, node: NodeId) -> bool {
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

    pub(super) fn is_effect_free_static_err_body(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        match self.il.kind(node) {
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.expr_is_static_runtime_err(expr, env)),
            NodeKind::Return => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.expr_is_static_runtime_err(expr, env)),
            NodeKind::Assign => self.assign_is_static_runtime_err(node, env),
            NodeKind::Block => {
                let Some((&last, prefix)) = self.il.children(node).split_last() else {
                    return false;
                };
                prefix
                    .iter()
                    .all(|&stmt| self.is_effect_free_throw_prefix(stmt))
                    && self.is_effect_free_static_err_body(last, env)
            }
            _ => false,
        }
    }

    pub(super) fn assign_is_static_runtime_err(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return false;
        }
        let target = kids[0];
        let rhs = kids[1];
        if self.expr_is_static_runtime_err(rhs, env) {
            return crate::is_pure(self.il, rhs);
        }
        crate::is_pure(self.il, rhs) && self.assignment_target_is_static_runtime_err(target, env)
    }

    pub(super) fn assignment_target_is_static_runtime_err(
        &mut self,
        target: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        match self.il.kind(target) {
            NodeKind::Field => self
                .il
                .children(target)
                .first()
                .is_some_and(|&receiver| self.expr_is_static_runtime_err(receiver, env)),
            NodeKind::Index => {
                self.il
                    .children(target)
                    .to_vec()
                    .split_first()
                    .is_some_and(|(&base, rest)| {
                        if self.expr_is_static_runtime_err(base, env) {
                            return true;
                        }
                        crate::is_pure(self.il, base)
                            && rest
                                .first()
                                .is_some_and(|&index| self.expr_is_static_runtime_err(index, env))
                    })
            }
            _ => false,
        }
    }

    pub(super) fn expr_is_static_runtime_err(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.il.kind(expr) == NodeKind::Seq {
            for child in self.il.children(expr).to_vec() {
                if self.expr_is_static_runtime_err(child, env) {
                    return crate::is_pure(self.il, child);
                }
                if !crate::is_pure(self.il, child) {
                    return false;
                }
            }
            return false;
        }
        if self.il.kind(expr) == NodeKind::HoF
            && matches!(
                self.il.node(expr).payload,
                Payload::HoF(HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter)
                    | Payload::HoF(HoFKind::FilterMap)
            )
        {
            let Payload::HoF(kind) = self.il.node(expr).payload else {
                return false;
            };
            let demand = admitted_hof_demand_effect_profile_at_node(self.il, expr, kind);
            let Some(demand) = demand else { return false };
            if !demand.proves_eager_per_element_callback_demand() {
                return false;
            }
            let kids = self.il.children(expr).to_vec();
            return kids
                .first()
                .is_some_and(|&coll| self.expr_is_static_non_empty_seq(coll))
                && kids
                    .get(1)
                    .is_some_and(|&lambda| self.lambda_body_is_static_runtime_err(lambda, env));
        }
        if self.il.kind(expr) == NodeKind::If {
            let kids = self.il.children(expr).to_vec();
            let Some(&cond) = kids.first() else {
                return false;
            };
            if self.expr_is_static_runtime_err(cond, env) {
                return true;
            }
            let cond_value = self.eval(cond, env);
            return match self.bool_const(cond_value) {
                Some(true) => kids
                    .get(1)
                    .is_some_and(|&then_expr| self.expr_is_static_runtime_err(then_expr, env)),
                Some(false) => kids
                    .get(2)
                    .is_some_and(|&else_expr| self.expr_is_static_runtime_err(else_expr, env)),
                None => false,
            };
        }
        if self.il.kind(expr) == NodeKind::Call && self.call_has_static_runtime_arg_err(expr, env) {
            return true;
        }
        if self.il.kind(expr) == NodeKind::UnOp {
            return self
                .il
                .children(expr)
                .first()
                .is_some_and(|&operand| self.expr_is_static_runtime_err(operand, env));
        }
        if self.il.kind(expr) == NodeKind::Field {
            return self
                .il
                .children(expr)
                .first()
                .is_some_and(|&receiver| self.expr_is_static_runtime_err(receiver, env));
        }
        if self.il.kind(expr) == NodeKind::Index {
            let kids = self.il.children(expr).to_vec();
            if kids.len() != 2 {
                return false;
            }
            if self.expr_is_static_runtime_err(kids[0], env) {
                return true;
            }
            return crate::is_pure(self.il, kids[0])
                && self.expr_is_static_runtime_err(kids[1], env);
        }
        if self.il.kind(expr) != NodeKind::BinOp {
            return false;
        }
        let kids = self.il.children(expr).to_vec();
        if kids.len() != 2 {
            return false;
        }
        let Payload::Op(op) = self.il.node(expr).payload else {
            return false;
        };
        if self.expr_is_static_runtime_err(kids[0], env) {
            return true;
        }
        if !crate::is_pure(self.il, kids[0]) {
            return false;
        }
        if self.expr_is_static_runtime_err(kids[1], env) {
            return true;
        }
        if !crate::is_pure(self.il, kids[1]) {
            return false;
        }
        let rhs = self.eval(kids[1], env);
        match op {
            Op::Div | Op::Mod => self.int_const_eq(rhs, 0),
            Op::Pow => self
                .static_int_expr(kids[1])
                .is_some_and(|exp| !(0..=u32::MAX as i64).contains(&exp)),
            _ => false,
        }
    }

    pub(super) fn call_has_static_runtime_arg_err(
        &mut self,
        call: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let kids = self.il.children(call).to_vec();
        match self.il.node(call).payload {
            Payload::Builtin(Builtin::ValueOrDefault)
                if self.admitted_builtin_call(call, Builtin::ValueOrDefault) =>
            {
                kids.first()
                    .is_some_and(|&value| self.expr_is_static_runtime_err(value, env))
            }
            Payload::Builtin(Builtin::Any) if self.admitted_builtin_call(call, Builtin::Any) => {
                kids.first()
                    .is_some_and(|&coll| self.expr_is_static_runtime_err(coll, env))
            }
            Payload::Builtin(Builtin::All) if self.admitted_builtin_call(call, Builtin::All) => {
                kids.first()
                    .is_some_and(|&coll| self.expr_is_static_runtime_err(coll, env))
            }
            Payload::Builtin(Builtin::Reduce)
                if self.admitted_builtin_call(call, Builtin::Reduce) =>
            {
                kids.get(1)
                    .is_some_and(|&coll| self.expr_is_static_runtime_err(coll, env))
                    || kids
                        .get(2)
                        .is_some_and(|&init| self.expr_is_static_runtime_err(init, env))
                    || (kids
                        .get(1)
                        .is_some_and(|&coll| self.expr_is_static_non_empty_seq(coll))
                        && kids.first().is_some_and(|&lambda| {
                            self.lambda_body_is_static_runtime_err(lambda, env)
                        }))
            }
            Payload::Builtin(Builtin::Range)
                if self.admitted_builtin_call(call, Builtin::Range) =>
            {
                self.call_args_have_static_runtime_err(kids.iter().copied(), env)
                    || self.range_has_static_zero_step(&kids)
            }
            Payload::Builtin(_) => self.call_args_have_static_runtime_err(kids, env),
            _ => self.call_args_have_static_runtime_err(kids.into_iter().skip(1), env),
        }
    }

    pub(super) fn range_has_static_zero_step(&self, kids: &[NodeId]) -> bool {
        kids.len() == 3
            && kids.iter().all(|&arg| crate::is_pure(self.il, arg))
            && self.static_int_expr(kids[2]) == Some(0)
    }

    pub(super) fn call_args_have_static_runtime_err<I>(
        &mut self,
        args: I,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool
    where
        I: IntoIterator<Item = NodeId>,
    {
        for arg in args {
            if self.expr_is_static_runtime_err(arg, env) {
                return crate::is_pure(self.il, arg);
            }
            if !crate::is_pure(self.il, arg) {
                return false;
            }
        }
        false
    }

    pub(super) fn expr_is_static_non_empty_seq(&self, expr: NodeId) -> bool {
        self.il.kind(expr) == NodeKind::Seq && !self.il.children(expr).is_empty()
    }

    pub(super) fn lambda_body_is_static_runtime_err(
        &mut self,
        lambda: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return false;
        }
        self.il
            .children(lambda)
            .last()
            .is_some_and(|&body| self.is_effect_free_static_err_body(body, env))
    }

    pub(super) fn static_int_expr(&self, expr: NodeId) -> Option<i64> {
        let node = self.il.node(expr);
        match (node.kind, node.payload) {
            (NodeKind::Lit, Payload::LitInt(value)) => Some(value),
            (NodeKind::UnOp, Payload::Op(Op::Pos)) => self
                .il
                .children(expr)
                .first()
                .and_then(|&child| self.static_int_expr(child)),
            (NodeKind::UnOp, Payload::Op(Op::Neg)) => self
                .il
                .children(expr)
                .first()
                .and_then(|&child| self.static_int_expr(child))
                .and_then(i64::checked_neg),
            _ => None,
        }
    }

    /// Walk a container (class/module body), folding each contained method's behavior
    /// into the current sinks. A `Func` is processed in its own parameter scope (its
    /// returns/effects become the container's), so the container's fingerprint is the
    /// aggregate of its methods; `Block` wrappers are descended; anything else (field
    /// initializers, attribute assigns) is processed as a normal statement.
    pub(super) fn process_container(&mut self, node: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        for c in self.il.children(node).to_vec() {
            match self.il.kind(c) {
                NodeKind::Func => {
                    let kids = self.il.children(c).to_vec();
                    let mut menv = env.clone();
                    let saved_param_domain = self.param_domain.clone();
                    let saved_param_ty = self.param_ty.clone();
                    self.param_domain.clear();
                    self.seed_param_domains(c);
                    self.seed_param_value_domains(c);
                    let mut pos = 0u32;
                    for &k in &kids {
                        if self.il.kind(k) == NodeKind::Param {
                            if let Payload::Cid(cid) = self.il.node(k).payload {
                                let v = self.mk(ValOp::Input(pos), vec![]);
                                menv.insert(cid, v);
                                pos += 1;
                            }
                        }
                    }
                    if let Some(&body) = kids.last() {
                        self.process_stmt(body, &mut menv);
                    }
                    self.param_domain = saved_param_domain;
                    self.param_ty = saved_param_ty;
                }
                NodeKind::Block => self.process_container(c, env),
                NodeKind::Assign => self.process_container_assignment(c, env),
                _ => self.process_stmt(c, env),
            }
        }
    }

    pub(super) fn process_container_assignment(
        &mut self,
        stmt: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        let binding = self.il.children(stmt).first().copied().and_then(|lhs| {
            match (self.il.kind(lhs), self.il.node(lhs).payload) {
                (NodeKind::Var, Payload::Cid(cid)) => self
                    .il
                    .cid_names
                    .get(cid as usize)
                    .copied()
                    .map(|name| (cid, name)),
                _ => None,
            }
        });
        self.process_stmt(stmt, env);
        let Some((cid, name)) = binding else {
            return;
        };
        if let Some(&value) = env.get(&cid) {
            self.global_env.insert(name, value);
        }
    }

    pub(super) fn compact_coupled_recurrence(&mut self, cid: u32, value: ValueId) -> ValueId {
        let should_compact = self.loop_recurrence.as_ref().is_some_and(|scope| {
            scope.loop_values.contains_key(&cid)
                && self.references_nonself_loop_dependency(value, cid, scope)
        });
        if !should_compact {
            return value;
        }

        let key = self
            .loop_recurrence
            .as_ref()
            .and_then(|scope| scope.loop_keys.get(&cid))
            .copied()
            .unwrap_or(cid);
        let h = combine(combine(0xC0AD_D1EC, key as u64), self.vhash[value as usize]);
        self.mk(ValOp::Recurrence(h), vec![])
    }

    pub(super) fn references_nonself_loop_dependency(
        &self,
        value: ValueId,
        self_cid: u32,
        scope: &LoopRecurrenceScope,
    ) -> bool {
        let self_key = scope.loop_keys.get(&self_cid).copied();
        let mut stack = vec![value];
        let mut seen = FxHashSet::default();
        while let Some(v) = stack.pop() {
            if !seen.insert(v) {
                continue;
            }
            match &self.nodes[v as usize].op {
                ValOp::Loop(key) if Some(*key) != self_key && scope.loop_key_set.contains(key) => {
                    return true;
                }
                ValOp::Recurrence(_) => return true,
                _ => {}
            }
            stack.extend(self.nodes[v as usize].args.iter().copied());
        }
        false
    }

    pub(super) fn process_stmt(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        match self.il.kind(stmt) {
            NodeKind::Block => self.process_block(stmt, env),
            NodeKind::Assign => {
                let kids = self.il.children(stmt).to_vec();
                if kids.len() == 2 {
                    // Go-style functional append `r = append(r, item)` to an ACTIVE builder var
                    // IS the per-element build (the reassignment is the append), exactly like
                    // the effect-form `r.append(item)`. Record the contribution so the loop
                    // becomes `Map(elem, contrib)` instead of an opaque reassign.
                    if self.try_record_reassign_append(kids[0], kids[1], env) {
                        return;
                    }
                    let rhs = self.eval(kids[1], env);
                    if self.il.kind(kids[0]) == NodeKind::Var {
                        if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                            let rhs = self.compact_coupled_recurrence(c, rhs);
                            env.insert(c, rhs);
                            return;
                        }
                    }
                    // An evidence-backed exact field write updates per-place state
                    // (last-write-wins), flushed as a (receiver, field, final-value)
                    // sink later — order-insensitive across distinct places, correct
                    // for same-place overwrites.
                    if let Some(key) = self.exact_field_write_state_key(stmt, kids[0]) {
                        let g = self.guarded(rhs);
                        self.field_env.insert(key, g);
                        return;
                    }
                    // `d[k] = v` to an ACTIVE dict-builder records a `DictEntry` contribution
                    // (so the loop becomes a `Map` of entries, converging with `{k:v for x}`).
                    if self.try_record_index_assign(stmt, rhs, env) {
                        return;
                    }
                    // store into an index / computed target → an ordered effect
                    let target = self.eval(kids[0], env);
                    let st = self.mk(ValOp::Call(0), vec![target, rhs]);
                    self.push_effect(st);
                }
            }
            NodeKind::Return => {
                // A bare `return;` (no value) is still behaviorally significant: as an
                // *early* exit inside a loop/branch it changes which later code runs, and
                // its path guard distinguishes a conditional early-return from an
                // unconditional one. Model it as a guarded void-return sink (previously a
                // valueless return pushed nothing, so two functions differing only in a
                // conditional early `return;` collapsed — cf. the break sink, §AS).
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.mk(ValOp::Const(0xF00D_0000), vec![]),
                };
                self.emit_return(v);
            }
            NodeKind::Throw => {
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.mk(ValOp::Const(0xE22E_0000), vec![]),
                };
                self.emit_throw(v);
            }
            NodeKind::ExprStmt => {
                if let Some(&e) = self.il.children(stmt).first() {
                    if matches!(
                        self.il.kind(e),
                        NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
                    ) {
                        self.process_stmt(e, env);
                        return;
                    }
                    // A `coll.append(x)` to an ACTIVE local builder var records the
                    // per-element contribution (so the loop becomes a `Map`) instead of an
                    // opaque effect. Anything irregular (2nd append, multi-arg) spoils it.
                    if self.try_record_append(e, env) {
                        return;
                    }
                    let v = self.eval(e, env);
                    // an expression statement is kept only if it has effect value
                    self.push_effect(v);
                }
            }
            NodeKind::If => self.process_if(stmt, env),
            NodeKind::Loop => self.process_loop(stmt, env),
            NodeKind::Try => {
                let kids = self.il.children(stmt).to_vec();
                if kids.len() == 2 && self.is_effect_free_static_err_body(kids[0], env) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                if kids.len() == 2 && self.branch_returns(kids[0]) {
                    self.process_stmt(kids[0], env);
                    return;
                }
                if kids.len() == 2 && self.is_effect_free_throw_body(kids[0]) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                for c in kids {
                    self.process_stmt(c, env);
                }
            }
            NodeKind::Break => {
                // An early `break` truncates the loop to a prefix — the result is NOT
                // the full-iteration fold. Record the break's *path condition* as a sink
                // so an early-exit loop no longer fingerprints identically to one that
                // runs to completion, and two loops breaking on different conditions stay
                // distinct. (`continue` needs no handling: desugaring already hoists the
                // remainder of the body into the negated guard, so its filtering effect
                // is captured by the normal path-guard machinery.)
                let marker = self.mk(ValOp::Const(0xB2EA_C0DE), vec![]);
                let g = self.guarded(marker);
                self.sinks.push(Sink::new(SinkKind::Break, g));
            }
            NodeKind::Continue => {}
            _ => {
                // unknown statement: evaluate as effect best-effort
                let v = self.eval(stmt, env);
                self.push_effect(v);
            }
        }
    }

    pub(super) fn process_if(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(stmt).to_vec();
        if kids.is_empty() {
            return;
        }
        // The condition is *not* a standalone sink: it is captured where it matters —
        // in the `Phi` merge of any variable the branches update, and in the path-guard
        // of any return/effect they perform. Emitting it separately would make a
        // statement-`if` that updates a variable (`if c { x = a }`) diverge from the
        // equivalent ternary (`x = a if c else x`), which has no such sink. (`cond` is
        // still evaluated — for the path stack and so its sub-values are interned.)
        let cond = self.eval(kids[0], env);
        let effect_slot_base = self.effect_slot;

        let mut env_then = env.clone();
        let then_effect_slot = if kids.len() >= 2 {
            self.effect_slot = effect_slot_base;
            self.path.push(cond);
            self.process_stmt(kids[1], &mut env_then);
            self.path.pop();
            self.effect_slot
        } else {
            effect_slot_base
        };
        let mut env_else = env.clone();
        let else_effect_slot = if kids.len() >= 3 {
            self.effect_slot = effect_slot_base;
            let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
            self.path.push(ncond);
            self.process_stmt(kids[2], &mut env_else);
            self.path.pop();
            self.effect_slot
        } else {
            effect_slot_base
        };
        self.effect_slot = then_effect_slot.max(else_effect_slot);

        // Merge: for each var that differs across branches, insert a Phi.
        let mut keys: Vec<u32> = env_then.keys().chain(env_else.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        for cid in keys {
            let base = env.get(&cid).copied();
            let t = env_then.get(&cid).copied().or(base);
            let e = env_else.get(&cid).copied().or(base);
            match (t, e) {
                (Some(tv), Some(ev)) if tv == ev => {
                    env.insert(cid, tv);
                }
                (Some(tv), Some(ev)) => {
                    let phi = self.mk(ValOp::Phi, vec![cond, tv, ev]);
                    env.insert(cid, phi);
                }
                (Some(v), None) | (None, Some(v)) => {
                    env.insert(cid, v);
                }
                (None, None) => {}
            }
        }
    }

    pub(super) fn process_loop(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(stmt).to_vec();
        let kind = match self.il.node(stmt).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let body = match kids.last() {
            Some(&b) => b,
            None => return,
        };
        if kind == LoopKind::While
            && kids.len() == 2
            && self.loop_entry_condition_is_proven_false(kids[0], env)
        {
            return;
        }

        // Discover the loop's *element source* so per-element computations converge
        // across loop shapes (the §AH representation axis):
        //   • `for pat in iterable`        → element is the pattern variable;
        //   • `while i < len(xs) { … i+=1 }` → element is `xs[i]` for the induction
        //     variable `i` (index bookkeeping is iteration mechanics, not a result).
        // Both bind a single canonical `Elem(iterable)` value; an indexed `while`
        // additionally rewrites `xs[i]` → `Elem(xs)` and drops the induction variable.
        // Bindings applied to the loop's pattern/index variables (cid → value), and the
        // set of values that play the role of an *index*. Any `C[idx]` for such an
        // `idx` is the element of `C`, so indexed iteration converges with value
        // iteration. Iteration variables are not accumulators.
        let mut pattern_bindings: Vec<(u32, ValueId)> = Vec::new();
        let mut index_vals: FxHashSet<ValueId> = FxHashSet::default();
        let mut induction: FxHashSet<u32> = FxHashSet::default();

        match kind {
            LoopKind::ForEach if kids.len() >= 3 => {
                let pat = kids[0];
                let it = kids[1];
                if let Some(c) = self.range_len_collection(it) {
                    // `for i in range(len(C))`: `i` is a canonical index into `C`.
                    let cv = self.eval(c, env);
                    let ix = self.idx(cv);
                    index_vals.insert(ix);
                    for cid in self.pattern_cids(pat) {
                        pattern_bindings.push((cid, ix));
                    }
                } else if self.il.kind(it) == NodeKind::Call
                    && matches!(
                        self.il.node(it).payload,
                        Payload::Builtin(Builtin::Enumerate)
                    )
                    && self.admitted_builtin_call(it, Builtin::Enumerate)
                {
                    // `for i, x in enumerate(C)`: `i` is the index, `x` the element.
                    let cids = self.pattern_cids(pat);
                    if let Some(&cnode) = self.il.children(it).first() {
                        let cv = self.eval(cnode, env);
                        let ix = self.idx(cv);
                        let el = self.elem(cv);
                        index_vals.insert(ix);
                        if cids.len() >= 2 {
                            pattern_bindings.push((cids[0], ix));
                            pattern_bindings.push((cids[1], el));
                        } else if let Some(&only) = cids.first() {
                            pattern_bindings.push((only, el));
                        }
                    }
                } else {
                    // Value iteration `for x in C` (or `for i in range(n)`): the
                    // pattern var is an element of the iterable. NOTE: we do *not* treat
                    // this element as a collection index — only a provably-*full* range
                    // (`range_len_collection`, above) licenses `C[i] → Elem(C)`. A bare
                    // `range(n)` or partial `range(1, len)` indexing `C[i]` must keep its
                    // `Index(C, …)` so it stays distinct from the full-range loop (else a
                    // subset sum merges with the full sum — a soundness bug).
                    let iv = self.eval(it, env);
                    let e = self.elem(iv);
                    for cid in self.pattern_cids(pat) {
                        pattern_bindings.push((cid, e));
                    }
                }
            }
            _ => {
                let cond = kids
                    .first()
                    .filter(|&&c| self.il.kind(c) != NodeKind::Block);
                // A genuine loop counter both steps by a constant *and* governs the
                // loop condition. An accumulator updated by `acc = acc + 1` (a counting
                // reduction) matches the `i = i ± c` shape too, so `induction_vars`
                // alone misclassifies it as iteration mechanics — which binds it to the
                // index and destroys the reduction (it would never reach a `Reduce`).
                // Intersect with the variables the condition actually mentions so only
                // the real counter(s) are treated as indices; the accumulator stays an
                // accumulator. (Sum loops were spared by luck — `sum += xs[i]` has a
                // non-literal operand, so `is_increment` already rejected them; counting
                // loops like `if p: count += 1` were the ones that collapsed.)
                let cond_cids = cond
                    .map(|&c| mentioned_cids(self.il, c))
                    .unwrap_or_default();
                induction = induction_vars(self.il, body)
                    .intersection(&cond_cids)
                    .copied()
                    .collect();
                let iter_node =
                    cond.and_then(|&c| self.loop_iterable(c, &induction).map(|it| (it, None)));
                let indexed_bound_loop = iter_node.or_else(|| {
                    cond.and_then(|&c| {
                        self.indexed_bound_loop_iterable(c, body, &induction)
                            .map(|(it, bound, cmp)| (it, Some((bound, cmp))))
                    })
                });
                match indexed_bound_loop {
                    // Indexed `while i < len(C)`: the raw `i < len(C)` guard is iteration
                    // mechanics. A counter that steps by +1 from 0 visits *every* index
                    // in order, so `C[i]` is the canonical `Elem(C)` (converges with `for
                    // x in C`). A non-unit stride (`i += 2`) or non-zero start (`i = 1`)
                    // visits a SUBSET — `C[i]` is NOT `Elem(C)` — so bind `i` to a strided
                    // index that encodes start+step (distinct strides stay distinct) and
                    // do NOT license the `C[i] → Elem(C)` rewrite (else a strided sum
                    // merges with the full sum — the while-loop analog of the range bug).
                    Some((it, bound_guard)) if !induction.is_empty() => {
                        let cv = self.eval(it, env);
                        if let Some((bound, cmp)) = bound_guard {
                            let bv = self.eval(bound, env);
                            if !self.full_pointer_length_contract(cmp, cv, bv) {
                                let gv = self.indexed_bound_guard(cmp, bv);
                                self.sinks.push(Sink::new(SinkKind::Cond, gv));
                            } else if let (Some(arr), Some(len)) =
                                (self.input_key(cv), self.input_key(bv))
                            {
                                // The bound `n` was dropped as "length of the array" — record
                                // (array_pos, length_pos) so the oracle interprets under n=len.
                                self.contracts.push((arr, len));
                            }
                        }
                        let zero = self.int_const(0);
                        for &i in &induction {
                            let step = induction_step(self.il, body, i);
                            let start_zero = env.get(&i).is_some_and(|&s| s == zero);
                            if step == Some(1) && start_zero {
                                let ix = self.idx(cv);
                                index_vals.insert(ix);
                                pattern_bindings.push((i, ix));
                            } else {
                                let start_val = env.get(&i).copied().unwrap_or(zero);
                                let step_val =
                                    self.int_const(step.unwrap_or(0).rem_euclid(1 << 24) as u32);
                                let base = self.idx(cv);
                                let h = combine(
                                    self.vhash[base as usize],
                                    combine(
                                        self.vhash[start_val as usize],
                                        self.vhash[step_val as usize],
                                    ),
                                );
                                let strided =
                                    self.mk(ValOp::Idx(h), vec![base, start_val, step_val]);
                                pattern_bindings.push((i, strided));
                            }
                        }
                    }
                    // Plain `while`/other: keep the raw condition; no element model.
                    _ => {
                        induction.clear();
                        if let Some(&c) = cond {
                            let cv = self.eval(c, env);
                            self.sinks.push(Sink::new(SinkKind::Cond, cv));
                        }
                    }
                }
            }
        }

        let mut assigned = FxHashSet::default();
        collect_assigned(self.il, body, &mut assigned);
        // List-builder vars (incl. Go's `r = append(r, …)`, which makes `r` assigned) are
        // activated as builders, NOT seeded as numeric loop-carried recurrences — otherwise a
        // Go builder var would be both a `Loop` placeholder and a `Map` build and collapse. The
        // seed (empty-collection) check reads the PRE-loop `env` (the real `[]` seed; the body's
        // reassignment would otherwise hide it). Builders are excluded from `carried`.
        let builder_cands = self.builder_candidates(body, env);
        let builder_set: FxHashSet<u32> = builder_cands
            .iter()
            .map(|candidate| candidate.cid)
            .collect();
        for candidate in &builder_cands {
            self.building.insert(candidate.cid, None);
            self.building_kind.insert(candidate.cid, candidate.kind);
        }
        let mut carried: Vec<u32> = assigned
            .iter()
            .copied()
            .filter(|c| !builder_set.contains(c))
            .collect();
        carried.sort_unstable();

        // Seed each loop-carried variable with a symbolic "previous iteration" value
        // so the body expresses its update as a *recurrence* over `Loop(cid)`.
        let mut body_env = env.clone();
        let mut loop_vals: FxHashMap<u32, ValueId> = FxHashMap::default();
        let loop_key_base = self.next_loop_key_base;
        let loop_key_count = u32::try_from(carried.len()).unwrap_or(u32::MAX);
        self.next_loop_key_base = self
            .next_loop_key_base
            .wrapping_add(loop_key_count.saturating_add(1));
        let mut loop_keys: FxHashMap<u32, u32> = FxHashMap::default();
        let mut loop_key_set: FxHashSet<u32> = FxHashSet::default();
        for (slot, &cid) in carried.iter().enumerate() {
            let key = loop_key_base.wrapping_add(slot as u32);
            loop_keys.insert(cid, key);
            loop_key_set.insert(key);
            let lv = self.mk(ValOp::Loop(key), vec![]);
            loop_vals.insert(cid, lv);
            body_env.insert(cid, lv);
        }
        // Pattern/index bindings override the `Loop` placeholder for iteration vars.
        let iter_vars: FxHashSet<u32> = pattern_bindings.iter().map(|&(c, _)| c).collect();
        for &(cid, v) in &pattern_bindings {
            body_env.insert(cid, v);
        }

        // Every `C[idx]` for an index value `idx` is morally `Elem(C)`. Rewrite it
        // everywhere the body deposits values — the sinks it pushes (guard conditions,
        // effects) and the carried recurrences — so indexed iteration (`while i<len`,
        // `for i in range(len)`, multi-collection `a[i]*b[i]`) matches value iteration,
        // even when the accumulation is conditional (filter+reduce) not a clean fold.
        // (List-builder vars were activated above, before `carried`, so a Go functional-append
        // builder is excluded from numeric recurrence seeding.)
        let sink_start = self.sinks.len();
        let outer_recurrence = self.loop_recurrence.replace(LoopRecurrenceScope {
            loop_values: loop_vals.clone(),
            loop_keys,
            loop_key_set,
        });
        let pre_body_env = body_env.clone();
        self.process_stmt(body, &mut body_env);
        self.loop_recurrence = outer_recurrence;
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            for idx in sink_start..self.sinks.len() {
                let v = self.sinks[idx].value;
                self.sinks[idx].value = self.rewrite_indices(v, &index_vals, &mut memo);
            }
            for &cid in &carried {
                if let Some(&v) = body_env.get(&cid) {
                    let nv = self.rewrite_indices(v, &index_vals, &mut memo);
                    body_env.insert(cid, nv);
                }
            }
        }
        let mut flag_break_reduction = None;
        for &cid in &carried {
            if let Some(&init) = env.get(&cid) {
                if let Some(v) =
                    self.flag_break_reduction(body, cid, init, &pre_body_env, &index_vals)
                {
                    flag_break_reduction = Some((cid, v));
                    self.sinks.truncate(sink_start);
                    break;
                }
            }
        }
        // Finalize list builders: `r = []; for x: r.append(f(x))` → `r = Map(elem, f(x))`,
        // converging the loop with the comprehension `[f(x) for x in xs]` / `.map`. A
        // guarded append (`if cond: r.append(f(x))`) becomes the filtered map `Map(_, pred)`.
        // If the append happens inside a nested loop, the inner loop has already produced a
        // `Map`/`FlatMap`; wrap that per-outer-iteration collection in a `FlatMap` so
        // `for x: for y: r.append(f(x,y))` converges with `[f(x,y) for x in xs for y in ys]`.
        for candidate in &builder_cands {
            let c = candidate.cid;
            if let Some(Some((mut contrib, guard))) = self.building.remove(&c) {
                let map = if !index_vals.is_empty() {
                    let mut memo = FxHashMap::default();
                    contrib = self.rewrite_indices(contrib, &index_vals, &mut memo);
                    match guard {
                        Some(g) => {
                            let g = self.rewrite_indices(g, &index_vals, &mut memo);
                            self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, g])
                        }
                        None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                    }
                } else {
                    match guard {
                        Some(g) => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, g]),
                        None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                    }
                };
                env.insert(c, map);
            } else if let Some(&nested) = body_env.get(&c) {
                if let Some(flat) = self.flat_map_builder_value(nested, &pattern_bindings) {
                    env.insert(c, flat);
                }
                self.building.remove(&c);
            } else {
                self.building.remove(&c);
            }
            self.building_kind.remove(&c);
        }

        // For each loop-carried accumulator, recognize an associative-commutative
        // reduction `acc = acc ⊕ contrib` and canonicalize it to a `Reduce` value —
        // so sum/product/min/max/count loops converge regardless of loop shape,
        // accumulator name, or operand grouping (the §AH representation axis). The
        // per-element `contrib` keys the value, so a `+`-loop and a `*`-loop (or
        // `a[i]*b[i]` vs `a[i]+b[i]`) stay distinct (the behavior axis). When the
        // update is not a clean reduction, thread the raw recurrence (still better
        // than a bare opaque `Loop` value reaching the sinks).
        let mut reduction_cache = ReductionCache::default();
        for &cid in &carried {
            if let Some((flag_cid, v)) = flag_break_reduction {
                if flag_cid == cid {
                    env.insert(cid, v);
                    continue;
                }
            }
            if let Some(&init) = env.get(&cid) {
                if let Some(v) =
                    self.ordered_string_concat_loop(body, cid, init, &pre_body_env, &index_vals)
                {
                    env.insert(cid, v);
                    continue;
                }
            }
            if iter_vars.contains(&cid) || induction.contains(&cid) {
                continue; // iteration mechanics, not an accumulator
            }
            let Some(&newv) = body_env.get(&cid) else {
                continue;
            };
            let loopv = loop_vals[&cid];
            let loop_context: Vec<ValueId> = pattern_bindings.iter().map(|&(_, v)| v).collect();
            match (
                env.get(&cid).copied(),
                self.as_loop_reduction_step(newv, loopv, &loop_context, &mut reduction_cache),
            ) {
                (Some(init), Some((op, contrib))) => {
                    // Selection reductions (min/max) carry no init; folds carry one.
                    let args = if is_selection_code(op) {
                        vec![contrib]
                    } else {
                        vec![init, contrib]
                    };
                    let red = self.mk(ValOp::Reduce(op), args);
                    env.insert(cid, red);
                }
                (init, _) => {
                    // A non-reduction loop-carried value still depends on its pre-loop
                    // SEED. The compact `Recurrence` key is the per-iteration update
                    // expression ONLY, so `acc = a` (a parameter seed, the loop returning
                    // `a + Σ`) collapsed onto `acc = 0` (returning `Σ`) — a false merge,
                    // since the two differ exactly by that seed. Re-key the recurrence on
                    // the seed as well so the seed reaches the fingerprint. (Clean
                    // reductions above already carry their `init`.)
                    let v = match (init, self.nodes[newv as usize].op.clone()) {
                        (Some(init), ValOp::Recurrence(h)) => {
                            self.mk(ValOp::Recurrence(h), vec![init])
                        }
                        _ => newv,
                    };
                    env.insert(cid, v);
                }
            }
        }
    }

    pub(super) fn flag_break_reduction(
        &mut self,
        body: NodeId,
        cid: u32,
        init: ValueId,
        env: &FxHashMap<u32, ValueId>,
        index_vals: &FxHashSet<ValueId>,
    ) -> Option<ValueId> {
        let init_bool = self.bool_const(init)?;
        let (cond_node, assigned_bool) = self.flag_break_if(body, cid)?;
        if init_bool == assigned_bool {
            return None;
        }
        let mut cond = self.eval(cond_node, env);
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            cond = self.rewrite_indices(cond, index_vals, &mut memo);
        }
        if !self.refs_elem(cond) {
            return None;
        }
        if !init_bool && assigned_bool {
            Some(self.mk(ValOp::Reduce(REDUCE_ANY), vec![cond]))
        } else {
            let pred = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
            Some(self.mk(ValOp::Reduce(REDUCE_ALL), vec![pred]))
        }
    }

    pub(super) fn flat_map_builder_value(
        &mut self,
        value: ValueId,
        pattern_bindings: &[(u32, ValueId)],
    ) -> Option<ValueId> {
        let outer_elem = pattern_bindings.first()?.1;
        let op = self.nodes[value as usize].op.clone();
        match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 || k == HoFKind::FlatMap as u32 => {
                Some(self.mk(ValOp::Hof(HoFKind::FlatMap as u32), vec![outer_elem, value]))
            }
            _ => None,
        }
    }

    pub(super) fn flag_break_if(&self, body: NodeId, cid: u32) -> Option<(NodeId, bool)> {
        let stmts = self.direct_block_statements(body);
        if stmts.len() != 1 || self.il.kind(stmts[0]) != NodeKind::If {
            return None;
        }
        let if_kids = self.il.children(stmts[0]);
        if if_kids.len() != 2 {
            return None;
        }
        let branch = self.direct_block_statements(if_kids[1]);
        if branch.len() != 2 || self.il.kind(branch[1]) != NodeKind::Break {
            return None;
        }
        Some((if_kids[0], self.flag_assignment(branch[0], cid)?))
    }

    pub(super) fn ordered_string_concat_loop(
        &mut self,
        body: NodeId,
        cid: u32,
        init: ValueId,
        env: &FxHashMap<u32, ValueId>,
        index_vals: &FxHashSet<ValueId>,
    ) -> Option<ValueId> {
        if !self.is_empty_string_value(init) {
            return None;
        }
        let contrib_node = self.ordered_concat_contribution(body, cid)?;
        let mut contrib = self.eval(contrib_node, env);
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            contrib = self.rewrite_indices(contrib, index_vals, &mut memo);
        }
        if !self.refs_elem(contrib) {
            return None;
        }
        let sep = self.empty_string_value();
        Some(self.mk(ValOp::Reduce(ORDERED_STRING_JOIN), vec![sep, contrib]))
    }

    pub(super) fn ordered_concat_contribution(&self, body: NodeId, cid: u32) -> Option<NodeId> {
        let stmts = self.direct_block_statements(body);
        if stmts.len() != 1 || self.il.kind(stmts[0]) != NodeKind::Assign {
            return None;
        }
        let kids = self.il.children(stmts[0]);
        if kids.len() != 2 || !self.is_var_cid(kids[0], cid) {
            return None;
        }
        if self.il.kind(kids[1]) != NodeKind::BinOp
            || op_code(self.il.node(kids[1]).payload) != Op::Add as u32
        {
            return None;
        }
        let add = self.il.children(kids[1]);
        if add.len() != 2 || !self.is_var_cid(add[0], cid) {
            return None;
        }
        if mentioned_cids(self.il, add[1]).contains(&cid) {
            return None;
        }
        Some(add[1])
    }

    pub(super) fn direct_block_statements(&self, node: NodeId) -> Vec<NodeId> {
        if self.il.kind(node) == NodeKind::Block {
            self.il.children(node).to_vec()
        } else {
            vec![node]
        }
    }

    pub(super) fn flag_assignment(&self, stmt: NodeId, cid: u32) -> Option<bool> {
        if self.il.kind(stmt) != NodeKind::Assign {
            return None;
        }
        let kids = self.il.children(stmt);
        if kids.len() != 2 || !self.is_var_cid(kids[0], cid) {
            return None;
        }
        match self.il.node(kids[1]).payload {
            Payload::LitBool(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn is_var_cid(&self, node: NodeId, cid: u32) -> bool {
        matches!(
            (self.il.kind(node), self.il.node(node).payload),
            (NodeKind::Var, Payload::Cid(c)) if c == cid
        )
    }

    pub(super) fn loop_entry_condition_is_proven_false(
        &self,
        cond: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.condition_atom_is_proven_false(cond, env) {
            return true;
        }
        if self.il.kind(cond) != NodeKind::BinOp
            || op_code(self.il.node(cond).payload) != Op::And as u32
        {
            return false;
        }
        let kids = self.il.children(cond);
        kids.len() == 2 && self.condition_atom_is_proven_false(kids[0], env)
    }

    pub(super) fn condition_atom_is_proven_false(
        &self,
        atom: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        match self.il.node(atom).payload {
            Payload::LitBool(false) if self.il.kind(atom) == NodeKind::Lit => true,
            Payload::Cid(cid) if self.il.kind(atom) == NodeKind::Var => env
                .get(&cid)
                .and_then(|&v| self.bool_const(v))
                .is_some_and(|value| !value),
            Payload::Op(Op::Not) if self.il.kind(atom) == NodeKind::UnOp => {
                let kids = self.il.children(atom);
                if kids.len() != 1 {
                    return false;
                }
                match self.il.node(kids[0]).payload {
                    Payload::LitBool(true) if self.il.kind(kids[0]) == NodeKind::Lit => true,
                    Payload::Cid(cid) if self.il.kind(kids[0]) == NodeKind::Var => env
                        .get(&cid)
                        .and_then(|&v| self.bool_const(v))
                        .is_some_and(|value| value),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// The iterable of a `while i < len(xs)`-style loop: from a comparison whose
    /// bound side is `len(iterable)`, return the `iterable` node. Requires the other
    /// side to reference an induction variable (so we don't misread `a < len(b)`).
    pub(super) fn loop_iterable(&self, cond: NodeId, induction: &FxHashSet<u32>) -> Option<NodeId> {
        if self.il.kind(cond) != NodeKind::BinOp {
            return None;
        }
        let kids = self.il.children(cond).to_vec();
        if kids.len() != 2 {
            return None;
        }
        let mentions_ind = |n: NodeId| {
            matches!((self.il.kind(n), self.il.node(n).payload),
                (NodeKind::Var, Payload::Cid(c)) if induction.contains(&c))
        };
        if !kids.iter().any(|&k| mentions_ind(k)) {
            return None;
        }
        // The other operand is `len(iterable)` → a Len builtin Call with one arg.
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Call
                && matches!(self.il.node(k).payload, Payload::Builtin(Builtin::Len))
                && self.admitted_builtin_call(k, Builtin::Len)
            {
                if let Some(&arg) = self.il.children(k).first() {
                    return Some(arg);
                }
            }
        }
        None
    }

    /// Conservative C-style pointer+length loop recognition:
    ///
    /// `while i < n { ... xs[i] ...; i += 1 }`
    ///
    /// Unlike `i < len(xs)`, the bound is not intrinsically tied to the collection.
    /// Therefore this only licenses the local `xs[i] -> Elem(xs)` rewrite and records a
    /// bound guard keyed by the normalized comparison and bound value. That lets
    /// C `for`/`while` spellings of the same `(ptr, len)` traversal converge without
    /// claiming the loop is automatically identical to a high-level full-collection
    /// traversal.
    pub(super) fn indexed_bound_loop_iterable(
        &self,
        cond: NodeId,
        body: NodeId,
        induction: &FxHashSet<u32>,
    ) -> Option<(NodeId, NodeId, u32)> {
        if self.il.kind(cond) != NodeKind::BinOp {
            return None;
        }
        let cmp = op_code(self.il.node(cond).payload);
        let kids = self.il.children(cond);
        if kids.len() != 2 {
            return None;
        }

        let left_ind = self.direct_induction_cid(kids[0], induction);
        let right_ind = self.direct_induction_cid(kids[1], induction);
        let (cid, bound, normalized_cmp) = match (left_ind, right_ind) {
            (Some(cid), None) if !mentioned_cids(self.il, kids[1]).contains(&cid) => {
                (cid, kids[1], cmp)
            }
            (None, Some(cid)) if !mentioned_cids(self.il, kids[0]).contains(&cid) => {
                (cid, kids[0], reverse_cmp_code(self.il.meta.lang, cmp)?)
            }
            _ => return None,
        };
        if normalized_cmp != Op::Lt as u32 && normalized_cmp != Op::Le as u32 {
            return None;
        }

        let collection = self.indexed_collection_in_body(body, cid)?;
        Some((collection, bound, normalized_cmp))
    }

    pub(super) fn direct_induction_cid(
        &self,
        node: NodeId,
        induction: &FxHashSet<u32>,
    ) -> Option<u32> {
        match (self.il.kind(node), self.il.node(node).payload) {
            (NodeKind::Var, Payload::Cid(c)) if induction.contains(&c) => Some(c),
            _ => None,
        }
    }

    pub(super) fn indexed_collection_in_body(&self, node: NodeId, cid: u32) -> Option<NodeId> {
        if self.il.kind(node) == NodeKind::Index {
            let kids = self.il.children(node);
            if kids.len() == 2
                && matches!(
                    (self.il.kind(kids[1]), self.il.node(kids[1]).payload),
                    (NodeKind::Var, Payload::Cid(c)) if c == cid
                )
            {
                return Some(kids[0]);
            }
        }
        for &c in self.il.children(node) {
            if let Some(collection) = self.indexed_collection_in_body(c, cid) {
                return Some(collection);
            }
        }
        None
    }

    pub(super) fn indexed_bound_guard(&mut self, cmp: u32, bound: ValueId) -> ValueId {
        let marker = self.int_const(0xC10C_0000);
        let cmp_value = self.int_const(0xC10C_1000u32.wrapping_add(cmp));
        self.mk(ValOp::Call(0), vec![marker, cmp_value, bound])
    }

    pub(super) fn full_pointer_length_contract(
        &self,
        cmp: u32,
        collection: ValueId,
        bound: ValueId,
    ) -> bool {
        if cmp != Op::Lt as u32 {
            return false;
        }
        matches!(
            (self.input_key(collection), self.input_key(bound)),
            // Single pointer-length convention: `(xs, n)`.
            (Some(0), Some(1))
                // Two aligned pointer arrays with shared length: `(a, b, n)`.
                | (Some(0), Some(2))
                | (Some(1), Some(2))
        )
    }

    pub(super) fn input_key(&self, value: ValueId) -> Option<u32> {
        match self.nodes[value as usize].op {
            ValOp::Input(key) => Some(key),
            _ => None,
        }
    }

    /// Rewrite every `Index(C, idx)` whose index is in `index_vals` to `Elem(C)`,
    /// throughout `val`'s subgraph (DAG-safe, memoized). This is what makes indexed
    /// iteration converge with value iteration: `xs[i]` (any collection, any index
    /// variable) becomes the canonical element of that collection.
    pub(super) fn rewrite_indices(
        &mut self,
        val: ValueId,
        index_vals: &FxHashSet<ValueId>,
        memo: &mut FxHashMap<ValueId, ValueId>,
    ) -> ValueId {
        if let Some(&m) = memo.get(&val) {
            return m;
        }
        let (op, args) = {
            let n = &self.nodes[val as usize];
            (n.op.clone(), n.args.clone())
        };
        let new_args: Vec<ValueId> = args
            .iter()
            .map(|&a| self.rewrite_indices(a, index_vals, memo))
            .collect();
        // `C[idx]` with an index-role `idx` → `Elem(C)`.
        let r = if matches!(op, ValOp::Index)
            && new_args.len() == 2
            && index_vals.contains(&new_args[1])
        {
            self.elem(new_args[0])
        } else if new_args == args {
            val
        } else {
            self.mk(op, new_args)
        };
        memo.insert(val, r);
        r
    }

    /// Recognize an accumulator update `acc = acc ⊕ contrib` (⊕ associative and
    /// commutative) where `acc` is the previous-iteration value `loopv`. Returns the
    /// operator code and the canonical per-element `contrib` (with `acc` removed), or
    /// `None` if the update is not a single clean reduction step.
    pub(super) fn as_reduction(&mut self, val: ValueId, loopv: ValueId) -> Option<(u32, ValueId)> {
        let mut cache = ReductionCache::default();
        self.as_reduction_cached(val, loopv, &mut cache)
    }

    pub(super) fn as_loop_reduction_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        loop_context: &[ValueId],
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        if let ValOp::Reduce(op) = self.nodes[val as usize].op {
            let args = self.nodes[val as usize].args.clone();
            if is_selection_code(op)
                && args.len() == 1
                && !self.references_cached(args[0], loopv, cache)
                && self.references_any_cached(args[0], loop_context, cache)
            {
                return Some((op, args[0]));
            }
            if args.len() == 2
                && args[0] == loopv
                && !self.references_cached(args[1], loopv, cache)
                && self.references_any_cached(args[1], loop_context, cache)
            {
                return Some((op, args[1]));
            }
        }
        self.as_reduction_cached(val, loopv, cache)
    }

    pub(super) fn as_reduction_cached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let key = (val, loopv);
        if let Some(cached) = cache.reductions.get(&key).copied() {
            return cached;
        }
        let result = self.as_reduction_uncached(val, loopv, cache);
        cache.reductions.insert(key, result);
        result
    }

    pub(super) fn as_reduction_uncached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        // Guarded (filtered) reduction: `if cond { acc = acc ⊕ contrib }` merges to
        // `Phi(cond, ⊕(acc, contrib), acc)`. Canonicalize to `Reduce(⊕, init, cond ?
        // contrib : identity)` so a filtered loop converges with `sum(c for x if cond)`
        // and the per-element contribution becomes 0/1 (the op identity) when filtered.
        if matches!(self.nodes[val as usize].op, ValOp::Phi) {
            let args = self.nodes[val as usize].args.clone();
            if args.len() == 3 && args[2] == loopv {
                // (a) guarded accumulation: `if cond { acc = acc ⊕ contrib }`.
                if let Some((op, contrib)) = self
                    .as_reduction_cached(args[1], loopv, cache)
                    .or_else(|| self.nested_reduce_step(args[1], loopv, cache))
                {
                    if let Some(id) = identity_of(op) {
                        let ident = self.int_const(id);
                        let guarded = self.mk(ValOp::Phi, vec![args[0], contrib, ident]);
                        return Some((op, guarded));
                    }
                }
                // (b) selection (min/max): `if cand {>,<} acc { acc = cand }` —
                // the new value does not reference the old accumulator and the guard
                // compares the two. `acc = max(acc, cand)` / `min`.
                let cand = args[1];
                if !self.references_cached(cand, loopv, cache) {
                    if let Some(code) = self.selection_code(args[0], cand, loopv) {
                        return Some((code, cand));
                    }
                }
            }
            // Swapped polarity: `if cond { acc } else { acc ⊕ contrib }`. cfg_norm can
            // flip a two-branch ternary's orientation, so the accumulator lands in the
            // THEN branch with a negated guard — a `functools.reduce(lambda acc,v: acc+v
            // if v>0 else acc, …)` lowers to `if v<=0 { acc } else { acc+v }`. Recognize
            // it with the negated guard so it converges with the loop form `if v>0:
            // acc+=v` (whose single-branch guard stays positive).
            if args.len() == 3 && args[1] == loopv {
                if let Some((op, contrib)) = self
                    .as_reduction_cached(args[2], loopv, cache)
                    .or_else(|| self.nested_reduce_step(args[2], loopv, cache))
                {
                    if let Some(id) = identity_of(op) {
                        let ident = self.int_const(id);
                        let ncond = self.negate_guard(args[0]);
                        let guarded = self.mk(ValOp::Phi, vec![ncond, contrib, ident]);
                        return Some((op, guarded));
                    }
                }
            }
            // Full conditional contribution: both branches update the accumulator once,
            // e.g. `if x < 0 { total += -x } else { total += x }`. This is one reduction
            // whose per-element contribution is itself a branch value:
            // `Reduce(⊕, init, cond ? then_contrib : else_contrib)`. The `Phi` builder
            // then canonicalizes idioms such as `x < 0 ? -x : x` to `Abs(x)`.
            if args.len() == 3 {
                if let (Some((then_op, then_contrib)), Some((else_op, else_contrib))) = (
                    self.as_reduction_cached(args[1], loopv, cache),
                    self.as_reduction_cached(args[2], loopv, cache),
                ) {
                    if then_op == else_op {
                        let contrib =
                            self.mk(ValOp::Phi, vec![args[0], then_contrib, else_contrib]);
                        return Some((then_op, contrib));
                    }
                }
            }
            return None;
        }
        // A min/max accumulator written as a Min/Max node (`minmax_pattern` turned the
        // conditional update `if x>acc { acc=x }` into `Max(acc, x)`): map the idiom code
        // to the selection-reduction code so the loop converges with the `max()`/`min()`
        // builtin (both → `Reduce(REDUCE_MAX/MIN, [contrib])`).
        if let ValOp::Bin(o) = self.nodes[val as usize].op {
            if o == MIN_CODE || o == MAX_CODE {
                let a = self.nodes[val as usize].args.clone();
                let red = if o == MAX_CODE {
                    REDUCE_MAX
                } else {
                    REDUCE_MIN
                };
                if a[0] == loopv && !self.references_cached(a[1], loopv, cache) {
                    return Some((red, a[1]));
                }
                if a[1] == loopv && !self.references_cached(a[0], loopv, cache) {
                    return Some((red, a[0]));
                }
            }
        }
        let op = match self.nodes[val as usize].op {
            ValOp::Bin(o) if is_assoc_comm_code(o) => o,
            _ => return None,
        };
        let mut operands = Vec::new();
        self.flatten_into(val, op, &mut operands);
        // Exactly one top-level operand must be the previous accumulator, and it must
        // not reappear nested in the remaining contribution (`acc = acc + acc*x`).
        if operands.iter().filter(|&&o| o == loopv).count() != 1 {
            return None;
        }
        let pos = operands.iter().position(|&o| o == loopv)?;
        operands.remove(pos);
        if operands.is_empty() {
            return None;
        }
        for &operand in &operands {
            if self.references_cached(operand, loopv, cache) {
                return None;
            }
        }
        operands.sort_by_key(|&v| self.vhash[v as usize]);
        let mut acc = operands[0];
        for &o in &operands[1..] {
            acc = self.mk(ValOp::Bin(op), vec![acc, o]);
        }
        Some((op, acc))
    }

    pub(super) fn nested_reduce_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let ValOp::Reduce(op) = self.nodes[val as usize].op else {
            return None;
        };
        let args = self.nodes[val as usize].args.clone();
        if args.len() == 2 && args[0] == loopv && !self.references_cached(args[1], loopv, cache) {
            return Some((op, args[1]));
        }
        None
    }

    /// The canonical negation of a guard value: a comparison flips to its complement
    /// (`a<=b` → `a>b`, `a==b` → `a!=b`, …) — same operands, so a negated guard
    /// converges with the positive guard a loop produces — and anything else is wrapped
    /// in logical `Not`.
    pub(super) fn negate_guard(&mut self, v: ValueId) -> ValueId {
        if self.comparison_law_enabled(ComparisonLaw::Negation) {
            if let ValOp::Bin(opc) = self.nodes[v as usize].op {
                if let Some(flip) = negate_cmp_code(self.il.meta.lang, opc) {
                    let args = self.nodes[v as usize].args.clone();
                    return self.mk(ValOp::Bin(flip), args);
                }
            }
        }
        self.mk(ValOp::Un(Op::Not as u32), vec![v])
    }

    /// If `cond` compares `cand` against the accumulator `loopv` (`cand > loopv` etc.),
    /// classify the selection as max or min. Operand order is meaningful (comparisons
    /// are not commutative-canonicalized), so `cand > acc` and `acc < cand` both → max.
    pub(super) fn selection_code(
        &self,
        cond: ValueId,
        cand: ValueId,
        loopv: ValueId,
    ) -> Option<u32> {
        if !self.comparison_law_enabled(ComparisonLaw::SelectionReductionGuard) {
            return None;
        }
        let n = &self.nodes[cond as usize];
        let opc = match n.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if n.args.len() != 2 {
            return None;
        }
        let cand_first = n.args[0] == cand && n.args[1] == loopv;
        let acc_first = n.args[0] == loopv && n.args[1] == cand;
        if !cand_first && !acc_first {
            return None;
        }
        // `cand > acc` / `acc < cand` → take the larger ⇒ max; the reverse ⇒ min.
        let greater = opc == Op::Gt as u32 || opc == Op::Ge as u32;
        let lesser = opc == Op::Lt as u32 || opc == Op::Le as u32;
        if (greater && cand_first) || (lesser && acc_first) {
            Some(REDUCE_MAX)
        } else if (lesser && cand_first) || (greater && acc_first) {
            Some(REDUCE_MIN)
        } else {
            None
        }
    }
    /// Walk a (possibly nested) block, applying assignments to `env`, and return the
    /// value of the first `return` expression reached.
    pub(super) fn eval_block_return(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match self.il.kind(node) {
            NodeKind::Block => {
                let kids = self.il.children(node).to_vec();
                let n = kids.len();
                for (i, &s) in kids.iter().enumerate() {
                    // An explicit `return` anywhere wins; a let-binding binds; the LAST
                    // statement is the *implicit* return value (Rust closures and Ruby
                    // blocks have no `return` — their trailing expression is the result).
                    if let Some(v) = self.eval_block_return(s, env) {
                        return Some(v);
                    }
                    if i + 1 == n {
                        if let NodeKind::ExprStmt = self.il.kind(s) {
                            return self.il.children(s).first().map(|&e| self.eval(e, env));
                        }
                    }
                }
                None
            }
            NodeKind::Return => self.il.children(node).first().map(|&e| self.eval(e, env)),
            NodeKind::Assign => {
                let kids = self.il.children(node).to_vec();
                if kids.len() == 2 && self.il.kind(kids[0]) == NodeKind::Var {
                    if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                        let rhs = self.eval(kids[1], env);
                        env.insert(c, rhs);
                    }
                }
                None
            }
            // A bare-expression lambda body (`|a, v| a + v`) — its value is the result.
            NodeKind::ExprStmt => self.il.children(node).first().map(|&e| self.eval(e, env)),
            _ => Some(self.eval(node, env)),
        }
    }
}
