use super::*;

pub(crate) fn strict_exact_collection_contains_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
        || result.args != MethodBuiltinArgs::FirstThenReceiver
    {
        return false;
    }
    let receiver_safe = match result.receiver {
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            let Some(receiver) = field_receiver(il, callee) else {
                return false;
            };
            strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
                || strict_exact_proven_collection_receiver_safe(il, interner, facts, receiver)
                || strict_exact_python_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, receiver)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, receiver)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_java_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_map_key_view_collection_safe(il, interner, facts, receiver)
        }
        MethodReceiverContract::ExactSetOrMap => {
            let Some(receiver) = field_receiver(il, callee) else {
                return false;
            };
            strict_exact_typed_set_param_receiver_safe(il, interner, facts, receiver)
                || strict_exact_set_constructor_collection_safe(il, interner, facts, receiver)
        }
        _ => false,
    };
    receiver_safe && strict_exact_call_args_safe(il, interner, facts, node)
}

pub(super) fn strict_exact_map_contains_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
        || result.args != MethodBuiltinArgs::FirstThenReceiver
        || !matches!(
            result.receiver,
            MethodReceiverContract::ExactMap
                | MethodReceiverContract::ExactCollectionOrMap
                | MethodReceiverContract::ExactSetOrMap
        )
    {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, true)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

pub(super) fn strict_exact_map_get_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    method: &str,
) -> bool {
    if method != "get" || admitted_map_get_at_call(il, interner, node).is_none() {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, false)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

pub(super) fn strict_exact_map_get_default_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::GetOrDefault)
        || result.receiver != MethodReceiverContract::ExactMap
        || !matches!(
            result.args,
            MethodBuiltinArgs::MapGetDefault | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
        )
    {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, false)
        && strict_exact_map_get_default_args_safe(il, interner, facts, node, result.args)
}

fn strict_exact_map_receiver_or_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
    allow_rust_std_factory: bool,
) -> bool {
    strict_exact_proven_map_receiver_safe(il, interner, facts, receiver)
        || strict_exact_java_map_factory_safe(il, interner, facts, receiver)
        || strict_exact_map_constructor_entries_safe(il, interner, facts, receiver)
        || (allow_rust_std_factory
            && strict_exact_rust_std_map_factory_safe(il, interner, facts, receiver))
}

fn strict_exact_map_get_default_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    contract: MethodBuiltinArgs,
) -> bool {
    let kids = il.children(node);
    let [_, key, default] = kids else {
        return false;
    };
    strict_exact_safe_tree(il, interner, facts, *key)
        && match contract {
            MethodBuiltinArgs::MapGetDefault => {
                strict_exact_safe_tree(il, interner, facts, *default)
            }
            MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda => {
                strict_exact_map_default_value_arg_safe(il, interner, facts, *default)
            }
            _ => false,
        }
}

fn strict_exact_map_default_value_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    default: NodeId,
) -> bool {
    if il.kind(default) != NodeKind::Lambda {
        return strict_exact_safe_tree(il, interner, facts, default);
    }
    let kids = il.children(default);
    let [body] = kids else {
        return false;
    };
    let value = implicit_single_value_body(il, *body).unwrap_or(*body);
    strict_exact_safe_tree(il, interner, facts, value)
}

fn implicit_single_value_body(il: &Il, body: NodeId) -> Option<NodeId> {
    if il.kind(body) != NodeKind::Block {
        return None;
    }
    let [stmt] = il.children(body) else {
        return None;
    };
    match il.kind(*stmt) {
        NodeKind::ExprStmt | NodeKind::Return => il.children(*stmt).first().copied(),
        _ => None,
    }
}

pub(super) fn strict_exact_iterator_identity_adapter_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    method: &str,
) -> bool {
    if method.is_empty() {
        return false;
    }
    let Some(admitted) = admitted_iterator_identity_adapter_at_call(il, interner, node) else {
        return false;
    };
    let Some(receiver) = admitted.receiver else {
        return false;
    };
    if admitted.callee != callee {
        return false;
    }
    strict_exact_iterator_receiver_safe(il, interner, facts, receiver)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_iterator_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_proven_collection_receiver_safe(il, interner, facts, receiver)
        || strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
        || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, receiver)
        || strict_exact_rust_std_collection_factory_safe(il, interner, facts, receiver)
        || strict_exact_rust_vec_new_safe(il, interner, receiver)
        || strict_exact_iterator_identity_adapter_node_safe(il, interner, facts, receiver)
}

fn strict_exact_iterator_identity_adapter_node_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    strict_exact_iterator_identity_adapter_call_safe(
        il,
        interner,
        facts,
        node,
        callee,
        interner.resolve(method),
    )
}

fn strict_exact_typed_set_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(il, interner, facts, receiver, DomainRequirement::SET)
}

fn strict_exact_typed_collection_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(
        il,
        interner,
        facts,
        receiver,
        DomainRequirement::ARRAY_COLLECTION_OR_SET,
    )
}

fn strict_exact_typed_receiver_safe(
    il: &Il,
    _interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
    requirement: DomainRequirement,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    facts.receiver_satisfies_domain(receiver, requirement)
}

pub(super) fn strict_exact_proven_collection_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_collection_param_receiver_safe(il, interner, facts, receiver)
}

fn strict_exact_typed_map_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(il, interner, facts, receiver, DomainRequirement::MAP)
}

pub(super) fn strict_exact_proven_map_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_map_param_receiver_safe(il, interner, facts, receiver)
}

fn strict_exact_map_key_view_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_map_key_view_safe_matching(il, interner, facts, node, |kind| {
        kind == MapKeyViewKind::Collection
    })
}

fn strict_exact_map_key_view_safe_matching(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    accepts: impl Fn(MapKeyViewKind) -> bool + Copy,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 1 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Some(admitted) = admitted_map_key_view_at_call(il, interner, node) else {
        return false;
    };
    let result = admitted.contract.result;
    if !accepts(result.kind) {
        return false;
    }
    let Some(receiver) = admitted.receiver else {
        return false;
    };
    strict_exact_proven_map_receiver_safe(il, interner, facts, receiver)
        || strict_exact_map_constructor_entries_safe(il, interner, facts, receiver)
        || strict_exact_java_map_factory_safe(il, interner, facts, receiver)
        || strict_exact_rust_std_map_factory_safe(il, interner, facts, receiver)
}

pub(super) fn strict_exact_map_key_view_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if strict_exact_map_key_view_safe(il, interner, facts, node) {
        return true;
    }
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Some(_admitted) = admitted_map_key_view_wrapper_at_call(il, interner, node) else {
        return false;
    };
    strict_exact_map_key_view_safe_matching(il, interner, facts, kids[1], |kind| {
        kind == MapKeyViewKind::Iterator
    })
}

fn strict_exact_literal_collection_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.kind(node) == NodeKind::Seq
        && strict_exact_membership_collection_safe(il, interner, facts, node)
}

pub(crate) fn strict_exact_membership_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        if il.kind(node) == NodeKind::Call {
            return strict_exact_set_constructor_collection_safe(il, interner, facts, node)
                || strict_exact_python_collection_factory_safe(il, interner, facts, node)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, node)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, node)
                || strict_exact_java_collection_factory_safe(il, interner, facts, node)
                || strict_exact_map_key_view_collection_safe(il, interner, facts, node);
        }
        if strict_exact_proven_collection_receiver_safe(il, interner, facts, node)
            || strict_exact_proven_map_receiver_safe(il, interner, facts, node)
        {
            return true;
        }
        return false;
    }
    let tag_safe = seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.membership_collection);
    tag_safe
        && il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
}
