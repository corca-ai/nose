#!/usr/bin/env python3
"""Replay divergent-edit queries over real merged changes — the consumer-2 fire benchmark (#243).

For each sampled first-parent commit C (parent P) of a pinned corpus repo, check C out
in a throwaway git worktree and run `nose query . base=P top=0` there. That is exactly the
PR-gate situation: the working tree holds the merged change, the base is what it merged
onto, and whatever the divergent-edit query flags is what a gate would have shown that
PR's author.

Two arms per change:
  default — conservative default channel mix (syntax,semantic)
  near    — --mode syntax,semantic,near (prices adding the fuzzy channel)

Subcommands:
  replay     run the replays, write raw per-(repo,commit,arm) records as JSONL
  summarize  fire-rate / findings-per-change tables from the raw JSONL
  sample     deterministic stratified finding sample, with embedded base-tree code and
             the change diff, so a judge can label findings without repo access
  redact-sample
             strip source excerpts and diffs from sampled findings for checked-in
             policy reproduction
  policy-eval
             recompute policy precision from a sampled-findings JSONL plus verdict JSONL
  selftest   run corpus-free checks for the harness helpers
  check-artifacts
             validate the checked 2026-06-11 summary/verdict/policy artifacts

The raw JSONL stays out of git (eval/hazard precedent); the checked-in artifacts are
the summary, verdicts, and policy evaluation. Results: docs/experiments.md.
"""

import argparse
from collections import Counter
import concurrent.futures
import hashlib
import json
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"

# 7 corpus languages x {dev, heldout}; chosen for active multi-contributor histories.
DEFAULT_REPOS = [
    "git", "redis",            # C
    "hugo", "minio",           # Go
    "netty", "rxjava",         # Java
    "scrapy", "sympy",         # Python
    "rubocop", "sidekiq",      # Ruby
    "clap", "tokio",           # Rust
    "jest", "rxjs",            # TypeScript
]

SUPPORTED_EXTS = {
    ".py", ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".go", ".rs",
    ".java", ".c", ".h", ".rb", ".vue", ".svelte", ".html",
}

MIN_CHANGED_SRC_LINES = 3
MAX_CHANGED_SRC_LINES = 600
QUERY_DEPTH = 800         # first-parent commits walked per repo
ELIGIBLE_POOL_CAP = 200   # eligible commits collected before even sampling

VERDICT_CLASSES = {
    "should_propagate",
    "intentional_divergence",
    "not_a_clone",
    "no_propagation_needed",
    "test_scaffolding",
    "unclear",
}

CHECKED_ARTIFACTS = {
    "summary": ROOT / "eval" / "divergence_fire" / "replay_summary_2026_06_11.json",
    "verdicts": ROOT / "eval" / "divergence_fire" / "verdicts_2026_06_11.jsonl",
    "policy": ROOT / "eval" / "divergence_fire" / "policy_eval_2026_06_11.json",
}

REFRESH_ARTIFACTS = {
    "summary": ROOT / "eval" / "divergence_fire" / "replay_summary_2026_07_06.json",
    "samples": ROOT / "eval" / "divergence_fire" / "sampled_findings_2026_07_06.jsonl",
    "verdicts": ROOT / "eval" / "divergence_fire" / "verdicts_2026_07_06.jsonl",
    "policy": ROOT / "eval" / "divergence_fire" / "policy_eval_2026_07_06.json",
}


def sh(args, cwd=None, timeout=None):
    return subprocess.run(
        args, cwd=cwd, capture_output=True, text=True, errors="replace", timeout=timeout
    )


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def git_output(args, cwd=ROOT):
    r = sh(["git", *args], cwd=cwd)
    return r.stdout.strip() if r.returncode == 0 else None


def command_line():
    return " ".join(shlex.quote(a) for a in sys.argv)


def nose_metadata():
    version = sh([str(NOSE), "--version"])
    return {
        "nose_binary": str(NOSE.relative_to(ROOT)),
        "nose_binary_sha256": sha256_file(NOSE) if NOSE.exists() else None,
        "nose_version": version.stdout.strip() if version.returncode == 0 else None,
        "source_commit": git_output(["rev-parse", "HEAD"]),
        "source_dirty": bool(git_output(["status", "--short"])),
    }


