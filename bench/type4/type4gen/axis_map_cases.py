"""Cross-surface map membership and default-lookup case matrices."""

from __future__ import annotations

from functools import partial
from pathlib import Path

from type4gen.axis_case_builder import materialize_axis_cross_item
from type4gen.axis_maps import (
    literal_map_default_axis_supported,
    map_default_lookup_axis_supported,
    map_key_membership_axis_supported,
)
from type4gen.axis_map_policy import AXIS_POLICIES
from type4gen.case_io import cross_pairs
from type4gen.model import SURFACES, GenerationFilter

make_axis_cross_item = partial(materialize_axis_cross_item, policies=AXIS_POLICIES)


def generate_map_key_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("map_key_membership"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if map_key_membership_axis_supported(s, "axis_map_key_membership_identity")
    ]
    surface_by_key = {s.key: s for s in SURFACES}
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_key_membership_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_key_membership_identity",
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
                    "axis_map_key_membership_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map_key_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_key_wrong_key_boundary",
            "axis_map_key_wrong_map_boundary",
            "axis_map_key_value_boundary",
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
                    "map-key-membership-boundary",
                )
            )
    special_views = [
        (
            surface_by_key["python"],
            (
                "axis_map_key_python_keys_in_identity",
                "axis_map_key_python_keys_contains_identity",
            ),
            (
                "axis_map_key_python_keys_wrong_key_boundary",
                "axis_map_key_python_keys_wrong_map_boundary",
                "axis_map_key_python_keys_value_boundary",
            ),
        ),
        (
            surface_by_key["typescript"],
            ("axis_map_key_ts_array_from_keys_identity",),
            (
                "axis_map_key_ts_array_from_keys_wrong_key_boundary",
                "axis_map_key_ts_array_from_keys_wrong_map_boundary",
                "axis_map_key_ts_array_from_keys_value_boundary",
            ),
        ),
    ]
    for right_surface, positive_proposals, boundary_proposals in special_views:
        reference_surfaces = [s for s in surfaces if s.key != right_surface.key]
        for proposal_id in positive_proposals:
            if not generation_filter.include_proposal(proposal_id):
                continue
            for left_surface in reference_surfaces:
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
                        "map_key_membership-semantic-mutation",
                    )
                )
        for proposal_id in boundary_proposals:
            if not generation_filter.include_proposal(proposal_id):
                continue
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "map-key-membership-boundary",
                    )
                )
    return items


