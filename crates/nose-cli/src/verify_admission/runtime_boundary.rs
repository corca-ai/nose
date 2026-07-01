use super::{
    callee_identity::callee_identity_call_evidence, push_unique, visit_subtree, AdmissionContext,
};
use async_runtime::push_async_runtime_call_missing_evidence;
use nose_il::{Interner, NodeId, NodeKind, Payload};

mod async_runtime;

#[cfg(test)]
pub(super) fn runtime_boundary_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
) -> Option<Vec<&'static str>> {
    runtime_boundary_missing_evidence_with_context(il, interner, root, &AdmissionContext::default())
}

pub(super) fn runtime_boundary_missing_evidence_with_context(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
    context: &AdmissionContext,
) -> Option<Vec<&'static str>> {
    let mut labels = vec!["lowered-runtime-boundary-contract"];
    let mut found = false;
    visit_subtree(il, root, |node| {
        if push_runtime_node_missing_evidence(il, interner, node, &mut labels) {
            found = true;
        }
        if il.kind(node) == NodeKind::Call {
            found |= push_promise_protocol_call_missing_evidence(il, interner, node, &mut labels);
            found |=
                push_async_runtime_call_missing_evidence(il, interner, node, context, &mut labels);
        }
    });
    found.then_some(labels)
}

fn push_runtime_node_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    labels: &mut Vec<&'static str>,
) -> bool {
    if push_source_protocol_missing_evidence(il, interner, node, labels) {
        return true;
    }
    match il.kind(node) {
        NodeKind::Raw => true,
        NodeKind::Try | NodeKind::Throw => {
            push_unique(labels, "exception-channel-contract");
            true
        }
        NodeKind::Splat | NodeKind::KwArg => {
            push_unique(labels, "runtime-call-shape-contract");
            true
        }
        _ => false,
    }
}

fn push_source_protocol_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    labels: &mut Vec<&'static str>,
) -> bool {
    let Some(protocol) = nose_semantics::source_protocol_at_node(il, node) else {
        return false;
    };
    match protocol {
        nose_il::SourceProtocolKind::Await => {
            push_unique(labels, "async-await-scheduling-contract");
        }
        nose_il::SourceProtocolKind::AsyncFunction => {
            push_unique(labels, "async-function-scheduling-contract");
        }
        nose_il::SourceProtocolKind::AsyncBlock => {
            push_unique(labels, "async-block-scheduling-contract");
        }
        nose_il::SourceProtocolKind::AsyncIteration => {
            push_unique(labels, "async-iteration-lifecycle-contract");
            push_unique(labels, "async-iteration-value-channel-contract");
            push_unique(labels, "async-await-scheduling-contract");
        }
        nose_il::SourceProtocolKind::AsyncContext => {
            push_unique(labels, "async-context-lifecycle-contract");
            push_unique(labels, "async-context-cleanup-contract");
            push_unique(labels, "exception-channel-contract");
            push_unique(labels, "async-await-scheduling-contract");
        }
        nose_il::SourceProtocolKind::Yield => {
            push_unique(labels, "generator-yield-lifecycle-contract");
            push_unique(labels, "generator-yield-protocol-contract");
        }
        nose_il::SourceProtocolKind::ChannelReceive | nose_il::SourceProtocolKind::ChannelSend => {
            push_go_channel_send_receive_missing_evidence(il, interner, node, labels);
            push_unique(labels, "channel-send-receive-protocol-contract");
            push_unique(labels, "channel-protocol-contract");
        }
        nose_il::SourceProtocolKind::ChannelSelect
        | nose_il::SourceProtocolKind::ChannelSelectCase
        | nose_il::SourceProtocolKind::ChannelSelectDefault => {
            push_go_channel_select_missing_evidence(Some(protocol), labels);
            push_unique(labels, "channel-select-protocol-contract");
            push_unique(labels, "channel-protocol-contract");
        }
        nose_il::SourceProtocolKind::Defer => {
            push_unique(labels, "defer-lifecycle-ordering-contract");
            push_unique(labels, "defer-callback-effect-contract");
            push_unique(labels, "concurrency-scheduling-contract");
        }
        nose_il::SourceProtocolKind::GoRoutine => {
            push_unique(labels, "goroutine-scheduling-contract");
            push_unique(labels, "goroutine-callback-effect-contract");
            push_unique(labels, "concurrency-scheduling-contract");
        }
        nose_il::SourceProtocolKind::TaskSpawn => {
            push_unique(labels, "task-spawn-scheduling-contract");
            push_unique(labels, "task-handle-lifecycle-contract");
            push_unique(labels, "task-cancellation-liveness-contract");
        }
        nose_il::SourceProtocolKind::TryPropagation => {
            push_unique(labels, "exception-channel-contract");
        }
    }
    true
}

