use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn java_completable_future_receiver_methods_report_settlement_and_timeout_obligations() {
    let complete = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { boolean run(CompletableFuture<String> future, String value) { return future.complete(value); } }\n",
        Lang::Java,
        "future.complete",
    );
    let complete_exceptionally = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { boolean run(CompletableFuture<String> future, Throwable error) { return future.completeExceptionally(error); } }\n",
        Lang::Java,
        "future.completeExceptionally",
    );
    let join = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.join(); } }\n",
        Lang::Java,
        "future.join",
    );
    let get_now = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.getNow(\"fallback\"); } }\n",
        Lang::Java,
        "future.getNow",
    );
    let status = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { boolean run(CompletableFuture<String> future) { return future.isCompletedExceptionally(); } }\n",
        Lang::Java,
        "future.isCompletedExceptionally",
    );
    let or_timeout = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.orTimeout(1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "future.orTimeout",
    );
    let complete_on_timeout = missing_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.completeOnTimeout(\"fallback\", 1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "future.completeOnTimeout",
    );

    assert!(complete.contains(&"future-settled-value-channel-contract"));
    assert!(complete.contains(&"task-handle-lifecycle-contract"));
    assert!(complete_exceptionally.contains(&"future-settled-value-channel-contract"));
    assert!(complete_exceptionally.contains(&"exception-channel-contract"));
    assert!(join.contains(&"future-settled-value-channel-contract"));
    assert!(join.contains(&"exception-channel-contract"));
    assert!(join.contains(&"task-cancellation-liveness-contract"));
    assert!(get_now.contains(&"future-settled-value-channel-contract"));
    assert!(get_now.contains(&"exception-channel-contract"));
    assert!(status.contains(&"future-settled-value-channel-contract"));
    assert!(status.contains(&"exception-channel-contract"));
    assert!(status.contains(&"task-handle-lifecycle-contract"));
    assert!(status.contains(&"task-cancellation-liveness-contract"));
    assert!(or_timeout.contains(&"timer-scheduling-contract"));
    assert!(or_timeout.contains(&"future-settled-value-channel-contract"));
    assert!(or_timeout.contains(&"exception-channel-contract"));
    assert!(complete_on_timeout.contains(&"timer-scheduling-contract"));
    assert!(complete_on_timeout.contains(&"future-settled-value-channel-contract"));
}

#[test]
fn java_completable_future_receiver_methods_require_completable_future_import_identity() {
    let custom_completable = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import example.CompletableFuture;\nclass Runtime { Object run(CompletableFuture<String> future) { return future.join(); } }\n",
        Lang::Java,
        "future.join",
    );
    let completion_stage_timeout = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletionStage;\nimport java.util.concurrent.TimeUnit;\nclass Runtime { Object run(CompletionStage<String> stage) { return stage.orTimeout(1, TimeUnit.SECONDS); } }\n",
        Lang::Java,
        "stage.orTimeout",
    );
    let local_completable = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { static class CompletableFuture<T> { Object join() { return null; } }\nObject run(CompletableFuture<String> future) { return future.join(); } }\n",
        Lang::Java,
        "future.join",
    );

    for (labels, surface) in [
        (custom_completable, "custom CompletableFuture receiver"),
        (
            completion_stage_timeout,
            "CompletionStage-only timeout receiver",
        ),
        (local_completable, "local CompletableFuture receiver"),
    ] {
        assert_missing_evidence_not_contains(
            labels,
            "future-settled-value-channel-contract",
            surface,
        );
    }
}
