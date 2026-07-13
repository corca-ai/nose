#!/usr/bin/env python3
"""Strict adapter for the product evaluator's nose query JSON boundary.

The labelset stores member coordinates as ``start_line``/``end_line``. Query
JSON schema v7 deliberately uses ``start``/``end``. Product-evaluation scripts
must pass through this module so a wire-schema change fails at one explicit
boundary instead of becoming a silent omission or a late ``KeyError``.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any


QUERY_SCHEMA_VERSION = 7
QUERY_SURFACES = (
    "default",
    "divergence",
    "hidden",
    "shallow",
    "generated",
    "declaration",
    "debug",
)
QUERY_SCOPES = ("prod", "test", "mixed")


class QuerySchemaError(ValueError):
    """The query response does not satisfy the evaluator's supported contract."""


def _fail(source: str, path: str, message: str) -> QuerySchemaError:
    return QuerySchemaError(f"{source}: {path}: {message}")


def _is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _validate_location(location: object, *, source: str, path: str) -> None:
    if not isinstance(location, dict):
        raise _fail(source, path, "expected an object")
    legacy = sorted({"start_line", "end_line"} & location.keys())
    if legacy:
        raise _fail(
            source,
            path,
            f"legacy location key(s) {', '.join(legacy)} are not valid in query JSON schema v7",
        )
    file = location.get("file")
    if not isinstance(file, str) or not file:
        raise _fail(source, f"{path}.file", "expected a non-empty string")
    for key in ("start", "end"):
        if not _is_int(location.get(key)):
            raise _fail(source, f"{path}.{key}", "expected an integer line number")
    start = location["start"]
    end = location["end"]
    if start < 1 or end < start:
        raise _fail(source, path, f"invalid inclusive line range {start}-{end}")


def _validate_family(
    family: object, *, source: str, index: int, field: str = "families"
) -> None:
    path = f"{field}[{index}]"
    if not isinstance(family, dict):
        raise _fail(source, path, "expected an object")
    family_id = family.get("id")
    if not isinstance(family_id, str) or not family_id:
        raise _fail(source, f"{path}.id", "expected a non-empty string")
    locations = family.get("locations")
    if not isinstance(locations, list):
        raise _fail(source, f"{path}.locations", "expected an array")
    if not locations:
        raise _fail(source, f"{path}.locations", "expected at least one member location")
    for location_index, location in enumerate(locations):
        _validate_location(
            location,
            source=source,
            path=f"{path}.locations[{location_index}]",
        )
    surface = family.get("surface")
    if surface not in QUERY_SURFACES:
        raise _fail(
            source,
            f"{path}.surface",
            f"expected one of {', '.join(QUERY_SURFACES)}, got {surface!r}",
        )
    scope = family.get("scope")
    if scope not in QUERY_SCOPES:
        raise _fail(
            source,
            f"{path}.scope",
            f"expected one of {', '.join(QUERY_SCOPES)}, got {scope!r}",
        )
    value = family.get("value")
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise _fail(source, f"{path}.value", "expected a numeric ranking value")


def _decode_query_payload(stdout: str, *, source: str) -> dict[str, Any]:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise _fail(source, "$", f"invalid JSON: {error.msg}") from error
    if not isinstance(payload, dict):
        raise _fail(source, "$", "expected the query JSON envelope object, not a top-level array")
    if payload.get("schema_version") != QUERY_SCHEMA_VERSION:
        raise _fail(
            source,
            "schema_version",
            f"expected {QUERY_SCHEMA_VERSION}, got {payload.get('schema_version')!r}",
        )
    if payload.get("tool") != "nose":
        raise _fail(source, "tool", f"expected 'nose', got {payload.get('tool')!r}")
    return payload


def _validate_families_field(
    payload: dict[str, Any], *, field: str, source: str
) -> list[dict[str, Any]]:
    families = payload.get(field)
    if not isinstance(families, list):
        raise _fail(source, field, "expected an array")
    for index, family in enumerate(families):
        _validate_family(family, source=source, index=index, field=field)
    return families


def decode_query_payload(stdout: str, *, source: str = "nose query") -> dict[str, Any]:
    """Decode and validate the schema-v7 list envelope used by evaluators."""

    payload = _decode_query_payload(stdout, source=source)
    if payload.get("view") != "list":
        raise _fail(source, "view", f"expected 'list', got {payload.get('view')!r}")
    _validate_families_field(payload, field="families", source=source)
    return payload


def query_families(stdout: str, *, source: str = "nose query") -> list[dict[str, Any]]:
    return decode_query_payload(stdout, source=source)["families"]


def dashboard_families(
    stdout: str, *, source: str = "nose query dashboard"
) -> list[dict[str, Any]]:
    payload = _decode_query_payload(stdout, source=source)
    if payload.get("view") != "dashboard":
        raise _fail(
            source, "view", f"expected 'dashboard', got {payload.get('view')!r}"
        )
    families = _validate_families_field(
        payload, field="families", source=source
    )
    top_candidates = _validate_families_field(
        payload, field="top_candidates", source=source
    )
    family_ids = [family["id"] for family in families]
    candidate_ids = [family["id"] for family in top_candidates]
    if family_ids != candidate_ids:
        raise _fail(
            source,
            "top_candidates",
            "expected the dashboard compatibility alias to match families IDs/order",
        )
    return families


def member_locations(family: dict[str, Any], *, source: str = "query family") -> list[dict[str, Any]]:
    """Adapt current wire coordinates to the labelset's internal coordinate names."""

    _validate_family(family, source=source, index=0)
    return [
        {
            "file": location["file"],
            "start_line": location["start"],
            "end_line": location["end"],
        }
        for location in family["locations"]
    ]


