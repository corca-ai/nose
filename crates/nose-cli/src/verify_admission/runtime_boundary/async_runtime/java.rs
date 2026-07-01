use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceKind, EvidenceStatus,
    ImportEvidenceKind, Interner, NodeId, NodeKind, Span, SymbolEvidenceKind, UnitKind,
};

mod import_conflicts;
mod receiver_provenance;

const JAVA_CONCURRENT_MODULE: &str = "java.util.concurrent";
const COMPLETABLE_FUTURE_TYPE: &str = "CompletableFuture";
const COMPLETABLE_FUTURE_QUALIFIED: &str = "java.util.concurrent.CompletableFuture";
const COMPLETION_STAGE_TYPE: &str = "CompletionStage";
const FUTURE_TYPE: &str = "Future";
const SCHEDULED_FUTURE_TYPE: &str = "ScheduledFuture";
const EXECUTOR_TYPE: &str = "Executor";
const EXECUTOR_SERVICE_TYPE: &str = "ExecutorService";
const SCHEDULED_EXECUTOR_SERVICE_TYPE: &str = "ScheduledExecutorService";

#[derive(Clone, Copy, PartialEq, Eq)]
enum JavaExecutorKind {
    Executor,
    ExecutorService,
    ScheduledExecutorService,
}

pub(super) fn push_java_future_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    callee: NodeId,
    callee_path: &str,
    context: &crate::verify_admission::AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    if completable_future_construct_call(il, interner, call, callee, callee_path, context) {
        push_completable_future_constructor_missing_evidence(labels);
        return true;
    }

    if let Some(method) =
        completable_future_static_method(il, interner, call, callee, callee_path, context)
    {
        return push_completable_future_static_method_missing_evidence(method, labels);
    }

    let Some(method) = super::super::callee_field_method(il, interner, callee) else {
        return false;
    };
    if receiver_provenance::completion_stage_receiver_proven(il, interner, callee, context)
        && push_completion_stage_continuation_missing_evidence(method, labels)
    {
        return true;
    }
    if receiver_provenance::completable_future_receiver_proven(il, interner, callee, context)
        && push_completable_future_receiver_method_missing_evidence(method, labels)
    {
        return true;
    }
    if receiver_provenance::future_handle_receiver_proven(il, interner, callee, context)
        && push_future_handle_method_missing_evidence(method, labels)
    {
        return true;
    }
    if let Some(kind) =
        receiver_provenance::executor_receiver_kind_proven(il, interner, callee, context)
    {
        if push_executor_method_missing_evidence(kind, method, labels) {
            return true;
        }
    }
    false
}

fn completable_future_construct_call(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    callee: NodeId,
    callee_path: &str,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    if !super::super::construct_call(il, call) {
        return false;
    }
    if callee_path == COMPLETABLE_FUTURE_QUALIFIED {
        return true;
    }
    if callee_path != COMPLETABLE_FUTURE_TYPE {
        return false;
    }
    java_completable_future_simple_receiver_proven(il, interner, call, callee, context)
}

fn completable_future_static_method<'a>(
    il: &nose_il::Il,
    interner: &'a Interner,
    call: NodeId,
    callee: NodeId,
    callee_path: &'a str,
    context: &crate::verify_admission::AdmissionContext,
) -> Option<&'a str> {
    let (receiver_path, method) = callee_path.rsplit_once('.')?;
    if receiver_path == COMPLETABLE_FUTURE_QUALIFIED {
        return Some(method);
    }
    if receiver_path != COMPLETABLE_FUTURE_TYPE {
        return None;
    }
    let receiver = super::super::method_receiver(il, callee)?;
    java_completable_future_simple_receiver_proven(il, interner, call, receiver, context)
        .then_some(method)
}

fn java_completable_future_simple_receiver_proven(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    receiver: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    il.kind(receiver) == NodeKind::Var
        && super::super::node_defines_name(il, interner, receiver, COMPLETABLE_FUTURE_TYPE)
        && !java_simple_type_shadowed(il, interner, receiver)
        && java_completable_future_import_proven(il, interner, call, receiver, context)
}

fn java_completable_future_import_proven(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    receiver: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    java_imported_binding_symbol_usable_for_type(
        il,
        interner,
        receiver,
        COMPLETABLE_FUTURE_TYPE,
        context,
    ) || java_wildcard_import_proves_completable_future(il, interner, call, context)
}

