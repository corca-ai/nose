#!/usr/bin/env python3
"""Mine bounded git history for divergent-edit findings using nose base= JSON."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


SCHEMA = "nose.divergent_history.v1"
DEFAULT_MAX_COMMITS = 25


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


def repository_root(path: Path) -> Path:
    root = run(
        ["git", "-C", path.as_posix(), "rev-parse", "--show-toplevel"],
        check=True,
    ).stdout.strip()
    return Path(root)


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
        "command": " ".join(command_for_output),
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
    ok_rows = [row for row in commit_rows if row.get("status") == "ok"]
    findings = sum(len(row.get("items", [])) for row in ok_rows)
    strict_findings = sum(
        1
        for row in ok_rows
        for item in row.get("items", [])
        if (item.get("gate") or {}).get("fail_default")
    )
    return {
        "schema": SCHEMA,
        "provenance": {
            "repo": args.repo.as_posix(),
            "range": args.rev_range,
            "current_head": git_output(repo, ["rev-parse", "HEAD"]),
            "range_first_commit": commits[0],
            "range_last_commit": commits[-1],
            "nose_binary": args.nose.as_posix(),
            "nose_binary_sha256": sha256_file(nose),
            "script": "scripts/divergent-history-mining.py",
        },
        "parameters": {
            "path": args.path,
            "mode": args.mode,
            "min_size": args.min_size,
            "min_lines": args.min_lines,
            "exclude": args.exclude,
            "ignore_file": args.ignore_file.as_posix() if args.ignore_file else None,
            "max_commits": args.max_commits,
            "first_parent": args.first_parent,
            "merge_policy": args.merge_policy,
        },
        "summary": {
            "commits_considered": len(commits),
            "commits_analyzed": len(ok_rows),
            "commits_skipped": len(skipped)
            + len([row for row in commit_rows if row.get("status") != "ok"]),
            "findings": findings,
            "strict_findings": strict_findings,
            "groups": len(groups),
            "strict_groups": sum(1 for group in groups if group["gate_fail_default"]),
        },
        "commits": commit_rows,
        "skipped_commits": skipped,
        "groups": groups,
    }


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
    groups = group_findings(sample_commits)
    assert len(groups) == 1
    assert groups[0]["occurrence_count"] == 2
    assert groups[0]["gate_fail_default"] is True
    assert groups[0]["commits"] == ["c1", "c2"]
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
    args = parser.parse_args()
    if args.self_test:
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
