use super::{callee_field_method, callee_path, method_receiver, node_defines_name, rust_imports};
use crate::verify_admission::AdmissionContext;
use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceKind,
    EvidenceStatus, Interner, NodeId, NodeKind, SymbolEvidenceKind,
};

pub(super) fn push_rust_async_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    if callee_path
        .and_then(runtime_root)
        .is_some_and(|root| context.rust_runtime_root_is_local_for_file(root, &il.meta.path))
    {
        return false;
    }
    let is_macro_invocation = nose_semantics::source_call_at_node(il, call)
        == Some(nose_il::SourceCallKind::MacroInvocation);
    if !is_macro_invocation {
        if callee_path.is_some_and(rust_async_spawn_path)
            || rust_imported_async_spawn_member(il, interner, callee, context)
        {
            super::push_task_spawn_missing_evidence(labels);
            return true;
        }
        if rust_future_drive_call(il, interner, callee, callee_path, context) {
            push_future_drive_missing_evidence(labels);
            return true;
        }
    }
    if !is_macro_invocation {
        return false;
    }
    if callee_path.is_some_and(rust_async_join_macro_path) {
        super::push_async_aggregate_all_missing_evidence(labels);
        return true;
    }
    if rust_imported_async_join_macro_member(il, interner, callee, context) {
        super::push_async_aggregate_all_missing_evidence(labels);
        return true;
    }
    if callee_path.is_some_and(rust_async_select_macro_path) {
        super::push_async_aggregate_first_missing_evidence(labels);
        return true;
    }
    if rust_imported_async_select_macro_member(il, interner, callee, context) {
        super::push_async_aggregate_first_missing_evidence(labels);
        return true;
    }
    false
}

fn runtime_root(callee_path: &str) -> Option<&str> {
    callee_path.split("::").next()
}

fn module_root(module: &str) -> &str {
    module.split("::").next().unwrap_or(module)
}

fn rust_async_spawn_path(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::spawn"
            | "tokio::task::spawn"
            | "tokio::task::spawn_blocking"
            | "async_std::task::spawn"
            | "async_std::task::spawn_blocking"
    )
}

fn rust_async_join_macro_path(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::join"
            | "tokio::try_join"
            | "futures::join"
            | "futures::try_join"
            | "futures_util::join"
            | "futures_util::try_join"
    )
}

fn rust_async_select_macro_path(callee_path: &str) -> bool {
    matches!(
        callee_path,
        "tokio::select" | "futures::select" | "futures_util::select"
    )
}

fn rust_future_drive_call(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    callee_path: Option<&str>,
    context: &AdmissionContext,
) -> bool {
    callee_path == Some("tokio_test::block_on")
        || rust_imported_runtime_member(il, interner, callee, "tokio_test", "block_on", context)
        || (callee_field_method(il, interner, callee) == Some("block_on")
            && method_receiver(il, callee).is_some_and(|receiver| {
                rust_tokio_runtime_block_on_receiver(il, interner, receiver, context)
            }))
}

fn rust_tokio_runtime_block_on_receiver(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if context.rust_runtime_root_is_local_for_file("tokio", &il.meta.path) {
        return false;
    }
    rust_tokio_runtime_driver_receiver_expr(il, interner, receiver, context)
        || rust_tokio_runtime_local_binding_receiver_expr(il, interner, receiver, context)
        || rust_tokio_runtime_parameter_receiver_expr(il, interner, receiver)
        || rust_tokio_runtime_field_receiver_expr(il, receiver)
}

fn rust_tokio_runtime_driver_receiver_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if let Some(inner) = rust_try_propagation_operand(il, receiver) {
        return rust_tokio_runtime_driver_result_expr(il, interner, inner, context);
    }
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(callee) = il.children(receiver).first().copied() else {
        return false;
    };
    if callee_path(il, interner, callee)
        .as_deref()
        .is_some_and(|path| rust_tokio_runtime_driver_path(il, interner, callee, path, context))
    {
        return true;
    }
    if !rust_tokio_runtime_unwrap_method(il, interner, callee) {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| rust_tokio_runtime_driver_result_expr(il, interner, inner, context))
}

fn rust_try_propagation_operand(il: &nose_il::Il, node: NodeId) -> Option<NodeId> {
    (nose_semantics::source_protocol_at_node(il, node)
        == Some(nose_il::SourceProtocolKind::TryPropagation))
    .then(|| il.children(node).first().copied())
    .flatten()
}

