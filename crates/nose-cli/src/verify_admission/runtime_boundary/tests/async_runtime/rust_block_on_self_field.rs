use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_corpus_call, runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn reports_future_drive_obligations_when_self_field_runtime_identity_is_proven() {
    let imported_runtime_self_field = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let nested_brace_runtime_self_field = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::{runtime::{Builder, Runtime}};\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let qualified_handle_self_field = missing_evidence_for_lang_call(
        "runtime.rs",
        "struct Runner { handle: tokio::runtime::Handle }\nimpl Runner { fn run(&self) { self.handle.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_runtime_self_field = missing_evidence_for_lang_call(
        "runtime.rs",
        "fn outer() { use tokio::runtime::Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );

    for labels in [
        imported_runtime_self_field,
        nested_brace_runtime_self_field,
        qualified_handle_self_field,
        local_runtime_self_field,
    ] {
        assert!(labels.contains(&"future-drive-scheduling-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
    }
}

#[test]
fn requires_proven_self_field_runtime_identity() {
    let non_self_receiver = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nfn run(runner: Runner) { runner.rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let wrong_runtime_import = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use project::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_runtime_type = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "struct Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let parent_import_not_visible = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nmod local { struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );
    let project_local_tokio_field = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { pub mod runtime { pub struct Runtime; } }\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        ".block_on",
    );
    let type_alias_field = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\ntype Rt = Runtime;\nstruct Runner { rt: Rt }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );

    for (labels, surface) in [
        (non_self_receiver, "Rust non-self field receiver"),
        (
            wrong_runtime_import,
            "Rust self field with non-tokio Runtime import",
        ),
        (
            local_runtime_type,
            "Rust self field with local Runtime type",
        ),
        (
            parent_import_not_visible,
            "Rust self field with parent-module Runtime import",
        ),
        (
            project_local_tokio_field,
            "project-local tokio Runtime self field type",
        ),
        (
            type_alias_field,
            "Rust self field through Runtime type alias",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
    }
}

#[test]
fn requires_proven_local_self_field_runtime_identity() {
    let local_scope_wrong_import = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn outer() { use project::runtime::Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_scope_runtime_type = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn outer() { use tokio::runtime::Runtime; struct Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_scope_namespace_alias = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn outer() { mod project { pub mod runtime { pub struct Runtime; } } use project as tokio; struct Runner { rt: tokio::runtime::Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );
    let duplicate_local_struct = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn outer() { use tokio::runtime::Runtime; struct Runner { rt: Runtime } struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        ".block_on",
    );

    for (labels, surface) in [
        (
            local_scope_wrong_import,
            "Rust local self field with non-tokio Runtime import",
        ),
        (
            local_scope_runtime_type,
            "Rust local self field with local Runtime type",
        ),
        (
            local_scope_namespace_alias,
            "Rust local self field with namespace alias named tokio",
        ),
        (
            duplicate_local_struct,
            "Rust local self field with duplicate struct declarations",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
    }
}
