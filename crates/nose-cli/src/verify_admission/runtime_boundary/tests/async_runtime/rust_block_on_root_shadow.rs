use super::{assert_missing_evidence_not_contains, runtime_boundary_evidence_for_lang_call};
use nose_il::Lang;

#[test]
fn requires_proven_self_field_runtime_root_for_qualified_field_types() {
    for (src, surface) in [
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project as tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through namespace alias named tokio",
        ),
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project\tas tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through namespace alias named tokio with non-space whitespace",
        ),
        (
            "extern crate project as tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through extern crate alias named tokio",
        ),
        (
            "extern   crate project\tas tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through extern crate alias named tokio with flexible whitespace",
        ),
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project as r#tokio;\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through raw namespace alias named tokio",
        ),
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project::{self as r#tokio};\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through brace-form raw namespace alias named tokio",
        ),
        (
            "mod r#tokio { pub mod runtime { pub struct Runtime; } }\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through raw local tokio module",
        ),
    ] {
        assert_root_shadow_stays_closed(src, surface);
    }
}

#[test]
fn requires_proven_self_field_runtime_root_for_imported_runtime_types() {
    for (src, surface) in [
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project as tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through Runtime import from namespace alias named tokio",
        ),
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project as r#tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through Runtime import from raw namespace alias named tokio",
        ),
        (
            "mod project { pub mod runtime { pub struct Runtime; } }\nuse project::{self as r#tokio};\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through Runtime import from brace-form raw namespace alias named tokio",
        ),
        (
            "mod r#tokio { pub mod runtime { pub struct Runtime; } }\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through Runtime import from raw local tokio module",
        ),
        (
            "extern crate project as tokio;\nuse tokio::runtime::Runtime;\nstruct Runner { rt: Runtime }\nimpl Runner { fn run(&self) { self.rt.block_on(work()); } }\n",
            "Rust self field through Runtime import from extern crate alias named tokio",
        ),
    ] {
        assert_root_shadow_stays_closed(src, surface);
    }
}

fn assert_root_shadow_stays_closed(src: &str, surface: &str) {
    let labels =
        runtime_boundary_evidence_for_lang_call("runtime.rs", src, Lang::Rust, ".block_on");
    assert_missing_evidence_not_contains(labels, "future-drive-scheduling-contract", surface);
}
