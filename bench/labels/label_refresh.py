#!/usr/bin/env python3
"""Collect and validate the split-safe #812 product-label refresh evidence."""

from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from labelset import (
    COMPONENT_SCHEMA,
    PRECISION_METRIC,
    VOTE_NAMES,
    WORTHY_REASONS,
    LoadedLabelset,
    load_labelset,
    metric_eligible,
    sha256_file,
    validate_vote,
)
from query_schema import QUERY_SCHEMA_VERSION, member_locations, query_families


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"
DEFAULT_CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
DEFAULT_BASE_LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v5.json"
DEFAULT_RUBRIC = ROOT / "bench" / "labels" / "RUBRIC.md"
ARTIFACT_SCHEMA = "nose.refactoring_label_refresh_candidates.v1"
DECISIONS_SCHEMA = "nose.refactoring_label_decisions.v1"
SELECTION_SEED = "nose-issue-812-existing-unmatched-v1"


def canonical_sha256(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def rel(path: str | Path) -> str:
    value = str(path).replace(str(ROOT) + "/", "")
    index = value.find("bench/repos/")
    return value[index:] if index >= 0 else value


def git_output(args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def nose_version(nose: Path) -> str:
    result = subprocess.run(
        [str(nose), "--version"], cwd=ROOT, check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise SystemExit(f"{nose} --version failed: {result.stderr.strip()}")
    return result.stdout.strip()


def repository_head(repo: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"cannot read corpus revision for {repo}: {result.stderr.strip()}")
    return result.stdout.strip()


def overlaps(a: dict[str, Any], b: dict[str, Any]) -> bool:
    return a["file"] == b["file"] and not (
        a["end_line"] < b["start_line"] or b["end_line"] < a["start_line"]
    )


def normalized_members(family: dict[str, Any]) -> list[dict[str, Any]]:
    members = []
    for location in member_locations(family, source="label refresh query family"):
        member = {
            "file": rel(location["file"]),
            "start_line": location["start_line"],
            "end_line": location["end_line"],
            "name": location.get("name"),
        }
        members.append(member)
    return members


def best_label_match(
    members: list[dict[str, Any]], labels: list[dict[str, Any]]
) -> tuple[str | None, int]:
    best_id = None
    best_overlap = 0
    for label in labels:
        overlap = sum(
            1
            for member in members
            for labeled_member in label["members"]
            if overlaps(member, labeled_member)
        )
        if overlap > best_overlap:
            best_id = label["family_id"]
            best_overlap = overlap
    return best_id, best_overlap


def candidate_content(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        key: candidate[key]
        for key in (
            "candidate_key",
            "repo",
            "split",
            "language",
            "lane",
            "rank",
            "family",
            "raw_family_sha256",
        )
    }


def selection_hash(candidate: dict[str, Any]) -> str:
    identity = f"{SELECTION_SEED}\0{candidate['candidate_key']}"
    return hashlib.sha256(identity.encode()).hexdigest()


def select_existing(
    candidates: list[dict[str, Any]], per_stratum: int
) -> dict[str, list[str]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for candidate in candidates:
        if candidate["lane"] == "existing-v5-unmatched":
            grouped[(candidate["language"], candidate["split"])].append(candidate)
    selected: dict[str, list[str]] = {}
    for (language, split), rows in sorted(grouped.items()):
        ordered = sorted(rows, key=lambda row: (selection_hash(row), row["candidate_key"]))
        chosen: list[dict[str, Any]] = []
        seen_repos: set[str] = set()
        for row in ordered:
            if row["repo"] not in seen_repos:
                chosen.append(row)
                seen_repos.add(row["repo"])
            if len(chosen) == per_stratum:
                break
        if len(chosen) < per_stratum:
            chosen_keys = {row["candidate_key"] for row in chosen}
            chosen.extend(
                row
                for row in ordered
                if row["candidate_key"] not in chosen_keys
            )
            chosen = chosen[:per_stratum]
        if len(chosen) != per_stratum:
            raise SystemExit(
                f"selection stratum {language}/{split} has only {len(chosen)} candidates"
            )
        selected[f"{language}/{split}"] = [row["candidate_key"] for row in chosen]
    return selected


def selected_keys(
    candidates: list[dict[str, Any]], *, existing_per_stratum: int, swift_per_repo: int
) -> tuple[set[str], dict[str, list[str]], dict[str, list[str]]]:
    existing = select_existing(candidates, existing_per_stratum)
    swift: dict[str, list[str]] = {}
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for candidate in candidates:
        if candidate["lane"] == "swift-real-top10":
            by_repo[candidate["repo"]].append(candidate)
    for repo, rows in sorted(by_repo.items()):
        chosen = sorted(rows, key=lambda row: (row["rank"], row["candidate_key"]))[
            :swift_per_repo
        ]
        if len(chosen) != swift_per_repo:
            raise SystemExit(f"Swift selection repo {repo} has only {len(chosen)} candidates")
        swift[repo] = [row["candidate_key"] for row in chosen]
    keys = {key for rows in existing.values() for key in rows}
    keys.update(key for rows in swift.values() for key in rows)
    return keys, existing, swift


def query_repo(nose: Path, repo: Path) -> tuple[bytes, list[dict[str, Any]], list[str]]:
    command = [str(nose), "query", rel(repo), "all", "top=10", "--format", "json"]
    result = subprocess.run(command, cwd=ROOT, check=False, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"query failed for {repo}: exit {result.returncode}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return result.stdout, query_families(result.stdout, source=f"nose query {repo}"), command


def source_file_record(path_text: str) -> dict[str, Any]:
    path = (ROOT / path_text).resolve()
    try:
        path.relative_to((ROOT / "bench" / "repos").resolve())
    except ValueError as error:
        raise SystemExit(f"query member is outside bench/repos: {path_text}") from error
    if not path.is_file():
        raise SystemExit(f"query member source is missing: {path_text}")
    return {"bytes": path.stat().st_size, "sha256": sha256_file(path)}


def collect(args: argparse.Namespace) -> dict[str, Any]:
    working_tree_status = git_output(["status", "--short"])
    corpus_payload = json.loads(args.corpus.read_text(encoding="utf-8"))
    corpus_rows = corpus_payload["repositories"]
    corpus = {row["id"]: row for row in corpus_rows}
    base = load_labelset(args.base_labelset)
    base_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in base.families:
        base_by_repo[family["repo"]].append(family)
    swift_repos = {
        row["id"] for row in corpus_rows if row["primary_language"].lower() == "swift"
    }
    repo_ids = sorted(set(base_by_repo) | swift_repos)
    candidates: list[dict[str, Any]] = []
    repositories: dict[str, dict[str, Any]] = {}
    source_files: dict[str, dict[str, Any]] = {}

    for repo_id in repo_ids:
        metadata = corpus[repo_id]
        repo = args.repos_root / repo_id
        if not repo.is_dir():
            raise SystemExit(f"missing pinned repository: {repo_id}")
        actual_commit = repository_head(repo)
        if actual_commit != metadata["commit"]:
            raise SystemExit(
                f"{repo_id}: corpus revision {actual_commit} != {metadata['commit']}"
            )
        stdout, families, command = query_repo(args.nose, repo)
        matched = 0
        candidate_count = 0
        for rank, family in enumerate(families, start=1):
            members = normalized_members(family)
            match_id, match_overlap = best_label_match(members, base_by_repo.get(repo_id, []))
            if match_id is not None:
                matched += 1
            lane = "swift-real-top10" if repo_id in swift_repos else "existing-v5-unmatched"
            if lane == "existing-v5-unmatched" and match_id is not None:
                continue
            candidate_count += 1
            compact_family = {
                "id": family["id"],
                "members": members,
                "member_count": len(members),
                "scope": family.get("scope"),
                "surface": family.get("surface"),
                "witness": family.get("witness"),
                "extraction_shape": family.get("extraction_shape"),
                "value": family.get("value"),
                "matched_v5_family_id": match_id,
                "matched_v5_member_overlap": match_overlap,
            }
            candidate = {
                "candidate_key": f"{repo_id}:{family['id']}",
                "repo": repo_id,
                "split": metadata["split"],
                "language": metadata["primary_language"],
                "lane": lane,
                "rank": rank,
                "family": compact_family,
                "raw_family_sha256": canonical_sha256(family),
            }
            candidate["candidate_sha256"] = canonical_sha256(candidate_content(candidate))
            candidates.append(candidate)
            for member in members:
                source_files.setdefault(member["file"], source_file_record(member["file"]))
        repositories[repo_id] = {
            "commit": actual_commit,
            "language": metadata["primary_language"],
            "split": metadata["split"],
            "query_command": shlex.join(command),
            "query_stdout_sha256": hashlib.sha256(stdout).hexdigest(),
            "top_10_reported": len(families),
            "v5_matched_top_10": matched,
            "candidate_count": candidate_count,
        }

    keys, existing_selection, swift_selection = selected_keys(
        candidates,
        existing_per_stratum=args.existing_per_stratum,
        swift_per_repo=args.swift_per_repo,
    )
    selection_order = {
        key: index
        for index, key in enumerate(
            [
                *[
                    key
                    for stratum in sorted(existing_selection)
                    for key in existing_selection[stratum]
                ],
                *[key for repo in sorted(swift_selection) for key in swift_selection[repo]],
            ],
            start=1,
        )
    }
    for candidate in candidates:
        candidate["selected"] = candidate["candidate_key"] in keys
        candidate["selection_order"] = selection_order.get(candidate["candidate_key"])

    selected_list = [key for key, _ in sorted(selection_order.items(), key=lambda row: row[1])]
    nose = args.nose.resolve()
    return {
        "schema": ARTIFACT_SCHEMA,
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": {
            "command": shlex.join(["python3", *sys.argv]),
            "git_sha": git_output(["rev-parse", "HEAD"]),
            "working_tree_status_before_collection": working_tree_status,
            "nose_binary": rel(nose),
            "nose_binary_sha256": sha256_file(nose),
            "nose_version": nose_version(nose),
            "corpus_manifest": rel(args.corpus.resolve()),
            "corpus_manifest_sha256": sha256_file(args.corpus),
            "base_labelset": rel(args.base_labelset.resolve()),
            "base_labelset_sha256": sha256_file(args.base_labelset),
            "rubric": rel(args.rubric.resolve()),
            "rubric_sha256": sha256_file(args.rubric),
        },
        "selection": {
            "existing": {
                "seed": SELECTION_SEED,
                "per_language_split": args.existing_per_stratum,
                "diversity": "prefer distinct repositories, then fill by seeded hash order",
                "selected": existing_selection,
            },
            "swift": {
                "per_repository": args.swift_per_repo,
                "rule": "lowest current product rank in every pinned real Swift repository",
                "selected": swift_selection,
            },
            "selected_candidate_keys": selected_list,
            "selected_candidate_keys_sha256": canonical_sha256(selected_list),
        },
        "pool": {
            "existing_v5_top_10_total": sum(
                row["top_10_reported"]
                for repo, row in repositories.items()
                if repo not in swift_repos
            ),
            "existing_v5_matched": sum(
                row["v5_matched_top_10"]
                for repo, row in repositories.items()
                if repo not in swift_repos
            ),
            "existing_v5_unmatched": sum(
                row["candidate_count"]
                for repo, row in repositories.items()
                if repo not in swift_repos
            ),
            "swift_top_10_total": sum(
                row["top_10_reported"] for repo, row in repositories.items() if repo in swift_repos
            ),
            "candidate_count": len(candidates),
            "selected_count": len(keys),
        },
        "repositories": repositories,
        "source_files": dict(sorted(source_files.items())),
        "candidates": sorted(candidates, key=lambda row: (row["repo"], row["rank"])),
    }


def load_artifact(path: Path) -> dict[str, Any]:
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"cannot read candidate artifact {path}: {error}") from error
    if not isinstance(artifact, dict) or artifact.get("schema") != ARTIFACT_SCHEMA:
        raise SystemExit(f"{path}: unsupported candidate artifact schema")
    return artifact


def validate_candidate_artifact(
    artifact: dict[str, Any], *, live_root: Path | None = None
) -> None:
    candidates = artifact.get("candidates")
    if not isinstance(candidates, list):
        raise SystemExit("candidate artifact needs a candidates array")
    seen: set[str] = set()
    for index, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            raise SystemExit(f"candidates[{index}] must be an object")
        key = candidate.get("candidate_key")
        if not isinstance(key, str) or not key or key in seen:
            raise SystemExit(f"candidates[{index}] has an invalid/duplicate key")
        seen.add(key)
        if candidate.get("candidate_sha256") != canonical_sha256(candidate_content(candidate)):
            raise SystemExit(f"{key}: candidate content digest mismatch")
    selection = artifact.get("selection", {})
    existing = selection.get("existing", {})
    swift = selection.get("swift", {})
    keys, expected_existing, expected_swift = selected_keys(
        candidates,
        existing_per_stratum=existing.get("per_language_split"),
        swift_per_repo=swift.get("per_repository"),
    )
    if existing.get("selected") != expected_existing or swift.get("selected") != expected_swift:
        raise SystemExit("candidate selection does not match the declared deterministic rule")
    recorded_keys = selection.get("selected_candidate_keys")
    if not isinstance(recorded_keys, list) or set(recorded_keys) != keys:
        raise SystemExit("selected candidate key list does not match selection")
    if selection.get("selected_candidate_keys_sha256") != canonical_sha256(recorded_keys):
        raise SystemExit("selected candidate key digest mismatch")
    for candidate in candidates:
        if candidate.get("selected") != (candidate["candidate_key"] in keys):
            raise SystemExit(f"{candidate['candidate_key']}: selected flag mismatch")

    if live_root is not None:
        provenance = artifact.get("provenance", {})
        checked_files = (
            ("corpus_manifest", "corpus_manifest_sha256"),
            ("base_labelset", "base_labelset_sha256"),
            ("rubric", "rubric_sha256"),
            ("nose_binary", "nose_binary_sha256"),
        )
        for path_key, hash_key in checked_files:
            path = live_root / provenance.get(path_key, "")
            if not path.is_file() or sha256_file(path) != provenance.get(hash_key):
                raise SystemExit(f"candidate provenance mismatch for {path_key}")
        for repo, row in artifact["repositories"].items():
            if repository_head(live_root / "bench" / "repos" / repo) != row["commit"]:
                raise SystemExit(f"{repo}: live corpus revision mismatch")
        for path_text, record in artifact["source_files"].items():
            path = live_root / path_text
            if (
                not path.is_file()
                or path.stat().st_size != record.get("bytes")
                or sha256_file(path) != record.get("sha256")
            ):
                raise SystemExit(f"source provenance mismatch: {path_text}")


def coverage(labels: LoadedLabelset, artifact: dict[str, Any]) -> dict[str, Any]:
    new_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    refresh_families = []
    for family in labels.families:
        if "candidate_key" in family:
            refresh_families.append(family)
            new_by_repo[family["repo"]].append(family)
    selected = {
        candidate["candidate_key"]: candidate
        for candidate in artifact["candidates"]
        if candidate["selected"]
    }
    labeled_keys = {family["candidate_key"] for family in refresh_families}
    if labeled_keys != set(selected):
        missing = sorted(set(selected) - labeled_keys)
        extra = sorted(labeled_keys - set(selected))
        raise SystemExit(f"refresh labels do not match selection; missing={missing}, extra={extra}")
    for family in refresh_families:
        candidate = selected[family["candidate_key"]]
        expected = candidate["family"]
        checks = {
            "candidate_sha256": candidate["candidate_sha256"],
            "repo": candidate["repo"],
            "split": candidate["split"],
            "language": candidate["language"],
            "scope": expected["scope"],
            "family_id": expected["id"],
            "members": expected["members"],
        }
        for key, value in checks.items():
            if family.get(key) != value:
                raise SystemExit(f"{family['candidate_key']}: label field {key} differs from source")
        if metric_eligible(family, "worthy_recall"):
            raise SystemExit(f"{family['candidate_key']}: top-10-selected label cannot enter recall")

    swift_repos = {
        repo
        for repo, row in artifact["repositories"].items()
        if row["language"].lower() == "swift"
    }
    swift_labels = [family for family in refresh_families if family["repo"] in swift_repos]
    if {family["repo"] for family in swift_labels} != swift_repos:
        raise SystemExit("Swift labels must cover every pinned real Swift repository")
    for split in ("dev", "heldout"):
        worthiness = {family["worthy"] for family in swift_labels if family["split"] == split}
        if worthiness != {False, True}:
            raise SystemExit(f"Swift {split} labels must include both worthiness classes")

    matched_candidates: set[str] = set()
    for candidate in artifact["candidates"]:
        match, _ = best_label_match(
            candidate["family"]["members"], new_by_repo.get(candidate["repo"], [])
        )
        if match is not None:
            matched_candidates.add(candidate["candidate_key"])
    repositories = artifact["repositories"]
    existing_repos = [repo for repo, row in repositories.items() if row["language"].lower() != "swift"]
    swift_repo_ids = [repo for repo, row in repositories.items() if row["language"].lower() == "swift"]

    def lane_report(repo_ids: list[str], lane: str) -> dict[str, Any]:
        total = sum(repositories[repo]["top_10_reported"] for repo in repo_ids)
        baseline = sum(repositories[repo]["v5_matched_top_10"] for repo in repo_ids)
        added = sum(
            1
            for candidate in artifact["candidates"]
            if candidate["repo"] in repo_ids and candidate["candidate_key"] in matched_candidates
        )
        current = baseline + added
        return {
            "lane": lane,
            "top_10": total,
            "v5_matched": baseline,
            "v6_matched": current,
            "added": added,
            "v5_pct": round(100 * baseline / total, 4) if total else 0.0,
            "v6_pct": round(100 * current / total, 4) if total else 0.0,
        }

    existing = lane_report(existing_repos, "existing-v5-repositories")
    swift = lane_report(swift_repo_ids, "real-swift-repositories")
    overall = lane_report(existing_repos + swift_repo_ids, "expanded-120-repository-corpus")
    if existing["added"] < 50 or existing["v6_pct"] - existing["v5_pct"] < 5.0:
        raise SystemExit("existing-repository label-match coverage did not materially improve")
    return {
        "existing": existing,
        "swift": swift,
        "overall": overall,
        "refresh_labels": len(refresh_families),
        "refresh_worthy": sum(1 for family in refresh_families if family["worthy"]),
        "refresh_not_worthy": sum(1 for family in refresh_families if not family["worthy"]),
    }


def representative_members(
    members: list[dict[str, Any]], limit: int = 3
) -> list[dict[str, Any]]:
    by_file: dict[str, dict[str, Any]] = {}
    for member in members:
        by_file.setdefault(member["file"], member)
    distinct = list(by_file.values())
    if len(distinct) <= limit:
        return distinct
    indexes = sorted({0, len(distinct) // 2, len(distinct) - 1})
    return [distinct[index] for index in indexes[:limit]]


def source_excerpt(member: dict[str, Any], context_lines: int = 2) -> str:
    path = ROOT / member["file"]
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    start = max(1, member["start_line"] - context_lines)
    end = min(len(lines), member["end_line"] + context_lines)
    if end - start + 1 > 40:
        end = start + 39
    return "\n".join(
        f"{line_number:>6}: {lines[line_number - 1]}"
        for line_number in range(start, end + 1)
    )


def render_context(artifact: dict[str, Any], split: str) -> str:
    selected = sorted(
        (
            candidate
            for candidate in artifact["candidates"]
            if candidate["selected"] and candidate["split"] == split
        ),
        key=lambda row: row["selection_order"],
    )
    lines = [
        f"# #812 {split} label context",
        "",
        "Generated from the frozen candidate artifact; excerpts are local review aids only.",
        "",
    ]
    for candidate in selected:
        family = candidate["family"]
        lines += [
            f"## {candidate['candidate_key']}",
            "",
            (
                f"- lane: `{candidate['lane']}`; language: `{candidate['language']}`; "
                f"rank: {candidate['rank']}; scope: `{family['scope']}`"
            ),
            (
                f"- shape: `{family['extraction_shape']}`; witness: `{family['witness']}`; "
                f"members: {family['member_count']}; value: {family['value']}"
            ),
            "- members:",
        ]
        members = family["members"]
        for member in members[:12]:
            lines.append(
                f"  - `{member['file']}:{member['start_line']}` "
                f"`{member.get('name') or '<anonymous>'}`"
            )
        if len(members) > 12:
            lines.append(f"  - … {len(members) - 12} more")
        for member in representative_members(members):
            lines += [
                "",
                f"### `{member['file']}:{member['start_line']}`",
                "",
                "```text",
                source_excerpt(member),
                "```",
            ]
        lines.append("")
    return "\n".join(lines)


def relative_file_record(path: Path, parent: Path) -> dict[str, str]:
    return {
        "path": path.resolve().relative_to(parent.resolve()).as_posix(),
        "sha256": sha256_file(path),
    }


def build_component(
    artifact_path: Path,
    artifact: dict[str, Any],
    decisions_path: Path,
    split: str,
    output: Path,
) -> dict[str, Any]:
    decisions_payload = json.loads(decisions_path.read_text(encoding="utf-8"))
    if (
        not isinstance(decisions_payload, dict)
        or decisions_payload.get("schema") != DECISIONS_SCHEMA
        or decisions_payload.get("split") != split
    ):
        raise SystemExit("decision input schema/split mismatch")
    decisions = decisions_payload.get("decisions")
    if not isinstance(decisions, list):
        raise SystemExit("decision input needs a decisions array")
    by_key: dict[str, dict[str, Any]] = {}
    for index, decision in enumerate(decisions):
        if not isinstance(decision, dict):
            raise SystemExit(f"decisions[{index}] must be an object")
        key = decision.get("candidate_key")
        if not isinstance(key, str) or not key or key in by_key:
            raise SystemExit(f"decisions[{index}] has an invalid/duplicate candidate_key")
        by_key[key] = decision
    selected = sorted(
        (
            candidate
            for candidate in artifact["candidates"]
            if candidate["selected"] and candidate["split"] == split
        ),
        key=lambda row: row["selection_order"],
    )
    expected_keys = {candidate["candidate_key"] for candidate in selected}
    if set(by_key) != expected_keys:
        raise SystemExit(
            "decision keys do not match frozen selection; "
            f"missing={sorted(expected_keys - set(by_key))}, "
            f"extra={sorted(set(by_key) - expected_keys)}"
        )

    families = []

    def expand_vote(value: object) -> object:
        if isinstance(value, str):
            return {"worthy": value in WORTHY_REASONS, "reason": value}
        return value

    for candidate in selected:
        decision = by_key[candidate["candidate_key"]]
        votes = decision.get("votes")
        if not isinstance(votes, dict) or set(votes) != set(VOTE_NAMES):
            raise SystemExit(f"{candidate['candidate_key']}: expected exactly three panel votes")
        normalized_votes = {name: expand_vote(votes[name]) for name in VOTE_NAMES}
        panel = [
            validate_vote(
                normalized_votes[name], f"{candidate['candidate_key']}.votes.{name}"
            )
            for name in VOTE_NAMES
        ]
        unanimous = len(set(panel)) == 1
        arbiter = expand_vote(decision.get("arbiter"))
        if unanimous:
            if arbiter is not None:
                raise SystemExit(f"{candidate['candidate_key']}: unanimous vote cannot use arbiter")
            worthy, reason = panel[0]
            labeler = "panel"
        else:
            worthy, reason = validate_vote(
                arbiter, f"{candidate['candidate_key']}.arbiter"
            )
            labeler = "llm-arbiter"
        confidence = decision.get("confidence")
        note = decision.get("note")
        if confidence not in {"high", "medium", "low"}:
            raise SystemExit(f"{candidate['candidate_key']}: invalid confidence")
        if not isinstance(note, str) or not note.strip():
            raise SystemExit(f"{candidate['candidate_key']}: a non-empty note is required")
        family = candidate["family"]
        families.append(
            {
                "family_id": family["id"],
                "candidate_key": candidate["candidate_key"],
                "candidate_sha256": candidate["candidate_sha256"],
                "repo": candidate["repo"],
                "split": candidate["split"],
                "language": candidate["language"],
                "channel": "current-default",
                "scope": family["scope"],
                "members": family["members"],
                "metric_eligibility": [PRECISION_METRIC],
                "worthy": worthy,
                "reason": reason,
                "confidence": confidence,
                "labeler": labeler,
                "votes": normalized_votes,
                "arbiter": arbiter,
                "note": note,
                "selection": {
                    "lane": candidate["lane"],
                    "product_rank": candidate["rank"],
                    "selection_order": candidate["selection_order"],
                },
            }
        )
    rubric = ROOT / artifact["provenance"]["rubric"]
    return {
        "schema": COMPONENT_SCHEMA,
        "split": split,
        "source_artifact": relative_file_record(artifact_path, output.parent),
        "rubric": relative_file_record(rubric, output.parent),
        "decision_input": relative_file_record(decisions_path, output.parent),
        "protocol": {
            "panel": list(VOTE_NAMES),
            "split_votes_escalate_to": "llm-arbiter",
            "metric_eligibility": [PRECISION_METRIC],
            "policy_or_ranking_changes": "none",
        },
        "families": families,
    }


def run_self_test() -> None:
    candidates = []
    for split in ("dev", "heldout"):
        for index in range(7):
            candidate = {
                "candidate_key": f"repo-{split}-{index}:family-{index}",
                "repo": f"repo-{split}-{index}",
                "split": split,
                "language": "Python",
                "lane": "existing-v5-unmatched",
                "rank": index + 1,
                "family": {},
                "raw_family_sha256": "0" * 64,
            }
            candidates.append(candidate)
    for index in range(5):
        candidates.append(
            {
                "candidate_key": f"swift:family-{index}",
                "repo": "swift",
                "split": "dev",
                "language": "Swift",
                "lane": "swift-real-top10",
                "rank": index + 1,
                "family": {},
                "raw_family_sha256": "1" * 64,
            }
        )
    keys, existing, swift = selected_keys(
        candidates, existing_per_stratum=5, swift_per_repo=3
    )
    assert len(keys) == 13
    assert all(len(rows) == 5 for rows in existing.values())
    assert swift == {"swift": ["swift:family-0", "swift:family-1", "swift:family-2"]}
    assert selected_keys(candidates, existing_per_stratum=5, swift_per_repo=3)[0] == keys
    with tempfile.TemporaryDirectory(prefix="nose-label-refresh-self-test-") as directory:
        path = Path(directory) / "artifact.json"
        path.write_text(json.dumps({"schema": ARTIFACT_SCHEMA, "candidates": []}))
        assert load_artifact(path)["schema"] == ARTIFACT_SCHEMA
    print("label refresh self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    subparsers = parser.add_subparsers(dest="command")
    collect_parser = subparsers.add_parser("collect")
    collect_parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    collect_parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    collect_parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    collect_parser.add_argument("--base-labelset", type=Path, default=DEFAULT_BASE_LABELSET)
    collect_parser.add_argument("--rubric", type=Path, default=DEFAULT_RUBRIC)
    collect_parser.add_argument("--existing-per-stratum", type=int, default=5)
    collect_parser.add_argument("--swift-per-repo", type=int, default=3)
    collect_parser.add_argument("--output", type=Path, required=True)
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--candidates", type=Path, required=True)
    validate_parser.add_argument("--labelset", type=Path)
    validate_parser.add_argument("--live", action="store_true")
    context_parser = subparsers.add_parser("context")
    context_parser.add_argument("--candidates", type=Path, required=True)
    context_parser.add_argument("--split", choices=("dev", "heldout"), required=True)
    context_parser.add_argument("--output", type=Path, required=True)
    component_parser = subparsers.add_parser("build-component")
    component_parser.add_argument("--candidates", type=Path, required=True)
    component_parser.add_argument("--decisions", type=Path, required=True)
    component_parser.add_argument("--split", choices=("dev", "heldout"), required=True)
    component_parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return
    if args.command == "collect":
        if args.existing_per_stratum <= 0 or args.swift_per_repo <= 0:
            raise SystemExit("selection counts must be positive")
        artifact = collect(args)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")
        print(
            f"wrote {args.output}: {artifact['pool']['candidate_count']} candidates, "
            f"{artifact['pool']['selected_count']} selected"
        )
        return
    if args.command == "validate":
        artifact = load_artifact(args.candidates)
        validate_candidate_artifact(artifact, live_root=ROOT if args.live else None)
        if args.labelset:
            report = coverage(load_labelset(args.labelset), artifact)
            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print("candidate artifact validation passed")
        return
    if args.command == "context":
        artifact = load_artifact(args.candidates)
        validate_candidate_artifact(artifact)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(render_context(artifact, args.split), encoding="utf-8")
        print(f"wrote {args.output}")
        return
    if args.command == "build-component":
        artifact = load_artifact(args.candidates)
        validate_candidate_artifact(artifact)
        component = build_component(
            args.candidates, artifact, args.decisions, args.split, args.output
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(component, indent=2, sort_keys=True) + "\n")
        print(f"wrote {args.output}: {len(component['families'])} labels")
        return
    raise SystemExit("choose collect or validate, or pass --self-test")


if __name__ == "__main__":
    main()
