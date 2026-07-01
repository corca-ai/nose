use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn java_completable_future_static_calls_report_shared_future_obligations() {
    let supply_async = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run() { return CompletableFuture.supplyAsync(() -> work()); } }\n",
        Lang::Java,
        "CompletableFuture.supplyAsync",
    );
    let run_async = missing_evidence_for_lang_call(
        "Runtime.java",
        "class Runtime { Object run() { return java.util.concurrent.CompletableFuture.runAsync(() -> work()); } }\n",
        Lang::Java,
        "java.util.concurrent.CompletableFuture.runAsync",
    );
    let wildcard_completed = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { Object run() { return CompletableFuture.completedFuture(value()); } }\n",
        Lang::Java,
        "CompletableFuture.completedFuture",
    );
    let failed = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(Throwable error) { return CompletableFuture.failedFuture(error); } }\n",
        Lang::Java,
        "CompletableFuture.failedFuture",
    );
    let all_of = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<?> a, CompletableFuture<?> b) { return CompletableFuture.allOf(a, b); } }\n",
        Lang::Java,
        "CompletableFuture.allOf",
    );
    let any_of = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<?> a, CompletableFuture<?> b) { return CompletableFuture.anyOf(a, b); } }\n",
        Lang::Java,
        "CompletableFuture.anyOf",
    );

    for labels in [&supply_async, &run_async] {
        assert!(labels.contains(&"task-spawn-scheduling-contract"));
        assert!(labels.contains(&"task-handle-lifecycle-contract"));
        assert!(labels.contains(&"task-cancellation-liveness-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
        assert!(labels.contains(&"future-callback-demand-effect-contract"));
    }
    assert!(wildcard_completed.contains(&"future-settled-value-channel-contract"));
    assert!(failed.contains(&"future-settled-value-channel-contract"));
    assert!(failed.contains(&"exception-channel-contract"));
    assert!(all_of.contains(&"async-aggregate-all-completion-contract"));
    assert!(all_of.contains(&"async-aggregate-result-channel-contract"));
    assert!(any_of.contains(&"async-aggregate-first-completion-contract"));
    assert!(any_of.contains(&"async-aggregate-result-channel-contract"));
}

#[test]
fn java_completion_stage_receiver_methods_require_future_like_receiver_proof() {
    let then_apply = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.thenApply(value -> value.trim()); } }\n",
        Lang::Java,
        "future.thenApply",
    );
    let exceptionally = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.exceptionally(error -> fallback()); } }\n",
        Lang::Java,
        "future.exceptionally",
    );
    let completion_stage = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletionStage;\nclass Runtime { Object run(CompletionStage<String> stage) { return stage.handle((value, error) -> value); } }\n",
        Lang::Java,
        "stage.handle",
    );
    let wildcard_then = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.thenApply(value -> value); } }\n",
        Lang::Java,
        "future.thenApply",
    );
    let either = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> a, CompletableFuture<String> b) { return a.applyToEither(b, value -> value); } }\n",
        Lang::Java,
        "a.applyToEither",
    );
    let untyped = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "class Runtime { Object run(Object future) { return future.thenApply(value -> value); } }\n",
        Lang::Java,
        "future.thenApply",
    );

    assert!(then_apply.contains(&"future-fulfillment-continuation-contract"));
    assert!(then_apply.contains(&"future-settled-value-channel-contract"));
    assert!(then_apply.contains(&"future-callback-demand-effect-contract"));
    assert!(exceptionally.contains(&"future-exception-continuation-contract"));
    assert!(exceptionally.contains(&"exception-channel-contract"));
    assert!(completion_stage.contains(&"future-settlement-continuation-contract"));
    assert!(wildcard_then.contains(&"future-fulfillment-continuation-contract"));
    assert!(either.contains(&"async-aggregate-first-completion-contract"));
    assert_missing_evidence_not_contains(
        untyped,
        "future-fulfillment-continuation-contract",
        "untyped Java thenApply receiver",
    );
}

