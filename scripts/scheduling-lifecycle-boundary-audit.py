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


RUBY_EXCEPTION_RAISE = Pattern(
    "ruby",
    "ruby.exception.raise",
    "raise",
    "exception-channel",
    "ruby-exception-channel-contract-missing",
    "raise exception channel",
    "Ruby raise calls expose source-backed Throw exception-channel boundaries",
    re.compile(r"(?<!\.)\braise\b"),
    "reporting-supported-closed-boundary",
)
RUBY_EXCEPTION_RESCUE = Pattern(
    "ruby",
    "ruby.exception.rescue",
    "rescue",
    "exception-channel",
    "ruby-exception-channel-contract-missing",
    "rescue exception channel",
    "Ruby rescue clauses lower to Try exception-channel boundaries",
    re.compile(r"\brescue\b"),
    "reporting-supported-closed-boundary",
)


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
    Pattern("python", "python.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "Python await has coroutine scheduling and exception channel semantics", re.compile(r"\bawait\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.async.function", "async def", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "async def creates coroutine protocol boundaries", re.compile(r"\basync\s+def\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.async.iteration", "async for", "lifecycle-materialization-boundary", "async-iteration-lifecycle-contract-missing", "async iteration lifecycle", "async for statements and comprehensions need async iterator lifecycle, value-channel, and scheduling proof", re.compile(r"\basync\s+for\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.async.context", "async with", "lifecycle-materialization-boundary", "async-context-lifecycle-contract-missing", "async context lifecycle", "async with needs async enter/exit cleanup, exception-channel, and scheduling proof", re.compile(r"\basync\s+with\b"), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.task", "asyncio.create_task/ensure_future", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio task spawn", "asyncio task APIs create scheduler, cancellation, and handle lifecycle boundaries", re.compile(r"\basyncio\s*\.\s*(?:create_task|ensure_future)\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.sleep", "asyncio.sleep", "scheduling-boundary", "timer-scheduling-contract-missing", "asyncio timer", "asyncio sleep creates a timer-backed scheduling boundary", re.compile(r"\basyncio\s*\.\s*sleep\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.gather", "asyncio.gather", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "asyncio all-completion aggregate", "asyncio gather needs all-completion, result-channel, cancellation, and exception semantics", re.compile(r"\basyncio\s*\.\s*gather\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.wait", "asyncio.wait", "success-error-result-channel", "async-aggregate-completion-contract-missing", "asyncio completion aggregate", "asyncio wait needs completion-selection, result-channel, cancellation, and exception semantics", re.compile(r"\basyncio\s*\.\s*wait\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.run", "asyncio.run", "scheduling-boundary", "future-drive-scheduling-contract-missing", "asyncio future drive", "asyncio.run drives a coroutine to completion with scheduling, result-channel, and exception boundaries", re.compile(r"\basyncio\s*\.\s*run\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.wait_for", "asyncio.wait_for", "scheduling-boundary", "timer-scheduling-contract-missing", "asyncio timeout wait", "asyncio.wait_for adds timer and cancellation/liveness boundaries around an awaitable result channel", re.compile(r"\basyncio\s*\.\s*wait_for\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.shield", "asyncio.shield", "cancellation-liveness-boundary", "task-cancellation-liveness-contract-missing", "asyncio cancellation shield", "asyncio.shield changes cancellation propagation while preserving an awaitable result channel", re.compile(r"\basyncio\s*\.\s*shield\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.run_coroutine_threadsafe", "asyncio.run_coroutine_threadsafe", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio thread-safe task submission", "asyncio.run_coroutine_threadsafe schedules a coroutine onto another loop and returns a future handle", re.compile(r"\basyncio\s*\.\s*run_coroutine_threadsafe\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.asyncio.to_thread", "asyncio.to_thread", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "asyncio thread offload", "asyncio.to_thread schedules a callback on a worker thread and returns an awaitable result channel", re.compile(r"\basyncio\s*\.\s*to_thread\s*\("), "reporting-supported-closed-boundary"),
    Pattern("python", "python.generator.yield", "yield", "lifecycle-materialization-boundary", "generator-yield-lifecycle-contract-missing", "generator lifecycle", "Python yield expressions expose source-backed generator lifecycle and protocol boundaries", re.compile(r"\byield(?:\s+from)?\b"), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.await", ".await", "scheduling-boundary", "async-await-scheduling-contract-missing", "future await", "Rust .await polls a Future and must keep wake/scheduling effects explicit", re.compile(r"\.\s*await\b"), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.function", "async fn", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "async fn creates a suspended async function boundary", re.compile(r"\basync\s+fn\b"), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.closure", "async closure", "scheduling-boundary", "async-function-scheduling-contract-missing", "async closure scheduling", "Rust async closures create suspended async callable protocol boundaries even when the surrounding function is synchronous", re.compile(r"\basync\s+(?:move\s+)?\|"), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.block", "async block", "scheduling-boundary", "async-block-scheduling-contract-missing", "async block construction", "async blocks create suspended async boundaries", re.compile(r"\basync\s+(?:move\s*)?\{"), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.spawn", "tokio/async-std spawn", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "task spawn", "async spawn APIs introduce scheduler, cancellation, and join-handle boundaries", re.compile(r"\b(?:tokio(?:::task)?|async_std::task)\s*::\s*spawn(?:_blocking)?\s*\("), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.join", "tokio/futures/futures_util join/try_join", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "future all-completion aggregate", "qualified runtime join style macros need all-completion result-channel proof", re.compile(r"\b(?:tokio|futures|futures_util)::(?:join|try_join)!\s*\("), "reporting-supported-closed-boundary"),
    Pattern("rust", "rust.async.select", "tokio/futures/futures_util select", "cancellation-liveness-boundary", "async-aggregate-first-completion-contract-missing", "future first-completion aggregate", "qualified runtime select style macros need first-completion, cancellation, and result-channel proof", re.compile(r"\b(?:tokio|futures|futures_util)::select!\s*\("), "reporting-supported-closed-boundary"),
    Pattern("go", "go.concurrent.goroutine", "go statement", "scheduling-boundary", "goroutine-scheduling-contract-missing", "goroutine scheduling", "go statements spawn concurrent execution", re.compile(r"\bgo\s+[A-Za-z_{(]"), "reporting-supported-closed-boundary"),
    Pattern("go", "go.concurrent.defer", "defer statement", "lifecycle-materialization-boundary", "defer-lifecycle-ordering-contract-missing", "defer lifecycle", "defer has scope-exit ordering and panic interaction semantics", re.compile(r"\bdefer\s+"), "reporting-supported-closed-boundary"),
    Pattern("go", "go.channel.send_receive", "channel send/receive", "channel-boundary", "channel-send-receive-protocol-contract-missing", "channel protocol", "channel send/receive has blocking and synchronization semantics", re.compile(r"<-")),
    Pattern("go", "go.channel.select", "select", "channel-boundary", "channel-select-readiness-contract-missing", "channel select", "select has readiness, default, and scheduling semantics", re.compile(r"\bselect\s*\{"), "reporting-supported-closed-boundary"),
    Pattern("java", "java.future.completable", "CompletableFuture", "success-error-result-channel", "future-settled-value-channel-contract-missing", "future channel", "CompletableFuture needs success/error channel and scheduling proof", re.compile(r"\bCompletableFuture\b(?!\s*\.\s*(?:supplyAsync|runAsync|completedFuture|completedStage|failedFuture|failedStage|allOf|anyOf)\s*\()")),
    Pattern("java", "java.future.completable.spawn", "CompletableFuture.supplyAsync/runAsync", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "future task spawn", "CompletableFuture async factories schedule executor callbacks and return future handles", re.compile(r"\bCompletableFuture\s*\.\s*(?:supplyAsync|runAsync)\s*\("), "reporting-supported-closed-boundary"),
    Pattern("java", "java.future.completable.factory", "CompletableFuture.completedFuture/failedFuture", "success-error-result-channel", "future-settled-value-channel-contract-missing", "future settled value", "CompletableFuture settled factories create fulfilled or exceptional future channels", re.compile(r"\bCompletableFuture\s*\.\s*(?:completedFuture|completedStage|failedFuture|failedStage)\s*\("), "reporting-supported-closed-boundary"),
    Pattern("java", "java.future.completable.all", "CompletableFuture.allOf", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "future all-completion aggregate", "CompletableFuture.allOf needs all-completion and exceptional completion proof", re.compile(r"\bCompletableFuture\s*\.\s*allOf\s*\("), "reporting-supported-closed-boundary"),
    Pattern("java", "java.future.completable.any", "CompletableFuture.anyOf", "cancellation-liveness-boundary", "async-aggregate-first-completion-contract-missing", "future first-completion aggregate", "CompletableFuture.anyOf needs first-completion and result-channel proof", re.compile(r"\bCompletableFuture\s*\.\s*anyOf\s*\("), "reporting-supported-closed-boundary"),
    Pattern("java", "java.future.executor", "Executor/Future", "scheduling-boundary", "java-executor-scheduling-contract-missing", "executor scheduling", "Historical lexical type-name bucket; use proof-backed Java Future, CompletionStage, Executor, and ScheduledExecutor receiver-method rows as actionable surfaces", re.compile(r"\b(?:ExecutorService|Executor|Future|ScheduledFuture)\b"), "superseded-overlap-boundary"),
    Pattern("java", "java.stream.lifecycle", "stream/parallelStream", "lifecycle-materialization-boundary", "java-stream-lifecycle-contract-missing", "stream lifecycle", "Java streams need lazy/eager lifecycle and terminal materialization proof", re.compile(r"\.\s*(?:stream|parallelStream)\s*\(")),
    Pattern("swift", "swift.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "Swift await has task scheduling and actor/lifetime boundaries", re.compile(r"\bawait\b"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.async.function", "async", "scheduling-boundary", "async-function-scheduling-contract-missing", "async function scheduling", "Swift async surfaces create task/future-like protocol boundaries", re.compile(r"\basync\b"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.async.closure", "async closure", "scheduling-boundary", "async-function-scheduling-contract-missing", "async closure scheduling", "Swift async closures create async callable protocol boundaries even when the surrounding function is synchronous", re.compile(r"(?!x)x"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.async.iteration", "for await/for try await", "lifecycle-materialization-boundary", "async-iteration-lifecycle-contract-missing", "async iteration lifecycle", "Swift async sequence loops need async iterator lifecycle, value-channel, scheduling, and throwing-channel proof", re.compile(r"\bfor\s+(?:try[!?]?\s+)?await\b"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.error.throws", "throws/try", "exception-channel", "swift-throws-exception-channel-contract-missing", "throws channel", "Historical lexical overlap bucket; use source-backed Swift try and throwing-callable rows as actionable exception-channel surfaces", re.compile(r"\b(?:throws|try)\b"), "superseded-overlap-boundary"),
    Pattern("swift", "swift.task.spawn", "Task/Task.detached", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "task scheduling", "Task APIs introduce scheduler, cancellation, and handle lifecycle boundaries", re.compile(r"\bTask(?:\s*\.\s*detached\s*(?:\{|\()|\s*\{)"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.task.sleep", "Task.sleep", "scheduling-boundary", "timer-scheduling-contract-missing", "task sleep timer", "Swift Task.sleep creates a timer-backed scheduling boundary and cancellation/liveness boundary", re.compile(r"\bTask\s*\.\s*sleep\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.task.yield", "Task.yield", "scheduling-boundary", "task-yield-scheduling-contract-missing", "task yield", "Swift Task.yield yields to the task scheduler and must not collapse into sync value equivalence", re.compile(r"\bTask\s*\.\s*yield\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.task.group", "withTaskGroup/withThrowingTaskGroup/withDiscardingTaskGroup/withThrowingDiscardingTaskGroup", "success-error-result-channel", "async-aggregate-all-completion-contract-missing", "task-group aggregate", "Swift task groups need all-completion, result-channel, cancellation/liveness, and throwing error-channel proof", re.compile(r"\bwith(?:Throwing)?(?:Discarding)?TaskGroup\s*\("), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.async.let", "async let", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "structured task binding", "Swift async let creates a child task with scheduling, handle lifecycle, cancellation/liveness, and awaited result boundaries", re.compile(r"\basync\s+let\b"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.continuation.checked", "withCheckedContinuation/withUnsafeContinuation", "success-error-result-channel", "future-settled-value-channel-contract-missing", "Swift continuation bridge", "Swift continuation bridges suspend and resume through a callback-settled future-like result channel", re.compile(r"\bwith(?:Checked|Unsafe)Continuation\s*(?:\(|\{)"), "reporting-supported-closed-boundary"),
    Pattern("swift", "swift.continuation.throwing", "withCheckedThrowingContinuation/withUnsafeThrowingContinuation", "success-error-result-channel", "future-settled-value-channel-contract-missing", "Swift throwing continuation bridge", "Swift throwing continuation bridges add exception-channel behavior to callback-settled future-like result channels", re.compile(r"\bwith(?:Checked|Unsafe)ThrowingContinuation\s*(?:\(|\{)"), "reporting-supported-closed-boundary"),
    Pattern("ruby", "ruby.thread.fiber", "Thread/Fiber", "scheduling-boundary", "task-spawn-scheduling-contract-missing", "thread/fiber scheduling", "Thread/Fiber APIs create scheduler and lifecycle boundaries", re.compile(r"\b(?:Thread\s*\.\s*(?:new|start|fork)|Fiber\s*\.\s*(?:new|schedule))\b"), "reporting-supported-closed-boundary"),
    RUBY_EXCEPTION_RAISE,
    RUBY_EXCEPTION_RESCUE,
    Pattern("ruby", "ruby.exception", "raise/rescue", "exception-channel", "ruby-exception-channel-contract-missing", "exception channel", "Historical lexical overlap bucket; use source-backed Ruby raise and rescue rows as actionable exception-channel surfaces", re.compile(r"\b(?:raise|rescue)\b"), "superseded-overlap-boundary"),
    Pattern("ruby", "ruby.block.yield", "yield", "callback-demand-effect", "ruby-yield-callback-demand-effect-contract-missing", "block callback", "Ruby yield invokes a block with demand/effect obligations", re.compile(r"\byield\b"), "reporting-supported-closed-boundary"),
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
)
JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR = Pattern(
    "java",
    "java.future.completable.constructor",
    "new CompletableFuture",
    "success-error-result-channel",
    "future-settled-value-channel-contract-missing",
    "future constructor channel",
    "Import- or qualified-name-backed Java CompletableFuture constructors create manual settlement future channels",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_FUTURE_HANDLE_GET = Pattern(
    "java",
    "java.future.handle.get",
    "Future.get",
    "success-error-result-channel",
    "future-settled-value-channel-contract-missing",
    "future handle get",
    "Import-backed Java Future receivers expose blocking settled-value and exception channels",
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
    "Import-backed Java Future receivers expose cancellation and task-handle liveness boundaries",
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
    "Import-backed Java Future receivers expose task-handle lifecycle status",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
JAVA_STREAM_RECEIVER_ADAPTER = Pattern(
    "java",
    "java.stream.receiver_adapter",
    "receiver.stream",
    "lifecycle-materialization-boundary",
    "java-stream-lifecycle-contract-missing",
    "stream lifecycle",
    "Typed collection/set receiver.stream() calls are already admitted through iterator identity adapter occurrence proof",
    re.compile(r"(?!x)x"),
    "exact-supported-boundary",
)
JAVA_STREAM_STATIC_ARRAYS_ADAPTER = Pattern(
    "java",
    "java.stream.arrays_adapter",
    "Arrays.stream",
    "lifecycle-materialization-boundary",
    "java-stream-lifecycle-contract-missing",
    "stream lifecycle",
    "Exact-import or fully qualified Arrays.stream(xs) calls are already admitted through static collection adapter occurrence proof",
    re.compile(r"(?!x)x"),
    "exact-supported-boundary",
)
JAVA_EXECUTOR_EXECUTE = Pattern(
    "java",
    "java.executor.execute",
    "Executor.execute",
    "scheduling-boundary",
    "task-spawn-scheduling-contract-missing",
    "executor runnable scheduling",
    "Import-backed Java Executor receivers schedule callback execution without returning a task handle",
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
    "Import-backed Java ExecutorService receivers schedule callbacks and return Future handles",
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
    "Import-backed Java ExecutorService invokeAll waits for all submitted tasks and returns Future result channels",
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
    "Import-backed Java ExecutorService invokeAny exposes first-success completion, cancellation, and exception boundaries",
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
    "Import-backed Java ScheduledExecutorService schedule calls add timer-backed task scheduling and Future result channels",
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
    "Import-backed Java ScheduledExecutorService repeating schedules expose interval lifecycle and cancellation boundaries",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
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
    "reporting-supported-closed-boundary",
)
SWIFT_THROWING_FUNCTION = Pattern(
    "swift",
    "swift.error.throwing_function",
    "func/init/subscript throws/rethrows",
    "exception-channel",
    "exception-channel-contract-missing",
    "throwing callable error channel",
    "Swift body-bearing throwing functions expose an error channel even when the body has no explicit try expression",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
SWIFT_THROWING_CLOSURE = Pattern(
    "swift",
    "swift.error.throwing_closure",
    "closure throws/rethrows",
    "exception-channel",
    "exception-channel-contract-missing",
    "throwing closure error channel",
    "Swift throwing closures expose the same error-channel obligation as throwing functions and async throwing closures",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)
SWIFT_TRY_EXPRESSION = Pattern(
    "swift",
    "swift.error.try_expression",
    "try/try?/try!",
    "exception-channel",
    "exception-channel-contract-missing",
    "try propagation error channel",
    "Swift try expressions and for-try-await loops expose source-backed TryPropagation boundaries",
    re.compile(r"(?!x)x"),
    "reporting-supported-closed-boundary",
)


def all_known_patterns() -> tuple[Pattern, ...]:
    return PATTERNS + (
        PYTHON_ASYNCIO_ALIAS_TASK,
        PYTHON_ASYNCIO_ALIAS_SLEEP,
        PYTHON_ASYNCIO_ALIAS_GATHER,
        PYTHON_ASYNCIO_ALIAS_WAIT,
        PYTHON_ASYNCIO_ALIAS_RUN,
        PYTHON_ASYNCIO_ALIAS_WAIT_FOR,
        PYTHON_ASYNCIO_ALIAS_SHIELD,
        PYTHON_ASYNCIO_ALIAS_THREADSAFE,
        PYTHON_ASYNCIO_ALIAS_TO_THREAD,
        PYTHON_ASYNCIO_IMPORTED_TASK,
        PYTHON_ASYNCIO_IMPORTED_SLEEP,
        PYTHON_ASYNCIO_IMPORTED_GATHER,
        PYTHON_ASYNCIO_IMPORTED_WAIT,
        PYTHON_ASYNCIO_IMPORTED_RUN,
        PYTHON_ASYNCIO_IMPORTED_WAIT_FOR,
        PYTHON_ASYNCIO_IMPORTED_SHIELD,
        PYTHON_ASYNCIO_IMPORTED_THREADSAFE,
        PYTHON_ASYNCIO_IMPORTED_TO_THREAD,
        RUST_IMPORTED_ASYNC_SPAWN,
        RUST_IMPORTED_ASYNC_JOIN,
        RUST_IMPORTED_ASYNC_SELECT,
        JAVA_FUTURE_FULFILLMENT_CONTINUATION,
        JAVA_FUTURE_EXCEPTION_CONTINUATION,
        JAVA_FUTURE_SETTLEMENT_CONTINUATION,
        JAVA_FUTURE_ALL_COMPLETION_CONTINUATION,
        JAVA_FUTURE_FIRST_COMPLETION_CONTINUATION,
        JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR,
        JAVA_FUTURE_HANDLE_GET,
        JAVA_FUTURE_HANDLE_CANCEL,
        JAVA_FUTURE_HANDLE_STATUS,
        JAVA_STREAM_RECEIVER_ADAPTER,
        JAVA_STREAM_STATIC_ARRAYS_ADAPTER,
        JAVA_EXECUTOR_EXECUTE,
        JAVA_EXECUTOR_SUBMIT,
        JAVA_EXECUTOR_INVOKE_ALL,
        JAVA_EXECUTOR_INVOKE_ANY,
        JAVA_SCHEDULED_EXECUTOR_SCHEDULE,
        JAVA_SCHEDULED_EXECUTOR_INTERVAL,
        GO_CHANNEL_SEND,
        GO_CHANNEL_RECEIVE,
        GO_CHANNEL_RECEIVE_STATUS,
        GO_CHANNEL_SELECT_CASE,
        GO_CHANNEL_SELECT_DEFAULT,
        SWIFT_THROWING_FUNCTION,
        SWIFT_THROWING_CLOSURE,
        SWIFT_TRY_EXPRESSION,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--manifest", default=DEFAULT_MANIFEST)
    parser.add_argument("--repos-root", default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    parser.add_argument("--generated-on", default=DEFAULT_GENERATED_ON)
    parser.add_argument("--recall-loss-report", default=None)
    parser.add_argument(
        "--include-zero-surface",
        action="append",
        default=[],
        help="Emit an explicitly searched surface even when the occurrence count is zero.",
    )
    return parser.parse_args()


def self_test() -> None:
    exact_future = count_file(
        "import java.util.concurrent.Future;\n"
        "class T { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        "java",
    )
    assert exact_future.get(JAVA_FUTURE_HANDLE_GET) == 1

    wildcard_submit = count_file(
        "import java.util.concurrent.*;\n"
        "class T { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        "java",
    )
    assert wildcard_submit.get(JAVA_EXECUTOR_SUBMIT) == 1

    java_future_executor_overlap = next(
        item for item in PATTERNS if item.surface == "java.future.executor"
    )
    assert java_future_executor_overlap.status == "superseded-overlap-boundary"

    exact_conflict = count_file(
        "import java.util.concurrent.Future;\n"
        "import example.Future;\n"
        "class T { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        "java",
    )
    assert JAVA_FUTURE_HANDLE_GET not in exact_conflict

    exact_shadow = count_file(
        "import java.util.concurrent.ExecutorService;\n"
        "class ExecutorService { Object submit(Object task) { return null; } }\n"
        "class T { Object run(ExecutorService executor) { return executor.submit(() -> work()); } }\n",
        "java",
    )
    assert JAVA_EXECUTOR_SUBMIT not in exact_shadow

    wildcard_conflict = count_file(
        "import java.util.concurrent.*;\n"
        "import example.Future;\n"
        "class T { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        "java",
    )
    assert JAVA_FUTURE_HANDLE_GET not in wildcard_conflict

    wildcard_shadow = count_file(
        "import java.util.concurrent.*;\n"
        "class Future<T> { Object get() { return null; } }\n"
        "class T { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        "java",
    )
    assert JAVA_FUTURE_HANDLE_GET not in wildcard_shadow

    java_completable_broad = next(
        item for item in PATTERNS if item.surface == "java.future.completable"
    )
    exact_completable_constructor = count_file(
        "import java.util.concurrent.CompletableFuture;\n"
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
    )
    assert exact_completable_constructor.get(JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR) == 1
    assert exact_completable_constructor.get(java_completable_broad) == 1

    wildcard_completable_constructor = count_file(
        "import java.util.concurrent.*;\n"
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
    )
    assert wildcard_completable_constructor.get(JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR) == 1

    wildcard_package_shadow_completable_constructor = count_file(
        "import java.util.concurrent.*;\n"
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
        {"CompletableFuture"},
    )
    assert (
        JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR
        not in wildcard_package_shadow_completable_constructor
    )
    assert wildcard_package_shadow_completable_constructor.get(java_completable_broad) == 1

    qualified_completable_constructor = count_file(
        "class T { Object run() { return new java.util.concurrent.CompletableFuture<String>(); } }\n",
        "java",
    )
    assert qualified_completable_constructor.get(JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR) == 1

    unimported_completable_constructor = count_file(
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
    )
    assert JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR not in unimported_completable_constructor
    assert unimported_completable_constructor.get(java_completable_broad) == 1

    java_stream_broad = next(
        item for item in PATTERNS if item.surface == "java.stream.lifecycle"
    )
    typed_stream = count_file(
        "class T { Object run(java.util.List<String> values) { return values.stream(); } }\n",
        "java",
    )
    assert typed_stream.get(JAVA_STREAM_RECEIVER_ADAPTER) == 1
    assert java_stream_broad not in typed_stream

    parallel_stream = count_file(
        "class T { Object run(java.util.List<String> values) { return values.parallelStream(); } }\n",
        "java",
    )
    assert JAVA_STREAM_RECEIVER_ADAPTER not in parallel_stream
    assert parallel_stream.get(java_stream_broad) == 1

    untyped_stream = count_file(
        "class T { Object run(Object values) { return values.stream(); } }\n",
        "java",
    )
    assert JAVA_STREAM_RECEIVER_ADAPTER not in untyped_stream
    assert untyped_stream.get(java_stream_broad) == 1

    same_name_other_scope_stream = count_file(
        "class T {\n"
        "  Object first(java.util.List<String> values) { return values; }\n"
        "  Object second(Object values) { return values.stream(); }\n"
        "}\n",
        "java",
    )
    assert JAVA_STREAM_RECEIVER_ADAPTER not in same_name_other_scope_stream
    assert same_name_other_scope_stream.get(java_stream_broad) == 1

    imported_arrays_stream = count_file(
        "import java.util.Arrays;\n"
        "class T { Object run(String[] values) { return Arrays.stream(values); } }\n",
        "java",
    )
    assert imported_arrays_stream.get(JAVA_STREAM_STATIC_ARRAYS_ADAPTER) == 1
    assert java_stream_broad not in imported_arrays_stream

    qualified_arrays_stream = count_file(
        "class T { Object run(String[] values) { return java.util.Arrays.stream(values); } }\n",
        "java",
    )
    assert qualified_arrays_stream.get(JAVA_STREAM_STATIC_ARRAYS_ADAPTER) == 1
    assert java_stream_broad not in qualified_arrays_stream

    shadowed_arrays_stream = count_file(
        "import java.util.Arrays;\n"
        "class Arrays { static Object stream(Object value) { return value; } }\n"
        "class T { Object run(String[] values) { return Arrays.stream(values); } }\n",
        "java",
    )
    assert JAVA_STREAM_STATIC_ARRAYS_ADAPTER not in shadowed_arrays_stream
    assert shadowed_arrays_stream.get(java_stream_broad) == 1

    value_shadowed_arrays_stream = count_file(
        "import java.util.Arrays;\n"
        "class FakeArrays { Object stream(Object value) { return value; } }\n"
        "class T { Object run(FakeArrays Arrays, String[] values) { return Arrays.stream(values); } }\n",
        "java",
    )
    assert JAVA_STREAM_STATIC_ARRAYS_ADAPTER not in value_shadowed_arrays_stream
    assert value_shadowed_arrays_stream.get(java_stream_broad) == 1

    lambda_shadowed_arrays_stream = count_file(
        "import java.util.Arrays;\n"
        "import java.util.function.Function;\n"
        "class FakeArrays { Object stream(Object value) { return value; } }\n"
        "class T { Object run(String[] values) { Function<FakeArrays, Object> f = Arrays -> Arrays.stream(values); return f; } }\n",
        "java",
    )
    assert JAVA_STREAM_STATIC_ARRAYS_ADAPTER not in lambda_shadowed_arrays_stream
    assert lambda_shadowed_arrays_stream.get(java_stream_broad) == 1

    ranged_arrays_stream = count_file(
        "import java.util.Arrays;\n"
        "class T { Object run(String[] values) { return Arrays.stream(values, 0, 1); } }\n",
        "java",
    )
    assert JAVA_STREAM_STATIC_ARRAYS_ADAPTER not in ranged_arrays_stream
    assert ranged_arrays_stream.get(java_stream_broad) == 1

    conflict_completable_constructor = count_file(
        "import java.util.concurrent.*;\n"
        "import example.CompletableFuture;\n"
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
    )
    assert JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR not in conflict_completable_constructor

    shadow_completable_constructor = count_file(
        "import java.util.concurrent.CompletableFuture;\n"
        "class CompletableFuture<T> {}\n"
        "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        "java",
    )
    assert JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR not in shadow_completable_constructor

    wildcard_package_shadow_future_receiver = count_file(
        "import java.util.concurrent.*;\n"
        "class T { Object run(Future<String> future) throws Exception { return future.get(); } }\n",
        "java",
        {"Future"},
    )
    assert JAVA_FUTURE_HANDLE_GET not in wildcard_package_shadow_future_receiver

    top_level_types = java_top_level_type_names(
        "class Top { class Nested {} void f() { class Local {} } }\n"
        "record Other(int value) {}\n"
    )
    assert top_level_types == {"Top", "Other"}
    assert (
        java_package_key(
            Path("src2/p/Runtime.java"),
            "package p;\nimport java.util.concurrent.*;\nclass Runtime {}\n",
        )
        == "package:p"
    )

    swift_throwing = count_file(
        "func f() throws -> Int { 1 }\n"
        "init(value: Int) rethrows { try setup(value) }\n"
        "func typed() throws(Failure) -> Int { 1 }\n"
        "let handler = { request async throws -> Response in try await request.load() }\n"
        "let typedHandler = { () throws(Failure) in try load() }\n",
        "swift",
    )
    assert swift_throwing.get(SWIFT_THROWING_FUNCTION) == 3
    assert swift_throwing.get(SWIFT_THROWING_CLOSURE) == 2
    assert swift_throwing.get(SWIFT_TRY_EXPRESSION) == 3

    swift_type_only = count_file(
        "let factory: (@escaping () async throws -> Void) -> Void = { closure in closure }\n"
        "func accepts(_ body: () throws(Failure) -> Int) -> Int { 1 }\n"
        "func returns() -> () throws(Failure) -> Int { { 1 } }\n",
        "swift",
    )
    assert SWIFT_THROWING_FUNCTION not in swift_type_only
    assert SWIFT_THROWING_CLOSURE not in swift_type_only
    assert SWIFT_TRY_EXPRESSION not in swift_type_only

    swift_try_expressions = count_file(
        "func run(_ stream: AsyncThrowingStream<Int, Error>) async throws {\n"
        "  let value = try load()\n"
        "  let optional = try? maybe()\n"
        "  let forced = try! definitely()\n"
        "  for try await item in stream { print(item) }\n"
        "}\n"
        "let tryawait = 1\n",
        "swift",
    )
    assert swift_try_expressions.get(SWIFT_TRY_EXPRESSION) == 4

    swift_async_function_pattern = next(
        item for item in PATTERNS if item.surface == "swift.async.function"
    )
    swift_async_functions = count_file(
        "func f() async -> Int { 1 }\n"
        "func g() async throws -> Int { try await work() }\n"
        "init(value: Int) async { self.init() }\n"
        "func accepts(_ body: () async -> Int) -> Int { 1 }\n"
        "func returns() -> () async -> Int { { 1 } }\n",
        "swift",
    )
    assert swift_async_functions.get(swift_async_function_pattern) == 3
    swift_async_type_only = count_file(
        "let factory: (@escaping () async throws -> Void) -> Void = { closure in closure }\n"
        "func accepts(_ body: () async -> Int) -> Int { 1 }\n"
        "func returns() -> () async -> Int { { 1 } }\n",
        "swift",
    )
    assert swift_async_function_pattern not in swift_async_type_only

    ruby_yield = count_file(
        "def render(value)\n"
        "  yield value\n"
        "end\n"
        "text = 'yield ignored in strings'\n",
        "ruby",
    )
    ruby_yield_pattern = next(item for item in PATTERNS if item.surface == "ruby.block.yield")
    assert ruby_yield.get(ruby_yield_pattern) == 1
    assert ruby_yield_pattern.status == "reporting-supported-closed-boundary"

    ruby_exception = count_file(
        "def f(error)\n"
        "  raise error\n"
        "rescue Error\n"
        "  recover\n"
        "end\n"
        "raise_error(:not_a_raise)\n",
        "ruby",
    )
    ruby_exception_overlap = next(item for item in PATTERNS if item.surface == "ruby.exception")
    assert ruby_exception.get(RUBY_EXCEPTION_RAISE) == 1
    assert ruby_exception.get(RUBY_EXCEPTION_RESCUE) == 1
    assert ruby_exception.get(ruby_exception_overlap) == 2
    assert RUBY_EXCEPTION_RAISE.status == "reporting-supported-closed-boundary"
    assert RUBY_EXCEPTION_RESCUE.status == "reporting-supported-closed-boundary"
    assert ruby_exception_overlap.status == "superseded-overlap-boundary"

    task_spawn_reporting_surfaces = {
        "python.asyncio.task",
        "rust.async.spawn",
        "swift.task.spawn",
        "java.future.completable.spawn",
    }
    for surface in task_spawn_reporting_surfaces:
        pattern = next(item for item in PATTERNS if item.surface == surface)
        assert pattern.status == "reporting-supported-closed-boundary", surface

    async_aggregate_reporting_surfaces = {
        "python.asyncio.gather",
        "python.asyncio.wait",
        "rust.async.join",
        "rust.async.select",
        "java.future.completable.all",
        "java.future.completable.any",
    }
    for surface in async_aggregate_reporting_surfaces:
        pattern = next(item for item in PATTERNS if item.surface == surface)
        assert pattern.status == "reporting-supported-closed-boundary", surface

    settled_future_and_await_reporting_surfaces = {
        "java.future.completable.factory",
        "python.asyncio.sleep",
        "swift.async.await",
    }
    for surface in settled_future_and_await_reporting_surfaces:
        pattern = next(item for item in PATTERNS if item.surface == surface)
        assert pattern.status == "reporting-supported-closed-boundary", surface

    java_future_continuation_reporting_surfaces = {
        "java.future.completion_stage.fulfillment",
        "java.future.completion_stage.exception",
        "java.future.completion_stage.settlement",
        "java.future.completion_stage.all",
        "java.future.completion_stage.first",
    }
    for surface in java_future_continuation_reporting_surfaces:
        pattern = next(item for item in all_known_patterns() if item.surface == surface)
        assert pattern.status == "reporting-supported-closed-boundary", surface

    source_protocol_reporting_surfaces = {
        "python.async.await",
        "python.async.function",
        "python.generator.yield",
        "rust.async.await",
        "rust.async.function",
        "rust.async.block",
        "swift.async.function",
    }
    for surface in source_protocol_reporting_surfaces:
        pattern = next(item for item in PATTERNS if item.surface == surface)
        assert pattern.status == "reporting-supported-closed-boundary", surface
    assert SWIFT_TRY_EXPRESSION.status == "reporting-supported-closed-boundary"
    swift_error_overlap = next(
        item for item in PATTERNS if item.surface == "swift.error.throws"
    )
    assert swift_error_overlap.status == "superseded-overlap-boundary"

    future_channel_reason = recommended_reason(
        {
            "obligation_subreason": "future-settled-value-channel-contract-missing",
        }
    )
    assert "Go channel" not in future_channel_reason


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


def count_file(
    text: str,
    language: str,
    java_package_local_types: set[str] | None = None,
) -> dict[Pattern, int]:
    masked = mask_comments_and_strings(text)
    counts: dict[Pattern, int] = {}
    for pattern in PATTERNS:
        if pattern.language != language:
            continue
        if pattern.surface in {
            "swift.async.function",
            "swift.async.closure",
            "swift.error.throwing_function",
            "swift.error.throwing_closure",
        }:
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
        java_completable_pattern = next(
            item for item in PATTERNS if item.surface == "java.future.completable"
        )
        java_stream_pattern = next(
            item for item in PATTERNS if item.surface == "java.stream.lifecycle"
        )
        counts.pop(java_completable_pattern, None)
        counts.pop(java_stream_pattern, None)
        counts.update(
            java_completable_future_counts(
                masked, java_completable_pattern, java_package_local_types
            )
        )
        counts.update(java_future_receiver_counts(masked, java_package_local_types))
        counts.update(
            java_stream_lifecycle_counts(
                masked, java_stream_pattern, java_package_local_types
            )
        )
    elif language == "swift":
        counts.update(swift_async_function_counts(masked))
        counts.update(swift_async_closure_counts(masked))
        counts.update(swift_throwing_callable_counts(masked))
        counts.update(swift_try_expression_counts(masked))
    return counts


def java_completable_future_counts(
    text: str,
    broad_pattern: Pattern,
    package_local_types: set[str] | None = None,
) -> dict[Pattern, int]:
    accepted_constructor_starts = java_completable_future_constructor_name_starts(
        text, package_local_types
    )
    broad_count = 0
    for match in re.finditer(
        r"\bCompletableFuture\b"
        r"(?!\s*\.\s*(?:supplyAsync|runAsync|completedFuture|completedStage|failedFuture|failedStage|allOf|anyOf)\s*\()",
        text,
    ):
        if match.start() in accepted_constructor_starts:
            continue
        broad_count += 1

    counts: dict[Pattern, int] = {}
    if broad_count:
        counts[broad_pattern] = broad_count
    if accepted_constructor_starts:
        counts[JAVA_COMPLETABLE_FUTURE_CONSTRUCTOR] = len(accepted_constructor_starts)
    return counts


def java_completable_future_constructor_name_starts(
    text: str,
    package_local_types: set[str] | None = None,
) -> set[int]:
    starts: set[int] = set()
    qualified = re.compile(
        r"\bnew\s+java\s*\.\s*util\s*\.\s*concurrent\s*\.\s*"
        r"(CompletableFuture)\b(?:\s*<[^;(){}]*>)?\s*\("
    )
    starts.update(match.start(1) for match in qualified.finditer(text))

    if "CompletableFuture" not in java_imported_concurrent_types(
        text, {"CompletableFuture"}, package_local_types
    ):
        return starts
    simple = re.compile(r"\bnew\s+(CompletableFuture)\b(?:\s*<[^;(){}]*>)?\s*\(")
    starts.update(match.start(1) for match in simple.finditer(text))
    return starts


def java_stream_lifecycle_counts(
    text: str,
    broad_pattern: Pattern,
    package_local_types: set[str] | None = None,
) -> dict[Pattern, int]:
    broad_dot_starts = {
        match.start()
        for match in re.finditer(r"\.\s*(?:stream|parallelStream)\s*\(", text)
    }
    receiver_dot_starts = java_stream_receiver_adapter_dot_starts(text)
    static_dot_starts = java_static_arrays_stream_adapter_dot_starts(
        text, package_local_types
    )
    proof_backed_dot_starts = receiver_dot_starts | static_dot_starts
    residual_count = sum(1 for start in broad_dot_starts if start not in proof_backed_dot_starts)

    counts: dict[Pattern, int] = {}
    if residual_count:
        counts[broad_pattern] = residual_count
    if receiver_dot_starts:
        counts[JAVA_STREAM_RECEIVER_ADAPTER] = len(receiver_dot_starts)
    if static_dot_starts:
        counts[JAVA_STREAM_STATIC_ARRAYS_ADAPTER] = len(static_dot_starts)
    return counts


JAVA_IDENTIFIER_RE = r"[A-Za-z_$][A-Za-z0-9_$]*"
JAVA_COLLECTION_OR_SET_TYPES = {
    "Collection",
    "List",
    "ArrayList",
    "LinkedList",
    "Deque",
    "Queue",
    "Set",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
}
JAVA_VALUE_BINDING_TYPE_KEYWORDS = {
    "assert",
    "case",
    "default",
    "else",
    "for",
    "if",
    "new",
    "return",
    "switch",
    "throw",
    "try",
    "while",
}
JAVA_VALUE_BINDING_PATTERN = re.compile(
    rf"(?:@[A-Za-z_$][A-Za-z0-9_$]*(?:\s*\([^)]*\))?\s+)*"
    rf"(?:(?:final|volatile|transient)\s+)*"
    rf"(?P<type>(?:{JAVA_IDENTIFIER_RE}\s*\.\s*)*{JAVA_IDENTIFIER_RE})"
    rf"(?:\s*<[^;()={{}}]*>)?(?:\s*\[\s*\])?\s+"
    rf"(?P<name>{JAVA_IDENTIFIER_RE})\b"
)


def java_stream_receiver_adapter_dot_starts(text: str) -> set[int]:
    starts: set[int] = set()
    pattern = re.compile(
        rf"\b(?P<receiver>{JAVA_IDENTIFIER_RE})\s*(?P<dot>\.)\s*stream\s*\("
    )
    for match in pattern.finditer(text):
        if java_call_arity(text, match.end() - 1) != 0:
            continue
        receiver = match.group("receiver")
        binding = java_latest_value_binding_supported_collection(
            text, match.start("dot"), receiver
        )
        if binding is True:
            starts.add(match.start("dot"))
    return starts


def java_latest_value_binding_supported_collection(
    text: str,
    before: int,
    name: str,
) -> bool | None:
    region = java_enclosing_method_like_region(text, before)
    if region is None:
        return None
    latest: tuple[int, bool] | None = None
    for match in JAVA_VALUE_BINDING_PATTERN.finditer(text, region[0], before):
        if match.group("name") != name:
            continue
        if match.group("name") in {"class", "interface", "enum", "record"}:
            continue
        if java_binding_type_is_keyword(match.group("type")):
            continue
        if java_binding_is_callable_declaration(text, match.end("name")):
            continue
        supported = java_collection_or_set_type_name(match.group("type")) is not None
        latest = (match.start("name"), supported)
    return latest[1] if latest is not None else None


def java_static_arrays_stream_adapter_dot_starts(
    text: str,
    package_local_types: set[str] | None = None,
) -> set[int]:
    starts: set[int] = set()
    for match in re.finditer(
        r"\bjava\s*\.\s*util\s*\.\s*Arrays\s*(?P<dot>\.)\s*stream\s*\(",
        text,
    ):
        if java_call_arity(text, match.end() - 1) == 1:
            starts.add(match.start("dot"))

    if java_exact_arrays_import_available(text, package_local_types):
        for match in re.finditer(r"\bArrays\s*(?P<dot>\.)\s*stream\s*\(", text):
            dot_start = match.start("dot")
            if java_call_arity(text, match.end() - 1) != 1:
                continue
            if java_value_binding_visible_before(text, dot_start, "Arrays"):
                continue
            if java_lambda_parameter_visible_before(text, dot_start, "Arrays"):
                continue
            starts.add(dot_start)
    return starts


def java_exact_arrays_import_available(
    text: str,
    package_local_types: set[str] | None = None,
) -> bool:
    if "Arrays" in java_local_type_names(text) or "Arrays" in (package_local_types or set()):
        return False
    if re.search(
        r"\bimport\s+(?!static\b)(?!java\s*\.\s*util\s*\.\s*Arrays\s*;)"
        r"[A-Za-z_$][A-Za-z0-9_$]*(?:\s*\.\s*[A-Za-z_$][A-Za-z0-9_$]*)*"
        r"\s*\.\s*Arrays\s*;",
        text,
    ):
        return False
    return re.search(r"\bimport\s+java\s*\.\s*util\s*\.\s*Arrays\s*;", text) is not None


def java_call_arity(text: str, open_paren: int) -> int | None:
    if open_paren >= len(text) or text[open_paren] != "(":
        return None
    paren_depth = 0
    bracket_depth = 0
    brace_depth = 0
    saw_argument = False
    comma_count = 0
    idx = open_paren + 1
    while idx < len(text):
        current = text[idx]
        if current == "(":
            paren_depth += 1
        elif current == ")":
            if paren_depth == 0:
                return comma_count + 1 if saw_argument else 0
            paren_depth -= 1
        elif current == "[":
            bracket_depth += 1
        elif current == "]":
            bracket_depth = max(0, bracket_depth - 1)
        elif current == "{":
            brace_depth += 1
        elif current == "}":
            brace_depth = max(0, brace_depth - 1)
        elif (
            current == ","
            and paren_depth == 0
            and bracket_depth == 0
            and brace_depth == 0
        ):
            comma_count += 1
        elif not current.isspace():
            saw_argument = True
        idx += 1
    return None


def java_value_binding_visible_before(text: str, before: int, name: str) -> bool:
    region = java_enclosing_method_like_region(text, before)
    if region is None:
        return False
    for match in JAVA_VALUE_BINDING_PATTERN.finditer(text, region[0], before):
        if match.group("name") != name:
            continue
        if java_binding_type_is_keyword(match.group("type")):
            continue
        if java_binding_is_callable_declaration(text, match.end("name")):
            continue
        return True
    return False


def java_lambda_parameter_visible_before(text: str, before: int, name: str) -> bool:
    region = java_enclosing_method_like_region(text, before)
    if region is None:
        return False
    segment = text[region[0] : before]
    if re.search(rf"\b{re.escape(name)}\s*->", segment):
        return True
    for match in re.finditer(r"\(([^()]*)\)\s*->", segment):
        if java_lambda_parameter_header_contains(match.group(1), name):
            return True
    return False


def java_lambda_parameter_header_contains(header: str, name: str) -> bool:
    for parameter in header.split(","):
        names = re.findall(JAVA_IDENTIFIER_RE, parameter)
        if names and names[-1] == name:
            return True
    return False


def java_binding_is_callable_declaration(text: str, name_end: int) -> bool:
    idx = name_end
    while idx < len(text) and text[idx].isspace():
        idx += 1
    return idx < len(text) and text[idx] == "("


def java_binding_type_is_keyword(type_text: str) -> bool:
    return re.sub(r"\s+", "", type_text) in JAVA_VALUE_BINDING_TYPE_KEYWORDS


def java_collection_or_set_type_name(type_text: str) -> str | None:
    normalized = re.sub(r"\s+", "", type_text)
    if normalized.startswith("java.util."):
        normalized = normalized[len("java.util.") :]
    return normalized if normalized in JAVA_COLLECTION_OR_SET_TYPES else None


def java_enclosing_method_like_region(text: str, index: int) -> tuple[int, int] | None:
    stack: list[int] = []
    for match in re.finditer(r"[{}]", text[:index]):
        if match.group(0) == "{":
            stack.append(match.start())
        elif stack:
            stack.pop()
    for open_brace in reversed(stack):
        header_start = java_header_start_before_brace(text, open_brace)
        header = text[header_start:open_brace].strip()
        if java_header_looks_like_method_body(header):
            return (header_start, open_brace + 1)
    return None


def java_header_start_before_brace(text: str, open_brace: int) -> int:
    idx = open_brace - 1
    while idx >= 0 and text[idx] not in ";{}":
        idx -= 1
    return idx + 1


def java_header_looks_like_method_body(header: str) -> bool:
    if not header or "(" not in header or ")" not in header or "->" in header:
        return False
    stripped = header.strip()
    first = re.match(
        r"(?:@\w+(?:\([^)]*\))?\s+)*"
        r"(?:public|private|protected|static|final|native|"
        r"synchronized|abstract|default|strictfp|\s)*"
        r"([A-Za-z_$][A-Za-z0-9_$]*)",
        stripped,
    )
    if first and first.group(1) in {
        "if",
        "for",
        "while",
        "switch",
        "catch",
        "synchronized",
        "try",
        "else",
        "do",
        "finally",
        "new",
    }:
        return False
    return True


def swift_async_closure_counts(text: str) -> dict[Pattern, int]:
    pattern = next(item for item in PATTERNS if item.surface == "swift.async.closure")
    count = 0
    for header in iter_swift_closure_headers(text):
        if swift_closure_header_has_top_level_async_modifier(header):
            count += 1
    return {pattern: count} if count else {}


def swift_async_function_counts(text: str) -> dict[Pattern, int]:
    pattern = next(item for item in PATTERNS if item.surface == "swift.async.function")
    count = sum(
        1
        for signature in iter_swift_body_bearing_callable_signatures(text)
        if swift_callable_signature_has_top_level_async_modifier(signature)
    )
    return {pattern: count} if count else {}


def swift_throwing_callable_counts(text: str) -> dict[Pattern, int]:
    counts: dict[Pattern, int] = {}
    function_count = sum(
        1
        for signature in iter_swift_body_bearing_callable_signatures(text)
        if swift_callable_signature_has_top_level_throwing_modifier(signature)
    )
    closure_count = sum(
        1
        for header in iter_swift_closure_headers(text)
        if swift_closure_header_has_top_level_throwing_modifier(header)
    )
    if function_count:
        counts[SWIFT_THROWING_FUNCTION] = function_count
    if closure_count:
        counts[SWIFT_THROWING_CLOSURE] = closure_count
    return counts


def swift_try_expression_counts(text: str) -> dict[Pattern, int]:
    count = sum(1 for _ in re.finditer(r"\btry\b[!?]?", text))
    return {SWIFT_TRY_EXPRESSION: count} if count else {}


def iter_swift_body_bearing_callable_signatures(text: str):
    for match in re.finditer(r"\b(?:func|init|subscript)\b", text):
        paren_depth = 0
        bracket_depth = 0
        brace_depth = 0
        idx = match.end()
        while idx < len(text):
            current = text[idx]
            if paren_depth == 0 and bracket_depth == 0 and brace_depth == 0:
                if current == "{":
                    yield text[match.start() : idx].strip()
                    break
                if current in {";", "}"}:
                    break
            if current == "(":
                paren_depth += 1
            elif current == ")":
                paren_depth = max(0, paren_depth - 1)
            elif current == "[":
                bracket_depth += 1
            elif current == "]":
                bracket_depth = max(0, bracket_depth - 1)
            elif current == "{":
                brace_depth += 1
            elif current == "}":
                if brace_depth == 0:
                    break
                brace_depth -= 1
            idx += 1


def swift_callable_signature_has_top_level_async_modifier(signature: str) -> bool:
    for idx, _ in iter_top_level_word_offsets(signature, "async"):
        before = signature[:idx].rstrip()
        after = signature[idx + len("async") :].lstrip()
        if swift_callable_async_prefix_is_valid(before) and swift_callable_async_tail_is_valid(after):
            return True
    return False


def swift_callable_signature_has_top_level_throwing_modifier(signature: str) -> bool:
    for keyword in ("throws", "rethrows"):
        for idx, _ in iter_top_level_word_offsets(signature, keyword):
            before = signature[:idx].rstrip()
            after = signature[idx + len(keyword) :].lstrip()
            if swift_callable_throwing_prefix_is_valid(before) and swift_callable_throwing_tail_is_valid(after):
                return True
    return False


def swift_callable_async_prefix_is_valid(before: str) -> bool:
    return not swift_has_top_level_return_arrow(before)


def swift_callable_async_tail_is_valid(after: str) -> bool:
    if not after or after.startswith("->") or is_word_at(after, 0, "where"):
        return True
    for keyword in ("throws", "rethrows"):
        if is_word_at(after, 0, keyword):
            rest = swift_consume_typed_throws_tail(after[len(keyword) :]).lstrip()
            return not rest or rest.startswith("->") or is_word_at(rest, 0, "where")
    return False


def swift_callable_throwing_prefix_is_valid(before: str) -> bool:
    return not swift_has_top_level_return_arrow(before)


def swift_callable_throwing_tail_is_valid(after: str) -> bool:
    rest = swift_consume_typed_throws_tail(after).lstrip()
    return not rest or rest.startswith("->") or is_word_at(rest, 0, "where")


def iter_swift_closure_headers(text: str):
    for start, ch in enumerate(text):
        if ch != "{":
            continue
        paren_depth = 0
        bracket_depth = 0
        brace_depth = 0
        idx = start + 1
        while idx < len(text):
            current = text[idx]
            if current == "\n":
                break
            if (
                paren_depth == 0
                and bracket_depth == 0
                and brace_depth == 0
                and is_word_at(text, idx, "in")
            ):
                yield text[start + 1 : idx].strip()
                break
            if current == "(":
                paren_depth += 1
            elif current == ")":
                paren_depth = max(0, paren_depth - 1)
            elif current == "[":
                bracket_depth += 1
            elif current == "]":
                bracket_depth = max(0, bracket_depth - 1)
            elif current == "{":
                brace_depth += 1
            elif current == "}":
                if brace_depth == 0:
                    break
                brace_depth -= 1
            idx += 1


def swift_closure_header_has_top_level_async_modifier(header: str) -> bool:
    header = swift_closure_header_without_capture_list(header)
    for idx, _ in iter_top_level_word_offsets(header, "async"):
        before = header[:idx].rstrip()
        after = header[idx + len("async") :].lstrip()
        if swift_closure_modifier_prefix_is_valid(before) and swift_closure_async_tail_is_valid(after):
            return True
    return False


def swift_closure_header_has_top_level_throwing_modifier(header: str) -> bool:
    header = swift_closure_header_without_capture_list(header)
    for keyword in ("throws", "rethrows"):
        for idx, _ in iter_top_level_word_offsets(header, keyword):
            before = header[:idx].rstrip()
            after = header[idx + len(keyword) :].lstrip()
            if swift_closure_modifier_prefix_is_valid(before) and swift_closure_throwing_tail_is_valid(after):
                return True
    return False


def swift_closure_header_without_capture_list(header: str) -> str:
    trimmed = header.lstrip()
    if not trimmed.startswith("["):
        return trimmed
    depth = 0
    for idx, ch in enumerate(trimmed):
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth = max(0, depth - 1)
            if depth == 0:
                return trimmed[idx + 1 :].lstrip()
    return trimmed


def iter_top_level_word_offsets(text: str, word: str):
    paren_depth = 0
    bracket_depth = 0
    brace_depth = 0
    for idx, ch in enumerate(text):
        if paren_depth == 0 and bracket_depth == 0 and brace_depth == 0 and is_word_at(text, idx, word):
            yield idx, word
        if ch == "(":
            paren_depth += 1
        elif ch == ")":
            paren_depth = max(0, paren_depth - 1)
        elif ch == "[":
            bracket_depth += 1
        elif ch == "]":
            bracket_depth = max(0, bracket_depth - 1)
        elif ch == "{":
            brace_depth += 1
        elif ch == "}":
            brace_depth = max(0, brace_depth - 1)


def swift_closure_modifier_prefix_is_valid(before: str) -> bool:
    if swift_has_top_level_colon(before):
        return False
    if before.endswith(")"):
        return True
    return any(
        token not in {"async", "throws", "rethrows"} and not token.startswith("@")
        for token in re.split(r"[\s(),:\-\>\[\]{}]+", before)
        if token
    )


def swift_closure_async_tail_is_valid(after: str) -> bool:
    if not after or after.startswith("->"):
        return True
    for keyword in ("throws", "rethrows"):
        if is_word_at(after, 0, keyword):
            rest = swift_consume_typed_throws_tail(after[len(keyword) :]).lstrip()
            return not rest or rest.startswith("->")
    return False


def swift_closure_throwing_tail_is_valid(after: str) -> bool:
    rest = swift_consume_typed_throws_tail(after).lstrip()
    return not rest or rest.startswith("->")


def swift_consume_typed_throws_tail(text: str) -> str:
    text = text.lstrip()
    if not text.startswith("("):
        return text
    depth = 0
    for idx, ch in enumerate(text):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
            if depth == 0:
                return text[idx + 1 :]
    return text


def swift_has_top_level_return_arrow(text: str) -> bool:
    paren_depth = 0
    bracket_depth = 0
    brace_depth = 0
    for idx, ch in enumerate(text):
        if paren_depth == 0 and bracket_depth == 0 and brace_depth == 0 and text.startswith("->", idx):
            return True
        if ch == "(":
            paren_depth += 1
        elif ch == ")":
            paren_depth = max(0, paren_depth - 1)
        elif ch == "[":
            bracket_depth += 1
        elif ch == "]":
            bracket_depth = max(0, bracket_depth - 1)
        elif ch == "{":
            brace_depth += 1
        elif ch == "}":
            brace_depth = max(0, brace_depth - 1)
    return False


def swift_has_top_level_colon(text: str) -> bool:
    paren_depth = 0
    bracket_depth = 0
    brace_depth = 0
    for ch in text:
        if paren_depth == 0 and bracket_depth == 0 and brace_depth == 0 and ch == ":":
            return True
        if ch == "(":
            paren_depth += 1
        elif ch == ")":
            paren_depth = max(0, paren_depth - 1)
        elif ch == "[":
            bracket_depth += 1
        elif ch == "]":
            bracket_depth = max(0, bracket_depth - 1)
        elif ch == "{":
            brace_depth += 1
        elif ch == "}":
            brace_depth = max(0, brace_depth - 1)
    return False


def is_word_at(text: str, idx: int, word: str) -> bool:
    if not text.startswith(word, idx):
        return False
    before = text[idx - 1] if idx > 0 else ""
    after_idx = idx + len(word)
    after = text[after_idx] if after_idx < len(text) else ""
    return not is_identifier_continue(before) and not is_identifier_continue(after)


def is_identifier_continue(ch: str) -> bool:
    return ch == "_" or ch.isalnum()


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


def java_future_receiver_counts(
    text: str,
    package_local_types: set[str] | None = None,
) -> dict[Pattern, int]:
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
    java_future_handle_counts(counts, text, package_local_types)
    java_executor_receiver_counts(counts, text, package_local_types)
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


def java_future_handle_counts(
    counts: dict[Pattern, int],
    text: str,
    package_local_types: set[str] | None = None,
) -> None:
    future_receivers = java_import_backed_receiver_names(
        text,
        {"CompletableFuture", "Future", "ScheduledFuture"},
        package_local_types,
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


def java_executor_receiver_counts(
    counts: dict[Pattern, int],
    text: str,
    package_local_types: set[str] | None = None,
) -> None:
    executor_receivers = java_import_backed_receiver_names(
        text, {"Executor"}, package_local_types
    )
    executor_service_receivers = java_import_backed_receiver_names(
        text,
        {"ExecutorService", "ScheduledExecutorService"},
        package_local_types,
    )
    scheduled_receivers = java_import_backed_receiver_names(
        text,
        {"ScheduledExecutorService"},
        package_local_types,
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


JAVA_CONCURRENT_RECEIVER_TYPE_NAMES = {
    "CompletableFuture",
    "CompletionStage",
    "Future",
    "ScheduledFuture",
    "Executor",
    "ExecutorService",
    "ScheduledExecutorService",
}


def java_import_backed_receiver_names(
    text: str,
    type_names: set[str],
    package_local_types: set[str] | None = None,
) -> set[str]:
    imported = java_imported_concurrent_types(text, type_names, package_local_types)
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


def java_imported_concurrent_types(
    text: str,
    type_names: set[str],
    package_local_types: set[str] | None = None,
) -> set[str]:
    blocked = java_local_type_names(text) | java_conflicting_exact_imported_type_names(text)
    imported = (java_exact_imported_concurrent_types(text) & type_names) - blocked
    if java_has_concurrent_wildcard_import(text):
        imported |= type_names - blocked - (package_local_types or set())
    return imported


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


def java_has_concurrent_wildcard_import(text: str) -> bool:
    return (
        re.search(r"\bimport\s+java\s*\.\s*util\s*\.\s*concurrent\s*\.\s*\*\s*;", text)
        is not None
    )


def java_conflicting_exact_imported_type_names(text: str) -> set[str]:
    conflicts: set[str] = set()
    for match in re.finditer(
        r"\bimport\s+(?!static\b)([A-Za-z_$][A-Za-z0-9_$]*(?:\s*\.\s*[A-Za-z_$][A-Za-z0-9_$]*)+)\s*;",
        text,
    ):
        path = re.sub(r"\s+", "", match.group(1))
        module, _, exported = path.rpartition(".")
        if exported in JAVA_CONCURRENT_RECEIVER_TYPE_NAMES and module != "java.util.concurrent":
            conflicts.add(exported)
    return conflicts


def java_local_type_names(text: str) -> set[str]:
    return {
        match.group(1)
        for match in re.finditer(
            r"\b(?:class|interface|enum|record)\s+([A-Za-z_$][A-Za-z0-9_$]*)\b",
            text,
        )
    }


def java_top_level_type_names(text: str) -> set[str]:
    names: set[str] = set()
    depth = 0
    tokens = re.finditer(
        r"[{}]|\b(?:class|interface|enum|record)\s+([A-Za-z_$][A-Za-z0-9_$]*)\b",
        text,
    )
    for token in tokens:
        if token.group(0) == "{":
            depth += 1
        elif token.group(0) == "}":
            depth = max(0, depth - 1)
        elif depth == 0 and token.group(1):
            names.add(token.group(1))
    return names


def java_package_local_types_by_package(root: Path) -> dict[str, set[str]]:
    by_package: dict[str, set[str]] = defaultdict(set)
    for path in source_files(root):
        if language_for_path(path) != "java":
            continue
        try:
            text = mask_comments_and_strings(path.read_text(errors="ignore"))
        except OSError:
            continue
        names = java_top_level_type_names(text)
        if names:
            by_package[java_package_key(path, text)].update(names)
    return by_package


def java_package_key(path: Path, text: str) -> str:
    declared = java_declared_package_name(text)
    if declared:
        return f"package:{declared}"
    return f"dir:{path.parent}"


def java_declared_package_name(text: str) -> str | None:
    match = re.search(
        r"\bpackage\s+([A-Za-z_$][A-Za-z0-9_$]*(?:\s*\.\s*[A-Za-z_$][A-Za-z0-9_$]*)*)\s*;",
        text,
    )
    if not match:
        return None
    return re.sub(r"\s+", "", match.group(1))


def summarize(args: argparse.Namespace) -> dict[str, Any]:
    repos = load_repos(Path(args.manifest))
    include_zero_surfaces = set(args.include_zero_surface)
    known_patterns = {pattern.surface: pattern for pattern in all_known_patterns()}
    unknown_zero_surfaces = sorted(include_zero_surfaces - set(known_patterns))
    if unknown_zero_surfaces:
        raise SystemExit(
            "unknown --include-zero-surface value(s): "
            + ", ".join(unknown_zero_surfaces)
        )
    by_pattern: dict[Pattern, Counter[str]] = defaultdict(Counter)
    file_counts: dict[Pattern, Counter[str]] = defaultdict(Counter)
    language_counts: Counter[str] = Counter()
    family_counts: Counter[str] = Counter()

    for repo in repos:
        repo_id = repo["id"]
        root = Path(args.repos_root) / repo_id
        java_package_types = java_package_local_types_by_package(root)
        for path in source_files(root):
            language = language_for_path(path)
            if language is None:
                continue
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            rel = str(path.relative_to(root))
            for pattern, count in count_file(
                text,
                language,
                java_package_types.get(java_package_key(path, mask_comments_and_strings(text)))
                if language == "java"
                else None,
            ).items():
                by_pattern[pattern][repo_id] += count
                file_counts[pattern][f"{repo_id}/{rel}"] += count
                language_counts[language] += count
                family_counts[pattern.obligation_family] += count

    surfaces = []
    patterns_for_report = list(by_pattern.keys())
    for surface in sorted(include_zero_surfaces):
        pattern = known_patterns[surface]
        if pattern not in by_pattern:
            patterns_for_report.append(pattern)
    for pattern in patterns_for_report:
        repo_counts = by_pattern[pattern]
        occurrences = sum(repo_counts.values())
        if occurrences == 0 and pattern.surface not in include_zero_surfaces:
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
    for surface in args.include_zero_surface:
        parts.extend(["--include-zero-surface", surface])
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
        if not item["status"].startswith(("reporting-", "superseded-", "exact-"))
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
    if subreason.startswith("channel-"):
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
            "class": "Ruby block yield callback demand/effect versus ordinary value return or direct call",
            "evidence": "crates/nose-frontend/src/ruby/tests.rs::yield_preserves_source_backed_protocol_boundary, crates/nose-cli/src/verify_admission/runtime_boundary/tests.rs::yield_protocol_missing_evidence_is_language_specific, and crates/nose-cli/tests/cli/semantic_boundaries/ruby_yield_protocol.rs::query_mode_semantic_rejects_unproven_ruby_yield_callback_convergence",
            "status": "expanded-this-slice",
        },
        {
            "class": "Rust brace/direct-imported runtime bindings shadowed by parameters, lets, local macros, block scopes, other modules, or project-local runtime roots",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/imported_bindings.rs::non_js_async_runtime_imported_bindings_reject_rust_shadows_and_scopes and ::non_js_async_runtime_context_rejects_project_local_imported_bindings",
            "status": "mapped-existing",
        },
        {
            "class": "Rust async closures versus synchronous closures and async blocks",
            "evidence": "crates/nose-frontend/src/rust/tests/async_protocols.rs::async_closure_preserves_source_backed_protocol_boundary, ::sync_closure_does_not_create_async_protocol_boundary, and crates/nose-cli/tests/cli/semantic_boundaries.rs::query_mode_semantic_rejects_unproven_rust_async_closure_sync_convergence",
            "status": "expanded-this-slice",
        },
        {
            "class": "Swift structured-concurrency runtime names shadowed by local Task bindings, Task extensions, same-file task-group functions, or project-visible task-group functions",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/swift.rs::swift_structured_concurrency_rejects_local_runtime_shadows",
            "status": "mapped-existing",
        },
        {
            "class": "Swift throwing functions and closures versus non-throwing callables, plus async throwing callables that must retain both scheduling and exception-channel obligations",
            "evidence": "crates/nose-frontend/src/swift/tests/async_protocols.rs::async_throwing_function_preserves_scheduling_and_exception_boundaries, ::async_throwing_closure_keeps_exception_boundary_inside_async_boundary, and crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/swift.rs::swift_throwing_callables_report_exception_obligations",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java CompletableFuture static calls without exact stdlib type identity, or with local/conflicting type names",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java_static_wildcard.rs::java_completable_future_static_attribution_requires_type_identity",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java CompletionStage-style receiver continuations without import-backed java.util.concurrent type-domain evidence",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_completion_stage_receiver_methods_require_import_backed_type_domain",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java Future local receivers with reassignment, local shadows, or conflicting imports, plus conflicting Executor local receivers",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_local_and_this_field_receivers_require_exact_type_identity",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java Future field receivers that are implicit, non-this, member-shadowed, duplicate, or conflicting, plus conflicting Executor field receivers",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary/tests/async_runtime/java.rs::java_local_and_this_field_receivers_require_exact_type_identity",
            "status": "expanded-this-slice",
        },
        {
            "class": "Java stream source adapters split into proof-backed receiver.stream/Arrays.stream rows versus residual untyped stream and parallelStream lifecycle boundaries",
            "evidence": "crates/nose-semantics/src/tests/library_api_evidence/admission_resolvers/iterator_adapter.rs::admitted_java_stream_identity_adapter_uses_same_protocol_pack and crates/nose-semantics/src/tests/library_api_evidence/admission_resolvers/pack_resolvers_5.rs::admitted_static_collection_adapter_resolver_requires_import_backed_api_occurrence_evidence",
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
    if args.self_test:
        self_test()
        print("scheduling lifecycle audit self-test passed")
        return
    report = summarize(args)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
