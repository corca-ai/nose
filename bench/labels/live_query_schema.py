#!/usr/bin/env python3
"""Validate the current query JSON contract without rewriting frozen evidence.

Historical quality artifacts bind the v7 adapter by content hash. This module
layers the live v9 envelope and generated provenance on that frozen structural
validator so old measurements remain reproducible as the product advances.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
import tempfile
from typing import Any

import query_schema as frozen


QUERY_SCHEMA_VERSION = 9
GENERATED_PROVENANCE_BASES = ("all-members", "compiled-css-pipeline")
GENERATED_PROVENANCE_SOURCES = ("caller-path", "nose-inferred")
QuerySchemaError = frozen.QuerySchemaError


@dataclass(frozen=True)
class DashboardQuery:
    families: list[dict[str, Any]]
    reported_families: int
    shown: int


def fail(source: str, path: str, message: str) -> QuerySchemaError:
    return QuerySchemaError(f"{source}: {path}: {message}")


def is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def validate_generated_provenance(
    family: dict[str, Any], *, source: str, path: str
) -> None:
    if family["surface"] != "generated":
        if "generated_provenance" in family:
            raise fail(
                source,
                f"{path}.generated_provenance",
                "expected only on a generated family",
            )
        return

    provenance = family.get("generated_provenance")
    provenance_path = f"{path}.generated_provenance"
    if not isinstance(provenance, dict):
        raise fail(source, provenance_path, "expected an object for a generated family")
    if set(provenance) != {"basis", "sources"}:
        raise fail(source, provenance_path, "expected exactly basis and sources")
    if provenance["basis"] not in GENERATED_PROVENANCE_BASES:
        raise fail(
            source,
            f"{provenance_path}.basis",
            f"expected one of {', '.join(GENERATED_PROVENANCE_BASES)}",
        )
    sources = provenance["sources"]
    if (
        not isinstance(sources, list)
        or not sources
        or any(value not in GENERATED_PROVENANCE_SOURCES for value in sources)
        or sources != sorted(set(sources))
    ):
        raise fail(
            source,
            f"{provenance_path}.sources",
            "expected a non-empty sorted unique array of caller-path/nose-inferred",
        )


def validate_family(family: object, *, source: str, index: int) -> dict[str, Any]:
    frozen._validate_family(family, source=source, index=index)
    assert isinstance(family, dict)
    validate_generated_provenance(family, source=source, path=f"families[{index}]")
    return family


def decode_envelope(stdout: str, *, source: str, view: str) -> dict[str, Any]:
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
    if payload.get("view") != view:
        raise fail(source, "view", f"expected {view!r}, got {payload.get('view')!r}")
    return payload


def validate_families(
    families: object, *, source: str, field: str = "families"
) -> list[dict[str, Any]]:
    if not isinstance(families, list):
        raise fail(source, field, "expected an array")
    return [
        validate_family(family, source=source, index=index)
        for index, family in enumerate(families)
    ]


def query_families(stdout: str, *, source: str = "nose query") -> list[dict[str, Any]]:
    payload = decode_envelope(stdout, source=source, view="list")
    return validate_families(payload.get("families"), source=source)


def family_surface(family: dict[str, Any], *, source: str = "query family") -> str:
    return validate_family(family, source=source, index=0)["surface"]


def dashboard_query(
    stdout: str, *, source: str = "nose query dashboard"
) -> DashboardQuery:
    payload = decode_envelope(stdout, source=source, view="dashboard")
    summary = payload.get("summary")
    if not isinstance(summary, dict):
        raise fail(source, "summary", "expected an object")
    for field in ("families", "shown"):
        value = summary.get(field)
        if not is_int(value) or value < 0:
            raise fail(source, f"summary.{field}", "expected a non-negative integer")
    families = validate_families(payload.get("families"), source=source)
    top_candidates = validate_families(
        payload.get("top_candidates"),
        source=f"{source} top_candidates",
        field="top_candidates",
    )
    if [family["id"] for family in families] != [
        family["id"] for family in top_candidates
    ]:
        raise fail(source, "top_candidates", "expected the alias to match families IDs/order")
    if summary["shown"] != len(families):
        raise fail(source, "summary.shown", "expected the emitted family count")
    if summary["families"] < summary["shown"]:
        raise fail(source, "summary.families", "expected at least summary.shown")
    return DashboardQuery(families, summary["families"], summary["shown"])


def example_family() -> dict[str, Any]:
    return {
        "id": "0123456789abcdef",
        "scope": "prod",
        "surface": "default",
        "value": 42.0,
        "locations": [{"file": "example/a.py", "start": 3, "end": 8}],
    }


def expect_error(payload: object, expected: str, *, dashboard: bool = False) -> None:
    try:
        decoder = dashboard_query if dashboard else query_families
        decoder(json.dumps(payload), source="self-test")
    except QuerySchemaError as error:
        assert expected in str(error), (expected, str(error))
    else:
        raise AssertionError(f"expected QuerySchemaError containing {expected!r}")


def check_live_binary(nose: Path) -> None:
    source = """\
