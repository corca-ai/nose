#!/usr/bin/env python3
"""Validate the checked #846 fresh-repository field audit."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/default_head_fresh_repository_audit_2026_07_14.v1.json"
SELECTION = ROOT / "bench/labels/default_head_fresh_repository_selection_2026_07_14.v1.json"
RUBRIC = ROOT / "bench/labels/RUBRIC.md"
SELECTION_SHA = "791db11158d018400bea5485a146cc882044da2d2169755b5655a2234119cd3b"
RUBRIC_SHA = "bbef42b67533921864126ce3445275090fe53526e3997eab2e995787db4c83e6"
BINARY_SHA = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
SOURCE_COMMIT = "cdab416706c32ea94bf808ec7ebb36781e483e65"

EXPECTED_OUTPUTS = {
    "check-fresh": (
        "44ce503dda41aa2422da061fdabc73e6233566e2255fe6860b6c5314f6155997",
        ["e1a9308c4b6d272b", "e30ffec71bad2fc6", "ce08511dcea5a72a", "11d67a282d349f53", "0fc7d04fd357c4d9", "b13a5475b743408e", "1d923aa30860687c", "414cb1e684aec80d", "b206b4af47cadd68", "61025c00223009f9"],
    ),
    "lipgloss-fresh": (
        "5558e1a5cc84b90c36455ff947bf2404304e771d5400027d7952d8c64c6adea2",
        ["8967c9cdfd7d874b", "3be6eb7bbb6595a5", "ee3bc9dc62c76118", "0f2149b01b7e24d5", "00d83c9a6d3bc61e", "e2a18bdfeeeee17e", "026eec909a11b765", "7e039c7377b7876c", "fabbdf020845041e", "40f029f3b8f81f35"],
    ),
    "picocli-fresh": (
        "7045ddb89d5fcf9a1b5187f34ff80648685b02b61175a3cc22965fc3fd49ee91",
        ["3f108864e50bfb20", "dd3cedb4a2634bbf", "59f4b0546356f041", "f6929ccb527e6201", "6fdbad83ac65f6d7", "68207808f17036b8", "798fdda539036da9", "4c563c860f36f863", "8684f0e42a0624bf", "38032fa92228722e"],
    ),
    "pydantic-fresh": (
        "9c9ef570c3e79192215b30f5370a19c58f82314cbb1d7d16379b2240dcb67d21",
        ["4cb6ec72bbe98a29", "1f127396e6e5a0a9", "5b3ac9f0d1c0fa0e", "ee5989e2fc5c8a11", "b3fe0113c37a45b1", "d2b17aff2150a987", "d89a99e649afd37b", "555c51eb6cf885f9", "d736d270e5eea337", "457e3aca2a03322a"],
    ),
    "dry-monads-fresh": (
        "9b6ac7854b1cc23bcc5b3ddb26a3eed88dddd73994cd86e3580c589e9388234f",
        ["a16578e897995305", "6c2fb0c95cc2cfe6", "5e4d862b66dcebfa", "0fad7243a01498e6", "7a7151b9ba09f65d", "fe01cd52e9df7865", "a57e459e20a4e770", "843331b74c4e86af", "158cb60b95819746", "f12aff93eb175d7f"],
    ),
    "console-fresh": (
        "be012aab39d1005ed514e3ebd5aa1b451e257f567ca8fa9d21a8a0672c5e9631",
        ["3fd36c0f2115770d", "4b0d645d8fb77351", "18dafc466fe9acc9", "18a3ee185c5a62dc", "b6a367e95a5db37c", "88c0a065ceaeb82d", "39f173ed5fe0836e", "55c0d2e0d8ba2680", "d57a630c63156ca0", "9ecd5d8f5f61afcb"],
    ),
    "nuke-fresh": (
        "cd4c186cea994283d74d49bfac56529a25f0b64e91bf8ab491ff20097c6763c5",
        ["e500f37c286dee74", "d8389034acac3f80", "094df44a6e13e4c7", "352e634a286f2c21", "f2ee5f18a8530d25", "f0545f4e0135af24", "f00cff7bfcf00c12", "b340a76e3ab7dbb5", "dc7b0d71987608ed", "96c9ebb491e639fb"],
    ),
    "hono-fresh": (
        "a6b2c13ab7209605debb3c184e96cca21d37c49080d5032cb711a59eda764739",
        ["8726d27e7a1b0ecd", "5d240841fe91cb8f", "1415e3cc82acc8f1", "bf3c0f33efa8909a", "480b66d90ee3069a", "d1599d485d7ae7a7", "a2d230c0538f13b6", "5b63dd7f78dc64da", "3c7963e7fd3467f6", "e2088bf766495b2a"],
    ),
}

WORTHY_REASONS = {"extract-helper", "extract-base", "extract-data-table", "parameterize"}
NOT_WORTHY_REASONS = {"parallel-by-design", "coincidental-shape", "type-def", "generated", "trivial"}
CONFIDENCE = {"high", "medium", "low"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path}: expected object")
    return value


def validate_value(value: dict[str, Any]) -> None:
    require(value.get("schema") == "nose.default_head_fresh_repository_audit.v1", "wrong schema")
    require(value.get("issue") == 846, "wrong issue")
    contract = value.get("contract", {})
    require(contract.get("selection_path") == str(SELECTION.relative_to(ROOT)), "wrong selection path")
    require(contract.get("selection_sha256") == SELECTION_SHA, "wrong selection binding")
    require(contract.get("rubric_path") == str(RUBRIC.relative_to(ROOT)), "wrong rubric path")
    require(contract.get("rubric_sha256") == RUBRIC_SHA, "wrong rubric binding")
    require(contract.get("query") == "nose query <repository> --format json", "wrong query surface")
    require(contract.get("reviewed_positions_per_repository") == 10, "wrong audit depth")
    require("cannot change" in contract.get("mutation_policy", ""), "missing frozen-product policy")

    product = value.get("product", {})
    require(product.get("source_commit") == SOURCE_COMMIT, "wrong product source")
    require(product.get("binary_sha256") == BINARY_SHA, "wrong product binary")

    selection = load(SELECTION)
    selected = selection.get("repositories", [])
    rows = value.get("repositories", [])
    require(isinstance(rows, list), "repositories must be a list")
    require([row.get("id") for row in rows] == [row.get("id") for row in selected], "selection order changed")
    require(len(rows) == 8, "expected eight repositories")

    total_worthy = 0
    seen_families: set[tuple[str, str]] = set()
    for row, chosen in zip(rows, selected, strict=True):
        repo_id = row.get("id")
        for key in ("repository", "language", "commit"):
            require(row.get(key) == chosen.get(key), f"{repo_id}: selection {key} changed")
        expected_sha, expected_ids = EXPECTED_OUTPUTS[repo_id]
        require(row.get("raw_query_sha256") == expected_sha, f"{repo_id}: raw query hash changed")
        candidates = row.get("candidates")
        require(isinstance(candidates, list) and len(candidates) == 10, f"{repo_id}: expected ten candidates")
        require([item.get("rank") for item in candidates] == list(range(1, 11)), f"{repo_id}: rank order changed")
        require([item.get("family_id") for item in candidates] == expected_ids, f"{repo_id}: family order changed")
        worthy = 0
        for item in candidates:
            family_id = item.get("family_id")
            require(isinstance(family_id, str) and re.fullmatch(r"[0-9a-f]{16}", family_id) is not None, f"{repo_id}: invalid family id")
            require((repo_id, family_id) not in seen_families, f"{repo_id}: duplicate family")
            seen_families.add((repo_id, family_id))
            is_worthy = item.get("worthy")
            require(type(is_worthy) is bool, f"{repo_id}:{family_id}: worthy must be bool")
            allowed = WORTHY_REASONS if is_worthy else NOT_WORTHY_REASONS
            require(item.get("reason") in allowed, f"{repo_id}:{family_id}: reason contradicts worthy")
            require(item.get("confidence") in CONFIDENCE, f"{repo_id}:{family_id}: invalid confidence")
            require(isinstance(item.get("note"), str) and item["note"].strip(), f"{repo_id}:{family_id}: missing note")
            worthy += int(is_worthy)
        summary = row.get("summary", {})
        require(summary == {"reported": 10, "worthy": worthy, "precision_at_10": worthy / 10}, f"{repo_id}: summary differs")
        total_worthy += worthy

    summary = value.get("summary", {})
    require(summary.get("repositories") == 8, "wrong repository total")
    require(summary.get("reported") == 80 and summary.get("reviewed") == 80, "wrong reviewed total")
    require(summary.get("worthy") == total_worthy, "wrong worthy total")
    require(summary.get("not_worthy") == 80 - total_worthy, "wrong not-worthy total")
    require(summary.get("precision_at_10") == total_worthy / 80, "wrong field precision")
    require(total_worthy == 40, "reviewed judgment total changed")
    require(all(item["reason"] == "generated" for item in rows[2]["candidates"]), "Picocli finding changed")
    require(sum(item["reason"] == "generated" for item in rows[3]["candidates"]) == 8, "Pydantic generated-output finding changed")
    findings = summary.get("follow_up_findings")
    require(isinstance(findings, list) and len(findings) == 2, "missing follow-up findings")


def validate(path: Path) -> None:
    require(sha256(SELECTION) == SELECTION_SHA, "selection file changed")
    require(sha256(RUBRIC) == RUBRIC_SHA, "rubric file changed")
    validate_value(load(path))


def self_test() -> None:
    original = load(DEFAULT)
    mutations = []

    changed = copy.deepcopy(original)
    changed["repositories"][0]["candidates"][0]["family_id"] = "0" * 16
    mutations.append(changed)

    changed = copy.deepcopy(original)
    changed["repositories"][2]["candidates"][0]["worthy"] = True
    mutations.append(changed)

    changed = copy.deepcopy(original)
    changed["repositories"][3]["summary"]["worthy"] += 1
    mutations.append(changed)

    changed = copy.deepcopy(original)
    changed["contract"]["mutation_policy"] = "retune after field review"
    mutations.append(changed)

    for index, mutation in enumerate(mutations, 1):
        try:
            validate_value(mutation)
        except ValueError:
            continue
        raise AssertionError(f"self-test mutation {index} was accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("default-head fresh-repository audit self-test passed")
    else:
        validate(args.path)
        print(f"validated {args.path}")


if __name__ == "__main__":
    main()
