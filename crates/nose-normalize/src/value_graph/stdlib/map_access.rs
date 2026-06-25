use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn proven_map_get_value(
        &mut self,
        value: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[node.args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        admitted_map_get_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(
                value,
                args[0],
                Some(callee.args[0]),
                args.len().saturating_sub(1),
            ),
            method,
        )?;
        let map = callee.args[0];
        let map = if self.is_param_value(map, DomainEvidence::Map) {
            map
        } else {
            self.proven_map_value(map)?
        };
        Some((map, args[1]))
    }
    pub(super) fn proven_map_key_view_value(
        &mut self,
        value: ValueId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        self.proven_map_key_view_value_matching(value, MapKeyViewKind::Collection, env)
    }
    fn proven_map_key_view_value_matching(
        &mut self,
        value: ValueId,
        accepted: MapKeyViewKind,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) {
            return None;
        }
        let args = node.args.clone();
        if args.len() == 1 {
            let callee = &self.nodes[args[0] as usize];
            let ValOp::Field(method) = callee.op else {
                return None;
            };
            if callee.args.len() != 1 {
                return None;
            }
            let admitted = admitted_map_key_view_at_call_span(
                self.il,
                self.interner,
                self.library_api_span_call(value, args[0], Some(callee.args[0]), 0),
                method,
            )?;
            if admitted.contract.result.kind != accepted {
                return None;
            }
            let map = callee.args[0];
            return if self.is_param_value(map, DomainEvidence::Map) {
                Some(map)
            } else {
                self.proven_map_value(map)
            };
        }
        if args.len() == 2 {
            let callee = &self.nodes[args[0] as usize];
            let ValOp::Field(method) = callee.op else {
                return None;
            };
            if accepted != MapKeyViewKind::Collection || callee.args.len() != 1 {
                return None;
            }
            if admitted_object_key_view_at_call_span(
                self.il,
                self.interner,
                self.library_api_span_call(value, args[0], callee.args.first().copied(), 1),
                "Object",
                method,
            )
            .is_some()
            {
                let map = js_object_key_view_argument_map_node_at_call_span(
                    self.il,
                    self.interner,
                    self.node_span[value as usize],
                )?;
                let value = self.eval(map, env);
                return self.proven_map_value(value);
            }
            admitted_map_key_view_wrapper_at_call_span(
                self.il,
                self.interner,
                self.library_api_span_call(value, args[0], callee.args.first().copied(), 1),
                "Array",
                method,
            )?;
            return self.proven_map_key_view_value_matching(args[1], MapKeyViewKind::Iterator, env);
        }
        None
    }
    pub(in crate::value_graph) fn proven_map_key_view_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        self.proven_map_key_view_value(value, env)
    }
    pub(in crate::value_graph) fn eval_proven_map_key_membership_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        let admitted = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let result = admitted.contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
            || result.args != MethodBuiltinArgs::FirstThenReceiver
            || !matches!(
                result.receiver,
                MethodReceiverContract::ExactMap
                    | MethodReceiverContract::ExactCollectionOrMap
                    | MethodReceiverContract::ExactSetOrMap
            )
        {
            return None;
        }
        let receiver = admitted.receiver?;
        let key = self.eval(kids[1], env);
        let map = self.resolve_receiver_map_value(receiver, env)?;
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![key, map]))
    }
    /// The proven map [`ValueId`] a map-method `receiver` denotes: the receiver value itself
    /// when it is a map parameter, else a proven map value or a proven local map binding.
    /// `None` when the receiver is not a provable map.
    fn resolve_receiver_map_value(
        &mut self,
        receiver: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let receiver_value = self.eval(receiver, env);
        if self.is_map_param_expr(receiver) {
            Some(receiver_value)
        } else {
            self.proven_map_value(receiver_value)
                .or_else(|| self.proven_local_map_binding_value(receiver, env))
        }
    }
    pub(in crate::value_graph) fn eval_proven_map_get_default_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 3 {
            return None;
        }
        let admitted = admitted_library_method_call_at_call(self.il, self.interner, expr)?;
        let result = admitted.contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::GetOrDefault)
            || result.receiver != MethodReceiverContract::ExactMap
            || !matches!(
                result.args,
                MethodBuiltinArgs::MapGetDefault | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
            )
        {
            return None;
        }
        let receiver = admitted.receiver?;
        let map = self.resolve_receiver_map_value(receiver, env)?;
        let key = self.eval(kids[1], env);
        let default = self.eval_map_get_default_arg(result.args, kids[2], env)?;
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }
    fn eval_map_get_default_arg(
        &mut self,
        contract: MethodBuiltinArgs,
        default: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match contract {
            MethodBuiltinArgs::MapGetDefault => Some(self.eval(default, env)),
            MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda => {
                if self.il.kind(default) == NodeKind::Lambda {
                    return self.eval_zero_arg_lambda_body(default, env);
                }
                Some(self.eval(default, env))
            }
            _ => None,
        }
    }
    fn eval_zero_arg_lambda_body(
        &mut self,
        lambda: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda);
        if kids.len() != 1 {
            return None;
        }
        self.eval_lambda_body(lambda, &[], env)
    }
}
