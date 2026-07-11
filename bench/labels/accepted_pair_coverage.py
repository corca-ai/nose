#!/usr/bin/env python3
"""Census direct structural accepted-edge coverage in final query output.

The raw detector is the conservative accepted-edge witness frozen by issue #816.
For each dev repository this script compares every distinct, non-nested accepted
pair with both the detector's union-find groups and the final ``query all`` list.
The complete classification is frozen by a digest; a deterministic bounded
sample of edges that disappear after grouping is retained for stage tracing.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import heapq
import json
import os
from pathlib import Path
import subprocess
import tempfile
from typing import Any, Iterable

from query_schema import decode_query_payload


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "nose.accepted_pair_coverage.dev.v1"
DEFAULT_CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
DEFAULT_REPOS = ROOT / "bench" / "repos"
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
DEFAULT_BASELINE = ROOT / "bench" / "labels" / "recall_ceiling_probe_2026_07_11.v2.json"
QUERY_ARGS = ("all", "top=0", "--min-value", "0", "--min-members", "2", "--format", "json")
DETECT_ARGS = ("--query-accepted", "--min-lines", "5", "--min-tokens", "24")
LOST_EDGE_SAMPLE_PER_REPOSITORY = 200


def canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_output(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, text=True, capture_output=True
    ).stdout.strip()


def clean_environment() -> dict[str, str]:
    return {key: value for key, value in os.environ.items() if not key.startswith("NOSE_")}


def display_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def repository_file(value: str, repository: Path) -> str:
    path = Path(value)
    if not path.is_absolute():
        path = ROOT / path
    try:
        return path.resolve().relative_to(repository.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def normalized_location(location: dict[str, Any], repository: Path) -> dict[str, Any]:
    start = location.get("start", location.get("start_line"))
    end = location.get("end", location.get("end_line"))
    if not isinstance(start, int) or not isinstance(end, int) or start < 1 or end < start:
        raise ValueError(f"invalid location range: {location!r}")
    return {
        "file": repository_file(str(location["file"]), repository),
        "start_line": start,
        "end_line": end,
    }


def canonical_edge(left: dict[str, Any], right: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    key = lambda loc: (loc["file"], loc["start_line"], loc["end_line"])
    return (left, right) if key(left) <= key(right) else (right, left)


def distinct_non_nested(left: dict[str, Any], right: dict[str, Any]) -> bool:
    if left["file"] != right["file"]:
        return True
    if left == right:
        return False
    return max(left["start_line"], right["start_line"]) > min(
        left["end_line"], right["end_line"]
    )


def generated_path(path: str) -> bool:
    lowered = f"/{path.lower()}"
    return any(
        marker in lowered
        for marker in (
            "vendor/",
            "third_party/",
            "third-party/",
            "/deps/",
            "node_modules/",
            "/dist/",
            "/build/",
            ".min.",
            ".pb.",
            "_pb2",
            ".g.dart",
            ".d.ts",
            "generated/",
            "/gen/",
            ".generated.",
        )
    )


class CoverageIndex:
    """File-bucketed, endpoint-cached family coverage lookup."""

    def __init__(self, families: Iterable[dict[str, Any]], repository: Path, *, query: bool):
        self.records: dict[str, dict[str, Any]] = {}
        self.by_file: dict[str, list[tuple[int, int, str, int]]] = defaultdict(list)
        self.cache: dict[tuple[str, int, int], dict[str, set[int]]] = {}
        for index, family in enumerate(families):
            family_id = str(family.get("id", index))
            locations = [normalized_location(loc, repository) for loc in family["locations" if query else "members"]]
            record = {
                "id": family_id,
                "surface": family.get("surface") if query else None,
                "locations": locations,
            }
            self.records[family_id] = record
            for member_index, loc in enumerate(locations):
                self.by_file[loc["file"]].append(
                    (loc["start_line"], loc["end_line"], family_id, member_index)
                )
        for intervals in self.by_file.values():
            intervals.sort()

    def endpoint_matches(self, endpoint: dict[str, Any]) -> dict[str, set[int]]:
        key = (endpoint["file"], endpoint["start_line"], endpoint["end_line"])
        cached = self.cache.get(key)
        if cached is not None:
            return cached
        required = (endpoint["end_line"] - endpoint["start_line"] + 2) // 2
        matches: dict[str, set[int]] = defaultdict(set)
        for start, end, family_id, member_index in self.by_file.get(endpoint["file"], ()):
            if start > endpoint["end_line"]:
                break
            overlap = min(end, endpoint["end_line"]) - max(start, endpoint["start_line"]) + 1
            if overlap >= required:
                matches[family_id].add(member_index)
        result = dict(matches)
        self.cache[key] = result
        return result

    def covering(self, left: dict[str, Any], right: dict[str, Any]) -> list[str]:
        left_matches = self.endpoint_matches(left)
        right_matches = self.endpoint_matches(right)
        covered = []
        covered.extend(left_matches.keys() & right_matches.keys())
        return sorted(covered)


def run_repository(
    nose: Path,
    repository: Path,
    repo: str,
    *,
    detect_nose: Path | None = None,
    comparison_nose: Path | None = None,
) -> tuple[
    dict[str, Any],
    Counter[str],
    list[dict[str, Any]],
    list[dict[str, Any]],
    str,
]:
    env = clean_environment()
    with tempfile.TemporaryDirectory(prefix=f"nose-pair-coverage-{repo}-") as tmp:
        raw_path = Path(tmp) / "detect.json"
        detect_command = [
            str(detect_nose or nose),
            "detect",
            str(repository),
            *DETECT_ARGS,
            "--out",
            str(raw_path),
        ]
        detect = subprocess.run(detect_command, cwd=ROOT, env=env, capture_output=True)
        if detect.returncode != 0:
            raise RuntimeError(detect.stderr.decode("utf-8", errors="replace").strip())
        query_command = [str(nose), "query", str(repository), *QUERY_ARGS]
        query = subprocess.run(query_command, cwd=ROOT, env=env, capture_output=True)
        if query.returncode != 0:
            raise RuntimeError(query.stderr.decode("utf-8", errors="replace").strip())

        raw = json.loads(raw_path.read_text(encoding="utf-8"))
        query_payload = decode_query_payload(
            query.stdout.decode("utf-8"), source=f"nose query {repo}"
        )
        group_index = CoverageIndex(raw["groups"], repository, query=False)
        final_index = CoverageIndex(query_payload["families"], repository, query=True)
        comparison_query = None
        comparison_index = None
        added_family_ids: set[str] = set()
        accounted_added_ids: set[str] = set()
        removed_family_ids: set[str] = set()
        if comparison_nose is not None:
            comparison_command = [
                str(comparison_nose),
                "query",
                str(repository),
                *QUERY_ARGS,
            ]
            comparison_query = subprocess.run(
                comparison_command, cwd=ROOT, env=env, capture_output=True
            )
            if comparison_query.returncode != 0:
                raise RuntimeError(
                    comparison_query.stderr.decode("utf-8", errors="replace").strip()
                )
            comparison_payload = decode_query_payload(
                comparison_query.stdout.decode("utf-8"),
                source=f"comparison nose query {repo}",
            )
            comparison_index = CoverageIndex(
                comparison_payload["families"], repository, query=True
            )
            current_ids = set(final_index.records)
            comparison_ids = set(comparison_index.records)
            added_family_ids = current_ids - comparison_ids
            removed_family_ids = comparison_ids - current_ids

        counts: Counter[str] = Counter()
        # Preserve a deterministic bounded sample for inspection. The digest below
        # still covers every raw edge in report order; retaining every row would
        # reproduce dense exact-bucket O(n²) output in the artifact itself.
        lost_sample: list[tuple[int, str, int, dict[str, Any]]] = []
        classifications = hashlib.sha256()
        for ordinal, pair in enumerate(raw["duplicates"]):
            counts["accepted_edges"] += 1
            left = normalized_location(pair["left"], repository)
            right = normalized_location(pair["right"], repository)
            left, right = canonical_edge(left, right)
            eligible = distinct_non_nested(left, right)
            component = group_index.covering(left, right) if eligible else []
            final_all = final_index.covering(left, right) if eligible else []
            comparison_all = (
                comparison_index.covering(left, right)
                if eligible and comparison_index is not None
                else []
            )
            comparison_default = (
                [
                    family_id
                    for family_id in comparison_all
                    if comparison_index.records[family_id]["surface"] == "default"
                ]
                if comparison_index is not None
                else []
            )
            final_default = [
                family_id
                for family_id in final_all
                if final_index.records[family_id]["surface"] == "default"
            ]
            if not eligible:
                state = "ineligible-same-or-nested-site"
            elif final_default:
                state = "final-default-covered"
            elif final_all:
                state = "final-non-default-covered"
            elif generated_path(left["file"]) and generated_path(right["file"]):
                state = "intentionally-suppressed-generated"
            elif component:
                state = "lost-after-component"
            else:
                state = "component-uncovered"
            counts[state] += 1
            if eligible:
                counts["eligible_edges"] += 1
                counts["component_covered"] += bool(component)
                counts["final_all_covered"] += bool(final_all)
                counts["final_default_covered"] += bool(final_default)
                if comparison_index is not None:
                    counts["comparison_final_all_covered"] += bool(comparison_all)
                    counts["comparison_final_default_covered"] += bool(comparison_default)
                    recovered = not comparison_all and bool(final_all)
                    default_recovered = not comparison_default and bool(final_default)
                    regressed = bool(comparison_all) and not final_all
                    default_regressed = bool(comparison_default) and not final_default
                    counts["recovered_edges"] += recovered
                    counts["default_recovered_edges"] += default_recovered
                    counts["regressed_edges"] += regressed
                    counts["default_regressed_edges"] += default_regressed
                    if recovered:
                        accounted_added_ids.update(added_family_ids & set(final_all))
                    if default_recovered:
                        accounted_added_ids.update(added_family_ids & set(final_default))
            classification = {
                "left": left,
                "right": right,
                "score": pair["score"],
                "state": state,
            }
            classifications.update(canonical_json(classification).encode("utf-8"))
            classifications.update(b"\n")
            if state in {"lost-after-component", "component-uncovered"}:
                edge_sha256 = sha256_bytes(canonical_json(classification).encode("utf-8"))
                row = {
                    "edge_id": edge_sha256[:16],
                    "edge_sha256": edge_sha256,
                    "ordinal": ordinal,
                    **classification,
                    "component_cover_count": len(component),
                }
                priority = int(edge_sha256, 16)
                item = (-priority, edge_sha256, ordinal, row)
                if len(lost_sample) < LOST_EDGE_SAMPLE_PER_REPOSITORY:
                    heapq.heappush(lost_sample, item)
                elif priority < -lost_sample[0][0]:
                    heapq.heapreplace(lost_sample, item)

        digest = classifications.hexdigest()
        counts["visible_families_added"] = len(added_family_ids)
        counts["visible_families_removed"] = len(removed_family_ids)
        counts["added_families_accounting_for_recovered_edge"] = len(accounted_added_ids)
        unaccounted_ids = sorted(added_family_ids - accounted_added_ids)
        counts["added_families_without_recovered_edge"] = len(unaccounted_ids)
        run = {
            "detect": {
                "command": " ".join(display_path(Path(arg)) if arg.startswith(str(ROOT)) else arg for arg in detect_command).replace(str(raw_path), "<temporary>/detect.json"),
                "returncode": detect.returncode,
                "report_sha256": sha256_file(raw_path),
                "stderr_sha256": sha256_bytes(detect.stderr),
            },
            "query": {
                "command": " ".join(display_path(Path(arg)) if arg.startswith(str(ROOT)) else arg for arg in query_command),
                "returncode": query.returncode,
                "stdout_sha256": sha256_bytes(query.stdout),
                "stderr_sha256": sha256_bytes(query.stderr),
            },
            "raw_groups": len(raw["groups"]),
            "final_families": len(query_payload["families"]),
            "counts": dict(sorted(counts.items())),
            "classification_sha256": digest,
        }
        if comparison_query is not None:
            run["comparison_query"] = {
                "command": " ".join(
                    display_path(Path(arg)) if arg.startswith(str(ROOT)) else arg
                    for arg in comparison_command
                ),
                "returncode": comparison_query.returncode,
                "stdout_sha256": sha256_bytes(comparison_query.stdout),
                "stderr_sha256": sha256_bytes(comparison_query.stderr),
            }
        sampled_rows = [
            item[3] for item in sorted(lost_sample, key=lambda item: (item[1], item[2]))
        ]
        unaccounted = [final_index.records[family_id] for family_id in unaccounted_ids]
        return run, counts, sampled_rows, unaccounted, digest


def collect(args: argparse.Namespace) -> dict[str, Any]:
    baseline = json.loads(args.baseline_provenance.read_text(encoding="utf-8"))
    expected_binary = baseline["provenance"]["nose"]["sha256"]
    actual_binary = sha256_file(args.nose)
    matches_frozen_binary = actual_binary == expected_binary
    if args.binary_role == "baseline" and not matches_frozen_binary:
        raise SystemExit(
            f"--nose must be the #816 frozen binary: {actual_binary} != {expected_binary}"
        )
    comparison_binary = None
    if args.comparison_binary is not None:
        comparison_sha256 = sha256_file(args.comparison_binary)
        if comparison_sha256 != expected_binary:
            raise SystemExit(
                "--comparison-binary must be the #816 frozen binary: "
                f"{comparison_sha256} != {expected_binary}"
            )
        comparison_binary = args.comparison_binary
    if args.binary_role == "head" and comparison_binary is None:
        raise SystemExit("head census requires --comparison-binary")
    detect_binary = args.detect_binary or args.nose
    corpus = json.loads(args.corpus_manifest.read_text(encoding="utf-8"))["repositories"]
    repositories = [row for row in corpus if row["split"] == "dev"]
    repositories.sort(key=lambda row: row["id"])
    if args.limit_repositories is not None:
        repositories = repositories[: args.limit_repositories]

    total: Counter[str] = Counter()
    by_language: dict[str, Counter[str]] = defaultdict(Counter)
    runs: dict[str, dict[str, Any]] = {}
    lost: list[dict[str, Any]] = []
    unaccounted: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    commits: dict[str, dict[str, str | None]] = {}
    repo_digests: list[tuple[str, str]] = []
    for repository_record in repositories:
        repo = repository_record["id"]
        repository = args.repos_root / repo
        observed = None
        if repository.is_dir():
            try:
                observed = git_output("rev-parse", "HEAD", cwd=repository)
            except subprocess.CalledProcessError:
                pass
        commits[repo] = {"expected": repository_record["commit"], "observed": observed}
        if observed != repository_record["commit"]:
            failures.append(
                {
                    "repo": repo,
                    "stage": "repository-commit",
                    "error": f"observed {observed}, expected {repository_record['commit']}",
                }
            )
            continue
        try:
            run, counts, repo_lost, repo_unaccounted, digest = run_repository(
                args.nose,
                repository,
                repo,
                detect_nose=detect_binary,
                comparison_nose=comparison_binary,
            )
        except (OSError, KeyError, TypeError, ValueError, RuntimeError, json.JSONDecodeError) as error:
            failures.append({"repo": repo, "stage": "coverage", "error": str(error)})
            continue
        run["language"] = repository_record["primary_language"]
        runs[repo] = run
        total.update(counts)
        by_language[repository_record["primary_language"]].update(counts)
        lost.extend({"repo": repo, **row} for row in repo_lost)
        unaccounted.extend({"repo": repo, **row} for row in repo_unaccounted)
        repo_digests.append((repo, digest))
        print(
            f"{repo}: accepted={counts['accepted_edges']} eligible={counts['eligible_edges']} "
            f"lost={counts['lost-after-component'] + counts['component-uncovered']}",
            file=os.sys.stderr,
        )

    lost.sort(key=lambda row: (row["repo"], row["edge_id"], row["ordinal"]))
    unaccounted.sort(key=lambda row: (row["repo"], row["id"]))
    return {
        "schema": SCHEMA,
        "split": "dev",
        "configuration": {
            "binary_role": args.binary_role,
            "comparison_binary": (
                display_path(comparison_binary) if comparison_binary is not None else None
            ),
            "detect_binary": display_path(detect_binary),
            "detect_args": list(DETECT_ARGS),
            "query_args": list(QUERY_ARGS),
            "endpoint_cover_fraction": 0.5,
            "allows_reused_covering_member": True,
            "classification_digest_order": "raw detect duplicates report order",
            "lost_edge_sample_per_repository": LOST_EDGE_SAMPLE_PER_REPOSITORY,
            "limit_repositories": args.limit_repositories,
        },
        "provenance": {
            "command": args.command,
            "git_sha": git_output("rev-parse", "HEAD"),
            "working_tree_status_before_measurement": git_output(
                "status", "--porcelain=v1", "--untracked-files=all"
            ),
            "nose": {
                "path": display_path(args.nose),
                "version": subprocess.run(
                    [str(args.nose), "--version"], check=True, text=True, capture_output=True
                ).stdout.strip(),
                "sha256": actual_binary,
                "matches_issue_816_frozen_binary": matches_frozen_binary,
            },
            "comparison_nose": (
                {
                    "path": display_path(comparison_binary),
                    "sha256": expected_binary,
                    "matches_issue_816_frozen_binary": True,
                }
                if comparison_binary is not None
                else None
            ),
            "detect_nose": {
                "path": display_path(detect_binary),
                "sha256": sha256_file(detect_binary),
                "role": "query accepted-edge diagnostic only",
            },
            "inputs": {
                "corpus_manifest": {
                    "path": display_path(args.corpus_manifest),
                    "sha256": sha256_file(args.corpus_manifest),
                },
                "issue_816_baseline": {
                    "path": display_path(args.baseline_provenance),
                    "sha256": sha256_file(args.baseline_provenance),
                },
                "collector": {
                    "path": display_path(Path(__file__)),
                    "sha256": sha256_file(Path(__file__)),
                },
            },
            "repository_commits": commits,
        },
        "summary": dict(sorted(total.items())),
        "by_language": {
            language: dict(sorted(counts.items()))
            for language, counts in sorted(by_language.items())
        },
        "repository_runs": runs,
        "lost_edge_sample": lost,
        "unaccounted_added_families": unaccounted,
        "full_census_sha256": sha256_bytes(canonical_json(repo_digests).encode("utf-8")),
        "failures": failures,
    }


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(payload: object, *, check_inputs: bool = True) -> None:
    require(isinstance(payload, dict), "artifact must be an object")
    require(payload.get("schema") == SCHEMA, "unsupported schema")
    require(payload.get("split") == "dev", "census must remain dev-only")
    configuration = payload.get("configuration")
    require(isinstance(configuration, dict), "configuration missing")
    require(configuration.get("binary_role") in {"baseline", "head"}, "binary role drifted")
    if configuration["binary_role"] == "baseline":
        require(configuration.get("comparison_binary") is None, "baseline must not compare")
    else:
        require(
            isinstance(configuration.get("comparison_binary"), str),
            "head comparison binary missing",
        )
    require(isinstance(configuration.get("detect_binary"), str), "detect binary missing")
    require(configuration.get("detect_args") == list(DETECT_ARGS), "detect args drifted")
    require(configuration.get("query_args") == list(QUERY_ARGS), "query args drifted")
    require(configuration.get("endpoint_cover_fraction") == 0.5, "coverage law drifted")
    require(
        configuration.get("allows_reused_covering_member") is True,
        "covering-member reuse policy drifted",
    )
    require(
        configuration.get("lost_edge_sample_per_repository")
        == LOST_EDGE_SAMPLE_PER_REPOSITORY,
        "lost-edge sample bound drifted",
    )
    require(configuration.get("limit_repositories") is None, "official census is limited")
    require(payload.get("failures") == [], "official census contains failures")
    summary = payload.get("summary")
    require(isinstance(summary, dict), "summary missing")
    require(
        summary.get("eligible_edges", 0)
        == summary.get("final_all_covered", 0)
        + summary.get("intentionally-suppressed-generated", 0)
        + summary.get("lost-after-component", 0)
        + summary.get("component-uncovered", 0),
        "eligible edge partition does not close",
    )
    runs = payload.get("repository_runs")
    require(isinstance(runs, dict) and runs, "repository runs missing")
    commits = payload.get("provenance", {}).get("repository_commits")
    require(isinstance(commits, dict), "repository commits missing")
    require(set(runs) == set(commits), "run and commit repository sets differ")
    sample = payload.get("lost_edge_sample")
    require(isinstance(sample, list), "lost edge sample missing")
    sample_counts = Counter(row.get("repo") for row in sample)
    require(
        all(count <= LOST_EDGE_SAMPLE_PER_REPOSITORY for count in sample_counts.values()),
        "lost edge sample exceeds its per-repository bound",
    )
    unaccounted = payload.get("unaccounted_added_families")
    require(isinstance(unaccounted, list), "unaccounted added-family list missing")
    if configuration["binary_role"] == "head":
        require(
            summary.get("added_families_without_recovered_edge") == len(unaccounted),
            "unaccounted added-family count drifted",
        )
    for repo, record in commits.items():
        require(record.get("observed") == record.get("expected"), f"{repo}: pin mismatch")
    repo_digests = sorted(
        (repo, run["classification_sha256"]) for repo, run in runs.items()
    )
    require(
        payload.get("full_census_sha256")
        == sha256_bytes(canonical_json(repo_digests).encode("utf-8")),
        "full census digest mismatch",
    )
    if check_inputs:
        inputs = payload["provenance"]["inputs"]
        for name, record in inputs.items():
            path = ROOT / record["path"]
            require(path.is_file(), f"{name}: missing {path}")
            require(sha256_file(path) == record["sha256"], f"{name}: hash mismatch")
        baseline = json.loads((ROOT / inputs["issue_816_baseline"]["path"]).read_text())
        if configuration["binary_role"] == "baseline":
            require(
                payload["provenance"]["nose"]["sha256"]
                == baseline["provenance"]["nose"]["sha256"],
                "binary is not the frozen #816 binary",
            )
        else:
            require(
                payload["provenance"]["comparison_nose"]["sha256"]
                == baseline["provenance"]["nose"]["sha256"],
                "head comparison binary is not the frozen #816 binary",
            )


def self_test() -> None:
    repository = ROOT / "bench" / "repos" / "example"
    families = [
        {
            "id": "outer",
            "surface": "default",
            "locations": [
                {"file": str(repository / "a.py"), "start": 1, "end": 30},
                {"file": str(repository / "b.py"), "start": 1, "end": 30},
            ],
        },
        {
            "id": "wide",
            "surface": "hidden",
            "locations": [
                {"file": str(repository / "a.py"), "start": 16, "end": 80},
                {"file": str(repository / "b.py"), "start": 16, "end": 80},
            ],
        },
    ]
    index = CoverageIndex(families, repository, query=True)
    leading = (
        {"file": "a.py", "start_line": 1, "end_line": 30},
        {"file": "b.py", "start_line": 1, "end_line": 30},
    )
    trailing = (
        {"file": "a.py", "start_line": 51, "end_line": 80},
        {"file": "b.py", "start_line": 51, "end_line": 80},
    )
    assert "outer" in index.covering(*leading)
    assert index.covering(*trailing) == ["wide"]
    assert distinct_non_nested(*trailing)
    assert not distinct_non_nested(trailing[0], dict(trailing[0]))


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    result.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS)
    result.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS)
    result.add_argument("--baseline-provenance", type=Path, default=DEFAULT_BASELINE)
    result.add_argument("--binary-role", choices=("baseline", "head"), default="baseline")
    result.add_argument("--comparison-binary", type=Path)
    result.add_argument("--detect-binary", type=Path)
    result.add_argument("--json-out", type=Path)
    result.add_argument("--limit-repositories", type=int)
    result.add_argument("--validate", type=Path)
    result.add_argument("--self-test", action="store_true")
    return result


def main() -> None:
    args = parser().parse_args()
    if args.self_test:
        self_test()
        return
    if args.validate is not None:
        validate(json.loads(args.validate.read_text(encoding="utf-8")))
        return
    if args.json_out is None:
        raise SystemExit("--json-out is required when collecting")
    args.command = " ".join(os.sys.argv)
    payload = collect(args)
    args.json_out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.limit_repositories is None:
        validate(payload)
    if payload["failures"]:
        raise SystemExit(f"coverage census completed with {len(payload['failures'])} failures")


if __name__ == "__main__":
    main()
