#!/usr/bin/env python3
"""Development-only pricing for #851 target-local variant evidence.

This harness consumes only the checked 2026-07-06 development sample/verdicts and
scratch replay rows. It never reads the sealed blind packet or temporal reserve.
"""

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import subprocess

import semantic_witness_eval as replay


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SAMPLES = ROOT / "eval/divergence_fire/sampled_findings_2026_07_06.jsonl"
DEFAULT_VERDICTS = ROOT / "eval/divergence_fire/verdicts_2026_07_06.jsonl"
DEFAULT_NOSE = ROOT / "target/release/nose"
STRONG_CODES = (
    "referent-mismatch",
    "decorator-mismatch",
    "async-role-mismatch",
    "effect-role-mismatch",
    "protocol-role-mismatch",
    "disjoint-platform-guard",
)
WEAK_CODES = ("name-mismatch", "path-mismatch", "version-label-mismatch")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def target_evidence(row):
    targets = [
        target for target in row.get("targets", [])
        if isinstance(target, dict) and target.get("changed", {}).get("touches_shared") is True
    ]
    output = []
    for target in targets:
        evidence = target.get("variant_evidence", {})
        signals = [signal for signal in evidence.get("signals", []) if isinstance(signal, dict)]
        output.append({
            "status": evidence.get("status", "missing"),
            "strong_codes": sorted({
                signal.get("code") for signal in signals
                if signal.get("strength") == "strong"
            } - {None}),
            "weak_codes": sorted({
                signal.get("code") for signal in signals
                if signal.get("strength") == "weak"
            } - {None}),
            "caveats": sorted({
                caveat.get("code") for caveat in evidence.get("caveats", [])
                if isinstance(caveat, dict) and caveat.get("code")
            }),
        })
    return output


def counts(rows):
    return dict(sorted(Counter(row["verdict"] for row in rows).items()))


def policy_summary(name, baseline, retained):
    positives = sum(row["verdict"] == "should_propagate" for row in retained)
    baseline_positives = sum(row["verdict"] == "should_propagate" for row in baseline)
    return {
        "name": name,
        "selected": len(retained),
        "should_propagate": positives,
        "false_positives": len(retained) - positives,
        "precision": round(positives / len(retained), 6) if retained else None,
        "should_propagate_retention": (
            round(positives / baseline_positives, 6) if baseline_positives else None
        ),
        "verdicts": counts(retained),
    }


def signal_effect(rows, code):
    exposed = [row for row in rows if any(code in target["strong_codes"] for target in row["evidence"])]
    fully_demoted = [
        row for row in rows
        if row["evidence"] and all(code in target["strong_codes"] for target in row["evidence"])
    ]
    return {
        "findings_exposed": len(exposed),
        "exposed_by_verdict": counts(exposed),
        "findings_fully_demoted": len(fully_demoted),
        "false_positives_removed": sum(
            row["verdict"] != "should_propagate" for row in fully_demoted
        ),
        "true_positives_demoted": sum(
            row["verdict"] == "should_propagate" for row in fully_demoted
        ),
        "fully_demoted_by_verdict": counts(fully_demoted),
    }


