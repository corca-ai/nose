use super::assignment::{
    exact_assignment_fragment_root, exact_index_assignment_fragment_root,
    exact_self_field_assignment_fragment_root,
};
use super::loop_effect::{
    append_call_args, append_effect_consumes_chained_temp, append_effect_consumes_temp,
    exact_loop_effect_fragment_root,
};
use super::{exact_conditional_fragment_root, exact_expr_statement_fragment_root};
use crate::il_utils::{
    empty_or_single_block_child, if_branch_blocks, local_nontrivial_assignment,
    local_nontrivial_assignment_chain, node_mentions_any_cid, EmptyOrSingleChild,
};
use nose_il::{Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::exact_non_overloadable_index_assignment_parts;
use rustc_hash::FxHashSet;

pub(super) fn empty_or_single_direct_exact_statement_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if exact_ordered_append_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_index_assignment_effect_sequence_block(il, node) {
        return Some(true);
    }
    if exact_ordered_self_field_assignment_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_conditional_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_conditional_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_conditional_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_conditional_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if kids.len() == 3 && exact_temp_chain_consumed_by_statement(il, kids[0], kids[1], kids[2]) {
        return Some(true);
    }
    if kids.len() == 2 && exact_temp_assignment_consumed_by_statement(il, kids[0], kids[1]) {
        return Some(true);
    }
    if kids.len() != 1 {
        return None;
    }
    match il.kind(kids[0]) {
        NodeKind::Return if il.children(kids[0]).len() <= 1 => Some(true),
        NodeKind::Throw if il.children(kids[0]).len() == 1 => Some(true),
        NodeKind::Assign if exact_assignment_fragment_root(il, interner, kids[0]) => Some(true),
        NodeKind::ExprStmt if exact_expr_statement_fragment_root(il, kids[0]) => Some(true),
        NodeKind::If if exact_conditional_fragment_root(il, interner, kids[0]) => Some(true),
        NodeKind::Loop if exact_loop_effect_fragment_root(il, interner, kids[0]) => Some(true),
        _ => None,
    }
}

fn exact_ordered_loop_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && kids
            .iter()
            .all(|&kid| exact_loop_effect_fragment_root(il, interner, kid))
}

fn exact_ordered_mixed_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if loop_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter()
        .all(|&kid| exact_ordered_mixed_effect_sequence_item(il, interner, kid))
}

fn exact_ordered_mixed_effect_sequence_item(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, node),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, node)
        }
        _ => false,
    }
}

fn exact_ordered_conditional_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && kids.iter().all(|&kid| il.kind(kid) == NodeKind::If)
        && kids
            .iter()
            .all(|&kid| exact_conditional_direct_effect_fragment_root(il, interner, kid))
}

fn exact_conditional_direct_effect_fragment_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    let Some(branches) = if_branch_blocks(il, node) else {
        return false;
    };
    let mut has_effect = false;
    for &branch in branches {
        let Some(branch_has_effect) =
            empty_or_single_direct_exact_effect_block(il, interner, branch)
        else {
            return false;
        };
        has_effect |= branch_has_effect;
    }
    has_effect
}

fn exact_ordered_conditional_mixed_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if conditional_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, kid)
        }
        _ => false,
    })
}

fn exact_ordered_loop_conditional_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    if loop_count != 1 || conditional_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, kid),
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        _ => false,
    })
}

fn exact_ordered_loop_conditional_mixed_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 3 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if loop_count != 1 || conditional_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, kid),
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, kid)
        }
        _ => false,
    })
}

fn empty_or_single_direct_exact_effect_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    match empty_or_single_block_child(il, node)? {
        EmptyOrSingleChild::Empty => Some(false),
        EmptyOrSingleChild::Single(child) => {
            exact_direct_effect_statement_root(il, interner, child).then_some(true)
        }
    }
}

fn exact_direct_effect_statement_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => exact_append_effect_statement_root(il, interner, node),
        NodeKind::Assign => exact_index_assignment_fragment_root(il, node),
        _ => false,
    }
}

fn exact_ordered_append_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if !(2..=5).contains(&kids.len()) {
        return false;
    }
    if !kids
        .iter()
        .all(|&kid| matches!(il.kind(kid), NodeKind::Assign | NodeKind::ExprStmt))
    {
        return false;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| exact_append_effect_statement_root(il, interner, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return false,
    };
    let mut idx = 0;
    let mut effects = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && exact_temp_chain_consumed_by_append_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
                kids[idx + 2],
            )
        {
            effects += 1;
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && exact_temp_assignment_consumed_by_append_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
            )
        {
            effects += 1;
            idx += 2;
            continue;
        }
        if exact_append_effect_statement_root(il, interner, kids[idx]) {
            effects += 1;
            idx += 1;
            continue;
        }
        return false;
    }
    effects == expected_effects
}

fn exact_append_effect_statement_root(il: &Il, interner: &Interner, stmt: NodeId) -> bool {
    if il.kind(stmt) != NodeKind::ExprStmt {
        return false;
    }
    let kids = il.children(stmt);
    kids.len() == 1 && exact_single_item_append_call(il, interner, kids[0])
}

fn exact_single_item_append_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    append_call_args(il, interner, call).is_some()
}