def protocol_metadata(repos, per_repo, timeout, jobs):
    return {
        "schema_version": 2,
        "arms": ARMS,
        "repos": list(repos),
        "per_repo": per_repo,
        "timeout_s": timeout,
        "jobs": jobs,
        "query_depth": QUERY_DEPTH,
        "eligible_pool_cap": ELIGIBLE_POOL_CAP,
        "min_changed_src_lines": MIN_CHANGED_SRC_LINES,
        "max_changed_src_lines": MAX_CHANGED_SRC_LINES,
        "supported_exts": sorted(SUPPORTED_EXTS),
        "query_template": "nose query . base=<parent> top=0 --format json [arm args]",
        "timing_fields": ["duration_s"],
        "selection": "even sample over newest eligible first-parent commits",
    }


def src_change(repo, parent, commit):
    """(supported-ext files touched, changed source lines) for parent->commit."""
    r = sh(["git", "-C", str(repo), "diff", "--numstat", parent, commit])
    files, lines = 0, 0
    for ln in r.stdout.splitlines():
        parts = ln.split("\t")
        if len(parts) != 3 or parts[0] == "-" or parts[1] == "-":
            continue
        if Path(parts[2]).suffix.lower() in SUPPORTED_EXTS:
            files += 1
            lines += int(parts[0]) + int(parts[1])
    return files, lines


def eligible_commits(repo):
    """Newest-first first-parent (sha, parent, subject) with a source diff in bounds."""
    r = sh(["git", "-C", str(repo), "log", "--first-parent",
            f"--max-count={QUERY_DEPTH}", "--pretty=%H|%P|%s"])
    out = []
    for ln in r.stdout.splitlines():
        sha, parents, subject = ln.split("|", 2)
        if not parents:
            continue
        parent = parents.split()[0]
        files, lines = src_change(repo, parent, sha)
        if files >= 1 and MIN_CHANGED_SRC_LINES <= lines <= MAX_CHANGED_SRC_LINES:
            out.append({"commit": sha, "parent": parent, "subject": subject[:120],
                        "src_files": files, "src_lines": lines})
            if len(out) >= ELIGIBLE_POOL_CAP:
                break
    return out


def even_sample(items, k):
    if len(items) <= k:
        return items
    step = len(items) / k
    return [items[int(i * step)] for i in range(k)]


ARMS = {"default": [], "near": ["--mode", "syntax,semantic,near"]}


def run_divergence_query(worktree, parent, arm, timeout):
    cmd = [str(NOSE), "query", ".", f"base={parent}", "top=0", "--format", "json"]
    cmd += ARMS[arm]
    t0 = time.monotonic()
    try:
        r = sh(cmd, cwd=worktree, timeout=timeout)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout", "duration_s": round(time.monotonic() - t0, 2)}
    dur = round(time.monotonic() - t0, 2)
    if r.returncode != 0:
        return {"ok": False, "error": r.stderr.strip()[-400:], "duration_s": dur}
    try:
        doc = json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "error": "bad json", "duration_s": dur}
    summary = doc.get("summary") or {}
    return {
        "ok": True,
        "duration_s": dur,
        "changed_files": summary.get("changed_files"),
        "divergences": summary.get("divergences"),
        "findings": doc.get("items", []),
    }


def replay_repo(repo_id, repos_root, per_repo, timeout):
    repo = repos_root / repo_id
    head = sh(["git", "-C", str(repo), "rev-parse", "HEAD"]).stdout.strip()
    picked = even_sample(eligible_commits(repo), per_repo)
    records = []
    with tempfile.TemporaryDirectory(prefix=f"nose-divergence-fire-{repo_id}-") as tmp:
        wt = Path(tmp) / "wt"
        add = sh(["git", "-C", str(repo), "worktree", "add", "--detach", str(wt), head])
        if add.returncode != 0:
            print(f"[{repo_id}] worktree add failed: {add.stderr.strip()}", file=sys.stderr)
            return records
        try:
            for c in picked:
                co = sh(["git", "-C", str(wt), "checkout", "-q", "--detach", c["commit"]])
                if co.returncode != 0:
                    continue
                for arm in ARMS:
                    res = run_divergence_query(wt, c["parent"], arm, timeout)
                    records.append({"repo": repo_id, **c, "arm": arm, **res})
        finally:
            sh(["git", "-C", str(repo), "worktree", "remove", "--force", str(wt)])
            sh(["git", "-C", str(repo), "worktree", "prune"])
    fired = sum(1 for r in records if r.get("findings"))
    print(f"[{repo_id}] {len(picked)} commits x {len(ARMS)} arms -> "
          f"{len(records)} runs, {fired} fired", file=sys.stderr)
    return records


