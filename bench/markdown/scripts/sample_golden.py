"""Deterministic stratified sample of candidate pairs → LLM judge input + anchor labels.

Reads the `nose markdown --dump-pairs` output and emits:
  /tmp/md_judge_sample.json  - {pairs:[{id,a,b,score,text_a,text_b}]} for the LLM panel
  /tmp/md_anchors.json       - construction-truth labels (normalized-identical → dup) for
                               self-calibration of the panel (no human, no LLM).
"""
import json, re, sys

# Optional args: <pairs.json> <sample-out.json> <anchors-out.json> (defaults below).
PAIRS = sys.argv[1] if len(sys.argv) > 1 else "/tmp/md_pairs.json"
OUT = sys.argv[2] if len(sys.argv) > 2 else "/tmp/md_judge_sample.json"
ANCH = sys.argv[3] if len(sys.argv) > 3 else "/tmp/md_anchors.json"
BANDS = [(0.0, 0.1, 25), (0.1, 0.3, 20), (0.3, 0.5, 20),
         (0.5, 0.7, 20), (0.7, 0.9, 20), (0.9, 1.001, 30)]

def norm(t):
    t = re.sub(r"```.*?```", " ", t, flags=re.S)
    t = re.sub(r"[#*_`>\-\[\]()!]", " ", t)
    return re.sub(r"\s+", " ", t).strip().lower()

def trim(t, n=420):
    t = t.replace("\n", " ").strip()
    return t[:n] + (" …" if len(t) > n else "")

def main():
    d = json.load(open(PAIRS))
    ps = d["pairs"]
    # deterministic order
    ps.sort(key=lambda p: (p["score"], p["a"]["path"], p["a"]["start"], p["b"]["path"], p["b"]["start"]))
    sample = []
    for lo, hi, k in BANDS:
        band = [p for p in ps if lo <= p["score"] < hi]
        if not band:
            continue
        # even stride across the band
        step = max(1, len(band) // k)
        picked = band[::step][:k]
        sample.extend(picked)
    # dedup by ref pair, assign ids
    seen = set()
    out = []
    anchors = []
    for p in sample:
        key = (p["a"]["path"], p["a"]["start"], p["b"]["path"], p["b"]["start"])
        if key in seen:
            continue
        seen.add(key)
        i = len(out)
        out.append({
            "id": i,
            "a": p["a"], "b": p["b"], "score": round(p["score"], 3),
            "text_a": trim(p["text_a"]), "text_b": trim(p["text_b"]),
        })
        if norm(p["text_a"]) == norm(p["text_b"]) and norm(p["text_a"]):
            anchors.append({"id": i, "label": True, "source": "construction-identical"})
    json.dump({"pairs": out}, open(OUT, "w"), ensure_ascii=False, indent=1)
    json.dump(anchors, open(ANCH, "w"), indent=1)
    print(f"sampled {len(out)} pairs; {len(anchors)} construction-identical anchors")
    import collections
    b = collections.Counter(round(p['score']*10)/10 for p in out)
    print("by band:", dict(sorted(b.items())))

if __name__ == "__main__":
    main()
