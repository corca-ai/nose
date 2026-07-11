"""Shared deterministic summary for query-regression measurement rows."""

from __future__ import annotations

import json
import statistics
from typing import Any


def summarize_runs(runs: list[dict[str, Any]], repos: list[str]) -> dict[str, Any]:
    """Rebuild the checked report summary from the raw alternating runs."""

    by_repo: dict[str, dict[str, dict[str, Any]]] = {}
    for repo in repos:
        by_repo[repo] = {}
        for label in ("baseline", "current"):
            rows = [row for row in runs if row["repo"] == repo and row["label"] == label]
            stage_names = sorted({name for row in rows for name in row["stages_ms"]})
            by_repo[repo][label] = {
                "bytes": sorted({row["bytes"] for row in rows}),
                "families": sorted({row["families"] for row in rows}),
                "hashes": sorted({row["sha256"] for row in rows}),
                "median_ms": statistics.median(row["elapsed_ms"] for row in rows),
                "schema_versions": sorted({row["schema_version"] for row in rows}),
                "stages_median_ms": {
                    name: statistics.median(
                        row["stages_ms"].get(name, 0.0) for row in rows
                    )
                    for name in stage_names
                },
                "surface_counts": [
                    json.loads(value)
                    for value in sorted(
                        {
                            json.dumps(row["surface_counts"], sort_keys=True)
                            for row in rows
                        }
                    )
                ],
            }

    aggregate_baseline = sum(by_repo[repo]["baseline"]["median_ms"] for repo in repos)
    aggregate_current = sum(by_repo[repo]["current"]["median_ms"] for repo in repos)
    delta_pct = (
        ((aggregate_current - aggregate_baseline) / aggregate_baseline) * 100.0
        if aggregate_baseline
        else 0.0
    )
    return {
        "aggregate_baseline_median_ms": aggregate_baseline,
        "aggregate_current_median_ms": aggregate_current,
        "aggregate_delta_ms": aggregate_current - aggregate_baseline,
        "aggregate_delta_pct": delta_pct,
        "by_repo": by_repo,
        "hashes_identical_by_repo": {
            repo: by_repo[repo]["baseline"]["hashes"]
            == by_repo[repo]["current"]["hashes"]
            for repo in repos
        },
    }
