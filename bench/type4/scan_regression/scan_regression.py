#!/usr/bin/env python3
"""Repeatable semantic-scan performance / output-regression harness (issue #37).

This measures the *product* semantic scan path — and only that path — so detector
changes (#33 and beyond) can be checked for runtime regression and output-volume
drift in a way a fresh worker can reproduce without any chat history.

Why this exists, and the rules it follows (from the issue #37 decision):

1. The product output-drift baseline command is fixed to
   `nose scan <repo> --mode semantic --format json --top 0`.
   The hidden `nose detect` path uses a different detector/scoring route and is
   NEVER used as a substitute for product family drift. (No `detect` here at all.)

2. Candidate counts are collected only when the *same* scan path exposes them.
   Today the product JSON does not expose `candidate_pairs`, so this harness does
   not report them. It records what `--format json` and `NOSE_TIME` actually emit
   on the product path, nothing borrowed from `detect`.

3. Binary identity is mandatory. Every run records `binary_path`, `nose --version`,
   the source git SHA + dirty flag, an optional build/source ref, and the binary
   sha256 + size. A bare version string ("nose 0.5.0") does not distinguish a brew
   build from a `main` build from a PR build, so we never rely on it alone.

4. Runtime is measured WITHOUT `--cache-dir` by default (cache state would mix the
   #33 normalize/extract cost with cache effects). The small subset is repeated
   `runtime_repeats` times (>= 3) and the *median* is reported. Cache performance is
   a separate `cache` subcommand that explicitly splits cold (fresh temp cache) vs
   warm (reused cache) and never feeds the default baseline.

5. Output drift is compared on the `--top 0` full JSON, canonicalized: family order
   and ranking tie-breaks are ignored. `family_id` is the stable key, and we also
   compare normalized (repo-relative) locations, unit kind, mean_lines / span size,
   location count, and product JSON byte size. Forward-compatible: when #33 starts
   emitting `fragment_kind` / `reason_code`, those are counted automatically; until
   then we bucket by unit `kind` (e.g. `Block`) + span size as the interim view (#35).

6. Durable artifacts live next to this script in `bench/type4/scan_regression/`:
   `subset.json` (the small subset), `baseline.v1.json` (the recorded baseline),
   and the `compare` markdown summary. See `README.md`.

7. Thresholds are *investigation triggers*, not merge blockers. `compare` exits 0 by
   default even when triggers fire; `--strict` flips that for once it is calibrated.
   A single noisy wall-clock run must not fail a build.

8. The subset is data-driven (`subset.json`), so #36's recommended repos/axes can be
   dropped in. When #35's output-quality buckets land, the interim kind/span buckets
   here are where they plug in.

Usage:
    python3 bench/type4/scan_regression/scan_regression.py baseline
    python3 bench/type4/scan_regression/scan_regression.py compare
    python3 bench/type4/scan_regression/scan_regression.py cache

Run with `--help` on any subcommand for the flags.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
# Project root = the dir four levels up: <root>/bench/type4/scan_regression/.
ROOT = HERE.parents[2]
DEFAULT_SUBSET = HERE / "subset.json"
DEFAULT_BASELINE = HERE / "baseline.v1.json"
DEFAULT_SUMMARY = HERE / "compare-summary.md"

# ---------------------------------------------------------------------------
# Investigation thresholds (rule 7). These are triggers for a human to look, NOT
# automatic merge blockers. Keep them documented in README.md alongside the values.
# ---------------------------------------------------------------------------
THRESHOLDS = {
    # Relative growth in product JSON byte size that's worth a look.
    "json_bytes_pct": 0.05,
    # Per-phase / total runtime median growth worth a look. Wall-clock is noisy, so
    # this is deliberately loose and gated by an absolute floor below.
    "runtime_pct": 0.25,
    # Ignore runtime moves smaller than this many milliseconds (noise floor).
    "runtime_floor_ms": 5.0,
}


# ---------------------------------------------------------------------------
# Binary identity (rule 3)
# ---------------------------------------------------------------------------
def _git(args: list[str], cwd: Path) -> str:
    try:
        out = subprocess.run(
            ["git", *args], cwd=cwd, capture_output=True, text=True, check=True
        )
        return out.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return ""


def binary_identity(nose: str, build_ref: str | None) -> dict:
    """Everything needed to know *exactly* which binary produced a result."""
    path = Path(shutil.which(nose) or nose).resolve()
    version = ""
    try:
        version = subprocess.run(
            [str(path), "--version"], capture_output=True, text=True, check=True
        ).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError, OSError):
        version = "<unavailable>"

    sha256 = size = None
    if path.is_file():
        data = path.read_bytes()
        sha256 = hashlib.sha256(data).hexdigest()
        size = len(data)

    return {
        "binary_path": str(path),
        "version": version,
        "sha256": sha256,
        "size_bytes": size,
        "source_git_sha": _git(["rev-parse", "HEAD"], ROOT),
        "source_git_describe": _git(["describe", "--always", "--dirty", "--tags"], ROOT),
        "source_dirty": bool(_git(["status", "--porcelain"], ROOT)),
        "build_ref": build_ref or "",
    }


# ---------------------------------------------------------------------------
# Product scan path (rules 1, 2)
# ---------------------------------------------------------------------------
def scan_command(nose: str, repo: Path, scan: dict, cache_dir: Path | None) -> list[str]:
    """The ONE fixed product command. Only --top/--mode/--format come from config so
    a typo can't silently switch detector paths; everything else is constant."""
    cmd = [
        nose,
        "scan",
        str(repo),
        "--mode",
        scan.get("mode", "semantic"),
        "--format",
        scan.get("format", "json"),
        "--top",
        str(scan.get("top", 0)),
    ]
    if cache_dir is not None:
        cmd += ["--cache-dir", str(cache_dir)]
    return cmd


