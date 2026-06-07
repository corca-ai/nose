//! Independent contract-path recognizer for the [`FragmentKind::LoopEffect`] shape.
//!
//! Issue #49 (Team A), migration 1. The production authority stays the predicate matrix in
//! [`crate::units`]; this re-expresses the *same acceptance boundary* on the contract path so
//! the differential gate (`super::recognize`) can hold the two in lockstep. Per the issue
//! decision, the re-expression is **independent**: it shares only substrate-level generic
//! utilities (AST traversal over canonical ids), never the predicate's acceptance helpers
//! (`units::exact_loop_effect_fragment_root`, `foreach_effect_body_depends_on_iter`, …).
//!
//! A `LoopEffect` is a `for-each` loop whose body produces at least one **iteration-dependent
//! observable effect** — an append or an index write whose written value/key depends on the
//! loop variable but whose receiver does not — possibly routed through one or two local temps,
//! and possibly nested inside `if` branches. Every statement in the body must be one of these
//! recognized effect shapes (or an `if`/block composed of them); any unrecognized statement
//! rejects the whole loop. That strictness is what keeps the fragment self-contained and
//! exactly reproducible behavior in the oracle wrapper.

use super::contract::{Effect, EffectSite, FragmentContract};
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind};
use nose_il::{Il, Interner, LoopKind, NodeId, NodeKind, Payload};
use nose_semantics::{builder_append_call_args, exact_non_overloadable_index_assignment_parts};
use rustc_hash::FxHashSet;

/// Recognize `node` as a `LoopEffect` contract, or `None` if it is not the shape.
///
/// Independent of `units::exact_loop_effect_fragment_root`; the caller has already applied the
/// shared span/context-safety gates (see [`super::recognize::recognize_contract`]).
pub(crate) fn recognize_loop_effect(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentContract> {
    if !matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 3 {
        return None;
    }
    // Loop variables bound by the for-each pattern. An effect's dependence on *these* cids is
    // what makes it iteration-dependent.
    let mut iter_cids = FxHashSet::default();
    collect_var_cids(il, kids[0], &mut iter_cids);
    if iter_cids.is_empty() {
        return None;
    }

    let mut effects = Vec::new();
    if !body_depends_on_iter(il, interner, kids[2], &iter_cids, &mut effects)? {
        return None;
    }

    Some(FragmentContract::ordered_effects(
        FragmentKind::LoopEffect,
        node,
        free_input_cids(il, node),
        Exit::Normal,
        effects,
    ))
}

/// Whether the loop `body` is composed entirely of recognized iteration-dependent effect
/// shapes, with at least one real effect. `None` means a statement was not recognized (the
/// whole loop is rejected); `Some(false)` means recognized but effect-free (e.g. an empty
/// branch). Recognized effects are pushed onto `effects` in body order.
fn body_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    effects: &mut Vec<EffectSite>,
) -> Option<bool> {
    match il.kind(node) {
        NodeKind::Block => {
            let kids = il.children(node);
            let mut has_effect = false;
            let mut idx = 0;
            while idx < kids.len() {
                // Two- and three-statement temp windows consume their statements as a unit.
                if idx + 2 < kids.len()
                    && temp_chain_consumed_by_effect(
                        il,
                        interner,
                        kids[idx],
                        kids[idx + 1],
                        kids[idx + 2],
                        iter_cids,
                        effects,
                    )
                {
                    has_effect = true;
                    idx += 3;
                    continue;
                }
                if idx + 1 < kids.len()
                    && temp_assignment_consumed_by_effect(
                        il,
                        interner,
                        kids[idx],
                        kids[idx + 1],
                        iter_cids,
                        effects,
                    )
                {
                    has_effect = true;
                    idx += 2;
                    continue;
                }
                has_effect |= body_depends_on_iter(il, interner, kids[idx], iter_cids, effects)?;
                idx += 1;
            }
            Some(has_effect)
        }
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            if kids.len() == 1 && append_depends_on_iter(il, interner, kids[0], iter_cids) {
                effects.push(EffectSite::observable(Effect::Append));
                Some(true)
            } else {
                None
            }
        }
        NodeKind::Assign => {
            if index_assignment_depends_on_iter(il, node, iter_cids) {
                effects.push(EffectSite::observable(Effect::IndexWrite));
                Some(true)
            } else {
                None
            }
        }
        NodeKind::If => {
            let kids = il.children(node);
            if !(kids.len() == 2 || kids.len() == 3) {
                return None;
            }
            let mut has_effect = false;
            for &branch in kids.iter().skip(1) {
                if il.kind(branch) != NodeKind::Block {
                    return None;
                }
                has_effect |= body_depends_on_iter(il, interner, branch, iter_cids, effects)?;
            }
            Some(has_effect)
        }
        _ => None,
    }
}

