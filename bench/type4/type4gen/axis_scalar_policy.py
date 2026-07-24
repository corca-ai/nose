"""Scalar, null, and call-identity axis policies."""

from __future__ import annotations

from type4gen.axis_policy import (
    AxisPolicy,
    boundary_case_plan,
    default_case_plans,
)
from type4gen.axis_scalar import (
    axis_c_u16_be_byte_pack_variant,
    axis_c_u32_be_byte_pack_variant,
    axis_callee_identity_variant,
    axis_hof_filter_map_variant,
    axis_immutable_binding_variant,
    axis_java_dead_loop_variant,
    axis_java_low_bit_toggle_variant,
    axis_null_presence_variant,
    axis_nullish_variant,
    axis_numeric_clamp_variant,
    axis_scalar_abs_variant,
    axis_scalar_minmax_variant,
    axis_table_access_variant,
    axis_total_order_compare_variant,
    c_u16_be_byte_pack_axis_supported,
    c_u32_be_byte_pack_axis_supported,
    java_dead_loop_axis_supported,
    java_low_bit_toggle_axis_supported,
    null_presence_axis_supported,
    nullish_axis_supported,
    numeric_clamp_axis_supported,
    scalar_abs_axis_supported,
    total_order_compare_axis_supported,
)
from type4gen.model import Surface, Variant


def _variants(factory, surface: Surface, proposal_id: str, negative: bool):
    return (
        factory(surface, proposal_id, False, False),
        factory(surface, proposal_id, negative, True),
    )


def nullish_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_nullish_variant, surface, proposal_id, negative)


def null_presence_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_null_presence_variant, surface, proposal_id, negative)


def numeric_minmax_variants(surface: Surface, proposal_id: str, negative: bool):
    factory = (
        axis_scalar_minmax_variant
        if proposal_id.startswith(
            (
                "axis_scalar_min_",
                "axis_scalar_max_",
                "axis_scalar_rust_min_",
                "axis_scalar_rust_max_",
            )
        )
        else axis_scalar_abs_variant
    )
    return _variants(factory, surface, proposal_id, negative)


def numeric_clamp_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_numeric_clamp_variant, surface, proposal_id, negative)


def hof_filter_map_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_hof_filter_map_variant, surface, proposal_id, negative)


def total_order_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_total_order_compare_variant, surface, proposal_id, negative)


def java_dead_loop_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_java_dead_loop_variant, surface, proposal_id, negative)


def java_low_bit_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_java_low_bit_toggle_variant, surface, proposal_id, negative)


def c_u16_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_c_u16_be_byte_pack_variant, surface, proposal_id, negative)


def c_u32_variants(surface: Surface, proposal_id: str, negative: bool):
    return _variants(axis_c_u32_be_byte_pack_variant, surface, proposal_id, negative)


def immutable_variants(surface: Surface, _proposal_id: str, negative: bool):
    return (
        axis_immutable_binding_variant(surface, False, False),
        axis_immutable_binding_variant(surface, negative, True),
    )


def callee_variants(surface: Surface, _proposal_id: str, negative: bool):
    return (
        axis_callee_identity_variant(surface, False, False),
        axis_callee_identity_variant(surface, negative, True),
    )


def table_variants(surface: Surface, _proposal_id: str, negative: bool):
    return (
        axis_table_access_variant(surface, False, False),
        axis_table_access_variant(surface, negative, True),
    )


def nullish_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not nullish_axis_supported(surface, proposal_id):
        return ()
    if proposal_id == "axis_nullish_truthy_boundary":
        return boundary_case_plan("truthy-default-boundary")
    if proposal_id in {
        "axis_option_wrong_default_boundary",
        "axis_option_wrong_value_boundary",
    }:
        return boundary_case_plan("option-default-boundary")
    return default_case_plans(surface, proposal_id, capability)


def null_presence_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not null_presence_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_null_presence_nonnull_boundary",
        "axis_null_presence_wrong_value_boundary",
        "axis_null_presence_iflet_none_boundary",
        "axis_null_presence_iflet_wrong_value_boundary",
    }:
        return boundary_case_plan("null-presence-boundary")
    return default_case_plans(surface, proposal_id, capability)


def numeric_minmax_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not scalar_abs_axis_supported(surface, proposal_id):
        return ()
    if proposal_id.startswith("axis_scalar_rust_"):
        return ()
    if proposal_id in {
        "axis_scalar_abs_sign_boundary",
        "axis_scalar_abs_wrong_value_boundary",
        "axis_scalar_abs_shadowed_math_boundary",
        "axis_scalar_min_wrong_value_boundary",
        "axis_scalar_max_wrong_value_boundary",
        "axis_scalar_min_shadowed_math_boundary",
        "axis_scalar_max_shadowed_math_boundary",
    }:
        return boundary_case_plan("numeric-abs-boundary")
    return default_case_plans(surface, proposal_id, capability)


