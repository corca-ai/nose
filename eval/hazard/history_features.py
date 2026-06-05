#!/usr/bin/env python3
"""Cheap test of the git-history harm hypothesis on the existing gold.

For each gold candidate, blame the changed vs lagging member's *function* at the `from`
snapshot and measure whether they were last touched apart (different commit / time skew)
— a realized-divergence proxy. Then report harm-AUC of those history signals, to decide
whether a full git-history feature layer is worth building.

Usage: history_features.py candidates.jsonl gold-verdicts.jsonl /tmp/hazard-mine
"""
import json, os, re, subprocess, sys, statistics

SUBDIR = {"kafka": "clients", "pandas": "pandas", "grpc": "src", "terraform": "internal"}


def sh(args):
    return subprocess.run(args, capture_output=True, text=True, errors="replace")


def last_touch(repo, sha, file, name):
    """(commit_hash, committer_time) of the most recent change to function `name` at/before sha."""
    r = sh(["git", "-C", repo, "log", "-1", "--format=%H %ct", "-L", f":{re.escape(name)}:{file}", sha])
    line = next((l for l in r.stdout.splitlines() if l and not l.startswith("@@")), "")
    parts = line.split()
    if len(parts) >= 2 and parts[1].isdigit():
        return parts[0], int(parts[1])
    return None, None


def auc(pos, neg):
    if not pos or not neg:
        return float("nan")
    allv = sorted([(s, 0) for s in neg] + [(s, 1) for s in pos])
    r = [0.0] * len(allv); i = 0
    while i < len(allv):
        j = i
        while j + 1 < len(allv) and allv[j + 1][0] == allv[i][0]:
            j += 1
        for k in range(i, j + 1):
            r[k] = (i + j) / 2 + 1
        i = j + 1
    sp = sum(rr for rr, (_, l) in zip(r, allv) if l == 1)
    return (sp - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))


def main():
    cands = {c["cand_id"]: c for c in (json.loads(l) for l in open(sys.argv[1]))}
    verd = {v["cand_id"]: v for v in (json.loads(l) for l in open(sys.argv[2]))}
    work = sys.argv[3]

    rows = []  # (is_positive, label_positive, same_commit, skew_days)
    for cid, v in verd.items():
        c = cands.get(cid)
        if not c:
            continue
        repo = c["repo"]
        dir_ = f"{work}/{repo}"
        cm, lm, frm = c["changed_member"], c["lagging_member"], c["from"]
        ch, ct = last_touch(dir_, frm, cm["file"], cm["name"])
        lh, lt = last_touch(dir_, frm, lm["file"], lm["name"])
        if ct is None or lt is None:
            continue
        same = 1.0 if ch == lh else 0.0
        skew_days = abs(ct - lt) / 86400.0
        rows.append((bool(v.get("is_positive")), v["label"] in ("harm", "should_propagate"),
                     same, skew_days))
    n = len(rows)
    print(f"computed history features for {n} gold candidates")
    for posidx, posname in [(0, "STRICT harm"), (1, "LENIENT harm")]:
        pos = [r for r in rows if r[posidx]]
        neg = [r for r in rows if not r[posidx]]
        print(f"\n== {posname}: {len(pos)} pos, {len(neg)} neg ==")
        # same_commit: harmful divergences likely had members last touched TOGETHER
        # (anomalous fresh divergence) vs benign ones already apart -> test both directions
        a_skew = auc([r[3] for r in pos], [r[3] for r in neg])
        a_same = auc([r[2] for r in pos], [r[2] for r in neg])
        print(f"  AUC blame_skew_days (apart->harm): {a_skew:.3f}")
        print(f"  AUC same_commit    (together->harm): {a_same:.3f}")
        print(f"  median skew_days  pos={statistics.median([r[3] for r in pos]):.0f}  "
              f"neg={statistics.median([r[3] for r in neg]):.0f}")
        print(f"  same_commit rate  pos={statistics.mean([r[2] for r in pos]):.2f}  "
              f"neg={statistics.mean([r[2] for r in neg]):.2f}")


if __name__ == "__main__":
    main()
