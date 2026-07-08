use super::super::*;

pub(in crate::value_graph) const PATH_COND_FORMULA_THRESHOLD: usize = 32;

impl<'a> Builder<'a> {
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
    pub(in crate::value_graph) fn recognize_existence_reduction(&mut self) {
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
        let bot = self.sentinel_const(sentinel::BOTTOM);
        let tru = self.bool_const_value(true);
        let fls = self.bool_const_value(false);
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
                // `Array.prototype.every` skips sparse-array holes. A JS/TS typed
                // Array parameter does not prove the for-of source is dense, so keep
                // universal counterexample loops over such sources outside this fold.
                if self.refs_js_like_array_param_elem(guard) {
                    continue;
                }
                (REDUCE_ALL, self.mk(ValOp::Un(Op::Not as u32), vec![guard]))
            } else {
                continue;
            };
            let red = self.mk(ValOp::Reduce(code), vec![pred]);
            self.sinks = vec![Sink::new(SinkKind::Return, red)];
            return;
        }
    }

    pub(in crate::value_graph) fn recognize_value_default_returns(&mut self) {
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

    pub(in crate::value_graph) fn map_default_from_partial_default_pair(
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

    pub(in crate::value_graph) fn map_default_from_guarded_pair(
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

    pub(in crate::value_graph) fn map_default_from_guarded_fallthrough(
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

    pub(in crate::value_graph) fn value_default_from_guarded_pair(
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
        Some(self.mk_nullish_map_default(value, default))
    }

    pub(in crate::value_graph) fn value_default_from_guarded_fallthrough(
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
        Some(self.mk_nullish_map_default(value, default))
    }

    pub(in crate::value_graph) fn guarded_return_parts(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Phi)
            || node.args.len() != 3
            || !self.is_bottom_value(node.args[2])
        {
            return None;
        }
        Some((node.args[0], node.args[1]))
    }

    pub(in crate::value_graph) fn map_default_bottom_call(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == builtin_tag(Builtin::GetOrDefault))
            && node.args.len() == 3
            && self.is_bottom_value(node.args[2])
        {
            return Some((node.args[0], node.args[1]));
        }
        None
    }

    pub(in crate::value_graph) fn is_bottom_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Const { kind: ConstKind::Sentinel, bits } if bits == sentinel::BOTTOM)
    }

    /// The conjunction of the current branch path (`c₁ ∧ c₂ ∧ …`), or `None` at top level.
    pub(in crate::value_graph) fn path_cond(&mut self) -> Option<ValueId> {
        self.path_cond_from(0)
    }

    /// The conjunction of path conditions from `base` (an earlier `path.len()`) to the
    /// top — the path suffix relative to a marked entry point, used by inline return
    /// capture to express a callee-internal guard without the caller's own conditions.
    pub(in crate::value_graph) fn path_cond_from(&mut self, base: usize) -> Option<ValueId> {
        let suffix_len = self.path.len().saturating_sub(base);
        if suffix_len == 0 {
            return None;
        }
        if suffix_len >= PATH_COND_FORMULA_THRESHOLD {
            return Some(self.compact_path_cond_from(base, suffix_len));
        }
        let mut pc: Option<ValueId> = None;
        // Indexed loop: `mk` needs `&mut self` and never touches `path`.
        for i in base..self.path.len() {
            let c = self.path[i];
            pc = Some(match pc {
                None => c,
                Some(p) => self.mk(ValOp::Bin(Op::And as u32), vec![p, c]),
            });
        }
        pc
    }

    fn compact_path_cond_from(&mut self, base: usize, suffix_len: usize) -> ValueId {
        // Long generated decision tables can produce hundreds of nested path guards.
        // Preserve the exact ordered condition identity in one Formula key instead of
        // materializing an equally-large `And` DAG for every guarded sink.
        let mut h = combine(0x9A7C_0D1D, Op::And as u64);
        h = combine(h, suffix_len as u64);
        for i in base..self.path.len() {
            h = combine(h, self.vhash[self.path[i] as usize]);
        }
        self.mk(ValOp::Formula(h), vec![])
    }

    /// Does `v`'s value subgraph reference an `Elem` (a collection element)? Bounded DAG
    /// walk; used to confirm a guard is a per-element predicate of a loop.
    pub(in crate::value_graph) fn refs_elem(&self, v: ValueId) -> bool {
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
}
