#!/usr/bin/env python3
"""Evidence-based tuning of hazard() from mined per-family data (docs/hazard-benchmark.md).

Builds a per-family table (label = ever had a G1 divergent edit), then:
  1. Fits a logistic regression (pure-python, standardized features) with
     leave-one-repo-out cross-validation -> data-driven WEIGHTS + DIRECTIONS.
  2. Scores parameter-free candidate hazard formulas under the same cross-repo split.
Cross-repo (not random) splits give the external validity the benchmark requires.

Usage: tune.py events.jsonl
"""
import json, sys, math, statistics
from collections import defaultdict


def auc(pos, neg):
    if not pos or not neg:
        return float("nan")
    allv = sorted([(s, 0) for s in neg] + [(s, 1) for s in pos])
    ranks = [0.0] * len(allv); i = 0
    while i < len(allv):
        j = i
        while j + 1 < len(allv) and allv[j + 1][0] == allv[i][0]:
            j += 1
        for k in range(i, j + 1):
            ranks[k] = (i + j) / 2.0 + 1.0
        i = j + 1
    sp = sum(r for r, (_, l) in zip(ranks, allv) if l == 1)
    return (sp - len(pos) * (len(pos) + 1) / 2.0) / (len(pos) * len(neg))


# ---- parameter-free candidate formulas (evidence-based variants) ----
def eff_copies(m): c = max(m - 1, 0); return c if c <= 6 else 6 + math.sqrt(c - 6)
def spread(f, mo, la): return 1.0 + 0.30 * max(min(f, 8) - 1, 0) + 0.50 * max(min(mo, 6) - 1, 0) + 0.50 * max(la - 1, 0)
def tightness(x): return min(max(x["shared_weight"] / max(x["mean_lines"], 1), 0.0), 1.0)
def invis(x): return 0.3 + 0.7 * (1.0 - tightness(x))
def scope_w(x): return {"prod": 1.0, "mixed": 0.5, "test": 0.25}.get(x["scope"], 1.0)

def pdamp(x): return 1.0 / (1.0 + 0.5 * x["params"])  # params is anti-predictive -> dampen

CANDIDATES = {
    "v1_size_led (current)": lambda x: x["mean_sem"] * eff_copies(x["members"]) * spread(x["files"], x["modules"], x["languages"]) * invis(x) * scope_w(x),
    "v2_no_size":            lambda x: eff_copies(x["members"]) * spread(x["files"], x["modules"], x["languages"]) * invis(x) * scope_w(x),
    "v3_dispersion_copies":  lambda x: spread(x["files"], x["modules"], x["languages"]) * eff_copies(x["members"]),
    "v4_modules_x_members":  lambda x: (1 + x["modules"]) * (1 + x["members"]),
    "v5_lines_led":          lambda x: x["mean_lines"] * spread(x["files"], x["modules"], x["languages"]) * invis(x) * scope_w(x),
    "v6_lines_mod_invis":    lambda x: x["mean_lines"] * (1 + x["modules"]) * invis(x),
    "v7_v5_param_damp":      lambda x: x["mean_lines"] * spread(x["files"], x["modules"], x["languages"]) * invis(x) * scope_w(x) * pdamp(x),
    "modules_only":          lambda x: x["modules"],
    "value_baseline":        lambda x: x["value"],
}

# ---- logistic-regression feature builder ----
FEATS = ["l_modules", "l_members", "invis", "l_mean_lines", "l_mean_sem", "params", "l_languages"]
def featvec(x):
    return [math.log1p(x["modules"]), math.log1p(x["members"]), invis(x),
            math.log1p(x["mean_lines"]), math.log1p(x["mean_sem"]), x["params"],
            math.log1p(x["languages"])]