def summarize(args):
    samples = {row["sid"]: row for row in replay.jsonl(args.samples)}
    verdicts = {row["sid"]: row for row in replay.jsonl(args.verdicts)}
    records = {row.get("sid"): row for row in replay.jsonl(args.records) if row.get("sid")}
    joined = []
    for sid, sample in samples.items():
        record = records.get(sid, {})
        joined.append({
            "sid": sid,
            "verdict": verdicts[sid]["verdict"],
            "baseline_strict": replay.current_v2_strict(sample),
            "matched": record.get("matched") is True,
            "evidence": target_evidence(record),
        })

    strict = [row for row in joined if row["baseline_strict"]]
    demoted = [
        row for row in strict
        if row["evidence"] and all(target["status"] == "disqualifying" for target in row["evidence"])
    ]
    retained = [row for row in strict if row not in demoted]
    target_signals = defaultdict(Counter)
    target_caveats = defaultdict(Counter)
    for row in strict:
        for target in row["evidence"]:
            for code in target["strong_codes"] + target["weak_codes"]:
                target_signals[code][row["verdict"]] += 1
            for code in target["caveats"]:
                target_caveats[code][row["verdict"]] += 1

    weak_only = [
        target
        for row in strict
        for target in row["evidence"]
        if target["weak_codes"] and not target["strong_codes"]
    ]
    intentional = [row for row in strict if row["verdict"] == "intentional_divergence"]
    intentional_demoted = [row for row in demoted if row["verdict"] == "intentional_divergence"]
    binary_version = subprocess.run(
        [str(args.nose), "--version"], capture_output=True, text=True, check=False
    ).stdout.strip()
    artifact = {
        "schema_version": 1,
        "issue": 851,
        "development_only": True,
        "blind_or_temporal_data_accessed": False,
        "inputs": {
            "samples": str(args.samples.relative_to(ROOT)),
            "samples_sha256": sha256(args.samples),
            "verdicts": str(args.verdicts.relative_to(ROOT)),
            "verdicts_sha256": sha256(args.verdicts),
            "labeled_findings": len(samples),
            "replay_records": str(args.records),
        },
        "implementation": {
            "source_commit": subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True
            ).stdout.strip(),
            "nose_binary": str(args.nose),
            "nose_binary_sha256": sha256(args.nose),
            "nose_version": binary_version,
            "summary_harness": str(Path(__file__).relative_to(ROOT)),
            "summary_harness_sha256": sha256(Path(__file__)),
            "replay_harness": "eval/divergence_fire/semantic_witness_eval.py",
            "replay_harness_sha256": sha256(ROOT / "eval/divergence_fire/semantic_witness_eval.py"),
        },
        "replay": {
            "matched_findings": sum(row["matched"] for row in joined),
            "unmatched_findings": sum(not row["matched"] for row in joined),
            "query_error_rows": sum(not row.get("ok", False) for row in replay.jsonl(args.records)),
        },
        "baseline_v2_strict": {
            "findings": len(strict),
            "verdicts": counts(strict),
            "direct_shared_targets": sum(len(row["evidence"]) for row in strict),
        },
        "all_strong_signal_effect": {
            "fully_demoted_findings": len(demoted),
            "fully_demoted_by_verdict": counts(demoted),
            "false_positives_removed": sum(row["verdict"] != "should_propagate" for row in demoted),
            "true_positives_demoted": sum(row["verdict"] == "should_propagate" for row in demoted),
        },
        "intentional_divergence_focus": {
            "baseline_false_positives": len(intentional),
            "fully_demoted": len(intentional_demoted),
            "retained": len(intentional) - len(intentional_demoted),
        },
        "per_strong_signal_effect": {
            code: signal_effect(strict, code) for code in STRONG_CODES
        },
        "target_signals_by_verdict": {
            code: dict(sorted(counter.items())) for code, counter in sorted(target_signals.items())
        },
        "target_caveats_by_verdict": {
            code: dict(sorted(counter.items())) for code, counter in sorted(target_caveats.items())
        },
        "weak_signal_safety": {
            "weak_only_targets": len(weak_only),
            "unexpected_disqualifying_weak_only_targets": sum(
                target["status"] == "disqualifying" for target in weak_only
            ),
            "closed_weak_codes": list(WEAK_CODES),
        },
        "simulations": [
            policy_summary("current-v2-strict", strict, strict),
            policy_summary("exclude-all-strong-variant-targets", strict, retained),
        ],
        "interpretation": (
            "Development evidence only. #851 records deterministic target-local variant evidence "
            "and leaves v2 gate.fail_default unchanged; #852 may consume only the closed strong "
            "codes in a separately frozen policy."
        ),
    }
    args.out.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def selftest(_args):
    row = {
        "targets": [{
            "changed": {"touches_shared": True},
            "variant_evidence": {
                "status": "disqualifying",
                "signals": [
                    {"code": "referent-mismatch", "strength": "strong"},
                    {"code": "path-mismatch", "strength": "weak"},
                ],
                "caveats": [{"code": "unresolved-referent"}],
            },
        }]
    }
    evidence = target_evidence(row)
    assert evidence[0]["strong_codes"] == ["referent-mismatch"]
    assert evidence[0]["weak_codes"] == ["path-mismatch"]
    assert evidence[0]["caveats"] == ["unresolved-referent"]
    print("variant_evidence_eval selftest: ok")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    summary = sub.add_parser("summarize")
    summary.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    summary.add_argument("--verdicts", type=Path, default=DEFAULT_VERDICTS)
    summary.add_argument("--records", type=Path, required=True)
    summary.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    summary.add_argument("--out", type=Path, required=True)
    summary.set_defaults(func=summarize)
    test = sub.add_parser("selftest")
    test.set_defaults(func=selftest)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
