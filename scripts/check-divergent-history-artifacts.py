#!/usr/bin/env python3
"""Validate checked divergent-history evidence artifacts."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
HISTORY_ARTIFACT = ROOT / "bench/divergent_history/issue-687-cli-tests-2026-07-06.v1.json"
PILOT_ARTIFACT = ROOT / "bench/divergent_history/issue-687-maintainer-pilot-2026-07-06.v1.json"
ISSUE_688_SUMMARY = (
    ROOT / "bench/divergent_history/issue-688-product-output-runtime-summary-2026-07-06.v1.json"
)
SOURCE_BEARING_KEYS = {
    "base_code",
    "change_diff",
    "current_code",
    "diff",
    "patch",
    "snippet",
    "snippets",
    "source_text",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    require(path.exists(), f"missing artifact: {path.relative_to(ROOT)}")
    return json.loads(path.read_text())


def find_source_bearing_keys(value: Any, path: str = "$") -> list[str]:
    matches: list[str] = []
    if isinstance(value, dict):
        for key, nested in value.items():
            next_path = f"{path}.{key}"
            if key in SOURCE_BEARING_KEYS:
                matches.append(next_path)
            matches.extend(find_source_bearing_keys(nested, next_path))
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            matches.extend(find_source_bearing_keys(nested, f"{path}[{index}]"))
    return matches


def validate_issue_687() -> None:
    subprocess.run(
        [
            sys.executable,
            "scripts/divergent-history-mining.py",
            "--check-artifact",
            HISTORY_ARTIFACT.relative_to(ROOT).as_posix(),
        ],
        cwd=ROOT,
        check=True,
    )

    history = load_json(HISTORY_ARTIFACT)
    pilot = load_json(PILOT_ARTIFACT)
    require(
        pilot.get("schema") == "nose.divergent_gate_pilot.v1",
        "pilot schema mismatch",
    )
    require(
        pilot.get("bounds", {}).get("pr_ci_required") is False,
        "pilot must not be a required PR CI claim",
    )
    require(
        pilot.get("maintainer_disposition", {}).get("default_on_readiness_claim") is False,
        "pilot must not claim default-on readiness",
    )
    require(
        pilot.get("source_policy", {}).get("raw_query_tracked") is False,
        "pilot raw query must stay outside git",
    )
    recorded_history = pilot.get("history_artifact", {})
    require(
        recorded_history.get("sha256") == sha256_file(HISTORY_ARTIFACT),
        "pilot history artifact sha256 is stale",
    )
    require(
        recorded_history.get("summary") == history.get("summary"),
        "pilot history summary is stale",
    )
    raw_query = pilot.get("observe_only_run", {}).get("raw_query", {})
    require(
        raw_query.get("tracked") is False
        and str(raw_query.get("path", "")).startswith("target/"),
        "pilot raw query path must be an untracked target/ artifact",
    )


def referenced_artifact(row: dict[str, Any]) -> tuple[Path, dict[str, Any]]:
    path = ROOT / row["path"]
    require(row.get("sha256") == sha256_file(path), f"stale sha: {row['path']}")
    return path, load_json(path)


def validate_query_regression(row: dict[str, Any]) -> None:
    _path, data = referenced_artifact(row)
    stability = row.get("stability") or {}
    require(stability.get("hashes_identical") is True, f"hash drift in {row['path']}")
    require(stability.get("bytes_identical") is True, f"byte-count drift in {row['path']}")
    require(stability.get("families_identical") is True, f"family-count drift in {row['path']}")
    require(stability.get("internally_deterministic") is True, f"nondeterministic run in {row['path']}")
    by_repo = data.get("summary", {}).get("by_repo", {})
    require(len(by_repo) == stability.get("repos"), f"repo count mismatch in {row['path']}")
    for repo, labels in by_repo.items():
        baseline = labels["baseline"]
        current = labels["current"]
        require(baseline["hashes"] == current["hashes"], f"{row['path']} {repo} hash drift")
        require(baseline["bytes"] == current["bytes"], f"{row['path']} {repo} byte drift")
        require(baseline["families"] == current["families"], f"{row['path']} {repo} family drift")
        for key in ("hashes", "bytes", "families"):
            require(
                len(baseline[key]) == 1,
                f"{row['path']} {repo} baseline {key} nondeterministic",
            )
            require(
                len(current[key]) == 1,
                f"{row['path']} {repo} current {key} nondeterministic",
            )


def validate_replay_pair(summary: dict[str, Any]) -> None:
    replay = summary["base_replay_runtime"]
    for label in ("baseline", "current"):
        _path, data = referenced_artifact(replay[label])
        require(data.get("schema_version") == 2, f"replay schema mismatch: {replay[label]['path']}")
        for arm, arm_row in data.get("per_arm", {}).items():
            require(arm_row.get("errors") == 0, f"replay errors in {replay[label]['path']} {arm}")

    for arm, row in replay["comparison"].items():
        require(row.get("counts_identical") is True, f"replay counts drift for {arm}")
        for key in ("divergence_s_p50", "divergence_s_p90"):
            require(
                row["timing_delta_pct"][key] < 20.0,
                f"replay {arm} {key} exceeded 20% threshold",
            )


def validate_issue_688() -> None:
    summary = load_json(ISSUE_688_SUMMARY)
    require(
        summary.get("schema") == "nose.divergent_gate_product_runtime.v1",
        "issue 688 summary schema mismatch",
    )
    require(
        summary.get("bounds", {}).get("default_on_readiness_claim") is False,
        "issue 688 must not claim default-on readiness",
    )
    require(
        summary.get("bounds", {}).get("final_epic_closeout") is False,
        "issue 688 must not claim final epic closeout",
    )
    non_base = summary["non_base_product_output"]
    validate_query_regression(non_base["allrepos"])
    validate_query_regression(non_base["same_binary_control"])
    validate_query_regression(non_base["nose_slice"])
    require(
        non_base["interpretation"].get("runtime_triage_required") is False,
        "issue 688 unexpectedly requires runtime triage",
    )
    validate_replay_pair(summary)
    conclusion = summary["conclusion"]
    for key in (
        "non_base_hashes_identical_all_repos",
        "non_base_bytes_identical_all_repos",
        "non_base_families_identical_all_repos",
        "non_base_hashes_identical_nose_slice",
        "base_replay_counts_identical",
    ):
        require(conclusion.get(key) is True, f"issue 688 conclusion {key} is not true")
    require(conclusion.get("unexplained_runtime_regression") is False, "unexplained runtime regression")
    require(conclusion.get("runtime_triage_opened") is False, "runtime triage should not be opened")
    require(conclusion.get("optimization_opened") is False, "optimization should not be opened")
    require(
        not find_source_bearing_keys(summary),
        "issue 688 summary contains source-bearing keys",
    )


def main() -> int:
    validate_issue_687()
    validate_issue_688()
    print("divergent history checked artifacts OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