def cmd_replay(args):
    if not NOSE.exists():
        sys.exit(f"missing release binary: {NOSE} (cargo build --release)")
    metadata = {**protocol_metadata(args.repos, args.per_repo, args.timeout, args.jobs),
                **nose_metadata()}
    metadata.setdefault("commands", {})["replay"] = command_line()
    all_records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as ex:
        futs = {ex.submit(replay_repo, rid, args.repos_root, args.per_repo, args.timeout): rid
                for rid in args.repos}
        for fut in concurrent.futures.as_completed(futs):
            all_records.extend(fut.result())
    all_records.sort(key=lambda r: (r["repo"], r["commit"], r["arm"]))
    with open(args.out, "w") as f:
        for r in all_records:
            f.write(json.dumps({"metadata": metadata, **r}) + "\n")
    print(f"wrote {len(all_records)} records -> {args.out}")


def load_records(path):
    return [json.loads(ln) for ln in open(path)]


def percentile(xs, p):
    return xs[min(len(xs) - 1, int(p * len(xs)))] if xs else 0


def summarize_records(records, records_path=None):
    arms = sorted({r["arm"] for r in records})
    repos = sorted({r["repo"] for r in records})
    metadata_rows = [r.get("metadata") for r in records if r.get("metadata")]
    metadata_keys = {json.dumps(m, sort_keys=True) for m in metadata_rows}
    if len(metadata_keys) > 1:
        raise ValueError("records contain mixed replay metadata; summarize one replay at a time")
    if metadata_rows and len(metadata_rows) != len(records):
        raise ValueError("records mix metadata-bearing and legacy rows")
    embedded_meta = metadata_rows[0] if metadata_rows else None
    summary = {
        "schema_version": 2,
        "metadata": embedded_meta or {
            "source": str(records_path) if records_path else None,
            "timing_fields": ["duration_s"],
            "query_template": "nose query . base=<parent> top=0 --format json [arm args]",
        },
        "per_arm": {},
        "per_repo": {},
    }
    summary["metadata"].setdefault("commands", {})["summarize"] = command_line()
    if records_path:
        summary["metadata"]["raw_records"] = {
            "path": str(records_path),
            "sha256": sha256_file(records_path) if Path(records_path).exists() else None,
        }
    summary["selected_replays"] = [
        {
            "repo": r["repo"],
            "arm": r["arm"],
            "commit": r.get("commit"),
            "parent": r.get("parent"),
            "subject": r.get("subject"),
            "ok": bool(r.get("ok")),
            "findings": len(r.get("findings") or []),
            "duration_s": r.get("duration_s"),
        }
        for r in sorted(records, key=lambda x: (x["repo"], x["commit"], x["arm"]))
    ]
    for arm in arms:
        rs = [r for r in records if r["arm"] == arm and r.get("ok")]
        errs = [r for r in records if r["arm"] == arm and not r.get("ok")]
        fired = [r for r in rs if r["findings"]]
        counts = sorted(len(r["findings"]) for r in fired)
        all_counts = sorted(len(r["findings"]) for r in rs)
        durs = sorted(r["duration_s"] for r in rs)
        findings = [f for r in rs for f in r["findings"]]
        lane_counts = Counter(f.get("lane", "base-divergence") for f in findings)
        tier_counts = Counter(f.get("tier", "legacy") for f in findings)

        summary["per_arm"][arm] = {
            "replays": len(rs), "errors": len(errs),
            "fired": len(fired),
            "fire_rate": round(len(fired) / len(rs), 4) if rs else 0,
            "strict_fired": sum(
                1 for r in rs if any(f.get("tier") == "strict" for f in r["findings"])),
            "new_copy_fired": sum(
                1 for r in rs if any(f.get("lane") == "new-copy" for f in r["findings"])),
            "findings_total": sum(counts),
            "lane_counts": dict(sorted(lane_counts.items())),
            "tier_counts": dict(sorted(tier_counts.items())),
            "findings_per_replay_p50": percentile(all_counts, 0.5),
            "findings_per_replay_p90": percentile(all_counts, 0.9),
            "findings_per_fire_p50": percentile(counts, 0.5),
            "findings_per_fire_p90": percentile(counts, 0.9),
            "findings_per_fire_max": counts[-1] if counts else 0,
            "divergence_s_p50": percentile(durs, 0.5),
            "divergence_s_p90": percentile(durs, 0.9),
            "divergence_s_max": durs[-1] if durs else 0,
        }
    for repo in repos:
        row = {}
        for arm in arms:
            rs = [r for r in records if r["repo"] == repo and r["arm"] == arm and r.get("ok")]
            fired = sum(1 for r in rs if r["findings"])
            durs = sorted(r["duration_s"] for r in rs)
            row[arm] = {"replays": len(rs), "fired": fired,
                        "findings": sum(len(r["findings"]) for r in rs),
                        "divergence_s_p50": percentile(durs, 0.5),
                        "divergence_s_p90": percentile(durs, 0.9)}
        summary["per_repo"][repo] = row
    return summary


