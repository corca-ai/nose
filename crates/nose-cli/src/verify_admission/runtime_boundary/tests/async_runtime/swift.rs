use super::super::{missing_evidence_for_protocol, missing_evidence_for_raw_tag};
use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_corpus_call, runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn swift_structured_concurrency_reports_shared_obligations() {
    let swift_sleep = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await Task.sleep(nanoseconds: 1)\n}\n",
        Lang::Swift,
        "Task.sleep",
    );
    let swift_yield = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await Task.yield()\n}\n",
        Lang::Swift,
        "Task.yield",
    );
    let swift_task_group = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await withTaskGroup(of: Int.self) { group in\n    group.addTask { 1 }\n  }\n}\n",
        Lang::Swift,
        "withTaskGroup",
    );
    let swift_throwing_task_group = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async throws {\n  try await withThrowingTaskGroup(of: Int.self) { group in\n    group.addTask { 1 }\n  }\n}\n",
        Lang::Swift,
        "withThrowingTaskGroup",
    );
    let swift_discarding_task_group = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await withDiscardingTaskGroup { group in\n    group.addTask { work() }\n  }\n}\n",
        Lang::Swift,
        "withDiscardingTaskGroup",
    );
    let swift_throwing_discarding_task_group = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async throws {\n  try await withThrowingDiscardingTaskGroup { group in\n    group.addTask { try work() }\n  }\n}\n",
        Lang::Swift,
        "withThrowingDiscardingTaskGroup",
    );

    assert!(swift_sleep.contains(&"timer-scheduling-contract"));
    assert!(swift_sleep.contains(&"task-cancellation-liveness-contract"));
    assert!(swift_yield.contains(&"task-yield-scheduling-contract"));
    for labels in [&swift_task_group, &swift_throwing_task_group] {
        assert!(labels.contains(&"async-aggregate-all-completion-contract"));
        assert!(labels.contains(&"async-aggregate-cancellation-liveness-contract"));
        assert!(labels.contains(&"async-aggregate-result-channel-contract"));
    }
    assert!(swift_throwing_task_group.contains(&"exception-channel-contract"));
    assert!(swift_discarding_task_group.contains(&"async-aggregate-all-completion-contract"));
    assert!(swift_discarding_task_group.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(!swift_discarding_task_group.contains(&"async-aggregate-result-channel-contract"));
    assert!(
        swift_throwing_discarding_task_group.contains(&"async-aggregate-all-completion-contract")
    );
    assert!(swift_throwing_discarding_task_group
        .contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(swift_throwing_discarding_task_group.contains(&"exception-channel-contract"));
    assert!(
        !swift_throwing_discarding_task_group.contains(&"async-aggregate-result-channel-contract")
    );
}

#[test]
fn swift_structured_concurrency_rejects_local_runtime_shadows() {
    let swift_shadowed_sleep = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "let Task = makeTask\nfunc run() async {\n  await Task.sleep(nanoseconds: 1)\n}\n",
        Lang::Swift,
        "Task.sleep",
    );
    let swift_shadowed_group = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "func withTaskGroup(_ body: () -> Void) { body() }\nfunc run() async {\n  await withTaskGroup { work() }\n}\n",
        Lang::Swift,
        "withTaskGroup",
    );
    let swift_project_task_group = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "TaskGroup.swift",
                "func withTaskGroup(_ body: () -> Void) { body() }\n",
                Lang::Swift,
            ),
            (
                "run.swift",
                "func run() async {\n  await withTaskGroup { work() }\n}\n",
                Lang::Swift,
            ),
        ],
        "run.swift",
        "withTaskGroup",
    );
    let swift_project_task_extension_sleep = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "TaskExtension.swift",
                "extension Task {\n  static func sleep(nanoseconds: Int) {}\n}\n",
                Lang::Swift,
            ),
            (
                "run.swift",
                "func run() async {\n  await Task.sleep(nanoseconds: 1)\n}\n",
                Lang::Swift,
            ),
        ],
        "run.swift",
        "Task.sleep",
    );
    let swift_task_extension_sleep = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "extension Task {\n  static func sleep(nanoseconds: Int) {}\n}\nfunc run() async {\n  await Task.sleep(nanoseconds: 1)\n}\n",
        Lang::Swift,
        "Task.sleep",
    );

    assert_missing_evidence_not_contains(
        swift_shadowed_sleep,
        "timer-scheduling-contract",
        "shadowed Swift Task.sleep",
    );
    assert_missing_evidence_not_contains(
        swift_shadowed_group,
        "async-aggregate-all-completion-contract",
        "shadowed Swift withTaskGroup",
    );
    assert_missing_evidence_not_contains(
        swift_project_task_group,
        "async-aggregate-all-completion-contract",
        "project-visible Swift withTaskGroup function",
    );
    assert_missing_evidence_not_contains(
        swift_project_task_extension_sleep,
        "timer-scheduling-contract",
        "project-visible Swift Task.sleep extension member",
    );
    assert_missing_evidence_not_contains(
        swift_task_extension_sleep,
        "timer-scheduling-contract",
        "Swift Task.sleep extension member",
    );
}

