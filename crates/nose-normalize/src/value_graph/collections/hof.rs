use super::super::*;

impl<'a> Builder<'a> {
    /// The element value(s) a map lambda binds to, plus any predicate CARRIED by the
    /// collection (map/filter fusion). If the collection evaluates to a *filtered* Map
    /// `Hof(Map,[c,p])`, the element is `c` and the carried predicate is `p` — so an outer
    /// map composes into one `filtered-map`, converging `map(h, map(f, filter p))` with the
    /// direct `filtered-map (h∘f)@p`. A pure Map collection is peeled by `elem`; `zip` binds
    /// multiple elements and carries no predicate.
    fn map_source(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
        allow_internal_python_filter: bool,
    ) -> (Vec<ValueId>, Option<ValueId>, Option<ValueId>) {
        let Some(c) = coll_node else {
            return (Vec::new(), None, None);
        };
        if self.il.kind(c) == NodeKind::Call
            && matches!(self.il.node(c).payload, Payload::Builtin(Builtin::Zip))
            && self.admitted_builtin_call(c, Builtin::Zip)
        {
            return (self.elem_bindings(Some(c), env), None, None);
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
        let flat_map_depth_source = matches!(
            self.nodes[cv as usize].op,
            ValOp::Hof(k) if k == HoFKind::FlatMap as u32
        )
        .then_some(cv)
        .or_else(|| {
            matches!(
                self.nodes[cv as usize].op,
                ValOp::Opaque(tag) if tag == UNPROVEN_FLAT_MAP_COORDINATE_TAG
            )
            .then_some(cv)
        });
        if let ValOp::Hof(k) = self.nodes[cv as usize].op {
            if k == HoFKind::Map as u32 && self.nodes[cv as usize].args.len() == 2 {
                let args = self.nodes[cv as usize].args.clone();
                return (vec![args[0]], Some(args[1]), flat_map_depth_source);
            }
        }
        (vec![self.elem(cv)], None, flat_map_depth_source)
    }

    fn eval_map_hof_value(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
        allow_internal_python_filter: bool,
    ) -> ValueId {
        let (elems, carried_pred, flat_map_depth_source) =
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
        if let Some(source) = flat_map_depth_source {
            let mut args = vec![source, contrib];
            args.extend(elems);
            if let Some(predicate) = carried_pred {
                args.push(predicate);
            }
            return self.mk(ValOp::Opaque(UNPROVEN_FLAT_MAP_COORDINATE_TAG), args);
        }
        let aliases_captured_coordinate = elems.iter().any(|&elem| {
            env.values()
                .any(|&captured| self.references(captured, elem))
        });
        let preserves_element_coordinates = elems.iter().all(|&elem| {
            self.references(contrib, elem)
                || carried_pred.is_some_and(|predicate| self.references(predicate, elem))
        });
        if aliases_captured_coordinate || !preserves_element_coordinates {
            // A constant/captured-only contribution with no element-dependent
            // filter still occurs once per source element. The compact Map
            // layout otherwise drops the source entirely, which can merge
            // reductions over different inner collections. An
            // element-dependent carried filter does retain the
            // source/cardinality coordinate and supports count reductions such
            // as `sum(1 for x in xs if p(x))`. A newly bound element that
            // aliases an already captured coordinate remains ambiguous:
            // repeated iteration over the same source needs distinct
            // outer/inner witnesses. Keep every unproven coordinate behind an
            // opaque boundary until multiplicity is modeled.
            let mut args =
                Vec::with_capacity(1 + elems.len() + usize::from(carried_pred.is_some()));
            args.push(contrib);
            args.extend(elems);
            if let Some(predicate) = carried_pred {
                args.push(predicate);
            }
            return self.mk(ValOp::Opaque(SOURCE_INDEPENDENT_MAP_TAG), args);
        }
        match carried_pred {
            Some(predicate) => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, predicate]),
            None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
        }
    }

    pub(in crate::value_graph) fn eval_hof_value(
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
            HoFKind::Map => self.eval_map_hof_value(&kids, env, allow_internal_python_filter),
            HoFKind::FlatMap => {
                let (elems, carried_pred, flat_map_depth_source) =
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
                if let Some(source) = flat_map_depth_source {
                    args.push(source);
                }
                if flat_map_depth_source.is_some()
                    || !matches!(self.nodes[outer_elem as usize].op, ValOp::Elem(_))
                {
                    return self.mk(ValOp::Opaque(UNPROVEN_FLAT_MAP_COORDINATE_TAG), args);
                }
                self.mk(ValOp::Hof(kind as u32), args)
            }
            HoFKind::FilterMap => {
                let (elems, carried_pred, flat_map_depth_source) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let Some((contrib, own_pred)) = kids
                    .get(1)
                    .and_then(|&lambda| self.eval_filter_map_lambda_body(lambda, &elems, env))
                else {
                    let args: Vec<ValueId> = kids.iter().map(|&kid| self.eval(kid, env)).collect();
                    return self.mk(ValOp::Hof(kind as u32), args);
                };
                if let Some(source) = flat_map_depth_source {
                    let mut args = vec![source, contrib];
                    if let Some(predicate) = self.and_preds(own_pred, carried_pred) {
                        args.push(predicate);
                    }
                    return self.mk(ValOp::Opaque(UNPROVEN_FLAT_MAP_COORDINATE_TAG), args);
                }
                match self.and_preds(own_pred, carried_pred) {
                    Some(predicate) => {
                        self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, predicate])
                    }
                    None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                }
            }
            HoFKind::Filter | HoFKind::Reject => {
                // `filter(p, coll)` ≡ the identity map with a predicate. This keeps the
                // element stream attached, lets nested filters fuse, and matches filtered
                // comprehensions/builders without letting raw HOF payloads bypass proof.
                let (elems, carried_pred, flat_map_depth_source) =
                    self.map_source(kids.first().copied(), env, allow_internal_python_filter);
                let elem = elems
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.fresh_opaque());
                let own_pred = kids
                    .get(1)
                    .and_then(|&lambda| self.eval_lambda_body(lambda, &elems, env));
                let own_pred = if kind == HoFKind::Reject {
                    own_pred.map(|predicate| self.mk(ValOp::Un(Op::Not as u32), vec![predicate]))
                } else {
                    own_pred
                };
                if let Some(source) = flat_map_depth_source {
                    let mut args = vec![source, elem];
                    if let Some(predicate) = self.and_preds(own_pred, carried_pred) {
                        args.push(predicate);
                    }
                    return self.mk(ValOp::Opaque(UNPROVEN_FLAT_MAP_COORDINATE_TAG), args);
                }
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
    pub(in crate::value_graph) fn and_preds(
        &mut self,
        a: Option<ValueId>,
        b: Option<ValueId>,
    ) -> Option<ValueId> {
        match (a, b) {
            (Some(x), Some(y)) => Some(self.mk(ValOp::Bin(Op::And as u32), vec![x, y])),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }

    /// The per-element value(s) a map/filter lambda's parameters bind to: a single
    /// `Elem(coll)`, or — for `zip(a, b)` with a tuple pattern — `[Elem(a), Elem(b)]`.
    fn elem_bindings(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> Vec<ValueId> {
        self.elem_bindings_with_pred(coll_node, env).0
    }

    pub(in crate::value_graph) fn elem_bindings_with_pred(
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
    pub(in crate::value_graph) fn lambda_return_source_operator_allowed(
        &self,
        lambda: NodeId,
        op: Op,
    ) -> bool {
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

    fn lambda_first_return_expr(&self, node: NodeId) -> Option<NodeId> {
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

    pub(in crate::value_graph) fn eval_lambda_body(
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

    /// Whether one lexical callback parameter is mentioned by the callback
    /// body. This preserves binding-coordinate evidence even when evaluation
    /// substitutes that parameter with a composite emitted value whose exact
    /// node may be rebuilt by canonicalization.
    pub(in crate::value_graph) fn lambda_param_referenced(
        &self,
        lambda: NodeId,
        param_index: usize,
    ) -> bool {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return false;
        }
        let kids = self.il.children(lambda);
        let Some(param) = kids
            .iter()
            .copied()
            .filter(|&child| self.il.kind(child) == NodeKind::Param)
            .nth(param_index)
        else {
            return false;
        };
        let Payload::Cid(cid) = self.il.node(param).payload else {
            return false;
        };
        kids.last()
            .is_some_and(|&body| mentioned_cids(self.il, body).contains(&cid))
    }

    fn eval_filter_map_lambda_body(
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

    fn eval_filter_map_lambda_result(
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

    fn eval_filter_map_output(
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
            NodeKind::Var if self.il.meta.lang == Lang::Swift => Some(FilterMapResult::Emit {
                value: self.eval(node, env),
                predicate: None,
            }),
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

    fn eval_filter_map_and_then(
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

    fn eval_filter_map_assignment(&mut self, node: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(node).to_vec();
        if kids.len() == 2 && self.il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                let rhs = self.eval(kids[1], env);
                env.insert(c, rhs);
            }
        }
    }

    fn eval_filter_map_if(
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

    fn is_null_literal(&self, node: NodeId) -> bool {
        matches!(
            (self.il.kind(node), self.il.node(node).payload),
            (NodeKind::Lit, Payload::Lit(LitClass::Null))
        )
    }
}
