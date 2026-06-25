//! JS/TS `Object.keys(...)` object-key-view proof helpers.

use super::*;
use crate::sequence_surface::sequence_surface_evidence_record_at_sequence_span;

pub fn js_object_key_view_argument_dependency_ids_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<Vec<EvidenceId>> {
    let (_, dependencies) = js_object_key_view_argument_map_node_for_call(il, interner, call)?;
    Some(dependencies)
}

pub fn js_object_key_view_argument_map_node_at_call_span(
    il: &Il,
    interner: &Interner,
    call_span: Option<Span>,
) -> Option<NodeId> {
    let span = call_span?;
    let call = node_at_exact_span_with_kind(il, span, NodeKind::Call)?;
    js_object_key_view_argument_map_node_for_call(il, interner, call).map(|(node, _)| node)
}

pub fn js_object_key_view_argument_map_node_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<(NodeId, Vec<EvidenceId>)> {
    if !js_like_lang(il.meta.lang) || il.kind(call) != NodeKind::Call {
        return None;
    }
    if has_js_with_statement_ancestor(il, interner, call) {
        return None;
    }
    let [callee, object] = il.children(call) else {
        return None;
    };
    if !object_keys_callee(il, interner, *callee) {
        return None;
    }
    js_object_key_view_argument_map_node(il, interner, *object, call)
}

fn js_object_key_view_argument_map_node(
    il: &Il,
    interner: &Interner,
    object: NodeId,
    use_node: NodeId,
) -> Option<(NodeId, Vec<EvidenceId>)> {
    if let Some(surface) = static_js_object_literal_surface_dependency_id(il, interner, object) {
        return Some((object, vec![surface]));
    }
    let (assign, rhs) = unique_static_object_literal_binding_initializer(il, interner, object)?;
    if binding_mutated_or_escaped_before_use(il, interner, object, assign, use_node) {
        return None;
    }
    let surface = static_js_object_literal_surface_dependency_id(il, interner, rhs)?;
    let write = binding_write_dependency_id(il, assign)?;
    Some((rhs, vec![write, surface]))
}

fn object_keys_callee(il: &Il, interner: &Interner, callee: NodeId) -> bool {
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    if interner.resolve(method) != "keys" {
        return false;
    }
    let Some(&receiver) = il.children(callee).first() else {
        return false;
    };
    matches!(
        (il.kind(receiver), il.node(receiver).payload),
        (NodeKind::Var, Payload::Name(name)) if interner.resolve(name) == "Object"
    )
}

fn static_js_object_literal_surface_dependency_id(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<EvidenceId> {
    if !js_like_lang(il.meta.lang) || !static_js_object_literal_shape(il, interner, node) {
        return None;
    }
    match sequence_surface_evidence_record_at_sequence_span(il, il.node(node).span) {
        EvidenceResolution::Found((SequenceSurfaceKind::Map, id)) => Some(id),
        EvidenceResolution::Found(_)
        | EvidenceResolution::Missing
        | EvidenceResolution::Ambiguous => None,
    }
}

fn static_js_object_literal_shape(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(tag) = il.node(node).payload else {
        return false;
    };
    if interner.resolve(tag) != "object" {
        return false;
    }
    il.children(node)
        .iter()
        .all(|&child| static_js_object_pair_with_string_key(il, interner, child))
}

fn static_js_object_pair_with_string_key(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(tag) = il.node(node).payload else {
        return false;
    };
    if interner.resolve(tag) != "pair" {
        return false;
    }
    let [key, _value] = il.children(node) else {
        return false;
    };
    matches!(
        il.node(*key).payload,
        Payload::LitStr(hash) if hash != stable_symbol_hash("__proto__")
    )
}

fn unique_static_object_literal_binding_initializer(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(reference) != NodeKind::Var {
        return None;
    }
    let scope = il.nearest_scope(reference);
    let use_block = nearest_ancestor_with_kind(il, reference, NodeKind::Block)?;
    if block_contains_nested_function(il, use_block) {
        return None;
    }
    let mut found = None;
    for &assign in il.assigns_in_scope(scope) {
        if il.node(assign).span.end_byte > il.node(reference).span.start_byte {
            continue;
        }
        if parent_of(il, assign) != Some(use_block) {
            continue;
        }
        let [lhs, rhs] = il.children(assign) else {
            continue;
        };
        if !var_references_same_binding(il, interner, *lhs, reference) {
            continue;
        }
        if !static_js_object_literal_shape(il, interner, *rhs) {
            return None;
        }
        if found.is_some() {
            return None;
        }
        found = Some((assign, *rhs));
    }
    found
}

fn binding_mutated_or_escaped_before_use(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
    initializer: NodeId,
    use_node: NodeId,
) -> bool {
    let use_start = il.node(use_node).span.start_byte;
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        if node.span.end_byte > use_start || node_id == initializer {
            return false;
        }
        match node.kind {
            NodeKind::Assign => {
                let [target, value] = il.children(node_id) else {
                    return false;
                };
                any_node_contains_binding_reference(il, interner, &[*target, *value], reference)
            }
            NodeKind::Call => {
                direct_eval_call(il, interner, node_id)
                    || any_node_contains_binding_reference(
                        il,
                        interner,
                        il.children(node_id),
                        reference,
                    )
            }
            NodeKind::Seq if js_delete_sequence(il, interner, node_id) => {
                any_node_contains_binding_reference(il, interner, il.children(node_id), reference)
            }
            NodeKind::Loop if loop_target_writes_binding(il, interner, node_id, reference) => true,
            NodeKind::Raw if js_with_scope_uses_binding(il, interner, node_id, reference) => true,
            _ => false,
        }
    })
}

