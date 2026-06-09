#!/usr/bin/env python3
"""Validate checked-in semantic-pack v0 schema examples.

This is intentionally a lightweight structural check, not a full JSON Schema
implementation. It keeps the design examples honest until nose has a real pack
loader and conformance harness.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "docs" / "schemas" / "semantic-pack-v0.schema.json"
EXAMPLES = sorted(
    (ROOT / "docs" / "examples" / "semantic-packs" / "v0").glob("*.json")
)

API_VERSION = "nose.semantic-pack.v0"
PACK_KINDS = {
    "LanguagePack",
    "StdlibPack",
    "LibraryPack",
    "ProtocolPack",
    "LawPack",
}
TRUST = {
    "default-first-party",
    "first-party-optional",
    "external-opt-in",
}
CHANNELS = {
    "syntax-only",
    "near-only",
    "abstraction-witness",
    "exact-empirical",
    "exact-proven",
}
PROOF_STATUSES = {
    "proven",
    "covered",
    "missing",
    "empirical-only",
    "rejected-counterexample",
}
ANCHORS = {
    "source-span",
    "node",
    "param",
    "binding",
    "sequence",
    "module",
    "package",
}
EVIDENCE_PREFIXES = (
    "Source.",
    "Symbol.",
    "Import.",
    "Domain.",
    "Type.",
    "Guard.",
    "Place.",
    "Effect.",
    "LibraryApi.",
    "CallTarget.",
    "SequenceSurface.",
)
ALLOWED_REQUIREMENT_PREFIXES = EVIDENCE_PREFIXES + ("nose.",)


def fail(path: Path, message: str) -> None:
    rel = path.relative_to(ROOT)
    raise SystemExit(f"{rel}: {message}")


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        fail(path, f"invalid JSON: {exc}")
    if not isinstance(value, dict):
        fail(path, "top-level JSON value must be an object")
    return value


def require_object(path: Path, parent: dict[str, Any], key: str) -> dict[str, Any]:
    value = parent.get(key)
    if not isinstance(value, dict):
        fail(path, f"`{key}` must be an object")
    return value


def require_array(path: Path, parent: dict[str, Any], key: str, *, min_items: int = 0) -> list[Any]:
    value = parent.get(key)
    if not isinstance(value, list):
        fail(path, f"`{key}` must be an array")
    if len(value) < min_items:
        fail(path, f"`{key}` must contain at least {min_items} item(s)")
    return value


def require_string(path: Path, parent: dict[str, Any], key: str) -> str:
    value = parent.get(key)
    if not isinstance(value, str) or not value:
        fail(path, f"`{key}` must be a non-empty string")
    return value


def require_bool(path: Path, parent: dict[str, Any], key: str) -> bool:
    value = parent.get(key)
    if not isinstance(value, bool):
        fail(path, f"`{key}` must be a boolean")
    return value


def require_enum(path: Path, value: Any, allowed: set[str], label: str) -> str:
    if not isinstance(value, str) or value not in allowed:
        fail(path, f"`{label}` must be one of {sorted(allowed)}")
    return value


def require_unique_ids(path: Path, items: list[Any], label: str) -> set[str]:
    ids: set[str] = set()
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            fail(path, f"`{label}[{index}]` must be an object")
        item_id = require_string(path, item, "id")
        if item_id in ids:
            fail(path, f"duplicate id `{item_id}` in `{label}`")
        ids.add(item_id)
    return ids


def validate_requirement(path: Path, requirement: Any, known_refs: set[str], context: str) -> None:
    if not isinstance(requirement, dict):
        fail(path, f"{context} requirement must be an object")
    ref = require_string(path, requirement, "ref")
    if ref not in known_refs and not ref.startswith(ALLOWED_REQUIREMENT_PREFIXES):
        fail(path, f"{context} requirement references unknown id `{ref}`")
    require_string(path, requirement, "subject")
    require_bool(path, requirement, "required")


def validate_evidence_producer(path: Path, producer: dict[str, Any], known_refs: set[str]) -> None:
    kind = require_string(path, producer, "kind")
    if not kind.startswith(EVIDENCE_PREFIXES):
        fail(path, f"evidence producer `{producer['id']}` has unknown kind `{kind}`")
    channel = require_enum(
        path,
        producer.get("channel"),
        CHANNELS,
        f"producer {producer['id']}.channel",
    )
    anchors = require_array(path, producer, "anchors", min_items=1)
    for anchor in anchors:
        require_enum(path, anchor, ANCHORS, f"producer {producer['id']}.anchors")
    stable_inputs = require_array(path, producer, "stable_hash_inputs", min_items=1)
    if "pack.id" not in stable_inputs or "producer.id" not in stable_inputs:
        fail(path, f"producer `{producer['id']}` stable_hash_inputs must include pack.id and producer.id")
    conflict_policy = producer.get("conflict_policy")
    if conflict_policy not in {"fail-closed", "near-only"}:
        fail(path, f"producer `{producer['id']}` conflict_policy must be fail-closed or near-only")
    if channel.startswith("exact") and conflict_policy != "fail-closed":
        fail(path, f"exact-capable producer `{producer['id']}` must fail closed on conflicts")
    for emitted in producer.get("emits", []):
        if not isinstance(emitted, str) or not emitted.startswith(EVIDENCE_PREFIXES):
            fail(path, f"producer `{producer['id']}` emits unknown evidence kind `{emitted}`")
    for requirement in producer.get("requires", []):
        validate_requirement(path, requirement, known_refs, f"producer {producer['id']}")


def validate_contract(
    path: Path,
    contract: dict[str, Any],
    known_refs: set[str],
    conformance_ids: set[str],
    *,
    require_surface: bool,
) -> None:
    contract_id = require_string(path, contract, "id")
    if require_surface:
        require_object(path, contract, "surface")
    requires = require_array(path, contract, "requires")
    semantics = require_object(path, contract, "semantics")
    channel = require_enum(
        path,
        contract.get("channel"),
        CHANNELS,
        f"contract {contract_id}.channel",
    )
    require_enum(
        path,
        contract.get("proof_status"),
        PROOF_STATUSES,
        f"contract {contract_id}.proof_status",
    )
    refs = require_array(path, contract, "conformance_refs", min_items=1)
    for ref in refs:
        if ref not in conformance_ids:
            fail(path, f"contract `{contract_id}` references missing conformance fixture `{ref}`")
    if channel.startswith("exact"):
        if not requires:
            fail(path, f"exact-capable contract `{contract_id}` must declare evidence requirements")
        if not any(ref in conformance_ids for ref in refs):
            fail(path, f"exact-capable contract `{contract_id}` must reference conformance fixtures")
        if "demand" not in semantics or "effects" not in semantics:
            fail(path, f"exact-capable contract `{contract_id}` must declare demand and effects")
    for requirement in requires:
        validate_requirement(path, requirement, known_refs, f"contract {contract_id}")


def validate_pack(path: Path, doc: dict[str, Any]) -> None:
    if doc.get("api_version") != API_VERSION:
        fail(path, f"`api_version` must be {API_VERSION}")
    forbidden = {"verdicts", "fingerprints", "exact_pairs", "value_graph_rewrites"}
    present_forbidden = forbidden.intersection(doc)
    if present_forbidden:
        fail(path, f"manifest must not contain verdict/fingerprint fields: {sorted(present_forbidden)}")

    pack = require_object(path, doc, "pack")
    require_string(path, pack, "id")
    require_string(path, pack, "version")
    require_string(path, pack, "display_name")
    require_enum(path, pack.get("kind"), PACK_KINDS, "pack.kind")
    trust = require_enum(path, pack.get("trust"), TRUST, "pack.trust")
    enabled_by_default = require_bool(path, pack, "enabled_by_default")
    if trust == "external-opt-in" and enabled_by_default:
        fail(path, "external-opt-in packs must not be enabled by default")

    provenance = require_object(path, doc, "provenance")
    require_object(path, provenance, "provider")
    require_string(path, provenance, "license")
    require_string(path, provenance, "repository")

    compatibility = require_object(path, doc, "compatibility")
    require_string(path, compatibility, "nose")

    languages = require_array(path, doc, "supported_languages", min_items=1)
    for language in languages:
        if not isinstance(language, dict):
            fail(path, "`supported_languages` entries must be objects")
        require_string(path, language, "id")

    dependencies = doc.get("dependencies", [])
    if dependencies is None:
        dependencies = []
    if not isinstance(dependencies, list):
        fail(path, "`dependencies` must be an array when present")
    dependency_ids = require_unique_ids(path, dependencies, "dependencies")

    declares = require_object(path, doc, "declares")
    producers = require_array(path, declares, "evidence_producers")
    contracts = require_array(path, declares, "contracts")
    laws = declares.get("value_laws", [])
    if laws is None:
        laws = []
    if not isinstance(laws, list):
        fail(path, "`declares.value_laws` must be an array when present")

    producer_ids = require_unique_ids(path, producers, "declares.evidence_producers")
    contract_ids = require_unique_ids(path, contracts, "declares.contracts")
    law_ids = require_unique_ids(path, laws, "declares.value_laws")
    known_refs = dependency_ids | producer_ids | contract_ids | law_ids

    conformance = require_object(path, doc, "conformance")
    positives = require_array(path, conformance, "positive_fixtures", min_items=1)
    negatives = require_array(path, conformance, "hard_negatives", min_items=1)
    conformance_ids = require_unique_ids(path, positives + negatives, "conformance fixtures")
    require_array(path, conformance, "known_unsupported")

    for producer in producers:
        validate_evidence_producer(path, producer, known_refs)
    for contract in contracts:
        validate_contract(path, contract, known_refs, conformance_ids, require_surface=True)
    for law in laws:
        validate_contract(path, law, known_refs, conformance_ids, require_surface=False)


def main() -> int:
    schema = read_json(SCHEMA)
    if schema.get("$id") is None or schema.get("title") is None:
        fail(SCHEMA, "schema must declare $id and title")
    if not EXAMPLES:
        fail(ROOT / "docs" / "examples" / "semantic-packs" / "v0", "no examples found")
    for example in EXAMPLES:
        validate_pack(example, read_json(example))
    print(f"validated {len(EXAMPLES)} semantic-pack v0 example(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
