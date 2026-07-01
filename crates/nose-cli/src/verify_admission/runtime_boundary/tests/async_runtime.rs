use super::{
    call_matches_callee_surface, missing_evidence_for_lang_call,
    runtime_boundary_evidence_for_lang_call,
};
use crate::verify_admission::AdmissionContext;
use nose_il::{FileId, Interner, Lang, NodeId, NodeKind};

use super::super::runtime_boundary_missing_evidence_with_context;

mod imported_bindings;
mod java;
mod java_static_wildcard;
mod python;
mod ruby;
mod rust_block_on;
mod rust_block_on_root_shadow;
mod rust_block_on_self_field;
mod scope_shadowing;
mod swift;

fn runtime_boundary_evidence_for_corpus_call(
    sources: &[(&str, &str, Lang)],
    target_path: &str,
    callee_suffix: &str,
) -> Option<Vec<&'static str>> {
    let interner = Interner::new();
    let files: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(idx, (path, src, lang))| {
            nose_frontend::lower_source(FileId(idx as u32), path, src.as_bytes(), *lang, &interner)
                .unwrap_or_else(|err| panic!("lower {path}: {err}"))
        })
        .collect();
    let corpus = nose_il::Corpus::new(interner, files);
    let context = AdmissionContext::from_corpus(&corpus);
    let il = corpus
        .files
        .iter()
        .find(|il| il.meta.path == target_path)
        .unwrap_or_else(|| panic!("expected target file {target_path}"));
    let call = (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .find(|&node| {
            il.kind(node) == NodeKind::Call
                && call_matches_callee_surface(il, &corpus.interner, node, callee_suffix)
        })
        .unwrap_or_else(|| panic!("expected call ending in {callee_suffix}"));
    runtime_boundary_missing_evidence_with_context(il, &corpus.interner, call, &context)
}

fn python_runtime_with_path_visible_local_asyncio(
    runtime_src: &str,
    callee_suffix: &str,
) -> Option<Vec<&'static str>> {
    runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "asyncio.py",
                "def create_task(x):\n    return x\n",
                Lang::Python,
            ),
            ("runtime.py", runtime_src, Lang::Python),
        ],
        "runtime.py",
        callee_suffix,
    )
}

fn rust_runtime_with_same_file_local_tokio(
    runtime_src: &str,
    callee_suffix: &str,
) -> Option<Vec<&'static str>> {
    let src = format!("mod tokio {{ pub fn spawn<T>(task: T) -> T {{ task }} }}\n{runtime_src}");
    runtime_boundary_evidence_for_corpus_call(
        &[("runtime.rs", &src, Lang::Rust)],
        "runtime.rs",
        callee_suffix,
    )
}

