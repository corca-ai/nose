#!/usr/bin/env python3
"""Validate the executable Tier-A Type-4 axis/language claim perimeter."""

from __future__ import annotations

import argparse
import copy
import json
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

BOUNDARY_CLASSES = {"domain", "effect", "protocol"}


def covered_row(row: dict) -> bool:
    return (
        row.get("status") == "covered"
        and row.get("pos", 0) > 0
        and row.get("pos_hit") == row.get("pos")
        and row.get("neg", 0) > 0
        and row.get("false_merges") == 0
    )


def validate(registry: dict, evidence: dict, blind: dict, declarative: dict) -> dict:
    errors: list[str] = []
    if registry.get("schema_version") != 1:
        errors.append("registry schema_version must be 1")
    if set(registry.get("attacker_modes", {})) != {"blind", "informed"}:
        errors.append("registry must distinguish blind and informed attacker modes")

    rows = evidence.get("evidence", [])
    oracle_rows = evidence.get("oracle", [])
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
    runtime_exact: dict[str, set[str]] = {}
    for row in rows:
        if covered_row(row):
            runtime_exact.setdefault(row["axis"], set()).add(row["language"])

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
        if not isinstance(group_id, str) or not group_id.startswith("axis."):
            errors.append(f"{axis_id}: invalid hard_negative_group")
        elif group_id in group_ids:
            errors.append(f"duplicate hard_negative_group: {group_id}")
        group_ids.add(group_id)

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
            qualifying = [
                row
                for row in rows
                if row.get("axis") == axis_id
                and row.get("language") == language
                and covered_row(row)
            ]
            if not qualifying:
                errors.append(f"{axis_id}/{language}: no positive + hard-negative evidence")
            credited_cells.add((axis_id, language))

    if blind.get("schema_version") != 1 or blind.get("attacker") != "blind-oracle":
        errors.append("blind attacker receipt identity is invalid")
    hard_gate = blind.get("hard_gate", {})
    if (
        not hard_gate.get("gate_passed")
        or hard_gate.get("false_merges") != 0
        or hard_gate.get("canon_preservation_violations") != 0
    ):
        errors.append(f"blind attacker hard gate failed: {hard_gate}")

    declarative_cases = declarative.get("cases", [])
    declarative_ids: set[str] = set()
    declarative_coordinates: set[tuple[str, str, str, str]] = set()
    declarative_negatives = 0
    if declarative.get("schema_version") != 1 or len(declarative_cases) <= 13:
        errors.append("declarative matrix must expand the former 13 positive groups")
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
        if len(positives) < 2 or not all(isinstance(value, str) and value for value in positives):
            errors.append(f"{case_id}: needs at least two positive source spellings")
        if not negatives or not all(isinstance(value, str) and value for value in negatives):
            errors.append(f"{case_id}: needs adjacent hard-negative source")
        declarative_negatives += len(negatives)
    if declarative_negatives <= 14:
        errors.append("declarative matrix must expand the former 14 hard negatives")

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


def expect_invalid(
    registry: dict, evidence: dict, blind: dict, declarative: dict, label: str
) -> None:
    try:
        validate(registry, evidence, blind, declarative)
    except ValueError:
        return
    raise AssertionError(f"self-test mutation unexpectedly passed: {label}")


def self_test(registry: dict, evidence: dict, blind: dict, declarative: dict) -> None:
    validate(registry, evidence, blind, declarative)

    missing = copy.deepcopy(registry)
    missing["axes"].pop()
    expect_invalid(missing, evidence, blind, declarative, "unregistered exact axis")

    unguarded = copy.deepcopy(evidence)
    first = registry["axes"][0]
    for row in unguarded["evidence"]:
        if row.get("axis") == first["axis_id"] and row.get("language") in first["exact_languages"]:
            row["neg"] = 0
    expect_invalid(registry, unguarded, blind, declarative, "exact cell without hard negative")

    drift = copy.deepcopy(registry)
    first_closed = next(claim for claim in drift["axes"] if claim["closed_languages"])
    language = first_closed["closed_languages"].pop()
    first_closed["exact_languages"].append(language)
    expect_invalid(drift, evidence, blind, declarative, "unsupported exact-language claim")

    blind_failure = copy.deepcopy(blind)
    blind_failure["hard_gate"]["false_merges"] = 1
    expect_invalid(registry, evidence, blind_failure, declarative, "blind false merge")

    disconnected = copy.deepcopy(declarative)
    disconnected["cases"][0]["hard_negatives"] = []
    expect_invalid(registry, evidence, blind, disconnected, "disconnected declarative row")

    unknown = copy.deepcopy(evidence)
    unknown["evidence"].append(
        {
            "axis": "unregistered_exact_axis",
            "gen_axis": "unregistered_exact_axis",
            "language": "rust",
            "status": "covered",
            "pos_hit": 1,
            "pos": 1,
            "false_merges": 0,
            "neg": 1,
            "source": "sweep",
        }
    )
    expect_invalid(registry, unknown, blind, declarative, "unknown exact taxonomy axis")


def load(path: Path) -> dict:
    return json.loads(path.read_text())


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
            member_sets = [
                {Path(location["file"]).name for location in family.get("locations", [])}
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--nose", type=Path, help="execute the CSS/HTML matrix with this binary")
    args = parser.parse_args()
    registry, evidence, blind, declarative = (
        load(REGISTRY),
        load(EVIDENCE),
        load(BLIND_RECEIPT),
        load(DECLARATIVE_MATRIX),
    )
    if args.self_test:
        self_test(registry, evidence, blind, declarative)
        print("axis-language claim checker self-test: ok")
        return 0
    try:
        summary = validate(registry, evidence, blind, declarative)
        if args.nose:
            execute_declarative_matrix(args.nose, declarative)
    except ValueError as exc:
        print(f"axis-language claim gate failed:\n{exc}", file=sys.stderr)
        return 1
    print(
        "axis-language claim gate: "
        f"{summary['axes']} Tier-A axes, {summary['cells']} exact cells, "
        f"{summary['closed_cells']} explicit closed cells; "
        f"declarative {summary['declarative_rows']} rows/"
        f"{summary['declarative_negatives']} hard negatives; "
        "0 informed/blind false merges, 0 canon violations"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
