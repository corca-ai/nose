//! Independent contract-path recognizer for [`FragmentKind::ConditionalGuard`].
//!
//! Conditional guards are the largest predicate-owned fragment shape: each non-empty branch
//! may be a direct exit/effect, a branch-local temp window, a small ordered effect body, a
//! loop effect, or another conditional guard. This module re-expresses that admissibility
//! matrix on the contract path without calling the predicate acceptance helpers in
//! `units.rs`.

use super::contract::{Effect, EffectSite, FragmentContract};
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind};
use nose_il::{Builtin, Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::{builder_append_method_contract, semantics};
use rustc_hash::FxHashSet;

pub(crate) fn recognize_conditional_guard(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentContract> {
    let summary = conditional_summary(il, interner, node)?;
    if !summary.has_statement {
        return None;
    }
    let contract = FragmentContract::ordered_effects(
        FragmentKind::ConditionalGuard,
        node,
        free_input_cids(il, node),
        Exit::Normal,
        summary.effects,
    );
    contract.writes_proven().then_some(contract)
}

#[derive(Default)]
struct Summary {
    has_statement: bool,
    effects: Vec<EffectSite>,
}

impl Summary {
    fn exact(effects: Vec<EffectSite>) -> Self {
        Summary {
            has_statement: true,
            effects,
        }
    }
}

fn conditional_summary(il: &Il, interner: &Interner, node: NodeId) -> Option<Summary> {
    if il.kind(node) != NodeKind::If {
        return None;
    }
    let kids = il.children(node);
    if !(kids.len() == 2 || kids.len() == 3) {
        return None;
    }
    let mut out = Summary::default();
    for &branch in kids.iter().skip(1) {
        let summary = branch_block(il, interner, branch)?;
        out.has_statement |= summary.has_statement;
        out.effects.extend(summary.effects);
    }
    Some(out)
}

fn branch_block(il: &Il, interner: &Interner, node: NodeId) -> Option<Summary> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(Summary::default());
    }
    if let Some(effects) = ordered_append_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_index_assignment_effect_sequence(il, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_self_field_assignment_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_loop_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_mixed_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_conditional_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_conditional_mixed_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_loop_conditional_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if let Some(effects) = ordered_loop_conditional_mixed_effect_sequence(il, interner, node) {
        return Some(Summary::exact(effects));
    }
    if kids.len() == 3 {
        if let Some(effects) =
            temp_chain_consumed_by_statement(il, interner, kids[0], kids[1], kids[2])
        {
            return Some(Summary::exact(effects));
        }
    }
    if kids.len() == 2 {
        if let Some(effects) = temp_assignment_consumed_by_statement(il, interner, kids[0], kids[1])
        {
            return Some(Summary::exact(effects));
        }
    }
    if kids.len() != 1 {
        return None;
    }
    single_branch_statement(il, interner, kids[0])
}

fn single_branch_statement(il: &Il, interner: &Interner, node: NodeId) -> Option<Summary> {
    match il.kind(node) {
        NodeKind::Return => (il.children(node).len() <= 1).then(|| Summary::exact(Vec::new())),
        NodeKind::Throw => (il.children(node).len() == 1).then(|| Summary::exact(Vec::new())),
        NodeKind::Assign => {
            assignment_effect_site(il, interner, node).map(|site| Summary::exact(vec![site]))
        }
        NodeKind::ExprStmt => {
            expr_statement_site(il, interner, node).map(|site| Summary::exact(vec![site]))
        }
        NodeKind::If => {
            let summary = conditional_summary(il, interner, node)?;
            summary.has_statement.then_some(summary)
        }
        NodeKind::Loop => loop_effect_sites(il, interner, node).map(Summary::exact),
        _ => None,
    }
}

// ---- ordered branch bodies --------------------------------------------------------------

fn ordered_loop_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 2)?;
    let mut effects = Vec::new();
    for &kid in kids {
        effects.extend(loop_effect_sites(il, interner, kid)?);
    }
    Some(effects)
}