def parse_phase_timings(stderr: str) -> dict:
    """Parse `  [time] <stage>   12.3ms ...` lines NOSE_TIME prints to stderr.

    These come from the product scan path itself (frontend `lower` + detect stages),
    so they describe the same options/path as the product JSON (rule 2)."""
    phases: dict[str, float] = {}
    for line in stderr.splitlines():
        line = line.strip()
        if not line.startswith("[time]"):
            continue
        rest = line[len("[time]"):].strip()
        # "stage   12.3ms  (..)" — stage may be multi-word like "parse+lower".
        ms_idx = rest.find("ms")
        if ms_idx < 0:
            continue
        head = rest[:ms_idx].strip()
        parts = head.rsplit(None, 1)
        if len(parts) != 2:
            continue
        stage, val = parts
        try:
            phases[stage] = float(val)
        except ValueError:
            continue
    return phases


def run_scan(
    nose: str, repo: Path, scan: dict, cache_dir: Path | None = None
) -> tuple[dict, dict, float]:
    """Run one product scan. Returns (scan_json, phase_timings_ms, wall_ms)."""
    cmd = scan_command(nose, repo, scan, cache_dir)
    env = dict(os.environ, NOSE_TIME="1")
    t0 = time.perf_counter()
    proc = subprocess.run(cmd, capture_output=True, text=True, env=env, cwd=ROOT)
    wall_ms = (time.perf_counter() - t0) * 1e3
    if proc.returncode != 0:
        raise RuntimeError(
            f"scan failed for {repo} (exit {proc.returncode}):\n{proc.stderr[-2000:]}"
        )
    try:
        scan_json = json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"scan emitted non-JSON for {repo}: {e}") from e
    return scan_json, parse_phase_timings(proc.stderr), wall_ms


# ---------------------------------------------------------------------------
# Canonicalization (rule 5)
# ---------------------------------------------------------------------------
def _span_bucket(mean_lines: int) -> str:
    if mean_lines <= 1:
        return "1"
    if mean_lines <= 3:
        return "2-3"
    if mean_lines <= 10:
        return "4-10"
    if mean_lines <= 30:
        return "11-30"
    return "31+"


def _count_meta(obj: dict, key: str, sink: dict) -> None:
    """Count a forward-compatible metadata field (#33: fragment_kind / reason_code)
    wherever it appears, so the harness picks it up the day it ships."""
    val = obj.get(key)
    if isinstance(val, str):
        sink[val] = sink.get(val, 0) + 1


