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
    Pattern("javascript-typescript", "js-ts.async.function", "async function", "scheduling-boundary", "promise-async-function-scheduling-contract-missing", "async function scheduling", "async functions have Promise scheduling and rejection boundaries even without explicit await", re.compile(r"\basync\s+(?:function\b|[A-Za-z_$]|\([^)]*\)\s*=>)")),
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
    Pattern("python", "python.async.function", "async def", "scheduling-boundary", "python-async-function-scheduling-contract-missing", "async function scheduling", "async def creates coroutine protocol boundaries", re.compile(r"\basync\s+def\b")),
    Pattern("python", "python.async.iteration", "async for/with", "lifecycle-materialization-boundary", "python-async-iterator-lifecycle-contract-missing", "async iterator lifecycle", "async for/with needs awaitable lifecycle and cleanup proof", re.compile(r"\basync\s+(?:for|with)\b")),
    Pattern("python", "python.asyncio.aggregate", "asyncio.gather/wait", "success-error-result-channel", "python-asyncio-aggregate-channel-contract-missing", "asyncio aggregate", "asyncio gather/wait need aggregate completion, cancellation, and exception semantics", re.compile(r"\basyncio\s*\.\s*(?:gather|wait)\s*\(")),
    Pattern("python", "python.asyncio.scheduler", "asyncio.create_task/sleep", "scheduling-boundary", "python-asyncio-scheduler-contract-missing", "asyncio scheduler", "asyncio task/sleep APIs create scheduler and cancellation boundaries", re.compile(r"\basyncio\s*\.\s*(?:create_task|ensure_future|sleep)\s*\(")),
    Pattern("python", "python.generator.yield", "yield", "lifecycle-materialization-boundary", "generator-yield-lifecycle-contract-missing", "generator lifecycle", "yield has suspension and iterator lifecycle semantics", re.compile(r"\byield(?:\s+from)?\b")),
    Pattern("rust", "rust.async.await", ".await", "scheduling-boundary", "async-await-scheduling-contract-missing", "future await", "Rust .await polls a Future and must keep wake/scheduling effects explicit", re.compile(r"\.\s*await\b")),
    Pattern("rust", "rust.async.function", "async fn/block", "scheduling-boundary", "future-async-block-scheduling-contract-missing", "future construction", "async fn/block creates a Future boundary", re.compile(r"\basync\s+(?:fn|move\b|async\b|\{)")),
    Pattern("rust", "rust.async.spawn", "tokio/std spawn", "scheduling-boundary", "rust-spawn-scheduling-contract-missing", "task/thread spawn", "spawn APIs introduce scheduler, cancellation, and join-handle boundaries", re.compile(r"\b(?:tokio|async_std|std::thread)\s*::\s*spawn(?:_blocking)?\s*\(")),
    Pattern("rust", "rust.async.aggregate", "join/select", "success-error-result-channel", "rust-future-aggregate-channel-contract-missing", "future aggregate", "join/select style macros need all/first completion and cancellation proof", re.compile(r"\b(?:join|try_join|select)!\s*\(")),
    Pattern("go", "go.concurrent.goroutine", "go statement", "scheduling-boundary", "goroutine-scheduling-contract-missing", "goroutine scheduling", "go statements spawn concurrent execution", re.compile(r"\bgo\s+[A-Za-z_{(]")),
    Pattern("go", "go.concurrent.defer", "defer statement", "lifecycle-materialization-boundary", "defer-lifecycle-ordering-contract-missing", "defer lifecycle", "defer has scope-exit ordering and panic interaction semantics", re.compile(r"\bdefer\s+")),
    Pattern("go", "go.channel.send_receive", "channel send/receive", "channel-boundary", "channel-send-receive-protocol-contract-missing", "channel protocol", "channel send/receive has blocking and synchronization semantics", re.compile(r"<-")),
    Pattern("go", "go.channel.select", "select", "channel-boundary", "channel-select-protocol-contract-missing", "channel select", "select has readiness, default, and scheduling semantics", re.compile(r"\bselect\s*\{")),
    Pattern("java", "java.future.completable", "CompletableFuture", "success-error-result-channel", "java-completable-future-channel-contract-missing", "future channel", "CompletableFuture needs success/error channel and scheduling proof", re.compile(r"\bCompletableFuture\b")),
    Pattern("java", "java.future.executor", "Executor/Future", "scheduling-boundary", "java-executor-scheduling-contract-missing", "executor scheduling", "Executor/Future APIs introduce scheduler and lifecycle boundaries", re.compile(r"\b(?:ExecutorService|Executor|Future|ScheduledFuture)\b")),
    Pattern("java", "java.stream.lifecycle", "stream/parallelStream", "lifecycle-materialization-boundary", "java-stream-lifecycle-contract-missing", "stream lifecycle", "Java streams need lazy/eager lifecycle and terminal materialization proof", re.compile(r"\.\s*(?:stream|parallelStream)\s*\(")),
    Pattern("swift", "swift.async.await", "await", "scheduling-boundary", "async-await-scheduling-contract-missing", "await scheduling", "Swift await has task scheduling and actor/lifetime boundaries", re.compile(r"\bawait\b")),
    Pattern("swift", "swift.async.function", "async", "scheduling-boundary", "swift-async-function-scheduling-contract-missing", "async function scheduling", "Swift async surfaces create task/future-like protocol boundaries", re.compile(r"\basync\b")),
    Pattern("swift", "swift.error.throws", "throws/try", "exception-channel", "swift-throws-exception-channel-contract-missing", "throws channel", "Swift throws/try is an explicit error channel", re.compile(r"\b(?:throws|try)\b")),
    Pattern("swift", "swift.task.spawn", "Task", "scheduling-boundary", "swift-task-scheduling-contract-missing", "task scheduling", "Task APIs introduce scheduler and cancellation boundaries", re.compile(r"\bTask(?:\s*\.\s*detached)?\s*\{")),
    Pattern("ruby", "ruby.thread.fiber", "Thread/Fiber", "scheduling-boundary", "ruby-thread-fiber-scheduling-contract-missing", "thread/fiber scheduling", "Thread/Fiber APIs create scheduler and lifecycle boundaries", re.compile(r"\b(?:Thread|Fiber)\s*\.\s*(?:new|schedule)\b")),
    Pattern("ruby", "ruby.exception", "raise/rescue", "exception-channel", "ruby-exception-channel-contract-missing", "exception channel", "raise/rescue changes error channels and non-local control", re.compile(r"\b(?:raise|rescue)\b")),
    Pattern("ruby", "ruby.generator.yield", "yield", "callback-demand-effect", "ruby-yield-callback-demand-effect-contract-missing", "block callback", "Ruby yield invokes a block with demand/effect obligations", re.compile(r"\byield\b")),
    Pattern("c", "c.thread.pthread", "pthread_create", "scheduling-boundary", "c-pthread-scheduling-contract-missing", "thread scheduling", "pthread_create introduces thread scheduling and lifetime boundaries", re.compile(r"\bpthread_create\s*\(")),
    Pattern("c", "c.nonlocal_jump", "setjmp/longjmp", "exception-channel", "c-nonlocal-jump-contract-missing", "non-local jump", "setjmp/longjmp is non-local control flow, not ordinary return", re.compile(r"\b(?:setjmp|longjmp)\s*\(")),
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
    return counts


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
        "regenerate": [
            "python3 scripts/scheduling-lifecycle-boundary-audit.py --recall-loss-report target/recall-loss.issue-602.crates.json --output target/scheduling-lifecycle-boundary-audit-602.v1.json",
        ],
    }
    return report