fn rust_tokio_runtime_driver_result_expr(
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
        .is_some_and(|path| rust_tokio_runtime_result_path(il, interner, callee, path, context))
    {
        return true;
    }
    if rust_tokio_runtime_result_adapter_method(il, interner, callee) {
        return method_receiver(il, callee).is_some_and(|inner| {
            rust_tokio_runtime_driver_result_expr(il, interner, inner, context)
        });
    }
    if callee_field_method(il, interner, callee) != Some("build") {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| rust_tokio_runtime_builder_expr(il, interner, inner, context))
}

fn rust_tokio_runtime_builder_expr(
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
        .is_some_and(|path| rust_tokio_runtime_builder_path(il, interner, callee, path, context))
    {
        return true;
    }
    if !rust_tokio_runtime_builder_chain_method(il, interner, callee) {
        return false;
    }
    method_receiver(il, callee)
        .is_some_and(|inner| rust_tokio_runtime_builder_expr(il, interner, inner, context))
}

fn rust_tokio_runtime_local_binding_receiver_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    context: &AdmissionContext,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Some(local_name) = super::super::super::node_exact_name(il, interner, receiver) else {
        return false;
    };
    rust_last_visible_local_assignment_rhs(il, interner, receiver, local_name)
        .is_some_and(|rhs| rust_tokio_runtime_driver_receiver_expr(il, interner, rhs, context))
}

fn rust_tokio_runtime_parameter_receiver_expr(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    matches!(
        nose_semantics::domain_evidence_for_receiver(il, interner, receiver),
        Some(DomainEvidence::Nominal { type_hash })
            if type_hash == stable_symbol_hash("tokio::runtime::Runtime")
                || type_hash == stable_symbol_hash("tokio::runtime::Handle")
    )
}

fn rust_tokio_runtime_field_receiver_expr(il: &nose_il::Il, receiver: NodeId) -> bool {
    if il.kind(receiver) != NodeKind::Field {
        return false;
    }
    matches!(
        nose_semantics::domain_evidence_for_node(il, receiver),
        Some(DomainEvidence::Nominal { type_hash })
            if type_hash == stable_symbol_hash("tokio::runtime::Runtime")
                || type_hash == stable_symbol_hash("tokio::runtime::Handle")
    )
}

fn rust_last_visible_local_assignment_rhs(
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
            || !rust_local_assignment_visible_at(il, node_id, receiver)
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

fn rust_local_assignment_visible_at(
    il: &nose_il::Il,
    assignment: NodeId,
    occurrence: NodeId,
) -> bool {
    let Some(block) = rust_nearest_block_containing_node(il, assignment) else {
        return false;
    };
    let block_span = il.node(block).span;
    let occurrence_span = il.node(occurrence).span;
    block_span.file == occurrence_span.file
        && block_span.start_byte <= occurrence_span.start_byte
        && occurrence_span.end_byte <= block_span.end_byte
}

fn rust_nearest_block_containing_node(il: &nose_il::Il, target: NodeId) -> Option<NodeId> {
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

fn rust_tokio_runtime_unwrap_method(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    matches!(
        callee_field_method(il, interner, callee),
        Some("unwrap" | "expect")
    )
}

fn rust_tokio_runtime_result_adapter_method(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
) -> bool {
    matches!(callee_field_method(il, interner, callee), Some("map_err"))
}

fn rust_tokio_runtime_builder_chain_method(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
) -> bool {
    matches!(
        callee_field_method(il, interner, callee),
        Some(
            "enable_all"
                | "enable_io"
                | "enable_time"
                | "worker_threads"
                | "max_blocking_threads"
                | "thread_name"
                | "thread_stack_size"
        )
    )
}

fn rust_tokio_runtime_driver_path(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    path: &str,
    context: &AdmissionContext,
) -> bool {
    match path {
        "tokio::runtime::Handle::current" => true,
        "Handle::current" => {
            rust_imported_runtime_type_visible(il, interner, callee, "Handle", context)
        }
        _ => false,
    }
}

fn rust_tokio_runtime_result_path(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    path: &str,
    context: &AdmissionContext,
) -> bool {
    match path {
        "tokio::runtime::Runtime::new" | "tokio::runtime::Handle::try_current" => true,
        "Runtime::new" => {
            rust_imported_runtime_type_visible(il, interner, callee, "Runtime", context)
        }
        "Handle::try_current" => {
            rust_imported_runtime_type_visible(il, interner, callee, "Handle", context)
        }
        _ => false,
    }
}

fn rust_tokio_runtime_builder_path(
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
            rust_imported_runtime_type_visible(il, interner, callee, "Builder", context)
        }
        _ => false,
    }
}