def canonicalize(scan_json: dict, repo: Path) -> dict:
    """Order-independent, corpus-location-independent product summary.

    Family order and ranking tie-breaks are dropped; locations are made
    repo-relative so the same corpus checked out elsewhere canonicalizes identically.
    """
    families_in = scan_json.get("families", [])
    repo_abs = repo.resolve()

    def relloc(loc: dict) -> str:
        f = loc.get("file", "")
        try:
            rel = os.path.relpath((ROOT / f).resolve(), repo_abs)
        except ValueError:
            rel = f
        return f"{rel}:{loc.get('start_line')}-{loc.get('end_line')}:{loc.get('kind')}"

    kind_counts: dict[str, int] = {}
    span_buckets: dict[str, int] = {}
    fragment_kind_counts: dict[str, int] = {}
    reason_code_counts: dict[str, int] = {}
    families: dict[str, dict] = {}

    for fam in families_in:
        fid = fam.get("family_id", "")
        locs = fam.get("locations", [])
        fam_kinds: dict[str, int] = {}
        for loc in locs:
            k = loc.get("kind", "?")
            kind_counts[k] = kind_counts.get(k, 0) + 1
            fam_kinds[k] = fam_kinds.get(k, 0) + 1
            _count_meta(loc, "fragment_kind", fragment_kind_counts)
            _count_meta(loc, "reason_code", reason_code_counts)
        _count_meta(fam, "fragment_kind", fragment_kind_counts)
        _count_meta(fam, "reason_code", reason_code_counts)

        mean_lines = int(fam.get("mean_lines", 0))
        bucket = _span_bucket(mean_lines)
        span_buckets[bucket] = span_buckets.get(bucket, 0) + 1

        families[fid] = {
            "members": fam.get("members"),
            "files": fam.get("files"),
            "languages": fam.get("languages"),
            "mean_lines": mean_lines,
            "span_bucket": bucket,
            "location_count": len(locs),
            "kinds": dict(sorted(fam_kinds.items())),
            "locations": sorted(relloc(l) for l in locs),
        }

    ranking = scan_json.get("ranking", {})
    scope = scan_json.get("scope", {})
    return {
        "scope_files": scope.get("files"),
        "languages": {
            l.get("language"): l.get("files") for l in scope.get("languages", [])
        },
        "total_families": ranking.get("total_families"),
        "shown_families": ranking.get("shown_families"),
        "product_json_bytes": product_json_bytes(scan_json),
        "kind_counts": dict(sorted(kind_counts.items())),
        "span_buckets": dict(sorted(span_buckets.items())),
        "fragment_kind_counts": dict(sorted(fragment_kind_counts.items())),
        "reason_code_counts": dict(sorted(reason_code_counts.items())),
        "families": families,
    }


def product_json_bytes(scan_json: dict) -> int:
    """Byte size of the product payload with the volatile `tool_version` removed, so a
    version-string change between binaries does not register as output drift."""
    payload = {k: v for k, v in scan_json.items() if k != "tool_version"}
    return len(json.dumps(payload, sort_keys=True, separators=(",", ":")))


# ---------------------------------------------------------------------------
# Runtime measurement (rule 4)
# ---------------------------------------------------------------------------
def measure_repo(
    nose: str, repo: Path, scan: dict, repeats: int
) -> tuple[dict, dict]:
    """Run a repo `repeats` times with NO cache. Returns (canonical_output, runtime).

    Output is asserted identical across runs (a determinism guard); runtime reports
    the median wall-clock and median per-phase timings."""
    walls: list[float] = []
    phase_samples: dict[str, list[float]] = {}
    canon0: dict | None = None
    for i in range(repeats):
        scan_json, phases, wall = run_scan(nose, repo, scan, cache_dir=None)
        walls.append(wall)
        for stage, ms in phases.items():
            phase_samples.setdefault(stage, []).append(ms)
        canon = canonicalize(scan_json, repo)
        if canon0 is None:
            canon0 = canon
        elif canon != canon0:
            raise RuntimeError(
                f"NON-DETERMINISTIC product output for {repo.name}: run {i} "
                f"differs from run 0 on the same binary. Investigate before trusting "
                f"any drift comparison."
            )
    runtime = {
        "repeats": repeats,
        "wall_ms_median": round(statistics.median(walls), 2),
        "wall_ms_min": round(min(walls), 2),
        "phase_ms_median": {
            s: round(statistics.median(v), 2) for s, v in sorted(phase_samples.items())
        },
    }
    return canon0 or {}, runtime


# ---------------------------------------------------------------------------
# Subset config (rules 6, 8)
# ---------------------------------------------------------------------------
def load_subset(path: Path) -> dict:
    cfg = json.loads(path.read_text())
    cfg.setdefault("repos_root", "bench/repos")
    cfg.setdefault("scan", {"mode": "semantic", "format": "json", "top": 0})
    cfg.setdefault("runtime_repeats", 5)
    if not cfg.get("repos"):
        raise ValueError(f"{path} has no `repos` list")
    return cfg


