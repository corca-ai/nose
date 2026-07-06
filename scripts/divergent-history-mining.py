#!/usr/bin/env python3
"""Mine bounded git history for divergent-edit findings using nose base= JSON."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


SCHEMA = "nose.divergent_history.v1"
SCHEMA_REVISION = 2
DEFAULT_MAX_COMMITS = 25
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


def run(
    command: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if check and result.returncode != 0:
        location = f" in {cwd}" if cwd else ""
        raise SystemExit(
            f"{' '.join(command)} failed{location}: {result.stderr.strip()}"
        )
    return result


def git(repo: Path, args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["git", "-C", repo.as_posix(), *args], check=check)


def git_output(repo: Path, args: list[str]) -> str:
    return git(repo, args).stdout.strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def repository_root(path: Path) -> Path:
    root = run(
        ["git", "-C", path.as_posix(), "rev-parse", "--show-toplevel"],
        check=True,
    ).stdout.strip()
    return Path(root)


def display_path(path: Path, repo: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(repo).as_posix()
    except ValueError:
        return path.as_posix()


def git_status_lines(repo: Path) -> list[str]:
    return [
        line
        for line in git_output(repo, ["status", "--short", "--untracked-files=no"]).splitlines()
        if line
    ]


def binary_version(binary: Path) -> dict[str, Any]:
    result = run([binary.as_posix(), "--version"], check=False)
    if result.returncode != 0:
        return {
            "status": "error",
            "stderr": result.stderr.strip(),
            "stdout": result.stdout.strip(),
        }
    return {"status": "ok", "text": result.stdout.strip()}


def rev_list(
    repo: Path,
    rev_range: str,
    *,
    first_parent: bool,
    max_commits: int,
) -> list[str]:
    args = ["rev-list", "--reverse"]
    if first_parent:
        args.append("--first-parent")
    args.extend(["--max-count", str(max_commits), rev_range])
    commits = [line.strip() for line in git_output(repo, args).splitlines() if line.strip()]
    if not commits:
        raise SystemExit(f"no commits matched range {rev_range!r}")
    return commits


def commit_parent(repo: Path, commit: str, *, merge_policy: str) -> tuple[str | None, str | None]:
    parents = git_output(repo, ["show", "-s", "--format=%P", commit]).split()
    if not parents:
        return None, "root-commit"
    if len(parents) > 1 and merge_policy == "skip":
        return None, "merge-commit"
    return parents[0], None


def commit_metadata(repo: Path, commit: str) -> dict[str, Any]:
    raw = git_output(repo, ["show", "-s", "--format=%H%x00%P%x00%ct%x00%s", commit])
    sha, parents, author_time, subject = raw.split("\x00", 3)
    return {
        "commit": sha,
        "parents": parents.split() if parents else [],
        "author_time": int(author_time),
        "subject": subject,
    }


def site_key_identity(site: dict[str, Any]) -> dict[str, Any]:
    return {
        "tree": site.get("tree"),
        "file": site.get("file"),
        "name": site.get("name"),
        "kind": site.get("kind"),
        "lang": site.get("lang"),
    }


def site_summary(site: dict[str, Any]) -> dict[str, Any]:
    return {
        **site_key_identity(site),
        "start_line": site.get("start_line"),
        "end_line": site.get("end_line"),
        "enclosing_unit": site.get("enclosing_unit"),
    }


def sorted_site_keys(sites: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(
        (site_key_identity(site) for site in sites),
        key=lambda site: (
            site.get("tree") or "",
            site.get("file") or "",
            site.get("name") or "",
            site.get("kind") or "",
            site.get("lang") or "",
        ),
    )


def sorted_site_summaries(sites: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(
        (site_summary(site) for site in sites),
        key=lambda site: (
            site.get("tree") or "",
            site.get("file") or "",
            site.get("name") or "",
            site.get("kind") or "",
            site.get("lang") or "",
            site.get("start_line") or 0,
            site.get("end_line") or 0,
        ),
    )


def finding_key(item: dict[str, Any]) -> str:
    lane = item.get("lane")
    if lane == "new-copy":
        site_groups = {"current_only": sorted_site_keys(item.get("current_only") or [])}
    else:
        site_groups = {
            "changed": sorted_site_keys(item.get("changed") or []),
            "not_updated": sorted_site_keys(item.get("not_updated") or []),
        }
    material = {
        "lane": lane,
        "base_family_id": item.get("base_family_id") or item.get("family_id"),
        "taxonomy_hint": item.get("taxonomy_hint"),
        "sites": site_groups,
    }
    encoded = json.dumps(material, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()[:16]


def occurrence_summary(commit: str, parent: str, item: dict[str, Any]) -> dict[str, Any]:
    sites: dict[str, Any] = {}
    if item.get("lane") == "new-copy":
        sites["current_only"] = sorted_site_summaries(item.get("current_only") or [])
    else:
        sites["changed"] = sorted_site_summaries(item.get("changed") or [])
        sites["not_updated"] = sorted_site_summaries(item.get("not_updated") or [])
    return {
        "commit": commit,
        "parent": parent,
        "family_id": item.get("family_id"),
        "base_family_id": item.get("base_family_id"),
        "lane": item.get("lane"),
        "tier": item.get("tier"),
        "taxonomy_hint": item.get("taxonomy_hint"),
        "gate_fail_default": (item.get("gate") or {}).get("fail_default"),
        "sites": sites,
    }


def group_findings(commits: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    for commit_row in commits:
        commit = commit_row["commit"]
        parent = commit_row["parent"]
        for item in commit_row.get("items", []):
            key = finding_key(item)
            occurrence = occurrence_summary(commit, parent, item)
            group = grouped.setdefault(
                key,
                {
                    "key": key,
                    "occurrences": [],
                    "commits": [],
                    "family_ids": [],
                    "base_family_ids": [],
                    "lanes": [],
                    "tiers": [],
                    "taxonomy_hints": [],
                    "gate_fail_default": False,
                    "representative": occurrence,
                },
            )
            group["occurrences"].append(occurrence)
            group["commits"].append(commit)
            if item.get("family_id"):
                group["family_ids"].append(item["family_id"])
            if item.get("base_family_id"):
                group["base_family_ids"].append(item["base_family_id"])
            if item.get("lane"):
                group["lanes"].append(item["lane"])
            if item.get("tier"):
                group["tiers"].append(item["tier"])
            if item.get("taxonomy_hint"):
                group["taxonomy_hints"].append(item["taxonomy_hint"])
            group["gate_fail_default"] = group["gate_fail_default"] or bool(
                (item.get("gate") or {}).get("fail_default")
            )

    results = []
    for group in grouped.values():
        group["commits"] = list(dict.fromkeys(group["commits"]))
        for field in ("family_ids", "base_family_ids", "lanes", "tiers", "taxonomy_hints"):
            group[field] = sorted(dict.fromkeys(group[field]))
        group["first_commit"] = group["commits"][0]
        group["last_commit"] = group["commits"][-1]
        group["occurrence_count"] = len(group["occurrences"])
        results.append(group)
    return sorted(
        results,
        key=lambda group: (
            not group["gate_fail_default"],
            group["tiers"],
            group["first_commit"],
            group["key"],
        ),
    )


def sorted_counts(values: list[str | None]) -> dict[str, int]:
    counter = Counter(value or "none" for value in values)
    return {key: counter[key] for key in sorted(counter)}


def history_summary(
    commit_rows: list[dict[str, Any]],
    skipped: list[dict[str, Any]],
    groups: list[dict[str, Any]],
) -> dict[str, Any]:
    ok_rows = [row for row in commit_rows if row.get("status") == "ok"]
    non_ok_rows = [row for row in commit_rows if row.get("status") != "ok"]
    items = [item for row in ok_rows for item in row.get("items", [])]
    gate_fail_default_findings = sum(
        1 for item in items if (item.get("gate") or {}).get("fail_default")
    )
    tier_counts = sorted_counts([item.get("tier") for item in items])
    lane_counts = sorted_counts([item.get("lane") for item in items])
    taxonomy_hint_counts = sorted_counts([item.get("taxonomy_hint") for item in items])
    return {
        "commits_considered": len(commit_rows) + len(skipped),
        "commits_analyzed": len(ok_rows),
        "commits_skipped": len(skipped) + len(non_ok_rows),
        "findings": len(items),
        "strict_findings": gate_fail_default_findings,
        "review_findings": tier_counts.get("review", 0),
        "report_only_findings": tier_counts.get("report-only", 0),
        "gate_fail_default_findings": gate_fail_default_findings,
        "groups": len(groups),
        "strict_groups": sum(1 for group in groups if group["gate_fail_default"]),
        "gate_fail_default_groups": sum(
            1 for group in groups if group["gate_fail_default"]
        ),
        "query_failed_commits": sum(
            1 for row in commit_rows if row.get("status") == "query-failed"
        ),
        "tier_counts": tier_counts,
        "lane_counts": lane_counts,
        "taxonomy_hint_counts": taxonomy_hint_counts,
    }


def suppression_behavior(ignore_file: Path | None) -> dict[str, Any]:
    return {
        "applied_before_grouping": True,
        "active_output_omits_suppressed": True,
        "source": (
            "The underlying base=<parent> query applies nose.ignore.json, "
            "--ignore-file, and configured ignore-file rules before this harness groups findings."
        ),
        "ignore_file": ignore_file.as_posix() if ignore_file else None,
    }


def redaction_policy() -> dict[str, Any]:
    return {
        "source_snippets": "omitted",
        "diffs": "omitted",
        "paths": "public-repository-relative",
        "symbols": "public-repository-symbol-names",
        "source_bearing_keys_forbidden": sorted(SOURCE_BEARING_KEYS),
    }


def query_args(args: argparse.Namespace, parent: str) -> list[str]:
    command = ["query", args.path, f"base={parent}", "--format", "json", "top=0"]
    if args.mode:
        command.extend(["--mode", args.mode])
    if args.min_size is not None:
        command.extend(["--min-size", str(args.min_size)])
    if args.min_lines is not None:
        command.extend(["--min-lines", str(args.min_lines)])
    for pattern in args.exclude:
        command.extend(["--exclude", pattern])
    if args.ignore_file:
        command.extend(["--ignore-file", args.ignore_file.as_posix()])
    return command


def analyze_commit(
    *,
    worktree: Path,
    nose: Path,
    args: argparse.Namespace,
    commit: str,
    parent: str,
) -> dict[str, Any]:
    git(worktree, ["checkout", "--detach", "--quiet", commit])
    if not (worktree / args.path).exists():
        return {
            **commit_metadata(worktree, commit),
            "parent": parent,
            "status": "skipped",
            "skip_reason": f"path-missing:{args.path}",
            "items": [],
        }

    command = [nose.as_posix(), *query_args(args, parent)]
    command_for_output = [args.nose.as_posix(), *query_args(args, parent)]
    result = run(command, cwd=worktree, check=False)
    row = {
        **commit_metadata(worktree, commit),
        "parent": parent,
        "status": "ok" if result.returncode == 0 else "query-failed",
        "command": shlex.join(command_for_output),
        "items": [],
        "summary": {},
    }
    if result.returncode != 0:
        row["stderr"] = result.stderr.strip()
        if args.fail_fast:
            raise SystemExit(
                f"query failed for {commit}: {result.stderr.strip()}"
            )
        return row
    payload = json.loads(result.stdout)
    row["query_schema_version"] = payload.get("schema_version")
    row["query_tool"] = payload.get("tool")
    row["query_view"] = payload.get("view")
    row["query_base"] = payload.get("base")
    row["query_path"] = payload.get("path")
    row["summary"] = payload.get("summary", {})
    row["items"] = payload.get("items", [])
    return row


def create_worktree(repo: Path, commit: str, root: Path) -> Path:
    worktree = root / "worktree"
    git(repo, ["worktree", "add", "--detach", "--quiet", worktree.as_posix(), commit])
    return worktree


def remove_worktree(repo: Path, worktree: Path) -> bool:
    result = git(repo, ["worktree", "remove", "--force", worktree.as_posix()], check=False)
    if result.returncode != 0:
        print(
            "warning: failed to remove temporary worktree; pruning stale metadata: "
            f"{result.stderr.strip()}",
            file=sys.stderr,
        )
        return False
    return True


def mine_history(args: argparse.Namespace) -> dict[str, Any]:
    repo = repository_root(args.repo.resolve())
    nose = args.nose.resolve()
    if not nose.exists():
        raise SystemExit(f"missing nose binary: {nose}")
    script = Path(__file__).resolve()
    source_status = git_status_lines(repo)
    commits = rev_list(
        repo,
        args.rev_range,
        first_parent=args.first_parent,
        max_commits=args.max_commits,
    )
    commit_rows: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []

    tmp_context = None
    worktree = None
    if args.keep_worktree:
        tmp_root = Path(tempfile.mkdtemp(prefix="nose-divergent-history-"))
    else:
        tmp_context = tempfile.TemporaryDirectory(prefix="nose-divergent-history-")
        tmp_root = Path(tmp_context.name)
    try:
        worktree = create_worktree(repo, commits[0], tmp_root)
        for commit in commits:
            parent, skip_reason = commit_parent(repo, commit, merge_policy=args.merge_policy)
            if parent is None:
                skipped.append(
                    {
                        **commit_metadata(repo, commit),
                        "status": "skipped",
                        "skip_reason": skip_reason,
                    }
                )
                continue
            commit_rows.append(
                analyze_commit(
                    worktree=worktree,
                    nose=nose,
                    args=args,
                    commit=commit,
                    parent=parent,
                )
            )
    finally:
        if args.keep_worktree and worktree is not None:
            print(f"kept temporary worktree: {worktree}", file=sys.stderr)
        else:
            removed = True
            if worktree is not None:
                removed = remove_worktree(repo, worktree)
            if tmp_context is not None:
                tmp_context.cleanup()
            if not removed:
                git(repo, ["worktree", "prune"], check=False)

    groups = group_findings([row for row in commit_rows if row.get("status") == "ok"])
    return {
        "schema": SCHEMA,
        "schema_revision": SCHEMA_REVISION,
        "artifact_kind": "divergent-history-mining-run",
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "bounds": {
            "offline_only": True,
            "pr_ci": False,
            "max_commits": args.max_commits,
        },
        "provenance": {
            "repo": args.repo.as_posix(),
            "repo_root": repo.as_posix(),
            "range": args.rev_range,
            "current_head": git_output(repo, ["rev-parse", "HEAD"]),
            "current_branch": git_output(repo, ["rev-parse", "--abbrev-ref", "HEAD"]),
            "source_dirty": bool(source_status),
            "source_status": source_status,
            "range_first_commit": commits[0],
            "range_last_commit": commits[-1],
            "nose_binary": args.nose.as_posix(),
            "nose_binary_sha256": sha256_file(nose),
            "nose_version": binary_version(nose),
            "script": display_path(script, repo),
            "script_sha256": sha256_file(script),
            "argv": sys.argv,
            "command": shlex.join(sys.argv),
        },
        "target": {
            "repo": args.repo.as_posix(),
            "resolved_range": args.rev_range,
            "first_commit": commits[0],
            "last_commit": commits[-1],
            "commit_count": len(commits),
            "commit_list_sha256": sha256_text("\n".join(commits) + "\n"),
        },
        "parameters": {
            "path": args.path,
            "mode": args.mode,
            "min_size": args.min_size,
            "min_lines": args.min_lines,
            "exclude": args.exclude,
            "ignore_file": args.ignore_file.as_posix() if args.ignore_file else None,
            "ignore_file_sha256": (
                sha256_file(args.ignore_file)
                if args.ignore_file and args.ignore_file.exists()
                else None
            ),
            "max_commits": args.max_commits,
            "first_parent": args.first_parent,
            "merge_policy": args.merge_policy,
        },
        "summary": history_summary(commit_rows, skipped, groups),
        "suppression_behavior": suppression_behavior(args.ignore_file),
        "redaction": redaction_policy(),
        "raw_records": None,
        "commits": commit_rows,
        "skipped_commits": skipped,
        "groups": groups,
    }


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


def require(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def validate_history_artifact(data: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    require(errors, data.get("schema") == SCHEMA, f"schema must be {SCHEMA!r}")
    revision = data.get("schema_revision", 1)
    require(errors, isinstance(revision, int), "schema_revision must be an integer")

    for key in ("provenance", "parameters", "summary", "commits", "skipped_commits", "groups"):
        require(errors, key in data, f"missing top-level {key}")
    if errors:
        return errors

    commits = data["commits"]
    skipped = data["skipped_commits"]
    groups = data["groups"]
    require(errors, isinstance(commits, list), "commits must be a list")
    require(errors, isinstance(skipped, list), "skipped_commits must be a list")
    require(errors, isinstance(groups, list), "groups must be a list")
    if errors:
        return errors

    ok_rows = [row for row in commits if row.get("status") == "ok"]
    recomputed_groups = group_findings(ok_rows)
    expected_summary = history_summary(commits, skipped, recomputed_groups)
    summary = data["summary"]
    legacy_summary_keys = (
        "commits_considered",
        "commits_analyzed",
        "commits_skipped",
        "findings",
        "strict_findings",
        "groups",
        "strict_groups",
    )
    revision2_summary_keys = (
        "review_findings",
        "report_only_findings",
        "gate_fail_default_findings",
        "gate_fail_default_groups",
        "query_failed_commits",
        "tier_counts",
        "lane_counts",
        "taxonomy_hint_counts",
    )
    for key in legacy_summary_keys:
        require(errors, key in summary, f"summary missing {key}")
        if key in summary:
            require(
                errors,
                summary[key] == expected_summary[key],
                f"summary.{key} is stale: expected {expected_summary[key]!r}, got {summary[key]!r}",
            )
    for key in revision2_summary_keys:
        if revision >= 2:
            require(errors, key in summary, f"summary missing {key}")
        if key in summary:
            require(
                errors,
                summary[key] == expected_summary[key],
                f"summary.{key} is stale: expected {expected_summary[key]!r}, got {summary[key]!r}",
            )

    existing_by_key = {group.get("key"): group for group in groups}
    recomputed_by_key = {group["key"]: group for group in recomputed_groups}
    require(errors, len(existing_by_key) == len(groups), "groups contain duplicate or missing keys")
    require(errors, set(existing_by_key) == set(recomputed_by_key), "groups do not match recomputed keys")
    for key, expected in recomputed_by_key.items():
        existing = existing_by_key.get(key)
        if not existing:
            continue
        for field in ("occurrence_count", "gate_fail_default", "first_commit", "last_commit"):
            require(
                errors,
                existing.get(field) == expected.get(field),
                f"group {key}.{field} is stale",
            )

    parameters = data["parameters"]
    max_commits = parameters.get("max_commits")
    require(errors, isinstance(max_commits, int), "parameters.max_commits must be an integer")
    if isinstance(max_commits, int):
        require(
            errors,
            expected_summary["commits_considered"] <= max_commits,
            "commits_considered exceeds parameters.max_commits",
        )

    if revision >= 2:
        for key in (
            "artifact_kind",
            "bounds",
            "provenance",
            "target",
            "suppression_behavior",
            "redaction",
            "raw_records",
        ):
            require(errors, key in data, f"revision 2 artifact missing {key}")
        provenance = data["provenance"]
        for key in (
            "repo_root",
            "current_branch",
            "source_dirty",
            "source_status",
            "nose_version",
            "script_sha256",
            "argv",
            "command",
        ):
            require(errors, key in provenance, f"provenance missing {key}")
        bounds = data.get("bounds") or {}
        require(errors, bounds.get("offline_only") is True, "bounds.offline_only must be true")
        require(errors, bounds.get("pr_ci") is False, "bounds.pr_ci must be false")
        require(errors, bounds.get("max_commits") == max_commits, "bounds.max_commits mismatch")
        suppression = data.get("suppression_behavior") or {}
        require(
            errors,
            suppression.get("applied_before_grouping") is True,
            "suppression_behavior.applied_before_grouping must be true",
        )
        require(
            errors,
            suppression.get("active_output_omits_suppressed") is True,
            "suppression_behavior.active_output_omits_suppressed must be true",
        )

    source_bearing = find_source_bearing_keys(data)
    require(
        errors,
        not source_bearing,
        f"artifact contains source-bearing keys: {', '.join(source_bearing)}",
    )
    return errors


def check_artifact(path: Path) -> None:
    data = json.loads(path.read_text())
    errors = validate_history_artifact(data)
    if errors:
        raise SystemExit(
            "divergent history artifact check failed:\n"
            + "\n".join(f"- {error}" for error in errors)
        )
    summary = data["summary"]
    default_failing = summary.get(
        "gate_fail_default_findings",
        summary.get("strict_findings", 0),
    )
    print(
        "divergent history artifact OK: "
        f"{path} ({summary['commits_analyzed']} commits, "
        f"{summary['groups']} groups, "
        f"{default_failing} default-failing findings)"
    )


def run_self_test() -> None:
    sample_commits = [
        {
            "commit": "c1",
            "parent": "p1",
            "items": [
                {
                    "family_id": "fam-a",
                    "base_family_id": "fam-a",
                    "lane": "base-divergence",
                    "tier": "strict",
                    "taxonomy_hint": "missed_propagation",
                    "gate": {"fail_default": True},
                    "changed": [
                        {
                            "tree": "base",
                            "file": "src/a.py",
                            "name": "f",
                            "kind": "function",
                            "lang": "python",
                            "start_line": 1,
                            "end_line": 3,
                        }
                    ],
                    "not_updated": [
                        {
                            "tree": "base",
                            "file": "src/b.py",
                            "name": "f",
                            "kind": "function",
                            "lang": "python",
                            "start_line": 10,
                            "end_line": 12,
                        }
                    ],
                }
            ],
        },
        {
            "commit": "c2",
            "parent": "p2",
            "items": [
                {
                    "family_id": "fam-a",
                    "base_family_id": "fam-a",
                    "lane": "base-divergence",
                    "tier": "strict",
                    "taxonomy_hint": "missed_propagation",
                    "gate": {"fail_default": True},
                    "changed": [
                        {
                            "tree": "base",
                            "file": "src/a.py",
                            "name": "f",
                            "kind": "function",
                            "lang": "python",
                            "start_line": 2,
                            "end_line": 4,
                        }
                    ],
                    "not_updated": [
                        {
                            "tree": "base",
                            "file": "src/b.py",
                            "name": "f",
                            "kind": "function",
                            "lang": "python",
                            "start_line": 11,
                            "end_line": 13,
                        }
                    ],
                }
            ],
        },
    ]
    for row in sample_commits:
        row["status"] = "ok"
    groups = group_findings(sample_commits)
    assert len(groups) == 1
    assert groups[0]["occurrence_count"] == 2
    assert groups[0]["gate_fail_default"] is True
    assert groups[0]["commits"] == ["c1", "c2"]
    sample_artifact = {
        "schema": SCHEMA,
        "schema_revision": SCHEMA_REVISION,
        "artifact_kind": "divergent-history-mining-run",
        "bounds": {"offline_only": True, "pr_ci": False, "max_commits": 2},
        "provenance": {
            "repo": ".",
            "repo_root": ".",
            "range": "p1..c2",
            "current_head": "c2",
            "current_branch": "main",
            "source_dirty": False,
            "source_status": [],
            "range_first_commit": "c1",
            "range_last_commit": "c2",
            "nose_binary": "target/release/nose",
            "nose_binary_sha256": "x",
            "nose_version": {"status": "ok", "text": "nose 0.0.0"},
            "script": "scripts/divergent-history-mining.py",
            "script_sha256": "y",
            "argv": ["scripts/divergent-history-mining.py"],
            "command": "scripts/divergent-history-mining.py",
        },
        "target": {
            "repo": ".",
            "resolved_range": "p1..c2",
            "first_commit": "c1",
            "last_commit": "c2",
            "commit_count": 2,
            "commit_list_sha256": "z",
        },
        "parameters": {
            "path": ".",
            "mode": "syntax,semantic",
            "min_size": 8,
            "min_lines": None,
            "exclude": [],
            "ignore_file": None,
            "ignore_file_sha256": None,
            "max_commits": 2,
            "first_parent": True,
            "merge_policy": "skip",
        },
        "summary": history_summary(sample_commits, [], groups),
        "suppression_behavior": suppression_behavior(None),
        "redaction": redaction_policy(),
        "raw_records": None,
        "commits": sample_commits,
        "skipped_commits": [],
        "groups": groups,
    }
    errors = validate_history_artifact(sample_artifact)
    assert not errors, errors
    print("divergent history mining self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--range", dest="rev_range", required=False)
    parser.add_argument("--base")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--path", default=".")
    parser.add_argument("--nose", type=Path, default=Path("target/release/nose"))
    parser.add_argument("--mode")
    parser.add_argument("--min-size", type=int)
    parser.add_argument("--min-lines", type=int)
    parser.add_argument("--exclude", action="append", default=[])
    parser.add_argument("--ignore-file", type=Path)
    parser.add_argument("--max-commits", type=int, default=DEFAULT_MAX_COMMITS)
    parser.add_argument("--first-parent", action="store_true")
    parser.add_argument("--merge-policy", choices=("skip", "first-parent"), default="skip")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--fail-fast", action="store_true")
    parser.add_argument("--keep-worktree", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check-artifact", type=Path)
    args = parser.parse_args()
    if args.self_test or args.check_artifact:
        return args
    if not args.rev_range:
        if not args.base:
            raise SystemExit("pass --range <rev-range> or --base <ref> [--head <ref>]")
        args.rev_range = f"{args.base}..{args.head}"
    if args.max_commits <= 0:
        raise SystemExit("--max-commits must be positive")
    if args.ignore_file:
        args.ignore_file = args.ignore_file.resolve()
    return args


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if args.check_artifact:
        check_artifact(args.check_artifact)
        return 0
    output = mine_history(args)
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