fn push_go_channel_send_receive_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    node: NodeId,
    labels: &mut Vec<&'static str>,
) {
    match raw_node_name(il, interner, node) {
        Some("channel_send") => {
            push_unique(labels, "channel-send-synchronization-contract");
        }
        Some("channel_receive_status") => {
            push_unique(labels, "channel-receive-status-contract");
            push_unique(labels, "channel-receive-value-channel-contract");
        }
        Some("channel_receive") => {
            push_unique(labels, "channel-receive-value-channel-contract");
        }
        _ => {}
    }
}

fn push_go_channel_select_missing_evidence(
    protocol: Option<nose_il::SourceProtocolKind>,
    labels: &mut Vec<&'static str>,
) {
    match protocol {
        Some(nose_il::SourceProtocolKind::ChannelSelect) => {
            push_unique(labels, "channel-select-readiness-contract");
        }
        Some(nose_il::SourceProtocolKind::ChannelSelectCase) => {
            push_unique(labels, "channel-select-case-selection-contract");
        }
        Some(nose_il::SourceProtocolKind::ChannelSelectDefault) => {
            push_unique(labels, "channel-select-default-liveness-contract");
        }
        _ => {}
    }
}

fn raw_node_name<'a>(il: &nose_il::Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        _ => None,
    }
}

fn push_promise_protocol_call_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    labels: &mut Vec<&'static str>,
) -> bool {
    if !js_like_runtime_lang(il.meta.lang) {
        return false;
    }
    let Some(callee) = il.children(call).first().copied() else {
        return false;
    };
    let path = callee_path(il, interner, callee);
    let method = callee_field_method(il, interner, callee);
    if path
        .as_deref()
        .is_some_and(|path| promise_construct_call(il, call, path))
    {
        push_unique(labels, "promise-executor-timing-contract");
        push_unique(labels, "promise-executor-resolve-reject-callback-contract");
        push_unique(labels, "promise-executor-throw-to-rejection-contract");
        push_unique(labels, "promise-executor-callback-effect-contract");
        return true;
    }
    if path.as_deref().is_some_and(|path| {
        push_scheduling_lifecycle_call_missing_evidence(path, construct_call(il, call), labels)
    }) {
        return true;
    }
    match path.as_deref() {
        Some("Promise") => {
            push_unique(labels, "promise-non-construct-call-boundary-contract");
            true
        }
        Some("Promise.resolve") => {
            push_unique(labels, "promise-factory-settled-value-contract");
            true
        }
        Some("Promise.reject") => {
            push_unique(labels, "promise-reject-rejected-value-channel-contract");
            true
        }
        Some("Promise.all" | "Promise.allSettled" | "Promise.any" | "Promise.race") => {
            push_promise_aggregate_missing_evidence(path.as_deref().unwrap(), labels);
            true
        }
        _ if method == Some("then") => {
            let receiver_proven = promise_receiver_has_promise_like_domain(il, interner, callee);
            push_promise_receiver_producer_missing_evidence(il, interner, callee, labels);
            if !receiver_proven {
                push_unique(labels, "promise-then-promise-like-receiver-proof");
            }
            push_unique(labels, "promise-then-fulfillment-continuation-contract");
            push_unique(labels, "promise-then-rejection-continuation-contract");
            if promise_then_has_callback_slot(il, call) {
                push_unique(labels, "promise-then-callback-demand-effect-contract");
            }
            true
        }
        _ if method == Some("catch") => {
            let receiver_proven = promise_receiver_has_promise_like_domain(il, interner, callee);
            push_promise_receiver_producer_missing_evidence(il, interner, callee, labels);
            push_unique(labels, "promise-catch-rejection-continuation-contract");
            push_unique(labels, "promise-catch-callback-demand-effect-contract");
            if !receiver_proven {
                push_unique(labels, "promise-like-receiver-proof");
            }
            true
        }
        _ if method == Some("finally") => {
            let receiver_proven = promise_receiver_has_promise_like_domain(il, interner, callee);
            push_promise_receiver_producer_missing_evidence(il, interner, callee, labels);
            push_unique(labels, "promise-finally-settlement-continuation-contract");
            push_unique(labels, "promise-finally-callback-demand-effect-contract");
            if !receiver_proven {
                push_unique(labels, "promise-like-receiver-proof");
            }
            true
        }
        _ => false,
    }
}

