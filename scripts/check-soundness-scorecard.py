#!/usr/bin/env python3
"""Freeze and verify the non-gameable Soundness Lab 0.19 scorecard."""

from __future__ import annotations

import argparse
import copy
import hashlib
import itertools
import json
import math
import os
import re
import shutil
import subprocess
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

SCHEMA = "nose-soundness-scorecard/v1"
COHORT_SCHEMA = "nose-soundness-cohort/v1"
OVERLAY_SCHEMA = "nose-soundness-scorecard-overlay/v2"
EXPANSION_SCHEMA = "nose-soundness-oracle-expansion/v1"
PERFORMANCE_SCHEMA = "nose.issue-859.performance/v1"
RELEASE_BINDING_SCHEMA = "nose-soundness-release-binding/v1"
CURRENT_EXCLUSION_SCHEMA = "nose-soundness-current-exclusion-attribution/v1"
CLAIM_ID = "nose.strict-exact.value-fingerprint/v0.19.0"
PAIR_CAP = 8
PPM = 1_000_000
RISK = {"tier-a": 5, "tier-b": 3, "tier-c": 1}
ROOT = Path(__file__).resolve().parents[1]
OFFICIAL_RELEASE_COMMIT = "0985e6963c58d5a97e523bc532b88aa5e34f2ef9"
HISTORICAL_ANCHOR_COMMIT = "bc4e9bc953ce3e2d11a13aebee33b5bced6258fe"
OFFICIAL_CRATES_TREE = "f57b078517fcd114657dfb90a2b72f44bfb6cafb"
OFFICIAL_ASSET_SHA256 = "097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981"
OFFICIAL_BINARY_SHA256 = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
HISTORICAL_BINARY_SHA256 = "559e1d565e231dda5ac7abacd749a5aac6337b63aef3bff008284e705096a25d"
HISTORICAL_CODE_SHA256 = "825a9c12c6ca260f500bb2dfa7a1d6ec9a9ed31160d8364e346b587c06500a8c"
OFFICIAL_REPORT_SHA256 = "149abb80c7ffb790d3fc0fbc2ad910add776d768ecf2961ee4321239f935d9c9"
INSTRUMENTED_UNITS_SHA256 = "c54efab792dbdcd2ec319e3903558cecd5384796962686cdff8abeaf1227cf91"
CORPUS_SELECTION_SHA256 = "ece26b22cf7662592cfc1e051f778b4dc73c3eb9899e3f47749464dc6c5ce9ac"
EXPECTED_RELEASE_METRICS = {
    "summary": {
        "total_units": 7835, "interpretable_units": 1122, "excluded_units": 6713,
        "canon_checked": 117, "canon_preservation_violations": 0,
        "admission_rejections": 929,
    },
    "soundness_gate": {
        "fingerprint_groups": 46, "false_merges": 0,
        "lossy_fingerprint_collisions": 0, "advisory_disagreements": 6,
        "canon_preservation_violations": 0, "max_violations": 0, "gate_passed": True,
    },
    "completeness": {
        "behavior_groups": 5, "behavior_equal_pairs": 180,
        "fingerprint_equal_pairs": 63, "completeness_percent": 35.0,
        "under_merged_behavior_groups": 2, "structurally_near_under_merged_groups": 0,
    },
    "exclusion_classification": {
        "missing-oracle-support": 6054, "semantic-boundary-attributed": 652,
        "path-exploration-budget": 5, "oracle-cost-budget": 1,
        "empty-value-fingerprint": 1,
    },
}


def release_binding_path() -> Path:
    current = ROOT / "bench/soundness/0.20.0/release-binding-943.v1.json"
    historical = ROOT / "bench/soundness/0.20.0/release-binding-862.v1.json"
    return current if current.is_file() else historical


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def pretty_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def identity(*parts: object) -> str:
    digest = hashlib.sha256()
    for part in parts:
        digest.update(str(part).encode())
        digest.update(b"\0")
    return digest.hexdigest()


def load(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def git_rev_parse(spec: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", spec], cwd=ROOT,
        text=True, capture_output=True, check=False,
    )
    if result.returncode != 0:
        raise ValueError(f"cannot resolve Git identity {spec}: {result.stderr.strip()}")
    return result.stdout.strip()


def language(path: str) -> str:
    ext = Path(path).suffix.lower()
    return {
        ".c": "c", ".h": "c", ".go": "go", ".java": "java",
        ".js": "javascript", ".jsx": "javascript", ".mjs": "javascript",
        ".cjs": "javascript", ".py": "python", ".rb": "ruby",
        ".rs": "rust", ".swift": "swift", ".ts": "typescript",
        ".tsx": "typescript", ".mts": "typescript", ".cts": "typescript",
    }.get(ext, "unknown")


def source_slice(root: Path, relpath: str, start: int, end: int) -> bytes:
    path = (root / relpath).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"source path escapes root: {relpath}") from error
    return source_slice_bytes(path.read_bytes(), relpath, start, end)


def source_slice_bytes(data: bytes, relpath: str, start: int, end: int) -> bytes:
    lines = data.splitlines(keepends=True)
    if start < 1 or end < start or end > len(lines):
        raise ValueError(f"invalid source span: {relpath}:{start}-{end}")
    return b"".join(lines[start - 1:end])


class GitSource:
    def __init__(self, repo_pin: str) -> None:
        self.repo_pin = repo_pin
        self.cache: dict[str, bytes] = {}

    def file(self, relpath: str) -> bytes:
        if relpath not in self.cache:
            if Path(relpath).is_absolute() or ".." in Path(relpath).parts:
                raise ValueError(f"invalid repository source path: {relpath}")
            result = subprocess.run(
                ["git", "show", f"{self.repo_pin}:{relpath}"],
                cwd=ROOT, capture_output=True, check=False,
            )
            if result.returncode != 0:
                detail = result.stderr.decode(errors="replace").strip()
                raise ValueError(f"source is not present at {self.repo_pin}: {relpath}: {detail}")
            self.cache[relpath] = result.stdout
        return self.cache[relpath]

    def slice(self, relpath: str, start: int, end: int) -> bytes:
        return source_slice_bytes(self.file(relpath), relpath, start, end)


def unit_identity(unit: dict[str, Any]) -> str:
    return identity(
        "nose-soundness-unit-v1", unit["repo_pin"], unit["path"],
        unit["start_line"], unit["end_line"], unit["source_sha256"],
        unit["core_il_sha256"], unit["claim_id"],
    )


def location_identity(member: dict[str, Any], repo_pin: str) -> str:
    return identity(
        "nose-soundness-location-v1", repo_pin, member["path"],
        member["start_line"], member["end_line"], member["source_sha256"], CLAIM_ID,
    )


def normalize_units(raw: dict[str, Any], source_root: Path, repo_pin: str) -> dict[str, Any]:
    units = []
    for record in raw["units"]:
        fp = record["value_fingerprint"]
        core_hash = sha256_bytes(canonical_bytes(fp))
        src_hash = sha256_bytes(source_slice(
            source_root, record["file"], record["start_line"], record["end_line"]
        ))
        unit = {
            "repo_pin": repo_pin,
            "path": record["file"],
            "start_line": record["start_line"],
            "end_line": record["end_line"],
            "language": language(record["file"]),
            "source_sha256": src_hash,
            "core_il_sha256": core_hash,
            "claim_id": CLAIM_ID,
            "claimable": record["claimable"],
            "canon_exposed": record["canon_exposed"],
            "domain_signature": record["domain_signature"],
            "domain_hosted": record.get("domain_hosted"),
            "input_projections": record.get("input_projections", []),
            "behavior_hash": record["behavior"],
            "trivial_behavior": record["trivial"],
            "symbolic_behavior": record["symbolic"],
            "constructs": sorted(record["constructs"]),
            "value_fingerprint": fp,
        }
        unit["unit_id"] = unit_identity(unit)
        units.append(unit)
    unique: dict[str, dict[str, Any]] = {}
    for unit in units:
        previous = unique.get(unit["unit_id"])
        if previous is None:
            unique[unit["unit_id"]] = unit
            continue
        previous_without_tags = dict(previous, constructs=[])
        unit_without_tags = dict(unit, constructs=[])
        if previous_without_tags != unit_without_tags:
            raise ValueError(f"ambiguous duplicate unit identity: {unit['unit_id']}")
        previous["constructs"] = sorted(
            set(previous["constructs"]) | set(unit["constructs"])
        )
    units = sorted(unique.values(), key=lambda row: row["unit_id"])
    return {
        "schema": COHORT_SCHEMA,
        "cohort_id": "nose-0.19.0-crates-exact-claim",
        "claim_id": CLAIM_ID,
        "identity_algorithm": "sha256-nul-v1",
        "core_il_algorithm": "sha256-canonical-value-fingerprint-v1",
        "source_algorithm": "sha256-exact-source-lines-v1",
        "repo_pin": repo_pin,
        "units": units,
    }


def construct_family(constructs: set[str], missing: bool) -> tuple[str, str]:
    if missing:
        return "oracle-gap", "tier-a"
    lowered = " ".join(sorted(constructs)).lower()
    if any(word in lowered for word in ("call:", "builtin:", "await", "yield", "throw")):
        return "runtime-protocol", "tier-a"
    if any(word in lowered for word in ("assign", "field", "index", "effect")):
        return "state-and-effects", "tier-a"
    if any(word in lowered for word in ("kind:if", "loop", "match", "switch", "select")):
        return "control-flow", "tier-b"
    if any(word in lowered for word in ("array", "map", "set", "collection", "iter")):
        return "collections", "tier-b"
    return "scalar-value", "tier-c"