def cmd_summarize(args):
    records = load_records(args.records)
    try:
        summary = summarize_records(records, args.records)
    except ValueError as exc:
        sys.exit(str(exc))
    out = json.dumps(summary, indent=2)
    if args.out:
        Path(args.out).write_text(out + "\n")
        print(f"wrote {args.out}")
    else:
        print(out)


def base_lines(repo, parent, file, start, end, pad=3, cap=80):
    return tree_lines(repo, parent, file, start, end, pad, cap)


def tree_lines(repo, rev, file, start, end, pad=3, cap=80):
    r = sh(["git", "-C", str(repo), "show", f"{rev}:{file}"])
    if r.returncode != 0:
        return None
    lines = r.stdout.splitlines()
    lo, hi = max(1, start - pad), min(len(lines), end + pad)
    body = lines[lo - 1:hi]
    if len(body) > cap:
        body = body[:cap] + ["... [truncated]"]
    return "\n".join(f"{n}: {t}" for n, t in zip(range(lo, lo + len(body)), body))


def file_diff(repo, parent, commit, file, cap=160):
    r = sh(["git", "-C", str(repo), "diff", parent, commit, "--", file])
    lines = r.stdout.splitlines()
    if len(lines) > cap:
        lines = lines[:cap] + ["... [truncated]"]
    return "\n".join(lines)


def sample_pool(records, findings_per_change):
    pool = []
    for r in records:
        for rank, f in enumerate(r["findings"]):
            if findings_per_change > 0 and rank >= findings_per_change:
                break
            pool.append((r, rank, f))
    return pool


def select_sample(pool, n):
    by_stratum = {}
    for item in pool:
        by_stratum.setdefault((item[0]["arm"], item[0]["repo"]), []).append(item)
    for items in by_stratum.values():
        items.sort(key=lambda it: (it[1], it[0]["commit"]))  # top-ranked first
    take, strata = [], sorted(by_stratum)
    while strata and (n == 0 or len(take) < n):
        for s in list(strata):
            items = by_stratum[s]
            if not items:
                strata.remove(s)
                continue
            take.append(items.pop(0))
            if n > 0 and len(take) >= n:
                break
    return take


