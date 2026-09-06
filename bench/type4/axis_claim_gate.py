#!/usr/bin/env python3
"""Implementation for the executable Type-4 axis/language claim gate."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REGISTRY = HERE / "axis_language_claims.v1.json"
EVIDENCE = HERE / "coverage_evidence.v1.json"
BLIND_RECEIPT = HERE / "blind_attack.v1.json"
DECLARATIVE_MATRIX = HERE / "declarative_claim_matrix.v1.json"

sys.path.insert(0, str(HERE))
import coverage_taxonomy as taxonomy  # noqa: E402
import coverage_probe  # noqa: E402
import coverage_sweep  # noqa: E402

BOUNDARY_CLASSES = {"domain", "effect", "protocol"}


def aggregate_cells(rows: list[dict]) -> dict[tuple[str, str], dict]:
    cells: dict[tuple[str, str], dict] = {}
    for row in rows:
        key = (row.get("axis"), row.get("language"))
        cell = cells.setdefault(
            key,
            {"pos": 0, "pos_hit": 0, "neg": 0, "false_merges": 0, "producers": set()},
        )
        for field in ("pos", "pos_hit", "neg", "false_merges"):
            cell[field] += row.get(field, 0)
        cell["producers"].add(row.get("gen_axis"))
    return cells


def covered_cell(cell: dict) -> bool:
    return (
        cell["pos"] > 0
        and cell["pos_hit"] == cell["pos"]
        and cell["neg"] > 0
        and cell["false_merges"] == 0
    )


def expected_probe_cells() -> set[tuple[str, str]]:
    expected: set[tuple[str, str]] = set()
    axes = taxonomy.axis_index()
    if not coverage_probe.PROBES.is_dir():
        return expected
    for axis_dir in sorted(path for path in coverage_probe.PROBES.iterdir() if path.is_dir()):
        for lang_dir in sorted(path for path in axis_dir.iterdir() if path.is_dir()):
            has_positive = (lang_dir / "pos").is_dir()
            has_negative = any(
                path.is_dir() and path.name.startswith("neg") for path in lang_dir.iterdir()
            )
            soundness_only = axes.get(axis_dir.name, {}).get("family") == "soundness"
            if has_positive or (has_negative and soundness_only):
                expected.add((f"probe:{axis_dir.name}", lang_dir.name))
    return expected


def exact_cells(registry: dict) -> set[tuple[str, str]]:
    return {
        (claim["axis_id"], language)
        for claim in registry.get("axes", [])
        for language in claim.get("exact_languages", [])
    }


def declarative_ids(declarative: dict) -> tuple[set[str], set[str]]:
    case_ids = {case["id"] for case in declarative.get("cases", []) if case.get("id")}
    negative_ids = {
        negative_id
        for case in declarative.get("cases", [])
        for negative_id in case.get("hard_negative_ids", [])
    }
    return case_ids, negative_ids


def validate_ratchet(
    registry: dict,
    declarative: dict,
    base_registry: dict | None,
    base_declarative: dict | None,
) -> None:
    errors: list[str] = []
    if base_registry:
        withdrawn = exact_cells(base_registry) - exact_cells(registry)
        if withdrawn:
            errors.append(f"exact-cell regression against base: {sorted(withdrawn)}")
    if base_declarative:
        current_cases, current_negatives = declarative_ids(declarative)
        base_cases, base_negatives = declarative_ids(base_declarative)
        if base_cases - current_cases:
            errors.append(f"declarative row regression: {sorted(base_cases - current_cases)}")
        if base_negatives - current_negatives:
            errors.append(
                f"declarative hard-negative regression: {sorted(base_negatives - current_negatives)}"
            )
    if errors:
        raise ValueError("\n".join(errors))


def validate(registry: dict, evidence: dict, blind: dict, declarative: dict) -> dict:
    errors: list[str] = []
    if registry.get("schema_version") != 1:
        errors.append("registry schema_version must be 1")
    if set(registry.get("attacker_modes", {})) != {"blind", "informed"}:
        errors.append("registry must distinguish blind and informed attacker modes")

    rows = evidence.get("evidence", [])
    oracle_rows = evidence.get("oracle", [])
    row_keys = [(row.get("gen_axis"), row.get("language")) for row in rows]
    if len(row_keys) != len(set(row_keys)):
        errors.append("evidence has duplicate producer/language rows")
    bad_sources = sorted(
        {row.get("source") for row in rows if row.get("source") not in {"probe", "sweep"}},
        key=str,
    )
    if bad_sources:
        errors.append(f"evidence has unsupported sources: {bad_sources}")

    expected_sweep = set(coverage_sweep.generatable_axes())
    actual_oracle = {row.get("gen_axis") for row in oracle_rows}
    if actual_oracle != expected_sweep:
        errors.append(
            "sweep producer inventory mismatch: "
            f"missing={sorted(expected_sweep - actual_oracle)}, "
            f"extra={sorted(actual_oracle - expected_sweep, key=str)}"
        )
    actual_sweep_rows = {
        row.get("gen_axis") for row in rows if row.get("source") == "sweep"
    }
    if actual_sweep_rows - expected_sweep:
        errors.append(
            "sweep evidence inventory mismatch: "
            f"extra={sorted(actual_sweep_rows - expected_sweep, key=str)}"
        )
    expected_probes = expected_probe_cells()
    actual_probes = {
        (row.get("gen_axis"), row.get("language"))
        for row in rows
        if row.get("source") == "probe"
    }
    if actual_probes != expected_probes:
        errors.append(
            "probe evidence inventory mismatch: "
            f"missing={sorted(expected_probes - actual_probes)}, "
            f"extra={sorted(actual_probes - expected_probes, key=str)}"
        )
    false_merges = [
        f"{row.get('axis')}/{row.get('language')}"
        for row in rows
        if row.get("false_merges", 0) > 0
    ]
    oracle_merges = [
        row.get("axis") for row in oracle_rows if row.get("hard_negatives_merged", 0) > 0
    ]
    if false_merges:
        errors.append(f"informed hard negatives merged: {sorted(false_merges)}")
    if oracle_merges:
        errors.append(f"sweep oracle hard negatives merged: {sorted(oracle_merges)}")

    tax_by_id = taxonomy.axis_index()
    cells = aggregate_cells(rows)
    runtime_exact: dict[str, set[str]] = {}
    for (axis_id, language), cell in cells.items():
        if covered_cell(cell):
            runtime_exact.setdefault(axis_id, set()).add(language)

    unknown_exact = sorted(set(runtime_exact) - set(tax_by_id))
    if unknown_exact:
        errors.append(f"exact evidence uses unregistered taxonomy axes: {unknown_exact}")

    required = {
        axis["axis_id"]
        for axis in taxonomy.AXES
        if axis["family"] != "soundness" and axis["feasibility"] == "landed"
    }
    required.update(
        axis_id
        for axis_id, languages in runtime_exact.items()
        if languages
        and axis_id in tax_by_id
        and tax_by_id[axis_id]["family"] != "soundness"
    )

    claims = registry.get("axes", [])
    claim_by_axis: dict[str, dict] = {}
    claim_ids: set[str] = set()
    group_ids: set[str] = set()
    for claim in claims:
        axis_id = claim.get("axis_id")
        if axis_id in claim_by_axis:
            errors.append(f"duplicate axis claim: {axis_id}")
            continue
        claim_by_axis[axis_id] = claim
        claim_id = claim.get("claim_id")
        if not isinstance(claim_id, str) or not claim_id.startswith("nose.type4."):
            errors.append(f"{axis_id}: invalid claim_id")
        elif claim_id in claim_ids:
            errors.append(f"duplicate claim_id: {claim_id}")
        claim_ids.add(claim_id)
        group_id = claim.get("hard_negative_group")
        expected_group = f"axis.{axis_id}.adjacent"
        if group_id != expected_group:
            errors.append(
                f"{axis_id}: hard_negative_group must be the linked group {expected_group}"
            )
        elif group_id in group_ids:
            errors.append(f"duplicate hard_negative_group: {group_id}")
        group_ids.add(group_id)
        declared_producers = claim.get("evidence_producers", [])
        observed_producers = sorted(
            {row.get("gen_axis") for row in rows if row.get("axis") == axis_id}
        )
        if declared_producers != observed_producers:
            errors.append(
                f"{axis_id}: evidence producer linkage mismatch; "
                f"declared={declared_producers}, observed={observed_producers}"
            )

    if set(claim_by_axis) != required:
        errors.append(
            "axis registry mismatch: "
            f"missing={sorted(required - set(claim_by_axis))}, "
            f"extra={sorted(set(claim_by_axis) - required)}"
        )

    credited_cells: set[tuple[str, str]] = set()
    for axis_id in sorted(set(claim_by_axis) & set(tax_by_id)):
        claim = claim_by_axis[axis_id]
        axis = tax_by_id[axis_id]
        if claim.get("risk_tier") != "A":
            errors.append(f"{axis_id}: every exact axis claim must be Tier A")
        boundaries = claim.get("boundary_classes", [])
        if not boundaries or not set(boundaries) <= BOUNDARY_CLASSES:
            errors.append(f"{axis_id}: invalid boundary_classes {boundaries}")
        if len(boundaries) != len(set(boundaries)):
            errors.append(f"{axis_id}: duplicate boundary class")

        exact = claim.get("exact_languages", [])
        closed = claim.get("closed_languages", [])
        if len(exact) != len(set(exact)) or len(closed) != len(set(closed)):
            errors.append(f"{axis_id}: duplicate language entry")
        exact_set, closed_set = set(exact), set(closed)
        applicable = set(axis["languages"])
        if exact_set & closed_set:
            errors.append(f"{axis_id}: exact and closed languages overlap")
        if exact_set | closed_set != applicable:
            errors.append(
                f"{axis_id}: language partition mismatch; "
                f"missing={sorted(applicable - exact_set - closed_set)}, "
                f"extra={sorted((exact_set | closed_set) - applicable)}"
            )

        observed = runtime_exact.get(axis_id, set()) & applicable
        if exact_set != observed:
            errors.append(
                f"{axis_id}: declared exact languages {sorted(exact_set)} "
                f"!= executable evidence {sorted(observed)}"
            )
        for language in exact:
            cell = cells.get((axis_id, language))
            if not cell or not covered_cell(cell):
                errors.append(
                    f"{axis_id}/{language}: not every positive producer passed with a hard negative"
                )
            credited_cells.add((axis_id, language))

    corpus_sha256, corpus_files = coverage_probe.corpus_identity(coverage_probe.PROBES)
    crates_proc = subprocess.run(
        ["git", "rev-parse", "HEAD:crates"],
        cwd=coverage_probe.REPO_ROOT,
        capture_output=True,
        text=True,
    )
    crates_tree = crates_proc.stdout.strip() if crates_proc.returncode == 0 else ""
    if (
        blind.get("schema_version") != 1
        or blind.get("attacker") != "blind-oracle"
        or blind.get("corpus") != "bench/type4/coverage_probes"
        or blind.get("corpus_sha256") != corpus_sha256
        or blind.get("corpus_files") != corpus_files
        or blind.get("product_crates_tree") != crates_tree
        or blind.get("product_dependencies") != coverage_probe.dependency_identity()
    ):
        errors.append("blind attacker receipt identity is invalid")
    summary = blind.get("summary", {})
    exclusions = blind.get("oracle_exclusions", {})
    summary_counts_valid = all(
        isinstance(summary.get(field), int) and summary[field] >= 0
        for field in ("total_units", "interpretable_units", "excluded_units")
    )
    exclusion_counts_valid = isinstance(exclusions, dict) and all(
        isinstance(value, int) and value >= 0 for value in exclusions.values()
    )
    if (
        not summary_counts_valid
        or summary.get("total_units", 0) <= 0
        or summary.get("interpretable_units", -1) + summary.get("excluded_units", -1)
        != summary.get("total_units")
        or not exclusion_counts_valid
        or sum(exclusions.values()) != summary.get("excluded_units")
    ):
        errors.append("blind attacker summary/exclusion arithmetic is invalid")
    hard_gate = blind.get("hard_gate", {})
    if (
        not hard_gate.get("gate_passed")
        or hard_gate.get("fingerprint_groups", 0) <= 0
        or hard_gate.get("false_merges") != 0
        or hard_gate.get("canon_preservation_violations") != 0
    ):
        errors.append(f"blind attacker hard gate failed: {hard_gate}")

    declarative_cases = declarative.get("cases", [])
    declarative_ids: set[str] = set()
    declarative_negative_ids: set[str] = set()
    declarative_coordinates: set[tuple[str, str, str, str]] = set()
    declarative_negatives = 0
    if declarative.get("schema_version") != 1 or len(declarative_cases) < 19:
        errors.append("declarative matrix must retain at least 19 connected rows")
    for case in declarative_cases:
        case_id = case.get("id")
        coordinate = tuple(
            case.get(field, "")
            for field in ("domain", "canonical_rule", "property_family", "boundary")
        )
        if not isinstance(case_id, str) or not case_id:
            errors.append("declarative case has no stable id")
        elif case_id in declarative_ids:
            errors.append(f"duplicate declarative id: {case_id}")
        declarative_ids.add(case_id)
        if coordinate[0] not in {"css", "html"} or not all(coordinate):
            errors.append(f"{case_id}: invalid declarative coordinate {coordinate}")
        elif coordinate in declarative_coordinates:
            errors.append(f"{case_id}: duplicate declarative coordinate")
        declarative_coordinates.add(coordinate)
        positives = case.get("positives", [])
        negatives = case.get("hard_negatives", [])
        negative_ids = case.get("hard_negative_ids", [])
        if len(positives) < 2 or not all(isinstance(value, str) and value for value in positives):
            errors.append(f"{case_id}: needs at least two positive source spellings")
        if not negatives or not all(isinstance(value, str) and value for value in negatives):
            errors.append(f"{case_id}: needs adjacent hard-negative source")
        if len(negative_ids) != len(negatives) or not all(
            isinstance(value, str) and value.startswith(f"{case_id}.hn.")
            for value in negative_ids
        ):
            errors.append(f"{case_id}: hard negatives need stable linked IDs")
        for negative_id in negative_ids:
            if negative_id in declarative_negative_ids:
                errors.append(f"duplicate declarative hard-negative id: {negative_id}")
            declarative_negative_ids.add(negative_id)
        declarative_negatives += len(negatives)
    if declarative_negatives < 25:
        errors.append("declarative matrix must retain at least 25 hard negatives")

    if errors:
        raise ValueError("\n".join(errors))
    return {
        "axes": len(claim_by_axis),
        "cells": len(credited_cells),
        "closed_cells": sum(len(c["closed_languages"]) for c in claims),
        "informed_false_merges": 0,
        "blind_false_merges": 0,
        "canon_violations": 0,
        "declarative_rows": len(declarative_cases),
        "declarative_negatives": declarative_negatives,
    }


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def load_at_revision(revision: str, path: Path) -> dict | None:
    relative = path.relative_to(coverage_probe.REPO_ROOT)
    proc = subprocess.run(
        ["git", "show", f"{revision}:{relative}"],
        cwd=coverage_probe.REPO_ROOT,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return None
    return json.loads(proc.stdout)


def execute_declarative_matrix(nose: Path, declarative: dict) -> None:
    failures: list[str] = []
    hard_negatives = 0
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for index, case in enumerate(declarative["cases"]):
            case_root = root / f"case-{index:02d}"
            case_root.mkdir()
            extension = "css" if case["domain"] == "css" else "html"
            positive_names = set()
            for pos_index, source in enumerate(case["positives"]):
                name = f"positive-{pos_index}.{extension}"
                (case_root / name).write_text(source)
                positive_names.add(name)
            negative_names = set()
            for neg_index, source in enumerate(case["hard_negatives"]):
                name = f"negative-{neg_index}.{extension}"
                (case_root / name).write_text(source)
                negative_names.add(name)
            hard_negatives += len(negative_names)

            proc = subprocess.run(
                [
                    str(nose),
                    "query",
                    str(case_root),
                    "all",
                    "witness=exact",
                    "--mode",
                    "semantic",
                    "--format",
                    "json",
                    "--min-size",
                    "1",
                ],
                capture_output=True,
                text=True,
            )
            if proc.returncode != 0:
                failures.append(f"{case['id']}: nose query failed: {proc.stderr[-500:]}")
                continue
            families = json.loads(proc.stdout or "{}").get("families", [])
            root_name = None
            if case["domain"] == "html":
                match = re.match(r"\s*<([A-Za-z][A-Za-z0-9:_-]*)", case["positives"][0])
                if not match:
                    failures.append(f"{case['id']}: cannot identify the root HTML element")
                    continue
                root_name = match.group(1).lower()
            member_sets = [
                {
                    Path(location["file"]).name
                    for location in family.get("locations", [])
                    if root_name is None or location.get("name", "").lower() == root_name
                }
                for family in families
                if family.get("witness") == "exact"
            ]
            if not any(positive_names <= members for members in member_sets):
                failures.append(f"{case['id']}: positive spellings did not converge")
            merged = sorted(
                name
                for name in negative_names
                if any(name in members and members & positive_names for members in member_sets)
            )
            if merged:
                failures.append(f"{case['id']}: hard negatives merged: {merged}")

    if failures:
        raise ValueError("\n".join(failures))
    print(
        "declarative executable matrix: "
        f"{len(declarative['cases'])}/{len(declarative['cases'])} rows converged, "
        f"{hard_negatives}/{hard_negatives} adjacent hard negatives stayed distinct"
    )