def family_surface(family: dict[str, Any], *, source: str = "query family") -> str:
    _validate_family(family, source=source, index=0)
    return family["surface"]


def _current_query_example() -> dict[str, Any]:
    return {
        "schema_version": 7,
        "tool": "nose",
        "view": "list",
        "path": "example",
        "families": [
            {
                "id": "0123456789abcdef",
                "scope": "prod",
                "surface": "default",
                "value": 42.0,
                "locations": [
                    {
                        "id": "1111111111111111",
                        "file": "example/a.py",
                        "start": 3,
                        "end": 8,
                        "name": "first",
                        "lang": "python",
                    },
                    {
                        "id": "2222222222222222",
                        "file": "example/b.py",
                        "start": 10,
                        "end": 15,
                        "name": "second",
                        "lang": "python",
                    },
                ],
            }
        ],
    }


def _expect_error(payload: object, expected: str) -> None:
    try:
        decode_query_payload(json.dumps(payload), source="self-test")
    except QuerySchemaError as error:
        assert expected in str(error), (expected, str(error))
    else:
        raise AssertionError(f"expected QuerySchemaError containing {expected!r}")


def _check_live_binary(nose: Path) -> None:
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
    with tempfile.TemporaryDirectory(prefix="nose-query-schema-") as directory:
        fixture = Path(directory) / "duplicate.py"
        fixture.write_text(source, encoding="utf-8")

        def run_list_query(*selectors: str) -> list[dict[str, Any]]:
            result = subprocess.run(
                [
                    str(nose),
                    "query",
                    directory,
                    *selectors,
                    "top=0",
                    "--format",
                    "json",
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=60,
            )
            if result.returncode != 0:
                selector = " ".join(selectors) or "<bare default>"
                raise AssertionError(
                    f"live query {selector} failed: {result.stderr.strip()}"
                )
            return query_families(
                result.stdout, source=f"live nose query self-test {selectors}"
            )

        dashboard_result = subprocess.run(
            [str(nose), "query", directory, "--format", "json"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=60,
        )
        if dashboard_result.returncode != 0:
            raise AssertionError(
                f"live bare dashboard query failed: {dashboard_result.stderr.strip()}"
            )

        all_families = run_list_query("all")
        default_families = run_list_query()
        dashboard = dashboard_families(
            dashboard_result.stdout, source="live bare dashboard query self-test"
        )
        assert all_families, "live query fixture must produce at least one family"
        locations = member_locations(
            all_families[0], source="live nose query self-test"
        )
        assert all(
            {"file", "start_line", "end_line"} == set(location)
            for location in locations
        )
        derived_default_ids = [
            family["id"]
            for family in all_families
            if family_surface(family, source="live all query family") == "default"
        ]
        default_ids = [family["id"] for family in default_families]
        assert all(
            family_surface(family, source="live default-list family") == "default"
            for family in default_families
        ), "default list must only return default-surface families"
        assert derived_default_ids == default_ids, (
            "default-list IDs/order must match default-filtered all query",
            derived_default_ids,
            default_ids,
        )
        dashboard_ids = [family["id"] for family in dashboard]
        assert dashboard_ids == default_ids[: len(dashboard_ids)], (
            "literal bare dashboard must be a prefix of the default list",
            dashboard_ids,
            default_ids,
        )


def run_self_test(nose: Path | None = None) -> None:
    example = _current_query_example()
    payload = decode_query_payload(json.dumps(example), source="self-test current example")
    assert family_surface(payload["families"][0]) == "default"
    assert member_locations(payload["families"][0])[0] == {
        "file": "example/a.py",
        "start_line": 3,
        "end_line": 8,
    }

    _expect_error(example["families"], "top-level array")
    wrong_version = json.loads(json.dumps(example))
    wrong_version["schema_version"] = 8
    _expect_error(wrong_version, "expected 7")
    old_location = json.loads(json.dumps(example))
    location = old_location["families"][0]["locations"][0]
    location["start_line"] = location.pop("start")
    location["end_line"] = location.pop("end")
    _expect_error(old_location, "legacy location key")
    missing_start = json.loads(json.dumps(example))
    del missing_start["families"][0]["locations"][0]["start"]
    _expect_error(missing_start, "locations[0].start")
    missing_surface = json.loads(json.dumps(example))
    del missing_surface["families"][0]["surface"]
    _expect_error(missing_surface, "families[0].surface")
    missing_scope = json.loads(json.dumps(example))
    del missing_scope["families"][0]["scope"]
    _expect_error(missing_scope, "families[0].scope")

    dashboard = json.loads(json.dumps(example))
    dashboard["view"] = "dashboard"
    dashboard["top_candidates"] = json.loads(json.dumps(dashboard["families"]))
    assert dashboard_families(json.dumps(dashboard))[0]["id"] == "0123456789abcdef"
    mismatched_alias = json.loads(json.dumps(dashboard))
    mismatched_alias["top_candidates"][0]["id"] = "different"
    try:
        dashboard_families(json.dumps(mismatched_alias), source="self-test dashboard")
    except QuerySchemaError as error:
        assert "compatibility alias" in str(error)
    else:
        raise AssertionError("mismatched dashboard alias must fail")

    if nose is not None:
        _check_live_binary(nose.resolve())
    print("product query schema adapter self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--nose", type=Path, help="also validate a real nose query response")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    run_self_test(args.nose)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
