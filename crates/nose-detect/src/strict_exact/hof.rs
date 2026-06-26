use super::*;

pub(super) fn strict_exact_safe_hof(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    match source_comprehension_at_node(il, node) {
        Some(SourceComprehensionKind::PythonListComprehension)
        | Some(SourceComprehensionKind::PythonDictComprehension) => {
            strict_exact_hof_children_safe(il, interner, facts, node)
        }
        Some(
            SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind).is_some() =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

pub(super) fn strict_exact_terminal_reduction_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return strict_exact_safe_tree(il, interner, facts, node);
    }
    match source_comprehension_at_node(il, node) {
        Some(
            SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonListComprehension,
        ) => strict_exact_hof_children_safe(il, interner, facts, node),
        Some(
            SourceComprehensionKind::PythonDictComprehension
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind).is_some() =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

pub(super) fn strict_exact_len_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return strict_exact_safe_tree(il, interner, facts, node);
    }
    match source_comprehension_at_node(il, node) {
        Some(SourceComprehensionKind::PythonListComprehension) => {
            strict_exact_hof_children_safe(il, interner, facts, node)
        }
        Some(
            SourceComprehensionKind::PythonDictComprehension
            | SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind)
                    .is_some_and(|profile| profile.proves_eager_per_element_callback_demand()) =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

fn strict_exact_hof_children_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node).iter().all(|&child| {
        if il.kind(child) == NodeKind::HoF {
            strict_exact_hof_internal_safe(il, interner, facts, child)
        } else {
            strict_exact_safe_tree(il, interner, facts, child)
        }
    })
}

fn strict_exact_hof_internal_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    matches!(
        il.node(node).payload,
        Payload::HoF(
            HoFKind::Map
                | HoFKind::FlatMap
                | HoFKind::Filter
                | HoFKind::FilterMap
                | HoFKind::Reject,
        )
    ) && strict_exact_hof_children_safe(il, interner, facts, node)
}

pub(super) fn strict_exact_in_membership_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    let Payload::Op(Op::In) = il.node(node).payload else {
        return false;
    };
    if semantics(il.meta.lang)
        .operators()
        .membership_operator(Op::In)
        .is_none()
    {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && strict_exact_safe_tree(il, interner, facts, kids[0])
        && strict_exact_in_membership_collection_safe(il, interner, facts, kids[1])
}

pub(super) fn strict_exact_in_membership_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if strict_exact_proven_collection_receiver_safe(il, interner, facts, node)
        || strict_exact_proven_map_receiver_safe(il, interner, facts, node)
    {
        return true;
    }
    match il.kind(node) {
        NodeKind::Seq => strict_exact_membership_collection_safe(il, interner, facts, node),
        NodeKind::Call => strict_exact_collection_factory_call_safe(il, interner, facts, node),
        NodeKind::Var => {
            matches!(il.node(node).payload, Payload::Name(name) if facts.exact_value_name(name))
        }
        _ => false,
    }
}
