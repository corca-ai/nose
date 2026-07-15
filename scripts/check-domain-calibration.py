#!/usr/bin/env python3
"""Check source-runtime facts that keep the offline oracle independently calibrated (#858)."""

from __future__ import annotations

import argparse
import copy
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT = ROOT / "bench/soundness/0.20.0/source-runtime-calibration.v1.json"

PYTHON_PROGRAM = r"""
import json
import math
import struct

def bits(value):
    return struct.pack(">d", value).hex()

left = (1e16 + -1e16) + 1.0
right = 1e16 + (-1e16 + 1.0)
derived_left = (1e16 * 1.0 + -1e16 * 1.0) + 1.0 * 1.0
derived_right = 1e16 * 1.0 + (-1e16 * 1.0 + 1.0 * 1.0)
first = [1, 2]
second = [1, 2]
first[0] = 9
second[1] = 9
print(json.dumps({
    "derived_float_associativity": {
        "left_bits": bits(derived_left), "right_bits": bits(derived_right)
    },
    "float_associativity": {"left_bits": bits(left), "right_bits": bits(right)},
    "integer_width": {"bitand": str(0xF00000003 & 0xF00000005)},
    "mutation_coordinate": {"index_0": first, "index_1": second},
    "signed_zero_nan": {
        "negative_zero": math.copysign(1.0, -0.0) < 0 < math.copysign(1.0, 0.0),
        "nan": math.isnan(float("nan")),
    },
    "string_order": {"forward": "a" + "b", "reverse": "b" + "a"},
}, sort_keys=True, separators=(",", ":")))
"""

NODE_PROGRAM = r"""
function bits(value) {
  const buffer = new ArrayBuffer(8);
  const view = new DataView(buffer);
  view.setFloat64(0, value, false);
  return view.getUint32(0, false).toString(16).padStart(8, "0") +
    view.getUint32(4, false).toString(16).padStart(8, "0");
}
const left = (1e16 + -1e16) + 1.0;
const right = 1e16 + (-1e16 + 1.0);
const derivedLeft = (1e16 * 1.0 + -1e16 * 1.0) + 1.0 * 1.0;
const derivedRight = 1e16 * 1.0 + (-1e16 * 1.0 + 1.0 * 1.0);
const literalLeft = (100000000 * 100000000 + -100000000 * 100000000) + 1;
const literalRight = 100000000 * 100000000 + (-100000000 * 100000000 + 1);
const bitwiseLeft = ((3 | 0) * (3 | 0)) * 4503599627370495;
const bitwiseRight = (3 | 0) * ((3 | 0) * 4503599627370495);
const factorLeft = 0 * Infinity + 1 * Infinity;
const factorRight = (0 + 1) * Infinity;
const reduceLeft = (1e16 + -1e16) + 1;
const reduceRight = 1e16 + (-1e16 + 1);
const first = [1, 2];
const second = [1, 2];
first[0] = 9;
second[1] = 9;
console.log(JSON.stringify({
  derived_float_associativity: {left_bits: bits(derivedLeft), right_bits: bits(derivedRight)},
  float_associativity: {left_bits: bits(left), right_bits: bits(right)},
  integer_width: {bitand: String(0xF00000003 & 0xF00000005)},
  mutation_coordinate: {index_0: first, index_1: second},
  number_edges: {
    bitwise_assoc: {left_bits: bits(bitwiseLeft), right_bits: bits(bitwiseRight)},
    bitwise_coercions: (true & 3) | (null | 0),
    coercive_pow: String("2" ** "3"),
    division_pair_left: String((1 / 0) + 1),
    division_pair_right: String((0 / 0) + (1 * 0)),
    empty_array_not: ![],
    empty_array_truthy: Boolean([]),
    exact_integer_equivalence: JSON.stringify([-(-1), 2, 3.5, -4]) === JSON.stringify([1, 2, 3.5, -4]),
    factor_distribution: {left_nan: Number.isNaN(factorLeft), right: String(factorRight)},
    large_literal_bitwise: {
      shift_right: 9223372036854775807 >> 0,
      bitand: 9223372036854775807 & 1,
      bitnot: ~9223372036854775807,
    },
    literal_assoc: {left_bits: bits(literalLeft), right_bits: bits(literalRight)},
    nested_bitwise: String(("1" - 0) & 1),
    nested_pow_nan: Number.isNaN(2 ** ("a" - "b")),
    overflow_product: {
      left_bits: bits(4611686018427387904 * 4), right_bits: bits(0 * 4)
    },
    positive_div_zero: String(1 / 0),
    reduce_association: {left_bits: bits(reduceLeft), right_bits: bits(reduceRight)},
    negative_div_zero: String(-1 / 0),
    zero_div_zero_nan: Number.isNaN(0 / 0),
    nan_truthy: Boolean(NaN),
    nan_not_equal_zero: NaN !== 0,
    shift_left: -8 << 1,
    shift_right: -8 >> 1,
    shift_masked: -8 << 33,
  },
  signed_zero_nan: {negative_zero: Object.is(-0, -0) && !Object.is(-0, 0), nan: Number.isNaN(NaN)},
  string_order: {forward: "a" + "b", reverse: "b" + "a"},
}));
"""


