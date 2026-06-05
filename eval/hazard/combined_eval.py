#!/usr/bin/env python3
"""Combined harm validation on the larger gated gold: static + git-history + a fitted combo.

Merges round-1 reused labels (in candidates2.jsonl `reused`) with round-2 fresh labels,
joins cached git-history features, and reports harm-AUC for each static baseline, the
git-history signal, and a leave-one-repo-out logistic combination.

Usage: combined_eval.py candidates2.jsonl round2-gold.jsonl candidates2-hist.jsonl
"""
import json, math, sys, statistics
from collections import defaultdict


def spread(f, mo, la):
    return 1.0 + 0.30 * max(min(f, 8) - 1, 0) + 0.50 * max(min(mo, 6) - 1, 0) + 0.50 * max(la - 1, 0)


def eff(m):
    c = max(m - 1, 0); return c if c <= 6 else 6 + math.sqrt(c - 6)


def tight(x):
    return min(max(x["shared_weight"] / max(x["mean_lines"], 1), 0.0), 1.0)


def auc(pos, neg):
    if not pos or not neg:
        return float("nan")
    allv = sorted([(s, 0) for s in neg] + [(s, 1) for s in pos]); r = [0.0] * len(allv); i = 0
    while i < len(allv):
        j = i
        while j + 1 < len(allv) and allv[j + 1][0] == allv[i][0]:
            j += 1
        for k in range(i, j + 1):
            r[k] = (i + j) / 2 + 1
        i = j + 1
    sp = sum(rr for rr, (_, l) in zip(r, allv) if l == 1)
    return (sp - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))


def fit_logistic(X, y, iters=600, lr=0.3, l2=2e-3):
    d = len(X[0]); w = [0.0] * d; b = 0.0; n = len(X)
    for _ in range(iters):
        gw = [0.0] * d; gb = 0.0
        for xi, yi in zip(X, y):
            z = b + sum(w[k] * xi[k] for k in range(d))
            p = 1 / (1 + math.exp(-max(min(z, 30), -30))); e = p - yi
            for k in range(d):
                gw[k] += e * xi[k]
            gb += e
        for k in range(d):
            w[k] -= lr * (gw[k] / n + l2 * w[k])
        b -= lr * gb / n
    return w, b


def main():
    cands = {c["cand_id"]: c for c in (json.loads(l) for l in open(sys.argv[1]))}
    r2 = {v["cand_id"]: v for v in (json.loads(l) for l in open(sys.argv[2]))}
    hist = {h["cand_id"]: h for h in (json.loads(l) for l in open(sys.argv[3]))}

    rows = []  # (repo, is_positive, feats, same_commit, skew_days)
    for cid, c in cands.items():
        if "reused" in c:
            isp = bool(c["reused"]["is_positive"])
        elif cid in r2:
            isp = bool(r2[cid].get("is_positive"))
        else:
            continue
        h = hist.get(cid, {})
        rows.append((c["repo"], isp, c["feats"], h.get("same_commit", 0.0), h.get("skew_days", 200.0)))
    npos = sum(r[1] for r in rows)
    print(f"gold: {len(rows)} labeled, {npos} positives ({npos/len(rows):.1%})")

    STATIC = {
        "hazard": lambda r: r[2]["mean_lines"] * spread(r[2]["files"], r[2]["modules"], r[2]["languages"])
        * (0.3 + 0.7 * (1 - tight(r[2]))) * {"prod": 1.0, "mixed": 0.5, "test": 0.25}.get(r[2]["scope"], 1.0),
        "mean_sem": lambda r: r[2]["mean_sem"],
        "extractability": lambda r: r[2]["shared_weight"] * eff(r[2]["members"])
        * spread(r[2]["files"], r[2]["modules"], r[2]["languages"]) * (1 / (1 + 0.5 * r[2]["params"])) * tight(r[2]),
        "value": lambda r: r[2]["value"],
        "same_commit (history)": lambda r: r[3],
        "-skew_days (history)": lambda r: -r[4],
    }
    pos = [r for r in rows if r[1]]; neg = [r for r in rows if not r[1]]
    print(f"\n### single-signal harm-AUC  (pos={len(pos)}, neg={len(neg)})")
    for name, fn in sorted(STATIC.items(), key=lambda kv: -auc([kv[1](r) for r in pos], [kv[1](r) for r in neg])):
        print(f"  {name:24s} {auc([fn(r) for r in pos], [fn(r) for r in neg]):.3f}")

    # leave-one-repo-out logistic combo: log(mean_sem), log(shared_weight), same_commit, log(mean_lines)
    def fv(r):
        f = r[2]
        return [math.log1p(f["mean_sem"]), math.log1p(f["shared_weight"]), r[3],
                math.log1p(f["mean_lines"]), math.log1p(f["modules"])]
    repos = sorted(set(r[0] for r in rows))
    test_pos, test_neg = [], []
    coefs = defaultdict(list)
    FN = ["log_sem", "log_sw", "same_commit", "log_lines", "log_modules"]
    for ho in repos:
        tr = [r for r in rows if r[0] != ho]; te = [r for r in rows if r[0] == ho]
        if not any(r[1] for r in te):
            continue
        Xtr = [fv(r) for r in tr]; ytr = [1.0 if r[1] else 0.0 for r in tr]
        cols = list(zip(*Xtr)); mu = [statistics.mean(c) for c in cols]; sd = [statistics.pstdev(c) or 1 for c in cols]
        Xn = [[(v - mu[k]) / sd[k] for k, v in enumerate(x)] for x in Xtr]
        w, b = fit_logistic(Xn, ytr)
        for k, nm in enumerate(FN):
            coefs[nm].append(w[k])
        for r in te:
            s = b + sum(w[k] * (fv(r)[k] - mu[k]) / sd[k] for k in range(len(FN)))
            (test_pos if r[1] else test_neg).append(s)
    print(f"\n### leave-one-repo-out logistic combo: harm-AUC {auc(test_pos, test_neg):.3f}")
    for nm in sorted(FN, key=lambda n: -abs(statistics.mean(coefs[n]))):
        print(f"  {nm:14s} {statistics.mean(coefs[nm]):+.3f}")


if __name__ == "__main__":
    main()