fn loop_target_writes_binding(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    reference: NodeId,
) -> bool {
    if !matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) {
        return false;
    }
    let Some(&target) = il.children(node).first() else {
        return false;
    };
    node_contains_binding_reference(il, interner, target, reference)
}

fn js_delete_sequence(il: &Il, interner: &Interner, node: NodeId) -> bool {
    matches!(
        il.node(node).payload,
        Payload::Name(tag) if interner.resolve(tag) == "js_delete"
    )
}

fn js_with_scope_uses_binding(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    reference: NodeId,
) -> bool {
    if !js_with_statement(il, interner, node) {
        return false;
    }
    let Some(&scope_object) = il.children(node).first() else {
        return false;
    };
    node_contains_binding_reference(il, interner, scope_object, reference)
}

fn has_js_with_statement_ancestor(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let mut current = node;
    for _ in 0..il.nodes.len() {
        let Some(parent) = parent_of(il, current) else {
            return false;
        };
        if js_with_statement(il, interner, parent) {
            return true;
        }
        current = parent;
    }
    false
}

fn js_with_statement(il: &Il, interner: &Interner, node: NodeId) -> bool {
    il.kind(node) == NodeKind::Raw
        && matches!(
            il.node(node).payload,
            Payload::Name(tag) if interner.resolve(tag) == "with_statement"
        )
}

fn direct_eval_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    matches!(
        (il.kind(callee), il.node(callee).payload),
        (NodeKind::Var, Payload::Name(name)) if interner.resolve(name) == "eval"
    )
}

fn any_node_contains_binding_reference(
    il: &Il,
    interner: &Interner,
    nodes: &[NodeId],
    reference: NodeId,
) -> bool {
    let mut visited = vec![false; il.nodes.len()];
    nodes.iter().any(|&node| {
        node_contains_binding_reference_with_seen(il, interner, node, reference, &mut visited)
    })
}

fn node_contains_binding_reference(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    reference: NodeId,
) -> bool {
    let mut visited = vec![false; il.nodes.len()];
    node_contains_binding_reference_with_seen(il, interner, node, reference, &mut visited)
}

fn node_contains_binding_reference_with_seen(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    reference: NodeId,
    visited: &mut [bool],
) -> bool {
    let idx = node.0 as usize;
    if idx >= visited.len() || visited[idx] {
        return false;
    }
    visited[idx] = true;
    var_references_same_binding(il, interner, node, reference)
        || il.children(node).iter().any(|&child| {
            node_contains_binding_reference_with_seen(il, interner, child, reference, visited)
        })
}

fn block_contains_nested_function(il: &Il, block: NodeId) -> bool {
    let mut visited = vec![false; il.nodes.len()];
    il.children(block)
        .iter()
        .any(|&child| node_contains_kind_with_seen(il, child, NodeKind::Func, &mut visited))
}

fn node_contains_kind_with_seen(
    il: &Il,
    node: NodeId,
    kind: NodeKind,
    visited: &mut [bool],
) -> bool {
    let idx = node.0 as usize;
    if idx >= visited.len() || visited[idx] {
        return false;
    }
    visited[idx] = true;
    il.kind(node) == kind
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_kind_with_seen(il, child, kind, visited))
}

fn var_references_same_binding(
    il: &Il,
    interner: &Interner,
    lhs: NodeId,
    reference: NodeId,
) -> bool {
    if il.kind(lhs) != NodeKind::Var || il.kind(reference) != NodeKind::Var {
        return false;
    }
    match (il.node(lhs).payload, il.node(reference).payload) {
        (Payload::Cid(left), Payload::Cid(right)) => left == right,
        (Payload::Name(left), Payload::Name(right)) => {
            interner.resolve(left) == interner.resolve(right)
        }
        _ => false,
    }
}

fn binding_write_dependency_id(il: &Il, assign: NodeId) -> Option<EvidenceId> {
    il.evidence_anchored_at(il.node(assign).span)
        .find_map(|record| {
            (record.anchor == EvidenceAnchor::node(il.node(assign).span, NodeKind::Assign)
                && record.kind == EvidenceKind::Effect(EffectEvidenceKind::BindingWrite)
                && record.status == EvidenceStatus::Asserted
                && il.evidence_dependencies_asserted(record))
            .then_some(record.id)
        })
}

fn node_at_exact_span_with_kind(il: &Il, span: Span, kind: NodeKind) -> Option<NodeId> {
    let mut found = None;
    for id in il.nodes_spanning(span) {
        let node = il.node(id);
        if node.span != span || node.kind != kind {
            continue;
        }
        match found {
            None => found = Some(id),
            Some(existing) if il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

fn nearest_ancestor_with_kind(il: &Il, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    let mut current = node;
    for _ in 0..il.nodes.len() {
        let parent = parent_of(il, current)?;
        if il.kind(parent) == kind {
            return Some(parent);
        }
        current = parent;
    }
    None
}

fn parent_of(il: &Il, child: NodeId) -> Option<NodeId> {
    let mut found = None;
    for (idx, _node) in il.nodes.iter().enumerate() {
        let candidate = NodeId(idx as u32);
        if !il.children(candidate).contains(&child) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(candidate);
    }
    found
}
