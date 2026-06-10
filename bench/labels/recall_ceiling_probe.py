#!/usr/bin/env python3
"""The design.md §5 recall-ceiling probe: how many *missed worthy* families could
sub-DAG matching and pure helper inlining still recover?

For every worthy v5 label the current detector misses, this estimates whether a
GENERALIZATION of the two shipped partial-clone mechanisms (#82: shared heavy
anchors in `near` candidate mode, single-return pure inlining in the value
graph) could plausibly recover it:

- arm0  — the default product surface (`syntax,semantic`).
- arm1  — the maximal current surface (`syntax,semantic,near`, `--min-value 0`),
          i.e. everything already reachable today, including anchor partials.
- sub-DAG ceiling — for labels arm1 misses, the value-fingerprint multiset
  intersection mass between the two members' best-overlap units. A common
  connected sub-DAG can never weigh more than this intersection, so
  `mass >= floor` OVER-approximates what anchor matching at that weight floor
  could reach (reported at floors 8/12/20; 20 is the shipped ANCHOR_MIN_WEIGHT).
- inline ceiling — for labels still under every sub-DAG floor, whether adding
  ONE same-file sibling unit's value multiset to either member (a one-step
  extract-method/inline over-approximation) lifts the intersection over the
  floor.

This is a CEILING estimate, deliberately optimistic (multiset intersection
ignores connectivity; single-file `features` runs lack whole-repo import
resolution). A small number here closes the question (design.md §3 gate); a
large one justifies mechanism work, not the reverse.

Usage: python3 bench/labels/recall_ceiling_probe.py [--repos-root bench/repos]
         [--json-out bench/labels/recall_ceiling_probe_YYYY_MM_DD.json]

Deterministic: output contains no timestamps; the corpus is identified by the
same per-repo commit digest scheme frontier_platform.py uses.
"""

import argparse
import hashlib
import json
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"

SUBDAG_FLOORS = (8, 12, 20)  # 20 == nose_normalize::ANCHOR_MIN_WEIGHT
INLINE_FLOOR = 20  # one-step inline must reach the shipped anchor weight
SCAN_TIMEOUT = 600


def rel(p: str) -> str:
    p = p.replace(str(ROOT) + "/", "")
    i = p.find("bench/repos/")
    return p[i:] if i >= 0 else p


def overlaps(a, b) -> bool:
    return a["file"] == b["file"] and not (
        a["end_line"] < b["start_line"] or b["end_line"] < a["start_line"]
    )


def member_locs(fam):
    return [
        {"file": rel(loc["file"]), "start_line": loc["start_line"], "end_line": loc["end_line"]}
        for loc in fam["locations"]
    ]


def scan_families(stdout: str):
    payload = json.loads(stdout or "[]")
    if isinstance(payload, dict):
        return payload.get("families", [])
    return payload


def run_scan(repo_dir: Path, extra):
    cmd = [str(NOSE), "scan", str(repo_dir), "--format", "json", "--top", "0", *extra]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=SCAN_TIMEOUT)
    if r.returncode != 0:
        return None
    return [member_locs(f) for f in scan_families(r.stdout)]


def label_hit(label, fam_locs) -> bool:
    return any(overlaps(m, s) for fam in fam_locs for s in fam for m in label["members"])


def coverage(unit, region) -> float:
    lo = max(unit["start_line"], region["start_line"])
    hi = min(unit["end_line"], region["end_line"])
    if hi < lo:
        return 0.0
    return (hi - lo + 1) / max(1, region["end_line"] - region["start_line"] + 1)


def best_unit(units, region):
    """The TIGHTEST unit covering the region (smallest span with coverage >= 0.5),
    so a sub-function window maps to its block unit, not the whole enclosing
    function; falls back to max coverage when nothing covers half the region."""
    cands = [(coverage(u, region), u) for u in units if rel(u["path"]) == region["file"]]
    cands = [(c, u) for c, u in cands if c > 0.0]
    if not cands:
        return None
    covering = [u for c, u in cands if c >= 0.5]
    if covering:
        return min(covering, key=lambda u: (u["end_line"] - u["start_line"], u["start_line"]))
    return max(cands, key=lambda cu: cu[0])[1]