def numeric_clamp_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not numeric_clamp_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_numeric_clamp_unproven_boundary",
        "axis_numeric_clamp_swapped_bounds_boundary",
        "axis_numeric_clamp_float_boundary",
    }:
        return boundary_case_plan("numeric-clamp-boundary")
    return default_case_plans(surface, proposal_id, capability)


def hof_filter_map_case_plans(
    _surface: Surface,
    _proposal_id: str,
    _capability: str,
):
    return ()


def total_order_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not total_order_compare_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_total_order_compare_descending_boundary",
        "axis_total_order_compare_equal_boundary",
        "axis_total_order_compare_wrong_value_boundary",
    }:
        return boundary_case_plan("total-order-compare-boundary")
    return default_case_plans(surface, proposal_id, capability)


def java_dead_loop_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not java_dead_loop_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_java_dead_loop_false_init_boundary",
        "axis_java_dead_loop_positive_guard_boundary",
        "axis_java_dead_loop_reassigned_guard_boundary",
    }:
        return boundary_case_plan("java-dead-loop-boundary")
    return default_case_plans(surface, proposal_id, capability)


def java_low_bit_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not java_low_bit_toggle_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_java_low_bit_toggle_reversed_branch_boundary",
        "axis_java_low_bit_toggle_xor_two_boundary",
        "axis_java_low_bit_toggle_positive_one_boundary",
        "axis_java_low_bit_toggle_wrong_delta_boundary",
    }:
        return boundary_case_plan("java-low-bit-toggle-boundary")
    return default_case_plans(surface, proposal_id, capability)


def c_u16_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not c_u16_be_byte_pack_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_c_u16_be_byte_pack_wrong_order_boundary",
        "axis_c_u16_be_byte_pack_overlap_boundary",
        "axis_c_u16_be_byte_pack_wrong_byte_boundary",
        "axis_c_u16_be_byte_pack_unproven_alias_boundary",
    }:
        return boundary_case_plan("c-u16-byte-pack-boundary")
    return default_case_plans(surface, proposal_id, capability)


def c_u32_case_plans(surface: Surface, proposal_id: str, capability: str):
    if not c_u32_be_byte_pack_axis_supported(surface, proposal_id):
        return ()
    if proposal_id in {
        "axis_c_u32_be_byte_pack_uncasted_high_boundary",
        "axis_c_u32_be_byte_pack_wrong_order_boundary",
        "axis_c_u32_be_byte_pack_wrong_byte_boundary",
        "axis_c_u32_be_byte_pack_wrong_alias_boundary",
    }:
        return boundary_case_plan("c-u32-byte-pack-boundary")
    return default_case_plans(surface, proposal_id, capability)


def table_case_plans(surface: Surface, proposal_id: str, capability: str):
    if capability != "supported":
        return ()
    return default_case_plans(surface, proposal_id, capability)


AXIS_POLICIES = {
    "immutable_binding": AxisPolicy(
        immutable_variants,
        "scalar<int>",
        default_case_plans,
    ),
    "proven_callee_identity": AxisPolicy(
        callee_variants,
        "scalar<int>",
        default_case_plans,
    ),
    "nullish_default": AxisPolicy(
        nullish_variants,
        "nullable<int>+fallback",
        nullish_case_plans,
    ),
    "null_presence_predicate": AxisPolicy(
        null_presence_variants,
        "nullable<T>+alternate",
        null_presence_case_plans,
    ),
    "numeric_minmax_abs": AxisPolicy(
        numeric_minmax_variants,
        "scalar<int>+alternate",
        numeric_minmax_case_plans,
    ),
    "numeric_clamp": AxisPolicy(
        numeric_clamp_variants,
        "scalar<int>+bounds",
        numeric_clamp_case_plans,
    ),
    "hof_filter_map": AxisPolicy(
        hof_filter_map_variants,
        "list<int>+optional-emission",
        hof_filter_map_case_plans,
    ),
    "total_order_compare": AxisPolicy(
        total_order_variants,
        "ordered-scalar-pair",
        total_order_case_plans,
    ),
    "java_statically_false_loop": AxisPolicy(
        java_dead_loop_variants,
        "java-array-iteration",
        java_dead_loop_case_plans,
    ),
    "java_integer_low_bit_toggle": AxisPolicy(
        java_low_bit_variants,
        "java-int-edge-key",
        java_low_bit_case_plans,
    ),
    "c_u16_be_byte_pack": AxisPolicy(
        c_u16_variants,
        "c-byte-buffer",
        c_u16_case_plans,
    ),
    "c_u32_be_byte_pack": AxisPolicy(
        c_u32_variants,
        "c-byte-buffer",
        c_u32_case_plans,
    ),
    "table_access": AxisPolicy(
        table_variants,
        "map<string,int>",
        table_case_plans,
    ),
}
