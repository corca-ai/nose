"""Map membership and default-lookup axis source templates."""

from __future__ import annotations

import json

from .model import Surface, Variant, js_axis_source


def map_key_membership_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_map_key_"):
        return False
    if proposal_id.startswith("axis_map_key_python_keys_"):
        return surface.key == "python"
    if proposal_id.startswith("axis_map_key_ts_array_from_keys_"):
        return surface.key == "typescript"
    return surface.key in {"python", "go", "java", "rust", "ruby", "typescript"}


def map_key_axis_parts(proposal_id: str, negative: bool, right: bool) -> tuple[str, str, str]:
    key = "key"
    receiver = "lookup"
    form = "key"
    if right and proposal_id == "axis_map_key_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_key_wrong_map_boundary":
        receiver = "other_lookup"
    if right and proposal_id == "axis_map_key_value_boundary":
        form = "value"
    if right and proposal_id in {
        "axis_map_key_python_keys_in_identity",
        "axis_map_key_python_keys_wrong_key_boundary",
        "axis_map_key_python_keys_wrong_map_boundary",
    }:
        form = "python_keys_in"
    if right and proposal_id == "axis_map_key_python_keys_contains_identity":
        form = "python_keys_contains"
    if right and proposal_id == "axis_map_key_python_keys_value_boundary":
        form = "python_keys_value"
    if right and proposal_id in {
        "axis_map_key_ts_array_from_keys_identity",
        "axis_map_key_ts_array_from_keys_wrong_key_boundary",
        "axis_map_key_ts_array_from_keys_wrong_map_boundary",
    }:
        form = "ts_array_from_keys"
    if right and proposal_id == "axis_map_key_ts_array_from_keys_value_boundary":
        form = "ts_array_from_values"
    if right and proposal_id in {
        "axis_map_key_python_keys_wrong_key_boundary",
        "axis_map_key_ts_array_from_keys_wrong_key_boundary",
    }:
        key = "other"
    if right and proposal_id in {
        "axis_map_key_python_keys_wrong_map_boundary",
        "axis_map_key_ts_array_from_keys_wrong_map_boundary",
    }:
        receiver = "other_lookup"
    if right and negative and proposal_id in {
        "axis_map_key_membership_identity",
        "axis_map_key_python_keys_in_identity",
        "axis_map_key_python_keys_contains_identity",
        "axis_map_key_ts_array_from_keys_identity",
    }:
        key = "other"
    return receiver, key, form


def axis_map_key_membership_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    receiver, key, form = map_key_axis_parts(proposal_id, negative, right)
    name = {
        "go": "BuildCase" if right else "AxisCase",
        "java": "buildCase" if right else "axisCase",
        "typescript": "buildCase" if right else "axisCase",
    }.get(surface.language, "build_case" if right else "axis_case")

    if surface.key == "python":
        if form == "python_keys_in":
            expr = f"{key} in {receiver}.keys()"
        elif form == "python_keys_contains":
            expr = f"{receiver}.keys().__contains__({key})"
        elif form == "python_keys_value":
            expr = f"{key} in {receiver}.values()"
        else:
            expr = (
                f"{key} in {receiver}.values()"
                if form == "value"
                else (f"{receiver}.__contains__({key})" if right else f"{key} in {receiver}")
            )
        typed = ": dict[str, str]" if form.startswith("python_keys_") else ""
        src = f"""def {name}(lookup{typed}, other_lookup{typed}, key: str, other: str):
    return {expr}
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        if form == "value":
            body = f"""for _, value := range {receiver} {{
        if value == {key} {{
            return true
        }}
    }}
    return false"""
        else:
            body = f"""_, ok := {receiver}[{key}]
    return ok"""
        src = f"""package p