#[test]
fn non_js_async_runtime_calls_report_shared_task_and_aggregate_obligations() {
    let py_task = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main():\n    return asyncio.create_task(work())\n",
        Lang::Python,
        "asyncio.create_task",
    );
    let py_sleep = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main():\n    return await asyncio.sleep(1)\n",
        Lang::Python,
        "asyncio.sleep",
    );
    let py_gather = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main(task):\n    return await asyncio.gather(task)\n",
        Lang::Python,
        "asyncio.gather",
    );
    let py_wait = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main(task):\n    return await asyncio.wait([task])\n",
        Lang::Python,
        "asyncio.wait",
    );
    let rust_spawn = missing_evidence_for_lang_call(
        "runtime.rs",
        "async fn run() { tokio::spawn(async { work().await }); }\n",
        Lang::Rust,
        "tokio::spawn",
    );
    let rust_join = missing_evidence_for_lang_call(
        "runtime.rs",
        "async fn run() { tokio::join!(work(), other()); }\n",
        Lang::Rust,
        "tokio::join",
    );
    let rust_try_join = missing_evidence_for_lang_call(
        "runtime.rs",
        "async fn run() { tokio::try_join!(work(), other()); }\n",
        Lang::Rust,
        "tokio::try_join",
    );
    let rust_select = missing_evidence_for_lang_call(
        "runtime.rs",
        "async fn run() { futures::select!(a = work() => a); }\n",
        Lang::Rust,
        "futures::select",
    );
    let swift_task = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  Task { await work() }\n}\n",
        Lang::Swift,
        "Task",
    );
    let swift_detached = missing_evidence_for_lang_call(
        "runtime.swift",
        "func run() async {\n  Task.detached { await work() }\n}\n",
        Lang::Swift,
        "Task.detached",
    );

    for labels in [&py_task, &rust_spawn, &swift_task, &swift_detached] {
        assert!(labels.contains(&"task-spawn-scheduling-contract"));
        assert!(labels.contains(&"task-handle-lifecycle-contract"));
        assert!(labels.contains(&"task-cancellation-liveness-contract"));
    }
    assert!(py_sleep.contains(&"timer-scheduling-contract"));
    for labels in [&py_gather, &rust_join, &rust_try_join] {
        assert!(labels.contains(&"async-aggregate-all-completion-contract"));
        assert!(labels.contains(&"async-aggregate-result-channel-contract"));
    }
    assert!(py_wait.contains(&"async-aggregate-completion-contract"));
    assert!(py_wait.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(py_wait.contains(&"async-aggregate-result-channel-contract"));
    assert!(rust_select.contains(&"async-aggregate-first-completion-contract"));
    assert!(rust_select.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(rust_select.contains(&"async-aggregate-result-channel-contract"));
}

#[test]
fn non_js_async_runtime_import_aliases_report_shared_obligations() {
    let py_alias_task = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio as aio\nasync def main():\n    return aio.create_task(work())\n",
        Lang::Python,
        "aio.create_task",
    );
    let py_alias_wait = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio as aio\nasync def main(task):\n    return await aio.wait([task])\n",
        Lang::Python,
        "aio.wait",
    );
    let rust_imported_spawn = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::spawn;\nfn run() { spawn(work()); }\n",
        Lang::Rust,
        "spawn",
    );
    let rust_imported_spawn_alias = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::spawn as tokio_spawn;\nfn run() { tokio_spawn(work()); }\n",
        Lang::Rust,
        "tokio_spawn",
    );
    let rust_imported_join = missing_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::join;\nasync fn run() { join!(work(), other()); }\n",
        Lang::Rust,
        "join",
    );
    let rust_imported_select = missing_evidence_for_lang_call(
        "runtime.rs",
        "use futures::select;\nasync fn run() { select!(a = work() => a); }\n",
        Lang::Rust,
        "select",
    );

    for labels in [
        &py_alias_task,
        &rust_imported_spawn,
        &rust_imported_spawn_alias,
    ] {
        assert!(labels.contains(&"task-spawn-scheduling-contract"));
        assert!(labels.contains(&"task-handle-lifecycle-contract"));
        assert!(labels.contains(&"task-cancellation-liveness-contract"));
    }
    assert!(py_alias_wait.contains(&"async-aggregate-completion-contract"));
    assert!(py_alias_wait.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(py_alias_wait.contains(&"async-aggregate-result-channel-contract"));
    assert!(rust_imported_join.contains(&"async-aggregate-all-completion-contract"));
    assert!(rust_imported_join.contains(&"async-aggregate-result-channel-contract"));
    assert!(rust_imported_select.contains(&"async-aggregate-first-completion-contract"));
    assert!(rust_imported_select.contains(&"async-aggregate-cancellation-liveness-contract"));
    assert!(rust_imported_select.contains(&"async-aggregate-result-channel-contract"));
}