def first(items):
    return sum(item * 2 for item in items if item > 0)
def second(values):
    return sum(value * 2 for value in values if value > 0)
"""
    with tempfile.TemporaryDirectory(prefix="nose-live-query-schema-") as directory:
        (Path(directory) / "duplicate.py").write_text(source, encoding="utf-8")

        def run(*selectors: str) -> str:
            result = subprocess.run(
                [str(nose), "query", directory, *selectors, "--format", "json"],
                check=False,
                capture_output=True,
                text=True,
                timeout=60,
            )
            if result.returncode != 0:
                raise AssertionError(
                    f"live query {selectors} failed: {result.stderr.strip()}"
                )
            return result.stdout

        all_families = query_families(run("all", "top=0"), source="live all list")
        default_families = query_families(run("top=0"), source="live default list")
        dashboard = dashboard_query(run(), source="live bare dashboard")
        assert all_families, "live query fixture must produce a family"
        default_ids = [family["id"] for family in default_families]
        assert default_ids == [
            family["id"]
            for family in all_families
            if family_surface(family, source="live all-list family") == "default"
        ]
        assert dashboard.reported_families == len(default_ids)
        assert [family["id"] for family in dashboard.families] == default_ids[:5]

        generated = query_families(
            run("all", "top=0", "--generated-path", "duplicate.py"),
            source="live generated list",
        )
        assert generated and all(
            family["surface"] == "generated"
            and family["generated_provenance"]
            == {"basis": "all-members", "sources": ["caller-path"]}
            for family in generated
        )


def run_self_test(nose: Path | None = None) -> None:
    family = example_family()
    listing = {
        "schema_version": QUERY_SCHEMA_VERSION,
        "tool": "nose",
        "view": "list",
        "families": [family],
    }
    assert query_families(json.dumps(listing))[0]["id"] == family["id"]
    wrong_version = json.loads(json.dumps(listing))
    wrong_version["schema_version"] = 8
    expect_error(wrong_version, "expected 9")

    generated = json.loads(json.dumps(listing))
    generated["families"][0].update(
        surface="generated",
        generated_provenance={
            "basis": "all-members",
            "sources": ["caller-path", "nose-inferred"],
        },
    )
    query_families(json.dumps(generated), source="self-test generated")
    missing = json.loads(json.dumps(generated))
    del missing["families"][0]["generated_provenance"]
    expect_error(missing, "expected an object for a generated family")
    unsorted = json.loads(json.dumps(generated))
    unsorted["families"][0]["generated_provenance"]["sources"].reverse()
    expect_error(unsorted, "expected a non-empty sorted unique array")
    unexpected = json.loads(json.dumps(listing))
    unexpected["families"][0]["generated_provenance"] = None
    expect_error(unexpected, "expected only on a generated family")

    dashboard = {
        "schema_version": QUERY_SCHEMA_VERSION,
        "tool": "nose",
        "view": "dashboard",
        "summary": {"families": 1, "shown": 1},
        "families": [family],
        "top_candidates": [family],
    }
    assert dashboard_query(json.dumps(dashboard)).shown == 1
    dashboard["top_candidates"] = [{**family, "id": "different"}]
    expect_error(dashboard, "alias", dashboard=True)

    if nose is not None:
        check_live_binary(nose.resolve())
    print("live query schema v9 self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--nose", type=Path, help="also validate a real nose binary")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    run_self_test(args.nose)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
