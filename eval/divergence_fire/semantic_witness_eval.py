#!/usr/bin/env python3
"""Development-only replay and pricing for #849 semantic-change witnesses.

This harness reads only the checked 2026-07-06 development sample and verdicts. It
does not read the sealed blind packet or temporal reserve. Raw replay rows are scratch
artifacts; ``summarize`` writes a source-free aggregate suitable for Git.
"""

import argparse
from collections import Counter, defaultdict
import concurrent.futures
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SAMPLES = ROOT / "eval/divergence_fire/sampled_findings_2026_07_06.jsonl"
DEFAULT_VERDICTS = ROOT / "eval/divergence_fire/verdicts_2026_07_06.jsonl"
DEFAULT_NOSE = ROOT / "target/release/nose"


def jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(args, cwd=None, timeout=None):
    return subprocess.run(
        args,
        cwd=cwd,
        capture_output=True,
        text=True,
        errors="replace",
        timeout=timeout,
        check=False,
    )


def query_key(sample):
    return sample["repo"], sample["commit"], sample["parent"], sample["arm"]


def current_v2_strict(sample):
    return (
        sample.get("scope") == "prod"
        and sample.get("fire_eligible") is True
        and any(site.get("touches_shared") is True for site in sample.get("changed", []))
    )


def replay_repo(repo, requests, repos_root, nose, timeout):
    source = repos_root / repo
    rows = []
    with tempfile.TemporaryDirectory(prefix=f"nose-849-{repo}-") as tmp:
        worktree = Path(tmp) / "worktree"
        head = run(["git", "-C", str(source), "rev-parse", "HEAD"])
        if head.returncode != 0:
            return [{"repo": repo, "ok": False, "error": "repo-head-failed"}]
        added = run(
            ["git", "-C", str(source), "worktree", "add", "--detach", "--quiet",
             str(worktree), head.stdout.strip()]
        )
        if added.returncode != 0:
            return [{"repo": repo, "ok": False, "error": "worktree-add-failed"}]
        try:
            for commit, parent, arm, samples in requests:
                checked = run(
                    ["git", "-C", str(worktree), "checkout", "--detach", "--quiet", commit]
                )
                if checked.returncode != 0:
                    rows.append({
                        "repo": repo, "commit": commit, "parent": parent, "arm": arm,
                        "ok": False, "error": "checkout-failed",
                    })
                    continue
                command = [
                    str(nose), "query", ".", f"base={parent}", "top=0", "--format", "json"
                ]
                if arm == "near":
                    command += ["--mode", "syntax,semantic,near"]
                started = time.monotonic()
                try:
                    result = run(command, cwd=worktree, timeout=timeout)
                except subprocess.TimeoutExpired:
                    rows.append({
                        "repo": repo, "commit": commit, "parent": parent, "arm": arm,
                        "ok": False, "error": "timeout",
                    })
                    continue
                duration = round(time.monotonic() - started, 6)
                if result.returncode != 0:
                    rows.append({
                        "repo": repo, "commit": commit, "parent": parent, "arm": arm,
                        "ok": False, "error": "query-failed", "duration_s": duration,
                    })
                    continue
                try:
                    document = json.loads(result.stdout)
                except json.JSONDecodeError:
                    rows.append({
                        "repo": repo, "commit": commit, "parent": parent, "arm": arm,
                        "ok": False, "error": "invalid-json", "duration_s": duration,
                    })
                    continue
                by_family = {item.get("family_id"): item for item in document.get("items", [])}
                for sample in samples:
                    finding = by_family.get(sample["family_id"])
                    rows.append({
                        "sid": sample["sid"],
                        "repo": repo,
                        "commit": commit,
                        "parent": parent,
                        "arm": arm,
                        "family_id": sample["family_id"],
                        "ok": True,
                        "matched": finding is not None,
                        "duration_s": duration,
                        "gate_fail_default": (
                            finding.get("gate", {}).get("fail_default") if finding else None
                        ),
                        "semantic_change": (
                            [site.get("semantic_change") for site in finding.get("changed", [])]
                            if finding else []
                        ),
                        # Additive replay evidence used by #850. The #849 summarizer ignores
                        # these fields, so its checked artifact remains reproducible.
                        "changed": finding.get("changed", []) if finding else [],
                        "not_updated": finding.get("not_updated", []) if finding else [],
                        "targets": finding.get("targets", []) if finding else [],
                    })
        finally:
            run(["git", "-C", str(source), "worktree", "remove", "--force", str(worktree)])
            run(["git", "-C", str(source), "worktree", "prune"])
    return rows


