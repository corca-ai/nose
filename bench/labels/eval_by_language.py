#!/usr/bin/env python3
"""Evaluate product precision and worthy-family recall by language and split.

The active default is the checked v6 composite labelset and the user-facing
default surface in nose's native extractability order. The base metric can also
use historical value order. The report retains the
historical anti-unification re-rank as a comparison, with deterministic bootstrap
confidence intervals.

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

from default_head_query_schema import DashboardQuery, dashboard_query
from labelset import PRECISION_METRIC, RECALL_METRIC, load_labelset, metric_eligible
from query_schema import (
    QUERY_SCHEMA_VERSION,
    family_surface,
    member_locations,
    query_families,
)


sys.setrecursionlimit(100000)
ROOT = Path(__file__).resolve().parents[2]
DEFAULT_NOSE = ROOT / "target" / "release" / "nose"
HISTORICAL_LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v5.json"
DEFAULT_LABELSET = ROOT / "bench" / "labels" / "refactoring_families.v6.json"
CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
PRUNE_MANIFEST = ROOT / "bench" / "labels" / "prune_manifest.json"
EVALUATION_SOURCES = (
    ROOT / "bench" / "labels" / "eval_by_language.py",
    ROOT / "bench" / "labels" / "default_head_query_schema.py",
    ROOT / "bench" / "labels" / "query_schema.py",
    ROOT / "bench" / "labels" / "labelset.py",
    ROOT / "bench" / "labels" / "antiunify_probe.py",
)
RNG_SEED = 1
COMPAT_RNG = random.Random(RNG_SEED)

spec = importlib.util.spec_from_file_location("au", ROOT / "bench" / "labels" / "antiunify_probe.py")
au = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(au)


def rel(path: str | Path) -> str:
    path = str(path).replace(str(ROOT) + "/", "")
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
    universe: str = "all",
    mode: str | None = None,
    cache_dir: Path | None = None,
    top: int = 1000000,
    timeout: int = 300,
) -> list[dict[str, Any]] | DashboardQuery:
    if universe not in ("all", "default", "dashboard"):
        raise ValueError(f"unsupported query universe: {universe}")
    command = [str(nose), "query", str(repo)]
    if universe == "all":
        command.append("all")
    if universe != "dashboard":
        command.append(f"top={top}")
    command += ["--format", "json"]
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
            f"{universe} query failed for {repo}: "
            f"exit {result.returncode}: {result.stderr.strip()}"
        )
    source = f"nose query {repo} ({universe})"
    if universe == "dashboard":
        return dashboard_query(result.stdout, source=source)
    return query_families(result.stdout, source=source)


def rank_families(
    families: list[dict[str, Any]], *, rank: str
) -> list[dict[str, Any]]:
    if rank == "value":
        return sorted(families, key=lambda family: -family["value"])
    if rank == "extractability":
        return list(families)
    raise ValueError(f"unsupported rank: {rank}")


def precision_families(
    families: list[dict[str, Any]], *, precision_surface: str
) -> list[dict[str, Any]]:
    if precision_surface == "all":
        return list(families)
    if precision_surface == "default":
        return [
            family
            for family in families
            if family_surface(family, source="precision surface family") == "default"
        ]
    raise ValueError(f"unsupported precision surface: {precision_surface}")


def assert_default_list_parity(
    derived: list[dict[str, Any]],
    default_list: list[dict[str, Any]],
    *,
    repo_id: str,
) -> None:
    non_default = [
        family["id"]
        for family in default_list
        if family_surface(family, source=f"{repo_id} default-list family")
        != "default"
    ]
    if non_default:
        raise SystemExit(
            f"default-list parity failed for {repo_id}: default list returned "
            f"non-default family IDs {', '.join(non_default[:5])}"
        )

    derived_ids = [family["id"] for family in derived]
    default_ids = [family["id"] for family in default_list]
    if derived_ids == default_ids:
        return
    first_difference = next(
        (
            index
            for index, (derived_id, default_id) in enumerate(
                zip(derived_ids, default_ids)
            )
            if derived_id != default_id
        ),
        min(len(derived_ids), len(default_ids)),
    )
    derived_id = (
        derived_ids[first_difference]
        if first_difference < len(derived_ids)
        else "<end>"
    )
    default_id = (
        default_ids[first_difference]
        if first_difference < len(default_ids)
        else "<end>"
    )
    raise SystemExit(
        f"default-list parity failed for {repo_id}: first difference at "
        f"index {first_difference}: derived={derived_id}, default={default_id}; "
        f"counts derived={len(derived_ids)}, default={len(default_ids)}"
    )


def assert_dashboard_prefix(
    default_families: list[dict[str, Any]],
    dashboard: DashboardQuery,
    *,
    repo_id: str,
) -> None:
    non_default = [
        family["id"]
        for family in dashboard.families
        if family_surface(family, source=f"{repo_id} dashboard family") != "default"
    ]
    if non_default:
        raise SystemExit(
            f"bare-dashboard parity failed for {repo_id}: dashboard returned "
            f"non-default family IDs {', '.join(non_default[:5])}"
        )
    default_ids = [family["id"] for family in default_families]
    dashboard_ids = [family["id"] for family in dashboard.families]
    expected = default_ids[: min(5, len(default_ids))]
    if dashboard.reported_families != len(default_ids):
        raise SystemExit(
            f"bare-dashboard parity failed for {repo_id}: summary.families "
            f"reported {dashboard.reported_families}, expected {len(default_ids)}"
        )
    if dashboard.shown != len(expected) or len(dashboard_ids) != len(expected):
        raise SystemExit(
            f"bare-dashboard parity failed for {repo_id}: dashboard showed "
            f"summary={dashboard.shown}, rows={len(dashboard_ids)}, "
            f"expected {len(expected)}"
        )
    if dashboard_ids != expected:
        raise SystemExit(
            f"bare-dashboard parity failed for {repo_id}: dashboard IDs "
            f"{dashboard_ids} are not the default-list prefix {expected}"
        )


def surface_counts(families: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = defaultdict(int)
    for family in families:
        counts[family_surface(family, source="surface count family")] += 1
    return dict(sorted(counts.items()))


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


def ratio_metric(hits: int, count: int) -> dict[str, Any]:
    return {
        "hits": hits,
        "n": count,
        "pct": round((100 * hits / count) if count else 0.0, 4),
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
    hits = worthy_recall_hit_ids(families, labels)
    return [
        1 if label["family_id"] in hits else 0
        for label in labels
        if label["worthy"] and metric_eligible(label, RECALL_METRIC)
    ]


def worthy_recall_hit_ids(
    families: list[dict[str, Any]], labels: list[dict[str, Any]]
) -> set[str]:
    hits = set()
    family_locations = [matched_locations(family) for family in families]
    for label in labels:
        if not label["worthy"] or not metric_eligible(label, RECALL_METRIC):
            continue
        hit = any(
            any(overlaps(site, member) for site in sites for member in label["members"])
            for sites in family_locations
        )
        if hit:
            hits.add(label["family_id"])
    return hits


def label_delta_row(
    label: dict[str, Any], *, repo_id: str, language: str
) -> dict[str, Any]:
    return {
        "candidate_key": f"{repo_id}:{label['family_id']}",
        "repo": repo_id,
        "family_id": label["family_id"],
        "language": language,
        "channel": label["channel"],
        "scope": label["scope"],
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def release_distribution_provenance(
    archive: Path | None, checksum: Path | None
) -> dict[str, str] | None:
    if (archive is None) != (checksum is None):
        raise SystemExit(
            "--nose-release-archive and --nose-release-checksum must be passed together"
        )
    if archive is None or checksum is None:
        return None
    if not archive.is_file():
        raise SystemExit(f"nose release archive is missing: {archive}")
    if not checksum.is_file():
        raise SystemExit(f"nose release checksum is missing: {checksum}")

    lines = [
        line.strip()
        for line in checksum.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    if len(lines) != 1:
        raise SystemExit(
            f"release checksum must contain exactly one non-empty line: {checksum}"
        )
    fields = lines[0].split(maxsplit=1)
    if len(fields) != 2:
        raise SystemExit(f"malformed release checksum: {checksum}")
    declared_digest, declared_name = fields
    declared_name = declared_name.lstrip("*")
    if (
        len(declared_digest) != 64
        or any(character not in "0123456789abcdef" for character in declared_digest)
    ):
        raise SystemExit(f"malformed SHA-256 digest in {checksum}")
    if declared_name != archive.name:
        raise SystemExit(
            f"release checksum names {declared_name}, expected {archive.name}"
        )
    archive_digest = sha256_file(archive)
    if archive_digest != declared_digest:
        raise SystemExit(
            f"release archive checksum mismatch: expected {declared_digest}, "
            f"got {archive_digest}"
        )
    return {
        "archive": rel(archive.resolve().as_posix()),
        "archive_sha256": archive_digest,
        "checksum": rel(checksum.resolve().as_posix()),
        "checksum_sha256": sha256_file(checksum),
        "checksum_declared_archive_sha256": declared_digest,
    }


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
    splits: tuple[str, ...] = ("dev", "heldout"),
) -> dict[str, dict[str, dict[str, Any]]]:
    rng = random.Random(RNG_SEED)
    report: dict[str, dict[str, dict[str, Any]]] = {}
    for split in splits:
        split_report: dict[str, dict[str, Any]] = {}
        rows = sorted(
            ((language, data) for (language, row_split), data in accumulator.items() if row_split == split),
            key=lambda row: row[0],
        )
        for language, data in rows:
            split_report[language] = {
                "repositories": data["repositories"],
                "labels": data["labels"],
                "precision_labels": data["precision_labels"],
                "worthy_labels": data["worthy_labels"],
                "label_match_coverage": ratio_metric(
                    data["matched_top_10"], data["reported_top_10"]
                ),
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
            "precision_labels": sum(data["precision_labels"] for _, data in rows),
            "worthy_labels": sum(data["worthy_labels"] for _, data in rows),
            "label_match_coverage": ratio_metric(
                sum(data["matched_top_10"] for _, data in rows),
                sum(data["reported_top_10"] for _, data in rows),
            ),
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
    if not args.nose.is_file():
        raise SystemExit(f"nose binary is missing: {args.nose}")
    if args.comparison_nose is not None and not args.comparison_nose.is_file():
        raise SystemExit(f"comparison nose binary is missing: {args.comparison_nose}")
    distribution = release_distribution_provenance(
        args.nose_release_archive, args.nose_release_checksum
    )
    au.NOSE = args.nose
    au._cache.clear()
    loaded_labelset = load_labelset(args.labelset)
    labels = loaded_labelset.families
    corpus_rows = json.loads(CORPUS.read_text(encoding="utf-8"))["repositories"]
    corpus = {record["id"]: record for record in corpus_rows}
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in labels:
        if family.get("split") not in args.splits:
            continue
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
            "precision_labels": 0,
            "worthy_labels": 0,
            "reported_top_10": 0,
            "matched_top_10": 0,
        }
    )
    repositories: dict[str, dict[str, Any]] = {}
    comparison_recovered: list[dict[str, Any]] = []
    comparison_regressed: list[dict[str, Any]] = []
    comparison_by_repository: dict[str, dict[str, int]] = {}
    comparison_hits = 0
    current_hits = 0

    for repo_id in repo_ids:
        repo_labels = by_repo[repo_id]
        precision_labels = [
            label for label in repo_labels if metric_eligible(label, PRECISION_METRIC)
        ]
        metadata = corpus[repo_id]
        language = metadata["primary_language"]
        split = metadata["split"]
        all_families = query_repo(
            args.repos_root / repo_id,
            nose=args.nose,
            universe="all",
            mode=mode or None,
            cache_dir=args.cache_dir,
            top=args.top,
            timeout=args.timeout,
        )
        precision_pool = precision_families(
            all_families, precision_surface=args.precision_surface
        )
        default_list_parity_checked = False
        dashboard_prefix_checked = False
        dashboard_reported_families: int | None = None
        dashboard_summary_families: int | None = None
        dashboard_summary_shown: int | None = None
        if (
            args.precision_surface == "default"
            and not args.no_check_bare_default_parity
        ):
            default_list = query_repo(
                args.repos_root / repo_id,
                nose=args.nose,
                universe="default",
                mode=mode or None,
                cache_dir=args.cache_dir,
                top=args.top,
                timeout=args.timeout,
            )
            assert_default_list_parity(
                precision_pool,
                default_list,
                repo_id=repo_id,
            )
            default_list_parity_checked = True
            dashboard = query_repo(
                args.repos_root / repo_id,
                nose=args.nose,
                universe="dashboard",
                mode=mode or None,
                cache_dir=args.cache_dir,
                timeout=args.timeout,
            )
            if not isinstance(dashboard, DashboardQuery):
                raise AssertionError("dashboard query returned list-view rows")
            assert_dashboard_prefix(
                precision_pool,
                dashboard,
                repo_id=repo_id,
            )
            dashboard_prefix_checked = True
            dashboard_reported_families = len(dashboard.families)
            dashboard_summary_families = dashboard.reported_families
            dashboard_summary_shown = dashboard.shown
        ordered = rank_families(precision_pool, rank=args.rank)
        top = ordered[:40]
        rerank_scores = {
            family["id"]: family["value"] * refactorability(family)
            for family in top
        }
        reranked = sorted(top, key=lambda family: -rerank_scores[family["id"]]) + ordered[40:]
        base_flags = best_label_flags(ordered, precision_labels)
        rerank_flags = best_label_flags(reranked, precision_labels)
        current_hit_ids = worthy_recall_hit_ids(all_families, repo_labels)
        recall_flags = [
            1 if label["family_id"] in current_hit_ids else 0
            for label in repo_labels
            if label["worthy"] and metric_eligible(label, RECALL_METRIC)
        ]

        if args.comparison_nose is not None:
            comparison_families = query_repo(
                args.repos_root / repo_id,
                nose=args.comparison_nose,
                universe="all",
                mode=mode or None,
                cache_dir=args.cache_dir,
                top=args.top,
                timeout=args.timeout,
            )
            comparison_hit_ids = worthy_recall_hit_ids(comparison_families, repo_labels)
            recovered_ids = current_hit_ids - comparison_hit_ids
            regressed_ids = comparison_hit_ids - current_hit_ids
            comparison_hits += len(comparison_hit_ids)
            current_hits += len(current_hit_ids)
            comparison_by_repository[repo_id] = {
                "comparison_hits": len(comparison_hit_ids),
                "current_hits": len(current_hit_ids),
                "delta": len(current_hit_ids) - len(comparison_hit_ids),
            }
            for label in repo_labels:
                family_id = label["family_id"]
                if family_id in recovered_ids:
                    comparison_recovered.append(
                        label_delta_row(label, repo_id=repo_id, language=language)
                    )
                if family_id in regressed_ids:
                    comparison_regressed.append(
                        label_delta_row(label, repo_id=repo_id, language=language)
                    )

        row = accumulator[(language, split)]
        row["base"].extend(base_flags)
        row["rerank"].extend(rerank_flags)
        row["recall"].extend(recall_flags)
        row["repositories"] += 1
        row["labels"] += len(repo_labels)
        row["precision_labels"] += len(precision_labels)
        row["worthy_labels"] += sum(
            1
            for label in repo_labels
            if label["worthy"] and metric_eligible(label, RECALL_METRIC)
        )
        row["reported_top_10"] += min(10, len(ordered))
        row["matched_top_10"] += len(base_flags)
        repositories[repo_id] = {
            "commit": metadata["commit"],
            "language": language,
            "split": split,
            "labels": len(repo_labels),
            "precision_labels": len(precision_labels),
            "worthy_labels": len(recall_flags),
            "reported_families": len(all_families),
            "full_universe_reported_families": len(all_families),
            "full_universe_surface_counts": surface_counts(all_families),
            "precision_surface": args.precision_surface,
            "precision_surface_reported_families": len(ordered),
            "default_list_parity": (
                "checked" if default_list_parity_checked else "not-checked"
            ),
            "bare_dashboard_prefix": (
                "checked" if dashboard_prefix_checked else "not-checked"
            ),
            "bare_dashboard_reported_families": dashboard_reported_families,
            "bare_dashboard_summary_families": dashboard_summary_families,
            "bare_dashboard_summary_shown": dashboard_summary_shown,
            "top_10_reported": min(10, len(ordered)),
            "unmatched_top_10": min(10, len(ordered)) - len(base_flags),
            "label_match_coverage": {
                "hits": len(base_flags),
                "n": min(10, len(ordered)),
            },
            "precision_at_10": {"hits": sum(base_flags), "n": len(base_flags)},
            "antiunification_rerank_precision_at_10": {
                "hits": sum(rerank_flags),
                "n": len(rerank_flags),
            },
            "worthy_recall": {"hits": sum(recall_flags), "n": len(recall_flags)},
        }

    metrics = build_metrics(accumulator, bootstrap=args.bootstrap, splits=args.splits)
    nose = args.nose.resolve()
    result = {
        "schema": "nose.product_quality_evaluation.v3",
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": {
            "command": shlex.join(["python3", *sys.argv]),
            "git_sha": git_output(["rev-parse", "HEAD"]),
            "working_tree_status_before_measurement": working_tree_status_before_measurement,
            "nose_binary": nose.as_posix(),
            "nose_binary_sha256": sha256_file(nose),
            "nose_version": nose_version(nose),
            "nose_release_distribution": distribution,
            "evaluation_sources": [
                {"path": rel(path), "sha256": sha256_file(path)}
                for path in EVALUATION_SOURCES
            ],
            "labelset": rel(args.labelset.resolve().as_posix()),
            "labelset_sha256": sha256_file(args.labelset),
            "labelset_version": loaded_labelset.version,
            "labelset_inputs": [
                {"path": rel(row["path"]), "sha256": row["sha256"]}
                for row in loaded_labelset.inputs
            ],
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
            "cache_policy": (
                "disabled (baseline-safe)"
                if args.cache_dir is None
                else "explicit diagnostic opt-in; not baseline-safe"
            ),
            "limit_repos": args.limit_repos,
            "mode": mode or "CLI default",
            "precision_surface": args.precision_surface,
            "precision_query": (
                "default-filtered all; raw-order parity against the default list; "
                "literal bare dashboard prefix parity"
                if args.precision_surface == "default"
                and not args.no_check_bare_default_parity
                else (
                    "default-filtered all (product parity checks disabled)"
                    if args.precision_surface == "default"
                    else "all (historical compatibility mode)"
                )
            ),
            "default_product_parity_check": (
                args.precision_surface == "default"
                and not args.no_check_bare_default_parity
            ),
            "precision_denominator": (
                f"top-10 {args.precision_surface}-surface families matching at least "
                "one active precision label"
            ),
            "recall_denominator": (
                "worthy labels eligible for unbiased worthy-recall; hits searched "
                "across the explicit all-surface universe"
            ),
            "rank": args.rank,
            "repos_root": rel(args.repos_root.resolve().as_posix()),
            "timeout_seconds_per_repo": args.timeout,
            "top": args.top,
            "splits": list(args.splits),
        },
        "repository_count": len(repo_ids),
        "metrics": metrics,
        "repositories": repositories,
    }
    if args.comparison_nose is not None:
        comparison_nose = args.comparison_nose.resolve()
        result["comparison"] = {
            "direction": "current --nose minus --comparison-nose",
            "provenance": {
                "nose_binary": comparison_nose.as_posix(),
                "nose_binary_sha256": sha256_file(comparison_nose),
                "nose_version": nose_version(comparison_nose),
            },
            "worthy_recall": {
                "comparison_hits": comparison_hits,
                "current_hits": current_hits,
                "delta": current_hits - comparison_hits,
                "recovered_count": len(comparison_recovered),
                "regressed_count": len(comparison_regressed),
                "by_repository": comparison_by_repository,
                "recovered": sorted(
                    comparison_recovered, key=lambda row: row["candidate_key"]
                ),
                "regressed": sorted(
                    comparison_regressed, key=lambda row: row["candidate_key"]
                ),
            },
        }
    return result


def format_metric(metric: dict[str, Any]) -> str:
    if not metric["n"]:
        return "    -        "
    low, high = metric["ci95"]
    return f"{metric['pct']:>3.0f}% [{low:>3.0f}-{high:>3.0f}] n={metric['n']}"


def print_report(result: dict[str, Any]) -> None:
    rank = result["configuration"]["rank"]
    precision_surface = result["configuration"]["precision_surface"]
    print(
        f"precision surface: {precision_surface}; "
        "worthy-recall search surface: all"
    )
    for split in result["configuration"]["splits"]:
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
        coverage = overall["label_match_coverage"]
        print(
            f"label-match coverage: {coverage['hits']}/{coverage['n']} "
            f"= {coverage['pct']:.2f}%"
        )


def run_self_test() -> None:
    def family(family_id: str, surface: str, value: float) -> dict[str, Any]:
        return {
            "id": family_id,
            "scope": "prod",
            "surface": surface,
            "value": value,
            "locations": [
                {
                    "file": f"{family_id}.py",
                    "start": 1,
                    "end": 2,
                }
            ],
        }

    low_default = family("low-default", "default", 1.0)
    hidden = family("hidden", "hidden", 3.0)
    high_default = family("high-default", "default", 2.0)
    families = [low_default, hidden, high_default]

    default_families = precision_families(
        families, precision_surface="default"
    )
    assert [row["id"] for row in default_families] == [
        "low-default",
        "high-default",
    ]
    assert precision_families(families, precision_surface="all") == families
    assert [row["id"] for row in rank_families(default_families, rank="value")] == [
        "high-default",
        "low-default",
    ]
    assert rank_families(default_families, rank="extractability") == default_families
    assert_default_list_parity(
        default_families, list(default_families), repo_id="self-test"
    )
    try:
        assert_default_list_parity(
            default_families,
            list(reversed(default_families)),
            repo_id="self-test-mismatch",
        )
    except SystemExit as error:
        assert "first difference at index 0" in str(error)
    else:
        raise AssertionError("default-list raw order mismatch must fail")
    try:
        assert_default_list_parity(
            default_families, [hidden], repo_id="self-test-surface"
        )
    except SystemExit as error:
        assert "non-default family IDs hidden" in str(error)
    else:
        raise AssertionError("non-default list family must fail")
    dashboard = DashboardQuery(
        families=list(default_families), reported_families=2, shown=2
    )
    assert_dashboard_prefix(default_families, dashboard, repo_id="self-test")
    try:
        assert_dashboard_prefix(
            default_families,
            DashboardQuery(
                families=list(reversed(default_families)),
                reported_families=2,
                shown=2,
            ),
            repo_id="self-test-dashboard",
        )
    except SystemExit as error:
        assert "not the default-list prefix" in str(error)
    else:
        raise AssertionError("non-prefix dashboard order must fail")
    try:
        assert_dashboard_prefix(
            default_families,
            DashboardQuery(families=[], reported_families=2, shown=0),
            repo_id="self-test-short-dashboard",
        )
    except SystemExit as error:
        assert "dashboard showed" in str(error)
    else:
        raise AssertionError("short dashboard must fail")
    try:
        assert_dashboard_prefix(
            default_families,
            DashboardQuery(
                families=list(default_families), reported_families=1, shown=2
            ),
            repo_id="self-test-dashboard-summary",
        )
    except SystemExit as error:
        assert "summary.families" in str(error)
    else:
        raise AssertionError("wrong dashboard total must fail")
    assert surface_counts(families) == {"default": 2, "hidden": 1}
    print("product-quality evaluator self-test passed")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--labelset",
        type=Path,
        default=DEFAULT_LABELSET,
        help=(
            "flat historical labelset or checked composite manifest "
            f"(default: {rel(DEFAULT_LABELSET)})"
        ),
    )
    parser.add_argument(
        "--split",
        action="append",
        choices=("dev", "heldout"),
        dest="splits",
        help="evaluate only one split; repeat for both (default: dev and heldout)",
    )
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
    parser.add_argument(
        "--nose-release-archive",
        type=Path,
        help="published release archive used to obtain --nose (recorded and verified)",
    )
    parser.add_argument(
        "--nose-release-checksum",
        type=Path,
        help="published SHA-256 file for --nose-release-archive",
    )
    parser.add_argument(
        "--comparison-nose",
        type=Path,
        help=(
            "optional baseline binary; records exact worthy-label recovery and "
            "regression IDs without changing the primary metrics"
        ),
    )
    parser.add_argument(
        "--cache-dir",
        type=Path,
        help="diagnostic-only query cache; requires --allow-cache",
    )
    parser.add_argument(
        "--allow-cache",
        action="store_true",
        help=(
            "explicitly allow a caller-managed diagnostic cache; cached reports "
            "are not eligible as release baselines"
        ),
    )
    parser.add_argument("--top", type=int, default=1000000, help="forwarded to query top=N")
    parser.add_argument("--timeout", type=int, default=300, help="per-repo query timeout in seconds")
    parser.add_argument("--bootstrap", type=int, default=2000, help="bootstrap resamples per CI")
    parser.add_argument("--limit-repos", type=int, help="evaluate the first N labeled repos")
    parser.add_argument("--json-out", type=Path, help="write a durable machine-readable report")
    parser.add_argument(
        "--precision-surface",
        choices=("default", "all"),
        default="default",
        help=(
            "surface used for precision@10 (default: default); 'all' reproduces "
            "the historical full-universe metric"
        ),
    )
    parser.add_argument(
        "--no-check-bare-default-parity",
        action="store_true",
        help=(
            "skip default-list raw-order and literal bare-dashboard prefix parity "
            "checks against the default-filtered all query"
        ),
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run deterministic evaluator unit checks without repositories or a nose binary",
    )
    parser.add_argument(
        "--rank",
        choices=("value", "extractability"),
        default="extractability",
        help=(
            "base P@10 order (default: extractability, nose's native JSON order); "
            "'value' is the historical volume ranking"
        ),
    )
    args = parser.parse_args(argv)
    args.splits = tuple(dict.fromkeys(args.splits or ("dev", "heldout")))
    if args.bootstrap <= 0:
        parser.error("--bootstrap must be positive")
    if args.top < 0:
        parser.error("--top must be non-negative")
    if args.cache_dir is not None and not args.allow_cache:
        parser.error(
            "--cache-dir may contain stale analysis; omit it for a baseline or "
            "pass --allow-cache for a diagnostic-only run"
        )
    if args.cache_dir is not None and args.comparison_nose is not None:
        parser.error("--comparison-nose cannot share one --cache-dir across binaries")
    if args.allow_cache and args.cache_dir is None:
        parser.error("--allow-cache requires --cache-dir")
    if (args.nose_release_archive is None) != (args.nose_release_checksum is None):
        parser.error(
            "--nose-release-archive and --nose-release-checksum must be passed together"
        )
    return args


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    if args.self_test:
        run_self_test()
        return
    result = evaluate(args)
    print_report(result)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"\nwrote {args.json_out}")


if __name__ == "__main__":
    main()
