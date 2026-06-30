use super::{callee_field_method, callee_path, method_receiver, node_defines_name};
use crate::verify_admission::AdmissionContext;
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceKind, EvidenceStatus, Interner,
    NodeId, NodeKind, SymbolEvidenceKind,
};

mod java;
mod ruby;
mod rust;
mod rust_imports;
mod swift;

pub(super) fn push_async_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    context: &AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    let Some(callee) = il.children(call).first().copied() else {
        return false;
    };
    let path = callee_path(il, interner, callee);
    match il.meta.lang {
        nose_il::Lang::Python => {
            let Some(path) = path else {
                return false;
            };
            push_python_async_runtime_call_missing_evidence(
                il, interner, callee, &path, context, labels,
            )
        }
        nose_il::Lang::Rust => rust::push_rust_async_runtime_call_missing_evidence(
            il,
            interner,
            call,
            callee,
            path.as_deref(),
            context,
            labels,
        ),
        nose_il::Lang::Java => {
            let Some(path) = path else {
                return false;
            };
            java::push_java_future_runtime_call_missing_evidence(
                il, interner, call, callee, &path, context, labels,
            )
        }
        nose_il::Lang::Swift => {
            let Some(path) = path else {
                return false;
            };
            swift::push_swift_async_runtime_call_missing_evidence(
                il, interner, callee, &path, context, labels,
            )
        }
        nose_il::Lang::Ruby => {
            let Some(path) = path else {
                return false;
            };
            ruby::push_ruby_thread_fiber_runtime_call_missing_evidence(il, interner, &path, labels)
        }
        _ => false,
    }
}

fn push_python_async_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    _callee_path: &str,
    context: &AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    if context.python_module_is_local_for_file("asyncio", &il.meta.path) {
        return false;
    }
    if let Some(receiver) = method_receiver(il, callee) {
        if python_asyncio_namespace_receiver_proven(il, interner, receiver) {
            return callee_field_method(il, interner, callee)
                .is_some_and(|method| push_python_asyncio_api_missing_evidence(method, labels));
        }
    }
    for exported in [
        "create_task",
        "ensure_future",
        "sleep",
        "gather",
        "wait",
        "run",
        "wait_for",
        "shield",
        "run_coroutine_threadsafe",
        "to_thread",
    ] {
        if python_asyncio_imported_binding_proven(il, interner, callee, exported) {
            return push_python_asyncio_api_missing_evidence(exported, labels);
        }
    }
    false
}

