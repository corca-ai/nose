"""Literal and proven-collection membership axis policy."""

from __future__ import annotations

from type4gen.axis_membership import (
    axis_membership_literal_variant,
    literal_membership_axis_supported,
)
from type4gen.axis_policy import (
    AxisPolicy,
    boundary_case_plan,
    default_case_plans,
)
from type4gen.model import Surface, Variant


CROSS_ONLY_PREFIXES = (
    "axis_membership_python_alias_",
    "axis_membership_python_deque_",
    "axis_membership_ruby_set_",
    "axis_membership_set_",
    "axis_membership_array_some_",
    "axis_membership_array_every_",
    "axis_membership_array_indexof_",
    "axis_membership_array_findindex_",
    "axis_membership_array_filter_length_",
    "axis_membership_java_",
    "axis_membership_module_",
    "axis_membership_local_",
    "axis_membership_go_slices_",
    "axis_membership_rust_local_",
    "axis_membership_rust_std_",
)

SAME_SURFACE_BOUNDARIES = {
    "axis_membership_wrong_element_boundary",
    "axis_membership_wrong_collection_boundary",
    "axis_membership_substring_boundary",
    "axis_membership_unproven_receiver_boundary",
    "axis_membership_typed_wrong_element_boundary",
    "axis_membership_typed_string_boundary",
    "axis_membership_python_factory_wrong_element_boundary",
    "axis_membership_python_factory_wrong_collection_boundary",
    "axis_membership_python_factory_shadowed_boundary",
    "axis_membership_local_wrong_element_boundary",
    "axis_membership_local_wrong_collection_boundary",
    "axis_membership_local_mutated_boundary",
    "axis_membership_array_some_wrong_element_boundary",
    "axis_membership_array_some_wrong_collection_boundary",
    "axis_membership_array_every_wrong_element_boundary",
    "axis_membership_array_every_wrong_collection_boundary",
    "axis_membership_array_indexof_wrong_element_boundary",
    "axis_membership_array_indexof_wrong_collection_boundary",
    "axis_membership_array_findindex_wrong_element_boundary",
    "axis_membership_array_findindex_wrong_collection_boundary",
    "axis_membership_array_filter_length_wrong_element_boundary",
    "axis_membership_array_filter_length_wrong_collection_boundary",
    "axis_membership_array_filter_length_absence_wrong_element_boundary",
    "axis_membership_array_filter_length_absence_wrong_collection_boundary",
}


def membership_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return (
        axis_membership_literal_variant(surface, proposal_id, False, False),
        axis_membership_literal_variant(surface, proposal_id, negative, True),
    )


def membership_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not literal_membership_axis_supported(surface, proposal_id):
        return ()
    if proposal_id.startswith(CROSS_ONLY_PREFIXES):
        return ()
    if proposal_id in SAME_SURFACE_BOUNDARIES:
        return boundary_case_plan("literal-membership-boundary")
    return default_case_plans(surface, proposal_id, capability)


AXIS_POLICIES = {
    "literal_collection_membership": AxisPolicy(
        membership_variants,
        "set<string>",
        membership_case_plans,
    ),
}
