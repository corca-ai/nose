"""Record, collection-state, and string Type-4 axis source templates."""

from __future__ import annotations

from .model import JS_LIKE_SURFACES, Surface, Variant, js_axis_source


def record_guard_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_record_guard_") and surface.key in JS_LIKE_SURFACES


def own_property_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_own_property_") and surface.key in JS_LIKE_SURFACES


def axis_own_property_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    name = "buildCase" if right else "axisCase"
    key = "enabled" if right and negative and proposal_id == "axis_own_property_hasown_identity" else "ready"
    if right and proposal_id == "axis_own_property_in_boundary":
        body = f"""function {name}(value) {{
  return '{key}' in value;
}}
"""
    elif right and proposal_id == "axis_own_property_method_boundary":
        body = f"""function {name}(value) {{
  return value.hasOwnProperty('{key}');
}}
"""
    elif right and proposal_id == "axis_own_property_shadow_boundary":
        body = f"""function {name}(Object, value) {{
  return Object.hasOwn(value, '{key}');
}}
"""
    elif right:
        body = f"""function {name}(candidate) {{
  return Object.prototype.hasOwnProperty.call(candidate, '{key}');
}}
"""
    else:
        body = f"""function {name}(value) {{
  return Object.hasOwn(value, '{key}');
}}
"""
    if surface.language == "javascript":
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        typed = body.replace(f"function {name}(value)", f"function {name}(value: object): boolean")
        typed = typed.replace(
            f"function {name}(candidate)", f"function {name}(candidate: object): boolean"
        )
        return Variant("axis", typed, name)

    raise ValueError(f"unsupported surface for own property axis: {surface.key}")


def axis_record_guard_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    name = "buildCase" if right else "axisCase"
    if (
        right
        and negative
        and proposal_id
        not in {"axis_record_guard_array_boundary", "axis_record_guard_null_boundary"}
    ):
        body = f"""function {name}(value) {{
  return typeof value === 'object' && value !== null && !Array.isArray(value) && value.ready === true;
}}
"""
    elif right and proposal_id == "axis_record_guard_truthy_identity":
        body = f"""function {name}(value) {{
  return !!value && typeof value === 'object' && !Array.isArray(value);
}}
"""
    elif right and proposal_id == "axis_record_guard_order_identity":
        body = f"""function {name}(input) {{
  return !Array.isArray(input) && input !== null && typeof input === 'object';
}}
"""
    elif right and proposal_id == "axis_record_guard_array_boundary":
        body = f"""function {name}(value) {{
  return typeof value === 'object' && value !== null;
}}
"""
    elif right and proposal_id == "axis_record_guard_null_boundary":
        body = f"""function {name}(value) {{
  return typeof value === 'object' && !Array.isArray(value);
}}
"""
    else:
        body = f"""function {name}(value) {{
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}}
"""
    if surface.language == "javascript":
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        typed = body.replace(f"function {name}(value)", f"function {name}(value: unknown): boolean")
        typed = typed.replace(f"function {name}(input)", f"function {name}(input: unknown): boolean")
        return Variant("axis", typed, name)

    raise ValueError(f"unsupported surface for record guard axis: {surface.key}")


def collection_empty_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_collection_"):
        return False
    if proposal_id.startswith("axis_collection_typed_domain_"):
        return surface.key == "java"
    return surface.key in {
        "python",
        "javascript",
        "typescript",
        "go",
        "rust",
        "java",
        "c",
        "ruby",
        "vue",
        "svelte",
        "html",
    }


