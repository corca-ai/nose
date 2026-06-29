use super::*;
use nose_il::{FileId, Lang};

fn lowered_js(src: &str) -> (nose_il::Il, Interner) {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "promise.js",
        src.as_bytes(),
        Lang::JavaScript,
        &interner,
    )
    .expect("lower JavaScript fixture");
    (il, interner)
}

fn lowered_source(path: &str, src: &str, lang: Lang) -> (nose_il::Il, Interner) {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(FileId(0), path, src.as_bytes(), lang, &interner)
        .unwrap_or_else(|err| panic!("lower {path}: {err}"));
    (il, interner)
}

fn missing_evidence_for_protocol(
    path: &str,
    src: &str,
    lang: Lang,
    protocol: nose_il::SourceProtocolKind,
) -> Vec<&'static str> {
    let (il, interner) = lowered_source(path, src, lang);
    let node = (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .find(|&node| nose_semantics::source_protocol_at_node(&il, node) == Some(protocol))
        .unwrap_or_else(|| panic!("expected {protocol:?} node in {path}"));
    runtime_boundary_missing_evidence(&il, &interner, node)
        .unwrap_or_else(|| panic!("expected runtime boundary evidence for {protocol:?} in {path}"))
}

fn missing_evidence_for_call(src: &str, callee_suffix: &str) -> Vec<&'static str> {
    let (il, interner) = lowered_js(src);
    let call = (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .find(|&node| {
            il.kind(node) == nose_il::NodeKind::Call
                && call_matches_callee_surface(&il, &interner, node, callee_suffix)
        })
        .unwrap_or_else(|| panic!("expected call ending in {callee_suffix}"));
    runtime_boundary_missing_evidence(&il, &interner, call)
        .unwrap_or_else(|| panic!("expected runtime boundary evidence for {callee_suffix}"))
}

fn call_matches_callee_surface(
    il: &nose_il::Il,
    interner: &Interner,
    call: NodeId,
    callee_suffix: &str,
) -> bool {
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    if callee_path(il, interner, callee).is_some_and(|path| path.ends_with(callee_suffix)) {
        return true;
    }
    callee_suffix
        .strip_prefix('.')
        .is_some_and(|method| callee_field_method(il, interner, callee) == Some(method))
}

#[test]
fn await_protocol_missing_evidence_is_language_neutral() {
    for (path, src, lang) in [
        (
            "await.js",
            "async function read(x) { return await x; }\n",
            Lang::JavaScript,
        ),
        (
            "await.ts",
            "async function read(x: Promise<number>) { return await x; }\n",
            Lang::TypeScript,
        ),
        (
            "await.py",
            "async def read(x):\n    return await x\n",
            Lang::Python,
        ),
        (
            "await.rs",
            "pub async fn read(x: i32) -> i32 { async move { x }.await }\n",
            Lang::Rust,
        ),
        (
            "await.swift",
            "func read(_ work: () async -> Int) async -> Int {\n  return await work()\n}\n",
            Lang::Swift,
        ),
    ] {
        let labels =
            missing_evidence_for_protocol(path, src, lang, nose_il::SourceProtocolKind::Await);
        assert!(
            labels.contains(&"async-await-scheduling-contract"),
            "{path} should report the shared await scheduling contract: {labels:?}"
        );
        assert!(
            !labels.contains(&"promise-await-scheduling-contract"),
            "{path} should not report plain await as Promise-specific evidence: {labels:?}"
        );
    }
}

#[test]
fn promise_then_missing_evidence_splits_receiver_fulfillment_rejection_and_callback() {
    let labels = missing_evidence_for_call(
        "function thenIt(p, f, r) { return p.then(f, r); }\n",
        ".then",
    );

    assert!(labels.contains(&"promise-then-promise-like-receiver-proof"));
    assert!(labels.contains(&"promise-then-fulfillment-continuation-contract"));
    assert!(labels.contains(&"promise-then-rejection-continuation-contract"));
    assert!(labels.contains(&"promise-then-callback-demand-effect-contract"));
    assert!(!labels.contains(&"promise-like-receiver-proof"));
}

#[test]
fn promise_then_on_expression_receiver_still_reports_receiver_obligation() {
    let labels = missing_evidence_for_call(
        "function thenIt(db, id, f) { return db.get(id).then(f); }\n",
        ".then",
    );

    assert!(labels.contains(&"promise-call-return-receiver-producer-proof"));
    assert!(labels.contains(&"promise-call-return-member-callee-proof"));
    assert!(labels.contains(&"promise-then-promise-like-receiver-proof"));
    assert!(labels.contains(&"promise-then-fulfillment-continuation-contract"));
    assert!(labels.contains(&"promise-then-rejection-continuation-contract"));
    assert!(labels.contains(&"promise-then-callback-demand-effect-contract"));
}

