use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn rust_some_call_arg(&self, node: NodeId) -> Option<NodeId> {
        let kids = self.il.children(node);
        admitted_rust_option_some_constructor_at_call(self.il, self.interner, node)?;
        kids.get(1).copied()
    }

    pub(in crate::value_graph) fn rust_option_and_then_call_parts(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, NodeId)> {
        let occurrence = admitted_rust_option_and_then_at_call(self.il, self.interner, node)?;
        let callback = *self.il.children(node).get(1)?;
        Some((occurrence.receiver?, callback))
    }

    pub(in crate::value_graph) fn is_rust_vec_new_call(&self, call: NodeId) -> bool {
        admitted_rust_vec_new_factory_at_call(self.il, self.interner, call).is_some()
    }

    pub(in crate::value_graph) fn is_rust_option_none_node(&self, node: NodeId) -> bool {
        admitted_rust_option_none_sentinel_at_node(self.il, self.interner, node).is_some()
    }

    pub(in crate::value_graph) fn eval_rust_result_predicate_call(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_rust_result_predicate_at_call(self.il, self.interner, node)?;
        let receiver = occurrence.receiver?;
        let channel = match occurrence.contract.id {
            LibraryApiContractId::RustResultIsOk => self.rust_result_ok_channel(),
            LibraryApiContractId::RustResultIsErr => self.rust_result_err_channel(),
            _ => return None,
        };
        let receiver = self.eval(receiver, env);
        Some(self.mk(ValOp::Bin(Op::Eq as u32), vec![receiver, channel]))
    }

    fn rust_option_some_wildcard_pattern(&self, node: NodeId) -> Option<NodeId> {
        if source_pattern_at_node(self.il, node)
            != Some(SourcePatternKind::RustTupleStructSingleWildcardPattern)
        {
            return None;
        }
        let (kind, Payload::Name(tag)) = (self.il.kind(node), self.il.node(node).payload) else {
            return None;
        };
        let tag = self.interner.resolve(tag);
        if !matches!(
            (kind, tag),
            (NodeKind::Raw, "tuple_struct_pattern") | (NodeKind::Seq, "rust_tuple_struct_pattern")
        ) {
            return None;
        }
        let kids = self.il.children(node);
        let callee = kids.first().copied()?;
        if !self.is_rust_option_some_node(callee) {
            return None;
        }
        Some(callee)
    }

    fn is_rust_option_some_node(&self, node: NodeId) -> bool {
        admitted_rust_option_some_constructor_at_node(self.il, self.interner, node).is_some()
    }

    fn rust_result_wildcard_pattern(&mut self, node: NodeId) -> Option<(NodeId, ValueId)> {
        if source_pattern_at_node(self.il, node)
            != Some(SourcePatternKind::RustTupleStructSingleWildcardPattern)
        {
            return None;
        }
        let (kind, Payload::Name(tag)) = (self.il.kind(node), self.il.node(node).payload) else {
            return None;
        };
        let tag = self.interner.resolve(tag);
        if !matches!(
            (kind, tag),
            (NodeKind::Raw, "tuple_struct_pattern") | (NodeKind::Seq, "rust_tuple_struct_pattern")
        ) {
            return None;
        }
        let callee = self.il.children(node).first().copied()?;
        if admitted_rust_result_ok_constructor_at_node(self.il, self.interner, callee).is_some() {
            return Some((callee, self.rust_result_ok_channel()));
        }
        if admitted_rust_result_err_constructor_at_node(self.il, self.interner, callee).is_some() {
            return Some((callee, self.rust_result_err_channel()));
        }
        None
    }

    pub(in crate::value_graph) fn eval_rust_option_some_pattern_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if op != Op::Eq as u32 || kids.len() != 2 {
            return None;
        }
        let (value_node, pattern_node) =
            if self.rust_option_some_wildcard_pattern(kids[1]).is_some() {
                (kids[0], kids[1])
            } else if self.rust_option_some_wildcard_pattern(kids[0]).is_some() {
                (kids[1], kids[0])
            } else {
                return None;
            };
        let _ = pattern_node;
        if self.domain_evidence_of_expr(value_node) != Some(DomainEvidence::Option) {
            return None;
        }
        let value = self.eval(value_node, env);
        let nil = self.null_const();
        Some(self.mk(ValOp::Bin(Op::Ne as u32), vec![value, nil]))
    }

    pub(in crate::value_graph) fn eval_rust_result_wildcard_pattern_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if op != Op::Eq as u32 || kids.len() != 2 {
            return None;
        }
        let (value_node, channel) =
            if let Some((_, channel)) = self.rust_result_wildcard_pattern(kids[1]) {
                (kids[0], channel)
            } else if let Some((_, channel)) = self.rust_result_wildcard_pattern(kids[0]) {
                (kids[1], channel)
            } else {
                return None;
            };
        if self.domain_evidence_of_expr(value_node) != Some(DomainEvidence::Result) {
            return None;
        }
        let value = self.eval(value_node, env);
        Some(self.mk(ValOp::Bin(Op::Eq as u32), vec![value, channel]))
    }

    fn rust_result_ok_channel(&mut self) -> ValueId {
        self.sentinel_const(sentinel::RUST_RESULT_OK)
    }

    fn rust_result_err_channel(&mut self) -> ValueId {
        self.sentinel_const(sentinel::RUST_RESULT_ERR)
    }
}
