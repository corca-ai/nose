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
    mapped_delta = [
        w for w in complete
        if w.get("change_kind") not in {"no-semantic-delta", "mixed", "unknown"}
        and w.get("facets")
        and w.get("coverage", {}).get("mapped_shared_nodes", 0) > 0
    ]
    return {
        "complete-mapped-semantic-delta": bool(mapped_delta),
        "complete-mapped-sink-delta": any(w.get("sink_deltas") for w in mapped_delta),
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
    assert not witness_predicates(row)["complete-mapped-sink-delta"]
    print("semantic_witness_eval selftest: ok")


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
    return root


if __name__ == "__main__":
    arguments = parser().parse_args()
    arguments.func(arguments)
