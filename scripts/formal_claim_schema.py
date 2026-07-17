#!/usr/bin/env python3
"""Structured claim metadata and reporting for formal obligations."""

from __future__ import annotations

import re
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

CLAIM_ID_RE = re.compile(r"^nose\.claim\.[a-z0-9_.-]+$")
CLAIM_MARKER_RE = re.compile(r"\bproof-claim:\s*(nose\.claim\.[a-z0-9_.-]+)\b")
PRECONDITION_ID_RE = re.compile(r"^[a-z][a-z0-9_]*$")
THEOREM_STATUSES = {
    "proven",
    "covered",
    "missing",
    "empirical",
    "rejected-counterexample",
}
PRECONDITION_STATUSES = {"proven", "empirical", "rejected"}
PRECONDITION_KINDS = {"modeled", "runtime"}
PRODUCT_SURFACES = {
    "canonicalization",
    "exact-normalization",
    "near-witness",
    "structural-invariant",
    "verification-boundary",
}
SOURCE_CLAIM_SURFACES = {"canonicalization", "exact-normalization"}


def non_empty_str(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def string_list(value: Any, field: str, errors: list[str], where: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        errors.append(f"{where}: `{field}` must be a list of strings")
        return []
    return value


def validate_reference(root: Path, reference: str, errors: list[str], where: str) -> None:
    path_text, _, anchor = reference.partition("#")
    path = root / path_text
    if not path.is_file():
        errors.append(f"{where}: evidence path does not exist: {path_text}")
        return
    if anchor:
        source = path.read_text(encoding="utf-8", errors="replace")
        if anchor in source:
            return
        local_name = anchor.rsplit(".", 1)[-1]
        if path.suffix == ".lean" and re.search(
            rf"^\s*(?:theorem|lemma|def)\s+{re.escape(local_name)}\b",
            source,
            re.MULTILINE,
        ):
            return
        errors.append(f"{where}: evidence anchor `{anchor}` not found in {path_text}")


def lint_claim_schema(obligation: Any, root: Path, errors: list[str]) -> None:
    rel = obligation.path.relative_to(root)
    where = str(rel / "meta.toml")
    meta = obligation.meta
    if "status" in meta:
        errors.append(
            f"{where}: top-level `status` is ambiguous; move theorem status to `[theorem]`"
        )

    claim = meta.get("claim")
    if not isinstance(claim, dict):
        errors.append(f"{where}: `[claim]` must be a table")
        claim = {}
    claim_id = claim.get("id")
    expected_claim_id = f"nose.claim.{obligation.id}"
    if claim_id != expected_claim_id or not (
        isinstance(claim_id, str) and CLAIM_ID_RE.fullmatch(claim_id)
    ):
        errors.append(f"{where}: `claim.id` must be `{expected_claim_id}`")

    theorem = meta.get("theorem")
    if not isinstance(theorem, dict):
        errors.append(f"{where}: `[theorem]` must be a table")
        theorem = {}
    theorem_status = theorem.get("status")
    if theorem_status not in THEOREM_STATUSES:
        errors.append(f"{where}: unknown theorem status `{theorem_status}`")
    for field in ("statement", "model"):
        if not non_empty_str(theorem.get(field)):
            errors.append(f"{where}: `theorem.{field}` must be a non-empty string")

    product = meta.get("product")
    if not isinstance(product, dict):
        errors.append(f"{where}: `[product]` must be a table")
        product = {}
    surface = product.get("surface")
    if surface not in PRODUCT_SURFACES:
        errors.append(f"{where}: unknown product surface `{surface}`")
    if not non_empty_str(product.get("guarantee")):
        errors.append(f"{where}: `product.guarantee` must be a non-empty string")

    preconditions = meta.get("preconditions")
    if not isinstance(preconditions, dict) or not preconditions:
        errors.append(f"{where}: `[preconditions.*]` must record at least one precondition")
        preconditions = {}
    empirical_preconditions = 0
    runtime_empirical_preconditions = 0
    lean = meta.get("lean", {})
    lean_theorems = set(lean.get("theorems", [])) if isinstance(lean, dict) else set()
    for precondition_id, precondition in preconditions.items():
        pre_where = f"{where}: precondition `{precondition_id}`"
        if not PRECONDITION_ID_RE.fullmatch(precondition_id):
            errors.append(f"{pre_where} has an invalid id")
        if not isinstance(precondition, dict):
            errors.append(f"{pre_where} must be a table")
            continue
        if precondition.get("kind") not in PRECONDITION_KINDS:
            errors.append(f"{pre_where} has unknown kind `{precondition.get('kind')}`")
        status = precondition.get("status")
        if status not in PRECONDITION_STATUSES:
            errors.append(f"{pre_where} has unknown status `{status}`")
        empirical_preconditions += int(status == "empirical")
        runtime_empirical_preconditions += int(
            status == "empirical" and precondition.get("kind") == "runtime"
        )
        if status == "proven":
            proof = precondition.get("proof")
            if precondition.get("kind") != "modeled" or proof not in lean_theorems:
                errors.append(
                    f"{pre_where}: proven preconditions must be modeled and name a theorem "
                    "from `lean.theorems` in `proof`"
                )
        if not non_empty_str(precondition.get("summary")):
            errors.append(f"{pre_where} needs a non-empty summary")
        references = string_list(
            precondition.get("evidence", []),
            f"preconditions.{precondition_id}.evidence",
            errors,
            where,
        )
        if not references:
            errors.append(f"{pre_where} needs evidence")
        for reference in references:
            validate_reference(root, reference, errors, pre_where)

    evidence = meta.get("evidence")
    if not isinstance(evidence, dict):
        errors.append(f"{where}: `[evidence]` must be a table")
        evidence = {}
    tests = string_list(evidence.get("executable_tests", []), "evidence.executable_tests", errors, where)
    counterexamples = string_list(evidence.get("counterexamples", []), "evidence.counterexamples", errors, where)
    if (theorem_status == "empirical" or empirical_preconditions) and not tests:
        errors.append(f"{where}: empirical theorem/preconditions need executable tests")
    if theorem_status == "empirical" and not counterexamples:
        errors.append(f"{where}: empirical theorem needs executable counterexamples")
    rust = meta.get("rust", {})
    rust_files = rust.get("files", []) if isinstance(rust, dict) else []
    if theorem_status == "proven" and rust_files and not runtime_empirical_preconditions:
        errors.append(
            f"{where}: Rust-backed proven theorem needs an empirical runtime precondition"
        )
    for reference in [*tests, *counterexamples]:
        validate_reference(root, reference, errors, where)


def collect_claim_markers(roots: Iterable[Path], root: Path) -> dict[str, set[str]]:
    markers: dict[str, set[str]] = {}
    for source_root in roots:
        if not source_root.exists():
            continue
        for rust_file in sorted(source_root.rglob("*.rs")):
            rel = str(rust_file.resolve().relative_to(root.resolve()))
            for match in CLAIM_MARKER_RE.finditer(rust_file.read_text(encoding="utf-8")):
                markers.setdefault(match.group(1), set()).add(rel)
    return markers


def lint_claim_marker_index(
    obligations: dict[str, Any], claim_markers: dict[str, set[str]], root: Path, errors: list[str]
) -> None:
    by_claim: dict[str, Any] = {}
    for obligation in obligations.values():
        claim = obligation.meta.get("claim", {})
        claim_id = claim.get("id") if isinstance(claim, dict) else None
        if not isinstance(claim_id, str):
            continue
        if claim_id in by_claim:
            errors.append(f"duplicate claim id `{claim_id}`")
        by_claim[claim_id] = obligation

    for claim_id, files in sorted(claim_markers.items()):
        if claim_id not in by_claim:
            for source_file in sorted(files):
                errors.append(f"{source_file}: marker references unregistered claim `{claim_id}`")
            continue
        obligation = by_claim[claim_id]
        rust = obligation.meta.get("rust", {})
        listed = set(rust.get("files", [])) if isinstance(rust, dict) else set()
        for source_file in sorted(files - listed):
            where = str(obligation.path.relative_to(root) / "meta.toml")
            errors.append(
                f"{where}: claim marker `{claim_id}` appears in unlisted `{source_file}`"
            )

    for claim_id, obligation in sorted(by_claim.items()):
        product = obligation.meta.get("product", {})
        surface = product.get("surface") if isinstance(product, dict) else None
        if surface not in SOURCE_CLAIM_SURFACES:
            continue
        rust = obligation.meta.get("rust", {})
        listed = set(rust.get("files", [])) if isinstance(rust, dict) else set()
        if not (claim_markers.get(claim_id, set()) & listed):
            where = str(obligation.path.relative_to(root) / "meta.toml")
            errors.append(
                f"{where}: exact/canonicalization claim `{claim_id}` needs a source marker"
            )


def coverage_report(obligations: Iterable[Any]) -> str:
    theorem_counts: Counter[str] = Counter()
    precondition_counts: Counter[str] = Counter()
    surface_counts: Counter[str] = Counter()
    for obligation in obligations:
        theorem = obligation.meta.get("theorem", {})
        product = obligation.meta.get("product", {})
        preconditions = obligation.meta.get("preconditions", {})
        theorem_counts[str(theorem.get("status", "missing"))] += 1
        surface_counts[str(product.get("surface", "missing"))] += 1
        if isinstance(preconditions, dict):
            for precondition in preconditions.values():
                if isinstance(precondition, dict):
                    precondition_counts[str(precondition.get("status", "missing"))] += 1

    def render(counts: Counter[str]) -> str:
        return ", ".join(f"{key}={counts[key]}" for key in sorted(counts))

    return (
        f"theorems[{render(theorem_counts)}]; "
        f"preconditions[{render(precondition_counts)}]; "
        f"product-surfaces[{render(surface_counts)}]"
    )
