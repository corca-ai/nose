use super::*;

#[test]
fn rust_tokio_runtime_self_field_domains_are_dependency_backed() {
    let interner = Interner::new();
    let runtime_domain = DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("tokio::runtime::Runtime"),
    };
    let handle_domain = DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("tokio::runtime::Handle"),
    };

    let imported_runtime_field = lower_fixture(
        "tokio_runtime_self_field.rs",
        b"use tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    let runtime_import_ids = imported_binding_symbol_ids(
        &imported_runtime_field.evidence,
        "tokio::runtime",
        "Runtime",
    );
    assert_eq!(runtime_import_ids.len(), 1);
    let runtime_fields = field_domain_records(&imported_runtime_field.evidence, runtime_domain);
    assert_eq!(runtime_fields.len(), 1);
    assert_eq!(
        runtime_fields[0].dependencies, runtime_import_ids,
        "Rust self field runtime domain evidence should be backed by the field type import"
    );

    let qualified_handle_field = lower_fixture(
        "tokio_handle_self_field.rs",
        b"struct Runner { handle: tokio::runtime::Handle }\nimpl Runner { fn run(&self) { self.handle.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    let handle_fields = field_domain_records(&qualified_handle_field.evidence, handle_domain);
    assert_eq!(handle_fields.len(), 1);
    assert!(
        handle_fields[0].dependencies.is_empty(),
        "fully qualified tokio Handle field evidence does not need import dependencies"
    );

    let local_runtime_field = lower_fixture(
        "tokio_local_runtime_self_field.rs",
        b"fn outer() { use tokio::runtime::Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    let local_runtime_import_ids =
        imported_binding_symbol_ids(&local_runtime_field.evidence, "tokio::runtime", "Runtime");
    assert_eq!(local_runtime_import_ids.len(), 1);
    let local_runtime_fields = field_domain_records(&local_runtime_field.evidence, runtime_domain);
    assert_eq!(local_runtime_fields.len(), 1);
    assert_eq!(
        local_runtime_fields[0].dependencies, local_runtime_import_ids,
        "local Rust self field runtime domain evidence should use the local declaration-scope import"
    );

    let local_runtime_field_with_module_import = lower_fixture(
        "tokio_local_runtime_self_field_module_import.rs",
        b"use tokio::runtime::Runtime;\nfn outer() { struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    let module_runtime_import_ids = imported_binding_symbol_ids(
        &local_runtime_field_with_module_import.evidence,
        "tokio::runtime",
        "Runtime",
    );
    assert_eq!(module_runtime_import_ids.len(), 1);
    let module_import_fields = field_domain_records(
        &local_runtime_field_with_module_import.evidence,
        runtime_domain,
    );
    assert_eq!(module_import_fields.len(), 1);
    assert_eq!(
        module_import_fields[0].dependencies, module_runtime_import_ids,
        "module-scope Rust runtime imports should remain visible to local function/block struct field types"
    );
}

#[test]
fn rust_tokio_runtime_self_field_domains_require_exact_receiver_and_type_proof() {
    let interner = Interner::new();
    let runtime_domain = DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("tokio::runtime::Runtime"),
    };

    let non_self_receiver = lower_fixture(
        "tokio_runtime_non_self_field.rs",
        b"use tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nfn run(runner: Runner) { runner.rt.block_on(work()); }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&non_self_receiver.evidence, runtime_domain).len(),
        0,
        "non-self field receivers must not receive Rust runtime field domain evidence"
    );

    let wrong_runtime_import = lower_fixture(
        "tokio_runtime_wrong_import_self_field.rs",
        b"use project::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&wrong_runtime_import.evidence, runtime_domain).len(),
        0,
        "same-named field imports from another module must not prove tokio runtime field identity"
    );

    let local_runtime_type = lower_fixture(
        "tokio_runtime_local_type_self_field.rs",
        b"struct Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&local_runtime_type.evidence, runtime_domain).len(),
        0,
        "a local Runtime type must close unqualified Rust runtime field evidence"
    );

    let raw_local_runtime_type = lower_fixture(
        "tokio_runtime_raw_local_type_self_field.rs",
        b"struct r#Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&raw_local_runtime_type.evidence, runtime_domain).len(),
        0,
        "a raw local Runtime type must close unqualified Rust runtime field evidence"
    );

    let local_scope_runtime_type = lower_fixture(
        "tokio_runtime_local_scope_type_self_field.rs",
        b"fn outer() { use tokio::runtime::Runtime; struct Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&local_scope_runtime_type.evidence, runtime_domain).len(),
        0,
        "a local-scope Runtime type must close unqualified local Rust runtime field evidence"
    );

    let local_scope_wrong_runtime_import = lower_fixture(
        "tokio_runtime_local_scope_wrong_import_self_field.rs",
        b"fn outer() { use project::runtime::Runtime; struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&local_scope_wrong_runtime_import.evidence, runtime_domain).len(),
        0,
        "a local-scope same-name Runtime import from another module must not prove tokio runtime field identity"
    );

    let local_scope_namespace_alias_shadow = lower_fixture(
        "tokio_runtime_local_scope_namespace_alias_shadow_self_field.rs",
        b"fn outer() { mod project { pub mod runtime { pub struct Runtime; } } use project as tokio; struct Runner { rt: tokio::runtime::Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&local_scope_namespace_alias_shadow.evidence, runtime_domain).len(),
        0,
        "a local-scope namespace alias named tokio must close qualified tokio runtime field evidence"
    );

    let local_scope_duplicate_struct = lower_fixture(
        "tokio_runtime_local_scope_duplicate_struct_self_field.rs",
        b"fn outer() { use tokio::runtime::Runtime; struct Runner { rt: Runtime } struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&local_scope_duplicate_struct.evidence, runtime_domain).len(),
        0,
        "duplicate local struct definitions must keep Rust self field runtime evidence closed"
    );

    let parent_import_not_visible = lower_fixture(
        "tokio_runtime_parent_import_self_field.rs",
        b"use tokio::runtime::Runtime;\nmod local { struct Runner { rt: Runtime } impl Runner { fn run(&self) { self.rt.block_on(work()); } } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&parent_import_not_visible.evidence, runtime_domain).len(),
        0,
        "parent-module imports must not prove child-module Rust runtime field evidence"
    );

    let namespace_alias_shadow = lower_fixture(
        "tokio_runtime_namespace_alias_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project as tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_alias_shadow.evidence, runtime_domain).len(),
        0,
        "a namespace alias named tokio must close qualified tokio runtime field evidence"
    );

    let namespace_alias_tab_shadow = lower_fixture(
        "tokio_runtime_namespace_alias_tab_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project\tas tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_alias_tab_shadow.evidence, runtime_domain).len(),
        0,
        "a namespace alias named tokio with non-space whitespace must close qualified tokio runtime field evidence"
    );

    let extern_crate_alias_shadow = lower_fixture(
        "tokio_runtime_extern_crate_alias_shadow_self_field.rs",
        b"extern crate project as tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&extern_crate_alias_shadow.evidence, runtime_domain).len(),
        0,
        "an extern crate alias named tokio must close qualified tokio runtime field evidence"
    );

    let extern_crate_alias_spaced_shadow = lower_fixture(
        "tokio_runtime_extern_crate_alias_spaced_shadow_self_field.rs",
        b"extern   crate project\tas tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&extern_crate_alias_spaced_shadow.evidence, runtime_domain).len(),
        0,
        "an extern crate alias named tokio with flexible whitespace must close qualified tokio runtime field evidence"
    );

    let namespace_alias_import_shadow = lower_fixture(
        "tokio_runtime_namespace_alias_import_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project as tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_alias_import_shadow.evidence, runtime_domain).len(),
        0,
        "a Runtime import through a namespace alias named tokio must close runtime field evidence"
    );

    let namespace_raw_alias_shadow = lower_fixture(
        "tokio_runtime_namespace_raw_alias_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project as r#tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_raw_alias_shadow.evidence, runtime_domain).len(),
        0,
        "a raw namespace alias named tokio must close qualified tokio runtime field evidence"
    );

    let namespace_raw_alias_import_shadow = lower_fixture(
        "tokio_runtime_namespace_raw_alias_import_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project as r#tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_raw_alias_import_shadow.evidence, runtime_domain).len(),
        0,
        "a Runtime import through a raw namespace alias named tokio must close runtime field evidence"
    );

    let namespace_brace_raw_alias_shadow = lower_fixture(
        "tokio_runtime_namespace_brace_raw_alias_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project::{self as r#tokio};\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&namespace_brace_raw_alias_shadow.evidence, runtime_domain).len(),
        0,
        "a brace-form raw namespace alias named tokio must close qualified tokio runtime field evidence"
    );

    let namespace_brace_raw_alias_import_shadow = lower_fixture(
        "tokio_runtime_namespace_brace_raw_alias_import_shadow_self_field.rs",
        b"mod project { pub mod runtime { pub struct Runtime; } }\nuse project::{self as r#tokio};\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(
            &namespace_brace_raw_alias_import_shadow.evidence,
            runtime_domain
        )
        .len(),
        0,
        "a Runtime import through a brace-form raw namespace alias named tokio must close runtime field evidence"
    );

    let raw_local_tokio_root = lower_fixture(
        "tokio_runtime_raw_local_tokio_root_self_field.rs",
        b"mod r#tokio { pub mod runtime { pub struct Runtime; } }\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&raw_local_tokio_root.evidence, runtime_domain).len(),
        0,
        "a raw local tokio module must close qualified tokio runtime field evidence"
    );

    let raw_local_tokio_root_import = lower_fixture(
        "tokio_runtime_raw_local_tokio_root_import_self_field.rs",
        b"mod r#tokio { pub mod runtime { pub struct Runtime; } }\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&raw_local_tokio_root_import.evidence, runtime_domain).len(),
        0,
        "a Runtime import through a raw local tokio module must close runtime field evidence"
    );

    let extern_crate_alias_import_shadow = lower_fixture(
        "tokio_runtime_extern_crate_alias_import_shadow_self_field.rs",
        b"extern crate project as tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        field_domain_records(&extern_crate_alias_import_shadow.evidence, runtime_domain).len(),
        0,
        "a Runtime import through an extern crate alias named tokio must close runtime field evidence"
    );
}

fn field_domain_records(
    evidence: &[EvidenceRecord],
    domain: DomainEvidence,
) -> Vec<&EvidenceRecord> {
    evidence
        .iter()
        .filter(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Node {
                    kind: NodeKind::Field,
                    ..
                }
            ) && matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain)
        })
        .collect()
}