#[test]
fn promise_then_on_local_call_receiver_reports_local_callee_obligation() {
    let labels = missing_evidence_for_call(
        "function thenIt(makePromise, f) { return makePromise().then(f); }\n",
        ".then",
    );

    assert!(labels.contains(&"promise-call-return-receiver-producer-proof"));
    assert!(labels.contains(&"promise-call-return-local-or-parameter-callee-proof"));
    assert!(labels.contains(&"promise-then-promise-like-receiver-proof"));
}

#[test]
fn imported_promise_call_return_targets_require_settled_value_contracts() {
    assert_eq!(
        promise_call_return_receiver_callee_evidence(
            "imported-function-target-present-call-contract-proof"
        ),
        "promise-call-return-imported-function-settled-value-contract"
    );
    assert_eq!(
        promise_call_return_receiver_callee_evidence(
            "imported-member-target-present-call-contract-proof"
        ),
        "promise-call-return-imported-member-settled-value-contract"
    );
}

#[test]
fn promise_then_constructor_receiver_reports_producer_obligation() {
    let labels = missing_evidence_for_call(
        "function thenIt(executor, f) { return new Promise(executor).then(f); }\n",
        ".then",
    );

    assert!(labels.contains(&"promise-constructor-receiver-producer-proof"));
    assert!(!labels.contains(&"promise-call-return-receiver-producer-proof"));
    assert!(labels.contains(&"promise-then-promise-like-receiver-proof"));
}

#[test]
fn promise_then_async_function_receiver_reports_producer_obligation() {
    let labels = missing_evidence_for_call(
        "async function load() { return 1; }\nfunction thenIt(f) { return load().then(f); }\n",
        ".then",
    );

    assert!(labels.contains(&"promise-async-function-return-producer-proof"));
    assert!(!labels.contains(&"promise-call-return-receiver-producer-proof"));
    assert!(labels.contains(&"promise-then-promise-like-receiver-proof"));
}

#[test]
fn promise_reject_missing_evidence_is_rejection_value_channel_specific() {
    let labels = missing_evidence_for_call(
        "function rejectIt(e) { return Promise.reject(e); }\n",
        "Promise.reject",
    );

    assert!(labels.contains(&"promise-reject-rejected-value-channel-contract"));
    assert!(!labels.contains(&"promise-rejection-channel-contract"));
}

#[test]
fn promise_constructor_missing_evidence_splits_executor_obligations() {
    let labels = missing_evidence_for_call(
        "function makeIt(executor) { return new Promise(executor); }\n",
        "Promise",
    );

    assert!(labels.contains(&"promise-executor-timing-contract"));
    assert!(labels.contains(&"promise-executor-resolve-reject-callback-contract"));
    assert!(labels.contains(&"promise-executor-throw-to-rejection-contract"));
    assert!(labels.contains(&"promise-executor-callback-effect-contract"));
}

#[test]
fn promise_aggregate_missing_evidence_splits_first_and_all_settled_shapes() {
    let all = missing_evidence_for_call(
        "function f(xs) { return Promise.all(xs); }\n",
        "Promise.all",
    );
    let race = missing_evidence_for_call(
        "function f(xs) { return Promise.race(xs); }\n",
        "Promise.race",
    );
    let all_settled = missing_evidence_for_call(
        "function f(xs) { return Promise.allSettled(xs); }\n",
        "Promise.allSettled",
    );
    let any = missing_evidence_for_call(
        "function f(xs) { return Promise.any(xs); }\n",
        "Promise.any",
    );

    assert!(all.contains(&"promise-aggregate-all-fulfilled-contract"));
    assert!(all.contains(&"promise-aggregate-ordered-values-contract"));
    assert!(race.contains(&"promise-aggregate-first-settled-contract"));
    assert!(race.contains(&"promise-aggregate-cancellation-liveness-contract"));
    assert!(all_settled.contains(&"promise-aggregate-all-settled-contract"));
    assert!(all_settled.contains(&"promise-aggregate-settled-record-shape-contract"));
    assert!(any.contains(&"promise-aggregate-first-fulfilled-contract"));
    assert!(any.contains(&"promise-aggregate-error-channel-contract"));
    for labels in [&all, &race, &all_settled, &any] {
        assert!(labels.contains(&"promise-aggregate-result-channel-contract"));
    }
}

