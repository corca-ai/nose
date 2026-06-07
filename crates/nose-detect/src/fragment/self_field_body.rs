//! Independent contract-path recognizer for [`FragmentKind::SelfFieldBody`].
//!
//! This migration mirrors the predicate's body-level acceptance boundary: a Java function
//! body block made only of fixed-`this` field writes, conditional fixed-`this` field writes,
//! and an optional terminal `return this`. Unlike ordinary statement fragments, this shape
//! proves self-containment through the fixed receiver rather than the shared top-level
//! context gate, so [`recognize_contract`](super::recognize::recognize_contract) calls it
//! before that gate and keeps the bypass scoped to this kind.

use super::contract::{Effect, EffectSite, FragmentContract};
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind};
use nose_il::{Il, Interner, NodeId, NodeKind};
use nose_semantics::{exact_java_return_this, exact_java_this_field, semantics};

pub(crate) fn recognize_self_field_body(
    il: &Il,
    interner: &Interner,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<FragmentContract> {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Block
    {
        return None;
    }
    let func = parents.get(node.0 as usize).copied().flatten()?;
    if il.kind(func) != NodeKind::Func {
        return None;
    }
    let kids = il.children(node);
    if kids.len() < 2 {
        return None;
    }

    let mut effects = Vec::new();
    let mut has_field_effect = false;
    for (idx, &child) in kids.iter().enumerate() {
        if exact_java_return_this(il, interner, child) {
            if idx + 1 != kids.len() {
                return None;
            }
            continue;
        }

        let before = effects.len();
        self_field_statement(il, interner, child, &mut effects)?;
        if effects.len() == before {
            return None;
        }
        has_field_effect = true;
    }
    if !has_field_effect {
        return None;
    }

    let contract = FragmentContract::ordered_effects(
        FragmentKind::SelfFieldBody,
        node,
        free_input_cids(il, node),
        Exit::Normal,
        effects,
    );
    contract.writes_proven().then_some(contract)
}

fn self_field_statement(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    effects: &mut Vec<EffectSite>,
) -> Option<()> {
    match il.kind(node) {
        NodeKind::Assign => self_field_assign(il, interner, node, effects),
        NodeKind::If => {
            let kids = il.children(node);
            if !(kids.len() == 2 || kids.len() == 3) {
                return None;
            }
            let mut has_field_assignment = false;
            for &branch in kids.iter().skip(1) {
                let before = effects.len();
                self_field_branch(il, interner, branch, effects)?;
                has_field_assignment |= effects.len() != before;
            }
            has_field_assignment.then_some(())
        }
        _ => None,
    }
}

fn self_field_branch(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    effects: &mut Vec<EffectSite>,
) -> Option<()> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(());
    }
    if kids.len() != 1 {
        return None;
    }
    self_field_statement(il, interner, kids[0], effects)
}

fn self_field_assign(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    effects: &mut Vec<EffectSite>,
) -> Option<()> {
    let kids = il.children(node);
    if kids.len() != 2 || !exact_java_this_field(il, interner, kids[0]) {
        return None;
    }
    let place = super::recognize::resolve_place(il, interner, kids[0]);
    if !place.is_exact_safe() {
        return None;
    }
    effects.push(EffectSite::at(Effect::FieldWrite, place));
    Some(())
}
