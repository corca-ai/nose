#!/usr/bin/env python3
"""Development-only direct-target pricing for #850.

Consumes only the checked 2026-07-06 development sample/verdicts and scratch replay
rows produced by semantic_witness_eval.py. It never reads the sealed blind packet or
temporal reserve.
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


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def site_key(site):
    return (
        site.get("file"), site.get("start_line"), site.get("end_line"),
        site.get("lang"), site.get("kind"), site.get("name"),
    )


def target_predicates(row):
    targets = [target for target in row.get("targets", []) if isinstance(target, dict)]
    direct_shared = [
        target for target in targets
        if target.get("changed", {}).get("touches_shared") is True
    ]
    skipped_targets = {site_key(target.get("skipped", {})) for target in targets}
    transitive_context = [
        site for site in row.get("not_updated", [])
        if site_key(site) not in skipped_targets
    ]
    return {
        "target_count": len(targets),
        "direct_target_present": bool(targets),
        "direct_shared_target_present": bool(direct_shared),
        "direct_shared_target_count": len(direct_shared),
        "transitive_context_count": len(transitive_context),
    }


def simulation(name, rows, predicate):
    selected = [row for row in rows if predicate(row)]
    positives = sum(row["verdict"] == "should_propagate" for row in selected)
    baseline_positives = sum(row["verdict"] == "should_propagate" for row in rows)
    return {
        "name": name,
        "selected": len(selected),
        "should_propagate": positives,
        "false_positives": len(selected) - positives,
        "precision": round(positives / len(selected), 6) if selected else None,
        "should_propagate_retention": (
            round(positives / baseline_positives, 6) if baseline_positives else None
        ),
        "verdicts": dict(sorted(Counter(row["verdict"] for row in selected).items())),
    }


def nested_counts(values):
    return {
        key: dict(sorted(counter.items()))
        for key, counter in sorted(values.items())
    }


def summarize(args):
    samples = {row["sid"]: row for row in replay.jsonl(args.samples)}
    verdicts = {row["sid"]: row for row in replay.jsonl(args.verdicts)}
    records = {row.get("sid"): row for row in replay.jsonl(args.records) if row.get("sid")}
    joined = []
    for sid, sample in samples.items():
        record = records.get(sid, {})
        joined.append({
            **record,
            "sid": sid,
            "verdict": verdicts[sid]["verdict"],
            "baseline_strict": replay.current_v2_strict(sample),
            "target_predicates": target_predicates(record),
        })

    strict = [row for row in joined if row["baseline_strict"]]
    matched = [row for row in joined if row.get("matched")]
    query_errors = [row for row in replay.jsonl(args.records) if not row.get("ok")]
    direct_demoted = [
        row for row in strict
        if not row["target_predicates"]["direct_shared_target_present"]
    ]
    target_kinds = defaultdict(Counter)
    target_semantic_status = defaultdict(Counter)
    for row in strict:
        verdict = row["verdict"]
        for target in row.get("targets", []):
            target_kinds[target.get("direct_witness", {}).get("kind", "missing")][verdict] += 1
            status = target.get("changed", {}).get("semantic_change", {}).get("status", "missing")
            target_semantic_status[status][verdict] += 1

    binary_version = subprocess.run(
        [str(args.nose), "--version"], capture_output=True, text=True, check=False
    ).stdout.strip()
    source_commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
    ).stdout.strip()
    artifact = {
        "schema_version": 1,
        "issue": 850,
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
            "source_commit": source_commit,
            "nose_binary": str(args.nose),
            "nose_binary_sha256": sha256(args.nose),
            "nose_version": binary_version,
            "summary_harness": str(Path(__file__).relative_to(ROOT)),
            "summary_harness_sha256": sha256(Path(__file__)),
            "replay_harness": "eval/divergence_fire/semantic_witness_eval.py",
            "replay_harness_sha256": sha256(ROOT / "eval/divergence_fire/semantic_witness_eval.py"),
        },
        "replay": {
            "matched_findings": len(matched),
            "unmatched_findings": len(joined) - len(matched),
            "query_error_rows": len(query_errors),
        },
        "baseline_v2_strict": {
            "findings": len(strict),
            "verdicts": dict(sorted(Counter(row["verdict"] for row in strict).items())),
            "targets": sum(row["target_predicates"]["target_count"] for row in strict),
            "direct_shared_targets": sum(
                row["target_predicates"]["direct_shared_target_count"] for row in strict
            ),
            "transitive_review_context_sites": sum(
                row["target_predicates"]["transitive_context_count"] for row in strict
            ),
        },
        "direct_requirement_effect": {
            "demoted_findings": len(direct_demoted),
            "demotions_by_verdict": dict(sorted(Counter(row["verdict"] for row in direct_demoted).items())),
            "not_a_clone_baseline": sum(row["verdict"] == "not_a_clone" for row in strict),
            "not_a_clone_demoted": sum(row["verdict"] == "not_a_clone" for row in direct_demoted),
            "not_a_clone_retained": sum(
                row["verdict"] == "not_a_clone"
                and row["target_predicates"]["direct_shared_target_present"]
                for row in strict
            ),
        },
        "target_evidence_by_verdict": {
            "witness_kind": nested_counts(target_kinds),
            "semantic_status": nested_counts(target_semantic_status),
        },
        "simulations": [
            simulation("current-v2-strict", strict, lambda row: True),
            simulation(
                "require-direct-target",
                strict,
                lambda row: row["target_predicates"]["direct_target_present"],
            ),
            simulation(
                "require-direct-target-with-shared-contact",
                strict,
                lambda row: row["target_predicates"]["direct_shared_target_present"],
            ),
        ],
        "interpretation": (
            "Development evidence only. Direct targets remove transitive endpoints from the "
            "action surface; this issue does not tune or freeze the v3 policy and does not "
            "access blind or temporal labels."
        ),
    }
    args.out.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def selftest(_args):
    row = {
        "not_updated": [
            {"file": "b.py", "start_line": 1, "end_line": 8},
            {"file": "c.py", "start_line": 1, "end_line": 8},
        ],
        "targets": [{
            "skipped": {"file": "b.py", "start_line": 1, "end_line": 8},
            "changed": {"touches_shared": True},
        }],
    }
    predicates = target_predicates(row)
    assert predicates["direct_shared_target_present"]
    assert predicates["transitive_context_count"] == 1
    print("direct_target_eval selftest: ok")


def runtime(args):
    samples = replay.jsonl(args.samples)
    queries = replay.runtime_queries(samples)
    binaries = {
        "baseline": args.baseline.resolve(),
        "current": args.current.resolve(),
    }
    rows = []
    compatibility = {}
    for sample in queries:
        repo_rows, legacy_equal = replay.runtime_repo(
            sample,
            args.repos_root,
            binaries,
            args.iterations,
            args.warmups,
            args.timeout,
        )
        rows.extend(repo_rows)
        compatibility[sample["repo"]] = legacy_equal
        print(f"[{sample['repo']}] runtime complete", flush=True)
    output = {
        "schema_version": 1,
        "issue": 850,
        "command": "nose query . base=<parent> top=0 --format json",
        "development_only": True,
        "blind_or_temporal_data_accessed": False,
        "inputs": {
            "samples": str(args.samples.relative_to(ROOT)),
            "samples_sha256": sha256(args.samples),
            "harness": str(Path(__file__).relative_to(ROOT)),
            "harness_sha256": sha256(Path(__file__)),
            "shared_harness": "eval/divergence_fire/semantic_witness_eval.py",
            "shared_harness_sha256": sha256(
                ROOT / "eval/divergence_fire/semantic_witness_eval.py"
            ),
            "selection": "first current-v2-strict default-arm finding per repository",
        },
        "configuration": {
            "iterations": args.iterations,
            "warmups": args.warmups,
            "timeout_s": args.timeout,
            "repositories": [sample["repo"] for sample in queries],
        },
        "binaries": {
            label: {
                "path": str(binary),
                "sha256": sha256(binary),
                "version": subprocess.run(
                    [str(binary), "--version"],
                    capture_output=True,
                    text=True,
                    check=False,
                ).stdout.strip(),
            }
            for label, binary in binaries.items()
        },
        "legacy_output_compatibility": {
            "all_equal_after_removing_semantic_change_and_targets": all(
                compatibility.values()
            ),
            "by_repo": compatibility,
        },
        "runs": rows,
        "summary": replay.runtime_summary(
            rows, [sample["repo"] for sample in queries]
        ),
    }
    args.out.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")


def parser():
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    command = commands.add_parser("summarize")
    command.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    command.add_argument("--verdicts", type=Path, default=DEFAULT_VERDICTS)
    command.add_argument("--records", type=Path, required=True)
    command.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    command.add_argument("--out", type=Path, required=True)
    command.set_defaults(func=summarize)
    command = commands.add_parser("selftest")
    command.set_defaults(func=selftest)
    command = commands.add_parser("runtime")
    command.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    command.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    command.add_argument("--baseline", type=Path, required=True)
    command.add_argument("--current", type=Path, required=True)
    command.add_argument("--iterations", type=int, default=3)
    command.add_argument("--warmups", type=int, default=1)
    command.add_argument("--timeout", type=int, default=240)
    command.add_argument("--out", type=Path, required=True)
    command.set_defaults(func=runtime)
    return root


if __name__ == "__main__":
    arguments = parser().parse_args()
    arguments.func(arguments)
