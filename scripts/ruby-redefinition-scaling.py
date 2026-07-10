#!/usr/bin/env python3
"""Detect superlinear Ruby same-file redefinition analysis with a fixed fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


SCHEMA = "nose.ruby_redefinition_scaling.v1"


def fixture_source(case_count: int) -> str:
    methods = []
    for index in range(case_count):
        methods.append(
            f"""\
  def present_{index:04d}(value)
    if value.nil?
      :missing_{index:04d}
    else
      value
    end
  end
"""
        )
    return "class ScalingPresenceChecks\n" + "\n".join(methods) + "end\n"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_once(binary: Path, fixture: Path) -> float:
    start = time.perf_counter()
    result = subprocess.run(
        [str(binary), "il", str(fixture), "--normalized", "--format", "json"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    if result.returncode != 0:
        raise SystemExit(
            f"Ruby scaling query failed for {fixture}: "
            f"{result.stderr.decode(errors='replace')}"
        )
    return elapsed_ms


def evaluate(
    medians_ms: dict[int, float], *, max_exponent: float, min_material_delta_ms: float
) -> dict[str, Any]:
    small, large = min(medians_ms), max(medians_ms)
    small_ms = medians_ms[small]
    large_ms = medians_ms[large]
    exponent = (
        math.log(large_ms / small_ms) / math.log(large / small)
        if small_ms > 0 and large_ms > 0 and large > small
        else 0.0
    )
    delta_ms = large_ms - small_ms
    material = delta_ms > min_material_delta_ms
    superlinear = material and exponent > max_exponent
    return {
        "delta_ms": delta_ms,
        "growth_exponent": exponent,
        "material": material,
        "max_growth_exponent": max_exponent,
        "min_material_delta_ms": min_material_delta_ms,
        "status": "regression" if superlinear else "within-threshold",
    }


def run_self_test() -> None:
    assert fixture_source(2).count(".nil?") == 2
    assert fixture_source(2) == fixture_source(2)
    linear = evaluate({10: 10.0, 40: 40.0}, max_exponent=1.35, min_material_delta_ms=5.0)
    quadratic = evaluate(
        {10: 10.0, 40: 160.0}, max_exponent=1.35, min_material_delta_ms=5.0
    )
    noisy = evaluate({10: 1.0, 40: 4.1}, max_exponent=1.0, min_material_delta_ms=5.0)
    assert linear["status"] == "within-threshold"
    assert quadratic["status"] == "regression"
    assert noisy["status"] == "within-threshold"
    print("Ruby redefinition scaling self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--small-cases", type=int, default=64)
    parser.add_argument("--large-cases", type=int, default=256)
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--max-growth-exponent", type=float, default=1.35)
    parser.add_argument("--min-material-delta-ms", type=float, default=5.0)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if args.binary is None or args.output is None:
        raise SystemExit("--binary and --output are required")
    if args.small_cases <= 0 or args.large_cases <= args.small_cases:
        raise SystemExit("case counts must satisfy 0 < small < large")
    if args.iterations <= 0 or args.warmups < 0:
        raise SystemExit("--iterations must be positive and --warmups non-negative")

    binary = args.binary.resolve()
    sizes = (args.small_cases, args.large_cases)
    with tempfile.TemporaryDirectory(prefix="nose-ruby-scaling-") as directory:
        root = Path(directory)
        fixtures = {}
        for size in sizes:
            fixture = root / f"ruby_redefinition_{size}.rb"
            fixture.write_text(fixture_source(size), encoding="utf-8")
            fixtures[size] = fixture

        for _ in range(args.warmups):
            for size in sizes:
                run_once(binary, fixtures[size])

        runs: list[dict[str, float | int]] = []
        for iteration in range(1, args.iterations + 1):
            order = sizes if iteration % 2 else tuple(reversed(sizes))
            for size in order:
                runs.append(
                    {
                        "case_count": size,
                        "elapsed_ms": run_once(binary, fixtures[size]),
                        "iteration": iteration,
                    }
                )

    medians = {
        size: statistics.median(
            row["elapsed_ms"] for row in runs if row["case_count"] == size
        )
        for size in sizes
    }
    evaluation = evaluate(
        medians,
        max_exponent=args.max_growth_exponent,
        min_material_delta_ms=args.min_material_delta_ms,
    )
    report = {
        "schema": SCHEMA,
        "binary": binary.as_posix(),
        "binary_sha256": sha256_file(binary),
        "fixture_sha256_by_case_count": {
            str(size): hashlib.sha256(fixture_source(size).encode()).hexdigest()
            for size in sizes
        },
        "iterations": args.iterations,
        "medians_ms": {str(size): medians[size] for size in sizes},
        "runs": runs,
        "evaluation": evaluation,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if evaluation["status"] == "regression":
        raise SystemExit(
            "Ruby redefinition scaling regression: "
            f"growth exponent {evaluation['growth_exponent']:.2f} exceeds "
            f"{args.max_growth_exponent:.2f}"
        )
    print(
        "Ruby redefinition scaling within threshold: "
        f"growth exponent {evaluation['growth_exponent']:.2f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
