#!/usr/bin/env python3
"""Evaluate hazard signals against mined G1/G0 events (docs/hazard-benchmark.md, Tier 1).

Positives = G1 (divergent edit). Negatives = G0c (consistent change) + G0s (stable).
Reports, per scoring function and per raw signal: rank-AUC (Mann-Whitney) of ranking
G1 above controls, and precision@k — both per-interval and per-family — overall and by
stratum. This is the evidence used to tune hazard()'s shape/weights before implementing.

Usage: analyze.py events.jsonl
"""
import json, sys, math, statistics
from collections import defaultdict


# --- scoring functions mirroring the shipped Rust hazard()/extractability() shape ---
def eff_copies(m):
    c = max(m - 1, 0)
    return c if c <= 6 else 6 + math.sqrt(c - 6)


def spread(files, modules, languages):
    return (1.0 + 0.30 * max(min(files, 8) - 1, 0)
            + 0.50 * max(min(modules, 6) - 1, 0)
            + 0.50 * max(languages - 1, 0))


def tightness(f):
    return min(max(f["shared_weight"] / max(f["mean_lines"], 1), 0.0), 1.0)


def scope_w(s):
    return {"prod": 1.0, "mixed": 0.5, "test": 0.25}.get(s, 1.0)


def hazard(f):
    inv = 0.3 + 0.7 * (1.0 - tightness(f))
    return (f["mean_lines"]
            * spread(f["files"], f["modules"], f["languages"])
            * inv * scope_w(f["scope"]))


def extractability(f):
    return (f["shared_weight"] * eff_copies(f["members"])
            * spread(f["files"], f["modules"], f["languages"])
            * (1.0 / (1.0 + 0.5 * f["params"])) * tightness(f))


def rnd(f):  # deterministic pseudo-random floor, varies by feature bytes
    h = hash((f["mean_sem"], f["members"], f["shared_weight"], f["params"])) & 0xFFFFFFFF
    return h / 0xFFFFFFFF


SCORERS = {
    "hazard": hazard,
    "size(mean_sem)": lambda f: f["mean_sem"],
    "extractability": extractability,
    "value": lambda f: f["value"],
    "sites(members)": lambda f: f["members"],
    "invisibility": lambda f: 0.3 + 0.7 * (1 - tightness(f)),
    "modules": lambda f: f["modules"],
    "languages": lambda f: f["languages"],
    "mean_lines": lambda f: f["mean_lines"],
    "params": lambda f: f["params"],
    "random": rnd,
}


def auc(scores_pos, scores_neg):
    """Rank-based AUC = P(score(pos) > score(neg)). 0.5 = no signal."""
    if not scores_pos or not scores_neg:
        return float("nan")
    allv = sorted([(s, 0) for s in scores_neg] + [(s, 1) for s in scores_pos])
    # average ranks for ties
    ranks = [0.0] * len(allv)
    i = 0
    while i < len(allv):
        j = i
        while j + 1 < len(allv) and allv[j + 1][0] == allv[i][0]:
            j += 1
        r = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[k] = r
        i = j + 1
    sum_pos = sum(rk for rk, (_, lab) in zip(ranks, allv) if lab == 1)
    n_pos, n_neg = len(scores_pos), len(scores_neg)
    return (sum_pos - n_pos * (n_pos + 1) / 2.0) / (n_pos * n_neg)


def precision_at_k(items, score, is_pos, ks):
    ranked = sorted(items, key=score, reverse=True)
    out = {}
    npos = sum(1 for it in items if is_pos(it))
    for k in ks + [("P", npos)] if False else ks:
        pass
    for k in ks:
        kk = min(k, len(ranked))
        out[k] = sum(1 for it in ranked[:kk] if is_pos(it)) / kk if kk else float("nan")
    out["P@npos"] = (sum(1 for it in ranked[:npos] if is_pos(it)) / npos) if npos else float("nan")
    return out, npos


def report(title, items, get_feats, is_pos):
    pos = [it for it in items if is_pos(it)]
    neg = [it for it in items if not is_pos(it)]
    print(f"\n### {title}  (N={len(items)}, pos={len(pos)}, neg={len(neg)}, "
          f"prevalence={len(pos)/max(len(items),1):.1%})")
    if not pos or not neg:
        print("  (insufficient positives/negatives)")
        return
    rows = []
    for name, fn in SCORERS.items():
        a = auc([fn(get_feats(it)) for it in pos], [fn(get_feats(it)) for it in neg])
        rows.append((a, name))
    rows.sort(reverse=True)
    print(f"  {'scorer':18s}  AUC    P@10   P@20   P@50   P@npos")
    for a, name in rows:
        fn = SCORERS[name]
        pk, npos = precision_at_k(items, lambda it, fn=fn: fn(get_feats(it)), is_pos, [10, 20, 50])
        print(f"  {name:18s}  {a:.3f}  {pk[10]:.3f}  {pk[20]:.3f}  {pk[50]:.3f}  {pk['P@npos']:.3f}")


def main():
    events = [json.loads(l) for l in open(sys.argv[1])]
    print(f"loaded {len(events)} events from {sys.argv[1]}")
    from collections import Counter
    print("labels:", dict(Counter(e["label"] for e in events)))
    print("repos:", dict(Counter(e["repo"] for e in events)))

    gf = lambda e: e["feats"]
    isg1 = lambda e: e["label"] == "G1"

    # --- per-interval ---
    report("PER-INTERVAL (all)", events, gf, isg1)
    for strat in ("S", "X"):
        sub = [e for e in events if e.get("stratum") == strat]
        if sub:
            report(f"PER-INTERVAL stratum {strat}", sub, gf, isg1)

    # --- per-family (requires fam_key) ---
    if all("fam_key" in e for e in events) and events:
        fams = defaultdict(list)
        for e in events:
            fams[(e["repo"], e["fam_key"])].append(e)
        fam_items = []
        for key, evs in fams.items():
            ever_g1 = any(e["label"] == "G1" for e in evs)
            # median feature across the family's life (forward-stable propensity view)
            med = {}
            for k in evs[0]["feats"]:
                vals = [e["feats"][k] for e in evs]
                if isinstance(vals[0], (int, float)):
                    med[k] = statistics.median(vals)
                else:
                    med[k] = Counter(vals).most_common(1)[0][0]
            fam_items.append({"feats": med, "ever_g1": ever_g1,
                              "stratum": evs[0].get("stratum")})
        report("PER-FAMILY (ever diverged?)", fam_items, lambda it: it["feats"],
               lambda it: it["ever_g1"])
        for strat in ("S", "X"):
            sub = [it for it in fam_items if it["stratum"] == strat]
            if sub:
                report(f"PER-FAMILY stratum {strat}", sub, lambda it: it["feats"],
                       lambda it: it["ever_g1"])

    # --- raw signal direction (G1 vs control means) ---
    print("\n### signal means: G1 vs control")
    g1 = [e["feats"] for e in events if e["label"] == "G1"]
    g0 = [e["feats"] for e in events if e["label"] != "G1"]
    if g1 and g0:
        print(f"  {'signal':16s}  {'G1 mean':>10s}  {'ctrl mean':>10s}  ratio")
        for k in ("mean_sem", "members", "modules", "mean_lines", "shared_weight", "params", "languages"):
            a = statistics.mean(x[k] for x in g1)
            b = statistics.mean(x[k] for x in g0)
            print(f"  {k:16s}  {a:10.2f}  {b:10.2f}  {a/b if b else float('nan'):.2f}")


if __name__ == "__main__":
    main()
