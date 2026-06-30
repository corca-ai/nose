use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_corpus_call, runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn reports_future_drive_obligations_when_runtime_identity_is_proven() {
    let imported_handle_current = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle;\nfn run() { Handle::current().block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let qualified_handle_current = missing_evidence_for_lang_call(
        "runtime.rs",
        "fn run() { tokio::runtime::Handle::current().block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let imported_runtime_new = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { Runtime::new().unwrap().block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let imported_builder_chain = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Builder;\nfn run() { Builder::new_current_thread().enable_all().build().unwrap().block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let imported_tokio_test_block_on = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio_test::block_on;\nfn run() { block_on(work()); }\n",
        Lang::Rust,
        "block_on",
    );

    for labels in [
        imported_handle_current,
        qualified_handle_current,
        imported_runtime_new,
        imported_builder_chain,
        imported_tokio_test_block_on,
    ] {
        assert!(labels.contains(&"future-drive-scheduling-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
    }
}

#[test]
fn requires_proven_runtime_identity() {
    let unproven_receiver = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn run(handle: H) { handle.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_tokio = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { pub mod runtime { pub struct Handle; impl Handle { pub fn current() -> Handle { Handle } } } }\nfn run() { tokio::runtime::Handle::current().block_on(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        ".block_on",
    );
    let shadowed_handle = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle;\nfn run(Handle: LocalHandle) { Handle::current().block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let shadowed_tokio_test_block_on = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio_test::block_on;\nfn block_on<T>(future: T) -> T { future }\nfn run() { block_on(work()); }\n",
        Lang::Rust,
        "block_on",
    );
    let local_tokio_test = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio_test { pub fn block_on<T>(future: T) -> T { future } }\nfn run() { tokio_test::block_on(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        "tokio_test::block_on",
    );
    let wrapped_runtime_constructor = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn run() { make_wrapper(tokio::runtime::Runtime::new()).block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let wrapped_handle_current = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle;\nfn run() { make_local(Handle::current()).block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let extension_method_changes_receiver_type = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\ntrait IntoLocal { fn expect(self, message: &str) -> Local; }\nimpl IntoLocal for Runtime { fn expect(self, _message: &str) -> Local { Local } }\nfn run() { Runtime::new().unwrap().expect(\"local wrapper\").block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );

    for (labels, surface) in [
        (unproven_receiver, "unproven Rust block_on receiver"),
        (local_tokio, "project-local Rust tokio root for block_on"),
        (shadowed_handle, "shadowed Rust Handle import for block_on"),
        (
            shadowed_tokio_test_block_on,
            "shadowed Rust tokio_test::block_on import",
        ),
        (local_tokio_test, "project-local Rust tokio_test root"),
        (
            wrapped_runtime_constructor,
            "wrapped Rust runtime constructor receiver",
        ),
        (
            wrapped_handle_current,
            "wrapped Rust Handle::current receiver",
        ),
        (
            extension_method_changes_receiver_type,
            "Rust extension method changes block_on receiver type",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
    }
}