def resolve_repo(repos_root: str, repo_id: str) -> Path:
    p = (ROOT / repos_root / repo_id) if not os.path.isabs(repos_root) else Path(repos_root) / repo_id
    return p


# ---------------------------------------------------------------------------
# baseline subcommand
# ---------------------------------------------------------------------------
def cmd_baseline(args: argparse.Namespace) -> int:
    cfg = load_subset(Path(args.subset))
    repos_root = args.repos_root or cfg["repos_root"]
    ident = binary_identity(args.nose, args.build_ref)
    repeats = args.repeats or cfg["runtime_repeats"]
    print(f"binary: {ident['binary_path']}  {ident['version']}", file=sys.stderr)
    print(f"  sha256={ident['sha256']}  source={ident['source_git_describe']}", file=sys.stderr)

    repos_out: dict[str, dict] = {}
    missing: list[str] = []
    for repo_id in cfg["repos"]:
        repo = resolve_repo(repos_root, repo_id)
        if not repo.is_dir():
            missing.append(repo_id)
            print(f"  SKIP {repo_id}: not found at {repo}", file=sys.stderr)
            continue
        print(f"  scan {repo_id} (x{repeats}) ...", file=sys.stderr)
        canon, runtime = measure_repo(args.nose, repo, cfg["scan"], repeats)
        repos_out[repo_id] = {"output": canon, "runtime": runtime}

    if missing and not repos_out:
        print(
            "ERROR: no subset repos found. Populate the corpus with "
            "`bench/setup_repos.sh`, or point --repos-root at an existing checkout "
            "(e.g. the main worktree's bench/repos).",
            file=sys.stderr,
        )
        return 2

    baseline = {
        "schema_version": 1,
        "generated_by": "bench/type4/scan_regression/scan_regression.py baseline",
        "scan_command": "nose scan <repo> --mode {mode} --format {format} --top {top}".format(
            **{"mode": cfg["scan"].get("mode"), "format": cfg["scan"].get("format"), "top": cfg["scan"].get("top")}
        ),
        "binary": ident,
        "subset": {"repos_root": cfg["repos_root"], "repos": cfg["repos"], "missing": missing},
        "repos": repos_out,
    }
    out = Path(args.out)
    out.write_text(json.dumps(baseline, indent=2, sort_keys=True) + "\n")
    print(f"wrote {out} ({len(repos_out)} repos)", file=sys.stderr)
    return 0


# ---------------------------------------------------------------------------
# compare subcommand
# ---------------------------------------------------------------------------
def _diff_dict(old: dict, new: dict) -> list[str]:
    """Human-readable per-key deltas for two flat count dicts."""
    out = []
    for k in sorted(set(old) | set(new)):
        a, b = old.get(k, 0), new.get(k, 0)
        if a != b:
            out.append(f"{k}: {a} -> {b}")
    return out


