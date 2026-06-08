//! Evidence-gated same-unit field state for value-graph construction.
//!
//! proof-obligation: normalize.value_graph.field_writes

use super::*;
use nose_semantics::{
    exact_java_this_field, exact_java_this_var, exact_self_field_write_assignment,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FieldStateKey {
    receiver: ValueId,
    field: u64,
}

impl<'a> Builder<'a> {
    /// Flush accumulated exact field writes to sinks: one (receiver, field-name, final-value)
    /// sink per distinct proven place, in canonical place order. See `field_env`.
    pub(super) fn flush_fields(&mut self) {
        let mut entries: Vec<(FieldStateKey, ValueId)> = self.field_env.drain().collect();
        entries.sort_unstable_by_key(|(key, _)| {
            (
                self.vhash[key.receiver as usize],
                key.field,
                key.receiver as u64,
            )
        });
        for (key, v) in entries {
            let f = self.mk(ValOp::Field(key.field), vec![key.receiver, v]);
            self.sinks.push(Sink::new(SinkKind::Effect, f));
        }
    }

    pub(super) fn exact_field_write_state_key(
        &mut self,
        assign: NodeId,
        target: NodeId,
    ) -> Option<FieldStateKey> {
        if !exact_self_field_write_assignment(self.il, self.interner, assign) {
            return None;
        }
        self.exact_field_state_key(target)
    }

    pub(super) fn exact_field_state_key(&mut self, target: NodeId) -> Option<FieldStateKey> {
        if self.il.kind(target) != NodeKind::Field {
            return None;
        }
        if !exact_java_this_field(self.il, self.interner, target) {
            return None;
        }
        let Payload::Name(field) = self.il.node(target).payload else {
            return None;
        };
        let receiver = self.il.children(target).first().copied()?;
        let receiver = self.exact_var_place_value(receiver)?;
        Some(FieldStateKey {
            receiver,
            field: self.interner.symbol_hash(field),
        })
    }

    fn exact_var_place_value(&mut self, node: NodeId) -> Option<ValueId> {
        if !exact_java_this_var(self.il, self.interner, node) {
            return None;
        }
        self.var_place_value(node)
    }

    fn var_place_value(&mut self, node: NodeId) -> Option<ValueId> {
        match self.il.node(node).payload {
            Payload::Cid(cid) => Some(self.mk(ValOp::Input(cid), vec![])),
            Payload::Name(name) => Some(self.mk(ValOp::Input(self.free_name_key(name)), vec![])),
            _ => None,
        }
    }
}
