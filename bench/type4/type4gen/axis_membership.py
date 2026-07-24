"""Literal and typed collection-membership axis source templates."""

from __future__ import annotations

from .model import JS_LIKE_SURFACES, Surface, Variant, js_axis_source


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
