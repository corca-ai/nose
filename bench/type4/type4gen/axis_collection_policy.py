"""Collection, record, and string axis policies."""

from __future__ import annotations

from type4gen.axis_collections import (
    axis_collection_empty_variant,
    axis_own_property_variant,
    axis_record_guard_variant,
    axis_string_prefix_variant,
    collection_empty_axis_supported,
    own_property_axis_supported,
    record_guard_axis_supported,
    string_prefix_axis_supported,
)
from type4gen.axis_policy import (
    AxisPolicy,
    boundary_case_plan,
    default_case_plans,
)
from type4gen.model import Surface, Variant


def _variants(factory, surface: Surface, proposal_id: str, negative: bool):
    return (
        factory(surface, proposal_id, False, False),
        factory(surface, proposal_id, negative, True),
    )


def own_property_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_own_property_variant, surface, proposal_id, negative)


def record_guard_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_record_guard_variant, surface, proposal_id, negative)


def collection_empty_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_collection_empty_variant, surface, proposal_id, negative)


def string_prefix_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_string_prefix_variant, surface, proposal_id, negative)


def own_property_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not own_property_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_own_property_in_boundary",
        "axis_own_property_method_boundary",
        "axis_own_property_shadow_boundary",
    }:
        return boundary_case_plan("unproven-own-property-guard")
    return default_case_plans(surface, proposal_id, capability)


def record_guard_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not record_guard_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_record_guard_array_boundary",
        "axis_record_guard_null_boundary",
    }:
        return boundary_case_plan("incomplete-record-guard")
    return default_case_plans(surface, proposal_id, capability)


def collection_empty_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not collection_empty_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_collection_threshold_boundary",
        "axis_collection_wrong_receiver_boundary",
        "axis_collection_typed_domain_array_boundary",
        "axis_collection_typed_domain_string_boundary",
    }:
        tag = (
            "typed-empty-domain-boundary"
            if proposal_id.startswith("axis_collection_typed_domain_")
            else "collection-empty-boundary"
        )
        return boundary_case_plan(tag)
    return default_case_plans(surface, proposal_id, capability)


def string_prefix_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not string_prefix_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_string_affix_boundary",
        "axis_string_direction_boundary",
        "axis_string_wrong_receiver_boundary",
    }:
        return boundary_case_plan("string-prefix-suffix-boundary")
    return default_case_plans(surface, proposal_id, capability)


AXIS_POLICIES = {
    "own_property_guard": AxisPolicy(
        own_property_variants,
        "scalar<int>",
        own_property_case_plans,
    ),
    "record_shape_guard": AxisPolicy(
        record_guard_variants,
        "scalar<int>",
        record_guard_case_plans,
    ),
    "collection_empty_check": AxisPolicy(
        collection_empty_variants,
        "list<int>",
        collection_empty_case_plans,
    ),
    "string_prefix_suffix": AxisPolicy(
        string_prefix_variants,
        "string",
        string_prefix_case_plans,
    ),
}
