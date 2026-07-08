use super::super::*;

impl<'a> Builder<'a> {
    /// Fold a reduction builtin (`sum`, `reduce`) to the canonical `Reduce(op, init,
    /// per-element contrib)` — the same value a loop accumulator produces. Returns
    /// `None` if it isn't a recognized clean reduction (caller falls back to a Call).
    pub(in crate::value_graph) fn eval_reduction_builtin(
        &mut self,
        call: NodeId,
        b: Builtin,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match reduction_builtin_contract(b)? {
            ReductionBuiltinContract::Len => self.reduction_len(call, kids, env),
            ReductionBuiltinContract::Sum => self.reduction_sum(kids, env),
            ReductionBuiltinContract::ExplicitFold => self.reduction_explicit_fold(kids, env),
            ReductionBuiltinContract::Selection { max } => self.reduction_selection(max, kids, env),
            ReductionBuiltinContract::Bool { all } => self.reduction_bool(all, kids, env),
            ReductionBuiltinContract::Join => self.reduction_join(kids, env),
        }
    }

    fn reduction_len(
        &mut self,
        call: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() == 1 {
            if admitted_terminal_count_reduction_at_call(self.il, call) {
                self.eval_terminal_count_builtin(kids[0], env)
            } else {
                self.eval_len_builtin(kids[0], env)
            }
        } else {
            None
        }
    }

    fn reduction_sum(&mut self, kids: &[NodeId], env: &FxHashMap<u32, ValueId>) -> Option<ValueId> {
        if kids
            .first()
            .is_some_and(|&arg| !self.terminal_reduction_arg_admitted(arg))
        {
            return None;
        }
        let av = self.eval(*kids.first()?, env);
        // `sum(map)` → the mapped stream's per-element contribution; a filtered
        // map/flat-map carries a predicate and becomes `pred ? contrib : 0`,
        // matching a guarded loop `if pred: acc += contrib`; `sum(xs)` → the raw
        // element.
        let zero = self.int_const(0);
        let (contrib, predicate) = self.collection_elem_with_pred(av);
        let contrib = if let Some(predicate) = predicate {
            self.mk(ValOp::Phi, vec![predicate, contrib, zero])
        } else {
            contrib
        };
        let init = self.int_const(0);
        Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
    }

    fn reduction_explicit_fold(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() < 2 {
            return None;
        }
        let filtered = self.filter_parts(kids[1]);
        let (elems, guard) = if let Some((source, predicate)) = filtered {
            let coll = self.eval(source, env);
            let elem = self.elem(coll);
            let guard = self.eval_lambda_body(predicate, &[elem], env)?;
            (vec![elem], Some(guard))
        } else {
            self.elem_bindings_with_pred(kids.get(1).copied(), env)
        };
        let acc = self.fresh_opaque();
        let mut params = Vec::with_capacity(elems.len() + 1);
        params.push(acc);
        params.extend(elems);
        let body = self.eval_lambda_body(kids[0], &params, env)?;
        let (op, contrib) = self.as_reduction(body, acc)?;
        let contrib = if let Some(guard) = guard {
            let ident = self.int_const(identity_of(op)?);
            self.mk(ValOp::Phi, vec![guard, contrib, ident])
        } else {
            contrib
        };
        // `reduce(λ, init)` carries its seed — for a selection (min/max)
        // the seed clamps the result, so dropping it merged
        // `reduce(max-λ, 0)` with true `max(…)`. A seedless `reduce(λ)`
        // folds from the first element: for a selection that IS the true
        // min/max — the same 1-arg shape the builtin builds.
        let init = kids.get(2).map(|&i| self.eval(i, env));
        let args = match (init, is_selection_code(op)) {
            (Some(init), _) => vec![init, contrib],
            (None, true) => vec![contrib],
            (None, false) => vec![self.int_const(0), contrib],
        };
        Some(self.mk(ValOp::Reduce(op), args))
    }

