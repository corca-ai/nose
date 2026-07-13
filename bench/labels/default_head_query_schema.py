#!/usr/bin/env python3
"""Strict dashboard adapter and live default-product parity self-test."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
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


def dashboard_families(
    stdout: str, *, source: str = "nose query dashboard"
) -> list[dict[str, Any]]:
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
    return families


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
        dashboard = dashboard_families(run(), source="live bare-dashboard self-test")
        derived_ids = [
            family["id"]
            for family in all_families
            if family_surface(family, source="live all-list family") == "default"
        ]
        default_ids = [family["id"] for family in default_families]
        dashboard_ids = [family["id"] for family in dashboard]
        assert derived_ids == default_ids, (
            "default-list IDs/order must match default-filtered all list",
            derived_ids,
            default_ids,
        )
        assert dashboard_ids == default_ids[: len(dashboard_ids)], (
            "bare dashboard must be a prefix of the default list",
            dashboard_ids,
            default_ids,
        )


def run_self_test(nose: Path | None = None) -> None:
    family = example_family()
    dashboard = {
        "schema_version": QUERY_SCHEMA_VERSION,
        "tool": "nose",
        "view": "dashboard",
        "families": [family],
        "top_candidates": [family],
    }
    assert dashboard_families(json.dumps(dashboard))[0]["id"] == family["id"]
    dashboard["top_candidates"] = [{**family, "id": "different"}]
    try:
        dashboard_families(json.dumps(dashboard), source="self-test dashboard")
    except QuerySchemaError as error:
        assert "compatibility alias" in str(error)
    else:
        raise AssertionError("mismatched dashboard alias must fail")
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