fn push_scheduling_lifecycle_call_missing_evidence(
    callee_path: &str,
    is_construct: bool,
    labels: &mut Vec<&'static str>,
) -> bool {
    match callee_path {
        "scheduler.wait" => {
            push_unique(labels, "scheduler-wait-timing-contract");
            push_unique(labels, "scheduler-wait-cancellation-liveness-contract");
            true
        }
        "scheduler.yield" => {
            push_unique(labels, "scheduler-yield-microtask-order-contract");
            true
        }
        "AbortSignal.abort" | "AbortSignal.any" | "AbortSignal.timeout" => {
            push_unique(labels, "abort-signal-cancellation-contract");
            push_unique(labels, "abort-signal-lifecycle-contract");
            true
        }
        "AbortController" if is_construct => {
            push_unique(labels, "abort-controller-signal-lifecycle-contract");
            push_unique(labels, "abort-signal-cancellation-contract");
            true
        }
        "setTimeout" | "setImmediate" | "queueMicrotask" | "requestAnimationFrame" => {
            push_unique(labels, "timer-scheduling-contract");
            true
        }
        "setInterval" | "timers.setInterval" | "scheduler.setInterval" => {
            push_unique(labels, "interval-async-iteration-lifecycle-contract");
            push_unique(labels, "interval-cancellation-liveness-contract");
            true
        }
        "clearInterval" => {
            push_unique(labels, "interval-cancellation-liveness-contract");
            true
        }
        "clearTimeout" | "cancelAnimationFrame" => {
            push_unique(labels, "timer-cancellation-liveness-contract");
            true
        }
        _ => false,
    }
}

fn push_promise_aggregate_missing_evidence(callee_path: &str, labels: &mut Vec<&'static str>) {
    match callee_path {
        "Promise.all" => {
            push_unique(labels, "promise-aggregate-all-fulfilled-contract");
            push_unique(labels, "promise-aggregate-ordered-values-contract");
        }
        "Promise.race" => {
            push_unique(labels, "promise-aggregate-first-settled-contract");
            push_unique(labels, "promise-aggregate-cancellation-liveness-contract");
        }
        "Promise.allSettled" => {
            push_unique(labels, "promise-aggregate-all-settled-contract");
            push_unique(labels, "promise-aggregate-settled-record-shape-contract");
        }
        "Promise.any" => {
            push_unique(labels, "promise-aggregate-first-fulfilled-contract");
            push_unique(labels, "promise-aggregate-error-channel-contract");
        }
        _ => {}
    }
    push_unique(labels, "promise-aggregate-result-channel-contract");
}

fn promise_receiver_has_promise_like_domain(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
) -> bool {
    method_receiver(il, callee).is_some_and(|receiver| {
        nose_semantics::domain_evidence_for_receiver(il, interner, receiver)
            == Some(nose_il::DomainEvidence::PromiseLike)
    })
}

fn push_promise_receiver_producer_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    labels: &mut Vec<&'static str>,
) {
    let Some(receiver) = method_receiver(il, callee) else {
        return;
    };
    if promise_receiver_has_promise_like_domain(il, interner, callee) {
        return;
    }
    if receiver_is_promise_constructor_call(il, interner, receiver) {
        push_unique(labels, "promise-constructor-receiver-producer-proof");
        return;
    }
    if receiver_is_async_function_return(il, receiver) {
        push_unique(labels, "promise-async-function-return-producer-proof");
        return;
    }
    if il.kind(receiver) == NodeKind::Call {
        push_promise_call_return_receiver_missing_evidence(il, interner, receiver, labels);
    }
}

fn push_promise_call_return_receiver_missing_evidence(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    labels: &mut Vec<&'static str>,
) {
    push_unique(labels, "promise-call-return-receiver-producer-proof");
    push_unique(
        labels,
        promise_call_return_receiver_callee_evidence(callee_identity_call_evidence(
            il, interner, receiver,
        )),
    );
}

