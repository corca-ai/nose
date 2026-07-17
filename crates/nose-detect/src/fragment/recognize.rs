//! The contract-path recognizer and its differential gate against the shape predicates.
//!
//! Issue #33 steps 4–5. As each fragment family migrates off the standalone shape
//! predicates in [`crate::units`], its recognition is re-expressed here as the
//! construction of a [`FragmentContract`]. [`recognize_contract`] is an *independent*
//! recognizer for the migrated shapes: it matches structure directly and builds a contract,
//! reusing only the shared invalidation-boundary gates (span containment + context safety),
//! which are substrate, not per-shape predicates.
//!
//! The differential test below is the acceptance gate the maintainer required: over a
//! representative corpus, the set of `(span, kind)` the predicate path accepts (restricted
//! to migrated kinds) must equal the set the contract path produces. A migration step that
//! changes which nodes are accepted fails this test. As the migrated set grows, the gate
//! keeps the two paths in lockstep until every shape is contract-expressed.

use super::contract::{Effect, EffectSite, FragmentContract};
use super::oracle::free_input_cids;
use super::{Exit, FragmentKind, Place};
use nose_il::{stable_symbol_hash, Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::{
    admitted_builder_append_call_args, exact_java_this_var,
    exact_non_overloadable_index_assignment, exact_self_field_write_assignment,
};

/// Fragment kinds that have been migrated onto the contract path. The differential gate
/// compares the predicate and contract paths over exactly this set; everything outside it
/// is still owned solely by the [`crate::units`] predicates.
#[cfg(test)]
pub(crate) const MIGRATED: &[FragmentKind] = &[
    FragmentKind::DirectReturn,
    FragmentKind::DirectThrow,
    FragmentKind::IndexAssignEffect,
    FragmentKind::SelfFieldAssign,
    FragmentKind::ExprEffect,
    FragmentKind::LoopEffect,
    FragmentKind::SelfFieldBody,
    FragmentKind::ConditionalGuard,
];

/// Recognize `node` as a migrated exact-fragment shape by building its contract directly,
/// independently of `units::exact_statement_fragment_root`. Returns `None` for
/// non-fragments and for shapes not yet migrated.
pub(crate) fn recognize_contract(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
) -> Option<FragmentContract> {
    // Shared substrate gates — the invalidation-boundary model, reused (not duplicated).
    if !crate::units::subtree_spans_within(il, node, il.node(node).span) {
        return None;
    }
    // `SelfFieldBody` proves self-containment through fixed Java `this` field writes. It is
    // the one migrated shape whose predicate acceptance boundary intentionally sits before
    // the shared top-level context gate; keep that bypass local to this recognizer.
    if let Some(contract) =
        super::self_field_body::recognize_self_field_body(il, interner, parents, node)
    {
        return Some(contract);
    }
    if !crate::units::top_level_statement_fragment_context_safe(il, node, parents, interner) {
        return None;
    }
    let kids = il.children(node);
    let computed_unary =
        || kids.len() == 1 && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit);
    match il.kind(node) {
        NodeKind::Return if computed_unary() => Some(FragmentContract::value_sink(
            FragmentKind::DirectReturn,
            node,
            free_input_cids(il, node),
            Exit::Return,
        )),
        NodeKind::Throw if computed_unary() => Some(FragmentContract::value_sink(
            FragmentKind::DirectThrow,
            node,
            free_input_cids(il, node),
            Exit::Throw,
        )),
        NodeKind::Assign => recognize_assignment_effect(il, interner, node),
        NodeKind::ExprStmt if expr_effect_shape(il, kids) => {
            let effect = if is_append_call(il, interner, kids[0]) {
                Effect::Append
            } else {
                Effect::Other
            };
            Some(effect_contract(
                FragmentKind::ExprEffect,
                il,
                node,
                EffectSite::observable(effect),
            ))
        }
        NodeKind::If => super::conditional_guard::recognize_conditional_guard(il, interner, node),
        NodeKind::Loop => super::loop_effect::recognize_loop_effect(il, interner, node),
        _ => None,
    }
}

/// Enumerate every migrated fragment contract recognized in `il`.
///
/// Product extraction remains the admission authority. This narrower public audit surface lets
/// the offline oracle span-match an admitted fully-normalized fragment to its pre-canonical core
/// counterpart without duplicating the recognizer or relying on arena node ids lining up.
pub fn recognized_contracts(il: &Il, interner: &Interner) -> Vec<FragmentContract> {
    let parents = crate::units::build_parent_index(il);
    let mut contracts = Vec::new();
    let mut stack = vec![il.root];
    while let Some(node) = stack.pop() {
        if matches!(
            il.kind(node),
            NodeKind::Return
                | NodeKind::Throw
                | NodeKind::Assign
                | NodeKind::ExprStmt
                | NodeKind::If
                | NodeKind::Loop
                | NodeKind::Block
        ) {
            if let Some(contract) = recognize_contract(il, node, &parents, interner) {
                contracts.push(contract);
            }
        }
        stack.extend(il.children(node).iter().rev().copied());
    }
    contracts
}

