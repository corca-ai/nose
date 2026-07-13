#!/usr/bin/env python3
"""Validate the checked #839 default-head baseline contract."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import defaultdict
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
ARTIFACT = (
    ROOT
    / "bench"
    / "labels"
    / "product_quality_evaluation_v0_19_0_default_head_2026_07_13.v3.json"
)
CHECKSUM = ARTIFACT.with_suffix(ARTIFACT.suffix + ".sha256")
EXPECTED_ARTIFACT_SHA256 = (
    "4fa11e653d8d15541f4dda6585950e2e5efb734827c321321995034e3931a29e"
)
EXPECTED_EVALUATOR_GIT_SHA = "326537acf4d528f50bfbaa2a7cfbc0515a3287a3"
EXPECTED_COMMAND = (
    "python3 bench/labels/eval_by_language.py "
    "--nose target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin/nose "
    "--nose-release-archive target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin.tar.xz "
    "--nose-release-checksum target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin.tar.xz.sha256 "
    "--rank extractability --bootstrap 2000 "
    "--json-out target/issue-839/final-reviewed.v3.json"
)
EXPECTED_DISTRIBUTION = {
    "archive_sha256": "097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981",
    "checksum_sha256": "f860777bc74bfe18b9be76d02cb1b53e4ea0c8db206ecdcfdc4f16a5f8af5274",
    "checksum_declared_archive_sha256": (
        "097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981"
    ),
}
EXPECTED_BINARY_SHA256 = (
    "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
)
EXPECTED_EVALUATION_SOURCES = {
    "bench/labels/antiunify_probe.py",
    "bench/labels/default_head_query_schema.py",
    "bench/labels/eval_by_language.py",
    "bench/labels/labelset.py",
    "bench/labels/query_schema.py",
}
EXPECTED_LABELSET_INPUTS = {
    "bench/labels/refactoring_families.v5.json",
    "bench/labels/refactoring_families.v6.dev.json",
    "bench/labels/refactoring_families.v6.heldout.json",
}
EXPECTED_CONFIGURATION = {
    "bootstrap_resamples": 2000,
    "bootstrap_seed": 1,
    "cache_dir": None,
    "cache_policy": "disabled (baseline-safe)",
    "default_product_parity_check": True,
    "limit_repos": None,
    "mode": "CLI default",
    "precision_denominator": (
        "top-10 default-surface families matching at least one active precision label"
    ),
    "precision_query": (
        "default-filtered all; raw-order parity against the default list; "
        "literal bare dashboard prefix parity"
    ),
    "precision_surface": "default",
    "rank": "extractability",
    "recall_denominator": (
        "worthy labels eligible for unbiased worthy-recall; hits searched across "
        "the explicit all-surface universe"
    ),
    "repos_root": "bench/repos",
    "splits": ["dev", "heldout"],
    "timeout_seconds_per_repo": 300,
    "top": 1000000,
}
EXPECTED_INPUTS = {
    "corpus_manifest": (
        "bench/goldens/corpus.json",
        "87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8",
    ),
    "prune_manifest": (
        "bench/labels/prune_manifest.json",
        "c22f34d3ab4da9b89b5938140bbfdf7664178b3b7b57e5ea3937ba0bb47c2980",
    ),
    "labelset": (
        "bench/labels/refactoring_families.v6.json",
        "6b72927d0e68e05406540016d3fa136029c52a406af0938b5a805d3fa199ac23",
    ),
}
EXPECTED_CORPUS_COMMIT_DIGEST = (
    "366c977c096a91d50095253cce77a3ec8468d3147ecbd819353dc01196281083"
)
EXPECTED_METRICS = {
    "dev": {
        "precision_at_10": (271, 437, 62.0137),
        "label_match_coverage": (437, 658, 66.4134),
        "worthy_recall": (2716, 2849, 95.3317),
    },
    "heldout": {
        "precision_at_10": (222, 375, 59.2),
        "label_match_coverage": (375, 538, 69.7026),
        "worthy_recall": (2005, 2091, 95.8871),
    },
}
EXPECTED_SURFACE_COUNTS = {
    "declaration": 131,
    "default": 53990,
    "divergence": 88,
    "generated": 973,
    "hidden": 14989,
    "shallow": 30795,
}


def fail(message: str) -> None:
    raise SystemExit(f"default-head baseline check failed: {message}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def check_frozen_digest(digest: str, *, label: str) -> None:
    if digest != EXPECTED_ARTIFACT_SHA256:
        fail(
            f"{label} does not match the frozen artifact digest: "
            f"{digest} != {EXPECTED_ARTIFACT_SHA256}"
        )


def check_sidecar() -> None:
    fields = CHECKSUM.read_text(encoding="utf-8").strip().split(maxsplit=1)
    if len(fields) != 2:
        fail(f"malformed sidecar: {CHECKSUM}")
    expected_digest, name = fields
    if name.lstrip("*") != ARTIFACT.name:
        fail(f"sidecar names {name}, expected {ARTIFACT.name}")
    check_frozen_digest(expected_digest, label="sidecar")
    actual_digest = sha256_file(ARTIFACT)
    check_frozen_digest(actual_digest, label="artifact")


def metric_tuple(metric: dict[str, Any]) -> tuple[int, int, float]:
    return metric["hits"], metric["n"], metric["pct"]


def checked_file_records(
    records: object, *, expected_paths: set[str], label: str
) -> dict[str, str]:
    if not isinstance(records, list):
        fail(f"{label} must be a list")
    mapped = {
        row["path"]: row["sha256"]
        for row in records
        if isinstance(row, dict)
        and isinstance(row.get("path"), str)
        and isinstance(row.get("sha256"), str)
    }
    if len(mapped) != len(records) or set(mapped) != expected_paths:
        fail(f"{label} source set changed: {sorted(mapped)}")
    for relative, recorded_digest in mapped.items():
        path = ROOT / relative
        if not path.is_file():
            fail(f"{label} input missing: {relative}")
        actual_digest = sha256_file(path)
        if recorded_digest != actual_digest:
            fail(
                f"{label} input drifted: {relative}: "
                f"{recorded_digest} != {actual_digest}"
            )
    return mapped


def check_configuration(configuration: object) -> None:
    if configuration != EXPECTED_CONFIGURATION:
        fail("baseline configuration changed")


def check_provenance(provenance: dict[str, Any]) -> None:
    exact = {
        "command": EXPECTED_COMMAND,
        "git_sha": EXPECTED_EVALUATOR_GIT_SHA,
        "working_tree_status_before_measurement": "",
        "nose_binary_sha256": EXPECTED_BINARY_SHA256,
        "nose_version": "nose 0.19.0",
        "labelset_version": "v6",
        "corpus_commit_digest": EXPECTED_CORPUS_COMMIT_DIGEST,
    }
    for key, expected in exact.items():
        if provenance.get(key) != expected:
            fail(f"provenance.{key} expected {expected!r}, got {provenance.get(key)!r}")
    expected_binary_suffix = (
        "/target/issue-839/official-v0.19.0/"
        "nose-cli-aarch64-apple-darwin/nose"
    )
    binary = provenance.get("nose_binary")
    if not isinstance(binary, str) or not binary.endswith(expected_binary_suffix):
        fail(f"unexpected baseline binary path: {binary!r}")

    distribution = provenance.get("nose_release_distribution") or {}
    for key, expected in EXPECTED_DISTRIBUTION.items():
        if distribution.get(key) != expected:
            fail(f"release distribution {key} changed")

    checked_file_records(
        provenance.get("evaluation_sources"),
        expected_paths=EXPECTED_EVALUATION_SOURCES,
        label="evaluation source",
    )
    checked_file_records(
        provenance.get("labelset_inputs"),
        expected_paths=EXPECTED_LABELSET_INPUTS,
        label="labelset",
    )
    for field, (expected_path, expected_digest) in EXPECTED_INPUTS.items():
        if provenance.get(field) != expected_path:
            fail(f"provenance.{field} changed")
        digest_field = f"{field}_sha256"
        if provenance.get(digest_field) != expected_digest:
            fail(f"provenance.{digest_field} changed")
        if sha256_file(ROOT / expected_path) != expected_digest:
            fail(f"checked input drifted: {expected_path}")


def aggregate_repository_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    metrics = (
        "label_match_coverage",
        "precision_at_10",
        "antiunification_rerank_precision_at_10",
        "worthy_recall",
    )
    return {
        "repositories": len(rows),
        "labels": sum(row["labels"] for row in rows),
        "precision_labels": sum(row["precision_labels"] for row in rows),
        "worthy_labels": sum(row["worthy_labels"] for row in rows),
        **{
            metric: {
                "hits": sum(row[metric]["hits"] for row in rows),
                "n": sum(row[metric]["n"] for row in rows),
            }
            for metric in metrics
        },
    }


def check_metric_group(
    actual: dict[str, Any], expected: dict[str, Any], *, label: str
) -> None:
    for field in ("repositories", "labels", "precision_labels", "worthy_labels"):
        if actual.get(field) != expected[field]:
            fail(f"{label}.{field} does not match repository rows")
    for metric in (
        "label_match_coverage",
        "precision_at_10",
        "antiunification_rerank_precision_at_10",
        "worthy_recall",
    ):
        for field in ("hits", "n"):
            if actual.get(metric, {}).get(field) != expected[metric][field]:
                fail(f"{label}.{metric}.{field} does not match repository rows")


def check_repository_rows(report: dict[str, Any]) -> None:
    corpus_payload = json.loads(
        (ROOT / "bench/goldens/corpus.json").read_text(encoding="utf-8")
    )
    corpus = {row["id"]: row for row in corpus_payload["repositories"]}
    repositories = report["repositories"]
    if set(repositories) != set(corpus):
        fail("artifact repository IDs differ from the pinned corpus")

    surface_counts: dict[str, int] = defaultdict(int)
    for repo_id, repository in repositories.items():
        pinned = corpus[repo_id]
        for field, expected in (
            ("commit", pinned["commit"]),
            ("language", pinned["primary_language"]),
            ("split", pinned["split"]),
            ("precision_surface", "default"),
            ("default_list_parity", "checked"),
            ("bare_dashboard_prefix", "checked"),
        ):
            if repository.get(field) != expected:
                fail(f"{repo_id}.{field} expected {expected!r}")
        default_count = repository["precision_surface_reported_families"]
        shown = min(5, default_count)
        if repository.get("bare_dashboard_reported_families") != shown:
            fail(f"bare-dashboard rows changed for {repo_id}")
        if repository.get("bare_dashboard_summary_shown") != shown:
            fail(f"bare-dashboard summary.shown changed for {repo_id}")
        if repository.get("bare_dashboard_summary_families") != default_count:
            fail(f"bare-dashboard summary.families changed for {repo_id}")
        if repository["full_universe_reported_families"] != sum(
            repository["full_universe_surface_counts"].values()
        ):
            fail(f"full-universe surface counts do not sum for {repo_id}")
        if repository["reported_families"] != repository["full_universe_reported_families"]:
            fail(f"reported family aliases disagree for {repo_id}")
        if repository["top_10_reported"] != min(10, default_count):
            fail(f"top-10 denominator changed for {repo_id}")
        coverage = repository["label_match_coverage"]
        if coverage["n"] != repository["top_10_reported"]:
            fail(f"coverage denominator changed for {repo_id}")
        if repository["precision_at_10"]["n"] != coverage["hits"]:
            fail(f"conditional precision denominator changed for {repo_id}")
        if repository["unmatched_top_10"] != coverage["n"] - coverage["hits"]:
            fail(f"unmatched top-10 count changed for {repo_id}")
        if repository["worthy_recall"]["n"] != repository["worthy_labels"]:
            fail(f"worthy-recall denominator changed for {repo_id}")
        for surface, count in repository["full_universe_surface_counts"].items():
            surface_counts[surface] += count
    if dict(sorted(surface_counts.items())) != EXPECTED_SURFACE_COUNTS:
        fail(f"surface counts changed: {dict(sorted(surface_counts.items()))}")

    for split in EXPECTED_CONFIGURATION["splits"]:
        split_rows = [row for row in repositories.values() if row["split"] == split]
        check_metric_group(
            report["metrics"][split]["OVERALL"],
            aggregate_repository_rows(split_rows),
            label=f"metrics.{split}.OVERALL",
        )
        languages = {row["language"] for row in split_rows}
        if set(report["metrics"][split]) != languages | {"OVERALL"}:
            fail(f"metrics.{split} language rows changed")
        for language in languages:
            language_rows = [row for row in split_rows if row["language"] == language]
            check_metric_group(
                report["metrics"][split][language],
                aggregate_repository_rows(language_rows),
                label=f"metrics.{split}.{language}",
            )


def check_report(report: dict[str, Any]) -> None:
    if report.get("schema") != "nose.product_quality_evaluation.v3":
        fail(f"unexpected schema: {report.get('schema')!r}")
    if report.get("query_schema_version") != 7:
        fail(f"unexpected query schema: {report.get('query_schema_version')!r}")
    if "comparison" in report:
        fail("baseline report must not include a comparison binary")
    if (
        report.get("repository_count") != 120
        or len(report.get("repositories", {})) != 120
    ):
        fail("expected exactly 120 repositories")
    check_configuration(report.get("configuration"))
    check_provenance(report["provenance"])
    check_repository_rows(report)

    for split, expected_metrics in EXPECTED_METRICS.items():
        overall = report["metrics"][split]["OVERALL"]
        for metric_name, expected in expected_metrics.items():
            actual = metric_tuple(overall[metric_name])
            if actual != expected:
                fail(f"{split}.{metric_name} expected {expected}, got {actual}")


def expect_self_test_failure(action: Any, expected_message: str) -> None:
    try:
        action()
    except SystemExit as error:
        if expected_message not in str(error):
            raise AssertionError(f"unexpected self-test failure: {error}") from error
    else:
        raise AssertionError(f"expected checker failure containing {expected_message!r}")


def run_self_test() -> None:
    expect_self_test_failure(
        lambda: check_frozen_digest("0" * 64, label="mutated sidecar"),
        "frozen artifact digest",
    )
    mutated_configuration = dict(EXPECTED_CONFIGURATION)
    mutated_configuration["bootstrap_resamples"] = 1
    expect_self_test_failure(
        lambda: check_configuration(mutated_configuration),
        "configuration changed",
    )
    report = json.loads(ARTIFACT.read_text(encoding="utf-8"))
    report["provenance"]["git_sha"] = "0" * 40
    expect_self_test_failure(
        lambda: check_report(report),
        "provenance.git_sha",
    )
    report = json.loads(ARTIFACT.read_text(encoding="utf-8"))
    first_repo = next(iter(report["repositories"]))
    report["repositories"][first_repo]["commit"] = "0" * 40
    expect_self_test_failure(
        lambda: check_report(report),
        f"{first_repo}.commit",
    )
    print("default-head baseline checker self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if not ARTIFACT.is_file() or not CHECKSUM.is_file():
        fail("artifact or SHA-256 sidecar is missing")
    check_sidecar()
    report = json.loads(ARTIFACT.read_text(encoding="utf-8"))
    check_report(report)

    print("default-head baseline artifact OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
