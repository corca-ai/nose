#!/usr/bin/env python3
"""Gate semantic query output drift and material base/head runtime regressions."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import statistics
import tempfile
from pathlib import Path
from typing import Any

from query_regression_summary import summarize_runs
from query_regression_control import run_self_test as run_order_aware_self_test
from query_regression_control import runtime_signals as order_aware_runtime_signals


STATUS_SCHEMA = "nose.semantic_regression_check.v1"
EXPECTED_DRIFT_SCHEMA = "nose.semantic_regression_expected_drift.v1"
REPORT_SCHEMA_V2 = "nose.query_regression_harness.v2"
REPORT_SCHEMA_V3 = "nose.query_regression_harness.v3"
REPORT_SCHEMAS = (REPORT_SCHEMA_V2, REPORT_SCHEMA_V3)
OUTPUT_KEYS = ("hashes", "bytes", "families", "schema_versions", "surface_counts")
HEX_RE = re.compile(r"^[0-9a-f]+$")


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


def binary_code_identity(provenance: dict[str, Any], side: str) -> object:
    return provenance.get(
        f"{side}_binary_code_sha256",
        provenance.get(f"{side}_binary_sha256"),
    )


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


def require_string(parent: dict[str, Any], key: str, label: str, *, allow_empty: bool = False) -> str:
    value = parent.get(key)
    if not isinstance(value, str) or (not allow_empty and not value):
        raise CheckFailed(f"{label}.{key}: expected a string")
    return value


def require_hex(parent: dict[str, Any], key: str, length: int, label: str) -> str:
    value = require_string(parent, key, label)
    if len(value) != length or HEX_RE.fullmatch(value) is None:
        raise CheckFailed(f"{label}.{key}: expected {length} lowercase hex characters")
    return value


def require_nonnegative_int(parent: dict[str, Any], key: str, label: str) -> int:
    value = parent.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise CheckFailed(f"{label}.{key}: expected a non-negative integer")
    return value


def validate_output_compatibility(
    report: dict[str, Any], label: str, repos: list[str], runs: list[dict[str, Any]]
) -> None:
    compatibility = report.get("output_compatibility")
    if compatibility is None:
        return
    if not isinstance(compatibility, dict):
        raise CheckFailed(f"{label}.output_compatibility: expected an object")
    normalizer = require_string(
        compatibility, "normalizer", f"{label}.output_compatibility"
    )
    by_repo = compatibility.get("by_repo")
    if not isinstance(by_repo, dict) or list(by_repo) != repos:
        raise CheckFailed(
            f"{label}.output_compatibility.by_repo: selection does not match repos"
        )
    rebuilt = {}
    for repo in repos:
        hashes = {}
        for run_label in ("baseline", "current"):
            selected = [
                (index, run)
                for index, run in enumerate(runs)
                if run.get("repo") == repo and run.get("label") == run_label
            ]
            hashes[run_label] = sorted(
                {
                    require_hex(
                        run,
                        "normalized_sha256",
                        64,
                        f"{label}.runs[{index}]",
                    )
                    for index, run in selected
                }
            )
        rebuilt[repo] = {
            "baseline_hashes": hashes["baseline"],
            "current_hashes": hashes["current"],
            "equal": (
                hashes["baseline"] == hashes["current"]
                and len(hashes["baseline"]) == 1
            ),
        }
    expected = {
        "all_equal": all(row["equal"] for row in rebuilt.values()),
        "by_repo": rebuilt,
        "normalizer": normalizer,
    }
    if compatibility != expected:
        raise CheckFailed(f"{label}.output_compatibility: does not match raw runs")
    if not expected["all_equal"]:
        failed = ", ".join(repo for repo, row in rebuilt.items() if not row["equal"])
        raise CheckFailed(f"{label}: normalized output compatibility failed for {failed}")


def validate_structured_report(
    report: dict[str, Any], label: str, *, require_corpus_provenance: bool = False
) -> None:
    repos = require_repos(report, label)
    require_string(report, "command", label)
    measurement = require_object(report, "measurement", label)
    iterations = require_nonnegative_int(measurement, "iterations", f"{label}.measurement")
    if iterations == 0:
        raise CheckFailed(f"{label}.measurement.iterations: expected a positive integer")
    require_nonnegative_int(measurement, "warmups", f"{label}.measurement")
    samples_per_observation = measurement.get("samples_per_observation", 1)
    if (
        isinstance(samples_per_observation, bool)
        or not isinstance(samples_per_observation, int)
        or (samples_per_observation != 1 and samples_per_observation < 5)
        or samples_per_observation % 2 == 0
    ):
        raise CheckFailed(
            f"{label}.measurement.samples_per_observation: expected 1 or an odd integer "
            "of at least 5"
        )
    if report.get("schema") == REPORT_SCHEMA_V3:
        design = require_object(measurement, "design", f"{label}.measurement")
        expected_design = {
            "kind": "paired-alternating-blocks/v1",
            "block": "iteration",
            "orders": ["baseline-current", "current-baseline"],
        }
        if design != expected_design:
            raise CheckFailed(f"{label}.measurement.design: unsupported paired-block design")

    provenance = require_provenance(report, label)
    for key in ("baseline_binary_sha256", "current_binary_sha256"):
        require_hex(provenance, key, 64, f"{label}.provenance")
    code_identity_keys = (
        "baseline_binary_code_sha256",
        "baseline_binary_code_sha256_algorithm",
        "current_binary_code_sha256",
        "current_binary_code_sha256_algorithm",
    )
    present_code_identity_keys = [key for key in code_identity_keys if key in provenance]
    if present_code_identity_keys and len(present_code_identity_keys) != len(code_identity_keys):
        raise CheckFailed(f"{label}.provenance: incomplete binary code identity")
    if present_code_identity_keys:
        for key in ("baseline_binary_code_sha256", "current_binary_code_sha256"):
            require_hex(provenance, key, 64, f"{label}.provenance")
        for key in (
            "baseline_binary_code_sha256_algorithm",
            "current_binary_code_sha256_algorithm",
        ):
            require_string(provenance, key, f"{label}.provenance")
    for key in ("baseline_source_sha", "current_source_sha"):
        require_hex(provenance, key, 40, f"{label}.provenance")
    for key in (
        "baseline_binary",
        "current_binary",
        "baseline_source_ref",
        "current_source_ref",
        "harness",
        "harness_command",
    ):
        require_string(provenance, key, f"{label}.provenance")
    require_string(
        provenance,
        "working_tree_status_before_measurement",
        f"{label}.provenance",
        allow_empty=True,
    )
    if provenance["harness"] != "scripts/query-regression-harness.py":
        raise CheckFailed(f"{label}.provenance.harness: unexpected harness")

    if "corpus" not in report:
        raise CheckFailed(f"{label}: missing `corpus` field")
    corpus = report["corpus"]
    if corpus is None:
        if require_corpus_provenance:
            raise CheckFailed(f"{label}: corpus provenance is required")
    elif not isinstance(corpus, dict):
        raise CheckFailed(f"{label}.corpus: expected an object or null")
    else:
        for key in ("corpus_manifest_sha256", "prune_manifest_sha256", "selection_sha256"):
            require_hex(corpus, key, 64, f"{label}.corpus")
        require_string(corpus, "corpus_manifest", f"{label}.corpus")
        require_string(corpus, "prune_manifest", f"{label}.corpus")
        revisions = corpus.get("repositories")
        if not isinstance(revisions, list):
            raise CheckFailed(f"{label}.corpus.repositories: expected an array")
        revision_repos = []
        for index, revision in enumerate(revisions):
            if not isinstance(revision, dict):
                raise CheckFailed(f"{label}.corpus.repositories[{index}]: expected an object")
            revision_repos.append(
                require_string(revision, "repo", f"{label}.corpus.repositories[{index}]")
            )
            require_hex(revision, "commit", 40, f"{label}.corpus.repositories[{index}]")
        if revision_repos != repos:
            raise CheckFailed(f"{label}.corpus.repositories: selection does not match repos")
        state_keys = {"corpus_state", "corpus_state_sha256", "subset_digest_after_prune"}
        if state_keys & corpus.keys():
            if not state_keys <= corpus.keys():
                raise CheckFailed(f"{label}.corpus: incomplete checked subset state provenance")
            require_string(corpus, "corpus_state", f"{label}.corpus")
            require_hex(corpus, "corpus_state_sha256", 64, f"{label}.corpus")
            digest = require_object(corpus, "subset_digest_after_prune", f"{label}.corpus")
            require_hex(digest, "hex", 64, f"{label}.corpus.subset_digest_after_prune")
            require_nonnegative_int(digest, "files", f"{label}.corpus.subset_digest_after_prune")
            require_nonnegative_int(digest, "bytes", f"{label}.corpus.subset_digest_after_prune")
        expected_state_keys = {"expected_corpus_state", "expected_corpus_state_sha256"}
        if expected_state_keys & corpus.keys():
            if not expected_state_keys <= corpus.keys():
                raise CheckFailed(f"{label}.corpus: incomplete expected subset state provenance")
            require_string(corpus, "expected_corpus_state", f"{label}.corpus")
            require_hex(corpus, "expected_corpus_state_sha256", 64, f"{label}.corpus")
        if require_corpus_provenance:
            required_state_keys = state_keys | expected_state_keys
            missing_state_keys = sorted(required_state_keys - corpus.keys())
            if missing_state_keys:
                raise CheckFailed(
                    f"{label}.corpus: checked subset provenance is required; "
                    f"missing {missing_state_keys}"
                )

    environment = require_object(report, "environment", label)
    for key in ("architecture", "os", "os_release", "python_version"):
        require_string(environment, key, f"{label}.environment")
    cpu_count = require_nonnegative_int(environment, "logical_cpu_count", f"{label}.environment")
    if cpu_count == 0:
        raise CheckFailed(f"{label}.environment.logical_cpu_count: expected a positive integer")
    execution = require_object(report, "execution", label)
    require_string(execution, "working_directory", f"{label}.execution")
    if require_string(execution, "repo_argument", f"{label}.execution") != "<repo-id>":
        raise CheckFailed(f"{label}.execution.repo_argument: expected stable <repo-id>")

    runs = report.get("runs")
    if not isinstance(runs, list):
        raise CheckFailed(f"{label}.runs: expected an array")
    expected_runs = {
        (repo, run_label, iteration)
        for repo in repos
        for run_label in ("baseline", "current")
        for iteration in range(1, iterations + 1)
    }
    observed_runs = set()
    for index, run in enumerate(runs):
        run_label = f"{label}.runs[{index}]"
        if not isinstance(run, dict):
            raise CheckFailed(f"{run_label}: expected an object")
        identity = (
            require_string(run, "repo", run_label),
            require_string(run, "label", run_label),
            require_nonnegative_int(run, "iteration", run_label),
        )
        if identity in observed_runs:
            raise CheckFailed(f"{run_label}: duplicate measurement identity {identity}")
        observed_runs.add(identity)
        if report.get("schema") == REPORT_SCHEMA_V3:
            pair_order = require_string(run, "pair_order", run_label)
            expected_order = "baseline-current" if identity[2] % 2 else "current-baseline"
            if pair_order != expected_order:
                raise CheckFailed(f"{run_label}.pair_order: does not alternate by block")
            pair_position = require_nonnegative_int(run, "pair_position", run_label)
            first_label = "baseline" if pair_order == "baseline-current" else "current"
            expected_position = 0 if identity[1] == first_label else 1
            if pair_position != expected_position:
                raise CheckFailed(f"{run_label}.pair_position: does not match pair order")
        require_nonnegative_int(run, "bytes", run_label)
        require_nonnegative_int(run, "families", run_label)
        require_nonnegative_int(run, "schema_version", run_label)
        require_hex(run, "sha256", 64, run_label)
        observation_samples = run.get("observation_samples", 1)
        if (
            isinstance(observation_samples, bool)
            or not isinstance(observation_samples, int)
            or observation_samples != samples_per_observation
        ):
            raise CheckFailed(
                f"{run_label}.observation_samples: does not match measurement settings"
            )
        elapsed_ms = finite_number(run.get("elapsed_ms"), f"{run_label}.elapsed_ms")
        if elapsed_ms <= 0:
            raise CheckFailed(f"{run_label}.elapsed_ms: expected a positive number")
        surfaces = require_object(run, "surface_counts", run_label)
        for surface, count in surfaces.items():
            if not isinstance(surface, str) or not surface:
                raise CheckFailed(f"{run_label}.surface_counts: invalid surface name")
            finite_count = count if isinstance(count, int) and not isinstance(count, bool) else -1
            if finite_count < 0:
                raise CheckFailed(f"{run_label}.surface_counts.{surface}: invalid count")
        stages = require_object(run, "stages_ms", run_label)
        for stage, value in stages.items():
            if not isinstance(stage, str) or not stage:
                raise CheckFailed(f"{run_label}.stages_ms: invalid stage name")
            if finite_number(value, f"{run_label}.stages_ms.{stage}") < 0:
                raise CheckFailed(f"{run_label}.stages_ms.{stage}: expected non-negative time")
        sample_timings = run.get("sample_timings")
        if sample_timings is None:
            if "samples_per_observation" in measurement:
                raise CheckFailed(f"{run_label}.sample_timings: missing raw sample timings")
            continue
        if not isinstance(sample_timings, list) or len(sample_timings) != observation_samples:
            raise CheckFailed(
                f"{run_label}.sample_timings: expected {observation_samples} samples"
            )
        timing_values: list[tuple[int, float, dict[str, float]]] = []
        for sample_index, timing in enumerate(sample_timings):
            timing_label = f"{run_label}.sample_timings[{sample_index}]"
            if not isinstance(timing, dict):
                raise CheckFailed(f"{timing_label}: expected an object")
            position = require_nonnegative_int(
                timing, "actual_process_position", timing_label
            )
            if position not in (0, 1):
                raise CheckFailed(
                    f"{timing_label}.actual_process_position: expected 0 or 1"
                )
            sample_elapsed = finite_number(timing.get("elapsed_ms"), f"{timing_label}.elapsed_ms")
            if sample_elapsed <= 0:
                raise CheckFailed(f"{timing_label}.elapsed_ms: expected a positive number")
            sample_stages = require_object(timing, "stages_ms", timing_label)
            if sample_stages.keys() != stages.keys():
                raise CheckFailed(f"{timing_label}.stages_ms: stage set does not match observation")
            checked_stages = {
                stage: finite_number(value, f"{timing_label}.stages_ms.{stage}")
                for stage, value in sample_stages.items()
            }
            if any(value < 0 for value in checked_stages.values()):
                raise CheckFailed(f"{timing_label}.stages_ms: expected non-negative times")
            timing_values.append((position, sample_elapsed, checked_stages))
        positions = [position for position, _, _ in timing_values]
        if observation_samples > 1 and set(positions) != {0, 1}:
            raise CheckFailed(f"{run_label}.sample_timings: both process positions are required")
        if report.get("schema") == REPORT_SCHEMA_V3:
            expected_positions = [
                run["pair_position"] if sample_index % 2 == 0 else 1 - run["pair_position"]
                for sample_index in range(observation_samples)
            ]
            if positions != expected_positions:
                raise CheckFailed(
                    f"{run_label}.sample_timings: positions do not follow mirrored sample order"
                )

        def position_neutral(values: list[tuple[int, float]]) -> float:
            if observation_samples == 1:
                return values[0][1]
            return statistics.fmean(
                statistics.median(value for position, value in values if position == expected)
                for expected in (0, 1)
            )

        rebuilt_elapsed = position_neutral(
            [(position, sample_elapsed) for position, sample_elapsed, _ in timing_values]
        )
        rebuilt_stages = {
            stage: position_neutral(
                [(position, sample_stages[stage]) for position, _, sample_stages in timing_values]
            )
            for stage in stages
        }
        if not math.isclose(elapsed_ms, rebuilt_elapsed, rel_tol=1e-12, abs_tol=1e-9):
            raise CheckFailed(f"{run_label}.elapsed_ms: does not match raw sample timings")
        for stage, rebuilt_value in rebuilt_stages.items():
            if not math.isclose(
                finite_number(stages[stage], f"{run_label}.stages_ms.{stage}"),
                rebuilt_value,
                rel_tol=1e-12,
                abs_tol=1e-9,
            ):
                raise CheckFailed(
                    f"{run_label}.stages_ms.{stage}: does not match raw sample timings"
                )
    if observed_runs != expected_runs:
        raise CheckFailed(f"{label}.runs: measurements do not match repos/iterations")
    rebuilt_summary = summarize_runs(runs, repos)
    if report.get("summary") != rebuilt_summary:
        raise CheckFailed(f"{label}.summary: does not match raw runs")
    validate_output_compatibility(report, label, repos, runs)


def validate_report_contract(
    report: dict[str, Any], label: str, *, require_corpus_provenance: bool = False
) -> str:
    schema = report.get("schema")
    if schema in REPORT_SCHEMAS:
        validate_structured_report(
            report, label, require_corpus_provenance=require_corpus_provenance
        )
        return "v3" if schema == REPORT_SCHEMA_V3 else "v2"
    if schema is not None:
        raise CheckFailed(f"{label}.schema: unsupported query regression schema {schema!r}")
    if require_corpus_provenance:
        raise CheckFailed(f"{label}: corpus provenance requires a structured report schema")
    forbidden = {"measurement", "environment", "execution", "corpus"} & report.keys()
    if forbidden:
        raise CheckFailed(f"{label}: schema-less report contains v2 fields: {sorted(forbidden)}")
    summary = require_summary(report, label)
    for repo, rows in summary["by_repo"].items():
        for run_label in ("baseline", "current"):
            row = require_object(rows, run_label, f"{label}.{repo}")
            v2_fields = {"schema_versions", "surface_counts", "stages_median_ms"} & row.keys()
            if v2_fields:
                raise CheckFailed(
                    f"{label}.{repo}.{run_label}: schema-less report contains v2 fields"
                )
    return "legacy"


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


def validate_same_binary_control(
    report: dict[str, Any],
    control: dict[str, Any],
    *,
    require_corpus_provenance: bool = False,
) -> None:
    report_kind = validate_report_contract(
        report, "report", require_corpus_provenance=require_corpus_provenance
    )
    control_kind = validate_report_contract(
        control,
        "same-binary control",
        require_corpus_provenance=require_corpus_provenance,
    )
    if report_kind != control_kind:
        raise CheckFailed("same-binary control report schema does not match report")
    if report.get("command") != control.get("command"):
        raise CheckFailed("same-binary control command does not match report command")
    if require_repos(report, "report") != require_repos(control, "same-binary control"):
        raise CheckFailed("same-binary control repo set does not match report repo set")
    if report.get("schema") in REPORT_SCHEMAS:
        if control.get("schema") != report.get("schema"):
            raise CheckFailed("same-binary control harness schema does not match report")
        if report.get("measurement") != control.get("measurement"):
            raise CheckFailed("same-binary control measurement settings do not match report")
        report_compatibility = report.get("output_compatibility")
        control_compatibility = control.get("output_compatibility")
        if (report_compatibility is None) != (control_compatibility is None):
            raise CheckFailed("same-binary control output normalizer does not match report")
        if isinstance(report_compatibility, dict) and isinstance(control_compatibility, dict):
            if report_compatibility.get("normalizer") != control_compatibility.get("normalizer"):
                raise CheckFailed("same-binary control output normalizer does not match report")
    report_provenance = require_provenance(report, "report")
    control_provenance = require_provenance(control, "same-binary control")
    for key in ("harness_sha256", "worktree_helper_sha256", "worktrees_root"):
        if report_provenance.get(key) != control_provenance.get(key):
            raise CheckFailed(f"same-binary control provenance `{key}` does not match report")
    baseline_sha = control_provenance.get("baseline_binary_sha256")
    current_sha = control_provenance.get("current_binary_sha256")
    if not isinstance(baseline_sha, str) or not isinstance(current_sha, str):
        raise CheckFailed("same-binary control missing baseline/current binary sha256 provenance")
    baseline_code_identity = binary_code_identity(control_provenance, "baseline")
    current_code_identity = binary_code_identity(control_provenance, "current")
    if baseline_code_identity != current_code_identity:
        raise CheckFailed(
            "same-binary control must compare one binary with itself; "
            f"got code identities {baseline_code_identity} vs {current_code_identity}"
        )
    report_code_identities = {
        binary_code_identity(report_provenance, "baseline"),
        binary_code_identity(report_provenance, "current"),
    }
    if baseline_code_identity not in report_code_identities:
        raise CheckFailed(
            "same-binary control code identity must match the report baseline or current binary"
        )
    if report_kind in ("v2", "v3"):
        control_base_source = control_provenance.get("baseline_source_sha")
        control_current_source = control_provenance.get("current_source_sha")
        if control_base_source != control_current_source:
            raise CheckFailed("same-binary control must record one source SHA on both sides")
        matching_source_shas = set()
        if baseline_code_identity == binary_code_identity(report_provenance, "baseline"):
            matching_source_shas.add(report_provenance.get("baseline_source_sha"))
        if baseline_code_identity == binary_code_identity(report_provenance, "current"):
            matching_source_shas.add(report_provenance.get("current_source_sha"))
        if control_base_source not in matching_source_shas:
            raise CheckFailed("same-binary control source SHA does not match its report binary")
    report_corpus = report.get("corpus")
    control_corpus = control.get("corpus")
    if isinstance(report_corpus, dict) or isinstance(control_corpus, dict):
        if not isinstance(report_corpus, dict) or not isinstance(control_corpus, dict):
            raise CheckFailed("same-binary control corpus provenance does not match report")
        for key in (
            "selection_sha256",
            "corpus_manifest_sha256",
            "prune_manifest_sha256",
            "corpus_state_sha256",
            "expected_corpus_state_sha256",
            "subset_digest_after_prune",
        ):
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


def legacy_runtime_signals(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
) -> list[dict[str, Any]]:
    summary = require_summary(report, "report")
    control_summary = require_summary(control, "same-binary control") if control else None
    provenance = require_provenance(report, "report")
    if binary_code_identity(provenance, "baseline") == binary_code_identity(
        provenance, "current"
    ):
        # Code-identical binaries cannot be a code regression. Full artifact digests
        # can still differ on Darwin because LC_UUID and ad-hoc signatures are fresh.
        # Use the paired movement as its own control so that metadata never turns
        # scheduling jitter into a hard failure.
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
    if report.get("schema") not in REPORT_SCHEMAS:
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


def uses_order_aware(report: dict[str, Any], runtime_policy: str) -> bool:
    return runtime_policy == "order-aware-v1" or (
        runtime_policy == "auto" and report.get("schema") == REPORT_SCHEMA_V3
    )


def runtime_signals(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
    runtime_policy: str,
) -> list[dict[str, Any]]:
    provenance = require_provenance(report, "report")
    if binary_code_identity(provenance, "baseline") == binary_code_identity(
        provenance, "current"
    ):
        return legacy_runtime_signals(
            report,
            report,
            max_delta_pct=max_delta_pct,
            min_delta_ms=min_delta_ms,
        )
    use_order_aware = uses_order_aware(report, runtime_policy)
    if use_order_aware:
        if report.get("schema") not in REPORT_SCHEMAS:
            raise CheckFailed("order-aware runtime policy requires a v2 or v3 report")
        try:
            return order_aware_runtime_signals(
                report,
                control,
                max_delta_pct=max_delta_pct,
                min_delta_ms=min_delta_ms,
            )
        except ValueError as error:
            raise CheckFailed(f"order-aware runtime evidence is malformed: {error}") from error
    return legacy_runtime_signals(
        report,
        control,
        max_delta_pct=max_delta_pct,
        min_delta_ms=min_delta_ms,
    )


def validate_focused_report(
    primary: dict[str, Any],
    focused: dict[str, Any],
    expected_repos: list[str],
    min_iterations: int,
    *,
    require_corpus_provenance: bool = False,
) -> None:
    primary_kind = validate_report_contract(
        primary,
        "primary report",
        require_corpus_provenance=require_corpus_provenance,
    )
    focused_kind = validate_report_contract(
        focused,
        "focused report",
        require_corpus_provenance=require_corpus_provenance,
    )
    if primary_kind != focused_kind:
        raise CheckFailed("focused rerun report schema does not match primary report")
    if focused.get("command") != primary.get("command"):
        raise CheckFailed("focused rerun command does not match primary report")
    measured_repos = sorted(focused.get("repos", []))
    if not set(expected_repos) <= set(measured_repos):
        raise CheckFailed("focused rerun does not cover every triggered repository")
    provenance_keys = [
        "baseline_binary_sha256",
        "current_binary_sha256",
        "baseline_source_sha",
        "current_source_sha",
        "harness_sha256",
        "worktree_helper_sha256",
        "worktrees_root",
    ]
    primary_provenance = require_provenance(primary, "primary report")
    if "baseline_binary_code_sha256" in primary_provenance:
        provenance_keys.extend(
            [
                "baseline_binary_code_sha256",
                "baseline_binary_code_sha256_algorithm",
                "current_binary_code_sha256",
                "current_binary_code_sha256_algorithm",
            ]
        )
    for key in provenance_keys:
        if require_provenance(focused, "focused report").get(key) != require_provenance(
            primary, "primary report"
        ).get(key):
            raise CheckFailed(f"focused rerun provenance `{key}` does not match primary report")
    primary_corpus = primary.get("corpus")
    focused_corpus = focused.get("corpus")
    if isinstance(primary_corpus, dict) or isinstance(focused_corpus, dict):
        if not isinstance(primary_corpus, dict) or not isinstance(focused_corpus, dict):
            raise CheckFailed("focused rerun corpus provenance does not match primary report")
        for key in (
            "corpus_manifest_sha256",
            "prune_manifest_sha256",
            "corpus_state_sha256",
            "expected_corpus_state_sha256",
            "subset_digest_after_prune",
        ):
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
    primary_samples = require_object(primary, "measurement", "primary report").get(
        "samples_per_observation", 1
    )
    if measurement.get("samples_per_observation", 1) != primary_samples:
        raise CheckFailed(
            "focused rerun samples-per-observation does not match primary report"
        )


def project_report_repos(report: dict[str, Any], repos: list[str]) -> dict[str, Any]:
    """Derive an exact checked view from a measured focused-report superset."""
    selected = set(repos)
    measured = set(report.get("repos", []))
    if not selected <= measured:
        raise CheckFailed("cannot project repositories absent from focused report")
    projected = json.loads(json.dumps(report))
    projected["repos"] = list(repos)
    projected["runs"] = [run for run in projected["runs"] if run.get("repo") in selected]
    projected["summary"] = summarize_runs(projected["runs"], repos)
    corpus = projected.get("corpus")
    if isinstance(corpus, dict):
        revisions = [
            revision
            for revision in corpus.get("repositories", [])
            if revision.get("repo") in selected
        ]
        corpus["repositories"] = revisions
        if isinstance(corpus.get("base_revisions"), list):
            corpus["base_revisions"] = [
                revision
                for revision in corpus["base_revisions"]
                if revision.get("repo") in selected
            ]
        selection_rows = revisions
        if isinstance(corpus.get("base_revisions"), list):
            bases = {
                revision["repo"]: revision["base"]
                for revision in corpus["base_revisions"]
            }
            selection_rows = [
                {**revision, "base": bases[revision["repo"]]} for revision in revisions
            ]
        corpus["selection_sha256"] = hashlib.sha256(
            json.dumps(selection_rows, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
    compatibility = projected.get("output_compatibility")
    if isinstance(compatibility, dict) and isinstance(compatibility.get("by_repo"), dict):
        compatibility["by_repo"] = {
            repo: compatibility["by_repo"][repo] for repo in repos
        }
        compatibility["all_equal"] = all(
            row.get("equal") is True for row in compatibility["by_repo"].values()
        )
    return projected


def report_phase(
    report: dict[str, Any],
    control: dict[str, Any] | None,
    manifest: dict[str, Any] | None,
    *,
    max_delta_pct: float,
    min_delta_ms: float,
    runtime_policy: str,
    require_corpus_provenance: bool = False,
) -> dict[str, Any]:
    validate_report_contract(
        report, "report", require_corpus_provenance=require_corpus_provenance
    )
    require_repos(report, "report")
    if control is not None:
        validate_same_binary_control(
            report,
            control,
            require_corpus_provenance=require_corpus_provenance,
        )
    drifts = output_drift_repos(require_summary(report, "report"))
    authorized, unexpected, unused = authorize_drifts(report, drifts, manifest)
    signals = runtime_signals(
        report,
        control,
        max_delta_pct=max_delta_pct,
        min_delta_ms=min_delta_ms,
        runtime_policy=runtime_policy,
    )
    runtime = {
        "signals": signals,
        "triggered": [signal for signal in signals if signal["triggered"]],
    }
    if uses_order_aware(report, runtime_policy):
        runtime["inconclusive"] = [
            signal for signal in signals if signal.get("inconclusive", False)
        ]
    return {
        "output": {
            "authorized_drifts": authorized,
            "unexpected_drifts": unexpected,
            "unused_declarations": unused,
        },
        "runtime": runtime,
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
    require_corpus_provenance: bool = False,
    runtime_policy: str = "auto",
) -> dict[str, Any]:
    max_runtime_delta_pct = finite_number(
        max_runtime_delta_pct, "max_runtime_delta_pct"
    )
    min_runtime_delta_ms = finite_number(min_runtime_delta_ms, "min_runtime_delta_ms")
    if max_runtime_delta_pct < 0:
        raise CheckFailed("max_runtime_delta_pct: expected a non-negative number")
    if min_runtime_delta_ms < 0:
        raise CheckFailed("min_runtime_delta_ms: expected a non-negative number")
    if (
        isinstance(min_focused_iterations, bool)
        or not isinstance(min_focused_iterations, int)
        or min_focused_iterations <= 0
    ):
        raise CheckFailed("min_focused_iterations: expected a positive integer")
    if require_same_binary_control and same_binary_control is None:
        raise CheckFailed("same-binary control is required")
    if runtime_policy not in ("auto", "legacy", "order-aware-v1"):
        raise CheckFailed("runtime_policy: expected auto, legacy, or order-aware-v1")
    primary = report_phase(
        report,
        same_binary_control,
        expected_drift_manifest,
        max_delta_pct=max_runtime_delta_pct,
        min_delta_ms=min_runtime_delta_ms,
        runtime_policy=runtime_policy,
        require_corpus_provenance=require_corpus_provenance,
    )
    thresholds = {
        "max_runtime_delta_pct": max_runtime_delta_pct,
        "min_runtime_delta_ms": min_runtime_delta_ms,
        "min_focused_iterations": min_focused_iterations,
    }
    if runtime_policy != "auto" or report.get("schema") == REPORT_SCHEMA_V3:
        thresholds["runtime_policy"] = (
            "order-aware-v1" if uses_order_aware(report, runtime_policy) else "legacy"
        )
    status = {
        "schema": STATUS_SCHEMA,
        "thresholds": thresholds,
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
    inconclusive = primary["runtime"].get("inconclusive", [])
    needs_focus = triggered + inconclusive
    if not needs_focus:
        return status
    all_repos = sorted(require_repos(report, "report"))
    affected_repos = sorted({signal["repo"] for signal in needs_focus if signal["repo"]})
    focused_repos = (
        all_repos
        if any(signal["scope"] == "aggregate" for signal in needs_focus)
        else affected_repos
    )
    status["focused_repos"] = focused_repos
    if focused_report is None:
        status["status"] = "focused-rerun-required"
        if uses_order_aware(report, runtime_policy):
            message = "runtime evidence needs a focused rerun for "
        else:
            # Preserve the checked schema-v2 closeout text byte-for-byte. The
            # order-aware wording is prospective with schema v3; historical
            # evidence chains remain reproducible under the legacy policy.
            message = "runtime threshold crossed; focused rerun required for "
        raise CheckFailed(
            message + ", ".join(focused_repos),
            status=status,
            exit_code=3,
        )
    validate_focused_report(
        report,
        focused_report,
        focused_repos,
        min_focused_iterations,
        require_corpus_provenance=require_corpus_provenance,
    )
    if require_same_binary_control and focused_same_binary_control is None:
        raise CheckFailed("focused same-binary control is required", status=status)
    measured_focused_repos = sorted(require_repos(focused_report, "focused report"))
    if focused_same_binary_control is not None:
        validate_same_binary_control(
            focused_report,
            focused_same_binary_control,
            require_corpus_provenance=require_corpus_provenance,
        )
    if measured_focused_repos != focused_repos:
        status["focused_measured_repos"] = measured_focused_repos
        focused_report = project_report_repos(focused_report, focused_repos)
        if focused_same_binary_control is not None:
            focused_same_binary_control = project_report_repos(
                focused_same_binary_control, focused_repos
            )
    focused = report_phase(
        focused_report,
        focused_same_binary_control,
        expected_drift_manifest,
        max_delta_pct=max_runtime_delta_pct,
        min_delta_ms=min_runtime_delta_ms,
        runtime_policy=runtime_policy,
        require_corpus_provenance=require_corpus_provenance,
    )
    status["focused"] = focused
    focused_output = focused["output"]
    if focused_output["unexpected_drifts"] or focused_output["unused_declarations"]:
        status["status"] = "fail"
        raise CheckFailed("focused rerun output drift is not exactly declared", status=status)
    if focused["runtime"].get("inconclusive"):
        status["status"] = "fail"
        labels = [
            signal["repo"] + (f":{signal['stage']}" if signal["stage"] else "")
            if signal["repo"]
            else "aggregate"
            for signal in focused["runtime"]["inconclusive"]
        ]
        raise CheckFailed(
            "focused runtime evidence remains insufficient in " + ", ".join(labels),
            status=status,
        )
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


SAMPLE_BASE_BINARY = "a" * 64
SAMPLE_CURRENT_BINARY = "b" * 64
SAMPLE_BASE_SOURCE = "c" * 40
SAMPLE_CURRENT_SOURCE = "d" * 40
SAMPLE_OUTPUT_HASH = "e" * 64
SAMPLE_CHANGED_HASH = "f" * 64


def sample_report(
    *, hash_current: str = SAMPLE_OUTPUT_HASH, delta: float = 2.0, iterations: int = 1
) -> dict[str, Any]:
    runs = []
    for iteration in range(1, iterations + 1):
        for label, elapsed_ms in (
            ("baseline", 100.0),
            ("current", 100.0 + delta),
        ):
            runs.append(
                {
                    "bytes": 123,
                    "elapsed_ms": elapsed_ms,
                    "families": 2,
                    "iteration": iteration,
                    "label": label,
                    "repo": "repo-a",
                    "schema_version": 7,
                    "sha256": SAMPLE_OUTPUT_HASH if label == "baseline" else hash_current,
                    "stages_ms": {"lower": 50.0 if label == "baseline" else 50.0 + delta},
                    "surface_counts": {"default": 2},
                }
            )
    return {
        "schema": REPORT_SCHEMA_V2,
        "command": "nose query <repo> all top=0 --mode semantic --format json",
        "repos": ["repo-a"],
        "measurement": {"iterations": iterations, "warmups": 0},
        "execution": {"repo_argument": "<repo-id>", "working_directory": "/tmp/repos"},
        "environment": {
            "architecture": "test-arch",
            "logical_cpu_count": 2,
            "os": "TestOS",
            "os_release": "1",
            "python_version": "3.14",
        },
        "corpus": {
            "corpus_manifest": "bench/goldens/corpus.json",
            "corpus_manifest_sha256": "1" * 64,
            "corpus_state": "/tmp/repos/.nose-corpus-state.json",
            "corpus_state_sha256": "5" * 64,
            "expected_corpus_state": "bench/semantic_regression_corpus.v1.json",
            "expected_corpus_state_sha256": "6" * 64,
            "prune_manifest": "bench/labels/prune_manifest.json",
            "prune_manifest_sha256": "2" * 64,
            "repositories": [{"commit": "3" * 40, "repo": "repo-a"}],
            "selection_sha256": "4" * 64,
            "subset_digest_after_prune": {
                "bytes": 1234,
                "files": 12,
                "hex": "7" * 64,
            },
        },
        "provenance": {
            "baseline_binary": "/tmp/base/nose",
            "baseline_binary_code_sha256": SAMPLE_BASE_BINARY,
            "baseline_binary_code_sha256_algorithm": "sha256/full-file-v1",
            "baseline_binary_sha256": SAMPLE_BASE_BINARY,
            "baseline_source_ref": "base",
            "baseline_source_sha": SAMPLE_BASE_SOURCE,
            "current_binary": "/tmp/head/nose",
            "current_binary_code_sha256": SAMPLE_CURRENT_BINARY,
            "current_binary_code_sha256_algorithm": "sha256/full-file-v1",
            "current_binary_sha256": SAMPLE_CURRENT_BINARY,
            "current_source_ref": "head",
            "current_source_sha": SAMPLE_CURRENT_SOURCE,
            "harness": "scripts/query-regression-harness.py",
            "harness_command": "python3 scripts/query-regression-harness.py ...",
            "working_tree_status_before_measurement": "",
        },
        "runs": runs,
        "summary": summarize_runs(runs, ["repo-a"]),
    }


def sample_control(*, delta: float = 2.0, iterations: int = 1) -> dict[str, Any]:
    report = sample_report(delta=delta, iterations=iterations)
    report["provenance"]["baseline_binary_code_sha256"] = SAMPLE_CURRENT_BINARY
    report["provenance"]["current_binary_code_sha256"] = SAMPLE_CURRENT_BINARY
    report["provenance"]["baseline_binary_sha256"] = SAMPLE_CURRENT_BINARY
    report["provenance"]["current_binary_sha256"] = SAMPLE_CURRENT_BINARY
    report["provenance"]["baseline_source_sha"] = SAMPLE_CURRENT_SOURCE
    report["provenance"]["current_source_sha"] = SAMPLE_CURRENT_SOURCE
    report["provenance"]["baseline_source_ref"] = "head"
    return report


def sample_v3(report: dict[str, Any]) -> dict[str, Any]:
    report = json.loads(json.dumps(report))
    report["schema"] = REPORT_SCHEMA_V3
    report["measurement"]["design"] = {
        "kind": "paired-alternating-blocks/v1",
        "block": "iteration",
        "orders": ["baseline-current", "current-baseline"],
    }
    for run in report["runs"]:
        baseline_first = run["iteration"] % 2 == 1
        run["pair_order"] = "baseline-current" if baseline_first else "current-baseline"
        first_label = "baseline" if baseline_first else "current"
        run["pair_position"] = 0 if run["label"] == first_label else 1
    report["runs"].sort(
        key=lambda run: (run["iteration"], run["repo"], run["pair_position"])
    )
    return report


def expected_manifest(hash_current: str = SAMPLE_CHANGED_HASH) -> dict[str, Any]:
    return {
        "schema": EXPECTED_DRIFT_SCHEMA,
        "entries": [
            {
                "baseline_source_sha": SAMPLE_BASE_SOURCE,
                "repo": "repo-a",
                "reason": "intentional fixture change",
                "issue": "#self-test",
                "changed": {
                    "hashes": {"baseline": [SAMPLE_OUTPUT_HASH], "current": [hash_current]}
                },
            }
        ],
    }


def run_self_test() -> None:
    run_order_aware_self_test()
    evaluate_gate(sample_report())
    v3_primary = sample_v3(sample_report(delta=2.0, iterations=2))
    v3_control = sample_v3(sample_control(delta=-3.0, iterations=2))
    v3_focused = sample_v3(sample_report(delta=2.0, iterations=6))
    v3_focused_control = sample_v3(sample_control(delta=-3.0, iterations=6))
    v3_status = evaluate_gate(
        v3_primary,
        same_binary_control=v3_control,
        focused_report=v3_focused,
        focused_same_binary_control=v3_focused_control,
    )
    assert v3_status["status"] == "pass"
    assert v3_status["focused"] is not None
    for key, value in (
        ("harness_sha256", "a" * 64),
        ("worktree_helper_sha256", "b" * 64),
        ("worktrees_root", "/stable/base-worktrees"),
    ):
        changed_control = json.loads(json.dumps(v3_control))
        changed_control["provenance"][key] = value
        try:
            validate_same_binary_control(v3_primary, changed_control)
        except CheckFailed as error:
            assert key in str(error)
        else:
            raise AssertionError(f"control must preserve {key}")
        changed_focus = json.loads(json.dumps(v3_focused))
        changed_focus["provenance"][key] = value
        try:
            validate_focused_report(v3_primary, changed_focus, ["repo-a"], 5)
        except CheckFailed as error:
            assert key in str(error)
        else:
            raise AssertionError(f"focused rerun must preserve {key}")
    assert evaluate_gate(
        sample_v3(sample_report(delta=2.0, iterations=5)),
        same_binary_control=sample_v3(sample_control(delta=0.0, iterations=5)),
    )["status"] == "pass"
    sampled_v3 = sample_v3(sample_report(delta=2.0, iterations=5))
    sampled_v3["measurement"]["samples_per_observation"] = 5
    for run in sampled_v3["runs"]:
        run["observation_samples"] = 5
        run["sample_timings"] = [
            {
                "actual_process_position": (
                    run["pair_position"] if sample_index % 2 == 0 else 1 - run["pair_position"]
                ),
                "elapsed_ms": run["elapsed_ms"],
                "stages_ms": run["stages_ms"],
            }
            for sample_index in range(5)
        ]
    assert evaluate_gate(sampled_v3)["status"] == "pass"
    compatible = sample_report()
    for run in compatible["runs"]:
        run["normalized_sha256"] = "9" * 64
    compatible["output_compatibility"] = {
        "all_equal": True,
        "by_repo": {
            "repo-a": {
                "baseline_hashes": ["9" * 64],
                "current_hashes": ["9" * 64],
                "equal": True,
            }
        },
        "normalizer": "base-v0.19",
    }
    assert evaluate_gate(compatible)["status"] == "pass"
    incompatible = json.loads(json.dumps(compatible))
    for run in incompatible["runs"]:
        if run["label"] == "current":
            run["normalized_sha256"] = "8" * 64
    incompatible["output_compatibility"] = {
        "all_equal": False,
        "by_repo": {
            "repo-a": {
                "baseline_hashes": ["9" * 64],
                "current_hashes": ["8" * 64],
                "equal": False,
            }
        },
        "normalizer": "base-v0.19",
    }
    try:
        evaluate_gate(incompatible)
    except CheckFailed as error:
        assert "normalized output compatibility failed" in str(error)
    else:
        raise AssertionError("normalized output inequality must fail")
    tampered_compatibility = json.loads(json.dumps(compatible))
    tampered_compatibility["output_compatibility"]["all_equal"] = False
    try:
        evaluate_gate(tampered_compatibility)
    except CheckFailed as error:
        assert "does not match raw runs" in str(error)
    else:
        raise AssertionError("normalized output summary must be rebuilt from raw runs")
    superset = sample_v3(sample_report(delta=2.0, iterations=2))
    superset["repos"].append("repo-b")
    extra_runs = json.loads(json.dumps(superset["runs"]))
    for run in extra_runs:
        run["repo"] = "repo-b"
    superset["runs"].extend(extra_runs)
    superset["runs"].sort(
        key=lambda run: (run["iteration"], run["repo"], run["pair_position"])
    )
    superset["corpus"]["repositories"].append(
        {"commit": "8" * 40, "repo": "repo-b"}
    )
    superset["summary"] = summarize_runs(superset["runs"], superset["repos"])
    projected = project_report_repos(superset, ["repo-a"])
    assert projected["repos"] == ["repo-a"]
    assert {run["repo"] for run in projected["runs"]} == {"repo-a"}
    validate_structured_report(projected, "projected self-test")
    tampered_samples = json.loads(json.dumps(sampled_v3))
    tampered_samples["runs"][0]["elapsed_ms"] += 1.0
    try:
        evaluate_gate(tampered_samples)
    except CheckFailed as error:
        assert "does not match raw sample timings" in str(error)
    else:
        raise AssertionError("sample aggregate must be rebuilt from raw timings")
    missing_samples = json.loads(json.dumps(sampled_v3))
    del missing_samples["runs"][0]["sample_timings"]
    try:
        evaluate_gate(missing_samples)
    except CheckFailed as error:
        assert "missing raw sample timings" in str(error)
    else:
        raise AssertionError("declared observation samples must retain raw timings")
    raw_sub_floor = sample_v3(sample_report(delta=2.5, iterations=6))
    negative_control = sample_v3(sample_control(delta=-3.0, iterations=6))
    assert evaluate_gate(
        raw_sub_floor, same_binary_control=negative_control
    )["status"] == "pass"
    tampered_run = sample_report()
    tampered_run["runs"][1]["sha256"] = SAMPLE_CHANGED_HASH
    try:
        evaluate_gate(tampered_run)
    except CheckFailed as error:
        assert "does not match raw runs" in str(error)
    else:
        raise AssertionError("v2 summary must be rebuilt from raw runs")
    for schema in ("typo", None):
        malformed = json.loads(json.dumps(sample_report()))
        malformed["schema"] = schema
        try:
            evaluate_gate(malformed)
        except CheckFailed as error:
            assert "schema" in str(error)
        else:
            raise AssertionError("unknown or removed v2 schema must fail closed")
    missing_provenance = json.loads(json.dumps(sample_report()))
    del missing_provenance["provenance"]["current_source_sha"]
    try:
        evaluate_gate(missing_provenance)
    except CheckFailed as error:
        assert "current_source_sha" in str(error)
    else:
        raise AssertionError("v2 source provenance must be mandatory")
    generic = sample_report()
    generic["corpus"] = None
    assert evaluate_gate(generic)["status"] == "pass"
    assert evaluate_gate(sample_report(), require_corpus_provenance=True)["status"] == "pass"
    try:
        evaluate_gate(generic, require_corpus_provenance=True)
    except CheckFailed as error:
        assert "corpus provenance is required" in str(error)
    else:
        raise AssertionError("semantic smoke mode must require corpus provenance")
    missing_corpus = sample_report()
    del missing_corpus["corpus"]
    try:
        evaluate_gate(missing_corpus)
    except CheckFailed as error:
        assert "missing `corpus` field" in str(error)
    else:
        raise AssertionError("v2 reports must declare whether corpus provenance is present")
    unchecked_corpus = sample_report()
    for key in (
        "corpus_state",
        "corpus_state_sha256",
        "expected_corpus_state",
        "expected_corpus_state_sha256",
        "subset_digest_after_prune",
    ):
        del unchecked_corpus["corpus"][key]
    try:
        evaluate_gate(unchecked_corpus, require_corpus_provenance=True)
    except CheckFailed as error:
        assert "checked subset provenance is required" in str(error)
    else:
        raise AssertionError("semantic smoke mode must require checked subset provenance")
    legacy = sample_report()
    for key in ("schema", "measurement", "environment", "execution", "corpus"):
        del legacy[key]
    for rows in legacy["summary"]["by_repo"].values():
        for label in ("baseline", "current"):
            for key in ("schema_versions", "surface_counts", "stages_median_ms"):
                del rows[label][key]
    assert evaluate_gate(legacy)["status"] == "pass"
    try:
        evaluate_gate(legacy, require_corpus_provenance=True)
    except CheckFailed as error:
        assert "corpus provenance requires a structured report schema" in str(error)
    else:
        raise AssertionError("semantic smoke mode must reject schema-less reports")
    for threshold_name, kwargs in (
        ("max_runtime_delta_pct", {"max_runtime_delta_pct": math.nan}),
        ("min_runtime_delta_ms", {"min_runtime_delta_ms": math.nan}),
        ("max_runtime_delta_pct", {"max_runtime_delta_pct": -1.0}),
        ("min_runtime_delta_ms", {"min_runtime_delta_ms": -1.0}),
    ):
        try:
            evaluate_gate(sample_report(delta=50.0), **kwargs)
        except CheckFailed as error:
            assert threshold_name in str(error)
        else:
            raise AssertionError(f"invalid {threshold_name} must fail closed")
    identical = sample_report(delta=50.0)
    identical["provenance"]["baseline_binary_code_sha256"] = "0" * 64
    identical["provenance"]["current_binary_code_sha256"] = "0" * 64
    identical["provenance"]["baseline_binary_sha256"] = "0" * 64
    identical["provenance"]["current_binary_sha256"] = "0" * 64
    assert evaluate_gate(identical)["status"] == "pass"
    code_identical = sample_report(delta=50.0)
    code_identical["provenance"]["baseline_binary_code_sha256"] = "0" * 64
    code_identical["provenance"]["current_binary_code_sha256"] = "0" * 64
    assert code_identical["provenance"]["baseline_binary_sha256"] != code_identical[
        "provenance"
    ]["current_binary_sha256"]
    assert evaluate_gate(code_identical)["status"] == "pass"
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
        evaluate_gate(sample_report(hash_current=SAMPLE_CHANGED_HASH))
    except CheckFailed as error:
        assert "unexpected product output drift" in str(error)
    else:
        raise AssertionError("unexpected output drift must fail")
    evaluate_gate(
        sample_report(hash_current=SAMPLE_CHANGED_HASH), expected_drift_manifest=expected_manifest()
    )
    try:
        evaluate_gate(sample_report(), expected_drift_manifest=expected_manifest())
    except CheckFailed as error:
        assert "unused expected-drift declaration" in str(error)
    else:
        raise AssertionError("an active declaration without drift must fail")
    invalid_control = sample_control()
    invalid_control["provenance"]["current_binary_code_sha256"] = "9" * 64
    invalid_control["provenance"]["current_binary_sha256"] = "9" * 64
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
    malformed_focused = sample_report(delta=12.0, iterations=5)
    malformed_focused["schema"] = "typo"
    try:
        evaluate_gate(
            sample_report(delta=12.0),
            same_binary_control=sample_control(delta=2.0),
            focused_report=malformed_focused,
            focused_same_binary_control=focused_control,
        )
    except CheckFailed as error:
        assert "schema" in str(error)
    else:
        raise AssertionError("focused rerun schema must match the primary report")
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
            f"{delta} | "
            f"{'triggered' if signal['triggered'] else 'inconclusive' if signal.get('inconclusive') else 'within threshold'} |"
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
    parser.add_argument("--require-corpus-provenance", action="store_true")
    parser.add_argument(
        "--runtime-policy",
        choices=("auto", "legacy", "order-aware-v1"),
        default="auto",
    )
    parser.add_argument("--status-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--check-status", type=Path)
    parser.add_argument("--check-markdown", type=Path)
    parser.add_argument("--print-json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def write_result(path: Path | None, content: str) -> None:
    if path is not None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def check_result(path: Path | None, content: str, label: str) -> None:
    if path is None:
        return
    try:
        checked = path.read_text(encoding="utf-8")
    except OSError as error:
        raise CheckFailed(f"{label}: cannot read {path}: {error}") from error
    if checked != content:
        raise CheckFailed(f"{label}: checked output does not match regenerated result")


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
        "require_corpus_provenance": args.require_corpus_provenance,
        "runtime_policy": args.runtime_policy,
    }
    try:
        status = evaluate_gate(report, **kwargs)
        status_json = json.dumps(status, indent=2, sort_keys=True) + "\n"
        status_markdown = markdown_summary(status, report)
        check_result(args.check_status, status_json, "status")
        check_result(args.check_markdown, status_markdown, "Markdown summary")
    except CheckFailed as error:
        status = error.status
        if status is not None:
            write_result(args.status_output, json.dumps(status, indent=2, sort_keys=True) + "\n")
            write_result(args.markdown_output, markdown_summary(status, report))
        print(f"query regression check failed: {error}", flush=True)
        return error.exit_code
    write_result(args.status_output, status_json)
    write_result(args.markdown_output, status_markdown)
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
