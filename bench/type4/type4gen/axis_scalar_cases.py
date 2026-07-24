"""Cross-surface scalar, null, and filter-map case matrices."""

from __future__ import annotations

from functools import partial
from pathlib import Path

from type4gen.axis_case_builder import materialize_axis_cross_item
from type4gen.axis_scalar import (
    null_presence_axis_supported,
    scalar_abs_axis_supported,
)
from type4gen.axis_scalar_policy import AXIS_POLICIES
from type4gen.case_io import cross_pairs
from type4gen.model import SURFACES, GenerationFilter

make_axis_cross_item = partial(materialize_axis_cross_item, policies=AXIS_POLICIES)


def generate_hof_filter_map_cross_items(
    out_dir: Path,
    capabilities: dict,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("hof_filter_map"):
        return []
    surface_by_key = {surface.key: surface for surface in SURFACES}
    rust = surface_by_key["rust"]
    reference_surfaces = [surface_by_key["python"], surface_by_key["javascript"]]
    items: list[dict] = []
    if generation_filter.include_proposal("axis_hof_filter_map_identity"):
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_hof_filter_map_none_boundary",
        "axis_hof_filter_map_value_boundary",
        "axis_hof_filter_map_falsey_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-boundary",
                )
            )
    return items


def generate_null_presence_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("null_presence_predicate"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if null_presence_axis_supported(s, "axis_null_presence_method_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_null_presence_method_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_null_presence_method_identity",
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
                    "axis_null_presence_method_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "null_presence_predicate-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_null_presence_nonnull_boundary",
            "axis_null_presence_wrong_value_boundary",
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
                    "null-presence-boundary",
                )
            )
    return items


def generate_scalar_abs_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in (
            "axis_scalar_abs_function_identity",
            "axis_scalar_min_function_identity",
            "axis_scalar_max_function_identity",
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
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_scalar_abs_sign_boundary",
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_min_wrong_value_boundary",
            "axis_scalar_max_wrong_value_boundary",
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
                    "numeric-abs-boundary",
                )
            )
    return items


def generate_rust_numeric_method_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if cross_mode == "none" or not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    rust_surface = next(s for s in SURFACES if s.key == "rust")
    reference_surfaces = [
        s
        for s in SURFACES
        if s.key != "rust" and scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    if cross_mode == "ring":
        reference_surfaces = reference_surfaces[:3]
    items: list[dict] = []
    for proposal_id in (
        "axis_scalar_rust_abs_method_identity",
        "axis_scalar_rust_min_method_identity",
        "axis_scalar_rust_max_method_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_surface,
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
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_scalar_rust_abs_wrong_value_boundary",
        "axis_scalar_rust_min_wrong_value_boundary",
        "axis_scalar_rust_max_wrong_value_boundary",
        "axis_scalar_rust_abs_custom_method_boundary",
        "axis_scalar_rust_min_custom_method_boundary",
        "axis_scalar_rust_max_custom_method_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric-rust-method-boundary",
                )
            )
    return items
