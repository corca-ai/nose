#!/usr/bin/env python3
"""Per-language refactoring-ranking eval with bootstrap CIs and a dev/heldout split.

Uses refactoring_families.v4.json (dev + heldout). Reports, per language and split:
precision@10 (baseline value-rank) and (anti-unification re-rank), each with a 95%
bootstrap CI so a difference like "Rust 47% vs 38%" can be judged significant or
noise; worthy-recall; and mean Raw-node ratio (the lowering confound). The CI is the
infrastructure that tells us whether a per-language number is trustworthy at the
current label count.

Usage: python3 bench/labels/eval_by_language.py
"""
import importlib.util
import json
import random
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.setrecursionlimit(100000)
ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"
spec = importlib.util.spec_from_file_location("au", ROOT / "bench/labels/antiunify_probe.py")
au = importlib.util.module_from_spec(spec)
spec.loader.exec_module(au)
RNG = random.Random(1)


def rel(p):
    p = p.replace(str(ROOT) + "/", "")
    i = p.find("bench/repos/")
    return p[i:] if i >= 0 else p


def ov(a, b):
    return a["file"] == b["file"] and not (a["end_line"] < b["start_line"] or b["end_line"] < a["start_line"])


def mlocs(f):
    return [{"file": rel(l["file"]), "start_line": l["start_line"], "end_line": l["end_line"]}
            for l in f["locations"]]


def refactorability(f):
    m = mlocs(f)
    if len(m) < 2:
        return 1.0
    ft = au.family_features(m[:2])
    if not ft:
        return 1.0
    r = ft["abstractness"]
    if ft["value_hole_ratio"] >= 0.15:
        r *= 0.4
    if ft["struct_hole_ratio"] >= 0.30:
        r *= 0.5
    return r


def ci(flags, b=2000):
    """95% bootstrap CI for the mean of a 0/1 list; returns (lo, hi) in %."""
    if not flags:
        return (0.0, 0.0)
    n = len(flags)
    means = []
    for _ in range(b):
        s = sum(flags[RNG.randrange(n)] for _ in range(n))
        means.append(s / n)
    means.sort()
    return (means[int(0.025 * b)] * 100, means[int(0.975 * b)] * 100)


def main():
    labels = json.loads((ROOT / "bench/labels/refactoring_families.v5.json").read_text())["families"]
    corpus = {r["id"]: r for r in json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]}
    by_repo = defaultdict(list)
    for f in labels:
        by_repo[f["repo"]].append(f)

    # (lang, split) -> {base:[flags], rr:[flags], rec:[hit/total]}
    acc = defaultdict(lambda: {"base": [], "rr": [], "rec": [0, 0], "n": 0, "worthy": 0})

    for rid, labs in by_repo.items():
        repo = ROOT / "bench" / "repos" / rid
        if not repo.is_dir():
            continue
        lang, split = corpus[rid]["primary_language"], corpus[rid]["split"]
        a = acc[(lang, split)]
        a["n"] += len(labs)
        a["worthy"] += sum(x["worthy"] for x in labs)
        r = subprocess.run([str(NOSE), "refactor", str(repo), "--format", "json", "--top", "1000000"],
                           cwd=ROOT, capture_output=True, text=True, timeout=300)
        fams = sorted(json.loads(r.stdout or "[]"), key=lambda f: -f["value"])
        top = fams[:40]
        for f in top:
            f["rv"] = f["value"] * refactorability(f)
        rr = sorted(top, key=lambda f: -f["rv"]) + fams[40:]

        def matched_flags(order):
            out = []
            for f in order[:10]:
                best, bo = None, 0
                for lab in labs:
                    o = sum(1 for s in mlocs(f) for m in lab["members"] if ov(s, m))
                    if o > bo:
                        best, bo = lab, o
                if best:
                    out.append(1 if best["worthy"] else 0)
            return out
        a["base"] += matched_flags(fams)
        a["rr"] += matched_flags(rr)
        for lab in labs:
            if lab["worthy"]:
                a["rec"][1] += 1
                a["rec"][0] += 1 if any(
                    sum(1 for s in mlocs(f) for m in lab["members"] if ov(s, m)) > 0 for f in fams) else 0

    def fmt(flags):
        if not flags:
            return "    -        "
        m = sum(flags) / len(flags) * 100
        lo, hi = ci(flags)
        return f"{m:>3.0f}% [{lo:>3.0f}-{hi:>3.0f}] n={len(flags)}"

    for split in ("dev", "heldout"):
        print(f"\n=== {split} ===")
        print(f"{'lang':<11}{'worthy':>7}  {'P@10 base':<18}{'P@10 re-rank':<18}{'recall':>8}")
        for lang in sorted({l for (l, s) in acc if s == split}):
            a = acc[(lang, split)]
            rec = f"{a['rec'][0]}/{a['rec'][1]}" if a["rec"][1] else "-"
            print(f"{lang:<11}{a['worthy']}/{a['n']:<5} {fmt(a['base']):<18}{fmt(a['rr']):<18}{rec:>8}")
        # overall this split
        ab = [x for (l, s), v in acc.items() if s == split for x in v["base"]]
        ar = [x for (l, s), v in acc.items() if s == split for x in v["rr"]]
        print(f"{'OVERALL':<11}{'':>7} {fmt(ab):<18}{fmt(ar):<18}")


if __name__ == "__main__":
    main()