def axis_collection_empty_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    empty = proposal_id == "axis_collection_empty_named_identity"
    nonempty = proposal_id == "axis_collection_nonempty_named_identity"
    wrong_threshold = proposal_id == "axis_collection_threshold_boundary"
    wrong_receiver = proposal_id == "axis_collection_wrong_receiver_boundary"
    typed_domain_array = proposal_id == "axis_collection_typed_domain_array_boundary"
    typed_domain_string = proposal_id == "axis_collection_typed_domain_string_boundary"
    semantic_mutation = right and negative and not (wrong_threshold or wrong_receiver)

    if typed_domain_array or typed_domain_string:
        if surface.key != "java":
            raise ValueError(f"unsupported typed-domain empty boundary surface: {surface.key}")
        name = "buildCase" if right else "axisCase"
        if not right:
            src = f"""import java.util.Queue;

class AxisCase {{
    static boolean {name}(Queue<String> values) {{
        return values == null || values.isEmpty();
    }}
}}
"""
            return Variant("java_queue_null_empty", src, name)
        if typed_domain_array:
            src = f"""class AxisCase {{
    static boolean {name}(Object[] values) {{
        return values == null || values.length == 0;
    }}
}}
"""
            return Variant("java_array_null_empty", src, name)
        src = f"""class AxisCase {{
    static boolean {name}(String value) {{
        return value == null || value.isEmpty();
    }}
}}
"""
        return Variant("java_string_null_empty", src, name)

    if surface.language == "javascript":
        name = "buildCase" if right else "axisCase"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"{param}.length === 1"
        elif semantic_mutation and nonempty:
            expr = f"{param}.length === 0"
        elif nonempty:
            expr = f"{param}.length !== 0"
        elif right and negative and wrong_threshold:
            expr = f"{param}.length === 1"
        elif right and not negative and surface.key in JS_LIKE_SURFACES:
            expr = f"0 === {param}.length"
        else:
            expr = f"{param}.length === 0"
        body = f"""function {name}(items, other) {{
  return {expr};
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        name = "buildCase" if right else "axisCase"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"{param}.length === 1"
        elif semantic_mutation and nonempty:
            expr = f"{param}.length === 0"
        elif nonempty:
            expr = f"{param}.length !== 0"
        elif right and negative and wrong_threshold:
            expr = f"{param}.length === 1"
        elif right and not negative:
            expr = f"0 === {param}.length"
        else:
            expr = f"{param}.length === 0"
        src = f"""function {name}(items: number[], other: number[]): boolean {{
  return {expr};
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        name = "build_case" if right else "axis_case"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"len({param}) == 1"
        elif semantic_mutation and nonempty:
            expr = f"len({param}) == 0"
        elif nonempty:
            expr = f"len({param}) != 0"
        elif right and negative and wrong_threshold:
            expr = f"len({param}) == 1"
        elif right and not negative:
            expr = f"0 == len({param})"
        else:
            expr = f"len({param}) == 0"
        src = f"""def {name}(items, other):
    return {expr}
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        name = "BuildCase" if right else "AxisCase"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"len({param}) == 1"
        elif semantic_mutation and nonempty:
            expr = f"len({param}) == 0"
        elif nonempty:
            expr = f"len({param}) != 0"
        elif right and negative and wrong_threshold:
            expr = f"len({param}) == 1"
        elif right and not negative:
            expr = f"0 == len({param})"
        else:
            expr = f"len({param}) == 0"
        src = f"""package p

func {name}(items []int, other []int) bool {{
    return {expr}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        name = "build_case" if right else "axis_case"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"{param}.len() == 1"
        elif semantic_mutation and nonempty:
            expr = f"{param}.is_empty()"
        elif nonempty:
            expr = f"!{param}.is_empty()" if right and not negative else f"{param}.len() != 0"
        elif right and negative and wrong_threshold:
            expr = f"{param}.len() == 1"
        elif right and not negative:
            expr = f"{param}.is_empty()"
        else:
            expr = f"{param}.len() == 0"
        src = f"""pub fn {name}(items: &[i32], other: &[i32]) -> bool {{
    {expr}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        name = "buildCase" if right else "axisCase"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"{param}.size() == 1"
        elif semantic_mutation and nonempty:
            expr = f"{param}.isEmpty()"
        elif nonempty:
            expr = f"!{param}.isEmpty()" if right and not negative else f"{param}.size() != 0"
        elif right and negative and wrong_threshold:
            expr = f"{param}.size() == 1"
        elif right and not negative:
            expr = f"{param}.isEmpty()"
        else:
            expr = f"{param}.size() == 0"
        src = f"""class AxisCase {{
    static boolean {name}(java.util.List<Integer> items, java.util.List<Integer> other) {{
        return {expr};
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "c":
        name = "build_case" if right else "axis_case"
        param = "m" if right and negative and wrong_receiver else "n"
        if semantic_mutation and empty:
            expr = f"{param} == 1"
        elif semantic_mutation and nonempty:
            expr = f"{param} == 0"
        elif nonempty:
            expr = f"{param} != 0"
        elif right and negative and wrong_threshold:
            expr = f"{param} == 1"
        elif right and not negative:
            expr = f"0 == {param}"
        else:
            expr = f"{param} == 0"
        src = f"""int {name}(int *items, int n, int *other, int m) {{
    return {expr};
}}
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        name = "build_case" if right else "axis_case"
        param = "other" if right and negative and wrong_receiver else "items"
        if semantic_mutation and empty:
            expr = f"{param}.length == 1"
        elif semantic_mutation and nonempty:
            expr = f"{param}.empty?"
        elif nonempty:
            expr = f"!{param}.empty?" if right and not negative else f"{param}.length != 0"
        elif right and negative and wrong_threshold:
            expr = f"{param}.length == 1"
        elif right and not negative:
            expr = f"{param}.empty?"
        else:
            expr = f"{param}.length == 0"
        src = f"""def {name}(items, other)
  {expr}
end
"""
        return Variant("axis", src, name)
    raise ValueError(f"unsupported surface for collection-empty axis: {surface.key}")


def string_prefix_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_string_"):
        return False
    return surface.key in {
        "python",
        "javascript",
        "typescript",
        "go",
        "rust",
        "java",
        "ruby",
        "vue",
        "svelte",
        "html",
    }


def string_axis_parts(proposal_id: str, negative: bool, right: bool) -> tuple[str, str, str]:
    op = "suffix" if proposal_id == "axis_string_suffix_identity" else "prefix"
    affix = "suf" if op == "suffix" else "pre"
    receiver = "value"

    if right and proposal_id == "axis_string_direction_boundary":
        op = "suffix" if op == "prefix" else "prefix"
    if right and proposal_id == "axis_string_affix_boundary":
        affix = "alt" if op == "prefix" else "end"
    if right and proposal_id == "axis_string_wrong_receiver_boundary":
        receiver = "other"
    if right and negative and proposal_id in {
        "axis_string_prefix_identity",
        "axis_string_suffix_identity",
    }:
        affix = "alt" if op == "prefix" else "end"
    return op, affix, receiver


def axis_string_prefix_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    op, affix, receiver = string_axis_parts(proposal_id, negative, right)
    name = {
        "javascript": "buildCase" if right else "axisCase",
        "typescript": "buildCase" if right else "axisCase",
        "go": "BuildCase" if right else "AxisCase",
        "java": "buildCase" if right else "axisCase",
    }.get(surface.language, "build_case" if right else "axis_case")

    if surface.language == "javascript":
        method = "startsWith" if op == "prefix" else "endsWith"
        body = f"""function {name}(value, other) {{
  return {receiver}.{method}("{affix}");
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        method = "startsWith" if op == "prefix" else "endsWith"
        src = f"""function {name}(value: string, other: string): boolean {{
  return {receiver}.{method}("{affix}");
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        method = "startswith" if op == "prefix" else "endswith"
        src = f"""def {name}(value, other):
    return {receiver}.{method}("{affix}")
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        method = "HasPrefix" if op == "prefix" else "HasSuffix"
        src = f"""package p

import "strings"

func {name}(value string, other string) bool {{
    return strings.{method}({receiver}, "{affix}")
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        method = "starts_with" if op == "prefix" else "ends_with"
        src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    {receiver}.{method}("{affix}")
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        method = "startsWith" if op == "prefix" else "endsWith"
        src = f"""class AxisCase {{
    static boolean {name}(String value, String other) {{
        return {receiver}.{method}("{affix}");
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        method = "start_with?" if op == "prefix" else "end_with?"
        src = f"""def {name}(value, other)
  {receiver}.{method}("{affix}")
end
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for string prefix/suffix axis: {surface.key}")
