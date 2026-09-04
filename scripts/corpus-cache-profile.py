#!/usr/bin/env python3
"""Development measurements using the pinned cache/watch harness contracts.

This records current-binary equivalence and descriptive timings. It does not
replace the published-release comparison or its 30-replay acceptance gates.
"""
from __future__ import annotations

import argparse
import dataclasses
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

from binary_identity import binary_identity

ROOT = Path(__file__).resolve().parents[1]


def harness(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def build_source_identity() -> str:
    names = subprocess.check_output([
        "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
        "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo",
    ], cwd=ROOT).decode().split("\0")
    digest = hashlib.sha256()
    for name in sorted(set(names)):
        path = ROOT / name
        if name and path.is_file():
            digest.update(name.encode() + b"\0")
            digest.update(path.read_bytes())
            digest.update(b"\0")
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--replays", type=int, default=6)
    parser.add_argument("--roots", nargs="+", default=["sympy", "prettier", "netty", "fastlane"])
    parser.add_argument("--watch", action="store_true")
    parser.add_argument("--semantic-only", action="store_true",
                        help="use the historical cache benchmark channel instead of query defaults")
    parser.add_argument("--small-units", action="store_true",
                        help="include one-token/one-line units (explicit capacity stress)")
    args = parser.parse_args()
    if args.replays < 2:
        parser.error("at least two replays are needed for restart coverage")
    cache = harness("cache-query-regression")
    watch = harness("watch-session-benchmark")
    if not args.small_units:
        cache.DEFAULT_FLAGS = ("--min-lines", "5", "--min-size", "24")
    if not args.semantic_only:
        cache.DEFAULT_MODES = ()
    binary = args.binary.resolve()
    diff = subprocess.check_output(["git", "diff", "HEAD", "--", "crates"], cwd=ROOT)
    report = {
        "schema": "nose.corpus_cache_profile/v1",
        "purpose": "development-characterization-not-release-acceptance",
        "binary": dataclasses.asdict(binary_identity(binary)),
        "revision": args.revision,
        "source_diff_sha256": hashlib.sha256(diff).hexdigest(),
        "source_tree_sha256": build_source_identity(),
        "harness_sha256": {name: hashlib.sha256((ROOT / "scripts" / name).read_bytes()).hexdigest()
                           for name in ["corpus-cache-profile.py", "cache-query-regression.py", "watch-session-benchmark.py"]},
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "query_terms": cache.DEFAULT_TERMS, "query_flags": cache.DEFAULT_FLAGS,
        "query_modes": cache.DEFAULT_MODES or ["syntax", "semantic", "near"],
        "environment": {"platform": platform.platform(), "limits": {
            k: v for k, v in os.environ.items() if k.startswith("NOSE_MAX_")
        }},
        "replays": args.replays, "workloads": {}, "failures": {},
    }
    pins = json.loads(cache.DEFAULT_MANIFEST.read_text())["workloads"]["real"]
    pins = {item["id"]: item["commit"] for item in pins}
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save():
        args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

    with tempfile.TemporaryDirectory(prefix="nose-corpus-profile-") as temporary:
        workspace = Path(temporary)
        for name in args.roots:
            print(f"[{name}] clean/cold/history", flush=True)
            repo = ROOT / "bench" / "repos" / name
            try:
                if cache.repository_head(repo) != pins[name]:
                    raise SystemExit(f"{name}: corpus revision differs from manifest")
                identity = cache.source_identity(repo)
                rows = []
                for replay in range(1, args.replays + 1):
                    batch, cold, history = cache.run_noop_replay(
                        binary, workspace / name, repo, replay, True
                    )
                    rows.extend(batch)
                    if not cold or not history:
                        raise SystemExit(f"{name}: clean/cold/history outputs differ")
                if cache.source_identity(repo) != identity:
                    raise SystemExit(f"{name}: source changed during measurement")
                report["workloads"][name] = {
                    "commit": pins[name], "source_identity": identity,
                    "equivalent": True, "runs": rows,
                    "summary": cache.summarize_phases(rows),
                }
            except (SystemExit, OSError, ValueError) as error:
                report["failures"][name] = str(error)
            save()
        if "sympy" in args.roots:
            print("[sympy-leaf] edited production source", flush=True)
            try:
                rows = []
                for replay in range(1, args.replays + 1):
                    batch, cold, history, before, after = cache.run_real_leaf_replay(
                        binary, workspace / "sympy-leaf", ROOT / "bench/repos/sympy",
                        cache.RealLeafMutation(Path("sympy/plotting/pygletplot/plot_object.py"),
                                               "if self.visible:", "if self.visible is True:"),
                        replay, True, include_seed=True,
                    )
                    if not cold or not history:
                        raise SystemExit("sympy-leaf: outputs differ")
                    rows.extend(batch)
                report["workloads"]["sympy-leaf"] = {
                    "source_identity": {"before": before, "after": after},
                    "equivalent": True, "runs": rows, "summary": cache.summarize_phases(rows),
                }
            except (SystemExit, OSError, ValueError) as error:
                report["failures"]["sympy-leaf"] = str(error)
            save()
        if args.watch:
            for files in [10_000, 100_000]:
                try:
                    result = watch.run_tier(
                        binary, files, args.replays, workspace / f"watch-{files}"
                    )
                    report["workloads"][f"watch-{files}"] = result
                    if not result["passed"]:
                        report["failures"][f"watch-{files}"] = "watch latency/recovery target failed"
                except (SystemExit, OSError, ValueError) as error:
                    report["failures"][f"watch-{files}"] = str(error)
                save()
    report["status"] = "passed" if not report["failures"] else "failed"
    save()
    return bool(report["failures"])


if __name__ == "__main__":
    raise SystemExit(main())
