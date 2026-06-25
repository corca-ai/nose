use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn is_effect_free_static_err_body(
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

    pub(in crate::value_graph) fn assign_is_static_runtime_err(
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

    pub(in crate::value_graph) fn assignment_target_is_static_runtime_err(
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

    pub(in crate::value_graph) fn expr_is_static_runtime_err(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.il.kind(expr) == NodeKind::Seq {
            return self.seq_is_static_runtime_err(expr, env);
        }
        if self.il.kind(expr) == NodeKind::HoF
            && matches!(
                self.il.node(expr).payload,
                Payload::HoF(HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter)
                    | Payload::HoF(HoFKind::FilterMap)
            )
        {
            return self.hof_is_static_runtime_err(expr, env);
        }
        if self.il.kind(expr) == NodeKind::If {
            return self.if_is_static_runtime_err(expr, env);
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
        self.binop_is_static_runtime_err(expr, env)
    }

    fn seq_is_static_runtime_err(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        for child in self.il.children(expr).to_vec() {
            if self.expr_is_static_runtime_err(child, env) {
                return crate::is_pure(self.il, child);
            }
            if !crate::is_pure(self.il, child) {
                return false;
            }
        }
        false
    }

    fn hof_is_static_runtime_err(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        let Payload::HoF(kind) = self.il.node(expr).payload else {
            return false;
        };
        let demand = admitted_hof_demand_effect_profile_at_node_with_interner(
            self.il,
            Some(self.interner),
            expr,
            kind,
        );
        let Some(demand) = demand else { return false };
        if !demand.proves_eager_per_element_callback_demand() {
            return false;
        }
        let kids = self.il.children(expr).to_vec();
        kids.first()
            .is_some_and(|&coll| self.expr_is_static_non_empty_seq(coll))
            && kids
                .get(1)
                .is_some_and(|&lambda| self.lambda_body_is_static_runtime_err(lambda, env))
    }

    fn if_is_static_runtime_err(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        let kids = self.il.children(expr).to_vec();
        let Some(&cond) = kids.first() else {
            return false;
        };
        if self.expr_is_static_runtime_err(cond, env) {
            return true;
        }
        let cond_value = self.eval(cond, env);
        match self.bool_const(cond_value) {
            Some(true) => kids
                .get(1)
                .is_some_and(|&then_expr| self.expr_is_static_runtime_err(then_expr, env)),
            Some(false) => kids
                .get(2)
                .is_some_and(|&else_expr| self.expr_is_static_runtime_err(else_expr, env)),
            None => false,
        }
    }

    fn binop_is_static_runtime_err(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
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
            Op::Div | Op::FloorDiv | Op::TrueDiv | Op::Mod | Op::FloorMod => {
                self.int_const_eq(rhs, 0)
            }
            Op::Pow => self
                .static_int_expr(kids[1])
                .is_some_and(|exp| !(0..=u32::MAX as i64).contains(&exp)),
            _ => false,
        }
    }

    pub(in crate::value_graph) fn call_has_static_runtime_arg_err(
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
            Payload::Builtin(builtin) if self.admitted_builtin_call(call, builtin) => {
                self.call_args_have_static_runtime_err(kids, env)
            }
            Payload::Builtin(_) => false,
            _ => self.call_args_have_static_runtime_err(kids.into_iter().skip(1), env),
        }
    }

    pub(in crate::value_graph) fn range_has_static_zero_step(&self, kids: &[NodeId]) -> bool {
        kids.len() == 3
            && kids.iter().all(|&arg| crate::is_pure(self.il, arg))
            && self.static_int_expr(kids[2]) == Some(0)
    }

    pub(in crate::value_graph) fn call_args_have_static_runtime_err<I>(
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

    pub(in crate::value_graph) fn expr_is_static_non_empty_seq(&self, expr: NodeId) -> bool {
        self.il.kind(expr) == NodeKind::Seq && !self.il.children(expr).is_empty()
    }

    pub(in crate::value_graph) fn lambda_body_is_static_runtime_err(
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

    pub(in crate::value_graph) fn static_int_expr(&self, expr: NodeId) -> Option<i64> {
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
}
