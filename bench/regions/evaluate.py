#!/usr/bin/env python3
"""Controlled region-identity edits; no network, repository writes, or approval claims.

Run: python3 bench/regions/evaluate.py --nose target/release/nose
The assertions distinguish exact-content retention from uncertain ancestry.
Output is development evidence, not a calibrated real-history accuracy claim.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

SOURCES = {
    "py": "def compute(x):\n    return (x * x + 7) // 3\n",
    "js": "function compute(x) {\n  return (x * x + 7) / 3;\n}\n",
    "rs": "fn compute(x: i64) -> i64 {\n    return (x * x + 7) / 3;\n}\n",
    "go": "package main\nfunc compute(x int) int {\n    return (x * x + 7) / 3\n}\n",
}


def run(nose: Path, args: list[str], root: Path, threads: int = 1) -> tuple[bytes, float]:
    import os
    started = time.perf_counter()
    result = subprocess.run([str(nose), *args], cwd=root, env={**os.environ, "RAYON_NUM_THREADS": str(threads)}, capture_output=True, timeout=120, check=True)
    return result.stdout, (time.perf_counter() - started) * 1000


def capture(nose: Path, root: Path) -> tuple[dict, float]:
    raw, elapsed = run(nose, ["regions", "snapshot", "."], root)
    other, _ = run(nose, ["regions", "snapshot", "."], root, threads=4)
    assert raw == other, "snapshot changed with thread count"
    snapshot = json.loads(raw)
    assert snapshot["regions"], "fixture must admit a region"
    return snapshot, elapsed


def evaluate(nose: Path) -> dict:
    rows = []
    for language, source in SOURCES.items():
        for operation in ("unchanged", "shift", "move", "edit", "copy", "ambiguous", "budget"):
            with tempfile.TemporaryDirectory(prefix="nose-region-eval-") as directory:
                root = Path(directory)
                first = root / f"a.{language}"
                first.write_text(source)
                if operation == "ambiguous":
                    (root / f"b.{language}").write_text(source)
                before, before_ms = capture(nose, root)
                if operation in ("shift", "budget"):
                    comment = "# α heading\r\n" if language == "py" else "// α heading\r\n"
                    first.write_bytes((comment + source).encode())
                elif operation == "move":
                    first.rename(root / f"moved.{language}")
                elif operation == "edit":
                    first.write_text(source.replace("+ 7", "+ 8"))
                elif operation == "copy":
                    (root / f"copy.{language}").write_text(source)
                elif operation == "ambiguous":
                    first.rename(root / f"c.{language}")
                    (root / f"b.{language}").rename(root / f"d.{language}")
                after, after_ms = capture(nose, root)
                for name, value in (("before.json", before), ("after.json", after)):
                    (root / name).write_text(json.dumps(value))
                args = ["regions", "compare", "before.json", "after.json"]
                if operation == "budget":
                    args += ["--max-candidates", "0"]
                raw, compare_ms = run(nose, args, root)
                repeated, _ = run(nose, args, root, threads=4)
                assert raw == repeated, "correspondence changed with thread count"
                result = json.loads(raw)
                old_rows = [r for r in result["correspondences"] if r["before"] is not None]
                kinds = {r["kind"] for r in result["correspondences"]}
                if operation in ("unchanged", "shift", "move"):
                    assert all(r["unchanged_evidence"] for r in old_rows), (language, operation, result)
                elif operation == "copy":
                    assert "copied-candidate" in kinds, (language, result)
                    assert all(not r["unchanged_evidence"] for r in result["correspondences"] if r["kind"] == "copied-candidate")
                else:
                    assert not any(r["unchanged_evidence"] for r in result["correspondences"]), (language, operation, result)
                if operation == "ambiguous":
                    assert "ambiguous" in kinds
                if operation == "budget":
                    assert result["complete"] is False and "budget-exceeded" in kinds
                rows.append({"language": language, "operation": operation, "regions_before": len(before["regions"]),
                             "regions_after": len(after["regions"]), "kinds": sorted(kinds), "passed": True,
                             "capture_ms": round(before_ms + after_ms, 3), "compare_ms": round(compare_ms, 3)})
    return {"schema": "nose.region-evaluation/v1", "binary_sha256": hashlib.sha256(nose.read_bytes()).hexdigest(),
            "scope": "controlled edits; no real-history ancestry or approval qualification",
            "cases": rows, "passed": len(rows), "incorrect_unchanged_evidence": 0}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nose", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(evaluate(args.nose.resolve()), indent=2))


if __name__ == "__main__":
    main()
