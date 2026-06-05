#!/usr/bin/env python3
"""Build the LLM-labeling candidate set from G1 evidence (Phase B of the gold-label work).

Combines every repo's `*-g1ev.jsonl`, attaches a global id and the hazard score (so the
sample spans the score range, not just the top), and stratified-samples by repo. The
output feeds the LLM gold-labeler workflow; the score is *withheld* from the judge.

Usage: build_candidates.py /tmp/hazard-mine out-candidates.jsonl [--cap 150]
"""
import glob, json, math, os, random, sys

random.seed(1234)


def spread(f, mo, la):
    return (1.0 + 0.30 * max(min(f, 8) - 1, 0)
            + 0.50 * max(min(mo, 6) - 1, 0) + 0.50 * max(la - 1, 0))


def hazard(x):
    tight = min(max(x["shared_weight"] / max(x["mean_lines"], 1), 0.0), 1.0)
    inv = 0.3 + 0.7 * (1.0 - tight)
    sw = {"prod": 1.0, "mixed": 0.5, "test": 0.25}.get(x["scope"], 1.0)
    return x["mean_lines"] * spread(x["files"], x["modules"], x["languages"]) * inv * sw


def main():
    work = sys.argv[1]
    out = sys.argv[2]
    cap = int(sys.argv[sys.argv.index("--cap") + 1]) if "--cap" in sys.argv else 150

    by_repo = {}
    for path in sorted(glob.glob(os.path.join(work, "*-g1ev.jsonl"))):
        repo = os.path.basename(path).replace("-g1ev.jsonl", "")
        rows = [json.loads(l) for l in open(path)]
        by_repo[repo] = rows

    sampled = []
    for repo, rows in by_repo.items():
        random.shuffle(rows)
        sampled.extend(rows[:cap])
    random.shuffle(sampled)

    with open(out, "w") as f:
        for i, r in enumerate(sampled):
            r["cand_id"] = i
            r["hazard"] = round(hazard(r["feats"]), 2)  # for our eval, NOT shown to judge
            r["xlang"] = r["feats"]["languages"] > 1
            f.write(json.dumps(r) + "\n")

    print(f"candidates: {len(sampled)} (cap {cap}/repo) from {len(by_repo)} repos", file=sys.stderr)
    from collections import Counter
    print("by repo:", dict(Counter(r["repo"] for r in sampled)), file=sys.stderr)
    print(f"g2-flagged (auto, ~11% precise): {sum(r['g2'] for r in sampled)} | "
          f"true cross-language: {sum(r['xlang'] for r in sampled)}", file=sys.stderr)


if __name__ == "__main__":
    main()