#[test]
fn scheduler_and_interval_calls_report_timing_and_lifecycle_obligations() {
    let wait = missing_evidence_for_call(
        "function waitIt(scheduler) { return scheduler.wait(1); }\n",
        "scheduler.wait",
    );
    let yield_now = missing_evidence_for_call(
        "function yieldIt(scheduler) { return scheduler.yield(); }\n",
        "scheduler.yield",
    );
    let interval = missing_evidence_for_call(
        "function intervalIt(f) { return setInterval(f, 10); }\n",
        "setInterval",
    );
    let timeout = missing_evidence_for_call(
        "function timeoutIt(f) { return setTimeout(f, 10); }\n",
        "setTimeout",
    );
    let microtask = missing_evidence_for_call(
        "function microtaskIt(f) { return queueMicrotask(f); }\n",
        "queueMicrotask",
    );
    let clear = missing_evidence_for_call(
        "function clearIt(handle) { return clearInterval(handle); }\n",
        "clearInterval",
    );
    let clear_timeout = missing_evidence_for_call(
        "function clearTimeoutIt(handle) { return clearTimeout(handle); }\n",
        "clearTimeout",
    );
    let cancel_frame = missing_evidence_for_call(
        "function cancelFrameIt(handle) { return cancelAnimationFrame(handle); }\n",
        "cancelAnimationFrame",
    );

    assert!(wait.contains(&"scheduler-wait-timing-contract"));
    assert!(wait.contains(&"scheduler-wait-cancellation-liveness-contract"));
    assert!(yield_now.contains(&"scheduler-yield-microtask-order-contract"));
    assert!(interval.contains(&"interval-async-iteration-lifecycle-contract"));
    assert!(interval.contains(&"interval-cancellation-liveness-contract"));
    assert!(timeout.contains(&"timer-scheduling-contract"));
    assert!(microtask.contains(&"timer-scheduling-contract"));
    assert!(clear.contains(&"interval-cancellation-liveness-contract"));
    assert!(clear_timeout.contains(&"timer-cancellation-liveness-contract"));
    assert!(cancel_frame.contains(&"timer-cancellation-liveness-contract"));
}

#[test]
fn abort_signal_calls_report_cancellation_liveness_obligations() {
    let timeout = missing_evidence_for_call(
        "function timeoutIt() { return AbortSignal.timeout(100); }\n",
        "AbortSignal.timeout",
    );
    let any = missing_evidence_for_call(
        "function anyIt(signals) { return AbortSignal.any(signals); }\n",
        "AbortSignal.any",
    );
    let aborted = missing_evidence_for_call(
        "function abortIt(reason) { return AbortSignal.abort(reason); }\n",
        "AbortSignal.abort",
    );
    let controller = missing_evidence_for_call(
        "function controllerIt() { return new AbortController(); }\n",
        "AbortController",
    );

    for labels in [&timeout, &any, &aborted] {
        assert!(labels.contains(&"abort-signal-cancellation-contract"));
        assert!(labels.contains(&"abort-signal-lifecycle-contract"));
    }
    assert!(controller.contains(&"abort-controller-signal-lifecycle-contract"));
    assert!(controller.contains(&"abort-signal-cancellation-contract"));
}

#[test]
fn promise_catch_missing_evidence_splits_continuation_from_callback_effect() {
    let labels =
        missing_evidence_for_call("function catchIt(p, h) { return p.catch(h); }\n", ".catch");

    assert!(labels.contains(&"promise-catch-rejection-continuation-contract"));
    assert!(labels.contains(&"promise-catch-callback-demand-effect-contract"));
    assert!(labels.contains(&"promise-like-receiver-proof"));
    assert!(!labels.contains(&"promise-rejection-continuation-contract"));
}

#[test]
fn promise_finally_missing_evidence_splits_settlement_from_callback_effect() {
    let labels = missing_evidence_for_call(
        "function finallyIt(p, h) { return p.finally(h); }\n",
        ".finally",
    );

    assert!(labels.contains(&"promise-finally-settlement-continuation-contract"));
    assert!(labels.contains(&"promise-finally-callback-demand-effect-contract"));
    assert!(labels.contains(&"promise-like-receiver-proof"));
    assert!(!labels.contains(&"promise-rejection-continuation-contract"));
}
