use super::{
    assert_missing_evidence_contains, assert_missing_evidence_not_contains,
    runtime_boundary_evidence_for_corpus_call, runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn java_completable_future_static_attribution_requires_type_identity() {
    let unimported = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "class Runtime { Object run() { return CompletableFuture.supplyAsync(() -> work()); } }\n",
        Lang::Java,
        "CompletableFuture.supplyAsync",
    );
    let local_type = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "class CompletableFuture { static Object supplyAsync(Object work) { return work; } }\nclass Runtime { Object run() { return CompletableFuture.supplyAsync(work()); } }\n",
        Lang::Java,
        "CompletableFuture.supplyAsync",
    );
    let local_variable = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.CompletableFuture;\nclass Runtime { Object run(Object CompletableFuture) { return CompletableFuture.supplyAsync(work()); } }\n",
        Lang::Java,
        "CompletableFuture.supplyAsync",
    );
    let conflicting_import = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nimport example.CompletableFuture;\nclass Runtime { Object run() { return CompletableFuture.supplyAsync(work()); } }\n",
        Lang::Java,
        "CompletableFuture.supplyAsync",
    );

    for (labels, surface) in [
        (unimported, "unimported Java CompletableFuture"),
        (local_type, "local Java CompletableFuture type"),
        (local_variable, "local Java CompletableFuture variable"),
        (
            conflicting_import,
            "conflicting Java CompletableFuture import",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "task-spawn-scheduling-contract", surface);
    }
}

#[test]
fn java_completable_future_wildcard_import_remains_open_without_conflicts() {
    let wildcard = runtime_boundary_evidence_for_lang_call(
        "Runtime.java",
        "import java.util.concurrent.*;\nclass Runtime { Object run() { return CompletableFuture.runAsync(() -> work()); } }\n",
        Lang::Java,
        "CompletableFuture.runAsync",
    );

    assert_missing_evidence_contains(
        wildcard,
        "task-spawn-scheduling-contract",
        "Java CompletableFuture wildcard import",
    );
}

#[test]
fn java_completable_future_wildcard_import_is_not_blocked_by_other_file_imports() {
    let wildcard = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "Other.java",
                "import java.util.concurrent.CompletableFuture;\nclass Other {}\n",
                Lang::Java,
            ),
            (
                "Runtime.java",
                "import java.util.concurrent.*;\nclass Runtime { Object run() { return CompletableFuture.runAsync(() -> work()); } }\n",
                Lang::Java,
            ),
        ],
        "Runtime.java",
        "CompletableFuture.runAsync",
    );

    assert_missing_evidence_contains(
        wildcard,
        "task-spawn-scheduling-contract",
        "Java CompletableFuture wildcard import with unrelated file import",
    );
}

#[test]
fn java_completable_future_wildcard_import_is_not_blocked_by_other_file_conflicts() {
    let wildcard = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "Other.java",
                "import example.CompletableFuture;\nclass Other {}\n",
                Lang::Java,
            ),
            (
                "Runtime.java",
                "import java.util.concurrent.*;\nclass Runtime { Object run() { return CompletableFuture.runAsync(() -> work()); } }\n",
                Lang::Java,
            ),
        ],
        "Runtime.java",
        "CompletableFuture.runAsync",
    );

    assert_missing_evidence_contains(
        wildcard,
        "task-spawn-scheduling-contract",
        "Java CompletableFuture wildcard import with unrelated file conflict",
    );
}
