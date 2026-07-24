"""Scalar, null, and low-level Type-4 axis source templates."""

from __future__ import annotations

from .model import JS_LIKE_SURFACES, Surface, Variant, js_axis_source


def axis_immutable_binding_variant(surface: Surface, negative: bool, right: bool) -> Variant:
    value = 8 if negative else 7
    if surface.language == "javascript":
        name = "axisCase"
        body = f"""function {name}(value) {{
  const base = {value};
  const limit = base;
  return value + limit;
}}
"""
        return js_axis_source(surface, body, name)
    if surface.key == "typescript":
        src = f"""function axisCase(value: number): number {{
  const base = {value};
  const limit = base;
  return value + limit;
}}
"""
        return Variant("axis", src, "axisCase")
    if surface.key == "python":
        src = f"""def axis_case(value):
    base = {value}
    limit = base
    return value + limit
"""
        return Variant("axis", src, "axis_case")
    if surface.key == "go":
        src = f"""package p

func AxisCase(value int) int {{
    base := {value}
    limit := base
    return value + limit
}}
"""
        return Variant("axis", src, "AxisCase")
    if surface.key == "rust":
        src = f"""pub fn axis_case(value: i32) -> i32 {{
    let base = {value};
    let limit = base;
    value + limit
}}
"""
        return Variant("axis", src, "axis_case")
    if surface.key == "java":
        src = f"""class AxisCase {{
    static int axisCase(int value) {{
        int base = {value};
        int limit = base;
        return value + limit;
    }}
}}
"""
        return Variant("axis", src, "axisCase")
    if surface.key == "c":
        src = f"""int axis_case(int value) {{
    int base = {value};
    int limit = base;
    return value + limit;
}}
"""
        return Variant("axis", src, "axis_case")
    if surface.key == "ruby":
        src = f"""def axis_case(value)
  base = {value}
  limit = base
  value + limit
end
"""
        return Variant("axis", src, "axis_case")
    if surface.key == "swift":
        src = f"""func axisCase(_ value: Int) -> Int {{
    let base = {value}
    let limit = base
    return value + limit
}}
"""
        return Variant("axis", src, "axisCase")
    raise ValueError(f"unsupported surface for immutable axis: {surface.key}")


def axis_callee_identity_variant(surface: Surface, negative: bool, right: bool) -> Variant:
    delta = 2 if negative else 1
    adjusted = "input" if right else "value"
    if surface.language == "javascript":
        name = "buildCase" if right else "axisCase"
        body = f"""function helper(v) {{
  return v + {delta};
}}

function {name}({adjusted}) {{
  const shifted = {adjusted} + 1;
  return helper(shifted);
}}
"""
        return js_axis_source(surface, body, name)
    if surface.key == "typescript":
        name = "buildCase" if right else "axisCase"
        src = f"""function helper(v: number): number {{
  return v + {delta};
}}

function {name}({adjusted}: number): number {{
  const shifted = {adjusted} + 1;
  return helper(shifted);
}}
"""
        return Variant("axis", src, name)
    if surface.key == "python":
        name = "build_case" if right else "axis_case"
        src = f"""def helper(v):
    return v + {delta}

def {name}({adjusted}):
    shifted = {adjusted} + 1
    return helper(shifted)
"""
        return Variant("axis", src, name)
    if surface.key == "go":
        name = "BuildCase" if right else "AxisCase"
        src = f"""package p

func helper(v int) int {{
    return v + {delta}
}}

func {name}({adjusted} int) int {{
    shifted := {adjusted} + 1
    return helper(shifted)
}}
"""
        return Variant("axis", src, name)
    if surface.key == "rust":
        name = "build_case" if right else "axis_case"
        src = f"""fn helper(v: i32) -> i32 {{
    v + {delta}
}}

pub fn {name}({adjusted}: i32) -> i32 {{
    let shifted = {adjusted} + 1;
    helper(shifted)
}}
"""
        return Variant("axis", src, name)
    if surface.key == "java":
        name = "buildCase" if right else "axisCase"
        src = f"""class AxisCase {{
    static int helper(int v) {{
        return v + {delta};
    }}

    static int {name}(int {adjusted}) {{
        int shifted = {adjusted} + 1;
        return helper(shifted);
    }}
}}
"""
        return Variant("axis", src, name)
    if surface.key == "c":
        name = "build_case" if right else "axis_case"
        src = f"""int helper(int v) {{
    return v + {delta};
}}

int {name}(int {adjusted}) {{
    int shifted = {adjusted} + 1;
    return helper(shifted);
}}
"""
        return Variant("axis", src, name)
    if surface.key == "ruby":
        name = "build_case" if right else "axis_case"
        src = f"""def helper(v)
  v + {delta}
end

def {name}({adjusted})
  shifted = {adjusted} + 1
  helper(shifted)
end
"""
        return Variant("axis", src, name)
    if surface.key == "swift":
        name = "buildCase" if right else "axisCase"
        src = f"""func helper(_ value: Int) -> Int {{
    return value + {delta}
}}

func {name}(_ {adjusted}: Int) -> Int {{
    let shifted = {adjusted} + 1
    return helper(shifted)
}}
"""
        return Variant("axis", src, name)
    raise ValueError(f"unsupported surface for callee axis: {surface.key}")