def pair_status(left: dict[str, Any] | None, right: dict[str, Any] | None) -> str:
    if left is None or right is None:
        return "exact-unsafe-uninterpretable"
    if not left["claimable"] or not right["claimable"]:
        return "exact-unsafe-unclaimable"
    if left["symbolic_behavior"] or right["symbolic_behavior"]:
        return "exact-unsafe-symbolic"
    if left.get("domain_hosted") is False or right.get("domain_hosted") is False:
        return "exact-unsafe-unhosted-domain"
    if left["domain_signature"] != right["domain_signature"]:
        return "exact-unsafe-domain"
    if left["behavior_hash"] != right["behavior_hash"]:
        return "exact-unsafe-disagreement"
    return "exact-safe-verified"


def member_from_location(
    location: dict[str, Any], unit: dict[str, Any] | None, source_root: Path, repo_pin: str
) -> dict[str, Any]:
    member = {
        "path": location["file"],
        "start_line": location["start"],
        "end_line": location["end"],
        "source_sha256": sha256_bytes(source_slice(
            source_root, location["file"], location["start"], location["end"]
        )),
        "core_il_sha256": unit["core_il_sha256"] if unit else None,
        "unit_id": unit["unit_id"] if unit else None,
    }
    member["location_id"] = location_identity(member, repo_pin)
    return member


def build_pairs(
    query: dict[str, Any], cohort: dict[str, Any], source_root: Path
) -> list[dict[str, Any]]:
    repo_pin = cohort["repo_pin"]
    index = {
        (row["path"], row["start_line"], row["end_line"]): row
        for row in cohort["units"]
    }
    pairs = []
    for family in query["families"]:
        if family["witness"] != "exact":
            raise ValueError(f"non-exact family in semantic baseline: {family['id']}")
        family_pairs = []
        for left_loc, right_loc in itertools.combinations(family["locations"], 2):
            left = index.get((left_loc["file"], left_loc["start"], left_loc["end"]))
            right = index.get((right_loc["file"], right_loc["start"], right_loc["end"]))
            members = sorted([
                member_from_location(left_loc, left, source_root, repo_pin),
                member_from_location(right_loc, right, source_root, repo_pin),
            ], key=lambda row: row["location_id"])
            constructs = set(left["constructs"] if left else [])
            constructs.update(right["constructs"] if right else [])
            construct, tier = construct_family(constructs, left is None or right is None)
            langs = {language(left_loc["file"]), language(right_loc["file"])}
            row = {
                "family_id": family["id"],
                "claim_id": CLAIM_ID,
                "obligation_id": f"exact-soundness.{construct}",
                "language": next(iter(langs)) if len(langs) == 1 else "cross-language",
                "construct": construct,
                "risk_tier": tier,
                "status": pair_status(left, right),
                "members": members,
            }
            row["pair_id"] = identity(
                "nose-soundness-pair-v1", CLAIM_ID,
                members[0]["location_id"], members[1]["location_id"],
            )
            family_pairs.append(row)
        family_pairs.sort(key=lambda row: row["pair_id"])
        for offset, row in enumerate(family_pairs):
            row["family_cap_selected"] = offset < PAIR_CAP
        pairs.extend(family_pairs)
    pairs.sort(key=lambda row: row["pair_id"])
    return pairs


def cell_key(pair: dict[str, Any]) -> tuple[str, str, str, str]:
    return pair["claim_id"], pair["obligation_id"], pair["language"], pair["construct"]


