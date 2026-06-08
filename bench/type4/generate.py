#!/usr/bin/env python3
"""Generate the seed corpus for the evidence-carrying Type-4 benchmark factory."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path

from type4gen.aggregate import (
    EMITTERS,
    evidence_contract_negative,
    evidence_negative,
    evidence_positive,
)
from type4gen.axis_proposals import AXIS_PROPOSALS
from type4gen.model import (
    JS_LIKE_SURFACES,
    OPERATIONS,
    PROPERTY_INPUTS,
    REQUIRED_BUDGET_FIELDS,
    REQUIRED_PROPOSAL_FIELDS,
    SEMANTIC_SCOPE,
    SURFACES,
    GenerationFilter,
    Surface,
    Variant,
    capability_exact_supported,
    js_axis_source,
    load_capabilities,
    snake_to_camel,
    snake_to_pascal,
    surface_capability,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROPOSALS = ROOT / "bench" / "type4" / "proposals.v1.json"
DEFAULT_CAPABILITIES = ROOT / "bench" / "type4" / "capabilities.v1.json"

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


def literal_membership_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_membership_"):
        return False
    if proposal_id in {
        "axis_membership_typed_receiver_identity",
        "axis_membership_typed_wrong_element_boundary",
    }:
        return surface.key in {"python", "typescript", "go", "rust", "java"}
    if proposal_id == "axis_membership_typed_string_boundary":
        return surface.key in {"typescript", "rust", "java"}
    if proposal_id == "axis_membership_unproven_receiver_boundary":
        return surface.key in {"java", "rust", "typescript"}
    if proposal_id == "axis_membership_typefact_python_tuple_identity":
        return surface.key == "python"
    if proposal_id == "axis_membership_typefact_java_queue_identity":
        return surface.key == "java"
    if proposal_id == "axis_membership_typefact_rust_vecdeque_identity":
        return surface.key == "rust"
    if proposal_id.startswith("axis_membership_python_"):
        return surface.key == "python"
    if proposal_id.startswith("axis_membership_local_"):
        return surface.key in {"go", "java", "rust"}
    if proposal_id.startswith("axis_membership_set_"):
        return surface.key in {"python", "javascript", "typescript", "go", "rust", "ruby"}
    if proposal_id.startswith("axis_membership_array_some_"):
        return surface.key in JS_LIKE_SURFACES
    if proposal_id.startswith("axis_membership_array_every_"):
        return surface.key in JS_LIKE_SURFACES
    if proposal_id.startswith("axis_membership_array_indexof_"):
        return surface.key in JS_LIKE_SURFACES
    if proposal_id.startswith("axis_membership_array_findindex_"):
        return surface.key in JS_LIKE_SURFACES
    if proposal_id.startswith("axis_membership_array_filter_length_"):
        return surface.key in JS_LIKE_SURFACES
    if proposal_id.startswith("axis_membership_java_"):
        return surface.key == "java"
    if proposal_id.startswith("axis_membership_module_"):
        return surface.key in {"python", "ruby", "javascript", "typescript", "java"}
    if proposal_id.startswith("axis_membership_go_slices_"):
        return surface.key in {"python", "ruby", "go"}
    if proposal_id.startswith("axis_membership_rust_local_"):
        return surface.key in {"python", "ruby", "rust"}
    if proposal_id.startswith("axis_membership_rust_std_"):
        return surface.key in {"python", "ruby", "rust"}
    if proposal_id.startswith("axis_membership_ruby_set_"):
        return surface.key == "ruby"
    return surface.key in {
        "python",
        "javascript",
        "typescript",
        "go",
        "rust",
        "ruby",
        "vue",
        "svelte",
        "html",
    }


def membership_axis_parts(
    proposal_id: str, negative: bool, right: bool
) -> tuple[str, tuple[str, str], str]:
    element = "value"
    items = ("red", "blue")
    form = "membership"

    if right and proposal_id == "axis_membership_wrong_element_boundary":
        element = "other"
    if right and proposal_id == "axis_membership_wrong_collection_boundary":
        items = ("green", "blue")
    if right and proposal_id == "axis_membership_substring_boundary":
        form = "substring"
    if proposal_id == "axis_membership_unproven_receiver_boundary":
        form = "unproven_receiver" if right else "dynamic_collection"
    if proposal_id.startswith("axis_membership_typed_"):
        form = "typed_membership"
    if right and negative and proposal_id == "axis_membership_typed_receiver_identity":
        element = "other"
    if right and proposal_id == "axis_membership_typed_wrong_element_boundary":
        element = "other"
    if right and proposal_id == "axis_membership_typed_string_boundary":
        form = "unproven_receiver"
    if right and negative and proposal_id == "axis_membership_literal_identity":
        items = ("green", "blue")
    if proposal_id == "axis_membership_set_param_identity":
        form = "set_param" if right else "typed_membership"
    if proposal_id == "axis_membership_typefact_python_tuple_identity":
        form = "python_tuple_param" if right else "typed_membership"
    if proposal_id == "axis_membership_typefact_java_queue_identity":
        form = "java_queue_param" if right else "typed_membership"
    if proposal_id == "axis_membership_typefact_rust_vecdeque_identity":
        form = "rust_vecdeque_param" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_sequence_identity":
        form = "python_alias_sequence" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_container_identity":
        form = "python_alias_container" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_set_identity":
        form = "python_alias_set" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_wrong_element_boundary":
        form = "python_alias_sequence" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_wrong_receiver_boundary":
        form = "python_alias_wrong_receiver" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_unresolved_boundary":
        form = "python_alias_unresolved" if right else "typed_membership"
    if proposal_id == "axis_membership_python_alias_shadowed_boundary":
        form = "python_alias_shadowed" if right else "typed_membership"
    if proposal_id == "axis_membership_python_set_factory_identity":
        form = "python_set_factory" if right else "membership"
    if proposal_id == "axis_membership_python_tuple_factory_identity":
        form = "python_tuple_factory" if right else "membership"
    if proposal_id == "axis_membership_python_frozenset_factory_identity":
        form = "python_frozenset_factory" if right else "membership"
    if proposal_id == "axis_membership_python_deque_import_identity":
        form = "python_deque_import" if right else "membership"
    if proposal_id == "axis_membership_python_deque_alias_identity":
        form = "python_deque_alias" if right else "membership"
    if proposal_id == "axis_membership_python_deque_namespace_identity":
        form = "python_deque_namespace" if right else "membership"
    if proposal_id in {
        "axis_membership_python_deque_wrong_element_boundary",
        "axis_membership_python_deque_wrong_collection_boundary",
    }:
        form = "python_deque_import" if right else "membership"
    if proposal_id == "axis_membership_python_deque_missing_import_boundary":
        form = "python_deque_missing_import" if right else "membership"
    if proposal_id == "axis_membership_python_deque_shadowed_boundary":
        form = "python_deque_shadowed" if right else "membership"
    if proposal_id == "axis_membership_python_deque_mutated_boundary":
        form = "python_deque_mutated" if right else "membership"
    if proposal_id in {
        "axis_membership_python_factory_wrong_element_boundary",
        "axis_membership_python_factory_wrong_collection_boundary",
    }:
        form = "python_set_factory" if right else "membership"
    if proposal_id == "axis_membership_python_factory_shadowed_boundary":
        form = "python_set_factory_shadowed" if right else "membership"
    if proposal_id == "axis_membership_local_go_slice_identity":
        form = "go_local_slice" if right else "membership"
    if proposal_id == "axis_membership_local_java_list_identity":
        form = "java_local_list" if right else "membership"
    if proposal_id == "axis_membership_local_rust_vec_identity":
        form = "rust_local_vec" if right else "membership"
    if proposal_id in {
        "axis_membership_local_wrong_element_boundary",
        "axis_membership_local_wrong_collection_boundary",
    }:
        form = "local_constructed" if right else "membership"
    if proposal_id == "axis_membership_local_mutated_boundary":
        form = "local_constructed_mutated" if right else "membership"
    if proposal_id in {
        "axis_membership_set_inline_identity",
        "axis_membership_set_wrong_element_boundary",
        "axis_membership_set_wrong_collection_boundary",
    }:
        form = "set_inline" if right else "membership"
    if proposal_id == "axis_membership_set_local_identity":
        form = "set_local" if right else "membership"
    if proposal_id == "axis_membership_set_untyped_receiver_boundary":
        form = "set_untyped" if right else "membership"
    if proposal_id in {
        "axis_membership_array_some_identity",
        "axis_membership_array_some_wrong_element_boundary",
        "axis_membership_array_some_wrong_collection_boundary",
    }:
        form = "array_some" if right else "membership"
    if proposal_id in {
        "axis_membership_array_every_absence_identity",
        "axis_membership_array_every_wrong_element_boundary",
        "axis_membership_array_every_wrong_collection_boundary",
    }:
        form = "array_every_absence" if right else "membership_absence"
    if proposal_id in {
        "axis_membership_array_indexof_identity",
        "axis_membership_array_indexof_wrong_element_boundary",
        "axis_membership_array_indexof_wrong_collection_boundary",
    }:
        form = "array_indexof" if right else "membership"
    if proposal_id in {
        "axis_membership_array_findindex_identity",
        "axis_membership_array_findindex_wrong_element_boundary",
        "axis_membership_array_findindex_wrong_collection_boundary",
    }:
        form = "array_findindex" if right else "membership"
    if proposal_id in {
        "axis_membership_array_filter_length_identity",
        "axis_membership_array_filter_length_wrong_element_boundary",
        "axis_membership_array_filter_length_wrong_collection_boundary",
    }:
        form = "array_filter_length" if right else "membership"
    if proposal_id in {
        "axis_membership_array_filter_length_absence_identity",
        "axis_membership_array_filter_length_absence_wrong_element_boundary",
        "axis_membership_array_filter_length_absence_wrong_collection_boundary",
    }:
        form = "array_filter_length_absence" if right else "membership_absence"
    if proposal_id.startswith("axis_membership_java_"):
        form = "java_list_of"
        if "_set_of_" in proposal_id:
            form = "java_set_of"
        elif "_arrays_aslist_" in proposal_id:
            form = "java_arrays_aslist"
    if proposal_id == "axis_membership_module_js_set_identity":
        form = "module_set" if right else "membership"
    if proposal_id == "axis_membership_module_ts_set_identity":
        form = "module_set" if right else "membership"
    if proposal_id == "axis_membership_module_java_list_identity":
        form = "java_module_list" if right else "membership"
    if proposal_id == "axis_membership_module_python_tuple_identity":
        form = "python_module_tuple" if right else "membership"
    if proposal_id == "axis_membership_module_python_set_identity":
        form = "python_module_set" if right else "membership"
    if proposal_id == "axis_membership_module_python_mutated_boundary":
        form = "python_module_mutated" if right else "membership"
    if proposal_id in {
        "axis_membership_module_wrong_element_boundary",
        "axis_membership_module_wrong_collection_boundary",
    }:
        form = "module_collection" if right else "membership"
    if proposal_id == "axis_membership_module_mutated_boundary":
        form = "module_set_mutated" if right else "membership"
    if proposal_id == "axis_membership_module_shadowed_boundary":
        form = "module_collection" if right else "membership"
    if proposal_id == "axis_membership_go_slices_package_identity":
        form = "go_slices_package" if right else "membership"
    if proposal_id == "axis_membership_go_slices_alias_package_identity":
        form = "go_slices_alias_package" if right else "membership"
    if proposal_id == "axis_membership_go_slices_const_package_identity":
        form = "go_slices_const_package" if right else "membership"
    if proposal_id in {
        "axis_membership_go_slices_wrong_element_boundary",
        "axis_membership_go_slices_wrong_collection_boundary",
    }:
        form = "go_slices_package" if right else "membership"
    if proposal_id == "axis_membership_go_slices_mutated_boundary":
        form = "go_slices_mutated" if right else "membership"
    if proposal_id == "axis_membership_go_slices_unimported_boundary":
        form = "go_slices_unimported" if right else "membership"
    if proposal_id == "axis_membership_rust_local_array_identity":
        form = "rust_local_array" if right else "membership"
    if proposal_id == "axis_membership_rust_local_typed_array_identity":
        form = "rust_local_typed_array" if right else "membership"
    if proposal_id == "axis_membership_rust_local_slice_ref_identity":
        form = "rust_local_slice_ref" if right else "membership"
    if proposal_id == "axis_membership_rust_std_hashset_identity":
        form = "rust_std_hashset" if right else "membership"
    if proposal_id == "axis_membership_rust_std_btreeset_identity":
        form = "rust_std_btreeset" if right else "membership"
    if proposal_id == "axis_membership_rust_std_vecdeque_identity":
        form = "rust_std_vecdeque" if right else "membership"
    if proposal_id in {
        "axis_membership_rust_local_wrong_element_boundary",
        "axis_membership_rust_local_wrong_collection_boundary",
    }:
        form = "rust_local_array" if right else "membership"
    if proposal_id == "axis_membership_rust_local_mutated_boundary":
        form = "rust_local_mutated" if right else "membership"
    if proposal_id == "axis_membership_rust_local_custom_receiver_boundary":
        form = "rust_local_custom_receiver" if right else "membership"
    if proposal_id in {
        "axis_membership_rust_std_wrong_element_boundary",
        "axis_membership_rust_std_wrong_collection_boundary",
    }:
        form = "rust_std_hashset" if right else "membership"
    if proposal_id == "axis_membership_rust_std_mutated_boundary":
        form = "rust_std_hashset_mutated" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_new_include_identity":
        form = "ruby_set_new_include" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_new_member_identity":
        form = "ruby_set_new_member" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_local_identity":
        form = "ruby_set_local" if right else "membership"
    if proposal_id in {
        "axis_membership_ruby_set_wrong_element_boundary",
        "axis_membership_ruby_set_wrong_collection_boundary",
    }:
        form = "ruby_set_new_include" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_missing_require_boundary":
        form = "ruby_set_missing_require" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_shadowed_boundary":
        form = "ruby_set_shadowed" if right else "membership"
    if proposal_id == "axis_membership_ruby_set_mutated_boundary":
        form = "ruby_set_mutated" if right else "membership"
    if right and negative and proposal_id in {
        "axis_membership_set_param_identity",
        "axis_membership_set_inline_identity",
        "axis_membership_set_local_identity",
        "axis_membership_array_some_identity",
        "axis_membership_array_every_absence_identity",
        "axis_membership_array_indexof_identity",
        "axis_membership_array_findindex_identity",
        "axis_membership_array_filter_length_identity",
        "axis_membership_array_filter_length_absence_identity",
        "axis_membership_java_list_of_identity",
        "axis_membership_java_set_of_identity",
        "axis_membership_java_arrays_aslist_identity",
        "axis_membership_module_js_set_identity",
        "axis_membership_module_ts_set_identity",
        "axis_membership_module_java_list_identity",
        "axis_membership_module_python_tuple_identity",
        "axis_membership_module_python_set_identity",
        "axis_membership_go_slices_package_identity",
        "axis_membership_go_slices_alias_package_identity",
        "axis_membership_go_slices_const_package_identity",
        "axis_membership_rust_local_array_identity",
        "axis_membership_rust_local_typed_array_identity",
        "axis_membership_rust_local_slice_ref_identity",
        "axis_membership_rust_std_hashset_identity",
        "axis_membership_rust_std_btreeset_identity",
        "axis_membership_rust_std_vecdeque_identity",
        "axis_membership_ruby_set_new_include_identity",
        "axis_membership_ruby_set_new_member_identity",
        "axis_membership_ruby_set_local_identity",
        "axis_membership_typefact_python_tuple_identity",
        "axis_membership_python_alias_sequence_identity",
        "axis_membership_python_alias_container_identity",
        "axis_membership_python_alias_set_identity",
        "axis_membership_typefact_java_queue_identity",
        "axis_membership_typefact_rust_vecdeque_identity",
        "axis_membership_python_set_factory_identity",
        "axis_membership_python_tuple_factory_identity",
        "axis_membership_python_frozenset_factory_identity",
        "axis_membership_python_deque_import_identity",
        "axis_membership_python_deque_alias_identity",
        "axis_membership_python_deque_namespace_identity",
        "axis_membership_local_go_slice_identity",
        "axis_membership_local_java_list_identity",
        "axis_membership_local_rust_vec_identity",
    }:
        element = "other"
    if right and proposal_id == "axis_membership_set_wrong_element_boundary":
        element = "other"
    if right and proposal_id == "axis_membership_set_wrong_collection_boundary":
        items = ("green", "blue")
    if right and proposal_id.endswith("_wrong_element_boundary"):
        element = "other"
    if right and proposal_id.endswith("_wrong_collection_boundary"):
        items = ("green", "blue")
    if (
        right
        and proposal_id.endswith("_shadowed_boundary")
        and not form.startswith(("python_", "ruby_"))
    ):
        form = f"{form}_shadowed"
    return element, items, form


def axis_membership_literal_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    element, items, form = membership_axis_parts(proposal_id, negative, right)
    if form == "python_tuple_param" and surface.key != "python":
        form = "typed_membership"
    if form == "java_queue_param" and surface.key != "java":
        form = "typed_membership"
    if form == "rust_vecdeque_param" and surface.key != "rust":
        form = "typed_membership"
    if form.startswith("python_") and surface.key != "python":
        form = "membership"
    if form.startswith("ruby_") and surface.key != "ruby":
        form = "membership"
    if form == "local_constructed":
        form = {
            "go": "go_local_slice",
            "java": "java_local_list",
            "rust": "rust_local_vec",
        }.get(surface.key, "membership")
    if form == "local_constructed_mutated":
        form = {
            "go": "go_local_slice_mutated",
            "java": "java_local_list_mutated",
            "rust": "rust_local_vec_mutated",
        }.get(surface.key, "membership")
    name = {
        "javascript": "buildCase" if right else "axisCase",
        "typescript": "buildCase" if right else "axisCase",
        "go": "BuildCase" if right else "AxisCase",
    }.get(surface.language, "build_case" if right else "axis_case")
    left, right_item = items

    if surface.language == "javascript":
        if form == "module_collection":
            form = "module_set"
        if form == "module_collection_shadowed":
            form = "module_set_shadowed"
        if form == "set_inline":
            expr = f'new Set(["{left}", "{right_item}"]).has({element})'
        elif form == "set_local":
            body = f"""function {name}(value, other) {{
  const values = new Set(["{left}", "{right_item}"]);
  return values.has({element});
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "module_set":
            body = f"""const VALUES = new Set(["{left}", "{right_item}"]);

function {name}(value, other) {{
  return VALUES.has({element});
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "module_set_mutated":
            body = f"""const VALUES = new Set(["{left}", "{right_item}"]);
VALUES.add("green");

function {name}(value, other) {{
  return VALUES.has(value);
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "module_set_shadowed":
            body = f"""const Set = function(_values) {{
  return {{ has: function() {{ return false; }} }};
}};
const VALUES = new Set(["{left}", "{right_item}"]);

function {name}(value, other) {{
  return VALUES.has({element});
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "set_untyped":
            body = f"""function {name}(values, value, other) {{
  return values.has(value);
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "array_some":
            body = f"""function {name}(value, other) {{
  return ["{left}", "{right_item}"].some((item) => item === {element});
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "array_every_absence":
            body = f"""function {name}(value, other) {{
  return ["{left}", "{right_item}"].every((item) => item !== {element});
}}
"""
            return js_axis_source(surface, body, name)
        elif form == "array_indexof":
            if surface.key in {"vue", "svelte"}:
                expr = f'["{left}", "{right_item}"].indexOf({element}) >= 0'
            elif surface.key == "html":
                expr = f'["{left}", "{right_item}"].indexOf({element}) > -1'
            else:
                expr = f'["{left}", "{right_item}"].indexOf({element}) !== -1'
        elif form == "array_findindex":
            if surface.key in {"vue", "svelte"}:
                expr = f'["{left}", "{right_item}"].findIndex((item) => item === {element}) >= 0'
            elif surface.key == "html":
                expr = f'["{left}", "{right_item}"].findIndex((item) => item === {element}) > -1'
            else:
                expr = f'["{left}", "{right_item}"].findIndex((item) => item === {element}) !== -1'
        elif form == "array_filter_length":
            if surface.key in {"vue", "svelte"}:
                expr = f'["{left}", "{right_item}"].filter((item) => item === {element}).length > 0'
            elif surface.key == "html":
                expr = f'0 < ["{left}", "{right_item}"].filter((item) => item === {element}).length'
            else:
                expr = f'["{left}", "{right_item}"].filter((item) => item === {element}).length !== 0'
        elif form == "array_filter_length_absence":
            if surface.key in {"vue", "svelte"}:
                expr = f'["{left}", "{right_item}"].filter((item) => item === {element}).length < 1'
            elif surface.key == "html":
                expr = f'0 === ["{left}", "{right_item}"].filter((item) => item === {element}).length'
            else:
                expr = f'["{left}", "{right_item}"].filter((item) => item === {element}).length === 0'
        elif form == "membership_absence":
            expr = f'!["{left}", "{right_item}"].includes({element})'
        elif form == "substring":
            expr = f'{element}.includes("{left}")'
        else:
            expr = f'["{left}", "{right_item}"].includes({element})'
        body = f"""function {name}(value, other) {{
  return {expr};
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        if form == "module_collection":
            form = "module_set"
        if form == "module_collection_shadowed":
            form = "module_set_shadowed"
        if form == "set_param":
            src = f"""function {name}(values: Set<string>, value: string, other: string): boolean {{
  return values.has({element});
}}
"""
            return Variant("axis", src, name)
        if form == "set_inline":
            src = f"""function {name}(value: string, other: string): boolean {{
  return new Set<string>(["{left}", "{right_item}"]).has({element});
}}
"""
            return Variant("axis", src, name)
        if form == "set_local":
            src = f"""function {name}(value: string, other: string): boolean {{
  const values = new Set<string>(["{left}", "{right_item}"]);
  return values.has({element});
}}
"""
            return Variant("axis", src, name)
        if form == "module_set":
            src = f"""const VALUES = new Set<string>(["{left}", "{right_item}"]);

function {name}(value: string, other: string): boolean {{
  return VALUES.has({element});
}}
"""
            return Variant("axis", src, name)
        if form == "module_set_mutated":
            src = f"""const VALUES = new Set<string>(["{left}", "{right_item}"]);
VALUES.add("green");

function {name}(value: string, other: string): boolean {{
  return VALUES.has(value);
}}
"""
            return Variant("axis", src, name)
        if form == "module_set_shadowed":
            src = f"""const Set: any = function(_values: any) {{
  return {{ has: function() {{ return false; }} }};
}};
const VALUES = new Set(["{left}", "{right_item}"]);

function {name}(value: string, other: string): boolean {{
  return VALUES.has({element});
}}
"""
            return Variant("axis", src, name)
        if form == "set_untyped":
            src = f"""function {name}(values: any, value: string, other: string): boolean {{
  return values.has(value);
}}
"""
            return Variant("axis", src, name)
        if form == "array_some":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].some((item: string) => item === {element});
}}
"""
            return Variant("axis", src, name)
        if form == "array_every_absence":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].every((item: string) => item !== {element});
}}
"""
            return Variant("axis", src, name)
        if form == "array_indexof":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].indexOf({element}) >= 0;
}}
"""
            return Variant("axis", src, name)
        if form == "array_findindex":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].findIndex((item: string) => item === {element}) >= 0;
}}
"""
            return Variant("axis", src, name)
        if form == "array_filter_length":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].filter((item: string) => item === {element}).length >= 1;
}}
"""
            return Variant("axis", src, name)
        if form == "array_filter_length_absence":
            src = f"""function {name}(value: string, other: string): boolean {{
  return ["{left}", "{right_item}"].filter((item: string) => item === {element}).length <= 0;
}}
"""
            return Variant("axis", src, name)
        if form == "membership_absence":
            src = f"""function {name}(value: string, other: string): boolean {{
  return !["{left}", "{right_item}"].includes({element});
}}
"""
            return Variant("axis", src, name)
        if form == "typed_membership":
            src = f"""function {name}(values: string[], value: string, other: string): boolean {{
  return values.includes({element});
}}
"""
            return Variant("axis", src, name)
        if form == "dynamic_collection":
            src = f"""function {name}(values: string[], value: string, other: string): boolean {{
  return values.includes(value);
}}
"""
            return Variant("axis", src, name)
        if form == "unproven_receiver":
            src = f"""function {name}(values: string, value: string, other: string): boolean {{
  return values.includes(value);
}}
"""
            return Variant("axis", src, name)
        if form == "substring":
            expr = f'{element}.includes("{left}")'
        else:
            expr = f'["{left}", "{right_item}"].includes({element})'
        src = f"""function {name}(value: string, other: string): boolean {{
  return {expr};
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        if form in {
            "python_module_tuple",
            "python_module_set",
            "python_module_mutated",
        }:
            binding = {
                "python_module_tuple": f'("{left}", "{right_item}")',
                "python_module_set": f'{{"{left}", "{right_item}"}}',
                "python_module_mutated": f'["{left}", "{right_item}"]',
            }[form]
            mutation = 'VALUES.append("green")\n' if form == "python_module_mutated" else ""
            src = f"""VALUES = {binding}
{mutation}
def {name}(value, other):
    return {element} in VALUES
"""
            return Variant("axis", src, name)
        if form in {
            "python_set_factory",
            "python_tuple_factory",
            "python_frozenset_factory",
            "python_set_factory_shadowed",
        }:
            ctor = {
                "python_set_factory": "set",
                "python_tuple_factory": "tuple",
                "python_frozenset_factory": "frozenset",
                "python_set_factory_shadowed": "set",
            }[form]
            shadow = ""
            if form == "python_set_factory_shadowed":
                shadow = """    def set(_values):
        class Box:
            def __contains__(self, _value):
                return False
        return Box()
"""
            src = f"""def {name}(value, other):
{shadow}    return {ctor}(["{left}", "{right_item}"]).__contains__({element})
"""
            return Variant("axis", src, name)
        if form.startswith("python_deque_"):
            import_line = {
                "python_deque_import": "from collections import deque\n\n",
                "python_deque_alias": "from collections import deque as Values\n\n",
                "python_deque_namespace": "import collections\n\n",
                "python_deque_missing_import": "",
                "python_deque_shadowed": "from collections import deque\n\n",
                "python_deque_mutated": "from collections import deque\n\n",
            }[form]
            factory = {
                "python_deque_import": "deque",
                "python_deque_alias": "Values",
                "python_deque_namespace": "collections.deque",
                "python_deque_missing_import": "deque",
                "python_deque_shadowed": "deque",
                "python_deque_mutated": "deque",
            }[form]
            if form == "python_deque_shadowed":
                src = f"""{import_line}def deque(_values):
    class Box:
        def __contains__(self, _value):
            return False
    return Box()

def {name}(value, other):
    return deque(["{left}", "{right_item}"]).__contains__({element})
"""
                return Variant("axis", src, name)
            if form == "python_deque_mutated":
                src = f"""{import_line}def {name}(value, other):
    values = deque(["{left}", "{right_item}"])
    values.append("green")
    return values.__contains__(value)
"""
                return Variant("axis", src, name)
            src = f"""{import_line}def {name}(value, other):
    return {factory}(["{left}", "{right_item}"]).__contains__({element})
"""
            return Variant("axis", src, name)
        if form == "python_tuple_param":
            src = f"""def {name}(values: tuple[str, ...], value: str, other: str) -> bool:
    return {element} in values
"""
            return Variant("axis", src, name)
        if form.startswith("python_alias_"):
            import_line = {
                "python_alias_sequence": "from typing import Sequence as Values\n\n",
                "python_alias_container": "from collections.abc import Container as Values\n\n",
                "python_alias_set": "from typing import Set as Values\n\n",
                "python_alias_wrong_receiver": "from typing import Sequence as Values\n\n",
                "python_alias_unresolved": "",
                "python_alias_shadowed": "from typing import Sequence as Values\nValues = str\n\n",
            }[form]
            receiver = "other_values" if form == "python_alias_wrong_receiver" else "values"
            src = f"""{import_line}def {name}(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:
    return {element} in {receiver}
"""
            return Variant("axis", src, name)
        if form == "typed_membership":
            src = f"""def {name}(values: list[str], value: str, other: str) -> bool:
    return {element} in values
"""
            return Variant("axis", src, name)
        if form == "membership_absence":
            expr = f'{element} not in ["{left}", "{right_item}"]'
        elif form == "substring":
            expr = f'"{left}" in {element}'
        elif right:
            expr = f'["{left}", "{right_item}"].__contains__({element})'
        else:
            expr = f'{element} in ["{left}", "{right_item}"]'
        src = f"""def {name}(value, other):
    return {expr}
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        if form == "go_local_slice":
            src = f"""package p

import "slices"

func {name}(value string, other string) bool {{
    values := []string{{"{left}", "{right_item}"}}
    return slices.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "go_local_slice_mutated":
            src = f"""package p

import "slices"

func {name}(value string, other string) bool {{
    values := []string{{"{left}", "{right_item}"}}
    values = append(values, "green")
    return slices.Contains(values, value)
}}
"""
            return Variant("axis", src, name)
        if form == "go_slices_package":
            src = f"""package p

import "slices"

var values = []string{{"{left}", "{right_item}"}}

func {name}(value string, other string) bool {{
    return slices.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "go_slices_alias_package":
            src = f"""package p

import sl "slices"

var values = []string{{"{left}", "{right_item}"}}

func {name}(value string, other string) bool {{
    return sl.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "go_slices_const_package":
            src = f"""package p

import "slices"

const first = "{left}"
var values = []string{{first, "{right_item}"}}

func {name}(value string, other string) bool {{
    return slices.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "go_slices_mutated":
            src = f"""package p

import "slices"

var values = append([]string{{"{left}", "{right_item}"}}, "green")

func {name}(value string, other string) bool {{
    return slices.Contains(values, value)
}}
"""
            return Variant("axis", src, name)
        if form == "go_slices_unimported":
            src = f"""package p

type fakeSlices struct{{}}

func (fakeSlices) Contains(values []string, value string) bool {{
    return false
}}

var slices fakeSlices
var values = []string{{"{left}", "{right_item}"}}

func {name}(value string, other string) bool {{
    return slices.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "typed_membership":
            src = f"""package p

import "slices"

func {name}(values []string, value string, other string) bool {{
    return slices.Contains(values, {element})
}}
"""
            return Variant("axis", src, name)
        if form == "substring":
            src = f"""package p

import "strings"

func {name}(value string, other string) bool {{
    return strings.Contains({element}, "{left}")
}}
"""
        else:
            src = f"""package p

import "slices"

func {name}(value string, other string) bool {{
    return slices.Contains([]string{{"{left}", "{right_item}"}}, {element})
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        if form == "rust_local_vec":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let values = vec!["{left}", "{right_item}"];
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_vec_mutated":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let mut values = vec!["{left}", "{right_item}"];
    values.push("green");
    values.contains(&value)
}}
"""
            return Variant("axis", src, name)
        if form == "rust_vecdeque_param":
            src = f"""use std::collections::VecDeque;

pub fn {name}(values: &VecDeque<&str>, value: &str, other: &str) -> bool {{
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_array":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let values = ["{left}", "{right_item}"];
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_typed_array":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let values: [&str; 2] = ["{left}", "{right_item}"];
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_slice_ref":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let values: &[&str] = &["{left}", "{right_item}"];
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form in {"rust_std_hashset", "rust_std_btreeset", "rust_std_vecdeque"}:
            factory = {
                "rust_std_hashset": "HashSet",
                "rust_std_btreeset": "BTreeSet",
                "rust_std_vecdeque": "VecDeque",
            }[form]
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let values = std::collections::{factory}::from(["{left}", "{right_item}"]);
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_mutated":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let mut values = vec!["{left}", "{right_item}"];
    values.push("green");
    values.contains(&value)
}}
"""
            return Variant("axis", src, name)
        if form == "rust_std_hashset_mutated":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let mut values = std::collections::HashSet::from(["{left}", "{right_item}"]);
    values.insert("green");
    values.contains(&value)
}}
"""
            return Variant("axis", src, name)
        if form == "rust_local_custom_receiver":
            src = f"""struct Values;

impl Values {{
    fn contains(&self, _value: &&str) -> bool {{
        false
    }}
}}

pub fn {name}(value: &str, other: &str) -> bool {{
    let values = Values;
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "typed_membership":
            src = f"""pub fn {name}(values: &[&str], value: &str, other: &str) -> bool {{
    values.contains(&{element})
}}
"""
            return Variant("axis", src, name)
        if form == "dynamic_collection":
            src = f"""pub fn {name}(values: &[&str], value: &str, other: &str) -> bool {{
    values.contains(&value)
}}
"""
            return Variant("axis", src, name)
        if form == "unproven_receiver":
            src = f"""pub fn {name}(values: &str, value: &str, other: &str) -> bool {{
    values.contains(value)
}}
"""
            return Variant("axis", src, name)
        if form == "substring":
            expr = f'{element}.contains("{left}")'
        else:
            expr = f'["{left}", "{right_item}"].contains({element})'
        src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    {expr}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        if form == "java_local_list":
            src = f"""import java.util.List;

class C {{
    static boolean {name}(String value, String other) {{
        var values = List.of("{left}", "{right_item}");
        return values.contains({element});
    }}
}}
"""
            return Variant("axis", src, name)
        if form == "java_local_list_mutated":
            src = f"""import java.util.ArrayList;
import java.util.List;

class C {{
    static boolean {name}(String value, String other) {{
        var values = new ArrayList<String>(List.of("{left}", "{right_item}"));
        values.add("green");
        return values.contains(value);
    }}
}}
"""
            return Variant("axis", src, name)
        if form == "java_queue_param":
            src = f"""import java.util.Queue;

class C {{ static boolean {name}(Queue<String> values, String value, String other) {{ return values.contains({element}); }} }}
"""
            return Variant("axis", src, name)
        if form == "membership":
            src = f"""import java.util.List;

class C {{ static boolean {name}(String value, String other) {{ return List.of("{left}", "{right_item}").contains({element}); }} }}
"""
            return Variant("axis", src, name)
        if form == "module_collection":
            form = "java_module_list"
        if form == "module_collection_shadowed":
            form = "java_module_list_shadowed"
        if form == "java_module_list":
            src = f"""import java.util.List;

class C {{
    static final List<String> VALUES = List.of("{left}", "{right_item}");

    static boolean {name}(String value, String other) {{
        return VALUES.contains({element});
    }}
}}
"""
            return Variant("axis", src, name)
        if form == "java_module_list_shadowed":
            src = f"""class C {{
    static final List<String> VALUES = List.of("{left}", "{right_item}");

    static boolean {name}(String value, String other) {{
        return VALUES.contains({element});
    }}
}}

class List<T> {{
    static java.util.List<String> of(String left, String right) {{
        return java.util.List.of("green", right);
    }}
}}
"""
            return Variant("axis", src, name)
        if form.startswith("java_"):
            ctor_form = form.removesuffix("_shadowed")
            shadowed = form.endswith("_shadowed")
            if ctor_form == "java_list_of":
                import_line = "import java.util.List;\n\n"
                factory = f'List.of("{left}", "{right_item}")'
                shadow_param = "Object List, "
            elif ctor_form == "java_set_of":
                import_line = "import java.util.Set;\n\n"
                factory = f'Set.of("{left}", "{right_item}")'
                shadow_param = "Object Set, "
            else:
                import_line = "import java.util.Arrays;\n\n"
                factory = f'Arrays.asList("{left}", "{right_item}")'
                shadow_param = "Object Arrays, "
            params = f"{shadow_param}String value, String other" if shadowed else "String value, String other"
            imports = "" if shadowed else import_line
            src = f"""{imports}class C {{ static boolean {name}({params}) {{ return {factory}.contains({element}); }} }}
"""
            return Variant("axis", src, name)
        if form in {"typed_membership", "set_param"}:
            src = f"""import java.util.List;

class C {{ static boolean {name}(List<String> values, String value, String other) {{ return values.contains({element}); }} }}
"""
        elif form == "dynamic_collection":
            src = f"""import java.util.List;

class C {{ static boolean {name}(List<String> values, String value, String other) {{ return values.contains(value); }} }}
"""
        elif form == "unproven_receiver":
            src = f"""class C {{ static boolean {name}(String values, String value, String other) {{ return values.contains(value); }} }}
"""
        else:
            raise ValueError(f"unsupported Java membership form: {form}")
        return Variant("axis", src, name)

    if surface.key == "ruby":
        if form.startswith("ruby_set_"):
            require = "" if form == "ruby_set_missing_require" else 'require "set"\n\n'
            if form == "ruby_set_new_member":
                method = "member?"
                body = f'Set.new(["{left}", "{right_item}"]).{method}({element})'
            elif form == "ruby_set_local":
                src = f"""{require}def {name}(value, other)
  values = Set.new(["{left}", "{right_item}"])
  values.include?({element})
end
"""
                return Variant("axis", src, name)
            elif form == "ruby_set_mutated":
                src = f"""{require}def {name}(value, other)
  values = Set.new(["{left}", "{right_item}"])
  values.add("green")
  values.include?(value)
end
"""
                return Variant("axis", src, name)
            elif form == "ruby_set_shadowed":
                src = f"""{require}class Set
  def self.new(_values)
    Box.new
  end
end

class Box
  def include?(_value)
    false
  end
end

def {name}(value, other)
  Set.new(["{left}", "{right_item}"]).include?({element})
end
"""
                return Variant("axis", src, name)
            else:
                body = f'Set.new(["{left}", "{right_item}"]).include?({element})'
            src = f"""{require}def {name}(value, other)
  {body}
end
"""
            return Variant("axis", src, name)
        if form == "membership_absence":
            expr = f'!["{left}", "{right_item}"].include?({element})'
        elif form == "substring":
            expr = f'{element}.include?("{left}")'
        else:
            expr = f'["{left}", "{right_item}"].include?({element})'
        src = f"""def {name}(value, other)
  {expr}
end
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for literal membership axis: {surface.key}")


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


def projection_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if proposal_id == "axis_projection_temp_identity":
        return True
    if proposal_id in {
        "axis_projection_destructure_identity",
        "axis_projection_destructure_shorthand_identity",
        "axis_projection_destructure_multi_identity",
    }:
        return surface.key in {"javascript", "typescript", "vue", "svelte", "html", "rust"}
    if proposal_id in {
        "axis_projection_static_key_identity",
        "axis_projection_default_boundary",
        "axis_projection_dynamic_key_boundary",
    }:
        return surface.key in JS_LIKE_SURFACES
    return False


def python_docstring_axis_supported(surface: Surface, proposal_id: str) -> bool:
    return proposal_id.startswith("axis_python_docstring_") and surface.key == "python"


def axis_python_docstring_variant(
    surface: Surface, proposal_id: str, negative: bool, right: bool
) -> Variant:
    if surface.key != "python":
        raise ValueError(f"unsupported surface for Python docstring axis: {surface.key}")

    name = "axis_case"
    if proposal_id == "axis_python_docstring_guard_identity":
        miss_value = 2 if right and negative else 0
        if right:
            src = f'''def {name}(i, j):
    """Return one when the indexes match."""
    if i == j:
        return 1
    else:
        return {miss_value}
'''
            return Variant("function-docstring-ifelse", src, name)
        src = f"""def {name}(i, j):
    if i == j:
        return 1
    return 0
"""
        return Variant("guard-return", src, name)

    if proposal_id == "axis_python_docstring_return_identity":
        doc = '    """Return the final valid index."""\n' if right else ""
        offset = "" if not (right and negative) else " + 1"
        src = f"""def {name}(values):
{doc}    return len(values) - 1{offset}
"""
        return Variant("function-docstring-return" if right else "direct-return", src, name)

    if proposal_id == "axis_python_docstring_different_text_identity":
        doc = (
            '    """First documentation text."""\n'
            if not right
            else '    """Second documentation text with different words."""\n'
        )
        addend = 2 if right and negative else 1
        src = f"""def {name}(value):
{doc}    return value * value + {addend}
"""
        return Variant("different-docstring-text", src, name)

    if proposal_id == "axis_python_docstring_returned_string_boundary":
        value = "blue" if right and negative else "red"
        src = f"""def {name}():
    return "{value}"
"""
        return Variant("returned-string", src, name)

    if proposal_id == "axis_python_docstring_assigned_string_boundary":
        value = "blue" if right and negative else "red"
        src = f"""def {name}():
    label = "{value}"
    return label
"""
        return Variant("assigned-string", src, name)

    if proposal_id == "axis_python_docstring_fstring_boundary":
        effect = '    observe(f"{value}")\n' if right and negative else ""
        src = f"""def {name}(value):
{effect}    return 1
"""
        return Variant("dynamic-fstring-effect" if effect else "no-effect", src, name)

    raise ValueError(f"unknown Python docstring proposal: {proposal_id}")


def axis_projection_variant(surface: Surface, proposal_id: str, negative: bool, right: bool) -> Variant:
    field = (
        "tomorrow"
        if negative
        and right
        and proposal_id
        not in {"axis_projection_default_boundary", "axis_projection_dynamic_key_boundary"}
        else "today"
    )

    if surface.language == "javascript":
        name = "buildCase" if right else "axisCase"
        if proposal_id == "axis_projection_destructure_identity" and right:
            body = f"""function {name}(row, amount) {{
  const {{ {field}: selected }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_destructure_shorthand_identity" and right:
            body = f"""function {name}(row, amount) {{
  const {{ {field} }} = row;
  return amount + {field};
}}
"""
        elif proposal_id == "axis_projection_destructure_multi_identity" and right:
            body = f"""function {name}(row, amount) {{
  const {{ tomorrow: unused, {field}: selected }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_default_boundary" and right:
            body = f"""function {name}(row, amount) {{
  const {{ today: selected = 0 }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_dynamic_key_boundary" and right:
            body = f"""function {name}(row, amount, key) {{
  return amount + row[key];
}}
"""
        elif proposal_id == "axis_projection_static_key_identity" and right:
            body = f"""function {name}(row, amount) {{
  return amount + row[{field!r}];
}}
"""
        elif right:
            body = f"""function {name}(row, amount) {{
  const selected = row.{field};
  return amount + selected;
}}
"""
        else:
            body = f"""function {name}(record, value) {{
  return value + record.today;
}}
"""
        return js_axis_source(surface, body, name)

    if surface.key == "typescript":
        name = "buildCase" if right else "axisCase"
        type_sig = "{ today: number; tomorrow: number }"
        if proposal_id == "axis_projection_destructure_identity" and right:
            src = f"""function {name}(row: {type_sig}, amount: number): number {{
  const {{ {field}: selected }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_destructure_shorthand_identity" and right:
            src = f"""function {name}(row: {type_sig}, amount: number): number {{
  const {{ {field} }} = row;
  return amount + {field};
}}
"""
        elif proposal_id == "axis_projection_destructure_multi_identity" and right:
            src = f"""function {name}(row: {type_sig}, amount: number): number {{
  const {{ tomorrow: unused, {field}: selected }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_default_boundary" and right:
            src = f"""function {name}(row: Partial<{type_sig}>, amount: number): number {{
  const {{ today: selected = 0 }} = row;
  return amount + selected;
}}
"""
        elif proposal_id == "axis_projection_dynamic_key_boundary" and right:
            src = f"""function {name}(row: {type_sig}, amount: number, key: keyof {type_sig}): number {{
  return amount + row[key];
}}
"""
        elif proposal_id == "axis_projection_static_key_identity" and right:
            src = f"""function {name}(row: {type_sig}, amount: number): number {{
  return amount + row[{field!r}];
}}
"""
        elif right:
            src = f"""function {name}(row: {type_sig}, amount: number): number {{
  const selected = row.{field};
  return amount + selected;
}}
"""
        else:
            src = f"""function {name}(record: {type_sig}, value: number): number {{
  return value + record.today;
}}
"""
        return Variant("axis", src, name)

    if surface.key == "python":
        name = "build_case" if right else "axis_case"
        if right:
            src = f"""def {name}(row, amount):
    selected = row.{field}
    return amount + selected
"""
        else:
            src = f"""def {name}(record, value):
    return value + record.today
"""
        return Variant("axis", src, name)

    if surface.key == "go":
        name = "BuildCase" if right else "AxisCase"
        if right:
            src = f"""package p

func {name}(row Reading, amount int) int {{
    selected := row.{field.title()}
    return amount + selected
}}
"""
        else:
            src = f"""package p

func {name}(record Reading, value int) int {{
    return value + record.Today
}}
"""
        return Variant("axis", src, name)

    if surface.key == "rust":
        name = "build_case" if right else "axis_case"
        if proposal_id == "axis_projection_destructure_identity" and right:
            src = f"""pub fn {name}(row: Reading, amount: i32) -> i32 {{
    let Reading {{ {field}: selected, .. }} = row;
    amount + selected
}}
"""
        elif proposal_id == "axis_projection_destructure_shorthand_identity" and right:
            src = f"""pub fn {name}(row: Reading, amount: i32) -> i32 {{
    let Reading {{ {field}, .. }} = row;
    amount + {field}
}}
"""
        elif proposal_id == "axis_projection_destructure_multi_identity" and right:
            src = f"""pub fn {name}(row: Reading, amount: i32) -> i32 {{
    let Reading {{ tomorrow: _unused, {field}: selected, .. }} = row;
    amount + selected
}}
"""
        elif right:
            src = f"""pub fn {name}(row: Reading, amount: i32) -> i32 {{
    let selected = row.{field};
    amount + selected
}}
"""
        else:
            src = f"""pub fn {name}(record: Reading, value: i32) -> i32 {{
    value + record.today
}}
"""
        return Variant("axis", src, name)

    if surface.key == "java":
        name = "buildCase" if right else "axisCase"
        if right:
            src = f"""class AxisCase {{
    static int {name}(Reading row, int amount) {{
        int selected = row.{field};
        return amount + selected;
    }}
}}
"""
        else:
            src = f"""class AxisCase {{
    static int {name}(Reading record, int value) {{
        return value + record.today;
    }}
}}
"""
        return Variant("axis", src, name)

    if surface.key == "c":
        name = "build_case" if right else "axis_case"
        if right:
            src = f"""int {name}(struct Reading row, int amount) {{
    int selected = row.{field};
    return amount + selected;
}}
"""
        else:
            src = f"""int {name}(struct Reading record, int value) {{
    return value + record.today;
}}
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        name = "build_case" if right else "axis_case"
        if right:
            src = f"""def {name}(row, amount)
  selected = row.{field}
  amount + selected
end
"""
        else:
            src = f"""def {name}(record, value)
  value + record.today
end
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for projection axis: {surface.key}")


def axis_unsafe_boundary_variant(surface: Surface, right: bool) -> Variant:
    name = "buildCase" if right else "axisCase"
    if surface.language == "javascript":
        body = f"""function {name}(value) {{
  return value + AMBIENT_LIMIT;
}}
"""
        return js_axis_source(surface, body, name)
    if surface.key == "typescript":
        src = f"""function {name}(value: number): number {{
  return value + AMBIENT_LIMIT;
}}
"""
        return Variant("axis", src, name)
    if surface.key == "python":
        py_name = "build_case" if right else "axis_case"
        src = f"""def {py_name}(value):
    return value + AMBIENT_LIMIT
"""
        return Variant("axis", src, py_name)
    if surface.key == "go":
        go_name = "BuildCase" if right else "AxisCase"
        src = f"""package p

func {go_name}(value int) int {{
    return value + AmbientLimit
}}
"""
        return Variant("axis", src, go_name)
    if surface.key == "rust":
        rs_name = "build_case" if right else "axis_case"
        src = f"""pub fn {rs_name}(value: i32) -> i32 {{
    value + AMBIENT_LIMIT
}}
"""
        return Variant("axis", src, rs_name)
    if surface.key == "java":
        java_name = "buildCase" if right else "axisCase"
        src = f"""class AxisCase {{
    static int {java_name}(int value) {{
        return value + AMBIENT_LIMIT;
    }}
}}
"""
        return Variant("axis", src, java_name)
    if surface.key == "c":
        c_name = "build_case" if right else "axis_case"
        src = f"""int {c_name}(int value) {{
    return value + AMBIENT_LIMIT;
}}
"""
        return Variant("axis", src, c_name)
    if surface.key == "ruby":
        rb_name = "build_case" if right else "axis_case"
        src = f"""def {rb_name}(value)
  value + AMBIENT_LIMIT
end
"""
        return Variant("axis", src, rb_name)
    raise ValueError(f"unsupported surface for unsafe axis: {surface.key}")


def import_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if proposal_id.startswith("axis_import_namespace_shadowed_param_"):
        return surface.key in {"javascript", "typescript"}
    if proposal_id in {"axis_import_named_identity", "axis_import_alias_identity"}:
        return surface.key in {
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "python",
            "rust",
            "java",
        }
    if proposal_id == "axis_import_namespace_identity":
        return surface.key in {
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "python",
            "go",
        }
    if proposal_id in {
        "axis_import_namespace_member_identity",
        "axis_import_namespace_member_wrong_boundary",
    }:
        return surface.key in {
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "python",
        }
    if proposal_id == "axis_import_default_identity":
        return surface.key in {"javascript", "typescript", "vue", "svelte", "html"}
    if proposal_id == "axis_import_default_named_boundary":
        return surface.key in {"javascript", "typescript", "vue", "svelte", "html"}
    if proposal_id == "axis_import_multi_specifier_identity":
        return surface.key in {"javascript", "typescript", "vue", "svelte", "html", "python"}
    if proposal_id == "axis_import_reexport_boundary":
        return surface.key in {"javascript", "typescript", "vue", "svelte", "html"}
    if proposal_id == "axis_import_unsafe_boundary":
        return True
    return False


def import_axis_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    entry = "buildCase" if right else "axisCase"
    local = "calc" if right else "helper"
    export = (
        "otherHelper"
        if negative
        and proposal_id
        in {
            "axis_import_named_identity",
            "axis_import_namespace_identity",
            "axis_import_namespace_member_identity",
            "axis_import_namespace_member_wrong_boundary",
        }
        else "helper"
    )
    module = (
        "./other-math"
        if negative and proposal_id in {"axis_import_alias_identity", "axis_import_default_identity"}
        else "./shared-math"
    )

    if proposal_id in {"axis_import_unsafe_boundary", "axis_import_reexport_boundary"}:
        if proposal_id == "axis_import_reexport_boundary" and surface.key not in JS_LIKE_SURFACES:
            raise ValueError(f"{surface.key} does not support {proposal_id}")
        if proposal_id == "axis_import_reexport_boundary":
            body = f"""export {{ helper }} from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
            return js_axis_source(surface, body, entry)
        if surface.key in JS_LIKE_SURFACES:
            body = f"""import * as maybeMath from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
            return js_axis_source(surface, body, entry)
        if surface.key == "python":
            py_entry = "build_case" if right else "axis_case"
            src = f"""from shared_math import *

def {py_entry}(value):
    return helper(value + 1)
"""
            return Variant("axis", src, py_entry)
        if surface.key == "rust":
            rs_entry = "build_case" if right else "axis_case"
            src = f"""use crate::shared_math::*;

pub fn {rs_entry}(value: i32) -> i32 {{
    helper(value + 1)
}}
"""
            return Variant("axis", src, rs_entry)
        if surface.key == "java":
            java_entry = "buildCase" if right else "axisCase"
            src = f"""import static shared.Math.*;

class AxisCase {{
    static int {java_entry}(int value) {{
        return helper(value + 1);
    }}
}}
"""
            return Variant("axis", src, java_entry)
        if surface.key == "go":
            go_entry = "BuildCase" if right else "AxisCase"
            src = f"""package p

import . "shared/math"

func {go_entry}(value int) int {{
    return Helper(value + 1)
}}
"""
            return Variant("axis", src, go_entry)
        if surface.key == "c":
            c_entry = "build_case" if right else "axis_case"
            src = f"""#include "shared_math.h"

int {c_entry}(int value) {{
    return helper(value + 1);
}}
"""
            return Variant("axis", src, c_entry)
        if surface.key == "ruby":
            rb_entry = "build_case" if right else "axis_case"
            src = f"""require_relative "shared_math"

def {rb_entry}(value)
  helper(value + 1)
end
"""
            return Variant("axis", src, rb_entry)
        raise ValueError(f"unsupported import unsafe surface: {surface.key}")

    if surface.key in JS_LIKE_SURFACES:
        if proposal_id.startswith("axis_import_namespace_shadowed_param_"):
            body = import_namespace_shadowed_param_body(entry, proposal_id, right, negative)
        elif proposal_id == "axis_import_namespace_member_identity":
            if right:
                ns = "mathOps"
                member = "otherHelper" if negative else "helper"
                body = f"""import * as {ns} from {module!r};
function {entry}(value) {{
  return {ns}.{member}(value + 1);
}}
"""
            else:
                body = f"""import {{ helper }} from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
        elif proposal_id == "axis_import_namespace_member_wrong_boundary":
            if right:
                body = f"""import * as mathOps from {module!r};
function {entry}(value) {{
  return mathOps.otherHelper(value + 1);
}}
"""
            else:
                body = f"""import {{ helper }} from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
        elif proposal_id == "axis_import_namespace_identity":
            ns = "mathOps" if right else "util"
            member = "otherHelper" if negative else "helper"
            body = f"""import * as {ns} from {module!r};
function {entry}(value) {{
  return {ns}.{member}(value + 1);
}}
"""
        elif proposal_id == "axis_import_default_identity":
            body = f"""import {local} from {module!r};
function {entry}(value) {{
  return {local}(value + 1);
}}
"""
        elif proposal_id == "axis_import_default_named_boundary":
            if negative and right:
                body = f"""import {{ helper }} from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
            else:
                body = f"""import helper from {module!r};
function {entry}(value) {{
  return helper(value + 1);
}}
"""
        elif proposal_id == "axis_import_multi_specifier_identity":
            imported = "otherHelper as calc" if negative and right else "helper as calc"
            body = f"""import {{ unusedHelper, {imported} }} from {module!r};
function {entry}(value) {{
  return calc(value + 1);
}}
"""
        else:
            imported = f"{export} as {local}" if local != export else export
            body = f"""import {{ {imported} }} from {module!r};
function {entry}(value) {{
  return {local}(value + 1);
}}
"""
        return js_axis_source(surface, body, entry)

    if surface.key == "python":
        py_entry = "build_case" if right else "axis_case"
        py_module = "other_math" if module == "./other-math" else "shared_math"
        if proposal_id == "axis_import_namespace_member_identity":
            if right:
                ns = "math_ops"
                member = "other_helper" if negative else "helper"
                src = f"""import {py_module} as {ns}

def {py_entry}(value):
    return {ns}.{member}(value + 1)
"""
            else:
                src = f"""from {py_module} import helper

def {py_entry}(value):
    return helper(value + 1)
"""
        elif proposal_id == "axis_import_namespace_member_wrong_boundary":
            if right:
                src = f"""import {py_module} as math_ops

def {py_entry}(value):
    return math_ops.other_helper(value + 1)
"""
            else:
                src = f"""from {py_module} import helper

def {py_entry}(value):
    return helper(value + 1)
"""
        elif proposal_id == "axis_import_namespace_identity":
            ns = "math_ops" if right else "util"
            member = "other_helper" if negative else "helper"
            src = f"""import {py_module} as {ns}

def {py_entry}(value):
    return {ns}.{member}(value + 1)
"""
        elif proposal_id == "axis_import_multi_specifier_identity":
            imported = "other_helper as calc" if negative and right else "helper as calc"
            src = f"""from {py_module} import unused_helper, {imported}

def {py_entry}(value):
    return calc(value + 1)
"""
        else:
            py_export = "other_helper" if export == "otherHelper" else "helper"
            imported = f"{py_export} as {local}" if local != py_export else py_export
            src = f"""from {py_module} import {imported}

def {py_entry}(value):
    return {local}(value + 1)
"""
        return Variant("axis", src, py_entry)

    if surface.key == "rust":
        rs_entry = "build_case" if right else "axis_case"
        rs_module = "other_math" if module == "./other-math" else "shared_math"
        rs_export = "other_helper" if export == "otherHelper" else "helper"
        imported = f"{rs_export} as {local}" if local != rs_export else rs_export
        src = f"""use crate::{rs_module}::{imported};

pub fn {rs_entry}(value: i32) -> i32 {{
    {local}(value + 1)
}}
"""
        return Variant("axis", src, rs_entry)

    if surface.key == "java":
        java_entry = "buildCase" if right else "axisCase"
        java_module = "other.Math" if module == "./other-math" else "shared.Math"
        java_export = "otherHelper" if export == "otherHelper" else "helper"
        src = f"""import static {java_module}.{java_export};

class AxisCase {{
    static int {java_entry}(int value) {{
        return {java_export}(value + 1);
    }}
}}
"""
        return Variant("axis", src, java_entry)

    if surface.key == "go":
        go_entry = "BuildCase" if right else "AxisCase"
        go_module = "other/math" if module == "./other-math" else "shared/math"
        member = "OtherHelper" if negative else "Helper"
        ns = "mathOps" if right else "util"
        src = f"""package p

import {ns} "{go_module}"

func {go_entry}(value int) int {{
    return {ns}.{member}(value + 1)
}}
"""
        return Variant("axis", src, go_entry)

    raise ValueError(f"{surface.key} does not support {proposal_id}")


def import_namespace_shadowed_param_body(
    entry: str,
    proposal_id: str,
    right: bool,
    negative: bool,
) -> str:
    template_body = f"""function {entry}(rootDir, filePath) {{
  if (!filePath.startsWith("<rootDir>")) {{
    return filePath;
  }}

  return path.resolve(
    rootDir,
    path.normalize(`./${{filePath.slice("<rootDir>".length)}}`),
  );
}}
"""
    concat_body = f"""function {entry}(rootDir, filePath) {{
  if (!filePath.startsWith("<rootDir>")) {{
    return filePath;
  }}

  return path.resolve(
    rootDir,
    path.normalize("./" + filePath.slice("<rootDir>".length)),
  );
}}
"""
    wrong_template_body = f"""function {entry}(rootDir, filePath) {{
  if (!filePath.startsWith("<rootDir>")) {{
    return filePath;
  }}

  return path.resolve(
    rootDir,
    path.normalize(`../${{filePath.slice("<rootDir>".length)}}`),
  );
}}
"""
    if proposal_id == "axis_import_namespace_shadowed_param_identity":
        helper = """const escapeGlobCharacters = path =>
  path.replaceAll(/([!()*?[\\]{}])/g, "\\$1");

"""
        prefix = 'import * as path from "node:path";\n\n'
        body = wrong_template_body if right and negative else template_body
        return prefix + (helper if right else "") + body
    if proposal_id == "axis_import_namespace_shadowed_param_template_identity":
        prefix = 'import * as path from "node:path";\n\n'
        body = wrong_template_body if right and negative else template_body
        return prefix + (body if right else concat_body)
    if proposal_id == "axis_import_namespace_shadowed_param_unshadowed_mutation_boundary":
        if right and negative:
            return (
                'import * as path from "node:path";\n\n'
                'function touchPath() {\n  path.replaceAll("x", "y");\n}\n\n'
                + template_body
            )
        return 'import * as path from "node:path";\n\n' + template_body
    if proposal_id == "axis_import_namespace_shadowed_param_fake_receiver_boundary":
        if right and negative:
            return (
                "const path = {\n"
                "  normalize: value => value,\n"
                "  resolve: (rootDir, value) => value,\n"
                "};\n\n"
                + template_body
            )
        return 'import * as path from "node:path";\n\n' + template_body
    raise ValueError(f"unsupported namespace shadow proposal: {proposal_id}")


def stable_id(*parts: str) -> str:
    h = hashlib.sha256()
    for p in parts:
        h.update(p.encode())
        h.update(b"\0")
    return h.hexdigest()[:16]


def rel_source_path(case_id: str, side: str, surface: Surface) -> Path:
    return Path("sources") / case_id / f"{side}.{surface.extension}"


def source_record(surface: Surface, variant: Variant, path: Path) -> dict:
    return {
        "language": surface.language,
        "surface": surface.key,
        "representation": variant.representation,
        "path": path.as_posix(),
        "entrypoint": variant.entrypoint,
        "start_line": variant.start_line,
        "end_line": variant.start_line + len(variant.source.rstrip("\n").splitlines()) - 1,
    }


def write_source(out_dir: Path, rel_path: Path, source: str) -> None:
    full = out_dir / rel_path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(source)


def validate_proposals(proposals_doc: dict) -> None:
    seen: set[str] = set()
    for proposal in proposals_doc.get("proposals", []):
        missing = REQUIRED_PROPOSAL_FIELDS - proposal.keys()
        if missing:
            raise ValueError(f"{proposal.get('proposal_id', '<unknown>')} missing fields: {sorted(missing)}")
        if proposal["proposal_id"] in seen:
            raise ValueError(f"duplicate proposal_id: {proposal['proposal_id']}")
        seen.add(proposal["proposal_id"])
        if proposal["operation"] not in OPERATIONS:
            raise ValueError(
                f"{proposal['proposal_id']} references unknown operation {proposal['operation']}"
            )
        budget = proposal["complexity_budget"]
        missing_budget = REQUIRED_BUDGET_FIELDS - budget.keys()
        if missing_budget:
            raise ValueError(f"{proposal['proposal_id']} missing budget fields: {sorted(missing_budget)}")
        for field in REQUIRED_BUDGET_FIELDS:
            if not isinstance(budget[field], int) or budget[field] < 0:
                raise ValueError(f"{proposal['proposal_id']} budget {field} must be a non-negative integer")


def check_variant_budget(proposal: dict, surface: Surface, variant: Variant) -> None:
    budget = proposal["complexity_budget"]
    lines = len(variant.source.rstrip("\n").splitlines())
    if lines > budget["max_lines"]:
        raise ValueError(
            f"{proposal['proposal_id']} {surface.key}:{variant.representation} has "
            f"{lines} lines > budget {budget['max_lines']}"
        )
    branch_count = len(re.findall(r"\bif\b", variant.source))
    if branch_count > budget["max_branch_count"]:
        raise ValueError(
            f"{proposal['proposal_id']} {surface.key}:{variant.representation} has "
            f"{branch_count} branches > budget {budget['max_branch_count']}"
        )


def make_item(
    out_dir: Path,
    proposal: dict,
    left_surface: Surface,
    right_surface: Surface,
    right_representation: str,
    semantic_status: str,
    cross_label: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    operation = proposal["operation"]
    if operation not in OPERATIONS:
        raise ValueError(f"{proposal['proposal_id']} references unknown operation {operation}")
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal["proposal_id"],
        left_surface.key,
        right_surface.key,
        right_representation,
        semantic_status,
        cross_label,
        negative_tag or "positive",
    )
    left = EMITTERS[left_surface.key](operation, "loop", False)
    right = EMITTERS[right_surface.key](operation, right_representation, negative)
    check_variant_budget(proposal, left_surface, left)
    check_variant_budget(proposal, right_surface, right)
    left_path = rel_source_path(case_id, "left", left_surface)
    right_path = rel_source_path(case_id, "right", right_surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    equivalent = semantic_status == "equivalent"
    evidence = evidence_positive(operation) if equivalent else evidence_negative(operation)
    transform_tags = proposal["transform_tags"].copy()
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal["proposal_id"],
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": equivalent,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": operation,
            "representations": ["loop", right_representation],
            "data_shape": "aligned-list<int>" if OPERATIONS[operation].arity == 2 else "list<int>",
            "language_relation": cross_label,
            "negative_tag": negative_tag,
            "semantic_axes": ["aggregate_reduction"],
            "capabilities": {},
            "template_split": split,
        },
        "left": source_record(left_surface, left, left_path),
        "right": source_record(right_surface, right, right_path),
        "evidence": evidence,
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": proposal["complexity_budget"],
        },
    }


def make_c_contract_negative_item(out_dir: Path, proposal: dict, representation: str) -> dict:
    operation = proposal["operation"]
    if operation not in OPERATIONS:
        raise ValueError(f"{proposal['proposal_id']} references unknown operation {operation}")
    surface = next(s for s in SURFACES if s.key == "c")
    case_id = stable_id(
        proposal["proposal_id"],
        "c",
        representation,
        "not_equivalent",
        "c-contract-hard-negative",
    )
    left = EMITTERS["c"](operation, "loop", False)
    right = EMITTERS["c"](operation, representation, False)
    check_variant_budget(proposal, surface, left)
    check_variant_budget(proposal, surface, right)
    left_path = rel_source_path(case_id, "left", surface)
    right_path = rel_source_path(case_id, "right", surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    return {
        "case_id": case_id,
        "proposal_id": proposal["proposal_id"],
        "split": "heldout",
        "semantic_status": "not_equivalent",
        "expected_exact_detect": False,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": proposal["transform_tags"]
        + ["c-contract-hard-negative", representation],
        "matrix": {
            "computation": operation,
            "representations": ["loop", representation],
            "data_shape": "aligned-list<int>" if OPERATIONS[operation].arity == 2 else "list<int>",
            "language_relation": "same-surface",
            "negative_tag": representation,
            "semantic_axes": ["aggregate_reduction", "pointer_length_contract"],
            "capabilities": {},
            "template_split": "heldout",
        },
        "left": source_record(surface, left, left_path),
        "right": source_record(surface, right, right_path),
        "evidence": evidence_contract_negative(operation, representation),
        "llm_proposal": {
            "why": (
                "Adversarial C pointer-length sibling: exact detection must not merge "
                "partial traversal with the full `(xs, n)` contract."
            ),
            "complexity_budget": proposal["complexity_budget"],
        },
    }


def axis_variants(
    surface: Surface,
    proposal_id: str,
    axis: str,
    negative: bool,
) -> tuple[Variant, Variant]:
    if proposal_id.startswith("axis_import_"):
        return (
            import_axis_variant(surface, proposal_id, False, False),
            import_axis_variant(surface, proposal_id, negative, True),
        )
    if axis == "nullish_default":
        return (
            axis_nullish_variant(surface, proposal_id, False, False),
            axis_nullish_variant(surface, proposal_id, negative, True),
        )
    if axis == "own_property_guard":
        return (
            axis_own_property_variant(surface, proposal_id, False, False),
            axis_own_property_variant(surface, proposal_id, negative, True),
        )
    if axis == "record_shape_guard":
        return (
            axis_record_guard_variant(surface, proposal_id, False, False),
            axis_record_guard_variant(surface, proposal_id, negative, True),
        )
    if axis == "collection_empty_check":
        return (
            axis_collection_empty_variant(surface, proposal_id, False, False),
            axis_collection_empty_variant(surface, proposal_id, negative, True),
        )
    if axis == "string_prefix_suffix":
        return (
            axis_string_prefix_variant(surface, proposal_id, False, False),
            axis_string_prefix_variant(surface, proposal_id, negative, True),
        )
    if axis == "literal_collection_membership":
        return (
            axis_membership_literal_variant(surface, proposal_id, False, False),
            axis_membership_literal_variant(surface, proposal_id, negative, True),
        )
    if axis == "map_key_membership":
        return (
            axis_map_key_membership_variant(surface, proposal_id, False, False),
            axis_map_key_membership_variant(surface, proposal_id, negative, True),
        )
    if axis == "literal_map_default_lookup":
        return (
            axis_map_default_variant(surface, proposal_id, False, False),
            axis_map_default_variant(surface, proposal_id, negative, True),
        )
    if axis == "map_default_lookup":
        return (
            axis_map_default_lookup_variant(surface, proposal_id, False, False),
            axis_map_default_lookup_variant(surface, proposal_id, negative, True),
        )
    if axis == "null_presence_predicate":
        return (
            axis_null_presence_variant(surface, proposal_id, False, False),
            axis_null_presence_variant(surface, proposal_id, negative, True),
        )
    if axis == "numeric_minmax_abs":
        if proposal_id.startswith(
            (
                "axis_scalar_min_",
                "axis_scalar_max_",
                "axis_scalar_rust_min_",
                "axis_scalar_rust_max_",
            )
        ):
            return (
                axis_scalar_minmax_variant(surface, proposal_id, False, False),
                axis_scalar_minmax_variant(surface, proposal_id, negative, True),
            )
        return (
            axis_scalar_abs_variant(surface, proposal_id, False, False),
            axis_scalar_abs_variant(surface, proposal_id, negative, True),
        )
    if axis == "numeric_clamp":
        return (
            axis_numeric_clamp_variant(surface, proposal_id, False, False),
            axis_numeric_clamp_variant(surface, proposal_id, negative, True),
        )
    if axis == "hof_filter_map":
        return (
            axis_hof_filter_map_variant(surface, proposal_id, False, False),
            axis_hof_filter_map_variant(surface, proposal_id, negative, True),
        )
    if axis == "total_order_compare":
        return (
            axis_total_order_compare_variant(surface, proposal_id, False, False),
            axis_total_order_compare_variant(surface, proposal_id, negative, True),
        )
    if axis == "java_statically_false_loop":
        return (
            axis_java_dead_loop_variant(surface, proposal_id, False, False),
            axis_java_dead_loop_variant(surface, proposal_id, negative, True),
        )
    if axis == "java_integer_low_bit_toggle":
        return (
            axis_java_low_bit_toggle_variant(surface, proposal_id, False, False),
            axis_java_low_bit_toggle_variant(surface, proposal_id, negative, True),
        )
    if axis == "c_u16_be_byte_pack":
        return (
            axis_c_u16_be_byte_pack_variant(surface, proposal_id, False, False),
            axis_c_u16_be_byte_pack_variant(surface, proposal_id, negative, True),
        )
    if axis == "c_u32_be_byte_pack":
        return (
            axis_c_u32_be_byte_pack_variant(surface, proposal_id, False, False),
            axis_c_u32_be_byte_pack_variant(surface, proposal_id, negative, True),
        )
    if axis == "immutable_binding":
        return (
            axis_immutable_binding_variant(surface, False, False),
            axis_immutable_binding_variant(surface, negative, True),
        )
    if axis == "proven_callee_identity":
        return (
            axis_callee_identity_variant(surface, False, False),
            axis_callee_identity_variant(surface, negative, True),
        )
    if axis == "table_access":
        return (
            axis_table_access_variant(surface, False, False),
            axis_table_access_variant(surface, negative, True),
        )
    if axis == "projection_identity":
        return (
            axis_projection_variant(surface, proposal_id, False, False),
            axis_projection_variant(surface, proposal_id, negative, True),
        )
    if axis == "python_docstring_noop":
        return (
            axis_python_docstring_variant(surface, proposal_id, False, False),
            axis_python_docstring_variant(surface, proposal_id, negative, True),
        )
    if axis == "unsafe_boundary":
        return (
            axis_unsafe_boundary_variant(surface, False),
            axis_unsafe_boundary_variant(surface, True),
        )
    raise ValueError(f"unknown axis: {axis}")


def axis_data_shape(axis: str) -> str:
    return {
        "collection_empty_check": "list<int>",
        "literal_collection_membership": "set<string>",
        "map_key_membership": "map<string,string>+key",
        "literal_map_default_lookup": "map<string,int>+key",
        "map_default_lookup": "map<string,int>+key+fallback",
        "null_presence_predicate": "nullable<T>+alternate",
        "hof_filter_map": "list<int>+optional-emission",
        "nullish_default": "nullable<int>+fallback",
        "numeric_clamp": "scalar<int>+bounds",
        "numeric_minmax_abs": "scalar<int>+alternate",
        "projection_identity": "record<today:int,tomorrow:int>",
        "python_docstring_noop": "python-callable",
        "string_prefix_suffix": "string",
        "table_access": "map<string,int>",
        "total_order_compare": "ordered-scalar-pair",
        "java_statically_false_loop": "java-array-scan",
        "java_integer_low_bit_toggle": "java-int-edge-key",
        "c_u16_be_byte_pack": "c-byte-buffer",
        "c_u32_be_byte_pack": "c-byte-buffer",
    }.get(axis, "scalar<int>")


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


def make_axis_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    capability = surface_capability(capabilities, surface, axis)
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left, right = axis_variants(surface, proposal_id, axis, negative)
    left_path = rel_source_path(case_id, "left", surface)
    right_path = rel_source_path(case_id, "right", surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    exact_supported = capability_exact_supported(capabilities, surface, axis)
    equivalent = semantic_status == "equivalent"
    transform_tags = [axis, "semantic-axis"]
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal_id,
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": equivalent and exact_supported,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": axis,
            "representations": [left.representation, right.representation],
            "data_shape": axis_data_shape(axis),
            "language_relation": "same-surface",
            "negative_tag": negative_tag,
            "semantic_axes": [axis],
            "capabilities": {axis: capability},
            "template_split": split,
        },
        "left": source_record(surface, left, left_path),
        "right": source_record(surface, right, right_path),
        "evidence": axis_evidence(axis, semantic_status, negative, proposal_id),
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": {
                "max_lines": 12,
                "max_branch_count": 0,
                "max_primary_transforms": 1,
                "max_secondary_transforms": 1,
            },
        },
    }


def generate_axis_items(
    out_dir: Path,
    capabilities: dict,
    generation_filter: GenerationFilter,
) -> list[dict]:
    items: list[dict] = []
    for surface in SURFACES:
        for proposal_id, proposal in AXIS_PROPOSALS.items():
            axis = proposal["axis"]
            if not generation_filter.include_axis_proposal(proposal_id, axis):
                continue
            capability = surface_capability(capabilities, surface, axis)
            if proposal_id.startswith("axis_import_") and not import_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_nullish_") and not nullish_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_option_") and not nullish_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_null_presence_") and not null_presence_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_scalar_") and not scalar_abs_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_scalar_rust_"):
                continue
            if proposal_id.startswith(
                "axis_numeric_clamp_"
            ) and not numeric_clamp_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_hof_filter_map_"):
                continue
            if proposal_id.startswith(
                "axis_total_order_compare_"
            ) and not total_order_compare_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_java_dead_loop_") and not java_dead_loop_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                "axis_java_low_bit_toggle_"
            ) and not java_low_bit_toggle_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith(
                "axis_c_u16_be_byte_pack_"
            ) and not c_u16_be_byte_pack_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith(
                "axis_c_u32_be_byte_pack_"
            ) and not c_u32_be_byte_pack_axis_supported(surface, proposal_id):
                continue
            if proposal_id.startswith("axis_own_property_") and not own_property_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id in {
                "axis_own_property_in_boundary",
                "axis_own_property_method_boundary",
                "axis_own_property_shadow_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "unproven-own-property-guard",
                    )
                )
                continue
            if proposal_id.startswith("axis_record_guard_") and not record_guard_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_collection_") and not collection_empty_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_string_") and not string_prefix_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_membership_") and not literal_membership_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_membership_python_alias_"):
                continue
            if proposal_id.startswith("axis_membership_python_deque_"):
                continue
            if proposal_id.startswith("axis_membership_ruby_set_"):
                continue
            if proposal_id.startswith("axis_membership_set_"):
                continue
            if proposal_id.startswith("axis_membership_array_some_"):
                continue
            if proposal_id.startswith("axis_membership_array_every_"):
                continue
            if proposal_id.startswith("axis_membership_array_indexof_"):
                continue
            if proposal_id.startswith("axis_membership_array_findindex_"):
                continue
            if proposal_id.startswith("axis_membership_array_filter_length_"):
                continue
            if proposal_id.startswith("axis_membership_java_"):
                continue
            if proposal_id.startswith("axis_membership_module_"):
                continue
            if proposal_id.startswith("axis_membership_local_"):
                continue
            if proposal_id.startswith("axis_membership_go_slices_"):
                continue
            if proposal_id.startswith("axis_membership_rust_local_"):
                continue
            if proposal_id.startswith("axis_membership_rust_std_"):
                continue
            if proposal_id.startswith("axis_map_key_") and not map_key_membership_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                ("axis_map_key_python_keys_", "axis_map_key_ts_array_from_keys_")
            ):
                continue
            if proposal_id.startswith(
                (
                    "axis_map_default_js_map_",
                    "axis_map_default_js_object_",
                    "axis_map_default_java_map_",
                    "axis_map_default_rust_",
                    "axis_map_default_go_map_",
                    "axis_map_default_go_zero_",
                    "axis_map_default_module_",
                )
            ):
                continue
            if proposal_id.startswith("axis_map_default_") and not literal_map_default_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith("axis_map_fallback_") and not map_default_lookup_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                (
                    "axis_map_fallback_ts_",
                    "axis_map_fallback_python_",
                    "axis_map_fallback_java_",
                )
            ):
                continue
            if proposal_id in {
                "axis_collection_threshold_boundary",
                "axis_collection_wrong_receiver_boundary",
                "axis_collection_typed_domain_array_boundary",
                "axis_collection_typed_domain_string_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "typed-empty-domain-boundary"
                        if proposal_id.startswith("axis_collection_typed_domain_")
                        else "collection-empty-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_java_dead_loop_false_init_boundary",
                "axis_java_dead_loop_positive_guard_boundary",
                "axis_java_dead_loop_reassigned_guard_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "java-dead-loop-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_java_low_bit_toggle_reversed_branch_boundary",
                "axis_java_low_bit_toggle_xor_two_boundary",
                "axis_java_low_bit_toggle_positive_one_boundary",
                "axis_java_low_bit_toggle_wrong_delta_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "java-low-bit-toggle-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_c_u16_be_byte_pack_wrong_order_boundary",
                "axis_c_u16_be_byte_pack_overlap_boundary",
                "axis_c_u16_be_byte_pack_wrong_byte_boundary",
                "axis_c_u16_be_byte_pack_unproven_alias_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "c-u16-byte-pack-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_c_u32_be_byte_pack_uncasted_high_boundary",
                "axis_c_u32_be_byte_pack_wrong_order_boundary",
                "axis_c_u32_be_byte_pack_wrong_byte_boundary",
                "axis_c_u32_be_byte_pack_wrong_alias_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "c-u32-byte-pack-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_string_affix_boundary",
                "axis_string_direction_boundary",
                "axis_string_wrong_receiver_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "string-prefix-suffix-boundary",
                    )
                )
                continue
            if proposal_id in {
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
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "literal-membership-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_key_wrong_key_boundary",
                "axis_map_key_wrong_map_boundary",
                "axis_map_key_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "map-key-membership-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_default_wrong_key_boundary",
                "axis_map_default_wrong_default_boundary",
                "axis_map_default_wrong_map_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "literal-map-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_map_fallback_wrong_key_boundary",
                "axis_map_fallback_wrong_default_boundary",
                "axis_map_fallback_wrong_map_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "map-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_record_guard_array_boundary",
                "axis_record_guard_null_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "incomplete-record-guard",
                    )
                )
                continue
            if proposal_id == "axis_nullish_truthy_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "truthy-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_option_wrong_default_boundary",
                "axis_option_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "option-default-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_null_presence_nonnull_boundary",
                "axis_null_presence_wrong_value_boundary",
                "axis_null_presence_iflet_none_boundary",
                "axis_null_presence_iflet_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "null-presence-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_scalar_abs_sign_boundary",
                "axis_scalar_abs_wrong_value_boundary",
                "axis_scalar_abs_shadowed_math_boundary",
                "axis_scalar_min_wrong_value_boundary",
                "axis_scalar_max_wrong_value_boundary",
                "axis_scalar_min_shadowed_math_boundary",
                "axis_scalar_max_shadowed_math_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "numeric-abs-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_numeric_clamp_unproven_boundary",
                "axis_numeric_clamp_swapped_bounds_boundary",
                "axis_numeric_clamp_float_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "numeric-clamp-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_total_order_compare_descending_boundary",
                "axis_total_order_compare_equal_boundary",
                "axis_total_order_compare_wrong_value_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "total-order-compare-boundary",
                    )
                )
                continue
            if proposal_id.startswith("axis_projection_") and not projection_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                "axis_python_docstring_"
            ) and not python_docstring_axis_supported(surface, proposal_id):
                continue
            if proposal_id in {
                "axis_projection_default_boundary",
                "axis_projection_dynamic_key_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "unproven-projection-binding",
                    )
                )
                continue
            if proposal_id in {
                "axis_python_docstring_returned_string_boundary",
                "axis_python_docstring_assigned_string_boundary",
                "axis_python_docstring_fstring_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "python-docstring-boundary",
                    )
                )
                continue
            if proposal_id in {"axis_import_unsafe_boundary", "axis_import_reexport_boundary"}:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "unknown",
                        "heldout",
                        "unproven-import-binding",
                    )
                )
                continue
            if proposal_id == "axis_import_namespace_member_wrong_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "import-member-boundary",
                    )
                )
                continue
            if proposal_id in {
                "axis_import_namespace_shadowed_param_unshadowed_mutation_boundary",
                "axis_import_namespace_shadowed_param_fake_receiver_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "import-namespace-shadow-boundary",
                    )
                )
                continue
            if axis == "table_access" and capability != "supported":
                continue
            if axis == "unsafe_boundary":
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "unknown",
                        "heldout",
                        "unproven-free-binding",
                    )
                )
                continue
            items.append(
                make_axis_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface,
                    "equivalent",
                    "dev",
                )
            )
            items.append(
                make_axis_item(
                    out_dir,
                    capabilities,
                    proposal_id,
                    surface,
                    "not_equivalent",
                    "heldout",
                    f"{axis}-semantic-mutation",
                )
            )
    return items


def make_axis_cross_item(
    out_dir: Path,
    capabilities: dict,
    proposal_id: str,
    left_surface: Surface,
    right_surface: Surface,
    semantic_status: str,
    split: str,
    negative_tag: str | None = None,
) -> dict:
    proposal = AXIS_PROPOSALS[proposal_id]
    axis = proposal["axis"]
    negative = semantic_status == "not_equivalent"
    case_id = stable_id(
        proposal_id,
        left_surface.key,
        right_surface.key,
        semantic_status,
        split,
        negative_tag or "positive",
    )
    left = axis_variants(left_surface, proposal_id, axis, False)[0]
    right = axis_variants(right_surface, proposal_id, axis, negative)[1]
    left_path = rel_source_path(case_id, "left", left_surface)
    right_path = rel_source_path(case_id, "right", right_surface)
    write_source(out_dir, left_path, left.source)
    write_source(out_dir, right_path, right.source)
    left_capability = surface_capability(capabilities, left_surface, axis)
    right_capability = surface_capability(capabilities, right_surface, axis)
    equivalent = semantic_status == "equivalent"
    expected = (
        equivalent
        and capability_exact_supported(capabilities, left_surface, axis)
        and capability_exact_supported(capabilities, right_surface, axis)
    )
    transform_tags = [axis, "semantic-axis"]
    if negative_tag is not None:
        transform_tags += ["hard-negative", negative_tag]
    return {
        "case_id": case_id,
        "proposal_id": proposal_id,
        "split": split,
        "semantic_status": semantic_status,
        "expected_exact_detect": expected,
        "semantic_scope": SEMANTIC_SCOPE,
        "transform_tags": transform_tags,
        "matrix": {
            "computation": axis,
            "representations": [left.representation, right.representation],
            "data_shape": axis_data_shape(axis),
            "language_relation": "cross-surface",
            "negative_tag": negative_tag,
            "semantic_axes": [axis],
            "capabilities": {
                f"{axis}:left": left_capability,
                f"{axis}:right": right_capability,
            },
            "template_split": split,
        },
        "left": source_record(left_surface, left, left_path),
        "right": source_record(right_surface, right, right_path),
        "evidence": axis_evidence(axis, semantic_status, negative, proposal_id),
        "llm_proposal": {
            "why": proposal["why"],
            "complexity_budget": {
                "max_lines": 12,
                "max_branch_count": 0,
                "max_primary_transforms": 1,
                "max_secondary_transforms": 1,
            },
        },
    }


def generate_hof_filter_map_cross_items(
    out_dir: Path,
    capabilities: dict,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("hof_filter_map"):
        return []
    surface_by_key = {surface.key: surface for surface in SURFACES}
    rust = surface_by_key["rust"]
    reference_surfaces = [surface_by_key["python"], surface_by_key["javascript"]]
    items: list[dict] = []
    if generation_filter.include_proposal("axis_hof_filter_map_identity"):
        for left_surface in reference_surfaces:
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "equivalent",
                    "heldout",
                )
            )
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_hof_filter_map_identity",
                    left_surface,
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_hof_filter_map_none_boundary",
        "axis_hof_filter_map_value_boundary",
        "axis_hof_filter_map_falsey_boundary",
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
                    rust,
                    "not_equivalent",
                    "heldout",
                    "hof-filter-map-boundary",
                )
            )
    return items


def generate_string_prefix_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("string_prefix_suffix"):
        return []
    surfaces = [s for s in SURFACES if string_prefix_axis_supported(s, "axis_string_prefix_identity")]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in ("axis_string_prefix_identity", "axis_string_suffix_identity"):
            if not generation_filter.include_proposal(proposal_id):
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
                    "string_prefix_suffix-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_string_affix_boundary",
            "axis_string_direction_boundary",
            "axis_string_wrong_receiver_boundary",
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
                    "string-prefix-suffix-boundary",
                )
            )
    return items


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


def generate_null_presence_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("null_presence_predicate"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if null_presence_axis_supported(s, "axis_null_presence_method_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        if generation_filter.include_proposal("axis_null_presence_method_identity"):
            items.append(
                make_axis_cross_item(
                    out_dir,
                    capabilities,
                    "axis_null_presence_method_identity",
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
                    "axis_null_presence_method_identity",
                    left_surface,
                    right_surface,
                    "not_equivalent",
                    "heldout",
                    "null_presence_predicate-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_null_presence_nonnull_boundary",
            "axis_null_presence_wrong_value_boundary",
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
                    "null-presence-boundary",
                )
            )
    return items


def generate_scalar_abs_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    surfaces = [
        s
        for s in SURFACES
        if scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    items: list[dict] = []
    for left_surface, right_surface in cross_pairs(surfaces, cross_mode):
        for proposal_id in (
            "axis_scalar_abs_function_identity",
            "axis_scalar_min_function_identity",
            "axis_scalar_max_function_identity",
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
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
        for proposal_id in (
            "axis_scalar_abs_sign_boundary",
            "axis_scalar_abs_wrong_value_boundary",
            "axis_scalar_min_wrong_value_boundary",
            "axis_scalar_max_wrong_value_boundary",
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
                    "numeric-abs-boundary",
                )
            )
    return items


def generate_rust_numeric_method_cross_items(
    out_dir: Path,
    capabilities: dict,
    cross_mode: str,
    generation_filter: GenerationFilter,
) -> list[dict]:
    if cross_mode == "none" or not generation_filter.include_axis("numeric_minmax_abs"):
        return []
    rust_surface = next(s for s in SURFACES if s.key == "rust")
    reference_surfaces = [
        s
        for s in SURFACES
        if s.key != "rust" and scalar_abs_axis_supported(s, "axis_scalar_abs_function_identity")
    ]
    if cross_mode == "ring":
        reference_surfaces = reference_surfaces[:3]
    items: list[dict] = []
    for proposal_id in (
        "axis_scalar_rust_abs_method_identity",
        "axis_scalar_rust_min_method_identity",
        "axis_scalar_rust_max_method_identity",
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
                    rust_surface,
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
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric_minmax_abs-semantic-mutation",
                )
            )
    for proposal_id in (
        "axis_scalar_rust_abs_wrong_value_boundary",
        "axis_scalar_rust_min_wrong_value_boundary",
        "axis_scalar_rust_max_wrong_value_boundary",
        "axis_scalar_rust_abs_custom_method_boundary",
        "axis_scalar_rust_min_custom_method_boundary",
        "axis_scalar_rust_max_custom_method_boundary",
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
                    rust_surface,
                    "not_equivalent",
                    "heldout",
                    "numeric-rust-method-boundary",
                )
            )
    return items


def cross_pairs(surfaces: list[Surface], mode: str) -> list[tuple[Surface, Surface]]:
    if mode == "none":
        return []
    if mode == "ring":
        return [(surfaces[i], surfaces[(i + 1) % len(surfaces)]) for i in range(len(surfaces))]
    if mode == "all":
        return [(a, b) for i, a in enumerate(surfaces) for b in surfaces[i + 1 :]]
    raise ValueError(f"unknown cross mode: {mode}")


def split_filters(values: list[str] | None) -> tuple[str, ...]:
    if not values:
        return ()
    parts: list[str] = []
    for value in values:
        parts.extend(part.strip() for part in value.split(",") if part.strip())
    return tuple(dict.fromkeys(parts))


def generate(
    out_dir: Path,
    proposal_file: Path,
    capability_file: Path,
    cross_mode: str,
    clean: bool,
    generation_filter: GenerationFilter,
) -> dict:
    if clean and out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    proposal_file = proposal_file.resolve()
    capability_file = capability_file.resolve()
    proposals_doc = json.loads(proposal_file.read_text())
    capabilities = load_capabilities(capability_file)
    validate_proposals(proposals_doc)
    items = []
    for proposal in proposals_doc["proposals"]:
        if not generation_filter.include_base_proposal(proposal):
            continue
        for surface in SURFACES:
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "aggregate",
                    "equivalent",
                    "same-surface",
                    "dev",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "aggregate",
                    "not_equivalent",
                    "same-surface",
                    "heldout",
                    "aggregate-semantic-mutation",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    surface,
                    surface,
                    "loop",
                    "not_equivalent",
                    "same-surface",
                    "heldout",
                    "same-template-semantic-mutation",
                )
            )
            if OPERATIONS[proposal["operation"]].arity == 1:
                items.append(
                    make_item(
                        out_dir,
                        proposal,
                        surface,
                        surface,
                        "indexed_loop",
                        "equivalent",
                        "same-surface",
                        "heldout",
                    )
                )
                items.append(
                    make_item(
                        out_dir,
                        proposal,
                        surface,
                        surface,
                        "indexed_loop",
                        "not_equivalent",
                        "same-surface",
                        "heldout",
                        "indexed-template-semantic-mutation",
                    )
                )
        for representation in ("c_start_one", "c_stride_two"):
            items.append(make_c_contract_negative_item(out_dir, proposal, representation))
        for left_surface, right_surface in cross_pairs(SURFACES, cross_mode):
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    left_surface,
                    right_surface,
                    "loop",
                    "equivalent",
                    "cross-surface",
                    "heldout",
                )
            )
            items.append(
                make_item(
                    out_dir,
                    proposal,
                    left_surface,
                    right_surface,
                    "loop",
                    "not_equivalent",
                    "cross-surface",
                    "heldout",
                    "cross-template-semantic-mutation",
                )
            )
    items.extend(generate_axis_items(out_dir, capabilities, generation_filter))
    items.extend(generate_hof_filter_map_cross_items(out_dir, capabilities, generation_filter))
    items.extend(
        generate_string_prefix_cross_items(out_dir, capabilities, cross_mode, generation_filter)
    )
    items.extend(
        generate_literal_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_java_factory_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_map_key_membership_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_literal_map_default_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_map_default_lookup_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_null_presence_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_scalar_abs_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    items.extend(
        generate_rust_numeric_method_cross_items(
            out_dir, capabilities, cross_mode, generation_filter
        )
    )
    return {
        "schema_version": "0.1.0",
        "source": {
            "generator": "bench/type4/generate.py",
            "proposal_file": str(proposal_file.relative_to(ROOT)),
            "capability_file": str(capability_file.relative_to(ROOT)),
            "cross_mode": cross_mode,
            "axis_filter": sorted(generation_filter.axes),
            "proposal_prefix_filter": list(generation_filter.proposal_prefixes),
        },
        "items": items,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--proposal-file", default=DEFAULT_PROPOSALS, type=Path)
    parser.add_argument("--capability-file", default=DEFAULT_CAPABILITIES, type=Path)
    parser.add_argument("--cross", choices=["none", "ring", "all"], default="ring")
    parser.add_argument(
        "--axis",
        action="append",
        help="only generate cases whose semantic axis/computation matches this value; may be repeated or comma-separated",
    )
    parser.add_argument(
        "--proposal-prefix",
        action="append",
        help="only generate proposal ids with this prefix; may be repeated or comma-separated",
    )
    parser.add_argument("--no-clean", action="store_true", help="do not clear the output directory first")
    args = parser.parse_args()
    generation_filter = GenerationFilter(
        axes=frozenset(split_filters(args.axis)),
        proposal_prefixes=split_filters(args.proposal_prefix),
    )
    manifest = generate(
        args.out_dir,
        args.proposal_file,
        args.capability_file,
        args.cross,
        clean=not args.no_clean,
        generation_filter=generation_filter,
    )
    manifest_path = args.out_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    by_status: dict[str, int] = {}
    for item in manifest["items"]:
        by_status[item["semantic_status"]] = by_status.get(item["semantic_status"], 0) + 1
    print(f"wrote {len(manifest['items'])} items to {manifest_path}")
    print("status:", ", ".join(f"{k}={v}" for k, v in sorted(by_status.items())))


if __name__ == "__main__":
    main()
