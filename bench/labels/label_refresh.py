#!/usr/bin/env python3
"""Collect and validate the split-safe #812/#840 product-label refresh evidence."""

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
    HELDOUT_SEAL_SCHEMA,
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
PANEL_VOTE_SCHEMA = "nose.refactoring_panel_vote.v1"
PANEL_MERGE_SCHEMA = "nose.refactoring_panel_merge.v1"
PANEL_ARBITRATION_SCHEMA = "nose.refactoring_panel_arbitration.v1"
SELECTION_SEED = "nose-issue-812-existing-unmatched-v1"
RUNWAY_SCHEMA = "nose.default_head_label_runway.v1"
RUNWAY_SELECTION_SEED = "nose-issue-840-default-head-v7-rank-11-30"
RUNWAY_TOP = 30
FROZEN_V5_SHA256 = "e18b65543f4b6373d7eadbc93159adda69699eafe8f5f814d9ba53e245a6d9f9"
FROZEN_V6_SHA256 = "6b72927d0e68e05406540016d3fa136029c52a406af0938b5a805d3fa199ac23"
FROZEN_V5_FAMILIES_SHA256 = (
    "a4364f09b21a9d08ed5d422b21ddcd14a93b7f37c7e814ddf1e52bc12002623a"
)
FROZEN_V6_FAMILIES_SHA256 = (
    "d03de27f0d9bba54e4ee6c28292e4d0b5ac1291ade100809af89d4ea30af147f"
)
RUNWAY_EVALUATION_SHA256 = (
    "bdc99a74bb2b58fe06ddf2eab1c01ea108e0aeaaa34e55fecb5ee4998f8f3443"
)


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


