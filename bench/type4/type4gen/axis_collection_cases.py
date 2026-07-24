"""Cross-surface collection and string-affix case matrices."""

from __future__ import annotations

from functools import partial
from pathlib import Path

from type4gen.axis_case_builder import materialize_axis_cross_item
from type4gen.axis_collection_policy import AXIS_POLICIES
from type4gen.axis_collections import string_prefix_axis_supported
from type4gen.case_io import cross_pairs
from type4gen.model import SURFACES, GenerationFilter

make_axis_cross_item = partial(materialize_axis_cross_item, policies=AXIS_POLICIES)


def generate_string_prefix_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("string_prefix_suffix"):
        return []
    surfaces = [s for s in SURFACES if string_prefix_axis_supported(s, "axis_string_prefix_identity")]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in ("axis_string_prefix_identity", "axis_string_suffix_identity"):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "string_prefix_suffix-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_string_affix_boundary",
            "axis_string_direction_boundary",
            "axis_string_wrong_receiver_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "string-prefix-suffix-boundary",
                )
            )
    return items
