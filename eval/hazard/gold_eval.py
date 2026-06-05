#!/usr/bin/env python3
"""Validate hazard() against the LLM-built gold label (Phase C).

Positives = G1 candidates the LLM judged a genuine should-propagate divergence (clone +
accidental divergence + the change applies to the sibling), surviving adversarial verify.
Two negative sets: (A) the *hard* negatives — G1 candidates judged benign (a divergence
happened but should not propagate); (B) easy negatives — G0 controls (no divergence).
Reports rank-AUC and precision@k of hazard vs baselines on each.

Usage: gold_eval.py candidates.jsonl gold-verdicts.jsonl all-events.jsonl
"""
import json, math, sys, random
random.seed(7)


def spread(f, mo, la):
    return (1.0 + 0.30 * max(min(f, 8) - 1, 0)
            + 0.50 * max(min(mo, 6) - 1, 0) + 0.50 * max(la - 1, 0))


def eff_copies(m):
    c = max(m - 1, 0)
    return c if c <= 6 else 6 + math.sqrt(c - 6)


def tight(x):
    return min(max(x["shared_weight"] / max(x["mean_lines"], 1), 0.0), 1.0)


SCORERS = {
    "hazard (shipped)": lambda x: x["mean_lines"] * spread(x["files"], x["modules"], x["languages"])
    * (0.3 + 0.7 * (1 - tight(x))) * {"prod": 1.0, "mixed": 0.5, "test": 0.25}.get(x["scope"], 1.0),
    "v1_size_led": lambda x: x["mean_sem"] * eff_copies(x["members"])
    * spread(x["files"], x["modules"], x["languages"]) * (0.3 + 0.7 * (1 - tight(x))),
    "extractability": lambda x: x["shared_weight"] * eff_copies(x["members"])
    * spread(x["files"], x["modules"], x["languages"]) * (1 / (1 + 0.5 * x["params"])) * tight(x),
    "value": lambda x: x["value"],
    "mean_sem only": lambda x: x["mean_sem"],
    "random": lambda x: (hash((x["mean_lines"], x["params"], x["shared_weight"])) & 0xffff) / 0xffff,
}


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


def patk(items, score, ispos, ks):
    ranked = sorted(items, key=score, reverse=True)
    npos = sum(1 for it in items if ispos(it))
    out = {k: (sum(ispos(it) for it in ranked[:k]) / min(k, len(ranked))) for k in ks}
    out["P@npos"] = sum(ispos(it) for it in ranked[:npos]) / npos if npos else float("nan")
    return out


def report(title, pos_items, neg_items):
    items = [("p", x) for x in pos_items] + [("n", x) for x in neg_items]
    print(f"\n### {title}  (pos={len(pos_items)}, neg={len(neg_items)})")
    print(f"  {'scorer':18s}  AUC    P@20   P@50   P@npos")
    rows = []
    for name, fn in SCORERS.items():
        rows.append((auc([fn(x) for x in pos_items], [fn(x) for x in neg_items]), name))
    for a, name in sorted(rows, reverse=True):
        pk = patk(items, lambda it: SCORERS[name](it[1]), lambda it: it[0] == "p", [20, 50])
        print(f"  {name:18s}  {a:.3f}  {pk[20]:.3f}  {pk[50]:.3f}  {pk['P@npos']:.3f}")


def main():
    cands = {c["cand_id"]: c for c in (json.loads(l) for l in open(sys.argv[1]))}
    verdicts = {v["cand_id"]: v for v in (json.loads(l) for l in open(sys.argv[2]))}

    # easy negatives: a sample of G0 (no divergence) families
    g0 = [json.loads(l)["feats"] for l in open(sys.argv[3]) if json.loads(l)["label"].startswith("G0")]
    random.shuffle(g0)
    g0 = g0[:2000]

    for field, name in [("is_positive", "STRICT (verified positives)"),
                        ("label_positive", "LENIENT (labeled positives, pre-verify)")]:
        pos, neg_g1 = [], []
        for cid, v in verdicts.items():
            c = cands.get(cid)
            if not c:
                continue
            (pos if v.get(field) else neg_g1).append(c["feats"])
        print(f"\n========== {name}: {len(pos)} positives, {len(neg_g1)} benign-G1 negatives ==========")
        report("A. within realized divergences (gold-positive vs benign divergence)", pos, neg_g1)
        report("B. vs G0 controls + benign-G1 (full ranking)", pos, neg_g1 + g0)


if __name__ == "__main__":
    main()
