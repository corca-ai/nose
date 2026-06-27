use super::*;

pub(super) fn strict_exact_safe_call(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if let Payload::Builtin(builtin) = il.node(node).payload {
        if !admitted_builtin_semantics_at_call_with_interner(il, interner, node, builtin) {
            return false;
        }
        let kids = il.children(node);
        return match builtin {
            Builtin::Contains if kids.len() == 2 => {
                strict_exact_safe_tree(il, interner, facts, kids[0])
                    && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
            }
            Builtin::GetOrDefault if kids.len() == 3 => {
                (strict_exact_map_receiver_or_factory_safe(il, interner, facts, kids[0], false)
                    || (il.kind(kids[0]) != NodeKind::Var
                        && strict_exact_safe_tree(il, interner, facts, kids[0])))
                    && strict_exact_safe_tree(il, interner, facts, kids[1])
                    && strict_exact_safe_tree(il, interner, facts, kids[2])
            }
            Builtin::Len if kids.len() == 1 => {
                if admitted_terminal_count_reduction_at_call(il, node) {
                    strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
                } else {
                    strict_exact_len_arg_safe(il, interner, facts, kids[0])
                }
            }
            Builtin::Sum | Builtin::Any | Builtin::All if kids.len() == 1 => {
                strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
            }
            Builtin::Min | Builtin::Max if kids.len() == 1 => {
                strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
            }
            _ => kids
                .iter()
                .all(|&c| strict_exact_safe_tree(il, interner, facts, c)),
        };
    }
    if strict_exact_factory_call_safe(il, interner, facts, node) {
        return true;
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if strict_exact_typeof_operator_safe(il, interner, facts, node, callee) {
        return true;
    }
    if il.kind(callee) != NodeKind::Field {
        return strict_exact_callee_identity(il, interner, facts, node, callee)
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(name);
    if let Some(regex_safe) =
        strict_exact_regex_test_safe(il, interner, facts, node, callee, method)
    {
        return regex_safe;
    }
    if strict_exact_js_array_is_array_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_collection_contains_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_contains_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_get_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_get_default_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_iterator_identity_adapter_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    // Opaque exact method identity: this keeps same-callee calls eligible as exact clones
    // without assigning semantic meaning to the method name. Cross-language/builtin
    // convergence still has to pass the proof-backed contracts above or in normalization.
    strict_exact_callee_identity(il, interner, facts, node, callee)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_factory_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_collection_factory_call_safe(il, interner, facts, node)
        || strict_exact_rust_vec_new_safe(il, interner, node)
        || strict_exact_java_collection_constructor_safe(il, interner, node)
        || strict_exact_java_map_factory_safe(il, interner, facts, node)
        || strict_exact_rust_std_map_factory_safe(il, interner, facts, node)
        || strict_exact_swift_map_factory_safe(il, interner, facts, node)
        || strict_exact_map_constructor_entries_safe(il, interner, facts, node)
}

pub(super) fn strict_exact_typeof_operator_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
) -> bool {
    let (NodeKind::Var, Payload::Name(name)) = (il.kind(callee), il.node(callee).payload) else {
        return false;
    };
    let Some(contract) = typeof_operator_contract(
        il.meta.lang,
        interner.resolve(name),
        il.children(node).len().saturating_sub(1),
    ) else {
        return false;
    };
    source_fact_at_node(il, node, contract.required_source_fact)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

pub(super) fn admitted_method_call_contract(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(LibraryMethodCallContract, usize)> {
    let admitted = admitted_library_method_call_at_call(il, interner, node)?;
    Some((admitted.contract, admitted.arg_count))
}

pub(super) fn field_receiver(il: &Il, callee: NodeId) -> Option<NodeId> {
    il.children(callee).first().copied()
}

pub(super) fn strict_exact_regex_test_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    _callee: NodeId,
    method: &str,
) -> Option<bool> {
    if method != "test" {
        return None;
    }
    if admitted_regex_test_at_call(il, interner, node).is_none() {
        return Some(false);
    }
    Some(strict_exact_call_args_safe(il, interner, facts, node))
}

pub(super) fn strict_exact_js_array_is_array_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    _callee: NodeId,
    method: &str,
) -> bool {
    if method != "isArray" || admitted_js_array_is_array_at_call(il, interner, node).is_none() {
        return false;
    }
    strict_exact_call_args_safe(il, interner, facts, node)
}
