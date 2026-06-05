#!/usr/bin/env python3
"""Combine the #23 cognitive-complexity/edit-surface signals with git-history + static,
and measure leave-one-repo-out harm-AUC — does it break the ~0.60 structural ceiling?

Usage: harm_model.py candidates2.jsonl round2-gold.jsonl candidates2-hist.jsonl
"""
import json, math, re, sys, statistics
from collections import defaultdict

FLOW = re.compile(r"\b(if|elif|else\s+if|for|while|case|catch|except|switch|when)\b|&&|\|\||\?(?!\.)|\b(and|or)\b")


def cog(code):
    score = breaks = maxnest = 0
    for line in code.splitlines():
        s = line.strip()
        if not s or s.startswith(("//", "#", "*", "/*")):
            continue
        nest = (len(line) - len(line.lstrip())) // 2
        h = len(FLOW.findall(s))
        if h:
            score += h * (1 + max(nest, 0)); breaks += h; maxnest = max(maxnest, nest)
    return score, breaks, maxnest, len(code.splitlines())


def difflines(d):
    return sum(1 for l in d.splitlines() if (l.startswith(("+", "-")) and not l.startswith(("+++", "---"))))


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


def fit(X, y, iters=700, lr=0.3, l2=3e-3):
    d = len(X[0]); w = [0.0] * d; b = 0.0; n = len(X)
    for _ in range(iters):
        gw = [0.0] * d; gb = 0.0
        for xi, yi in zip(X, y):
            p = 1 / (1 + math.exp(-max(min(b + sum(w[k] * xi[k] for k in range(d)), 30), -30))); e = p - yi
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
    rows = []
    for cid, c in cands.items():
        if "reused" in c:
            isp = bool(c["reused"]["is_positive"])
        elif cid in r2:
            isp = bool(r2[cid].get("is_positive"))
        else:
            continue
        cc, _, mn, _ = cog(c["changed_member"]["code"])
        dl = difflines(c["changed_member"].get("diff", ""))
        h = hist.get(cid, {})
        f = c["feats"]
        fv = {
            "cog": cc, "maxnest": mn, "diff_lines": dl,
            "diff_per_cog": dl / (cc + 1),
            "same_commit": h.get("same_commit", 0.0),
            "log_sem": math.log1p(f["mean_sem"]),
            "log_sw": math.log1p(f["shared_weight"]),
        }
        rows.append((c["repo"], isp, fv))
    pos = [r[2] for r in rows if r[1]]; neg = [r[2] for r in rows if not r[1]]
    print(f"gold: {len(rows)} labeled, {len(pos)} positives")

    FEATS = ["cog", "diff_per_cog", "same_commit", "log_sem", "log_sw", "maxnest", "diff_lines"]
    print("\n### single-signal harm-AUC")
    for k in sorted(FEATS, key=lambda k: -max(auc([f[k] for f in pos], [f[k] for f in neg]),
                                              1 - auc([f[k] for f in pos], [f[k] for f in neg]))):
        a = auc([f[k] for f in pos], [f[k] for f in neg])
        print(f"  {k:14s} {max(a,1-a):.3f}  ({'+' if a>0.5 else '-'})")

    # leave-one-repo-out logistic over the directed feature set
    use = ["cog", "diff_per_cog", "same_commit", "log_sem", "log_sw"]
    repos = sorted(set(r[0] for r in rows))
    tp, tn = [], []; coefs = defaultdict(list)
    for ho in repos:
        tr = [r for r in rows if r[0] != ho]; te = [r for r in rows if r[0] == ho]
        if not any(r[1] for r in te):
            continue
        Xtr = [[r[2][k] for k in use] for r in tr]; ytr = [1.0 if r[1] else 0.0 for r in tr]
        cols = list(zip(*Xtr)); mu = [statistics.mean(c) for c in cols]; sd = [statistics.pstdev(c) or 1 for c in cols]
        Xn = [[(v - mu[k]) / sd[k] for k, v in enumerate(x)] for x in Xtr]
        w, b = fit(Xn, ytr)
        for k, nm in enumerate(use):
            coefs[nm].append(w[k])
        for r in te:
            x = [r[2][k] for k in use]
            s = b + sum(w[k] * (x[k] - mu[k]) / sd[k] for k in range(len(use)))
            (tp if r[1] else tn).append(s)
    print(f"\n### leave-one-repo-out logistic combo (#23 + history + static): harm-AUC {auc(tp, tn):.3f}")
    for nm in sorted(use, key=lambda n: -abs(statistics.mean(coefs[n]))):
        print(f"  {nm:14s} {statistics.mean(coefs[nm]):+.3f}")


if __name__ == "__main__":
    main()
