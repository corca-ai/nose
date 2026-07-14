#!/usr/bin/env python3
"""Collect exclusion-census v2 from the pinned 120-repository corpus."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any

import soundness_exclusions as sx

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "bench/goldens/corpus.json"
DEFAULT_PRUNE = ROOT / "bench/labels/prune_manifest.json"


def repositories(manifest: Path, selected: set[str]) -> list[dict[str, Any]]:
    rows = sx.load(manifest)["repositories"]
    known = {row["id"] for row in rows}
    unknown = selected - known
    if unknown:
        raise ValueError(f"unknown corpus repositories: {', '.join(sorted(unknown))}")
    return sorted(
        (row for row in rows if not selected or row["id"] in selected),
        key=lambda row: row["id"],
    )


def git_head(repo: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        text=True, capture_output=True, check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def collect_one(
    nose: Path, repos_root: Path, output: Path, repo: dict[str, Any]
) -> dict[str, Any]:
    repo_id = repo["id"]
    source = repos_root / repo_id
    observed = git_head(source)
    if observed != repo["commit"]:
        raise ValueError(
            f"{repo_id}: expected pin {repo['commit']}, observed {observed or 'missing'}"
        )
    census = (output / "raw" / f"{repo_id}.json").resolve()
    log = output / "logs" / f"{repo_id}.log"
    try:
        source_arg = source.relative_to(ROOT)
    except ValueError:
        source_arg = source
    environment = os.environ.copy()
    environment["RAYON_NUM_THREADS"] = "1"
    with log.open("wb") as handle:
        result = subprocess.run(
            [str(nose), "verify", str(source_arg), "--max-violations", "0",
             "--exclusion-census", str(census)],
            cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT,
            env=environment, check=False,
        )
    if result.returncode != 0:
        raise ValueError(f"{repo_id}: nose verify failed ({result.returncode}); see {log}")
    sx.validate_raw(sx.load(census))
    return {
        "id": repo_id,
        "commit": repo["commit"],
        "census_sha256": sx.sha256_file(census),
    }


def prune_check(repos_root: Path) -> None:
    subprocess.run(
        ["python3", "bench/prune_corpus.py", "--repos-root", str(repos_root),
         "--check-manifest"],
        cwd=ROOT, check=True,
    )


def collect(args: argparse.Namespace) -> None:
    nose = args.nose.resolve()
    manifest = args.manifest.resolve()
    repos_root = args.repos_root.resolve()
    output = args.output.resolve()
    if not nose.is_file() or not os.access(nose, os.X_OK):
        raise ValueError(f"nose binary is not executable: {nose}")
    selected = set(args.repo)
    rows = repositories(manifest, selected)
    official = not selected and manifest == DEFAULT_MANIFEST.resolve() and len(rows) == 120
    if official:
        prune_check(repos_root)
    shutil.rmtree(output, ignore_errors=True)
    (output / "raw").mkdir(parents=True)
    (output / "logs").mkdir()
    print(f"soundness census: {len(rows)} repositories, jobs={args.jobs}")
    results = []
    failures = []
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = {
            executor.submit(collect_one, nose, repos_root, output, row): row["id"]
            for row in rows
        }
        for future in as_completed(futures):
            repo_id = futures[future]
            try:
                results.append(future.result())
                print(f"  ok {repo_id}")
            except Exception as error:  # aggregate independent repository failures
                failures.append(f"{repo_id}: {error}")
                print(f"  FAIL {repo_id}: {error}")
    if failures:
        raise ValueError("census collection failed:\n" + "\n".join(sorted(failures)))
    if official:
        prune_check(repos_root)
    results.sort(key=lambda row: row["id"])
    prune = sx.load(DEFAULT_PRUNE) if official else None
    evidence = {
        "schema": "nose-soundness-census-run/v2",
        "complete": official,
        "nose": {
            "sha256": sx.sha256_file(nose),
            "version": subprocess.check_output([str(nose), "--version"], text=True).strip(),
        },
        "corpus_manifest_sha256": sx.sha256_file(manifest),
        "prune_manifest_sha256": sx.sha256_file(DEFAULT_PRUNE) if official else None,
        "pruned_corpus_digest_sha256": (
            prune["corpus_digest_after_prune"]["hex"] if prune else None
        ),
        "repositories": results,
    }
    sx.write(output / "evidence.json", evidence)
    print(f"wrote {output / 'evidence.json'}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nose", type=Path, default=ROOT / "target/release/nose")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    parser.add_argument(
        "--output", type=Path, default=ROOT / "target/soundness-lab/corpus-exclusions"
    )
    parser.add_argument("--jobs", type=int, default=min(6, os.cpu_count() or 2))
    parser.add_argument("--repo", action="append", default=[])
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    collect(args)


if __name__ == "__main__":
    main()
