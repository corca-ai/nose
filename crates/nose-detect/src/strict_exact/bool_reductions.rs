use super::*;
use nose_semantics::js_like_lang;

pub(super) fn strict_exact_bool_reduction_method_call_safe(
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
    if result.args != MethodBuiltinArgs::BoolReduction {
        return false;
    }
    let all = match result.semantic {
        MethodSemanticContract::Builtin(Builtin::Any) => false,
        MethodSemanticContract::Builtin(Builtin::All) => true,
        _ => return false,
    };
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    if js_like_lang(il.meta.lang) && !strict_exact_bool_reduction_callback_value_only(il, node) {
        return false;
    }
    strict_exact_bool_reduction_receiver_safe(il, interner, facts, receiver, result.receiver, all)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_bool_reduction_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
    contract: MethodReceiverContract,
    all: bool,
) -> bool {
    if !matches!(
        contract,
        MethodReceiverContract::ExactArray
            | MethodReceiverContract::ExactArrayOrCollection
            | MethodReceiverContract::ExactCollection
            | MethodReceiverContract::ExactProtocol
            | MethodReceiverContract::ExactCollectionOrJavaKeySet
    ) {
        return false;
    }
    if strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
        || strict_exact_collection_factory_call_safe(il, interner, facts, receiver)
    {
        return true;
    }
    if all && js_like_lang(il.meta.lang) {
        return false;
    }
    strict_exact_proven_collection_receiver_safe(il, interner, facts, receiver)
}

fn strict_exact_bool_reduction_callback_value_only(il: &Il, node: NodeId) -> bool {
    let Some(&callback) = il.children(node).get(1) else {
        return false;
    };
    il.kind(callback) == NodeKind::Lambda && lambda_param_count(il, callback) == 1
}

fn lambda_param_count(il: &Il, lambda: NodeId) -> usize {
    il.children(lambda)
        .iter()
        .filter(|&&child| il.kind(child) == NodeKind::Param)
        .count()
}