func {name}(lookup map[string]string, otherLookup map[string]string, key string, other string) bool {{
    other_lookup := otherLookup
    {body}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        expr = (
            f"{receiver}.containsValue({key})"
            if form == "value"
            else (f"{receiver}.keySet().contains({key})" if right else f"{receiver}.containsKey({key})")
        )
        src = f"""import java.util.Map;

class AxisCase {{
    static boolean {name}(Map<String, String> lookup, Map<String, String> other_lookup, String key, String other) {{
        return {expr};
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        expr = (
            f"{receiver}.values().any(|value| value == {key})"
            if form == "value"
            else (
                f"{receiver}.get({key}).is_some()"
                if right
                else f"{receiver}.contains_key({key})"
            )
        )
        src = f"""use std::collections::HashMap;

pub fn {name}(lookup: &HashMap<String, String>, other_lookup: &HashMap<String, String>, key: &str, other: &str) -> bool {{
    {expr}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        expr = (
            f"{receiver}.value?({key})"
            if form == "value"
            else (f"{receiver}.has_key?({key})" if right else f"{receiver}.key?({key})")
        )
        src = f"""def {name}(lookup, other_lookup, key, other)
  {expr}
end
"""
        return Variant("axis", src, name)

    if surface.key == "typescript":
        if form == "value":
            expr = f"Array.from({receiver}.values()).includes({key})"
        elif form == "ts_array_from_keys":
            expr = f"Array.from({receiver}.keys()).includes({key})"
        elif form == "ts_array_from_values":
            expr = f"Array.from({receiver}.values()).includes({key})"
        else:
            expr = f"{receiver}.has({key})"
        src = f"""function {name}(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {{
  return {expr};
}}
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for map-key membership axis: {surface.key}")


def literal_map_default_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_map_default_"):
        return False
    if proposal_id.startswith("axis_map_default_ruby_fetch_block_"):
        return surface.key == "ruby"
    if proposal_id.startswith(("axis_map_default_js_map_", "axis_map_default_js_object_")):
        return surface.key in {"python", "ruby", "javascript", "typescript"}
    if proposal_id.startswith("axis_map_default_java_map_"):
        return surface.key == "java"
    if proposal_id.startswith("axis_map_default_rust_"):
        return surface.key in {"python", "ruby", "rust"}
    if proposal_id.startswith(("axis_map_default_go_map_", "axis_map_default_go_zero_")):
        return surface.key in {"python", "ruby", "go"}
    if proposal_id.startswith("axis_map_default_module_"):
        return surface.key in {"python", "ruby", "javascript", "typescript", "java"}
    return surface.key in {"python", "ruby"}


def map_default_lookup_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_map_fallback_"):
        return False
    if proposal_id.startswith("axis_map_fallback_python_"):
        return surface.key in {"go", "java", "rust", "python"}
    if proposal_id.startswith("axis_map_fallback_ts_"):
        return surface.key in {"go", "java", "rust", "typescript"}
    if proposal_id.startswith("axis_map_fallback_java_"):
        return surface.key in {"go", "java", "rust"}
    return surface.key in {"go", "java", "rust"}


def map_default_lookup_axis_parts(
    proposal_id: str, negative: bool, right: bool
) -> tuple[str, str, str, str]:
    receiver = "lookup"
    key = "key"
    default = "fallback"
    form = "default_api"
    if proposal_id == "axis_map_fallback_ts_nullish_identity":
        form = "ts_nullish"
    if proposal_id == "axis_map_fallback_ts_has_get_identity":
        form = "ts_has_get"
    if proposal_id == "axis_map_fallback_ts_temp_guard_identity":
        form = "ts_temp_guard"
    if proposal_id == "axis_map_fallback_ts_guard_return_identity":
        form = "ts_guard_return"
    if proposal_id == "axis_map_fallback_java_guard_return_identity":
        form = "java_guard_return"
    if proposal_id.startswith("axis_map_fallback_ts_wrong_"):
        form = "ts_nullish"
    if proposal_id == "axis_map_fallback_ts_untyped_boundary":
        form = "ts_untyped"
    if proposal_id == "axis_map_fallback_python_dict_get_identity":
        form = "py_dict"
    if proposal_id == "axis_map_fallback_python_mapping_get_identity":
        form = "py_mapping"
    if proposal_id == "axis_map_fallback_python_mutable_mapping_get_identity":
        form = "py_mutable_mapping"
    if proposal_id == "axis_map_fallback_python_alias_mapping_identity":
        form = "py_alias_mapping"
    if proposal_id == "axis_map_fallback_python_alias_mutable_mapping_identity":
        form = "py_alias_mutable_mapping"
    if proposal_id == "axis_map_fallback_python_alias_dict_identity":
        form = "py_alias_dict"
    if proposal_id == "axis_map_fallback_python_guard_return_identity":
        form = "py_guard_return"
    if proposal_id.startswith("axis_map_fallback_python_wrong_"):
        form = "py_dict"
    if proposal_id == "axis_map_fallback_python_untyped_boundary":
        form = "py_untyped"
    if proposal_id.startswith("axis_map_fallback_python_alias_wrong_"):
        form = "py_alias_mapping"
    if proposal_id == "axis_map_fallback_python_alias_unresolved_boundary":
        form = "py_alias_unresolved"
    if proposal_id == "axis_map_fallback_python_alias_shadowed_boundary":
        form = "py_alias_shadowed"
    if right and proposal_id == "axis_map_fallback_wrong_key_boundary":
        key = "other_key"
    if right and proposal_id == "axis_map_fallback_wrong_default_boundary":
        default = "other_default"
    if right and proposal_id == "axis_map_fallback_wrong_map_boundary":
        receiver = "other_lookup"
    if right and proposal_id == "axis_map_fallback_ts_wrong_key_boundary":
        key = "other_key"
    if right and proposal_id == "axis_map_fallback_ts_wrong_default_boundary":
        default = "other_default"
    if right and proposal_id == "axis_map_fallback_ts_wrong_map_boundary":
        receiver = "other_lookup"
    if right and proposal_id == "axis_map_fallback_python_wrong_key_boundary":
        key = "other_key"
    if right and proposal_id == "axis_map_fallback_python_wrong_default_boundary":
        default = "other_default"
    if right and proposal_id == "axis_map_fallback_python_wrong_map_boundary":
        receiver = "other_lookup"
    if right and proposal_id == "axis_map_fallback_python_alias_wrong_key_boundary":
        key = "other_key"
    if right and proposal_id == "axis_map_fallback_python_alias_wrong_default_boundary":
        default = "other_default"
    if right and proposal_id == "axis_map_fallback_python_alias_wrong_map_boundary":
        receiver = "other_lookup"
    if right and negative and proposal_id == "axis_map_fallback_identity":
        key = "other_key"
    if right and negative and proposal_id in {
        "axis_map_fallback_ts_nullish_identity",
        "axis_map_fallback_ts_has_get_identity",
        "axis_map_fallback_ts_temp_guard_identity",
        "axis_map_fallback_ts_guard_return_identity",
        "axis_map_fallback_java_guard_return_identity",
        "axis_map_fallback_python_dict_get_identity",
        "axis_map_fallback_python_mapping_get_identity",
        "axis_map_fallback_python_mutable_mapping_get_identity",
        "axis_map_fallback_python_alias_mapping_identity",
        "axis_map_fallback_python_alias_mutable_mapping_identity",
        "axis_map_fallback_python_alias_dict_identity",
        "axis_map_fallback_python_guard_return_identity",
    }:
        key = "other_key"
    return receiver, key, default, form


def axis_map_default_lookup_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    receiver, key, default, form = map_default_lookup_axis_parts(proposal_id, negative, right)
    name = {
        "go": "BuildCase" if right else "AxisCase",
        "java": "buildCase" if right else "axisCase",
        "typescript": "buildCase" if right else "axisCase",
    }.get(surface.language, "build_case" if right else "axis_case")

    if surface.key == "go":
        receiver_go = "otherLookup" if receiver == "other_lookup" else receiver
        key_go = "otherKey" if key == "other_key" else key
        default_go = "otherDefault" if default == "other_default" else default
        src = f"""package p

func {name}(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int {{
    value, ok := {receiver_go}[{key_go}]
    if !ok {{
        value = {default_go}
    }}
    return value
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        if form == "java_guard_return" and right:
            body = f"""if ({receiver}.containsKey({key})) {{
            return {receiver}.get({key});
        }}
        return {default};"""
        elif right:
            expr = f"{receiver}.getOrDefault({key}, {default})"
            body = f"return {expr};"
        else:
            expr = f"{receiver}.containsKey({key}) ? {receiver}.get({key}) : {default}"
            body = f"return {expr};"
        src = f"""import java.util.Map;

class AxisCase {{
    static int {name}(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) {{
        {body}
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        if right:
            expr = f"*{receiver}.get({key}).unwrap_or(&{default})"
        else:
            expr = f"if {receiver}.contains_key({key}) {{ {receiver}[{key}] }} else {{ {default} }}"
        src = f"""use std::collections::HashMap;

pub fn {name}(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 {{
    {expr}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "typescript":
        receiver_type = "Map<string, number>" if form != "ts_untyped" else "any"
        if form == "ts_has_get":
            expr = f"{receiver}.has({key}) ? {receiver}.get({key})! : {default}"
            body = f"return {expr};"
        elif form == "ts_temp_guard":
            body = f"""const selected = {receiver}.get({key});
  return selected === undefined ? {default} : selected;"""
        elif form == "ts_guard_return":
            body = f"""if ({receiver}.has({key})) {{
    return {receiver}.get({key})!;
  }}
  return {default};"""
        else:
            expr = f"{receiver}.get({key}) ?? {default}"
            body = f"return {expr};"
        src = f"""function {name}(lookup: {receiver_type}, other_lookup: {receiver_type}, key: string, other_key: string, fallback: number, other_default: number): number {{
  {body}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        annotation = "dict[str, int]"
        import_line = ""
        if form == "py_mapping":
            annotation = "Mapping[str, int]"
            import_line = "from collections.abc import Mapping\n\n"
        elif form == "py_mutable_mapping":
            annotation = "MutableMapping[str, int]"
            import_line = "from collections.abc import MutableMapping\n\n"
        elif form == "py_alias_mapping":
            annotation = "MapLike[str, int]"
            import_line = "from collections.abc import Mapping as MapLike\n\n"
        elif form == "py_alias_mutable_mapping":
            annotation = "MapLike[str, int]"
            import_line = "from collections.abc import MutableMapping as MapLike\n\n"
        elif form == "py_alias_dict":
            annotation = "MapLike[str, int]"
            import_line = "from typing import Dict as MapLike\n\n"
        elif form == "py_alias_unresolved":
            annotation = "MapLike[str, int]"
        elif form == "py_alias_shadowed":
            annotation = "MapLike[str, int]"
            import_line = "from collections.abc import Mapping as MapLike\nMapLike = list\n\n"
        elif form == "py_untyped":
            annotation = None
        receiver_annotation = f": {annotation}" if annotation else ""
        if form == "py_guard_return":
            body = f"""if {key} in {receiver}:
        return {receiver}[{key}]
    return {default}"""
        else:
            body = f"return {receiver}.get({key}, {default})"
        src = f"""{import_line}def {name}(lookup{receiver_annotation}, other_lookup{receiver_annotation}, key: str, other_key: str, fallback: int, other_default: int) -> int:
    {body}
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for dynamic map default axis: {surface.key}")


GO_NIL_PTR = "__go_nil_ptr__"


def map_default_py_literal(value: object) -> str:
    if value == GO_NIL_PTR:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, str):
        return json.dumps(value)
    return str(value)


def map_default_ruby_literal(value: object) -> str:
    if value == GO_NIL_PTR:
        return "nil"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return json.dumps(value)
    return str(value)


def map_default_go_literal(value: object) -> str:
    if value == GO_NIL_PTR:
        return "nil"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return json.dumps(value)
    return str(value)


def map_default_go_kind(value: object) -> str:
    if value == GO_NIL_PTR:
        return "*Item"
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, float):
        return "float64"
    if isinstance(value, str):
        return "string"
    return "int"


def map_default_go_value_type(entries: tuple[tuple[str, object], tuple[str, object]]) -> str:
    kinds = {map_default_go_kind(value) for _, value in entries}
    if len(kinds) == 1:
        return next(iter(kinds))
    return "any"


def map_default_axis_parts(
    proposal_id: str, negative: bool, right: bool
) -> tuple[str, tuple[tuple[str, object], tuple[str, object]], object, str]:
    key = "key"
    entries = (("red", 1), ("blue", 2))
    default = 0
    form = "literal_api"

    if proposal_id.startswith("axis_map_default_go_zero_"):
        entries = (("red", "apple"), ("blue", "berry"))
        default = ""
    if proposal_id == "axis_map_default_go_zero_bool_inline_identity":
        entries = (("red", True), ("blue", False))
        default = False
    if proposal_id in {
        "axis_map_default_go_zero_float_inline_identity",
        "axis_map_default_go_zero_float_local_identity",
    }:
        entries = (("red", 1.5), ("blue", 2.5))
        default = 0.0
    if proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
        entries = (("red", GO_NIL_PTR), ("blue", GO_NIL_PTR))
        default = GO_NIL_PTR

    if right and proposal_id == "axis_map_default_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and negative and proposal_id == "axis_map_default_literal_identity":
        default = 9
    if proposal_id in {
        "axis_map_default_ruby_fetch_block_int_identity",
        "axis_map_default_ruby_fetch_block_string_identity",
        "axis_map_default_ruby_fetch_block_bool_identity",
    }:
        form = "ruby_fetch_block" if right else "literal_api"
    if proposal_id == "axis_map_default_ruby_fetch_block_string_identity":
        entries = (("red", "apple"), ("blue", "berry"))
        default = ""
    if proposal_id == "axis_map_default_ruby_fetch_block_bool_identity":
        entries = (("red", True), ("blue", False))
        default = False
    if proposal_id == "axis_map_default_js_map_inline_identity":
        form = "js_map_inline" if right else "literal_api"
    if proposal_id == "axis_map_default_js_map_local_identity":
        form = "js_map_local" if right else "literal_api"
    if proposal_id == "axis_map_default_js_map_has_get_identity":
        form = "js_map_has_get" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_js_map_wrong_key_boundary",
        "axis_map_default_js_map_wrong_default_boundary",
        "axis_map_default_js_map_wrong_map_boundary",
    }:
        form = "js_map_inline" if right else "literal_api"
    if proposal_id == "axis_map_default_js_map_untyped_receiver_boundary":
        form = "js_map_untyped" if right else "literal_api"
    if proposal_id == "axis_map_default_js_map_shadowed_constructor_boundary":
        form = "js_map_shadowed" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_hasown_identity":
        form = "js_object_hasown" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_call_identity":
        form = "js_object_call" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_negated_identity":
        form = "js_object_negated" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_js_object_wrong_key_boundary",
        "axis_map_default_js_object_wrong_default_boundary",
        "axis_map_default_js_object_wrong_map_boundary",
    }:
        form = "js_object_hasown" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_unguarded_boundary":
        form = "js_object_unguarded" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_in_boundary":
        form = "js_object_in" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_method_boundary":
        form = "js_object_method" if right else "literal_api"
    if proposal_id == "axis_map_default_js_object_shadowed_boundary":
        form = "js_object_shadowed" if right else "literal_api"
    if proposal_id == "axis_map_default_java_map_of_identity":
        form = "java_map_of" if right else "literal_api"
    if proposal_id == "axis_map_default_java_map_of_entries_identity":
        form = "java_map_of_entries" if right else "literal_api"
    if proposal_id == "axis_map_default_java_map_local_identity":
        form = "java_map_local" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_java_map_wrong_key_boundary",
        "axis_map_default_java_map_wrong_default_boundary",
        "axis_map_default_java_map_wrong_map_boundary",
    }:
        form = "java_map_of" if right else "literal_api"
    if proposal_id == "axis_map_default_java_map_shadowed_factory_boundary":
        form = "java_map_shadowed_factory" if right else "literal_api"
    if proposal_id == "axis_map_default_java_map_type_shadow_boundary":
        form = "java_map_type_shadow" if right else "literal_api"
    if proposal_id == "axis_map_default_rust_hashmap_from_identity":
        form = "rust_hashmap_from" if right else "literal_api"
    if proposal_id == "axis_map_default_rust_btreemap_from_identity":
        form = "rust_btreemap_from" if right else "literal_api"
    if proposal_id == "axis_map_default_rust_hashmap_local_identity":
        form = "rust_hashmap_local" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_rust_wrong_key_boundary",
        "axis_map_default_rust_wrong_default_boundary",
        "axis_map_default_rust_wrong_map_boundary",
    }:
        form = "rust_hashmap_from" if right else "literal_api"
    if proposal_id == "axis_map_default_rust_mutated_boundary":
        form = "rust_hashmap_mutated" if right else "literal_api"
    if proposal_id == "axis_map_default_go_map_inline_identity":
        form = "go_map_inline" if right else "literal_api"
    if proposal_id == "axis_map_default_go_map_local_identity":
        form = "go_map_local" if right else "literal_api"
    if proposal_id == "axis_map_default_go_map_var_identity":
        form = "go_map_var" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_go_map_wrong_key_boundary",
        "axis_map_default_go_map_wrong_map_boundary",
    }:
        form = "go_map_inline" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_go_zero_string_inline_identity",
        "axis_map_default_go_zero_bool_inline_identity",
        "axis_map_default_go_zero_float_inline_identity",
        "axis_map_default_go_zero_nil_pointer_identity",
    }:
        form = "go_map_inline" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_go_zero_string_local_identity",
        "axis_map_default_go_zero_float_local_identity",
    }:
        form = "go_map_local" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_go_zero_wrong_key_boundary",
        "axis_map_default_go_zero_wrong_map_boundary",
        "axis_map_default_go_zero_mixed_value_boundary",
    }:
        form = "go_map_inline" if right else "literal_api"
    if proposal_id == "axis_map_default_module_js_map_identity":
        form = "js_map_module" if right else "literal_api"
    if proposal_id == "axis_map_default_module_ts_map_identity":
        form = "js_map_module" if right else "literal_api"
    if proposal_id == "axis_map_default_module_java_map_identity":
        form = "java_map_static" if right else "literal_api"
    if proposal_id in {
        "axis_map_default_module_wrong_key_boundary",
        "axis_map_default_module_wrong_default_boundary",
        "axis_map_default_module_wrong_map_boundary",
    }:
        form = "module_map" if right else "literal_api"
    if proposal_id == "axis_map_default_module_mutated_boundary":
        form = "js_map_module_mutated" if right else "literal_api"
    if proposal_id == "axis_map_default_module_shadowed_boundary":
        form = "module_map_shadowed" if right else "literal_api"
    if right and proposal_id == "axis_map_default_js_map_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_js_map_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_js_map_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and proposal_id == "axis_map_default_js_object_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_js_object_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_js_object_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and proposal_id == "axis_map_default_java_map_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_java_map_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_java_map_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and proposal_id == "axis_map_default_rust_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_rust_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_rust_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and proposal_id == "axis_map_default_go_map_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_go_map_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and proposal_id == "axis_map_default_go_zero_wrong_key_boundary":
        key = "other"
    if proposal_id == "axis_map_default_go_zero_wrong_map_boundary":
        entries = (("red", True), ("blue", False))
        default = False
    if right and proposal_id == "axis_map_default_go_zero_wrong_map_boundary":
        entries = (("red", False), ("blue", False))
    if right and proposal_id == "axis_map_default_go_zero_mixed_value_boundary":
        entries = (("red", "apple"), ("blue", False))
    if right and proposal_id == "axis_map_default_module_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_module_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_module_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and negative and proposal_id in {
        "axis_map_default_js_map_inline_identity",
        "axis_map_default_js_map_local_identity",
        "axis_map_default_js_map_has_get_identity",
        "axis_map_default_js_object_hasown_identity",
        "axis_map_default_js_object_call_identity",
        "axis_map_default_js_object_negated_identity",
        "axis_map_default_java_map_of_identity",
        "axis_map_default_java_map_of_entries_identity",
        "axis_map_default_java_map_local_identity",
        "axis_map_default_rust_hashmap_from_identity",
        "axis_map_default_rust_btreemap_from_identity",
        "axis_map_default_rust_hashmap_local_identity",
        "axis_map_default_go_map_inline_identity",
        "axis_map_default_go_map_local_identity",
        "axis_map_default_go_map_var_identity",
        "axis_map_default_go_zero_string_inline_identity",
        "axis_map_default_go_zero_string_local_identity",
        "axis_map_default_go_zero_bool_inline_identity",
        "axis_map_default_go_zero_float_inline_identity",
        "axis_map_default_go_zero_float_local_identity",
        "axis_map_default_module_js_map_identity",
        "axis_map_default_module_ts_map_identity",
        "axis_map_default_module_java_map_identity",
        "axis_map_default_ruby_fetch_block_int_identity",
        "axis_map_default_ruby_fetch_block_string_identity",
        "axis_map_default_ruby_fetch_block_bool_identity",
    }:
        if proposal_id.startswith(("axis_map_default_go_map_", "axis_map_default_go_zero_")):
            key = "other"
        else:
            default = 9
    if right and negative and proposal_id == "axis_map_default_go_zero_nil_pointer_identity":
        entries = (("red", "apple"), ("blue", "berry"))
        default = ""
    return key, entries, default, form


