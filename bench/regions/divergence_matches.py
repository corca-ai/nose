#!/usr/bin/env python3
"""Compare bounded moved-region evidence with the pre-change divergence binary."""
from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

BODY = "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n"
OTHER = "def other(y):\n    return y + 999\n"


def run(binary: Path, root: Path, workers: int = 2) -> tuple[dict, int, float]:
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env["RAYON_NUM_THREADS"] = str(workers)
    start = time.perf_counter()
    out = subprocess.run([str(binary), "query", ".", "base=HEAD", "--mode", "semantic",
                          "--min-size", "1", "--min-lines", "1", "--format", "json", "top=0"],
                         cwd=root, env=env, capture_output=True, check=True, timeout=120)
    return json.loads(out.stdout), len(out.stdout), 1000 * (time.perf_counter() - start)


def detection_and_gate_fields(value):
    if isinstance(value, dict):
        return {k: detection_and_gate_fields(v) for k, v in value.items() if k not in {"semantic_change", "next", "actions"}}
    if isinstance(value, list):
        return [detection_and_gate_fields(v) for v in value]
    return value


def evidence(report: dict) -> dict:
    return next(site["semantic_change"] for item in report["items"] for site in item["changed"]
                if site["file"] == "a.py" and site["name"] == "compute")


def audit(nose: Path, baseline: Path, samples: int) -> dict:
    result = {"schema": "nose.divergence-region-audit/v1", "samples": samples, "cases": {},
              "measurement": "local alternating process timings; not a runtime regression bound"}
    with tempfile.TemporaryDirectory(prefix="nose-divergence-region-") as temporary:
        root = Path(temporary)
        for name in ("a.py", "b.py", "c.py", "d.py"):
            (root / name).write_text(BODY if name in ("a.py", "b.py") else OTHER)
        env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        for args in (("init", "-q", "-b", "main"), ("add", "."),
                     ("-c", "user.name=nose audit", "-c", "user.email=audit@example.invalid",
                      "commit", "-qm", "base")):
            subprocess.run(["git", *args], cwd=root, env=env, check=True, capture_output=True)
        for case in ("moved", "two-copies", "actual-edit-and-copy"):
            (root / "a.py").write_text(BODY.replace("+ 7", "+ 8") if case == "actual-edit-and-copy"
                                       else "def replacement(z):\n    return z - 100\n")
            (root / "c.py").write_text(OTHER + "\n" + BODY)
            (root / "d.py").write_text(OTHER + ("\n" + BODY if case == "two-copies" else ""))
            old, old_bytes, _ = run(baseline, root)
            new, new_bytes, _ = run(nose, root)
            assert detection_and_gate_fields(old) == detection_and_gate_fields(new), case
            assert new == run(nose, root, workers=4)[0], case
            old_evidence, new_evidence = evidence(old), evidence(new)
            candidates = new_evidence["region_matches"]
            assert candidates["status"] == ("ambiguous" if case == "two-copies"
                                             else "unique-content-candidate"), case
            assert new_evidence["status"] == ("complete" if case == "actual-edit-and-copy"
                                               else "advisory"), case
            timings = {"baseline": [], "current": []}
            for sample in range(samples):
                order = [("baseline", baseline), ("current", nose)]
                for label, binary in (order if sample % 2 == 0 else reversed(order)):
                    timings[label].append(run(binary, root)[2])
            result["cases"][case] = {
                "detection_and_gate_fields_equal": True, "workers_equal": True,
                "previous_evidence_status": old_evidence["status"],
                "evidence_status": new_evidence["status"], "candidate_status": candidates["status"],
                "candidates": len(candidates["candidates"]), "files_examined": candidates["files_examined"],
                "baseline_bytes": old_bytes, "current_bytes": new_bytes, "timings_ms": timings,
                "median_ms": {label: statistics.median(values) for label, values in timings.items()},
            }
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nose", type=Path, required=True)
    parser.add_argument("--baseline-nose", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=6)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    report = audit(args.nose.resolve(), args.baseline_nose.resolve(), args.samples)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({case: {k: v for k, v in row.items() if k != "timings_ms"}
                      for case, row in report["cases"].items()}, indent=2))


if __name__ == "__main__":
    main()
