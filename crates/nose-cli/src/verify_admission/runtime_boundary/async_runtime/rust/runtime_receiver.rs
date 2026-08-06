use super::super::{callee_field_method, callee_path, method_receiver, node_defines_name};
use super::import_identity::{imported_runtime_member, imported_runtime_type_visible};
use crate::verify_admission::AdmissionContext;
use nose_il::{stable_symbol_hash, DomainEvidence, Interner, NodeId, NodeKind};

pub(super) fn is_future_drive_call(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
) -> bool {
    callee_path == Some("tokio_test::block_on")
        || imported_runtime_member(il, interner, callee, "tokio_test", "block_on", context)
        || (callee_field_method(il, interner, callee) == Some("block_on")
            && method_receiver(il, callee)
                .is_some_and(|receiver| is_tokio_runtime_receiver(il, interner, receiver, context)))
}

fn is_tokio_runtime_receiver(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if context.rust_runtime_root_is_local_for_file("tokio", &il.meta.path) {
        return false;
    }
    is_runtime_driver_expr(il, interner, receiver, context)
        || is_local_runtime_binding(il, interner, receiver, context)
        || has_runtime_parameter_domain(il, interner, receiver)
        || has_runtime_field_domain(il, receiver)
}

fn is_runtime_driver_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if let Some(inner) = try_propagation_operand(il, receiver) {
        return is_runtime_result_expr(il, interner, inner, context);
    }
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee) = il.children(receiver).first().copied() else {
        return false;
    };
    if callee_path(il, interner, callee)
        .as_deref()
        .is_some_and(|path| is_runtime_driver_path(il, interner, callee, path, context))
    {
        return true;
    }
    if !is_runtime_unwrap_method(il, interner, callee) {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| is_runtime_result_expr(il, interner, inner, context))
}

fn try_propagation_operand(il: &nose_il::Il, node: NodeId) -> Option<NodeId> {
    (nose_semantics::source_protocol_at_node(il, node)
        == Some(nose_il::SourceProtocolKind::TryPropagation))
    .then(|| il.children(node).first().copied())
    .flatten()
}

fn is_runtime_result_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee) = il.children(receiver).first().copied() else {
        return false;
    };
    if callee_path(il, interner, callee)
        .as_deref()
        .is_some_and(|path| is_runtime_result_path(il, interner, callee, path, context))
    {
        return true;
    }
    if is_runtime_result_adapter(il, interner, callee) {
        return method_receiver(il, callee)
            .is_some_and(|inner| is_runtime_result_expr(il, interner, inner, context));
    }
    if callee_field_method(il, interner, callee) != Some("build") {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| is_runtime_builder_expr(il, interner, inner, context))
}

fn is_runtime_builder_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee) = il.children(receiver).first().copied() else {
        return false;
    };
    if callee_path(il, interner, callee)
        .as_deref()
        .is_some_and(|path| is_runtime_builder_path(il, interner, callee, path, context))
    {
        return true;
    }
    if !is_runtime_builder_chain_method(il, interner, callee) {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| is_runtime_builder_expr(il, interner, inner, context))
}

fn is_local_runtime_binding(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Some(local_name) = super::super::super::super::node_exact_name(il, interner, receiver)
    else {
        return false;
    };
    last_visible_local_assignment_rhs(il, interner, receiver, local_name)
        .is_some_and(|rhs| is_runtime_driver_expr(il, interner, rhs, context))
}

fn has_runtime_parameter_domain(il: &nose_il::Il, interner: &Interner, receiver: NodeId) -> bool {
    il.kind(receiver) == NodeKind::Var
        && is_runtime_domain(nose_semantics::domain_evidence_for_receiver(
            il, interner, receiver,
        ))
}

fn has_runtime_field_domain(il: &nose_il::Il, receiver: NodeId) -> bool {
    il.kind(receiver) == NodeKind::Field
        && is_runtime_domain(nose_semantics::domain_evidence_for_node(il, receiver))
}