fn java_imported_binding_symbol_usable_for_type(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    type_name: &str,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    if il.kind(node) != NodeKind::Var
        || !super::super::node_defines_name(il, interner, node, type_name)
    {
        return false;
    }
    let span = il.node(node).span;
    let local_hash = stable_symbol_hash(type_name);
    let expected = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(JAVA_CONCURRENT_MODULE),
        exported_hash: stable_symbol_hash(type_name),
    });
    il.evidence.iter().any(|record| {
        record.kind == expected
            && matches!(
                record.anchor,
                EvidenceAnchor::Binding { span: import_span, local_hash: actual }
                    if actual == local_hash
                        && import_span.file == span.file
                        && import_span.end_byte <= span.start_byte
            )
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && il.evidence_dependencies_asserted(record)
            && (!java_imported_binding_is_wildcard_backed(il, record)
                || !context.java_package_local_type_is_visible_in_file(il, interner, type_name))
    })
}

fn java_simple_type_shadowed(il: &nose_il::Il, interner: &Interner, receiver: NodeId) -> bool {
    let occurrence_span = il.node(receiver).span;
    let top_level_statements = super::top_level_statements(il);
    for unit in &il.units {
        if il.node(unit.root).span.file == occurrence_span.file
            && unit.kind == UnitKind::Class
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == COMPLETABLE_FUTURE_TYPE)
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
                if !super::super::node_defines_name(il, interner, lhs, COMPLETABLE_FUTURE_TYPE) {
                    continue;
                }
                if !top_level_statements.contains(&node_id)
                    || !java_imported_completable_future_at_span(il, node.span)
                {
                    return true;
                }
            }
            NodeKind::Param
                if super::super::node_defines_name(
                    il,
                    interner,
                    node_id,
                    COMPLETABLE_FUTURE_TYPE,
                ) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn java_imported_completable_future_at_span(il: &nose_il::Il, span: Span) -> bool {
    let local_hash = stable_symbol_hash(COMPLETABLE_FUTURE_TYPE);
    let module_hash = stable_symbol_hash(JAVA_CONCURRENT_MODULE);
    let exported_hash = stable_symbol_hash(COMPLETABLE_FUTURE_TYPE);
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

fn java_wildcard_import_proves_completable_future(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    if context.java_package_local_type_is_visible_in_file(il, interner, COMPLETABLE_FUTURE_TYPE) {
        return false;
    }
    let call_span = il.node(call).span;
    if import_conflicts::type_import_conflicted_at_span(il, call_span, COMPLETABLE_FUTURE_TYPE) {
        return false;
    }
    let expected = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(JAVA_CONCURRENT_MODULE),
    });
    il.evidence.iter().any(|record| {
        record.kind == expected
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && matches!(
                record.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == call_span.file && span.end_byte <= call_span.start_byte
            )
            && il.evidence_dependencies_asserted(record)
    })
}

pub(super) fn java_imported_binding_is_wildcard_backed(
    il: &nose_il::Il,
    record: &nose_il::EvidenceRecord,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(JAVA_CONCURRENT_MODULE),
    });
    record.dependencies.iter().copied().any(|dependency| {
        il.evidence
            .get(dependency.0 as usize)
            .is_some_and(|dependency_record| {
                dependency_record.kind == expected
                    && dependency_record.status == EvidenceStatus::Asserted
                    && dependency_record.provenance.emitter == EvidenceEmitter::Builtin
                    && il.evidence_dependencies_asserted(dependency_record)
            })
    })
}

fn push_completable_future_constructor_missing_evidence(labels: &mut Vec<&'static str>) {
    super::push_future_settled_value_missing_evidence(labels);
    super::super::push_unique(labels, "exception-channel-contract");
    super::super::push_unique(labels, "task-handle-lifecycle-contract");
    super::super::push_unique(labels, "task-cancellation-liveness-contract");
}

