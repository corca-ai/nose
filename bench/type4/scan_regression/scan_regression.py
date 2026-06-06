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
   and ranking tie-breaks are ignored. Families are keyed by their normalized
   (repo-relative) location set, because the product `family_id` is NOT unique (distinct
   families can share one id); family_id is compared as an attribute and is itself a
   drift signal. We also compare unit kind, mean_lines / span size, location count,
   product JSON byte size, fragment product-surface placement, all-fragment vs mixed
   family shape, fragment span buckets, enclosing-unit recovery, and family-local
   `fragment_kind` / `reason_code` summaries. `fragment_kind` / `reason_code` are
   counted from product scan JSON when exact-fragment locations expose them, so #45
   metadata changes become visible as bucket drift instead of hiding inside generic
   `Block` counts.

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
    python3 bench/type4/scan_regression/scan_regression.py selftest

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
# Project root = three levels up: <root>/bench/type4/scan_regression/.
ROOT = HERE.parents[2]
# Hard wall-clock cap per single scan, so a hung scan can't make the harness wait
# forever (the subset is sub-second per repo; this is a generous safety net).
SCAN_TIMEOUT_S = 600
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

SURFACE_BUCKETS = ("default", "review", "hidden", "debug")
FAMILY_SHAPE_BUCKETS = ("whole-only", "all-fragment", "mixed")


# ---------------------------------------------------------------------------
# Binary identity (rule 3)
# ---------------------------------------------------------------------------
def nose_binary_path(nose: str) -> Path:
    return Path(shutil.which(nose) or nose).resolve()


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
    path = nose_binary_path(nose)
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
def scan_command(nose: str, scan: dict, cache_dir: Path | None) -> list[str]:
    """The ONE fixed product command. Only --top/--mode/--format come from config so
    a typo can't silently switch detector paths; everything else is constant.

    The scan target is always `.`; `run_scan` sets `cwd` to the repo so the CLI emits
    repo-relative location paths. That makes `family_id`, locations, and
    `product_json_bytes` independent of where the corpus is checked out — the same repo
    canonicalizes identically whether it lives under the main worktree or elsewhere."""
    cmd = [
        str(nose_binary_path(nose)),
        "scan",
        ".",
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
    cmd = scan_command(nose, scan, cache_dir)
    env = dict(os.environ, NOSE_TIME="1")
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, env=env, cwd=repo, timeout=SCAN_TIMEOUT_S
        )
    except subprocess.TimeoutExpired as e:
        raise RuntimeError(
            f"scan for {repo.name} exceeded {SCAN_TIMEOUT_S}s and was killed"
        ) from e
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


def _token_span_bucket(tokens: int) -> str:
    if tokens <= 0:
        return "0"
    if tokens <= 8:
        return "1-8"
    if tokens <= 23:
        return "9-23"
    if tokens <= 49:
        return "24-49"
    if tokens <= 99:
        return "50-99"
    return "100+"


def _inc(sink: dict[str, int], key: str, n: int = 1) -> None:
    sink[key] = sink.get(key, 0) + n


def _zeroed(keys: tuple[str, ...]) -> dict[str, int]:
    return {k: 0 for k in keys}


def _int_field(obj: dict, key: str) -> int | None:
    val = obj.get(key)
    if isinstance(val, bool):
        return None
    if isinstance(val, int):
        return val
    return None


def _line_span_of(loc: dict) -> int | None:
    explicit = _int_field(loc, "span_lines")
    if explicit is not None:
        return explicit
    start = _int_field(loc, "start_line")
    end = _int_field(loc, "end_line")
    if start is None or end is None:
        return None
    return max(0, end - start + 1)


def _is_fragment_location(loc: dict) -> bool:
    """Forward-compatible fragment test.

    Current scan JSON emits `is_fragment`, but older/intermediate artifacts may only
    expose `fragment_kind` / `reason_code`. Treat those as fragment evidence so drift is
    measured instead of silently disappearing when comparing across schema additions.
    """
    return (
        loc.get("is_fragment") is True
        or isinstance(loc.get("fragment_kind"), str)
        or isinstance(loc.get("reason_code"), str)
    )


