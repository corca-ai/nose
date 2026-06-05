#!/usr/bin/env python3
"""Precompute git-history features for every candidate (cached for combined eval).

For each candidate, blame the changed vs lagging member's function at `from` and record
whether they were last touched together (same_commit) and the time skew (days). Output:
{cand_id, same_commit, skew_days} JSONL.

Usage: history_cache.py candidates2.jsonl /tmp/hazard-mine out-hist.jsonl
"""
import json, re, subprocess, sys


def sh(args):
    return subprocess.run(args, capture_output=True, text=True, errors="replace")


def last_touch(repo, sha, file, name):
    r = sh(["git", "-C", repo, "log", "-1", "--format=%H %ct", "-L", f":{re.escape(name)}:{file}", sha])
    line = next((l for l in r.stdout.splitlines() if l and not l.startswith("@@")), "")
    p = line.split()
    return (p[0], int(p[1])) if len(p) >= 2 and p[1].isdigit() else (None, None)


def main():
    cands = [json.loads(l) for l in open(sys.argv[1])]
    work, out = sys.argv[2], sys.argv[3]
    n_ok = 0
    with open(out, "w") as f:
        for i, c in enumerate(cands):
            d = f"{work}/{c['repo']}"
            ch, ct = last_touch(d, c["from"], c["changed_member"]["file"], c["changed_member"]["name"])
            lh, lt = last_touch(d, c["from"], c["lagging_member"]["file"], c["lagging_member"]["name"])
            rec = {"cand_id": c["cand_id"]}
            if ct is not None and lt is not None:
                rec["same_commit"] = 1.0 if ch == lh else 0.0
                rec["skew_days"] = abs(ct - lt) / 86400.0
                n_ok += 1
            f.write(json.dumps(rec) + "\n")
            if (i + 1) % 300 == 0:
                print(f"[hist] {i+1}/{len(cands)} ({n_ok} with features)", file=sys.stderr)
    print(f"[hist] DONE: {n_ok}/{len(cands)} -> {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
