#!/usr/bin/env python3
"""One-time held-out stage confirmation for a frozen dev proposal.

The dev proposal is loaded and validated before any held-out detector run.  This
reuses the exact raw-stage method from ``missed_worthy_stage_audit.py`` and emits
no source snippets or human held-out judgments.  Its pre-registered gate asks
only whether the accepted-pair coverage-loss mechanism is material and spans
multiple languages; it does not tune a detector or threshold.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
from pathlib import Path
import subprocess
import sys
from typing import Any

from labelset import sha256_file
from missed_worthy_frontier import (
    DECISIONS_SCHEMA_V2,
    ROOT,
    load_and_validate_artifact,
    load_and_validate_decisions,
    relative_path,
)
from missed_worthy_stage_audit import (
    display_arg,
    display_command,
    git_output,
    run_repository,
    summarize,
    validate_repository_runs,
    validate_stage_record,
)


SCHEMA = "nose.missed_worthy_stage_confirmation.heldout.v1"
SCHEMA_V2 = "nose.missed_worthy_stage_confirmation.heldout.v2"
MIN_ACCEPTED_PAIRS = 15
MIN_ACCEPTED_LANGUAGES = 3
EXPECTED_POST_817_ACCEPTED_PAIRS = 0
EXPECTED_POST_817_CANDIDATE_SUBDAG = 2
EXPECTED_POST_817_CANDIDATE_SUBDAG_LANGUAGES = ["C", "Java"]
DEFAULT_ARTIFACT = ROOT / "bench" / "labels" / "recall_ceiling_probe_2026_07_11.v2.json"
DEFAULT_DECISIONS = (
    ROOT / "bench" / "labels" / "missed_worthy_audit_decisions_2026_07_11.dev.v1.json"
)
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"


def confirmation_result(summary: dict[str, Any]) -> dict[str, Any]:
    accepted = summary["states"].get("accepted-pair", 0)
    languages = sorted(
        language
        for language, counts in summary["by_language"].items()
        if counts.get("accepted-pair", 0) > 0
    )
    return {
        "min_accepted_pairs": MIN_ACCEPTED_PAIRS,
        "min_accepted_languages": MIN_ACCEPTED_LANGUAGES,
        "observed_accepted_pairs": accepted,
        "observed_accepted_languages": languages,
        "passed": accepted >= MIN_ACCEPTED_PAIRS
        and len(languages) >= MIN_ACCEPTED_LANGUAGES,
    }


def confirmation_result_v2(
    summary: dict[str, Any],
    records: list[dict[str, Any]],
    source_candidates: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    """Check the pre-registered post-#817 mechanical stage expectation."""
    candidate_only_subdag = []
    for record in records:
        source = source_candidates[record["candidate_key"]]
        if (
            record["stage"] == "candidate-only"
            and source["class"] == "subdag-ceiling"
            and source.get("subdag_ge_20") is True
        ):
            candidate_only_subdag.append(record["candidate_key"])
    languages = sorted(
        {source_candidates[key]["language"] for key in candidate_only_subdag}
    )
    accepted = summary["states"].get("accepted-pair", 0)
    result = {
        "expected_accepted_pairs": EXPECTED_POST_817_ACCEPTED_PAIRS,
        "expected_candidate_only_subdag_ge20": EXPECTED_POST_817_CANDIDATE_SUBDAG,
        "expected_candidate_only_subdag_ge20_languages": (
            EXPECTED_POST_817_CANDIDATE_SUBDAG_LANGUAGES
        ),
        "observed_accepted_pairs": accepted,
        "observed_candidate_only_subdag_ge20": len(candidate_only_subdag),
        "observed_candidate_only_subdag_ge20_ids": sorted(candidate_only_subdag),
        "observed_candidate_only_subdag_ge20_languages": languages,
    }
    result["passed"] = (
        accepted == EXPECTED_POST_817_ACCEPTED_PAIRS
        and len(candidate_only_subdag) == EXPECTED_POST_817_CANDIDATE_SUBDAG
        and languages == EXPECTED_POST_817_CANDIDATE_SUBDAG_LANGUAGES
    )
    return result


