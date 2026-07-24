"""Evidence records for generated semantic-axis cases."""

from __future__ import annotations

from type4gen.model import PROPERTY_INPUTS


def axis_evidence(axis: str, status: str, negative: bool, proposal_id: str | None = None) -> dict:
    if status == "equivalent":
        if axis == "literal_collection_membership":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": "red", "other": "green"},
                    {"value": "blue", "other": "green"},
                    {"value": "green", "other": "red"},
                ],
                "outputs": [],
            }
        if axis == "map_key_membership":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {
                        "lookup": {"red": "apple", "blue": "berry"},
                        "other_lookup": {"green": "grape"},
                        "key": "red",
                        "other": "green",
                    },
                    {
                        "lookup": {"red": "apple", "blue": "berry"},
                        "other_lookup": {"green": "grape"},
                        "key": "green",
                        "other": "red",
                    },
                ],
                "outputs": [],
            }
        if axis == "literal_map_default_lookup":
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_bool_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": True, "blue": False},
                            "other_lookup": {"red": False, "blue": False},
                            "key": "red",
                            "other_key": "green",
                            "fallback": False,
                            "other_default": True,
                        },
                        {
                            "lookup": {"red": True, "blue": False},
                            "other_lookup": {"red": False, "blue": False},
                            "key": "green",
                            "other_key": "red",
                            "fallback": False,
                            "other_default": True,
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_float_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": 1.5, "blue": 2.5},
                            "other_lookup": {"red": 9.5, "blue": 2.5},
                            "key": "red",
                            "other_key": "green",
                            "fallback": 0.0,
                            "other_default": 9.0,
                        },
                        {
                            "lookup": {"red": 1.5, "blue": 2.5},
                            "other_lookup": {"red": 9.5, "blue": 2.5},
                            "key": "green",
                            "other_key": "red",
                            "fallback": 0.0,
                            "other_default": 9.0,
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": None, "blue": None},
                            "other_lookup": {"red": "apple", "blue": "berry"},
                            "key": "red",
                            "other_key": "green",
                            "fallback": None,
                            "other_default": "missing",
                        },
                        {
                            "lookup": {"red": None, "blue": None},
                            "other_lookup": {"red": "apple", "blue": "berry"},
                            "key": "green",
                            "other_key": "red",
                            "fallback": None,
                            "other_default": "missing",
                        },
                    ],
                    "outputs": [],
                }
            if proposal_id and proposal_id.startswith("axis_map_default_go_zero_"):
                return {
                    "level": "E1",
                    "kind": f"same-spec-{axis}",
                    "property_inputs": [
                        {
                            "lookup": {"red": "apple", "blue": "berry"},
                            "other_lookup": {"red": "apricot", "blue": "berry"},
                            "key": "red",
                            "other_key": "green",
                            "fallback": "",
                            "other_default": "missing",
                        },
                        {
                            "lookup": {"red": "apple", "blue": "berry"},
                            "other_lookup": {"red": "apricot", "blue": "berry"},
                            "key": "green",
                            "other_key": "red",
                            "fallback": "",
                            "other_default": "missing",
                        },
                    ],
                    "outputs": [],
                }
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"key": "red", "other": "green"},
                    {"key": "blue", "other": "green"},
                    {"key": "green", "other": "red"},
                ],
                "outputs": [],
            }
        if axis == "map_default_lookup":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {
                        "lookup": {"red": 1, "blue": 2},
                        "other_lookup": {"red": 9, "blue": 2},
                        "key": "red",
                        "other_key": "green",
                        "fallback": 0,
                        "other_default": 9,
                    },
                    {
                        "lookup": {"red": 1, "blue": 2},
                        "other_lookup": {"red": 9, "blue": 2},
                        "key": "green",
                        "other_key": "red",
                        "fallback": 0,
                        "other_default": 9,
                    },
                ],
                "outputs": [],
            }
        if axis == "null_presence_predicate":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": None, "other": 1},
                    {"value": 1, "other": None},
                    {"value": 0, "other": None},
                ],
                "outputs": [],
            }
        if axis == "nullish_default":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"value": 5, "fallback": 0, "other": 7, "other_default": 9},
                    {"value": None, "fallback": 0, "other": 7, "other_default": 9},
                ],
                "outputs": [],
            }
        if axis == "numeric_minmax_abs":
            property_inputs = (
                [
                    {"left": 2, "right": 5, "other": 1},
                    {"left": -4, "right": 3, "other": 2},
                    {"left": 7, "right": 7, "other": -3},
                ]
                if proposal_id
                and (
                    proposal_id.startswith(("axis_scalar_min_", "axis_scalar_max_"))
                    or proposal_id.startswith(
                        ("axis_scalar_rust_min_", "axis_scalar_rust_max_")
                    )
                )
                else [
                    {"value": -3, "other": 4},
                    {"value": 0, "other": -2},
                    {"value": 5, "other": -7},
                ]
            )
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": property_inputs,
                "outputs": [],
            }
        if axis == "numeric_clamp":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"x": -5, "lo": 0, "hi": 10},
                    {"x": 4, "lo": 0, "hi": 10},
                    {"x": 15, "lo": 0, "hi": 10},
                    {"x": 5, "lo": 10, "hi": 0},
                ],
                "claim": "The exiting invalid-bound guard proves lo <= hi on the return path.",
                "outputs": [],
            }
        if axis == "hof_filter_map":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": PROPERTY_INPUTS,
                "claim": "Rust filter_map emits Some(value), drops None, and matches explicit filter+map for the same predicate and emitted value.",
                "outputs": [],
            }
        if axis == "string_prefix_suffix":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": ["prelude", "case-suf", "other"],
                "outputs": [],
            }
        if axis == "python_docstring_noop":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"i": 1, "j": 1, "values": [1, 2, 3], "value": 2},
                    {"i": 1, "j": 2, "values": [1], "value": -3},
                ],
                "outputs": [],
            }
        if axis == "total_order_compare":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"left": -1, "right": 2},
                    {"left": 4, "right": 4},
                    {"left": 7, "right": 3},
                ],
                "claim": "Ascending three-way total-order comparator returns -1, 0, or 1 from the same ordered pair.",
                "outputs": [],
            }
        if axis == "java_statically_false_loop":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"numVertices": 0, "strideInBytes": 4},
                    {"numVertices": 1, "strideInBytes": 4},
                ],
                "claim": "`found=true` makes `!found && ...` false on loop entry, so the loop body and update are unreachable.",
                "outputs": [],
            }
        if axis == "java_integer_low_bit_toggle":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": [
                    {"edgeKey": -3},
                    {"edgeKey": 0},
                    {"edgeKey": 7},
                ],
                "claim": "For Java primitive integers, even values take `+1` and odd values take `-1`, exactly toggling bit 0.",
                "outputs": [],
            }
        return {
            "level": "E1",
            "kind": f"same-spec-{axis}",
            "property_inputs": [0, 1, 4],
            "outputs": [],
        }
    if status == "unknown":
        return {
            "level": "E0",
            "kind": f"unproven-{axis}-boundary",
            "property_inputs": [],
            "outputs": [],
        }
    if axis == "proven_callee_identity":
        left_output = 3
        right_output = 4
    elif axis == "string_prefix_suffix":
        value = "case-suf" if proposal_id == "axis_string_suffix_identity" else "prelude"
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": {
                "input": {"value": value, "other": "other"},
                "left": True,
                "right": False,
            },
        }
    elif axis == "literal_collection_membership":
        if proposal_id == "axis_membership_module_mutated_boundary":
            counterexample = {
                "input": {"value": "green", "other": "red"},
                "left": False,
                "right": True,
            }
        elif proposal_id in {
            "axis_membership_go_slices_mutated_boundary",
            "axis_membership_rust_local_mutated_boundary",
            "axis_membership_rust_std_mutated_boundary",
        }:
            counterexample = {
                "input": {"value": "green", "other": "red"},
                "left": False,
                "right": True,
            }
        elif proposal_id == "axis_membership_substring_boundary":
            counterexample = {
                "input": {"value": "predator", "other": "green"},
                "left": False,
                "right": True,
            }
        else:
            counterexample = {
                "input": {"value": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "python_docstring_noop":
        if proposal_id == "axis_python_docstring_fstring_boundary":
            counterexample = {
                "input": {"value": "red", "observer": "records calls"},
                "left": {"return": 1, "effects": []},
                "right": {"return": 1, "effects": ["observe(red)"]},
            }
        elif proposal_id == "axis_python_docstring_assigned_string_boundary":
            counterexample = {"input": {}, "left": "red", "right": "blue"}
        else:
            counterexample = {"input": {}, "left": "red", "right": "blue"}
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "map_key_membership":
        counterexample = {
            "input": {
                "lookup": {"red": "apple", "blue": "berry"},
                "other_lookup": {"green": "grape"},
                "key": "red",
                "other": "green",
            },
            "left": True,
            "right": False,
        }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "literal_map_default_lookup":
        if proposal_id in {
            "axis_map_default_literal_identity",
            "axis_map_default_js_map_inline_identity",
            "axis_map_default_js_map_local_identity",
            "axis_map_default_js_map_has_get_identity",
            "axis_map_default_js_object_hasown_identity",
            "axis_map_default_js_object_call_identity",
            "axis_map_default_js_object_negated_identity",
            "axis_map_default_wrong_default_boundary",
            "axis_map_default_js_map_wrong_default_boundary",
            "axis_map_default_js_object_wrong_default_boundary",
            "axis_map_default_java_map_of_identity",
            "axis_map_default_java_map_of_entries_identity",
            "axis_map_default_java_map_local_identity",
            "axis_map_default_java_map_wrong_default_boundary",
            "axis_map_default_rust_hashmap_from_identity",
            "axis_map_default_rust_btreemap_from_identity",
            "axis_map_default_rust_hashmap_local_identity",
            "axis_map_default_rust_wrong_default_boundary",
            "axis_map_default_module_js_map_identity",
            "axis_map_default_module_ts_map_identity",
            "axis_map_default_module_java_map_identity",
            "axis_map_default_module_wrong_default_boundary",
            "axis_map_default_ruby_fetch_block_int_identity",
        }:
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": 0,
                "right": 9,
            }
        elif proposal_id == "axis_map_default_ruby_fetch_block_string_identity":
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": "",
                "right": 9,
            }
        elif proposal_id == "axis_map_default_ruby_fetch_block_bool_identity":
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": False,
                "right": 9,
            }
        elif proposal_id in {
            "axis_map_default_go_map_inline_identity",
            "axis_map_default_go_map_local_identity",
            "axis_map_default_go_map_var_identity",
            "axis_map_default_go_map_wrong_key_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 0,
            }
        elif proposal_id in {
            "axis_map_default_go_zero_string_inline_identity",
            "axis_map_default_go_zero_string_local_identity",
            "axis_map_default_go_zero_wrong_key_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": "apple",
                "right": "",
            }
        elif proposal_id == "axis_map_default_go_zero_bool_inline_identity":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        elif proposal_id in {
            "axis_map_default_go_zero_float_inline_identity",
            "axis_map_default_go_zero_float_local_identity",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1.5,
                "right": 0.0,
            }
        elif proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": None,
                "right": "apple",
            }
        elif proposal_id in {
            "axis_map_default_wrong_map_boundary",
            "axis_map_default_js_map_wrong_map_boundary",
            "axis_map_default_js_object_wrong_map_boundary",
            "axis_map_default_java_map_wrong_map_boundary",
            "axis_map_default_rust_wrong_map_boundary",
            "axis_map_default_go_map_wrong_map_boundary",
            "axis_map_default_rust_mutated_boundary",
            "axis_map_default_module_wrong_map_boundary",
            "axis_map_default_module_mutated_boundary",
            "axis_map_default_module_shadowed_boundary",
        }:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 9,
            }
        elif proposal_id == "axis_map_default_go_zero_wrong_map_boundary":
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": True,
                "right": False,
            }
        elif proposal_id == "axis_map_default_go_zero_mixed_value_boundary":
            counterexample = {
                "input": {"key": "blue", "other": "green"},
                "left": "berry",
                "right": False,
            }
        elif proposal_id in {
            "axis_map_default_js_object_unguarded_boundary",
            "axis_map_default_js_object_in_boundary",
        }:
            counterexample = {
                "input": {"key": "toString", "other": "green"},
                "left": 0,
                "right": "prototype property value",
            }
        elif proposal_id == "axis_map_default_js_object_method_boundary":
            counterexample = {
                "input": {
                    "key": "red",
                    "other": "green",
                    "environment": "Object.prototype.hasOwnProperty patched to return false",
                },
                "left": 1,
                "right": 0,
            }
        elif proposal_id == "axis_map_default_js_object_shadowed_boundary":
            counterexample = {
                "input": {
                    "key": "red",
                    "other": "green",
                    "Object": {"hasOwn": "returns false"},
                },
                "left": 1,
                "right": 0,
            }
        else:
            counterexample = {
                "input": {"key": "red", "other": "green"},
                "left": 1,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "map_default_lookup":
        input_values = {
            "lookup": {"red": 1, "blue": 2},
            "other_lookup": {"red": 9, "blue": 2},
            "key": "red",
            "other_key": "green",
            "fallback": 0,
            "other_default": 9,
        }
        if proposal_id in {
            "axis_map_fallback_wrong_default_boundary",
            "axis_map_fallback_ts_wrong_default_boundary",
            "axis_map_fallback_python_wrong_default_boundary",
        }:
            input_values["key"] = "green"
            input_values["other_key"] = "red"
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        elif proposal_id in {
            "axis_map_fallback_wrong_map_boundary",
            "axis_map_fallback_ts_wrong_map_boundary",
            "axis_map_fallback_python_wrong_map_boundary",
        }:
            counterexample = {
                "input": input_values,
                "left": 1,
                "right": 9,
            }
        else:
            counterexample = {
                "input": input_values,
                "left": 1,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "null_presence_predicate":
        if proposal_id in {
            "axis_null_presence_wrong_value_boundary",
            "axis_null_presence_iflet_wrong_value_boundary",
        }:
            counterexample = {
                "input": {"value": None, "other": 1},
                "left": True,
                "right": False,
            }
        else:
            counterexample = {
                "input": {"value": None, "other": 1},
                "left": True,
                "right": False,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "nullish_default":
        input_values = {"value": None, "fallback": 0, "other": 7, "other_default": 9}
        if proposal_id == "axis_option_wrong_value_boundary":
            input_values["value"] = 5
            counterexample = {
                "input": input_values,
                "left": 5,
                "right": 7,
            }
        elif proposal_id == "axis_nullish_truthy_boundary":
            input_values["value"] = 0
            input_values["fallback"] = 9
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        else:
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "numeric_minmax_abs":
        if proposal_id in {
            "axis_scalar_min_wrong_value_boundary",
            "axis_scalar_max_wrong_value_boundary",
            "axis_scalar_rust_min_wrong_value_boundary",
            "axis_scalar_rust_max_wrong_value_boundary",
        }:
            is_min = proposal_id in {
                "axis_scalar_min_wrong_value_boundary",
                "axis_scalar_rust_min_wrong_value_boundary",
            }
            counterexample = {
                "input": {"left": 2, "right": 5, "other": -1},
                "left": (2 if is_min else 5) - 1,
                "right": (-1 if is_min else 2) - 1,
            }
        elif proposal_id in {
            "axis_scalar_min_shadowed_math_boundary",
            "axis_scalar_max_shadowed_math_boundary",
        }:
            is_min = proposal_id == "axis_scalar_min_shadowed_math_boundary"
            counterexample = {
                "input": {"left": 2, "right": 5, "other": 1},
                "left": (2 if is_min else 5) + 1,
                "right": 1,
            }
        elif proposal_id in {
            "axis_scalar_min_function_identity",
            "axis_scalar_max_function_identity",
            "axis_scalar_rust_min_method_identity",
            "axis_scalar_rust_max_method_identity",
        }:
            is_min = proposal_id in {
                "axis_scalar_min_function_identity",
                "axis_scalar_rust_min_method_identity",
            }
            counterexample = {
                "input": {"left": 2, "right": 5, "other": 1},
                "left": (2 if is_min else 5) + 1,
                "right": (5 if is_min else 2) + 1,
            }
        elif proposal_id in {
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_rust_abs_wrong_value_boundary",
        }:
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 8,
            }
        elif proposal_id in {
            "axis_scalar_rust_abs_custom_method_boundary",
            "axis_scalar_rust_min_custom_method_boundary",
            "axis_scalar_rust_max_custom_method_boundary",
        }:
            counterexample = {
                "input": {"method": "custom receiver method returns 0"},
                "left": "numeric intrinsic result",
                "right": 0,
            }
        elif proposal_id == "axis_scalar_abs_shadowed_math_boundary":
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 4,
            }
        else:
            counterexample = {
                "input": {"value": -3, "other": 4},
                "left": 7,
                "right": 1,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "numeric_clamp":
        if proposal_id == "axis_numeric_clamp_unproven_boundary":
            counterexample = {
                "input": {"x": 5, "lo": 10, "hi": 0},
                "left": 0,
                "right": 10,
            }
        elif proposal_id == "axis_numeric_clamp_float_boundary":
            counterexample = {
                "input": {"x": "NaN", "lo": 0.0, "hi": 10.0},
                "left": "NaN-sensitive min/max result",
                "right": "requires separate float-domain proof",
            }
        else:
            counterexample = {
                "input": {"x": 5, "lo": 0, "hi": 10},
                "left": 5,
                "right": 0,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "hof_filter_map":
        if proposal_id == "axis_hof_filter_map_none_boundary":
            counterexample = {
                "input": {"xs": [-1, 2]},
                "left": [4],
                "right": [None, 4],
            }
        elif proposal_id == "axis_hof_filter_map_value_boundary":
            counterexample = {
                "input": {"xs": [2]},
                "left": [4],
                "right": [6],
            }
        else:
            counterexample = {
                "input": {"xs": [2]},
                "left": [0],
                "right": [],
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "total_order_compare":
        if proposal_id == "axis_total_order_compare_equal_boundary":
            counterexample = {
                "input": {"left": 4, "right": 4},
                "left": 0,
                "right": -1,
            }
        elif proposal_id == "axis_total_order_compare_wrong_value_boundary":
            counterexample = {
                "input": {"left": 7, "right": 3},
                "left": 1,
                "right": 2,
            }
        else:
            counterexample = {
                "input": {"left": -1, "right": 2},
                "left": -1,
                "right": 1,
            }
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    elif axis == "java_statically_false_loop":
        if proposal_id == "axis_java_dead_loop_false_init_boundary":
            right = "body can execute because found starts false"
        elif proposal_id == "axis_java_dead_loop_positive_guard_boundary":
            right = "body can execute because found starts true and the guard is positive"
        elif proposal_id == "axis_java_dead_loop_guard_identity":
            right = "wrong reachable return value"
        else:
            right = "body can execute after the guard variable is reassigned"
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": {
                "input": {"numVertices": 1, "strideInBytes": 4},
                "left": "first index is returned before comparing elements",
                "right": right,
            },
        }
    elif axis == "java_integer_low_bit_toggle":
        if proposal_id == "axis_java_low_bit_toggle_positive_one_boundary":
            counterexample = {"input": {"edgeKey": -1}, "left": -2, "right": 0}
        elif proposal_id in {
            "axis_java_low_bit_toggle_xor_two_boundary",
            "axis_java_low_bit_toggle_even_identity",
        }:
            counterexample = {"input": {"edgeKey": 0}, "left": 1, "right": 2}
        elif proposal_id == "axis_java_low_bit_toggle_wrong_delta_boundary":
            counterexample = {"input": {"edgeKey": 3}, "left": 2, "right": 1}
        else:
            counterexample = {"input": {"edgeKey": 0}, "left": 1, "right": -1}
        return {
            "level": "E2",
            "kind": f"counterexample-{axis}",
            "counterexample": counterexample,
        }
    else:
        left_output = 8
        right_output = 9
    return {
        "level": "E2",
        "kind": f"counterexample-{axis}",
        "counterexample": {"input": 1, "left": left_output, "right": right_output},
    }