fn push_completable_future_static_method_missing_evidence(
    method: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match method {
        "supplyAsync" | "runAsync" => {
            super::push_task_spawn_missing_evidence(labels);
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "completedFuture" | "completedStage" => {
            super::push_future_settled_value_missing_evidence(labels);
            true
        }
        "failedFuture" | "failedStage" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "allOf" => {
            super::push_async_aggregate_all_missing_evidence(labels);
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "anyOf" => {
            super::push_async_aggregate_first_missing_evidence(labels);
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        _ => false,
    }
}

fn push_completion_stage_continuation_missing_evidence(
    method: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match method {
        "thenApply" | "thenApplyAsync" | "thenAccept" | "thenAcceptAsync" | "thenRun"
        | "thenRunAsync" | "thenCompose" | "thenComposeAsync" => {
            super::super::push_unique(labels, "future-fulfillment-continuation-contract");
            push_future_result_callback_missing_evidence(labels);
            true
        }
        "thenCombine"
        | "thenCombineAsync"
        | "thenAcceptBoth"
        | "thenAcceptBothAsync"
        | "runAfterBoth"
        | "runAfterBothAsync" => {
            super::push_async_aggregate_all_missing_evidence(labels);
            super::super::push_unique(labels, "future-fulfillment-continuation-contract");
            push_future_result_callback_missing_evidence(labels);
            true
        }
        "applyToEither"
        | "applyToEitherAsync"
        | "acceptEither"
        | "acceptEitherAsync"
        | "runAfterEither"
        | "runAfterEitherAsync" => {
            super::push_async_aggregate_first_missing_evidence(labels);
            super::super::push_unique(labels, "future-fulfillment-continuation-contract");
            push_future_result_callback_missing_evidence(labels);
            true
        }
        "exceptionally"
        | "exceptionallyAsync"
        | "exceptionallyCompose"
        | "exceptionallyComposeAsync" => {
            super::super::push_unique(labels, "future-exception-continuation-contract");
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "handle" | "handleAsync" | "whenComplete" | "whenCompleteAsync" => {
            super::super::push_unique(labels, "future-settlement-continuation-contract");
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        _ => false,
    }
}

fn push_completable_future_receiver_method_missing_evidence(
    method: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match method {
        "complete" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            true
        }
        "completeExceptionally" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            true
        }
        "join" | "getNow" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            true
        }
        "isCompletedExceptionally" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            true
        }
        "orTimeout" => {
            super::super::push_unique(labels, "timer-scheduling-contract");
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            true
        }
        "completeOnTimeout" => {
            super::super::push_unique(labels, "timer-scheduling-contract");
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            true
        }
        _ => false,
    }
}

fn push_future_handle_method_missing_evidence(
    method: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match method {
        "get" => {
            super::push_future_settled_value_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            true
        }
        "cancel" => {
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            true
        }
        "isDone" => {
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            true
        }
        "isCancelled" => {
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            true
        }
        _ => false,
    }
}

fn push_executor_method_missing_evidence(
    kind: JavaExecutorKind,
    method: &str,
    labels: &mut Vec<&'static str>,
) -> bool {
    match method {
        "execute" => {
            super::super::push_unique(labels, "task-spawn-scheduling-contract");
            super::push_future_callback_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        "submit" if kind != JavaExecutorKind::Executor => {
            super::push_task_spawn_missing_evidence(labels);
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "invokeAll" if kind != JavaExecutorKind::Executor => {
            super::push_async_aggregate_all_missing_evidence(labels);
            super::super::push_unique(labels, "async-aggregate-cancellation-liveness-contract");
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "invokeAny" if kind != JavaExecutorKind::Executor => {
            super::push_async_aggregate_first_missing_evidence(labels);
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "schedule" if kind == JavaExecutorKind::ScheduledExecutorService => {
            super::super::push_unique(labels, "timer-scheduling-contract");
            super::push_task_spawn_missing_evidence(labels);
            push_future_result_callback_exception_missing_evidence(labels);
            true
        }
        "scheduleAtFixedRate" | "scheduleWithFixedDelay"
            if kind == JavaExecutorKind::ScheduledExecutorService =>
        {
            super::super::push_unique(labels, "timer-scheduling-contract");
            super::super::push_unique(labels, "interval-async-iteration-lifecycle-contract");
            super::super::push_unique(labels, "interval-cancellation-liveness-contract");
            super::super::push_unique(labels, "task-handle-lifecycle-contract");
            super::super::push_unique(labels, "task-cancellation-liveness-contract");
            super::push_future_callback_missing_evidence(labels);
            super::super::push_unique(labels, "exception-channel-contract");
            true
        }
        _ => false,
    }
}

fn push_future_result_callback_missing_evidence(labels: &mut Vec<&'static str>) {
    super::push_future_settled_value_missing_evidence(labels);
    super::push_future_callback_missing_evidence(labels);
}

fn push_future_result_callback_exception_missing_evidence(labels: &mut Vec<&'static str>) {
    push_future_result_callback_missing_evidence(labels);
    super::super::push_unique(labels, "exception-channel-contract");
}
