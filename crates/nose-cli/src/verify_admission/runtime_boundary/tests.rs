use super::*;
use nose_il::{FileId, Lang};

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

fn missing_evidence_for_raw_tag(path: &str, src: &str, lang: Lang, tag: &str) -> Vec<&'static str> {
    let (il, interner) = lowered_source(path, src, lang);
    let node = (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .find(|&node| match il.node(node).payload {
            Payload::Name(symbol) => interner.resolve(symbol) == tag,
            _ => false,
        })
        .unwrap_or_else(|| panic!("expected raw tag {tag} in {path}"));
    runtime_boundary_missing_evidence(&il, &interner, node)
        .unwrap_or_else(|| panic!("expected runtime boundary evidence for {tag} in {path}"))
}

fn missing_evidence_for_call(src: &str, callee_suffix: &str) -> Vec<&'static str> {
    missing_evidence_for_lang_call("promise.js", src, Lang::JavaScript, callee_suffix)
}

fn missing_evidence_for_lang_call(
    path: &str,
    src: &str,
    lang: Lang,
    callee_suffix: &str,
) -> Vec<&'static str> {
    runtime_boundary_evidence_for_lang_call(path, src, lang, callee_suffix)
        .unwrap_or_else(|| panic!("expected runtime boundary evidence for {callee_suffix}"))
}

fn runtime_boundary_evidence_for_lang_call(
    path: &str,
    src: &str,
    lang: Lang,
    callee_suffix: &str,
) -> Option<Vec<&'static str>> {
    let (il, interner) = lowered_source(path, src, lang);
    let call = (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .find(|&node| {
            il.kind(node) == NodeKind::Call
                && call_matches_callee_surface(&il, &interner, node, callee_suffix)
        })
        .unwrap_or_else(|| panic!("expected call ending in {callee_suffix}"));
    runtime_boundary_missing_evidence(&il, &interner, call)
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

mod async_runtime;

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
fn go_channel_protocol_boundaries_report_specific_obligations() {
    let send = missing_evidence_for_raw_tag(
        "channel.go",
        "package p\nfunc send(ch chan int, x int) { ch <- x }\n",
        Lang::Go,
        "channel_send",
    );
    assert!(send.contains(&"channel-send-synchronization-contract"));
    assert!(send.contains(&"channel-send-receive-protocol-contract"));
    assert!(send.contains(&"channel-protocol-contract"));

    let receive = missing_evidence_for_raw_tag(
        "channel.go",
        "package p\nfunc recv(ch chan int) int { return <-ch }\n",
        Lang::Go,
        "channel_receive",
    );
    assert!(receive.contains(&"channel-receive-value-channel-contract"));
    assert!(!receive.contains(&"channel-receive-status-contract"));
    assert!(receive.contains(&"channel-send-receive-protocol-contract"));

    let status = missing_evidence_for_raw_tag(
        "channel.go",
        "package p\nfunc recv(ch chan int) bool { _, ok := <-ch; return ok }\n",
        Lang::Go,
        "channel_receive_status",
    );
    assert!(status.contains(&"channel-receive-status-contract"));
    assert!(status.contains(&"channel-receive-value-channel-contract"));
    assert!(status.contains(&"channel-send-receive-protocol-contract"));
}

#[test]
fn go_select_defer_and_goroutine_boundaries_report_specific_obligations() {
    let select = missing_evidence_for_raw_tag(
        "select.go",
        "package p\nfunc f(ch chan int) { select { case <-ch: return; default: return } }\n",
        Lang::Go,
        "select",
    );
    assert!(select.contains(&"channel-select-readiness-contract"));
    assert!(select.contains(&"channel-select-protocol-contract"));

    let select_case = missing_evidence_for_raw_tag(
        "select.go",
        "package p\nfunc f(ch chan int) { select { case <-ch: return; default: return } }\n",
        Lang::Go,
        "select_case",
    );
    assert!(select_case.contains(&"channel-select-case-selection-contract"));
    assert!(select_case.contains(&"channel-select-protocol-contract"));

    let select_default = missing_evidence_for_raw_tag(
        "select.go",
        "package p\nfunc f(ch chan int) { select { case <-ch: return; default: return } }\n",
        Lang::Go,
        "select_default",
    );
    assert!(select_default.contains(&"channel-select-default-liveness-contract"));
    assert!(select_default.contains(&"channel-select-protocol-contract"));

    let select_status = missing_evidence_for_raw_tag(
        "select_status.go",
        "package p\nfunc f(ch chan int) bool { select { case _, ok := <-ch: return ok; default: return false } }\n",
        Lang::Go,
        "select",
    );
    assert!(select_status.contains(&"channel-select-readiness-contract"));
    assert!(select_status.contains(&"channel-select-case-selection-contract"));
    assert!(select_status.contains(&"channel-receive-status-contract"));
    assert!(select_status.contains(&"channel-receive-value-channel-contract"));
    assert!(select_status.contains(&"channel-select-protocol-contract"));

    let deferred = missing_evidence_for_protocol(
        "defer.go",
        "package p\nfunc f(x int) { defer record(x) }\n",
        Lang::Go,
        nose_il::SourceProtocolKind::Defer,
    );
    assert!(deferred.contains(&"defer-lifecycle-ordering-contract"));
    assert!(deferred.contains(&"defer-callback-effect-contract"));

    let goroutine = missing_evidence_for_protocol(
        "goroutine.go",
        "package p\nfunc f(x int) { go record(x) }\n",
        Lang::Go,
        nose_il::SourceProtocolKind::GoRoutine,
    );
    assert!(goroutine.contains(&"goroutine-scheduling-contract"));
    assert!(goroutine.contains(&"goroutine-callback-effect-contract"));
}

#[test]
fn async_function_protocol_missing_evidence_is_language_neutral() {
    for (path, src, lang) in [
        (
            "async-function.js",
            "async function read(x) { return x; }\n",
            Lang::JavaScript,
        ),
        (
            "async-function.ts",
            "async function read(x: number): Promise<number> { return x; }\n",
            Lang::TypeScript,
        ),
        (
            "async-function.py",
            "async def read(x):\n    return x\n",
            Lang::Python,
        ),
        (
            "async-function.rs",
            "pub async fn read(x: i32) -> i32 { x }\n",
            Lang::Rust,
        ),
        (
            "async-function.swift",
            "func read(_ x: Int) async -> Int {\n  return x\n}\n",
            Lang::Swift,
        ),
    ] {
        let labels = missing_evidence_for_protocol(
            path,
            src,
            lang,
            nose_il::SourceProtocolKind::AsyncFunction,
        );
        assert!(
            labels.contains(&"async-function-scheduling-contract"),
            "{path} should report the shared async-function scheduling contract: {labels:?}"
        );
        assert!(
            !labels.contains(&"promise-async-function-scheduling-contract"),
            "{path} should not report async function scheduling as Promise-specific evidence: {labels:?}"
        );
    }
}

#[test]
fn async_block_protocol_missing_evidence_is_language_neutral() {
    let labels = missing_evidence_for_protocol(
        "async-block.rs",
        "pub async fn read(x: i32) -> i32 { async move { x }.await }\n",
        Lang::Rust,
        nose_il::SourceProtocolKind::AsyncBlock,
    );

    assert!(
        labels.contains(&"async-block-scheduling-contract"),
        "async block should report the shared async-block scheduling contract: {labels:?}"
    );
    assert!(
        !labels.contains(&"future-async-block-scheduling-contract"),
        "async block should not report scheduling as Future-specific evidence: {labels:?}"
    );
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
