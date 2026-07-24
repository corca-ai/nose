"""Map membership and default-lookup axis policies."""

from __future__ import annotations

from type4gen.axis_maps import (
    axis_map_default_lookup_variant,
    axis_map_default_variant,
    axis_map_key_membership_variant,
    literal_map_default_axis_supported,
    map_default_lookup_axis_supported,
    map_key_membership_axis_supported,
)
from type4gen.axis_policy import (
    AxisPolicy,
    boundary_case_plan,
    default_case_plans,
)
from type4gen.model import Surface, Variant


LITERAL_DEFAULT_CROSS_ONLY_PREFIXES = (
    "axis_map_default_js_map_",
    "axis_map_default_js_object_",
    "axis_map_default_java_map_",
    "axis_map_default_rust_",
    "axis_map_default_go_map_",
    "axis_map_default_go_zero_",
    "axis_map_default_module_",
)


def _variants(factory, surface: Surface, proposal_id: str, negative: bool):
    return (
        factory(surface, proposal_id, False, False),
        factory(surface, proposal_id, negative, True),
    )


def map_key_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_map_key_membership_variant, surface, proposal_id, negative)


def literal_map_default_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_map_default_variant, surface, proposal_id, negative)


def map_default_variants(
    surface: Surface,
    proposal_id: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    return _variants(axis_map_default_lookup_variant, surface, proposal_id, negative)


def map_key_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not map_key_membership_axis_supported(surface, proposal_id):
        return ()
    if proposal_id.startswith(
        ("axis_map_key_python_keys_", "axis_map_key_ts_array_from_keys_")
    ):
        return ()
    if proposal_id in {
        "axis_map_key_wrong_key_boundary",
        "axis_map_key_wrong_map_boundary",
        "axis_map_key_value_boundary",
    }:
        return boundary_case_plan("map-key-membership-boundary")
    return default_case_plans(surface, proposal_id, capability)


def literal_map_default_case_plans(
    surface: Surface,
    proposal_id: str,
    capability: str,
):
    if proposal_id.startswith(LITERAL_DEFAULT_CROSS_ONLY_PREFIXES):
        return ()
    if not literal_map_default_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_map_default_wrong_key_boundary",
        "axis_map_default_wrong_default_boundary",
        "axis_map_default_wrong_map_boundary",
    }:
        return boundary_case_plan("literal-map-default-boundary")
    return default_case_plans(surface, proposal_id, capability)


def map_default_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not map_default_lookup_axis_supported(surface, proposal_id):
        return ()
    if proposal_id.startswith(
        (
            "axis_map_fallback_ts_",
            "axis_map_fallback_python_",
            "axis_map_fallback_java_",
        )
    ):
        return ()
    if proposal_id in {
        "axis_map_fallback_wrong_key_boundary",
        "axis_map_fallback_wrong_default_boundary",
        "axis_map_fallback_wrong_map_boundary",
    }:
        return boundary_case_plan("map-default-boundary")
    return default_case_plans(surface, proposal_id, capability)


AXIS_POLICIES = {
    "map_key_membership": AxisPolicy(
        map_key_variants,
        "map<string,string>+key",
        map_key_case_plans,
    ),
    "literal_map_default_lookup": AxisPolicy(
        literal_map_default_variants,
        "map<string,int>+key",
        literal_map_default_case_plans,
    ),
    "map_default_lookup": AxisPolicy(
        map_default_variants,
        "map<string,int>+key+fallback",
        map_default_case_plans,
    ),
}
