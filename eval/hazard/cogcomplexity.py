#!/usr/bin/env python3
"""Test cognitive-complexity / edit-surface signals for harm (issue #23 direction).

Uses the member code + diff already captured in the gold candidates (no re-mining). A
text-based, language-agnostic cognitive-complexity proxy (SonarSource-style: each
control-flow break costs 1 + its nesting depth) plus diff-localization. Tests harm-AUC
of per-member edit-surface, the *asymmetry* between the two members (the #23 axis-B
hypothesis: similar surface = accidental divergence = harm), and change locality.

Usage: cogcomplexity.py candidates2.jsonl round2-gold.jsonl
"""
import json, re, sys, statistics

# flow-break tokens (cross-language). Each occurrence on a line is a break; weight by nesting.
FLOW = re.compile(r"\b(if|elif|else\s+if|for|while|case|catch|except|switch|when)\b|&&|\|\||\?(?!\.)|\b(and|or)\b")


def cog(code):
    """SonarSource-ish cognitive complexity from text: sum over flow breaks of (1 + nesting)."""
    score = 0
    breaks = 0
    maxnest = 0
    for line in code.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(("//", "#", "*", "/*")):
            continue
        indent = len(line) - len(line.lstrip())
        nest = indent // 2  # rough: 2-space step (tabs count as 1 char -> shallow, ok as proxy)
        hits = len(FLOW.findall(stripped))
        if hits:
            score += hits * (1 + max(nest, 0))
            breaks += hits
            maxnest = max(maxnest, nest)
    return {"cog": score, "breaks": breaks, "maxnest": maxnest, "lines": len(code.splitlines())}


def diff_lines(diff):
    n = 0
    for l in diff.splitlines():
        if (l.startswith("+") or l.startswith("-")) and not l.startswith(("+++", "---")):
            n += 1
    return n


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


def main():
    cands = {c["cand_id"]: c for c in (json.loads(l) for l in open(sys.argv[1]))}
    r2 = {v["cand_id"]: v for v in (json.loads(l) for l in open(sys.argv[2]))}
    rows = []
    for cid, c in cands.items():
        if "reused" in c:
            isp = bool(c["reused"]["is_positive"])
        elif cid in r2:
            isp = bool(r2[cid].get("is_positive"))
        else:
            continue
        cc = cog(c["changed_member"]["code"]); lc = cog(c["lagging_member"]["code"])
        dl = diff_lines(c["changed_member"].get("diff", ""))
        feats = {
            "cog_changed": cc["cog"], "cog_lagging": lc["cog"],
            "cog_mean": (cc["cog"] + lc["cog"]) / 2,
            "cog_min": min(cc["cog"], lc["cog"]),
            "cog_absdiff": abs(cc["cog"] - lc["cog"]),
            "cog_reldiff": abs(cc["cog"] - lc["cog"]) / (max(cc["cog"], lc["cog"]) + 1),
            "maxnest_mean": (cc["maxnest"] + lc["maxnest"]) / 2,
            "breaks_mean": (cc["breaks"] + lc["breaks"]) / 2,
            "diff_lines": dl,
            "diff_locality": dl / (cc["lines"] + 1),  # small change in a big member -> subtle
            "diff_per_cog": dl / (cc["cog"] + 1),
        }
        rows.append((isp, feats))
    pos = [r[1] for r in rows if r[0]]; neg = [r[1] for r in rows if not r[0]]
    print(f"gold: {len(rows)} labeled, {len(pos)} positives")
    print(f"\n### cognitive-complexity / edit-surface harm-AUC (#23)")
    keys = list(rows[0][1].keys())
    res = []
    for k in keys:
        a = auc([f[k] for f in pos], [f[k] for f in neg])
        res.append((a, k))
    for a, k in sorted(res, reverse=True):
        # report both directions (a and 1-a) so anti-signals are visible
        print(f"  {k:16s} AUC {a:.3f}  (inverted {1-a:.3f})")
    print(f"\n  medians (pos vs neg):")
    for k in ["cog_mean", "cog_absdiff", "cog_reldiff", "diff_lines", "diff_locality", "maxnest_mean"]:
        print(f"    {k:16s} pos={statistics.median([f[k] for f in pos]):.2f}  "
              f"neg={statistics.median([f[k] for f in neg]):.2f}")


if __name__ == "__main__":
    main()