def score_pairs(pairs: list[dict[str, Any]], baseline_ids: set[str]) -> dict[str, Any]:
    by_id = {row["pair_id"]: row for row in pairs}
    if len(by_id) != len(pairs):
        raise ValueError("duplicate pair identity in score input")
    missing = baseline_ids - set(by_id)
    if missing:
        raise ValueError(f"frozen baseline pair is missing: {sorted(missing)[0]}")
    if not baseline_ids:
        raise ValueError("frozen baseline pair set is empty")
    selected = [by_id[pair_id] for pair_id in sorted(baseline_ids)]
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in selected:
        grouped[cell_key(row)].append(row)
    cells = []
    for key, rows in sorted(grouped.items()):
        safe = sum(row["status"] == "exact-safe-verified" for row in rows)
        risk_tiers = {row["risk_tier"] for row in rows}
        if len(risk_tiers) != 1:
            raise ValueError(f"mixed risk tiers in cell {key}")
        tier = risk_tiers.pop()
        prevalence = 1 + math.ceil(math.log2(1 + len(rows)))
        weight = RISK[tier] * prevalence
        cells.append({
            "claim_id": key[0], "obligation_id": key[1], "language": key[2],
            "construct": key[3], "risk_tier": tier,
            "baseline_pair_mass": len(rows), "verified_pair_mass": safe,
            "exact_unsafe_pair_mass": len(rows) - safe,
            "coverage_ppm": safe * PPM // len(rows),
            "prevalence_weight": prevalence, "risk_weight": RISK[tier],
            "weight_units": weight,
        })
    denominator = sum(cell["weight_units"] for cell in cells)
    macro = sum(cell["weight_units"] * cell["coverage_ppm"] for cell in cells) // denominator
    total = len(selected)
    safe = sum(row["status"] == "exact-safe-verified" for row in selected)
    micro = safe * PPM // total
    by_language = {}
    for lang in sorted({cell["language"] for cell in cells}):
        language_cells = [cell for cell in cells if cell["language"] == lang]
        den = sum(cell["weight_units"] for cell in language_cells)
        by_language[lang] = {
            "macro_ppm": sum(cell["weight_units"] * cell["coverage_ppm"] for cell in language_cells) // den,
            "pair_micro_ppm": sum(cell["verified_pair_mass"] for cell in language_cells) * PPM
            // sum(cell["baseline_pair_mass"] for cell in language_cells),
        }
    target = min(PPM, macro + max(100_000, (PPM - macro) // 4))
    return {
        "cells": cells,
        "summary": {
            "macro_ppm": macro, "pair_micro_ppm": micro,
            "baseline_pair_mass": total, "verified_pair_mass": safe,
            "exact_unsafe_pair_mass": total - safe,
            "cell_count": len(cells), "by_language": by_language,
            "release_target_ppm": target,
        },
    }


def build_scorecard(query: dict[str, Any], cohort: dict[str, Any], source_root: Path) -> dict[str, Any]:
    pairs = build_pairs(query, cohort, source_root)
    baseline_ids = {
        pair["pair_id"] for pair in pairs if pair["family_cap_selected"]
    }
    scored = score_pairs(pairs, baseline_ids)
    return {
        "schema": SCHEMA,
        "baseline": "0.19.0",
        "claim_id": CLAIM_ID,
        "family_pair_cap": PAIR_CAP,
        "weight_contract": {
            "risk": RISK,
            "prevalence": "1 + ceil(log2(1 + capped baseline pair mass))",
            "macro": "risk/prevalence weighted cell coverage",
            "micro": "capped verified pairs / capped baseline pairs",
        },
        "baseline_pair_ids": sorted(baseline_ids),
        "pairs": pairs,
        **scored,
    }


def candidate_unit_at(
    index: dict[tuple[str, int, int], list[dict[str, Any]]],
    member: dict[str, Any],
) -> dict[str, Any] | None:
    candidates = index.get(
        (member["path"], member["start_line"], member["end_line"]), []
    )
    if len(candidates) <= 1:
        return candidates[0] if candidates else None
    matching = [
        row for row in candidates
        if row["core_il_sha256"] == member.get("core_il_sha256")
    ]
    if len(matching) == 1:
        return matching[0]
    raise ValueError(
        "candidate location has ambiguous executable evidence: "
        f"{member['path']}:{member['start_line']}-{member['end_line']}"
    )


def frozen_pair_status(
    left: dict[str, Any] | None, right: dict[str, Any] | None
) -> str:
    if left is None or right is None:
        return "exact-unsafe-uninterpretable"
    if left["core_il_sha256"] != right["core_il_sha256"]:
        return "exact-unsafe-fingerprint-drift"
    if left["symbolic_behavior"] or right["symbolic_behavior"]:
        return "exact-unsafe-symbolic"
    if left.get("domain_hosted") is not True or right.get("domain_hosted") is not True:
        return "exact-unsafe-unhosted-domain"
    if left["domain_signature"] != right["domain_signature"]:
        return "exact-unsafe-domain"
    if left["behavior_hash"] != right["behavior_hash"]:
        return "exact-unsafe-disagreement"
    return "exact-safe-verified"


def score_frozen_contract(
    frozen: dict[str, Any], statuses: dict[str, str]
) -> dict[str, Any]:
    selected_ids = set(frozen["baseline_pair_ids"])
    selected = {
        row["pair_id"]: row for row in frozen["pairs"]
        if row["pair_id"] in selected_ids
    }
    if set(selected) != selected_ids or set(statuses) != selected_ids:
        raise ValueError("candidate overlay does not cover the frozen pair ledger exactly")

    cells = []
    for frozen_cell in frozen["cells"]:
        key = (
            frozen_cell["claim_id"], frozen_cell["obligation_id"],
            frozen_cell["language"], frozen_cell["construct"],
        )
        pair_ids = [
            pair_id for pair_id, pair in selected.items() if cell_key(pair) == key
        ]
        if len(pair_ids) != frozen_cell["baseline_pair_mass"]:
            raise ValueError(f"frozen cell membership drifted: {key}")
        safe = sum(statuses[pair_id] == "exact-safe-verified" for pair_id in pair_ids)
        cell = copy.deepcopy(frozen_cell)
        cell["verified_pair_mass"] = safe
        cell["exact_unsafe_pair_mass"] = len(pair_ids) - safe
        cell["coverage_ppm"] = safe * PPM // len(pair_ids)
        cells.append(cell)

    denominator = sum(cell["weight_units"] for cell in cells)
    macro = sum(
        cell["weight_units"] * cell["coverage_ppm"] for cell in cells
    ) // denominator
    safe = sum(status == "exact-safe-verified" for status in statuses.values())
    by_language = {}
    for name in sorted({cell["language"] for cell in cells}):
        language_cells = [cell for cell in cells if cell["language"] == name]
        language_denominator = sum(cell["weight_units"] for cell in language_cells)
        by_language[name] = {
            "macro_ppm": sum(
                cell["weight_units"] * cell["coverage_ppm"]
                for cell in language_cells
            ) // language_denominator,
            "pair_micro_ppm": (
                sum(cell["verified_pair_mass"] for cell in language_cells) * PPM
                // sum(cell["baseline_pair_mass"] for cell in language_cells)
            ),
        }
    return {
        "cells": cells,
        "summary": {
            "macro_ppm": macro,
            "pair_micro_ppm": safe * PPM // len(statuses),
            "baseline_pair_mass": len(statuses),
            "verified_pair_mass": safe,
            "exact_unsafe_pair_mass": len(statuses) - safe,
            "cell_count": len(cells),
            "by_language": by_language,
            "release_target_ppm": frozen["summary"]["release_target_ppm"],
        },
    }


def frozen_contract_identity(frozen: dict[str, Any]) -> dict[str, Any]:
    keys = (
        "claim_id", "obligation_id", "language", "construct", "risk_tier",
        "baseline_pair_mass", "prevalence_weight", "risk_weight", "weight_units",
    )
    return {
        "scorecard_sha256": sha256_bytes(pretty_bytes(frozen)),
        "baseline_pair_ids_sha256": sha256_bytes(
            canonical_bytes(frozen["baseline_pair_ids"])
        ),
        "cell_contract_sha256": sha256_bytes(canonical_bytes([
            {key: cell[key] for key in keys} for cell in frozen["cells"]
        ])),
        "release_target_ppm": frozen["summary"]["release_target_ppm"],
    }


def overlay_gates(
    frozen: dict[str, Any], ledger: list[dict[str, Any]], scored: dict[str, Any]
) -> dict[str, Any]:
    regressions = sorted(
        row["pair_id"] for row in ledger
        if row["baseline_status"] == "exact-safe-verified"
        and row["candidate_status"] != "exact-safe-verified"
    )
    candidate_languages = scored["summary"]["by_language"]
    language_regressions = sorted(
        name for name, baseline in frozen["summary"]["by_language"].items()
        if candidate_languages[name]["macro_ppm"] < baseline["macro_ppm"]
        or candidate_languages[name]["pair_micro_ppm"] < baseline["pair_micro_ppm"]
    )
    target_met = (
        scored["summary"]["macro_ppm"] >= frozen["summary"]["release_target_ppm"]
    )
    denominator_unchanged = (
        scored["summary"]["baseline_pair_mass"]
        == frozen["summary"]["baseline_pair_mass"]
        and scored["summary"]["cell_count"] == frozen["summary"]["cell_count"]
    )
    return {
        "denominator_unchanged": denominator_unchanged,
        "baseline_verified_pair_regressions": regressions,
        "language_regressions": language_regressions,
        "release_target_met": target_met,
        "passed": (
            denominator_unchanged
            and not regressions
            and not language_regressions
            and target_met
        ),
    }


def build_overlay(frozen: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    by_location: dict[tuple[str, int, int], list[dict[str, Any]]] = defaultdict(list)
    for unit in candidate["units"]:
        location = (unit["path"], unit["start_line"], unit["end_line"])
        by_location[location].append(unit)

    selected_ids = set(frozen["baseline_pair_ids"])
    ledger = []
    statuses = {}
    for pair in frozen["pairs"]:
        if pair["pair_id"] not in selected_ids:
            continue
        units = [candidate_unit_at(by_location, member) for member in pair["members"]]
        status = frozen_pair_status(units[0], units[1])
        statuses[pair["pair_id"]] = status
        ledger.append({
            "pair_id": pair["pair_id"],
            "frozen_cell": {
                key: pair[key] for key in (
                    "claim_id", "obligation_id", "language", "construct", "risk_tier"
                )
            },
            "baseline_status": pair["status"],
            "candidate_status": status,
            "candidate_members": [
                None if unit is None else {
                    key: unit[key] for key in (
                        "unit_id", "core_il_sha256", "behavior_hash", "claimable",
                        "symbolic_behavior", "domain_signature", "domain_hosted",
                        "input_projections",
                    )
                }
                for unit in units
            ],
        })
    ledger.sort(key=lambda row: row["pair_id"])
    scored = score_frozen_contract(frozen, statuses)
    return {
        "schema": OVERLAY_SCHEMA,
        "claim_id": frozen["claim_id"],
        "repo_pin": candidate["repo_pin"],
        "frozen_contract": frozen_contract_identity(frozen),
        "candidate_cohort_sha256": sha256_bytes(pretty_bytes(candidate)),
        "pair_ledger": ledger,
        **scored,
        "gates": overlay_gates(frozen, ledger, scored),
    }


def verify_overlay(frozen: dict[str, Any], overlay: dict[str, Any]) -> None:
    selected = {
        row["pair_id"]: row for row in frozen["pairs"]
        if row["pair_id"] in set(frozen["baseline_pair_ids"])
    }
    ledger = overlay.get("pair_ledger")
    if (
        overlay.get("schema") != OVERLAY_SCHEMA
        or overlay.get("claim_id") != frozen["claim_id"]
        or overlay.get("repo_pin") != scorecard_repo_pin(frozen)
        or not isinstance(ledger, list)
        or ledger != sorted(ledger, key=lambda row: row["pair_id"])
        or len(ledger) != len(selected)
        or {row["pair_id"] for row in ledger} != set(selected)
    ):
        raise ValueError("candidate overlay is not bound to the frozen pair ledger")

    statuses = {}
    for row in ledger:
        pair = selected[row["pair_id"]]
        expected_cell = {
            key: pair[key] for key in (
                "claim_id", "obligation_id", "language", "construct", "risk_tier"
            )
        }
        members = row.get("candidate_members")
        if (
            row.get("frozen_cell") != expected_cell
            or row.get("baseline_status") != pair["status"]
            or not isinstance(members, list)
            or len(members) != 2
        ):
            raise ValueError(f"candidate overlay pair metadata drifted: {row['pair_id']}")
        status = frozen_pair_status(members[0], members[1])
        if row.get("candidate_status") != status:
            raise ValueError(
                f"candidate overlay status is not evidence-derived: {row['pair_id']}"
            )
        statuses[row["pair_id"]] = status

    candidate_sha = overlay.get("candidate_cohort_sha256")
    if not isinstance(candidate_sha, str) or len(candidate_sha) != 64:
        raise ValueError("candidate overlay cohort hash is malformed")
    scored = score_frozen_contract(frozen, statuses)
    expected = {
        "schema": OVERLAY_SCHEMA,
        "claim_id": frozen["claim_id"],
        "repo_pin": scorecard_repo_pin(frozen),
        "frozen_contract": frozen_contract_identity(frozen),
        "candidate_cohort_sha256": candidate_sha,
        "pair_ledger": ledger,
        **scored,
        "gates": overlay_gates(frozen, ledger, scored),
    }
    if overlay != expected:
        raise ValueError("candidate overlay aggregates or gates do not replay from its ledger")


def require_sha256(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(char not in "0123456789abcdef" for char in value)
    ):
        raise ValueError(f"{label} is not a SHA-256 digest")
    return value


def require_git_oid(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 40
        or any(char not in "0123456789abcdef" for char in value)
    ):
        raise ValueError(f"{label} is not a full Git object ID")
    return value


def checked_artifact(receipt_path: Path, relative: Any) -> Path:
    if not isinstance(relative, str) or not relative:
        raise ValueError("oracle expansion artifact path is missing")
    path = (receipt_path.parent / relative).resolve()
    try:
        path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError(f"oracle expansion artifact escapes the repository: {relative}") from error
    if not path.is_file():
        raise ValueError(f"oracle expansion artifact is missing: {relative}")
    return path


def parse_falsification(path: Path) -> tuple[dict[str, int], dict[str, int]]:
    text = path.read_text(encoding="utf-8")

    def captures(pattern: str, label: str) -> tuple[int, ...]:
        match = re.search(pattern, text)
        if match is None:
            raise ValueError(f"falsification output is missing {label}")
        return tuple(int(value) for value in match.groups())

    total, interpretable = captures(
        r"units: (\d+) total, (\d+) interpretable", "unit counts"
    )
    (canon_checked,) = captures(
        r"units checked \(interpretable both ways\): (\d+)", "canon count"
    )
    (advisory,) = captures(
        r"advisory \(symbolic or non-hard-domain disagreements[^\n]*\): (\d+)",
        "advisory count",
    )
    equal_pairs, behavior_pairs = captures(
        r"completeness: (\d+)/(\d+) =", "completeness counts"
    )
    (under_merged,) = captures(
        r"under-merged behavior groups \(missed clones\): (\d+)",
        "under-merged count",
    )
    (structurally_near,) = captures(
        r"of which structurally-near[^\n]*: (\d+)", "structurally-near count"
    )
    eligible, searched, skipped, cases = captures(
        r"eligible pairs: (\d+); searched: (\d+); skipped: (\d+); executed cases: (\d+)",
        "falsification search counts",
    )
    false_merges, max_violations = captures(
        r"GATE: (\d+) ≤ (\d+) false merges", "false-merge gate"
    )
    if "PRESERVED: every canon-changed unit computes the same thing" not in text:
        raise ValueError("falsification output does not preserve canonical behavior")
    if "no new distinguishers" not in text:
        raise ValueError("falsification output does not close the distinguishing-input search")
    return (
        {
            "advisory_disagreements": advisory,
            "canon_checked": canon_checked,
            "canon_preservation_violations": 0,
            "eligible_pairs": eligible,
            "executed_cases": cases,
            "false_merges": false_merges,
            "new_distinguishers": 0,
            "searched_pairs": searched,
            "skipped_pairs": skipped,
            "total_units": total,
            "interpretable_units": interpretable,
        },
        {
            "behavior_equal_pairs": behavior_pairs,
            "fingerprint_equal_pairs": equal_pairs,
            "structurally_near_under_merged_groups": structurally_near,
            "under_merged_behavior_groups": under_merged,
            "max_violations": max_violations,
        },
    )


def verify_performance_receipt(receipt: dict[str, Any], expansion: dict[str, Any]) -> None:
    if receipt.get("schema") != PERFORMANCE_SCHEMA or receipt.get("issue") != 859:
        raise ValueError("oracle expansion performance receipt has the wrong identity")
    baseline = receipt.get("baseline", {})
    candidate = receipt.get("candidate", {})
    if (
        baseline.get("source_sha") != expansion["release_tree"]
        or baseline.get("binary_sha256") != OFFICIAL_BINARY_SHA256
        or candidate.get("source_sha") != expansion["implementation_source_sha"]
        or candidate.get("crates_tree") != expansion["implementation_crates_tree"]
    ):
        raise ValueError("oracle expansion performance provenance drifted")
    for side in (baseline, candidate):
        require_sha256(side.get("binary_sha256"), "performance binary")
        require_sha256(side.get("binary_code_sha256"), "performance code identity")
    corpus = receipt.get("corpus", {})
    expected_repositories = [
        "alamofire", "guava", "hugo", "netty", "rxjava", "sqlalchemy", "sympy"
    ]
    if corpus.get("repositories") != expected_repositories:
        raise ValueError("oracle expansion performance corpus changed")
    for key, value in corpus.items():
        if key.endswith("sha256"):
            require_sha256(value, f"performance corpus {key}")

    query = receipt.get("default_query", {})
    parity = query.get("pre_issue_output_parity", {})
    focused = query.get("focused", {})
    if (
        parity.get("baseline_source_sha") != expansion["branch_base"]
        or parity.get("candidate_source_sha") != expansion["implementation_source_sha"]
        or parity.get("byte_identical_repositories") != parity.get("repository_count")
        or parity.get("repository_count") != len(expected_repositories)
        or focused.get("status") != "known-stage-regressions"
        or focused.get("tracked_by") != "https://github.com/corca-ai/nose/issues/892"
        or not focused.get("confirmed_stage_signals")
        or focused.get("aggregate_adjusted_delta_pct", math.inf) >= 5.0
    ):
        raise ValueError("oracle expansion performance conclusion drifted")
    swift = receipt.get("swift_verify", {})
    if (
        swift.get("repository") != "alamofire"
        or swift.get("verdict") != "below-50ms-materiality-threshold"
        or not 0 <= swift.get("adjusted_delta_ms", math.inf) < 50.0
    ):
        raise ValueError("oracle expansion Swift verify performance conclusion drifted")
    for section in (query.get("primary", {}), focused, parity, swift):
        for key, value in section.items():
            if key.endswith("sha256"):
                require_sha256(value, f"performance {key}")


def verify_expansion_receipt(
    frozen: dict[str, Any], overlay_path: Path, receipt_path: Path
) -> None:
    overlay = load(overlay_path)
    receipt = load(receipt_path)
    verify_overlay(frozen, overlay)
    if (
        receipt.get("schema") != EXPANSION_SCHEMA
        or receipt.get("issue") != 859
        or receipt.get("parent_issue") != 855
        or receipt.get("release_tree") != scorecard_repo_pin(frozen)
    ):
        raise ValueError("oracle expansion receipt has the wrong identity")
    require_git_oid(receipt.get("branch_base"), "oracle expansion branch base")
    require_git_oid(receipt.get("implementation_source_sha"), "oracle expansion source")
    implementation_tree = require_git_oid(
        receipt.get("implementation_crates_tree"), "oracle expansion crates tree"
    )
    if git_rev_parse(f"{receipt['implementation_source_sha']}:crates") != implementation_tree:
        raise ValueError("oracle expansion source does not match its measured crates tree")

    frozen_contract = receipt.get("frozen_contract", {})
    expected_contract = {
        "baseline_macro_ppm": frozen["summary"]["macro_ppm"],
        "baseline_pair_ids_sha256": sha256_bytes(canonical_bytes(frozen["baseline_pair_ids"])),
        "baseline_pair_mass": frozen["summary"]["baseline_pair_mass"],
        "baseline_pair_micro_ppm": frozen["summary"]["pair_micro_ppm"],
        "baseline_verified_pair_mass": frozen["summary"]["verified_pair_mass"],
        "release_target_ppm": frozen["summary"]["release_target_ppm"],
        "scorecard_sha256": sha256_file(
            ROOT / "bench/soundness/0.19.0/scorecard.v1.json"
        ),
    }
    if frozen_contract != expected_contract:
        raise ValueError("oracle expansion frozen contract drifted")

    artifacts = receipt.get("artifacts", {})
    loaded_artifacts = {}
    for name in ("overlay", "falsification", "performance", "expected_drift_manifest"):
        artifact = artifacts.get(name, {})
        path = checked_artifact(receipt_path, artifact.get("path"))
        if sha256_file(path) != require_sha256(
            artifact.get("sha256"), f"oracle expansion {name}"
        ):
            raise ValueError(f"oracle expansion {name} hash drifted")
        loaded_artifacts[name] = path
    if loaded_artifacts["overlay"] != overlay_path.resolve():
        raise ValueError("oracle expansion receipt points at a different overlay")

    summary = overlay["summary"]
    gates = overlay["gates"]
    expected_result = {
        "baseline_verified_pair_regressions": gates["baseline_verified_pair_regressions"],
        "candidate_cohort_sha256": overlay["candidate_cohort_sha256"],
        "denominator_unchanged": gates["denominator_unchanged"],
        "exact_unsafe_pair_mass": summary["exact_unsafe_pair_mass"],
        "gate_passed": gates["passed"],
        "language_regressions": gates["language_regressions"],
        "macro_ppm": summary["macro_ppm"],
        "pair_micro_ppm": summary["pair_micro_ppm"],
        "release_target_met": gates["release_target_met"],
        "verified_pair_mass": summary["verified_pair_mass"],
    }
    if receipt.get("result") != expected_result or not gates["passed"]:
        raise ValueError("oracle expansion result does not match its overlay")

    measured_falsification, measured_completeness = parse_falsification(
        loaded_artifacts["falsification"]
    )
    continuity = receipt.get("completeness_continuity", {})
    measured_completeness.pop("max_violations")
    if (
        receipt.get("falsification") != measured_falsification
        or continuity.get("final") != measured_completeness
        or continuity.get("frozen_release")
        != {
            "behavior_equal_pairs": EXPECTED_RELEASE_METRICS["completeness"]["behavior_equal_pairs"],
            "fingerprint_equal_pairs": EXPECTED_RELEASE_METRICS["completeness"]["fingerprint_equal_pairs"],
            "structurally_near_under_merged_groups": 0,
            "under_merged_behavior_groups": 2,
        }
        or not isinstance(continuity.get("reason"), str)
    ):
        raise ValueError("oracle expansion falsification or completeness receipt drifted")

    selected = set(frozen["baseline_pair_ids"])
    baseline_safe = {
        row["pair_id"] for row in frozen["pairs"]
        if row["pair_id"] in selected and row["status"] == "exact-safe-verified"
    }
    final_safe = {
        row["pair_id"] for row in overlay["pair_ledger"]
        if row["candidate_status"] == "exact-safe-verified"
    }
    previous_safe = set(baseline_safe)
    previous_macro = frozen["summary"]["macro_ppm"]
    expected_names = [
        "fragment-cardinality-projection",
        "immutable-swift-module-string",
        "unused-trailing-parameters",
    ]
    tranches = receipt.get("tranches")
    if not isinstance(tranches, list) or [row.get("name") for row in tranches] != expected_names:
        raise ValueError("oracle expansion tranche sequence changed")
    rust_sources = "\n".join(
        path.read_text(encoding="utf-8") for path in (ROOT / "crates").rglob("*.rs")
    )
    for order, tranche in enumerate(tranches, 1):
        new_ids = tranche.get("new_verified_pair_ids")
        if (
            tranche.get("implementation_order") != order
            or not isinstance(new_ids, list)
            or new_ids != sorted(new_ids)
            or len(new_ids) != len(set(new_ids))
            or set(new_ids) & previous_safe
            or not set(new_ids) <= final_safe
            or tranche.get("verified_pair_mass") != len(previous_safe) + len(new_ids)
            or tranche.get("macro_delta_ppm") != tranche.get("macro_ppm") - previous_macro
        ):
            raise ValueError(f"oracle expansion tranche accounting drifted: {tranche.get('name')}")
        for key in ("candidate_cohort_sha256", "overlay_sha256", "raw_units_sha256"):
            require_sha256(tranche.get(key), f"oracle expansion tranche {key}")
        for key in ("source_independent_positive", "adjacent_negative", "unsupported_boundary"):
            reference = tranche.get(key)
            symbol = reference.rsplit("::", 1)[-1] if isinstance(reference, str) else ""
            if not symbol or f"fn {symbol}(" not in rust_sources:
                raise ValueError(f"oracle expansion test reference is missing: {reference}")
        previous_safe.update(new_ids)
        previous_macro = tranche["macro_ppm"]
    if (
        previous_safe != final_safe
        or tranches[-1]["candidate_cohort_sha256"] != overlay["candidate_cohort_sha256"]
        or tranches[-1]["overlay_sha256"] != sha256_file(overlay_path)
        or tranches[-1]["macro_ppm"] != summary["macro_ppm"]
    ):
        raise ValueError("oracle expansion tranche chain does not reach the final overlay")

    performance = load(loaded_artifacts["performance"])
    verify_performance_receipt(performance, receipt)
    drift = load(loaded_artifacts["expected_drift_manifest"])
    entries = drift.get("entries")
    if (
        drift.get("schema") != "nose.semantic_regression_expected_drift.v1"
        or not isinstance(entries, list)
        or len(entries) != 5
        or any(row.get("baseline_source_sha") != receipt["release_tree"] for row in entries)
    ):
        raise ValueError("oracle expansion expected-drift manifest changed")


def verify_release_binding(
    frozen: dict[str, Any], binding_path: Path, release_commit: str | None = None
) -> dict[str, Any]:
    binding = load(binding_path)
    binding_issue = binding.get("issue")
    if (
        binding.get("schema") != RELEASE_BINDING_SCHEMA
        or binding_issue not in (862, 943)
        or binding.get("parent_issue") != 855
        or binding.get("release_tree") != scorecard_repo_pin(frozen)
    ):
        raise ValueError("release binding has the wrong identity")

    implementation = binding.get("implementation", {})
    source = require_git_oid(implementation.get("source_sha"), "release binding source")
    crates_tree = require_git_oid(
        implementation.get("crates_tree"), "release binding crates tree"
    )
    if git_rev_parse(f"{source}:crates") != crates_tree:
        raise ValueError("release binding source does not match its crates tree")
    if release_commit is not None:
        candidate = require_git_oid(release_commit, "release candidate commit")
        candidate_crates = git_rev_parse(f"{candidate}:crates")
        if candidate_crates != crates_tree:
            transition = binding.get("release_transition", {})
            allowed = transition.get("allowed_crates_changes", [])
            if (
                binding_issue != 943
                or transition.get("measured_crates_tree") != crates_tree
                or transition.get("release_crates_tree") != candidate_crates
                or transition.get("product_code_changed") is not False
                or not isinstance(allowed, list)
                or not allowed
            ):
                raise ValueError(
                    "release candidate product crates do not match the release binding"
                )
            completed = subprocess.run(
                ["git", "diff", "--name-only", source, candidate, "--", "crates"],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            if completed.returncode:
                raise ValueError("could not audit the release crates transition")
            changed = [line for line in completed.stdout.splitlines() if line]
            expected = [row.get("path") for row in allowed]
            if changed != expected or any(
                not isinstance(path, str)
                or ("/tests/" not in path and not path.endswith("/tests.rs"))
                for path in expected
            ):
                raise ValueError("release crates transition contains a non-test change")
            for row in allowed:
                path = row["path"]
                if (
                    git_rev_parse(f"{source}:{path}") != row.get("before_blob")
                    or git_rev_parse(f"{candidate}:{path}") != row.get("after_blob")
                    or not isinstance(row.get("reason"), str)
                    or not row["reason"]
                ):
                    raise ValueError("release crates transition blob identity drifted")
    require_sha256(implementation.get("binary_sha256"), "release binding binary")
    require_sha256(implementation.get("raw_units_sha256"), "release binding raw units")

    artifacts = binding.get("artifacts", {})
    loaded: dict[str, Path] = {}
    for name in (
        "blind_attack",
        "current_exclusion_attribution",
        "overlay",
        "falsification",
        "historical_overlay",
        "expansion_receipt",
        "performance",
    ):
        artifact = artifacts.get(name, {})
        path = checked_artifact(binding_path, artifact.get("path"))
        if sha256_file(path) != require_sha256(
            artifact.get("sha256"), f"release binding {name}"
        ):
            raise ValueError(f"release binding {name} hash drifted")
        loaded[name] = path

    attribution = load(loaded["current_exclusion_attribution"])
    attribution_summary = attribution.get("summary", {})
    if (
        attribution.get("schema") != CURRENT_EXCLUSION_SCHEMA
        or attribution.get("issue") != 862
        or attribution.get("parent_issue") != 855
        or attribution.get("source_sha") != source
        or attribution.get("crates_tree") != crates_tree
        or attribution.get("binary_sha256") != implementation["binary_sha256"]
        or attribution.get("raw_census", {}).get("schema")
        != "nose-oracle-exclusion-census/v2"
    ):
        raise ValueError("current exclusion attribution provenance drifted")
    require_sha256(
        attribution.get("raw_census", {}).get("sha256"),
        "current exclusion attribution raw census",
    )
    if (
        attribution_summary.get("generic_unattributed_exclusions") != 0
        or attribution_summary.get("total_units")
        != attribution_summary.get("interpretable_units", -1)
        + attribution_summary.get("excluded_units", -1)
        or sum(attribution_summary.get("by_classification", {}).values())
        != attribution_summary.get("excluded_units")
        or sum(attribution_summary.get("by_capability", {}).values())
        != attribution_summary.get("excluded_units")
    ):
        raise ValueError("current exclusion attribution counts are inconsistent")

    overlay = load(loaded["overlay"])
    historical = load(loaded["historical_overlay"])
    verify_overlay(frozen, overlay)
    verify_overlay(frozen, historical)

    def decision(value: dict[str, Any]) -> list[tuple[str, str, str]]:
        return [
            (row["pair_id"], row["baseline_status"], row["candidate_status"])
            for row in value["pair_ledger"]
        ]

    if (
        overlay["summary"] != historical["summary"]
        or overlay["gates"] != historical["gates"]
        or decision(overlay) != decision(historical)
        or binding.get("continuity", {}).get("score_and_pair_status_unchanged") is not True
    ):
        raise ValueError("release binding changed the frozen score or pair decisions")

    measured_falsification, measured_completeness = parse_falsification(
        loaded["falsification"]
    )
    measured_completeness.pop("max_violations")
    if (
        binding.get("falsification") != measured_falsification
        or binding.get("completeness") != measured_completeness
    ):
        raise ValueError("release binding falsification evidence drifted")

    summary = overlay["summary"]
    gates = overlay["gates"]
    expected_result = {
        "baseline_verified_pair_regressions": gates["baseline_verified_pair_regressions"],
        "candidate_cohort_sha256": overlay["candidate_cohort_sha256"],
        "denominator_unchanged": gates["denominator_unchanged"],
        "exact_unsafe_pair_mass": summary["exact_unsafe_pair_mass"],
        "gate_passed": gates["passed"],
        "language_regressions": gates["language_regressions"],
        "macro_ppm": summary["macro_ppm"],
        "pair_micro_ppm": summary["pair_micro_ppm"],
        "release_target_met": gates["release_target_met"],
        "verified_pair_mass": summary["verified_pair_mass"],
    }
    if binding.get("result") != expected_result or not gates["passed"]:
        raise ValueError("release binding result does not match its overlay")

    blind = load(loaded["blind_attack"])
    blind_gate = blind.get("hard_gate", {})
    if (
        blind.get("product_crates_tree") != crates_tree
        or blind_gate.get("gate_passed") is not True
        or blind_gate.get("false_merges") != 0
        or blind_gate.get("canon_preservation_violations") != 0
    ):
        raise ValueError("release binding Type-4 blind attack failed or drifted")

    performance = load(loaded["performance"])
    baseline = performance.get("baseline", {})
    candidate = performance.get("candidate", {})
    if binding_issue == 862:
        if (
            performance.get("schema") != "nose.issue-862.performance/v1"
            or performance.get("issue") != 862
            or performance.get("parent_issue") != 855
            or baseline.get("source_sha") != binding["release_tree"]
            or baseline.get("binary_sha256") != OFFICIAL_BINARY_SHA256
            or candidate.get("source_sha") != source
            or candidate.get("crates_tree") != crates_tree
            or candidate.get("binary_sha256") != implementation["binary_sha256"]
        ):
            raise ValueError("release performance provenance drifted")
        for side in (baseline, candidate):
            require_sha256(
                side.get("binary_code_sha256"), "release performance code identity"
            )
        expected_repositories = [
            "alamofire", "guava", "hugo", "netty", "rxjava", "sqlalchemy", "sympy"
        ]
        corpus = performance.get("corpus", {})
        if corpus.get("repositories") != expected_repositories:
            raise ValueError("release performance corpus changed")
        for key, value in corpus.items():
            if key.endswith("sha256"):
                require_sha256(value, f"release performance corpus {key}")
        measurement = performance.get("measurement", {})
        for key in ("primary_raw_sha256", "control_raw_sha256"):
            require_sha256(measurement.get(key), f"release performance {key}")
        baseline_ms = measurement.get("aggregate_baseline_median_ms", 0.0)
        raw_delta = measurement.get("aggregate_current_median_ms", 0.0) - baseline_ms
        adjusted = raw_delta - measurement.get("control_delta_ms", math.inf)
        adjusted_pct = adjusted / baseline_ms * 100.0 if baseline_ms > 0 else math.inf
        performance_gate = performance.get("gate", {})
        limit = performance_gate.get("adjusted_delta_pct_limit", -math.inf)
        if (
            measurement.get("iterations", 0) < 9
            or measurement.get("warmups", 0) < 2
            or not math.isclose(
                raw_delta, measurement.get("aggregate_raw_delta_ms", math.inf)
            )
            or not math.isclose(adjusted, measurement.get("adjusted_delta_ms", math.inf))
            or not math.isclose(
                adjusted_pct, measurement.get("adjusted_delta_pct", math.inf)
            )
            or adjusted_pct >= limit
            or performance_gate.get("passed") is not True
        ):
            raise ValueError("release performance gate failed or drifted")
    else:
        if (
            performance.get("schema") != "nose.release_query_performance/v1"
            or performance.get("issue") != 943
            or performance.get("release") != "0.20.0"
            or performance.get("status") != "pass"
            or baseline.get("source_sha") != binding["release_tree"]
            or baseline.get("binary_sha256") != OFFICIAL_BINARY_SHA256
            or baseline.get("published_and_checksum_verified") is not True
            or candidate.get("source_sha") != source
            or candidate.get("crates_tree") != crates_tree
            or candidate.get("binary_sha256") != implementation["binary_sha256"]
        ):
            raise ValueError("release query-performance provenance drifted")
        for side in (baseline, candidate):
            require_sha256(
                side.get("binary_code_sha256"), "release performance code identity"
            )
            require_sha256(side.get("archive_sha256"), "release archive identity")
        policy = performance.get("policy", {})
        if (
            policy.get("runtime") != "order-aware-v3"
            or policy.get("material_only_if_delta_pct_gt") != 5.0
            or policy.get("material_only_if_delta_ms_gt") != 5.0
            or policy.get("primary_blocks") != 5
            or policy.get("focused_blocks") != 6
            or policy.get("single_focused_rerun") is not True
        ):
            raise ValueError("release query-performance policy drifted")
        corpus = performance.get("corpus", {})
        if (
            corpus.get("manifest_sha256")
            != "87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8"
            or corpus.get("prune_manifest_sha256")
            != "c22f34d3ab4da9b89b5938140bbfdf7664178b3b7b57e5ea3937ba0bb47c2980"
            or corpus.get("expected_state_sha256")
            != "b28d7245aa34d7e0320d0c80ef988803ba6acb65b63eaf1bae6d7ac840b168e0"
            or corpus.get("base_workload_sha256")
            != "ed76a6a2b5b2551dfd61f627998c6db50e0be70fb479067ccabf7b42f97b2ad6"
            or corpus.get("repositories") != 120
            or corpus.get("base_repositories") != 17
        ):
            raise ValueError("release query-performance corpus drifted")
        workloads = performance.get("workloads", [])
        expected = {
            "base": 17,
            "default": 120,
            "semantic": 120,
            "near-no-pack": 120,
        }
        if [row.get("id") for row in workloads] != list(expected):
            raise ValueError("release query-performance workloads drifted")
        for row in workloads:
            measurement = row.get("measurement", {})
            if (
                row.get("repositories") != expected[row["id"]]
                or row.get("status") != "pass"
                or any(row.get("final_unresolved", {}).values())
                or measurement.get("iterations") != 5
                or measurement.get("samples_per_observation") != 5
            ):
                raise ValueError(f"release query-performance workload failed: {row['id']}")
            for evidence in row.get("artifacts", {}).values():
                require_sha256(evidence.get("sha256"), "release query artifact")
    return overlay


def verify_cohort(cohort: dict[str, Any]) -> None:
    if cohort.get("schema") != COHORT_SCHEMA:
        raise ValueError("unsupported cohort schema")
    source = GitSource(cohort["repo_pin"])
    ids = set()
    for unit in cohort["units"]:
        if unit["core_il_sha256"] != sha256_bytes(canonical_bytes(unit["value_fingerprint"])):
            raise ValueError(f"core IL hash mismatch: {unit['path']}:{unit['start_line']}")
        expected_source = sha256_bytes(source.slice(
            unit["path"], unit["start_line"], unit["end_line"]
        ))
        if unit["source_sha256"] != expected_source:
            raise ValueError(
                f"source hash is not bound to {cohort['repo_pin']}: "
                f"{unit['path']}:{unit['start_line']}"
            )
        if unit["unit_id"] != unit_identity(unit):
            raise ValueError(f"unit identity mismatch: {unit['path']}:{unit['start_line']}")
        if unit["unit_id"] in ids:
            raise ValueError(f"duplicate unit identity: {unit['unit_id']}")
        if (
            unit["repo_pin"] != cohort["repo_pin"]
            or unit["claim_id"] != CLAIM_ID
            or unit["language"] != language(unit["path"])
            or unit["constructs"] != sorted(unit["constructs"])
        ):
            raise ValueError(f"unit metadata is not canonical: {unit['unit_id']}")
        ids.add(unit["unit_id"])


def verify_scorecard(
    scorecard: dict[str, Any], cohort: dict[str, Any], query: dict[str, Any]
) -> None:
    if scorecard.get("schema") != SCHEMA or scorecard.get("family_pair_cap") != PAIR_CAP:
        raise ValueError("unsupported scorecard contract")
    repo_pin = scorecard_repo_pin(scorecard)
    if repo_pin != cohort["repo_pin"] or scorecard.get("claim_id") != CLAIM_ID:
        raise ValueError("scorecard is not bound to the frozen cohort claim")
    frozen_ids = scorecard.get("baseline_pair_ids")
    if (
        not isinstance(frozen_ids, list)
        or frozen_ids != sorted(frozen_ids)
        or len(frozen_ids) != len(set(frozen_ids))
    ):
        raise ValueError("frozen baseline pair identities are not canonical")
    frozen_id_set = set(frozen_ids)
    source = GitSource(repo_pin)
    units_by_id = {unit["unit_id"]: unit for unit in cohort["units"]}
    units_by_location = {
        (unit["path"], unit["start_line"], unit["end_line"]): unit
        for unit in cohort["units"]
    }
    pair_ids = set()
    family_offsets: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for pair in scorecard["pairs"]:
        if len(pair["members"]) != 2:
            raise ValueError(f"pair does not have exactly two members: {pair['pair_id']}")
        members = sorted(pair["members"], key=lambda row: row["location_id"])
        if members != pair["members"]:
            raise ValueError(f"pair members are not in canonical order: {pair['pair_id']}")
        member_units = []
        for member in members:
            if member.get("repo_pin") != repo_pin:
                raise ValueError(f"pair member has the wrong repository pin: {member['path']}")
            expected_source = sha256_bytes(source.slice(
                member["path"], member["start_line"], member["end_line"]
            ))
            if member["source_sha256"] != expected_source:
                raise ValueError(f"pair source hash mismatch: {member['path']}")
            expected = location_identity(member, repo_pin)
            if member["location_id"] != expected:
                raise ValueError(f"location identity mismatch: {member['path']}")
            unit_id = member["unit_id"]
            unit = units_by_id.get(unit_id) if unit_id is not None else None
            location = (member["path"], member["start_line"], member["end_line"])
            if unit is None:
                if unit_id is not None or location in units_by_location or member["core_il_sha256"] is not None:
                    raise ValueError(f"uninterpretable member has inconsistent cohort data: {member['path']}")
            elif (
                (unit["path"], unit["start_line"], unit["end_line"]) != location
                or member["source_sha256"] != unit["source_sha256"]
                or member["core_il_sha256"] != unit["core_il_sha256"]
            ):
                raise ValueError(f"pair member does not match its cohort unit: {unit_id}")
            member_units.append(unit)
        expected = identity(
            "nose-soundness-pair-v1", CLAIM_ID,
            members[0]["location_id"], members[1]["location_id"],
        )
        if pair["pair_id"] != expected or pair["pair_id"] in pair_ids:
            raise ValueError(f"invalid or duplicate pair identity: {pair['pair_id']}")
        expected_status = pair_status(member_units[0], member_units[1])
        constructs = set(member_units[0]["constructs"] if member_units[0] else [])
        constructs.update(member_units[1]["constructs"] if member_units[1] else [])
        expected_construct, expected_tier = construct_family(
            constructs, member_units[0] is None or member_units[1] is None
        )
        langs = {language(member["path"]) for member in members}
        expected_language = next(iter(langs)) if len(langs) == 1 else "cross-language"
        if (
            pair["status"] != expected_status
            or pair["construct"] != expected_construct
            or pair["risk_tier"] != expected_tier
            or pair["language"] != expected_language
            or pair["claim_id"] != CLAIM_ID
            or pair["obligation_id"] != f"exact-soundness.{expected_construct}"
        ):
            raise ValueError(f"pair claim metadata disagrees with cohort evidence: {pair['pair_id']}")
        pair_ids.add(pair["pair_id"])
        family_offsets[pair["family_id"]].append(pair)

    expected_ledger = sorted(
        (
            family["id"],
            tuple(sorted(
                (location["file"], location["start"], location["end"])
                for location in locations
            )),
        )
        for family in query["families"]
        for locations in itertools.combinations(family["locations"], 2)
    )
    actual_ledger = sorted(
        (
            pair["family_id"],
            tuple(sorted(
                (member["path"], member["start_line"], member["end_line"])
                for member in pair["members"]
            )),
        )
        for pair in scorecard["pairs"]
    )
    if actual_ledger != expected_ledger:
        raise ValueError("scorecard pair ledger does not match the frozen semantic query")

    expected_frozen_ids = set()
    for rows in family_offsets.values():
        expected = {row["pair_id"] for row in sorted(rows, key=lambda row: row["pair_id"])[:PAIR_CAP]}
        actual = {row["pair_id"] for row in rows if row["family_cap_selected"]}
        if actual != expected:
            raise ValueError("family cap selection is not stable")
        expected_frozen_ids.update(expected)
    if frozen_id_set != expected_frozen_ids:
        raise ValueError("frozen baseline pair identities disagree with the family caps")
    rescored = score_pairs(scorecard["pairs"], frozen_id_set)
    if scorecard["cells"] != rescored["cells"] or scorecard["summary"] != rescored["summary"]:
        raise ValueError("scorecard cells or summary do not match the frozen pair ledger")
    if any(cell["verified_pair_mass"] + cell["exact_unsafe_pair_mass"] != cell["baseline_pair_mass"] for cell in scorecard["cells"]):
        raise ValueError("exact-unsafe mass escaped the score denominator")


def scorecard_repo_pin(scorecard: dict[str, Any]) -> str:
    pins = {
        member.get("repo_pin")
        for pair in scorecard["pairs"] for member in pair["members"]
        if member.get("repo_pin")
    }
    if pins:
        if len(pins) != 1:
            raise ValueError("pair ledger contains multiple repository pins")
        return pins.pop()
    return scorecard["repo_pin"]


def verify_manifest(base: Path, manifest: dict[str, Any]) -> None:
    expected_source = {
        "release_tag": "v0.19.0",
        "release_commit": OFFICIAL_RELEASE_COMMIT,
        "historical_anchor_commit": HISTORICAL_ANCHOR_COMMIT,
        "crates_tree": OFFICIAL_CRATES_TREE,
    }
    if manifest.get("source") != expected_source:
        raise ValueError("official v0.19.0 source identity changed")
    if (
        git_rev_parse("v0.19.0^{commit}") != OFFICIAL_RELEASE_COMMIT
        or git_rev_parse(f"{OFFICIAL_RELEASE_COMMIT}:crates") != OFFICIAL_CRATES_TREE
        or git_rev_parse(f"{HISTORICAL_ANCHOR_COMMIT}:crates") != OFFICIAL_CRATES_TREE
    ):
        raise ValueError("Git history no longer resolves the frozen v0.19.0 crates tree")

    historical_identity = manifest.get("historical_anchor_identity", {})
    published_identity = manifest.get("published_asset_identity", {})
    if (
        historical_identity.get("binary_sha256") != HISTORICAL_BINARY_SHA256
        or historical_identity.get("binary_code_sha256") != HISTORICAL_CODE_SHA256
        or historical_identity.get("binary_code_sha256_algorithm")
        != "sha256/mach-o-zero-uuid-signature-v1"
        or historical_identity.get("report_sha256") != OFFICIAL_REPORT_SHA256
        or published_identity.get("asset") != "nose-cli-aarch64-apple-darwin.tar.xz"
        or published_identity.get("asset_sha256") != OFFICIAL_ASSET_SHA256
        or published_identity.get("binary_sha256") != OFFICIAL_BINARY_SHA256
        or published_identity.get("report_sha256") != OFFICIAL_REPORT_SHA256
    ):
        raise ValueError("official v0.19.0 binary or report identity changed")
    if manifest.get("release_identity_reconciliation", {}).get("status") != (
        "distinct-binaries-identical-release-tree-report"
    ):
        raise ValueError("release binary reconciliation status changed")

    for name, expected in manifest["tracked_artifacts"].items():
        path = ROOT / name
        if not path.is_file() or sha256_file(path) != expected:
            raise ValueError(f"tracked artifact hash mismatch: {name}")
    for name, expected in manifest["baseline_artifacts"].items():
        path = base / name
        if not path.is_file() or sha256_file(path) != expected:
            raise ValueError(f"baseline artifact hash mismatch: {name}")
    expected_attribution = {
        "schema": "nose-oracle-exclusion-census/v2",
        "instrument_binary_sha256": "b2c4770f724d1e194ebe47b84eaa60fb550142a20842f35e0e0691aeb5f029f5",
        "release_tree_census_sha256": "e3136ef3fb4165315b4b7fdd31ef281812c973d057139debe22d31e8026da36f",
        "threads": [1, 4],
        "generic_unattributed_exclusions": 0,
        "semantic_boundaries_closed": 652,
        "corpus_units": 639516,
        "corpus_interpretable_units": 197369,
        "corpus_excluded_units": 442147,
        "claimable_families": 659,
        "claimable_pair_mass": 2522,
        "capped_claimable_pair_mass": 1256,
    }
    if manifest.get("exclusion_attribution") != expected_attribution:
        raise ValueError("exclusion attribution evidence identity changed")
    report_path = base / "crates-report.v1.json"
    report = load(report_path)
    if (
        sha256_file(report_path) != OFFICIAL_REPORT_SHA256
        or report_metrics(report) != EXPECTED_RELEASE_METRICS
        or manifest.get("continuity", {}).get("historical_anchor") != EXPECTED_RELEASE_METRICS
        or manifest.get("continuity", {}).get("published_asset_replay") != EXPECTED_RELEASE_METRICS
    ):
        raise ValueError("official and historical v0.19.0 report continuity changed")
    cohort = load(base / "cohort.v1.json")
    instrument = manifest["instrumentation"]
    if (
        cohort.get("repo_pin") != OFFICIAL_RELEASE_COMMIT
        or instrument.get("source_commit") != OFFICIAL_RELEASE_COMMIT
        or instrument.get("threads") != [1, 4]
        or instrument.get("artifact_sha256") != INSTRUMENTED_UNITS_SHA256
        or len(cohort["units"]) != instrument["unit_count"]
        or sum(row["canon_exposed"] for row in cohort["units"]) != instrument["canon_exposed"]
        or sum(row["claimable"] for row in cohort["units"]) != instrument["claimable_units"]
    ):
        raise ValueError("instrumented cohort continuity changed")
    query = load(base / "semantic-query.v1.json")
    query_pairs = sum(len(row["locations"]) * (len(row["locations"]) - 1) // 2 for row in query["families"])
    scorecard = load(base / "scorecard.v1.json")
    if (
        query.get("path") != "crates"
        or query_pairs != len(scorecard["pairs"])
        or any(row["witness"] != "exact" for row in query["families"])
    ):
        raise ValueError("semantic query and exact-pair ledger disagree")
    corpus = load(ROOT / manifest["corpus"]["manifest"])
    actual = {row["id"]: row["commit"] for row in corpus["repositories"]}
    selection = [{"commit": actual[repo], "id": repo} for repo in sorted(actual)]
    if (
        len(actual) != manifest["corpus"]["repository_count"]
        or manifest["corpus"]["repository_count"] != 120
        or manifest["corpus"]["selection_sha256"] != CORPUS_SELECTION_SHA256
        or sha256_bytes(canonical_bytes(selection)) != CORPUS_SELECTION_SHA256
    ):
        raise ValueError("pinned 120-repository cohort changed")
    prune_path = ROOT / manifest["corpus"]["prune_manifest"]
    prune = load(prune_path)
    if (
        sha256_file(prune_path) != manifest["corpus"]["prune_manifest_sha256"]
        or prune["corpus_digest_after_prune"]["hex"]
        != manifest["corpus"]["pruned_corpus_digest_sha256"]
    ):
        raise ValueError("pinned corpus prune identity changed")


def report_metrics(report: dict[str, Any]) -> dict[str, Any]:
    classes = {
        row["classification"]: row["count"]
        for row in report["oracle_exclusions"]["by_classification"]
    }
    return {
        "summary": report["summary"], "soundness_gate": report["soundness_gate"],
        "completeness": report["completeness"], "exclusion_classification": classes,
    }


def verify_reproduction(path: Path, manifest: dict[str, Any]) -> None:
    candidates = [path / "crates.json", path.parent / "crates.json"]
    report_path = next((candidate for candidate in candidates if candidate.is_file()), None)
    if report_path is None:
        raise ValueError(f"reproduction report missing under {path} or its parent")
    if (
        sha256_file(report_path) != OFFICIAL_REPORT_SHA256
        or report_metrics(load(report_path)) != EXPECTED_RELEASE_METRICS
    ):
        raise ValueError("reproduction is not the official v0.19.0 release-tree replay")
    summary_path = path / "summary.tsv"
    if not summary_path.is_file():
        raise ValueError(f"120-repository summary missing: {summary_path}")
    lines = summary_path.read_text().splitlines()
    legacy_header = "repo\tstatus\texit_code\tfalse_merges\tcanon_changes\tadvisory\tseconds"
    deterministic_header = "repo\tstatus\texit_code\tfalse_merges\tcanon_changes\tadvisory"
    if not lines or lines[0] not in {legacy_header, deterministic_header}:
        raise ValueError("120-repository summary schema changed")
    rows = [line.split("\t") for line in lines[1:] if line]
    expected_width = 7 if lines[0] == legacy_header else 6
    if any(len(row) != expected_width for row in rows):
        raise ValueError("120-repository summary row is malformed")
    corpus = load(ROOT / manifest["corpus"]["manifest"])
    expected_repositories = sorted(
        ({"id": row["id"], "expected_commit": row["commit"], "observed_commit": row["commit"]}
         for row in corpus["repositories"]),
        key=lambda row: row["id"],
    )
    expected_repos = {row["id"] for row in expected_repositories}
    if {row[0] for row in rows} != expected_repos or len(rows) != len(expected_repos):
        raise ValueError("reproduction did not cover exactly the pinned 120 repositories")
    failures = [row for row in rows if row[1] != "pass" or any(int(row[i]) for i in (2, 3, 4))]
    if failures:
        raise ValueError(f"repository soundness gate failed: {failures[0][0]}")
    canonical = ("\n".join(sorted("\t".join(row[:6]) for row in rows)) + "\n").encode()
    canonical_sha = sha256_bytes(canonical)
    if canonical_sha != manifest["corpus"]["canonical_gate_result_sha256"]:
        raise ValueError("120-repository canonical result changed")

    evidence_path = path / "evidence.json"
    if not evidence_path.is_file():
        raise ValueError(f"corpus identity evidence missing: {evidence_path}")
    evidence = load(evidence_path)
    corpus_path = ROOT / manifest["corpus"]["manifest"]
    prune_path = ROOT / manifest["corpus"]["prune_manifest"]
    expected_v1 = {
        "schema": "nose-corpus-verify-evidence/v1",
        "complete": True,
        "nose": {"sha256": OFFICIAL_BINARY_SHA256, "version": "nose 0.19.0"},
        "corpus_manifest_sha256": sha256_file(corpus_path),
        "prune_manifest_sha256": sha256_file(prune_path),
        "pruned_corpus_digest_sha256": manifest["corpus"]["pruned_corpus_digest_sha256"],
        "repositories": expected_repositories,
        "summary_sha256": sha256_file(summary_path),
        "canonical_result_sha256": canonical_sha,
    }
    expected_results = [
        {
            "id": row[0],
            "status": row[1],
            "exit_code": int(row[2]),
            "false_merges": int(row[3]),
            "canon_changes": int(row[4]),
            "advisory": int(row[5]),
        }
        for row in sorted(rows)
    ]
    expected_v2 = {
        **expected_v1,
        "schema": "nose-corpus-verify-evidence/v2",
        "results": expected_results,
        "totals": {
            "repositories": len(expected_results),
            "failed_repositories": 0,
            "false_merges": 0,
            "canon_changes": 0,
            "advisory": sum(row["advisory"] for row in expected_results),
        },
    }
    if evidence not in (expected_v1, expected_v2):
        raise ValueError("reproduction evidence is not bound to the official binary and pinned corpus")


def self_test() -> None:
    pair = {
        "pair_id": "base", "family_id": "family", "claim_id": CLAIM_ID,
        "obligation_id": "exact-soundness.scalar-value", "language": "rust",
        "construct": "scalar-value", "risk_tier": "tier-c",
        "status": "exact-safe-verified", "family_cap_selected": True,
    }
    baseline = score_pairs([pair], {"base"})
    easy = dict(pair, pair_id="synthetic-easy", family_id="new-family")
    frozen = score_pairs([pair, easy], {"base"})
    if baseline != frozen:
        raise AssertionError("post-baseline easy pair changed the frozen score")
    displacement = dict(pair, pair_id="000-cap-displacement", family_id="family")
    if score_pairs([pair, displacement], {"base"}) != baseline:
        raise AssertionError("same-family synthetic pair displaced a frozen denominator pair")
    try:
        score_pairs([easy], {"base"})
    except ValueError as error:
        if "frozen baseline pair is missing" not in str(error):
            raise
    else:
        raise AssertionError("missing frozen pair reduced the denominator")
    unsafe = dict(pair, pair_id="unsafe", status="exact-unsafe-symbolic")
    scored = score_pairs([unsafe], {"unsafe"})
    if scored["summary"]["verified_pair_mass"] != 0 or scored["summary"]["exact_unsafe_pair_mass"] != 1:
        raise AssertionError("exact-unsafe mass entered the claimable numerator")
    ordered = canonical_bytes(sorted([pair, unsafe], key=lambda row: row["pair_id"]))
    reversed_order = canonical_bytes(sorted([unsafe, pair], key=lambda row: row["pair_id"]))
    if ordered != reversed_order:
        raise AssertionError("artifact order is not deterministic")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "x.rs").write_bytes(b"fn x() {}\n")
        if source_slice(root, "x.rs", 1, 1) != b"fn x() {}\n":
            raise AssertionError("source identity slice changed")
        baseline = ROOT / "bench/soundness/0.19.0"
        if baseline.is_dir():
            manifest = load(baseline / "manifest.v1.json")
            cohort = load(baseline / "cohort.v1.json")
            query = load(baseline / "semantic-query.v1.json")
            scorecard = load(baseline / "scorecard.v1.json")
            tampered = copy.deepcopy(scorecard)
            victim = next(
                row for row in tampered["pairs"]
                if row["family_cap_selected"] and row["status"] != "exact-safe-verified"
            )
            victim["status"] = "exact-safe-verified"
            rescored = score_pairs(tampered["pairs"], set(tampered["baseline_pair_ids"]))
            tampered["cells"] = rescored["cells"]
            tampered["summary"] = rescored["summary"]
            try:
                verify_scorecard(tampered, cohort, query)
            except ValueError as error:
                if "cohort evidence" not in str(error):
                    raise
            else:
                raise AssertionError("declared exact-unsafe pair entered the verified numerator")
            reproduce = root / "reproduce"
            reproduce.mkdir()
            later_tree_report = (
                ROOT / "bench/recall_loss/issue-846-crates-verify-2026-07-14.v1.json"
            )
            shutil.copyfile(later_tree_report, root / "crates.json")
            try:
                verify_reproduction(reproduce, manifest)
            except ValueError as error:
                if "official v0.19.0 release-tree replay" not in str(error):
                    raise
            else:
                raise AssertionError("a post-release source tree counted as the v0.19.0 replay")
            shutil.copyfile(baseline / "crates-report.v1.json", root / "crates.json")
            repos = sorted(load(ROOT / manifest["corpus"]["manifest"])["repositories"], key=lambda row: row["id"])
            rows = ["repo\tstatus\texit_code\tfalse_merges\tcanon_changes\tadvisory\tseconds"]
            rows.extend(f"{row['id']}\tpass\t0\t0\t0\t0\t0" for row in repos[:-1])
            (reproduce / "summary.tsv").write_text("\n".join(rows) + "\n")
            try:
                verify_reproduction(reproduce, manifest)
            except ValueError as error:
                if "exactly the pinned" not in str(error):
                    raise
            else:
                raise AssertionError("a missing pinned repository counted as success")
            expansion_overlay = ROOT / "bench/soundness/0.20.0/oracle-expansion-overlay.v2.json"
            expansion_receipt = ROOT / "bench/soundness/0.20.0/oracle-expansion-859.v1.json"
            if expansion_overlay.is_file() and expansion_receipt.is_file():
                verify_expansion_receipt(scorecard, expansion_overlay, expansion_receipt)
                tampered_overlay = load(expansion_overlay)
                tampered_overlay["summary"]["verified_pair_mass"] += 1
                try:
                    verify_overlay(scorecard, tampered_overlay)
                except ValueError as error:
                    if "replay" not in str(error):
                        raise
                else:
                    raise AssertionError("tampered expansion aggregate passed replay")
            release_binding = release_binding_path()
            if release_binding.is_file():
                verify_release_binding(scorecard, release_binding)
    print("ok soundness scorecard self-test")


def freeze(args: argparse.Namespace) -> None:
    if not args.units or not args.query or not args.source_root:
        raise ValueError("--freeze requires --units, --query, and --source-root")
    base = args.baseline.resolve()
    base.mkdir(parents=True, exist_ok=True)
    cohort = normalize_units(load(args.units), args.source_root.resolve(), args.repo_pin)
    query = load(args.query)
    scorecard = build_scorecard(query, cohort, args.source_root.resolve())
    scorecard["repo_pin"] = args.repo_pin
    for pair in scorecard["pairs"]:
        for member in pair["members"]:
            member["repo_pin"] = args.repo_pin
    verify_cohort(cohort)
    verify_scorecard(scorecard, cohort, query)
    (base / "cohort.v1.json").write_bytes(pretty_bytes(cohort))
    (base / "scorecard.v1.json").write_bytes(pretty_bytes(scorecard))
    print(f"froze {len(cohort['units'])} units and {len(scorecard['pairs'])} exact pairs")


def evaluate(args: argparse.Namespace) -> None:
    if not args.units or not args.source_root or not args.output:
        raise ValueError("--evaluate requires --units, --source-root, and --output")
    frozen = load(args.baseline.resolve() / "scorecard.v1.json")
    candidate = normalize_units(load(args.units), args.source_root.resolve(), args.repo_pin)
    verify_cohort(candidate)
    overlay = build_overlay(frozen, candidate)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(pretty_bytes(overlay))
    summary = overlay["summary"]
    print(
        "soundness overlay: "
        f"macro={summary['macro_ppm'] / 10000:.2f}% "
        f"target={summary['release_target_ppm'] / 10000:.2f}% "
        f"pairs={summary['verified_pair_mass']}/{summary['baseline_pair_mass']} "
        f"passed={str(overlay['gates']['passed']).lower()}"
    )
    if not overlay["gates"]["passed"]:
        raise ValueError("candidate does not satisfy the frozen scorecard gates")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, default=ROOT / "bench/soundness/0.19.0")
    parser.add_argument("--reproduce", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--evaluate", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--units", type=Path)
    parser.add_argument("--query", type=Path)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--repo-pin", default=OFFICIAL_RELEASE_COMMIT)
    parser.add_argument(
        "--release-commit",
        help="also require this release candidate commit's crates tree to match the binding",
    )
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
            return 0
        if args.freeze:
            freeze(args)
            return 0
        if args.evaluate:
            evaluate(args)
            return 0
        base = args.baseline.resolve()
        manifest = load(base / "manifest.v1.json")
        verify_manifest(base, manifest)
        cohort = load(base / "cohort.v1.json")
        query = load(base / "semantic-query.v1.json")
        scorecard = load(base / "scorecard.v1.json")
        verify_cohort(cohort)
        verify_scorecard(scorecard, cohort, query)
        expansion_overlay = ROOT / "bench/soundness/0.20.0/oracle-expansion-overlay.v2.json"
        expansion_receipt = ROOT / "bench/soundness/0.20.0/oracle-expansion-859.v1.json"
        if expansion_overlay.is_file() != expansion_receipt.is_file():
            raise ValueError("oracle expansion evidence must include its overlay and receipt")
        expansion_summary = None
        if expansion_overlay.is_file():
            verify_expansion_receipt(scorecard, expansion_overlay, expansion_receipt)
            expansion_summary = load(expansion_overlay)["summary"]
        release_binding = release_binding_path()
        if release_binding.is_file():
            expansion_summary = verify_release_binding(
                scorecard, release_binding, args.release_commit
            )["summary"]
        if args.reproduce:
            verify_reproduction(args.reproduce.resolve(), manifest)
        summary = scorecard["summary"]
        message = (
            "soundness scorecard ok: "
            f"macro={summary['macro_ppm'] / 10000:.2f}% "
            f"micro={summary['pair_micro_ppm'] / 10000:.2f}% "
            f"pairs={summary['verified_pair_mass']}/{summary['baseline_pair_mass']}"
        )
        if expansion_summary is not None:
            message += (
                "; expansion "
                f"macro={expansion_summary['macro_ppm'] / 10000:.2f}% "
                f"pairs={expansion_summary['verified_pair_mass']}/"
                f"{expansion_summary['baseline_pair_mass']}"
            )
        print(message)
        return 0
    except (KeyError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"soundness scorecard error: {error}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
