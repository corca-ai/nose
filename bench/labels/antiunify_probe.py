#!/usr/bin/env python3
"""Experiment: does anti-unification structure predict refactoring-worthiness?

For each labeled family, take two representative members, get their normalized-IL
subtrees, and compute the anti-unifier (most-specific generalization) by a parallel
tree walk:
  - template      : positions where the two trees agree in kind (the shared skeleton)
  - struct_holes  : positions where kind/arity diverge (whole differing subtrees)
  - value_holes   : leaf positions, same kind, different payload (clean parameters:
                    a Var/Lit/Field whose VALUE differs — e.g. a locale's strings)

Hypotheses to test against `worthy`:
  abstractness = template / (template + struct_hole_nodes)   (high = clean skeleton)
  value_holes / template                                     (many = data variant,
                                                              not an extractable fn)

Usage: python3 bench/labels/antiunify_probe.py
"""
import json
import subprocess
import sys
from pathlib import Path

sys.setrecursionlimit(100000)
ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"
_cache = {}


def il(file):
    if file in _cache:
        return _cache[file]
    p = ROOT / file
    r = subprocess.run([str(NOSE), "il", str(p), "--normalized", "--format", "json"],
                       cwd=ROOT, capture_output=True, text=True, timeout=120)
    try:
        d = json.loads(r.stdout)
    except json.JSONDecodeError:
        d = None
    _cache[file] = d
    return d


def kids(d, i):
    n = d["nodes"][i]
    cs, cl = n["child_start"], n["child_len"]
    return d["edges"][cs:cs + cl]


def node_at(d, s, e):
    """Index of the node whose span best matches [s,e] (the unit root)."""
    best, bd = None, 1 << 30
    for i, n in enumerate(d["nodes"]):
        sp = n["span"]
        dd = abs(sp["start_line"] - s) + abs(sp["end_line"] - e)
        if dd < bd:
            best, bd = i, dd
    return best


def size(d, i):
    n = 1
    for c in kids(d, i):
        n += size(d, c)
    return n


def antiunify(d1, i1, d2, i2, acc):
    n1, n2 = d1["nodes"][i1], d2["nodes"][i2]
    if n1["kind"] != n2["kind"]:
        acc["struct_hole_nodes"] += size(d1, i1) + size(d2, i2)
        acc["struct_holes"] += 1
        return
    # same kind → part of the template
    acc["template"] += 1
    c1, c2 = kids(d1, i1), kids(d2, i2)
    if not c1 and not c2:  # leaf: a differing payload is a clean parameter (value hole)
        if n1["payload"] != n2["payload"]:
            acc["value_holes"] += 1
    for k in range(max(len(c1), len(c2))):
        if k < len(c1) and k < len(c2):
            antiunify(d1, c1[k], d2, c2[k], acc)
        elif k < len(c1):
            acc["struct_hole_nodes"] += size(d1, c1[k]); acc["struct_holes"] += 1
        else:
            acc["struct_hole_nodes"] += size(d2, c2[k]); acc["struct_holes"] += 1


def family_features(members):
    a, b = members[0], members[1]
    d1, d2 = il(a["file"]), il(b["file"])
    if not d1 or not d2:
        return None
    i1 = node_at(d1, a["start_line"], a["end_line"])
    i2 = node_at(d2, b["start_line"], b["end_line"])
    if i1 is None or i2 is None:
        return None
    acc = {"template": 0, "struct_holes": 0, "struct_hole_nodes": 0, "value_holes": 0}
    antiunify(d1, i1, d2, i2, acc)
    tmpl = max(acc["template"], 1)
    return {
        "abstractness": acc["template"] / (acc["template"] + acc["struct_hole_nodes"]),
        "value_hole_ratio": acc["value_holes"] / tmpl,
        "struct_hole_ratio": acc["struct_hole_nodes"] / (acc["template"] + acc["struct_hole_nodes"]),
        "value_holes": acc["value_holes"],
        "template": acc["template"],
    }


def main():
    fams = json.loads((ROOT / "bench/labels/refactoring_families.v5.json").read_text())["families"]
    corpus = {r["id"]: r for r in json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]}
    rows = []
    for f in fams:
        if corpus[f["repo"]]["split"] != "dev" or len(f["members"]) < 2:
            continue
        feat = family_features(f["members"])
        if feat:
            rows.append((f["worthy"], feat, f["reason"]))
    print(f"computed anti-unification for {len(rows)} families\n")

    def bucket(key, edges):
        print(f"{key}:")
        for lo, hi in edges:
            b = [w for w, ft, _ in rows if lo <= ft[key] < hi]
            if b:
                print(f"  [{lo:.2f},{hi:.2f}): worthy {sum(b):>3}/{len(b):<3} = {sum(b)/len(b):.0%}")
    bucket("abstractness", [(0, .5), (.5, .8), (.8, .95), (.95, 1.001)])
    bucket("value_hole_ratio", [(0, .02), (.02, .08), (.08, .20), (.20, 100)])
    bucket("struct_hole_ratio", [(0, .05), (.05, .20), (.20, .50), (.50, 1.001)])
    # combined heuristic: worthy ⇐ clean skeleton, few param holes
    print("\ncombined (abstractness>=0.8 AND value_hole_ratio<0.08 AND struct_hole_ratio<0.5):")
    for cond, label in [(True, "matches"), (False, "fails")]:
        b = [w for w, ft, _ in rows
             if (ft["abstractness"] >= .8 and ft["value_hole_ratio"] < .08 and ft["struct_hole_ratio"] < .5) == cond]
        if b:
            print(f"  {label:<8} worthy {sum(b):>3}/{len(b):<3} = {sum(b)/len(b):.0%}")


if __name__ == "__main__":
    main()