def cmd_sample(args):
    records = [r for r in load_records(args.records) if r.get("ok") and r["findings"]]
    # The CLI default emits all findings so policy pricing can cover lower ranks.
    # Pass --findings-per-change 1 for the historical strict top-1 gate metric.
    pool = sample_pool(records, args.findings_per_change)
    take = select_sample(pool, args.n)
    out = []
    for i, (r, rank, f) in enumerate(take):
        repo = args.repos_root / r["repo"]
        sites = {}
        for role in ("changed", "not_updated"):
            sites[role] = []
            for s in f.get(role, [])[:3]:
                entry = {k: s.get(k) for k in
                         ("file", "name", "start_line", "end_line", "lang", "kind",
                          "is_fragment", "fragment_kind", "reason_code", "span_lines",
                          "span_tokens", "touches_shared", "enclosing_unit")}
                entry["base_code"] = base_lines(
                    repo, r["parent"], s["file"], s["start_line"], s["end_line"])
                if role == "changed":
                    entry["change_diff"] = file_diff(repo, r["parent"], r["commit"], s["file"])
                sites[role].append(entry)
        sites["current_only"] = []
        for s in f.get("current_only", [])[:3]:
            entry = {k: s.get(k) for k in
                     ("file", "name", "start_line", "end_line", "lang", "kind",
                      "is_fragment", "fragment_kind", "reason_code", "span_lines",
                      "span_tokens", "touches_shared", "enclosing_unit", "tree")}
            entry["current_code"] = tree_lines(
                repo, r["commit"], s["file"], s["start_line"], s["end_line"])
            entry["change_diff"] = file_diff(repo, r["parent"], r["commit"], s["file"])
            sites["current_only"].append(entry)
        out.append({
            "sid": f"{args.sid_prefix}-{i:03d}", "repo": r["repo"], "arm": r["arm"],
            "commit": r["commit"], "parent": r["parent"], "subject": r["subject"],
            "rank": rank, "findings_in_change": len(r["findings"]),
            "family_id": f["family_id"],
            "lane": f.get("lane"),
            "similarity": f.get("similarity"), "complexity": f.get("complexity"),
            "fire_eligible": f.get("fire_eligible"),
            "tier": f.get("tier"),
            "tier_reasons": f.get("tier_reasons"),
            "taxonomy_hint": f.get("taxonomy_hint"),
            "gate": f.get("gate"),
            "witness_kind": f.get("witness_kind"),
            "scope": f.get("scope"),
            "changed": sites["changed"], "not_updated": sites["not_updated"],
            "current_only": sites["current_only"],
        })
    with open(args.out, "w") as fh:
        for rec in out:
            fh.write(json.dumps(rec) + "\n")
    print(f"wrote {len(out)} sampled findings -> {args.out}")


def redact_site(site):
    return {k: site.get(k) for k in
            ("file", "name", "start_line", "end_line", "lang", "kind",
             "is_fragment", "fragment_kind", "reason_code", "span_lines",
             "span_tokens", "touches_shared", "enclosing_unit", "tree")}


def redact_sample_row(row):
    return {
        "sid": row["sid"],
        "repo": row["repo"],
        "arm": row["arm"],
        "commit": row["commit"],
        "parent": row.get("parent"),
        "subject": row.get("subject"),
        "rank": row.get("rank"),
        "findings_in_change": row.get("findings_in_change"),
        "family_id": row.get("family_id"),
        "lane": row.get("lane"),
        "similarity": row.get("similarity"),
        "complexity": row.get("complexity"),
        "fire_eligible": row.get("fire_eligible"),
        "tier": row.get("tier"),
        "tier_reasons": row.get("tier_reasons"),
        "taxonomy_hint": row.get("taxonomy_hint"),
        "gate": row.get("gate"),
        "witness_kind": row.get("witness_kind"),
        "scope": row.get("scope"),
        "changed": [redact_site(s) for s in row.get("changed", [])],
        "not_updated": [redact_site(s) for s in row.get("not_updated", [])],
        "current_only": [redact_site(s) for s in row.get("current_only", [])],
    }


def cmd_redact_sample(args):
    samples = load_jsonl(args.samples)
    with open(args.out, "w") as fh:
        for row in samples:
            fh.write(json.dumps(redact_sample_row(row)) + "\n")
    print(f"wrote {len(samples)} redacted sampled findings -> {args.out}")


def load_jsonl(path):
    return [json.loads(ln) for ln in Path(path).read_text().splitlines() if ln.strip()]


def changed_touches_shared(sample):
    return any(site.get("touches_shared") is True for site in sample.get("changed", []))


def identity_key(row):
    keys = ("repo", "commit", "arm", "rank", "family_id")
    if not all(k in row and row[k] is not None for k in keys):
        return None
    return tuple(row[k] for k in keys)


def policy_row(name, rows, predicate):
    fires = [r for r in rows if predicate(r)]
    tp = sum(1 for r in fires if r["pos"])
    fp = len(fires) - tp
    return {
        "policy": name,
        "fires": len(fires),
        "tp": tp,
        "fp": fp,
        "precision": round(tp / len(fires), 3) if fires else 0.0,
    }


def v2_strict(row):
    if row.get("tier") is not None:
        return row.get("tier") == "strict"
    return row.get("fire_eligible") is True and row.get("scope") == "prod"