fn ordered_mixed_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 2)?;
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count()
        != 1
    {
        return None;
    }
    if kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count()
        != 1
    {
        return None;
    }
    let mut effects = Vec::new();
    for &kid in kids {
        match il.kind(kid) {
            NodeKind::Loop => effects.extend(loop_effect_sites(il, interner, kid)?),
            NodeKind::ExprStmt | NodeKind::Assign => {
                effects.push(direct_effect_site(il, interner, kid)?)
            }
            _ => return None,
        }
    }
    Some(effects)
}

fn ordered_conditional_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 2)?;
    if !kids.iter().all(|&kid| il.kind(kid) == NodeKind::If) {
        return None;
    }
    let mut effects = Vec::new();
    for &kid in kids {
        effects.extend(conditional_direct_effect_sites(il, interner, kid)?);
    }
    Some(effects)
}

fn ordered_conditional_mixed_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 2)?;
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count()
        != 1
    {
        return None;
    }
    if kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count()
        != 1
    {
        return None;
    }
    let mut effects = Vec::new();
    for &kid in kids {
        match il.kind(kid) {
            NodeKind::If => effects.extend(conditional_direct_effect_sites(il, interner, kid)?),
            NodeKind::ExprStmt | NodeKind::Assign => {
                effects.push(direct_effect_site(il, interner, kid)?)
            }
            _ => return None,
        }
    }
    Some(effects)
}

fn ordered_loop_conditional_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 2)?;
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count()
        != 1
    {
        return None;
    }
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count()
        != 1
    {
        return None;
    }
    let mut effects = Vec::new();
    for &kid in kids {
        match il.kind(kid) {
            NodeKind::Loop => effects.extend(loop_effect_sites(il, interner, kid)?),
            NodeKind::If => effects.extend(conditional_direct_effect_sites(il, interner, kid)?),
            _ => return None,
        }
    }
    Some(effects)
}

fn ordered_loop_conditional_mixed_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = block_children_exact_len(il, node, 3)?;
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count()
        != 1
    {
        return None;
    }
    if kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count()
        != 1
    {
        return None;
    }
    if kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count()
        != 1
    {
        return None;
    }
    let mut effects = Vec::new();
    for &kid in kids {
        match il.kind(kid) {
            NodeKind::Loop => effects.extend(loop_effect_sites(il, interner, kid)?),
            NodeKind::If => effects.extend(conditional_direct_effect_sites(il, interner, kid)?),
            NodeKind::ExprStmt | NodeKind::Assign => {
                effects.push(direct_effect_site(il, interner, kid)?)
            }
            _ => return None,
        }
    }
    Some(effects)
}

fn ordered_append_effect_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    let kids = il.children(node);
    if il.kind(node) != NodeKind::Block || !(2..=5).contains(&kids.len()) {
        return None;
    }
    if !kids
        .iter()
        .all(|&kid| matches!(il.kind(kid), NodeKind::Assign | NodeKind::ExprStmt))
    {
        return None;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| append_statement(il, interner, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return None,
    };
    let mut effects = Vec::new();
    let mut idx = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && temp_chain_consumed_by_append(il, interner, kids[idx], kids[idx + 1], kids[idx + 2])
        {
            effects.push(EffectSite::observable(Effect::Append));
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && temp_assignment_consumed_by_append(il, interner, kids[idx], kids[idx + 1])
        {
            effects.push(EffectSite::observable(Effect::Append));
            idx += 2;
            continue;
        }
        if append_statement(il, interner, kids[idx]) {
            effects.push(EffectSite::observable(Effect::Append));
            idx += 1;
            continue;
        }
        return None;
    }
    (effects.len() == expected_effects).then_some(effects)
}