def compare_repo(repo_id: str, base: dict, cur: dict) -> dict:
    """Compare one repo's baseline vs current canonical output + runtime."""
    bo, co = base["output"], cur["output"]
    triggers: list[str] = []

    # Family set drift (rule 5: family_id stable key).
    base_fams, cur_fams = set(bo["families"]), set(co["families"])
    added = sorted(cur_fams - base_fams)
    removed = sorted(base_fams - cur_fams)
    changed = []
    for fid in sorted(base_fams & cur_fams):
        bf, cf = bo["families"][fid], co["families"][fid]
        fields = ["members", "location_count", "mean_lines", "kinds", "locations"]
        if any(bf.get(f) != cf.get(f) for f in fields):
            changed.append(fid)

    if bo["total_families"] != co["total_families"]:
        triggers.append(
            f"total_families {bo['total_families']} -> {co['total_families']}"
        )
    if added or removed:
        triggers.append(f"family set: +{len(added)} / -{len(removed)}")
    if changed:
        triggers.append(f"{len(changed)} family(ies) changed shape")

    # Product JSON byte-size drift (rule 5).
    ba, ca = bo["product_json_bytes"], co["product_json_bytes"]
    if ba and abs(ca - ba) / ba > THRESHOLDS["json_bytes_pct"]:
        triggers.append(f"json bytes {ba} -> {ca} ({(ca-ba)/ba:+.1%})")

    # Kind / span / fragment bucket drift (rule 5, interim #35 buckets).
    for label, key in [
        ("kind", "kind_counts"),
        ("span", "span_buckets"),
        ("fragment_kind", "fragment_kind_counts"),
        ("reason_code", "reason_code_counts"),
    ]:
        d = _diff_dict(bo.get(key, {}), co.get(key, {}))
        if d:
            triggers.append(f"{label} counts: " + "; ".join(d))

    # Runtime drift (rule 4): median per-phase, loose + floored because it's noisy.
    rt_notes: list[str] = []
    bphase, cphase = base["runtime"]["phase_ms_median"], cur["runtime"]["phase_ms_median"]
    for stage in sorted(set(bphase) | set(cphase)):
        a, b = bphase.get(stage, 0.0), cphase.get(stage, 0.0)
        if b - a > THRESHOLDS["runtime_floor_ms"] and a > 0 and (b - a) / a > THRESHOLDS["runtime_pct"]:
            rt_notes.append(f"{stage} {a:.1f}ms -> {b:.1f}ms ({(b-a)/a:+.0%})")
    bw, cw = base["runtime"]["wall_ms_median"], cur["runtime"]["wall_ms_median"]
    if cw - bw > THRESHOLDS["runtime_floor_ms"] and bw > 0 and (cw - bw) / bw > THRESHOLDS["runtime_pct"]:
        rt_notes.append(f"wall {bw:.1f}ms -> {cw:.1f}ms ({(cw-bw)/bw:+.0%})")
    if rt_notes:
        triggers.append("runtime: " + "; ".join(rt_notes))

    return {
        "repo": repo_id,
        "triggers": triggers,
        "added": added,
        "removed": removed,
        "changed": changed,
        "wall_ms": {"baseline": bw, "current": cw},
    }


def render_summary(ident_base: dict, ident_cur: dict, results: list[dict], skipped: list[str]) -> str:
    lines = ["# Scan-regression compare summary", ""]
    lines.append("> Investigation triggers, not merge blockers (issue #37, rule 7).")
    lines.append("")
    lines.append("## Binaries")
    lines.append("")
    lines.append("| | baseline | current |")
    lines.append("|---|---|---|")
    for field in ["version", "sha256", "source_git_describe", "build_ref"]:
        lines.append(f"| {field} | `{ident_base.get(field)}` | `{ident_cur.get(field)}` |")
    if ident_base.get("sha256") and ident_base.get("sha256") == ident_cur.get("sha256"):
        lines.append("")
        lines.append("**Note: identical binary sha256 — any output/runtime delta below is environment noise, not a code change.**")
    lines.append("")

    flagged = [r for r in results if r["triggers"]]
    lines.append("## Results")
    lines.append("")
    lines.append(f"- repos compared: {len(results)}")
    lines.append(f"- repos with triggers: {len(flagged)}")
    if skipped:
        lines.append(f"- skipped (missing in one side): {', '.join(skipped)}")
    lines.append("")

    if not flagged:
        lines.append("No investigation triggers fired. ✅")
    for r in flagged:
        lines.append(f"### {r['repo']}")
        for t in r["triggers"]:
            lines.append(f"- ⚠️ {t}")
        if r["added"]:
            lines.append(f"  - added families: {', '.join(r['added'][:10])}" + (" …" if len(r["added"]) > 10 else ""))
        if r["removed"]:
            lines.append(f"  - removed families: {', '.join(r['removed'][:10])}" + (" …" if len(r["removed"]) > 10 else ""))
        if r["changed"]:
            lines.append(f"  - changed families: {', '.join(r['changed'][:10])}" + (" …" if len(r["changed"]) > 10 else ""))
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def cmd_compare(args: argparse.Namespace) -> int:
    baseline = json.loads(Path(args.baseline).read_text())
    cfg_scan = {
        "mode": "semantic",
        "format": "json",
        "top": 0,
    }
    # Trust the baseline's recorded subset so compare measures the same repos.
    repos_root = args.repos_root or baseline["subset"]["repos_root"]
    repeats = args.repeats or baseline["repos"][next(iter(baseline["repos"]))]["runtime"]["repeats"]

    ident_cur = binary_identity(args.nose, args.build_ref)
    print(f"current binary: {ident_cur['binary_path']}  {ident_cur['version']}", file=sys.stderr)

    results: list[dict] = []
    skipped: list[str] = []
    for repo_id, base_rec in baseline["repos"].items():
        repo = resolve_repo(repos_root, repo_id)
        if not repo.is_dir():
            skipped.append(repo_id)
            print(f"  SKIP {repo_id}: not found at {repo}", file=sys.stderr)
            continue
        print(f"  scan {repo_id} (x{repeats}) ...", file=sys.stderr)
        canon, runtime = measure_repo(args.nose, repo, cfg_scan, repeats)
        results.append(compare_repo(repo_id, base_rec, {"output": canon, "runtime": runtime}))

    summary = render_summary(baseline["binary"], ident_cur, results, skipped)
    Path(args.summary).write_text(summary)
    print(summary)
    print(f"wrote {args.summary}", file=sys.stderr)

    flagged = [r for r in results if r["triggers"]]
    if flagged and args.strict:
        return 1
    return 0


