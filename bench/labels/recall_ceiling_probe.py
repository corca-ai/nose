#!/usr/bin/env python3
"""Collect and validate the current missed-worthy recovery frontier.

For every source-independent v5 worthy family missed by the maximal current
query surface, the probe records an optimistic sub-DAG / one-step-inline
ceiling.  The ceiling ignores connectedness and helper purity, so it is a way to
bound and stratify a source audit, never evidence for detector admission.

The checked artifact also freezes a language-balanced dev-only audit selection.
Detailed source judgment happens only after that selection is committed.
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
from typing import Any

from labelset import RECALL_METRIC, load_labelset, metric_eligible, sha256_file
from missed_worthy_frontier import (
    ARTIFACT_SCHEMA,
    EXPECTED_CURRENT_RECALL,
    INLINE_FLOOR,
    REQUIRED_RESIDUAL_LANES,
    ROOT,
    SELECTION_PER_LANGUAGE,
    SELECTION_SEED,
    SUBDAG_FLOORS,
    aggregate_metrics,
    candidate_sha256,
    canonical_sha256,
    load_and_validate_artifact,
    load_and_validate_decisions,
    relative_path,
    render_dev_context,
    run_self_test,
    select_dev_audit,
    validate_artifact,
)
from query_schema import QUERY_SCHEMA_VERSION, member_locations, query_families


DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"
DEFAULT_RECALL_LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v5.json"
DEFAULT_PRECISION_LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v6.json"
DEFAULT_EVALUATION = (
    ROOT / "bench" / "labels" / "product_quality_evaluation_2026_07_11.v2.json"
)
DEFAULT_CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
DEFAULT_PRUNE = ROOT / "bench" / "labels" / "prune_manifest.json"
QUERY_SCHEMA_PATH = ROOT / "bench" / "labels" / "query_schema.py"
QUERY_TIMEOUT_SECONDS = 600

ARM0_ARGS = ("--mode", "syntax,semantic")
ARM1_ARGS = (
    "--mode",
    "syntax,semantic,near",
    "--min-value",
    "0",
    "--min-members",
    "2",
)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def display_arg(value: str | Path) -> str:
    rendered = str(value)
    root_prefix = str(ROOT) + "/"
    return rendered[len(root_prefix) :] if rendered.startswith(root_prefix) else rendered


def display_command(command: list[str | Path]) -> str:
    return shlex.join([display_arg(argument) for argument in command])


def git_output(*arguments: str, cwd: Path = ROOT) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(arguments)} failed in {cwd}: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def corpus_commit_digest(repositories: list[dict[str, Any]]) -> str:
    digest = hashlib.sha256()
    for repository in sorted(repositories, key=lambda row: row["id"]):
        digest.update(
            (
                f"{repository['id']}\t{repository['split']}\t"
                f"{repository['primary_language']}\t{repository['commit']}\n"
            ).encode("utf-8")
        )
    return digest.hexdigest()


def normalized_member_locations(family: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "file": display_arg(location["file"]),
            "start_line": location["start_line"],
            "end_line": location["end_line"],
        }
        for location in member_locations(
            family, source="missed-worthy recall-ceiling query family"
        )
    ]


def overlaps(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return left["file"] == right["file"] and not (
        left["end_line"] < right["start_line"]
        or right["end_line"] < left["start_line"]
    )


def label_hit(label: dict[str, Any], families: list[list[dict[str, Any]]]) -> bool:
    return any(
        overlaps(member, reported_member)
        for family in families
        for reported_member in family
        for member in label["members"]
    )


def run_process(command: list[str | Path]) -> tuple[subprocess.CompletedProcess[bytes] | None, str | None]:
    byte_command = [str(argument) for argument in command]
    try:
        result = subprocess.run(
            byte_command,
            cwd=ROOT,
            capture_output=True,
            check=False,
            timeout=QUERY_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return None, f"timed out after {QUERY_TIMEOUT_SECONDS}s"
    return result, None


def process_record(
    command: list[str | Path],
    result: subprocess.CompletedProcess[bytes] | None,
    error: str | None,
) -> dict[str, Any]:
    if result is None:
        return {
            "command": display_command(command),
            "returncode": None,
            "stdout_sha256": sha256_bytes(b""),
            "stderr_sha256": sha256_bytes((error or "").encode("utf-8")),
            "error": error,
        }
    return {
        "command": display_command(command),
        "returncode": result.returncode,
        "stdout_sha256": sha256_bytes(result.stdout),
        "stderr_sha256": sha256_bytes(result.stderr),
    }


def run_query(
    nose: Path,
    repository: Path,
    extra_arguments: tuple[str, ...],
) -> tuple[list[list[dict[str, Any]]] | None, dict[str, Any], str | None]:
    command: list[str | Path] = [
        display_arg(nose),
        "query",
        display_arg(repository),
        "all",
        "top=0",
        "--format",
        "json",
        *extra_arguments,
    ]
    result, process_error = run_process(command)
    record = process_record(command, result, process_error)
    if result is None or result.returncode != 0:
        message = process_error or result.stderr.decode("utf-8", errors="replace").strip()
        return None, record, message or "query failed without an error message"
    try:
        families = [
            normalized_member_locations(family)
            for family in query_families(
                result.stdout.decode("utf-8"),
                source=f"missed-worthy query {display_arg(repository)}",
            )
        ]
    except (UnicodeDecodeError, ValueError) as error:
        record["parse_error"] = str(error)
        return None, record, f"query JSON validation failed: {error}"
    record["reported_families"] = len(families)
    return families, record, None


def coverage(unit: dict[str, Any], region: dict[str, Any]) -> float:
    lower = max(unit["start_line"], region["start_line"])
    upper = min(unit["end_line"], region["end_line"])
    if upper < lower:
        return 0.0
    return (upper - lower + 1) / max(
        1, region["end_line"] - region["start_line"] + 1
    )


def best_unit(
    units: list[dict[str, Any]], region: dict[str, Any]
) -> dict[str, Any] | None:
    candidates = [
        (coverage(unit, region), unit)
        for unit in units
        if display_arg(unit["path"]) == region["file"]
    ]
    candidates = [(score, unit) for score, unit in candidates if score > 0.0]
    if not candidates:
        return None
    covering = [unit for score, unit in candidates if score >= 0.5]
    if covering:
        return min(
            covering,
            key=lambda unit: (
                unit["end_line"] - unit["start_line"],
                unit["start_line"],
                unit.get("name", ""),
            ),
        )
    # Preserve the historical feature-stream tie break.  A low-coverage label
    # may cross several units; changing equal-coverage ordering would rewrite
    # the checked ceiling without any detector change.
    return max(candidates, key=lambda pair: pair[0])[1]


def unit_summary(unit: dict[str, Any]) -> dict[str, Any]:
    return {
        "file": display_arg(unit["path"]),
        "start_line": unit["start_line"],
        "end_line": unit["end_line"],
        "kind": unit.get("kind"),
        "name": unit.get("name"),
        "token_count": unit.get("token_count"),
        "value_size": len(unit["value"]),
        "pure_single_return": unit.get("pure_single_return"),
    }


def intersection_mass(left: list[int], right: list[int]) -> int:
    return sum((Counter(left) & Counter(right)).values())


def member_path(repository: Path, member_file: str) -> Path:
    marker = f"bench/repos/{repository.name}/"
    if member_file.startswith(marker):
        return repository / member_file[len(marker) :]
    if member_file.startswith("bench/repos/"):
        parts = Path(member_file).parts
        if len(parts) >= 4:
            return repository / Path(*parts[3:])
    return ROOT / member_file


def run_features(
    nose: Path,
    files: list[Path],
    feature_cache: dict[tuple[str, ...], tuple[list[dict[str, Any]] | None, str]],
    feature_runs: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]] | None, str]:
    file_keys = tuple(sorted(relative_path(path) for path in files))
    if file_keys in feature_cache:
        return feature_cache[file_keys]
    run_key = canonical_sha256(list(file_keys))[:20]
    command: list[str | Path] = [
        display_arg(nose),
        "features",
        *file_keys,
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
    ]
    result, process_error = run_process(command)
    record = process_record(command, result, process_error)
    record["files"] = list(file_keys)
    units: list[dict[str, Any]] | None = None
    if result is not None and result.returncode == 0:
        try:
            decoded = json.loads(result.stdout)
            parsed_units = decoded["units"]
            if not isinstance(parsed_units, list):
                raise ValueError("features.units is not an array")
            units = parsed_units
            record["units"] = len(units)
        except (KeyError, json.JSONDecodeError, ValueError) as error:
            record["parse_error"] = str(error)
    feature_runs[run_key] = record
    feature_cache[file_keys] = (units, run_key)
    return units, run_key


def classify_missed(
    nose: Path,
    label: dict[str, Any],
    repository: Path,
    feature_cache: dict[tuple[str, ...], tuple[list[dict[str, Any]] | None, str]],
    feature_runs: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    members = label["members"][:2]
    files = sorted({member["file"] for member in members})
    paths = [member_path(repository, member_file) for member_file in files]
    base: dict[str, Any] = {"source_files": files}
    if not all(path.is_file() for path in paths):
        missing = [relative_path(path) for path in paths if not path.is_file()]
        return dict(base, **{"class": "member-file-missing", "missing_files": missing})
    units, feature_run = run_features(nose, paths, feature_cache, feature_runs)
    base["feature_run"] = feature_run
    if units is None:
        return dict(base, **{"class": "features-failed"})
    left, right = best_unit(units, members[0]), best_unit(units, members[1])
    matched_units = [unit_summary(unit) if unit is not None else None for unit in (left, right)]
    base["matched_units"] = matched_units
    if left is None or right is None:
        return dict(base, **{"class": "no-overlapping-unit"})
    if left is right:
        return dict(
            base,
            **{
                "class": "same-unit-window",
                "enclosing_unit_lines": [left["start_line"], left["end_line"]],
            },
        )

    mass = intersection_mass(left["value"], right["value"])
    record: dict[str, Any] = dict(
        base,
        intersection_mass=mass,
        unit_value_sizes=[len(left["value"]), len(right["value"])],
    )
    for floor in SUBDAG_FLOORS:
        record[f"subdag_ge_{floor}"] = mass >= floor
    if mass >= SUBDAG_FLOORS[0]:
        record["class"] = "subdag-ceiling"
        return record

    best_augmented_mass = mass
    best_helper: dict[str, Any] | None = None
    for side, target, other in (("left", left, right), ("right", right, left)):
        siblings = [
            unit
            for unit in units
            if display_arg(unit["path"]) == display_arg(target["path"])
            and unit is not target
        ]
        base_values = Counter(target["value"])
        for sibling in siblings:
            augmented_mass = sum(
                ((base_values + Counter(sibling["value"])) & Counter(other["value"])).values()
            )
            helper_rank = (
                augmented_mass,
                -sibling["start_line"],
                sibling.get("name", ""),
            )
            previous_rank = (
                best_augmented_mass,
                -(best_helper or {}).get("unit", {}).get("start_line", 0),
                (best_helper or {}).get("unit", {}).get("name", ""),
            )
            if helper_rank > previous_rank:
                best_augmented_mass = augmented_mass
                best_helper = {"side": side, "unit": unit_summary(sibling)}
    record["inline_aug_mass"] = best_augmented_mass
    if best_helper is not None:
        record["inline_helper"] = best_helper
    record["class"] = (
        "inline-ceiling" if best_augmented_mass >= INLINE_FLOOR else "unrecovered"
    )
    return record


def source_file_records(
    repository: Path,
    members: list[dict[str, Any]],
    records: dict[str, dict[str, Any]],
) -> None:
    for member_file in sorted({member["file"] for member in members}):
        if member_file in records:
            continue
        path = member_path(repository, member_file)
        if path.is_file():
            records[member_file] = {
                "sha256": sha256_file(path),
                "size_bytes": path.stat().st_size,
            }


def tracked_input(path: Path, **metadata: Any) -> dict[str, Any]:
    return {"path": relative_path(path), "sha256": sha256_file(path), **metadata}


def repository_commit_records(
    corpus: list[dict[str, Any]],
    repos_root: Path,
    failures: list[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for repository in sorted(corpus, key=lambda row: row["id"]):
        path = repos_root / repository["id"]
        observed: str | None = None
        if path.is_dir():
            try:
                observed = git_output("rev-parse", "HEAD", cwd=path)
            except RuntimeError as error:
                failures.append(
                    {"repo": repository["id"], "stage": "repository-commit", "error": str(error)}
                )
        else:
            failures.append(
                {
                    "repo": repository["id"],
                    "stage": "repository-checkout",
                    "error": f"missing {relative_path(path)}",
                }
            )
        records[repository["id"]] = {
            "expected": repository["commit"],
            "observed": observed,
        }
        if observed is not None and observed != repository["commit"]:
            failures.append(
                {
                    "repo": repository["id"],
                    "stage": "repository-commit",
                    "error": f"observed {observed}, expected {repository['commit']}",
                }
            )
    return records


def collect(args: argparse.Namespace) -> dict[str, Any]:
    status_before = git_output("status", "--porcelain=v1", "--untracked-files=all")
    if status_before and not args.allow_dirty:
        raise SystemExit(
            "refusing to measure from a dirty worktree; commit the pipeline first "
            "or use --allow-dirty for a non-checkable diagnostic run"
        )
    if not args.nose.is_file():
        raise SystemExit(f"release binary is missing: {args.nose}")

    recall_labelset = load_labelset(args.recall_labelset)
    if recall_labelset.version != "v5":
        raise SystemExit("--recall-labelset must be the source-independent v5 pool")
    precision_labelset = load_labelset(args.precision_labelset)
    if precision_labelset.version != "v6":
        raise SystemExit("--precision-labelset must be the checked v6 composite")
    corpus_payload = json.loads(args.corpus_manifest.read_text(encoding="utf-8"))
    corpus = corpus_payload["repositories"]
    language_of = {repository["id"]: repository["primary_language"] for repository in corpus}
    failures: list[dict[str, Any]] = []
    repository_commits = repository_commit_records(corpus, args.repos_root, failures)

    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in recall_labelset.families:
        if family.get("worthy") is True and metric_eligible(family, RECALL_METRIC):
            by_repo[family["repo"]].append(family)
    repo_ids = sorted(by_repo)
    if args.limit_repositories is not None:
        repo_ids = repo_ids[: args.limit_repositories]

    query_runs: dict[str, dict[str, Any]] = {}
    feature_runs: dict[str, dict[str, Any]] = {}
    feature_cache: dict[
        tuple[str, ...], tuple[list[dict[str, Any]] | None, str]
    ] = {}
    source_files: dict[str, dict[str, Any]] = {}
    missed_records: list[dict[str, Any]] = []

    for repo_id in repo_ids:
        repository = args.repos_root / repo_id
        labels = by_repo[repo_id]
        split = labels[0]["split"]
        language = language_of[repo_id]
        arm0, arm0_record, arm0_error = run_query(args.nose, repository, ARM0_ARGS)
        arm1, arm1_record, arm1_error = run_query(args.nose, repository, ARM1_ARGS)
        run_record: dict[str, Any] = {
            "split": split,
            "language": language,
            "worthy": len(labels),
            "hit_arm0": 0,
            "hit_arm1": 0,
            "arm0": arm0_record,
            "arm1": arm1_record,
        }
        query_runs[repo_id] = run_record
        for arm_name, error in (("arm0", arm0_error), ("arm1", arm1_error)):
            if error is not None:
                failures.append(
                    {"repo": repo_id, "stage": f"query-{arm_name}", "error": error}
                )
        if arm0 is None or arm1 is None:
            continue

        run_record["hit_arm0"] = sum(label_hit(label, arm0) for label in labels)
        run_record["hit_arm1"] = sum(label_hit(label, arm1) for label in labels)
        for label in labels:
            if label_hit(label, arm1):
                continue
            classification = classify_missed(
                args.nose,
                label,
                repository,
                feature_cache,
                feature_runs,
            )
            members = [
                {
                    key: member[key]
                    for key in ("file", "start_line", "end_line")
                }
                for member in label["members"][:2]
            ]
            source_file_records(repository, members, source_files)
            record: dict[str, Any] = {
                **classification,
                "candidate_key": f"{repo_id}:{label['family_id']}",
                "family_id": label["family_id"],
                "repo": repo_id,
                "split": split,
                "language": language,
                "reason": label["reason"],
                "channel": label["channel"],
                "scope": label["scope"],
                "confidence": label.get("confidence"),
                "members": members,
            }
            record["candidate_sha256"] = candidate_sha256(record)
            missed_records.append(record)

        print(
            f"{repo_id}: worthy={len(labels)} "
            f"arm0={run_record['hit_arm0']} arm1={run_record['hit_arm1']} "
            f"missed={len(labels) - run_record['hit_arm1']}",
            file=sys.stderr,
        )

    missed_records.sort(key=lambda record: (record["repo"], record["family_id"]))
    source_files = dict(sorted(source_files.items()))
    feature_runs = dict(sorted(feature_runs.items()))
    for run_key, run in feature_runs.items():
        if run.get("returncode") != 0 or "parse_error" in run:
            failures.append(
                {
                    "stage": "features",
                    "feature_run": run_key,
                    "error": run.get("parse_error", "features command failed"),
                }
            )
    metrics_by_language, metrics = aggregate_metrics(query_runs, missed_records)
    selection_candidates = select_dev_audit(missed_records)
    selection = {
        "seed": SELECTION_SEED,
        "per_language": SELECTION_PER_LANGUAGE,
        "required_residual_lanes": list(REQUIRED_RESIDUAL_LANES),
        "policy": (
            "Reserve one deterministic dev family from each residual lane, then fill "
            "every language to five families, preferring weight-20 sub-DAG ceilings "
            "and distinct repositories. Selection uses metadata and probe measurements "
            "only; no candidate source text is consulted."
        ),
        "candidates": selection_candidates,
        "sha256": canonical_sha256(selection_candidates),
    }

    version_result = subprocess.run(
        [display_arg(args.nose), "--version"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if version_result.returncode != 0:
        raise SystemExit(f"cannot identify nose binary: {version_result.stderr.strip()}")
    prune_payload = json.loads(args.prune_manifest.read_text(encoding="utf-8"))
    invocation = ["python3", relative_path(Path(__file__)), *sys.argv[1:]]
    artifact: dict[str, Any] = {
        "schema": ARTIFACT_SCHEMA,
        "configuration": {
            "arm0": list(ARM0_ARGS),
            "arm1": list(ARM1_ARGS),
            "subdag_floors": list(SUBDAG_FLOORS),
            "inline_floor": INLINE_FLOOR,
            "query_timeout_seconds": QUERY_TIMEOUT_SECONDS,
            "limit_repositories": args.limit_repositories,
        },
        "provenance": {
            "command": display_command(invocation),
            "git_sha": git_output("rev-parse", "HEAD"),
            "working_tree_status_before_measurement": status_before,
            "nose": {
                "path": relative_path(args.nose),
                "version": version_result.stdout.strip(),
                "sha256": sha256_file(args.nose),
            },
            "query_schema_version": QUERY_SCHEMA_VERSION,
            "corpus_commit_digest": corpus_commit_digest(corpus),
            "post_prune_corpus_digest": prune_payload["corpus_digest_after_prune"],
            "repository_commits": repository_commits,
            "inputs": {
                "recall_labelset": tracked_input(
                    args.recall_labelset,
                    version="v5",
                    role="only-worthy-recall-pool",
                ),
                "precision_labelset": tracked_input(
                    args.precision_labelset,
                    version="v6",
                    role="precision-only-current-output-overlay",
                ),
                "evaluation_report": tracked_input(
                    args.evaluation_report,
                    expected_worthy_recall=EXPECTED_CURRENT_RECALL,
                ),
                "corpus_manifest": tracked_input(args.corpus_manifest),
                "prune_manifest": tracked_input(args.prune_manifest),
                "query_schema": tracked_input(QUERY_SCHEMA_PATH),
            },
        },
        "failures": sorted(
            failures,
            key=lambda failure: (
                failure.get("repo", ""),
                failure.get("stage", ""),
                failure.get("error", ""),
            ),
        ),
        "query_runs": dict(sorted(query_runs.items())),
        "feature_runs": feature_runs,
        "source_files": source_files,
        "metrics_by_language": metrics_by_language,
        "metrics": metrics,
        "dev_audit_selection": selection,
        "missed_worthy": missed_records,
    }
    return artifact


def print_report(artifact: dict[str, Any]) -> None:
    for split in ("dev", "heldout"):
        metrics = artifact["metrics"].get(split)
        if metrics is None:
            continue
        extraction_other = sum(
            metrics.get(probe_class, 0)
            for probe_class in (
                "no-overlapping-unit",
                "member-file-missing",
                "features-failed",
            )
        )
        print(f"\n=== {split} ===")
        print(
            f"worthy {metrics['worthy']} | arm0 {metrics['hit_arm0']} | "
            f"arm1 {metrics['hit_arm1']} | missed {metrics['missed_arm1']}"
        )
        print(
            "ceilings: "
            f"subdag {metrics.get('subdag-ceiling', 0)} "
            f"(>=8 {metrics.get('subdag_ge_8', 0)}, "
            f">=12 {metrics.get('subdag_ge_12', 0)}, "
            f">=20 {metrics.get('subdag_ge_20', 0)}), "
            f"inline {metrics.get('inline-ceiling', 0)}, "
            f"window {metrics.get('same-unit-window', 0)}, "
            f"unrecovered {metrics.get('unrecovered', 0)}, "
            f"extraction/other {extraction_other}"
        )
    selection = artifact["dev_audit_selection"]
    lane_counts = Counter(record["lane"] for record in selection["candidates"])
    language_counts = Counter(record["language"] for record in selection["candidates"])
    print(f"\ndev audit selection: {len(selection['candidates'])} families")
    print(f"languages: {dict(sorted(language_counts.items()))}")
    print(f"lanes: {dict(sorted(lane_counts.items()))}")
    print(f"selection SHA-256: {selection['sha256']}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--validate", type=Path, metavar="ARTIFACT")
    modes.add_argument("--validate-decisions", type=Path, metavar="DECISIONS")
    modes.add_argument("--render-dev", type=Path, metavar="ARTIFACT")
    modes.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifact", type=Path, help="source artifact for decision validation")
    parser.add_argument("--context-out", type=Path, help="output path for --render-dev")
    parser.add_argument("--context-lines", type=int, default=8)
    parser.add_argument("--check-sources", action="store_true")
    parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    parser.add_argument("--recall-labelset", type=Path, default=DEFAULT_RECALL_LABELSET)
    parser.add_argument("--precision-labelset", type=Path, default=DEFAULT_PRECISION_LABELSET)
    parser.add_argument("--evaluation-report", type=Path, default=DEFAULT_EVALUATION)
    parser.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--prune-manifest", type=Path, default=DEFAULT_PRUNE)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--limit-repositories", type=int)
    parser.add_argument("--allow-dirty", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return
    if args.validate is not None:
        artifact = load_and_validate_artifact(
            args.validate, check_sources=args.check_sources
        )
        print_report(artifact)
        print(f"\nvalidated {args.validate}")
        return
    if args.validate_decisions is not None:
        if args.artifact is None:
            raise SystemExit("--validate-decisions requires --artifact")
        load_and_validate_decisions(args.validate_decisions, args.artifact)
        print(f"validated {args.validate_decisions}")
        return
    if args.render_dev is not None:
        if args.context_out is None:
            raise SystemExit("--render-dev requires --context-out")
        artifact = load_and_validate_artifact(args.render_dev, check_sources=True)
        render_dev_context(artifact, args.context_out, args.context_lines)
        print(f"wrote dev-only context to {args.context_out}")
        return

    artifact = collect(args)
    print_report(artifact)
    if args.json_out is not None:
        args.json_out.write_text(
            json.dumps(artifact, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(f"\nwrote {args.json_out}")
    if not args.allow_dirty and args.limit_repositories is None:
        validate_artifact(artifact, check_inputs=True, check_sources=True)
        print("validated official artifact in memory")


if __name__ == "__main__":
    main()