def axis_map_default_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    key, entries, default, form = map_default_axis_parts(proposal_id, negative, right)
    name = {
        "javascript": "buildCase" if right else "axisCase",
        "typescript": "buildCase" if right else "axisCase",
    }.get(surface.key, "build_case" if right else "axis_case")
    (k1, v1), (k2, v2) = entries
    if (
        surface.key in {"javascript", "typescript"}
        and form == "literal_api"
        and proposal_id.startswith("axis_map_default_js_map_")
    ):
        form = "js_map_inline"
    if (
        surface.key in {"javascript", "typescript"}
        and form == "literal_api"
        and proposal_id.startswith("axis_map_default_js_object_")
    ):
        form = "js_object_hasown"
    if (
        surface.key in {"javascript", "typescript"}
        and form == "literal_api"
        and proposal_id.startswith("axis_map_default_module_")
    ):
        form = "js_map_module"

    if surface.key == "python":
        src = f"""def {name}(key, other):
    return {{"{k1}": {map_default_py_literal(v1)}, "{k2}": {map_default_py_literal(v2)}}}.get({key}, {map_default_py_literal(default)})
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        if form == "ruby_fetch_block":
            src = f"""def {name}(key, other)
  {{"{k1}" => {map_default_ruby_literal(v1)}, "{k2}" => {map_default_ruby_literal(v2)}}}.fetch({key}) {{ {map_default_ruby_literal(default)} }}
