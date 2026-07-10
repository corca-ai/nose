#!/usr/bin/env python3
"""Gate semantic query output drift and material base/head runtime regressions."""

from __future__ import annotations

import argparse
import json
import math
import tempfile
from pathlib import Path
from typing import Any


STATUS_SCHEMA = "nose.semantic_regression_check.v1"
EXPECTED_DRIFT_SCHEMA = "nose.semantic_regression_expected_drift.v1"
OUTPUT_KEYS = ("hashes", "bytes", "families", "schema_versions", "surface_counts")


class CheckFailed(Exception):
    def __init__(self, message: str, *, status: dict[str, Any] | None = None, exit_code: int = 1):
        super().__init__(message)
        self.status = status
        self.exit_code = exit_code


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise CheckFailed(f"{path}: read failed: {error}") from error
    except json.JSONDecodeError as error:
        raise CheckFailed(f"{path}: invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise CheckFailed(f"{path}: top-level JSON value must be an object")
    return value


def require_object(parent: dict[str, Any], key: str, label: str) -> dict[str, Any]:
    value = parent.get(key)
    if not isinstance(value, dict):
        raise CheckFailed(f"{label}: missing object `{key}`")
    return value


def require_summary(report: dict[str, Any], label: str) -> dict[str, Any]:
    summary = require_object(report, "summary", label)
    require_object(summary, "by_repo", f"{label}.summary")
    return summary


def require_provenance(report: dict[str, Any], label: str) -> dict[str, Any]:
    return require_object(report, "provenance", label)


def finite_number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CheckFailed(f"{label}: expected a number")
    number = float(value)
    if not math.isfinite(number):
        raise CheckFailed(f"{label}: number must be finite")
    return number


def numeric(parent: dict[str, Any], key: str, label: str) -> float:
    return finite_number(parent.get(key), f"{label}.{key}")


def require_repos(report: dict[str, Any], label: str) -> list[str]:
    repos = report.get("repos")
    if (
        not isinstance(repos, list)
        or any(not isinstance(repo, str) or not repo for repo in repos)
        or len(set(repos)) != len(repos)
    ):
        raise CheckFailed(f"{label}: `repos` must be an array of unique non-empty strings")
    summary_repos = set(require_summary(report, label)["by_repo"])
    if set(repos) != summary_repos:
        raise CheckFailed(f"{label}: `repos` does not match summary.by_repo")
    return repos


def output_drift_repos(summary: dict[str, Any]) -> list[dict[str, Any]]:
    drifts = []
    for repo, rows in sorted(summary["by_repo"].items()):
        if not isinstance(rows, dict):
            raise CheckFailed(f"{repo}: summary row must be an object")
        baseline = require_object(rows, "baseline", repo)
        current = require_object(rows, "current", repo)
        changed = {}
        for key in OUTPUT_KEYS:
            baseline_value = baseline.get(key)
            current_value = current.get(key)
            if baseline_value is None and current_value is None and key in OUTPUT_KEYS[3:]:
                continue
            if not isinstance(baseline_value, list) or not isinstance(current_value, list):
                raise CheckFailed(f"{repo}: baseline/current `{key}` must be arrays")
            if baseline_value != current_value:
                changed[key] = {"baseline": baseline_value, "current": current_value}
        if changed:
            drifts.append({"repo": repo, "changed": changed})
    return drifts


def validate_same_binary_control(report: dict[str, Any], control: dict[str, Any]) -> None:
    if report.get("command") != control.get("command"):
        raise CheckFailed("same-binary control command does not match report command")
    if require_repos(report, "report") != require_repos(control, "same-binary control"):
        raise CheckFailed("same-binary control repo set does not match report repo set")
    if report.get("schema") == "nose.query_regression_harness.v2":
        if control.get("schema") != report.get("schema"):
            raise CheckFailed("same-binary control harness schema does not match report")
        if report.get("measurement") != control.get("measurement"):
            raise CheckFailed("same-binary control measurement settings do not match report")
    report_provenance = require_provenance(report, "report")
    control_provenance = require_provenance(control, "same-binary control")
    baseline_sha = control_provenance.get("baseline_binary_sha256")
    current_sha = control_provenance.get("current_binary_sha256")
    if not isinstance(baseline_sha, str) or not isinstance(current_sha, str):
        raise CheckFailed("same-binary control missing baseline/current binary sha256 provenance")
    if baseline_sha != current_sha:
        raise CheckFailed(
            "same-binary control must compare one binary with itself; "
            f"got {baseline_sha} vs {current_sha}"
        )
    report_shas = {
        report_provenance.get("baseline_binary_sha256"),
        report_provenance.get("current_binary_sha256"),
    }
    if baseline_sha not in report_shas:
        raise CheckFailed(
            "same-binary control binary sha256 must match the report baseline or current binary"
        )
    report_corpus = report.get("corpus")
    control_corpus = control.get("corpus")
    if isinstance(report_corpus, dict) or isinstance(control_corpus, dict):
        if not isinstance(report_corpus, dict) or not isinstance(control_corpus, dict):
            raise CheckFailed("same-binary control corpus provenance does not match report")
        for key in ("selection_sha256", "corpus_manifest_sha256", "prune_manifest_sha256"):
            if report_corpus.get(key) != control_corpus.get(key):
                raise CheckFailed(f"same-binary control corpus `{key}` does not match report")
    control_drifts = output_drift_repos(require_summary(control, "same-binary control"))
    if control_drifts:
        raise CheckFailed(
            "same-binary control has product output drift: "
            + ", ".join(row["repo"] for row in control_drifts)
        )


def load_expected_entries(manifest: dict[str, Any] | None) -> list[dict[str, Any]]:
    if manifest is None:
        return []
    if manifest.get("schema") != EXPECTED_DRIFT_SCHEMA:
        raise CheckFailed(f"expected-drift manifest schema must be {EXPECTED_DRIFT_SCHEMA}")
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        raise CheckFailed("expected-drift manifest `entries` must be an array")
    seen = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise CheckFailed(f"expected-drift entry {index} must be an object")
        for key in ("baseline_source_sha", "repo", "reason", "issue"):
            if not isinstance(entry.get(key), str) or not entry[key].strip():
                raise CheckFailed(f"expected-drift entry {index} needs non-empty `{key}`")
        if not isinstance(entry.get("changed"), dict) or not entry["changed"]:
            raise CheckFailed(f"expected-drift entry {index} needs exact non-empty `changed`")
        identity = (entry["baseline_source_sha"], entry["repo"])
        if identity in seen:
            raise CheckFailed(f"duplicate expected-drift declaration for {identity[0]} {identity[1]}")
        seen.add(identity)
    return entries


def authorize_drifts(
    report: dict[str, Any],
    drifts: list[dict[str, Any]],
    manifest: dict[str, Any] | None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    baseline_sha = require_provenance(report, "report").get("baseline_source_sha")
    if not isinstance(baseline_sha, str):
        raise CheckFailed("report missing baseline source SHA provenance")
    repos = set(require_repos(report, "report"))
    active = [
        entry
        for entry in load_expected_entries(manifest)
        if entry["baseline_source_sha"] == baseline_sha and entry["repo"] in repos
    ]
    by_repo = {entry["repo"]: entry for entry in active}
    authorized, unexpected = [], []
    for drift in drifts:
        declaration = by_repo.get(drift["repo"])
        if declaration is not None and declaration["changed"] == drift["changed"]:
            authorized.append({**drift, "declaration": declaration})
        else:
            unexpected.append(drift)
    drift_repos = {row["repo"] for row in drifts}
    unused = [entry for entry in active if entry["repo"] not in drift_repos]
    return authorized, unexpected, unused


def time_signal(
    *,
    scope: str,
    repo: str | None,
    stage: str | None,
    baseline_ms: float,
    current_ms: float,
    control_baseline_ms: float,
    control_current_ms: float,
    max_delta_pct: float,
    min_delta_ms: float,
) -> dict[str, Any]:
    raw_delta_ms = current_ms - baseline_ms
    control_delta_ms = control_current_ms - control_baseline_ms
    adjusted_delta_ms = raw_delta_ms - control_delta_ms
    adjusted_delta_pct = (adjusted_delta_ms / baseline_ms) * 100.0 if baseline_ms > 0 else None
    relative_trigger = adjusted_delta_pct is None or adjusted_delta_pct > max_delta_pct
    triggered = adjusted_delta_ms > min_delta_ms and relative_trigger
    return {
        "scope": scope,
        "repo": repo,
        "stage": stage,
        "baseline_ms": baseline_ms,
        "current_ms": current_ms,
        "raw_delta_ms": raw_delta_ms,
        "control_delta_ms": control_delta_ms,
        "adjusted_delta_ms": adjusted_delta_ms,
        "adjusted_delta_pct": adjusted_delta_pct,
        "triggered": triggered,
    }


def runtime_signals(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
) -> list[dict[str, Any]]:
    summary = require_summary(report, "report")
    control_summary = require_summary(control, "same-binary control") if control else None
    provenance = require_provenance(report, "report")
    if provenance.get("baseline_binary_sha256") == provenance.get("current_binary_sha256"):
        # A byte-identical binary cannot be a code regression. Use its own paired
        # movement as the noise control so scheduling jitter never hard-fails it.
        control_summary = summary
    signals = [
        time_signal(
            scope="aggregate",
            repo=None,
            stage=None,
            baseline_ms=numeric(summary, "aggregate_baseline_median_ms", "report.summary"),
            current_ms=numeric(summary, "aggregate_current_median_ms", "report.summary"),
            control_baseline_ms=(
                numeric(control_summary, "aggregate_baseline_median_ms", "control.summary")
                if control_summary
                else 0.0
            ),
            control_current_ms=(
                numeric(control_summary, "aggregate_current_median_ms", "control.summary")
                if control_summary
                else 0.0
            ),
            max_delta_pct=max_delta_pct,
            min_delta_ms=min_delta_ms,
        )
    ]
    # Historical v1 artifacts were explicitly aggregate-only. Keep those
    # reproducible; the v2 CI contract adds per-repo and per-stage enforcement.
    if report.get("schema") != "nose.query_regression_harness.v2":
        return signals
    for repo, rows in sorted(summary["by_repo"].items()):
        baseline = require_object(rows, "baseline", repo)
        current = require_object(rows, "current", repo)
        control_rows = (
            require_object(control_summary["by_repo"], repo, "control.summary.by_repo")
            if control_summary
            else {}
        )
        control_baseline = (
            require_object(control_rows, "baseline", f"control.{repo}") if control_summary else {}
        )
        control_current = (
            require_object(control_rows, "current", f"control.{repo}") if control_summary else {}
        )
        signals.append(
            time_signal(
                scope="repo",
                repo=repo,
                stage=None,
                baseline_ms=numeric(baseline, "median_ms", f"{repo}.baseline"),
                current_ms=numeric(current, "median_ms", f"{repo}.current"),
                control_baseline_ms=(
                    numeric(control_baseline, "median_ms", f"control.{repo}.baseline")
                    if control_summary
                    else 0.0
                ),
                control_current_ms=(
                    numeric(control_current, "median_ms", f"control.{repo}.current")
                    if control_summary
                    else 0.0
                ),
                max_delta_pct=max_delta_pct,
                min_delta_ms=min_delta_ms,
            )
        )
        baseline_stages = baseline.get("stages_median_ms", {})
        current_stages = current.get("stages_median_ms", {})
        if not isinstance(baseline_stages, dict) or not isinstance(current_stages, dict):
            raise CheckFailed(f"{repo}: stages_median_ms must be objects")
        control_baseline_stages = control_baseline.get("stages_median_ms", {})
        control_current_stages = control_current.get("stages_median_ms", {})
        if not isinstance(control_baseline_stages, dict) or not isinstance(
            control_current_stages, dict
        ):
            raise CheckFailed(f"control.{repo}: stages_median_ms must be objects")
        for stage in sorted(set(baseline_stages) | set(current_stages)):
            signals.append(
                time_signal(
                    scope="stage",
                    repo=repo,
                    stage=stage,
                    baseline_ms=finite_number(
                        baseline_stages.get(stage, 0.0), f"{repo}.baseline stage {stage}"
                    ),
                    current_ms=finite_number(
                        current_stages.get(stage, 0.0), f"{repo}.current stage {stage}"
                    ),
                    control_baseline_ms=finite_number(
                        control_baseline_stages.get(stage, 0.0),
                        f"control.{repo}.baseline stage {stage}",
                    ),
                    control_current_ms=finite_number(
                        control_current_stages.get(stage, 0.0),
                        f"control.{repo}.current stage {stage}",
                    ),
                    max_delta_pct=max_delta_pct,
                    min_delta_ms=min_delta_ms,
                )
            )
    return signals


def validate_focused_report(
    primary: dict[str, Any], focused: dict[str, Any], expected_repos: list[str], min_iterations: int
) -> None:
    if focused.get("command") != primary.get("command"):
        raise CheckFailed("focused rerun command does not match primary report")
    if sorted(focused.get("repos", [])) != expected_repos:
        raise CheckFailed("focused rerun repo set does not match triggered repositories")
    for key in (
        "baseline_binary_sha256",
        "current_binary_sha256",
        "baseline_source_sha",
        "current_source_sha",
    ):
        if require_provenance(focused, "focused report").get(key) != require_provenance(
            primary, "primary report"
        ).get(key):
            raise CheckFailed(f"focused rerun provenance `{key}` does not match primary report")
    primary_corpus = primary.get("corpus")
    focused_corpus = focused.get("corpus")
    if isinstance(primary_corpus, dict) or isinstance(focused_corpus, dict):
        if not isinstance(primary_corpus, dict) or not isinstance(focused_corpus, dict):
            raise CheckFailed("focused rerun corpus provenance does not match primary report")
        for key in ("selection_sha256", "corpus_manifest_sha256", "prune_manifest_sha256"):
            if focused_corpus.get(key) != primary_corpus.get(key):
                raise CheckFailed(f"focused rerun corpus `{key}` does not match primary report")
    measurement = require_object(focused, "measurement", "focused report")
    iterations = measurement.get("iterations")
    if isinstance(iterations, bool) or not isinstance(iterations, int) or iterations < min_iterations:
        raise CheckFailed(f"focused rerun needs at least {min_iterations} measured iterations")
    primary_iterations = require_object(primary, "measurement", "primary report").get("iterations")
    if not isinstance(primary_iterations, int) or isinstance(primary_iterations, bool):
        raise CheckFailed("primary report needs an integer measurement iteration count")
    if iterations <= primary_iterations:
        raise CheckFailed("focused rerun must use more measured iterations than the primary report")


def report_phase(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    manifest: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
) -> dict[str, Any]:
    require_repos(report, "report")
    if control is not None:
        validate_same_binary_control(report, control)
    drifts = output_drift_repos(require_summary(report, "report"))
    authorized, unexpected, unused = authorize_drifts(report, drifts, manifest)
    signals = runtime_signals(
        report, control, max_delta_pct=max_delta_pct, min_delta_ms=min_delta_ms
    )
    return {
        "output": {
            "authorized_drifts": authorized,
            "unexpected_drifts": unexpected,
            "unused_declarations": unused,
        },
        "runtime": {
            "signals": signals,
            "triggered": [signal for signal in signals if signal["triggered"]],
        },
    }


def evaluate_gate(
    report: dict[str, Any],
    *,
    same_binary_control: dict[str, Any] | None = None,
    expected_drift_manifest: dict[str, Any] | None = None,
    focused_report: dict[str, Any] | None = None,
    focused_same_binary_control: dict[str, Any] | None = None,
    max_runtime_delta_pct: float = 5.0,
    min_runtime_delta_ms: float = 5.0,
    min_focused_iterations: int = 5,
    require_same_binary_control: bool = False,
) -> dict[str, Any]:
    if require_same_binary_control and same_binary_control is None:
        raise CheckFailed("same-binary control is required")
    primary = report_phase(
        report,
        same_binary_control,
        expected_drift_manifest,
        max_delta_pct=max_runtime_delta_pct,
        min_delta_ms=min_runtime_delta_ms,
    )
    status = {
        "schema": STATUS_SCHEMA,
        "thresholds": {
            "max_runtime_delta_pct": max_runtime_delta_pct,
            "min_runtime_delta_ms": min_runtime_delta_ms,
            "min_focused_iterations": min_focused_iterations,
        },
        "primary": primary,
        "focused": None,
        "focused_repos": [],
        "status": "pass",
    }
    output = primary["output"]
    if output["unexpected_drifts"] or output["unused_declarations"]:
        status["status"] = "fail"
        reasons = []
        if output["unexpected_drifts"]:
            reasons.append(
                "unexpected product output drift (declare these exact changes only if intentional): "
                + json.dumps(output["unexpected_drifts"], sort_keys=True)
            )
        if output["unused_declarations"]:
            reasons.append(
                "unused expected-drift declaration for "
                + ", ".join(row["repo"] for row in output["unused_declarations"])
            )
        raise CheckFailed("; ".join(reasons), status=status)

    triggered = primary["runtime"]["triggered"]
    if not triggered:
        return status
    all_repos = sorted(require_repos(report, "report"))
    triggered_repos = sorted({signal["repo"] for signal in triggered if signal["repo"]})
    focused_repos = all_repos if any(signal["scope"] == "aggregate" for signal in triggered) else triggered_repos
    status["focused_repos"] = focused_repos
    if focused_report is None:
        status["status"] = "focused-rerun-required"
        raise CheckFailed(
            "runtime threshold crossed; focused rerun required for " + ", ".join(focused_repos),
            status=status,
            exit_code=3,
        )
    validate_focused_report(report, focused_report, focused_repos, min_focused_iterations)
    if require_same_binary_control and focused_same_binary_control is None:
        raise CheckFailed("focused same-binary control is required", status=status)
    focused = report_phase(
        focused_report,
        focused_same_binary_control,
        expected_drift_manifest,
        max_delta_pct=max_runtime_delta_pct,
        min_delta_ms=min_runtime_delta_ms,
    )
    status["focused"] = focused
    focused_output = focused["output"]
    if focused_output["unexpected_drifts"] or focused_output["unused_declarations"]:
        status["status"] = "fail"
        raise CheckFailed("focused rerun output drift is not exactly declared", status=status)
    if focused["runtime"]["triggered"]:
        status["status"] = "fail"
        labels = [
            signal["repo"] + (f":{signal['stage']}" if signal["stage"] else "")
            if signal["repo"]
            else "aggregate"
            for signal in focused["runtime"]["triggered"]
        ]
        raise CheckFailed(
            "confirmed material runtime regression in " + ", ".join(labels), status=status
        )
    return status


def check_report(report: dict[str, Any], **kwargs: Any) -> dict[str, Any]:
    return evaluate_gate(report, **kwargs)


def sample_report(*, hash_current: str = "h", delta: float = 2.0, iterations: int = 1) -> dict[str, Any]:
    return {
        "schema": "nose.query_regression_harness.v2",
        "command": "nose query <repo> all top=0 --mode semantic --format json",
        "repos": ["repo-a"],
        "measurement": {"iterations": iterations, "warmups": 0},
        "provenance": {
            "baseline_binary_sha256": "baseline",
            "current_binary_sha256": "current",
            "baseline_source_sha": "base-sha",
            "current_source_sha": "head-sha",
        },
        "summary": {
            "aggregate_baseline_median_ms": 100.0,
            "aggregate_current_median_ms": 100.0 + delta,
            "by_repo": {
                "repo-a": {
                    "baseline": {
                        "bytes": [123],
                        "families": [2],
                        "hashes": ["h"],
                        "median_ms": 100.0,
                        "schema_versions": [7],
                        "stages_median_ms": {"lower": 50.0},
                        "surface_counts": [{"default": 2}],
                    },
                    "current": {
                        "bytes": [123],
                        "families": [2],
                        "hashes": [hash_current],
                        "median_ms": 100.0 + delta,
                        "schema_versions": [7],
                        "stages_median_ms": {"lower": 50.0 + delta},
                        "surface_counts": [{"default": 2}],
                    },
                }
            },
        },
    }


def sample_control(*, delta: float = 2.0, iterations: int = 1) -> dict[str, Any]:
    report = sample_report(delta=delta, iterations=iterations)
    report["provenance"]["baseline_binary_sha256"] = "current"
    report["provenance"]["current_binary_sha256"] = "current"
    return report


def expected_manifest(hash_current: str = "changed") -> dict[str, Any]:
    return {
        "schema": EXPECTED_DRIFT_SCHEMA,
        "entries": [
            {
                "baseline_source_sha": "base-sha",
                "repo": "repo-a",
                "reason": "intentional fixture change",
                "issue": "#self-test",
                "changed": {"hashes": {"baseline": ["h"], "current": [hash_current]}},
            }
        ],
    }


def run_self_test() -> None:
    evaluate_gate(sample_report())
    identical = sample_report(delta=50.0)
    identical["provenance"]["baseline_binary_sha256"] = "same"
    identical["provenance"]["current_binary_sha256"] = "same"
    assert evaluate_gate(identical)["status"] == "pass"
    assert time_signal(
        scope="repo", repo="a", stage=None,
        baseline_ms=100.0, current_ms=106.0,
        control_baseline_ms=100.0, control_current_ms=100.0,
        max_delta_pct=5.0, min_delta_ms=5.0,
    )["triggered"]
    assert not time_signal(
        scope="repo", repo="a", stage=None,
        baseline_ms=100.0, current_ms=104.9,
        control_baseline_ms=100.0, control_current_ms=100.0,
        max_delta_pct=4.0, min_delta_ms=5.0,
    )["triggered"]
    try:
        evaluate_gate(sample_report(hash_current="changed"))
    except CheckFailed as error:
        assert "unexpected product output drift" in str(error)
    else:
        raise AssertionError("unexpected output drift must fail")
    evaluate_gate(
        sample_report(hash_current="changed"), expected_drift_manifest=expected_manifest()
    )
    try:
        evaluate_gate(sample_report(), expected_drift_manifest=expected_manifest())
    except CheckFailed as error:
        assert "unused expected-drift declaration" in str(error)
    else:
        raise AssertionError("an active declaration without drift must fail")
    invalid_control = sample_control()
    invalid_control["provenance"]["current_binary_sha256"] = "different"
    try:
        evaluate_gate(sample_report(), same_binary_control=invalid_control)
    except CheckFailed as error:
        assert "must compare one binary with itself" in str(error)
    else:
        raise AssertionError("a non-identical same-binary control must fail")
    try:
        evaluate_gate(sample_report(delta=12.0), same_binary_control=sample_control(delta=2.0))
    except CheckFailed as error:
        assert error.exit_code == 3
        assert error.status and error.status["focused_repos"] == ["repo-a"]
    else:
        raise AssertionError("material primary runtime delta must request a focused rerun")
    focused = sample_report(delta=3.0, iterations=5)
    focused_control = sample_control(delta=1.0, iterations=5)
    status = evaluate_gate(
        sample_report(delta=12.0),
        same_binary_control=sample_control(delta=2.0),
        focused_report=focused,
        focused_same_binary_control=focused_control,
    )
    assert status["status"] == "pass"
    try:
        evaluate_gate(
            sample_report(delta=12.0),
            same_binary_control=sample_control(delta=2.0),
            focused_report=sample_report(delta=12.0, iterations=5),
            focused_same_binary_control=focused_control,
        )
    except CheckFailed as error:
        assert "confirmed material runtime regression" in str(error)
    else:
        raise AssertionError("confirmed focused runtime regression must fail")
    with tempfile.TemporaryDirectory(prefix="nose-query-check-") as directory:
        path = Path(directory) / "report.json"
        path.write_text(json.dumps(sample_report()), encoding="utf-8")
        assert evaluate_gate(load_json(path))["status"] == "pass"
    print("query regression checker self-test passed")


def markdown_summary(status: dict[str, Any], report: dict[str, Any]) -> str:
    primary = status["primary"]
    result_phase = status["focused"] or primary
    lines = [
        "## Semantic regression smoke",
        "",
        f"**Status:** `{status['status']}`",
        "",
        "| Signal | Baseline | Current | Adjusted delta | Result |",
        "| --- | ---: | ---: | ---: | --- |",
    ]
    signals = [
        signal for signal in result_phase["runtime"]["signals"] if signal["scope"] != "stage"
    ]
    signals += [
        signal for signal in result_phase["runtime"]["triggered"] if signal["scope"] == "stage"
    ]
    for signal in signals:
        label = signal["repo"] or "aggregate"
        if signal["stage"]:
            label += f":{signal['stage']}"
        pct = signal["adjusted_delta_pct"]
        delta = f"{signal['adjusted_delta_ms']:+.2f} ms"
        if pct is not None:
            delta += f" / {pct:+.2f}%"
        lines.append(
            f"| `{label}` | {signal['baseline_ms']:.2f} ms | {signal['current_ms']:.2f} ms | "
            f"{delta} | {'triggered' if signal['triggered'] else 'within threshold'} |"
        )
    output = primary["output"]
    if status["focused"] is not None:
        lines += [
            "",
            "Initial material signal confirmed with a focused rerun of: "
            + ", ".join(f"`{repo}`" for repo in status["focused_repos"])
            + ".",
        ]
    lines += [
        "",
        f"Output drift: {len(output['authorized_drifts'])} declared, "
        f"{len(output['unexpected_drifts'])} unexpected.",
        "",
        f"Base `{require_provenance(report, 'report').get('baseline_source_sha')}` → "
        f"head `{require_provenance(report, 'report').get('current_source_sha')}`.",
        "",
    ]
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", nargs="?", type=Path)
    parser.add_argument("--same-binary-control", type=Path)
    parser.add_argument("--expected-drift-manifest", type=Path)
    parser.add_argument("--focused-report", type=Path)
    parser.add_argument("--focused-same-binary-control", type=Path)
    parser.add_argument("--max-runtime-delta-pct", type=float, default=5.0)
    parser.add_argument("--min-runtime-delta-ms", type=float, default=5.0)
    parser.add_argument("--min-focused-iterations", type=int, default=5)
    parser.add_argument("--require-same-binary-control", action="store_true")
    parser.add_argument("--status-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--print-json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def write_result(path: Path | None, content: str) -> None:
    if path is not None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if args.report is None:
        raise SystemExit("report path is required unless --self-test is used")
    report = load_json(args.report)
    kwargs = {
        "same_binary_control": load_json(args.same_binary_control) if args.same_binary_control else None,
        "expected_drift_manifest": load_json(args.expected_drift_manifest) if args.expected_drift_manifest else None,
        "focused_report": load_json(args.focused_report) if args.focused_report else None,
        "focused_same_binary_control": (
            load_json(args.focused_same_binary_control) if args.focused_same_binary_control else None
        ),
        "max_runtime_delta_pct": args.max_runtime_delta_pct,
        "min_runtime_delta_ms": args.min_runtime_delta_ms,
        "min_focused_iterations": args.min_focused_iterations,
        "require_same_binary_control": args.require_same_binary_control,
    }
    try:
        status = evaluate_gate(report, **kwargs)
    except CheckFailed as error:
        status = error.status
        if status is not None:
            write_result(args.status_output, json.dumps(status, indent=2, sort_keys=True) + "\n")
            write_result(args.markdown_output, markdown_summary(status, report))
        print(f"query regression check failed: {error}", flush=True)
        return error.exit_code
    write_result(args.status_output, json.dumps(status, indent=2, sort_keys=True) + "\n")
    write_result(args.markdown_output, markdown_summary(status, report))
    if args.print_json:
        print(json.dumps(status, indent=2, sort_keys=True))
    else:
        declared = len(status["primary"]["output"]["authorized_drifts"])
        rerun = " after focused rerun" if status["focused"] is not None else ""
        print(
            f"query regression check passed{rerun}: {declared} declared output drift(s); "
            "runtime within threshold"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
