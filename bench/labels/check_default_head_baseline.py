#!/usr/bin/env python3
"""Validate the checked #839 default-head baseline contract."""

from __future__ import annotations

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


def check_sidecar() -> None:
    fields = CHECKSUM.read_text(encoding="utf-8").strip().split(maxsplit=1)
    if len(fields) != 2:
        fail(f"malformed sidecar: {CHECKSUM}")
    expected_digest, name = fields
    if name.lstrip("*") != ARTIFACT.name:
        fail(f"sidecar names {name}, expected {ARTIFACT.name}")
    actual_digest = sha256_file(ARTIFACT)
    if actual_digest != expected_digest:
        fail(f"artifact SHA-256 expected {expected_digest}, got {actual_digest}")


def metric_tuple(metric: dict[str, Any]) -> tuple[int, int, float]:
    return metric["hits"], metric["n"], metric["pct"]


def main() -> int:
    if not ARTIFACT.is_file() or not CHECKSUM.is_file():
        fail("artifact or SHA-256 sidecar is missing")
    check_sidecar()
    report = json.loads(ARTIFACT.read_text(encoding="utf-8"))
    if report.get("schema") != "nose.product_quality_evaluation.v3":
        fail(f"unexpected schema: {report.get('schema')!r}")
    if (
        report.get("repository_count") != 120
        or len(report.get("repositories", {})) != 120
    ):
        fail("expected exactly 120 repositories")

    configuration = report["configuration"]
    expected_configuration = {
        "cache_dir": None,
        "cache_policy": "disabled (baseline-safe)",
        "default_product_parity_check": True,
        "precision_surface": "default",
        "rank": "extractability",
    }
    for key, expected in expected_configuration.items():
        actual = configuration.get(key)
        if actual != expected:
            fail(f"configuration.{key} expected {expected!r}, got {actual!r}")
    if "bare_default_parity_check" in configuration:
        fail("obsolete bare_default_parity_check field is present")

    provenance = report["provenance"]
    if provenance.get("nose_binary_sha256") != EXPECTED_BINARY_SHA256:
        fail("official v0.19.0 binary digest changed")
    distribution = provenance.get("nose_release_distribution") or {}
    for key, expected in EXPECTED_DISTRIBUTION.items():
        if distribution.get(key) != expected:
            fail(f"release distribution {key} changed")

    sources = {
        row["path"]: row["sha256"] for row in provenance.get("evaluation_sources", [])
    }
    if set(sources) != EXPECTED_EVALUATION_SOURCES:
        fail(f"evaluation source set changed: {sorted(sources)}")
    for relative, expected_digest in sources.items():
        path = ROOT / relative
        if not path.is_file() or sha256_file(path) != expected_digest:
            fail(f"evaluation source drifted: {relative}")
    for repo_id, repository in report["repositories"].items():
        if repository.get("default_list_parity") != "checked":
            fail(f"default-list parity not checked for {repo_id}")
        if repository.get("bare_dashboard_prefix") != "checked":
            fail(f"bare-dashboard prefix not checked for {repo_id}")
        if not isinstance(repository.get("bare_dashboard_reported_families"), int):
            fail(f"bare-dashboard count missing for {repo_id}")

    for split, expected_metrics in EXPECTED_METRICS.items():
        overall = report["metrics"][split]["OVERALL"]
        for metric_name, expected in expected_metrics.items():
            actual = metric_tuple(overall[metric_name])
            if actual != expected:
                fail(f"{split}.{metric_name} expected {expected}, got {actual}")

    surface_counts: dict[str, int] = defaultdict(int)
    for repository in report["repositories"].values():
        for surface, count in repository["full_universe_surface_counts"].items():
            surface_counts[surface] += count
    if dict(sorted(surface_counts.items())) != EXPECTED_SURFACE_COUNTS:
        fail(f"surface counts changed: {dict(sorted(surface_counts.items()))}")

    print("default-head baseline artifact OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
