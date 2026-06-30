use crate::verify_admission::AdmissionContext;
use nose_il::{Interner, NodeId, NodeKind};

pub(super) fn push_swift_async_runtime_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    callee_path: &str,
    context: &AdmissionContext,
    labels: &mut Vec<&'static str>,
) -> bool {
    match callee_path {
        "Task" | "Task.detached"
            if swift_task_runtime_root_unshadowed(il, interner, callee, context) =>
        {
            super::push_task_spawn_missing_evidence(labels);
            true
        }
        "Task.sleep" if swift_task_runtime_root_unshadowed(il, interner, callee, context) => {
            push_swift_task_sleep_missing_evidence(labels);
            true
        }
        "Task.yield" if swift_task_runtime_root_unshadowed(il, interner, callee, context) => {
            push_task_yield_missing_evidence(labels);
            true
        }
        "withTaskGroup"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                "withTaskGroup",
                context,
            ) =>
        {
            push_swift_task_group_missing_evidence(labels, false, false);
            true
        }
        "withThrowingTaskGroup"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                "withThrowingTaskGroup",
                context,
            ) =>
        {
            push_swift_task_group_missing_evidence(labels, true, false);
            true
        }
        "withDiscardingTaskGroup"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                "withDiscardingTaskGroup",
                context,
            ) =>
        {
            push_swift_task_group_missing_evidence(labels, false, true);
            true
        }
        "withThrowingDiscardingTaskGroup"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                "withThrowingDiscardingTaskGroup",
                context,
            ) =>
        {
            push_swift_task_group_missing_evidence(labels, true, true);
            true
        }
        "withCheckedContinuation" | "withUnsafeContinuation"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                callee_path,
                context,
            ) =>
        {
            push_swift_continuation_missing_evidence(labels, false);
            true
        }
        "withCheckedThrowingContinuation" | "withUnsafeThrowingContinuation"
            if swift_free_runtime_function_unshadowed(
                il,
                interner,
                callee,
                callee_path,
                context,
            ) =>
        {
            push_swift_continuation_missing_evidence(labels, true);
            true
        }
        _ => false,
    }
}

fn swift_task_root_unshadowed(il: &nose_il::Il, interner: &Interner, callee: NodeId) -> bool {
    let root = if il.kind(callee) == NodeKind::Field {
        super::super::method_receiver(il, callee).unwrap_or(callee)
    } else {
        callee
    };
    il.kind(root) == NodeKind::Var
        && super::super::node_defines_name(il, interner, root, "Task")
        && !nose_semantics::file_defines_name_visible_at(il, interner, "Task", il.node(root).span)
}

fn swift_task_runtime_root_unshadowed(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    !context.swift_name_is_visible("Task") && swift_task_root_unshadowed(il, interner, callee)
}

fn swift_free_runtime_function_unshadowed(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    name: &str,
    context: &AdmissionContext,
) -> bool {
    il.kind(callee) == NodeKind::Var
        && super::super::node_defines_name(il, interner, callee, name)
        && !context.swift_name_is_visible(name)
        && !nose_semantics::file_defines_name_visible_at(il, interner, name, il.node(callee).span)
}

fn push_swift_task_sleep_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "timer-scheduling-contract");
    super::super::push_unique(labels, "task-cancellation-liveness-contract");
}

fn push_task_yield_missing_evidence(labels: &mut Vec<&'static str>) {
    super::super::push_unique(labels, "task-yield-scheduling-contract");
}

fn push_swift_task_group_missing_evidence(
    labels: &mut Vec<&'static str>,
    throwing: bool,
    discarding: bool,
) {
    super::super::push_unique(labels, "async-aggregate-all-completion-contract");
    super::super::push_unique(labels, "async-aggregate-cancellation-liveness-contract");
    if !discarding {
        super::super::push_unique(labels, "async-aggregate-result-channel-contract");
    }
    if throwing {
        super::super::push_unique(labels, "exception-channel-contract");
    }
}

fn push_swift_continuation_missing_evidence(labels: &mut Vec<&'static str>, throwing: bool) {
    super::push_future_settled_value_missing_evidence(labels);
    super::super::push_unique(labels, "future-settlement-continuation-contract");
    super::push_future_callback_missing_evidence(labels);
    if throwing {
        super::super::push_unique(labels, "exception-channel-contract");
    }
}