def collect(args: argparse.Namespace) -> dict[str, Any]:
    status = git_output("status", "--porcelain=v1", "--untracked-files=all")
    if status and not args.allow_dirty:
        raise SystemExit("refusing to confirm from a dirty worktree")
    source = load_and_validate_artifact(args.artifact, check_sources=True)
    decisions = load_and_validate_decisions(args.decisions, args.artifact)
    proposal = decisions["dev_proposal"]
    if proposal["status"] != "frozen-before-heldout-confirmation":
        raise SystemExit("dev proposal is not frozen")
    if proposal["heldout_source_confirmation"] != "not-run":
        raise SystemExit("dev artifact says held-out confirmation already ran")

    heldout = [
        record for record in source["missed_worthy"] if record["split"] == "heldout"
    ]
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in heldout:
        by_repo[record["repo"]].append(record)
    runs: dict[str, dict[str, Any]] = {}
    records: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for repo in sorted(by_repo):
        run, audited, failure = run_repository(
            args.nose, args.repos_root, repo, by_repo[repo]
        )
        runs[repo] = run
        records.extend(audited)
        if failure is not None:
            failures.append(failure)
        print(
            f"{repo}: misses={len(by_repo[repo])} "
            f"accepted={sum(record['direct_accepted'] for record in audited)}",
            file=sys.stderr,
        )
    records.sort(key=lambda record: record["candidate_key"])
    summary = summarize(records)
    source_candidates = {record["candidate_key"]: record for record in heldout}
    is_post_817 = decisions.get("schema") == DECISIONS_SCHEMA_V2
    result = (
        confirmation_result_v2(summary, records, source_candidates)
        if is_post_817
        else confirmation_result(summary)
    )
    version = subprocess.run(
        [display_arg(args.nose), "--version"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    invocation = ["python3", relative_path(Path(__file__)), *sys.argv[1:]]
    return {
        "schema": SCHEMA_V2 if is_post_817 else SCHEMA,
        "split": "heldout",
        "method": {
            "source_review": "none",
            "threshold_tuning": "none",
            "reused_dev_method": "nose detect --candidates direct extracted/candidate/accepted stages",
        },
        "provenance": {
            "command": display_command(invocation),
            "git_sha": git_output("rev-parse", "HEAD"),
            "working_tree_status_before_measurement": status,
            "nose": {
                "path": relative_path(args.nose),
                "version": version.stdout.strip(),
                "sha256": sha256_file(args.nose),
            },
            "source_artifact": {
                "path": relative_path(args.artifact),
                "sha256": sha256_file(args.artifact),
            },
            "frozen_dev_decisions": {
                "path": relative_path(args.decisions),
                "sha256": sha256_file(args.decisions),
                "commit": git_output("log", "-1", "--format=%H", "--", relative_path(args.decisions)),
                (
                    "selected_tranche" if is_post_817 else "route"
                ): proposal["selected_tranche"] if is_post_817 else proposal["route"],
            },
        },
        "failures": failures,
        "repository_runs": dict(sorted(runs.items())),
        "summary": summary,
        "confirmation_gate": result,
        "candidates": records,
    }


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(payload: object, args: argparse.Namespace, *, check_binary: bool = False) -> None:
    require(isinstance(payload, dict), "confirmation must be an object")
    schema = payload.get("schema")
    require(schema in {SCHEMA, SCHEMA_V2}, "unsupported confirmation schema")
    require(payload.get("split") == "heldout", "confirmation must be held-out")
    provenance = payload.get("provenance")
    require(isinstance(provenance, dict), "missing provenance")
    require(
        provenance.get("working_tree_status_before_measurement") == "",
        "official confirmation was not run from a clean worktree",
    )
    source_record = provenance.get("source_artifact")
    require(isinstance(source_record, dict), "missing source artifact")
    require(source_record.get("path") == relative_path(args.artifact), "source path drifted")
    require(source_record.get("sha256") == sha256_file(args.artifact), "source hash drifted")
    source = load_and_validate_artifact(args.artifact)
    decision_record = provenance.get("frozen_dev_decisions")
    require(isinstance(decision_record, dict), "missing frozen dev decisions")
    require(decision_record.get("path") == relative_path(args.decisions), "decision path drifted")
    require(decision_record.get("sha256") == sha256_file(args.decisions), "decision hash drifted")
    decisions = load_and_validate_decisions(args.decisions, args.artifact)
    is_post_817 = decisions.get("schema") == DECISIONS_SCHEMA_V2
    require(
        schema == (SCHEMA_V2 if is_post_817 else SCHEMA),
        "confirmation schema does not match the dev decision contract",
    )
    if is_post_817:
        require(
            decision_record.get("selected_tranche")
            == decisions["dev_proposal"]["selected_tranche"],
            "selected tranche drifted",
        )
    else:
        require(decision_record.get("route") == decisions["dev_proposal"]["route"], "route drifted")
    require(payload.get("failures") == [], "confirmation contains failures")
    require(payload.get("method", {}).get("source_review") == "none", "held-out source was judged")

    expected = {
        record["candidate_key"]: record
        for record in source["missed_worthy"]
        if record["split"] == "heldout"
    }
    records = payload.get("candidates")
    require(isinstance(records, list), "candidate records missing")
    require(
        [record.get("candidate_key") for record in records] == sorted(expected),
        "confirmation does not exactly cover held-out misses",
    )
    for record in records:
        key = record["candidate_key"]
        validate_stage_record(record, expected[key])
    require(payload.get("summary") == summarize(records), "confirmation summary drifted")
    expected_gate = (
        confirmation_result_v2(payload["summary"], records, expected)
        if is_post_817
        else confirmation_result(payload["summary"])
    )
    require(payload.get("confirmation_gate") == expected_gate, "confirmation gate drifted")
    require(payload["confirmation_gate"]["passed"] is True, "held-out confirmation failed")
    validate_repository_runs(payload.get("repository_runs"), expected)
    if check_binary:
        nose = provenance["nose"]
        binary = ROOT / nose["path"]
        require(binary.is_file() and sha256_file(binary) == nose["sha256"], "binary hash drifted")


def run_self_test() -> None:
    passing = {
        "states": {"accepted-pair": 15},
        "by_language": {
            "C": {"accepted-pair": 5},
            "Go": {"accepted-pair": 5},
            "Rust": {"accepted-pair": 5},
        },
    }
    require(confirmation_result(passing)["passed"] is True, "passing gate rejected")
    failing = {
        "states": {"accepted-pair": 14},
        "by_language": {
            "C": {"accepted-pair": 7},
            "Rust": {"accepted-pair": 7},
        },
    }
    require(confirmation_result(failing)["passed"] is False, "failing gate accepted")

    post_817_candidates = {
        "c-one": {
            "candidate_key": "c-one",
            "language": "C",
            "class": "subdag-ceiling",
            "subdag_ge_20": True,
        },
        "java-one": {
            "candidate_key": "java-one",
            "language": "Java",
            "class": "subdag-ceiling",
            "subdag_ge_20": True,
        },
    }
    post_817_records = [
        {"candidate_key": key, "stage": "candidate-only"}
        for key in post_817_candidates
    ]
    post_817_summary = {
        "states": {"candidate-only": 2},
        "by_language": {"C": {"candidate-only": 1}, "Java": {"candidate-only": 1}},
    }
    require(
        confirmation_result_v2(
            post_817_summary,
            post_817_records,
            post_817_candidates,
        )["passed"]
        is True,
        "post-817 passing gate rejected",
    )
    post_817_records[0]["stage"] = "accepted-pair"
    post_817_summary["states"] = {"accepted-pair": 1, "candidate-only": 1}
    require(
        confirmation_result_v2(
            post_817_summary,
            post_817_records,
            post_817_candidates,
        )["passed"]
        is False,
        "post-817 accepted-pair drift passed",
    )
    print("missed-worthy held-out confirmation self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--validate", type=Path, metavar="CONFIRMATION")
    modes.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    parser.add_argument("--decisions", type=Path, default=DEFAULT_DECISIONS)
    parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--check-binary", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return
    if args.validate is not None:
        payload = json.loads(args.validate.read_text())
        validate(payload, args, check_binary=args.check_binary)
        print(json.dumps(payload["summary"], indent=2, sort_keys=True))
        print(json.dumps(payload["confirmation_gate"], indent=2, sort_keys=True))
        print(f"validated {args.validate}")
        return
    payload = collect(args)
    if args.json_out is not None:
        args.json_out.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {args.json_out}")
    print(json.dumps(payload["summary"], indent=2, sort_keys=True))
    print(json.dumps(payload["confirmation_gate"], indent=2, sort_keys=True))
    if not args.allow_dirty:
        validate(payload, args, check_binary=True)
        print("validated official held-out confirmation in memory")


if __name__ == "__main__":
    main()
