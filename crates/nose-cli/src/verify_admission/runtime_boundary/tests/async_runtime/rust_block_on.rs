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
    let runtime_new_try_chain = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() -> Result<(), E> { Runtime::new()?.block_on(work()); Ok(()) }\n",
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
        runtime_new_try_chain,
        imported_tokio_test_block_on,
    ] {
        assert!(labels.contains(&"future-drive-scheduling-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
    }
}

#[test]
fn reports_future_drive_obligations_when_local_runtime_binding_is_proven() {
    let local_handle_current = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle;\nfn run() { let handle = Handle::current(); handle.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_runtime_new = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { let rt = Runtime::new().unwrap(); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let parent_block_runtime_new = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { let rt = Runtime::new().unwrap(); { rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_builder_chain = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Builder;\nfn run() { let rt = Builder::new_current_thread().enable_all().build().unwrap(); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_runtime_new_try = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() -> Result<(), E> { let rt = Runtime::new()?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_builder_try = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Builder;\nfn run() -> Result<(), E> { let rt = Builder::new_current_thread().enable_all().build()?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_try_current = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle;\nfn run() { let handle = Handle::try_current().expect(\"runtime\"); handle.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_runtime_new_map_err_try = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() -> Result<(), E> { let rt = Runtime::new().map_err(convert)?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_builder_map_err_try = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Builder;\nfn run() -> Result<(), E> { let rt = Builder::new_current_thread().enable_all().build().map_err(convert)?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );

    for labels in [
        local_handle_current,
        local_runtime_new,
        parent_block_runtime_new,
        local_builder_chain,
        local_runtime_new_try,
        local_builder_try,
        local_try_current,
        local_runtime_new_map_err_try,
        local_builder_map_err_try,
    ] {
        assert!(labels.contains(&"future-drive-scheduling-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
    }
}

#[test]
fn reports_future_drive_obligations_when_parameter_runtime_identity_is_proven() {
    let imported_runtime_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run(rt: Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let imported_runtime_ref_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run(rt: &Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let nested_brace_imported_runtime_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::{runtime::{Builder, Runtime}};\nfn run(rt: Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let aliased_handle_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Handle as TokioHandle;\nfn run(handle: TokioHandle) { handle.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let qualified_runtime_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "fn run(rt: tokio::runtime::Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let scoped_imported_runtime_param = missing_evidence_for_lang_call(
        "runtime.rs",
        "mod local { use tokio::runtime::Runtime; fn run(rt: Runtime) { rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );

    for labels in [
        imported_runtime_param,
        imported_runtime_ref_param,
        nested_brace_imported_runtime_param,
        aliased_handle_param,
        qualified_runtime_param,
        scoped_imported_runtime_param,
    ] {
        assert!(labels.contains(&"future-drive-scheduling-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
    }
}

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

    for labels in [
        imported_runtime_self_field,
        nested_brace_runtime_self_field,
        qualified_handle_self_field,
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
fn requires_proven_parameter_runtime_identity() {
    let local_runtime_type = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "struct Runtime;\nfn run(rt: Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let wrong_runtime_import = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use project::runtime::Runtime;\nfn run(rt: Runtime) { rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let project_local_tokio_runtime = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { pub mod runtime { pub struct Runtime; } }\nfn run(rt: tokio::runtime::Runtime) { rt.block_on(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        ".block_on",
    );
    let case_mismatched_tokio_runtime = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod Tokio { pub mod runtime { pub struct Runtime; } }\nfn run(rt: Tokio::runtime::Runtime) { rt.block_on(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        ".block_on",
    );
    let parent_module_import_not_visible = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nmod local { fn run(rt: Runtime) { rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let parent_nested_brace_import_not_visible = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::{runtime::{Runtime}};\nmod local { fn run(rt: Runtime) { rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let parameter_reassigned_to_local = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run(rt: Runtime) { let rt = make_local(rt); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );

    for (labels, surface) in [
        (local_runtime_type, "Rust local Runtime parameter type"),
        (
            wrong_runtime_import,
            "Rust non-tokio Runtime parameter import",
        ),
        (
            project_local_tokio_runtime,
            "project-local tokio Runtime parameter type",
        ),
        (
            case_mismatched_tokio_runtime,
            "case-mismatched Tokio Runtime parameter type",
        ),
        (
            parent_module_import_not_visible,
            "parent-module import for child-module Runtime parameter type",
        ),
        (
            parent_nested_brace_import_not_visible,
            "parent-module nested brace import for child-module Runtime parameter type",
        ),
        (
            parameter_reassigned_to_local,
            "Rust Runtime parameter reassigned before block_on",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
    }
}

#[test]
fn requires_proven_local_runtime_binding_identity() {
    let local_binding_wrapped_runtime = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "fn run() { let rt = make_wrapper(tokio::runtime::Runtime::new().unwrap()); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_binding_reassigned_to_local = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { let rt = Runtime::new().unwrap(); let rt = make_local(rt); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_binding_shadowed_in_inner_block = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { let rt = Runtime::new().unwrap(); { let rt = make_local(rt); rt.block_on(work()); } }\n",
        Lang::Rust,
        ".block_on",
    );
    let field_assignment_with_same_name_as_receiver = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run(rt: Local, s: S) { s.rt = Runtime::new().unwrap(); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_binding_only_visible_in_nested_block = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() { { let rt = Runtime::new().unwrap(); } rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_binding_shadowed_runtime_type = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run(Runtime: LocalRuntime) { let rt = Runtime::new().unwrap(); rt.block_on(work()); }\n",
        Lang::Rust,
        ".block_on",
    );
    let local_binding_project_local_tokio = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { pub mod runtime { pub struct Runtime; impl Runtime { pub fn new() -> Result<Runtime, ()> { Ok(Runtime) } } } }\nfn run() { let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        ".block_on",
    );
    let wrapped_runtime_result_map_err_try = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\nfn run() -> Result<(), E> { let rt = make_wrapper(Runtime::new()).map_err(convert)?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );
    let runtime_value_map_err_try = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::runtime::Runtime;\ntrait IntoLocal { fn map_err(self, convert: fn(E) -> E) -> Result<LocalRuntime, E>; }\nimpl IntoLocal for Runtime { fn map_err(self, _convert: fn(E) -> E) -> Result<LocalRuntime, E> { todo!() } }\nfn run() -> Result<(), E> { let rt = Runtime::new().unwrap().map_err(convert)?; rt.block_on(work()); Ok(()) }\n",
        Lang::Rust,
        ".block_on",
    );

    for (labels, surface) in [
        (
            local_binding_wrapped_runtime,
            "wrapped Rust runtime local binding receiver",
        ),
        (
            local_binding_reassigned_to_local,
            "Rust runtime local binding reassigned before block_on",
        ),
        (
            local_binding_shadowed_in_inner_block,
            "Rust runtime local binding shadowed inside receiver block",
        ),
        (
            field_assignment_with_same_name_as_receiver,
            "Rust field assignment does not prove same-name local receiver",
        ),
        (
            local_binding_only_visible_in_nested_block,
            "Rust runtime local binding not visible at block_on",
        ),
        (
            local_binding_shadowed_runtime_type,
            "Rust local binding with shadowed Runtime import",
        ),
        (
            local_binding_project_local_tokio,
            "project-local Rust tokio root for local runtime binding",
        ),
        (
            wrapped_runtime_result_map_err_try,
            "wrapped Rust runtime Result through map_err callback",
        ),
        (
            runtime_value_map_err_try,
            "Rust runtime value changed by extension map_err callback",
        ),
    ] {
        assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
    }
}
