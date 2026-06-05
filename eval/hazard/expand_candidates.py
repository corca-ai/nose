#!/usr/bin/env python3
"""Build a LARGER, clone-quality-gated candidate set, reusing existing gold labels.

The first gold (1,403 candidates) was ungated — ~50% were not real clones (near@0.70).
This gates by shared_weight (the best static is_clone separator, AUC 0.68) and samples
more per repo. Candidates already labeled (matched by repo+fam_key+from) are emitted to
`reused`; the rest go to a blind file for a fresh labeling pass.

Usage: expand_candidates.py /tmp/hazard-mine old-candidates.jsonl gold-verdicts.jsonl \
         out-all.jsonl out-tolabel-blind.jsonl [--cap 450] [--min-sw 4]
"""
import glob, json, os, random, sys
random.seed(99)


def key(c):
    return (c["repo"], c["fam_key"], c["from"])


def main():
    work, oldc, goldv, out_all, out_blind = sys.argv[1:6]
    cap = int(sys.argv[sys.argv.index("--cap") + 1]) if "--cap" in sys.argv else 450
    min_sw = float(sys.argv[sys.argv.index("--min-sw") + 1]) if "--min-sw" in sys.argv else 4.0

    old = {c["cand_id"]: c for c in (json.loads(l) for l in open(oldc))}
    verd = {v["cand_id"]: v for v in (json.loads(l) for l in open(goldv))}
    # reuse: key -> verdict (label, is_positive), from already-judged old candidates
    reused = {}
    for cid, v in verd.items():
        c = old.get(cid)
        if c:
            reused[key(c)] = {"label": v["label"], "is_positive": v.get("is_positive"),
                              "label_positive": v["label"] in ("harm", "should_propagate")}

    by_repo = {}
    for path in sorted(glob.glob(os.path.join(work, "*-g1ev.jsonl"))):
        repo = os.path.basename(path).replace("-g1ev.jsonl", "")
        rows = [json.loads(l) for l in open(path)]
        rows = [r for r in rows if r["feats"]["shared_weight"] >= min_sw]  # clone-quality gate
        by_repo[repo] = rows

    allc, tolabel = [], []
    cid = 0
    for repo, rows in by_repo.items():
        random.shuffle(rows)
        for r in rows[:cap]:
            r["cand_id"] = cid
            k = key(r)
            if k in reused:
                r["reused"] = reused[k]
            else:
                tolabel.append(r)
            allc.append(r)
            cid += 1

    with open(out_all, "w") as f:
        for r in allc:
            f.write(json.dumps(r) + "\n")
    with open(out_blind, "w") as f:
        for r in tolabel:
            f.write(json.dumps({"cand_id": r["cand_id"], "repo": r["repo"],
                                "changed_member": r["changed_member"],
                                "lagging_member": r["lagging_member"],
                                "commit_subjects": r["commit_subjects"],
                                "n_members": r["n_members"]}) + "\n")

    print(f"gated candidates: {len(allc)} (cap {cap}/repo, shared_weight>={min_sw})", file=sys.stderr)
    print(f"  reused labels: {len(allc) - len(tolabel)} | to label fresh: {len(tolabel)}", file=sys.stderr)
    from collections import Counter
    print("  by repo:", dict(Counter(r["repo"] for r in allc)), file=sys.stderr)


if __name__ == "__main__":
    main()
