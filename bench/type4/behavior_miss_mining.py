#!/usr/bin/env python3
"""Behavior-keyed miss mining: measure the vj<0.8 Type-4 frontier (#246).

The §BK structural arm is blind below vj 0.8 by construction (its candidates are
confirmed by exact value-multiset Jaccard); true different-algorithm Type-4 clones
live exactly there. This arm replaces the candidate source with the interpreter
oracle: `nose verify --leads` groups interpretable units by concrete battery
behavior and exports under-merged pairs (behavior-equal, fingerprint-split) with
the max-vj cross-fingerprint pair per behavior group. Everything this arm can see
is therefore §AU's "oracle as generator" executed: structure plays no part in
candidate generation.

Instrument limits (stated, not hidden): only interpretable units participate
(~29% of units post-§BL.1; concrete-trace lane only), and each behavior group
contributes its single best (max-vj) pair, so counts are per-group, not per-pair.

Subcommands:
  mine      run `nose verify <repo> --leads ...` over the pinned corpus (threaded),
            merge the per-repo leads into one annotated JSON
  classify  annotate each lead: unit spans/sizes via file-scoped `nose features`,
            unreported-on-maximal-surface check, text similarity, vj band,
            triviality gate — writes the checked-in artifact
  sample    deterministic stratified sample of the unreported non-trivial
            vj<0.8 slice with embedded source, for judge labeling

Output is a QUEUE SIGNAL only (#36 discipline): records carry
`evidence_tier: oracle-suggested`; nothing here writes frontier status.
Results: docs/experiments.md §BS.
"""

import argparse
import concurrent.futures
import importlib.util
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"

spec = importlib.util.spec_from_file_location("mm", ROOT / "bench/type4/miss_mining.py")
mm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mm)

MIN_LINES, MIN_TOKENS = 5, 24  # the product floor; below it is the §H tiny-function regime
VERIFY_TIMEOUT = 1200

# The corpus may live in a different checkout than this script (worktree runs);
# artifacts keep `bench/repos/...`-relative paths, file access resolves here.
BASE = ROOT  # reset in main() from --repos-root


def norm(path):
    i = str(path).find("bench/repos/")
    return str(path)[i:] if i >= 0 else str(path)


def abspath(norm_path):
    return BASE / norm_path


def read_span(norm_path, start, end, cap=None):
    lines = abspath(norm_path).read_text(errors="replace").splitlines()
    body = lines[start - 1:end]
    if cap and len(body) > cap:
        body = body[:cap] + ["... [truncated]"]
    return body


def text_similarity(span_a, span_b):
    import difflib
    a = "\n".join(read_span(span_a["file"], span_a["start_line"], span_a["end_line"]))[:4000]
    b = "\n".join(read_span(span_b["file"], span_b["start_line"], span_b["end_line"]))[:4000]
    return round(difflib.SequenceMatcher(None, a, b, autojunk=False).ratio(), 3)


def corpus_repos(repos_root, limit=None):
    corpus = json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    repos = sorted(corpus, key=lambda r: r["id"])
    if limit:
        repos = repos[:limit]
    return [(r["id"], r["primary_language"], Path(repos_root) / r["id"])
            for r in repos if (Path(repos_root) / r["id"]).is_dir()]


