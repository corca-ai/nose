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
    matches!(il.node(*key).payload, Payload::LitStr(_))
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
    let reference_is_free_name = matches!(il.node(reference).payload, Payload::Name(_));
    let module_level: &[NodeId] = if reference_is_free_name && scope.is_some() {
        il.assigns_in_scope(None)
    } else {
        &[]
    };
    let mut found = None;
    for &assign in il.assigns_in_scope(scope).iter().chain(module_level) {
        if il.node(assign).span.end_byte > il.node(reference).span.start_byte {
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
                let Some(&target) = il.children(node_id).first() else {
                    return false;
                };
                node_contains_binding_reference(il, interner, target, reference)
            }
            NodeKind::Call => il
                .children(node_id)
                .iter()
                .skip(1)
                .any(|&arg| node_contains_binding_reference(il, interner, arg, reference)),
            _ => false,
        }
    })
}

fn node_contains_binding_reference(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    reference: NodeId,
) -> bool {
    var_references_same_binding(il, interner, node, reference)
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_binding_reference(il, interner, child, reference))
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