def policy_rows_from_labeled_rows(rows, name_prefix=""):
    return [
        policy_row(f"{name_prefix}any sampled finding", rows, lambda _r: True),
        policy_row(f"{name_prefix}touches-shared (line)", rows, lambda r: r["line"]),
        policy_row(f"{name_prefix}exact-witness only", rows,
                   lambda r: r["witness"] == "exact-value-graph"),
        policy_row(f"{name_prefix}line OR exact-witness", rows,
                   lambda r: r["line"] or r["witness"] == "exact-value-graph"),
        policy_row(f"{name_prefix}V1 conservative: (line OR exact-witness) AND scope!=test", rows,
                   lambda r: (r["line"] or r["witness"] == "exact-value-graph")
                   and r["scope"] != "test"),
        policy_row(f"{name_prefix}serialized fire_eligible", rows,
                   lambda r: r.get("fire_eligible") is True),
        policy_row(f"{name_prefix}V2 strict: tier=strict", rows, v2_strict),
    ]


def compute_policy_eval(samples, verdicts):
    verdict_by_sid = {v["sid"]: v for v in verdicts}
    verdict_by_identity = {}
    for verdict in verdicts:
        key = identity_key(verdict)
        if key is not None:
            if key in verdict_by_identity:
                raise ValueError(f"duplicate verdict identity: {key}")
            verdict_by_identity[key] = verdict
    rows = []
    for sample in samples:
        key = identity_key(sample)
        verdict = verdict_by_identity.get(key) if key is not None else None
        if verdict is not None and verdict.get("sid") != sample.get("sid"):
            # The stable identity wins; sid is ordinal and may change between samples.
            pass
        elif verdict is None:
            verdict = verdict_by_sid.get(sample["sid"])
        if not verdict:
            continue
        witness = sample.get("witness_kind")
        line = changed_touches_shared(sample)
        scope = sample.get("scope")
        row = {
            "sid": sample["sid"],
            "pos": verdict["verdict"] == "should_propagate",
            "verdict": verdict["verdict"],
            "line": line,
            "witness": witness,
            "scope": scope,
            "rank": sample.get("rank"),
            "identity": list(key) if key is not None else None,
            "fire_eligible": sample.get("fire_eligible"),
        }
        if sample.get("tier") is not None:
            row["tier"] = sample.get("tier")
        rows.append(row)
    return {
        "schema_version": 2,
        "method": "policy simulation from sampled findings joined to verdict labels by stable finding identity, with sid fallback for legacy verdict drafts",
        "labeled": len(rows),
        "positives": sum(1 for r in rows if r["pos"]),
        "finding_level": policy_rows_from_labeled_rows(rows),
        "rows": rows,
    }


def cmd_policy_eval(args):
    samples = load_jsonl(args.samples)
    verdicts = load_jsonl(args.verdicts)
    verdict_sids = {v["sid"] for v in verdicts}
    verdict_identities = {identity_key(v) for v in verdicts if identity_key(v) is not None}
    missing = []
    for sample in samples:
        key = identity_key(sample)
        if key is not None and key in verdict_identities:
            continue
        if sample["sid"] not in verdict_sids:
            missing.append(sample["sid"])
    if missing and not args.allow_unlabeled:
        sys.exit(f"{len(missing)} sampled findings lack verdicts; pass --allow-unlabeled")
    try:
        report = compute_policy_eval(samples, verdicts)
    except ValueError as exc:
        sys.exit(str(exc))
    report["inputs"] = {
        "samples": args.samples,
        "verdicts": args.verdicts,
    }
    report["command"] = command_line()
    out = json.dumps(report, indent=2)
    if args.out:
        Path(args.out).write_text(out + "\n")
        print(f"wrote {args.out}")
    else:
        print(out)


def require(cond, msg):
    if not cond:
        raise AssertionError(msg)