end
"""
            return Variant("axis", src, name)
        src = f"""def {name}(key, other)
  {{"{k1}" => {map_default_ruby_literal(v1)}, "{k2}" => {map_default_ruby_literal(v2)}}}.fetch({key}, {map_default_ruby_literal(default)})
end
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        value_type = map_default_go_value_type(entries)
        go_type = "interface{}" if value_type == "any" else value_type
        type_decl = "type Item struct{}\n\n" if go_type == "*Item" else ""
        map_expr = (
            f'map[string]{go_type}{{"{k1}": {map_default_go_literal(v1)}, '
            f'"{k2}": {map_default_go_literal(v2)}}}'
        )
        if form == "literal_api":
            form = "go_map_inline"
        if form == "go_map_inline":
            src = f"""package p

{type_decl}\
func {name}(key string, other string) {go_type} {{
    return {map_expr}[{key}]
}}
"""
            return Variant("axis", src, name)
        if form == "go_map_local":
            src = f"""package p

{type_decl}\
func {name}(key string, other string) {go_type} {{
    lookup := {map_expr}
    return lookup[{key}]
}}
"""
            return Variant("axis", src, name)
        if form == "go_map_var":
            src = f"""package p

{type_decl}\
func {name}(key string, other string) {go_type} {{
    var lookup = {map_expr}
    return lookup[{key}]
}}
"""
            return Variant("axis", src, name)

    if surface.key == "java":
        if form == "literal_api":
            form = "java_map_of"
        if form == "module_map":
            form = "java_map_static"
        if form == "module_map_shadowed":
            form = "java_map_type_shadow"
        method_name = "buildCase" if right else "axisCase"
        map_of = f'Map.of("{k1}", {v1}, "{k2}", {v2})'
        map_entries = f'Map.ofEntries(Map.entry("{k1}", {v1}), Map.entry("{k2}", {v2}))'
        if form == "java_map_of":
            src = f"""import java.util.Map;

class AxisCase {{
    static int {method_name}(String key, String other) {{
        return {map_of}.getOrDefault({key}, {default});
    }}
}}
"""
            return Variant("axis", src, method_name)
        if form == "java_map_of_entries":
            src = f"""import java.util.Map;

class AxisCase {{
    static int {method_name}(String key, String other) {{
        return {map_entries}.getOrDefault({key}, {default});
    }}
}}
"""
            return Variant("axis", src, method_name)
        if form == "java_map_local":
            src = f"""import java.util.Map;

class AxisCase {{
    static int {method_name}(String key, String other) {{
        Map<String, Integer> lookup = {map_of};
        return lookup.getOrDefault({key}, {default});
    }}
}}
"""
            return Variant("axis", src, method_name)
        if form == "java_map_shadowed_factory":
            src = f"""class AxisCase {{
    static class MapFactory {{
        java.util.Map<String, Integer> of(Object... values) {{
            return java.util.Map.of();
        }}
    }}

    static int {method_name}(String key, String other, MapFactory Map) {{
        return {map_of}.getOrDefault({key}, {default});
    }}
}}
"""
            return Variant("axis", src, method_name)
        if form == "java_map_type_shadow":
            src = f"""class AxisCase {{
    static int {method_name}(String key, String other) {{
        return {map_of}.getOrDefault({key}, {default});
    }}
}}

class Map {{
    static java.util.Map<String, Integer> of(Object... values) {{
        return java.util.Map.of();
    }}
}}
"""
            return Variant("axis", src, method_name)
        if form == "java_map_static":
            src = f"""import java.util.Map;

class AxisCase {{
    static final Map<String, Integer> LOOKUP = {map_of};

    static int {method_name}(String key, String other) {{
        return LOOKUP.getOrDefault({key}, {default});
    }}
}}
"""
            return Variant("axis", src, method_name)

    if surface.key == "rust":
        map_entries = f' [("{k1}", {v1}), ("{k2}", {v2})]'
        if form == "literal_api":
            form = "rust_hashmap_from"
        if form == "rust_hashmap_from":
            src = f"""pub fn {name}(key: &str, other: &str) -> i32 {{
    *std::collections::HashMap::from({map_entries}).get({key}).unwrap_or(&{default})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_btreemap_from":
            src = f"""pub fn {name}(key: &str, other: &str) -> i32 {{
    *std::collections::BTreeMap::from({map_entries}).get({key}).unwrap_or(&{default})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_hashmap_local":
            src = f"""pub fn {name}(key: &str, other: &str) -> i32 {{
    let lookup = std::collections::HashMap::from({map_entries});
    *lookup.get({key}).unwrap_or(&{default})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_hashmap_mutated":
            src = f"""pub fn {name}(key: &str, other: &str) -> i32 {{
    let mut lookup = std::collections::HashMap::from({map_entries});
    lookup.insert("{k1}", 9);
    *lookup.get(key).unwrap_or(&0)
}}
"""
            return Variant("axis", src, name)

    if surface.key in {"javascript", "typescript"}:
        typed = surface.key == "typescript"
        type_args = "<string, number>" if typed else ""
        key_sig = "key: string, other: string" if typed else "key, other"
        return_ty = ": number" if typed else ""
        map_entries = f'[["{k1}", {v1}], ["{k2}", {v2}]]'
        map_expr = f"new Map{type_args}({map_entries})"
        if form == "module_map":
            form = "js_map_module"
        if form == "module_map_shadowed":
            form = "js_map_module_shadowed"
        if form == "js_map_inline":
            body = f"return {map_expr}.get({key}) ?? {default};"
            src = f"""function {name}({key_sig}){return_ty} {{
  {body}
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_local":
            src = f"""function {name}({key_sig}){return_ty} {{
  const lookup = {map_expr};
  return lookup.get({key}) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_has_get":
            get_expr = f"lookup.get({key})!" if typed else f"lookup.get({key})"
            src = f"""function {name}({key_sig}){return_ty} {{
  const lookup = {map_expr};
  return lookup.has({key}) ? {get_expr} : {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_module":
            src = f"""const LOOKUP = {map_expr};

function {name}({key_sig}){return_ty} {{
  return LOOKUP.get({key}) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_module_mutated":
            src = f"""const LOOKUP = {map_expr};
LOOKUP.set("{k1}", 9);

function {name}({key_sig}){return_ty} {{
  return LOOKUP.get({key}) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_module_shadowed":
            ts_any = ": any" if typed else ""
            src = f"""const Map{ts_any} = function(_entries{ts_any}) {{
  return {{ get: function() {{ return 9; }} }};
}};
const LOOKUP = new Map({map_entries});

function {name}({key_sig}){return_ty} {{
  return LOOKUP.get({key}) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_untyped":
            sig = (
                "lookup: any, key: string, other: string"
                if typed
                else "lookup, key, other"
            )
            src = f"""function {name}({sig}){return_ty} {{
  return lookup.get(key) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        if form == "js_map_shadowed":
            sig = (
                "key: string, other: string, Map: any"
                if typed
                else "key, other, Map"
            )
            src = f"""function {name}({sig}){return_ty} {{
  return {map_expr}.get({key}) ?? {default};
}}
"""
            return js_axis_source(surface, src, name)
        object_type = ": Record<string, number>" if typed else ""
        object_entries = f'{{ "{k1}": {v1}, "{k2}": {v2} }}'
        if form.startswith("js_object_"):
            shadow_param = ", Object: any" if typed and form == "js_object_shadowed" else ""
            shadow_param = ", Object" if not typed and form == "js_object_shadowed" else shadow_param
            src_key_sig = key_sig.replace(")", "")
            guard = f"Object.hasOwn(lookup, {key})"
            if form == "js_object_call":
                guard = f"Object.prototype.hasOwnProperty.call(lookup, {key})"
            elif form == "js_object_negated":
                guard = f"!Object.hasOwn(lookup, {key})"
            elif form == "js_object_in":
                guard = f"{key} in lookup"
            elif form == "js_object_method":
                guard = f"lookup.hasOwnProperty({key})"
            then_expr = default if form == "js_object_negated" else f"lookup[{key}]"
            else_expr = f"lookup[{key}]" if form == "js_object_negated" else default
            if form == "js_object_unguarded":
                body = f"return lookup[{key}] ?? {default};"
            else:
                body = f"return {guard} ? {then_expr} : {else_expr};"
            src = f"""function {name}({src_key_sig}{shadow_param}){return_ty} {{
  const lookup{object_type} = {object_entries};
  {body}
}}
"""
            return js_axis_source(surface, src, name)

    raise ValueError(f"unsupported surface for literal map default axis: {surface.key}")