def cmd_replay(args):
    samples = jsonl(args.samples)
    grouped = defaultdict(list)
    by_query = defaultdict(list)
    for sample in samples:
        by_query[query_key(sample)].append(sample)
    for (repo, commit, parent, arm), rows in sorted(by_query.items()):
        grouped[repo].append((commit, parent, arm, rows))

    output = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = {
            executor.submit(
                replay_repo, repo, requests, args.repos_root, args.nose, args.timeout
            ): repo
            for repo, requests in grouped.items()
        }
        for future in concurrent.futures.as_completed(futures):
            repo = futures[future]
            rows = future.result()
            output.extend(rows)
            print(f"[{repo}] {len(rows)} labeled findings", flush=True)
    output.sort(key=lambda row: (row.get("sid", ""), row.get("repo", "")))
    with args.out.open("w") as stream:
        for row in output:
            stream.write(json.dumps(row, sort_keys=True) + "\n")


def witness_predicates(row):
    witnesses = [w for w in row.get("semantic_change", []) if isinstance(w, dict)]
    complete = [w for w in witnesses if w.get("status") == "complete"]
    mapped_delta_evidence = [
        w for w in witnesses
        if w.get("change_kind") not in {"no-semantic-delta", "mixed", "unknown"}
        and w.get("facets")
        and w.get("coverage", {}).get("mapped_shared_nodes", 0) > 0
    ]
    complete_mapped_delta = [w for w in mapped_delta_evidence if w.get("status") == "complete"]
    return {
        "mapped-semantic-delta-evidence": bool(mapped_delta_evidence),
        "mapped-sink-delta-evidence": any(w.get("sink_deltas") for w in mapped_delta_evidence),
        "no-semantic-delta-evidence": any(
            w.get("change_kind") == "no-semantic-delta" for w in witnesses
        ),
        "no-shared-semantic-node-evidence": any(
            "no-shared-semantic-node" in w.get("caveats", []) for w in witnesses
        ),
        "complete-mapped-semantic-delta": bool(complete_mapped_delta),
        "complete-mapped-sink-delta": any(w.get("sink_deltas") for w in complete_mapped_delta),
        "complete-no-semantic-delta": any(
            w.get("status") == "complete" and w.get("change_kind") == "no-semantic-delta"
            for w in witnesses
        ),
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


def nested_counts(rows, values):
    result = {}
    for value, verdicts in sorted(values.items()):
        result[value] = dict(sorted(verdicts.items()))
    return result


def cmd_summarize(args):
    samples = {row["sid"]: row for row in jsonl(args.samples)}
    verdicts = {row["sid"]: row for row in jsonl(args.verdicts)}
    replay = {row.get("sid"): row for row in jsonl(args.records) if row.get("sid")}
    joined = []
    for sid, sample in samples.items():
        row = replay.get(sid, {})
        joined.append({
            **row,
            "sid": sid,
            "verdict": verdicts[sid]["verdict"],
            "baseline_strict": current_v2_strict(sample),
            "predicates": witness_predicates(row),
        })

    statuses = defaultdict(Counter)
    change_kinds = defaultdict(Counter)
    facets = defaultdict(Counter)
    caveats = defaultdict(Counter)
    for row in joined:
        for witness in row.get("semantic_change", []):
            if not isinstance(witness, dict):
                continue
            verdict = row["verdict"]
            statuses[witness.get("status", "missing")][verdict] += 1
            change_kinds[witness.get("change_kind", "missing")][verdict] += 1
            for facet in witness.get("facets", []):
                facets[facet][verdict] += 1
            for caveat in witness.get("caveats", []):
                caveats[caveat][verdict] += 1

    strict = [row for row in joined if row["baseline_strict"]]
    matched = [row for row in joined if row.get("matched")]
    query_errors = [row for row in jsonl(args.records) if not row.get("ok")]
    binary_version = run([str(args.nose), "--version"]).stdout.strip()
    source_commit = run(["git", "rev-parse", "HEAD"], cwd=ROOT).stdout.strip()
    artifact = {
        "schema_version": 1,
        "issue": 849,
        "development_only": True,
        "blind_or_temporal_data_accessed": False,
        "inputs": {
            "samples": str(args.samples.relative_to(ROOT)),
            "samples_sha256": sha256(args.samples),
            "verdicts": str(args.verdicts.relative_to(ROOT)),
            "verdicts_sha256": sha256(args.verdicts),
            "labeled_findings": len(samples),
        },
        "implementation": {
            "source_commit": source_commit,
            "nose_binary": str(args.nose),
            "nose_binary_sha256": sha256(args.nose),
            "nose_version": binary_version,
        },
        "replay": {
            "matched_findings": len(matched),
            "unmatched_findings": len(joined) - len(matched),
            "query_error_rows": len(query_errors),
        },
        "baseline_v2_strict": {
            "findings": len(strict),
            "verdicts": dict(sorted(Counter(row["verdict"] for row in strict).items())),
        },
        "site_evidence_by_verdict": {
            "status": nested_counts(joined, statuses),
            "change_kind": nested_counts(joined, change_kinds),
            "facets": nested_counts(joined, facets),
            "caveats": nested_counts(joined, caveats),
        },
        "strict_no_propagation_needed": {
            "findings": sum(row["verdict"] == "no_propagation_needed" for row in strict),
            "predicate_hits": {
                name: sum(
                    row["verdict"] == "no_propagation_needed"
                    and row["predicates"].get(name, False)
                    for row in strict
                )
                for name in sorted(next(iter(strict))["predicates"] if strict else [])
            },
        },
        "simulations": [
            simulation("current-v2-strict", strict, lambda row: True),
            simulation(
                "evidence-slice-mapped-semantic-delta",
                strict,
                lambda row: row["predicates"]["mapped-semantic-delta-evidence"],
            ),
            simulation(
                "evidence-slice-mapped-sink-delta",
                strict,
                lambda row: row["predicates"]["mapped-sink-delta-evidence"],
            ),
            simulation(
                "require-complete-mapped-semantic-delta",
                strict,
                lambda row: row["predicates"]["complete-mapped-semantic-delta"],
            ),
            simulation(
                "require-complete-mapped-sink-delta",
                strict,
                lambda row: row["predicates"]["complete-mapped-sink-delta"],
            ),
            simulation(
                "demote-complete-no-semantic-delta",
                strict,
                lambda row: not row["predicates"]["complete-no-semantic-delta"],
            ),
            simulation(
                "demote-on-any-no-semantic-delta-evidence",
                strict,
                lambda row: not row["predicates"]["no-semantic-delta-evidence"],
            ),
            simulation(
                "demote-on-no-shared-semantic-node-evidence",
                strict,
                lambda row: not row["predicates"]["no-shared-semantic-node-evidence"],
            ),
        ],
        "interpretation": (
            "Development evidence only. These predicates do not change gate.fail_default and "
            "must not be tuned against or presented as a blind/default-on result."
        ),
    }
    args.out.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def cmd_selftest(_args):
    sample = {
        "scope": "prod", "fire_eligible": True,
        "changed": [{"touches_shared": True}],
    }
    assert current_v2_strict(sample)
    row = {
        "semantic_change": [{
            "status": "complete", "change_kind": "replacement", "facets": ["value"],
            "coverage": {"mapped_shared_nodes": 2}, "sink_deltas": [],
        }]
    }
    assert witness_predicates(row)["complete-mapped-semantic-delta"]
    assert witness_predicates(row)["mapped-semantic-delta-evidence"]
    assert not witness_predicates(row)["complete-mapped-sink-delta"]
    print("semantic_witness_eval selftest: ok")


def strip_semantic_change(document):
    document = json.loads(json.dumps(document))
    for finding in document.get("items", []):
        # #850 adds pair-local evidence after #849. Removing it here lets the same
        # official-release compatibility harness compare the pre-evidence contract.
        finding.pop("targets", None)
        for key in ("changed", "not_updated", "current_only"):
            for site in finding.get(key, []):
                site.pop("semantic_change", None)
    return document


def runtime_queries(samples):
    by_repo = {}
    for sample in sorted(samples, key=lambda row: (row["repo"], row["sid"])):
        if sample["arm"] == "default" and current_v2_strict(sample):
            by_repo.setdefault(sample["repo"], sample)
    return list(by_repo.values())


def runtime_repo(sample, repos_root, binaries, iterations, warmups, timeout):
    repo = sample["repo"]
    source = repos_root / repo
    rows = []
    legacy_equal = True
    with tempfile.TemporaryDirectory(prefix=f"nose-849-runtime-{repo}-") as tmp:
        worktree = Path(tmp) / "worktree"
        added = run([
            "git", "-C", str(source), "worktree", "add", "--detach", "--quiet",
            str(worktree), sample["commit"],
        ])
        if added.returncode != 0:
            raise SystemExit(f"runtime worktree add failed for {repo}: {added.stderr}")
        try:
            command_tail = [
                "query", ".", f"base={sample['parent']}", "top=0", "--format", "json"
            ]
            for _ in range(warmups):
                for binary in binaries.values():
                    result = run([str(binary), *command_tail], cwd=worktree, timeout=timeout)
                    if result.returncode != 0:
                        raise SystemExit(f"runtime warmup failed for {repo}: {result.stderr}")
            documents = {}
            for iteration in range(1, iterations + 1):
                order = ("baseline", "current") if iteration % 2 else ("current", "baseline")
                for label in order:
                    started = time.perf_counter()
                    result = run(
                        [str(binaries[label]), *command_tail],
                        cwd=worktree,
                        timeout=timeout,
                    )
                    elapsed_ms = (time.perf_counter() - started) * 1000
                    if result.returncode != 0:
                        raise SystemExit(
                            f"runtime query failed for {repo}/{label}: {result.stderr}"
                        )
                    document = json.loads(result.stdout)
                    documents[label] = document
                    rows.append({
                        "repo": repo,
                        "iteration": iteration,
                        "label": label,
                        "elapsed_ms": round(elapsed_ms, 6),
                        "output_bytes": len(result.stdout.encode()),
                    })
                legacy_equal &= documents["baseline"] == strip_semantic_change(
                    documents["current"]
                )
        finally:
            run(["git", "-C", str(source), "worktree", "remove", "--force", str(worktree)])
            run(["git", "-C", str(source), "worktree", "prune"])
    return rows, legacy_equal


def runtime_summary(rows, repos):
    by_repo = {}
    for repo in repos:
        labels = {}
        for label in ("baseline", "current"):
            elapsed = [
                row["elapsed_ms"] for row in rows
                if row["repo"] == repo and row["label"] == label
            ]
            labels[label] = round(statistics.median(elapsed), 6)
        baseline = labels["baseline"]
        current = labels["current"]
        by_repo[repo] = {
            **labels,
            "delta_ms": round(current - baseline, 6),
            "delta_pct": round((current - baseline) / baseline * 100, 6),
        }
    baseline = sum(row["baseline"] for row in by_repo.values())
    current = sum(row["current"] for row in by_repo.values())
    return {
        "aggregate": {
            "baseline_median_sum_ms": round(baseline, 6),
            "current_median_sum_ms": round(current, 6),
            "delta_ms": round(current - baseline, 6),
            "delta_pct": round((current - baseline) / baseline * 100, 6),
        },
        "by_repo": by_repo,
    }


def cmd_runtime(args):
    samples = jsonl(args.samples)
    queries = runtime_queries(samples)
    binaries = {"baseline": args.baseline.resolve(), "current": args.current.resolve()}
    rows = []
    compatibility = {}
    for sample in queries:
        repo_rows, legacy_equal = runtime_repo(
            sample, args.repos_root, binaries, args.iterations, args.warmups, args.timeout
        )
        rows.extend(repo_rows)
        compatibility[sample["repo"]] = legacy_equal
        print(f"[{sample['repo']}] runtime complete", flush=True)
    output = {
        "schema_version": 1,
        "issue": 849,
        "command": "nose query . base=<parent> top=0 --format json",
        "development_only": True,
        "inputs": {
            "samples": str(args.samples.relative_to(ROOT)),
            "samples_sha256": sha256(args.samples),
            "harness_sha256": sha256(Path(__file__)),
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
                "version": run([str(binary), "--version"]).stdout.strip(),
            }
            for label, binary in binaries.items()
        },
        "legacy_output_compatibility": {
            "all_equal_after_removing_semantic_change": all(compatibility.values()),
            "by_repo": compatibility,
        },
        "runs": rows,
        "summary": runtime_summary(rows, [sample["repo"] for sample in queries]),
    }
    args.out.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")


