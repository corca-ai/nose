#!/usr/bin/env python3
"""Build the #602 scheduling, aggregate, cancellation, and lifecycle audit.

This is a lexical source-prevalence report, not semantic proof. It prices the
remaining #602 boundary surfaces across the pinned 120-repo corpus and can attach
the current local recall-loss rollups when a report is provided.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_MANIFEST = "bench/goldens/corpus.json"
DEFAULT_REPOS_ROOT = "bench/repos"
DEFAULT_OUTPUT = "target/scheduling-lifecycle-boundary-audit-602.v1.json"
DEFAULT_GENERATED_ON = "2026-06-29"

SKIP_DIRS = {
    ".bundle",
    ".git",
    ".gradle",
    ".next",
    ".nuxt",
    ".pytest_cache",
    ".svelte-kit",
    ".tox",
    ".venv",
    "__pycache__",
    "build",
    "coverage",
    "DerivedData",
    "dist",
    "node_modules",
    "out",
    "Pods",
    "target",
    "vendor",
    "venv",
}

LANG_EXTS = {
    "c": {".c", ".h", ".cc", ".cpp", ".hpp"},
    "go": {".go"},
    "java": {".java"},
    "javascript-typescript": {
        ".cjs",
        ".cts",
        ".js",
        ".jsx",
        ".mjs",
        ".mts",
        ".svelte",
        ".ts",
        ".tsx",
        ".vue",
    },
    "python": {".py"},
    "ruby": {".rb"},
    "rust": {".rs"},
    "swift": {".swift"},
}


@dataclass(frozen=True)
class Pattern:
    language: str
    surface: str
    operation: str
    obligation_family: str
    obligation_subreason: str
    boundary: str
    note: str
    regex: re.Pattern[str]
    status: str = "closed-boundary"


PATTERNS: tuple[Pattern, ...] = (
    Pattern("javascript-typescript", "js-ts.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "await is scheduling and thenable assimilation, not sync value equivalence", re.compile(r"\bawait\b")),
    Pattern("javascript-typescript", "js-ts.async.function", "async function", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "async functions have scheduling and rejection boundaries even without explicit await", re.compile(r"\basync\s+(?:function\b|[A-Za-z_$]|\([^)]*\)\s*=>)")),
    Pattern("javascript-typescript", "js-ts.promise.executor", "new Promise", "executor-callback", "promise-executor-timing-contract-missing", "executor timing", "new Promise needs executor timing, resolve/reject callback, and throw-to-rejection contracts", re.compile(r"\bnew\s+Promise\s*(?:<[^;\n(){}]*>)?\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.aggregate", "Promise.all", "success-error-result-channel", "promise-aggregate-all-fulfilled-contract-missing", "all-fulfilled aggregate", "Promise.all needs ordered all-fulfilled value and first rejection semantics", re.compile(r"\bPromise\s*\.\s*all\s*(?:<[^;\n(){}]*>)?\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.aggregate", "Promise.race", "cancellation-liveness-boundary", "promise-aggregate-first-settled-contract-missing", "first-settled aggregate", "Promise.race needs first-settled ordering and liveness proof", re.compile(r"\bPromise\s*\.\s*race\s*(?:<[^;\n(){}]*>)?\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.aggregate", "Promise.allSettled", "success-error-result-channel", "promise-aggregate-all-settled-contract-missing", "all-settled aggregate", "Promise.allSettled needs settled-record channel and shape proof", re.compile(r"\bPromise\s*\.\s*allSettled\s*(?:<[^;\n(){}]*>)?\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.aggregate", "Promise.any", "success-error-result-channel", "promise-aggregate-first-fulfilled-contract-missing", "first-fulfilled aggregate", "Promise.any needs first-fulfilled and AggregateError rejection proof", re.compile(r"\bPromise\s*\.\s*any\s*(?:<[^;\n(){}]*>)?\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.scheduler", "scheduler.wait", "scheduling-boundary", "scheduler-wait-timing-contract-missing", "scheduler wait", "scheduler.wait has timer and AbortSignal rejection boundaries", re.compile(r"\bscheduler\s*\.\s*wait\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.scheduler", "scheduler.yield", "scheduling-boundary", "scheduler-yield-microtask-order-contract-missing", "scheduler yield", "scheduler.yield needs microtask/order proof", re.compile(r"\bscheduler\s*\.\s*yield\s*\(")),
    Pattern("javascript-typescript", "js-ts.promise.interval", "setInterval", "lifecycle-materialization-boundary", "interval-async-iteration-lifecycle-contract-missing", "interval lifecycle", "setInterval and timers interval streams have repeated emission, cancellation, and liveness semantics", re.compile(r"(?<![A-Za-z0-9_$])setInterval\s*\(")),
    Pattern("javascript-typescript", "js-ts.cancellation.abort", "AbortController/AbortSignal", "cancellation-liveness-boundary", "abort-signal-cancellation-contract-missing", "cancellation signal", "AbortSignal/AbortController can change scheduling and rejection outcomes", re.compile(r"\b(?:AbortController|AbortSignal)\b")),
    Pattern("python", "python.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "Python await has coroutine scheduling and exception channel semantics", re.compile(r"\bawait\b")),
    Pattern("python", "python.async.function", "async def", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "async def creates coroutine protocol boundaries", re.compile(r"\basync\s+def\b")),
    Pattern("python", "python.async.iteration", "async for", "lifecycle-materialization-boundary", "async-iteration-lifecycle-contract-missing", "async iteration lifecycle", "async for statements and comprehensions need async iterator lifecycle, value-channel, and scheduling proof", re.compile(r"\basync\s+for\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.async.context", "async with", "lifecycle-materialization-boundary", "async-context-lifecycle-contract-missing", "async context lifecycle", "async with needs async enter/exit cleanup, exception-channel, and scheduling proof", re.compile(r"\basync\s+with\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.task", "asyncio.create_task/ensure_future", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio task spawn", "asyncio task APIs create scheduler, cancellation, and handle lifecycle boundaries", re.compile(r"\basyncio\s*\.\s*(?:create_task|ensure_future)\s*\(")),
    Pattern("python", "python.asyncio.sleep", "asyncio.sleep", "scheduling-boundary", "timer-scheduling-contract-missing", "asyncio timer", "asyncio sleep creates a timer-backed scheduling boundary", re.compile(r"\basyncio\s*\.\s*sleep\s*\(")),
    Pattern("python", "python.asyncio.gather", "asyncio.gather", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "asyncio all-completion aggregate", "asyncio gather needs all-completion, result-channel, cancellation, and exception semantics", re.compile(r"\basyncio\s*\.\s*gather\s*\(")),
    Pattern("python", "python.asyncio.wait", "asyncio.wait", "success-error-result-channel", "async-aggregate-completion-contract-missing", "asyncio completion aggregate", "asyncio wait needs completion-selection, result-channel, cancellation, and exception semantics", re.compile(r"\basyncio\s*\.\s*wait\s*\(")),
    Pattern("python", "python.asyncio.run", "asyncio.run", "scheduling-boundary", "future-drive-scheduling-contract-missing", "asyncio future drive", "asyncio.run drives a coroutine to completion with scheduling, result-channel, and exception boundaries", re.compile(r"\basyncio\s*\.\s*run\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.wait_for", "asyncio.wait_for", "scheduling-boundary", "timer-scheduling-contract-missing", "asyncio timeout wait", "asyncio.wait_for adds timer and cancellation/liveness boundaries around an awaitable result channel", re.compile(r"\basyncio\s*\.\s*wait_for\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.shield", "asyncio.shield", "cancellation-liveness-boundary", "task-cancellation-liveness-contract-missing", "asyncio cancellation shield", "asyncio.shield changes cancellation propagation while preserving an awaitable result channel", re.compile(r"\basyncio\s*\.\s*shield\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.run_coroutine_threadsafe", "asyncio.run_coroutine_threadsafe", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio thread-safe task submission", "asyncio.run_coroutine_threadsafe schedules a coroutine onto another loop and returns a future handle", re.compile(r"\basyncio\s*\.\s*run_coroutine_threadsafe\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.to_thread", "asyncio.to_thread", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio thread offload", "asyncio.to_thread schedules a callback on a worker thread and returns an awaitable result channel", re.compile(r"\basyncio\s*\.\s*to_thread\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.generator.yield", "yield", "lifecycle-materialization-boundary", "generator-yield-lifecycle-contract-missing", "generator lifecycle", "yield has suspension and iterator lifecycle semantics", re.compile(r"\byield(?:\s+from)?\b")),
    Pattern("rust", "rust.async.await", ".await", "scheduling-boundary", "async-await-scheduling-contract-missing", "future await", "Rust .await polls a Future and must keep wake/scheduling effects explicit", re.compile(r"\.\s*await\b")),
    Pattern("rust", "rust.async.function", "async fn", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "async fn creates a suspended async function boundary", re.compile(r"\basync\s+fn\b")),
    Pattern("rust", "rust.async.block", "async block", "scheduling-boundary", "async-block-scheduling-contract-missing", "async block construction", "async blocks create suspended async boundaries", re.compile(r"\basync\s+(?:move\b|\{)")),
    Pattern("rust", "rust.async.spawn", "tokio/async-std spawn", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "task spawn", "async spawn APIs introduce scheduler, cancellation, and join-handle boundaries", re.compile(r"\b(?:tokio(?:::task)?|async_std::task)\s*::\s*spawn(?:_blocking)?\s*\(")),
    Pattern("rust", "rust.async.join", "tokio/futures/futures_util join/try_join", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "future all-completion aggregate", "qualified runtime join style macros need all-completion result-channel proof", re.compile(r"\b(?:tokio|futures|futures_util)::(?:join|try_join)!\s*\(")),
    Pattern("rust", "rust.async.select", "tokio/futures/futures_util select", "cancellation-liveness-boundary", "async-aggregate-first-completion-contract-missing", "future first-completion aggregate", "qualified runtime select style macros need first-completion, cancellation, and result-channel proof", re.compile(r"\b(?:tokio|futures|futures_util)::select!\s*\(")),
    Pattern("go", "go.concurrent.goroutine", "go statement", "scheduling-boundary", "goroutine-scheduling-contract-missing", "goroutine scheduling", "go statements spawn concurrent execution", re.compile(r"\bgo\s+[A-Za-z_{(]")),
    Pattern("go", "go.concurrent.defer", "defer statement", "lifecycle-materialization-boundary", "defer-lifecycle-ordering-contract-missing", "defer lifecycle", "defer has scope-exit ordering and panic interaction semantics", re.compile(r"\bdefer\s+")),
    Pattern("go", "go.channel.send_receive", "channel send/receive", "channel-boundary", "channel-send-receive-protocol-contract-missing", "channel protocol", "channel send/receive has blocking and synchronization semantics", re.compile(r"<-")),
    Pattern("go", "go.channel.select", "select", "channel-boundary", "channel-select-readiness-contract-missing", "channel select", "select has readiness, default, and scheduling semantics", re.compile(r"\bselect\s*\{")),
    Pattern("java", "java.future.completable", "CompletableFuture", "success-error-result-channel", "future-settled-value-channel-contract-missing", "future channel", "CompletableFuture needs success/error channel and scheduling proof", re.compile(r"\bCompletableFuture\b(?!\s*\.\s*(?:supplyAsync|runAsync|completedFuture|completedStage|failedFuture|failedStage|allOf|anyOf)\s*\()")),
    Pattern("java", "java.future.completable.spawn", "CompletableFuture.supplyAsync/runAsync", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "future task spawn", "CompletableFuture async factories schedule executor callbacks and return future handles", re.compile(r"\bCompletableFuture\s*\.\s*(?:supplyAsync|runAsync)\s*\("), "reporting-candidate-closed-boundary"),
    Pattern("java", "java.future.completable.factory", "CompletableFuture.completedFuture/failedFuture", "success-error-result-channel", "future-settled-value-channel-contract-missing", "future settled value", "CompletableFuture settled factories create fulfilled or exceptional future channels", re.compile(r"\bCompletableFuture\s*\.\s*(?:completedFuture|completedStage|failedFuture|failedStage)\s*\("), "reporting-candidate-closed-boundary"),
    Pattern("java", "java.future.completable.all", "CompletableFuture.allOf", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "future all-completion aggregate", "CompletableFuture.allOf needs all-completion and exceptional completion proof", re.compile(r"\bCompletableFuture\s*\.\s*allOf\s*\("), "reporting-candidate-closed-boundary"),
    Pattern("java", "java.future.completable.any", "CompletableFuture.anyOf", "cancellation-liveness-boundary", "async-aggregate-first-completion-contract-missing", "future first-completion aggregate", "CompletableFuture.anyOf needs first-completion and result-channel proof", re.compile(r"\bCompletableFuture\s*\.\s*anyOf\s*\("), "reporting-candidate-closed-boundary"),
    Pattern("java", "java.future.executor", "Executor/Future", "scheduling-boundary", "java-executor-scheduling-contract-missing", "executor scheduling", "Executor/Future APIs introduce scheduler and lifecycle boundaries", re.compile(r"\b(?:ExecutorService|Executor|Future|ScheduledFuture)\b")),
    Pattern("java", "java.stream.lifecycle", "stream/parallelStream", "lifecycle-materialization-boundary", "java-stream-lifecycle-contract-missing", "stream lifecycle", "Java streams need lazy/eager lifecycle and terminal materialization proof", re.compile(r"\.\s*(?:stream|parallelStream)\s*\(")),
    Pattern("swift", "swift.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "Swift await has task scheduling and actor/lifetime boundaries", re.compile(r"\bawait\b")),
    Pattern("swift", "swift.async.function", "async", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "Swift async surfaces create task/future-like protocol boundaries", re.compile(r"\basync\b")),
    Pattern("swift", "swift.async.iteration", "for await/for try await", "lifecycle-materialization-boundary", "async-iteration-lifecycle-contract-missing", "async iteration lifecycle", "Swift async sequence loops need async iterator lifecycle, value-channel, scheduling, and throwing-channel proof", re.compile(r"\bfor\s+(?:try[!?]?\s+)?await\b"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.error.throws", "throws/try", "exception-channel", "swift-throws-exception-channel-contract-missing", "throws channel", "Swift throws/try is an explicit error channel", re.compile(r"\b(?:throws|try)\b")),
    Pattern("swift", "swift.task.spawn", "Task/Task.detached", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "task scheduling", "Task APIs introduce scheduler, cancellation, and handle lifecycle boundaries", re.compile(r"\bTask(?:\s*\.\s*detached\s*(?:\{|\()|\s*\{)")),
    Pattern("swift", "swift.task.sleep", "Task.sleep", "scheduling-boundary", "timer-scheduling-contract-missing", "task sleep timer", "Swift Task.sleep creates a timer-backed scheduling boundary and cancellation/liveness boundary", re.compile(r"\bTask\s*\.\s*sleep\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.task.yield", "Task.yield", "scheduling-boundary", "task-yield-scheduling-contract-missing", "task yield", "Swift Task.yield yields to the task scheduler and must not collapse into sync value equivalence", re.compile(r"\bTask\s*\.\s*yield\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.task.group", "withTaskGroup/withThrowingTaskGroup/withDiscardingTaskGroup/withThrowingDiscardingTaskGroup", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "task-group aggregate", "Swift task groups need all-completion, result-channel, cancellation/liveness, and throwing error-channel proof", re.compile(r"\bwith(?:Throwing)?(?:Discarding)?TaskGroup\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.continuation.checked", "withCheckedContinuation/withUnsafeContinuation", "success-error-result-channel", "future-settled-value-channel-contract-missing", "Swift continuation bridge", "Swift continuation bridges suspend and resume through a callback-settled future-like result channel", re.compile(r"\bwith(?:Checked|Unsafe)Continuation\s*(?:\(|\{)"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.continuation.throwing", "withCheckedThrowingContinuation/withUnsafeThrowingContinuation", "success-error-result-channel", "future-settled-value-channel-contract-missing", "Swift throwing continuation bridge", "Swift throwing continuation bridges add exception-channel behavior to callback-settled future-like result channels", re.compile(r"\bwith(?:Checked|Unsafe)ThrowingContinuation\s*(?:\(|\{)"), "reporting-supported-closed-boundary"),
    Pattern("ruby", "ruby.thread.fiber", "Thread/Fiber", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "thread/fiber scheduling", "Thread/Fiber APIs create scheduler and lifecycle boundaries", re.compile(r"\b(?:Thread\s*\.\s*(?:new|start|fork)|Fiber\s*\.\s*(?:new|schedule))\b"), "reporting-supported-closed-boundary"),
    Pattern("ruby", "ruby.exception", "raise/rescue", "exception-channel", "ruby-exception-channel-contract-missing", "exception channel", "raise/rescue changes error channels and non-local control", re.compile(r"\b(?:raise|rescue)\b")),
    Pattern("ruby", "ruby.generator.yield", "yield", "callback-demand-effect", "ruby-yield-callback-demand-effect-contract-missing", "block callback", "Ruby yield invokes a block with demand/effect obligations", re.compile(r"\byield\b")),
    Pattern("c", "c.thread.pthread", "pthread_create", "scheduling-boundary", "c-pthread-scheduling-contract-missing", "thread scheduling", "pthread_create introduces thread scheduling and lifetime boundaries", re.compile(r"\bpthread_create\s*\(")),
    Pattern("c", "c.nonlocal_jump", "setjmp/longjmp", "exception-channel", "c-nonlocal-jump-contract-missing", "non-local jump", "setjmp/longjmp is non-local control flow, not ordinary return", re.compile(r"\b(?:setjmp|longjmp)\s*\(")),
)


PYTHON_ASYNCIO_ALIAS_TASK = Pattern(
    "python",
    "python.asyncio.alias.task",
    "import asyncio as alias; alias.create_task/ensure_future",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "asyncio alias task spawn",
    "import-backed asyncio aliases create the same scheduler, cancellation, and handle lifecycle boundaries as asyncio.*",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_SLEEP = Pattern(
    "python",
    "python.asyncio.alias.sleep",
    "import asyncio as alias; alias.sleep",
    "scheduling-boundary",
    "timer-scheduling-contract-missing",
    "asyncio alias timer",
    "import-backed asyncio aliases create timer-backed scheduling boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_GATHER = Pattern(
    "python",
    "python.asyncio.alias.gather",
    "import asyncio as alias; alias.gather",
    "success-error-result-channel",
    "async-aggregate-all-completion-contract-missing",
    "asyncio alias all-completion aggregate",
    "import-backed asyncio aliases need all-completion, result-channel, cancellation, and exception semantics",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_WAIT = Pattern(
    "python",
    "python.asyncio.alias.wait",
    "import asyncio as alias; alias.wait",
    "success-error-result-channel",
    "async-aggregate-completion-contract-missing",
    "asyncio alias completion aggregate",
    "import-backed asyncio aliases need completion-selection, result-channel, cancellation, and exception semantics",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_RUN = Pattern(
    "python",
    "python.asyncio.alias.run",
    "import asyncio as alias; alias.run",
    "scheduling-boundary",
    "future-drive-scheduling-contract-missing",
    "asyncio alias future drive",
    "import-backed asyncio aliases drive coroutine completion through the same scheduling and result-channel boundaries as asyncio.run",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_WAIT_FOR = Pattern(
    "python",
    "python.asyncio.alias.wait_for",
    "import asyncio as alias; alias.wait_for",
    "scheduling-boundary",
    "timer-scheduling-contract-missing",
    "asyncio alias timeout wait",
    "import-backed asyncio aliases add timer and cancellation/liveness boundaries around wait_for result channels",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_SHIELD = Pattern(
    "python",
    "python.asyncio.alias.shield",
    "import asyncio as alias; alias.shield",
    "cancellation-liveness-boundary",
    "task-cancellation-liveness-contract-missing",
    "asyncio alias cancellation shield",
    "import-backed asyncio aliases preserve shield cancellation/liveness boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_THREADSAFE = Pattern(
    "python",
    "python.asyncio.alias.run_coroutine_threadsafe",
    "import asyncio as alias; alias.run_coroutine_threadsafe",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "asyncio alias thread-safe task submission",
    "import-backed asyncio aliases schedule coroutine submission onto another loop and return a future handle",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_ALIAS_TO_THREAD = Pattern(
    "python",
    "python.asyncio.alias.to_thread",
    "import asyncio as alias; alias.to_thread",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "asyncio alias thread offload",
    "import-backed asyncio aliases schedule callback execution on a worker thread and return an awaitable result channel",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_TASK = Pattern(
    "python",
    "python.asyncio.imported.task",
    "from asyncio import create_task/ensure_future; binding",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "imported asyncio task spawn",
    "import-backed asyncio task bindings create the same scheduler, cancellation, and handle lifecycle boundaries as asyncio.*",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_SLEEP = Pattern(
    "python",
    "python.asyncio.imported.sleep",
    "from asyncio import sleep; binding",
    "scheduling-boundary",
    "timer-scheduling-contract-missing",
    "imported asyncio timer",
    "import-backed asyncio sleep bindings create timer-backed scheduling boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_GATHER = Pattern(
    "python",
    "python.asyncio.imported.gather",
    "from asyncio import gather; binding",
    "success-error-result-channel",
    "async-aggregate-all-completion-contract-missing",
    "imported asyncio all-completion aggregate",
    "import-backed asyncio gather bindings need all-completion, result-channel, cancellation, and exception semantics",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_WAIT = Pattern(
    "python",
    "python.asyncio.imported.wait",
    "from asyncio import wait; binding",
    "success-error-result-channel",
    "async-aggregate-completion-contract-missing",
    "imported asyncio completion aggregate",
    "import-backed asyncio wait bindings need completion-selection, result-channel, cancellation, and exception semantics",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_RUN = Pattern(
    "python",
    "python.asyncio.imported.run",
    "from asyncio import run; binding",
    "scheduling-boundary",
    "future-drive-scheduling-contract-missing",
    "imported asyncio future drive",
    "import-backed asyncio run bindings drive coroutine completion through scheduling and result-channel boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_WAIT_FOR = Pattern(
    "python",
    "python.asyncio.imported.wait_for",
    "from asyncio import wait_for; binding",
    "scheduling-boundary",
    "timer-scheduling-contract-missing",
    "imported asyncio timeout wait",
    "import-backed asyncio wait_for bindings add timer and cancellation/liveness boundaries around an awaitable result channel",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_SHIELD = Pattern(
    "python",
    "python.asyncio.imported.shield",
    "from asyncio import shield; binding",
    "cancellation-liveness-boundary",
    "task-cancellation-liveness-contract-missing",
    "imported asyncio cancellation shield",
    "import-backed asyncio shield bindings preserve cancellation/liveness boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_THREADSAFE = Pattern(
    "python",
    "python.asyncio.imported.run_coroutine_threadsafe",
    "from asyncio import run_coroutine_threadsafe; binding",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "imported asyncio thread-safe task submission",
    "import-backed asyncio run_coroutine_threadsafe bindings schedule onto another loop and return a future handle",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
PYTHON_ASYNCIO_IMPORTED_TO_THREAD = Pattern(
    "python",
    "python.asyncio.imported.to_thread",
    "from asyncio import to_thread; binding",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "imported asyncio thread offload",
    "import-backed asyncio to_thread bindings schedule callback execution on a worker thread and return an awaitable result channel",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
RUST_IMPORTED_ASYNC_SPAWN = Pattern(
    "rust",
    "rust.async.spawn.imported",
    "use runtime::spawn; spawn",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "imported task spawn",
    "import-backed tokio/async-std spawn bindings introduce scheduler, cancellation, and join-handle boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
RUST_IMPORTED_ASYNC_JOIN = Pattern(
    "rust",
    "rust.async.join.imported",
    "use runtime::join; join!",
    "success-error-result-channel",
    "async-aggregate-all-completion-contract-missing",
    "imported future all-completion aggregate",
    "import-backed tokio/futures/futures_util join-style macros need all-completion result-channel proof",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
RUST_IMPORTED_ASYNC_SELECT = Pattern(
    "rust",
    "rust.async.select.imported",
    "use runtime::select; select!",
    "cancellation-liveness-boundary",
    "async-aggregate-first-completion-contract-missing",
    "imported future first-completion aggregate",
    "import-backed tokio/futures/futures_util select macros need first-completion, cancellation, and result-channel proof",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_FUTURE_FULFILLMENT_CONTINUATION = Pattern(
    "java",
    "java.future.completion_stage.fulfillment",
    "FutureLike.thenApply/thenAccept/thenRun/thenCompose",
    "success-error-result-channel",
    "future-fulfillment-continuation-contract-missing",
    "future fulfillment continuation",
    "Future-like receivers need fulfillment continuation and callback demand/effect proof",
    re.compile(r"(?!x)x"),
    "reporting-candidate-closed-boundary",
)
JAVA_FUTURE_EXCEPTION_CONTINUATION = Pattern(
    "java",
    "java.future.completion_stage.exception",
    "FutureLike.exceptionally",
    "exception-channel",
    "future-exception-continuation-contract-missing",
    "future exception continuation",
    "Future-like receivers need exceptional completion continuation and callback demand/effect proof",
    re.compile(r"(?!x)x"),
    "reporting-candidate-closed-boundary",
)
JAVA_FUTURE_SETTLEMENT_CONTINUATION = Pattern(
    "java",
    "java.future.completion_stage.settlement",
    "FutureLike.handle/whenComplete",
    "success-error-result-channel",
    "future-settlement-continuation-contract-missing",
    "future settlement continuation",
    "Future-like receivers need settlement continuation and callback demand/effect proof",
    re.compile(r"(?!x)x"),
    "reporting-candidate-closed-boundary",
)
JAVA_FUTURE_ALL_COMPLETION_CONTINUATION = Pattern(
    "java",
    "java.future.completion_stage.all",
    "FutureLike.thenCombine/thenAcceptBoth/runAfterBoth",
    "success-error-result-channel",
    "async-aggregate-all-completion-contract-missing",
    "future all-completion continuation",
    "Future-like pair continuations need all-completion and callback demand/effect proof",
    re.compile(r"(?!x)x"),
    "reporting-candidate-closed-boundary",
)
JAVA_FUTURE_FIRST_COMPLETION_CONTINUATION = Pattern(
    "java",
    "java.future.completion_stage.first",
    "FutureLike.applyToEither/acceptEither/runAfterEither",
    "cancellation-liveness-boundary",
    "async-aggregate-first-completion-contract-missing",
    "future first-completion continuation",
    "Future-like either continuations need first-completion and callback demand/effect proof",
    re.compile(r"(?!x)x"),
    "reporting-candidate-closed-boundary",
)
JAVA_FUTURE_HANDLE_GET = Pattern(
    "java",
    "java.future.handle.get",
    "Future.get",
    "success-error-result-channel",
    "future-settled-value-channel-contract-missing",
    "future handle get",
    "Exact-import-backed Java Future receivers expose blocking settled-value and exception channels",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_FUTURE_HANDLE_CANCEL = Pattern(
    "java",
    "java.future.handle.cancel",
    "Future.cancel/isCancelled",
    "cancellation-liveness-boundary",
    "task-cancellation-liveness-contract-missing",
    "future handle cancellation",
    "Exact-import-backed Java Future receivers expose cancellation and task-handle liveness boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_FUTURE_HANDLE_STATUS = Pattern(
    "java",
    "java.future.handle.status",
    "Future.isDone",
    "lifecycle-materialization-boundary",
    "task-handle-lifecycle-contract-missing",
    "future handle lifecycle",
    "Exact-import-backed Java Future receivers expose task-handle lifecycle status",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_EXECUTOR_EXECUTE = Pattern(
    "java",
    "java.executor.execute",
    "Executor.execute",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "executor runnable scheduling",
    "Exact-import-backed Java Executor receivers schedule callback execution without returning a task handle",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_EXECUTOR_SUBMIT = Pattern(
    "java",
    "java.executor_service.submit",
    "ExecutorService.submit",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "executor service future scheduling",
    "Exact-import-backed Java ExecutorService receivers schedule callbacks and return Future handles",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_EXECUTOR_INVOKE_ALL = Pattern(
    "java",
    "java.executor_service.invoke_all",
    "ExecutorService.invokeAll",
    "success-error-result-channel",
    "async-aggregate-all-completion-contract-missing",
    "executor service all-completion aggregate",
    "Exact-import-backed Java ExecutorService invokeAll waits for all submitted tasks and returns Future result channels",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_EXECUTOR_INVOKE_ANY = Pattern(
    "java",
    "java.executor_service.invoke_any",
    "ExecutorService.invokeAny",
    "cancellation-liveness-boundary",
    "async-aggregate-first-completion-contract-missing",
    "executor service first-completion aggregate",
    "Exact-import-backed Java ExecutorService invokeAny exposes first-success completion, cancellation, and exception boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_SCHEDULED_EXECUTOR_SCHEDULE = Pattern(
    "java",
    "java.scheduled_executor.schedule",
    "ScheduledExecutorService.schedule",
    "scheduling-boundary",
    "timer-scheduling-contract-missing",
    "scheduled executor timer",
    "Exact-import-backed Java ScheduledExecutorService schedule calls add timer-backed task scheduling and Future result channels",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_SCHEDULED_EXECUTOR_INTERVAL = Pattern(
    "java",
    "java.scheduled_executor.interval",
    "ScheduledExecutorService.scheduleAtFixedRate/scheduleWithFixedDelay",
    "lifecycle-materialization-boundary",
    "interval-async-iteration-lifecycle-contract-missing",
    "scheduled executor interval lifecycle",
    "Exact-import-backed Java ScheduledExecutorService repeating schedules expose interval lifecycle and cancellation boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
GO_CHANNEL_SEND = Pattern(
    "go",
    "go.channel.send",
    "channel send",
    "channel-boundary",
    "channel-send-synchronization-contract-missing",
    "channel send synchronization",
    "Go channel sends have blocking and synchronization semantics",
    re.compile(r"(?!x)x"),
)
GO_CHANNEL_RECEIVE = Pattern(
    "go",
    "go.channel.receive",
    "channel receive",
    "channel-boundary",
    "channel-receive-value-channel-contract-missing",
    "channel receive value",
    "Go channel receives have blocking, synchronization, and close/zero-value channel semantics",
    re.compile(r"(?!x)x"),
)
GO_CHANNEL_RECEIVE_STATUS = Pattern(
    "go",
    "go.channel.receive_status",
    "comma-ok channel receive",
    "channel-boundary",
    "channel-receive-status-contract-missing",
    "channel receive status",
    "Go comma-ok receives expose the channel close status as an additional protocol result",
    re.compile(r"(?!x)x"),
)
GO_CHANNEL_SELECT_CASE = Pattern(
    "go",
    "go.channel.select.case",
    "select case",
    "channel-boundary",
    "channel-select-case-selection-contract-missing",
    "select case selection",
    "Go select cases require readiness, case-selection, and send/receive side-effect proof",
    re.compile(r"(?!x)x"),
)
GO_CHANNEL_SELECT_DEFAULT = Pattern(
    "go",
    "go.channel.select.default",
    "select default",
    "channel-boundary",
    "channel-select-default-liveness-contract-missing",
    "select default liveness",
    "Go select defaults change blocking and liveness behavior",
    re.compile(r"(?!x)x"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default=DEFAULT_MANIFEST)
    parser.add_argument("--repos-root", default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    parser.add_argument("--generated-on", default=DEFAULT_GENERATED_ON)
    parser.add_argument("--recall-loss-report", default=None)
    return parser.parse_args()


def load_repos(manifest: Path) -> list[dict[str, Any]]:
    return json.loads(manifest.read_text()).get("repositories", [])


def language_for_path(path: Path) -> str | None:
    for language, exts in LANG_EXTS.items():
        if path.suffix in exts:
            return language
    return None


def source_files(repo_root: Path) -> list[Path]:
    if not repo_root.exists():
        return []
    files: list[Path] = []
    for path in repo_root.rglob("*"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.is_file() and language_for_path(path):
            files.append(path)
    return files


def mask_comments_and_strings(text: str) -> str:
    chars = list(text)
    i = 0
    while i < len(chars):
        ch = chars[i]
        nxt = chars[i + 1] if i + 1 < len(chars) else ""
        if ch in {"'", '"', "`"}:
            quote = ch
            chars[i] = " "
            i += 1
            escaped = False
            while i < len(chars):
                cur = chars[i]
                if cur == "\n" and quote != "`":
                    break
                chars[i] = "\n" if cur == "\n" else " "
                if cur == quote and not escaped:
                    i += 1
                    break
                escaped = cur == "\\" and not escaped
                if cur != "\\":
                    escaped = False
                i += 1
            continue
        if ch == "/" and nxt == "/":
            while i < len(chars) and chars[i] != "\n":
                chars[i] = " "
                i += 1
            continue
        if ch == "/" and nxt == "*":
            chars[i] = chars[i + 1] = " "
            i += 2
            while i + 1 < len(chars) and not (chars[i] == "*" and chars[i + 1] == "/"):
                chars[i] = "\n" if chars[i] == "\n" else " "
                i += 1
            if i + 1 < len(chars):
                chars[i] = chars[i + 1] = " "
                i += 2
            continue
        if ch == "#":
            while i < len(chars) and chars[i] != "\n":
                chars[i] = " "
                i += 1
            continue
        i += 1
    return "".join(chars)


def count_file(text: str, language: str) -> dict[Pattern, int]:
    masked = mask_comments_and_strings(text)
    counts: dict[Pattern, int] = {}
    for pattern in PATTERNS:
        if pattern.language != language:
            continue
        count = sum(1 for _ in pattern.regex.finditer(masked))
        if count:
            counts[pattern] = count
    if language == "python":
        counts.update(python_asyncio_alias_counts(masked))
        counts.update(python_asyncio_imported_counts(masked))
    elif language == "rust":
        counts.update(rust_imported_async_runtime_counts(masked))
    elif language == "go":
        counts = {
            pattern: count
            for pattern, count in counts.items()
            if pattern.surface not in {"go.channel.send_receive"}
        }
        counts.update(go_channel_protocol_counts(masked))
    elif language == "java":
        counts.update(java_future_receiver_counts(masked))
    return counts


def go_channel_protocol_counts(text: str) -> dict[Pattern, int]:
    counts: dict[Pattern, int] = {}
    send_count = 0
    receive_count = 0
    status_count = len(
        re.findall(
            r"\b[A-Za-z_][A-Za-z0-9_]*\s*,\s*[A-Za-z_][A-Za-z0-9_]*\s*(?::=|=)\s*<-",
            text,
        )
    )
    for line in text.splitlines():
        for match in re.finditer(r"<-", line):
            if go_channel_arrow_is_directional_type(line, match.start(), match.end()):
                continue
            left = line[: match.start()].strip()
            if go_channel_operator_is_receive(left):
                receive_count += 1
            else:
                send_count += 1
    if send_count:
        counts[GO_CHANNEL_SEND] = send_count
    if receive_count:
        counts[GO_CHANNEL_RECEIVE] = receive_count
    if status_count:
        counts[GO_CHANNEL_RECEIVE_STATUS] = status_count
    select_case_count, select_default_count = go_select_arm_counts(text)
    if select_case_count:
        counts[GO_CHANNEL_SELECT_CASE] = select_case_count
    if select_default_count:
        counts[GO_CHANNEL_SELECT_DEFAULT] = select_default_count
    return counts


def go_channel_arrow_is_directional_type(line: str, start: int, end: int) -> bool:
    before = line[:start].rstrip()
    after = line[end:].lstrip()
    return re.search(r"\bchan\s*$", before) is not None or re.match(r"chan\b", after) is not None


def go_channel_operator_is_receive(left: str) -> bool:
    if not left:
        return True
    stripped = left.rstrip()
    if stripped.endswith((":=", "=", ",", "(", "[", "{")):
        return True
    last = stripped.rsplit(None, 1)[-1]
    return last in {"return", "case", "range"}


def go_select_arm_counts(text: str) -> tuple[int, int]:
    case_count = 0
    default_count = 0
    for match in re.finditer(r"\bselect\s*\{", text):
        open_brace = text.find("{", match.start(), match.end())
        if open_brace < 0:
            continue
        close_brace = matching_brace(text, open_brace)
        if close_brace is None:
            continue
        cases, defaults = go_top_level_case_counts(text[open_brace + 1 : close_brace])
        case_count += cases
        default_count += defaults
    return case_count, default_count


def matching_brace(text: str, open_brace: int) -> int | None:
    depth = 0
    for idx in range(open_brace, len(text)):
        if text[idx] == "{":
            depth += 1
        elif text[idx] == "}":
            depth -= 1
            if depth == 0:
                return idx
    return None


def go_top_level_case_counts(block: str) -> tuple[int, int]:
    case_count = 0
    default_count = 0
    depth = 0
    idx = 0
    while idx < len(block):
        ch = block[idx]
        if ch == "{":
            depth += 1
            idx += 1
            continue
        if ch == "}":
            depth = max(0, depth - 1)
            idx += 1
            continue
        if depth == 0 and go_word_at(block, idx, "case"):
            colon = block.find(":", idx + len("case"))
            newline = block.find("\n", idx)
            if colon >= 0 and (newline < 0 or colon < newline):
                case_count += 1
                idx = colon + 1
                continue
        if depth == 0 and go_word_at(block, idx, "default"):
            colon = block.find(":", idx + len("default"))
            newline = block.find("\n", idx)
            if colon >= 0 and (newline < 0 or colon < newline):
                default_count += 1
                idx = colon + 1
                continue
        idx += 1
    return case_count, default_count


def go_word_at(text: str, idx: int, word: str) -> bool:
    if not text.startswith(word, idx):
        return False
    before = text[idx - 1] if idx > 0 else ""
    after_idx = idx + len(word)
    after = text[after_idx] if after_idx < len(text) else ""
    return not (before.isalnum() or before == "_") and not (after.isalnum() or after == "_")


def python_asyncio_alias_counts(text: str) -> dict[Pattern, int]:
    aliases = python_asyncio_aliases(text)
    if not aliases:
        return {}
    counts: dict[Pattern, int] = {}
    count_by_methods(
        counts,
        PYTHON_ASYNCIO_ALIAS_TASK,
        text,
        aliases,
        ("create_task", "ensure_future"),
        ".",
    )
    count_by_methods(counts, PYTHON_ASYNCIO_ALIAS_SLEEP, text, aliases, ("sleep",), ".")
    count_by_methods(counts, PYTHON_ASYNCIO_ALIAS_GATHER, text, aliases, ("gather",), ".")
    count_by_methods(counts, PYTHON_ASYNCIO_ALIAS_WAIT, text, aliases, ("wait",), ".")
    count_by_methods(counts, PYTHON_ASYNCIO_ALIAS_RUN, text, aliases, ("run",), ".")
    count_by_methods(
        counts, PYTHON_ASYNCIO_ALIAS_WAIT_FOR, text, aliases, ("wait_for",), "."
    )
    count_by_methods(counts, PYTHON_ASYNCIO_ALIAS_SHIELD, text, aliases, ("shield",), ".")
    count_by_methods(
        counts,
        PYTHON_ASYNCIO_ALIAS_THREADSAFE,
        text,
        aliases,
        ("run_coroutine_threadsafe",),
        ".",
    )
    count_by_methods(
        counts, PYTHON_ASYNCIO_ALIAS_TO_THREAD, text, aliases, ("to_thread",), "."
    )
    return counts


def python_asyncio_aliases(text: str) -> set[str]:
    aliases: set[str] = set()
    for match in re.finditer(r"(?m)^\s*import\s+([^\n]+)", text):
        for part in match.group(1).split(","):
            pieces = part.strip().split()
            if len(pieces) == 3 and pieces[0] == "asyncio" and pieces[1] == "as":
                alias = pieces[2]
                if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", alias) and alias != "asyncio":
                    aliases.add(alias)
    return aliases


def python_asyncio_imported_counts(text: str) -> dict[Pattern, int]:
    bindings = python_asyncio_imported_bindings(text)
    if not bindings:
        return {}
    counts: dict[Pattern, int] = {}
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_TASK,
        text,
        bindings_for_python(bindings, ("create_task", "ensure_future")),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_SLEEP,
        text,
        bindings_for_python(bindings, ("sleep",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_GATHER,
        text,
        bindings_for_python(bindings, ("gather",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_WAIT,
        text,
        bindings_for_python(bindings, ("wait",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_RUN,
        text,
        bindings_for_python(bindings, ("run",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_WAIT_FOR,
        text,
        bindings_for_python(bindings, ("wait_for",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_SHIELD,
        text,
        bindings_for_python(bindings, ("shield",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_THREADSAFE,
        text,
        bindings_for_python(bindings, ("run_coroutine_threadsafe",)),
        "(",
    )
    count_bindings(
        counts,
        PYTHON_ASYNCIO_IMPORTED_TO_THREAD,
        text,
        bindings_for_python(bindings, ("to_thread",)),
        "(",
    )
    return counts


def python_asyncio_imported_bindings(text: str) -> dict[str, set[str]]:
    bindings: dict[str, set[str]] = defaultdict(set)
    for match in re.finditer(r"(?m)^\s*from\s+asyncio\s+import\s+([^\n]+)", text):
        for part in match.group(1).split(","):
            parsed = python_imported_name(part)
            if not parsed:
                continue
            exported, local = parsed
            if exported in {
                "create_task",
                "ensure_future",
                "sleep",
                "gather",
                "wait",
                "run",
                "wait_for",
                "shield",
                "run_coroutine_threadsafe",
                "to_thread",
            }:
                bindings[exported].add(local)
    return bindings


def python_imported_name(part: str) -> tuple[str, str] | None:
    pieces = part.strip().split()
    if len(pieces) == 1:
        exported = local = pieces[0]
    elif len(pieces) == 3 and pieces[1] == "as":
        exported = pieces[0]
        local = pieces[2]
    else:
        return None
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", exported) and re.fullmatch(
        r"[A-Za-z_][A-Za-z0-9_]*",
        local,
    ):
        return exported, local
    return None


def bindings_for_python(bindings: dict[str, set[str]], targets: tuple[str, ...]) -> set[str]:
    out: set[str] = set()
    for target in targets:
        out.update(bindings.get(target, set()))
    return out


def rust_imported_async_runtime_counts(text: str) -> dict[Pattern, int]:
    bindings = rust_imported_runtime_bindings(text)
    if not bindings:
        return {}
    counts: dict[Pattern, int] = {}
    spawn_bindings = bindings_for(
        bindings,
        {
            ("tokio", "spawn"),
            ("tokio::task", "spawn"),
            ("tokio::task", "spawn_blocking"),
            ("async_std::task", "spawn"),
            ("async_std::task", "spawn_blocking"),
        },
    )
    join_bindings = bindings_for(
        bindings,
        {
            ("tokio", "join"),
            ("tokio", "try_join"),
            ("futures", "join"),
            ("futures", "try_join"),
            ("futures_util", "join"),
            ("futures_util", "try_join"),
        },
    )
    select_bindings = bindings_for(
        bindings,
        {
            ("tokio", "select"),
            ("futures", "select"),
            ("futures_util", "select"),
        },
    )
    count_bindings(counts, RUST_IMPORTED_ASYNC_SPAWN, text, spawn_bindings, "(")
    count_bindings(counts, RUST_IMPORTED_ASYNC_JOIN, text, join_bindings, "!")
    count_bindings(counts, RUST_IMPORTED_ASYNC_SELECT, text, select_bindings, "!")
    return counts


def count_by_methods(
    counts: dict[Pattern, int],
    pattern: Pattern,
    text: str,
    names: set[str],
    methods: tuple[str, ...],
    separator: str,
) -> None:
    method_pattern = "|".join(re.escape(method) for method in methods)
    total = 0
    for name in names:
        total += len(
            re.findall(
                rf"\b{re.escape(name)}\s*{re.escape(separator)}\s*(?:{method_pattern})\s*\(",
                text,
            )
        )
    if total:
        counts[pattern] = total


def count_bindings(
    counts: dict[Pattern, int],
    pattern: Pattern,
    text: str,
    bindings: set[str],
    call_marker: str,
) -> None:
    total = 0
    for binding in bindings:
        if call_marker == "!":
            total += len(re.findall(rf"\b{re.escape(binding)}\s*!\s*\(", text))
        else:
            total += len(re.findall(rf"\b{re.escape(binding)}\s*\(", text))
    if total:
        counts[pattern] = total


def bindings_for(
    bindings: dict[tuple[str, str], set[str]],
    targets: set[tuple[str, str]],
) -> set[str]:
    out: set[str] = set()
    for target in targets:
        out.update(bindings.get(target, set()))
    return out


def rust_imported_runtime_bindings(text: str) -> dict[tuple[str, str], set[str]]:
    bindings: dict[tuple[str, str], set[str]] = defaultdict(set)
    for match in re.finditer(r"\buse\s+([^;]+);", text, re.DOTALL):
        body = " ".join(match.group(1).split())
        if "*" in body:
            continue
        for module, exported, local in rust_use_bindings(body):
            if rust_runtime_import_target(module, exported):
                bindings[(module, exported)].add(local)
    return bindings


def rust_use_bindings(body: str) -> list[tuple[str, str, str]]:
    if "{" in body or "}" in body:
        return rust_brace_use_bindings(body)
    parsed = parse_rust_use_item(body)
    if not parsed:
        return []
    path, local = parsed
    split = split_rust_path_for_binding(path)
    if not split:
        return []
    module, exported = split
    return [(module, exported, local or exported)]


def rust_brace_use_bindings(body: str) -> list[tuple[str, str, str]]:
    open_idx = body.find("{")
    close_idx = body.rfind("}")
    if open_idx < 0 or close_idx <= open_idx:
        return []
    items = body[open_idx + 1 : close_idx]
    if "{" in items or "}" in items or body[close_idx + 1 :].strip():
        return []
    prefix = body[:open_idx].strip().removesuffix("::").strip()
    if not prefix or prefix.startswith(("self", "super")):
        return []
    bindings: list[tuple[str, str, str]] = []
    for raw_item in items.split(","):
        parsed = parse_rust_use_item(raw_item)
        if not parsed:
            continue
        path, local = parsed
        if path == "self":
            continue
        split = split_rust_path_tail(path)
        if not split:
            continue
        path_prefix, exported = split
        module = f"{prefix}::{path_prefix}" if path_prefix else prefix
        bindings.append((module, exported, local or exported))
    return bindings


def parse_rust_use_item(item: str) -> tuple[str, str | None] | None:
    item = item.strip()
    if not item or any(token in item for token in ("{", "}", "*")):
        return None
    if " as " in item:
        path, local = item.split(" as ", 1)
        path = path.strip()
        local = local.strip()
        if not path or not local:
            return None
        return path, local
    return item, None


def split_rust_path_for_binding(path: str) -> tuple[str, str] | None:
    if "::" not in path:
        return None
    module, exported = path.rsplit("::", 1)
    module = module.strip()
    exported = exported.strip()
    if not module or not exported or module.startswith(("self", "super")):
        return None
    return module, exported


def split_rust_path_tail(path: str) -> tuple[str | None, str] | None:
    path = path.strip()
    if not path:
        return None
    if "::" not in path:
        return None, path
    path_prefix, exported = path.rsplit("::", 1)
    if not exported:
        return None
    return path_prefix, exported


def rust_runtime_import_target(module: str, exported: str) -> bool:
    return (module, exported) in {
        ("tokio", "spawn"),
        ("tokio::task", "spawn"),
        ("tokio::task", "spawn_blocking"),
        ("async_std::task", "spawn"),
        ("async_std::task", "spawn_blocking"),
        ("tokio", "join"),
        ("tokio", "try_join"),
        ("futures", "join"),
        ("futures", "try_join"),
        ("futures_util", "join"),
        ("futures_util", "try_join"),
        ("tokio", "select"),
        ("futures", "select"),
        ("futures_util", "select"),
    }


def java_future_receiver_counts(text: str) -> dict[Pattern, int]:
    receivers = java_future_like_receiver_names(text)
    counts: dict[Pattern, int] = {}
    if receivers:
        count_by_methods(
            counts,
            JAVA_FUTURE_FULFILLMENT_CONTINUATION,
            text,
            receivers,
            (
                "thenApply",
                "thenApplyAsync",
                "thenAccept",
                "thenAcceptAsync",
                "thenRun",
                "thenRunAsync",
                "thenCompose",
                "thenComposeAsync",
            ),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_FUTURE_EXCEPTION_CONTINUATION,
            text,
            receivers,
            (
                "exceptionally",
                "exceptionallyAsync",
                "exceptionallyCompose",
                "exceptionallyComposeAsync",
            ),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_FUTURE_SETTLEMENT_CONTINUATION,
            text,
            receivers,
            ("handle", "handleAsync", "whenComplete", "whenCompleteAsync"),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_FUTURE_ALL_COMPLETION_CONTINUATION,
            text,
            receivers,
            (
                "thenCombine",
                "thenCombineAsync",
                "thenAcceptBoth",
                "thenAcceptBothAsync",
                "runAfterBoth",
                "runAfterBothAsync",
            ),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_FUTURE_FIRST_COMPLETION_CONTINUATION,
            text,
            receivers,
            (
                "applyToEither",
                "applyToEitherAsync",
                "acceptEither",
                "acceptEitherAsync",
                "runAfterEither",
                "runAfterEitherAsync",
            ),
            ".",
        )
    java_future_handle_counts(counts, text)
    java_executor_receiver_counts(counts, text)
    return counts


def java_future_like_receiver_names(text: str) -> set[str]:
    type_name = (
        r"(?:java\s*\.\s*util\s*\.\s*concurrent\s*\.\s*)?"
        r"(?:CompletableFuture|CompletionStage)"
    )
    pattern = re.compile(
        rf"\b{type_name}\b(?:\s*<[^;()={{}}]*>)?(?:\s*\[\s*\])?\s+"
        rf"([A-Za-z_$][A-Za-z0-9_$]*)"
    )
    return {
        match.group(1)
        for match in pattern.finditer(text)
        if match.group(1) not in {"class", "interface", "enum", "record"}
    }


def java_future_handle_counts(counts: dict[Pattern, int], text: str) -> None:
    future_receivers = java_exact_import_backed_receiver_names(
        text,
        {"CompletableFuture", "Future", "ScheduledFuture"},
    )
    if not future_receivers:
        return
    count_by_methods(counts, JAVA_FUTURE_HANDLE_GET, text, future_receivers, ("get",), ".")
    count_by_methods(
        counts,
        JAVA_FUTURE_HANDLE_CANCEL,
        text,
        future_receivers,
        ("cancel", "isCancelled"),
        ".",
    )
    count_by_methods(
        counts,
        JAVA_FUTURE_HANDLE_STATUS,
        text,
        future_receivers,
        ("isDone",),
        ".",
    )


def java_executor_receiver_counts(counts: dict[Pattern, int], text: str) -> None:
    executor_receivers = java_exact_import_backed_receiver_names(text, {"Executor"})
    executor_service_receivers = java_exact_import_backed_receiver_names(
        text,
        {"ExecutorService", "ScheduledExecutorService"},
    )
    scheduled_receivers = java_exact_import_backed_receiver_names(
        text,
        {"ScheduledExecutorService"},
    )
    if executor_receivers or executor_service_receivers:
        count_by_methods(
            counts,
            JAVA_EXECUTOR_EXECUTE,
            text,
            executor_receivers | executor_service_receivers,
            ("execute",),
            ".",
        )
    if executor_service_receivers:
        count_by_methods(
            counts,
            JAVA_EXECUTOR_SUBMIT,
            text,
            executor_service_receivers,
            ("submit",),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_EXECUTOR_INVOKE_ALL,
            text,
            executor_service_receivers,
            ("invokeAll",),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_EXECUTOR_INVOKE_ANY,
            text,
            executor_service_receivers,
            ("invokeAny",),
            ".",
        )
    if scheduled_receivers:
        count_by_methods(
            counts,
            JAVA_SCHEDULED_EXECUTOR_SCHEDULE,
            text,
            scheduled_receivers,
            ("schedule",),
            ".",
        )
        count_by_methods(
            counts,
            JAVA_SCHEDULED_EXECUTOR_INTERVAL,
            text,
            scheduled_receivers,
            ("scheduleAtFixedRate", "scheduleWithFixedDelay"),
            ".",
        )


def java_exact_import_backed_receiver_names(text: str, type_names: set[str]) -> set[str]:
    imported = java_exact_imported_concurrent_types(text) & type_names
    if not imported:
        return set()
    type_pattern = "|".join(re.escape(type_name) for type_name in sorted(imported))
    pattern = re.compile(
        rf"\b(?:{type_pattern})\b(?:\s*<[^;()={{}}]*>)?(?:\s*\[\s*\])?\s+"
        rf"([A-Za-z_$][A-Za-z0-9_$]*)"
    )
    return {
        match.group(1)
        for match in pattern.finditer(text)
        if match.group(1) not in {"class", "interface", "enum", "record"}
    }


def java_exact_imported_concurrent_types(text: str) -> set[str]:
    return {
        match.group(1)
        for match in re.finditer(
            r"\bimport\s+java\s*\.\s*util\s*\.\s*concurrent\s*\.\s*"
            r"(CompletableFuture|Future|ScheduledFuture|Executor|ExecutorService|ScheduledExecutorService)"
            r"\s*;",
            text,
        )
    }


def summarize(args: argparse.Namespace) -> dict[str, Any]:
    repos = load_repos(Path(args.manifest))
    by_pattern: dict[Pattern, Counter[str]] = defaultdict(Counter)
    file_counts: dict[Pattern, Counter[str]] = defaultdict(Counter)
    language_counts: Counter[str] = Counter()
    family_counts: Counter[str] = Counter()

    for repo in repos:
        repo_id = repo["id"]
        root = Path(args.repos_root) / repo_id
        for path in source_files(root):
            language = language_for_path(path)
            if language is None:
                continue
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            rel = str(path.relative_to(root))
            for pattern, count in count_file(text, language).items():
                by_pattern[pattern][repo_id] += count
                file_counts[pattern][f"{repo_id}/{rel}"] += count
                language_counts[language] += count
                family_counts[pattern.obligation_family] += count

    surfaces = []
    for pattern, repo_counts in by_pattern.items():
        occurrences = sum(repo_counts.values())
        if occurrences == 0:
            continue
        surfaces.append(
            {
                "language": pattern.language,
                "surface": pattern.surface,
                "operation": pattern.operation,
                "status": pattern.status,
                "boundary": pattern.boundary,
                "obligation_family": pattern.obligation_family,
                "obligation_subreason": pattern.obligation_subreason,
                "occurrences": occurrences,
                "repos": len(repo_counts),
                "top_repos": [
                    {"repo": repo, "occurrences": count}
                    for repo, count in repo_counts.most_common(8)
                ],
                "top_files": [
                    {"path": path, "occurrences": count}
                    for path, count in file_counts[pattern].most_common(8)
                ],
                "note": pattern.note,
            }
        )
    surfaces.sort(key=lambda item: (-item["occurrences"], item["language"], item["operation"]))

    total_occurrences = sum(language_counts.values())
    report: dict[str, Any] = {
        "report_kind": "scheduling-lifecycle-boundary-audit",
        "schema_version": 1,
        "generated_on": args.generated_on,
        "tracking_issue": {
            "number": 602,
            "url": "https://github.com/corca-ai/nose/issues/602",
        },
        "policy": {
            "semantic_admission_delta": 0,
            "source_prevalence_only": True,
            "raw_source_snippets_included": False,
            "note": "Counts price boundary surfaces; they do not prove exact semantics.",
            "go_channel_operation_pricing": "Directional channel type arrows such as <-chan and chan<- are excluded from send/receive operation counts.",
        },
        "summary": {
            "repos_in_manifest": len(repos),
            "total_source_prevalence": total_occurrences,
            "languages": dict(sorted(language_counts.items())),
            "obligation_families": dict(sorted(family_counts.items())),
        },
        "surfaces": surfaces,
        "recommended_order": recommended_order(surfaces),
        "hard_negative_inventory": hard_negative_inventory(),
        "current_recall_loss": current_recall_loss(args.recall_loss_report),
        "regenerate": [regenerate_command(args)],
    }
    return report


def regenerate_command(args: argparse.Namespace) -> str:
    parts = ["python3", "scripts/scheduling-lifecycle-boundary-audit.py"]
    if args.manifest != DEFAULT_MANIFEST:
        parts.extend(["--manifest", args.manifest])
    if args.repos_root != DEFAULT_REPOS_ROOT:
        parts.extend(["--repos-root", args.repos_root])
    if args.recall_loss_report:
        parts.extend(["--recall-loss-report", args.recall_loss_report])
    if args.output != DEFAULT_OUTPUT:
        parts.extend(["--output", args.output])
    if args.generated_on != DEFAULT_GENERATED_ON:
        parts.extend(["--generated-on", args.generated_on])
    return " ".join(parts)


def recommended_order(surfaces: list[dict[str, Any]]) -> list[dict[str, Any]]:
    priority = {
        "promise-aggregate-all-fulfilled-contract-missing": 1,
        "promise-aggregate-first-settled-contract-missing": 2,
        "promise-executor-timing-contract-missing": 3,
        "abort-signal-cancellation-contract-missing": 4,
        "interval-async-iteration-lifecycle-contract-missing": 5,
        "goroutine-scheduling-contract-missing": 6,
        "channel-receive-value-channel-contract-missing": 7,
        "channel-send-synchronization-contract-missing": 8,
        "channel-select-readiness-contract-missing": 9,
        "channel-select-case-selection-contract-missing": 10,
        "channel-select-default-liveness-contract-missing": 11,
        "channel-receive-status-contract-missing": 12,
        "task-spawn-scheduling-contract-missing": 13,
        "async-aggregate-all-completion-contract-missing": 14,
        "async-aggregate-first-completion-contract-missing": 15,
        "async-aggregate-completion-contract-missing": 16,
        "future-settled-value-channel-contract-missing": 17,
    }
    surface_priority = {
        "swift.async.await": 18,
    }
    candidates = [
        item
        for item in surfaces
        if not item["status"].startswith("reporting-")
        and (
            item["obligation_subreason"] in priority
            or item["surface"] in surface_priority
        )
    ]

    def item_priority(item: dict[str, Any]) -> int:
        subreason = item["obligation_subreason"]
        if subreason in priority:
            return priority[subreason]
        return surface_priority[item["surface"]]

    candidates.sort(
        key=lambda item: (
            item_priority(item),
            -item["occurrences"],
            item["language"],
        )
    )
    return [
        {
            "rank": idx + 1,
            "language": item["language"],
            "operation": item["operation"],
            "obligation_family": item["obligation_family"],
            "obligation_subreason": item["obligation_subreason"],
            "occurrences": item["occurrences"],
            "repos": item["repos"],
            "why": recommended_reason(item),
            "next_action": "reporting-only split and hard-negative expansion before exact admission",
        }
        for idx, item in enumerate(candidates[:10])
    ]


def recommended_reason(item: dict[str, Any]) -> str:
    subreason = item["obligation_subreason"]
    if subreason.startswith("promise-aggregate"):
        return "Promise aggregate semantics are already corpus-priced and have adjacent first/all-settled hard negatives."
    if subreason == "promise-executor-timing-contract-missing":
        return "new Promise is high-risk because timing, resolve/reject callback, and throw-to-rejection behavior interact."
    if "abort" in subreason:
        return "Cancellation appears across JS/TS scheduling APIs and must stay separate from fulfillment/rejection recovery."
    if "interval" in subreason:
        return "Repeated emission/liveness needs lifecycle proof before interval streams can be compared exactly."
    if "channel" in subreason:
        return "Go channel protocol boundaries now split blocking, synchronization, close-status, and select readiness obligations."
    return "High-prevalence boundary surface with reusable obligation vocabulary."


def hard_negative_inventory() -> list[dict[str, Any]]:
    return [
        {
            "class": "thenable assimilation and custom Promise-like receivers",
            "evidence": "crates/nose-cli/tests/cli/semantic_boundaries.rs::query_mode_semantic_rejects_unproven_js_promise_protocol_convergence",
            "status": "mapped-existing",
        },
        {
            "class": "first-settled versus all-settled aggregate semantics",
            "evidence": "crates/nose-cli/tests/cli/semantic_boundaries.rs::query_mode_semantic_rejects_unproven_js_promise_protocol_convergence",
            "status": "mapped-existing",
        },
        {
            "class": "executor callback timing and thrown executor errors",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary.rs::promise_constructor_missing_evidence_splits_executor_obligations",
            "status": "mapped-existing",
        },
        {
            "class": "scheduler/microtask ordering versus synchronous evaluation",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary.rs::scheduler_and_interval_calls_report_timing_and_lifecycle_obligations",
            "status": "mapped-existing",
        },
        {
            "class": "interval stream liveness/cardinality",
            "evidence": "crates/nose-cli/tests/cli/commands/recall_loss_report.rs::recall_loss_report_splits_promise_protocol_boundaries",
            "status": "mapped-existing",
        },
        {
            "class": "cross-language lifecycle one-shot/reusable/materialized distinctions",
            "evidence": "docs/scheduling-channel-callback-obligations-594.md",
            "status": "mapped-doc-policy",
        },
        {
            "class": "Go direct call versus goroutine/defer scheduling and callback effects",
            "evidence": "crates/nose-cli/tests/cli/semantic_boundaries.rs::query_mode_semantic_rejects_unproven_go_concurrency_protocol_convergence and crates/nose-cli/src/verify_admission/runtime_boundary/tests.rs::go_select_defer_and_goroutine_boundaries_report_specific_obligations",
            "status": "expanded-this-slice",
        },
        {
            "class": "Go channel receive value/status, channel send, and select/default readiness boundaries",
            "evidence": "crates/nose-cli/tests/cli/semantic_boundaries.rs::query_mode_semantic_rejects_unproven_go_concurrency_protocol_convergence and crates/nose-cli/src/verify_admission/runtime_boundary/tests.rs::go_channel_protocol_boundaries_report_specific_obligations",
            "status": "expanded-this-slice",
        },
        {
            "class": "Python imported asyncio bindings shadowed by parameters, assignments, nested imports, or project-local asyncio modules",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/imported_bindings.rs::non_js_async_runtime_imported_bindings_reject_local_shadows and ::non_js_async_runtime_context_rejects_project_local_imported_bindings",
            "status": "mapped-existing",
        },
        {
            "class": "Python async iterator and async context-manager protocol boundaries versus synchronous loops, comprehensions, and with-blocks",
            "evidence": "crates/nose-cli/tests/cli/semantic_boundaries/python_async_protocol.rs::query_mode_semantic_rejects_unproven_python_async_protocol_lifecycle_convergence, ::query_mode_semantic_rejects_unproven_python_async_comprehension_convergence, crates/nose-frontend/src/python/tests.rs::async_for_preserves_source_backed_iteration_boundary, ::async_comprehension_preserves_source_backed_iteration_boundary, ::multi_clause_async_comprehension_preserves_source_backed_iteration_boundary, ::async_with_preserves_source_backed_context_boundary, and crates/nose-cli/src/verify_admission/runtime_boundary/tests.rs::python_async_lifecycle_protocols_report_specific_obligations",
            "status": "expanded-this-slice",
        },
        {
            "class": "Rust brace/direct-imported runtime bindings shadowed by parameters, lets, local macros, block scopes, other modules, or project-local runtime roots",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/imported_bindings.rs::non_js_async_runtime_imported_bindings_reject_rust_shadows_and_scopes and ::non_js_async_runtime_context_rejects_project_local_imported_bindings",
            "status": "mapped-existing",
        },
        {
            "class": "Swift structured-concurrency runtime names shadowed by local Task bindings, Task extensions, same-file task-group functions, or project-visible task-group functions",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/swift.rs::swift_structured_concurrency_rejects_local_runtime_shadows",
            "status": "mapped-existing",
        },
        {
            "class": "Java CompletableFuture static calls without exact stdlib type identity, or with local/conflicting type names",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_completable_future_static_attribution_requires_type_identity",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java CompletionStage-style receiver continuations without import-backed java.util.concurrent type-domain evidence",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_completion_stage_receiver_methods_require_import_backed_type_domain",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java Future local receivers with wildcard-only imports, reassignment, local shadows, or conflicting imports, plus wildcard-only Executor local receivers",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_local_and_this_field_receivers_require_exact_type_identity",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java Future field receivers that are implicit, non-this, member-shadowed, duplicate, or conflicting, plus conflicting Executor field receivers",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_local_and_this_field_receivers_require_exact_type_identity",
            "status": "expanded-this-slice",
        },
    ]


def current_recall_loss(path: str | None) -> dict[str, Any] | None:
    if not path:
        return None
    report_path = Path(path)
    if not report_path.exists():
        return {"report": path, "status": "missing"}
    report = json.loads(report_path.read_text())
    relevant_interpretable = relevant_recall_loss_obligations(report.get("by_obligation", []))
    oracle = report.get("oracle_exclusions", {})
    relevant_oracle_exclusions = relevant_recall_loss_obligations(
        oracle.get("by_obligation", [])
    )
    return {
        "report": path,
        "summary": report.get("summary", {}),
        "soundness_gate": report.get("soundness_gate", {}),
        "relevant_obligations": relevant_interpretable,
        "relevant_oracle_exclusion_obligations": relevant_oracle_exclusions,
    }


def relevant_recall_loss_obligations(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    relevant = []
    for item in items:
        family = item.get("obligation_family", "")
        subreason = item.get("obligation_subreason", "")
        if family in {
            "scheduling-boundary",
            "channel-boundary",
            "executor-callback",
            "success-error-result-channel",
            "cancellation-liveness-boundary",
            "lifecycle-materialization-boundary",
            "exception-channel",
            "callback-demand-effect",
        } or any(key in subreason for key in ("promise", "scheduler", "channel", "goroutine", "defer", "interval")):
            relevant.append(item)
    return relevant


def main() -> None:
    args = parse_args()
    report = summarize(args)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
