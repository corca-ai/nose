"""Content-bound review identity, separate from live source-navigation IDs."""
from __future__ import annotations

import hashlib
import json
from typing import Any

REVIEW_SCHEMA = "nose.duplication-review.v1"


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def fingerprint(record: dict[str, Any]) -> str:
    for field in ("scope", "witness"):
        if not isinstance(record.get(field), str) or not record[field]:
            raise ValueError(f"review record requires a nonempty {field}")
    members = record.get("members")
    if not isinstance(members, list) or not members:
        raise ValueError("review record requires members")
    for member in members:
        if not isinstance(member, dict) or not isinstance(member.get("file"), str):
            raise ValueError("review member requires a file")
        if member.get("symbol") is not None and not isinstance(member["symbol"], str):
            raise ValueError("review member symbol must be a string or null")
        key = member.get("region_key")
        if not isinstance(key, str) or len(key) != 64 or any(c not in "0123456789abcdef" for c in key):
            raise ValueError("review member lacks a complete region content key")
        if set(member) != {"file", "symbol", "region_key", "enclosing", "shared_subdag_offsets"}:
            raise ValueError("review member fields do not match the fingerprint schema")
    payload = {
        "schema": REVIEW_SCHEMA,
        "scope": record["scope"],
        "witness": record["witness"],
        # Retain multiplicity: adding an identical occurrence needs a new review.
        "members": sorted(members, key=canonical),
    }
    return hashlib.sha256(canonical(payload).encode("utf-8")).hexdigest()


def review_record(family: dict[str, Any]) -> dict[str, Any]:
    members = []
    for location in family.get("locations", []):
        boundary = location.get("boundary") or {}
        enclosing = boundary.get("enclosing_unit", location.get("enclosing_unit"))
        if enclosing is not None:
            if not isinstance(enclosing, dict):
                raise ValueError("enclosing ownership is not structured")
            enclosing = {
                "file": enclosing.get("file", location.get("file")),
                "symbol": enclosing.get("name"),
                "kind": enclosing.get("kind", enclosing.get("unit_kind")),
            }
        shared = location.get("shared_subdag")
        if shared is not None:
            start = location.get("start")
            if type(start) is not int or not isinstance(shared, list) or len(shared) != 2 or any(type(n) is not int for n in shared):
                raise ValueError("shared-subdag coordinates cannot be made member-relative")
            shared = [n - start for n in shared]
        members.append({
            "file": location.get("file"),
            "symbol": location.get("name"),
            "region_key": location.get("region_key"),
            "enclosing": enclosing,
            "shared_subdag_offsets": shared,
        })
    record = {
        "scope": family.get("scope") or (family.get("metrics") or {}).get("scope"),
        "witness": family.get("witness"),
        "members": sorted(members, key=canonical),
    }
    key = fingerprint(record)
    return {"fingerprint": key, "display_family_id": family.get("id") or family.get("family_id"), **record}