#[test]
fn swift_continuation_bridges_report_future_and_callback_obligations() {
    let checked = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await withCheckedContinuation { continuation in\n    continuation.resume(returning: 1)\n  }\n}\n",
        Lang::Swift,
        "withCheckedContinuation",
    );
    let checked_throwing = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async throws {\n  try await withCheckedThrowingContinuation { continuation in\n    continuation.resume(throwing: error)\n  }\n}\n",
        Lang::Swift,
        "withCheckedThrowingContinuation",
    );
    let unsafe_continuation = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  await withUnsafeContinuation { continuation in\n    continuation.resume(returning: 1)\n  }\n}\n",
        Lang::Swift,
        "withUnsafeContinuation",
    );
    let unsafe_throwing = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async throws {\n  try await withUnsafeThrowingContinuation { continuation in\n    continuation.resume(throwing: error)\n  }\n}\n",
        Lang::Swift,
        "withUnsafeThrowingContinuation",
    );

    for labels in [
        &checked,
        &checked_throwing,
        &unsafe_continuation,
        &unsafe_throwing,
    ] {
        assert!(labels.contains(&"future-settled-value-channel-contract"));
        assert!(labels.contains(&"future-settlement-continuation-contract"));
        assert!(labels.contains(&"future-callback-demand-effect-contract"));
    }
    assert!(!checked.contains(&"exception-channel-contract"));
    assert!(!unsafe_continuation.contains(&"exception-channel-contract"));
    assert!(checked_throwing.contains(&"exception-channel-contract"));
    assert!(unsafe_throwing.contains(&"exception-channel-contract"));
}

#[test]
fn swift_continuation_bridges_reject_local_runtime_shadows() {
    let local_function_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "func withCheckedContinuation(_ body: () -> Void) { body() }\nfunc run() async {\n  await withCheckedContinuation { work() }\n}\n",
        Lang::Swift,
        "withCheckedContinuation",
    );
    let local_value_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "let withCheckedThrowingContinuation = localBridge\nfunc run() async throws {\n  try await withCheckedThrowingContinuation { work() }\n}\n",
        Lang::Swift,
        "withCheckedThrowingContinuation",
    );
    let project_visible_shadow = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "ContinuationShim.swift",
                "func withUnsafeContinuation(_ body: () -> Void) { body() }\n",
                Lang::Swift,
            ),
            (
                "run.swift",
                "func run() async {\n  await withUnsafeContinuation { work() }\n}\n",
                Lang::Swift,
            ),
        ],
        "run.swift",
        "withUnsafeContinuation",
    );

    for (labels, surface) in [
        (
            local_function_shadow,
            "Swift local withCheckedContinuation function",
        ),
        (
            local_value_shadow,
            "Swift local withCheckedThrowingContinuation value",
        ),
        (
            project_visible_shadow,
            "Swift project-visible withUnsafeContinuation function",
        ),
    ] {
        assert_missing_evidence_not_contains(
            labels.clone(),
            "future-settled-value-channel-contract",
            surface,
        );
        assert_missing_evidence_not_contains(
            labels,
            "future-callback-demand-effect-contract",
            surface,
        );
    }
}

#[test]
fn swift_async_let_reports_task_spawn_protocol_obligations() {
    let labels = missing_evidence_for_protocol(
        "async-let.swift",
        "func run() async throws -> Int {\n  async let value: Int = try await work()\n  return try await value\n}\n",
        Lang::Swift,
        nose_il::SourceProtocolKind::TaskSpawn,
    );

    assert!(labels.contains(&"task-spawn-scheduling-contract"));
    assert!(labels.contains(&"task-handle-lifecycle-contract"));
    assert!(labels.contains(&"task-cancellation-liveness-contract"));
    assert!(labels.contains(&"async-await-scheduling-contract"));
    assert!(labels.contains(&"exception-channel-contract"));
}

#[test]
fn swift_async_iteration_reports_shared_protocol_obligations() {
    let labels = missing_evidence_for_protocol(
        "stream.swift",
        "func read(_ stream: AsyncStream<Int>) async {\n  for await value in stream {\n    print(value)\n  }\n}\n",
        Lang::Swift,
        nose_il::SourceProtocolKind::AsyncIteration,
    );

    assert!(labels.contains(&"async-iteration-lifecycle-contract"));
    assert!(labels.contains(&"async-iteration-value-channel-contract"));
    assert!(labels.contains(&"async-await-scheduling-contract"));
}

#[test]
fn swift_throwing_async_iteration_preserves_exception_boundary() {
    let iteration = missing_evidence_for_protocol(
        "stream.swift",
        "func read(_ stream: AsyncThrowingStream<Int, Error>) async throws {\n  for try await value in stream {\n    print(value)\n  }\n}\n",
        Lang::Swift,
        nose_il::SourceProtocolKind::AsyncIteration,
    );
    let throwing = missing_evidence_for_raw_tag(
        "stream.swift",
        "func read(_ stream: AsyncThrowingStream<Int, Error>) async throws {\n  for try await value in stream {\n    print(value)\n  }\n}\n",
        Lang::Swift,
        "try",
    );

    assert!(iteration.contains(&"async-iteration-lifecycle-contract"));
    assert!(iteration.contains(&"async-iteration-value-channel-contract"));
    assert!(iteration.contains(&"async-await-scheduling-contract"));
    assert!(throwing.contains(&"exception-channel-contract"));
    assert!(throwing.contains(&"async-iteration-lifecycle-contract"));
}