def axis_table_access_variant(surface: Surface, negative: bool, right: bool) -> Variant:
    key = "tomorrow" if negative else "today"
    if surface.language == "javascript":
        name = "buildCase" if right else "axisCase"
        body = f"""function {name}(value) {{
  const table = {{ today: 7, tomorrow: 8 }};
  return value + table.{key};
}}
"""
        return js_axis_source(surface, body, name)
    if surface.key == "typescript":
        name = "buildCase" if right else "axisCase"
        src = f"""function {name}(value: number): number {{
  const table = {{ today: 7, tomorrow: 8 }};
  return value + table.{key};
}}
"""
        return Variant("axis", src, name)
    if surface.key == "python":
        name = "build_case" if right else "axis_case"
        src = f"""def {name}(value):
    table = {{"today": 7, "tomorrow": 8}}
    return value + table["{key}"]
"""
        return Variant("axis", src, name)
    if surface.key == "ruby":
        name = "build_case" if right else "axis_case"
        ruby_key = f":{key}"
        src = f"""def {name}(value)
  table = {{ today: 7, tomorrow: 8 }}
  value + table[{ruby_key}]
end
"""
        return Variant("axis", src, name)
    raise ValueError(f"unsupported surface for table axis: {surface.key}")


def nullish_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if proposal_id.startswith("axis_option_"):
        return surface.key == "rust"
    return proposal_id.startswith("axis_nullish_") and surface.key in JS_LIKE_SURFACES