def parser():
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    replay = commands.add_parser("replay")
    replay.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    replay.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    replay.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    replay.add_argument("--jobs", type=int, default=4)
    replay.add_argument("--timeout", type=int, default=240)
    replay.add_argument("--out", type=Path, required=True)
    replay.set_defaults(func=cmd_replay)

    summarize = commands.add_parser("summarize")
    summarize.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    summarize.add_argument("--verdicts", type=Path, default=DEFAULT_VERDICTS)
    summarize.add_argument("--records", type=Path, required=True)
    summarize.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    summarize.add_argument("--out", type=Path, required=True)
    summarize.set_defaults(func=cmd_summarize)

    selftest = commands.add_parser("selftest")
    selftest.set_defaults(func=cmd_selftest)

    runtime = commands.add_parser("runtime")
    runtime.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    runtime.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    runtime.add_argument("--baseline", type=Path, required=True)
    runtime.add_argument("--current", type=Path, required=True)
    runtime.add_argument("--iterations", type=int, default=3)
    runtime.add_argument("--warmups", type=int, default=1)
    runtime.add_argument("--timeout", type=int, default=240)
    runtime.add_argument("--out", type=Path, required=True)
    runtime.set_defaults(func=cmd_runtime)
    return root


if __name__ == "__main__":
    arguments = parser().parse_args()
    arguments.func(arguments)