// ---- direct iteration-dependent effects --------------------------------------------------

/// `recv.append(value)` where the appended value depends on the loop var but the receiver
/// (the appended-to collection) does not — so the receiver is loop-invariant.
fn append_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some(kids) = append_call_args(il, interner, node) else {
        return false;
    };
    !mentions_any(il, kids.0, iter_cids) && mentions_any(il, kids.1, iter_cids)
}

/// `recv[key] = value` (non-overloadable C/Go/Java index write) where key or value depends on
/// the loop var and the receiver does not.
fn index_assignment_depends_on_iter(il: &Il, node: NodeId, iter_cids: &FxHashSet<u32>) -> bool {
    let Some((receiver, key, value)) = index_assignment_parts(il, node) else {
        return false;
    };
    if mentions_any(il, receiver, iter_cids) {
        return false;
    }
    key.is_some_and(|k| mentions_any(il, k, iter_cids)) || mentions_any(il, value, iter_cids)
}

// ---- single-temp window: `t = f(iter); recv.append/idx(t)` -------------------------------

/// `t = <expr over iter>;  <effect consuming t>` over two adjacent statements.
fn temp_assignment_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
    effects: &mut Vec<EffectSite>,
) -> bool {
    let Some(temp) = iter_temp_assignment(il, assign, iter_cids) else {
        return false;
    };
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp);
    effect_consumes_temp(il, interner, effect, iter_cids, &temp_cids, effects)
}

/// A local `t = rhs` whose `rhs` is non-trivial, depends on the loop var, does not read `t`
/// itself, and whose target is not a loop var. Returns the temp cid.
fn iter_temp_assignment(il: &Il, node: NodeId, iter_cids: &FxHashSet<u32>) -> Option<u32> {
    let (temp, rhs) = local_nontrivial_assignment(il, node)?;
    if iter_cids.contains(&temp) {
        return None;
    }
    if !mentions_any(il, rhs, iter_cids) {
        return None;
    }
    Some(temp)
}

/// An effect that consumes `temp_cids`: an append whose value reads the temp (receiver reads
/// neither iter nor temp), or an index write whose key/value reads the temp.
fn effect_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
    effects: &mut Vec<EffectSite>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            if kids.len() == 1 && append_consumes_temp(il, interner, kids[0], iter_cids, temp_cids)
            {
                effects.push(EffectSite::observable(Effect::Append));
                return true;
            }
            false
        }
        NodeKind::Assign => {
            if index_assignment_consumes_temp(il, node, iter_cids, temp_cids) {
                effects.push(EffectSite::observable(Effect::IndexWrite));
                return true;
            }
            false
        }
        _ => false,
    }
}

