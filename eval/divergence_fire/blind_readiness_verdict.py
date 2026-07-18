#!/usr/bin/env python3
"""Emit the #853 pre-unseal verdict when #852 froze no blind candidate.

This command intentionally has no option for a private packet. If a candidate was
qualified, it stops and requires the separate sealed replay workflow instead of
silently opening or replacing that evaluation.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_QUALIFICATION = ROOT / "eval/divergence_fire/v3_policy_dev_2026_07_18.v1.json"
DEFAULT_PROTOCOL = ROOT / "eval/divergence_fire/precision_protocol_2026_07_14.v2.json"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def checked_input(path):
    return {
        "path": str(path.relative_to(ROOT)),
        "bytes": path.stat().st_size,
        "sha256": sha256(path),
    }


def verdict(args):
    qualification = json.loads(args.qualification.read_text())
    protocol = json.loads(args.protocol.read_text())
    result = qualification.get("qualification", {})
    require(result.get("status") == "no-policy-qualifies", "#852 did not record no-policy-qualifies")
    require(
        result.get("candidate_frozen_for_blind_replay") is False,
        "a blind candidate exists; use the sealed replay workflow instead",
    )
    require(
        qualification["inputs"]["precision_protocol_sha256"] == sha256(args.protocol),
        "#852 qualification references a different precision protocol",
    )
    allowed = protocol["decision_matrix"]["allowed_verdicts"]
    require("failed" in allowed, "sealed protocol does not allow the failed verdict")
    require(protocol.get("state") == "sealed-unjudged", "blind protocol is no longer sealed-unjudged")
    require(
        protocol["verdict_protocol"].get("state") == "unopened-no-quality-labels-exist",
        "blind quality-label state changed before preflight",
    )

    artifact = {
        "schema_version": 1,
        "issue": 853,
        "verdict": "failed",
        "verdict_count": 1,
        "stage": "pre-unseal-development-qualification",
        "blind_or_temporal_data_accessed": False,
        "inputs": {
            "development_qualification": checked_input(args.qualification),
            "precision_protocol": checked_input(args.protocol),
        },
        "generator": {
            "path": str(Path(__file__).relative_to(ROOT)),
            "sha256": sha256(Path(__file__)),
            "source_commit": subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip(),
        },
        "preflight": {
            "qualification_status": result["status"],
            "candidate_frozen_for_blind_replay": False,
            "policy_hash_available": qualification.get("frozen_policy_class_sha256") is not None,
            "binary_hash_available": qualification.get("implementation", {}).get(
                "nose_binary_sha256"
            ) is not None,
            "runnable_candidate_identity_available": False,
            "protocol_state_before": protocol["state"],
            "quality_label_state_before": protocol["verdict_protocol"]["state"],
        },
        "blind_replay": {
            "run": False,
            "repositories_opened": 0,
            "changes_replayed": 0,
            "strict_findings": None,
            "strict_targets": None,
            "labels_created_or_revealed": 0,
            "population_consumed": False,
        },
        "decision": {
            "default_on_ready": False,
            "opt_in_only": True,
            "active_runtime_policy": result["active_runtime_policy"],
            "reason": (
                "#852 froze no runnable v3 candidate after the admissible class had zero "
                "development support. Opening the held-out population could not evaluate a "
                "defined candidate and would only spend the seal."
            ),
            "classification_basis": (
                "failed before unseal because the frozen-candidate identity required by #853 "
                "does not exist"
            ),
        },
        "held_out_state_after": {
            "protocol": "sealed-unjudged",
            "quality_labels": "unopened-no-quality-labels-exist",
            "consumed": False,
        },
        "next_action": (
            "#854 must ship only the supported opt-in/review claim and close performance, "
            "determinism, compatibility, CI, and documentation evidence. Any future v3 cycle "
            "needs target-adjudicated development evidence and a newly qualified candidate "
            "before this held-out seal is opened."
        ),
    }
    args.out.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def selftest(_args):
    allowed = ["default-on-ready", "improved-opt-in-only", "failed", "insufficient-evidence"]
    require("failed" in allowed, "failed verdict missing")
    try:
        require(False, "fixture")
    except SystemExit as error:
        assert str(error) == "fixture"
    else:
        raise AssertionError("require did not stop")
    print("blind_readiness_verdict selftest: ok")


def parser():
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    command = commands.add_parser("emit")
    command.add_argument("--qualification", type=Path, default=DEFAULT_QUALIFICATION)
    command.add_argument("--protocol", type=Path, default=DEFAULT_PROTOCOL)
    command.add_argument("--out", type=Path, required=True)
    command.set_defaults(func=verdict)
    command = commands.add_parser("selftest")
    command.set_defaults(func=selftest)
    return root


if __name__ == "__main__":
    arguments = parser().parse_args()
    arguments.func(arguments)