def recommended_order(surfaces: list[dict[str, Any]]) -> list[dict[str, Any]]:
    priority = {
        "promise-aggregate-all-fulfilled-contract-missing": 1,
        "promise-aggregate-first-settled-contract-missing": 2,
        "promise-executor-timing-contract-missing": 3,
        "abort-signal-cancellation-contract-missing": 4,
        "interval-async-iteration-lifecycle-contract-missing": 5,
        "goroutine-scheduling-contract-missing": 6,
        "java-completable-future-channel-contract-missing": 7,
    }
    surface_priority = {
        "swift.async.await": 8,
    }
    candidates = [
        item
        for item in surfaces
        if item["obligation_subreason"] in priority
        or item["surface"] in surface_priority
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
            "status": "expanded-this-slice",
        },
        {
            "class": "executor callback timing and thrown executor errors",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary.rs::promise_constructor_missing_evidence_splits_executor_obligations",
            "status": "reporting-only-this-slice",
        },
        {
            "class": "scheduler/microtask ordering versus synchronous evaluation",
            "evidence": "crates/nose-cli/src/verify_admission/runtime_boundary.rs::scheduler_and_interval_calls_report_timing_and_lifecycle_obligations",
            "status": "reporting-only-this-slice",
        },
        {
            "class": "interval stream liveness/cardinality",
            "evidence": "crates/nose-cli/tests/cli/commands/recall_loss_report.rs::recall_loss_report_splits_promise_protocol_boundaries",
            "status": "reporting-only-this-slice",
        },
        {
            "class": "cross-language lifecycle one-shot/reusable/materialized distinctions",
            "evidence": "docs/scheduling-channel-callback-obligations-594.md",
            "status": "mapped-doc-policy",
        },
    ]


def current_recall_loss(path: str | None) -> dict[str, Any] | None:
    if not path:
        return None
    report_path = Path(path)
    if not report_path.exists():
        return {"report": path, "status": "missing"}
    report = json.loads(report_path.read_text())
    relevant = []
    for item in report.get("by_obligation", []):
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
        } or any(key in subreason for key in ("promise", "scheduler", "channel", "goroutine", "defer", "interval")):
            relevant.append(item)
    return {
        "report": path,
        "summary": report.get("summary", {}),
        "soundness_gate": report.get("soundness_gate", {}),
        "relevant_obligations": relevant,
    }


def main() -> None:
    args = parse_args()
    report = summarize(args)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