def mine_repo(repo_id, lang, repo_dir, tmp_dir):
    leads_path = Path(tmp_dir) / f"{repo_id}-leads.json"
    try:
        r = subprocess.run(
            [str(NOSE), "verify", str(repo_dir), "--leads", str(leads_path)],
            capture_output=True, text=True, errors="replace", timeout=VERIFY_TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return repo_id, None, "verify-timeout"
    if not leads_path.exists():
        return repo_id, None, f"no-leads-file (exit {r.returncode})"
    doc = json.loads(leads_path.read_text())
    leads = [{**l, "repo": repo_id, "lang": lang} for l in doc.get("leads", [])]
    return repo_id, leads, None


def cmd_mine(args):
    repos = corpus_repos(args.repos_root, args.limit_repos)
    all_leads, failures = [], []
    Path(args.tmp_dir).mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as ex:
        futs = [ex.submit(mine_repo, rid, lang, rdir, args.tmp_dir)
                for rid, lang, rdir in repos]
        for fut in concurrent.futures.as_completed(futs):
            rid, leads, err = fut.result()
            if err:
                failures.append({"repo": rid, "error": err})
                print(f"[{rid}] FAIL {err}", file=sys.stderr)
            else:
                all_leads.extend(leads)
                print(f"[{rid}] {len(leads)} leads", file=sys.stderr)
    all_leads.sort(key=lambda l: (l["repo"], l["a"], l["b"]))
    out = {
        "schema_version": 1,
        "arm": "behavior-keyed (verify --leads; concrete-trace lane)",
        "repos_mined": len(repos) - len(failures),
        "failures": failures,
        "leads": all_leads,
    }
    Path(args.out).write_text(json.dumps(out, indent=1) + "\n")
    print(f"wrote {len(all_leads)} leads from {out['repos_mined']} repos -> {args.out}")


def parse_loc(loc):
    file, line = loc.rsplit(":", 1)
    return norm(file), int(line)


def unit_at(file, line, cache):
    """The features unit starting at (or covering) `line` in `file` (norm path)."""
    if file not in cache:
        doc = mm.run_json([str(NOSE), "features", str(abspath(file)),
                           "--min-lines", "1", "--min-tokens", "1"])
        cache[file] = (doc or {}).get("units", [])
    units = cache[file]
    starting = [u for u in units if u["start_line"] == line]
    if starting:
        return max(starting, key=lambda u: len(u["value"]))
    covering = [u for u in units if u["start_line"] <= line <= u["end_line"]]
    return min(covering, key=lambda u: u["end_line"] - u["start_line"]) if covering else None


def vj_band(v):
    if v >= 0.8:
        return ">=0.8"
    if v >= 0.7:
        return "0.7-0.8"
    if v >= 0.5:
        return "0.5-0.7"
    return "<0.5"


def cmd_classify(args):
    doc = json.loads(Path(args.mined).read_text())
    spans_by_repo, feat_cache = {}, {}
    out_leads = []
    for lead in doc["leads"]:
        rec = dict(lead)
        fa, la = parse_loc(lead["a"])
        fb, lb = parse_loc(lead["b"])
        ua, ub = unit_at(fa, la, feat_cache), unit_at(fb, lb, feat_cache)
        rec["band"] = vj_band(lead["vj"])
        rec["evidence_tier"] = "oracle-suggested"
        if ua and ub:
            rec["spans"] = [
                {"file": fa, "start_line": ua["start_line"], "end_line": ua["end_line"],
                 "name": ua.get("name"), "kind": ua["kind"], "tokens": len(ua["value"])},
                {"file": fb, "start_line": ub["start_line"], "end_line": ub["end_line"],
                 "name": ub.get("name"), "kind": ub["kind"], "tokens": len(ub["value"])},
            ]
            rec["trivial"] = any(
                (s["end_line"] - s["start_line"] + 1) < MIN_LINES or s["tokens"] < MIN_TOKENS
                for s in rec["spans"])
            rec["text_similarity"] = text_similarity(*rec["spans"])
            repo = lead["repo"]
            if repo not in spans_by_repo:
                spans_by_repo[repo] = mm.reported_groups(Path(args.repos_root) / repo)
            spans = spans_by_repo[repo]
            if spans is None:
                rec["unreported"] = None
            else:
                fam_a = mm.fams_covering(spans, fa, ua["start_line"], ua["end_line"])
                fam_b = mm.fams_covering(spans, fb, ub["start_line"], ub["end_line"])
                rec["unreported"] = not (fam_a & fam_b)
        else:
            rec["spans"] = None
            rec["unit_resolution"] = "failed"
        out_leads.append(rec)

    frontier = [l for l in out_leads
                if l.get("spans") and l["unreported"] and not l["trivial"]
                and l["vj"] < 0.8]
    summary = {
        "leads_total": len(out_leads),
        "by_band": dict(Counter(l["band"] for l in out_leads)),
        "frontier_vj_lt_0.8_unreported_nontrivial": len(frontier),
        "frontier_by_band": dict(Counter(l["band"] for l in frontier)),
        "frontier_by_lang": dict(Counter(l["lang"] for l in frontier)),
        "frontier_text_sim_lt_0.5": sum(1 for l in frontier
                                        if l["text_similarity"] < 0.5),
    }
    Path(args.out).write_text(json.dumps(
        {"schema_version": 1, "summary": summary,
         "failures": doc.get("failures", []), "leads": out_leads}, indent=1) + "\n")
    print(json.dumps(summary, indent=2))
    print(f"wrote {args.out}")


def snippet(file, start, end, cap=90):
    body = read_span(file, start, end, cap=cap)
    return "\n".join(f"{n}: {t}" for n, t in zip(range(start, start + len(body)), body))


def cmd_sample(args):
    doc = json.loads(Path(args.classified).read_text())
    frontier = [l for l in doc["leads"]
                if l.get("spans") and l["unreported"] and not l["trivial"]
                and l["vj"] < 0.8]
    # Deterministic stratified order: round-robin over bands, vj-desc inside.
    by_band = {}
    for l in sorted(frontier, key=lambda l: (-l["vj"], l["a"], l["b"])):
        by_band.setdefault(l["band"], []).append(l)
    take, bands = [], sorted(by_band)
    while len(take) < args.n and bands:
        for b in list(bands):
            if not by_band[b]:
                bands.remove(b)
                continue
            take.append(by_band[b].pop(0))
            if len(take) >= args.n:
                break
    out = []
    for i, l in enumerate(take):
        sa, sb = l["spans"]
        out.append({
            "sid": f"bm-{i:03d}", "repo": l["repo"], "lang": l["lang"],
            "vj": l["vj"], "band": l["band"], "text_similarity": l["text_similarity"],
            "a": {**sa, "code": snippet(sa["file"], sa["start_line"], sa["end_line"])},
            "b": {**sb, "code": snippet(sb["file"], sb["start_line"], sb["end_line"])},
        })
    with open(args.out, "w") as fh:
        for rec in out:
            fh.write(json.dumps(rec) + "\n")
    print(f"wrote {len(out)} sampled frontier pairs -> {args.out}")


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--repos-root", type=Path, default=ROOT / "bench" / "repos")
    sub = p.add_subparsers(dest="cmd", required=True)

    pm = sub.add_parser("mine")
    pm.add_argument("--jobs", type=int, default=4)
    pm.add_argument("--limit-repos", type=int)
    pm.add_argument("--tmp-dir", default="/tmp/behavior-mine")
    pm.add_argument("--out", required=True)
    pm.set_defaults(fn=cmd_mine)

    pc = sub.add_parser("classify")
    pc.add_argument("--mined", required=True)
    pc.add_argument("--out", required=True)
    pc.set_defaults(fn=cmd_classify)

    ps = sub.add_parser("sample")
    ps.add_argument("--classified", required=True)
    ps.add_argument("--n", type=int, default=50)
    ps.add_argument("--out", required=True)
    ps.set_defaults(fn=cmd_sample)

    args = p.parse_args()
    global BASE
    root = Path(args.repos_root).resolve()
    BASE = root.parent.parent if root.name == "repos" else root.parent
    args.fn(args)


if __name__ == "__main__":
    main()