    fn reduction_selection(
        &mut self,
        max: bool,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() == 1 && !self.terminal_reduction_arg_admitted(kids[0]) {
            return None;
        }
        let (reduce_code, choice_code) = if max {
            (REDUCE_MAX, MAX_CODE)
        } else {
            (REDUCE_MIN, MIN_CODE)
        };
        if kids.len() == 2 {
            let left = self.eval(kids[0], env);
            let right = self.eval(kids[1], env);
            return Some(self.mk(ValOp::Bin(choice_code), vec![left, right]));
        }
        let av = self.eval(*kids.first()?, env);
        // `max(f(x) for x in xs)` → the mapped per-element value; `max(xs)` →
        // the raw element. Seedless (1-arg) by construction: a SEEDED loop
        // (`best = 0; …`) clamps at its seed, so it keys differently and
        // must not merge with the true builtin selection.
        let (op, args) = {
            let n = &self.nodes[av as usize];
            (n.op.clone(), n.args.clone())
        };
        let contrib = match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && !args.is_empty() => args[0],
            _ => self.elem(av),
        };
        Some(self.mk(ValOp::Reduce(reduce_code), vec![contrib]))
    }

    fn reduction_bool(
        &mut self,
        all: bool,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let code = if all { REDUCE_ALL } else { REDUCE_ANY };
        // `xs.some(p)` / `xs.any(p)` — method form `[coll, λ]`: the per-element
        // contribution is `p(Elem coll)`. `any(p(x) for x in xs)` — generator form
        // `[Map]`: the mapped predicate value; a *filtered* generator carries its
        // predicate, guarded by the OR/AND identity (false for any, true for all).
        let contrib = if kids.len() >= 2 && self.il.kind(kids[1]) == NodeKind::Lambda {
            let coll = self.eval(kids[0], env);
            // JS/TS Array.every skips sparse-array holes, while value iteration over a
            // `number[]` parameter can observe `undefined` at those positions. A typed
            // Array parameter is therefore not dense-source proof for universal folds.
            if all && self.js_like_array_param_universal_closed(coll) {
                return None;
            }
            let (elem, carried_guard) = self.collection_elem_with_pred(coll);
            let pred = self.eval_lambda_body(kids[1], &[elem], env)?;
            if carried_guard.is_none()
                && code == REDUCE_ANY
                && self.is_static_non_float_collection_expr(kids[0])
                && self.lambda_return_source_operator_allowed(kids[1], Op::Eq)
            {
                if let Some((element, collection)) = self.static_literal_membership_predicate(pred)
                {
                    return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
                }
            }
            if carried_guard.is_none()
                && code == REDUCE_ALL
                && self.is_static_non_float_collection_expr(kids[0])
                && self.lambda_return_source_operator_allowed(kids[1], Op::Ne)
            {
                if let Some((element, collection)) = self.static_literal_absence_predicate(pred) {
                    let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                    return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
                }
            }
            if let Some(carried_guard) = carried_guard {
                let ident = self.bool_const_value(code != REDUCE_ANY);
                self.mk(ValOp::Phi, vec![carried_guard, pred, ident])
            } else {
                pred
            }
        } else {
            if kids
                .first()
                .is_some_and(|&arg| !self.terminal_reduction_arg_admitted(arg))
            {
                return None;
            }
            let av = self.eval(*kids.first()?, env);
            let (contrib, predicate) = self.collection_elem_with_pred(av);
            if let Some(predicate) = predicate {
                let ident = self.bool_const_value(code != REDUCE_ANY);
                self.mk(ValOp::Phi, vec![predicate, contrib, ident])
            } else {
                contrib
            }
        };
        Some(self.mk(ValOp::Reduce(code), vec![contrib]))
    }

    fn reduction_join(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        let sep = self.eval(kids[0], env);
        let coll = self.eval(kids[1], env);
        let elem = self.elem(coll);
        Some(self.mk(ValOp::Reduce(ORDERED_STRING_JOIN), vec![sep, elem]))
    }
}
