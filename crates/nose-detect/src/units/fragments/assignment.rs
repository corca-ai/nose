use super::super::tree::parent_of;
use crate::fragment::FragmentKind;
use crate::il_utils::{empty_or_single_block_child, if_branch_blocks, EmptyOrSingleChild};
use crate::strict_exact::{strict_exact_safe_tree, StrictFacts};
use nose_il::{Il, Interner, NodeId, NodeKind};
use nose_semantics::{
    exact_java_return_this, exact_non_overloadable_index_assignment,
    exact_self_field_write_assignment,
};

pub(super) fn exact_assignment_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_assignment_fragment_kind(il, interner, node).is_some()
}

/// Classify an assignment fragment as an index-assignment effect or a Java self-field
/// write — the two exact assignment shapes. Index assignment is checked first so the
/// classification is deterministic (the two shapes are structurally disjoint regardless).
pub(super) fn exact_assignment_fragment_kind(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentKind> {
    if exact_index_assignment_fragment_root(il, node) {
        Some(FragmentKind::IndexAssignEffect)
    } else if exact_self_field_assignment_fragment_root(il, interner, node) {
        Some(FragmentKind::SelfFieldAssign)
    } else {
        None
    }
}

pub(super) fn exact_index_assignment_fragment_root(il: &Il, node: NodeId) -> bool {
    exact_non_overloadable_index_assignment(il, node)
}

// Field-write fingerprints model final receiver+field state. Expose only Java's fixed
// `this.field = ...`; arbitrary receivers such as `other.field = ...` need a
// receiver-place proof fact before they can be exact fragments.
pub(super) fn exact_self_field_assignment_fragment_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    exact_self_field_write_assignment(il, interner, node)
}

fn exact_java_return_this_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_java_return_this(il, interner, node)
}

pub(super) fn exact_function_body_self_field_fragment_root(
    il: &Il,
    interner: &Interner,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let Some(func) = parent_of(parents, node) else {
        return false;
    };
    if il.kind(func) != NodeKind::Func {
        return false;
    }
    let kids = il.children(node);
    if kids.len() < 2 {
        return false;
    }
    let mut has_field_effect = false;
    for (idx, &child) in kids.iter().enumerate() {
        match exact_self_field_body_statement_root(il, interner, child) {
            Some(SelfFieldBodyStatement::FieldEffect) => {
                has_field_effect = true;
            }
            Some(SelfFieldBodyStatement::ReturnThis) if idx + 1 == kids.len() => {}
            _ => return false,
        }
    }
    has_field_effect
}

#[derive(Clone, Copy)]
enum SelfFieldBodyStatement {
    FieldEffect,
    ReturnThis,
}

fn exact_self_field_body_statement_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<SelfFieldBodyStatement> {
    if exact_java_return_this_fragment_root(il, interner, node) {
        return Some(SelfFieldBodyStatement::ReturnThis);
    }
    exact_self_field_statement_fragment_root(il, interner, node)
        .then_some(SelfFieldBodyStatement::FieldEffect)
}

fn exact_self_field_statement_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Assign => exact_self_field_assignment_fragment_root(il, interner, node),
        NodeKind::If => {
            let Some(branches) = if_branch_blocks(il, node) else {
                return false;
            };
            let mut has_field_assignment = false;
            for &branch in branches {
                let Some(branch_has_field_assignment) =
                    exact_self_field_statement_branch_root(il, interner, branch)
                else {
                    return false;
                };
                has_field_assignment |= branch_has_field_assignment;
            }
            has_field_assignment
        }
        _ => false,
    }
}

fn exact_self_field_statement_branch_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    match empty_or_single_block_child(il, node)? {
        EmptyOrSingleChild::Empty => Some(false),
        EmptyOrSingleChild::Single(child) => Some(exact_self_field_statement_fragment_root(
            il, interner, child,
        )),
    }
}

pub(in crate::units) fn strict_exact_self_field_fragment_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Block => {
            let kids = il.children(node);
            exact_function_body_self_field_fragment_root(il, interner, parents, node)
                && kids.iter().all(|&child| {
                    strict_exact_self_field_body_statement_safe(il, interner, facts, parents, child)
                })
        }
        _ => strict_exact_self_field_effect_safe(il, interner, facts, parents, node),
    }
}

fn strict_exact_self_field_body_statement_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    exact_java_return_this_fragment_root(il, interner, node)
        || strict_exact_self_field_effect_safe(il, interner, facts, parents, node)
}

fn strict_exact_self_field_effect_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Assign => {
            let kids = il.children(node);
            kids.len() == 2
                && exact_self_field_assignment_fragment_root(il, interner, node)
                && strict_exact_safe_tree(il, interner, facts, kids[1])
        }
        NodeKind::If => {
            let Some(branches) = if_branch_blocks(il, node) else {
                return false;
            };
            if !strict_exact_safe_tree(il, interner, facts, il.children(node)[0]) {
                return false;
            }
            let mut has_field_assignment = false;
            for &branch in branches {
                let Some(branch_has_field_assignment) =
                    strict_exact_self_field_branch_safe(il, interner, facts, parents, branch)
                else {
                    return false;
                };
                has_field_assignment |= branch_has_field_assignment;
            }
            has_field_assignment
        }
        _ => false,
    }
}

fn strict_exact_self_field_branch_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if (2..=3).contains(&kids.len()) && kids.iter().all(|&kid| il.kind(kid) == NodeKind::Assign) {
        return Some(
            kids.iter()
                .all(|&kid| strict_exact_self_field_effect_safe(il, interner, facts, parents, kid)),
        );
    }
    if kids.len() != 1 {
        return None;
    }
    Some(strict_exact_self_field_effect_safe(
        il, interner, facts, parents, kids[0],
    ))
}
