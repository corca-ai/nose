use super::*;

impl<'a> Interp<'a> {
    pub(super) fn exact_field_place(&self, node: NodeId) -> Option<FieldPlace> {
        if self.il.kind(node) == NodeKind::Var && exact_java_this_var(self.il, self.interner, node)
        {
            Some(FieldPlace::SelfReceiver)
        } else {
            None
        }
    }

    pub(super) fn exact_field_write_key(&self, assign: NodeId, target: NodeId) -> Option<FieldKey> {
        if !exact_self_field_write_assignment(self.il, self.interner, assign) {
            return None;
        }
        self.exact_field_key(target)
    }

    pub(super) fn exact_field_key(&self, target: NodeId) -> Option<FieldKey> {
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
        let receiver = self.exact_field_place(receiver)?;
        Some(FieldKey {
            receiver,
            field: stable_symbol_hash(self.interner.resolve(field)),
        })
    }

    pub(super) fn field_receiver_errored(
        &mut self,
        receiver: NodeId,
        env: &mut FxHashMap<u32, Value>,
    ) -> R<bool> {
        if exact_java_this_var(self.il, self.interner, receiver) {
            return Ok(false);
        }
        Ok(matches!(self.eval(receiver, env)?, Value::Err))
    }
    pub(super) fn eval_field(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let Some(&receiver) = self.il.children(node).first() else {
            return Err(Unsupported::il("il.field-receiver-missing"));
        };
        // Proven self-field reads keep their concrete store semantics; an
        // UNWRITTEN self-field reads its (symbolic) initial state.
        if let Some(key) = self.exact_field_key(node) {
            if self.field_receiver_errored(receiver, env)? {
                return Ok(Value::Err);
            }
            return match self.fields.get(&key) {
                Some(v) => Ok(v.clone()),
                None => Ok(Value::Sym(sym_id(0x00F1_E1D0, &[key.field]))),
            };
        }
        // Any other field read is a symbolic projection keyed by the field
        // name and the receiver VALUE (pure-read convention, applied to both
        // sides of a merge alike).
        let Payload::Name(field) = self.il.node(node).payload else {
            return Err(Unsupported::il("il.field-name-missing"));
        };
        let rv = self.eval(receiver, env)?;
        if let Value::KeySet(keys) = &rv {
            if self.interner.resolve(field) == "size" {
                return Ok(Value::Int(keys.len() as i64));
            }
        }
        if matches!(rv, Value::Err) {
            return Ok(Value::Err);
        }
        Ok(Value::Sym(sym_id(
            0x00F1_E1D1,
            &[hashed(&self.interner.resolve(field)), vhash(&rv)],
        )))
    }
}
