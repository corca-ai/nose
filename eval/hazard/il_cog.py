#!/usr/bin/env python3
"""Empirical test: is cognitive complexity better from nose's IL or from source text?

For each gold candidate's changed member, compute cognitive complexity two ways —
(a) IL-based: run `nose il --normalized --format json` on the member code and count
If/Loop (1 + nesting) + And/Or BinOps over the IL tree; (b) source-text proxy (the
SonarSource-ish heuristic already used). Compare harm-AUC. Tests whether nose's
normalization obscures the cog signal.

Usage: il_cog.py candidates2.jsonl round2-gold.jsonl /path/to/nose
"""
import json, re, subprocess, sys, tempfile, os, statistics

EXT = {"python": ".py", "javascript": ".js", "typescript": ".ts", "go": ".go",
       "rust": ".rs", "c": ".c", "cpp": ".cc", "java": ".java"}
FLOW = re.compile(r"\b(if|elif|else\s+if|for|while|case|catch|except|switch|when)\b|&&|\|\||\?(?!\.)|\b(and|or)\b")


def src_cog(code):
    s = 0
    for line in code.splitlines():
        t = line.strip()
        if not t or t.startswith(("//", "#", "*", "/*")):
            continue
        nest = (len(line) - len(line.lstrip())) // 2
        s += len(FLOW.findall(t)) * (1 + max(nest, 0))
    return s


def il_cog(nodes, root):
    """SonarSource-ish cog over the IL tree: If/Loop cost 1+nesting; And/Or cost 1."""
    score = [0]
    def walk(i, depth):
        n = nodes[i]; k = n["kind"]; inc = 0
        if k in ("If", "Loop", "Match", "Switch"):
            score[0] += 1 + depth; inc = 1
        elif k == "BinOp" and isinstance(n.get("payload"), dict) and n["payload"].get("Op") in ("And", "Or"):
            score[0] += 1
        for c in range(n["child_start"], n["child_start"] + n["child_len"]):
            walk(c, depth + inc)
    walk(root, 0)
    return score[0]


def ext_of(path):
    e = os.path.splitext(path)[1].lower()
    return e if e in EXT.values() else ".py"


def main():
    cands = {c["cand_id"]: c for c in (json.loads(l) for l in open(sys.argv[1]))}
    r2 = {v["cand_id"]: v for v in (json.loads(l) for l in open(sys.argv[2]))}
    nose = sys.argv[3]

    rows = []  # (is_positive, src, il_or_None)
    n_ok = 0
    for cid, c in cands.items():
        if "reused" in c:
            isp = bool(c["reused"]["is_positive"])
        elif cid in r2:
            isp = bool(r2[cid].get("is_positive"))
        else:
            continue
        cm = c["changed_member"]
        code = cm["code"]
        ext = ext_of(cm["file"])
        with tempfile.NamedTemporaryFile("w", suffix=ext, delete=False) as tf:
            tf.write(code); path = tf.name
        r = subprocess.run([nose, "il", path, "--normalized", "--format", "json"],
                           capture_output=True, text=True, errors="replace")
        os.unlink(path)
        ilc = None
        try:
            d = json.loads(r.stdout)
            if any(n["kind"] in ("If", "Loop", "Func") for n in d["nodes"]):
                ilc = il_cog(d["nodes"], d["root"]); n_ok += 1
        except Exception:
            pass
        rows.append((isp, src_cog(code), ilc))
        if len(rows) % 400 == 0:
            print(f"[il_cog] {len(rows)} ({n_ok} parsed)", file=sys.stderr)

    print(f"\nlabeled {len(rows)}, IL parsed for {n_ok} ({n_ok/len(rows):.0%})")

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

    # compare on the SUBSET where IL parsed (fair comparison)
    sub = [r for r in rows if r[2] is not None]
    pos = [r for r in sub if r[0]]; neg = [r for r in sub if not r[0]]
    print(f"on IL-parsed subset ({len(pos)} pos, {len(neg)} neg):")
    print(f"  source-cog harm-AUC : {auc([r[1] for r in pos], [r[1] for r in neg]):.3f}")
    print(f"  IL-cog     harm-AUC : {auc([r[2] for r in pos], [r[2] for r in neg]):.3f}")
    print(f"  median cog  pos: src={statistics.median([r[1] for r in pos]):.0f} il={statistics.median([r[2] for r in pos]):.0f}  "
          f"neg: src={statistics.median([r[1] for r in neg]):.0f} il={statistics.median([r[2] for r in neg]):.0f}")


if __name__ == "__main__":
    main()