/// Classify an assignment-effect fragment: a non-overloadable index write (C/Go/Java) or a
/// Java fixed-receiver `this.field` write. The two shapes are structurally disjoint.
fn recognize_assignment_effect(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentContract> {
    let kids = il.children(node);
    if kids.len() != 2 {
        return None;
    }
    let target = kids[0];
    if exact_non_overloadable_index_assignment(il, node) {
        // An index write is observable in the effect trace (key and value are recorded), so
        // it carries no receiver-identity obligation and records no `Place` on the contract.
        // The write target is identity, not proof; surfacing it is a separate diagnostic
        // concern, deliberately kept out of the contract model.
        return Some(effect_contract(
            FragmentKind::IndexAssignEffect,
            il,
            node,
            EffectSite::observable(Effect::IndexWrite),
        ));
    }
    if exact_self_field_write_assignment(il, interner, node) {
        let place = resolve_place(il, interner, target);
        // Field-write final state is keyed by receiver+field place, so the write is exact-safe
        // only with a proven receiver. The `this.field` recognizer guarantees this; assert the
        // invariant fail-closed.
        debug_assert!(
            place.is_exact_safe(),
            "self-field write must resolve to a proven place, got {place:?}"
        );
        return Some(effect_contract(
            FragmentKind::SelfFieldAssign,
            il,
            node,
            EffectSite::at(Effect::FieldWrite, place),
        ));
    }
    None
}

/// An expression statement evaluated for its side effect: a single child that is not a
/// control sink, bare variable, or bare literal (those carry no observable effect).
fn expr_effect_shape(il: &Il, kids: &[NodeId]) -> bool {
    kids.len() == 1
        && !matches!(
            il.kind(kids[0]),
            NodeKind::Return
                | NodeKind::Throw
                | NodeKind::Break
                | NodeKind::Continue
                | NodeKind::Var
                | NodeKind::Lit
        )
}

fn is_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    admitted_builder_append_call_args(il, interner, node).is_some()
}

fn effect_contract(
    kind: FragmentKind,
    il: &Il,
    node: NodeId,
    site: EffectSite,
) -> FragmentContract {
    FragmentContract::single_effect(kind, node, free_input_cids(il, node), site)
}

/// Resolve a write target's [`Place`] receiver identity, fail-closed to [`Place::Unknown`].
///
/// - `this` (Java) → [`Place::This`]
/// - a free variable → [`Place::Param`] (its canonical id)
/// - `base.field` → [`Place::Field`] over the resolved base, keyed by field-name hash
/// - `base[key]` → [`Place::Index`] over the resolved base, keyed by a coarse key hash
/// - anything else (a call result, an unresolved receiver) → [`Place::Unknown`]
pub(super) fn resolve_place(il: &Il, interner: &Interner, node: NodeId) -> Place {
    match il.kind(node) {
        NodeKind::Var if exact_java_this_var(il, interner, node) => Place::This,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => Place::Param(c),
            _ => Place::Unknown,
        },
        NodeKind::Field => {
            let base = il.children(node).first().copied();
            let receiver = base.map_or(Place::Unknown, |b| resolve_place(il, interner, b));
            match il.node(node).payload {
                Payload::Name(sym) => Place::Field(
                    Box::new(receiver),
                    stable_symbol_hash(interner.resolve(sym)),
                ),
                _ => Place::Unknown,
            }
        }
        NodeKind::Index => {
            let kids = il.children(node);
            let receiver = kids
                .first()
                .map_or(Place::Unknown, |&b| resolve_place(il, interner, b));
            let key = kids.get(1).map_or(0, |&k| place_key_hash(il, interner, k));
            Place::Index(Box::new(receiver), key)
        }
        _ => Place::Unknown,
    }
}

/// A coarse, stable identity for an index/key expression — enough to distinguish constant
/// keys and variable keys in a [`Place`], without modeling arbitrary key expressions.
fn place_key_hash(il: &Il, interner: &Interner, node: NodeId) -> u64 {
    match il.node(node).payload {
        Payload::Cid(c) => 0x01_0000_0000 | u64::from(c),
        Payload::Name(sym) => stable_symbol_hash(interner.resolve(sym)),
        Payload::LitInt(v) => 0x02_0000_0000 ^ (v as u64),
        Payload::LitStr(h) | Payload::LitFloat(h) => h,
        _ => u64::from(il.kind(node) as u8),
    }
}

#[cfg(test)]
mod tests;
