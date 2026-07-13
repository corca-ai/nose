#!/usr/bin/env python3
"""Validate the checked #842 closeout from bound behavior and raw performance evidence."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import subprocess
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/generated_provenance_closeout_2026_07_13.dev.v1.json"
TOP_KEYS = {
    "schema",
    "issue",
    "split",
    "heldout_policy",
    "implementation",
    "behavior_evidence",
    "frozen_841_evidence",
    "expanded_default_behavior",
    "established_semantic_behavior",
    "performance",
    "result",
}
OFFICIAL_SHA = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
OFFICIAL_SOURCE = "54f8a67436e39e24c777a85e14224273116add6b"
CURRENT_SHA = "6d906e88270994a6ac2589977b2ce9b7616788c1bba67f9dc1b66791161de3dc"
CURRENT_SOURCE = "1f5d6b450a2a68b1382e6ce843843fe8f195c898"
BEHAVIOR_DIGEST = "17158a23270a2ba902dfd58b916b0f0720f9bbaffbe9760cf52bf732cecef6a8"
PRUNE_SHA = "c22f34d3ab4da9b89b5938140bbfdf7664178b3b7b57e5ea3937ba0bb47c2980"
QUERY_COMMAND = "nose query <repo> all top=0 --mode semantic --format json"


def fail(message: str) -> None:
    raise SystemExit(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"{path}: {error}")
    require(isinstance(value, dict), f"{path}: expected a JSON object")
    return value


def close(actual: float, expected: float, label: str) -> None:
    require(math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-9), f"{label}: {actual} != {expected}")


def stable_outputs(report: dict[str, Any], label: str) -> None:
    for repo, row in report["summary"]["by_repo"].items():
        for role in ("baseline", "current"):
            require(len(row[role]["hashes"]) == 1, f"{label}:{repo}:{role}: unstable output")


def report_corpus_rows(report: dict[str, Any], label: str) -> list[dict[str, str]]:
    rows = report.get("corpus", {}).get("repositories")
    require(isinstance(rows, list), f"{label}: missing corpus repository rows")
    require(
        all(isinstance(row, dict) and set(row) == {"repo", "commit"} for row in rows),
        f"{label}: malformed corpus repository row",
    )
    repos = report.get("repos")
    require(repos == [row["repo"] for row in rows], f"{label}: report/corpus repository order differs")
    require(len(repos) == len(set(repos)), f"{label}: duplicate corpus repository")
    return rows


def validate_bound_corpus(
    report: dict[str, Any],
    bound_rows: list[dict[str, str]],
    manifest_sha: str,
    label: str,
) -> None:
    corpus = report["corpus"]
    require(corpus.get("corpus_manifest_sha256") == manifest_sha, f"{label}: corpus manifest binding changed")
    require(corpus.get("prune_manifest_sha256") == PRUNE_SHA, f"{label}: prune manifest binding changed")
    bound = {row["repo"]: row["commit"] for row in bound_rows}
    require(len(bound) == len(bound_rows), "behavior evidence has duplicate corpus repositories")
    expected = []
    for repo in report["repos"]:
        require(repo in bound, f"{label}: {repo} is outside the bound dev corpus")
        expected.append({"repo": repo, "commit": bound[repo]})
    require(report_corpus_rows(report, label) == expected, f"{label}: corpus revision binding changed")


def validate_report_role(
    report: dict[str, Any],
    control: dict[str, Any],
    *,
    repos: int,
    iterations: int,
    warmups: int,
    label: str,
) -> None:
    for value, role in ((report, "report"), (control, "control")):
        require(value.get("schema") == "nose.query_regression_harness.v2", f"{label}:{role}: wrong schema")
        require(value.get("command") == QUERY_COMMAND, f"{label}:{role}: wrong query command")
        require(value.get("measurement") == {"iterations": iterations, "warmups": warmups}, f"{label}:{role}: wrong measurement")
        require(len(value.get("repos", [])) == repos, f"{label}:{role}: wrong repository count")
        report_corpus_rows(value, f"{label}:{role}")
        stable_outputs(value, f"{label}:{role}")
    require(report["repos"] == control["repos"], f"{label}: control repository set differs")
    require(report["corpus"]["repositories"] == control["corpus"]["repositories"], f"{label}: control corpus differs")
    provenance = report["provenance"]
    require(provenance["baseline_binary_sha256"] == OFFICIAL_SHA, f"{label}: baseline is not official v0.19.0")
    require(provenance["baseline_source_sha"] == OFFICIAL_SOURCE, f"{label}: wrong baseline source")
    require(provenance["current_binary_sha256"] == CURRENT_SHA, f"{label}: wrong current binary")
    require(provenance["current_source_sha"] == CURRENT_SOURCE, f"{label}: wrong current source")
    control_provenance = control["provenance"]
    for key in ("baseline_binary_sha256", "current_binary_sha256"):
        require(control_provenance[key] == CURRENT_SHA, f"{label}: control is not current/current")
    for key in ("baseline_source_sha", "current_source_sha"):
        require(control_provenance[key] == CURRENT_SOURCE, f"{label}: control source is not current/current")


def aggregate(report: dict[str, Any], control: dict[str, Any]) -> dict[str, float | bool]:
    summary = report["summary"]
    control_summary = control["summary"]
    baseline = summary["aggregate_baseline_median_ms"]
    current = summary["aggregate_current_median_ms"]
    raw = current - baseline
    control_delta = control_summary["aggregate_current_median_ms"] - control_summary["aggregate_baseline_median_ms"]
    adjusted = raw - control_delta
    adjusted_pct = adjusted / baseline * 100.0
    return {
        "baseline_median_ms": baseline,
        "current_median_ms": current,
        "raw_delta_ms": raw,
        "raw_delta_percent": raw / baseline * 100.0,
        "control_delta_ms": control_delta,
        "control_delta_percent": control_delta / control_summary["aggregate_baseline_median_ms"] * 100.0,
        "control_adjusted_delta_ms": adjusted,
        "control_adjusted_delta_percent": adjusted_pct,
        "material_regression": adjusted > 5.0 and adjusted_pct > 5.0,
    }


def query_surface_adjusted(report: dict[str, Any], control: dict[str, Any], repo: str) -> float:
    row = report["summary"]["by_repo"][repo]
    control_row = control["summary"]["by_repo"][repo]
    raw = row["current"]["stages_median_ms"]["query_surface"] - row["baseline"]["stages_median_ms"]["query_surface"]
    control_delta = control_row["current"]["stages_median_ms"]["query_surface"] - control_row["baseline"]["stages_median_ms"]["query_surface"]
    return raw - control_delta


def compare_metrics(actual: dict[str, Any], expected: dict[str, Any], label: str) -> None:
    require(set(actual) == set(expected), f"{label}: metric keys differ")
    for key, value in expected.items():
        if isinstance(value, bool):
            require(actual[key] is value, f"{label}:{key}: wrong verdict")
        else:
            close(float(actual[key]), float(value), f"{label}:{key}")


def run_checked(command: list[str], label: str) -> str:
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    require(result.returncode == 0, f"{label} failed: {result.stdout}{result.stderr}")
    return result.stdout.strip()


def run_checker(command: list[str], label: str, expected_code: int) -> str:
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    output = (result.stdout + result.stderr).strip()
    require(result.returncode == expected_code, f"{label}: exit {result.returncode}, expected {expected_code}: {output}")
    return output


def checker_base(report: str, control: str, drift: str) -> list[str]:
    return [
        "python3",
        "scripts/check-query-regression.py",
        report,
        "--same-binary-control",
        control,
        "--expected-drift-manifest",
        drift,
        "--require-same-binary-control",
        "--max-runtime-delta-pct",
        "5",
        "--min-runtime-delta-ms",
        "5",
    ]


def requested_focused_repos(report: str, control: str, drift: str, label: str) -> tuple[list[str], str]:
    with tempfile.TemporaryDirectory(prefix="nose-842-closeout-") as directory:
        status_path = Path(directory) / "status.json"
        command = checker_base(report, control, drift) + ["--status-output", str(status_path)]
        output = run_checker(command, label, 3)
        status = load(status_path)
    require(status.get("status") == "focused-rerun-required", f"{label}: wrong checker status")
    repos = status.get("focused_repos")
    require(isinstance(repos, list) and all(isinstance(repo, str) for repo in repos), f"{label}: invalid focused repo set")
    return repos, output


def validate(path: Path) -> None:
    artifact = load(path)
    require(set(artifact) == TOP_KEYS, f"{path}: unexpected top-level schema")
    require(artifact["schema"] == "nose.generated_provenance_closeout.v1", "wrong schema")
    require(artifact["issue"] == 842 and artifact["split"] == "dev", "wrong issue or split")
    require(artifact["result"] == "pass", "closeout result is not pass")
    require("held-out" in artifact["heldout_policy"], "held-out policy is missing")

    implementation = artifact["implementation"]
    require(implementation["commit"] == CURRENT_SOURCE, "implementation source binding changed")
    require(implementation["binary_sha256"] == CURRENT_SHA, "implementation binary binding changed")
    predicate = implementation["predicate"]
    require(predicate["bounded_prefix_bytes"] == 65536, "wrong byte bound")
    require(predicate["op"] == "all_unique_member_files", "wrong family quantifier")
    require(predicate["suffix"] == ".html", "wrong source suffix")
    require(predicate["requires_any"] == [["jazzy.css", "jazzy.js"], ['class="dashanchor"', "//apple_ref/"]], "wrong Jazzy signals")
    require(not implementation["repository_or_path_allowlist"], "allowlist must remain false")
    require(implementation["json_reason_code"] == {"field": "surface", "value": "generated"}, "wrong JSON reason")
    require(implementation["human_reason_code"] == "generated-code", "wrong human reason")

    behavior_ref = artifact["behavior_evidence"]
    behavior_path = ROOT / behavior_ref["path"]
    require(sha256(behavior_path) == behavior_ref["sha256"], "behavior artifact SHA-256 mismatch")
    behavior = load(behavior_path)
    require(behavior["evidence_digest"] == behavior_ref["evidence_digest"] == BEHAVIOR_DIGEST, "behavior semantic binding changed")
    run_checked(["python3", "bench/labels/generated_provenance_behavior.py", "validate", str(behavior_path)], "behavior validation")

    frozen = artifact["frozen_841_evidence"]
    taxonomy_path = ROOT / frozen["taxonomy_artifact"]
    require(frozen["taxonomy_artifact"] == "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json", "wrong #841 taxonomy path")
    require(sha256(taxonomy_path) == frozen["taxonomy_file_sha256"], "#841 taxonomy file binding changed")
    taxonomy = load(taxonomy_path)
    require(taxonomy["artifact_sha256"] == frozen["taxonomy_artifact_sha256"], "#841 taxonomy semantic binding changed")
    cohorts = behavior["cohorts"]
    require(frozen["head_positives"] == {"expected": 10, "generated": sum(row["surface"] == "generated" for row in cohorts["head_positives"]), "missing": 0}, "head positives changed")
    require(frozen["deep_audit_positives"] == {"expected": 20, "generated": sum(row["surface"] == "generated" for row in cohorts["deep_audit_positives"]), "missing": 0}, "deep positives changed")
    require(frozen["html_hard_negatives"] == {"expected": 3, "default": sum(row["surface"] == "default" for row in cohorts["html_hard_negatives"]), "false_demotions": 0}, "hard negatives changed")

    require(artifact["expanded_default_behavior"] == behavior["expanded_summary"], "expanded behavior summary is not derived")
    require(artifact["established_semantic_behavior"] == behavior["semantic_summary"], "semantic behavior summary is not derived")

    performance = artifact["performance"]
    require(performance["published_baseline"] == {"version": "v0.19.0", "commit": OFFICIAL_SOURCE, "binary_sha256": OFFICIAL_SHA}, "official baseline changed")
    require(performance["current"] == {"commit": CURRENT_SOURCE, "binary_sha256": CURRENT_SHA}, "current performance role changed")
    evidence = performance["artifacts"]
    require(len(evidence) == 8, "expected four primary/control report pairs")
    reports = []
    for item in evidence:
        evidence_path = ROOT / item["path"]
        require(sha256(evidence_path) == item["sha256"], f"{evidence_path}: SHA-256 mismatch")
        reports.append(load(evidence_path))
    pair_specs = [
        (66, 3, 1, "all-dev"),
        (15, 9, 2, "focused-9"),
        (6, 21, 2, "focused-21"),
        (2, 40, 2, "final-40"),
    ]
    pairs = [
        (reports[index], reports[index + 1], repos, iterations, warmups, label)
        for index, (repos, iterations, warmups, label) in zip(range(0, 8, 2), pair_specs, strict=True)
    ]
    for report, control, repos, iterations, warmups, label in pairs:
        validate_report_role(report, control, repos=repos, iterations=iterations, warmups=warmups, label=label)
        validate_bound_corpus(
            report,
            behavior["corpus"]["repositories"],
            behavior["corpus"]["manifest_sha256"],
            f"{label}:report",
        )
        validate_bound_corpus(
            control,
            behavior["corpus"]["repositories"],
            behavior["corpus"]["manifest_sha256"],
            f"{label}:control",
        )
    compare_metrics(performance["all_dev_three_iteration"], aggregate(reports[0], reports[1]), "all-dev aggregate")
    compare_metrics(performance["final_forty_iteration"], aggregate(reports[6], reports[7]), "final-40 aggregate")
    expected_surface = {repo: query_surface_adjusted(reports[6], reports[7], repo) for repo in reports[6]["repos"]}
    require(set(performance["final_query_surface_adjusted_delta_ms"]) == set(expected_surface), "query_surface repo set changed")
    for repo, value in expected_surface.items():
        close(performance["final_query_surface_adjusted_delta_ms"][repo], value, f"query_surface:{repo}")

    drift = performance["expected_drift_manifest"]
    drift_path = ROOT / drift["path"]
    require(sha256(drift_path) == drift["sha256"], "expected-drift manifest SHA-256 mismatch")
    chain = performance["escalation_chain"]
    require(len(chain) == 3, "expected the complete 3 -> 9 -> 21 -> 40 escalation chain")
    for edge in range(3):
        primary = edge * 2
        focused = primary + 2
        requested, request_output = requested_focused_repos(
            evidence[primary]["path"],
            evidence[primary + 1]["path"],
            drift["path"],
            f"escalation request {pair_specs[edge][1]} -> {pair_specs[edge + 1][1]}",
        )
        require(requested == reports[focused]["repos"], f"escalation edge {edge}: focused repo set is not exact")
        command = checker_base(evidence[primary]["path"], evidence[primary + 1]["path"], drift["path"]) + [
            "--focused-report",
            evidence[focused]["path"],
            "--focused-same-binary-control",
            evidence[focused + 1]["path"],
            "--min-focused-iterations",
            str(pair_specs[edge + 1][1]),
        ]
        expected_code = 0 if edge == 2 else 1
        focused_output = run_checker(command, f"escalation edge {edge}", expected_code)
        expected = {
            "primary_iterations": pair_specs[edge][1],
            "focused_iterations": pair_specs[edge + 1][1],
            "focused_repositories": requested,
            "request_result": request_output,
            "focused_result": focused_output,
        }
        require(chain[edge] == expected, f"escalation edge {edge}: checked chain record changed")
    require(
        chain[-1]["focused_result"] == performance["official_checker_result"],
        "official checker result text changed",
    )

    print(f"generated provenance closeout OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    report = load(ROOT / "bench/recall_loss/issue-842-official-v0.19.0-focused-r40-2026-07-14.v2.json")
    control = load(ROOT / "bench/recall_loss/issue-842-current-control-focused-r40-2026-07-14.v2.json")
    behavior = load(ROOT / "bench/labels/generated_provenance_behavior_2026_07_13.dev.v1.json")
    validate_report_role(report, control, repos=2, iterations=40, warmups=2, label="self-test-good")
    validate_bound_corpus(
        report,
        behavior["corpus"]["repositories"],
        behavior["corpus"]["manifest_sha256"],
        "self-test-good",
    )
    tampered = copy.deepcopy(report)
    tampered["provenance"]["current_binary_sha256"] = OFFICIAL_SHA
    try:
        validate_report_role(tampered, control, repos=2, iterations=40, warmups=2, label="self-test-tampered")
    except SystemExit:
        pass
    else:
        fail("role-substitution self-test was not rejected")
    tampered = copy.deepcopy(report)
    tampered["corpus"]["repositories"][0]["commit"] = "0" * 40
    try:
        validate_bound_corpus(
            tampered,
            behavior["corpus"]["repositories"],
            behavior["corpus"]["manifest_sha256"],
            "self-test-tampered-corpus",
        )
    except SystemExit:
        pass
    else:
        fail("coherent corpus-revision substitution self-test was not rejected")
    require(hashlib.sha256(b"nose").hexdigest() == "d77e22123e64d3d87f1f95d9cff7a0b6af6c32b9a81552cb90e991eb55cf63d4", "SHA self-test failed")
    print("generated provenance closeout self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        validate(args.artifact.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
