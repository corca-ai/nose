#!/usr/bin/env python3
"""Validate the checked evidence-artifact lifecycle catalog."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CATALOG = ROOT / "scripts/evidence/artifacts.json"
SCHEMA = "nose.evidence-artifact-lifecycle.v1"
CANDIDATE_SUFFIXES = (".json", ".jsonl", ".sha256")
LIFECYCLE_CLASSES = {
    "canonical-input",
    "gold-input",
    "sealed-evidence",
    "derived-artifact",
    "receipt",
    "active-baseline",
    "historical-evidence",
    "superseded-output",
}
PRODUCED_CLASSES = {"derived-artifact", "receipt", "active-baseline"}
ALLOWED_SUPERSESSION = {"current", "historical", "sealed", "superseded", "not-applicable"}
CATALOG_SELF_PATH = "scripts/evidence/artifacts.json"


class CatalogError(ValueError):
    pass


@dataclass(frozen=True)
class InventoryEntry:
    size: int
    sha256: str


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repository_inventory(root: Path) -> dict[str, InventoryEntry]:
    completed = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    paths = sorted(
        item.decode("utf-8")
        for item in completed.stdout.split(b"\0")
        if item and item.decode("utf-8").endswith(CANDIDATE_SUFFIXES)
    )
    return {
        path: InventoryEntry(
            size=(root / path).stat().st_size,
            sha256=sha256_file(root / path),
        )
        for path in paths
    }


def load_catalog(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CatalogError(f"cannot read catalog {path}: {error}") from error
    if not isinstance(value, dict):
        raise CatalogError("catalog root must be an object")
    return value


def require_keys(value: dict[str, Any], required: set[str], context: str) -> None:
    missing = sorted(required - value.keys())
    if missing:
        raise CatalogError(f"{context} is missing fields: {', '.join(missing)}")


def require_nonempty_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise CatalogError(f"{context} must be a non-empty string")
    return value


def require_string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise CatalogError(f"{context} must be a non-empty list")
    for index, item in enumerate(value):
        require_nonempty_string(item, f"{context}[{index}]")
    return value


def matches_any(path: str, patterns: list[str]) -> bool:
    candidate = PurePosixPath(path)
    return any(candidate.match(pattern) for pattern in patterns)


def set_inventory_digest(
    paths: set[str],
    inventory: dict[str, InventoryEntry],
) -> str:
    digest = hashlib.sha256()
    for path in sorted(paths):
        if path == CATALOG_SELF_PATH:
            row = f"{path}\0<catalog-self>\n"
        else:
            entry = inventory[path]
            row = f"{path}\0{entry.size}\0{entry.sha256}\n"
        digest.update(row.encode("utf-8"))
    return digest.hexdigest()


def relation_required(path: str) -> bool:
    basename = PurePosixPath(path).name.lower()
    if basename.endswith(".sha256"):
        return True
    if "/schemas/" in f"/{path.lower()}/":
        return False
    return "receipt" in basename or "seal" in basename


def validate_catalog(
    catalog: dict[str, Any],
    inventory: dict[str, InventoryEntry],
) -> dict[str, int]:
    require_keys(
        catalog,
        {
            "schema",
            "large_file_threshold_bytes",
            "lifecycle_classes",
            "artifact_sets",
            "artifacts",
            "relations",
        },
        "catalog",
    )
    if catalog["schema"] != SCHEMA:
        raise CatalogError(f"schema must be {SCHEMA!r}")
    threshold = catalog["large_file_threshold_bytes"]
    if not isinstance(threshold, int) or threshold <= 0:
        raise CatalogError("large_file_threshold_bytes must be a positive integer")

    classes = catalog["lifecycle_classes"]
    if not isinstance(classes, dict) or set(classes) != LIFECYCLE_CLASSES:
        raise CatalogError(
            "lifecycle_classes must define exactly: "
            + ", ".join(sorted(LIFECYCLE_CLASSES))
        )
    for name, policy in classes.items():
        if not isinstance(policy, dict):
            raise CatalogError(f"lifecycle class {name!r} must be an object")
        require_keys(policy, {"definition", "retention_rule"}, f"lifecycle class {name!r}")
        require_nonempty_string(policy["definition"], f"{name}.definition")
        require_nonempty_string(policy["retention_rule"], f"{name}.retention_rule")

    candidate_paths = set(inventory)
    artifact_sets = catalog["artifact_sets"]
    if not isinstance(artifact_sets, list) or not artifact_sets:
        raise CatalogError("artifact_sets must be a non-empty list")
    set_ids: set[str] = set()
    covered: set[str] = set()
    for index, artifact_set in enumerate(artifact_sets):
        context = f"artifact_sets[{index}]"
        if not isinstance(artifact_set, dict):
            raise CatalogError(f"{context} must be an object")
        require_keys(
            artifact_set,
            {
                "id",
                "globs",
                "owner",
                "producer",
                "validator",
                "consumers",
                "retention_policy",
                "artifact_count",
                "inventory_sha256",
            },
            context,
        )
        set_id = require_nonempty_string(artifact_set["id"], f"{context}.id")
        if set_id in set_ids:
            raise CatalogError(f"duplicate artifact-set id: {set_id}")
        set_ids.add(set_id)
        patterns = require_string_list(artifact_set["globs"], f"{context}.globs")
        require_nonempty_string(artifact_set["owner"], f"{context}.owner")
        require_nonempty_string(artifact_set["producer"], f"{context}.producer")
        require_nonempty_string(artifact_set["validator"], f"{context}.validator")
        require_string_list(artifact_set["consumers"], f"{context}.consumers")
        require_nonempty_string(
            artifact_set["retention_policy"], f"{context}.retention_policy"
        )
        matches = {path for path in candidate_paths if matches_any(path, patterns)}
        if not matches:
            raise CatalogError(f"{context} globs do not match a checked artifact")
        overlap = covered & matches
        if overlap:
            raise CatalogError(
                f"{context} overlaps another owning set:\n  "
                + "\n  ".join(sorted(overlap))
            )
        if artifact_set["artifact_count"] != len(matches):
            raise CatalogError(
                f"{context} artifact-count drift "
                f"(catalog {artifact_set['artifact_count']}, actual {len(matches)})"
            )
        actual_set_digest = set_inventory_digest(matches, inventory)
        if artifact_set["inventory_sha256"] != actual_set_digest:
            raise CatalogError(
                f"{context} inventory drift "
                f"(catalog {artifact_set['inventory_sha256']}, "
                f"actual {actual_set_digest})"
            )
        covered.update(matches)

    uncovered = sorted(candidate_paths - covered)
    if uncovered:
        raise CatalogError(
            "checked JSON/JSONL/checksum artifacts lack an owning set:\n  "
            + "\n  ".join(uncovered)
        )

    artifacts = catalog["artifacts"]
    if not isinstance(artifacts, list):
        raise CatalogError("artifacts must be a list")
    explicit_paths: set[str] = set()
    for index, artifact in enumerate(artifacts):
        context = f"artifacts[{index}]"
        if not isinstance(artifact, dict):
            raise CatalogError(f"{context} must be an object")
        require_keys(
            artifact,
            {
                "path",
                "bytes",
                "sha256",
                "lifecycle",
                "owner",
                "producer",
                "producer_exception",
                "validator",
                "consumers",
                "source_identity",
                "supersession",
                "retention",
            },
            context,
        )
        path = require_nonempty_string(artifact["path"], f"{context}.path")
        if path in explicit_paths:
            raise CatalogError(f"duplicate explicit artifact: {path}")
        explicit_paths.add(path)
        if path not in inventory:
            raise CatalogError(f"explicit artifact is not checked in: {path}")
        if path not in covered:
            raise CatalogError(f"explicit artifact lacks an owning set: {path}")
        actual = inventory[path]
        if artifact["bytes"] != actual.size:
            raise CatalogError(
                f"{path}: size drift (catalog {artifact['bytes']}, actual {actual.size})"
            )
        digest = artifact["sha256"]
        if not isinstance(digest, str) or len(digest) != 64:
            raise CatalogError(f"{path}: sha256 must be a 64-character digest")
        if digest != actual.sha256:
            raise CatalogError(
                f"{path}: digest drift (catalog {digest}, actual {actual.sha256})"
            )
        lifecycle = artifact["lifecycle"]
        if lifecycle not in LIFECYCLE_CLASSES:
            raise CatalogError(f"{path}: unknown lifecycle class {lifecycle!r}")
        require_nonempty_string(artifact["owner"], f"{path}.owner")
        producer = artifact["producer"]
        exception = artifact["producer_exception"]
        if producer is not None:
            require_nonempty_string(producer, f"{path}.producer")
        if exception is not None:
            require_nonempty_string(exception, f"{path}.producer_exception")
        if lifecycle in PRODUCED_CLASSES and producer is None and exception is None:
            raise CatalogError(
                f"{path}: {lifecycle} requires a producer or a producer exception"
            )
        require_nonempty_string(artifact["validator"], f"{path}.validator")
        require_string_list(artifact["consumers"], f"{path}.consumers")

        identity = artifact["source_identity"]
        if not isinstance(identity, dict):
            raise CatalogError(f"{path}.source_identity must be an object")
        require_keys(identity, {"kind", "value"}, f"{path}.source_identity")
        require_nonempty_string(identity["kind"], f"{path}.source_identity.kind")
        require_nonempty_string(identity["value"], f"{path}.source_identity.value")

        supersession = artifact["supersession"]
        if not isinstance(supersession, dict):
            raise CatalogError(f"{path}.supersession must be an object")
        require_keys(supersession, {"status", "by"}, f"{path}.supersession")
        if supersession["status"] not in ALLOWED_SUPERSESSION:
            raise CatalogError(f"{path}: invalid supersession status")
        superseded_by = supersession["by"]
        if supersession["status"] == "superseded":
            require_nonempty_string(superseded_by, f"{path}.supersession.by")
        elif superseded_by is not None:
            raise CatalogError(f"{path}: only superseded artifacts may set supersession.by")

        retention = artifact["retention"]
        if not isinstance(retention, dict):
            raise CatalogError(f"{path}.retention must be an object")
        require_keys(
            retention,
            {"decision", "reason", "removal_conditions"},
            f"{path}.retention",
        )
        if retention["decision"] != "retain":
            raise CatalogError(f"{path}: this closeout only permits retain decisions")
        require_nonempty_string(retention["reason"], f"{path}.retention.reason")
        require_nonempty_string(
            retention["removal_conditions"], f"{path}.retention.removal_conditions"
        )

    expected_large = {
        path for path, entry in inventory.items() if entry.size >= threshold
    }
    if explicit_paths != expected_large:
        missing = sorted(expected_large - explicit_paths)
        stale = sorted(explicit_paths - expected_large)
        details = []
        if missing:
            details.append("missing large artifacts:\n  " + "\n  ".join(missing))
        if stale:
            details.append("no longer large:\n  " + "\n  ".join(stale))
        raise CatalogError("\n".join(details))

    relations = catalog["relations"]
    if not isinstance(relations, list):
        raise CatalogError("relations must be a list")
    relation_keys: set[tuple[str, str, str]] = set()
    related: set[str] = set()
    supersedes: dict[str, str] = {}
    for index, relation in enumerate(relations):
        context = f"relations[{index}]"
        if not isinstance(relation, dict):
            raise CatalogError(f"{context} must be an object")
        require_keys(relation, {"kind", "binder", "bound", "rationale"}, context)
        kind = require_nonempty_string(relation["kind"], f"{context}.kind")
        binder = require_nonempty_string(relation["binder"], f"{context}.binder")
        bound = require_nonempty_string(relation["bound"], f"{context}.bound")
        require_nonempty_string(relation["rationale"], f"{context}.rationale")
        if binder not in inventory or bound not in inventory:
            raise CatalogError(
                f"{context} endpoint is not a checked JSON/JSONL/checksum artifact"
            )
        key = (kind, binder, bound)
        if key in relation_keys:
            raise CatalogError(f"duplicate relation: {key}")
        relation_keys.add(key)
        related.update((binder, bound))
        if kind == "supersedes":
            if binder == bound:
                raise CatalogError(f"{context}: artifact cannot supersede itself")
            supersedes[bound] = binder

    must_be_related = {
        path for path in candidate_paths if relation_required(path)
    } | {
        artifact["path"]
        for artifact in artifacts
        if artifact["lifecycle"] in {"sealed-evidence", "receipt", "active-baseline"}
    }
    unrelated = sorted(must_be_related - related)
    if unrelated:
        raise CatalogError(
            "receipt/seal/baseline artifacts lack a binding relation:\n  "
            + "\n  ".join(unrelated)
        )

    for start in supersedes:
        seen: set[str] = set()
        cursor = start
        while cursor in supersedes:
            if cursor in seen:
                raise CatalogError(f"supersession cycle includes {cursor}")
            seen.add(cursor)
            cursor = supersedes[cursor]

    return {
        "artifact_sets": len(artifact_sets),
        "checked_artifacts": len(candidate_paths),
        "large_artifacts": len(explicit_paths),
        "relations": len(relations),
    }


def run_self_test() -> None:
    payloads = {
        "bench/goldens/gold.json": b'{"gold":true}\n',
        "bench/review/receipt.json": b'{"receipt":true}\n',
    }
    inventory = {
        path: InventoryEntry(len(payload), hashlib.sha256(payload).hexdigest())
        for path, payload in payloads.items()
    }
    catalog: dict[str, Any] = {
        "schema": SCHEMA,
        "large_file_threshold_bytes": 16,
        "lifecycle_classes": {
            name: {"definition": f"{name} definition", "retention_rule": "retain"}
            for name in sorted(LIFECYCLE_CLASSES)
        },
        "artifact_sets": [
            {
                "id": "test",
                "globs": ["bench/goldens/*.json", "bench/review/*.json"],
                "owner": "test owner",
                "producer": "test producer",
                "validator": "test validator",
                "consumers": ["test consumer"],
                "retention_policy": "test retention",
                "artifact_count": 2,
                "inventory_sha256": set_inventory_digest(set(inventory), inventory),
            }
        ],
        "artifacts": [
            {
                "path": "bench/review/receipt.json",
                "bytes": inventory["bench/review/receipt.json"].size,
                "sha256": inventory["bench/review/receipt.json"].sha256,
                "lifecycle": "receipt",
                "owner": "test owner",
                "producer": "test producer",
                "producer_exception": None,
                "validator": "test validator",
                "consumers": ["test consumer"],
                "source_identity": {"kind": "test", "value": "test source"},
                "supersession": {"status": "current", "by": None},
                "retention": {
                    "decision": "retain",
                    "reason": "test evidence",
                    "removal_conditions": "replacement is verified",
                },
            }
        ],
        "relations": [
            {
                "kind": "receipt-binds",
                "binder": "bench/review/receipt.json",
                "bound": "bench/goldens/gold.json",
                "rationale": "test binding",
            }
        ],
    }
    counts = validate_catalog(catalog, inventory)
    if counts["large_artifacts"] != 1:
        raise CatalogError("self-test did not exercise the large-file boundary")

    mutations = [
        ("digest drift", lambda value: value["artifacts"][0].update(sha256="0" * 64)),
        ("coverage gap", lambda value: value["artifact_sets"][0].update(globs=["bench/review/*.json"])),
        ("broken relation", lambda value: value.update(relations=[])),
    ]
    for label, mutate in mutations:
        broken = copy.deepcopy(catalog)
        mutate(broken)
        try:
            validate_catalog(broken, inventory)
        except CatalogError:
            continue
        raise CatalogError(f"self-test mutation did not fail closed: {label}")

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "catalog.json"
        path.write_text(json.dumps(catalog), encoding="utf-8")
        if load_catalog(path)["schema"] != SCHEMA:
            raise CatalogError("self-test catalog round trip failed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.self_test:
            run_self_test()
            print("evidence artifact lifecycle self-test: ok")
        catalog_path = args.catalog
        if not catalog_path.is_absolute():
            catalog_path = ROOT / catalog_path
        counts = validate_catalog(
            load_catalog(catalog_path),
            repository_inventory(ROOT),
        )
    except (CatalogError, subprocess.CalledProcessError) as error:
        print(f"evidence artifact lifecycle validation failed: {error}", file=sys.stderr)
        return 1
    print(
        "evidence artifact lifecycle: "
        f"{counts['checked_artifacts']} checked artifacts, "
        f"{counts['artifact_sets']} owning sets, "
        f"{counts['large_artifacts']} files >= 1 MiB, "
        f"{counts['relations']} bindings"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
