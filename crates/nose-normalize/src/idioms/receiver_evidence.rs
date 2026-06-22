use super::*;

pub(super) fn exact_collection_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_protocol_receiver(old, interner, domains, node)
}

pub(super) fn exact_protocol_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if exact_collection_literal(old, interner, node) || exact_collection_param(domains, node) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = old.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if old.kind(callee) != NodeKind::Field {
        return false;
    }
    let Some(&receiver) = old.children(callee).first() else {
        return false;
    };
    if let Some(arg) = static_collection_adapter_arg(old, interner, node, kids) {
        return exact_collection_receiver(old, interner, domains, arg);
    }
    let admitted_method = admitted_library_method_call_at_call(old, interner, node);
    if let Some(admitted) = admitted_method {
        let contract = admitted.contract;
        if contract.result.semantic == MethodSemanticContract::Builtin(Builtin::Zip)
            && contract.result.receiver == MethodReceiverContract::ExactProtocolPairArgument
            && contract.result.args == MethodBuiltinArgs::RustZip
        {
            return exact_protocol_receiver(old, interner, domains, receiver)
                && exact_protocol_receiver(old, interner, domains, kids[1]);
        }
    }
    if admitted_iterator_identity_adapter_at_call(old, interner, node).is_some() {
        return exact_protocol_receiver(old, interner, domains, receiver);
    }
    if let Some(admitted) = admitted_method {
        let contract = admitted.contract;
        if matches!(contract.result.semantic, MethodSemanticContract::HoF(_))
            && admitted.arg_count >= 1
        {
            return exact_protocol_receiver(old, interner, domains, receiver);
        }
    }
    false
}

pub(super) fn exact_collection_param(
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::ARRAY_COLLECTION_OR_SET)
}

pub(super) fn static_collection_adapter_arg(
    old: &Il,
    interner: &Interner,
    call: NodeId,
    call_kids: &[NodeId],
) -> Option<NodeId> {
    admitted_static_collection_adapter_at_call(old, interner, call)?;
    call_kids.get(1).copied()
}

pub(super) fn exact_set_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::SET)
}

pub(super) fn exact_map_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::MAP)
}

pub(super) fn exact_map_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_map_param(domains, node) || map_like_literal(old, interner, node)
}

pub(super) fn exact_option_param(domains: &ReceiverDomainEvidenceIndex<'_>, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(node, DomainRequirement::OPTION)
}

pub(super) fn exact_collection_literal(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if old.kind(node) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(old, interner, node)
        .is_some_and(|contract| contract.membership_collection)
}

pub(super) fn exact_option_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if exact_option_param(domains, node) {
        return true;
    }
    if matches!(
        old.node(node).payload,
        Payload::Lit(nose_il::LitClass::Null)
    ) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    admitted_rust_option_some_constructor_at_call(old, interner, node).is_some()
}

pub(super) fn exact_string_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitStr(_) | Payload::Lit(nose_il::LitClass::Str)
    ) || domains.receiver_satisfies_domain(node, DomainRequirement::STRING)
}

pub(super) fn literal_string_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    exact_string_receiver(old, domains, node)
}

pub(super) fn exact_integer_receiver(
    old: &Il,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitInt(_) | Payload::Lit(nose_il::LitClass::Int)
    ) || domains.receiver_satisfies_domain(node, DomainRequirement::INTEGER)
}