#[test]
fn non_js_async_runtime_attribution_requires_runtime_identity() {
    let py_shadowed = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "async def main(asyncio, task):\n    return asyncio.gather(task)\n",
        Lang::Python,
        "asyncio.gather",
    );
    let py_unimported = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "async def main():\n    return asyncio.sleep(1)\n",
        Lang::Python,
        "asyncio.sleep",
    );
    let rust_bare_join = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "macro_rules! join { ($a:expr, $b:expr) => { ($a, $b) }; }\nasync fn run() { join!(work(), other()); }\n",
        Lang::Rust,
        "join",
    );
    let rust_bare_select = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "macro_rules! select { ($($t:tt)*) => { () }; }\nasync fn run() { select!(a = work() => a); }\n",
        Lang::Rust,
        "select",
    );
    let rust_imported_spawn_macro = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::spawn;\nfn run() { spawn!(work()); }\n",
        Lang::Rust,
        "spawn",
    );
    let swift_shadowed_task = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "let Task = makeTask\nfunc run() {\n  Task { work() }\n}\n",
        Lang::Swift,
        "Task",
    );
    let swift_shadowed_detached = runtime_boundary_evidence_for_lang_call(
        "runtime.swift",
        "let Task = makeTask\nfunc run() {\n  Task.detached { work() }\n}\n",
        Lang::Swift,
        "Task.detached",
    );

    assert_missing_evidence_not_contains(
        py_shadowed,
        "async-aggregate-all-completion-contract",
        "shadowed Python asyncio.gather",
    );
    assert_missing_evidence_not_contains(
        py_unimported,
        "timer-scheduling-contract",
        "unimported Python asyncio.sleep",
    );
    let py_shadowed_alias = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "import asyncio as aio\nasync def main(aio, task):\n    return await aio.gather(task)\n",
        Lang::Python,
        "aio.gather",
    );
    assert_missing_evidence_not_contains(
        py_shadowed_alias,
        "async-aggregate-all-completion-contract",
        "shadowed Python asyncio alias",
    );
    assert_missing_evidence_not_contains(
        rust_bare_join,
        "async-aggregate-all-completion-contract",
        "unqualified Rust join! macro",
    );
    assert_missing_evidence_not_contains(
        rust_bare_select,
        "async-aggregate-first-completion-contract",
        "unqualified Rust select! macro",
    );
    assert_missing_evidence_not_contains(
        rust_imported_spawn_macro,
        "task-spawn-scheduling-contract",
        "imported Rust spawn used as macro",
    );
    for (labels, surface) in [
        (swift_shadowed_task, "shadowed Swift Task"),
        (swift_shadowed_detached, "shadowed Swift Task.detached"),
    ] {
        assert_missing_evidence_not_contains(labels, "task-spawn-scheduling-contract", surface);
    }
}

#[test]
fn non_js_async_runtime_imported_runtime_bindings_reject_local_shadows() {
    let rust_imported_spawn_param_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::spawn;\nfn run<F>(spawn: F) { spawn(work()); }\n",
        Lang::Rust,
        "spawn",
    );
    let rust_imported_spawn_let_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::spawn;\nfn run() { let spawn = local; spawn(work()); }\n",
        Lang::Rust,
        "spawn",
    );
    let rust_imported_join_macro_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.rs",
        "use tokio::join;\nmacro_rules! join { ($a:expr, $b:expr) => { ($a, $b) }; }\nasync fn run() { join!(work(), other()); }\n",
        Lang::Rust,
        "join",
    );

    for (labels, surface) in [
        (
            rust_imported_spawn_param_shadow,
            "Rust spawn parameter shadow",
        ),
        (rust_imported_spawn_let_shadow, "Rust spawn let shadow"),
    ] {
        assert_missing_evidence_not_contains(labels, "task-spawn-scheduling-contract", surface);
    }
    assert_missing_evidence_not_contains(
        rust_imported_join_macro_shadow,
        "async-aggregate-all-completion-contract",
        "Rust imported join shadowed by local macro",
    );
}

