#!/usr/bin/env python3
"""Development-only qualification for the #852 divergent-edit v3 policy.

The admissible policy class is monotone: every hard-block target must satisfy all
protocol-required positive evidence, and closed variant signals may only demote it.
The harness reads the public development sample and verdicts plus a scratch replay;
it never opens the sealed blind packet or temporal reserve.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess

import semantic_witness_eval as replay


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SAMPLES = ROOT / "eval/divergence_fire/sampled_findings_2026_07_06.jsonl"
DEFAULT_VERDICTS = ROOT / "eval/divergence_fire/verdicts_2026_07_06.jsonl"
DEFAULT_PROTOCOL = ROOT / "eval/divergence_fire/precision_protocol_2026_07_14.v2.json"
DEFAULT_NOSE = ROOT / "target/release/nose"
QUALIFIED_CHANGE_KINDS = ("replacement", "deletion")
QUALIFIED_ALIGNMENTS = ("exact-span", "stable-name", "changed-range")
TARGET_PREDICATE_NAMES = (
    "direct_target",
    "shared_contact",
    "complete_semantic_witness",
    "qualified_change_kind",
    "qualified_alignment",
    "mapped_shared_node",
    "semantic_facets",
    "semantic_projections_ok",
    "no_semantic_caveat",
    "no_strong_variant",
    "no_variant_caveat",
)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_sha256(value):
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def target_predicates(target):
    semantic = target.get("changed", {}).get("semantic_change") or {}
    variant = target.get("variant_evidence") or {}
    coverage = semantic.get("coverage") or {}
    direct = target.get("direct_witness") or {}
    return {
        "direct_target": bool(target.get("target_id") and direct.get("kind")),
        "shared_contact": target.get("changed", {}).get("touches_shared") is True,
        "complete_semantic_witness": semantic.get("status") == "complete",
        "qualified_change_kind": semantic.get("change_kind") in QUALIFIED_CHANGE_KINDS,
        "qualified_alignment": semantic.get("alignment") in QUALIFIED_ALIGNMENTS,
        "mapped_shared_node": coverage.get("mapped_shared_nodes", 0) > 0,
        "semantic_facets": bool(semantic.get("facets")),
        "semantic_projections_ok": (
            semantic.get("base_projection") == "ok"
            and semantic.get("current_projection") == "ok"
        ),
        "no_semantic_caveat": not semantic.get("caveats"),
        "no_strong_variant": variant.get("status") != "disqualifying",
        "no_variant_caveat": not variant.get("caveats"),
    }


def strict_target(target):
    return all(target_predicates(target).values())


def relaxed_diagnostic_target(target):
    """Non-policy diagnostic: prices mapped evidence while allowing known caveats."""
    predicates = target_predicates(target)
    return all(
        predicates[name]
        for name in (
            "direct_target",
            "shared_contact",
            "qualified_change_kind",
            "qualified_alignment",
            "mapped_shared_node",
            "semantic_facets",
            "semantic_projections_ok",
            "no_strong_variant",
        )
    )


def finding_summary(name, rows, target_selector):
    selected = []
    target_count = 0
    for row in rows:
        targets = [target for target in row["targets"] if target_selector(target)]
        if targets:
            selected.append(row)
            target_count += len(targets)
    positives = sum(row["verdict"] == "should_propagate" for row in selected)
    return {
        "name": name,
        "strict_findings": len(selected),
        "strict_targets": target_count,
        "should_propagate_findings": positives,
        "false_positive_findings": len(selected) - positives,
        "finding_precision": round(positives / len(selected), 6) if selected else None,
        "verdicts": dict(sorted(Counter(row["verdict"] for row in selected).items())),
        "target_precision": None,
        "target_precision_reason": "development labels adjudicate findings, not direct targets",
    }


def summarize(args):
    samples = {row["sid"]: row for row in replay.jsonl(args.samples)}
    verdicts = {row["sid"]: row for row in replay.jsonl(args.verdicts)}
    record_rows = replay.jsonl(args.records)
    records = {row.get("sid"): row for row in record_rows if row.get("sid")}
    rows = []
    for sid, sample in samples.items():
        if not replay.current_v2_strict(sample):
            continue
        record = records.get(sid, {})
        rows.append({
            "sid": sid,
            "verdict": verdicts[sid]["verdict"],
            "targets": [
                target for target in record.get("targets", []) if isinstance(target, dict)
            ],
        })

    policy_spec = {
        "policy_class": "divergent-edit-v3-monotone-precision-first",
        "finding_fails_when": "at least one target has disposition strict",
        "target_requires_all": list(TARGET_PREDICATE_NAMES),
        "allowed_tuning": "closed strong variant codes may only demote",
        "unknown_or_caveated": "review",
        "weak_variant_signals": "advisory-only",
        "gate_authority": "items[].gate.fail_default",
    }
    qualified = finding_summary("protocol-admissible-v3", rows, strict_target)
    relaxed = finding_summary(
        "diagnostic-only-allow-incomplete-semantic-and-variant-caveats",
        rows,
        relaxed_diagnostic_target,
    )
    strict_targets = [
        target for row in rows for target in row["targets"] if strict_target(target)
    ]
    complete_targets = [
        target
        for row in rows
        for target in row["targets"]
        if target_predicates(target)["complete_semantic_witness"]
    ]
    protocol = json.loads(args.protocol.read_text())
    artifact = {
        "schema_version": 1,
        "issue": 852,
        "development_only": True,
        "blind_or_temporal_data_accessed": False,
        "inputs": {
            "samples": str(args.samples.relative_to(ROOT)),
            "samples_sha256": sha256(args.samples),
            "verdicts": str(args.verdicts.relative_to(ROOT)),
            "verdicts_sha256": sha256(args.verdicts),
            "precision_protocol": str(args.protocol.relative_to(ROOT)),
            "precision_protocol_sha256": sha256(args.protocol),
            "labeled_findings": len(samples),
            "v2_strict_findings": len(rows),
            "replay_records": str(args.records),
            "replay_records_sha256": sha256(args.records),
        },
        "implementation": {
            "source_commit": subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip(),
            "nose_binary": str(args.nose),
            "nose_binary_sha256": sha256(args.nose),
            "nose_version": subprocess.run(
                [str(args.nose), "--version"],
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip(),
            "harness": str(Path(__file__).relative_to(ROOT)),
            "harness_sha256": sha256(Path(__file__)),
        },
        "frozen_policy_class": policy_spec,
        "frozen_policy_class_sha256": canonical_sha256(policy_spec),
        "replay": {
            "matched_findings": sum(
                records.get(sid, {}).get("matched") is True for sid in samples
            ),
            "unmatched_findings": sum(
                records.get(sid, {}).get("matched") is not True for sid in samples
            ),
            "query_error_rows": sum(row.get("ok") is not True for row in record_rows),
        },
        "qualification_thresholds": {
            "strict_finding_precision_min": 0.95,
            "strict_target_precision_min": protocol["decision_matrix"]["blind_policy_gate"][
                "strict_target_precision_min"
            ],
            "non_degenerate_strict_findings_min": 1,
            "non_degenerate_strict_targets_min": 1,
        },
        "evidence_ceiling": {
            "direct_targets": sum(len(row["targets"]) for row in rows),
            "complete_semantic_witness_targets": len(complete_targets),
            "protocol_admissible_targets": len(strict_targets),
            "proof": (
                "Every admissible target requires a complete semantic witness. The development "
                "replay has none, so every monotone policy in the frozen class has zero support."
            ),
        },
        "simulations": [qualified, relaxed],
        "qualification": {
            "status": "no-policy-qualifies",
            "candidate_frozen_for_blind_replay": False,
            "active_runtime_policy": "divergent-edit-v2-strict",
            "schema_or_capability_bump": False,
            "reason": (
                "The protocol-admissible class has zero development support; target precision "
                "also cannot be estimated from finding-level development labels. Activating or "
                "versioning a v3 hard-block contract would overstate the evidence."
            ),
        },
    }
    args.out.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def selftest(_args):
    complete = {
        "target_id": "abc",
        "direct_witness": {"kind": "exact-value-graph"},
        "changed": {
            "touches_shared": True,
            "semantic_change": {
                "status": "complete",
                "change_kind": "replacement",
                "alignment": "stable-name",
                "base_projection": "ok",
                "current_projection": "ok",
                "coverage": {"mapped_shared_nodes": 1},
                "facets": ["value"],
                "caveats": [],
            },
        },
        "variant_evidence": {"status": "none", "caveats": []},
    }
    assert strict_target(complete)
    caveated = json.loads(json.dumps(complete))
    caveated["changed"]["semantic_change"]["caveats"] = ["lossy-base-lowering"]
    assert not strict_target(caveated)
    assert relaxed_diagnostic_target(caveated)
    variant = json.loads(json.dumps(complete))
    variant["variant_evidence"]["status"] = "disqualifying"
    assert not strict_target(variant)
    print("v3_policy_eval selftest: ok")


def parser():
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    command = commands.add_parser("summarize")
    command.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    command.add_argument("--verdicts", type=Path, default=DEFAULT_VERDICTS)
    command.add_argument("--protocol", type=Path, default=DEFAULT_PROTOCOL)
    command.add_argument("--records", type=Path, required=True)
    command.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    command.add_argument("--out", type=Path, required=True)
    command.set_defaults(func=summarize)
    command = commands.add_parser("selftest")
    command.set_defaults(func=selftest)
    return root


if __name__ == "__main__":
    arguments = parser().parse_args()
    arguments.func(arguments)