def fit_logistic(X, y, iters=800, lr=0.3, l2=1e-3):
    n, d = len(X), len(X[0])
    w = [0.0] * d; b = 0.0
    for _ in range(iters):
        gw = [0.0] * d; gb = 0.0
        for xi, yi in zip(X, y):
            z = b + sum(w[k] * xi[k] for k in range(d))
            p = 1.0 / (1.0 + math.exp(-max(min(z, 30), -30)))
            e = p - yi
            for k in range(d):
                gw[k] += e * xi[k]
            gb += e
        for k in range(d):
            w[k] -= lr * (gw[k] / n + l2 * w[k])
        b -= lr * gb / n
    return w, b


def standardize(rows):
    cols = list(zip(*rows))
    mu = [statistics.mean(c) for c in cols]
    sd = [statistics.pstdev(c) or 1.0 for c in cols]
    return [[(v - mu[k]) / sd[k] for k, v in enumerate(r)] for r in rows], mu, sd


def main():
    events = [json.loads(l) for l in open(sys.argv[1])]
    fams = defaultdict(list)
    for e in events:
        fams[(e["repo"], e["fam_key"])].append(e)
    table = []  # (repo, label, feats-median-dict)
    for (repo, _k), evs in fams.items():
        med = {}
        for key in evs[0]["feats"]:
            vals = [e["feats"][key] for e in evs]
            med[key] = statistics.median(vals) if isinstance(vals[0], (int, float)) else \
                statistics.mode([e["feats"][key] for e in evs])
        table.append((repo, 1 if any(e["label"] == "G1" for e in evs) else 0, med))

    repos = sorted(set(r for r, _, _ in table))
    print(f"per-family rows: {len(table)} | positives: {sum(l for _, l, _ in table)} "
          f"({sum(l for _,l,_ in table)/len(table):.1%}) | repos: {len(repos)}")

    # ---- candidate formulas, leave-one-repo-out ----
    print("\n### candidate formulas — leave-one-repo-out test AUC")
    print(f"  {'formula':24s} {'mean':>6s}  per-repo")
    for name, fn in CANDIDATES.items():
        aucs = []
        for ho in repos:
            te = [(fn(m), l) for r, l, m in table if r == ho]
            pos = [s for s, l in te if l == 1]; neg = [s for s, l in te if l == 0]
            a = auc(pos, neg)
            if not math.isnan(a): aucs.append(a)
        m = statistics.mean(aucs) if aucs else float("nan")
        print(f"  {name:24s} {m:6.3f}")

    # ---- logistic regression, leave-one-repo-out ----
    print("\n### logistic regression — leave-one-repo-out test AUC + learned weights")
    coefs = defaultdict(list); test_aucs = []
    for ho in repos:
        tr = [(m, l) for r, l, m in table if r != ho]
        te = [(m, l) for r, l, m in table if r == ho]
        Xtr_raw = [featvec(m) for m, _ in tr]; ytr = [l for _, l in tr]
        Xtr, mu, sd = standardize(Xtr_raw)
        w, b = fit_logistic(Xtr, ytr)
        for k, name in enumerate(FEATS): coefs[name].append(w[k])
        score = lambda m: b + sum(w[k] * (featvec(m)[k] - mu[k]) / sd[k] for k in range(len(FEATS)))
        pos = [score(m) for m, l in te if l == 1]; neg = [score(m) for m, l in te if l == 0]
        a = auc(pos, neg)
        if not math.isnan(a): test_aucs.append(a)
    print(f"  mean test AUC: {statistics.mean(test_aucs):.3f}")
    print(f"  {'feature':14s} {'weight (mean±sd)':>22s}   direction")
    order = sorted(FEATS, key=lambda f: -abs(statistics.mean(coefs[f])))
    for f in order:
        mu_w = statistics.mean(coefs[f]); sd_w = statistics.pstdev(coefs[f])
        d = "↑ hazard" if mu_w > 0 else "↓ hazard"
        print(f"  {f:14s} {mu_w:>10.3f} ± {sd_w:5.3f}      {d}")


if __name__ == "__main__":
    main()
