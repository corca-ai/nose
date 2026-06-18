"""Aggregate the 3-judge panel into a frozen golden + quality numbers (κ, anchor calibration).

binary positive = relation label in {dup, near}; negative = not.
Writes the committed golden and prints the panel-quality metrics required by #436.
"""
import json

WT = "/Users/ak/prjs/cc/nose/.claude/worktrees/md-dup-survey"
sample = {p["id"]: p for p in json.load(open("/tmp/md_judge_sample.json"))["pairs"]}
judges = [json.load(open(f"/tmp/md_judge_{k}.json")) for k in (1, 2, 3)]
anchors = {a["id"]: a for a in json.load(open("/tmp/md_anchors.json"))}

def pos(label):  # binary: is it a near-duplicate relation?
    return label in ("dup", "near")

# id -> [bool,bool,bool]
votes = {}
for ji, j in enumerate(judges):
    for rec in j:
        votes.setdefault(rec["id"], [None, None, None])[ji] = pos(rec["label"])

ids = sorted(i for i in sample if votes.get(i) and all(v is not None for v in votes[i]))

# Fleiss kappa (binary, 3 raters)
N = 3
P_is = []
total_pos = 0
for i in ids:
    npos = sum(votes[i])
    nneg = N - npos
    total_pos += npos
    P_is.append((npos * npos + nneg * nneg - N) / (N * (N - 1)))
P_bar = sum(P_is) / len(P_is)
p_pos = total_pos / (len(ids) * N)
P_e = p_pos ** 2 + (1 - p_pos) ** 2
kappa = (P_bar - P_e) / (1 - P_e) if P_e < 1 else 1.0

# Build golden
golden = []
unanimous = split = 0
for i in ids:
    npos = sum(votes[i])
    label = npos >= 2
    agree = "unanimous" if npos in (0, 3) else "split"
    if agree == "unanimous":
        unanimous += 1
    else:
        split += 1
    src = "llm-panel"
    if i in anchors:  # construction-certain anchor overrides
        label = True
        src = "construction-identical"
    golden.append({
        "a": sample[i]["a"], "b": sample[i]["b"],
        "label": label, "source": src, "agreement": agree,
    })

# anchor self-calibration: of construction-identical pairs, fraction the PANEL majority called positive
anchor_ids = [i for i in ids if i in anchors]
anchor_panel_pos = sum(1 for i in anchor_ids if sum(votes[i]) >= 2)
anchor_cal = anchor_panel_pos / len(anchor_ids) if anchor_ids else float("nan")

pos_n = sum(1 for g in golden if g["label"])
out = {"pairs": golden}
json.dump(out, open(f"{WT}/bench/markdown/golden.v1.json", "w"), indent=1)

summary = {
    "pairs": len(golden),
    "positives": pos_n,
    "negatives": len(golden) - pos_n,
    "fleiss_kappa": round(kappa, 3),
    "unanimous": unanimous,
    "split_2_1": split,
    "anchors": len(anchor_ids),
    "anchor_panel_calibration": round(anchor_cal, 3),
    "judge_positive_rates": [round(sum(votes[i][k] for i in ids) / len(ids), 3) for k in range(3)],
}
json.dump(summary, open(f"{WT}/bench/markdown/golden.v1.meta.json", "w"), indent=1)
print(json.dumps(summary, indent=2))