#[test]
fn non_js_async_runtime_context_rejects_project_local_runtime_names() {
    let py_local_asyncio = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "asyncio.py",
                "def create_task(x):\n    return x\n",
                Lang::Python,
            ),
            (
                "runtime.py",
                "import asyncio\nasync def main():\n    return asyncio.create_task(work())\n",
                Lang::Python,
            ),
        ],
        "runtime.py",
        "asyncio.create_task",
    );
    let rust_local_tokio_spawn = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { pub fn spawn<T>(task: T) -> T { task } }\nfn run() { tokio::spawn(work()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        "tokio::spawn",
    );
    let rust_local_tokio_join = runtime_boundary_evidence_for_corpus_call(
        &[(
            "runtime.rs",
            "mod tokio { macro_rules! join { ($a:expr, $b:expr) => { ($a, $b) }; } }\nasync fn run() { tokio::join!(work(), other()); }\n",
            Lang::Rust,
        )],
        "runtime.rs",
        "tokio::join",
    );
    let swift_project_task = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "Task.swift",
                "struct Task { init(operation: () -> Void) {} }\n",
                Lang::Swift,
            ),
            (
                "run.swift",
                "func run() {\n  Task { work() }\n}\n",
                Lang::Swift,
            ),
        ],
        "run.swift",
        "Task",
    );

    assert_missing_evidence_not_contains(
        py_local_asyncio,
        "task-spawn-scheduling-contract",
        "project-local Python asyncio module",
    );
    assert_missing_evidence_not_contains(
        rust_local_tokio_spawn,
        "task-spawn-scheduling-contract",
        "project-local Rust tokio root for spawn",
    );
    assert_missing_evidence_not_contains(
        rust_local_tokio_join,
        "async-aggregate-all-completion-contract",
        "project-local Rust tokio root for join",
    );
    assert_missing_evidence_not_contains(
        swift_project_task,
        "task-spawn-scheduling-contract",
        "project-visible Swift Task type",
    );
}

#[test]
fn non_js_async_runtime_context_keeps_unrelated_runtime_names_open() {
    let py_unrelated_local_asyncio = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "pkg/asyncio.py",
                "def create_task(x):\n    return x\n",
                Lang::Python,
            ),
            (
                "runtime.py",
                "import asyncio\nasync def main():\n    return asyncio.create_task(work())\n",
                Lang::Python,
            ),
        ],
        "runtime.py",
        "asyncio.create_task",
    );
    let rust_unrelated_local_tokio = runtime_boundary_evidence_for_corpus_call(
        &[
            (
                "decoy.rs",
                "mod tokio { pub fn spawn<T>(task: T) -> T { task } }\n",
                Lang::Rust,
            ),
            (
                "runtime.rs",
                "fn run() { tokio::spawn(work()); }\n",
                Lang::Rust,
            ),
        ],
        "runtime.rs",
        "tokio::spawn",
    );

    assert_missing_evidence_contains(
        py_unrelated_local_asyncio,
        "task-spawn-scheduling-contract",
        "unrelated Python asyncio module",
    );
    assert_missing_evidence_contains(
        rust_unrelated_local_tokio,
        "task-spawn-scheduling-contract",
        "unrelated Rust tokio root",
    );
}

#[test]
fn non_js_async_runtime_context_rejects_project_local_import_aliases() {
    let py_local_asyncio_alias = python_runtime_with_path_visible_local_asyncio(
        "import asyncio as aio\nasync def main():\n    return aio.create_task(work())\n",
        "aio.create_task",
    );
    let rust_local_tokio_imported_spawn = rust_runtime_with_same_file_local_tokio(
        "use tokio::spawn;\nfn run() { spawn(work()); }\n",
        "spawn",
    );

    assert_missing_evidence_not_contains(
        py_local_asyncio_alias,
        "task-spawn-scheduling-contract",
        "project-local Python asyncio module through alias",
    );
    assert_missing_evidence_not_contains(
        rust_local_tokio_imported_spawn,
        "task-spawn-scheduling-contract",
        "project-local Rust tokio root for imported spawn",
    );
}

#[test]
fn non_js_async_runtime_alias_fallback_requires_top_level_import() {
    let py_nested_import_alias = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "async def helper():\n    import asyncio as aio\nasync def main():\n    return await aio.sleep(1)\n",
        Lang::Python,
        "aio.sleep",
    );

    assert_missing_evidence_not_contains(
        py_nested_import_alias,
        "timer-scheduling-contract",
        "Python asyncio alias imported only in another local scope",
    );
}

fn assert_missing_evidence_contains(
    labels: Option<Vec<&'static str>>,
    label: &'static str,
    surface: &str,
) {
    let labels = labels.unwrap_or_else(|| panic!("{surface} should report {label}"));
    assert!(
        labels.contains(&label),
        "{surface} should report {label}: {labels:?}"
    );
}

fn assert_missing_evidence_not_contains(
    labels: Option<Vec<&'static str>>,
    label: &'static str,
    surface: &str,
) {
    if let Some(labels) = labels {
        assert!(
            !labels.contains(&label),
            "{surface} should not report {label}: {labels:?}"
        );
    }
}
