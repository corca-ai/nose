#!/usr/bin/env python3
"""Replay `nose review` over real merged changes — the consumer-2 fire benchmark (#243).

For each sampled first-parent commit C (parent P) of a pinned corpus repo, check C out
in a throwaway git worktree and run `nose review --base P` there. That is exactly the
PR-gate situation: the working tree holds the merged change, the base is what it merged
onto, and whatever review flags is what a gate would have shown that PR's author.

Two arms per change:
  default — review's conservative default channel mix (syntax,semantic)
  near    — --mode syntax,semantic,near (prices adding the fuzzy channel to review)

Subcommands:
  replay     run the replays, write raw per-(repo,commit,arm) records as JSONL
  summarize  fire-rate / findings-per-change tables from the raw JSONL
  sample     deterministic stratified finding sample, with embedded base-tree code and
             the change diff, so a judge can label findings without repo access

The raw JSONL stays out of git (eval/hazard precedent); the checked-in artifacts are
the summary, the labeled sample, and the verdicts. Results: docs/experiments.md.
"""

import argparse
import concurrent.futures
import json
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
SCAN_DEPTH = 800          # first-parent commits walked per repo
ELIGIBLE_POOL_CAP = 200   # eligible commits collected before even sampling


def sh(args, cwd=None, timeout=None):
    return subprocess.run(
        args, cwd=cwd, capture_output=True, text=True, errors="replace", timeout=timeout
    )


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
            f"--max-count={SCAN_DEPTH}", "--pretty=%H|%P|%s"])
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


def run_review(worktree, parent, arm, timeout):
    cmd = [str(NOSE), "review", "--base", parent, "--format", "json", "--top", "0"]
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
    return {
        "ok": True,
        "duration_s": dur,
        "changed_files": doc.get("changed_files"),
        "inconsistent_families": doc.get("inconsistent_families"),
        "findings": doc.get("findings", []),
    }


def replay_repo(repo_id, repos_root, per_repo, timeout):
    repo = repos_root / repo_id
    head = sh(["git", "-C", str(repo), "rev-parse", "HEAD"]).stdout.strip()
    picked = even_sample(eligible_commits(repo), per_repo)
    records = []
    with tempfile.TemporaryDirectory(prefix=f"nose-review-fire-{repo_id}-") as tmp:
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
                    res = run_review(wt, c["parent"], arm, timeout)
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
    all_records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as ex:
        futs = {ex.submit(replay_repo, rid, args.repos_root, args.per_repo, args.timeout): rid
                for rid in args.repos}
        for fut in concurrent.futures.as_completed(futs):
            all_records.extend(fut.result())
    all_records.sort(key=lambda r: (r["repo"], r["commit"], r["arm"]))
    with open(args.out, "w") as f:
        for r in all_records:
            f.write(json.dumps(r) + "\n")
    print(f"wrote {len(all_records)} records -> {args.out}")


def load_records(path):
    return [json.loads(ln) for ln in open(path)]


def cmd_summarize(args):
    records = load_records(args.records)
    arms = sorted({r["arm"] for r in records})
    repos = sorted({r["repo"] for r in records})
    summary = {"per_arm": {}, "per_repo": {}}
    for arm in arms:
        rs = [r for r in records if r["arm"] == arm and r.get("ok")]
        errs = [r for r in records if r["arm"] == arm and not r.get("ok")]
        fired = [r for r in rs if r["findings"]]
        counts = sorted(len(r["findings"]) for r in fired)
        durs = sorted(r["duration_s"] for r in rs)

        def pct(xs, p):
            return xs[min(len(xs) - 1, int(p * len(xs)))] if xs else 0

        summary["per_arm"][arm] = {
            "replays": len(rs), "errors": len(errs),
            "fired": len(fired),
            "fire_rate": round(len(fired) / len(rs), 4) if rs else 0,
            "findings_total": sum(counts),
            "findings_per_fire_p50": pct(counts, 0.5),
            "findings_per_fire_p90": pct(counts, 0.9),
            "findings_per_fire_max": counts[-1] if counts else 0,
            "review_s_p50": pct(durs, 0.5),
            "review_s_p90": pct(durs, 0.9),
        }
    for repo in repos:
        row = {}
        for arm in arms:
            rs = [r for r in records if r["repo"] == repo and r["arm"] == arm and r.get("ok")]
            fired = sum(1 for r in rs if r["findings"])
            row[arm] = {"replays": len(rs), "fired": fired,
                        "findings": sum(len(r["findings"]) for r in rs)}
        summary["per_repo"][repo] = row
    out = json.dumps(summary, indent=2)
    if args.out:
        Path(args.out).write_text(out + "\n")
        print(f"wrote {args.out}")
    else:
        print(out)


def base_lines(repo, parent, file, start, end, pad=3, cap=80):
    r = sh(["git", "-C", str(repo), "show", f"{parent}:{file}"])
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


def cmd_sample(args):
    records = [r for r in load_records(args.records) if r.get("ok") and r["findings"]]
    # The labeling unit is a fired change's TOP-RANKED finding: `--fail` is a
    # per-change decision and review orders most-likely-unpropagated first, so
    # top-1 precision is the gate metric. Stratified round-robin by (arm, repo);
    # lower-ranked findings stay unlabeled (a stated protocol limit).
    pool = []
    for r in records:
        for rank, f in enumerate(r["findings"]):
            pool.append((r, rank, f))
    by_stratum = {}
    for item in pool:
        by_stratum.setdefault((item[0]["arm"], item[0]["repo"]), []).append(item)
    for items in by_stratum.values():
        items.sort(key=lambda it: (it[1], it[0]["commit"]))  # top-ranked first
    take, strata = [], sorted(by_stratum)
    while len(take) < args.n and strata:
        for s in list(strata):
            items = by_stratum[s]
            if not items:
                strata.remove(s)
                continue
            take.append(items.pop(0))
            if len(take) >= args.n:
                break
    out = []
    for i, (r, rank, f) in enumerate(take):
        repo = args.repos_root / r["repo"]
        sites = {}
        for role in ("changed", "not_updated"):
            sites[role] = []
            for s in f[role][:3]:
                entry = {k: s.get(k) for k in
                         ("file", "name", "start_line", "end_line", "lang", "kind",
                          "is_fragment", "reason_code", "enclosing_unit")}
                entry["base_code"] = base_lines(
                    repo, r["parent"], s["file"], s["start_line"], s["end_line"])
                if role == "changed":
                    entry["change_diff"] = file_diff(repo, r["parent"], r["commit"], s["file"])
                sites[role].append(entry)
        out.append({
            "sid": f"rf-{i:03d}", "repo": r["repo"], "arm": r["arm"],
            "commit": r["commit"], "parent": r["parent"], "subject": r["subject"],
            "rank": rank, "family_id": f["family_id"],
            "similarity": f.get("similarity"), "complexity": f.get("complexity"),
            "changed": sites["changed"], "not_updated": sites["not_updated"],
        })
    with open(args.out, "w") as fh:
        for rec in out:
            fh.write(json.dumps(rec) + "\n")
    print(f"wrote {len(out)} sampled findings -> {args.out}")


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
    pm.add_argument("--n", type=int, default=120)
    pm.add_argument("--out", required=True)
    pm.set_defaults(fn=cmd_sample)

    args = p.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
