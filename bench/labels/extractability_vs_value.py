#!/usr/bin/env python3
"""One-off: does the shipped `extractability` default beat `value` on worthy-P@10?
Reuses eval_by_language's matching helpers; ranks by nose's NATIVE output order
(= extractability, the default) vs value order."""
import importlib.util, json, subprocess
from collections import defaultdict

spec = importlib.util.spec_from_file_location("ev", "bench/labels/eval_by_language.py")
ev = importlib.util.module_from_spec(spec); spec.loader.exec_module(ev)
ROOT, NOSE = ev.ROOT, ev.NOSE

labels = json.loads((ROOT/"bench/labels/refactoring_families.v5.json").read_text())["families"]
corpus = {r["id"]: r for r in json.loads((ROOT/"bench/goldens/corpus.json").read_text())["repositories"]}
by_repo = defaultdict(list)
for f in labels: by_repo[f["repo"]].append(f)

acc = defaultdict(lambda: {"val": [], "ext": []})
for rid, labs in by_repo.items():
    repo = ROOT/"bench"/"repos"/rid
    if not repo.is_dir(): continue
    lang, split = corpus[rid]["primary_language"], corpus[rid]["split"]
    r = subprocess.run([str(NOSE),"scan",str(repo),"--format","json","--top","1000000"],
                       cwd=ROOT, capture_output=True, text=True, timeout=300)
    fams = json.loads(r.stdout or "[]")           # native order == extractability (default)
    val_order = sorted(fams, key=lambda f: -f["value"])
    def flags(order):
        out=[]
        for f in order[:10]:
            best,bo=None,0
            for lab in labs:
                o=sum(1 for s in ev.mlocs(f) for m in lab["members"] if ev.ov(s,m))
                if o>bo: best,bo=lab,o
            if best: out.append(1 if best["worthy"] else 0)
        return out
    acc[(lang,split)]["val"]+=flags(val_order)
    acc[(lang,split)]["ext"]+=flags(fams)

def fmt(fl):
    if not fl: return "-"
    m=sum(fl)/len(fl)*100; lo,hi=ev.ci(fl); return f"{m:>3.0f}% [{lo:>3.0f}-{hi:>3.0f}] n={len(fl)}"

for split in ("dev","heldout"):
    print(f"\n=== {split} ===")
    print(f"{'lang':<11}{'value':<22}{'extractability':<22}")
    for lang in sorted({l for (l,s) in acc if s==split}):
        a=acc[(lang,split)]
        print(f"{lang:<11}{fmt(a['val']):<22}{fmt(a['ext']):<22}")
    vb=[x for (l,s),v in acc.items() if s==split for x in v["val"]]
    eb=[x for (l,s),v in acc.items() if s==split for x in v["ext"]]
    print(f"{'OVERALL':<11}{fmt(vb):<22}{fmt(eb):<22}")