#[test]
fn java_completion_stage_receiver_methods_require_import_backed_type_domain() {
    let local_completable_future = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "class CompletableFuture<T> { Object thenApply(Object callback) { return this; } }\nclass Runtime { Object run(CompletableFuture<String> future) { return future.thenApply(value -> value); } }\n",
        Lang::Java,
        "future.thenApply",
    );
    let custom_completion_stage_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import example.CompletionStage;\nclass Runtime { Object run(CompletionStage<String> stage) { return stage.handle((value, error) -> value); } }\n",
        Lang::Java,
        "stage.handle",
    );
    let imported_shadowed_member = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { static class CompletableFuture<T> { Object thenApply(Object callback) { return this; } }\nObject run(CompletableFuture<String> future) { return future.thenApply(value -> value); } }\n",
        Lang::Java,
        "future.thenApply",
    );

    for (labels, surface) in [
        (
            local_completable_future,
            "local Java CompletableFuture receiver",
        ),
        (
            custom_completion_stage_import,
            "custom Java CompletionStage receiver import",
        ),
        (
            imported_shadowed_member,
            "imported Java CompletableFuture hidden by member type",
        ),
    ] {
        for label in [
            "future-fulfillment-continuation-contract",
            "future-settlement-continuation-contract",
        ] {
            assert_missing_evidence_not_contains(labels.clone(), label, surface);
        }
    }
}

#[test]
fn java_future_handle_methods_report_future_lifecycle_obligations() {
    let get = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let cancel = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ScheduledFuture;\nclass Runtime { boolean run(ScheduledFuture<?> future) { return future.cancel(true); } }\n",
        Lang::Java,
        "future.cancel",
    );
    let is_done = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { boolean run(CompletableFuture<String> future) { return future.isDone(); } }\n",
        Lang::Java,
        "future.isDone",
    );

    assert!(get.contains(&"future-settled-value-channel-contract"));
    assert!(get.contains(&"exception-channel-contract"));
    assert!(get.contains(&"task-handle-lifecycle-contract"));
    assert!(get.contains(&"task-cancellation-liveness-contract"));
    assert!(cancel.contains(&"task-cancellation-liveness-contract"));
    assert!(cancel.contains(&"task-handle-lifecycle-contract"));
    assert!(is_done.contains(&"task-handle-lifecycle-contract"));
}

#[test]
fn java_executor_methods_report_scheduler_handle_and_callback_obligations() {
    let execute = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Executor;\nclass Runtime { Object run(Executor executor) { executor.execute(() -> work()); return null; } }\n",
        Lang::Java,
        "executor.execute",
    );
    let submit = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nclass Runtime { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let invoke_all = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.List;\nimport java.util.concurrent.Callable;\nimport java.util.concurrent.ExecutorService;\nclass Runtime { Object run(ExecutorService executor, List<Callable<String>> calls) throws Exception { return executor.invokeAll(calls); } }\n",
        Lang::Java,
        "executor.invokeAll",
    );
    let invoke_any = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.List;\nimport java.util.concurrent.Callable;\nimport java.util.concurrent.ExecutorService;\nclass Runtime { Object run(ExecutorService executor, List<Callable<String>> calls) throws Exception { return executor.invokeAny(calls); } }\n",
        Lang::Java,
        "executor.invokeAny",
    );

    assert!(execute.contains(&"task-spawn-scheduling-contract"));
    assert!(execute.contains(&"future-callback-demand-effect-contract"));
    assert!(execute.contains(&"exception-channel-contract"));
    assert!(!execute.contains(&"task-handle-lifecycle-contract"));

    for labels in [&submit, &invoke_all, &invoke_any] {
        assert!(labels.contains(&"future-settled-value-channel-contract"));
        assert!(labels.contains(&"future-callback-demand-effect-contract"));
        assert!(labels.contains(&"exception-channel-contract"));
    }
    assert!(submit.contains(&"task-handle-lifecycle-contract"));
    assert!(submit.contains(&"task-cancellation-liveness-contract"));
    assert!(invoke_all.contains(&"async-aggregate-all-completion-contract"));
    assert!(invoke_all.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(invoke_any.contains(&"async-aggregate-first-completion-contract"));
    assert!(invoke_any.contains(&"async-aggregate-cancellation-liveness-contract"));
}

