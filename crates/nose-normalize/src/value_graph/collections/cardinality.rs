use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn eval_len_builtin(
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

    pub(in crate::value_graph) fn eval_terminal_count_builtin(
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

    pub(in crate::value_graph) fn len_arg_admitted(&self, arg: NodeId) -> bool {
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
                Payload::HoF(kind) => admitted_hof_demand_effect_profile_at_node_with_interner(
                    self.il,
                    Some(self.interner),
                    arg,
                    kind,
                )
                .is_some_and(|profile| profile.proves_eager_per_element_callback_demand()),
                _ => false,
            },
        }
    }

    pub(in crate::value_graph) fn terminal_reduction_arg_admitted(&self, arg: NodeId) -> bool {
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
                Payload::HoF(kind) => admitted_hof_demand_effect_profile_at_node_with_interner(
                    self.il,
                    Some(self.interner),
                    arg,
                    kind,
                )
                .is_some(),
                _ => false,
            },
        }
    }

    pub(in crate::value_graph) fn hof_value_admission(
        &self,
        node: NodeId,
        kind: HoFKind,
    ) -> Option<HofAdmission> {
        match source_comprehension_at_node(self.il, node) {
            Some(
                SourceComprehensionKind::PythonDictComprehension
                | SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonListComprehension,
            ) => Some(HofAdmission::SourceComprehension),
            Some(SourceComprehensionKind::PythonSetComprehension) => None,
            None if admitted_hof_demand_effect_profile_at_node_with_interner(
                self.il,
                Some(self.interner),
                node,
                kind,
            )
            .is_some() =>
            {
                Some(HofAdmission::LibraryApi)
            }
            None => None,
        }
    }

    pub(in crate::value_graph) fn eval_len_value(&mut self, value: ValueId) -> Option<ValueId> {
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

    pub(in crate::value_graph) fn eval_len_zero_comparison(
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

    pub(in crate::value_graph) fn eval_static_filter_membership_comparison(
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

    fn static_filter_membership_parts(
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

    fn is_count_nonempty_threshold(
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

    fn is_count_zero_threshold(&self, op: u32, count_on_right: bool, threshold: NodeId) -> bool {
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

    pub(in crate::value_graph) fn eval_static_index_membership_comparison(
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

    fn static_index_membership_parts(
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

    fn is_index_membership_threshold(
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

    fn is_minus_one_literal(&self, node: NodeId) -> bool {
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

    fn len_call_arg(&self, node: NodeId) -> Option<NodeId> {
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

    fn is_zero_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(0))
    }

    fn is_one_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(1))
    }
}