def axis_nullish_variant(surface: Surface, proposal_id: str, negative: bool, right: bool) -> Variant:
    name = "buildCase" if right else "axisCase"
    snake_name = "build_case" if right else "axis_case"
    fallback = (
        "fallback + 1"
        if negative and right and proposal_id != "axis_nullish_truthy_boundary"
        else "fallback"
    )
    if surface.language == "javascript":
        if proposal_id == "axis_nullish_guard_identity" and right:
            body = f"""function {name}(value, fallback) {{
  if (value == null) {{
    return {fallback};
  }}
  return value;
}}
"""
        elif proposal_id == "axis_nullish_truthy_boundary" and right:
            body = f"""function {name}(value, fallback) {{
  return value || {fallback};
}}
"""
        elif right:
            body = f"""function {name}(value, fallback) {{
  return value == null ? {fallback} : value;
}}
"""
        else:
            body = f"""function {name}(value, fallback) {{
  return value ?? fallback;
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        if proposal_id == "axis_nullish_guard_identity" and right:
            src = f"""function {name}(value: number | null | undefined, fallback: number): number {{
  if (value == null) {{
    return {fallback};
  }}
  return value;
}}
"""
        elif proposal_id == "axis_nullish_truthy_boundary" and right:
            src = f"""function {name}(value: number | null | undefined, fallback: number): number {{
  return value || {fallback};
}}
"""
        elif right:
            src = f"""function {name}(value: number | null | undefined, fallback: number): number {{
  return value == null ? {fallback} : value;
}}
"""
        else:
            src = f"""function {name}(value: number | null | undefined, fallback: number): number {{
  return value ?? fallback;
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        rust_name = snake_name
        target = "other" if right and proposal_id == "axis_option_wrong_value_boundary" else "value"
        default = (
            "other_default"
            if right and (negative or proposal_id == "axis_option_wrong_default_boundary")
            else "fallback"
        )
        if right and proposal_id == "axis_option_unwrap_or_else_identity":
            expr = f"{target}.unwrap_or_else(|| {default})"
        elif right and proposal_id == "axis_option_map_or_identity":
            expr = f"{target}.map_or({default}, |inner| inner)"
        elif right:
            expr = f"{target}.unwrap_or({default})"
        else:
            expr = f"if {target}.is_some() {{ {target}.unwrap_or({default}) }} else {{ {default} }}"
        src = f"""pub fn {rust_name}(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 {{
    {expr}
}}
"""
        return Variant("axis", src, rust_name)

    raise ValueError(f"unsupported surface for nullish axis: {surface.key}")


def null_presence_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if proposal_id.startswith("axis_null_presence_iflet_"):
        return surface.key == "rust"
    return proposal_id.startswith("axis_null_presence_")


def null_presence_expr(surface: Surface, proposal_id: str, negative: bool, right: bool) -> str:
    target = "other" if right and proposal_id == "axis_null_presence_wrong_value_boundary" else "value"
    nonnull = right and (
        proposal_id == "axis_null_presence_nonnull_boundary"
        or (negative and proposal_id == "axis_null_presence_method_identity")
    )
    method = right and proposal_id == "axis_null_presence_method_identity"

    if surface.key == "python":
        return f"{target} is not None" if nonnull else f"{target} is None"
    if surface.key == "ruby":
        if nonnull:
            return f"!{target}.nil?"
        return f"{target}.nil?" if method else f"{target} == nil"
    if surface.key == "rust":
        if nonnull:
            return f"{target}.is_some()"
        return f"{target}.is_none()" if method else f"{target} == None"
    if surface.key == "go":
        return f"{target} != nil" if nonnull else f"{target} == nil"
    if surface.key == "java":
        return f"{target} != null" if nonnull else f"{target} == null"
    if surface.key == "c":
        return f"{target} != NULL" if nonnull else f"{target} == NULL"
    if surface.key in JS_LIKE_SURFACES:
        return f"{target} != null" if nonnull else f"{target} == null"
    if surface.key == "swift":
        return f"{target} != nil" if nonnull else f"{target} == nil"
    raise ValueError(f"unsupported surface for null presence axis: {surface.key}")


def axis_null_presence_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if proposal_id.startswith("axis_null_presence_iflet_"):
        return axis_null_presence_iflet_variant(surface, proposal_id, negative, right)

    name = "buildCase" if right else "axisCase"
    snake_name = "build_case" if right else "axis_case"
    expr = null_presence_expr(surface, proposal_id, negative, right)

    if surface.language == "javascript":
        body = f"""function {name}(value, other) {{
  return {expr};
}}
"""
        return js_axis_source(surface, body, name)
    if surface.key == "typescript":
        src = f"""function {name}(value: unknown | null | undefined, other: unknown | null | undefined): boolean {{
  return {expr};
}}
"""
        return Variant("axis", src, name)
    if surface.key == "python":
        src = f"""def {snake_name}(value, other):
    return {expr}
"""
        return Variant("axis", src, snake_name)
    if surface.key == "ruby":
        src = f"""def {snake_name}(value, other)
  {expr}
end
"""
        return Variant("axis", src, snake_name)
    if surface.key == "rust":
        src = f"""pub fn {snake_name}(value: Option<i32>, other: Option<i32>) -> bool {{
    {expr}
}}
"""
        return Variant("axis", src, snake_name)
    if surface.key == "go":
        go_name = "BuildCase" if right else "AxisCase"
        src = f"""package p

func {go_name}(value any, other any) bool {{
    return {expr}
}}
"""
        return Variant("axis", src, go_name)
    if surface.key == "java":
        src = f"""class AxisCase {{
    static boolean {name}(Object value, Object other) {{
        return {expr};
    }}
}}
"""
        return Variant("axis", src, name)
    if surface.key == "c":
        src = f"""#include <stddef.h>

int {snake_name}(void *value, void *other) {{
    return {expr};
}}
"""
        return Variant("axis", src, snake_name)
    if surface.key == "swift":
        src = f"""func {name}(_ value: Int?, _ other: Int?) -> Bool {{
    return {expr}
}}
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for null presence axis: {surface.key}")


def axis_null_presence_iflet_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "rust":
        raise ValueError(f"unsupported surface for Rust if-let null presence axis: {surface.key}")
    name = "build_case" if right else "axis_case"
    target = (
        "other" if right and proposal_id == "axis_null_presence_iflet_wrong_value_boundary" else "value"
    )
    if right and (
        proposal_id == "axis_null_presence_iflet_none_boundary"
        or (negative and proposal_id == "axis_null_presence_iflet_some_identity")
    ):
        pattern = "None"
    else:
        pattern = "Some(_)"

    if right and proposal_id == "axis_null_presence_iflet_some_identity" and not negative:
        body = f"{target}.is_some()"
    else:
        body = f"if let {pattern} = {target} {{ true }} else {{ false }}"

    src = f"""pub fn {name}(value: Option<i32>, other: Option<i32>) -> bool {{
    {body}
}}
"""
    return Variant("axis", src, name)

def scalar_abs_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if proposal_id.startswith("axis_scalar_rust_"):
        return surface.key == "rust"
    if proposal_id in {
        "axis_scalar_abs_shadowed_math_boundary",
        "axis_scalar_min_shadowed_math_boundary",
        "axis_scalar_max_shadowed_math_boundary",
    }:
        return surface.key in JS_LIKE_SURFACES
    return surface.key in {
        "python",
        "javascript",
        "typescript",
        "go",
        "java",
        "c",
        "ruby",
        "vue",
        "svelte",
        "html",
    }


def numeric_clamp_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return surface.key == "python"


def hof_filter_map_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return surface.key in {"python", "javascript", "rust"}


def axis_numeric_clamp_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "python":
        raise ValueError(f"unsupported surface for numeric clamp axis: {surface.key}")
    name = "build_case" if right else "axis_case"
    annotation = "float" if proposal_id == "axis_numeric_clamp_float_boundary" else "int"
    guarded = proposal_id not in {"axis_numeric_clamp_unproven_boundary"}
    if proposal_id == "axis_numeric_clamp_swapped_bounds_boundary" and right:
        expr = "min(max(x, hi), lo)"
    elif proposal_id == "axis_numeric_clamp_float_boundary" and right:
        expr = "max(min(x, hi), lo)"
    elif proposal_id == "axis_numeric_clamp_unproven_boundary" and right:
        expr = "max(min(x, hi), lo)"
    elif right and not negative:
        expr = "max(min(x, hi), lo)"
    elif right and negative:
        expr = "min(max(x, hi), lo)"
    else:
        expr = "min(max(x, lo), hi)"
    guard = "    if hi < lo:\n        raise 0\n" if guarded else ""
    src = f"""def {name}(x: {annotation}, lo: {annotation}, hi: {annotation}):
{guard}    return {expr}
"""
    return Variant("axis", src, name)


def axis_hof_filter_map_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key not in {"python", "javascript", "rust"}:
        raise ValueError(f"unsupported surface for filter-map axis: {surface.key}")
    name = "build_case" if right else "axis_case"

    if surface.key == "python":
        if proposal_id == "axis_hof_filter_map_falsey_boundary":
            expr = "0"
        else:
            expr = "x * 2"
        src = f"""def {name}(xs):
    return [{expr} for x in xs if x > 0]
"""
        return Variant("filtered_comprehension", src, name)

    if surface.key == "javascript":
        if proposal_id == "axis_hof_filter_map_falsey_boundary":
            expr = "0"
        else:
            expr = "x * 2"
        src = f"""function {name}(xs) {{
  return xs.filter((x) => x > 0).map((x) => {expr});
}}
"""
        return Variant("filter_map_chain", src, name)

    if proposal_id == "axis_hof_filter_map_none_boundary" and right and negative:
        src = f"""fn {name}(xs: &[i32]) -> Vec<Option<i32>> {{
    xs.iter().copied().map(|x| if x > 0 {{ Some(x * 2) }} else {{ None }}).collect()
}}
"""
        return Variant("option_value_map", src, name)

    if right and negative and proposal_id in {
        "axis_hof_filter_map_identity",
        "axis_hof_filter_map_value_boundary",
    }:
        some_expr = "x * 3"
    elif proposal_id == "axis_hof_filter_map_falsey_boundary":
        some_expr = "0"
    else:
        some_expr = "x * 2"

    chain = f"xs.iter().copied().filter_map(|x| if x > 0 {{ Some({some_expr}) }} else {{ None }})"
    if proposal_id == "axis_hof_filter_map_falsey_boundary" and right and negative:
        chain = f"{chain}.filter(|x| *x != 0)"
    src = f"""fn {name}(xs: &[i32]) -> Vec<i32> {{
    {chain}.collect()
}}
"""
    return Variant("rust_filter_map", src, name)


def axis_scalar_abs_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    name = "buildCase" if right else "axisCase"
    snake_name = "build_case" if right else "axis_case"
    target = (
        "other"
        if right
        and proposal_id
        in {
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_rust_abs_wrong_value_boundary",
        }
        else "value"
    )
    if right and negative and proposal_id in {
        "axis_scalar_abs_function_identity",
        "axis_scalar_abs_sign_boundary",
        "axis_scalar_rust_abs_method_identity",
    }:
        mode = "identity"
    elif right and proposal_id == "axis_scalar_abs_shadowed_math_boundary":
        mode = "shadowed_math"
    elif right and proposal_id == "axis_scalar_rust_abs_custom_method_boundary":
        mode = "custom_method"
    else:
        mode = "builtin" if right else "conditional"

    if surface.language == "javascript":
        if mode == "conditional":
            expr = f"{target} >= 0 ? {target} : -{target}"
        elif mode == "identity":
            expr = target
        elif mode == "shadowed_math":
            body = f"""function {name}(value, other) {{
  const Math = {{ abs: function(_value) {{ return 0; }} }};
  const magnitude = Math.abs({target});
  return magnitude + other;
}}
"""
            return js_axis_source(surface, body, name)
        else:
            expr = f"Math.abs({target})"
        body = f"""function {name}(value, other) {{
  const magnitude = {expr};
  return magnitude + other;
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        if mode == "conditional":
            expr = f"{target} >= 0 ? {target} : -{target}"
        elif mode == "identity":
            expr = target
        elif mode == "shadowed_math":
            src = f"""function {name}(value: number, other: number): number {{
  const Math = {{ abs: function(_value: number): number {{ return 0; }} }};
  const magnitude = Math.abs({target});
  return magnitude + other;
}}
"""
            return Variant("axis", src, name)
        else:
            expr = f"Math.abs({target})"
        src = f"""function {name}(value: number, other: number): number {{
  const magnitude = {expr};
  return magnitude + other;
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        expr = (
            f"{target} if {target} >= 0 else -{target}"
            if mode == "conditional"
            else target
            if mode == "identity"
            else f"abs({target})"
        )
        src = f"""def {snake_name}(value, other):
    magnitude = {expr}
    return magnitude + other
"""
        return Variant("axis", src, snake_name)

    if surface.key == "ruby":
        expr = (
            f"{target} >= 0 ? {target} : -{target}"
            if mode == "conditional"
            else target
            if mode == "identity"
            else f"{target}.abs"
        )
        src = f"""def {snake_name}(value, other)
  magnitude = {expr}
  magnitude + other
end
"""
        return Variant("axis", src, snake_name)

    if surface.key == "go":
        go_name = "BuildCase" if right else "AxisCase"
        if mode == "conditional":
            body = f"""magnitude := {target}
    if {target} < 0 {{
        magnitude = -{target}
    }}
    return magnitude + other"""
        elif mode == "identity":
            body = f"""magnitude := {target}
    return magnitude + other"""
        else:
            body = f"""magnitude := math.Abs({target})
    return magnitude + other"""
        src = f"""package p

import "math"

func {go_name}(value float64, other float64) float64 {{
    {body}
}}
"""
        return Variant("axis", src, go_name)

    if surface.key == "java":
        if mode == "conditional":
            expr = f"{target} >= 0 ? {target} : -{target}"
        elif mode == "identity":
            expr = target
        else:
            expr = f"Math.abs({target})"
        src = f"""class AxisCase {{
    static int {name}(int value, int other) {{
        int magnitude = {expr};
        return magnitude + other;
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "c":
        if mode == "conditional":
            expr = f"{target} >= 0 ? {target} : -{target}"
        elif mode == "identity":
            expr = target
        else:
            expr = f"abs({target})"
        src = f"""#include <stdlib.h>

int {snake_name}(int value, int other) {{
    int magnitude = {expr};
    return magnitude + other;
}}
"""
        return Variant("axis", src, snake_name)

    if surface.key == "rust":
        if mode == "custom_method":
            src = f"""struct Wrap(i64);

impl Wrap {{
    fn abs(&self) -> i64 {{
        0
    }}
}}

pub fn {snake_name}(value: Wrap) -> i64 {{
    let magnitude = value.abs();
    magnitude + 1
}}
"""
            return Variant("axis", src, snake_name)
        if mode == "conditional":
            expr = f"if {target} >= 0 {{ {target} }} else {{ -{target} }}"
        elif mode == "identity":
            expr = target
        else:
            expr = f"{target}.abs()"
        src = f"""pub fn {snake_name}(value: i64, other: i64) -> i64 {{
    let magnitude = {expr};
    magnitude + other
}}
"""
        return Variant("axis", src, snake_name)

    raise ValueError(f"unsupported surface for scalar abs axis: {surface.key}")


def scalar_minmax_op(proposal_id: str) -> str:
    if "_max_" in proposal_id:
        return "max"
    return "min"


def axis_scalar_minmax_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    name = "buildCase" if right else "axisCase"
    snake_name = "build_case" if right else "axis_case"
    op = scalar_minmax_op(proposal_id)
    if right and negative and proposal_id in {
        "axis_scalar_min_function_identity",
        "axis_scalar_max_function_identity",
        "axis_scalar_rust_min_method_identity",
        "axis_scalar_rust_max_method_identity",
    }:
        op = "max" if op == "min" else "min"
    wrong_value = right and proposal_id in {
        "axis_scalar_min_wrong_value_boundary",
        "axis_scalar_max_wrong_value_boundary",
        "axis_scalar_rust_min_wrong_value_boundary",
        "axis_scalar_rust_max_wrong_value_boundary",
    }
    shadowed_math = right and proposal_id in {
        "axis_scalar_min_shadowed_math_boundary",
        "axis_scalar_max_shadowed_math_boundary",
    }
    custom_method = right and proposal_id in {
        "axis_scalar_rust_min_custom_method_boundary",
        "axis_scalar_rust_max_custom_method_boundary",
    }
    a = "left"
    b = "other" if wrong_value else "right"
    cmp = "<=" if op == "min" else ">="

    if surface.language == "javascript":
        if shadowed_math:
            body = f"""function {name}(left, right, other) {{
  const Math = {{ {op}: function(_left, _right) {{ return 0; }} }};
  const selected = Math.{op}({a}, {b});
  return selected + other;
}}
"""
            return js_axis_source(surface, body, name)
        expr = f"{a} {cmp} {b} ? {a} : {b}" if not right else f"Math.{op}({a}, {b})"
        body = f"""function {name}(left, right, other) {{
  const selected = {expr};
  return selected + other;
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        if shadowed_math:
            src = f"""function {name}(left: number, right: number, other: number): number {{
  const Math = {{ {op}: function(_left: number, _right: number): number {{ return 0; }} }};
  const selected = Math.{op}({a}, {b});
  return selected + other;
}}
"""
            return Variant("axis", src, name)
        expr = f"{a} {cmp} {b} ? {a} : {b}" if not right else f"Math.{op}({a}, {b})"
        src = f"""function {name}(left: number, right: number, other: number): number {{
  const selected = {expr};
  return selected + other;
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        expr = f"{a} if {a} {cmp} {b} else {b}" if not right else f"{op}({a}, {b})"
        src = f"""def {snake_name}(left, right, other):
    selected = {expr}
    return selected + other
"""
        return Variant("axis", src, snake_name)

    if surface.key == "ruby":
        expr = f"{a} {cmp} {b} ? {a} : {b}" if not right else f"[{a}, {b}].{op}"
        src = f"""def {snake_name}(left, right, other)
  selected = {expr}
  selected + other
end
"""
        return Variant("axis", src, snake_name)

    if surface.key == "go":
        go_name = "BuildCase" if right else "AxisCase"
        if right:
            expr = f"math.{op.capitalize()}({a}, {b})"
            body = f"""selected := {expr}
    return selected + other"""
        else:
            body = f"""selected := {a}
    if {b} {cmp} {a} {{
        selected = {b}
    }}
    return selected + other"""
        src = f"""package p

import "math"

func {go_name}(left float64, right float64, other float64) float64 {{
    {body}
}}
"""
        return Variant("axis", src, go_name)

    if surface.key == "java":
        expr = f"{a} {cmp} {b} ? {a} : {b}" if not right else f"Math.{op}({a}, {b})"
        src = f"""class AxisCase {{
    static int {name}(int left, int right, int other) {{
        int selected = {expr};
        return selected + other;
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "c":
        fn = "fmin" if op == "min" else "fmax"
        expr = f"{a} {cmp} {b} ? {a} : {b}" if not right else f"{fn}({a}, {b})"
        src = f"""#include <math.h>

double {snake_name}(double left, double right, double other) {{
    double selected = {expr};
    return selected + other;
}}
"""
        return Variant("axis", src, snake_name)

    if surface.key == "rust":
        if custom_method:
            src = f"""struct Wrap(i64);

impl Wrap {{
    fn {op}(&self, _right: i64) -> i64 {{
        0
    }}
}}

pub fn {snake_name}(left: Wrap, right: i64, other: i64) -> i64 {{
    let selected = left.{op}(right);
    selected + other
}}
"""
            return Variant("axis", src, snake_name)
        expr = f"if {a} {cmp} {b} {{ {a} }} else {{ {b} }}" if not right else f"{a}.{op}({b})"
        src = f"""pub fn {snake_name}(left: i64, right: i64, other: i64) -> i64 {{
    let selected = {expr};
    selected + other
}}
"""
        return Variant("axis", src, snake_name)

    raise ValueError(f"unsupported surface for scalar min/max axis: {surface.key}")


def total_order_compare_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_total_order_compare_") and surface.key == "c"


def axis_total_order_compare_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "c":
        raise ValueError(f"unsupported surface for total-order comparator axis: {surface.key}")
    snake_name = "build_case" if right else "axis_case"
    mode = "less_first"
    if right and proposal_id == "axis_total_order_compare_guard_order_identity" and not negative:
        mode = "greater_first"
    elif right and proposal_id == "axis_total_order_compare_ternary_identity" and not negative:
        mode = "ternary"
    elif right and proposal_id == "axis_total_order_compare_descending_boundary":
        mode = "descending"
    elif right and proposal_id == "axis_total_order_compare_equal_boundary":
        mode = "equal_as_less"
    elif right and (
        negative
        or proposal_id == "axis_total_order_compare_wrong_value_boundary"
    ):
        mode = "wrong_value"

    if mode == "less_first":
        body = """    if (left < right)
        return -1;
    if (left > right)
        return 1;
    return 0;"""
    elif mode == "greater_first":
        body = """    if (left > right)
        return 1;
    if (left < right)
        return -1;
    return 0;"""
    elif mode == "ternary":
        body = "    return left > right ? 1 : left < right ? -1 : 0;"
    elif mode == "descending":
        body = """    if (left < right)
        return 1;
    if (left > right)
        return -1;
    return 0;"""
    elif mode == "equal_as_less":
        body = """    if (left <= right)
        return -1;
    if (left > right)
        return 1;
    return 0;"""
    elif mode == "wrong_value":
        body = """    if (left < right)
        return -1;
    if (left > right)
        return 2;
    return 0;"""
    else:
        raise ValueError(f"unknown total-order comparator mode: {mode}")

    src = f"""int {snake_name}(const void *a, const void *b) {{
    const int left = *(const int *)a;
    const int right = *(const int *)b;
{body}
}}
"""
    return Variant("axis", src, snake_name)


def java_dead_loop_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_java_dead_loop_") and surface.key == "java"


def java_low_bit_toggle_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_java_low_bit_toggle_") and surface.key == "java"


def c_u16_be_byte_pack_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_c_u16_be_byte_pack_") and surface.key == "c"


def c_u32_be_byte_pack_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_c_u32_be_byte_pack_") and surface.key == "c"


def axis_java_dead_loop_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "java":
        raise ValueError(f"unsupported surface for Java dead-loop axis: {surface.key}")
    name = "buildCase" if right else "axisCase"
    mode = "exact_dead"
    if right and proposal_id == "axis_java_dead_loop_guard_identity" and not negative:
        mode = "epsilon_dead"
    elif right and proposal_id == "axis_java_dead_loop_guard_identity" and negative:
        mode = "wrong_return"
    elif right and proposal_id == "axis_java_dead_loop_false_init_boundary":
        mode = "false_init"
    elif right and proposal_id == "axis_java_dead_loop_positive_guard_boundary":
        mode = "positive_guard"
    elif right and proposal_id == "axis_java_dead_loop_reassigned_guard_boundary":
        mode = "reassigned_guard"

    params = "float[] vertex, int strideInBytes, float[] vertices, int numVertices"
    body = "if (vertices[offset + j] != vertex[j]) found = false;"
    found_setup = "boolean found = true;"
    guard = "!found && j < size"
    return_expr = "(long)i"
    if mode == "epsilon_dead":
        params += ", float epsilon"
        body = """if ((vertices[offset + j] > vertex[j]
                    ? vertices[offset + j] - vertex[j]
                    : vertex[j] - vertices[offset + j]) > epsilon) found = false;"""
    elif mode == "wrong_return":
        return_expr = "(long)i + 1"
    elif mode == "false_init":
        found_setup = "boolean found = false;"
        body = "if (vertices[offset + j] == vertex[j]) found = true;"
    elif mode == "positive_guard":
        guard = "found && j < size"
    elif mode == "reassigned_guard":
        found_setup = "boolean found = true;\n            found = vertices == vertex;"

    src = f"""class C {{
    static long {name}({params}) {{
        final int size = strideInBytes / 4;
        for (int i = 0; i < numVertices; i++) {{
            final int offset = i * size;
            {found_setup}
            for (int j = 0; {guard}; j++)
                {body}
            if (found) return {return_expr};
        }}
        return -1;
    }}
}}
"""
    return Variant("axis", src, name)


def axis_java_low_bit_toggle_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "java":
        raise ValueError(f"unsupported surface for Java low-bit toggle axis: {surface.key}")
    name = "reverseEdgeKey" if right else "getPosOfReverseEdge"
    expr = "edgeId % 2 == 0 ? edgeId + 1 : edgeId - 1"
    param = "edgeId"
    if right:
        param = "edgeKey"
        if proposal_id == "axis_java_low_bit_toggle_even_identity" and not negative:
            expr = "edgeKey ^ 1"
        elif proposal_id == "axis_java_low_bit_toggle_odd_identity" and not negative:
            expr = "edgeKey % 2 != 0 ? edgeKey - 1 : edgeKey + 1"
        elif proposal_id == "axis_java_low_bit_toggle_even_identity" and negative:
            expr = "edgeKey ^ 2"
        elif proposal_id == "axis_java_low_bit_toggle_odd_identity" and negative:
            expr = "edgeKey % 2 == 0 ? edgeKey - 1 : edgeKey + 1"
        elif proposal_id == "axis_java_low_bit_toggle_reversed_branch_boundary":
            expr = "edgeKey % 2 == 0 ? edgeKey - 1 : edgeKey + 1"
        elif proposal_id == "axis_java_low_bit_toggle_xor_two_boundary":
            expr = "edgeKey ^ 2"
        elif proposal_id == "axis_java_low_bit_toggle_positive_one_boundary":
            expr = "edgeKey % 2 == 1 ? edgeKey - 1 : edgeKey + 1"
        elif proposal_id == "axis_java_low_bit_toggle_wrong_delta_boundary":
            expr = "edgeKey % 2 == 0 ? edgeKey + 1 : edgeKey - 2"

    src = f"""class C {{
    static int {name}(int {param}) {{
        return {expr};
    }}
}}
"""
    return Variant("axis", src, name)


def axis_c_u16_be_byte_pack_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "c":
        raise ValueError(f"unsupported surface for C byte-pack axis: {surface.key}")
    name = "build_case" if right else "axis_case"
    typedef = "typedef unsigned char u8;\n"
    param = "const u8 *a"
    expr = "(((unsigned int)a[0]) << 8) + ((unsigned int)a[1])"
    if right:
        param = "unsigned char *a"
        expr = "(a[0] << 8) | a[1]"
        typedef = ""
        if proposal_id == "axis_c_u16_be_byte_pack_uint8_identity" and not negative:
            param = "const uint8_t *a"
        elif proposal_id == "axis_c_u16_be_byte_pack_uncasted_add_identity" and not negative:
            typedef = "typedef unsigned char u8;\n"
            param = "u8 *a"
            expr = "(a[0] << 8) + a[1]"
        elif proposal_id == "axis_c_u16_be_byte_pack_wrong_order_boundary":
            expr = "(a[1] << 8) | a[0]"
        elif proposal_id == "axis_c_u16_be_byte_pack_overlap_boundary":
            expr = "(a[0] << 4) | a[1]"
        elif (
            negative
            or proposal_id == "axis_c_u16_be_byte_pack_wrong_byte_boundary"
        ):
            expr = "(a[0] << 8) | a[2]"
        elif proposal_id == "axis_c_u16_be_byte_pack_unproven_alias_boundary":
            typedef = "typedef unsigned short u8;\n"
            param = "const u8 *a"

    src = f"""{typedef}unsigned int {name}({param}) {{
    return {expr};
}}
"""
    return Variant("axis", src, name)


def axis_c_u32_be_byte_pack_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    if surface.key != "c":
        raise ValueError(f"unsupported surface for C u32 byte-pack axis: {surface.key}")
    name = "build_case" if right else "axis_case"
    typedef = "typedef unsigned char u8;\ntypedef unsigned int u32;\n"
    param = "const u8 *a"
    expr = "(((u32)a[0]) << 24) + (((u32)a[1]) << 16) + (((u32)a[2]) << 8) + ((u32)a[3])"
    if right:
        expr = "((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3])"
        if proposal_id == "axis_c_u32_be_byte_pack_unsigned_int_identity" and not negative:
            typedef = ""
            param = "unsigned char *a"
            expr = "((unsigned int)a[0] << 24) + ((unsigned int)a[1] << 16) + ((unsigned int)a[2] << 8) + (unsigned int)a[3]"
        elif proposal_id == "axis_c_u32_be_byte_pack_uint8_identity" and not negative:
            typedef = ""
            param = "const uint8_t *a"
            expr = "((uint32_t)a[0] << 24) | ((uint32_t)a[1] << 16) | ((uint32_t)a[2] << 8) | ((uint32_t)a[3])"
        elif proposal_id == "axis_c_u32_be_byte_pack_uncasted_high_boundary":
            expr = "(a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3]"
        elif proposal_id == "axis_c_u32_be_byte_pack_wrong_order_boundary":
            expr = "((u32)a[1] << 24) | ((u32)a[0] << 16) | ((u32)a[2] << 8) | ((u32)a[3])"
        elif proposal_id == "axis_c_u32_be_byte_pack_wrong_byte_boundary":
            expr = "((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[3] << 8) | ((u32)a[2])"
        elif proposal_id == "axis_c_u32_be_byte_pack_wrong_alias_boundary":
            typedef = "typedef unsigned char u8;\ntypedef signed int u32;\n"
        elif negative:
            expr = "((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[3] << 8) | ((u32)a[2])"

    src = f"""{typedef}unsigned int {name}({param}) {{
    return {expr};
}}
"""
    return Variant("axis", src, name)
