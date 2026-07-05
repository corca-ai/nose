use super::super::tree::collect_cids;
use crate::il_utils::{if_branch_blocks, local_nontrivial_assignment, node_mentions_any_cid};
use nose_il::{Il, Interner, LoopKind, NodeId, NodeKind, Payload};
use nose_semantics::{builder_append_call_args, exact_non_overloadable_index_assignment_parts};
use rustc_hash::FxHashSet;

pub(super) fn exact_loop_effect_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 3 {
        return false;
    }
    let mut iter_cids = FxHashSet::default();
    collect_cids(il, kids[0], &mut iter_cids);
    if iter_cids.is_empty() {
        return false;
    }
    foreach_effect_body_depends_on_iter(il, interner, kids[2], &iter_cids).unwrap_or(false)
}

fn foreach_effect_body_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    match il.kind(node) {
        NodeKind::Block => foreach_block_depends_on_iter(il, interner, node, iter_cids),
        NodeKind::ExprStmt => foreach_expr_statement_depends_on_iter(il, interner, node, iter_cids),
        NodeKind::Assign => foreach_assignment_depends_on_iter(il, interner, node, iter_cids),
        NodeKind::If => foreach_if_depends_on_iter(il, interner, node, iter_cids),
        _ => None,
    }
}

fn foreach_block_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    let mut has_effect = false;
    let kids = il.children(node);
    let mut idx = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && loop_temp_chain_consumed_by_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
                kids[idx + 2],
                iter_cids,
            )
        {
            has_effect = true;
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && loop_temp_assignment_consumed_by_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
                iter_cids,
            )
        {
            has_effect = true;
            idx += 2;
            continue;
        }
        has_effect |= foreach_effect_body_depends_on_iter(il, interner, kids[idx], iter_cids)?;
        idx += 1;
    }
    Some(has_effect)
}

fn foreach_expr_statement_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    let kids = il.children(node);
    (kids.len() == 1 && append_effect_depends_on_iter(il, interner, kids[0], iter_cids))
        .then_some(true)
}

fn foreach_assignment_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    index_assignment_effect_depends_on_iter(il, interner, node, iter_cids).then_some(true)
}

fn foreach_if_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    let mut has_append = false;
    for &branch in if_branch_blocks(il, node)? {
        has_append |= foreach_effect_body_depends_on_iter(il, interner, branch, iter_cids)?;
    }
    Some(has_append)
}

fn loop_temp_assignment_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some(temp_cid) = loop_local_iter_temp_assignment(il, assign, iter_cids) else {
        return false;
    };
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    loop_effect_consumes_temp(il, interner, effect, iter_cids, &temp_cids)
}

fn loop_temp_chain_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((first_cid, first_rhs)) = local_nontrivial_assignment(il, first_assign) else {
        return false;
    };
    if iter_cids.contains(&first_cid) || !node_mentions_any_cid(il, first_rhs, iter_cids) {
        return false;
    }
    let Some((second_cid, second_rhs)) = local_nontrivial_assignment(il, second_assign) else {
        return false;
    };
    if iter_cids.contains(&second_cid) || first_cid == second_cid {
        return false;
    }

    let mut first_temp = FxHashSet::default();
    first_temp.insert(first_cid);
    if !node_mentions_any_cid(il, second_rhs, &first_temp) {
        return false;
    }

    let mut all_temps = first_temp.clone();
    all_temps.insert(second_cid);
    let mut final_temp = FxHashSet::default();
    final_temp.insert(second_cid);
    loop_effect_consumes_chained_temp(
        il,
        interner,
        effect,
        iter_cids,
        &all_temps,
        &final_temp,
        &first_temp,
    )
}

fn loop_local_iter_temp_assignment(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<u32> {
    let (lhs, rhs) = il.assignment_var_parts(node)?;
    if matches!(il.kind(rhs), NodeKind::Var | NodeKind::Lit) {
        return None;
    }
    let temp_cid = il.var_cid(lhs)?;
    if iter_cids.contains(&temp_cid) {
        return None;
    }
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    if node_mentions_any_cid(il, rhs, &temp_cids) || !node_mentions_any_cid(il, rhs, iter_cids) {
        return None;
    }
    Some(temp_cid)
}

fn loop_effect_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            kids.len() == 1
                && append_effect_consumes_temp(il, interner, kids[0], iter_cids, temp_cids)
        }
        NodeKind::Assign => index_assignment_effect_consumes_temp(il, node, iter_cids, temp_cids),
        _ => false,
    }
}

fn loop_effect_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            kids.len() == 1
                && append_effect_consumes_chained_temp(
                    il,
                    interner,
                    kids[0],
                    iter_cids,
                    all_temp_cids,
                    final_temp_cids,
                    prior_temp_cids,
                )
        }
        NodeKind::Assign => index_assignment_effect_consumes_chained_temp(
            il,
            node,
            iter_cids,
            all_temp_cids,
            final_temp_cids,
            prior_temp_cids,
        ),
        _ => false,
    }
}

pub(super) fn append_effect_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids)
        && !node_mentions_any_cid(il, receiver, temp_cids)
        && node_mentions_any_cid(il, value, temp_cids)
}

pub(super) fn append_effect_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids)
        && !node_mentions_any_cid(il, receiver, all_temp_cids)
        && node_mentions_any_cid(il, value, final_temp_cids)
        && !node_mentions_any_cid(il, value, prior_temp_cids)
}

fn index_assignment_effect_consumes_temp(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids)
        || node_mentions_any_cid(il, receiver, temp_cids)
    {
        return false;
    }
    key.is_some_and(|key| node_mentions_any_cid(il, key, temp_cids))
        || node_mentions_any_cid(il, value, temp_cids)
}

fn index_assignment_effect_consumes_chained_temp(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids)
        || node_mentions_any_cid(il, receiver, all_temp_cids)
    {
        return false;
    }
    let key_uses_final = key.is_some_and(|key| node_mentions_any_cid(il, key, final_temp_cids));
    let key_uses_prior = key.is_some_and(|key| node_mentions_any_cid(il, key, prior_temp_cids));
    let value_uses_final = node_mentions_any_cid(il, value, final_temp_cids);
    let value_uses_prior = node_mentions_any_cid(il, value, prior_temp_cids);
    (key_uses_final || value_uses_final) && !key_uses_prior && !value_uses_prior
}

fn append_effect_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids) && node_mentions_any_cid(il, value, iter_cids)
}

pub(super) fn append_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    builder_append_call_args(il, interner, node)
}

fn index_assignment_effect_depends_on_iter(
    il: &Il,
    _interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids) {
        return false;
    }
    key.is_some_and(|key| node_mentions_any_cid(il, key, iter_cids))
        || node_mentions_any_cid(il, value, iter_cids)
}
