#!/usr/bin/env python3
"""Evaluate v5 product precision and worthy-family recall by language and split.

The base metric uses either historical value order or nose's native
extractability order. The report also retains the historical anti-unification
re-rank as a comparison, with deterministic bootstrap confidence intervals.

Examples:

    python3 bench/labels/eval_by_language.py --rank extractability --bootstrap 500
    python3 bench/labels/eval_by_language.py --limit-repos 1 --bootstrap 20
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import random
import shlex
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

from query_schema import QUERY_SCHEMA_VERSION, member_locations, query_families


sys.setrecursionlimit(100000)
ROOT = Path(__file__).resolve().parents[2]
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v5.json"
CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
PRUNE_MANIFEST = ROOT / "bench" / "labels" / "prune_manifest.json"
RNG_SEED = 1
COMPAT_RNG = random.Random(RNG_SEED)

spec = importlib.util.spec_from_file_location("au", ROOT / "bench" / "labels" / "antiunify_probe.py")
au = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(au)


def rel(path: str) -> str:
    path = path.replace(str(ROOT) + "/", "")
    index = path.find("bench/repos/")
    return path[index:] if index >= 0 else path


def overlaps(a: dict[str, Any], b: dict[str, Any]) -> bool:
    return a["file"] == b["file"] and not (
        a["end_line"] < b["start_line"] or b["end_line"] < a["start_line"]
    )


def matched_locations(family: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "file": rel(location["file"]),
            "start_line": location["start_line"],
            "end_line": location["end_line"],
        }
        for location in member_locations(family, source="product evaluator query family")
    ]


# Historical name retained for the adjacent experiment scripts that import it.
mlocs = matched_locations
ov = overlaps


def refactorability(family: dict[str, Any]) -> float:
    locations = matched_locations(family)
    if len(locations) < 2:
        return 1.0
    features = au.family_features(locations[:2])
    if not features:
        return 1.0
    result = features["abstractness"]
    if features["value_hole_ratio"] >= 0.15:
        result *= 0.4
    if features["struct_hole_ratio"] >= 0.30:
        result *= 0.5
    return result


def split_modes(raw_modes: list[str] | None) -> list[str]:
    modes = []
    for raw in raw_modes or []:
        modes.extend(part.strip() for part in raw.split(",") if part.strip())
    return modes


def query_repo(
    repo: Path,
    *,
    nose: Path = DEFAULT_NOSE,
    mode: str | None = None,
    cache_dir: Path | None = None,
    top: int = 1000000,
    timeout: int = 300,
) -> list[dict[str, Any]]:
    command = [str(nose), "query", str(repo), "all", f"top={top}", "--format", "json"]
    modes = split_modes([mode] if mode else [])
    if modes:
        command += ["--mode", ",".join(modes)]
    if cache_dir:
        command += ["--cache-dir", str(cache_dir)]
    result = subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"query failed for {repo}: exit {result.returncode}: {result.stderr.strip()}"
        )
    return query_families(result.stdout, source=f"nose query {repo}")


def confidence_interval(
    flags: list[int],
    *,
    bootstrap: int,
    rng: random.Random,
) -> tuple[float, float]:
    if not flags:
        return (0.0, 0.0)
    count = len(flags)
    means = []
    for _ in range(bootstrap):
        hits = sum(flags[rng.randrange(count)] for _ in range(count))
        means.append(hits / count)
    means.sort()
    return (
        means[int(0.025 * bootstrap)] * 100,
        means[int(0.975 * bootstrap)] * 100,
    )


def binary_metric(flags: list[int], *, bootstrap: int, rng: random.Random) -> dict[str, Any]:
    hits = sum(flags)
    count = len(flags)
    low, high = confidence_interval(flags, bootstrap=bootstrap, rng=rng)
    return {
        "hits": hits,
        "n": count,
        "pct": round((100 * hits / count) if count else 0.0, 4),
        "ci95": [round(low, 4), round(high, 4)],
    }


# Historical helper signature retained for extractability_vs_value.py.
def ci(flags: list[int], b: int = 2000) -> tuple[float, float]:
    return confidence_interval(flags, bootstrap=b, rng=COMPAT_RNG)


def best_label_flags(
    order: list[dict[str, Any]],
    labels: list[dict[str, Any]],
    *,
    limit: int = 10,
) -> list[int]:
    flags = []
    for family in order[:limit]:
        best = None
        best_overlap = 0
        sites = matched_locations(family)
        for label in labels:
            overlap = sum(
                1
                for site in sites
                for member in label["members"]
                if overlaps(site, member)
            )
            if overlap > best_overlap:
                best = label
                best_overlap = overlap
        if best is not None:
            flags.append(1 if best["worthy"] else 0)
    return flags


def worthy_recall_flags(
    families: list[dict[str, Any]], labels: list[dict[str, Any]]
) -> list[int]:
    flags = []
    family_locations = [matched_locations(family) for family in families]
    for label in labels:
        if not label["worthy"]:
            continue
        hit = any(
            any(overlaps(site, member) for site in sites for member in label["members"])
            for sites in family_locations
        )
        flags.append(1 if hit else 0)
    return flags


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_output(args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def corpus_digest(corpus: dict[str, dict[str, Any]], repo_ids: list[str]) -> str:
    digest = hashlib.sha256()
    for repo_id in repo_ids:
        record = corpus[repo_id]
        digest.update(
            (
                f"{repo_id}\t{record['split']}\t{record['primary_language']}\t"
                f"{record['commit']}\n"
            ).encode()
        )
    return digest.hexdigest()


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


def nose_version(nose: Path) -> str:
    result = subprocess.run(
        [str(nose), "--version"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"{nose} --version failed: {result.stderr.strip()}")
    return result.stdout.strip()


def build_metrics(
    accumulator: dict[tuple[str, str], dict[str, Any]],
    *,
    bootstrap: int,
) -> dict[str, dict[str, dict[str, Any]]]:
    rng = random.Random(RNG_SEED)
    report: dict[str, dict[str, dict[str, Any]]] = {}
    for split in ("dev", "heldout"):
        split_report: dict[str, dict[str, Any]] = {}
        rows = sorted(
            ((language, data) for (language, row_split), data in accumulator.items() if row_split == split),
            key=lambda row: row[0],
        )
        for language, data in rows:
            split_report[language] = {
                "repositories": data["repositories"],
                "labels": data["labels"],
                "worthy_labels": data["worthy_labels"],
                "precision_at_10": binary_metric(data["base"], bootstrap=bootstrap, rng=rng),
                "antiunification_rerank_precision_at_10": binary_metric(
                    data["rerank"], bootstrap=bootstrap, rng=rng
                ),
                "worthy_recall": binary_metric(data["recall"], bootstrap=bootstrap, rng=rng),
            }
        all_base = [flag for _, data in rows for flag in data["base"]]
        all_rerank = [flag for _, data in rows for flag in data["rerank"]]
        all_recall = [flag for _, data in rows for flag in data["recall"]]
        split_report["OVERALL"] = {
            "repositories": sum(data["repositories"] for _, data in rows),
            "labels": sum(data["labels"] for _, data in rows),
            "worthy_labels": sum(data["worthy_labels"] for _, data in rows),
            "precision_at_10": binary_metric(all_base, bootstrap=bootstrap, rng=rng),
            "antiunification_rerank_precision_at_10": binary_metric(
                all_rerank, bootstrap=bootstrap, rng=rng
            ),
            "worthy_recall": binary_metric(all_recall, bootstrap=bootstrap, rng=rng),
        }
        report[split] = split_report
    return report


def evaluate(args: argparse.Namespace) -> dict[str, Any]:
    working_tree_status_before_measurement = git_output(["status", "--short"])
    au.NOSE = args.nose
    au._cache.clear()
    labels = json.loads(LABELSET.read_text(encoding="utf-8"))["families"]
    corpus_rows = json.loads(CORPUS.read_text(encoding="utf-8"))["repositories"]
    corpus = {record["id"]: record for record in corpus_rows}
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in labels:
        by_repo[family["repo"]].append(family)

    repo_ids = sorted(by_repo)
    unknown = sorted(set(repo_ids) - set(corpus))
    if unknown:
        raise SystemExit(f"labelset repositories missing from corpus manifest: {', '.join(unknown)}")
    if args.limit_repos is not None:
        if args.limit_repos <= 0:
            raise SystemExit("--limit-repos must be positive")
        repo_ids = repo_ids[: args.limit_repos]
    missing = [repo_id for repo_id in repo_ids if not (args.repos_root / repo_id).is_dir()]
    if missing:
        raise SystemExit(f"missing labeled repositories: {', '.join(missing)}")
    checkout_heads = {
        repo_id: repository_head(args.repos_root / repo_id) for repo_id in repo_ids
    }
    revision_mismatches = [
        f"{repo_id} expected {corpus[repo_id]['commit']} got {checkout_heads[repo_id]}"
        for repo_id in repo_ids
        if checkout_heads[repo_id] != corpus[repo_id]["commit"]
    ]
    if revision_mismatches:
        raise SystemExit("corpus revision mismatch: " + "; ".join(revision_mismatches))

    mode = ",".join(split_modes(args.mode))
    accumulator: dict[tuple[str, str], dict[str, Any]] = defaultdict(
        lambda: {
            "base": [],
            "rerank": [],
            "recall": [],
            "repositories": 0,
            "labels": 0,
            "worthy_labels": 0,
        }
    )
    repositories: dict[str, dict[str, Any]] = {}

    for repo_id in repo_ids:
        repo_labels = by_repo[repo_id]
        metadata = corpus[repo_id]
        language = metadata["primary_language"]
        split = metadata["split"]
        families = query_repo(
            args.repos_root / repo_id,
            nose=args.nose,
            mode=mode or None,
            cache_dir=args.cache_dir,
            top=args.top,
            timeout=args.timeout,
        )
        ordered = (
            sorted(families, key=lambda family: -family["value"])
            if args.rank == "value"
            else list(families)
        )
        top = ordered[:40]
        rerank_scores = {
            family["id"]: family["value"] * refactorability(family)
            for family in top
        }
        reranked = sorted(top, key=lambda family: -rerank_scores[family["id"]]) + ordered[40:]
        base_flags = best_label_flags(ordered, repo_labels)
        rerank_flags = best_label_flags(reranked, repo_labels)
        recall_flags = worthy_recall_flags(ordered, repo_labels)

        row = accumulator[(language, split)]
        row["base"].extend(base_flags)
        row["rerank"].extend(rerank_flags)
        row["recall"].extend(recall_flags)
        row["repositories"] += 1
        row["labels"] += len(repo_labels)
        row["worthy_labels"] += sum(1 for label in repo_labels if label["worthy"])
        repositories[repo_id] = {
            "commit": metadata["commit"],
            "language": language,
            "split": split,
            "labels": len(repo_labels),
            "worthy_labels": len(recall_flags),
            "reported_families": len(families),
            "top_10_reported": min(10, len(ordered)),
            "unmatched_top_10": min(10, len(ordered)) - len(base_flags),
            "precision_at_10": {"hits": sum(base_flags), "n": len(base_flags)},
            "antiunification_rerank_precision_at_10": {
                "hits": sum(rerank_flags),
                "n": len(rerank_flags),
            },
            "worthy_recall": {"hits": sum(recall_flags), "n": len(recall_flags)},
        }

    metrics = build_metrics(accumulator, bootstrap=args.bootstrap)
    nose = args.nose.resolve()
    return {
        "schema": "nose.product_quality_evaluation.v1",
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": {
            "command": shlex.join(["python3", *sys.argv]),
            "git_sha": git_output(["rev-parse", "HEAD"]),
            "working_tree_status_before_measurement": working_tree_status_before_measurement,
            "nose_binary": nose.as_posix(),
            "nose_binary_sha256": sha256_file(nose),
            "nose_version": nose_version(nose),
            "labelset": rel(LABELSET.as_posix()),
            "labelset_sha256": sha256_file(LABELSET),
            "corpus_manifest": rel(CORPUS.as_posix()),
            "corpus_manifest_sha256": sha256_file(CORPUS),
            "corpus_commit_digest": corpus_digest(corpus, repo_ids),
            "prune_manifest": rel(PRUNE_MANIFEST.as_posix()),
            "prune_manifest_sha256": sha256_file(PRUNE_MANIFEST),
        },
        "configuration": {
            "bootstrap_resamples": args.bootstrap,
            "bootstrap_seed": RNG_SEED,
            "cache_dir": args.cache_dir.as_posix() if args.cache_dir else None,
            "limit_repos": args.limit_repos,
            "mode": mode or "CLI default",
            "precision_denominator": "reported top-10 families matching at least one v5 label",
            "rank": args.rank,
            "repos_root": rel(args.repos_root.resolve().as_posix()),
            "timeout_seconds_per_repo": args.timeout,
            "top": args.top,
        },
        "repository_count": len(repo_ids),
        "metrics": metrics,
        "repositories": repositories,
    }


def format_metric(metric: dict[str, Any]) -> str:
    if not metric["n"]:
        return "    -        "
    low, high = metric["ci95"]
    return f"{metric['pct']:>3.0f}% [{low:>3.0f}-{high:>3.0f}] n={metric['n']}"


def print_report(result: dict[str, Any]) -> None:
    rank = result["configuration"]["rank"]
    for split in ("dev", "heldout"):
        print(f"\n=== {split} ===")
        print(
            f"{'lang':<11}{'worthy':>11}  {f'P@10 {rank}':<23}"
            f"{'P@10 re-rank':<23}{'worthy recall':<23}"
        )
        rows = result["metrics"][split]
        for language in sorted(key for key in rows if key != "OVERALL"):
            row = rows[language]
            print(
                f"{language:<11}{row['worthy_labels']}/{row['labels']:<7} "
                f"{format_metric(row['precision_at_10']):<23}"
                f"{format_metric(row['antiunification_rerank_precision_at_10']):<23}"
                f"{format_metric(row['worthy_recall']):<23}"
            )
        overall = rows["OVERALL"]
        print(
            f"{'OVERALL':<11}{overall['worthy_labels']}/{overall['labels']:<7} "
            f"{format_metric(overall['precision_at_10']):<23}"
            f"{format_metric(overall['antiunification_rerank_precision_at_10']):<23}"
            f"{format_metric(overall['worthy_recall']):<23}"
        )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        action="append",
        help=(
            "nose query mode list. Omit for the CLI default; repeat or pass a "
            "comma-list, e.g. --mode syntax,semantic,near"
        ),
    )
    parser.add_argument(
        "--repos-root",
        type=Path,
        default=ROOT / "bench" / "repos",
        help="checkout root containing one directory per corpus repo",
    )
    parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    parser.add_argument("--cache-dir", type=Path, help="forwarded to nose query --cache-dir")
    parser.add_argument("--top", type=int, default=1000000, help="forwarded to query top=N")
    parser.add_argument("--timeout", type=int, default=300, help="per-repo query timeout in seconds")
    parser.add_argument("--bootstrap", type=int, default=2000, help="bootstrap resamples per CI")
    parser.add_argument("--limit-repos", type=int, help="evaluate the first N labeled repos")
    parser.add_argument("--json-out", type=Path, help="write a durable machine-readable report")
    parser.add_argument(
        "--rank",
        choices=("value", "extractability"),
        default="value",
        help=(
            "base P@10 order. 'value' preserves the historical report; "
            "'extractability' uses nose's native JSON order."
        ),
    )
    args = parser.parse_args(argv)
    if args.bootstrap <= 0:
        parser.error("--bootstrap must be positive")
    if args.top < 0:
        parser.error("--top must be non-negative")
    return args


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    result = evaluate(args)
    print_report(result)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"\nwrote {args.json_out}")


if __name__ == "__main__":
    main()
