use super::{
    assert_missing_evidence_not_contains, missing_evidence_for_lang_call,
    python_runtime_with_path_visible_local_asyncio, runtime_boundary_evidence_for_lang_call,
};
use nose_il::Lang;

#[test]
fn python_asyncio_event_loop_helpers_report_shared_obligations() {
    let run = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\ndef main():\n    return asyncio.run(work())\n",
        Lang::Python,
        "asyncio.run",
    );
    let wait_for = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main(task):\n    return await asyncio.wait_for(task, timeout=1)\n",
        Lang::Python,
        "asyncio.wait_for",
    );
    let shield = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main(task):\n    return await asyncio.shield(task)\n",
        Lang::Python,
        "asyncio.shield",
    );
    let thread_safe = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\ndef main(loop):\n    return asyncio.run_coroutine_threadsafe(work(), loop)\n",
        Lang::Python,
        "asyncio.run_coroutine_threadsafe",
    );
    let to_thread = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio\nasync def main():\n    return await asyncio.to_thread(work)\n",
        Lang::Python,
        "asyncio.to_thread",
    );

    assert!(run.contains(&"future-drive-scheduling-contract"));
    assert!(run.contains(&"future-settled-value-channel-contract"));
    assert!(run.contains(&"exception-channel-contract"));
    assert!(wait_for.contains(&"timer-scheduling-contract"));
    assert!(wait_for.contains(&"timer-cancellation-liveness-contract"));
    assert!(wait_for.contains(&"future-settled-value-channel-contract"));
    assert!(wait_for.contains(&"exception-channel-contract"));
    assert!(shield.contains(&"task-cancellation-liveness-contract"));
    assert!(shield.contains(&"future-settled-value-channel-contract"));
    for labels in [&thread_safe, &to_thread] {
        assert!(labels.contains(&"task-spawn-scheduling-contract"));
        assert!(labels.contains(&"task-handle-lifecycle-contract"));
        assert!(labels.contains(&"task-cancellation-liveness-contract"));
        assert!(labels.contains(&"future-settled-value-channel-contract"));
        assert!(labels.contains(&"exception-channel-contract"));
    }
    assert!(to_thread.contains(&"future-callback-demand-effect-contract"));
}

#[test]
fn python_asyncio_event_loop_helpers_accept_alias_and_imported_binding_proof() {
    let alias_run = missing_evidence_for_lang_call(
        "runtime.py",
        "import asyncio as aio\ndef main():\n    return aio.run(work())\n",
        Lang::Python,
        "aio.run",
    );
    let imported_wait_for = missing_evidence_for_lang_call(
        "runtime.py",
        "from asyncio import wait_for as bounded\nasync def main(task):\n    return await bounded(task, timeout=1)\n",
        Lang::Python,
        "bounded",
    );
    let imported_to_thread = missing_evidence_for_lang_call(
        "runtime.py",
        "from asyncio import to_thread\nasync def main():\n    return await to_thread(work)\n",
        Lang::Python,
        "to_thread",
    );

    assert!(alias_run.contains(&"future-drive-scheduling-contract"));
    assert!(alias_run.contains(&"future-settled-value-channel-contract"));
    assert!(imported_wait_for.contains(&"timer-scheduling-contract"));
    assert!(imported_wait_for.contains(&"timer-cancellation-liveness-contract"));
    assert!(imported_wait_for.contains(&"future-settled-value-channel-contract"));
    assert!(imported_to_thread.contains(&"task-spawn-scheduling-contract"));
    assert!(imported_to_thread.contains(&"future-callback-demand-effect-contract"));
}

#[test]
fn python_asyncio_event_loop_helpers_require_runtime_identity() {
    let unimported_run = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "def main():\n    return asyncio.run(work())\n",
        Lang::Python,
        "asyncio.run",
    );
    let alias_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "import asyncio as aio\ndef main(aio):\n    return aio.run(work())\n",
        Lang::Python,
        "aio.run",
    );
    let imported_wait_for_shadow = runtime_boundary_evidence_for_lang_call(
        "runtime.py",
        "from asyncio import wait_for as bounded\nasync def main(bounded, task):\n    return await bounded(task, timeout=1)\n",
        Lang::Python,
        "bounded",
    );
    let project_local_run = python_runtime_with_path_visible_local_asyncio(
        "import asyncio\ndef main():\n    return asyncio.run(work())\n",
        "asyncio.run",
    );
    let project_local_imported_to_thread = python_runtime_with_path_visible_local_asyncio(
        "from asyncio import to_thread\nasync def main():\n    return await to_thread(work)\n",
        "to_thread",
    );

    assert_missing_evidence_not_contains(
        unimported_run,
        "future-drive-scheduling-contract",
        "unimported Python asyncio.run",
    );
    assert_missing_evidence_not_contains(
        alias_shadow,
        "future-drive-scheduling-contract",
        "Python asyncio alias shadowed before run",
    );
    assert_missing_evidence_not_contains(
        imported_wait_for_shadow,
        "timer-scheduling-contract",
        "Python asyncio wait_for imported binding shadowed by parameter",
    );
    assert_missing_evidence_not_contains(
        project_local_run,
        "future-drive-scheduling-contract",
        "project-local Python asyncio module through run",
    );
    assert_missing_evidence_not_contains(
        project_local_imported_to_thread,
        "task-spawn-scheduling-contract",
        "project-local Python asyncio module through to_thread binding",
    );
}
