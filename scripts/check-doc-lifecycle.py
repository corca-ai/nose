#!/usr/bin/env python3
"""Validate the checked documentation lifecycle catalog."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
CATALOG = DOCS / "lifecycle.json"

KINDS = {"guide", "reference", "decision", "active-roadmap", "historical-record"}
STATUSES = {"active", "historical", "superseded"}
METADATA_KEYS = {
    "kind",
    "status",
    "owner",
    "last_verified",
    "review_after_days",
    "retention",
    "superseded_by",
}


class CatalogError(ValueError):
    """A lifecycle catalog contract violation."""


def fail(message: str) -> None:
    raise CatalogError(message)


def load_json(path: Path) -> dict[str, Any]:
    def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                fail(f"{path}: duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(path.read_text(), object_pairs_hook=reject_duplicate_keys)
    except (OSError, json.JSONDecodeError) as error:
        fail(f"{path}: cannot read catalog: {error}")
    if not isinstance(value, dict):
        fail(f"{path}: catalog root must be an object")
    return value


def inventory_digest(pages: list[str]) -> str:
    payload = "".join(f"{page}\n" for page in sorted(pages))
    return hashlib.sha256(payload.encode()).hexdigest()


def parse_date(value: Any, context: str) -> dt.date:
    if not isinstance(value, str):
        fail(f"{context}: last_verified must be an ISO date")
    try:
        return dt.date.fromisoformat(value)
    except ValueError:
        fail(f"{context}: invalid last_verified date {value!r}")


def validate_metadata(
    metadata: dict[str, Any],
    context: str,
    *,
    as_of: dt.date,
) -> None:
    unknown = set(metadata) - METADATA_KEYS
    if unknown:
        fail(f"{context}: unknown metadata keys: {', '.join(sorted(unknown))}")

    kind = metadata.get("kind")
    status = metadata.get("status")
    owner = metadata.get("owner")
    if kind not in KINDS:
        fail(f"{context}: unsupported kind {kind!r}")
    if status not in STATUSES:
        fail(f"{context}: unsupported status {status!r}")
    if not isinstance(owner, str) or not owner.strip():
        fail(f"{context}: owner must be a non-empty string")

    if kind == "historical-record":
        if status not in {"historical", "superseded"}:
            fail(f"{context}: historical-record must be historical or superseded")
        if metadata.get("retention") != "append-only":
            fail(f"{context}: historical-record must declare append-only retention")
        if "review_after_days" in metadata:
            fail(f"{context}: historical records do not use a freshness window")
    elif status != "active":
        fail(f"{context}: non-historical pages must have active status")

    if status == "superseded":
        replacement = metadata.get("superseded_by")
        if not isinstance(replacement, str) or not replacement.endswith(".md"):
            fail(f"{context}: superseded pages must name a Markdown replacement")
    elif "superseded_by" in metadata:
        fail(f"{context}: superseded_by is only valid for superseded pages")

    verified = parse_date(metadata.get("last_verified"), context)
    if verified > as_of:
        fail(f"{context}: last_verified {verified} is in the future")
    if status == "active":
        window = metadata.get("review_after_days")
        if not isinstance(window, int) or isinstance(window, bool) or window <= 0:
            fail(f"{context}: active pages need a positive review_after_days")
        age = (as_of - verified).days
        if age > window:
            fail(
                f"{context}: verification is stale ({age} days; "
                f"limit is {window}); review the pages and refresh last_verified"
            )


def validate_model(
    catalog: dict[str, Any],
    pages: list[str],
    *,
    as_of: dt.date,
) -> dict[str, int]:
    expected_root_keys = {"schema_version", "inventory", "default", "collections"}
    if set(catalog) != expected_root_keys:
        fail(
            "catalog keys must be exactly "
            + ", ".join(sorted(expected_root_keys))
        )
    if catalog["schema_version"] != 1:
        fail(f"unsupported schema_version {catalog['schema_version']!r}")

    inventory = catalog["inventory"]
    if not isinstance(inventory, dict) or set(inventory) != {"glob", "sha256"}:
        fail("inventory must contain exactly glob and sha256")
    pattern = inventory["glob"]
    if pattern != "**/*.md":
        fail("inventory glob must remain the recursive wiki rule '**/*.md'")
    matched_pages = sorted(pages)
    for page in matched_pages:
        path = Path(page)
        if path.is_absolute() or ".." in path.parts or path.suffix != ".md":
            fail(f"invalid Markdown inventory path: {page!r}")
    actual_digest = inventory_digest(matched_pages)
    if inventory["sha256"] != actual_digest:
        fail(
            "documentation inventory changed: classify the added/removed page, "
            f"then set inventory.sha256 to {actual_digest}"
        )

    default = catalog["default"]
    if not isinstance(default, dict):
        fail("default metadata must be an object")
    validate_metadata(default, "default", as_of=as_of)

    collections = catalog["collections"]
    if not isinstance(collections, list) or not collections:
        fail("collections must be a non-empty array")

    classified: dict[str, str] = {}
    resolved: dict[str, dict[str, Any]] = {}
    collection_ids: set[str] = set()
    for index, collection in enumerate(collections):
        context = f"collections[{index}]"
        if not isinstance(collection, dict):
            fail(f"{context}: collection must be an object")
        allowed = {"id", "pages"} | METADATA_KEYS
        unknown = set(collection) - allowed
        if unknown:
            fail(f"{context}: unknown keys: {', '.join(sorted(unknown))}")
        collection_id = collection.get("id")
        if not isinstance(collection_id, str) or not collection_id.strip():
            fail(f"{context}: id must be a non-empty string")
        if collection_id in collection_ids:
            fail(f"{context}: duplicate id {collection_id!r}")
        collection_ids.add(collection_id)

        collection_pages = collection.get("pages")
        if (
            not isinstance(collection_pages, list)
            or not collection_pages
            or not all(isinstance(page, str) for page in collection_pages)
        ):
            fail(f"{context}: pages must be a non-empty string array")
        if collection_pages != sorted(collection_pages):
            fail(f"{context}: pages must be sorted")
        if len(collection_pages) != len(set(collection_pages)):
            fail(f"{context}: pages contains duplicates")

        metadata = {key: value for key, value in collection.items() if key in METADATA_KEYS}
        validate_metadata(metadata, context, as_of=as_of)
        for page in collection_pages:
            if page not in matched_pages:
                fail(f"{context}: page is not in the Markdown inventory: {page}")
            if page in classified:
                fail(
                    f"{context}: {page} is already classified by "
                    f"{classified[page]!r}"
                )
            classified[page] = collection_id
            resolved[page] = metadata

    for page in matched_pages:
        resolved.setdefault(page, default)

    counts = {kind: 0 for kind in sorted(KINDS)}
    for page, metadata in resolved.items():
        counts[metadata["kind"]] += 1
        if metadata["status"] == "superseded":
            replacement = metadata["superseded_by"]
            if replacement not in matched_pages:
                fail(f"{page}: superseded_by target is not in inventory: {replacement}")
            if replacement == page:
                fail(f"{page}: cannot supersede itself")
            if resolved[replacement]["status"] == "superseded":
                fail(
                    f"{page}: superseded_by target is also superseded: "
                    f"{replacement}; point directly to the current replacement"
                )
    return counts


def selftest() -> None:
    today = dt.date(2026, 7, 30)
    pages = ["guide.md", "record.md", "reference.md"]
    digest = inventory_digest(pages)
    good = {
        "schema_version": 1,
        "inventory": {"glob": "**/*.md", "sha256": digest},
        "default": {
            "kind": "reference",
            "status": "active",
            "owner": "maintainers",
            "last_verified": "2026-07-30",
            "review_after_days": 180,
        },
        "collections": [
            {
                "id": "history",
                "kind": "historical-record",
                "status": "historical",
                "owner": "evidence maintainers",
                "last_verified": "2026-07-30",
                "retention": "append-only",
                "pages": ["record.md"],
            }
        ],
    }
    validate_model(good, pages, as_of=today)

    cases: list[tuple[str, dict[str, Any], list[str]]] = []
    changed_inventory = json.loads(json.dumps(good))
    cases.append(("inventory drift", changed_inventory, pages + ["new.md"]))
    unowned = json.loads(json.dumps(good))
    unowned["default"]["owner"] = ""
    cases.append(("unowned active page", unowned, pages))
    stale = json.loads(json.dumps(good))
    stale["default"]["last_verified"] = "2025-01-01"
    cases.append(("stale active page", stale, pages))
    duplicate = json.loads(json.dumps(good))
    duplicate["collections"][0]["pages"].append("record.md")
    cases.append(("duplicate page", duplicate, pages))
    supersession_cycle = {
        "schema_version": 1,
        "inventory": {"glob": "**/*.md", "sha256": inventory_digest(["a.md", "b.md"])},
        "default": good["default"],
        "collections": [
            {
                "id": "old-a",
                "kind": "historical-record",
                "status": "superseded",
                "owner": "evidence maintainers",
                "last_verified": "2026-07-30",
                "retention": "append-only",
                "superseded_by": "b.md",
                "pages": ["a.md"],
            },
            {
                "id": "old-b",
                "kind": "historical-record",
                "status": "superseded",
                "owner": "evidence maintainers",
                "last_verified": "2026-07-30",
                "retention": "append-only",
                "superseded_by": "a.md",
                "pages": ["b.md"],
            },
        ],
    }
    cases.append(("supersession cycle", supersession_cycle, ["a.md", "b.md"]))

    for name, model, model_pages in cases:
        try:
            validate_model(model, model_pages, as_of=today)
        except CatalogError:
            continue
        raise AssertionError(f"selftest did not reject {name}")
    print(f"doc-lifecycle selftest: passed {len(cases) + 1} cases")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selftest", action="store_true")
    parser.add_argument(
        "--as-of",
        type=dt.date.fromisoformat,
        default=dt.date.today(),
        help="override the freshness date (YYYY-MM-DD)",
    )
    args = parser.parse_args()
    try:
        if args.selftest:
            selftest()
            return 0
        catalog = load_json(CATALOG)
        pages = sorted(path.relative_to(DOCS).as_posix() for path in DOCS.rglob("*.md"))
        counts = validate_model(catalog, pages, as_of=args.as_of)
    except (CatalogError, AssertionError) as error:
        print(f"doc-lifecycle: {error}", file=sys.stderr)
        return 1
    summary = ", ".join(f"{kind}={counts[kind]}" for kind in sorted(counts))
    print(f"doc-lifecycle: {len(pages)} pages classified ({summary})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
