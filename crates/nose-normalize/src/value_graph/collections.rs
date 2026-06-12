//! Collection, higher-order, and evidence-gated library value evaluation.
//!
//! proof-obligation: normalize.value_graph.functor

use super::*;

impl<'a> Builder<'a> {
    /// A canonical "element of `coll`" value. The collection is carried as an argument
    /// (not just folded into the key) so it is reachable from the fingerprint wherever
    /// the element is used — identically for a loop, a `reduce`, and a `sum(map)` over
    /// the same collection, so they converge without a separate iterable sink.
    pub(super) fn elem(&mut self, coll: ValueId) -> ValueId {
        if let Some(value) = self.hof_emitted_elem(coll) {
            return value;
        }
        self.raw_elem(coll)
    }

    pub(super) fn hof_emitted_elem(&mut self, coll: ValueId) -> Option<ValueId> {
        let (emitted, predicate) = self.hof_emitted_elem_with_pred(coll)?;
        predicate.is_none().then_some(emitted)
    }

    pub(super) fn raw_elem(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Elem(self.vhash[coll as usize]), vec![coll])
    }

    pub(super) fn collection_elem_with_pred(
        &mut self,
        coll: ValueId,
    ) -> (ValueId, Option<ValueId>) {
        if let Some(parts) = self.hof_emitted_elem_with_pred(coll) {
            return parts;
        }
        (self.raw_elem(coll), None)
    }

    pub(super) fn hof_emitted_elem_with_pred(
        &mut self,
        coll: ValueId,
    ) -> Option<(ValueId, Option<ValueId>)> {
        // FUNCTOR LAW / map fusion: an element drawn from `map(f, c)` is `f` applied to an
        // element of `c`, and a pure Map node's `contrib` (args[0]) already *is* that
        // per-element value. So `Elem(Map(f, c)) -> contrib`, which fuses nested maps:
        // `map(g, map(f, xs))` and `map(g o f, xs)` converge to one fingerprint. Sound
        // (functor composition law: map g o map f = map (g o f)). A *filtered* map carries a
        // predicate (args.len() == 2) and is NOT peeled (the filter changes which elements).
        //
        // FlatMap emits the elements produced by its inner stream. When the modeled inner
        // stream is a pure Map, `Elem(FlatMap(xs, map(f, ys)))` is the same emitted `f(y)`
        // value. Keep this separate from the filtered-Map two-argument layout so aggregate
        // consumers do not confuse `FlatMap[outer, inner]` with `Map[contrib, pred]`.
        let (op, args) = {
            let n = &self.nodes[coll as usize];
            (n.op.clone(), n.args.clone())
        };
        match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 1 => Some((args[0], None)),
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 2 => {
                Some((args[0], Some(args[1])))
            }
            ValOp::Hof(k)
                if k == HoFKind::FlatMap as u32 && (args.len() == 2 || args.len() == 3) =>
            {
                let outer = args[0];
                let inner = args[1];
                let outer_predicate = args.get(2).copied();
                let (emitted, inner_predicate) = self.hof_emitted_elem_with_pred(inner)?;
                if !self.references(emitted, outer) {
                    return None;
                }
                let predicate = self.and_preds(outer_predicate, inner_predicate);
                Some((emitted, predicate))
            }
            _ => None,
        }
    }

    /// A canonical "iteration index into `coll`". `for i in range(len(xs))`, an indexed
    /// `while`, and `for i, _ in enumerate(xs)` all bind their index variable to this,
    /// so they converge — and `C[Idx(C)]` rewrites to `Elem(C)`.
    pub(super) fn idx(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Idx(self.vhash[coll as usize]), vec![coll])
    }

    /// The canonical-id variables of a loop pattern: a single `Var`, or the elements
    /// of a tuple pattern `(i, x)` (lowered as a `Seq` of `Var`s).
    pub(super) fn pattern_cids(&self, pat: NodeId) -> Vec<u32> {
        let mut out = Vec::new();
        let push = |n: NodeId, out: &mut Vec<u32>| {
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(n), self.il.node(n).payload) {
                out.push(c);
            }
        };
        if self.il.kind(pat) == NodeKind::Seq {
            for c in self.il.children(pat) {
                push(*c, &mut out);
            }
        } else {
            push(pat, &mut out);
        }
        out
    }

    /// If `node` is a *full* index range over `C` — `range(len(C))` or `range(0,
    /// len(C))` — return `C`: the loop visits every index of `C`, so `C[i]` is the
    /// canonical `Elem(C)`. A non-zero start (`range(1, len(C))`), an explicit step
    /// (`range(_, _, k)`), or any other form iterates a *subset*, so its element is NOT
    /// `Elem(C)` — abstracting `C[i]` to `Elem(C)` there drops the start/step bound and
    /// merges behaviorally-different loops (a soundness bug). Such forms return `None`.
    pub(super) fn range_len_collection(&self, node: NodeId) -> Option<NodeId> {
        let len_arg = if self.il.kind(node) == NodeKind::Call
            && matches!(self.il.node(node).payload, Payload::Builtin(Builtin::Range))
            && self.admitted_builtin_call(node, Builtin::Range)
        {
            let kids = self.il.children(node);
            match kids.len() {
                1 => kids[0],
                // `range(start, stop)` is a full iteration only when `start` is literally 0.
                2 if matches!(self.il.node(kids[0]).payload, Payload::LitInt(0)) => kids[1],
                _ => return None,
            }
        } else if self.il.kind(node) == NodeKind::Seq
            && source_range_at_node(self.il, node)
                == Some(SourceRangeKind::RustHalfOpenRangeExpression)
        {
            let kids = self.il.children(node);
            match kids {
                // Rust `0..len(C)` lowers as `Seq(0, Len(C), inclusive=0)`.
                // The source range fact proves this is a Rust half-open range expression;
                // raw `Seq(0, Len(C), 0)` shape alone is not an iteration contract.
                [start, stop, inclusive]
                    if matches!(self.il.node(*start).payload, Payload::LitInt(0))
                        && matches!(self.il.node(*inclusive).payload, Payload::LitInt(0)) =>
                {
                    *stop
                }
                _ => return None,
            }
        } else {
            return None;
        };
        if self.il.kind(len_arg) == NodeKind::Call
            && matches!(
                self.il.node(len_arg).payload,
                Payload::Builtin(Builtin::Len)
            )
            && self.admitted_builtin_call(len_arg, Builtin::Len)
        {
            return self.il.children(len_arg).first().copied();
        }
        None
    }

    /// Fold a reduction builtin (`sum`, `reduce`) to the canonical `Reduce(op, init,
    /// per-element contrib)` — the same value a loop accumulator produces. Returns
    /// `None` if it isn't a recognized clean reduction (caller falls back to a Call).
    pub(super) fn eval_reduction_builtin(
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

    pub(super) fn eval_len_builtin(
        &mut self,
        arg: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if !self.len_arg_admitted(arg) {
            return None;
        }
        if let Some(count) = self.eval_filter_count(arg, env) {
            return Some(count);
        }

        let av = self.eval(arg, env);
        self.eval_len_value(av)
    }

    pub(super) fn eval_terminal_count_builtin(
        &mut self,
        arg: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if !self.terminal_reduction_arg_admitted(arg) {
            return None;
        }
        if let Some(count) = self.eval_filter_count(arg, env) {
            return Some(count);
        }

        let av = self.eval(arg, env);
        self.eval_len_value(av)
    }

    pub(super) fn len_arg_admitted(&self, arg: NodeId) -> bool {
        if self.il.kind(arg) != NodeKind::HoF {
            return true;
        }
        match source_comprehension_at_node(self.il, arg) {
            Some(SourceComprehensionKind::PythonListComprehension) => true,
            Some(
                SourceComprehensionKind::PythonDictComprehension
                | SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonSetComprehension,
            ) => false,
            None => match self.il.node(arg).payload {
                Payload::HoF(kind) => {
                    admitted_hof_demand_effect_profile_at_node(self.il, arg, kind)
                        .is_some_and(|profile| profile.proves_eager_per_element_callback_demand())
                }
                _ => false,
            },
        }
    }

    pub(super) fn terminal_reduction_arg_admitted(&self, arg: NodeId) -> bool {
        if self.il.kind(arg) != NodeKind::HoF {
            return true;
        }
        match source_comprehension_at_node(self.il, arg) {
            Some(
                SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonListComprehension,
            ) => true,
            Some(
                SourceComprehensionKind::PythonDictComprehension
                | SourceComprehensionKind::PythonSetComprehension,
            ) => false,
            None => match self.il.node(arg).payload {
                Payload::HoF(kind) => {
                    admitted_hof_demand_effect_profile_at_node(self.il, arg, kind).is_some()
                }
                _ => false,
            },
        }
    }

    pub(super) fn hof_value_admission(&self, node: NodeId, kind: HoFKind) -> Option<HofAdmission> {
        match source_comprehension_at_node(self.il, node) {
            Some(
                SourceComprehensionKind::PythonDictComprehension
                | SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonListComprehension,
            ) => Some(HofAdmission::SourceComprehension),
            Some(SourceComprehensionKind::PythonSetComprehension) => None,
            None if admitted_hof_demand_effect_profile_at_node(self.il, node, kind).is_some() => {
                Some(HofAdmission::LibraryApi)
            }
            None => None,
        }
    }

    pub(super) fn source_salted_opaque(&mut self, expr: NodeId, tag: u64) -> ValueId {
        let salt = self.source_salted_hash(expr, tag);
        self.mk(ValOp::Opaque(salt), vec![])
    }

    pub(super) fn eval_len_value(&mut self, value: ValueId) -> Option<ValueId> {
        let (op, args) = {
            let n = &self.nodes[value as usize];
            (n.op.clone(), n.args.clone())
        };
        let ValOp::Hof(k) = op else { return None };
        if k != HoFKind::Map as u32 {
            return None;
        }

        let one = self.int_const(1);
        let contrib = if args.len() >= 2 {
            let zero = self.int_const(0);
            self.mk(ValOp::Phi, vec![args[1], one, zero])
        } else {
            one
        };
        let init = self.int_const(0);
        Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
    }

    pub(super) fn eval_len_zero_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let cardinality = semantics(self.il.meta.lang)
            .operators()
            .zero_cardinality_equality(op_from_code(op)?)?;
        if kids.len() != 2 {
            return None;
        }
        let coll = if self.is_zero_literal(kids[0]) {
            self.len_call_arg(kids[1])?
        } else if self.is_zero_literal(kids[1]) {
            self.len_call_arg(kids[0])?
        } else {
            return None;
        };
        let coll_value = self.eval(coll, env);
        let empty = self.is_empty_value(coll_value);
        match cardinality.predicate {
            CardinalityPredicate::Empty => Some(empty),
            CardinalityPredicate::NonEmpty => Some(self.mk(ValOp::Un(Op::Not as u32), vec![empty])),
        }
    }

    pub(super) fn eval_static_filter_membership_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        if let Some((element, collection)) = self.static_filter_membership_parts(kids[0], env) {
            if self.is_count_nonempty_threshold(op, false, kids[1]) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            if self.is_count_zero_threshold(op, false, kids[1]) {
                let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
            }
        }
        if let Some((element, collection)) = self.static_filter_membership_parts(kids[1], env) {
            if self.is_count_nonempty_threshold(op, true, kids[0]) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            if self.is_count_zero_threshold(op, true, kids[0]) {
                let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
            }
        }
        None
    }

    pub(super) fn static_filter_membership_parts(
        &mut self,
        len_expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, ValueId)> {
        let filter = self.len_call_arg(len_expr)?;
        let (source, predicate) = self.filter_parts(filter)?;
        if !self.is_static_non_float_collection_expr(source) {
            return None;
        }
        if !self.lambda_return_source_operator_allowed(predicate, Op::Eq) {
            return None;
        }
        let collection = self.eval(source, env);
        let elem = self.elem(collection);
        let pred = self.eval_lambda_body(predicate, &[elem], env)?;
        self.static_literal_membership_predicate(pred)
    }

    pub(super) fn is_count_nonempty_threshold(
        &self,
        op: u32,
        count_on_right: bool,
        threshold: NodeId,
    ) -> bool {
        let Some(op) = op_from_code(op) else {
            return false;
        };
        if self.is_zero_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .cardinality_threshold(
                    op,
                    count_on_right,
                    CardinalityThreshold::Zero,
                    CardinalityPredicate::NonEmpty,
                )
                .is_some();
        }
        if self.is_one_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .cardinality_threshold(
                    op,
                    count_on_right,
                    CardinalityThreshold::One,
                    CardinalityPredicate::NonEmpty,
                )
                .is_some();
        }
        false
    }

    pub(super) fn is_count_zero_threshold(
        &self,
        op: u32,
        count_on_right: bool,
        threshold: NodeId,
    ) -> bool {
        let Some(op) = op_from_code(op) else {
            return false;
        };
        if self.is_zero_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .cardinality_threshold(
                    op,
                    count_on_right,
                    CardinalityThreshold::Zero,
                    CardinalityPredicate::Empty,
                )
                .is_some();
        }
        if self.is_one_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .cardinality_threshold(
                    op,
                    count_on_right,
                    CardinalityThreshold::One,
                    CardinalityPredicate::Empty,
                )
                .is_some();
        }
        false
    }

    pub(super) fn eval_static_index_membership_comparison(
        &mut self,
        op: Op,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        if self.is_index_membership_threshold(op, false, kids[1]) {
            if let Some((element, collection)) = self.static_index_membership_parts(kids[0], env) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
        }
        if self.is_index_membership_threshold(op, true, kids[0]) {
            if let Some((element, collection)) = self.static_index_membership_parts(kids[1], env) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
        }
        None
    }

    pub(super) fn static_index_membership_parts(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, ValueId)> {
        let (receiver, _method, arg) = self.single_arg_field_call_parts(node)?;
        if !self.is_static_non_float_collection_expr(receiver) {
            return None;
        }
        let contract =
            admitted_static_index_membership_at_call(self.il, self.interner, node)?.contract;
        match contract.result.kind {
            StaticIndexMembershipKind::IndexOf => {
                let element = self.eval(arg, env);
                let collection = self.eval_membership_collection(receiver, env);
                Some((element, collection))
            }
            StaticIndexMembershipKind::FindIndex if self.il.kind(arg) == NodeKind::Lambda => {
                if !self.lambda_return_source_operator_allowed(arg, Op::Eq) {
                    return None;
                }
                let collection = self.eval(receiver, env);
                let elem = self.elem(collection);
                let pred = self.eval_lambda_body(arg, &[elem], env)?;
                self.static_literal_membership_predicate(pred)
            }
            StaticIndexMembershipKind::FindIndex => None,
        }
    }

    pub(super) fn is_index_membership_threshold(
        &self,
        op: Op,
        index_call_on_right: bool,
        threshold: NodeId,
    ) -> bool {
        if self.is_minus_one_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .static_index_membership_threshold(
                    op,
                    index_call_on_right,
                    IndexMembershipThreshold::MinusOne,
                )
                .is_some();
        }
        if self.is_zero_literal(threshold) {
            return semantics(self.il.meta.lang)
                .operators()
                .static_index_membership_threshold(
                    op,
                    index_call_on_right,
                    IndexMembershipThreshold::Zero,
                )
                .is_some();
        }
        false
    }

    pub(super) fn is_minus_one_literal(&self, node: NodeId) -> bool {
        if matches!(self.il.node(node).payload, Payload::LitInt(-1)) {
            return true;
        }
        if self.il.kind(node) != NodeKind::UnOp {
            return false;
        }
        if op_code(self.il.node(node).payload) != Op::Neg as u32 {
            return false;
        }
        let kids = self.il.children(node);
        kids.len() == 1 && matches!(self.il.node(kids[0]).payload, Payload::LitInt(1))
    }

    pub(super) fn is_empty_value(&mut self, coll: ValueId) -> ValueId {
        let coll = self.param_domain_value(coll);
        let len = self.mk(ValOp::Call(builtin_tag(Builtin::Len)), vec![coll]);
        let zero = self.int_const(0);
        self.mk(ValOp::Bin(Op::Eq as u32), vec![len, zero])
    }

    pub(super) fn map_default_pattern(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        let (key, map, negated) = self
            .own_property_condition(cond)
            .or_else(|| self.membership_condition(cond))?;
        let default = if negated {
            if !self.map_lookup_value_matches(else_v, map, key) {
                return None;
            }
            then_v
        } else {
            if !self.map_lookup_value_matches(then_v, map, key) {
                return None;
            }
            else_v
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    pub(super) fn value_default_pattern(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.is_bottom_value(then_v) || self.is_bottom_value(else_v) {
            return None;
        }
        let (value, present) = self.null_condition(cond)?;
        let default = if present {
            if !self.value_branch_returns_value(then_v, value) {
                return None;
            }
            else_v
        } else {
            if !self.value_branch_returns_value(else_v, value) {
                return None;
            }
            then_v
        };
        Some(self.mk_value_or_map_default(value, default))
    }

    pub(super) fn value_default_call(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == builtin_tag(Builtin::ValueOrDefault))
            && node.args.len() == 2
        {
            Some((node.args[0], node.args[1]))
        } else {
            None
        }
    }

    pub(super) fn value_branch_returns_value(&self, branch: ValueId, value: ValueId) -> bool {
        branch == value
            || self
                .value_default_call(branch)
                .is_some_and(|(inner_value, _)| inner_value == value)
    }

    pub(super) fn mk_value_default(&mut self, value: ValueId, default: ValueId) -> ValueId {
        if self
            .value_default_call(value)
            .is_some_and(|(_, inner_default)| inner_default == default)
        {
            return value;
        }
        self.mk(
            ValOp::Call(builtin_tag(Builtin::ValueOrDefault)),
            vec![value, default],
        )
    }

    pub(super) fn mk_value_or_map_default(&mut self, value: ValueId, default: ValueId) -> ValueId {
        if let Some((map, key)) = self.proven_map_get_value(value) {
            return self.mk(
                ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                vec![map, key, default],
            );
        }
        self.mk_value_default(value, default)
    }

    pub(super) fn null_condition(&self, cond: ValueId) -> Option<(ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        if node.args.len() == 2 {
            if matches!(node.op, ValOp::Bin(o) if o == Op::Eq as u32) {
                if self.is_null_value(node.args[0]) {
                    return Some((node.args[1], false));
                }
                if self.is_null_value(node.args[1]) {
                    return Some((node.args[0], false));
                }
            }
            if matches!(node.op, ValOp::Bin(o) if o == Op::Ne as u32) {
                if self.is_null_value(node.args[0]) {
                    return Some((node.args[1], true));
                }
                if self.is_null_value(node.args[1]) {
                    return Some((node.args[0], true));
                }
            }
        }
        None
    }

    pub(super) fn is_null_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        )
    }

    pub(super) fn membership_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        if matches!(node.op, ValOp::Bin(o) if o == Op::In as u32) && node.args.len() == 2 {
            return Some((node.args[0], node.args[1], false));
        }
        if matches!(node.op, ValOp::Un(o) if o == Op::Not as u32) && node.args.len() == 1 {
            let inner = &self.nodes[node.args[0] as usize];
            if matches!(inner.op, ValOp::Bin(o) if o == Op::In as u32) && inner.args.len() == 2 {
                return Some((inner.args[0], inner.args[1], true));
            }
        }
        None
    }

    pub(super) fn map_presence_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let (key, map, negated) = self
            .own_property_condition(cond)
            .or_else(|| self.membership_condition(cond))?;
        Some((key, map, !negated))
    }

    pub(super) fn static_literal_membership_predicate(
        &mut self,
        pred: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[pred as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Eq as u32) || node.args.len() != 2 {
            return None;
        }
        let left = node.args[0];
        let right = node.args[1];
        if let Some(collection) = self.static_literal_elem_collection(left) {
            return Some((right, collection));
        }
        if let Some(collection) = self.static_literal_elem_collection(right) {
            return Some((left, collection));
        }
        None
    }

    pub(super) fn static_literal_absence_predicate(
        &mut self,
        pred: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[pred as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Ne as u32) || node.args.len() != 2 {
            return None;
        }
        let left = node.args[0];
        let right = node.args[1];
        if let Some(collection) = self.static_literal_elem_collection(left) {
            return Some((right, collection));
        }
        if let Some(collection) = self.static_literal_elem_collection(right) {
            return Some((left, collection));
        }
        None
    }

    pub(super) fn static_literal_elem_collection(&mut self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Elem(_)) || node.args.len() != 1 {
            return None;
        }
        let collection = node.args[0];
        self.is_static_membership_collection(collection)
            .then(|| self.canonical_membership_collection_value(collection))
    }

    pub(super) fn is_static_membership_collection(&self, value: ValueId) -> bool {
        let node = &self.nodes[value as usize];
        matches!(node.op, ValOp::Seq(SEQ_VALUE_COLLECTION)) && !node.args.is_empty()
    }

    pub(super) fn own_property_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let parse = |value: ValueId| {
            let node = &self.nodes[value as usize];
            if matches!(node.op, ValOp::Seq(SEQ_VALUE_OWN_PROPERTY_GUARD)) && node.args.len() == 4 {
                let map = node.args[0];
                if !matches!(self.nodes[map as usize].op, ValOp::Seq(SEQ_VALUE_MAP)) {
                    return None;
                }
                return Some((node.args[1], map, false));
            }
            None
        };
        let node = &self.nodes[cond as usize];
        if let Some(parts) = parse(cond) {
            return Some(parts);
        }
        if matches!(node.op, ValOp::Un(o) if o == Op::Not as u32) && node.args.len() == 1 {
            if let Some((key, map, _)) = parse(node.args[0]) {
                return Some((key, map, true));
            }
        }
        None
    }

    pub(super) fn map_lookup_value_matches(
        &mut self,
        value: ValueId,
        map: ValueId,
        key: ValueId,
    ) -> bool {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Index) && node.args.as_slice() == [map, key] {
            return true;
        }
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return false;
        }
        let args = node.args.clone();
        if args[1] != key {
            return false;
        }
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return false;
        };
        if callee.args.len() != 1 {
            return false;
        }
        if admitted_map_get_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(value, args[0], Some(callee.args[0]), 1),
            method,
        )
        .is_none()
        {
            return false;
        }
        let receiver = callee.args[0];
        receiver == map
            || self
                .proven_map_value(receiver)
                .is_some_and(|candidate| candidate == map)
    }

    pub(super) fn eval_membership_collection(
        &mut self,
        collection: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        if self.il.kind(collection) == NodeKind::Seq {
            if self
                .seq_surface(collection)
                .is_some_and(|contract| contract.membership_collection)
            {
                let kids = self.il.children(collection).to_vec();
                let mut items: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                items.sort_by_key(|&v| (self.vhash[v as usize], v));
                items.dedup();
                return self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items);
            }
            let value = self.eval(collection, env);
            return self.canonical_membership_collection_value(value);
        }
        let value = self.eval(collection, env);
        if let Some(collection) = self
            .proven_collection_value(value)
            .or_else(|| self.proven_local_collection_binding_value(collection, env))
        {
            return self.canonical_membership_collection_value(collection);
        }
        if self.is_collection_param_expr(collection) {
            return self.mk(ValOp::CollectionParam, vec![value]);
        }
        self.canonical_membership_collection_value(value)
    }

    pub(super) fn canonical_membership_collection_value(&mut self, value: ValueId) -> ValueId {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(SEQ_VALUE_COLLECTION)) {
            return value;
        }
        let mut items = node.args.clone();
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items)
    }

    pub(super) fn is_static_non_float_collection_expr(&self, collection: NodeId) -> bool {
        if self.il.kind(collection) != NodeKind::Seq {
            return false;
        }
        if !self
            .seq_surface(collection)
            .is_some_and(|contract| contract.membership_collection)
        {
            return false;
        }
        let kids = self.il.children(collection);
        !kids.is_empty()
            && kids.iter().all(|&kid| {
                self.il.kind(kid) == NodeKind::Lit
                    && matches!(
                        self.il.node(kid).payload,
                        Payload::LitInt(_)
                            | Payload::LitBool(_)
                            | Payload::LitStr(_)
                            | Payload::Lit(LitClass::Null)
                    )
            })
    }

    pub(super) fn eval_map_lookup_collection(
        &mut self,
        collection: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let value = self.eval(collection, env);
        self.proven_map_value(value).unwrap_or(value)
    }

    pub(super) fn len_call_arg(&self, node: NodeId) -> Option<NodeId> {
        if self.il.kind(node) != NodeKind::Call {
            return None;
        }
        if !matches!(self.il.node(node).payload, Payload::Builtin(Builtin::Len))
            || !self.admitted_builtin_call(node, Builtin::Len)
        {
            return None;
        }
        self.il.children(node).first().copied()
    }

    pub(super) fn is_zero_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(0))
    }

    pub(super) fn is_one_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(1))
    }

    pub(super) fn filter_parts(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        if self.il.kind(node) != NodeKind::HoF {
            return None;
        }
        if !matches!(self.il.node(node).payload, Payload::HoF(HoFKind::Filter)) {
            return None;
        }
        self.hof_value_admission(node, HoFKind::Filter)?;
        let kids = self.il.children(node);
        Some((*kids.first()?, *kids.get(1)?))
    }

    pub(super) fn eval_filter_count(
        &mut self,
        filter_node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let (source, predicate) = self.filter_parts(filter_node)?;
        self.eval_predicate_count(source, predicate, env)
    }

    pub(super) fn eval_predicate_count(
        &mut self,
        source: NodeId,
        predicate: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let coll = self.eval(source, env);
        let elem = self.elem(coll);
        let pred = self.eval_lambda_body(predicate, &[elem], env)?;
        let one = self.int_const(1);
        let zero = self.int_const(0);
        let contrib = self.mk(ValOp::Phi, vec![pred, one, zero]);
        let init = self.int_const(0);
        Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
    }

    pub(super) fn eval_count_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let contract = occurrence.contract;
        let result = contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::Len)
            || result.receiver != MethodReceiverContract::ExactProtocol
            || result.args != MethodBuiltinArgs::CollectionReduction
        {
            return None;
        }
        let base = occurrence.receiver?;
        let base_value = self.eval(base, env);
        if !matches!(
            self.nodes[base_value as usize].op,
            ValOp::Seq(_) | ValOp::ArrayParam | ValOp::CollectionParam | ValOp::Hof(_)
        ) {
            return None;
        }
        match kids {
            // Rust-style `iter.filter(p).count()`.
            [_] => self.eval_filter_count(base, env),
            _ => None,
        }
    }

    pub(super) fn eval_product_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence =
            admitted_imported_namespace_function_at_call(self.il, self.interner, expr)?;
        let contract = occurrence.contract;
        let ImportedNamespaceFunctionSemantic::ProductReduction { op, identity } =
            contract.result.semantic;
        // Split args into the positional iterable and an optional `start=` seed: the
        // seed may be positional (`prod(xs, 1)`) or keyword (`prod(xs, start=1)`). An
        // unrecognized keyword is an arg shape this recognizer does not model — bail to
        // the opaque path (sound) rather than guess (#301).
        let mut iterable = None;
        let mut seed_node = None;
        for &a in &kids[1..] {
            if self.il.kind(a) == NodeKind::KwArg {
                let Payload::Name(name) = self.il.node(a).payload else {
                    return None;
                };
                if self.interner.resolve(name) != "start" {
                    return None;
                }
                seed_node = Some(*self.il.children(a).first()?);
            } else if iterable.is_none() {
                iterable = Some(a);
            } else if seed_node.is_none() {
                seed_node = Some(a);
            } else {
                return None;
            }
        }
        let coll = self.eval(iterable?, env);
        let (coll_op, args) = {
            let n = &self.nodes[coll as usize];
            (n.op.clone(), n.args.clone())
        };
        let contrib = match coll_op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() >= 2 => {
                let one = self.int_const(1);
                self.mk(ValOp::Phi, vec![args[1], args[0], one])
            }
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 1 => args[0],
            _ => self.elem(coll),
        };
        let init = seed_node
            .map(|i| self.eval(i, env))
            .unwrap_or_else(|| self.int_const(identity));
        Some(self.mk(ValOp::Reduce(op as u32), vec![init, contrib]))
    }

    pub(super) fn eval_iterator_identity_adapter(
        &mut self,
        expr: NodeId,
        _kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_iterator_identity_adapter_at_call(self.il, self.interner, expr)?;
        let result = occurrence.contract.result;
        let base = occurrence.receiver?;
        let value = self.eval(base, env);
        let value = self.param_domain_value(value);
        self.iterator_adapter_receiver_proven(result.receiver, value)
            .then_some(value)
    }

    pub(super) fn iterator_adapter_receiver_proven(
        &self,
        receiver: IteratorAdapterReceiverContract,
        value: ValueId,
    ) -> bool {
        match receiver {
            IteratorAdapterReceiverContract::ExactIterableValue => matches!(
                self.nodes[value as usize].op,
                ValOp::Seq(_) | ValOp::ArrayParam | ValOp::CollectionParam | ValOp::Hof(_)
            ),
        }
    }

    pub(super) fn eval_rust_map_get_unwrap_or_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let contract = occurrence.contract;
        let result = contract.result;
        if result.receiver != MethodReceiverContract::RustMapGetOrExactOption
            || result.args != MethodBuiltinArgs::RustMapGetOrOptionDefault
        {
            return None;
        }
        let value = self.eval(occurrence.receiver?, env);
        let (map, key) = self.proven_map_get_value(value)?;
        let default = self.eval(kids[1], env);
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    pub(super) fn eval_rust_map_get_is_some_call(
        &mut self,
        expr: NodeId,
        _kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let contract = occurrence.contract;
        let result = contract.result;
        if result.receiver != MethodReceiverContract::RustMapGetOrExactOption
            || result.args != MethodBuiltinArgs::ReceiverOnly
            || result.semantic != MethodSemanticContract::Builtin(Builtin::IsNotNull)
        {
            return None;
        }
        let value = self.eval(occurrence.receiver?, env);
        let (map, key) = self.proven_map_get_value(value)?;
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![key, map]))
    }

    /// The element value(s) a map lambda binds to, plus any predicate CARRIED by the
    /// collection (map/filter fusion). If the collection evaluates to a *filtered* Map
    /// `Hof(Map,[c,p])`, the element is `c` and the carried predicate is `p` — so an outer
    /// map composes into one `filtered-map`, converging `map(h, map(f, filter p))` with the
    /// direct `filtered-map (h∘f)@p`. A pure Map collection is peeled by `elem`; `zip` binds
    /// multiple elements and carries no predicate.
    pub(super) fn map_source(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
        allow_internal_python_filter: bool,
    ) -> (Vec<ValueId>, Option<ValueId>) {
        let Some(c) = coll_node else {
            return (Vec::new(), None);
        };
        if self.il.kind(c) == NodeKind::Call
            && matches!(self.il.node(c).payload, Payload::Builtin(Builtin::Zip))
            && self.admitted_builtin_call(c, Builtin::Zip)
        {
            return (self.elem_bindings(Some(c), env), None);
        }
        let cv = if allow_internal_python_filter
            && self.il.meta.lang == Lang::Python
            && self.il.kind(c) == NodeKind::HoF
            && source_comprehension_at_node(self.il, c).is_none()
            && matches!(self.il.node(c).payload, Payload::HoF(HoFKind::Filter))
        {
            self.eval_hof_value(c, HoFKind::Filter, env, true)
        } else {
            self.eval(c, env)
        };
        if let ValOp::Hof(k) = self.nodes[cv as usize].op {
            if k == HoFKind::Map as u32 && self.nodes[cv as usize].args.len() == 2 {
                let args = self.nodes[cv as usize].args.clone();
                return (vec![args[0]], Some(args[1]));
            }
        }
        (vec![self.elem(cv)], None)
    }

    pub(super) fn eval_hof_value(
        &mut self,
        expr: NodeId,
        kind: HoFKind,
        env: &FxHashMap<u32, ValueId>,
        allow_internal_python_filter: bool,
    ) -> ValueId {
        let kids = self.il.children(expr).to_vec();
        match kind {
            // `xs.map(λx. body)` / a comprehension -> the per-element value over a
            // canonical `Elem(xs)`, so `[x*x for x in xs]` and `xs.map(x=>x*x)`
            // converge regardless of the lambda's syntax. `map_source` resolves
            // the collection to its element stream and any carried predicate.
            HoFKind::Map => {
                let (elems, carried_pred) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let fallback = elems
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.fresh_opaque());
                let contrib = match kids.get(1) {
                    Some(&lambda) => self
                        .eval_lambda_body(lambda, &elems, env)
                        .unwrap_or(fallback),
                    None => fallback,
                };
                match carried_pred {
                    Some(predicate) => self.mk(ValOp::Hof(kind as u32), vec![contrib, predicate]),
                    None => self.mk(ValOp::Hof(kind as u32), vec![contrib]),
                }
            }
            HoFKind::FlatMap => {
                let (elems, carried_pred) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let outer_elem = elems
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.fresh_opaque());
                let inner = match kids.get(1) {
                    Some(&lambda) => self
                        .eval_lambda_body(lambda, &elems, env)
                        .unwrap_or_else(|| self.fresh_opaque()),
                    None => self.fresh_opaque(),
                };
                // proof-obligation: normalize.value_graph.flatmap_identity
                // `flatMap(λx. x)` (identity inner: the lambda returns the outer
                // element unchanged) ≡ `flatMap(λx. map(λy. y, x))` ≡ flatten.
                let inner = if inner == outer_elem {
                    let elem = self.elem(outer_elem);
                    self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem])
                } else {
                    inner
                };
                let mut args = vec![outer_elem, inner];
                if let Some(predicate) = carried_pred {
                    args.push(predicate);
                }
                self.mk(ValOp::Hof(kind as u32), args)
            }
            HoFKind::FilterMap => {
                let (elems, carried_pred) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let Some((contrib, own_pred)) = kids
                    .get(1)
                    .and_then(|&lambda| self.eval_filter_map_lambda_body(lambda, &elems, env))
                else {
                    let args: Vec<ValueId> = kids.iter().map(|&kid| self.eval(kid, env)).collect();
                    return self.mk(ValOp::Hof(kind as u32), args);
                };
                match self.and_preds(own_pred, carried_pred) {
                    Some(predicate) => {
                        self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, predicate])
                    }
                    None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                }
            }
            HoFKind::Filter => {
                // `filter(p, coll)` ≡ the identity map with a predicate. This keeps the
                // element stream attached, lets nested filters fuse, and matches filtered
                // comprehensions/builders without letting raw HOF payloads bypass proof.
                let (elems, carried_pred) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let elem = elems
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.fresh_opaque());
                let own_pred = kids
                    .get(1)
                    .and_then(|&lambda| self.eval_lambda_body(lambda, &elems, env));
                match self.and_preds(own_pred, carried_pred) {
                    Some(predicate) => {
                        self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem, predicate])
                    }
                    None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem]),
                }
            }
            HoFKind::Reduce => {
                let args: Vec<ValueId> = kids.iter().map(|&kid| self.eval(kid, env)).collect();
                self.mk(ValOp::Hof(kind as u32), args)
            }
        }
    }

    /// Conjoin two optional predicates (a filter's own predicate and one carried up through
    /// a fused collection): both → `p ∧ q`, one → it, none → none.
    pub(super) fn and_preds(&mut self, a: Option<ValueId>, b: Option<ValueId>) -> Option<ValueId> {
        match (a, b) {
            (Some(x), Some(y)) => Some(self.mk(ValOp::Bin(Op::And as u32), vec![x, y])),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }

    /// The per-element value(s) a map/filter lambda's parameters bind to: a single
    /// `Elem(coll)`, or — for `zip(a, b)` with a tuple pattern — `[Elem(a), Elem(b)]`.
    pub(super) fn elem_bindings(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> Vec<ValueId> {
        self.elem_bindings_with_pred(coll_node, env).0
    }

    pub(super) fn elem_bindings_with_pred(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> (Vec<ValueId>, Option<ValueId>) {
        let Some(c) = coll_node else {
            return (Vec::new(), None);
        };
        if self.il.kind(c) == NodeKind::Call
            && matches!(self.il.node(c).payload, Payload::Builtin(Builtin::Zip))
            && self.admitted_builtin_call(c, Builtin::Zip)
        {
            let mut out = Vec::new();
            for k in self.il.children(c).to_vec() {
                let v = self.eval(k, env);
                out.push(self.elem(v));
            }
            return (out, None);
        }
        let cv = self.eval(c, env);
        let (elem, predicate) = self.collection_elem_with_pred(cv);
        (vec![elem], predicate)
    }

    /// Evaluate a lambda's body with its positional parameters bound to `params`,
    /// returning the value of its first `return` (intermediate assignments update the
    /// local env). Used to unfold a `map`/`reduce` lambda over a canonical `Elem`, and the
    /// `.then` continuation callback (see `rules::promise_then`).
    pub(super) fn lambda_return_source_operator_allowed(&self, lambda: NodeId, op: Op) -> bool {
        let Some(ret) = self.lambda_first_return_expr(lambda) else {
            return false;
        };
        if self.il.kind(ret) != NodeKind::BinOp
            || !matches!(self.il.node(ret).payload, Payload::Op(actual) if actual == op)
        {
            return false;
        }
        source_operator_at_node(self.il, ret).is_some_and(|source| {
            exact_static_membership_predicate_operator(self.il.meta.lang, op, source)
        })
    }

    pub(super) fn lambda_first_return_expr(&self, node: NodeId) -> Option<NodeId> {
        if self.il.kind(node) == NodeKind::Return {
            return self.il.children(node).first().copied();
        }
        if self.il.kind(node) == NodeKind::Block || self.il.kind(node) == NodeKind::Lambda {
            return self
                .il
                .children(node)
                .iter()
                .find_map(|&child| self.lambda_first_return_expr(child));
        }
        None
    }

    pub(super) fn eval_lambda_body(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda).to_vec();
        let mut env = parent_env.clone();
        let mut pi = 0;
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Param {
                if let Payload::Cid(c) = self.il.node(k).payload {
                    if let Some(&v) = params.get(pi) {
                        env.insert(c, v);
                    }
                    pi += 1;
                }
            }
        }
        let body = *kids.last()?;
        self.eval_block_return(body, &mut env)
    }

    pub(super) fn eval_filter_map_lambda_body(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, Option<ValueId>)> {
        match self.eval_filter_map_lambda_result(lambda, params, parent_env)? {
            FilterMapResult::Emit { value, predicate } => Some((value, predicate)),
            FilterMapResult::Drop => None,
        }
    }

    pub(super) fn eval_filter_map_lambda_result(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda).to_vec();
        let mut env = parent_env.clone();
        let mut pi = 0;
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Param {
                if let Payload::Cid(c) = self.il.node(k).payload {
                    if let Some(&v) = params.get(pi) {
                        env.insert(c, v);
                    }
                    pi += 1;
                }
            }
        }
        self.eval_filter_map_output(*kids.last()?, &mut env)
    }

    pub(super) fn eval_filter_map_output(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        match self.il.kind(node) {
            NodeKind::Block => {
                let kids = self.il.children(node).to_vec();
                let n = kids.len();
                for (i, &stmt) in kids.iter().enumerate() {
                    if self.il.kind(stmt) == NodeKind::Assign {
                        self.eval_filter_map_assignment(stmt, env);
                        continue;
                    }
                    if self.il.kind(stmt) == NodeKind::Return || i + 1 == n {
                        return self.eval_filter_map_output(stmt, env);
                    }
                    if !matches!(self.il.kind(stmt), NodeKind::ExprStmt) {
                        continue;
                    }
                    return None;
                }
                None
            }
            NodeKind::Return | NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .and_then(|&expr| self.eval_filter_map_output(expr, env)),
            NodeKind::Assign => {
                self.eval_filter_map_assignment(node, env);
                None
            }
            NodeKind::If => self.eval_filter_map_if(node, env),
            NodeKind::Lit if self.is_null_literal(node) => Some(FilterMapResult::Drop),
            NodeKind::Var if self.is_rust_option_none_node(node) => Some(FilterMapResult::Drop),
            NodeKind::Call => {
                if let Some((receiver, lambda)) = self.rust_option_and_then_call_parts(node) {
                    return self.eval_filter_map_and_then(receiver, lambda, env);
                }
                let value = self
                    .rust_some_call_arg(node)
                    .map(|value| self.eval(value, env))?;
                Some(FilterMapResult::Emit {
                    value,
                    predicate: None,
                })
            }
            _ => None,
        }
    }

    pub(super) fn eval_filter_map_and_then(
        &mut self,
        receiver: NodeId,
        lambda: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        let receiver_result = self.eval_filter_map_output(receiver, env)?;
        let FilterMapResult::Emit { value, predicate } = receiver_result else {
            return Some(FilterMapResult::Drop);
        };
        let inner_result = self.eval_filter_map_lambda_result(lambda, &[value], env)?;
        match inner_result {
            FilterMapResult::Emit {
                value,
                predicate: inner_predicate,
            } => Some(FilterMapResult::Emit {
                value,
                predicate: self.and_preds(predicate, inner_predicate),
            }),
            FilterMapResult::Drop => Some(FilterMapResult::Drop),
        }
    }

    pub(super) fn eval_filter_map_assignment(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        let kids = self.il.children(node).to_vec();
        if kids.len() == 2 && self.il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                let rhs = self.eval(kids[1], env);
                env.insert(c, rhs);
            }
        }
    }

    pub(super) fn eval_filter_map_if(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        let kids = self.il.children(node).to_vec();
        let cond_node = *kids.first()?;
        let then_node = *kids.get(1)?;
        let else_node = *kids.get(2)?;
        let cond = self.eval(cond_node, env);
        let mut then_env = env.clone();
        let then_result = self.eval_filter_map_output(then_node, &mut then_env)?;
        let mut else_env = env.clone();
        let else_result = self.eval_filter_map_output(else_node, &mut else_env)?;
        match (then_result, else_result) {
            (FilterMapResult::Emit { value, predicate }, FilterMapResult::Drop) => {
                Some(FilterMapResult::Emit {
                    value,
                    predicate: self.and_preds(Some(cond), predicate),
                })
            }
            (FilterMapResult::Drop, FilterMapResult::Emit { value, predicate }) => {
                let not_cond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
                Some(FilterMapResult::Emit {
                    value,
                    predicate: self.and_preds(Some(not_cond), predicate),
                })
            }
            (
                FilterMapResult::Emit {
                    value: then_value,
                    predicate: None,
                },
                FilterMapResult::Emit {
                    value: else_value,
                    predicate: None,
                },
            ) => Some(FilterMapResult::Emit {
                value: self.mk(ValOp::Phi, vec![cond, then_value, else_value]),
                predicate: None,
            }),
            _ => None,
        }
    }

    pub(super) fn rust_some_call_arg(&self, node: NodeId) -> Option<NodeId> {
        let kids = self.il.children(node);
        admitted_rust_option_some_constructor_at_call(self.il, self.interner, node)?;
        kids.get(1).copied()
    }

    pub(super) fn rust_option_and_then_call_parts(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        let occurrence = admitted_rust_option_and_then_at_call(self.il, self.interner, node)?;
        let callback = *self.il.children(node).get(1)?;
        Some((occurrence.receiver?, callback))
    }

    pub(super) fn is_rust_vec_new_call(&self, call: NodeId) -> bool {
        admitted_rust_vec_new_factory_at_call(self.il, self.interner, call).is_some()
    }

    pub(super) fn is_rust_option_none_node(&self, node: NodeId) -> bool {
        admitted_rust_option_none_sentinel_at_node(self.il, self.interner, node).is_some()
    }

    pub(super) fn rust_option_some_wildcard_pattern(&self, node: NodeId) -> Option<NodeId> {
        if source_pattern_at_node(self.il, node)
            != Some(SourcePatternKind::RustTupleStructSingleWildcardPattern)
        {
            return None;
        }
        let (NodeKind::Raw, Payload::Name(tag)) = (self.il.kind(node), self.il.node(node).payload)
        else {
            return None;
        };
        if self.interner.resolve(tag) != "tuple_struct_pattern" {
            return None;
        }
        let kids = self.il.children(node);
        let [callee] = kids else {
            return None;
        };
        if !self.is_rust_option_some_node(*callee) {
            return None;
        }
        Some(*callee)
    }

    pub(super) fn is_rust_option_some_node(&self, node: NodeId) -> bool {
        admitted_rust_option_some_constructor_at_node(self.il, self.interner, node).is_some()
    }

    pub(super) fn eval_rust_option_some_pattern_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if op != Op::Eq as u32 || kids.len() != 2 {
            return None;
        }
        self.rust_option_some_wildcard_pattern(kids[1])?;
        if self.domain_evidence_of_expr(kids[0]) != Some(DomainEvidence::Option) {
            return None;
        }
        let value = self.eval(kids[0], env);
        let nil = self.null_const();
        Some(self.mk(ValOp::Bin(Op::Ne as u32), vec![value, nil]))
    }

    pub(super) fn is_null_literal(&self, node: NodeId) -> bool {
        matches!(
            (self.il.kind(node), self.il.node(node).payload),
            (NodeKind::Lit, Payload::Lit(LitClass::Null))
        )
    }
}