def git_file_sha256(revision: str, path: str) -> str:
    result = subprocess.run(
        ["git", "show", f"{revision}:{path}"],
        cwd=ROOT,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"cannot read collection source {path} at {revision}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return hashlib.sha256(result.stdout).hexdigest()


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


def ordered_selection(
    existing: dict[str, list[str]], swift: dict[str, list[str]]
) -> list[str]:
    return [
        *[key for stratum in sorted(existing) for key in existing[stratum]],
        *[key for repo in sorted(swift) for key in swift[repo]],
    ]


def query_repo(nose: Path, repo: Path) -> tuple[bytes, list[dict[str, Any]], list[str]]:
    command = [str(nose), "query", rel(repo), "all", "top=10", "--format", "json"]
    result = subprocess.run(command, cwd=ROOT, check=False, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"query failed for {repo}: exit {result.returncode}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return result.stdout, query_families(result.stdout, source=f"nose query {repo}"), command


def query_default_runway_repo(
    nose: Path, repo: Path
) -> tuple[bytes, list[dict[str, Any]], list[str]]:
    command = [
        str(nose),
        "query",
        rel(repo),
        f"top={RUNWAY_TOP}",
        "--format",
        "json",
    ]
    result = subprocess.run(command, cwd=ROOT, check=False, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"default-head query failed for {repo}: exit {result.returncode}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return (
        result.stdout,
        query_families(result.stdout, source=f"nose query {repo} default runway"),
        command,
    )


def runway_candidate_content(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        key: candidate[key]
        for key in (
            "candidate_key",
            "repo",
            "split",
            "language",
            "lane",
            "rank",
            "base_matched",
            "family",
            "raw_family_sha256",
        )
    }


def runway_selection_hash(candidate: dict[str, Any]) -> str:
    identity = f"{RUNWAY_SELECTION_SEED}\0{candidate['candidate_key']}"
    return hashlib.sha256(identity.encode()).hexdigest()


def selected_runway_candidates(
    candidates: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, str]]:
    head = [
        candidate
        for candidate in candidates
        if candidate["rank"] <= 10 and not candidate["base_matched"]
    ]
    deep_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for candidate in candidates:
        if 11 <= candidate["rank"] <= RUNWAY_TOP and not candidate["base_matched"]:
            deep_by_repo[candidate["repo"]].append(candidate)
    deep = [
        min(rows, key=lambda row: (runway_selection_hash(row), row["candidate_key"]))
        for _, rows in sorted(deep_by_repo.items())
    ]
    ordered = sorted(
        head,
        key=lambda row: (row["language"], row["repo"], row["rank"], row["candidate_key"]),
    ) + sorted(
        deep,
        key=lambda row: (row["language"], row["repo"], row["rank"], row["candidate_key"]),
    )
    reasons = {candidate["candidate_key"]: "unmatched-default-head" for candidate in head}
    reasons.update(
        {candidate["candidate_key"]: "deterministic-rank-11-30" for candidate in deep}
    )
    return ordered, reasons


def runway_selection_summary(
    candidates: list[dict[str, Any]],
) -> tuple[list[str], dict[str, str]]:
    selected, reasons = selected_runway_candidates(candidates)
    keys = [candidate["candidate_key"] for candidate in selected]
    if len(keys) != len(set(keys)):
        raise SystemExit("runway selection contains duplicate candidate keys")
    return keys, reasons


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
            ordered_selection(existing_selection, swift_selection), start=1
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


def apply_runway_selection(candidates: list[dict[str, Any]]) -> list[str]:
    keys, reasons = runway_selection_summary(candidates)
    positions = {key: index for index, key in enumerate(keys, start=1)}
    for candidate in candidates:
        key = candidate["candidate_key"]
        candidate["selected"] = key in positions
        candidate["selection_reason"] = reasons.get(key)
        candidate["selection_order"] = positions.get(key)
    return keys


def runway_pool_summary(
    candidates: list[dict[str, Any]], repositories: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    return {
        "repositories": len(repositories),
        "default_head_positions": sum(row["top_10_reported"] for row in repositories.values()),
        "base_matched_default_head": sum(
            row["base_matched_top_10"] for row in repositories.values()
        ),
        "unmatched_default_head": sum(
            row["unmatched_top_10"] for row in repositories.values()
        ),
        "rank_11_30_candidates": sum(
            1 for candidate in candidates if 11 <= candidate["rank"] <= RUNWAY_TOP
        ),
        "rank_11_30_unmatched": sum(
            1
            for candidate in candidates
            if 11 <= candidate["rank"] <= RUNWAY_TOP
            and not candidate["base_matched"]
        ),
        "selected_count": sum(1 for candidate in candidates if candidate["selected"]),
        "selected_unmatched_default_head": sum(
            1
            for candidate in candidates
            if candidate["selected"] and candidate["rank"] <= 10
        ),
        "selected_rank_11_30": sum(
            1
            for candidate in candidates
            if candidate["selected"] and candidate["rank"] > 10
        ),
    }


def collect_runway(args: argparse.Namespace) -> tuple[dict[str, Any], dict[str, Any]]:
    working_tree_status = git_output(["status", "--short"])
    corpus_payload = json.loads(args.corpus.read_text(encoding="utf-8"))
    corpus_rows = corpus_payload["repositories"]
    base = load_labelset(args.base_labelset)
    if base.version != "v6":
        raise SystemExit(f"v7 runway must extend v6, got {base.version}")
    base_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in base.families:
        base_by_repo[family["repo"]].append(family)

    candidates_by_split: dict[str, list[dict[str, Any]]] = {
        "dev": [],
        "heldout": [],
    }
    repositories_by_split: dict[str, dict[str, dict[str, Any]]] = {
        "dev": {},
        "heldout": {},
    }
    dev_source_files: dict[str, dict[str, Any]] = {}

    for metadata in sorted(corpus_rows, key=lambda row: row["id"]):
        repo_id = metadata["id"]
        split = metadata["split"]
        repo = args.repos_root / repo_id
        if not repo.is_dir():
            raise SystemExit(f"missing pinned repository: {repo_id}")
        actual_commit = repository_head(repo)
        if actual_commit != metadata["commit"]:
            raise SystemExit(
                f"{repo_id}: corpus revision {actual_commit} != {metadata['commit']}"
            )
        stdout, families, command = query_default_runway_repo(args.nose, repo)
        base_matched_top_10 = 0
        for rank, family in enumerate(families, start=1):
            members = normalized_members(family)
            match_id, match_overlap = best_label_match(
                members, base_by_repo.get(repo_id, [])
            )
            base_matched = match_id is not None
            if rank <= 10 and base_matched:
                base_matched_top_10 += 1
            lane = (
                "base-matched-default-head"
                if rank <= 10 and base_matched
                else (
                    "unmatched-default-head"
                    if rank <= 10
                    else (
                        "base-matched-rank-11-30"
                        if base_matched
                        else "unmatched-rank-11-30"
                    )
                )
            )
            compact_family = {
                "id": family["id"],
                "members": members,
                "member_count": len(members),
                "scope": family.get("scope"),
                "surface": family.get("surface"),
                "witness": family.get("witness"),
                "extraction_shape": family.get("extraction_shape"),
                "value": family.get("value"),
                "matched_v6_family_id": match_id,
                "matched_v6_member_overlap": match_overlap,
            }
            candidate = {
                "candidate_key": f"{repo_id}:{family['id']}:rank-{rank}",
                "repo": repo_id,
                "split": split,
                "language": metadata["primary_language"],
                "lane": lane,
                "rank": rank,
                "base_matched": base_matched,
                "family": compact_family,
                "raw_family_sha256": canonical_sha256(family),
            }
            candidate["candidate_sha256"] = canonical_sha256(
                runway_candidate_content(candidate)
            )
            candidates_by_split[split].append(candidate)
            if split == "dev":
                for member in members:
                    dev_source_files.setdefault(
                        member["file"], source_file_record(member["file"])
                    )
        top_10_reported = min(10, len(families))
        repositories_by_split[split][repo_id] = {
            "commit": actual_commit,
            "language": metadata["primary_language"],
            "split": split,
            "query_command": shlex.join(command),
            "query_stdout_sha256": hashlib.sha256(stdout).hexdigest(),
            "top_30_reported": len(families),
            "top_10_reported": top_10_reported,
            "base_matched_top_10": base_matched_top_10,
            "unmatched_top_10": top_10_reported - base_matched_top_10,
        }

    dev_candidates = candidates_by_split["dev"]
    heldout_candidates = candidates_by_split["heldout"]
    dev_keys = apply_runway_selection(dev_candidates)
    heldout_keys = apply_runway_selection(heldout_candidates)
    swift_repos = {
        row["id"]
        for row in corpus_rows
        if row["primary_language"].lower() == "swift"
    }
    selected_swift_repos = {
        candidate["repo"]
        for candidate in (*dev_candidates, *heldout_candidates)
        if candidate["selected"] and candidate["language"].lower() == "swift"
    }
    if selected_swift_repos != swift_repos:
        raise SystemExit(
            "v7 runway must select every Swift repository; missing="
            f"{sorted(swift_repos - selected_swift_repos)}"
        )

    nose = args.nose.resolve()
    sources = (
        ROOT / "bench/labels/label_refresh.py",
        ROOT / "bench/labels/labelset.py",
        ROOT / "bench/labels/query_schema.py",
    )
    provenance = {
        "command": shlex.join(["python3", *sys.argv]),
        "git_sha": git_output(["rev-parse", "HEAD"]),
        "working_tree_status_before_collection": working_tree_status,
        "nose_binary": rel(nose),
        "nose_binary_sha256": sha256_file(nose),
        "nose_version": nose_version(nose),
        "corpus_manifest": rel(args.corpus.resolve()),
        "corpus_manifest_sha256": sha256_file(args.corpus),
        "corpus_commit_digest": canonical_sha256(
            [
                {"id": row["id"], "commit": row["commit"]}
                for row in sorted(corpus_rows, key=lambda row: row["id"])
            ]
        ),
        "base_labelset": rel(args.base_labelset.resolve()),
        "base_labelset_sha256": sha256_file(args.base_labelset),
        "base_labelset_version": base.version,
        "rubric": rel(args.rubric.resolve()),
        "rubric_sha256": sha256_file(args.rubric),
        "collection_sources": [
            {"path": rel(path), "sha256": sha256_file(path)} for path in sources
        ],
    }
    selection_contract = {
        "seed": RUNWAY_SELECTION_SEED,
        "head_rule": "select every v6-unmatched default-surface rank 1-10 family",
        "deep_rule": (
            "select one v6-unmatched rank 11-30 family per repository by seeded hash"
        ),
        "heldout_policy": "selection commitments only; no source paths or judgments",
    }
    dev_artifact = {
        "schema": RUNWAY_SCHEMA,
        "split": "dev",
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": provenance,
        "selection_contract": selection_contract,
        "selection": {
            "selected_candidate_keys": dev_keys,
            "selected_candidate_keys_sha256": canonical_sha256(dev_keys),
        },
        "pool": runway_pool_summary(
            dev_candidates, repositories_by_split["dev"]
        ),
        "repositories": repositories_by_split["dev"],
        "source_files": dict(sorted(dev_source_files.items())),
        "candidates": sorted(
            dev_candidates, key=lambda row: (row["repo"], row["rank"])
        ),
    }
    commitments = [
        {
            key: candidate[key]
            for key in (
                "candidate_key",
                "candidate_sha256",
                "repo",
                "split",
                "language",
                "lane",
                "rank",
                "base_matched",
                "selected",
                "selection_reason",
                "selection_order",
            )
        }
        for candidate in sorted(
            heldout_candidates, key=lambda row: (row["repo"], row["rank"])
        )
    ]
    heldout_seal = {
        "schema": HELDOUT_SEAL_SCHEMA,
        "split": "heldout",
        "judgment_status": "sealed-unjudged",
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": provenance,
        "selection_contract": selection_contract,
        "selection": {
            "selected_candidate_keys": heldout_keys,
            "selected_candidate_keys_sha256": canonical_sha256(heldout_keys),
        },
        "pool": runway_pool_summary(
            heldout_candidates, repositories_by_split["heldout"]
        ),
        "repositories": repositories_by_split["heldout"],
        "candidate_commitments": commitments,
    }
    heldout_seal["commitment_sha256"] = canonical_sha256(heldout_seal)
    return dev_artifact, heldout_seal


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
    if existing.get("seed") != SELECTION_SEED:
        raise SystemExit("candidate selection seed mismatch")
    keys, expected_existing, expected_swift = selected_keys(
        candidates,
        existing_per_stratum=existing.get("per_language_split"),
        swift_per_repo=swift.get("per_repository"),
    )
    if existing.get("selected") != expected_existing or swift.get("selected") != expected_swift:
        raise SystemExit("candidate selection does not match the declared deterministic rule")
    expected_order = ordered_selection(expected_existing, expected_swift)
    recorded_keys = selection.get("selected_candidate_keys")
    if recorded_keys != expected_order:
        raise SystemExit("selected candidate key order does not match selection")
    if selection.get("selected_candidate_keys_sha256") != canonical_sha256(recorded_keys):
        raise SystemExit("selected candidate key digest mismatch")
    expected_positions = {key: index for index, key in enumerate(expected_order, start=1)}
    for candidate in candidates:
        key = candidate["candidate_key"]
        if candidate.get("selected") != (key in keys):
            raise SystemExit(f"{candidate['candidate_key']}: selected flag mismatch")
        if candidate.get("selection_order") != expected_positions.get(key):
            raise SystemExit(f"{candidate['candidate_key']}: selection order mismatch")

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


def load_schema_artifact(path: Path, schema: str, label: str) -> dict[str, Any]:
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"cannot read {label} {path}: {error}") from error
    if not isinstance(artifact, dict) or artifact.get("schema") != schema:
        raise SystemExit(f"{path}: unsupported {label} schema")
    return artifact


def load_panel_vote(
    path: Path,
    *,
    persona: str,
    artifact_path: Path,
    artifact: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    payload = load_schema_artifact(path, PANEL_VOTE_SCHEMA, f"{persona} panel vote")
    if payload.get("persona") != persona:
        raise SystemExit(f"{path}: expected {persona} persona")
    expected_source = {
        "path": rel(artifact_path.resolve()),
        "sha256": sha256_file(artifact_path),
    }
    if payload.get("source_artifact") != expected_source:
        raise SystemExit(f"{path}: panel vote source artifact mismatch")
    votes = payload.get("votes")
    if not isinstance(votes, list):
        raise SystemExit(f"{path}: votes must be an array")
    selected_keys = [
        candidate["candidate_key"]
        for candidate in sorted(
            (row for row in artifact["candidates"] if row["selected"]),
            key=lambda row: row["selection_order"],
        )
    ]
    recorded_keys: list[str] = []
    by_key: dict[str, dict[str, Any]] = {}
    for index, vote in enumerate(votes):
        if not isinstance(vote, dict):
            raise SystemExit(f"{path}: votes[{index}] must be an object")
        key = vote.get("candidate_key")
        if not isinstance(key, str) or not key or key in by_key:
            raise SystemExit(f"{path}: votes[{index}] has an invalid/duplicate key")
        try:
            validate_vote(vote, f"{path}:votes[{index}]")
        except ValueError as error:
            raise SystemExit(str(error)) from error
        rationale = vote.get("rationale")
        if not isinstance(rationale, str) or not rationale.strip():
            raise SystemExit(f"{path}: {key}: rationale is required")
        recorded_keys.append(key)
        by_key[key] = {
            "worthy": vote["worthy"],
            "reason": vote["reason"],
            "rationale": rationale.strip(),
        }
    if set(recorded_keys) != set(selected_keys):
        raise SystemExit(f"{path}: vote keys differ from the frozen selection")
    return by_key


def merge_runway_votes(
    artifact_path: Path,
    artifact: dict[str, Any],
    vote_paths: dict[str, Path],
) -> dict[str, Any]:
    votes = {
        persona: load_panel_vote(
            vote_paths[persona],
            persona=persona,
            artifact_path=artifact_path,
            artifact=artifact,
        )
        for persona in VOTE_NAMES
    }
    selected = sorted(
        (candidate for candidate in artifact["candidates"] if candidate["selected"]),
        key=lambda row: row["selection_order"],
    )
    rows = []
    for candidate in selected:
        key = candidate["candidate_key"]
        panel = {persona: votes[persona][key] for persona in VOTE_NAMES}
        decisions = {
            (vote["worthy"], vote["reason"]) for vote in panel.values()
        }
        rows.append(
            {
                "candidate_key": key,
                "repo": candidate["repo"],
                "language": candidate["language"],
                "rank": candidate["rank"],
                "votes": panel,
                "unanimous": len(decisions) == 1,
            }
        )
    return {
        "schema": PANEL_MERGE_SCHEMA,
        "split": "dev",
        "source_artifact": {
            "path": rel(artifact_path.resolve()),
            "sha256": sha256_file(artifact_path),
        },
        "vote_inputs": {
            persona: {
                "path": rel(path.resolve()),
                "sha256": sha256_file(path),
            }
            for persona, path in vote_paths.items()
        },
        "summary": {
            "candidates": len(rows),
            "unanimous": sum(1 for row in rows if row["unanimous"]),
            "disagreements": sum(1 for row in rows if not row["unanimous"]),
        },
        "rows": rows,
    }


def freeze_panel_vote(
    artifact_path: Path,
    artifact: dict[str, Any],
    input_path: Path,
    persona: str,
) -> dict[str, Any]:
    by_key = load_panel_vote(
        input_path,
        persona=persona,
        artifact_path=artifact_path,
        artifact=artifact,
    )
    selected_keys = [
        candidate["candidate_key"]
        for candidate in sorted(
            (row for row in artifact["candidates"] if row["selected"]),
            key=lambda row: row["selection_order"],
        )
    ]
    return {
        "schema": PANEL_VOTE_SCHEMA,
        "persona": persona,
        "source_artifact": {
            "path": rel(artifact_path.resolve()),
            "sha256": sha256_file(artifact_path),
        },
        "votes": [
            {"candidate_key": key, **by_key[key]} for key in selected_keys
        ],
    }


def build_runway_decisions(
    merge: dict[str, Any], arbitration_path: Path, arbitration: dict[str, Any]
) -> dict[str, Any]:
    if arbitration.get("split") != "dev":
        raise SystemExit("runway arbitration split must be dev")
    if arbitration.get("source_artifact") != merge.get("source_artifact"):
        raise SystemExit("runway arbitration source artifact mismatch")
    if arbitration.get("vote_inputs") != merge.get("vote_inputs"):
        raise SystemExit("runway arbitration vote inputs mismatch")
    records = arbitration.get("arbitrations")
    if not isinstance(records, list):
        raise SystemExit("runway arbitration needs an arbitrations array")
    by_key: dict[str, dict[str, Any]] = {}
    recorded_keys = []
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise SystemExit(f"arbitrations[{index}] must be an object")
        key = record.get("candidate_key")
        if not isinstance(key, str) or not key or key in by_key:
            raise SystemExit(f"arbitrations[{index}] has an invalid/duplicate key")
        try:
            validate_vote(record, f"arbitrations[{index}]")
        except ValueError as error:
            raise SystemExit(str(error)) from error
        confidence = record.get("confidence")
        if confidence not in {"high", "medium", "low"}:
            raise SystemExit(f"arbitrations[{index}].confidence is invalid")
        rationale = record.get("rationale")
        if not isinstance(rationale, str) or not rationale.strip():
            raise SystemExit(f"arbitrations[{index}].rationale is required")
        recorded_keys.append(key)
        by_key[key] = record
    expected_keys = [row["candidate_key"] for row in merge["rows"] if not row["unanimous"]]
    if recorded_keys != expected_keys:
        raise SystemExit("arbitration keys/order differ from the panel disagreement queue")

    decisions = []
    for row in merge["rows"]:
        if row["unanimous"]:
            panel_decision = row["votes"][VOTE_NAMES[0]]
            decision = {
                "candidate_key": row["candidate_key"],
                "votes": row["votes"],
                "arbiter": None,
                "confidence": "high",
                "note": (
                    "All three personas independently selected "
                    f"{panel_decision['reason']}."
                ),
            }
        else:
            resolution = by_key[row["candidate_key"]]
            decision = {
                "candidate_key": row["candidate_key"],
                "votes": row["votes"],
                "arbiter": {
                    "worthy": resolution["worthy"],
                    "reason": resolution["reason"],
                    "rationale": resolution["rationale"].strip(),
                },
                "confidence": resolution["confidence"],
                "note": resolution["rationale"].strip(),
            }
        decisions.append(decision)
    return {
        "schema": DECISIONS_SCHEMA,
        "split": "dev",
        "source_artifact": merge["source_artifact"],
        "vote_inputs": merge["vote_inputs"],
        "arbitration_input": {
            "path": rel(arbitration_path.resolve()),
            "sha256": sha256_file(arbitration_path),
        },
        "decisions": decisions,
    }


def validate_runway_selection(
    candidates: list[dict[str, Any]], selection: dict[str, Any], *, label: str
) -> None:
    expected_keys, expected_reasons = runway_selection_summary(candidates)
    recorded_keys = selection.get("selected_candidate_keys")
    if recorded_keys != expected_keys:
        raise SystemExit(f"{label}: selected candidate order differs from the rule")
    if selection.get("selected_candidate_keys_sha256") != canonical_sha256(expected_keys):
        raise SystemExit(f"{label}: selected candidate digest mismatch")
    positions = {key: index for index, key in enumerate(expected_keys, start=1)}
    for candidate in candidates:
        key = candidate["candidate_key"]
        if candidate.get("selected") != (key in positions):
            raise SystemExit(f"{label}: {key}: selected flag mismatch")
        if candidate.get("selection_reason") != expected_reasons.get(key):
            raise SystemExit(f"{label}: {key}: selection reason mismatch")
        if candidate.get("selection_order") != positions.get(key):
            raise SystemExit(f"{label}: {key}: selection order mismatch")


def validate_runway_dev(
    artifact: dict[str, Any], *, live_root: Path | None = None
) -> None:
    if artifact.get("split") != "dev" or artifact.get("query_schema_version") != QUERY_SCHEMA_VERSION:
        raise SystemExit("dev runway split/query schema mismatch")
    candidates = artifact.get("candidates")
    if not isinstance(candidates, list) or not candidates:
        raise SystemExit("dev runway needs a non-empty candidates array")
    seen: set[str] = set()
    referenced_sources: set[str] = set()
    for index, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            raise SystemExit(f"dev candidates[{index}] must be an object")
        key = candidate.get("candidate_key")
        if not isinstance(key, str) or not key or key in seen:
            raise SystemExit(f"dev candidates[{index}] has an invalid/duplicate key")
        seen.add(key)
        if candidate.get("split") != "dev":
            raise SystemExit(f"{key}: held-out candidate leaked into dev runway")
        if candidate.get("candidate_sha256") != canonical_sha256(
            runway_candidate_content(candidate)
        ):
            raise SystemExit(f"{key}: candidate content digest mismatch")
        family = candidate.get("family")
        if not isinstance(family, dict) or not isinstance(family.get("members"), list):
            raise SystemExit(f"{key}: candidate family/members missing")
        referenced_sources.update(member["file"] for member in family["members"])
    validate_runway_selection(candidates, artifact.get("selection", {}), label="dev runway")
    repositories = artifact.get("repositories")
    if not isinstance(repositories, dict) or not repositories:
        raise SystemExit("dev runway repositories missing")
    if artifact.get("pool") != runway_pool_summary(candidates, repositories):
        raise SystemExit("dev runway pool summary mismatch")
    source_files = artifact.get("source_files")
    if not isinstance(source_files, dict) or set(source_files) != referenced_sources:
        raise SystemExit("dev runway source-file inventory mismatch")
    provenance = artifact.get("provenance")
    if not isinstance(provenance, dict):
        raise SystemExit("dev runway provenance missing")
    if provenance.get("working_tree_status_before_collection"):
        raise SystemExit("dev runway must be collected from a clean working tree")
    revision = provenance.get("git_sha")
    collection_sources = provenance.get("collection_sources")
    if not isinstance(revision, str) or not revision:
        raise SystemExit("dev runway collection revision missing")
    if not isinstance(collection_sources, list) or not collection_sources:
        raise SystemExit("dev runway collection source provenance missing")
    seen_collection_sources: set[str] = set()
    for index, record in enumerate(collection_sources):
        if not isinstance(record, dict):
            raise SystemExit(f"collection_sources[{index}] must be an object")
        path = record.get("path")
        digest = record.get("sha256")
        if (
            not isinstance(path, str)
            or not path
            or path in seen_collection_sources
            or not isinstance(digest, str)
            or len(digest) != 64
        ):
            raise SystemExit(f"collection_sources[{index}] is invalid")
        seen_collection_sources.add(path)
        if git_file_sha256(revision, path) != digest:
            raise SystemExit(f"collection source provenance mismatch: {path}")
    for path_key, hash_key in (
        ("corpus_manifest", "corpus_manifest_sha256"),
        ("base_labelset", "base_labelset_sha256"),
        ("rubric", "rubric_sha256"),
    ):
        path = ROOT / provenance.get(path_key, "")
        if not path.is_file() or sha256_file(path) != provenance.get(hash_key):
            raise SystemExit(f"dev runway provenance mismatch for {path_key}")
    base_path = ROOT / provenance["base_labelset"]
    if provenance.get("base_labelset_sha256") != FROZEN_V6_SHA256:
        raise SystemExit("dev runway does not extend the byte-frozen v6 manifest")
    base = load_labelset(base_path)
    if (
        base.version != "v6"
        or canonical_sha256(base.families) != FROZEN_V6_FAMILIES_SHA256
        or not base.inputs
        or base.inputs[0].get("sha256") != FROZEN_V5_SHA256
    ):
        raise SystemExit("byte-frozen v5/v6 label projection changed")
    v5 = load_labelset(ROOT / "bench/labels/refactoring_families.v5.json")
    if canonical_sha256(v5.families) != FROZEN_V5_FAMILIES_SHA256:
        raise SystemExit("byte-frozen v5 label projection changed")

    if live_root is not None:
        for path_key, hash_key in (
            ("nose_binary", "nose_binary_sha256"),
        ):
            path = live_root / provenance.get(path_key, "")
            if not path.is_file() or sha256_file(path) != provenance.get(hash_key):
                raise SystemExit(f"dev runway provenance mismatch for {path_key}")
        for repo, row in repositories.items():
            if repository_head(live_root / "bench/repos" / repo) != row["commit"]:
                raise SystemExit(f"{repo}: live corpus revision mismatch")
        for path_text, record in source_files.items():
            path = live_root / path_text
            if (
                not path.is_file()
                or path.stat().st_size != record.get("bytes")
                or sha256_file(path) != record.get("sha256")
            ):
                raise SystemExit(f"dev runway source provenance mismatch: {path_text}")


def contains_heldout_judgment_or_source(value: object) -> str | None:
    forbidden = {
        "family",
        "families",
        "members",
        "source_files",
        "worthy",
        "reason",
        "votes",
        "arbiter",
        "note",
        "confidence",
        "labeler",
    }
    if isinstance(value, dict):
        for key, nested in value.items():
            if key in forbidden:
                return key
            found = contains_heldout_judgment_or_source(nested)
            if found is not None:
                return found
    elif isinstance(value, list):
        for nested in value:
            found = contains_heldout_judgment_or_source(nested)
            if found is not None:
                return found
    return None


def validate_heldout_seal(seal: dict[str, Any]) -> None:
    if (
        seal.get("split") != "heldout"
        or seal.get("judgment_status") != "sealed-unjudged"
        or seal.get("query_schema_version") != QUERY_SCHEMA_VERSION
    ):
        raise SystemExit("held-out seal split/status/query schema mismatch")
    forbidden = contains_heldout_judgment_or_source(seal)
    if forbidden is not None:
        raise SystemExit(f"held-out seal leaks forbidden field: {forbidden}")
    commitment = seal.get("commitment_sha256")
    content = dict(seal)
    content.pop("commitment_sha256", None)
    if commitment != canonical_sha256(content):
        raise SystemExit("held-out seal commitment mismatch")
    candidates = seal.get("candidate_commitments")
    if not isinstance(candidates, list) or not candidates:
        raise SystemExit("held-out seal needs candidate commitments")
    seen: set[str] = set()
    for index, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            raise SystemExit(f"held-out commitments[{index}] must be an object")
        key = candidate.get("candidate_key")
        if not isinstance(key, str) or not key or key in seen:
            raise SystemExit(f"held-out commitments[{index}] has an invalid/duplicate key")
        seen.add(key)
        if candidate.get("split") != "heldout":
            raise SystemExit(f"{key}: non-held-out candidate in seal")
        digest = candidate.get("candidate_sha256")
        if not isinstance(digest, str) or len(digest) != 64:
            raise SystemExit(f"{key}: candidate commitment digest missing")
    validate_runway_selection(candidates, seal.get("selection", {}), label="held-out seal")
    repositories = seal.get("repositories")
    if not isinstance(repositories, dict) or not repositories:
        raise SystemExit("held-out seal repositories missing")
    if seal.get("pool") != runway_pool_summary(candidates, repositories):
        raise SystemExit("held-out seal pool summary mismatch")


def validate_runway_pair(
    dev: dict[str, Any], seal: dict[str, Any], *, live_root: Path | None = None
) -> None:
    validate_runway_dev(dev, live_root=live_root)
    validate_heldout_seal(seal)
    if dev.get("provenance") != seal.get("provenance"):
        raise SystemExit("dev and held-out provenance differs")
    if dev.get("selection_contract") != seal.get("selection_contract"):
        raise SystemExit("dev and held-out selection contracts differ")
    selected_swift = {
        candidate["repo"]
        for candidate in [
            *dev["candidates"],
            *seal["candidate_commitments"],
        ]
        if candidate["selected"] and candidate["language"].lower() == "swift"
    }
    all_swift = {
        repo
        for repo, row in {
            **dev["repositories"],
            **seal["repositories"],
        }.items()
        if row["language"].lower() == "swift"
    }
    if selected_swift != all_swift or len(all_swift) != 15:
        raise SystemExit("v7 runway selection must cover all 15 Swift repositories")


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


def runway_coverage(
    labels: LoadedLabelset, dev: dict[str, Any], seal: dict[str, Any]
) -> dict[str, Any]:
    if labels.version != "v7":
        raise SystemExit(f"runway coverage requires v7 labelset, got {labels.version}")
    selected = {
        candidate["candidate_key"]: candidate
        for candidate in dev["candidates"]
        if candidate["selected"]
    }
    runway_labels = [
        family
        for family in labels.families
        if family.get("selection", {}).get("runway") == "v7-default-head"
    ]
    if any(family.get("split") != "dev" for family in runway_labels):
        raise SystemExit("held-out v7 judgments must remain unavailable before closeout")
    by_key = {family["candidate_key"]: family for family in runway_labels}
    if len(by_key) != len(runway_labels) or set(by_key) != set(selected):
        raise SystemExit(
            "v7 runway labels do not match dev selection; "
            f"missing={sorted(set(selected) - set(by_key))}, "
            f"extra={sorted(set(by_key) - set(selected))}"
        )
    for key, family in by_key.items():
        candidate = selected[key]
        source_family = candidate["family"]
        checks = {
            "candidate_sha256": candidate["candidate_sha256"],
            "repo": candidate["repo"],
            "split": "dev",
            "language": candidate["language"],
            "scope": source_family["scope"],
            "family_id": source_family["id"],
            "members": source_family["members"],
        }
        for field, expected in checks.items():
            if family.get(field) != expected:
                raise SystemExit(f"{key}: v7 label field {field} differs from source")
        if family.get("metric_eligibility") != [PRECISION_METRIC]:
            raise SystemExit(f"{key}: v7 label is not precision-only")
        if metric_eligible(family, "worthy_recall"):
            raise SystemExit(f"{key}: v7 label contaminated worthy recall")

    head = [candidate for candidate in dev["candidates"] if candidate["rank"] <= 10]
    unmatched_head = [candidate for candidate in head if not candidate["base_matched"]]
    missing_head = [
        candidate["candidate_key"]
        for candidate in unmatched_head
        if candidate["candidate_key"] not in by_key
    ]
    if missing_head:
        raise SystemExit(f"v7 misses unmatched dev default-head candidates: {missing_head}")
    matched = sum(1 for candidate in head if candidate["base_matched"]) + len(
        unmatched_head
    )
    pct = round(100 * matched / len(head), 4) if head else 0.0
    if pct < 90.0:
        raise SystemExit(f"v7 dev default-head coverage {pct}% is below 90%")

    swift_labels = [
        family for family in runway_labels if family["language"].lower() == "swift"
    ]
    if {family["worthy"] for family in swift_labels} != {False, True}:
        raise SystemExit("v7 dev Swift runway must contain both worthiness classes")
    selected_swift_repos = {
        candidate["repo"]
        for candidate in [
            *dev["candidates"],
            *seal["candidate_commitments"],
        ]
        if candidate["selected"] and candidate["language"].lower() == "swift"
    }
    return {
        "labelset_version": labels.version,
        "dev": {
            "top_10": len(head),
            "v6_matched": sum(1 for candidate in head if candidate["base_matched"]),
            "v7_matched": matched,
            "v7_pct": pct,
            "new_unmatched_head_labels": len(unmatched_head),
            "rank_11_30_labels": sum(
                1 for candidate in selected.values() if candidate["rank"] > 10
            ),
        },
        "runway_labels": len(runway_labels),
        "runway_worthy": sum(1 for family in runway_labels if family["worthy"]),
        "runway_not_worthy": sum(1 for family in runway_labels if not family["worthy"]),
        "swift_selected_repositories": sorted(selected_swift_repos),
        "heldout_judgments": 0,
    }


def validate_runway_evaluation(
    path: Path,
    labels: LoadedLabelset,
    dev: dict[str, Any],
) -> None:
    if sha256_file(path) != RUNWAY_EVALUATION_SHA256:
        raise SystemExit("checked v7 runway evaluation digest changed")
    report = load_schema_artifact(
        path, "nose.product_quality_evaluation.v3", "v7 runway evaluation"
    )
    if report.get("query_schema_version") != QUERY_SCHEMA_VERSION:
        raise SystemExit("v7 runway evaluation query schema changed")
    if report.get("repository_count") != 120:
        raise SystemExit("v7 runway evaluation must cover all 120 repositories")
    configuration = report.get("configuration", {})
    expected_configuration = {
        "bootstrap_resamples": 2000,
        "bootstrap_seed": 1,
        "cache_policy": "disabled (baseline-safe)",
        "default_product_parity_check": True,
        "precision_surface": "default",
        "rank": "extractability",
        "splits": ["dev", "heldout"],
    }
    for field, expected in expected_configuration.items():
        if configuration.get(field) != expected:
            raise SystemExit(f"v7 runway evaluation configuration changed: {field}")
    provenance = report.get("provenance", {})
    if provenance.get("working_tree_status_before_measurement"):
        raise SystemExit("v7 runway evaluation was not measured from a clean tree")
    if provenance.get("nose_binary_sha256") != dev["provenance"]["nose_binary_sha256"]:
        raise SystemExit("v7 runway evaluation binary differs from the frozen runway")
    if (
        provenance.get("labelset_version") != labels.version
        or provenance.get("labelset_sha256") != sha256_file(labels.path)
    ):
        raise SystemExit("v7 runway evaluation labelset provenance changed")
    expected_inputs = [
        {"path": rel(record["path"]), "sha256": record["sha256"]}
        for record in labels.inputs
    ]
    if provenance.get("labelset_inputs") != expected_inputs:
        raise SystemExit("v7 runway evaluation labelset inputs changed")
    if provenance.get("corpus_manifest_sha256") != dev["provenance"][
        "corpus_manifest_sha256"
    ]:
        raise SystemExit("v7 runway evaluation corpus manifest changed")
    revision = provenance.get("git_sha")
    sources = provenance.get("evaluation_sources")
    if not isinstance(revision, str) or not isinstance(sources, list) or not sources:
        raise SystemExit("v7 runway evaluation source provenance missing")
    for index, record in enumerate(sources):
        if not isinstance(record, dict):
            raise SystemExit(f"evaluation_sources[{index}] must be an object")
        source_path = record.get("path")
        digest = record.get("sha256")
        if not isinstance(source_path, str) or not isinstance(digest, str):
            raise SystemExit(f"evaluation_sources[{index}] is invalid")
        if git_file_sha256(revision, source_path) != digest:
            raise SystemExit(f"evaluation source provenance mismatch: {source_path}")

    overall = report.get("metrics", {})
    expected_metrics = {
        "dev": {
            "precision_at_10": (382, 658, 58.0547),
            "label_match_coverage": (658, 658, 100.0),
            "worthy_recall": (2716, 2849, 95.3317),
            "labels": 5790,
        },
        "heldout": {
            "precision_at_10": (222, 375, 59.2),
            "label_match_coverage": (375, 538, 69.7026),
            "worthy_recall": (2005, 2091, 95.8871),
            "labels": 4072,
        },
    }
    for split, expected in expected_metrics.items():
        metrics = overall.get(split, {}).get("OVERALL", {})
        if metrics.get("labels") != expected["labels"]:
            raise SystemExit(f"v7 runway evaluation {split} label count changed")
        for name in ("precision_at_10", "label_match_coverage", "worthy_recall"):
            metric = metrics.get(name, {})
            observed = (metric.get("hits"), metric.get("n"), metric.get("pct"))
            if observed != expected[name]:
                raise SystemExit(f"v7 runway evaluation {split} {name} changed")


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
    issue = "#840" if artifact.get("schema") == RUNWAY_SCHEMA else "#812"
    lines = [
        f"# {issue} {split} label context",
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
        selection = {
            "lane": candidate["lane"],
            "product_rank": candidate["rank"],
            "selection_order": candidate["selection_order"],
        }
        if artifact.get("schema") == RUNWAY_SCHEMA:
            selection.update(
                {
                    "runway": "v7-default-head",
                    "selection_reason": candidate["selection_reason"],
                }
            )
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
                "selection": selection,
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
    selected_order = ordered_selection(existing, swift)
    positions = {key: index for index, key in enumerate(selected_order, start=1)}
    for candidate in candidates:
        key = candidate["candidate_key"]
        candidate["candidate_sha256"] = canonical_sha256(candidate_content(candidate))
        candidate["selected"] = key in keys
        candidate["selection_order"] = positions.get(key)
    artifact = {
        "schema": ARTIFACT_SCHEMA,
        "candidates": candidates,
        "selection": {
            "existing": {
                "seed": SELECTION_SEED,
                "per_language_split": 5,
                "selected": existing,
            },
            "swift": {"per_repository": 3, "selected": swift},
            "selected_candidate_keys": selected_order,
            "selected_candidate_keys_sha256": canonical_sha256(selected_order),
        },
    }
    validate_candidate_artifact(artifact)
    artifact["selection"]["selected_candidate_keys"] = list(reversed(selected_order))
    artifact["selection"]["selected_candidate_keys_sha256"] = canonical_sha256(
        artifact["selection"]["selected_candidate_keys"]
    )
    try:
        validate_candidate_artifact(artifact)
    except SystemExit as error:
        assert "key order" in str(error)
    else:
        raise AssertionError("selection-order drift must fail closed")
    with tempfile.TemporaryDirectory(prefix="nose-label-refresh-self-test-") as directory:
        path = Path(directory) / "artifact.json"
        path.write_text(json.dumps({"schema": ARTIFACT_SCHEMA, "candidates": []}))
        assert load_artifact(path)["schema"] == ARTIFACT_SCHEMA

    runway_candidates = []
    for rank, matched in ((1, False), (2, True), (11, False), (12, False)):
        candidate = {
            "candidate_key": f"runway:family-{rank}:rank-{rank}",
            "repo": "runway",
            "split": "dev",
            "language": "Test",
            "lane": "synthetic",
            "rank": rank,
            "base_matched": matched,
            "family": {"members": []},
            "raw_family_sha256": f"{rank:064x}",
        }
        candidate["candidate_sha256"] = canonical_sha256(
            runway_candidate_content(candidate)
        )
        runway_candidates.append(candidate)
    runway_keys = apply_runway_selection(runway_candidates)
    assert runway_keys[0] == "runway:family-1:rank-1"
    assert len(runway_keys) == 2
    runway_selection = {
        "selected_candidate_keys": runway_keys,
        "selected_candidate_keys_sha256": canonical_sha256(runway_keys),
    }
    validate_runway_selection(
        runway_candidates, runway_selection, label="synthetic runway"
    )
    runway_selection["selected_candidate_keys"] = list(reversed(runway_keys))
    try:
        validate_runway_selection(
            runway_candidates, runway_selection, label="synthetic runway"
        )
    except SystemExit as error:
        assert "selected candidate order" in str(error)
    else:
        raise AssertionError("runway selection-order drift must fail closed")

    commitments = [
        {
            key: candidate[key]
            for key in (
                "candidate_key",
                "candidate_sha256",
                "repo",
                "split",
                "language",
                "lane",
                "rank",
                "base_matched",
                "selected",
                "selection_reason",
                "selection_order",
            )
        }
        for candidate in runway_candidates
    ]
    for commitment in commitments:
        commitment["split"] = "heldout"
    heldout_keys = [
        row["candidate_key"] for row in commitments if row["selected"]
    ]
    repositories = {
        "runway": {
            "commit": "0" * 40,
            "language": "Test",
            "split": "heldout",
            "top_10_reported": 2,
            "base_matched_top_10": 1,
            "unmatched_top_10": 1,
        }
    }
    seal = {
        "schema": HELDOUT_SEAL_SCHEMA,
        "split": "heldout",
        "judgment_status": "sealed-unjudged",
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": {},
        "selection_contract": {},
        "selection": {
            "selected_candidate_keys": heldout_keys,
            "selected_candidate_keys_sha256": canonical_sha256(heldout_keys),
        },
        "pool": runway_pool_summary(commitments, repositories),
        "repositories": repositories,
        "candidate_commitments": commitments,
    }
    seal["commitment_sha256"] = canonical_sha256(seal)
    validate_heldout_seal(seal)
    seal["candidate_commitments"][0]["worthy"] = True
    seal["commitment_sha256"] = canonical_sha256(seal)
    try:
        validate_heldout_seal(seal)
    except SystemExit as error:
        assert "leaks forbidden field" in str(error)
    else:
        raise AssertionError("held-out judgments must fail closed")

    with tempfile.TemporaryDirectory(prefix="nose-runway-panel-self-test-") as directory:
        root = Path(directory)
        artifact_path = root / "dev.json"
        artifact_path.write_text("{}\n", encoding="utf-8")
        panel_paths = {}
        selected = sorted(
            (row for row in runway_candidates if row["selected"]),
            key=lambda row: row["selection_order"],
        )
        for persona in VOTE_NAMES:
            path = root / f"{persona}.json"
            path.write_text(
                json.dumps(
                    {
                        "schema": PANEL_VOTE_SCHEMA,
                        "persona": persona,
                        "source_artifact": {
                            "path": rel(artifact_path.resolve()),
                            "sha256": sha256_file(artifact_path),
                        },
                        "votes": [
                            {
                                "candidate_key": candidate["candidate_key"],
                                "worthy": True,
                                "reason": "extract-helper",
                                "rationale": "The repeated body has one helper boundary.",
                            }
                            for candidate in selected
                        ],
                    }
                ),
                encoding="utf-8",
            )
            panel_paths[persona] = path
        merged = merge_runway_votes(
            artifact_path, {"candidates": runway_candidates}, panel_paths
        )
        assert merged["summary"] == {
            "candidates": 2,
            "unanimous": 2,
            "disagreements": 0,
        }
        arbitration_path = root / "arbitration.json"
        arbitration = {
            "schema": PANEL_ARBITRATION_SCHEMA,
            "split": "dev",
            "source_artifact": merged["source_artifact"],
            "vote_inputs": merged["vote_inputs"],
            "arbitrations": [],
        }
        arbitration_path.write_text(json.dumps(arbitration), encoding="utf-8")
        decisions = build_runway_decisions(merged, arbitration_path, arbitration)
        assert len(decisions["decisions"]) == 2
        assert all(row["arbiter"] is None for row in decisions["decisions"])
        merged["rows"][0]["unanimous"] = False
        arbitration["arbitrations"] = [
            {
                "candidate_key": merged["rows"][0]["candidate_key"],
                "worthy": False,
                "reason": "trivial",
                "rationale": "The repeated expression is too small to extract.",
                "confidence": "medium",
            }
        ]
        arbitration_path.write_text(json.dumps(arbitration), encoding="utf-8")
        decisions = build_runway_decisions(merged, arbitration_path, arbitration)
        assert decisions["decisions"][0]["arbiter"]["reason"] == "trivial"
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
    runway_parser = subparsers.add_parser("collect-runway")
    runway_parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    runway_parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    runway_parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    runway_parser.add_argument(
        "--base-labelset",
        type=Path,
        default=ROOT / "bench/labels/refactoring_families.v6.json",
    )
    runway_parser.add_argument("--rubric", type=Path, default=DEFAULT_RUBRIC)
    runway_parser.add_argument("--dev-output", type=Path, required=True)
    runway_parser.add_argument("--heldout-seal-output", type=Path, required=True)
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--candidates", type=Path, required=True)
    validate_parser.add_argument("--labelset", type=Path)
    validate_parser.add_argument("--live", action="store_true")
    validate_runway_parser = subparsers.add_parser("validate-runway")
    validate_runway_parser.add_argument("--dev-candidates", type=Path, required=True)
    validate_runway_parser.add_argument("--heldout-seal", type=Path, required=True)
    validate_runway_parser.add_argument("--labelset", type=Path)
    validate_runway_parser.add_argument("--evaluation", type=Path)
    validate_runway_parser.add_argument("--live", action="store_true")
    context_parser = subparsers.add_parser("context")
    context_parser.add_argument("--candidates", type=Path, required=True)
    context_parser.add_argument("--split", choices=("dev", "heldout"), required=True)
    context_parser.add_argument("--output", type=Path, required=True)
    runway_context_parser = subparsers.add_parser("runway-context")
    runway_context_parser.add_argument("--dev-candidates", type=Path, required=True)
    runway_context_parser.add_argument("--output", type=Path, required=True)
    component_parser = subparsers.add_parser("build-component")
    component_parser.add_argument("--candidates", type=Path, required=True)
    component_parser.add_argument("--decisions", type=Path, required=True)
    component_parser.add_argument("--split", choices=("dev", "heldout"), required=True)
    component_parser.add_argument("--output", type=Path, required=True)
    runway_component_parser = subparsers.add_parser("build-runway-component")
    runway_component_parser.add_argument("--dev-candidates", type=Path, required=True)
    runway_component_parser.add_argument("--decisions", type=Path, required=True)
    runway_component_parser.add_argument("--output", type=Path, required=True)
    runway_votes_parser = subparsers.add_parser("merge-runway-votes")
    runway_votes_parser.add_argument("--dev-candidates", type=Path, required=True)
    for persona in VOTE_NAMES:
        runway_votes_parser.add_argument(f"--{persona}", type=Path, required=True)
    runway_votes_parser.add_argument("--output", type=Path, required=True)
    freeze_vote_parser = subparsers.add_parser("freeze-runway-vote")
    freeze_vote_parser.add_argument("--dev-candidates", type=Path, required=True)
    freeze_vote_parser.add_argument("--persona", choices=VOTE_NAMES, required=True)
    freeze_vote_parser.add_argument("--input", type=Path, required=True)
    freeze_vote_parser.add_argument("--output", type=Path, required=True)
    runway_decisions_parser = subparsers.add_parser("build-runway-decisions")
    runway_decisions_parser.add_argument("--dev-candidates", type=Path, required=True)
    for persona in VOTE_NAMES:
        runway_decisions_parser.add_argument(f"--{persona}", type=Path, required=True)
    runway_decisions_parser.add_argument("--arbitrations", type=Path, required=True)
    runway_decisions_parser.add_argument("--output", type=Path, required=True)
    freeze_arbitration_parser = subparsers.add_parser("freeze-runway-arbitration")
    freeze_arbitration_parser.add_argument("--dev-candidates", type=Path, required=True)
    for persona in VOTE_NAMES:
        freeze_arbitration_parser.add_argument(f"--{persona}", type=Path, required=True)
    freeze_arbitration_parser.add_argument("--input", type=Path, required=True)
    freeze_arbitration_parser.add_argument("--output", type=Path, required=True)
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
    if args.command == "collect-runway":
        dev, seal = collect_runway(args)
        args.dev_output.parent.mkdir(parents=True, exist_ok=True)
        args.heldout_seal_output.parent.mkdir(parents=True, exist_ok=True)
        args.dev_output.write_text(
            json.dumps(dev, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        args.heldout_seal_output.write_text(
            json.dumps(seal, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(
            f"wrote {args.dev_output}: {dev['pool']['selected_count']} dev selected; "
            f"{args.heldout_seal_output}: {seal['pool']['selected_count']} held-out sealed"
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
    if args.command == "validate-runway":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        seal = load_schema_artifact(
            args.heldout_seal, HELDOUT_SEAL_SCHEMA, "held-out seal"
        )
        validate_runway_pair(dev, seal, live_root=ROOT if args.live else None)
        if args.labelset:
            labels = load_labelset(args.labelset)
            report = runway_coverage(labels, dev, seal)
            if args.evaluation:
                validate_runway_evaluation(args.evaluation, labels, dev)
            print(json.dumps(report, indent=2, sort_keys=True))
        elif args.evaluation:
            raise SystemExit("--evaluation requires --labelset")
        else:
            print("default-head v7 runway validation passed")
        return
    if args.command == "context":
        artifact = load_artifact(args.candidates)
        validate_candidate_artifact(artifact)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(render_context(artifact, args.split), encoding="utf-8")
        print(f"wrote {args.output}")
        return
    if args.command == "runway-context":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(render_context(dev, "dev"), encoding="utf-8")
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
    if args.command == "build-runway-component":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        component = build_component(
            args.dev_candidates, dev, args.decisions, "dev", args.output
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(component, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {args.output}: {len(component['families'])} labels")
        return
    if args.command == "merge-runway-votes":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        merged = merge_runway_votes(
            args.dev_candidates,
            dev,
            {persona: getattr(args, persona) for persona in VOTE_NAMES},
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(merged, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(
            f"wrote {args.output}: {merged['summary']['unanimous']} unanimous, "
            f"{merged['summary']['disagreements']} disagreements"
        )
        return
    if args.command == "freeze-runway-vote":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        frozen = freeze_panel_vote(
            args.dev_candidates, dev, args.input, args.persona
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(frozen, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {args.output}: {len(frozen['votes'])} {args.persona} votes")
        return
    if args.command == "build-runway-decisions":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        merged = merge_runway_votes(
            args.dev_candidates,
            dev,
            {persona: getattr(args, persona) for persona in VOTE_NAMES},
        )
        arbitration = load_schema_artifact(
            args.arbitrations, PANEL_ARBITRATION_SCHEMA, "runway arbitration"
        )
        decisions = build_runway_decisions(merged, args.arbitrations, arbitration)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(decisions, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {args.output}: {len(decisions['decisions'])} decisions")
        return
    if args.command == "freeze-runway-arbitration":
        dev = load_schema_artifact(args.dev_candidates, RUNWAY_SCHEMA, "dev runway")
        validate_runway_dev(dev)
        merged = merge_runway_votes(
            args.dev_candidates,
            dev,
            {persona: getattr(args, persona) for persona in VOTE_NAMES},
        )
        arbitration = load_schema_artifact(
            args.input, PANEL_ARBITRATION_SCHEMA, "runway arbitration"
        )
        build_runway_decisions(merged, args.input, arbitration)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(arbitration, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(
            f"wrote {args.output}: {len(arbitration['arbitrations'])} arbitrations"
        )
        return
    raise SystemExit("choose collect or validate, or pass --self-test")


if __name__ == "__main__":
    main()