# ---------------------------------------------------------------------------
# cache subcommand (rule 4 — kept separate from the default baseline)
# ---------------------------------------------------------------------------
def cmd_cache(args: argparse.Namespace) -> int:
    cfg = load_subset(Path(args.subset))
    print("Cache mode: cold (fresh temp cache) vs warm (reused). Separate from the", file=sys.stderr)
    print("no-cache runtime baseline; does NOT feed baseline.v1.json (rule 4).", file=sys.stderr)
    print(f"\n{'repo':16} {'no-cache':>10} {'cold-cache':>11} {'warm-cache':>11}", file=sys.stderr)
    repos_root = args.repos_root or cfg["repos_root"]
    rows = []
    for repo_id in cfg["repos"]:
        repo = resolve_repo(repos_root, repo_id)
        if not repo.is_dir():
            print(f"  SKIP {repo_id}: not found", file=sys.stderr)
            continue
        _, _, no_cache = run_scan(args.nose, repo, cfg["scan"], cache_dir=None)
        with tempfile.TemporaryDirectory(prefix="nose-cache-") as td:
            cdir = Path(td)
            _, _, cold = run_scan(args.nose, repo, cfg["scan"], cache_dir=cdir)
            _, _, warm = run_scan(args.nose, repo, cfg["scan"], cache_dir=cdir)
        rows.append({"repo": repo_id, "no_cache_ms": round(no_cache, 2),
                     "cold_cache_ms": round(cold, 2), "warm_cache_ms": round(warm, 2)})
        print(f"{repo_id:16} {no_cache:8.1f}ms {cold:9.1f}ms {warm:9.1f}ms", file=sys.stderr)
    if args.out:
        Path(args.out).write_text(json.dumps({"schema_version": 1, "cache_runs": rows}, indent=2) + "\n")
        print(f"wrote {args.out}", file=sys.stderr)
    return 0


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    def common(sp):
        sp.add_argument("--nose", default=os.environ.get("NOSE_BIN", "nose"),
                        help="nose binary to run (default: $NOSE_BIN or `nose` on PATH)")
        sp.add_argument("--repeats", type=int, default=0,
                        help="override runtime repeats (default: subset/baseline value)")
        sp.add_argument("--build-ref", default=None,
                        help="freeform build/source ref recorded in binary identity")
        sp.add_argument("--repos-root", default=None,
                        help="corpus root override, e.g. the main worktree's bench/repos "
                             "(a fresh worktree has no corpus)")

    b = sub.add_parser("baseline", help="record a baseline.v1.json from the current binary")
    common(b)
    b.add_argument("--subset", default=str(DEFAULT_SUBSET))
    b.add_argument("--out", default=str(DEFAULT_BASELINE))
    b.set_defaults(func=cmd_baseline)

    c = sub.add_parser("compare", help="compare current binary against a baseline")
    common(c)
    c.add_argument("--baseline", default=str(DEFAULT_BASELINE))
    c.add_argument("--summary", default=str(DEFAULT_SUMMARY))
    c.add_argument("--strict", action="store_true",
                   help="exit non-zero when any trigger fires (default: 0, advisory)")
    c.set_defaults(func=cmd_compare)

    ca = sub.add_parser("cache", help="cold-vs-warm cache timing (separate from baseline)")
    common(ca)
    ca.add_argument("--subset", default=str(DEFAULT_SUBSET))
    ca.add_argument("--out", default=None)
    ca.set_defaults(func=cmd_cache)

    args = p.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
