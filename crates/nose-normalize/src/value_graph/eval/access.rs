use super::super::*;

const SWIFT_UNPROVEN_DEFAULT_SUBSCRIPT_TAG: u64 = 0x5357_4446;

impl<'a> Builder<'a> {
    pub(super) fn eval_field_expr(
        &mut self,
        expr: NodeId,
        payload: Payload,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let kids = self.il.children(expr).to_vec();
        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        let name = match payload {
            Payload::Name(s) => self.interner.symbol_hash(s),
            _ => 0,
        };
        if a.len() == 1 {
            if let Some(v) = self.eval_field_builtin_or_import(expr, payload, &kids, &a) {
                return v;
            }
        }
        if a.len() == 1 {
            let Some(key) = self.exact_field_state_key(expr) else {
                return self.mk(ValOp::Field(name), a);
            };
            if let Some(&written) = self.field_env.get(&key) {
                return written;
            }
        }
        self.mk(ValOp::Field(name), a)
    }
    fn eval_field_builtin_or_import(
        &mut self,
        expr: NodeId,
        payload: Payload,
        kids: &[NodeId],
        a: &[ValueId],
    ) -> Option<ValueId> {
        if let Some(admitted) = admitted_property_builtin_at_field(self.il, self.interner, expr) {
            if admitted.contract.result == Builtin::Len {
                if let Some(len) = self.eval_len_value(a[0]) {
                    return Some(len);
                }
                if self
                    .domain_evidence_of_expr(kids[0])
                    .is_some_and(DomainEvidence::is_array_or_collection)
                {
                    return Some(self.mk(
                        ValOp::Call(builtin_tag(admitted.contract.result)),
                        a.to_vec(),
                    ));
                }
            }
        }
        if let Payload::Name(s) = payload {
            let receiver = &self.nodes[a[0] as usize];
            if let ValOp::ImportNamespace { module_hash } = receiver.op {
                return Some(self.mk(
                    ValOp::ImportBinding {
                        module_hash,
                        exported_hash: stable_symbol_hash(self.interner.resolve(s)),
                    },
                    vec![],
                ));
            }
        }
        None
    }
    pub(super) fn eval_index_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        if swift_dictionary_default_subscript_surface_for_node(self.il, self.interner, expr)
            .is_some()
        {
            if let Some(contract) =
                swift_dictionary_default_subscript_contract_for_node(self.il, self.interner, expr)
            {
                let map = self.eval(contract.receiver, env);
                let key = self.eval(contract.key, env);
                let default = self.eval(contract.default, env);
                return self.mk(
                    ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                    vec![map, key, default],
                );
            }
            // The default expression is an autoclosure and therefore cannot be
            // evaluated with ordinary eager Index children. Preserve the whole
            // unsupported source as an opaque, source-salted boundary before
            // walking any child; otherwise a lazy call can collapse with a
            // hoisted eager call when the key is present.
            return self.source_salted_opaque(expr, SWIFT_UNPROVEN_DEFAULT_SUBSCRIPT_TAG);
        }
        let kids = self.il.children(expr).to_vec();
        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        if a.len() == 2 {
            // #337: a read of an element written earlier in this straight-line run forwards to
            // the written value, so `clobber`'s post-write `a[i]` read (= the value just put in
            // `a[j]`) differs from `swap`'s pre-write read. Only fires for a place that was the
            // most recent indexed write with no intervening effect (see `record_index_write`).
            if let Some(forwarded) = self.forwarded_index_read(a[0], a[1]) {
                return forwarded;
            }
            if let Some((map, default)) = self.proven_go_literal_zero_map_value(a[0]) {
                return self.mk(
                    ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                    vec![map, a[1], default],
                );
            }
        }
        self.mk(ValOp::Index, a)
    }
}