def cmd_selftest(_args):
    records = [
        {
            "repo": "r1", "commit": "c1", "arm": "default", "ok": True,
            "duration_s": 1.2, "findings": [{"family_id": "a"}, {"family_id": "b"}],
        },
        {
            "repo": "r1", "commit": "c1", "arm": "near", "ok": True,
            "duration_s": 2.5, "findings": [],
        },
        {
            "repo": "r2", "commit": "c2", "arm": "default", "ok": False,
            "duration_s": 3.0, "error": "timeout",
        },
    ]
    summary = summarize_records(records)
    require(summary["schema_version"] == 2, "summary schema")
    require(summary["per_arm"]["default"]["replays"] == 1, "default replay count")
    require(summary["per_arm"]["default"]["errors"] == 1, "default error count")
    require(summary["per_arm"]["default"]["fired"] == 1, "default fired count")
    require(summary["per_arm"]["default"]["findings_total"] == 2, "finding total")
    require(len(sample_pool([records[0]], 1)) == 1, "top-only sample")
    require(len(sample_pool([records[0]], 0)) == 2, "all-findings sample")
    require(len(select_sample(sample_pool([records[0]], 0), 0)) == 2, "uncapped sample")
    require(even_sample(list(range(10)), 3) == [0, 3, 6], "even_sample")
    policy = compute_policy_eval(
        [{"sid": "s1", "witness_kind": "copy-paste-run", "scope": "prod",
          "repo": "r", "commit": "c", "arm": "default", "rank": 0, "family_id": "f",
          "fire_eligible": True,
          "changed": [{"touches_shared": True}]}],
        [{"sid": "renumbered", "repo": "r", "commit": "c", "arm": "default",
          "rank": 0, "family_id": "f", "verdict": "should_propagate"}],
    )
    require(policy["finding_level"][0]["tp"] == 1, "policy tp")
    require(policy["finding_level"][-1]["precision"] == 1.0, "fire_eligible policy")
    print("selftest OK")


