#!/usr/bin/env python3
"""Audit saved-family EDA after a controlled Rust header edit, with optional legacy drift check."""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
import time
from pathlib import Path


def invoke(binary: Path, args: list[str], cwd: Path, workers: int = 2) -> tuple[dict, int, float]:
    import os
    env = dict(os.environ, RAYON_NUM_THREADS=str(workers))
    start = time.perf_counter()
    result = subprocess.run([str(binary), *args], cwd=cwd, env=env, capture_output=True, check=True, timeout=120)
    return json.loads(result.stdout), len(result.stdout), time.perf_counter() - start


def audit(nose: Path, baseline: Path | None, corpus: Path) -> dict:
    result = {"schema": "nose.analysis-eda-audit/v1", "corpus": str(corpus), "modes": {},
              "measurement": "single local observations; timings are diagnostic, not a performance bound"}
    with tempfile.TemporaryDirectory(prefix="nose-analysis-audit-") as temporary:
        root = Path(temporary)
        shutil.copytree(corpus, root / "source")
        captures = {}
        for mode in ("semantic", "syntax", "near", "abstraction"):
            ordinary = ["query", "source", "all", "top=0", "--mode", mode, "--format", "json"]
            query, query_bytes, _ = invoke(nose, ordinary, root)
            legacy_equal = baseline is None or invoke(baseline, ordinary, root)[0] == query
            if not legacy_equal:
                raise AssertionError(f"legacy query output drift: {mode}")
            if baseline:
                subprocess.run([str(baseline), "query", "source", "--mode", mode,
                                "--baseline", f"{mode}-baseline.json", "--write-baseline"],
                               cwd=root, capture_output=True, check=True, timeout=120)
            name = f"{mode}-before.json"
            invoke(nose, ["query", "source", "--mode", mode, "--save-analysis", name, "--format", "json"], root)
            captures[mode] = json.loads((root / name).read_text())
            result["modes"][mode] = {"legacy_query_equal": legacy_equal if baseline else None,
                                      "ordinary_query_rows": len(query["families"]), "ordinary_query_bytes": query_bytes,
                                      "capture_bytes": (root / name).stat().st_size}
        for path in (root / "source").rglob("*.rs"):
            path.write_bytes(b"// controlled unrelated header\r\n" + path.read_bytes())
        for mode, before in captures.items():
            if baseline:
                previous, previous_bytes, _ = invoke(baseline, ["query", "source", "--mode", mode,
                    f"since={mode}-baseline.json", "status!=unchanged", "all", "top=0", "--format", "json"], root)
                result["modes"][mode]["baseline_since_changed_rows"] = len(previous["families"])
                result["modes"][mode]["baseline_since_bytes"] = previous_bytes
            after_name = f"{mode}-after.json"
            invoke(nose, ["query", "source", "--mode", mode, "--save-analysis", after_name, "--format", "json"], root)
            command = ["query", "--before", f"{mode}-before.json", "--after", after_name, "--format", "json"]
            complete, full_bytes, duration = invoke(nose, [*command, "top=0"], root)
            threaded = invoke(nose, [*command, "top=0"], root, workers=4)[0]
            if complete != threaded:
                raise AssertionError(f"nondeterministic comparison: {mode}")
            after = json.loads((root / after_name).read_text())
            old = {family["id"]: family for family in before["families"]}
            new = {family["id"]: family for family in after["families"]}
            for row in complete["items"]:
                if row["unchanged_evidence"]:
                    assert complete["complete"] and complete["profile_matches"] and len(row["after"]) == 1
                    assert old[row["before"]]["review_key"] == new[row["after"][0]]["review_key"] is not None
            landing, landing_bytes, _ = invoke(nose, command, root)
            recheck, recheck_bytes, _ = invoke(nose, [*command, "evidence=recheck", "top=0"], root)
            result["modes"][mode].update({"families_before": len(before["families"]),
                "families_after": len(after["families"]), "comparison": complete["summary"],
                "complete": complete["complete"], "candidates_examined": complete["candidates_examined"],
                "recheck_changes": recheck["summary"]["selected"], "landing_shown": landing["summary"]["shown"],
                "full_comparison_bytes": full_bytes, "landing_bytes": landing_bytes,
                "recheck_bytes": recheck_bytes, "comparison_seconds": duration, "worker_equality": True})
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nose", type=Path, default=Path("target/release/nose"))
    parser.add_argument("--baseline-nose", type=Path)
    parser.add_argument("--corpus", type=Path, default=Path("crates"))
    parser.add_argument("--output", type=Path, default=Path("target/analysis-changes/audit.json"))
    args = parser.parse_args()
    report = audit(args.nose.resolve(), args.baseline_nose.resolve() if args.baseline_nose else None, args.corpus.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
