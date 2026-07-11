#!/usr/bin/env python3
"""Audit which detector stage owns each current dev missed-worthy family.

This is a dev-only diagnostic layered on the frozen recall-ceiling artifact.  It
runs the raw structural candidate surface once per dev repository, then asks
whether each missed label has extracted units, a direct candidate edge, and a
direct accepted pair.  It deliberately does not read or emit held-out source.

``nose detect --candidates`` does not include query's syntax channel or its
additional shape-candidate arm, so accepted-pair counts are a conservative
structural witness, not a complete simulation of query family presentation.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
from typing import Any

from labelset import sha256_file
from missed_worthy_frontier import (
    ROOT,
    canonical_sha256,
    load_and_validate_artifact,
    relative_path,
)


SCHEMA = "nose.missed_worthy_stage_audit.dev.v1"
DEFAULT_ARTIFACT = ROOT / "bench" / "labels" / "recall_ceiling_probe_2026_07_11.v2.json"
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"
STATES = {
    "accepted-pair",
    "candidate-only",
    "extracted-no-candidate",
    "missing-unit",
}


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def display_arg(value: str | Path) -> str:
    rendered = str(value)
    prefix = str(ROOT) + "/"
    return rendered[len(prefix) :] if rendered.startswith(prefix) else rendered


def display_command(command: list[str | Path]) -> str:
    return shlex.join([display_arg(argument) for argument in command])


def git_output(*arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()


def normalized_member(repo: str, member: dict[str, Any]) -> dict[str, Any]:
    prefix = f"bench/repos/{repo}/"
    path = member["file"]
    return {
        **member,
        "file": path[len(prefix) :] if path.startswith(prefix) else path,
    }


def overlaps(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return left["file"] == right["file"] and not (
        left["end_line"] < right["start_line"]
        or right["end_line"] < left["start_line"]
    )


def audit_state(extracted_counts: list[int], candidate: bool, accepted: bool) -> str:
    if accepted:
        return "accepted-pair"
    if candidate:
        return "candidate-only"
    if all(count > 0 for count in extracted_counts):
        return "extracted-no-candidate"
    return "missing-unit"


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    states: Counter[str] = Counter()
    by_language: dict[str, Counter[str]] = defaultdict(Counter)
    by_probe_class: dict[str, Counter[str]] = defaultdict(Counter)
    for record in records:
        state = record["stage"]
        states[state] += 1
        by_language[record["language"]][state] += 1
        by_probe_class[record["probe_class"]][state] += 1
    return {
        "total": len(records),
        "states": dict(sorted(states.items())),
        "by_language": {
            language: dict(sorted(counts.items()))
            for language, counts in sorted(by_language.items())
        },
        "by_probe_class": {
            probe_class: dict(sorted(counts.items()))
            for probe_class, counts in sorted(by_probe_class.items())
        },
    }


def run_repository(
    nose: Path,
    repos_root: Path,
    repo: str,
    candidates: list[dict[str, Any]],
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any] | None]:
    with tempfile.TemporaryDirectory(prefix="nose-missed-worthy-stage-") as directory:
        dump = Path(directory)
        command: list[str | Path] = [
            display_arg(nose),
            "detect",
            "--candidates",
            "--dump",
            dump,
            "--repos-root",
            display_arg(repos_root),
            display_arg(repos_root / repo),
        ]
        result = subprocess.run(
            [str(argument) for argument in command],
            cwd=ROOT,
            capture_output=True,
            check=False,
        )
        normalized_stderr = result.stderr.replace(
            str(dump).encode("utf-8"), b"<temporary-dump>"
        )
        run_record: dict[str, Any] = {
            "command": display_command(command).replace(str(dump), "<temporary-dump>"),
            "returncode": result.returncode,
            "stdout_sha256": sha256_bytes(result.stdout),
            "stderr_sha256": sha256_bytes(normalized_stderr),
        }
        if result.returncode != 0:
            error = result.stderr.decode("utf-8", errors="replace").strip()
            return run_record, [], {"repo": repo, "stage": "detect", "error": error}

        paths = {
            name: dump / name
            for name in ("units.json", "candidates.json", "predictions.json")
        }
        try:
            units_payload = json.loads(paths["units.json"].read_text())
            candidate_payload = json.loads(paths["candidates.json"].read_text())
            prediction_payload = json.loads(paths["predictions.json"].read_text())
            units = units_payload["units"]
            pairs = {tuple(pair) for pair in candidate_payload["candidates"]}
            predictions = prediction_payload["duplicates"]
        except (OSError, KeyError, json.JSONDecodeError, TypeError) as error:
            return run_record, [], {
                "repo": repo,
                "stage": "read-dump",
                "error": str(error),
            }
        run_record["dump"] = {
            name: {"sha256": sha256_file(path), "size_bytes": path.stat().st_size}
            for name, path in sorted(paths.items())
        }
        run_record["counts"] = {
            "units": len(units),
            "candidate_pairs": len(pairs),
            "accepted_pairs": len(predictions),
        }

        audited: list[dict[str, Any]] = []
        for candidate in candidates:
            members = [normalized_member(repo, member) for member in candidate["members"]]
            sides = [
                {
                    index
                    for index, unit in enumerate(units)
                    if overlaps(member, unit)
                }
                for member in members
            ]
            direct_candidate = any(
                (min(left, right), max(left, right)) in pairs
                for left in sides[0]
                for right in sides[1]
                if left != right
            )
            direct_accepted = any(
                (
                    overlaps(members[0], prediction["left"])
                    and overlaps(members[1], prediction["right"])
                )
                or (
                    overlaps(members[0], prediction["right"])
                    and overlaps(members[1], prediction["left"])
                )
                for prediction in predictions
            )
            counts = [len(side) for side in sides]
            audited.append(
                {
                    "candidate_key": candidate["candidate_key"],
                    "candidate_sha256": candidate["candidate_sha256"],
                    "repo": repo,
                    "language": candidate["language"],
                    "probe_class": candidate["class"],
                    "extracted_unit_counts": counts,
                    "direct_candidate": direct_candidate,
                    "direct_accepted": direct_accepted,
                    "stage": audit_state(counts, direct_candidate, direct_accepted),
                }
            )
        return run_record, audited, None


def collect(args: argparse.Namespace) -> dict[str, Any]:
    status = git_output("status", "--porcelain=v1", "--untracked-files=all")
    if status and not args.allow_dirty:
        raise SystemExit("refusing to run the official dev stage audit from a dirty worktree")
    source = load_and_validate_artifact(args.artifact, check_sources=True)
    dev = [record for record in source["missed_worthy"] if record["split"] == "dev"]
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in dev:
        by_repo[record["repo"]].append(record)

    runs: dict[str, dict[str, Any]] = {}
    audited: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for repo in sorted(by_repo):
        run, records, failure = run_repository(
            args.nose, args.repos_root, repo, by_repo[repo]
        )
        runs[repo] = run
        audited.extend(records)
        if failure is not None:
            failures.append(failure)
        print(
            f"{repo}: misses={len(by_repo[repo])} "
            f"accepted={sum(record['direct_accepted'] for record in records)}",
            file=sys.stderr,
        )
    audited.sort(key=lambda record: record["candidate_key"])

    version = subprocess.run(
        [display_arg(args.nose), "--version"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    invocation = ["python3", relative_path(Path(__file__)), *sys.argv[1:]]
    artifact = {
        "schema": SCHEMA,
        "split": "dev",
        "method": {
            "surface": "nose detect --candidates",
            "interpretation": (
                "A direct accepted pair is already admitted by the raw structural detector. "
                "Because query adds syntax and shape-candidate arms, this is a conservative "
                "accepted-pair witness, not a complete query simulation."
            ),
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
            "selection_sha256": source["dev_audit_selection"]["sha256"],
        },
        "failures": failures,
        "repository_runs": dict(sorted(runs.items())),
        "summary": summarize(audited),
        "candidates": audited,
    }
    return artifact


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(payload: object, artifact_path: Path, *, check_binary: bool = False) -> None:
    require(isinstance(payload, dict), "stage audit must be an object")
    require(payload.get("schema") == SCHEMA, "unsupported stage-audit schema")
    require(payload.get("split") == "dev", "stage audit must be dev-only")
    provenance = payload.get("provenance")
    require(isinstance(provenance, dict), "missing provenance")
    require(
        provenance.get("working_tree_status_before_measurement") == "",
        "official stage audit was not measured from a clean worktree",
    )
    source_record = provenance.get("source_artifact")
    require(isinstance(source_record, dict), "missing source artifact")
    require(source_record.get("path") == relative_path(artifact_path), "source path drifted")
    require(source_record.get("sha256") == sha256_file(artifact_path), "source hash drifted")
    source = load_and_validate_artifact(artifact_path)
    require(
        provenance.get("selection_sha256") == source["dev_audit_selection"]["sha256"],
        "selection hash drifted",
    )
    require(payload.get("failures") == [], "stage audit contains failures")
    nose = provenance.get("nose")
    require(isinstance(nose, dict) and nose.get("version") == "nose 0.18.0", "bad nose version")
    if check_binary:
        binary = ROOT / nose["path"]
        require(binary.is_file(), "recorded nose binary is missing")
        require(sha256_file(binary) == nose["sha256"], "nose binary hash drifted")

    source_dev = {
        record["candidate_key"]: record
        for record in source["missed_worthy"]
        if record["split"] == "dev"
    }
    records = payload.get("candidates")
    require(isinstance(records, list), "candidates must be an array")
    require(
        [record.get("candidate_key") for record in records] == sorted(source_dev),
        "stage records do not exactly cover dev misses in canonical order",
    )
    for record in records:
        key = record["candidate_key"]
        require(record.get("candidate_sha256") == source_dev[key]["candidate_sha256"], f"{key}: hash drift")
        counts = record.get("extracted_unit_counts")
        require(
            isinstance(counts, list)
            and len(counts) == 2
            and all(isinstance(count, int) and count >= 0 for count in counts),
            f"{key}: invalid unit counts",
        )
        candidate = record.get("direct_candidate")
        accepted = record.get("direct_accepted")
        require(isinstance(candidate, bool) and isinstance(accepted, bool), f"{key}: bad flags")
        require(not accepted or candidate, f"{key}: accepted pair was not a candidate")
        require(record.get("stage") in STATES, f"{key}: invalid stage")
        require(
            record["stage"] == audit_state(counts, candidate, accepted),
            f"{key}: stage disagrees with evidence",
        )
    require(payload.get("summary") == summarize(records), "summary drifted")

    runs = payload.get("repository_runs")
    require(isinstance(runs, dict), "repository runs missing")
    require(set(runs) == {record["repo"] for record in source_dev.values()}, "repository run set drifted")
    for repo, run in runs.items():
        require(run.get("returncode") == 0, f"{repo}: detector run failed")
        require(isinstance(run.get("counts"), dict), f"{repo}: counts missing")
        dump = run.get("dump")
        require(isinstance(dump, dict) and len(dump) == 3, f"{repo}: dump provenance missing")


def run_self_test() -> None:
    require(audit_state([1, 1], True, True) == "accepted-pair", "accepted state")
    require(audit_state([1, 1], True, False) == "candidate-only", "candidate state")
    require(
        audit_state([1, 1], False, False) == "extracted-no-candidate",
        "extracted state",
    )
    require(audit_state([1, 0], False, False) == "missing-unit", "missing state")
    rows = [
        {
            "stage": "accepted-pair",
            "language": "Rust",
            "probe_class": "subdag-ceiling",
        },
        {
            "stage": "candidate-only",
            "language": "Rust",
            "probe_class": "subdag-ceiling",
        },
    ]
    expected = {
        "total": 2,
        "states": {"accepted-pair": 1, "candidate-only": 1},
        "by_language": {"Rust": {"accepted-pair": 1, "candidate-only": 1}},
        "by_probe_class": {
            "subdag-ceiling": {"accepted-pair": 1, "candidate-only": 1}
        },
    }
    require(summarize(rows) == expected, "summary self-test drifted")
    require(len(canonical_sha256(expected)) == 64, "canonical hash self-test")
    print("missed-worthy stage audit self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--validate", type=Path, metavar="STAGE_ARTIFACT")
    modes.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
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
        validate(payload, args.artifact, check_binary=args.check_binary)
        print(json.dumps(payload["summary"], indent=2, sort_keys=True))
        print(f"validated {args.validate}")
        return
    payload = collect(args)
    if args.json_out is not None:
        args.json_out.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {args.json_out}")
    print(json.dumps(payload["summary"], indent=2, sort_keys=True))
    if not args.allow_dirty:
        validate(payload, args.artifact, check_binary=True)
        print("validated official dev stage audit in memory")


if __name__ == "__main__":
    main()
