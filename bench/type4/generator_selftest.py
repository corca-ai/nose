#!/usr/bin/env python3
"""Fast structural and cross-mode checks for the Type-4 generator."""

from __future__ import annotations

import tempfile
from pathlib import Path

import generate as generator
from type4gen.model import GenerationFilter


EXPECTED_COUNTS = {
    "membership": {"none": 18, "ring": 36, "all": 90},
    "map": {"none": 90, "ring": 120, "all": 165},
    "scalar": {"none": 45, "ring": 85, "all": 225},
}

FOCUSED_FILTERS = {
    "membership": GenerationFilter(
        frozenset({"literal_collection_membership"}),
        ("axis_membership_literal_",),
    ),
    "map": GenerationFilter(
        frozenset({"map_key_membership"}),
        ("axis_map_key_",),
    ),
    "scalar": GenerationFilter(
        frozenset({"numeric_minmax_abs"}),
        ("axis_scalar_abs_",),
    ),
}

STABLE_IMPORTS = {
    "AXIS_PROPOSALS",
    "axis_data_shape",
    "axis_evidence",
    "axis_variants",
    "generate",
    "generate_axis_items",
    "make_axis_cross_item",
    "make_axis_item",
}


def check_stable_imports() -> None:
    for name in STABLE_IMPORTS:
        if not hasattr(generator, name):
            raise AssertionError(f"generate.py no longer exports {name}")


def check_focused_cross_modes(root: Path) -> None:
    for domain, generation_filter in FOCUSED_FILTERS.items():
        counts: dict[str, int] = {}
        for cross_mode in ("none", "ring", "all"):
            manifest = generator.generate(
                root / f"{domain}-{cross_mode}",
                generator.DEFAULT_PROPOSALS,
                generator.DEFAULT_CAPABILITIES,
                cross_mode,
                True,
                generation_filter,
            )
            items = manifest["items"]
            counts[cross_mode] = len(items)
            case_ids = [item["case_id"] for item in items]
            if len(case_ids) != len(set(case_ids)):
                raise AssertionError(f"{domain}/{cross_mode} generated duplicate case IDs")
            for item in items:
                axis = item["matrix"]["computation"]
                proposal_id = item["proposal_id"]
                if not generation_filter.include_axis_proposal(proposal_id, axis):
                    raise AssertionError(
                        f"{domain}/{cross_mode} escaped its filter: {proposal_id}/{axis}"
                    )
        if counts != EXPECTED_COUNTS[domain]:
            raise AssertionError(
                f"{domain} cross-mode counts changed: expected "
                f"{EXPECTED_COUNTS[domain]}, observed {counts}"
            )


def main() -> None:
    check_stable_imports()
    with tempfile.TemporaryDirectory(prefix="nose-type4-generator-selftest-") as temp_dir:
        check_focused_cross_modes(Path(temp_dir))
    print("Type-4 generator self-test passed")


if __name__ == "__main__":
    main()