fn promise_call_return_receiver_callee_evidence(callee_evidence: &'static str) -> &'static str {
    match callee_evidence {
        "direct-function-target-present-call-contract-proof" => {
            "promise-call-return-direct-function-return-domain-proof"
        }
        "direct-method-target-present-call-contract-proof" => {
            "promise-call-return-direct-method-return-domain-proof"
        }
        "imported-function-target-present-call-contract-proof" => {
            "promise-call-return-imported-function-settled-value-contract"
        }
        "imported-member-target-present-call-contract-proof" => {
            "promise-call-return-imported-member-settled-value-contract"
        }
        "dynamic-dispatch-target-present-concrete-target-proof" => {
            "promise-call-return-dynamic-dispatch-return-domain-proof"
        }
        "call-target-evidence-rejected" => "promise-call-return-rejected-call-target-proof",
        "scoped-path-call-target-proof" => "promise-call-return-scoped-path-callee-proof",
        "local-or-parameter-call-target-proof" => {
            "promise-call-return-local-or-parameter-callee-proof"
        }
        "imported-binding-call-target-proof" => "promise-call-return-imported-binding-callee-proof",
        "imported-member-call-target-proof" => "promise-call-return-imported-member-callee-proof",
        "qualified-global-call-target-proof" => "promise-call-return-qualified-global-callee-proof",
        "unshadowed-global-call-target-proof" => {
            "promise-call-return-unshadowed-global-callee-proof"
        }
        "member-call-target-proof" => "promise-call-return-member-callee-proof",
        _ => "promise-call-return-unknown-callee-proof",
    }
}

fn method_receiver(il: &nose_il::Il, callee: NodeId) -> Option<NodeId> {
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    il.children(callee).first().copied()
}

fn node_defines_name(il: &nose_il::Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => interner.resolve(symbol) == expected,
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .is_some_and(|symbol| interner.resolve(*symbol) == expected),
        _ => false,
    }
}

fn receiver_is_promise_constructor_call(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
) -> bool {
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(&callee) = il.children(receiver).first() else {
        return false;
    };
    callee_path(il, interner, callee)
        .as_deref()
        .is_some_and(|path| promise_construct_call(il, receiver, path))
}

fn receiver_is_async_function_return(il: &nose_il::Il, receiver: NodeId) -> bool {
    if subtree_has_source_protocol(il, receiver, nose_il::SourceProtocolKind::AsyncFunction) {
        return true;
    }
    if il.kind(receiver) != NodeKind::Call {
        return false;
    }
    let Some(&callee) = il.children(receiver).first() else {
        return false;
    };
    let Some(callee_name) = callee_var_symbol(il, callee) else {
        return false;
    };
    il.units.iter().any(|unit| {
        unit.name == Some(callee_name)
            && subtree_has_source_protocol(
                il,
                unit.root,
                nose_il::SourceProtocolKind::AsyncFunction,
            )
    })
}

fn callee_var_symbol(il: &nose_il::Il, callee: NodeId) -> Option<nose_il::Symbol> {
    if il.kind(callee) != NodeKind::Var {
        return None;
    }
    match il.node(callee).payload {
        Payload::Name(name) => Some(name),
        _ => None,
    }
}

fn subtree_has_source_protocol(
    il: &nose_il::Il,
    root: NodeId,
    protocol: nose_il::SourceProtocolKind,
) -> bool {
    let mut found = false;
    visit_subtree(il, root, |node| {
        found |= nose_semantics::source_protocol_at_node(il, node) == Some(protocol);
    });
    found
}

fn promise_then_has_callback_slot(il: &nose_il::Il, call: NodeId) -> bool {
    il.children(call).len() > 1
}

fn js_like_runtime_lang(lang: nose_il::Lang) -> bool {
    matches!(
        lang,
        nose_il::Lang::JavaScript
            | nose_il::Lang::TypeScript
            | nose_il::Lang::Vue
            | nose_il::Lang::Svelte
            | nose_il::Lang::Html
    )
}

fn promise_construct_call(il: &nose_il::Il, call: NodeId, callee_path: &str) -> bool {
    callee_path == "Promise" && construct_call(il, call)
}

fn construct_call(il: &nose_il::Il, call: NodeId) -> bool {
    nose_semantics::source_call_at_node(il, call) == Some(nose_il::SourceCallKind::Construct)
}

fn callee_path(il: &nose_il::Il, interner: &Interner, node: NodeId) -> Option<String> {
    match il.kind(node) {
        NodeKind::Var => match il.node(node).payload {
            Payload::Name(name) => Some(interner.resolve(name).to_string()),
            _ => None,
        },
        NodeKind::Field => {
            let Payload::Name(method) = il.node(node).payload else {
                return None;
            };
            let receiver = il.children(node).first().copied()?;
            let receiver = callee_path(il, interner, receiver)?;
            Some(format!("{}.{}", receiver, interner.resolve(method)))
        }
        _ => None,
    }
}

fn callee_field_method<'a>(
    il: &nose_il::Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(method))
}

#[cfg(test)]
mod tests;