def _recommended_surface(fam: dict) -> str:
    val = fam.get("recommended_surface")
    if isinstance(val, str) and val:
        return val
    return "<missing>"


def _family_shape(locs: list[dict]) -> tuple[str, int]:
    fragment_count = sum(1 for loc in locs if _is_fragment_location(loc))
    if fragment_count == 0:
        return "whole-only", fragment_count
    if fragment_count == len(locs):
        return "all-fragment", fragment_count
    return "mixed", fragment_count


def _count_meta(obj: dict, key: str, sink: dict) -> None:
    """Count a forward-compatible metadata field (fragment_kind / reason_code) wherever
    it appears. The product scan path emits these for exact-fragment locations; older
    baselines that lack the fields naturally count as empty buckets."""
    val = obj.get(key)
    if isinstance(val, str):
        _inc(sink, val)


def canonicalize(scan_json: dict, repo: Path) -> dict:
    """Order-independent, corpus-location-independent product summary.

    Family order and ranking tie-breaks are dropped; locations are made
    repo-relative so the same corpus checked out elsewhere canonicalizes identically.
    """
    families_in = scan_json.get("families", [])
    repo_abs = repo.resolve()

    def relloc(loc: dict) -> str:
        # The scan runs with cwd=repo, so `file` is already repo-relative (e.g.
        # "middleware/x_test.go"). Re-base against the repo anyway to absorb a leading
        # "./" or any absolute path, keeping the key checkout-location independent.
        f = loc.get("file", "")
        try:
            rel = os.path.relpath((repo_abs / f).resolve(), repo_abs)
        except ValueError:
            rel = f
        return f"{rel}:{loc.get('start_line')}-{loc.get('end_line')}:{loc.get('kind')}"

    kind_counts: dict[str, int] = {}
    span_buckets: dict[str, int] = {}
    recommended_surface_counts: dict[str, int] = _zeroed(SURFACE_BUCKETS)
    family_shape_counts: dict[str, int] = _zeroed(FAMILY_SHAPE_BUCKETS)
    fragment_kind_counts: dict[str, int] = {}
    reason_code_counts: dict[str, int] = {}
    fragment_kind_surface_counts: dict[str, int] = {}
    reason_code_surface_counts: dict[str, int] = {}
    fragment_line_span_buckets: dict[str, int] = {}
    fragment_token_span_buckets: dict[str, int] = {}
    enclosing_unit_recovery_counts: dict[str, int] = {"recovered": 0, "missing": 0}
    # NOTE: the product `family_id` is NOT unique — distinct families can share one id
    # (observed in chi: two Block families both keyed `c55b843732270ba0`). Keying by
    # family_id would silently drop a family, so families are keyed by their normalized
    # location set (the true identity per the #37 decision), with family_id kept as an
    # attribute (and itself a drift signal). The location-set key also keeps the diff
    # robust to family_id scheme changes.
    families: dict[str, dict] = {}

    for fam in families_in:
        locs = fam.get("locations", [])
        surface = _recommended_surface(fam)
        _inc(recommended_surface_counts, surface)
        family_shape, fragment_count = _family_shape(locs)
        _inc(family_shape_counts, family_shape)
        fam_kinds: dict[str, int] = {}
        fam_fragment_kind_counts: dict[str, int] = {}
        fam_reason_code_counts: dict[str, int] = {}
        fam_fragment_kind_surface_counts: dict[str, int] = {}
        fam_reason_code_surface_counts: dict[str, int] = {}
        fam_fragment_line_span_buckets: dict[str, int] = {}
        fam_fragment_token_span_buckets: dict[str, int] = {}
        fam_enclosing_recovery = {"recovered": 0, "missing": 0}
        for loc in locs:
            k = loc.get("kind", "?")
            kind_counts[k] = kind_counts.get(k, 0) + 1
            fam_kinds[k] = fam_kinds.get(k, 0) + 1
            _count_meta(loc, "fragment_kind", fragment_kind_counts)
            _count_meta(loc, "reason_code", reason_code_counts)
            _count_meta(loc, "fragment_kind", fam_fragment_kind_counts)
            _count_meta(loc, "reason_code", fam_reason_code_counts)
            if _is_fragment_location(loc):
                fragment_kind = loc.get("fragment_kind")
                if isinstance(fragment_kind, str):
                    _inc(fragment_kind_surface_counts, f"{fragment_kind}|{surface}")
                    _inc(fam_fragment_kind_surface_counts, f"{fragment_kind}|{surface}")
                reason_code = loc.get("reason_code")
                if isinstance(reason_code, str):
                    _inc(reason_code_surface_counts, f"{reason_code}|{surface}")
                    _inc(fam_reason_code_surface_counts, f"{reason_code}|{surface}")

                line_span = _line_span_of(loc)
                line_bucket = _span_bucket(line_span) if line_span is not None else "<missing>"
                _inc(fragment_line_span_buckets, line_bucket)
                _inc(fam_fragment_line_span_buckets, line_bucket)

                token_span = _int_field(loc, "span_tokens")
                token_bucket = (
                    _token_span_bucket(token_span) if token_span is not None else "<missing>"
                )
                _inc(fragment_token_span_buckets, token_bucket)
                _inc(fam_fragment_token_span_buckets, token_bucket)

                recovery = "recovered" if isinstance(loc.get("enclosing_unit"), dict) else "missing"
                _inc(enclosing_unit_recovery_counts, recovery)
                _inc(fam_enclosing_recovery, recovery)
        _count_meta(fam, "fragment_kind", fragment_kind_counts)
        _count_meta(fam, "reason_code", reason_code_counts)
        _count_meta(fam, "fragment_kind", fam_fragment_kind_counts)
        _count_meta(fam, "reason_code", fam_reason_code_counts)

        mean_lines = int(fam.get("mean_lines", 0))
        bucket = _span_bucket(mean_lines)
        span_buckets[bucket] = span_buckets.get(bucket, 0) + 1

        loc_list = sorted(relloc(l) for l in locs)
        key = hashlib.sha1("\n".join(loc_list).encode()).hexdigest()[:16]
        families[key] = {
            "family_id": fam.get("family_id", ""),
            "members": fam.get("members"),
            "files": fam.get("files"),
            "languages": fam.get("languages"),
            "recommended_surface": surface,
            "family_shape": family_shape,
            "mean_lines": mean_lines,
            "span_bucket": bucket,
            "location_count": len(locs),
            "fragment_location_count": fragment_count,
            "kinds": dict(sorted(fam_kinds.items())),
            "fragment_kind_counts": dict(sorted(fam_fragment_kind_counts.items())),
            "reason_code_counts": dict(sorted(fam_reason_code_counts.items())),
            "fragment_kind_surface_counts": dict(
                sorted(fam_fragment_kind_surface_counts.items())
            ),
            "reason_code_surface_counts": dict(
                sorted(fam_reason_code_surface_counts.items())
            ),
            "fragment_line_span_buckets": dict(sorted(fam_fragment_line_span_buckets.items())),
            "fragment_token_span_buckets": dict(sorted(fam_fragment_token_span_buckets.items())),
            "enclosing_unit_recovery": dict(sorted(fam_enclosing_recovery.items())),
            "locations": loc_list,
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
        # If two families ever shared an identical location set, this would fall below
        # shown_families — surfaced rather than silently collapsed.
        "distinct_location_sets": len(families),
        "product_json_bytes": product_json_bytes(scan_json),
        "kind_counts": dict(sorted(kind_counts.items())),
        "span_buckets": dict(sorted(span_buckets.items())),
        "recommended_surface_counts": dict(sorted(recommended_surface_counts.items())),
        "family_shape_counts": dict(sorted(family_shape_counts.items())),
        "fragment_kind_counts": dict(sorted(fragment_kind_counts.items())),
        "reason_code_counts": dict(sorted(reason_code_counts.items())),
        "fragment_kind_surface_counts": dict(sorted(fragment_kind_surface_counts.items())),
        "reason_code_surface_counts": dict(sorted(reason_code_surface_counts.items())),
        "fragment_line_span_buckets": dict(sorted(fragment_line_span_buckets.items())),
        "fragment_token_span_buckets": dict(sorted(fragment_token_span_buckets.items())),
        "enclosing_unit_recovery_counts": dict(sorted(enclosing_unit_recovery_counts.items())),
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

    # Family set drift (rule 5). Families are keyed by their normalized location set,
    # not the non-unique family_id; we report each by family_id + a location for humans.
    bfam, cfam = bo["families"], co["families"]
    base_keys, cur_keys = set(bfam), set(cfam)

    def label(rec: dict) -> str:
        loc = rec["locations"][0] if rec["locations"] else "?"
        return f"{rec.get('family_id') or '<no-id>'} ({loc})"

    added = [label(cfam[k]) for k in sorted(cur_keys - base_keys)]
    removed = [label(bfam[k]) for k in sorted(base_keys - cur_keys)]
    changed = []
    for k in sorted(base_keys & cur_keys):
        bf, cf = bfam[k], cfam[k]
        # locations are baked into the key, so they match; compare the rest, including
        # family_id (a new id for the same location set is itself worth a look).
        fields = [
            "family_id",
            "members",
            "location_count",
            "mean_lines",
            "recommended_surface",
            "family_shape",
            "fragment_location_count",
            "kinds",
            "fragment_kind_counts",
            "reason_code_counts",
            "fragment_kind_surface_counts",
            "reason_code_surface_counts",
            "fragment_line_span_buckets",
            "fragment_token_span_buckets",
            "enclosing_unit_recovery",
        ]
        if any(bf.get(f) != cf.get(f) for f in fields):
            changed.append(label(cf))

    if bo["total_families"] != co["total_families"]:
        triggers.append(
            f"total_families {bo['total_families']} -> {co['total_families']}"
        )
    if added or removed:
        triggers.append(f"family set: +{len(added)} / -{len(removed)}")
    if changed:
        triggers.append(f"{len(changed)} family(ies) changed shape/metadata")

    # Product JSON byte-size drift (rule 5).
    ba, ca = bo["product_json_bytes"], co["product_json_bytes"]
    if ba and abs(ca - ba) / ba > THRESHOLDS["json_bytes_pct"]:
        triggers.append(f"json bytes {ba} -> {ca} ({(ca-ba)/ba:+.1%})")

    # Kind / span / fragment product-surface drift (rule 5, issue #51 C1 buckets).
    for label, key in [
        ("kind", "kind_counts"),
        ("span", "span_buckets"),
        ("recommended_surface", "recommended_surface_counts"),
        ("family_shape", "family_shape_counts"),
        ("fragment_kind", "fragment_kind_counts"),
        ("reason_code", "reason_code_counts"),
        ("fragment_kind_surface", "fragment_kind_surface_counts"),
        ("reason_code_surface", "reason_code_surface_counts"),
        ("fragment_line_span", "fragment_line_span_buckets"),
        ("fragment_token_span", "fragment_token_span_buckets"),
        ("enclosing_unit_recovery", "enclosing_unit_recovery_counts"),
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
    lines.append(
        "> Artifact identity: current `source_git_describe` / `build_ref` name the "
        "checkout and binary that generated this report. If the report is committed "
        "after generation, those refs intentionally point at the generator commit, "
        "not at the later artifact commit."
    )
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


def _assert_eq(actual, expected, label: str) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")


def _sample_scan_json() -> dict:
    return {
        "schema_version": 1,
        "tool_version": "nose test",
        "scope": {"files": 3, "languages": [{"language": "python", "files": 3}]},
        "ranking": {"total_families": 3, "shown_families": 3, "limit": None},
        "families": [
            {
                "family_id": "whole",
                "recommended_surface": "default",
                "members": 2,
                "files": 2,
                "languages": 1,
                "mean_lines": 20,
                "locations": [
                    {
                        "file": "a.py",
                        "start_line": 1,
                        "end_line": 20,
                        "kind": "Function",
                        "is_fragment": False,
                    },
                    {
                        "file": "b.py",
                        "start_line": 1,
                        "end_line": 20,
                        "kind": "Function",
                        "is_fragment": False,
                    },
                ],
            },
            {
                "family_id": "hidden-frag",
                "recommended_surface": "hidden",
                "members": 2,
                "files": 2,
                "languages": 1,
                "mean_lines": 2,
                "locations": [
                    {
                        "file": "a.py",
                        "start_line": 4,
                        "end_line": 5,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "conditional-guard",
                        "reason_code": "exact-conditional-guard",
                        "enclosing_unit": {
                            "file": "a.py",
                            "start_line": 1,
                            "end_line": 20,
                            "kind": "Function",
                            "unit_key": "a.py:Function:1-20:",
                        },
                    },
                    {
                        "file": "b.py",
                        "start_line": 4,
                        "end_line": 5,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "conditional-guard",
                        "reason_code": "exact-conditional-guard",
                    },
                ],
            },
            {
                "family_id": "mixed-review",
                "recommended_surface": "review",
                "members": 2,
                "files": 2,
                "languages": 1,
                "mean_lines": 5,
                "locations": [
                    {
                        "file": "c.py",
                        "start_line": 1,
                        "end_line": 12,
                        "kind": "Function",
                        "is_fragment": False,
                    },
                    {
                        "file": "c.py",
                        "start_line": 10,
                        "end_line": 14,
                        "kind": "Block",
                        "span_tokens": 30,
                        "is_fragment": True,
                        "fragment_kind": "direct-return",
                        "reason_code": "exact-direct-return",
                        "enclosing_unit": {
                            "file": "c.py",
                            "start_line": 1,
                            "end_line": 14,
                            "kind": "Function",
                            "unit_key": "c.py:Function:1-14:",
                        },
                    },
                ],
            },
        ],
    }


def _balanced_swap_scan_json() -> dict:
    return {
        "schema_version": 1,
        "tool_version": "nose test",
        "scope": {"files": 4, "languages": [{"language": "python", "files": 4}]},
        "ranking": {"total_families": 2, "shown_families": 2, "limit": None},
        "families": [
            {
                "family_id": "hidden-guard",
                "recommended_surface": "hidden",
                "members": 2,
                "files": 2,
                "languages": 1,
                "mean_lines": 2,
                "locations": [
                    {
                        "file": "guard_a.py",
                        "start_line": 4,
                        "end_line": 5,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "conditional-guard",
                        "reason_code": "exact-conditional-guard",
                    },
                    {
                        "file": "guard_b.py",
                        "start_line": 4,
                        "end_line": 5,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "conditional-guard",
                        "reason_code": "exact-conditional-guard",
                    },
                ],
            },
            {
                "family_id": "hidden-return",
                "recommended_surface": "hidden",
                "members": 2,
                "files": 2,
                "languages": 1,
                "mean_lines": 2,
                "locations": [
                    {
                        "file": "return_a.py",
                        "start_line": 8,
                        "end_line": 9,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "direct-return",
                        "reason_code": "exact-direct-return",
                    },
                    {
                        "file": "return_b.py",
                        "start_line": 8,
                        "end_line": 9,
                        "kind": "Block",
                        "span_lines": 2,
                        "span_tokens": 12,
                        "is_fragment": True,
                        "fragment_kind": "direct-return",
                        "reason_code": "exact-direct-return",
                    },
                ],
            },
        ],
    }


def _with_runtime(output: dict) -> dict:
    return {
        "output": output,
        "runtime": {"wall_ms_median": 1.0, "phase_ms_median": {}},
    }


def cmd_selftest(_args: argparse.Namespace) -> int:
    cmd = scan_command(
        "./target/release/nose", {"mode": "semantic", "format": "json", "top": 0}, None
    )
    if not Path(cmd[0]).is_absolute():
        raise AssertionError(f"scan command did not absolutize nose path: {cmd[0]}")

    canon = canonicalize(_sample_scan_json(), Path("/tmp/nose-scan-regression-selftest"))
    _assert_eq(
        canon["recommended_surface_counts"],
        {"debug": 0, "default": 1, "hidden": 1, "review": 1},
        "recommended surface counts",
    )
    _assert_eq(
        canon["family_shape_counts"],
        {"all-fragment": 1, "mixed": 1, "whole-only": 1},
        "family shape counts",
    )
    _assert_eq(
        canon["fragment_kind_counts"],
        {"conditional-guard": 2, "direct-return": 1},
        "fragment kind counts",
    )
    _assert_eq(
        canon["reason_code_counts"],
        {"exact-conditional-guard": 2, "exact-direct-return": 1},
        "reason code counts",
    )
    _assert_eq(
        canon["fragment_kind_surface_counts"],
        {"conditional-guard|hidden": 2, "direct-return|review": 1},
        "fragment kind by surface counts",
    )
    _assert_eq(
        canon["reason_code_surface_counts"],
        {"exact-conditional-guard|hidden": 2, "exact-direct-return|review": 1},
        "reason code by surface counts",
    )
    _assert_eq(
        canon["fragment_line_span_buckets"],
        {"2-3": 2, "4-10": 1},
        "fragment line-span buckets",
    )
    _assert_eq(
        canon["fragment_token_span_buckets"],
        {"24-49": 1, "9-23": 2},
        "fragment token-span buckets",
    )
    _assert_eq(
        canon["enclosing_unit_recovery_counts"],
        {"missing": 1, "recovered": 2},
        "enclosing-unit recovery counts",
    )
    hidden_family = next(
        f for f in canon["families"].values() if f["family_id"] == "hidden-frag"
    )
    _assert_eq(
        hidden_family["fragment_kind_counts"],
        {"conditional-guard": 2},
        "family-local fragment kind counts",
    )
    _assert_eq(
        hidden_family["reason_code_counts"],
        {"exact-conditional-guard": 2},
        "family-local reason code counts",
    )
    _assert_eq(
        hidden_family["fragment_kind_surface_counts"],
        {"conditional-guard|hidden": 2},
        "family-local fragment kind by surface counts",
    )
    _assert_eq(
        hidden_family["reason_code_surface_counts"],
        {"exact-conditional-guard|hidden": 2},
        "family-local reason code by surface counts",
    )

    surface_changed = json.loads(json.dumps(canon))
    surface_changed["recommended_surface_counts"]["hidden"] += 1
    result = compare_repo("sample", _with_runtime(canon), _with_runtime(surface_changed))
    if not any("recommended_surface counts" in t for t in result["triggers"]):
        raise AssertionError(f"surface drift was not reported: {result['triggers']}")

    family_changed = json.loads(json.dumps(canon))
    first_family = next(iter(family_changed["families"].values()))
    first_family["recommended_surface"] = "review"
    result = compare_repo("sample", _with_runtime(canon), _with_runtime(family_changed))
    if not any("family(ies) changed shape/metadata" in t for t in result["triggers"]):
        raise AssertionError(f"family-level surface drift was not reported: {result['triggers']}")

    balanced = _balanced_swap_scan_json()
    swapped = json.loads(json.dumps(balanced))
    for loc in swapped["families"][0]["locations"]:
        loc["fragment_kind"] = "direct-return"
        loc["reason_code"] = "exact-direct-return"
    for loc in swapped["families"][1]["locations"]:
        loc["fragment_kind"] = "conditional-guard"
        loc["reason_code"] = "exact-conditional-guard"
    balanced_canon = canonicalize(balanced, Path("/tmp/nose-scan-regression-selftest"))
    swapped_canon = canonicalize(swapped, Path("/tmp/nose-scan-regression-selftest"))
    _assert_eq(
        swapped_canon["fragment_kind_surface_counts"],
        balanced_canon["fragment_kind_surface_counts"],
        "balanced swap global fragment kind by surface counts",
    )
    _assert_eq(
        swapped_canon["reason_code_surface_counts"],
        balanced_canon["reason_code_surface_counts"],
        "balanced swap global reason code by surface counts",
    )
    result = compare_repo(
        "balanced-swap",
        _with_runtime(balanced_canon),
        _with_runtime(swapped_canon),
    )
    if not any("family(ies) changed shape/metadata" in t for t in result["triggers"]):
        raise AssertionError(
            f"family-local fragment metadata drift was not reported: {result['triggers']}"
        )

    print("selftest OK")
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

    st = sub.add_parser("selftest", help="run corpus-free unit tests for canonicalization")
    st.set_defaults(func=cmd_selftest)

    args = p.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
