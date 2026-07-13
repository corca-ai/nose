#!/usr/bin/env python3
"""Strict dashboard adapter and live default-product parity self-test."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from query_schema import (
    QUERY_SCHEMA_VERSION,
    QuerySchemaError,
    decode_query_payload,
    family_surface,
    query_families,
)


def fail(source: str, path: str, message: str) -> QuerySchemaError:
    return QuerySchemaError(f"{source}: {path}: {message}")


@dataclass(frozen=True)
class DashboardQuery:
    families: list[dict[str, Any]]
    reported_families: int
    shown: int


def is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def dashboard_query(
    stdout: str, *, source: str = "nose query dashboard"
) -> DashboardQuery:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise fail(source, "$", f"invalid JSON: {error.msg}") from error
    if not isinstance(payload, dict):
        raise fail(source, "$", "expected the query JSON envelope object")
    if payload.get("schema_version") != QUERY_SCHEMA_VERSION:
        raise fail(
            source,
            "schema_version",
            f"expected {QUERY_SCHEMA_VERSION}, got {payload.get('schema_version')!r}",
        )
    if payload.get("tool") != "nose":
        raise fail(source, "tool", f"expected 'nose', got {payload.get('tool')!r}")
    if payload.get("view") != "dashboard":
        raise fail(
            source, "view", f"expected 'dashboard', got {payload.get('view')!r}"
        )
    summary = payload.get("summary")
    if not isinstance(summary, dict):
        raise fail(source, "summary", "expected an object")
    for field in ("families", "shown"):
        value = summary.get(field)
        if not is_int(value) or value < 0:
            raise fail(source, f"summary.{field}", "expected a non-negative integer")

    def validated_field(field: str) -> list[dict[str, Any]]:
        adapted = dict(payload)
        adapted["view"] = "list"
        adapted["families"] = payload.get(field)
        return decode_query_payload(
            json.dumps(adapted), source=f"{source} {field}"
        )["families"]

    families = validated_field("families")
    top_candidates = validated_field("top_candidates")
    family_ids = [family["id"] for family in families]
    candidate_ids = [family["id"] for family in top_candidates]
    if family_ids != candidate_ids:
        raise fail(
            source,
            "top_candidates",
            "expected the dashboard compatibility alias to match families IDs/order",
        )
    if summary["shown"] != len(families):
        raise fail(
            source,
            "summary.shown",
            f"expected {len(families)} for the emitted families, got {summary['shown']}",
        )
    if summary["families"] < summary["shown"]:
        raise fail(
            source,
            "summary.families",
            f"expected at least summary.shown={summary['shown']}, got {summary['families']}",
        )
    return DashboardQuery(
        families=families,
        reported_families=summary["families"],
        shown=summary["shown"],
    )


def dashboard_families(
    stdout: str, *, source: str = "nose query dashboard"
) -> list[dict[str, Any]]:
    """Compatibility helper for consumers that only need the dashboard rows."""
    return dashboard_query(stdout, source=source).families


def example_family() -> dict[str, Any]:
    return {
        "id": "0123456789abcdef",
        "scope": "prod",
        "surface": "default",
        "value": 42.0,
        "locations": [
            {
                "file": "example/a.py",
                "start": 3,
                "end": 8,
            }
        ],
    }


def check_live_binary(nose: Path) -> None:
    source = """\
def first(items):
    total = 0
    for item in items:
        if item > 0:
            total += item * 2
    return total

def second(values):
    result = 0
    for value in values:
        if value > 0:
            result += value * 2
    return result
"""
    with tempfile.TemporaryDirectory(prefix="nose-default-head-schema-") as directory:
        fixture = Path(directory) / "duplicate.py"
        fixture.write_text(source, encoding="utf-8")

        def run(*selectors: str) -> str:
            result = subprocess.run(
                [str(nose), "query", directory, *selectors, "--format", "json"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=60,
            )
            if result.returncode != 0:
                raise AssertionError(
                    f"live query {selectors} failed: {result.stderr.strip()}"
                )
            return result.stdout

        all_families = query_families(
            run("all", "top=0"), source="live all-list self-test"
        )
        default_families = query_families(
            run("top=0"), source="live default-list self-test"
        )
        dashboard = dashboard_query(run(), source="live bare-dashboard self-test")
        derived_ids = [
            family["id"]
            for family in all_families
            if family_surface(family, source="live all-list family") == "default"
        ]
        default_ids = [family["id"] for family in default_families]
        dashboard_ids = [family["id"] for family in dashboard.families]
        assert derived_ids == default_ids, (
            "default-list IDs/order must match default-filtered all list",
            derived_ids,
            default_ids,
        )
        expected_dashboard_ids = default_ids[: min(5, len(default_ids))]
        assert dashboard.reported_families == len(default_ids), (
            "bare dashboard summary must report the full default-list count",
            dashboard.reported_families,
            len(default_ids),
        )
        assert dashboard.shown == len(expected_dashboard_ids), (
            "bare dashboard summary must report the product top-five count",
            dashboard.shown,
            len(expected_dashboard_ids),
        )
        assert dashboard_ids == expected_dashboard_ids, (
            "bare dashboard must be the complete product top-five prefix",
            dashboard_ids,
            expected_dashboard_ids,
        )


def run_self_test(nose: Path | None = None) -> None:
    family = example_family()
    dashboard = {
        "schema_version": QUERY_SCHEMA_VERSION,
        "tool": "nose",
        "view": "dashboard",
        "summary": {"families": 1, "shown": 1},
        "families": [family],
        "top_candidates": [family],
    }
    decoded = dashboard_query(json.dumps(dashboard))
    assert decoded.families[0]["id"] == family["id"]
    assert decoded.reported_families == 1
    assert decoded.shown == 1
    dashboard["top_candidates"] = [{**family, "id": "different"}]
    try:
        dashboard_families(json.dumps(dashboard), source="self-test dashboard")
    except QuerySchemaError as error:
        assert "compatibility alias" in str(error)
    else:
        raise AssertionError("mismatched dashboard alias must fail")
    dashboard["top_candidates"] = [family]
    dashboard["summary"] = {"families": 1, "shown": 0}
    try:
        dashboard_query(json.dumps(dashboard), source="self-test short dashboard")
    except QuerySchemaError as error:
        assert "summary.shown" in str(error)
    else:
        raise AssertionError("dashboard summary must match emitted families")
    if nose is not None:
        check_live_binary(nose.resolve())
    print("default-head query schema self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--nose", type=Path, help="also test a real nose binary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    run_self_test(args.nose)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
