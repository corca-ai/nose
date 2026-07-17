#!/usr/bin/env python3
"""Build and verify Soundness Lab exclusion-attribution evidence."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import shutil
import subprocess
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "bench/soundness/0.19.0"
RAW_SCHEMA = "nose-oracle-exclusion-census/v2"
LEDGER_SCHEMA = "nose-soundness-exclusion-ledger/v2"
CORPUS_SCHEMA = "nose-claimable-mass-census/v2"
PRIORITY_SCHEMA = "nose-interpreter-priority/v2"
CURRENT_ATTRIBUTION_SCHEMA = "nose-soundness-current-exclusion-attribution/v1"
RELEASE_COMMIT = "0985e6963c58d5a97e523bc532b88aa5e34f2ef9"
CRATES_TREE = "f57b078517fcd114657dfb90a2b72f44bfb6cafb"
REPORT_SHA256 = "149abb80c7ffb790d3fc0fbc2ad910add776d768ecf2961ee4321239f935d9c9"
EXPECTED_CLASSIFICATIONS = {
    "missing-oracle-support": 6054,
    "semantic-boundary-attributed": 652,
    "path-exploration-budget": 5,
    "oracle-cost-budget": 1,
    "empty-value-fingerprint": 1,
}
PAIR_CAP = 8
RISK_WEIGHT = 3


def load(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def pretty(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(pretty(value))


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_byte_identical(primary: Path, peer: Path, label: str) -> str:
    if primary.resolve() == peer.resolve() or primary.samefile(peer):
        raise ValueError(f"{label} must use distinct files for 1/4 threads")
    primary_hash = sha256_file(primary)
    peer_hash = sha256_file(peer)
    if primary_hash != peer_hash or primary.read_bytes() != peer.read_bytes():
        raise ValueError(f"{label} is not byte-identical across 1/4 threads")
    return primary_hash


def identity(*parts: object) -> str:
    digest = hashlib.sha256()
    for part in parts:
        digest.update(str(part).encode())
        digest.update(b"\0")
    return digest.hexdigest()


def pairs(count: int) -> int:
    return count * max(0, count - 1) // 2


def fp_key(unit: dict[str, Any]) -> tuple[int, ...]:
    return tuple(unit["value_fingerprint"])


def blocker_key(unit: dict[str, Any]) -> tuple[str, str, str, str, str] | None:
    blocker = unit.get("first_blocker")
    if blocker is None:
        return None
    stack = blocker["blocker_stack"]
    construct = stack[0]["construct"] if stack else "kind:Func"
    return (
        unit["language"], unit["obligation_family"], construct,
        blocker["category"], blocker["capability_id"],
    )


def count_pairs(units: Iterable[dict[str, Any]]) -> dict[str, int]:
    groups: dict[tuple[int, ...], list[dict[str, Any]]] = defaultdict(list)
    for unit in units:
        if unit["value_fingerprint"]:
            groups[fp_key(unit)].append(unit)
    total = verified = 0
    for group in groups.values():
        total += pairs(len(group))
        interpreted = sum(row["reason"] == "interpretable" for row in group)
        verified += pairs(interpreted)
    return {"total": total, "verified": verified, "unverified": total - verified}


def priority_rows(units: list[dict[str, Any]], cap: int) -> list[dict[str, Any]]:
    counts: dict[tuple[str, ...], dict[str, Any]] = defaultdict(
        lambda: {"excluded_units": 0, "pair_mass": 0, "capped": 0, "examples": []}
    )
    for unit in units:
        if unit["reason"] != "interpretable" and unit["claimable"]:
            key = blocker_key(unit)
            if key is not None:
                counts[key]["excluded_units"] += 1
                counts[key]["examples"].append(unit["loc"])
    groups: dict[tuple[int, ...], list[dict[str, Any]]] = defaultdict(list)
    for unit in units:
        if unit["claimable"] and unit["value_fingerprint"]:
            groups[fp_key(unit)].append(unit)
    for group in groups.values():
        interpreted = sum(row["reason"] == "interpretable" for row in group)
        unverified = pairs(len(group)) - pairs(interpreted)
        if unverified == 0:
            continue
        keys = {blocker_key(row) for row in group if row["reason"] != "interpretable"}
        for key in keys - {None}:
            counts[key]["pair_mass"] += unverified
            counts[key]["capped"] += min(cap, unverified)
    result = []
    for key, count in counts.items():
        if count["capped"] == 0:
            continue
        language, obligation, construct, category, capability = key
        result.append({
            "language": language,
            "obligation_family": obligation,
            "construct": construct,
            "blocker_category": category,
            "capability_id": capability,
            "risk_tier": "A",
            "risk_weight": RISK_WEIGHT,
            "excluded_units": count["excluded_units"],
            "claimable_pair_mass": count["pair_mass"],
            "capped_claimable_pair_mass": count["capped"],
            "priority_score": RISK_WEIGHT * count["capped"],
            "example_excluded": sorted(set(count["examples"]))[:3],
        })
    result.sort(key=lambda row: (
        -row["priority_score"], -row["claimable_pair_mass"], row["language"],
        row["obligation_family"], row["construct"], row["capability_id"],
    ))
    return result


def validate_raw(raw: dict[str, Any]) -> None:
    if raw.get("schema") != RAW_SCHEMA:
        raise ValueError(f"unexpected census schema: {raw.get('schema')}")
    units = raw["units"]
    if raw["units_total"] != len(units):
        raise ValueError("census units_total does not match unit rows")
    locations = [unit["loc"] for unit in units]
    if len(locations) != len(set(locations)):
        raise ValueError("census contains duplicate unit locations")
    interpreted = sum(unit["reason"] == "interpretable" for unit in units)
    if raw["interpretable_units"] != interpreted:
        raise ValueError("census interpretable count does not match unit rows")
    generic = 0
    for unit in units:
        admission = unit.get("product_admission")
        if not isinstance(admission, str) or not admission:
            raise ValueError(f"missing product admission at {unit['loc']}")
        expected_claimable = (
            admission == "admitted"
            and unit["exact_safe"]
            and len(unit["value_fingerprint"]) >= 4
        )
        if unit["claimable"] != expected_claimable:
            raise ValueError(f"claim eligibility drift at {unit['loc']}")
        excluded = unit["reason"] != "interpretable"
        if excluded and (
            not unit.get("first_blocker")
            or not unit["obligation_family"]
            or not unit["obligation_subreason"]
        ):
            generic += 1
        if not excluded and unit.get("first_blocker") is not None:
            raise ValueError(f"interpretable unit has a blocker: {unit['loc']}")
    if generic or raw["generic_unattributed_exclusions"] != 0:
        raise ValueError(f"generic/unattributed exclusions: {generic}")
    if count_pairs(iter(units)) != raw["merge_pairs"]:
        raise ValueError("all-unit merge-pair aggregate does not match unit rows")
    claimable = (unit for unit in units if unit["claimable"])
    if count_pairs(claimable) != raw["claimable_merge_pairs"]:
        raise ValueError("claimable merge-pair aggregate does not match unit rows")
    expected_priority = priority_rows(units, raw["claimable_family_cap"])
    if expected_priority != raw["priority"]:
        raise ValueError("priority aggregate does not match claimable unit rows")


def current_attribution_receipt(
    raw: dict[str, Any],
    raw_sha256: str,
    source_commit: str,
    crates_tree: str,
    binary_sha256: str,
) -> dict[str, Any]:
    """Compact a fully validated candidate census without checking in every unit row."""
    validate_raw(raw)
    excluded = [unit for unit in raw["units"] if unit["reason"] != "interpretable"]
    classifications = Counter(unit["classification"] for unit in excluded)
    capabilities = Counter(
        unit["first_blocker"]["capability_id"]
        for unit in excluded
        if unit.get("first_blocker")
    )
    return {
        "schema": CURRENT_ATTRIBUTION_SCHEMA,
        "issue": 862,
        "parent_issue": 855,
        "source_sha": source_commit,
        "crates_tree": crates_tree,
        "binary_sha256": binary_sha256,
        "raw_census": {
            "schema": RAW_SCHEMA,
            "sha256": raw_sha256,
        },
        "summary": {
            "total_units": raw["units_total"],
            "interpretable_units": raw["interpretable_units"],
            "excluded_units": len(excluded),
            "generic_unattributed_exclusions": raw["generic_unattributed_exclusions"],
            "by_classification": dict(sorted(classifications.items())),
            "by_capability": dict(sorted(capabilities.items())),
        },
    }


def summarize_current(
    census: Path, source_commit: str, binary_sha256: str, output: Path
) -> None:
    if len(source_commit) != 40 or any(char not in "0123456789abcdef" for char in source_commit):
        raise ValueError("--source-commit must be a full lowercase Git object id")
    if len(binary_sha256) != 64 or any(char not in "0123456789abcdef" for char in binary_sha256):
        raise ValueError("--binary-sha256 must be 64 lowercase hex characters")
    observed = subprocess.check_output(
        ["git", "rev-parse", "--verify", f"{source_commit}^{{commit}}"],
        cwd=ROOT,
        text=True,
    ).strip()
    if observed != source_commit:
        raise ValueError("--source-commit did not resolve exactly")
    crates_tree = subprocess.check_output(
        ["git", "rev-parse", f"{source_commit}:crates"], cwd=ROOT, text=True
    ).strip()
    receipt = current_attribution_receipt(
        load(census), sha256_file(census), source_commit, crates_tree, binary_sha256
    )
    write(output, receipt)


def parse_loc(loc: str) -> tuple[str, int]:
    path, span = loc.rsplit(":", 1)
    return path, int(span.split("@", 1)[0].split("-", 1)[0])


def parse_loc_span(loc: str) -> tuple[str, int, int]:
    path, span = loc.rsplit(":", 1)
    lines = span.split("@", 1)[0]
    start, end = lines.split("-", 1)
    return path, int(start), int(end)


def source_slice(root: Path, path: str, start: int, end: int) -> bytes:
    candidate = (root / path).resolve()
    candidate.relative_to(root.resolve())
    return source_slice_bytes(candidate.read_bytes(), path, start, end)


def source_slice_bytes(data: bytes, path: str, start: int, end: int) -> bytes:
    lines = data.splitlines(keepends=True)
    if start < 1 or end < start or end > len(lines):
        raise ValueError(f"invalid source span: {path}:{start}-{end}")
    return b"".join(lines[start - 1:end])


def freeze_baseline(
    census_path: Path,
    census_peer_path: Path,
    report_path: Path,
    report_peer_path: Path,
    source_root: Path,
) -> dict[str, Any]:
    census_hash = require_byte_identical(census_path, census_peer_path, "raw census")
    report_hash = require_byte_identical(report_path, report_peer_path, "recall-loss report")
    raw = load(census_path)
    validate_raw(raw)
    if report_hash != REPORT_SHA256:
        raise ValueError("recall-loss report is not the frozen v0.19.0 report")
    commit = subprocess.check_output(
        ["git", "-C", str(source_root), "rev-parse", "HEAD"], text=True,
    ).strip()
    tree = subprocess.check_output(
        ["git", "-C", str(source_root), "rev-parse", "HEAD:crates"], text=True,
    ).strip()
    if (commit, tree) != (RELEASE_COMMIT, CRATES_TREE):
        raise ValueError(f"source identity mismatch: {commit} / {tree}")
    report = load(report_path)
    reports = {
        (row["loc"]["file"], row["loc"]["start_line"]): row
        for row in report["oracle_exclusions"]["units"]
    }
    excluded = [row for row in raw["units"] if row["reason"] != "interpretable"]
    if len(reports) != len(excluded):
        raise ValueError("census/report exclusion cardinality mismatch")
    units = []
    for unit in excluded:
        path, start = parse_loc(unit["loc"])
        report_unit = reports.get((path, start))
        if report_unit is None:
            raise ValueError(f"excluded unit missing from release report: {unit['loc']}")
        location = report_unit["loc"]
        end = location["end_line"]
        source_hash = sha256_bytes(source_slice(source_root, path, start, end))
        fp_hash = sha256_bytes(pretty(unit["value_fingerprint"]))
        capability = unit["first_blocker"]["capability_id"]
        row = {
            "status": "excluded",
            "release_commit": RELEASE_COMMIT,
            "raw_loc": unit["loc"],
            "path": path,
            "start_line": start,
            "end_line": end,
            "tokens": location["tokens"],
            "language": unit["language"],
            "source_sha256": source_hash,
            "value_fingerprint_sha256": fp_hash,
            "value_fingerprint": unit["value_fingerprint"],
            "exact_safe": unit["exact_safe"],
            "product_admission": unit["product_admission"],
            "claimable": unit["claimable"],
            "report_reason": report_unit["reason"],
            "classification": unit["classification"],
            "obligation_family": unit["obligation_family"],
            "obligation_subreason": unit["obligation_subreason"],
            "constructs": unit["constructs"],
            "first_blocker": unit["first_blocker"],
        }
        row["unit_id"] = identity(
            "nose-soundness-exclusion-v2", RELEASE_COMMIT, unit["loc"], path, start, end,
            source_hash, fp_hash, capability,
        )
        units.append(row)
    units.sort(key=lambda row: row["unit_id"])
    classifications = Counter(row["classification"] for row in units)
    if dict(classifications) != EXPECTED_CLASSIFICATIONS:
        raise ValueError(f"baseline classification drift: {dict(classifications)}")
    boundaries = [row for row in units if row["classification"] == "semantic-boundary-attributed"]
    if len(boundaries) != 652 or any(row["claimable"] for row in boundaries):
        raise ValueError("the 652 semantic boundaries are not explicitly closed")
    return {
        "schema": LEDGER_SCHEMA,
        "release": {"version": "0.19.0", "commit": RELEASE_COMMIT, "crates_tree": CRATES_TREE},
        "instrument": {
            "raw_schema": RAW_SCHEMA,
            "raw_census_sha256": census_hash,
            "raw_census_peer_sha256": sha256_file(census_peer_path),
            "unchanged_recall_report_sha256": report_hash,
            "unchanged_recall_report_peer_sha256": sha256_file(report_peer_path),
            "threads_compared": [1, 4],
        },
        "identity_algorithm": "sha256-nul-v2",
        "summary": {
            "excluded_units": len(units),
            "generic_unattributed_exclusions": 0,
            "by_classification": dict(sorted(classifications.items())),
            "by_capability": dict(sorted(Counter(
                row["first_blocker"]["capability_id"] for row in units
            ).items())),
        },
        "units": units,
    }


def family_rows(repo: str, units: list[dict[str, Any]], cap: int) -> list[dict[str, Any]]:
    groups: dict[tuple[int, ...], list[dict[str, Any]]] = defaultdict(list)
    for unit in units:
        if unit["claimable"] and unit["value_fingerprint"]:
            groups[fp_key(unit)].append(unit)
    result = []
    for fingerprint, group in groups.items():
        interpreted = sum(row["reason"] == "interpretable" for row in group)
        unverified = pairs(len(group)) - pairs(interpreted)
        if unverified == 0:
            continue
        key_examples: dict[tuple[str, ...], list[str]] = defaultdict(list)
        for unit in group:
            if unit["reason"] != "interpretable":
                key = blocker_key(unit)
                if key is not None:
                    key_examples[key].append(unit["loc"])
        blockers = []
        for key, examples in sorted(key_examples.items()):
            language, obligation, construct, category, capability = key
            blockers.append({
                "language": language, "obligation_family": obligation,
                "construct": construct, "blocker_category": category,
                "capability_id": capability,
                "excluded_units": len(examples),
                "example_excluded": sorted(set(examples))[:3],
            })
        result.append({
            "repository": repo,
            "fingerprint_sha256": sha256_bytes(pretty(list(fingerprint))),
            "units": len(group),
            "interpretable_units": interpreted,
            "excluded_units": len(group) - interpreted,
            "claimable_pair_mass": unverified,
            "capped_claimable_pair_mass": min(cap, unverified),
            "blockers": blockers,
        })
    result.sort(key=lambda row: (row["repository"], row["fingerprint_sha256"]))
    return result


def claimable_unit_commitment(units: list[dict[str, Any]]) -> tuple[int, str]:
    rows = [
        {
            "loc": unit["loc"],
            "reason": unit["reason"],
            "language": unit["language"],
            "value_fingerprint": unit["value_fingerprint"],
            "product_admission": unit["product_admission"],
            "exact_safe": unit["exact_safe"],
            "classification": unit["classification"],
            "obligation_family": unit["obligation_family"],
            "first_blocker": unit.get("first_blocker"),
        }
        for unit in units
        if unit["claimable"]
    ]
    rows.sort(key=lambda row: row["loc"])
    return len(rows), sha256_bytes(pretty(rows))


def aggregate_corpus(raw_dir: Path, evidence_path: Path) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    evidence = load(evidence_path)
    for repository in evidence.get("repositories", []):
        repository.pop("log_sha256", None)
    if evidence.get("schema") != "nose-soundness-census-run/v2" or not evidence.get("complete"):
        raise ValueError("corpus run evidence is not a complete v2 census")
    all_families = []
    repositories = []
    classifications: Counter[str] = Counter()
    capabilities: Counter[str] = Counter()
    totals: Counter[str] = Counter()
    for repo in evidence["repositories"]:
        raw_path = raw_dir / f"{repo['id']}.json"
        if sha256_file(raw_path) != repo["census_sha256"]:
            raise ValueError(f"raw census hash mismatch: {repo['id']}")
        raw = load(raw_path)
        validate_raw(raw)
        units = raw["units"]
        excluded = [row for row in units if row["reason"] != "interpretable"]
        class_counts = Counter(row["classification"] for row in excluded)
        capability_counts = Counter(row["first_blocker"]["capability_id"] for row in excluded)
        classifications.update(class_counts)
        capabilities.update(capability_counts)
        totals.update(total=len(units), interpreted=raw["interpretable_units"], excluded=len(excluded))
        families = family_rows(repo["id"], units, PAIR_CAP)
        claimable_count, claimable_sha = claimable_unit_commitment(units)
        family_sha = sha256_bytes(pretty(families))
        family_mass = sum(row["claimable_pair_mass"] for row in families)
        family_capped = sum(row["capped_claimable_pair_mass"] for row in families)
        repo.update({
            "units": len(units),
            "interpretable_units": raw["interpretable_units"],
            "excluded_units": len(excluded),
            "generic_unattributed_exclusions": 0,
            "by_classification": dict(sorted(class_counts.items())),
            "by_capability": dict(sorted(capability_counts.items())),
            "claimable_units": claimable_count,
            "claimable_units_sha256": claimable_sha,
            "claimable_families": len(families),
            "claimable_families_sha256": family_sha,
            "claimable_pair_mass": family_mass,
            "capped_claimable_pair_mass": family_capped,
        })
        all_families.extend(families)
        repositories.append({
            "id": repo["id"], "commit": repo["commit"],
            "census_sha256": repo["census_sha256"],
            "units": len(units), "interpretable_units": raw["interpretable_units"],
            "excluded_units": len(excluded),
            "generic_unattributed_exclusions": 0,
            "claimable_families": len(families),
            "claimable_units": claimable_count,
            "claimable_units_sha256": claimable_sha,
            "claimable_families_sha256": family_sha,
            "claimable_pair_mass": family_mass,
            "capped_claimable_pair_mass": family_capped,
            "by_classification": dict(sorted(class_counts.items())),
            "by_capability": dict(sorted(capability_counts.items())),
        })
    repositories.sort(key=lambda row: row["id"])
    all_families.sort(key=lambda row: (row["repository"], row["fingerprint_sha256"]))
    census = {
        "schema": CORPUS_SCHEMA,
        "scope": "120-pinned-pruned-repositories",
        "eligibility": (
            "default_product_admission == admitted && exact_safe "
            "&& value_fingerprint.len >= 4"
        ),
        "validation": {
            "checked_commitments": "per-repository claimable-unit and family SHA-256",
            "source_completeness_requires_raw_replay": True,
        },
        "claimable_family_cap": PAIR_CAP,
        "evidence_sha256": sha256_bytes(pretty(evidence)),
        "corpus_manifest_sha256": evidence["corpus_manifest_sha256"],
        "prune_manifest_sha256": evidence["prune_manifest_sha256"],
        "pruned_corpus_digest_sha256": evidence["pruned_corpus_digest_sha256"],
        "nose": evidence["nose"],
        "summary": {
            "repositories": len(repositories), **dict(totals),
            "generic_unattributed_exclusions": 0,
            "claimable_families": len(all_families),
            "claimable_pair_mass": sum(row["claimable_pair_mass"] for row in all_families),
            "capped_claimable_pair_mass": sum(
                row["capped_claimable_pair_mass"] for row in all_families
            ),
            "by_classification": dict(sorted(classifications.items())),
            "by_capability": dict(sorted(capabilities.items())),
        },
        "repositories": repositories,
        "claimable_families": all_families,
    }
    priority = priority_from_families(census)
    return census, priority, evidence


def priority_from_families(census: dict[str, Any]) -> dict[str, Any]:
    counts: dict[tuple[str, ...], dict[str, Any]] = defaultdict(
        lambda: {"families": 0, "units": 0, "mass": 0, "capped": 0, "examples": []}
    )
    for family in census["claimable_families"]:
        for blocker in family["blockers"]:
            key = (
                blocker["language"], blocker["obligation_family"], blocker["construct"],
                blocker["blocker_category"], blocker["capability_id"],
            )
            count = counts[key]
            count["families"] += 1
            count["units"] += blocker["excluded_units"]
            count["mass"] += family["claimable_pair_mass"]
            count["capped"] += family["capped_claimable_pair_mass"]
            count["examples"].extend(blocker["example_excluded"])
    rows = []
    for key, count in counts.items():
        language, obligation, construct, category, capability = key
        rows.append({
            "language": language, "obligation_family": obligation, "construct": construct,
            "blocker_category": category, "capability_id": capability,
            "risk_tier": "A", "risk_weight": RISK_WEIGHT,
            "claimable_families": count["families"],
            "excluded_units": count["units"],
            "claimable_pair_mass": count["mass"],
            "capped_claimable_pair_mass": count["capped"],
            "priority_score": RISK_WEIGHT * count["capped"],
            "example_excluded": sorted(set(count["examples"]))[:3],
        })
    rows.sort(key=lambda row: (
        -row["priority_score"], -row["claimable_pair_mass"], row["language"],
        row["obligation_family"], row["construct"], row["capability_id"],
    ))
    return {
        "schema": PRIORITY_SCHEMA,
        "source_census_sha256": sha256_bytes(pretty(census)),
        "policy": {
            "only_claimable_units": True, "family_pair_cap": PAIR_CAP,
            "multi_attributed": True, "risk_tier": "A", "risk_weight": RISK_WEIGHT,
        },
        "rows": rows,
    }


def validate_ledger_raw_fields(row: dict[str, Any], raw_row: dict[str, Any]) -> None:
    raw_path, raw_start, raw_end = parse_loc_span(row["raw_loc"])
    expected = {
        "path": raw_path,
        "start_line": raw_start,
        "end_line": raw_end,
        "language": raw_row["language"],
        "value_fingerprint": raw_row["value_fingerprint"],
        "exact_safe": raw_row["exact_safe"],
        "product_admission": raw_row["product_admission"],
        "claimable": raw_row["claimable"],
        "classification": raw_row["classification"],
        "obligation_family": raw_row["obligation_family"],
        "obligation_subreason": raw_row["obligation_subreason"],
        "constructs": raw_row["constructs"],
        "first_blocker": raw_row["first_blocker"],
    }
    for key, value in expected.items():
        if row[key] != value:
            raise ValueError(f"ledger/raw {key} drift: {row['raw_loc']}")


def validate_ledger(ledger: dict[str, Any], raw: dict[str, Any], raw_sha256: str) -> None:
    if ledger.get("schema") != LEDGER_SCHEMA or ledger["release"]["commit"] != RELEASE_COMMIT:
        raise ValueError("invalid exclusion ledger identity")
    instrument = ledger.get("instrument", {})
    if (
        instrument.get("threads_compared") != [1, 4]
        or instrument.get("raw_census_sha256")
        != instrument.get("raw_census_peer_sha256")
        or instrument.get("unchanged_recall_report_sha256") != REPORT_SHA256
        or instrument.get("unchanged_recall_report_peer_sha256") != REPORT_SHA256
    ):
        raise ValueError("exclusion ledger lacks verified 1/4-thread reproduction")
    validate_raw(raw)
    if raw_sha256 != instrument["raw_census_sha256"]:
        raise ValueError("checked baseline raw census does not match ledger evidence")
    raw_excluded = {
        row["loc"]: row for row in raw["units"] if row["reason"] != "interpretable"
    }
    units = ledger["units"]
    if len(units) != 6713 or len({row["unit_id"] for row in units}) != len(units):
        raise ValueError("exclusion ledger cardinality or identity mismatch")
    classifications = Counter(row["classification"] for row in units)
    if dict(classifications) != EXPECTED_CLASSIFICATIONS:
        raise ValueError("exclusion ledger classification drift")
    files: dict[str, bytes] = {}
    reasons = {
        "missing-oracle-support": "uninterpretable",
        "semantic-boundary-attributed": "uninterpretable",
        "path-exploration-budget": "path-bail",
        "oracle-cost-budget": "battery-bail",
        "empty-value-fingerprint": "empty-fingerprint",
    }
    for row in units:
        expected = identity(
            "nose-soundness-exclusion-v2", RELEASE_COMMIT, row["raw_loc"], row["path"],
            row["start_line"], row["end_line"], row["source_sha256"],
            row["value_fingerprint_sha256"], row["first_blocker"]["capability_id"],
        )
        if row["unit_id"] != expected or row["status"] != "excluded":
            raise ValueError(f"invalid ledger unit: {row['path']}:{row['start_line']}")
        if sha256_bytes(pretty(row["value_fingerprint"])) != row["value_fingerprint_sha256"]:
            raise ValueError(f"fingerprint hash mismatch: {row['unit_id']}")
        if row["report_reason"] != reasons[row["classification"]]:
            raise ValueError(f"release exclusion reason drift: {row['unit_id']}")
        raw_row = raw_excluded.get(row["raw_loc"])
        if raw_row is None:
            raise ValueError(f"ledger unit missing from raw census: {row['raw_loc']}")
        validate_ledger_raw_fields(row, raw_row)
        if row["path"] not in files:
            files[row["path"]] = subprocess.check_output(
                ["git", "show", f"{RELEASE_COMMIT}:{row['path']}"], cwd=ROOT,
            )
        source = source_slice_bytes(
            files[row["path"]], row["path"], row["start_line"], row["end_line"]
        )
        if sha256_bytes(source) != row["source_sha256"]:
            raise ValueError(f"release source hash mismatch: {row['unit_id']}")
    if set(raw_excluded) != {row["raw_loc"] for row in units}:
        raise ValueError("ledger/raw exclusion membership drift")
    boundaries = [row for row in units if row["classification"] == "semantic-boundary-attributed"]
    if len(boundaries) != 652 or any(row["claimable"] for row in boundaries):
        raise ValueError("semantic boundary closure drift")


def validate_repository_commitments(
    repo_id: str,
    repository: dict[str, Any],
    source: dict[str, Any],
    repo_families: list[dict[str, Any]],
) -> None:
    expected_fields = {
        "commit": source["commit"],
        "census_sha256": source["census_sha256"],
        "units": source["units"],
        "interpretable_units": source["interpretable_units"],
        "excluded_units": source["excluded_units"],
        "generic_unattributed_exclusions": source["generic_unattributed_exclusions"],
        "by_classification": source["by_classification"],
        "by_capability": source["by_capability"],
        "claimable_units": source["claimable_units"],
        "claimable_units_sha256": source["claimable_units_sha256"],
        "claimable_families": len(repo_families),
        "claimable_families_sha256": sha256_bytes(pretty(repo_families)),
        "claimable_pair_mass": sum(row["claimable_pair_mass"] for row in repo_families),
        "capped_claimable_pair_mass": sum(
            row["capped_claimable_pair_mass"] for row in repo_families
        ),
    }
    for key, value in expected_fields.items():
        if repository.get(key) != value or source.get(key) != value:
            raise ValueError(f"claimable-mass {repo_id} {key} drift")


def validate_corpus(
    census: dict[str, Any], priority: dict[str, Any], evidence: dict[str, Any]
) -> None:
    if census.get("schema") != CORPUS_SCHEMA or census["summary"]["repositories"] != 120:
        raise ValueError("claimable-mass census is not the complete 120-repository artifact")
    if census.get("validation", {}).get("source_completeness_requires_raw_replay") is not True:
        raise ValueError("claimable-mass validation scope is not explicit")
    if census.get("corpus_manifest_sha256") != sha256_file(ROOT / "bench/goldens/corpus.json"):
        raise ValueError("claimable-mass corpus manifest hash drift")
    if census.get("prune_manifest_sha256") != sha256_file(
        ROOT / "bench/labels/prune_manifest.json"
    ):
        raise ValueError("claimable-mass prune manifest hash drift")
    if (
        evidence.get("schema") != "nose-soundness-census-run/v2"
        or not evidence.get("complete")
        or len(evidence["repositories"]) != 120
        or census["evidence_sha256"] != sha256_bytes(pretty(evidence))
        or census["nose"] != evidence["nose"]
    ):
        raise ValueError("claimable-mass census provenance drift")
    evidence_repos = {row["id"]: row for row in evidence["repositories"]}
    observed_repos = {row["id"]: row for row in census["repositories"]}
    if len(observed_repos) != 120 or set(observed_repos) != set(evidence_repos):
        raise ValueError("claimable-mass repository evidence drift")
    families = census["claimable_families"]
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in families:
        if (
            family["excluded_units"] < 1
            or not family["blockers"]
            or family["units"] != family["interpretable_units"] + family["excluded_units"]
        ):
            raise ValueError("claimable family lacks valid attributed membership")
        expected_mass = pairs(family["units"]) - pairs(family["interpretable_units"])
        if (
            family["claimable_pair_mass"] != expected_mass
            or family["capped_claimable_pair_mass"] != min(PAIR_CAP, expected_mass)
            or sum(row["excluded_units"] for row in family["blockers"])
            != family["excluded_units"]
        ):
            raise ValueError("claimable family aggregate drift")
        by_repo[family["repository"]].append(family)
    total_classifications: Counter[str] = Counter()
    total_capabilities: Counter[str] = Counter()
    total_units: Counter[str] = Counter()
    for repo_id, repository in observed_repos.items():
        source = evidence_repos[repo_id]
        repo_families = sorted(
            by_repo.get(repo_id, []),
            key=lambda row: (row["repository"], row["fingerprint_sha256"]),
        )
        validate_repository_commitments(repo_id, repository, source, repo_families)
        total_units.update(
            total=repository["units"],
            interpreted=repository["interpretable_units"],
            excluded=repository["excluded_units"],
        )
        total_classifications.update(repository["by_classification"])
        total_capabilities.update(repository["by_capability"])
    expected_summary = {
        "repositories": len(observed_repos),
        **dict(total_units),
        "generic_unattributed_exclusions": 0,
        "claimable_families": len(families),
        "claimable_pair_mass": sum(row["claimable_pair_mass"] for row in families),
        "capped_claimable_pair_mass": sum(
            row["capped_claimable_pair_mass"] for row in families
        ),
        "by_classification": dict(sorted(total_classifications.items())),
        "by_capability": dict(sorted(total_capabilities.items())),
    }
    if census["summary"] != expected_summary:
        raise ValueError("claimable-mass summary drift")
    expected = priority_from_families(census)
    if priority != expected:
        raise ValueError("interpreter priority does not match claimable families")


def self_test() -> None:
    blocker = {
        "category": "il", "capability_id": "il.test",
        "blocker_stack": [{"role": "eval", "construct": "kind:Var"}],
    }
    units = [
        {"loc": "a:1", "language": "rust", "reason": "interpretable",
         "exact_safe": True, "product_admission": "admitted", "claimable": True,
         "classification": "interpretable",
         "obligation_family": "interpretable", "obligation_subreason": "interpretable",
         "value_fingerprint": [1, 2, 3, 4], "constructs": []},
        {"loc": "a:2", "language": "rust", "reason": "battery-bail",
         "exact_safe": True, "product_admission": "admitted", "claimable": True,
         "classification": "missing-oracle-support",
         "obligation_family": "oracle-capability", "obligation_subreason": "il.test",
         "value_fingerprint": [1, 2, 3, 4], "constructs": [], "first_blocker": blocker},
    ]
    before = priority_rows(units, PAIR_CAP)
    poisoned = copy.deepcopy(units)
    for index in range(100):
        row = copy.deepcopy(units[1])
        row.update(loc=f"unsafe:{index}", exact_safe=False, claimable=False,
                   value_fingerprint=[9, 9, 9, 9])
        poisoned.append(row)
    if priority_rows(poisoned, PAIR_CAP) != before:
        raise AssertionError("exact-unsafe cluster changed implementation priority")
    product_ineligible = copy.deepcopy(units)
    for index in range(100):
        row = copy.deepcopy(units[1])
        row.update(
            loc=f"large-test:{index}",
            product_admission="large-test-file",
            claimable=False,
            value_fingerprint=[8, 8, 8, 8],
        )
        product_ineligible.append(row)
    if priority_rows(product_ineligible, PAIR_CAP) != before:
        raise AssertionError("product-ineligible cluster changed implementation priority")
    broken = copy.deepcopy(units)
    del broken[1]["first_blocker"]
    raw = {
        "schema": RAW_SCHEMA, "units_total": 2, "interpretable_units": 1,
        "generic_unattributed_exclusions": 0, "claimable_family_cap": PAIR_CAP,
        "merge_pairs": count_pairs(iter(broken)),
        "claimable_merge_pairs": count_pairs(iter(broken)), "priority": [], "units": broken,
    }
    try:
        validate_raw(raw)
    except ValueError:
        pass
    else:
        raise AssertionError("missing blocker was accepted")
    valid_raw = {
        "schema": RAW_SCHEMA,
        "units_total": 2,
        "interpretable_units": 1,
        "generic_unattributed_exclusions": 0,
        "claimable_family_cap": PAIR_CAP,
        "merge_pairs": count_pairs(iter(units)),
        "claimable_merge_pairs": count_pairs(iter(units)),
        "priority": before,
        "units": units,
    }
    receipt = current_attribution_receipt(
        valid_raw, "1" * 64, "2" * 40, "3" * 40, "4" * 64
    )
    if receipt["summary"] != {
        "total_units": 2,
        "interpretable_units": 1,
        "excluded_units": 1,
        "generic_unattributed_exclusions": 0,
        "by_classification": {"missing-oracle-support": 1},
        "by_capability": {"il.test": 1},
    }:
        raise AssertionError("current attribution receipt did not preserve validated counts")
    with tempfile.TemporaryDirectory() as tmp:
        first = Path(tmp) / "first"
        peer = Path(tmp) / "peer"
        first.write_bytes(b"same\n")
        peer.write_bytes(b"different\n")
        try:
            require_byte_identical(first, peer, "self-test evidence")
        except ValueError:
            pass
        else:
            raise AssertionError("mismatched 1/4-thread evidence was accepted")
        peer.write_bytes(b"same\n")
        require_byte_identical(first, peer, "self-test evidence")
        alias = Path(tmp) / "alias"
        alias.hardlink_to(first)
        for duplicate in (first, alias):
            try:
                require_byte_identical(first, duplicate, "self-test evidence")
            except ValueError:
                pass
            else:
                raise AssertionError("non-distinct 1/4-thread evidence was accepted")
    family = {
        "repository": "fixture",
        "fingerprint_sha256": "a",
        "claimable_pair_mass": 1,
        "capped_claimable_pair_mass": 1,
    }
    source = {
        "commit": "pin",
        "census_sha256": "raw",
        "units": 2,
        "interpretable_units": 1,
        "excluded_units": 1,
        "generic_unattributed_exclusions": 0,
        "by_classification": {"missing-oracle-support": 1},
        "by_capability": {"il.test": 1},
        "claimable_units": 2,
        "claimable_units_sha256": "units",
        "claimable_families": 1,
        "claimable_families_sha256": sha256_bytes(pretty([family])),
        "claimable_pair_mass": 1,
        "capped_claimable_pair_mass": 1,
    }
    validate_repository_commitments("fixture", copy.deepcopy(source), source, [family])
    try:
        validate_repository_commitments("fixture", copy.deepcopy(source), source, [])
    except ValueError:
        pass
    else:
        raise AssertionError("deleted claimable family passed commitment validation")
    raw_row = copy.deepcopy(units[1])
    raw_row["loc"] = "fixture.rs:2-3@10-20"
    ledger_row = {
        "raw_loc": raw_row["loc"],
        "path": "fixture.rs",
        "start_line": 2,
        "end_line": 3,
        **{
            key: raw_row[key]
            for key in (
                "language", "value_fingerprint", "exact_safe", "product_admission",
                "claimable", "classification", "obligation_family",
                "obligation_subreason", "constructs", "first_blocker",
            )
        },
    }
    validate_ledger_raw_fields(ledger_row, raw_row)
    ledger_row["first_blocker"] = {
        "category": "il", "capability_id": "il.forged", "blocker_stack": []
    }
    try:
        validate_ledger_raw_fields(ledger_row, raw_row)
    except ValueError:
        pass
    else:
        raise AssertionError("forged blocker passed raw-ledger validation")
    print("ok soundness exclusion attribution self-test")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--freeze-baseline", action="store_true")
    parser.add_argument("--freeze-corpus", action="store_true")
    parser.add_argument("--summarize-current", action="store_true")
    parser.add_argument("--census", type=Path)
    parser.add_argument("--census-peer", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--report-peer", type=Path)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--raw-dir", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--source-commit")
    parser.add_argument("--binary-sha256")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.summarize_current:
        if not all((args.census, args.source_commit, args.binary_sha256, args.output)):
            parser.error(
                "--summarize-current requires --census, --source-commit, "
                "--binary-sha256, and --output"
            )
        summarize_current(args.census, args.source_commit, args.binary_sha256, args.output)
        return
    if args.freeze_baseline:
        if not all((
            args.census, args.census_peer, args.report, args.report_peer, args.source_root
        )):
            parser.error(
                "--freeze-baseline requires --census, --census-peer, --report, "
                "--report-peer, and --source-root"
            )
        ledger = freeze_baseline(
            args.census,
            args.census_peer,
            args.report,
            args.report_peer,
            args.source_root,
        )
        write(args.baseline / "exclusion-ledger.v2.json", ledger)
        shutil.copyfile(args.census, args.baseline / "exclusion-census.v2.json")
    elif args.freeze_corpus:
        if not all((args.raw_dir, args.evidence)):
            parser.error("--freeze-corpus requires --raw-dir and --evidence")
        census, priority, evidence = aggregate_corpus(args.raw_dir, args.evidence)
        write(args.baseline / "claimable-mass-census.v2.json", census)
        write(args.baseline / "interpreter-priority.v2.json", priority)
        write(args.baseline / "corpus-census-evidence.v2.json", evidence)
    else:
        checked_ledger = load(args.baseline / "exclusion-ledger.v2.json")
        checked_raw_path = args.baseline / "exclusion-census.v2.json"
        checked_raw = load(checked_raw_path)
        checked_census = load(args.baseline / "claimable-mass-census.v2.json")
        checked_priority = load(args.baseline / "interpreter-priority.v2.json")
        checked_evidence = load(args.baseline / "corpus-census-evidence.v2.json")
        validate_ledger(checked_ledger, checked_raw, sha256_file(checked_raw_path))
        validate_corpus(checked_census, checked_priority, checked_evidence)
        if args.raw_dir or args.evidence:
            if not all((args.raw_dir, args.evidence)):
                parser.error("raw replay requires both --raw-dir and --evidence")
            replay_census, replay_priority, replay_evidence = aggregate_corpus(
                args.raw_dir, args.evidence
            )
            if (
                replay_census != checked_census
                or replay_priority != checked_priority
                or replay_evidence != checked_evidence
            ):
                raise ValueError("raw corpus replay does not match checked artifacts")
            print("ok soundness exclusion attribution evidence (raw replay complete)")
        else:
            print(
                "ok soundness exclusion attribution evidence "
                "(commitments verified; source completeness requires --raw-dir replay)"
            )
if __name__ == "__main__":
    main()
