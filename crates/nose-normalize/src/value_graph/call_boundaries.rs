use super::*;

impl<'a> Builder<'a> {
    pub(super) fn is_unproven_membership_like_call(&self, expr: NodeId, kids: &[NodeId]) -> bool {
        if matches!(self.il.node(expr).payload, Payload::Builtin(_)) {
            return false;
        }
        let Some(&callee) = kids.first() else {
            return false;
        };
        if self.il.kind(callee) != NodeKind::Field {
            return false;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return false;
        };
        unproven_membership_like_method_contract(
            self.il.meta.lang,
            self.interner.resolve(name),
            kids.len().saturating_sub(1),
        )
        .is_some()
    }

    pub(super) fn is_unproven_sequence_hof_like_call(&self, expr: NodeId, kids: &[NodeId]) -> bool {
        if self.il.meta.lang != Lang::Ruby {
            return false;
        }
        if matches!(self.il.node(expr).payload, Payload::Builtin(_)) {
            return false;
        }
        let Some(&callee) = kids.first() else {
            return false;
        };
        if self.il.kind(callee) != NodeKind::Field {
            return false;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return false;
        };
        let Some(contract) = library_method_call_contract(
            self.il.meta.lang,
            self.interner.resolve(name),
            kids.len().saturating_sub(1),
        ) else {
            return false;
        };
        matches!(
            contract.id,
            LibraryApiContractId::MethodCall(
                MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::Reject)
                    | MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
            )
        ) && matches!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ExactArrayOrCollection,
                ..
            }
        )
    }
}