#[test]
fn java_scheduled_executor_methods_report_timer_and_interval_obligations() {
    let schedule = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ScheduledExecutorService;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { Object run(ScheduledExecutorService executor) { return executor.schedule(() -> work(), 1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "executor.schedule",
    );
    let fixed_rate = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ScheduledExecutorService;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { Object run(ScheduledExecutorService executor) { return executor.scheduleAtFixedRate(() -> work(), 1, 1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "executor.scheduleAtFixedRate",
    );

    assert!(schedule.contains(&"timer-scheduling-contract"));
    assert!(schedule.contains(&"task-spawn-scheduling-contract"));
    assert!(schedule.contains(&"future-settled-value-channel-contract"));
    assert!(schedule.contains(&"future-callback-demand-effect-contract"));
    assert!(fixed_rate.contains(&"timer-scheduling-contract"));
    assert!(fixed_rate.contains(&"interval-async-iteration-lifecycle-contract"));
    assert!(fixed_rate.contains(&"interval-cancellation-liveness-contract"));
    assert!(fixed_rate.contains(&"task-cancellation-liveness-contract"));
    assert!(fixed_rate.contains(&"future-callback-demand-effect-contract"));
}

#[test]
fn java_local_and_this_field_executor_future_receivers_report_obligations() {
    let local_get = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { Object run() throws Exception { Future<String> future = make(); return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let local_then = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run() { CompletableFuture<String> future = make(); return future.thenApply(value -> value.trim()); } }\n",
        Lang::Java,
        "future.thenApply",
    );
    let local_submit = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nclass Runtime { Object run() { ExecutorService executor = make(); return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let wildcard_local_get = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { Object run() throws Exception { Future<String> future = make(); return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let wildcard_local_submit = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { Object run() { ExecutorService executor = make(); return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let field_get = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { private Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );
    let field_submit = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nclass Runtime { private ExecutorService executor; Object run() { return this.executor.submit(() -> work()); } }\n",
        Lang::Java,
        "this.executor.submit",
    );
    let field_schedule = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ScheduledExecutorService;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { private ScheduledExecutorService executor; Object run() { return this.executor.schedule(() -> work(), 1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "this.executor.schedule",
    );
    let wildcard_field_get = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { private Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );

    assert!(local_get.contains(&"future-settled-value-channel-contract"));
    assert!(local_get.contains(&"exception-channel-contract"));
    assert!(local_then.contains(&"future-fulfillment-continuation-contract"));
    assert!(local_submit.contains(&"task-spawn-scheduling-contract"));
    assert!(local_submit.contains(&"task-handle-lifecycle-contract"));
    assert!(wildcard_local_get.contains(&"future-settled-value-channel-contract"));
    assert!(wildcard_local_submit.contains(&"task-spawn-scheduling-contract"));
    assert!(wildcard_local_submit.contains(&"task-handle-lifecycle-contract"));
    assert!(field_get.contains(&"future-settled-value-channel-contract"));
    assert!(field_submit.contains(&"future-settled-value-channel-contract"));
    assert!(field_submit.contains(&"future-callback-demand-effect-contract"));
    assert!(field_schedule.contains(&"timer-scheduling-contract"));
    assert!(wildcard_field_get.contains(&"future-settled-value-channel-contract"));
}

#[test]
fn java_local_and_this_field_receivers_require_exact_type_identity() {
    let reassigned_local = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { Object run() throws Exception { Future<String> future = make(); future = other(); return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let implicit_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { private Future<String> future; Object run() throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let non_this_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { private Future<String> future; Object run(Runtime other) throws Exception { return other.future.get(); } }\n",
        Lang::Java,
        "other.future.get",
    );
    let shadowed_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { static class Future<T> { Object get() { return null; } }\nprivate Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );
    let wildcard_shadowed_local = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { static class Future<T> { Object get() { return null; } }\nObject run() throws Exception { Future<String> future = make(); return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let wildcard_shadowed_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { static class Future<T> { Object get() { return null; } }\nprivate Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );
    let conflicting_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nimport example.Future;\nclass Runtime { private Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );
    let duplicate_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { private Future<String> future; private Future<String> future; Object run() throws Exception { return this.future.get(); } }\n",
        Lang::Java,
        "this.future.get",
    );
    let conflicting_executor_field = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nimport example.ExecutorService;\nclass Runtime { private ExecutorService executor; Object run() { return this.executor.submit(() -> work()); } }\n",
        Lang::Java,
        "this.executor.submit",
    );

    let assert_not = assert_missing_evidence_not_contains;
    for (labels, surface) in [
        (reassigned_local, "reassigned Java Future local"),
        (implicit_field, "implicit Java Future field"),
        (non_this_field, "non-this Java Future field"),
        (shadowed_field, "member-shadowed Java Future field"),
        (
            wildcard_shadowed_local,
            "wildcard member-shadowed Java Future local",
        ),
        (
            wildcard_shadowed_field,
            "wildcard member-shadowed Java Future field",
        ),
        (conflicting_field, "conflicting Java Future field import"),
        (duplicate_field, "duplicate Java Future field"),
    ] {
        assert_not(labels, "future-settled-value-channel-contract", surface);
    }

    assert_not(
        conflicting_executor_field,
        "task-spawn-scheduling-contract",
        "conflicting Executor field import",
    );
}

#[test]
fn java_executor_future_receivers_reject_wrong_or_insufficient_import_domains() {
    let custom_future = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import example.Future;\nclass Runtime { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let plain_executor_submit = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Executor;\nclass Runtime { Object run(Executor executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );

    for (labels, label, surface) in [
        (
            custom_future,
            "future-settled-value-channel-contract",
            "custom Java Future receiver import",
        ),
        (
            plain_executor_submit,
            "task-handle-lifecycle-contract",
            "plain Java Executor submit receiver",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, label, surface);
    }
}

#[test]
fn java_executor_future_receivers_reject_local_type_shadows() {
    let imported_future_shadowed_by_member_type = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nclass Runtime { static class Future<T> { Object get() { return null; } }\nObject run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let wildcard_future_shadowed_by_member_type = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { static class Future<T> { Object get() { return null; } }\nObject run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let imported_executor_shadowed_by_member_type = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nclass Runtime { static class ExecutorService { Object submit(Object work) { return null; } }\nObject run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let wildcard_executor_shadowed_by_member_type = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { static class ExecutorService { Object submit(Object work) { return null; } }\nObject run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );

    for (labels, label, surface) in [
        (
            imported_future_shadowed_by_member_type,
            "future-settled-value-channel-contract",
            "imported Java Future hidden by member type",
        ),
        (
            wildcard_future_shadowed_by_member_type,
            "future-settled-value-channel-contract",
            "wildcard Java Future hidden by member type",
        ),
        (
            imported_executor_shadowed_by_member_type,
            "task-spawn-scheduling-contract",
            "imported Java ExecutorService hidden by member type",
        ),
        (
            wildcard_executor_shadowed_by_member_type,
            "task-spawn-scheduling-contract",
            "wildcard Java ExecutorService hidden by member type",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, label, surface);
    }
}

#[test]
fn java_executor_future_receivers_reject_conflicting_imports_in_both_orders() {
    let conflicting_future_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.Future;\nimport example.Future;\nclass Runtime { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let wildcard_then_conflicting_future_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nimport example.Future;\nclass Runtime { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let conflicting_then_wildcard_future_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import example.Future;\nimport java.util.concurrent.*;\nclass Runtime { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        Lang::Java,
        "future.get",
    );
    let conflicting_executor_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.ExecutorService;\nimport example.ExecutorService;\nclass Runtime { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let wildcard_then_conflicting_executor_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nimport example.ExecutorService;\nclass Runtime { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );
    let conflicting_then_wildcard_executor_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import example.ExecutorService;\nimport java.util.concurrent.*;\nclass Runtime { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        Lang::Java,
        "executor.submit",
    );

    for (labels, label, surface) in [
        (
            conflicting_future_import,
            "future-settled-value-channel-contract",
            "conflicting Java Future import",
        ),
        (
            wildcard_then_conflicting_future_import,
            "future-settled-value-channel-contract",
            "wildcard then conflicting Java Future import",
        ),
        (
            conflicting_then_wildcard_future_import,
            "future-settled-value-channel-contract",
            "conflicting then wildcard Java Future import",
        ),
        (
            conflicting_executor_import,
            "task-spawn-scheduling-contract",
            "conflicting Java ExecutorService import",
        ),
        (
            wildcard_then_conflicting_executor_import,
            "task-spawn-scheduling-contract",
            "wildcard then conflicting Java ExecutorService import",
        ),
        (
            conflicting_then_wildcard_executor_import,
            "task-spawn-scheduling-contract",
            "conflicting then wildcard Java ExecutorService import",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, label, surface);
    }
}