fn append_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((recv, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !mentions_any(il, recv, iter_cids)
        && !mentions_any(il, recv, temp_cids)
        && mentions_any(il, value, temp_cids)
}

fn index_assignment_consumes_temp(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = index_assignment_parts(il, node) else {
        return false;
    };
    if mentions_any(il, receiver, iter_cids) || mentions_any(il, receiver, temp_cids) {
        return false;
    }
    key.is_some_and(|k| mentions_any(il, k, temp_cids)) || mentions_any(il, value, temp_cids)
}

// ---- two-temp chain: `a = f(iter); b = g(a); recv.append/idx(b)` --------------------------

/// `a = <expr over iter>;  b = <expr over a>;  <effect consuming b but not a>` over three
/// adjacent statements.
fn temp_chain_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    first: NodeId,
    second: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
    effects: &mut Vec<EffectSite>,
) -> bool {
    let Some((first_cid, first_rhs)) = local_nontrivial_assignment(il, first) else {
        return false;
    };
    if iter_cids.contains(&first_cid) || !mentions_any(il, first_rhs, iter_cids) {
        return false;
    }
    let Some((second_cid, second_rhs)) = local_nontrivial_assignment(il, second) else {
        return false;
    };
    if iter_cids.contains(&second_cid) || first_cid == second_cid {
        return false;
    }
    let mut first_temp = FxHashSet::default();
    first_temp.insert(first_cid);
    if !mentions_any(il, second_rhs, &first_temp) {
        return false;
    }
    let mut all_temps = first_temp.clone();
    all_temps.insert(second_cid);
    let mut final_temp = FxHashSet::default();
    final_temp.insert(second_cid);
    let evidence = TempChainEvidence {
        iter_cids,
        all_temps: &all_temps,
        final_temp: &final_temp,
        prior_temp: &first_temp,
    };
    effect_consumes_chained_temp(il, interner, effect, evidence, effects)
}

#[derive(Clone, Copy)]
struct TempChainEvidence<'a> {
    iter_cids: &'a FxHashSet<u32>,
    all_temps: &'a FxHashSet<u32>,
    final_temp: &'a FxHashSet<u32>,
    prior_temp: &'a FxHashSet<u32>,
}

fn effect_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    evidence: TempChainEvidence<'_>,
    effects: &mut Vec<EffectSite>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            if kids.len() == 1 && append_consumes_chained_temp(il, interner, kids[0], evidence) {
                effects.push(EffectSite::observable(Effect::Append));
                return true;
            }
            false
        }
        NodeKind::Assign => {
            if index_assignment_consumes_chained_temp(il, node, evidence) {
                effects.push(EffectSite::observable(Effect::IndexWrite));
                return true;
            }
            false
        }
        _ => false,
    }
}

fn append_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    evidence: TempChainEvidence<'_>,
) -> bool {
    let Some((recv, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !mentions_any(il, recv, evidence.iter_cids)
        && !mentions_any(il, recv, evidence.all_temps)
        && mentions_any(il, value, evidence.final_temp)
        && !mentions_any(il, value, evidence.prior_temp)
}

fn index_assignment_consumes_chained_temp(
    il: &Il,
    node: NodeId,
    evidence: TempChainEvidence<'_>,
) -> bool {
    let Some((receiver, key, value)) = index_assignment_parts(il, node) else {
        return false;
    };
    if mentions_any(il, receiver, evidence.iter_cids)
        || mentions_any(il, receiver, evidence.all_temps)
    {
        return false;
    }
    let key_final = key.is_some_and(|k| mentions_any(il, k, evidence.final_temp));
    let key_prior = key.is_some_and(|k| mentions_any(il, k, evidence.prior_temp));
    let value_final = mentions_any(il, value, evidence.final_temp);
    let value_prior = mentions_any(il, value, evidence.prior_temp);
    (key_final || value_final) && !key_prior && !value_prior
}

// ---- shared structural readers (no acceptance semantics) ----------------------------------

/// `(receiver, value)` of an `append`/`push` builtin call with exactly two children.
fn append_call_args(il: &Il, interner: &Interner, node: NodeId) -> Option<(NodeId, NodeId)> {
    builder_append_call_args(il, interner, node)
}

/// `(receiver, key, value)` of a non-overloadable `recv[key] = value` index assignment
/// (C/Go/Java only — the same surface the migrated `IndexAssignEffect` admits).
fn index_assignment_parts(il: &Il, node: NodeId) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    exact_non_overloadable_index_assignment_parts(il, node)
}

/// A local `var = rhs` where `rhs` is not a bare var/lit and does not read the target itself.
/// Returns `(target cid, rhs node)`.
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

// ---- generic AST utilities (substrate-level, shareable) -----------------------------------

/// Collect every canonical id read as a `Var` in the subtree.
fn collect_var_cids(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if let (NodeKind::Var, Payload::Cid(cid)) = (il.kind(node), il.node(node).payload) {
        out.insert(cid);
    }
    for &child in il.children(node) {
        collect_var_cids(il, child, out);
    }
}

/// Whether the subtree reads any of `cids` as a `Var`.
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
