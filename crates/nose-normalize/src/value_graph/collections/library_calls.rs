use super::super::*;
use nose_semantics::js_like_lang;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn filter_parts(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
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

    pub(in crate::value_graph) fn eval_filter_count(
        &mut self,
        filter_node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let (source, predicate) = self.filter_parts(filter_node)?;
        self.eval_predicate_count(source, predicate, env)
    }

    fn eval_predicate_count(
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

    pub(in crate::value_graph) fn eval_count_call(
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

    pub(in crate::value_graph) fn eval_product_call(
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

    pub(in crate::value_graph) fn eval_bool_reduction_method_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let occurrence = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let result = occurrence.contract.result;
        if result.args != MethodBuiltinArgs::BoolReduction {
            return None;
        }
        let all = match result.semantic {
            MethodSemanticContract::Builtin(Builtin::Any) => false,
            MethodSemanticContract::Builtin(Builtin::All) => true,
            _ => return None,
        };
        let lambda = *kids.get(1)?;
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        if js_like_lang(self.il.meta.lang) && self.lambda_param_count(lambda) != 1 {
            return None;
        }
        let coll = self.eval(occurrence.receiver?, env);
        if all && self.js_like_array_param_universal_closed(coll) {
            return None;
        }
        let (elem, carried_guard) = self.collection_elem_with_pred(coll);
        let pred = self.eval_lambda_body(lambda, &[elem], env)?;
        let contrib = if let Some(carried_guard) = carried_guard {
            let ident = self.bool_const_value(!all);
            self.mk(ValOp::Phi, vec![carried_guard, pred, ident])
        } else {
            pred
        };
        let code = if all { REDUCE_ALL } else { REDUCE_ANY };
        Some(self.mk(ValOp::Reduce(code), vec![contrib]))
    }

    pub(in crate::value_graph) fn eval_iterator_identity_adapter(
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

    fn iterator_adapter_receiver_proven(
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

    pub(in crate::value_graph) fn eval_rust_map_get_unwrap_or_call(
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

    pub(in crate::value_graph) fn eval_rust_map_get_is_some_call(
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
}