fn ordered_index_assignment_effect_sequence(il: &Il, node: NodeId) -> Option<Vec<EffectSite>> {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .non_overloadable_index_assignment()
    {
        return None;
    }
    let kids = il.children(node);
    if il.kind(node) != NodeKind::Block || !(2..=5).contains(&kids.len()) {
        return None;
    }
    if !kids.iter().all(|&kid| il.kind(kid) == NodeKind::Assign) {
        return None;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| index_assignment(il, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return None,
    };
    let mut effects = Vec::new();
    let mut idx = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && temp_chain_consumed_by_index_assignment(il, kids[idx], kids[idx + 1], kids[idx + 2])
        {
            effects.push(EffectSite::observable(Effect::IndexWrite));
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && temp_assignment_consumed_by_index_assignment(il, kids[idx], kids[idx + 1])
        {
            effects.push(EffectSite::observable(Effect::IndexWrite));
            idx += 2;
            continue;
        }
        if index_assignment(il, kids[idx]) {
            effects.push(EffectSite::observable(Effect::IndexWrite));
            idx += 1;
            continue;
        }
        return None;
    }
    (effects.len() == expected_effects).then_some(effects)
}

fn ordered_self_field_assignment_sequence(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
    {
        return None;
    }
    let kids = il.children(node);
    if il.kind(node) != NodeKind::Block || !(2..=3).contains(&kids.len()) {
        return None;
    }
    kids.iter()
        .map(|&kid| self_field_assignment_site(il, interner, kid))
        .collect()
}

// ---- conditional/direct effect branches -------------------------------------------------

fn conditional_direct_effect_sites(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EffectSite>> {
    if il.kind(node) != NodeKind::If {
        return None;
    }
    let kids = il.children(node);
    if !(kids.len() == 2 || kids.len() == 3) {
        return None;
    }
    let mut has_effect = false;
    let mut effects = Vec::new();
    for &branch in kids.iter().skip(1) {
        let branch_effect = empty_or_single_direct_effect_block(il, interner, branch)?;
        has_effect |= branch_effect.is_some();
        if let Some(site) = branch_effect {
            effects.push(site);
        }
    }
    has_effect.then_some(effects)
}

fn empty_or_single_direct_effect_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Option<EffectSite>> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(None);
    }
    if kids.len() != 1 {
        return None;
    }
    direct_effect_site(il, interner, kids[0]).map(Some)
}

fn direct_effect_site(il: &Il, interner: &Interner, node: NodeId) -> Option<EffectSite> {
    match il.kind(node) {
        NodeKind::ExprStmt if append_statement(il, interner, node) => {
            Some(EffectSite::observable(Effect::Append))
        }
        NodeKind::Assign if index_assignment(il, node) => {
            Some(EffectSite::observable(Effect::IndexWrite))
        }
        _ => None,
    }
}

// ---- single statements ------------------------------------------------------------------

fn assignment_effect_site(il: &Il, interner: &Interner, node: NodeId) -> Option<EffectSite> {
    if index_assignment(il, node) {
        return Some(EffectSite::observable(Effect::IndexWrite));
    }
    self_field_assignment_site(il, interner, node)
}

fn self_field_assignment_site(il: &Il, interner: &Interner, node: NodeId) -> Option<EffectSite> {
    let kids = il.children(node);
    if semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        && kids.len() == 2
        && is_java_this_field(il, interner, kids[0])
    {
        let place = super::recognize::resolve_place(il, interner, kids[0]);
        return place
            .is_exact_safe()
            .then(|| EffectSite::at(Effect::FieldWrite, place));
    }
    None
}