fn push_python_asyncio_api_missing_evidence(
    exported: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match exported {
        "create_task" | "ensure_future" => {
            push_task_spawn_missing_evidence(labels);
            true
        }
        "sleep" => {
            super::super::push_unique(labels, "timer-scheduling-contract");
            true
        }
        "gather" => {
            push_async_aggregate_all_missing_evidence(labels);
            true
        }
        "wait" => {
            push_async_aggregate_completion_missing_evidence(labels);
            super::super::push_unique(labels, "async-aggregate-cancellation-liveness-contract");
            true
        }
        "run" => {
            super::super::push_unique(labels, "future-drive-scheduling-contract");
            push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "wait_for" => {
            super::super::push_unique(labels, "timer-scheduling-contract");
            super::super::push_unique(labels, "timer-cancellation-liveness-contract");
            push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "shield" => {
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            push_future_settled_value_missing_evidence(labels);
            true
        }
        "run_coroutine_threadsafe" => {
            push_task_spawn_missing_evidence(labels);
            push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "to_thread" => {
            push_task_spawn_missing_evidence(labels);
            push_future_settled_value_missing_evidence(labels);
            push_future_callback_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        _ => false,
    }
}

fn python_asyncio_imported_binding_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    exported: &str,
) -> bool {
    nose_semantics::imported_binding_symbol(il, interner, callee, "asyncio", exported)
        && !python_imported_binding_shadowed(il, interner, callee, "asyncio", exported)
}

fn python_asyncio_namespace_receiver_proven(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Some(receiver_name) = super::super::node_exact_name(il, interner, receiver) else {
        return false;
    };
    let occurrence_span = il.node(receiver).span;
    let top_level_statements = top_level_statements(il);
    let mut import_bindings = usize::from(nose_semantics::imported_namespace_symbol(
        il, interner, receiver, "asyncio",
    ));
    let mut shadow_definitions = 0usize;
    for unit in &il.units {
        if il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == receiver_name)
        {
            shadow_definitions += 1;
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
                if !node_defines_name(il, interner, lhs, receiver_name) {
                    continue;
                }
                if top_level_statements.contains(&node_id)
                    && python_import_namespace_binding_at_span(
                        il,
                        node.span,
                        receiver_name,
                        "asyncio",
                    )
                {
                    import_bindings += 1;
                } else if definition_shadows_occurrence(il, node_id, receiver) {
                    shadow_definitions += 1;
                }
            }
            NodeKind::Param
                if node_defines_name(il, interner, node_id, receiver_name)
                    && definition_shadows_occurrence(il, node_id, receiver) =>
            {
                shadow_definitions += 1;
            }
            _ => {}
        }
    }
    import_bindings > 0 && shadow_definitions == 0
}

fn python_imported_binding_shadowed(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let Some(local_name) = super::super::node_exact_name(il, interner, callee) else {
        return false;
    };
    let occurrence_span = il.node(callee).span;
    let top_level_statements = top_level_statements(il);
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
                let is_top_level_import_binding = top_level_statements.contains(&node_id)
                    && python_import_binding_at_span(il, node.span, local_name, module, exported);
                if !is_top_level_import_binding
                    && definition_shadows_occurrence(il, node_id, callee)
                {
                    return true;
                }
            }
            NodeKind::Param
                if node_defines_name(il, interner, node_id, local_name)
                    && definition_shadows_occurrence(il, node_id, callee) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn python_import_namespace_binding_at_span(
    il: &nose_il::Il,
    span: nose_il::Span,
    local: &str,
    module: &str,
) -> bool {
    let local_hash = stable_symbol_hash(local);
    let module_hash = stable_symbol_hash(module);
    il.evidence_anchored_at(span).any(|record| {
        record.anchor == EvidenceAnchor::binding(span, local_hash)
            && record.kind
                == EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace { module_hash })
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

fn python_import_binding_at_span(
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

fn top_level_statements(il: &nose_il::Il) -> Vec<NodeId> {
    il.children(il.root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            if il.kind(node) == NodeKind::Block {
                statements.extend_from_slice(il.children(node));
            } else {
                statements.push(node);
            }
            statements
        })
}

pub(super) fn push_task_spawn_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "task-spawn-scheduling-contract");
    super::super::push_unique(labels, "task-handle-lifecycle-contract");
    super::super::push_unique(labels, "task-cancellation-liveness-contract");
}

pub(super) fn definition_shadows_occurrence(
    il: &nose_il::Il,
    definition: NodeId,
    occurrence: NodeId,
) -> bool {
    let definition_span = il.node(definition).span;
    let occurrence_span = il.node(occurrence).span;
    if definition_span.file != occurrence_span.file {
        return false;
    }
    match (il.nearest_scope(definition), il.nearest_scope(occurrence)) {
        (Some(definition_scope), Some(occurrence_scope)) => {
            let definition_scope_span = il.node(definition_scope).span;
            definition_scope == occurrence_scope
                || (definition_scope_span.file == occurrence_span.file
                    && definition_scope_span.start_byte <= occurrence_span.start_byte
                    && occurrence_span.end_byte <= definition_scope_span.end_byte)
        }
        (Some(_), None) => false,
        (None, _) => {
            il.nearest_module_scope_containing_span(definition_span)
                == il.nearest_module_scope_containing_span(occurrence_span)
        }
    }
}

pub(super) fn push_async_aggregate_all_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "async-aggregate-all-completion-contract");
    super::super::push_unique(labels, "async-aggregate-result-channel-contract");
}

pub(super) fn push_async_aggregate_first_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "async-aggregate-first-completion-contract");
    super::super::push_unique(labels, "async-aggregate-cancellation-liveness-contract");
    super::super::push_unique(labels, "async-aggregate-result-channel-contract");
}

pub(super) fn push_async_aggregate_completion_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "async-aggregate-completion-contract");
    super::super::push_unique(labels, "async-aggregate-result-channel-contract");
}

pub(super) fn push_future_settled_value_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "future-settled-value-channel-contract");
}

pub(super) fn push_future_callback_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "future-callback-demand-effect-contract");
}