fn exact_temp_assignment_consumed_by_append_effect(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    let empty = FxHashSet::default();
    let kids = il.children(effect);
    il.kind(effect) == NodeKind::ExprStmt
        && kids.len() == 1
        && exact_single_item_append_call(il, interner, kids[0])
        && append_effect_consumes_temp(il, interner, kids[0], &empty, &temp_cids)
}

fn exact_temp_chain_consumed_by_append_effect(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some(chain) = local_nontrivial_assignment_chain(il, first_assign, second_assign) else {
        return false;
    };
    let empty = FxHashSet::default();
    let kids = il.children(effect);
    il.kind(effect) == NodeKind::ExprStmt
        && kids.len() == 1
        && exact_single_item_append_call(il, interner, kids[0])
        && append_effect_consumes_chained_temp(
            il,
            interner,
            kids[0],
            &empty,
            &chain.all,
            &chain.second,
            &chain.first,
        )
}

fn exact_ordered_index_assignment_effect_sequence_block(il: &Il, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if !(2..=5).contains(&kids.len()) {
        return false;
    }
    if !kids.iter().all(|&kid| il.kind(kid) == NodeKind::Assign) {
        return false;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| exact_index_assignment_fragment_root(il, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return false,
    };
    let mut idx = 0;
    let mut effects = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && exact_temp_chain_consumed_by_index_assignment_effect(
                il,
                kids[idx],
                kids[idx + 1],
                kids[idx + 2],
            )
        {
            effects += 1;
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && exact_temp_assignment_consumed_by_index_assignment_effect(
                il,
                kids[idx],
                kids[idx + 1],
            )
        {
            effects += 1;
            idx += 2;
            continue;
        }
        if exact_index_assignment_fragment_root(il, kids[idx]) {
            effects += 1;
            idx += 1;
            continue;
        }
        return false;
    }
    effects == expected_effects
}

fn exact_ordered_self_field_assignment_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    (2..=3).contains(&kids.len())
        && kids
            .iter()
            .all(|&kid| exact_self_field_assignment_fragment_root(il, interner, kid))
}

fn exact_temp_assignment_consumed_by_index_assignment_effect(
    il: &Il,
    assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    exact_index_assignment_consumes_temp(il, effect, temp_cid, None)
}

fn exact_temp_chain_consumed_by_index_assignment_effect(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some(chain) = local_nontrivial_assignment_chain(il, first_assign, second_assign) else {
        return false;
    };
    exact_index_assignment_consumes_temp(il, effect, chain.second_cid, Some(&chain.first))
}

fn exact_temp_assignment_consumed_by_statement(il: &Il, assign: NodeId, stmt: NodeId) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    exact_statement_consumes_temp(il, stmt, temp_cid, None)
}

fn exact_temp_chain_consumed_by_statement(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
    stmt: NodeId,
) -> bool {
    let Some((first_cid, _)) = local_nontrivial_assignment(il, first_assign) else {
        return false;
    };
    let Some((second_cid, second_rhs)) = local_nontrivial_assignment(il, second_assign) else {
        return false;
    };
    if first_cid == second_cid {
        return false;
    }
    let mut first = FxHashSet::default();
    first.insert(first_cid);
    if !node_mentions_any_cid(il, second_rhs, &first) {
        return false;
    }
    exact_statement_consumes_temp(il, stmt, second_cid, Some(&first))
}

fn exact_statement_consumes_temp(
    il: &Il,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> bool {
    match il.kind(stmt) {
        NodeKind::Return | NodeKind::Throw => {
            let kids = il.children(stmt);
            if kids.len() != 1 {
                return false;
            }
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            (il.kind(kids[0]) == NodeKind::Var
                && matches!(il.node(kids[0]).payload, Payload::Cid(cid) if cid == temp_cid))
                || (!matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit)
                    && node_mentions_any_cid(il, kids[0], &temp)
                    && match forbidden_cids {
                        Some(cids) => !node_mentions_any_cid(il, kids[0], cids),
                        None => true,
                    })
        }
        NodeKind::ExprStmt if exact_expr_statement_fragment_root(il, stmt) => {
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            node_mentions_any_cid(il, stmt, &temp)
                && match forbidden_cids {
                    Some(cids) => !node_mentions_any_cid(il, stmt, cids),
                    None => true,
                }
        }
        NodeKind::Assign => {
            exact_index_assignment_consumes_temp(il, stmt, temp_cid, forbidden_cids)
        }
        _ => false,
    }
}

fn exact_index_assignment_consumes_temp(
    il: &Il,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, stmt)
    else {
        return false;
    };

    let mut temp = FxHashSet::default();
    temp.insert(temp_cid);
    if node_mentions_any_cid(il, receiver, &temp)
        || forbidden_cids.is_some_and(|cids| node_mentions_any_cid(il, receiver, cids))
    {
        return false;
    }

    let key_uses_temp = key.is_some_and(|key| node_mentions_any_cid(il, key, &temp));
    let value_uses_temp = node_mentions_any_cid(il, value, &temp);
    if !(key_uses_temp || value_uses_temp) {
        return false;
    }
    match forbidden_cids {
        Some(cids) => {
            !key.is_some_and(|key| node_mentions_any_cid(il, key, cids))
                && !node_mentions_any_cid(il, value, cids)
        }
        None => true,
    }
}