fn expr_statement_site(il: &Il, interner: &Interner, node: NodeId) -> Option<EffectSite> {
    if il.kind(node) != NodeKind::ExprStmt {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 1
        || matches!(
            il.kind(kids[0]),
            NodeKind::Return
                | NodeKind::Throw
                | NodeKind::Break
                | NodeKind::Continue
                | NodeKind::Var
                | NodeKind::Lit
        )
    {
        return None;
    }
    if is_append_call(il, interner, kids[0]) {
        Some(EffectSite::observable(Effect::Append))
    } else {
        Some(EffectSite::observable(Effect::Other))
    }
}

fn append_statement(il: &Il, interner: &Interner, stmt: NodeId) -> bool {
    if il.kind(stmt) != NodeKind::ExprStmt {
        return false;
    }
    let kids = il.children(stmt);
    kids.len() == 1 && is_append_call(il, interner, kids[0])
}

fn index_assignment(il: &Il, node: NodeId) -> bool {
    semantics(il.meta.lang)
        .exact_fragments()
        .non_overloadable_index_assignment()
        && il.kind(node) == NodeKind::Assign
        && il.children(node).len() == 2
        && il.kind(il.children(node)[0]) == NodeKind::Index
}

fn loop_effect_sites(il: &Il, interner: &Interner, node: NodeId) -> Option<Vec<EffectSite>> {
    super::loop_effect::recognize_loop_effect(il, interner, node).map(|contract| contract.effects)
}

// ---- temp windows -----------------------------------------------------------------------

fn temp_assignment_consumed_by_statement(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    stmt: NodeId,
) -> Option<Vec<EffectSite>> {
    let (temp_cid, _) = local_nontrivial_assignment(il, assign)?;
    statement_consumes_temp(il, interner, stmt, temp_cid, None)
}

fn temp_chain_consumed_by_statement(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    stmt: NodeId,
) -> Option<Vec<EffectSite>> {
    let (first_cid, _) = local_nontrivial_assignment(il, first_assign)?;
    let (second_cid, second_rhs) = local_nontrivial_assignment(il, second_assign)?;
    if first_cid == second_cid {
        return None;
    }
    let mut first = FxHashSet::default();
    first.insert(first_cid);
    if !mentions_any(il, second_rhs, &first) {
        return None;
    }
    statement_consumes_temp(il, interner, stmt, second_cid, Some(&first))
}

fn statement_consumes_temp(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> Option<Vec<EffectSite>> {
    match il.kind(stmt) {
        NodeKind::Return | NodeKind::Throw => {
            let kids = il.children(stmt);
            if kids.len() != 1 {
                return None;
            }
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            let direct_temp = il.kind(kids[0]) == NodeKind::Var
                && matches!(il.node(kids[0]).payload, Payload::Cid(cid) if cid == temp_cid);
            let computed_temp = !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit)
                && mentions_any(il, kids[0], &temp)
                && forbidden_cids.is_none_or(|cids| !mentions_any(il, kids[0], cids));
            (direct_temp || computed_temp).then(Vec::new)
        }
        NodeKind::ExprStmt => {
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            if !mentions_any(il, stmt, &temp)
                || forbidden_cids.is_some_and(|cids| mentions_any(il, stmt, cids))
            {
                return None;
            }
            expr_statement_site(il, interner, stmt).map(|site| vec![site])
        }
        NodeKind::Assign => index_assignment_consumes_temp(il, stmt, temp_cid, forbidden_cids)
            .then(|| vec![EffectSite::observable(Effect::IndexWrite)]),
        _ => None,
    }
}

fn temp_assignment_consumed_by_append(
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
    il.kind(effect) == NodeKind::ExprStmt
        && il.children(effect).len() == 1
        && is_append_call(il, interner, il.children(effect)[0])
        && append_consumes_temp(il, interner, il.children(effect)[0], &empty, &temp_cids)
}

fn temp_chain_consumed_by_append(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((first_cid, first_rhs)) = local_nontrivial_assignment(il, first_assign) else {
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
    let mut second = FxHashSet::default();
    second.insert(second_cid);
    if mentions_any(il, first_rhs, &first)
        || mentions_any(il, first_rhs, &second)
        || !mentions_any(il, second_rhs, &first)
    {
        return false;
    }
    let mut all_temps = first.clone();
    all_temps.insert(second_cid);
    let mut final_temp = FxHashSet::default();
    final_temp.insert(second_cid);
    il.kind(effect) == NodeKind::ExprStmt
        && il.children(effect).len() == 1
        && is_append_call(il, interner, il.children(effect)[0])
        && append_consumes_chained_temp(
            il,
            interner,
            il.children(effect)[0],
            &all_temps,
            &final_temp,
            &first,
        )
}

fn temp_assignment_consumed_by_index_assignment(il: &Il, assign: NodeId, effect: NodeId) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    index_assignment_consumes_temp(il, effect, temp_cid, None)
}

fn temp_chain_consumed_by_index_assignment(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((first_cid, first_rhs)) = local_nontrivial_assignment(il, first_assign) else {
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
    let mut second = FxHashSet::default();
    second.insert(second_cid);
    if mentions_any(il, first_rhs, &first)
        || mentions_any(il, first_rhs, &second)
        || !mentions_any(il, second_rhs, &first)
    {
        return false;
    }
    index_assignment_consumes_temp(il, effect, second_cid, Some(&first))
}

fn index_assignment_consumes_temp(
    il: &Il,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> bool {
    let Some((receiver, key, value)) = index_assignment_parts(il, stmt) else {
        return false;
    };
    let mut temp = FxHashSet::default();
    temp.insert(temp_cid);
    if mentions_any(il, receiver, &temp)
        || forbidden_cids.is_some_and(|cids| mentions_any(il, receiver, cids))
    {
        return false;
    }
    let key_uses_temp = key.is_some_and(|key| mentions_any(il, key, &temp));
    let value_uses_temp = mentions_any(il, value, &temp);
    if !(key_uses_temp || value_uses_temp) {
        return false;
    }
    forbidden_cids.is_none_or(|cids| {
        !key.is_some_and(|key| mentions_any(il, key, cids)) && !mentions_any(il, value, cids)
    })
}

fn append_consumes_temp(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    forbidden_receiver_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, call) else {
        return false;
    };
    !mentions_any(il, receiver, forbidden_receiver_cids)
        && !mentions_any(il, receiver, temp_cids)
        && mentions_any(il, value, temp_cids)
}

fn append_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, call) else {
        return false;
    };
    !mentions_any(il, receiver, all_temp_cids)
        && mentions_any(il, value, final_temp_cids)
        && !mentions_any(il, value, prior_temp_cids)
}

