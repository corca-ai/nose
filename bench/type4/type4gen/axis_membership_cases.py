"""Cross-surface literal and proven-collection membership case matrices."""

from __future__ import annotations

from functools import partial
from pathlib import Path

from type4gen.axis_case_builder import materialize_axis_cross_item
from type4gen.axis_membership import literal_membership_axis_supported
from type4gen.axis_membership_policy import AXIS_POLICIES
from type4gen.case_io import cross_pairs
from type4gen.model import SURFACES, GenerationFilter

make_axis_cross_item = partial(materialize_axis_cross_item, policies=AXIS_POLICIES)


def generate_literal_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("literal_collection_membership"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if literal_membership_axis_supported(s, "axis_membership_literal_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_membership_literal_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_literal_identity",
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
                    "axis_membership_literal_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_membership_wrong_element_boundary",
            "axis_membership_wrong_collection_boundary",
            "axis_membership_substring_boundary",
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
                    "literal-membership-boundary",
                )
            )
    if generation_filter.include_proposal("axis_membership_typed_receiver_identity"):
        typed_surfaces = [
            s
            for s in SURFACES
            if literal_membership_axis_supported(s, "axis_membership_typed_receiver_identity")
        ]
        for left_surface, right_surface in cross_pairs(typed_surfaces, cross_mode):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_typed_receiver_identity",
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
                    "axis_membership_typed_receiver_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_membership_typed_wrong_element_boundary",
            "axis_membership_typed_string_boundary",
        ):
            if not generation_filter.include_proposal(proposal_id):
                continue
            boundary_surfaces = [
                s for s in SURFACES if literal_membership_axis_supported(s, proposal_id)
            ]
            for left_surface, right_surface in cross_pairs(boundary_surfaces, cross_mode):
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )

    surface_by_key = {surface.key: surface for surface in SURFACES}
    typefact_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["java"],
    ]
    if cross_mode == "ring":
        typefact_reference_surfaces = [surface_by_key["typescript"]]
    elif cross_mode == "none":
        typefact_reference_surfaces = []
    typefact_right_surface_by_proposal = {
        "axis_membership_typefact_python_tuple_identity": surface_by_key["python"],
        "axis_membership_python_alias_sequence_identity": surface_by_key["python"],
        "axis_membership_python_alias_container_identity": surface_by_key["python"],
        "axis_membership_python_alias_set_identity": surface_by_key["python"],
        "axis_membership_typefact_java_queue_identity": surface_by_key["java"],
        "axis_membership_typefact_rust_vecdeque_identity": surface_by_key["rust"],
    }
    for proposal_id, right_surface in typefact_right_surface_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in typefact_reference_surfaces:
            if left_surface.key == right_surface.key:
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
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_python_alias_wrong_element_boundary",
        "axis_membership_python_alias_wrong_receiver_boundary",
        "axis_membership_python_alias_unresolved_boundary",
        "axis_membership_python_alias_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        right_surface = surface_by_key["python"]
        for left_surface in typefact_reference_surfaces:
            if left_surface.key == right_surface.key:
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
                    "literal-membership-boundary",
                )
            )
    python_factory_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["java"],
    ]
    if cross_mode == "ring":
        python_factory_reference_surfaces = [surface_by_key["typescript"]]
    elif cross_mode == "none":
        python_factory_reference_surfaces = []
    python_factory_right = surface_by_key["python"]
    for proposal_id in (
        "axis_membership_python_set_factory_identity",
        "axis_membership_python_tuple_factory_identity",
        "axis_membership_python_frozenset_factory_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_factory_reference_surfaces:
            if left_surface.key == python_factory_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_factory_right,
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
                    python_factory_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    python_deque_reference_surfaces = python_factory_reference_surfaces
    python_deque_right = surface_by_key["python"]
    for proposal_id in (
        "axis_membership_python_deque_import_identity",
        "axis_membership_python_deque_alias_identity",
        "axis_membership_python_deque_namespace_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_deque_reference_surfaces:
            if left_surface.key == python_deque_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_deque_right,
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
                    python_deque_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_python_deque_wrong_element_boundary",
        "axis_membership_python_deque_wrong_collection_boundary",
        "axis_membership_python_deque_missing_import_boundary",
        "axis_membership_python_deque_shadowed_boundary",
        "axis_membership_python_deque_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in python_deque_reference_surfaces:
            if left_surface.key == python_deque_right.key:
                continue
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    python_deque_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    local_constructed_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    if cross_mode == "ring":
        local_constructed_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        local_constructed_reference_surfaces = []
    local_constructed_right_surface_by_proposal = {
        "axis_membership_local_go_slice_identity": surface_by_key["go"],
        "axis_membership_local_java_list_identity": surface_by_key["java"],
        "axis_membership_local_rust_vec_identity": surface_by_key["rust"],
    }
    for proposal_id, right_surface in local_constructed_right_surface_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in local_constructed_reference_surfaces:
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
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_local_wrong_element_boundary",
        "axis_membership_local_wrong_collection_boundary",
        "axis_membership_local_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in (
            surface_by_key["go"],
            surface_by_key["java"],
            surface_by_key["rust"],
        ):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface_by_key["python"],
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    set_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["go"],
        surface_by_key["rust"],
        surface_by_key["ruby"],
    ]
    set_right_surfaces = [surface_by_key["javascript"], surface_by_key["typescript"]]
    if cross_mode == "ring":
        set_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        set_reference_surfaces = []
    for proposal_id in (
        "axis_membership_set_inline_identity",
        "axis_membership_set_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in set_right_surfaces:
            for left_surface in set_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    if generation_filter.include_proposal("axis_membership_set_param_identity"):
        typed_reference_surfaces = [
            surface_by_key["python"],
            surface_by_key["go"],
            surface_by_key["rust"],
            surface_by_key["java"],
        ]
        if cross_mode == "ring":
            typed_reference_surfaces = [surface_by_key["python"]]
        elif cross_mode == "none":
            typed_reference_surfaces = []
        for left_surface in typed_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_set_param_identity",
                    left_surface,
                    surface_by_key["typescript"],
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_set_param_identity",
                    left_surface,
                    surface_by_key["typescript"],
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_set_wrong_element_boundary",
        "axis_membership_set_wrong_collection_boundary",
        "axis_membership_set_untyped_receiver_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in set_right_surfaces:
            for left_surface in set_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_some_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_some_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_some_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_some_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_some_identity"):
        for right_surface in array_some_right_surfaces:
            for left_surface in array_some_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_some_identity",
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
                        "axis_membership_array_some_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_some_wrong_element_boundary",
        "axis_membership_array_some_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_some_right_surfaces:
            for left_surface in array_some_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_every_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_every_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_every_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_every_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_every_absence_identity"):
        for right_surface in array_every_right_surfaces:
            for left_surface in array_every_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_every_absence_identity",
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
                        "axis_membership_array_every_absence_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_every_wrong_element_boundary",
        "axis_membership_array_every_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_every_right_surfaces:
            for left_surface in array_every_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_indexof_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_indexof_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_indexof_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_indexof_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_indexof_identity"):
        for right_surface in array_indexof_right_surfaces:
            for left_surface in array_indexof_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_indexof_identity",
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
                        "axis_membership_array_indexof_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_indexof_wrong_element_boundary",
        "axis_membership_array_indexof_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_indexof_right_surfaces:
            for left_surface in array_indexof_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_findindex_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_findindex_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_findindex_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_findindex_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_findindex_identity"):
        for right_surface in array_findindex_right_surfaces:
            for left_surface in array_findindex_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_findindex_identity",
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
                        "axis_membership_array_findindex_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_findindex_wrong_element_boundary",
        "axis_membership_array_findindex_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_findindex_right_surfaces:
            for left_surface in array_findindex_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_filter_length_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_filter_length_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_filter_length_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_filter_length_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_filter_length_identity"):
        for right_surface in array_filter_length_right_surfaces:
            for left_surface in array_filter_length_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_identity",
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
                        "axis_membership_array_filter_length_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_filter_length_wrong_element_boundary",
        "axis_membership_array_filter_length_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_filter_length_right_surfaces:
            for left_surface in array_filter_length_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    array_filter_length_absence_reference_surfaces = [
        surface_by_key["python"],
        surface_by_key["ruby"],
        surface_by_key["javascript"],
        surface_by_key["typescript"],
    ]
    array_filter_length_absence_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["vue"],
        surface_by_key["svelte"],
        surface_by_key["html"],
    ]
    if cross_mode == "ring":
        array_filter_length_absence_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        array_filter_length_absence_reference_surfaces = []
    if generation_filter.include_proposal("axis_membership_array_filter_length_absence_identity"):
        for right_surface in array_filter_length_absence_right_surfaces:
            for left_surface in array_filter_length_absence_reference_surfaces:
                if left_surface.key == right_surface.key:
                    continue
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_array_filter_length_absence_identity",
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
                        "axis_membership_array_filter_length_absence_identity",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    for proposal_id in (
        "axis_membership_array_filter_length_absence_wrong_element_boundary",
        "axis_membership_array_filter_length_absence_wrong_collection_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in array_filter_length_absence_right_surfaces:
            for left_surface in array_filter_length_absence_reference_surfaces:
                if left_surface.key == right_surface.key:
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
                        "literal-membership-boundary",
                    )
                )
    module_reference_surfaces = [surface_by_key["python"], surface_by_key["ruby"]]
    if cross_mode == "ring":
        module_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        module_reference_surfaces = []
    module_right_surfaces_by_proposal = {
        "axis_membership_module_js_set_identity": [surface_by_key["javascript"]],
        "axis_membership_module_ts_set_identity": [surface_by_key["typescript"]],
        "axis_membership_module_java_list_identity": [surface_by_key["java"]],
        "axis_membership_module_python_tuple_identity": [surface_by_key["python"]],
        "axis_membership_module_python_set_identity": [surface_by_key["python"]],
    }
    for proposal_id, module_right_surfaces in module_right_surfaces_by_proposal.items():
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in module_reference_surfaces:
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
                        "literal_collection_membership-semantic-mutation",
                    )
                )
    module_right_surfaces = [
        surface_by_key["javascript"],
        surface_by_key["typescript"],
        surface_by_key["java"],
    ]
    for proposal_id in (
        "axis_membership_module_wrong_element_boundary",
        "axis_membership_module_wrong_collection_boundary",
        "axis_membership_module_shadowed_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for right_surface in module_right_surfaces:
            for left_surface in module_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_membership_module_mutated_boundary"):
        for right_surface in (surface_by_key["javascript"], surface_by_key["typescript"]):
            for left_surface in module_reference_surfaces:
                items.append(
                    make_axis_cross_item(
                        out_dir,
                        capabilities,
                        "axis_membership_module_mutated_boundary",
                        left_surface,
                        right_surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
    if generation_filter.include_proposal("axis_membership_module_python_mutated_boundary"):
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_membership_module_python_mutated_boundary",
                    left_surface,
                    surface_by_key["python"],
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    go_slices_right = surface_by_key["go"]
    for proposal_id in (
        "axis_membership_go_slices_package_identity",
        "axis_membership_go_slices_alias_package_identity",
        "axis_membership_go_slices_const_package_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    go_slices_right,
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
                    go_slices_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_go_slices_wrong_element_boundary",
        "axis_membership_go_slices_wrong_collection_boundary",
        "axis_membership_go_slices_mutated_boundary",
        "axis_membership_go_slices_unimported_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    go_slices_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    rust_local_right = surface_by_key["rust"]
    for proposal_id in (
        "axis_membership_rust_local_array_identity",
        "axis_membership_rust_local_typed_array_identity",
        "axis_membership_rust_local_slice_ref_identity",
        "axis_membership_rust_std_hashset_identity",
        "axis_membership_rust_std_btreeset_identity",
        "axis_membership_rust_std_vecdeque_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_local_right,
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
                    rust_local_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_rust_local_wrong_element_boundary",
        "axis_membership_rust_local_wrong_collection_boundary",
        "axis_membership_rust_local_mutated_boundary",
        "axis_membership_rust_local_custom_receiver_boundary",
        "axis_membership_rust_std_wrong_element_boundary",
        "axis_membership_rust_std_wrong_collection_boundary",
        "axis_membership_rust_std_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    rust_local_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    ruby_set_right = surface_by_key["ruby"]
    for proposal_id in (
        "axis_membership_ruby_set_new_include_identity",
        "axis_membership_ruby_set_new_member_identity",
        "axis_membership_ruby_set_local_identity",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ruby_set_right,
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
                    ruby_set_right,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_ruby_set_wrong_element_boundary",
        "axis_membership_ruby_set_wrong_collection_boundary",
        "axis_membership_ruby_set_missing_require_boundary",
        "axis_membership_ruby_set_shadowed_boundary",
        "axis_membership_ruby_set_mutated_boundary",
    ):
        if not generation_filter.include_proposal(proposal_id):
            continue
        for left_surface in module_reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    left_surface,
                    ruby_set_right,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    return items


def generate_java_factory_membership_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if cross_mode == "none" or not generation_filter.include_axis("literal_collection_membership"):
        return []
    surface_by_key = {surface.key: surface for surface in SURFACES}
    java_surface = surface_by_key["java"]
    reference_surfaces = [
        s
        for s in SURFACES
        if s.key != "java" and literal_membership_axis_supported(s, "axis_membership_literal_identity")
    ]
    if cross_mode == "ring":
        reference_surfaces = reference_surfaces[:1]
    items: list[dict] = []
    for proposal_id in (
        "axis_membership_java_list_of_identity",
        "axis_membership_java_set_of_identity",
        "axis_membership_java_arrays_aslist_identity",
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
                    java_surface,
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
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "literal_collection_membership-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_membership_java_list_of_wrong_element_boundary",
        "axis_membership_java_set_of_wrong_element_boundary",
        "axis_membership_java_arrays_aslist_wrong_element_boundary",
        "axis_membership_java_list_of_wrong_collection_boundary",
        "axis_membership_java_set_of_wrong_collection_boundary",
        "axis_membership_java_arrays_aslist_wrong_collection_boundary",
        "axis_membership_java_list_of_shadowed_boundary",
        "axis_membership_java_set_of_shadowed_boundary",
        "axis_membership_java_arrays_aslist_shadowed_boundary",
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
                    java_surface,
                    "not_equivalent",
                    "heldout",
                    "literal-membership-boundary",
                )
            )
    return items