def cmd_check_artifacts(_args):
    missing = [str(p) for p in CHECKED_ARTIFACTS.values() if not p.exists()]
    require(not missing, f"missing artifacts: {missing}")

    summary = json.loads(CHECKED_ARTIFACTS["summary"].read_text())
    require("per_arm" in summary and "per_repo" in summary, "summary shape")
    for arm in ("default", "near"):
        require(arm in summary["per_arm"], f"missing arm {arm}")
        row = summary["per_arm"][arm]
        for key in ("replays", "errors", "fired", "fire_rate", "findings_total",
                    "findings_per_fire_p50", "findings_per_fire_p90",
                    "divergence_s_p50", "divergence_s_p90"):
            require(key in row, f"summary {arm}.{key}")

    verdicts = [json.loads(ln) for ln in CHECKED_ARTIFACTS["verdicts"].read_text().splitlines()
                if ln.strip()]
    require(verdicts, "empty verdicts")
    seen = set()
    for row in verdicts:
        for key in ("sid", "verdict", "repo", "arm", "commit", "rank", "family_id"):
            require(key in row, f"verdict missing {key}")
        require(row["sid"] not in seen, f"duplicate sid {row['sid']}")
        seen.add(row["sid"])
        require(row["verdict"] in VERDICT_CLASSES,
                f"unknown verdict {row['sid']}: {row['verdict']}")

    policy = json.loads(CHECKED_ARTIFACTS["policy"].read_text())
    require(policy.get("schema_version") == 1, "policy schema")
    finding_level = policy.get("finding_level") or []
    require(finding_level, "policy finding_level")
    policy_rows = policy.get("rows") or []
    require({r["sid"] for r in policy_rows} == {v["sid"] for v in verdicts},
            "policy rows do not cover verdict sids")
    require(sum(1 for r in policy_rows if r.get("pos"))
            == sum(1 for v in verdicts if v["verdict"] == "should_propagate"),
            "policy positives do not match verdict positives")

    recomputed = {
        "any (pre-#245 --fail)": policy_row(
            "any (pre-#245 --fail)", policy_rows, lambda _r: True),
        "touches-shared (line)": policy_row(
            "touches-shared (line)", policy_rows, lambda r: r["line"]),
        "exact-witness only": policy_row(
            "exact-witness only", policy_rows,
            lambda r: r["witness"] == "exact-value-graph"),
        "line OR exact-witness": policy_row(
            "line OR exact-witness", policy_rows,
            lambda r: r["line"] or r["witness"] == "exact-value-graph"),
        "SHIPPED: (line OR exact-witness) AND scope!=test": policy_row(
            "SHIPPED: (line OR exact-witness) AND scope!=test", policy_rows,
            lambda r: (r["line"] or r["witness"] == "exact-value-graph")
            and r["scope"] != "test"),
    }
    for existing in finding_level:
        expected = recomputed.get(existing.get("policy"))
        if not expected:
            continue
        for key in ("fires", "tp", "fp", "precision"):
            require(existing.get(key) == expected[key],
                    f"policy {existing['policy']} stale field {key}")
    shipped = [r for r in finding_level if str(r.get("policy", "")).startswith("SHIPPED")]
    require(len(shipped) == 1, "missing shipped policy row")
    require(shipped[0].get("tp") == sum(1 for v in verdicts if v["verdict"] == "should_propagate"),
            "shipped tp does not match verdict positives")

    missing_refresh = [str(p) for p in REFRESH_ARTIFACTS.values() if not p.exists()]
    require(not missing_refresh, f"missing refresh artifacts: {missing_refresh}")
    refresh_summary = json.loads(REFRESH_ARTIFACTS["summary"].read_text())
    require(refresh_summary.get("schema_version") == 2, "refresh summary schema")
    require(refresh_summary.get("metadata", {}).get("per_repo") == 10,
            "refresh summary per_repo")
    refresh_verdicts = [
        json.loads(ln)
        for ln in REFRESH_ARTIFACTS["verdicts"].read_text().splitlines()
        if ln.strip()
    ]
    refresh_samples = [
        json.loads(ln)
        for ln in REFRESH_ARTIFACTS["samples"].read_text().splitlines()
        if ln.strip()
    ]
    refresh_policy = json.loads(REFRESH_ARTIFACTS["policy"].read_text())
    require(refresh_policy.get("schema_version") == 2, "refresh policy schema")
    require(refresh_policy.get("labeled") == len(refresh_verdicts),
            "refresh policy labeled count")
    require(refresh_policy.get("positives")
            == sum(1 for v in refresh_verdicts if v["verdict"] == "should_propagate"),
            "refresh policy positives")
    require(len(refresh_samples) == len(refresh_verdicts),
            "refresh sample/verdict count mismatch")
    for sample in refresh_samples:
        require("base_code" not in json.dumps(sample), "refresh sample leaks base_code")
        require("change_diff" not in json.dumps(sample), "refresh sample leaks change_diff")
    expected_policy = compute_policy_eval(refresh_samples, refresh_verdicts)
    require(refresh_policy.get("finding_level") == expected_policy["finding_level"],
            "refresh policy finding_level is stale")
    require(refresh_policy.get("rows") == expected_policy["rows"],
            "refresh policy rows are stale")
    require({tuple(r.get("identity") or []) for r in refresh_policy.get("rows", [])}
            == {identity_key(v) for v in refresh_verdicts},
            "refresh policy rows do not cover verdict identities")
    print("artifact check OK")


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--repos-root", type=Path, default=ROOT / "bench" / "repos")
    sub = p.add_subparsers(dest="cmd", required=True)

    pr = sub.add_parser("replay")
    pr.add_argument("--repos", nargs="+", default=DEFAULT_REPOS)
    pr.add_argument("--per-repo", type=int, default=25)
    pr.add_argument("--timeout", type=int, default=240)
    pr.add_argument("--jobs", type=int, default=4)
    pr.add_argument("--out", required=True)
    pr.set_defaults(fn=cmd_replay)

    ps = sub.add_parser("summarize")
    ps.add_argument("--records", required=True)
    ps.add_argument("--out")
    ps.set_defaults(fn=cmd_summarize)

    pm = sub.add_parser("sample")
    pm.add_argument("--records", required=True)
    pm.add_argument("--n", type=int, default=120,
                    help="sample size; 0 means emit the full selected pool")
    pm.add_argument("--findings-per-change", type=int, default=0,
                    help="findings sampled per fired change; 0 means all findings")
    pm.add_argument("--sid-prefix", default="rf")
    pm.add_argument("--out", required=True)
    pm.set_defaults(fn=cmd_sample)

    prd = sub.add_parser("redact-sample")
    prd.add_argument("--samples", required=True)
    prd.add_argument("--out", required=True)
    prd.set_defaults(fn=cmd_redact_sample)

    pp = sub.add_parser("policy-eval")
    pp.add_argument("--samples", required=True)
    pp.add_argument("--verdicts", required=True)
    pp.add_argument("--out")
    pp.add_argument("--allow-unlabeled", action="store_true")
    pp.set_defaults(fn=cmd_policy_eval)

    pst = sub.add_parser("selftest")
    pst.set_defaults(fn=cmd_selftest)

    pc = sub.add_parser("check-artifacts")
    pc.set_defaults(fn=cmd_check_artifacts)

    args = p.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