// ---- structural readers -----------------------------------------------------------------

fn block_children_exact_len(il: &Il, node: NodeId, len: usize) -> Option<&[NodeId]> {
    (il.kind(node) == NodeKind::Block && il.children(node).len() == len).then(|| il.children(node))
}

fn append_call_args(il: &Il, interner: &Interner, node: NodeId) -> Option<(NodeId, NodeId)> {
    if !is_append_call(il, interner, node) {
        return None;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return Some((kids[0], kids[1]));
    }
    let receiver = *il.children(kids[0]).first()?;
    Some((receiver, kids[1]))
}

fn is_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return kids.len() == 2;
    }
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    if args.len() != 1 || il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    builder_append_method_contract(il.meta.lang, interner.resolve(method), args.len())
}

fn index_assignment_parts(il: &Il, node: NodeId) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    if !index_assignment(il, node) {
        return None;
    }
    let kids = il.children(node);
    let target = il.children(kids[0]);
    Some((*target.first()?, target.get(1).copied(), kids[1]))
}

fn local_nontrivial_assignment(il: &Il, node: NodeId) -> Option<(u32, NodeId)> {
    if il.kind(node) != NodeKind::Assign {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return None;
    }
    if matches!(il.kind(kids[1]), NodeKind::Var | NodeKind::Lit) {
        return None;
    }
    let Payload::Cid(cid) = il.node(kids[0]).payload else {
        return None;
    };
    let mut target = FxHashSet::default();
    target.insert(cid);
    if mentions_any(il, kids[1], &target) {
        return None;
    }
    Some((cid, kids[1]))
}

fn is_java_this_field(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Field
    {
        return false;
    }
    if !matches!(il.node(node).payload, Payload::Name(_)) {
        return false;
    }
    il.children(node)
        .first()
        .is_some_and(|&receiver| is_java_this_var(il, interner, receiver))
}

fn is_java_this_var(il: &Il, interner: &Interner, node: NodeId) -> bool {
    semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        && il.kind(node) == NodeKind::Var
        && matches!(il.node(node).payload, Payload::Name(name) if interner.resolve(name) == "this")
}

fn mentions_any(il: &Il, node: NodeId, cids: &FxHashSet<u32>) -> bool {
    if let (NodeKind::Var, Payload::Cid(cid)) = (il.kind(node), il.node(node).payload) {
        if cids.contains(&cid) {
            return true;
        }
    }
    il.children(node)
        .iter()
        .any(|&child| mentions_any(il, child, cids))
}
