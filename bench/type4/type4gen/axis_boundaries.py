"""Projection, unsafe, docstring, and import-boundary axis templates."""

from __future__ import annotations

from .model import JS_LIKE_SURFACES, Surface, Variant, js_axis_source


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

    if surface.key == "swift":
        name = "buildCase" if right else "axisCase"
        if right:
            src = f"""func {name}(_ row: Reading, _ amount: Int) -> Int {{
    let selected = row.{field}
    return amount + selected
}}
"""
        else:
            src = f"""func {name}(_ record: Reading, _ value: Int) -> Int {{
    return value + record.today
}}
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
    if surface.key == "swift":
        swift_name = "buildCase" if right else "axisCase"
        src = f"""func {swift_name}(_ value: Int) -> Int {{
    return value + AMBIENT_LIMIT
}}
"""
        return Variant("axis", src, swift_name)
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
        if surface.key == "swift":
            swift_entry = "buildCase" if right else "axisCase"
            src = f"""func {swift_entry}(_ value: Int) -> Int {{
    return helper(value + 1)
}}
"""
            return Variant("axis", src, swift_entry)
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
        prefix = (
            'import * as path from "node:path";\n\n'
            'function touchPath() {\n  path.replaceAll("x", "y");\n}\n\n'
            if right
            else 'import * as path from "node:path";\n\n'
        )
        return prefix + (wrong_template_body if right and negative else template_body)
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