def inter_mass(a, b) -> int:
    return sum((Counter(a) & Counter(b)).values())


def features_units(files):
    cmd = [str(NOSE), "features", *files, "--min-lines", "1", "--min-tokens", "1"]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=SCAN_TIMEOUT)
    if r.returncode != 0:
        return None
    return json.loads(r.stdout)["units"]


def corpus_digest(corpus) -> str:
    h = hashlib.sha256()
    for r in sorted(corpus, key=lambda r: r["id"]):
        h.update(f"{r['id']}\t{r['split']}\t{r['primary_language']}\t{r['commit']}\n".encode())
    return h.hexdigest()


def classify_missed(label, repo_dir):
    """Estimate recoverability for one arm1-missed worthy label. Returns a record."""
    members = label["members"][:2]
    files = sorted({m["file"] for m in members})
    paths = [str(ROOT / f) for f in files]
    if not all(Path(p).is_file() for p in paths):
        return {"class": "member-file-missing"}
    units = features_units(paths)
    if units is None:
        return {"class": "features-failed"}
    ua, ub = best_unit(units, members[0]), best_unit(units, members[1])
    if ua is None or ub is None:
        return {"class": "no-overlapping-unit"}
    if ua is ub:
        # Two windows of ONE enclosing unit (the contiguous-channel shape):
        # outside unit-pair sub-DAG matching by construction — this is the
        # statement-window / fragment axis, reported as its own category.
        return {
            "class": "same-unit-window",
            "enclosing_unit_lines": [ua["start_line"], ua["end_line"]],
        }
    mass = inter_mass(ua["value"], ub["value"])
    rec = {
        "intersection_mass": mass,
        "unit_value_sizes": [len(ua["value"]), len(ub["value"])],
    }
    for floor in SUBDAG_FLOORS:
        rec[f"subdag_ge_{floor}"] = mass >= floor
    if mass >= SUBDAG_FLOORS[0]:
        rec["class"] = "subdag-ceiling"
        return rec
    # One-step inline over-approximation: one same-file sibling's value multiset,
    # added to either side, stands in for inlining one helper call.
    best_aug = mass
    for tgt, other in ((ua, ub), (ub, ua)):
        sibs = [u for u in units if rel(u["path"]) == rel(tgt["path"]) and u is not tgt]
        base = Counter(tgt["value"])
        for s in sibs:
            aug = sum(((base + Counter(s["value"])) & Counter(other["value"])).values())
            best_aug = max(best_aug, aug)
    rec["inline_aug_mass"] = best_aug
    rec["class"] = "inline-ceiling" if best_aug >= INLINE_FLOOR else "unrecovered"
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repos-root", default=str(ROOT / "bench" / "repos"))
    ap.add_argument("--json-out", default=None)
    ap.add_argument("--limit-repos", type=int, default=None, help="debug: probe first N repos only")
    args = ap.parse_args()
    repos_root = Path(args.repos_root)

    labels = json.loads((ROOT / "bench/labels/refactoring_families.v5.json").read_text())["families"]
    corpus = json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    lang_of = {r["id"]: r["primary_language"] for r in corpus}

    by_repo = defaultdict(list)
    for f in labels:
        if f["worthy"]:
            by_repo[f["repo"]].append(f)

    repo_ids = sorted(by_repo)
    if args.limit_repos:
        repo_ids = repo_ids[: args.limit_repos]

    agg = defaultdict(Counter)  # (lang, split) -> counters
    missed_records = []
    scan_failures = []

    for rid in repo_ids:
        repo_dir = repos_root / rid
        if not repo_dir.is_dir():
            continue
        labs = by_repo[rid]
        split = labs[0]["split"]
        lang = lang_of.get(rid, "?")
        arm0 = run_scan(repo_dir, [])
        arm1 = run_scan(
            repo_dir, ["--mode", "syntax,semantic,near", "--min-value", "0", "--min-members", "2"]
        )
        if arm0 is None or arm1 is None:
            scan_failures.append(rid)
            continue
        key = (lang, split)
        for lab in labs:
            agg[key]["worthy"] += 1
            h0, h1 = label_hit(lab, arm0), label_hit(lab, arm1)
            agg[key]["hit_arm0"] += h0
            agg[key]["hit_arm1"] += h1
            if h1:
                continue
            rec = classify_missed(lab, repo_dir)
            rec.update(
                family_id=lab["family_id"], repo=rid, split=split, language=lang,
                reason=lab["reason"], channel=lab["channel"],
                members=[
                    {k: m[k] for k in ("file", "start_line", "end_line")} for m in lab["members"][:2]
                ],
            )
            missed_records.append(rec)
            agg[key][rec["class"]] += 1
        sys.stderr.write(
            f"{rid}: worthy={len(labs)} missed_arm0={sum(1 for l in labs if not label_hit(l, arm0))} "
            f"missed_arm1={sum(1 for l in labs if not label_hit(l, arm1))}\n"
        )

    nose_ver = subprocess.run([str(NOSE), "--version"], capture_output=True, text=True).stdout.strip()
    out = {
        "schema_version": "0.1.0",
        "corpus_commit_digest": corpus_digest(corpus),
        "nose_version": nose_ver,
        "subdag_floors": list(SUBDAG_FLOORS),
        "inline_floor": INLINE_FLOOR,
        "scan_failures": sorted(scan_failures),
        "summary": {
            f"{lang}/{split}": dict(sorted(c.items())) for (lang, split), c in sorted(agg.items())
        },
        "missed_worthy": sorted(missed_records, key=lambda r: (r["repo"], r["family_id"])),
    }

    # ---- printed report ----
    other_classes = ["no-overlapping-unit", "member-file-missing", "features-failed"]
    for split in ("dev", "heldout"):
        rows = {k: v for k, v in agg.items() if k[1] == split}
        if not rows:
            continue
        print(f"\n=== {split} ===")
        print(f"{'lang':<12}{'worthy':>7}{'rec@arm0':>9}{'rec@arm1':>9}"
              f"{'subdag':>8}{'inline':>8}{'window':>8}{'unrec':>7}{'other':>7}")
        tot = Counter()
        for (lang, _), c in sorted(rows.items()):
            tot += c
            other = sum(c[x] for x in other_classes)
            print(f"{lang:<12}{c['worthy']:>7}{c['hit_arm0']:>9}{c['hit_arm1']:>9}"
                  f"{c['subdag-ceiling']:>8}{c['inline-ceiling']:>8}{c['same-unit-window']:>8}"
                  f"{c['unrecovered']:>7}{other:>7}")
        other = sum(tot[x] for x in other_classes)
        print(f"{'TOTAL':<12}{tot['worthy']:>7}{tot['hit_arm0']:>9}{tot['hit_arm1']:>9}"
              f"{tot['subdag-ceiling']:>8}{tot['inline-ceiling']:>8}{tot['same-unit-window']:>8}"
              f"{tot['unrecovered']:>7}{other:>7}")
        missed1 = tot["worthy"] - tot["hit_arm1"]
        if tot["worthy"]:
            print(f"worthy-recall arm0 {100 * tot['hit_arm0'] / tot['worthy']:.1f}% | "
                  f"arm1 {100 * tot['hit_arm1'] / tot['worthy']:.1f}% | "
                  f"arm1-missed {missed1} of which sub-DAG-ceiling {tot['subdag-ceiling']}, "
                  f"inline-ceiling {tot['inline-ceiling']}, "
                  f"same-unit-window {tot['same-unit-window']}")
    if scan_failures:
        print(f"\nscan failures (excluded): {', '.join(sorted(scan_failures))}")

    if args.json_out:
        Path(args.json_out).write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
        print(f"\nwrote {args.json_out}")


if __name__ == "__main__":
    main()