def generate_literal_map_default_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("literal_map_default_lookup"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if literal_map_default_axis_supported(s, "axis_map_default_literal_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_default_literal_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_default_literal_identity",
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
                    "axis_map_default_literal_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_map_default_lookup-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_default_wrong_key_boundary",
            "axis_map_default_wrong_default_boundary",
            "axis_map_default_wrong_map_boundary",
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
                    "literal-map-default-boundary",
                )
            )

    surface_by_key = {surface.key: surface for surface in SURFACES}
    reference_surfaces = [surface_by_key["python"], surface_by_key["ruby"]]
    right_surfaces = [surface_by_key["javascript"], surface_by_key["typescript"]]
    if cross_mode == "ring":
        reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        reference_surfaces = []
    ruby_block_reference_surfaces = [surface_by_key["ruby"]]
    ruby_block_right_surfaces = [surface_by_key["ruby"]]
    for proposal_id in (
        "axis_map_default_ruby_fetch_block_int_identity",
        "axis_map_default_ruby_fetch_block_string_identity",
        "axis_map_default_ruby_fetch_block_bool_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in ruby_block_right_surfaces:
            for left_surface in ruby_block_reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_map_inline_identity",
        "axis_map_default_js_map_local_identity",
        "axis_map_default_js_map_has_get_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_map_wrong_key_boundary",
        "axis_map_default_js_map_wrong_default_boundary",
        "axis_map_default_js_map_wrong_map_boundary",
        "axis_map_default_js_map_untyped_receiver_boundary",
        "axis_map_default_js_map_shadowed_constructor_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    java_right_surfaces = [surface_by_key["java"]]
    for proposal_id in (
        "axis_map_default_java_map_of_identity",
        "axis_map_default_java_map_of_entries_identity",
        "axis_map_default_java_map_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in java_right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_java_map_wrong_key_boundary",
        "axis_map_default_java_map_wrong_default_boundary",
        "axis_map_default_java_map_wrong_map_boundary",
        "axis_map_default_java_map_shadowed_factory_boundary",
        "axis_map_default_java_map_type_shadow_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in java_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    rust_right_surfaces = [surface_by_key["rust"]]
    for proposal_id in (
        "axis_map_default_rust_hashmap_from_identity",
        "axis_map_default_rust_btreemap_from_identity",
        "axis_map_default_rust_hashmap_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in rust_right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_rust_wrong_key_boundary",
        "axis_map_default_rust_wrong_default_boundary",
        "axis_map_default_rust_wrong_map_boundary",
        "axis_map_default_rust_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in rust_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    go_right_surfaces = [surface_by_key["go"]]
    for proposal_id in (
        "axis_map_default_go_map_inline_identity",
        "axis_map_default_go_map_local_identity",
        "axis_map_default_go_map_var_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_map_wrong_key_boundary",
        "axis_map_default_go_map_wrong_map_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_zero_string_inline_identity",
        "axis_map_default_go_zero_string_local_identity",
        "axis_map_default_go_zero_bool_inline_identity",
        "axis_map_default_go_zero_float_inline_identity",
        "axis_map_default_go_zero_float_local_identity",
        "axis_map_default_go_zero_nil_pointer_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_go_zero_wrong_key_boundary",
        "axis_map_default_go_zero_wrong_map_boundary",
        "axis_map_default_go_zero_mixed_value_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in go_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    module_right_surfaces_by_proposal = {
        "axis_map_default_module_js_map_identity": [surface_by_key["javascript"]],
        "axis_map_default_module_ts_map_identity": [surface_by_key["typescript"]],
        "axis_map_default_module_java_map_identity": [surface_by_key["java"]],
    }
    for proposal_id, module_right_surfaces in module_right_surfaces_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    module_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["java"],
    ]
    for proposal_id in (
        "axis_map_default_module_wrong_key_boundary",
        "axis_map_default_module_wrong_default_boundary",
        "axis_map_default_module_wrong_map_boundary",
        "axis_map_default_module_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_map_default_module_mutated_boundary"):
        for right_surface in (surface_by_key["javascript"], surface_by_key["typescript"]):
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_map_default_module_mutated_boundary",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_object_hasown_identity",
        "axis_map_default_js_object_call_identity",
        "axis_map_default_js_object_negated_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
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
                        "literal_map_default_lookup-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_map_default_js_object_wrong_key_boundary",
        "axis_map_default_js_object_wrong_default_boundary",
        "axis_map_default_js_object_wrong_map_boundary",
        "axis_map_default_js_object_unguarded_boundary",
        "axis_map_default_js_object_in_boundary",
        "axis_map_default_js_object_method_boundary",
        "axis_map_default_js_object_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in right_surfaces:
            for left_surface in reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
    return items


def generate_map_default_lookup_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("map_default_lookup"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if map_default_lookup_axis_supported(s, "axis_map_fallback_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_map_fallback_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_identity",
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
                    "axis_map_fallback_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_map_fallback_wrong_key_boundary",
            "axis_map_fallback_wrong_default_boundary",
            "axis_map_fallback_wrong_map_boundary",
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
                    "map-default-boundary",
                )
            )
    surface_by_key = {surface.key: surface for surface in SURFACES}
    ts_surface = surface_by_key["typescript"]
    reference_surfaces = [
        surface_by_key["go"],
        surface_by_key["java"],
        surface_by_key["rust"],
    ]
    if cross_mode == "ring":
        reference_surfaces = [surface_by_key["go"]]
    elif cross_mode == "none":
        reference_surfaces = []

    for proposal_id in (
        "axis_map_fallback_ts_nullish_identity",
        "axis_map_fallback_ts_has_get_identity",
        "axis_map_fallback_ts_temp_guard_identity",
        "axis_map_fallback_ts_guard_return_identity",
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
                    ts_surface,
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
                    ts_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    java_surface = surface_by_key["java"]
    if generation_filter.include_proposal("axis_map_fallback_java_guard_return_identity"):
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_java_guard_return_identity",
                    left_surface,
                    java_surface,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_map_fallback_java_guard_return_identity",
                    left_surface,
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_map_fallback_ts_wrong_key_boundary",
        "axis_map_fallback_ts_wrong_default_boundary",
        "axis_map_fallback_ts_wrong_map_boundary",
        "axis_map_fallback_ts_untyped_boundary",
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
                    ts_surface,
                    "not_equivalent",
                    "heldout",
                    "map-default-boundary",
                )
            )
    python_surface = surface_by_key["python"]
    for proposal_id in (
        "axis_map_fallback_python_dict_get_identity",
        "axis_map_fallback_python_mapping_get_identity",
        "axis_map_fallback_python_mutable_mapping_get_identity",
        "axis_map_fallback_python_alias_mapping_identity",
        "axis_map_fallback_python_alias_mutable_mapping_identity",
        "axis_map_fallback_python_alias_dict_identity",
        "axis_map_fallback_python_guard_return_identity",
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
                    python_surface,
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
                    python_surface,
                    "not_equivalent",
                    "heldout",
                    "map_default_lookup-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_map_fallback_python_wrong_key_boundary",
        "axis_map_fallback_python_wrong_default_boundary",
        "axis_map_fallback_python_wrong_map_boundary",
        "axis_map_fallback_python_untyped_boundary",
        "axis_map_fallback_python_alias_wrong_key_boundary",
        "axis_map_fallback_python_alias_wrong_default_boundary",
        "axis_map_fallback_python_alias_wrong_map_boundary",
        "axis_map_fallback_python_alias_unresolved_boundary",
        "axis_map_fallback_python_alias_shadowed_boundary",
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
                    python_surface,
                    "not_equivalent",
                    "heldout",
                    "map-default-boundary",
                )
            )
    return items