fn is_runtime_domain(domain: Option<DomainEvidence>) -> bool {
    matches!(
        domain,
        Some(DomainEvidence::Nominal { type_hash })
            if type_hash == stable_symbol_hash("tokio::runtime::Runtime")
                || type_hash == stable_symbol_hash("tokio::runtime::Handle")
    )
}

fn last_visible_local_assignment_rhs(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    local_name: &str,
) -> Option<NodeId> {
    let occurrence_span = il.node(receiver).span;
    let mut last_assignment = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.kind != NodeKind::Assign
            || node.span.file != occurrence_span.file
            || occurrence_span.start_byte < node.span.end_byte
        {
            continue;
        }
        let node_id = NodeId(idx as u32);
        let Some((lhs, rhs)) = il.assignment_parts(node_id) else {
            continue;
        };
        if il.kind(lhs) != NodeKind::Var
            || !node_defines_name(il, interner, lhs, local_name)
            || !local_assignment_visible_at(il, node_id, receiver)
        {
            continue;
        }
        if last_assignment
            .map(|(start, _)| start <= node.span.start_byte)
            .unwrap_or(true)
        {
            last_assignment = Some((node.span.start_byte, rhs));
        }
    }
    last_assignment.map(|(_, rhs)| rhs)
}

fn local_assignment_visible_at(il: &nose_il::Il, assignment: NodeId, occurrence: NodeId) -> bool {
    let Some(block) = nearest_block_containing_node(il, assignment) else {
        return false;
    };
    let block_span = il.node(block).span;
    let occurrence_span = il.node(occurrence).span;
    block_span.file == occurrence_span.file
        && block_span.start_byte <= occurrence_span.start_byte
        && occurrence_span.end_byte <= block_span.end_byte
}

fn nearest_block_containing_node(il: &nose_il::Il, target: NodeId) -> Option<NodeId> {
    let target_span = il.node(target).span;
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.kind == NodeKind::Block
                && node.span.file == target_span.file
                && node.span.start_byte <= target_span.start_byte
                && target_span.end_byte <= node.span.end_byte
        })
        .min_by_key(|(_, node)| node.span.end_byte.saturating_sub(node.span.start_byte))
        .map(|(idx, _)| NodeId(idx as u32))
}

fn is_runtime_unwrap_method(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    matches!(
        callee_field_method(il, interner, callee),
        Some("unwrap" | "expect")
    )
}

fn is_runtime_result_adapter(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    callee_field_method(il, interner, callee) == Some("map_err")
}

fn is_runtime_builder_chain_method(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    matches!(
        callee_field_method(il, interner, callee),
        Some(
            "disable_lifo_slot"
                | "enable_all"
                | "enable_io"
                | "enable_time"
                | "event_interval"
                | "global_queue_interval"
                | "worker_threads"
                | "max_blocking_threads"
                | "start_paused"
                | "thread_keep_alive"
                | "thread_name"
                | "thread_stack_size"
                | "unhandled_panic"
        )
    )
}

fn is_runtime_driver_path(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    path: &str,
    context: &AdmissionContext,
) -> bool {
    match path {
        "tokio::runtime::Handle::current" => true,
        "Handle::current" => imported_runtime_type_visible(il, interner, callee, "Handle", context),
        _ => false,
    }
}

fn is_runtime_result_path(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    path: &str,
    context: &AdmissionContext,
) -> bool {
    match path {
        "tokio::runtime::Runtime::new" | "tokio::runtime::Handle::try_current" => true,
        "Runtime::new" => imported_runtime_type_visible(il, interner, callee, "Runtime", context),
        "Handle::try_current" => {
            imported_runtime_type_visible(il, interner, callee, "Handle", context)
        }
        _ => false,
    }
}

fn is_runtime_builder_path(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    path: &str,
    context: &AdmissionContext,
) -> bool {
    match path {
        "tokio::runtime::Builder::new_current_thread"
        | "tokio::runtime::Builder::new_multi_thread" => true,
        "Builder::new_current_thread" | "Builder::new_multi_thread" => {
            imported_runtime_type_visible(il, interner, callee, "Builder", context)
        }
        _ => false,
    }
}