def run_json(command: list[str], program: str) -> dict[str, Any]:
    executable = shutil.which(command[0])
    if executable is None:
        raise RuntimeError(f"required source runtime is missing: {command[0]}")
    completed = subprocess.run(
        [executable, *command[1:]],
        input=program,
        text=True,
        capture_output=True,
        check=False,
        cwd=ROOT,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"{command[0]} calibration failed ({completed.returncode}): "
            f"{completed.stderr.strip()}"
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{command[0]} emitted invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise RuntimeError(f"{command[0]} calibration root must be an object")
    return value


def observe() -> dict[str, Any]:
    return {
        "schema": "nose.source_runtime_calibration.v1",
        "issue": 858,
        "required_oracle_distinctions": [
            "derived_float_associativity",
            "float_associativity",
            "javascript_number_edges",
            "javascript_int32_width",
            "mutation_coordinate",
            "string_order",
        ],
        "observations": {
            "node": run_json(["node"], NODE_PROGRAM),
            "python": run_json(["python3", "-"], PYTHON_PROGRAM),
        },
    }


def canonical(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def differing_paths(expected: Any, actual: Any, prefix: str = "") -> list[str]:
    if isinstance(expected, dict) and isinstance(actual, dict):
        paths: list[str] = []
        for key in sorted(set(expected) | set(actual)):
            path = f"{prefix}.{key}" if prefix else key
            if key not in expected or key not in actual:
                paths.append(path)
            else:
                paths.extend(differing_paths(expected[key], actual[key], path))
        return paths
    return [] if expected == actual else [prefix]


def check_artifact(observed: dict[str, Any]) -> None:
    if not ARTIFACT.is_file():
        raise RuntimeError(f"missing calibration artifact: {ARTIFACT.relative_to(ROOT)}")
    checked = ARTIFACT.read_bytes()
    expected = canonical(observed)
    if checked != expected:
        try:
            stored = json.loads(checked)
            paths = differing_paths(stored, observed)
        except (UnicodeDecodeError, json.JSONDecodeError):
            paths = ["<invalid-json-or-formatting>"]
        detail = ", ".join(paths[:8]) or "canonical JSON formatting"
        raise RuntimeError(f"source-runtime calibration drift: {detail}")


def internal_channel_drift(
    source: dict[str, Any], channels: dict[str, dict[str, Any]]
) -> dict[str, list[str]]:
    """Return every internal channel fact that disagrees with independent runtimes."""
    return {
        name: differing_paths(source, receipt, name)
        for name, receipt in channels.items()
        if differing_paths(source, receipt, name)
    }


def self_test(observed: dict[str, Any]) -> None:
    source = observed["observations"]
    channels = {
        "frontend": copy.deepcopy(source),
        "interpreter": copy.deepcopy(source),
    }
    for channel in channels.values():
        for runtime in ("node", "python"):
            facts = channel[runtime]["float_associativity"]
            facts["right_bits"] = facts["left_bits"]

    drift = internal_channel_drift(source, channels)
    if set(drift) != {"frontend", "interpreter"}:
        raise AssertionError("shared frontend+interpreter mutant escaped calibration")
    if not all(
        any(path.endswith("float_associativity.right_bits") for path in paths)
        for paths in drift.values()
    ):
        raise AssertionError("shared mutant was rejected for the wrong calibration fact")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--print", action="store_true", dest="print_observed")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        observed = observe()
        if args.print_observed:
            sys.stdout.buffer.write(canonical(observed))
            return 0
        check_artifact(observed)
        if args.self_test:
            self_test(observed)
            print("domain calibration comparator self-test: shared mutant rejected")
        else:
            print("domain calibration: source runtimes match checked facts")
        return 0
    except (OSError, RuntimeError, AssertionError) as error:
        print(f"domain calibration failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