fn rust_imported_runtime_type_visible(
    il: &nose_il::Il,
    interner: &Interner,
    occurrence: NodeId,
    exported: &str,
    context: &AdmissionContext,
) -> bool {
    let module = "tokio::runtime";
    if context.rust_runtime_root_is_local_for_file(module_root(module), &il.meta.path) {
        return false;
    }
    rust_imports::rust_imported_binding_evidence_only_symbol_for_local(
        il, exported, occurrence, module, exported,
    ) && !rust_imported_local_shadowed(il, interner, occurrence, exported, module, exported)
}

fn rust_imported_async_spawn_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    rust_imported_runtime_member(il, interner, callee, "tokio", "spawn", context)
        || rust_imported_runtime_member(il, interner, callee, "tokio::task", "spawn", context)
        || rust_imported_runtime_member(
            il,
            interner,
            callee,
            "tokio::task",
            "spawn_blocking",
            context,
        )
        || rust_imported_runtime_member(il, interner, callee, "async_std::task", "spawn", context)
        || rust_imported_runtime_member(
            il,
            interner,
            callee,
            "async_std::task",
            "spawn_blocking",
            context,
        )
}

fn rust_imported_async_join_macro_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    rust_imported_runtime_member(il, interner, callee, "tokio", "join", context)
        || rust_imported_runtime_member(il, interner, callee, "tokio", "try_join", context)
        || rust_imported_runtime_member(il, interner, callee, "futures", "join", context)
        || rust_imported_runtime_member(il, interner, callee, "futures", "try_join", context)
        || rust_imported_runtime_member(il, interner, callee, "futures_util", "join", context)
        || rust_imported_runtime_member(il, interner, callee, "futures_util", "try_join", context)
}

fn rust_imported_async_select_macro_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    rust_imported_runtime_member(il, interner, callee, "tokio", "select", context)
        || rust_imported_runtime_member(il, interner, callee, "futures", "select", context)
        || rust_imported_runtime_member(il, interner, callee, "futures_util", "select", context)
}

fn rust_imported_runtime_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
    context: &AdmissionContext,
) -> bool {
    if context.rust_runtime_root_is_local_for_file(module_root(module), &il.meta.path) {
        return false;
    }
    (nose_semantics::imported_member_symbol(il, interner, callee, module, exported)
        || rust_imports::rust_imported_binding_evidence_only_symbol(
            il, interner, callee, module, exported,
        ))
        && !rust_imported_member_shadowed(il, interner, callee, module, exported)
}

fn rust_imported_member_shadowed(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let Some(local_name) = super::super::super::node_exact_name(il, interner, callee) else {
        return false;
    };
    rust_imported_local_shadowed(il, interner, callee, local_name, module, exported)
}

fn rust_imported_local_shadowed(
    il: &nose_il::Il,
    interner: &Interner,
    occurrence: NodeId,
    local_name: &str,
    module: &str,
    exported: &str,
) -> bool {
    let occurrence_span = il.node(occurrence).span;
    for unit in &il.units {
        if il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == local_name)
        {
            return true;
        }
    }
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span.file != occurrence_span.file {
            continue;
        }
        let node_id = NodeId(idx as u32);
        match node.kind {
            NodeKind::Assign => {
                let Some(lhs) = il.children(node_id).first().copied() else {
                    continue;
                };
                if !node_defines_name(il, interner, lhs, local_name) {
                    continue;
                }
                if !rust_imported_binding_at_span(il, node.span, local_name, module, exported)
                    && super::definition_shadows_occurrence(il, node_id, occurrence)
                {
                    return true;
                }
            }
            NodeKind::Block | NodeKind::Module | NodeKind::Param
                if node_defines_name(il, interner, node_id, local_name)
                    && super::definition_shadows_occurrence(il, node_id, occurrence) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn rust_imported_binding_at_span(
    il: &nose_il::Il,
    span: nose_il::Span,
    local: &str,
    module: &str,
    exported: &str,
) -> bool {
    let local_hash = stable_symbol_hash(local);
    let module_hash = stable_symbol_hash(module);
    let exported_hash = stable_symbol_hash(exported);
    il.evidence_anchored_at(span).any(|record| {
        record.anchor == EvidenceAnchor::binding(span, local_hash)
            && record.kind
                == EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                    module_hash,
                    exported_hash,
                })
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

fn push_future_drive_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "future-drive-scheduling-contract");
    super::super::push_unique(labels, "future-settled-value-channel-contract");
}
