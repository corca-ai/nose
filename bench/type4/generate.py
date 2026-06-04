#!/usr/bin/env python3
"""Generate the seed corpus for the evidence-carrying Type-4 benchmark factory."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROPOSALS = ROOT / "bench" / "type4" / "proposals.v1.json"
DEFAULT_CAPABILITIES = ROOT / "bench" / "type4" / "capabilities.v1.json"

SEMANTIC_SCOPE = (
    "pure integer lists, deterministic evaluation, no overflow modeling; "
    "C `(int *xs, int n)` and aligned `(int *a, int *b, int n)` cases treat `n` "
    "as the exact logical length of the traversed array(s); "
    "C predicate reductions use 1/0 as boolean true/false"
)
PROPERTY_INPUTS = [
    [],
    [-1, 0, 2],
    [0],
    [0, -1],
    [0, 2],
    [1, -2, 3],
    [1, 0],
    [1, 2],
    [2],
    [2, 3],
    [3],
    [-2, 4],
]
PAIR_PROPERTY_INPUTS = [
    {"a": [], "b": []},
    {"a": [1], "b": [2]},
    {"a": [1, 2], "b": [3, 4]},
    {"a": [-1, 2, 0], "b": [4, -3, 5]},
    {"a": [0, 2, 3], "b": [7, 0, -1]},
    {"a": [-2, 4], "b": [-5, 6]},
]

AXIS_PROPOSALS = {
    "axis_immutable_binding": {
        "axis": "immutable_binding",
        "why": "Strict proof must carry immutable binding values instead of treating free names as equal.",
    },
    "axis_proven_callee_identity": {
        "axis": "proven_callee_identity",
        "why": "Opaque calls are exact only when the callee binding identity is proven and behavior-defining.",
    },
    "axis_import_named_identity": {
        "axis": "import_identity",
        "why": "Static named imports should prove helper identity by module coordinate and exported symbol.",
    },
    "axis_import_alias_identity": {
        "axis": "import_identity",
        "why": "Aliases should not break exact helper identity when the imported coordinate is unchanged.",
    },
    "axis_import_namespace_identity": {
        "axis": "import_identity",
        "why": "Namespace imports should prove receiver identity before member calls become exact.",
    },
    "axis_import_namespace_member_identity": {
        "axis": "import_identity",
        "why": "A named import and a namespace member import should prove the same exported helper coordinate.",
    },
    "axis_import_default_identity": {
        "axis": "import_identity",
        "why": "Default imports are a distinct static import coordinate, not a free-name guess.",
    },
    "axis_import_default_named_boundary": {
        "axis": "import_identity",
        "why": "A default export and a named export with the same local spelling are different coordinates.",
    },
    "axis_import_multi_specifier_identity": {
        "axis": "import_identity",
        "why": "Multiple static specifiers in one import statement should still prove each local binding separately.",
    },
    "axis_import_namespace_member_wrong_boundary": {
        "axis": "import_identity",
        "why": "Namespace member identity is a proof over a specific exported member coordinate.",
    },
    "axis_import_reexport_boundary": {
        "axis": "import_identity",
        "why": "Re-export syntax does not create a proven local binding for strict exact calls.",
    },
    "axis_import_unsafe_boundary": {
        "axis": "import_identity",
        "why": "Wildcard, dynamic, or unresolved import forms must stay outside strict exact reporting.",
    },
    "axis_nullish_coalesce_identity": {
        "axis": "nullish_default",
        "why": "Nullish coalescing should converge with the equivalent explicit null/undefined defaulting condition.",
    },
    "axis_nullish_guard_identity": {
        "axis": "nullish_default",
        "why": "A guard return for a nullish value should prove the same defaulting behavior as nullish coalescing.",
    },
    "axis_nullish_truthy_boundary": {
        "axis": "nullish_default",
        "why": "Truthy-or defaulting is not equivalent to nullish defaulting for falsy non-null values.",
    },
    "axis_option_unwrap_or_identity": {
        "axis": "nullish_default",
        "why": "Rust `Option::unwrap_or` should prove the same value-or-fallback behavior as nullish defaulting.",
    },
    "axis_option_unwrap_or_else_identity": {
        "axis": "nullish_default",
        "why": "A capture-only `Option::unwrap_or_else(|| fallback)` should prove the same value-or-fallback behavior as `unwrap_or`.",
    },
    "axis_option_map_or_identity": {
        "axis": "nullish_default",
        "why": "Rust `Option::map_or(fallback, |inner| inner)` should prove the same value-or-fallback behavior as `unwrap_or`.",
    },
    "axis_option_wrong_default_boundary": {
        "axis": "nullish_default",
        "why": "Option defaulting is a proof over a specific fallback coordinate.",
    },
    "axis_option_wrong_value_boundary": {
        "axis": "nullish_default",
        "why": "Option defaulting is a proof over a specific optional value coordinate.",
    },
    "axis_null_presence_method_identity": {
        "axis": "null_presence_predicate",
        "why": "Null/none/nil method predicates should prove the same absence check as explicit null comparison.",
    },
    "axis_null_presence_nonnull_boundary": {
        "axis": "null_presence_predicate",
        "why": "Presence and absence predicates are opposite directions and must not merge.",
    },
    "axis_null_presence_wrong_value_boundary": {
        "axis": "null_presence_predicate",
        "why": "Null presence is a proof over a specific value coordinate; checking another value is not equivalent.",
    },
    "axis_null_presence_iflet_some_identity": {
        "axis": "null_presence_predicate",
        "why": "Rust `if let Some(_)` presence tests should prove the same option-presence predicate as `is_some()`.",
    },
    "axis_null_presence_iflet_none_boundary": {
        "axis": "null_presence_predicate",
        "why": "Rust `if let None` and `if let Some(_)` have opposite option-presence directions and must not merge.",
    },
    "axis_null_presence_iflet_wrong_value_boundary": {
        "axis": "null_presence_predicate",
        "why": "Rust option-pattern presence is a proof over a specific option value coordinate.",
    },
    "axis_scalar_abs_function_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar absolute-value builtins should prove the same sign-normalizing expression as the explicit conditional idiom.",
    },
    "axis_scalar_abs_sign_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Absolute value and signed identity differ for negative inputs and must not merge.",
    },
    "axis_scalar_abs_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar absolute value is a proof over a specific numeric value coordinate.",
    },
    "axis_scalar_abs_shadowed_math_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Shadowed JavaScript Math bindings are not the built-in absolute-value proof.",
    },
    "axis_scalar_min_function_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar minimum builtins should prove the same two-way selection as the explicit conditional idiom.",
    },
    "axis_scalar_max_function_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar maximum builtins should prove the same two-way selection as the explicit conditional idiom.",
    },
    "axis_scalar_min_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar minimum is a proof over a specific pair of numeric value coordinates.",
    },
    "axis_scalar_max_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Scalar maximum is a proof over a specific pair of numeric value coordinates.",
    },
    "axis_scalar_min_shadowed_math_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Shadowed JavaScript Math bindings are not the built-in minimum proof.",
    },
    "axis_scalar_max_shadowed_math_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Shadowed JavaScript Math bindings are not the built-in maximum proof.",
    },
    "axis_scalar_rust_abs_method_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.abs()` should prove the same scalar absolute-value semantics as conditional and builtin forms.",
    },
    "axis_scalar_rust_min_method_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.min()` should prove the same scalar minimum semantics as conditional and builtin forms.",
    },
    "axis_scalar_rust_max_method_identity": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.max()` should prove the same scalar maximum semantics as conditional and builtin forms.",
    },
    "axis_scalar_rust_abs_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.abs()` over a different value coordinate changes behavior.",
    },
    "axis_scalar_rust_min_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.min()` over a different right-hand value coordinate changes behavior.",
    },
    "axis_scalar_rust_max_wrong_value_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "Rust numeric `.max()` over a different right-hand value coordinate changes behavior.",
    },
    "axis_scalar_rust_abs_custom_method_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "A Rust custom `.abs()` method is not a numeric intrinsic and must stay outside strict scalar normalization.",
    },
    "axis_scalar_rust_min_custom_method_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "A Rust custom `.min()` method is not a numeric intrinsic and must stay outside strict scalar normalization.",
    },
    "axis_scalar_rust_max_custom_method_boundary": {
        "axis": "numeric_minmax_abs",
        "why": "A Rust custom `.max()` method is not a numeric intrinsic and must stay outside strict scalar normalization.",
    },
    "axis_own_property_hasown_identity": {
        "axis": "own_property_guard",
        "why": "Object.hasOwn and Object.prototype.hasOwnProperty.call prove the same own-property presence check.",
    },
    "axis_own_property_in_boundary": {
        "axis": "own_property_guard",
        "why": "The `in` operator includes prototype properties and must not merge with an own-property guard.",
    },
    "axis_own_property_method_boundary": {
        "axis": "own_property_guard",
        "why": "A direct hasOwnProperty method call can be shadowed and is not a strict own-property proof.",
    },
    "axis_own_property_shadow_boundary": {
        "axis": "own_property_guard",
        "why": "A locally shadowed Object binding is not the built-in Object.hasOwn proof.",
    },
    "axis_projection_temp_identity": {
        "axis": "projection_identity",
        "why": "Projecting the same static field through a temporary binding should preserve exact value identity.",
    },
    "axis_projection_destructure_identity": {
        "axis": "projection_identity",
        "why": "Static destructuring patterns should prove the same field projection as direct member access.",
    },
    "axis_projection_destructure_shorthand_identity": {
        "axis": "projection_identity",
        "why": "Shorthand destructuring should prove the same field projection as direct member access.",
    },
    "axis_projection_destructure_multi_identity": {
        "axis": "projection_identity",
        "why": "Multiple static destructuring fields should still prove each selected field independently.",
    },
    "axis_projection_static_key_identity": {
        "axis": "projection_identity",
        "why": "Static string-key property access should prove the same coordinate as dotted member access where the surface semantics make them identical.",
    },
    "axis_projection_default_boundary": {
        "axis": "projection_identity",
        "why": "Destructuring defaults change behavior when the field is absent and must not become strict projection evidence without a presence proof.",
    },
    "axis_projection_dynamic_key_boundary": {
        "axis": "projection_identity",
        "why": "Dynamic property keys do not prove a fixed projected coordinate for strict exact reporting.",
    },
    "axis_record_guard_order_identity": {
        "axis": "record_shape_guard",
        "why": "A complete record-shape guard should be order-insensitive across its static clauses.",
    },
    "axis_record_guard_truthy_identity": {
        "axis": "record_shape_guard",
        "why": "A truthiness guard is equivalent to a non-null guard when paired with a static typeof-object clause.",
    },
    "axis_record_guard_array_boundary": {
        "axis": "record_shape_guard",
        "why": "A non-null object guard without the array exclusion is not a strict record guard.",
    },
    "axis_record_guard_null_boundary": {
        "axis": "record_shape_guard",
        "why": "A typeof-object and non-array guard without a null exclusion still accepts null.",
    },
    "axis_collection_empty_named_identity": {
        "axis": "collection_empty_check",
        "why": "Named emptiness predicates and zero-length comparisons should prove the same collection-empty check when the receiver coordinate is fixed.",
    },
    "axis_collection_nonempty_named_identity": {
        "axis": "collection_empty_check",
        "why": "Negated named emptiness predicates and nonzero length comparisons should prove the same collection-nonempty check when the receiver coordinate is fixed.",
    },
    "axis_collection_threshold_boundary": {
        "axis": "collection_empty_check",
        "why": "A zero-length check and a one-length check differ and must not merge as strict collection emptiness.",
    },
    "axis_collection_wrong_receiver_boundary": {
        "axis": "collection_empty_check",
        "why": "Length or emptiness checks over different collection parameters are different proof coordinates.",
    },
    "axis_string_prefix_identity": {
        "axis": "string_prefix_suffix",
        "why": "Case-sensitive starts-with predicates should prove the same string-prefix check when receiver and literal prefix coordinates are fixed.",
    },
    "axis_string_suffix_identity": {
        "axis": "string_prefix_suffix",
        "why": "Case-sensitive ends-with predicates should prove the same string-suffix check when receiver and literal suffix coordinates are fixed.",
    },
    "axis_string_affix_boundary": {
        "axis": "string_prefix_suffix",
        "why": "Different literal affixes are different proof coordinates and must not merge.",
    },
    "axis_string_direction_boundary": {
        "axis": "string_prefix_suffix",
        "why": "A prefix predicate is not equivalent to a suffix predicate even when the literal affix is the same.",
    },
    "axis_string_wrong_receiver_boundary": {
        "axis": "string_prefix_suffix",
        "why": "Prefix/suffix checks over different string parameters are different proof coordinates.",
    },
    "axis_membership_literal_identity": {
        "axis": "literal_collection_membership",
        "why": "Static literal collection membership should prove the same element-in-collection predicate when element and literal set coordinates are fixed.",
    },
    "axis_membership_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Membership checks over different element parameters are different proof coordinates.",
    },
    "axis_membership_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Different literal collections are different proof coordinates even when their APIs look identical.",
    },
    "axis_membership_substring_boundary": {
        "axis": "literal_collection_membership",
        "why": "Substring contains and static literal collection membership are different semantics and must not merge.",
    },
    "axis_membership_unproven_receiver_boundary": {
        "axis": "literal_collection_membership",
        "why": "Receiver-overloaded membership-like calls are not strict proof unless the collection or map receiver coordinate is proven.",
    },
    "axis_membership_typed_receiver_identity": {
        "axis": "literal_collection_membership",
        "why": "Typed collection receivers should prove dynamic element-in-collection membership without relying on method names alone.",
    },
    "axis_membership_typed_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Typed dynamic collection membership is still a proof over a specific element coordinate.",
    },
    "axis_membership_typed_string_boundary": {
        "axis": "literal_collection_membership",
        "why": "A typed string receiver's substring predicate is not dynamic collection membership.",
    },
    "axis_membership_set_param_identity": {
        "axis": "literal_collection_membership",
        "why": "A typed TypeScript `Set<T>.has(value)` receiver should prove the same collection-membership predicate as other typed dynamic collections.",
    },
    "axis_membership_set_inline_identity": {
        "axis": "literal_collection_membership",
        "why": "An inline `new Set([...]).has(value)` over a static literal should prove the same membership predicate as literal collection APIs.",
    },
    "axis_membership_set_local_identity": {
        "axis": "literal_collection_membership",
        "why": "A local immutable `new Set([...])` binding should preserve literal collection-membership proof coordinates.",
    },
    "axis_membership_set_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Set construction membership over a different element parameter changes the proof coordinate.",
    },
    "axis_membership_set_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Set construction membership over different literal items changes membership behavior.",
    },
    "axis_membership_set_untyped_receiver_boundary": {
        "axis": "literal_collection_membership",
        "why": "An arbitrary `.has` receiver is not proof of strict collection-membership semantics.",
    },
    "axis_membership_java_list_of_identity": {
        "axis": "literal_collection_membership",
        "why": "Java `List.of(...).contains(value)` over static literal items should prove the same element-in-collection predicate as other literal collection APIs.",
    },
    "axis_membership_java_set_of_identity": {
        "axis": "literal_collection_membership",
        "why": "Java `Set.of(...).contains(value)` over static literal items should prove the same element-in-collection predicate as other literal collection APIs.",
    },
    "axis_membership_java_arrays_aslist_identity": {
        "axis": "literal_collection_membership",
        "why": "Java `Arrays.asList(...).contains(value)` over static literal items should prove the same element-in-collection predicate as other literal collection APIs.",
    },
    "axis_membership_java_list_of_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `List.of(...).contains(...)` is still a proof over a specific element coordinate.",
    },
    "axis_membership_java_set_of_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `Set.of(...).contains(...)` is still a proof over a specific element coordinate.",
    },
    "axis_membership_java_arrays_aslist_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `Arrays.asList(...).contains(...)` is still a proof over a specific element coordinate.",
    },
    "axis_membership_java_list_of_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `List.of(...).contains(value)` over different literal items changes membership behavior.",
    },
    "axis_membership_java_set_of_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `Set.of(...).contains(value)` over different literal items changes membership behavior.",
    },
    "axis_membership_java_arrays_aslist_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Java `Arrays.asList(...).contains(value)` over different literal items changes membership behavior.",
    },
    "axis_membership_java_list_of_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A locally shadowed Java `List` name is not proof of the standard `java.util.List.of` collection factory.",
    },
    "axis_membership_java_set_of_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A locally shadowed Java `Set` name is not proof of the standard `java.util.Set.of` collection factory.",
    },
    "axis_membership_java_arrays_aslist_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A locally shadowed Java `Arrays` name is not proof of the standard `java.util.Arrays.asList` collection factory.",
    },
    "axis_membership_module_js_set_identity": {
        "axis": "literal_collection_membership",
        "why": "A module-level immutable JavaScript `Set` binding should prove literal collection membership when the binding is not mutated.",
    },
    "axis_membership_module_ts_set_identity": {
        "axis": "literal_collection_membership",
        "why": "A module-level immutable TypeScript `Set` binding should prove literal collection membership when the binding is not mutated.",
    },
    "axis_membership_module_java_list_identity": {
        "axis": "literal_collection_membership",
        "why": "A Java static final `List.of(...)` binding should prove literal collection membership through the same collection/element coordinates.",
    },
    "axis_membership_module_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Module-level collection membership over different element parameters is a different proof coordinate.",
    },
    "axis_membership_module_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Module-level collection membership over different static items changes membership behavior.",
    },
    "axis_membership_module_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A module-level collection binding mutated after construction is not a strict literal collection proof.",
    },
    "axis_membership_module_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A module-level collection factory with a shadowed `Set` constructor or Java `List` type is not proof of a standard collection factory.",
    },
    "axis_membership_go_slices_package_identity": {
        "axis": "literal_collection_membership",
        "why": "Go `slices.Contains` over a package-level immutable slice literal should prove literal collection membership.",
    },
    "axis_membership_go_slices_alias_package_identity": {
        "axis": "literal_collection_membership",
        "why": "An aliased Go import of `slices` should preserve the same strict package coordinate for `Contains` membership.",
    },
    "axis_membership_go_slices_const_package_identity": {
        "axis": "literal_collection_membership",
        "why": "A Go package-level slice literal built from immutable const elements should prove the same literal collection membership.",
    },
    "axis_membership_go_slices_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Go `slices.Contains` over a package-level slice is still a proof over a specific element coordinate.",
    },
    "axis_membership_go_slices_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Go `slices.Contains` over a different package-level literal slice changes membership behavior.",
    },
    "axis_membership_go_slices_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Go package-level slice expanded through `append` is not a strict literal collection proof.",
    },
    "axis_membership_go_slices_unimported_boundary": {
        "axis": "literal_collection_membership",
        "why": "A receiver named `slices` is not proof of the standard Go `slices` package without a static import coordinate.",
    },
    "axis_membership_rust_local_array_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust local immutable array literal followed by `.contains` should prove literal collection membership.",
    },
    "axis_membership_rust_local_typed_array_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust local array literal with an explicit array type should prove the same literal collection membership.",
    },
    "axis_membership_rust_local_slice_ref_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust local slice reference to a literal array should prove the same literal collection membership.",
    },
    "axis_membership_rust_local_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Rust local literal collection membership remains tied to a specific element coordinate.",
    },
    "axis_membership_rust_local_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Rust local literal collection membership remains tied to a specific collection value.",
    },
    "axis_membership_rust_local_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Rust local vector mutated after construction is not a strict literal collection proof.",
    },
    "axis_membership_rust_local_custom_receiver_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Rust receiver with a custom `contains` method is not proof of literal collection membership.",
    },
    "axis_map_key_membership_identity": {
        "axis": "map_key_membership",
        "why": "Map key-presence APIs should prove the same key-in-map predicate when receiver and key coordinates are fixed.",
    },
    "axis_map_key_wrong_key_boundary": {
        "axis": "map_key_membership",
        "why": "Map key membership is a proof over a specific key coordinate.",
    },
    "axis_map_key_wrong_map_boundary": {
        "axis": "map_key_membership",
        "why": "Map key membership is a proof over a specific map receiver coordinate.",
    },
    "axis_map_key_value_boundary": {
        "axis": "map_key_membership",
        "why": "Map value membership is not the same predicate as map key membership.",
    },
    "axis_map_default_literal_identity": {
        "axis": "literal_map_default_lookup",
        "why": "Static literal-map lookup with a literal fallback should prove the same key/default behavior across map APIs.",
    },
    "axis_map_default_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Map default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Different fallback values change missing-key behavior and must not merge.",
    },
    "axis_map_default_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Different literal map values change present-key behavior and must not merge.",
    },
    "axis_map_default_js_map_inline_identity": {
        "axis": "literal_map_default_lookup",
        "why": "An inline JavaScript/TypeScript `new Map([...]).get(key) ?? fallback` over static entries should prove the same literal-map default lookup.",
    },
    "axis_map_default_js_map_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local immutable JavaScript/TypeScript `new Map([...])` binding should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_js_map_has_get_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A proven JavaScript/TypeScript `Map.has(key) ? Map.get(key) : fallback` over static entries should prove literal-map default lookup.",
    },
    "axis_map_default_js_map_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Constructed Map default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_js_map_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Constructed Map default lookups with different fallbacks change missing-key behavior.",
    },
    "axis_map_default_js_map_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Constructed Map default lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_js_map_untyped_receiver_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "An arbitrary `.get` receiver is not proof of strict literal-map default semantics.",
    },
    "axis_map_default_js_map_shadowed_constructor_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A shadowed `Map` constructor cannot prove static literal-map default semantics.",
    },
    "axis_map_default_js_object_hasown_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A static JavaScript/TypeScript object literal guarded by `Object.hasOwn` should prove literal-map default lookup.",
    },
    "axis_map_default_js_object_call_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A static JavaScript/TypeScript object literal guarded by `Object.prototype.hasOwnProperty.call` should prove literal-map default lookup.",
    },
    "axis_map_default_js_object_negated_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A negated own-property guard around a static JavaScript/TypeScript object literal should prove the same literal-map default lookup.",
    },
    "axis_map_default_js_object_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Object-literal default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_js_object_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Object-literal default lookups with different fallbacks change missing-key behavior.",
    },
    "axis_map_default_js_object_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Object-literal default lookups over different static values change present-key behavior.",
    },
    "axis_map_default_js_object_unguarded_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A nullish default over arbitrary object indexing is not an own-property proof.",
    },
    "axis_map_default_js_object_in_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "The JavaScript `in` operator includes prototype properties and is not strict map-key presence.",
    },
    "axis_map_default_js_object_method_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A direct `hasOwnProperty` method call can be shadowed and is not a strict own-property proof.",
    },
    "axis_map_default_js_object_shadowed_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A locally shadowed `Object` binding is not the built-in own-property proof.",
    },
    "axis_map_default_java_map_of_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Java `Map.of(...).getOrDefault(key, fallback)` over static entries should prove the same literal-map default lookup.",
    },
    "axis_map_default_java_map_of_entries_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Java `Map.ofEntries(Map.entry(...)).getOrDefault(key, fallback)` over static entries should prove the same literal-map default lookup.",
    },
    "axis_map_default_java_map_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Java immutable `Map.of(...)` binding should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_java_map_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Java literal-map factory default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_java_map_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Java literal-map factory default lookups with different fallbacks change missing-key behavior.",
    },
    "axis_map_default_java_map_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Java literal-map factory default lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_java_map_shadowed_factory_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A local Java variable named `Map` is not proof of the standard `java.util.Map` factory.",
    },
    "axis_map_default_java_map_type_shadow_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A same-file Java class named `Map` is not proof of the standard `java.util.Map` factory.",
    },
    "axis_map_default_rust_hashmap_from_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Rust `std::collections::HashMap::from([...])` lookup with `unwrap_or` over static entries should prove literal-map default lookup.",
    },
    "axis_map_default_rust_btreemap_from_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Rust `std::collections::BTreeMap::from([...])` lookup with `unwrap_or` over static entries should prove the same literal-map default lookup.",
    },
    "axis_map_default_rust_hashmap_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Rust binding initialized from `std::collections::HashMap::from([...])` should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_rust_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Rust std map factory default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_rust_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Rust std map factory default lookups with different fallbacks change missing-key behavior.",
    },
    "axis_map_default_rust_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Rust std map factory default lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_rust_mutated_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A local Rust map binding mutated after construction is not a strict literal-map proof.",
    },
    "axis_map_default_module_js_map_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A module-level immutable JavaScript `new Map([...])` binding should prove literal-map default lookup when the binding is not mutated.",
    },
    "axis_map_default_module_ts_map_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A module-level immutable TypeScript `new Map([...])` binding should prove literal-map default lookup when the binding is not mutated.",
    },
    "axis_map_default_module_java_map_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Java static final `Map.of(...)` binding should prove literal-map default lookup through the same map/key/default coordinates.",
    },
    "axis_map_default_module_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Module-level map default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_module_wrong_default_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Module-level map default lookups with different fallbacks change missing-key behavior.",
    },
    "axis_map_default_module_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Module-level map default lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_module_mutated_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A module-level `Map` binding that is mutated after construction is not a strict literal-map proof.",
    },
    "axis_map_default_module_shadowed_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A module-level map factory with a shadowed `Map` constructor/type is not proof of the standard map constructor.",
    },
    "axis_map_fallback_identity": {
        "axis": "map_default_lookup",
        "why": "Typed map default lookups should prove the same map/key/fallback behavior across contains-get and defaulting API forms.",
    },
    "axis_map_fallback_ts_nullish_identity": {
        "axis": "map_default_lookup",
        "why": "A typed TypeScript `Map.get(key) ?? fallback` should prove the same map/key/fallback behavior as existing map-default forms.",
    },
    "axis_map_fallback_ts_has_get_identity": {
        "axis": "map_default_lookup",
        "why": "A typed TypeScript `Map.has(key) ? Map.get(key) : fallback` should prove the same map/key/fallback behavior as existing map-default forms.",
    },
    "axis_map_fallback_ts_temp_guard_identity": {
        "axis": "map_default_lookup",
        "why": "A typed TypeScript temp-bound `Map.get` undefined guard should prove the same map/key/fallback behavior as existing map-default forms.",
    },
    "axis_map_fallback_wrong_key_boundary": {
        "axis": "map_default_lookup",
        "why": "Map default lookups over different dynamic key parameters are different proof coordinates.",
    },
    "axis_map_fallback_wrong_default_boundary": {
        "axis": "map_default_lookup",
        "why": "Different fallback parameters change absent-key behavior and must not merge.",
    },
    "axis_map_fallback_wrong_map_boundary": {
        "axis": "map_default_lookup",
        "why": "Different map receivers change present-key behavior and must not merge.",
    },
    "axis_map_fallback_ts_wrong_key_boundary": {
        "axis": "map_default_lookup",
        "why": "TypeScript Map default lookups over different dynamic key parameters are different proof coordinates.",
    },
    "axis_map_fallback_ts_wrong_default_boundary": {
        "axis": "map_default_lookup",
        "why": "Different TypeScript Map fallback parameters change absent-key behavior and must not merge.",
    },
    "axis_map_fallback_ts_wrong_map_boundary": {
        "axis": "map_default_lookup",
        "why": "Different TypeScript Map receivers change present-key behavior and must not merge.",
    },
    "axis_map_fallback_ts_untyped_boundary": {
        "axis": "map_default_lookup",
        "why": "An untyped TypeScript `.get` receiver is not proof of strict Map default semantics.",
    },
    "axis_table_access": {
        "axis": "table_access",
        "why": "Literal table access must preserve key/index identity and reject neighboring table values.",
    },
    "axis_unsafe_boundary": {
        "axis": "unsafe_boundary",
        "why": "Unproven free globals and dynamic boundaries are not strict exact Type-4 evidence.",
    },
}


@dataclass(frozen=True)
class Surface:
    key: str
    language: str
    extension: str
    wrapper: str = "base"


@dataclass(frozen=True)
class GenerationFilter:
    axes: frozenset[str]
    proposal_prefixes: tuple[str, ...]

    @property
    def active(self) -> bool:
        return bool(self.axes or self.proposal_prefixes)

    def include_axis(self, axis: str) -> bool:
        return not self.axes or axis in self.axes

    def include_proposal(self, proposal_id: str) -> bool:
        return not self.proposal_prefixes or any(
            proposal_id.startswith(prefix) for prefix in self.proposal_prefixes
        )

    def include_base_proposal(self, proposal: dict) -> bool:
        if not self.include_proposal(proposal["proposal_id"]):
            return False
        if not self.axes:
            return True
        return "aggregate_reduction" in self.axes or proposal["operation"] in self.axes

    def include_axis_proposal(self, proposal_id: str, axis: str) -> bool:
        return self.include_axis(axis) and self.include_proposal(proposal_id)


@dataclass(frozen=True)
class Variant:
    representation: str
    source: str
    entrypoint: str
    start_line: int = 1


@dataclass(frozen=True)
class Operation:
    key: str
    kind: str
    positive_predicate: str
    negative_predicate: str | None = None
    negative_kind: str | None = None
    positive_contribution: str = "identity"
    negative_contribution: str | None = None
    positive_init: int = 0
    negative_init: int | None = None
    negative_reason: str = "predicate mutation"
    arity: int = 1


SURFACES = [
    Surface("python", "python", "py"),
    Surface("javascript", "javascript", "js"),
    Surface("typescript", "typescript", "ts"),
    Surface("go", "go", "go"),
    Surface("rust", "rust", "rs"),
    Surface("java", "java", "java"),
    Surface("c", "c", "c"),
    Surface("ruby", "ruby", "rb"),
    Surface("vue", "javascript", "vue", "script"),
    Surface("svelte", "javascript", "svelte", "script"),
    Surface("html", "javascript", "html", "script"),
]
JS_LIKE_SURFACES = {"javascript", "typescript", "vue", "svelte", "html"}

OPERATIONS = {
    "sum_positive": Operation(
        "sum_positive",
        "sum",
        "gt0",
        negative_init=1,
        negative_reason="initial accumulator 0 -> 1",
    ),
    "count_positive": Operation("count_positive", "count", "gt0", "ge0"),
    "any_positive": Operation("any_positive", "any", "gt0", "ge0"),
    "all_nonnegative": Operation("all_nonnegative", "all", "ge0", "gt0"),
    "product_positive": Operation(
        "product_positive",
        "product",
        "gt0",
        positive_init=1,
        negative_init=2,
        negative_reason="initial accumulator 1 -> 2",
    ),
    "sum_even": Operation("sum_even", "sum", "even", "odd"),
    "count_negative": Operation("count_negative", "count", "lt0", "le0"),
    "any_zero": Operation("any_zero", "any", "eq0", "ne0"),
    "all_nonzero": Operation("all_nonzero", "all", "ne0", "gt0"),
    "product_nonzero": Operation("product_nonzero", "product", "ne0", "gt0", positive_init=1),
    "sum_small": Operation("sum_small", "sum", "lt3", "le3"),
    "count_small": Operation("count_small", "count", "lt3", "le3"),
    "any_even": Operation("any_even", "any", "even", "odd"),
    "max_seed_zero": Operation(
        "max_seed_zero",
        "max",
        "true",
        negative_kind="min",
        positive_init=0,
        negative_reason="max selection -> min selection",
    ),
    "min_seed_zero": Operation(
        "min_seed_zero",
        "min",
        "true",
        negative_kind="max",
        positive_init=0,
        negative_reason="min selection -> max selection",
    ),
    "sum_positive_squares": Operation(
        "sum_positive_squares",
        "sum",
        "gt0",
        positive_contribution="square",
        negative_contribution="identity",
        negative_reason="per-element contribution x*x -> x",
    ),
    "product_nonzero_squares": Operation(
        "product_nonzero_squares",
        "product",
        "ne0",
        positive_contribution="square",
        negative_contribution="identity",
        positive_init=1,
        negative_reason="per-element contribution x*x -> x",
    ),
    "dot_product": Operation(
        "dot_product",
        "sum",
        "true",
        arity=2,
        positive_contribution="pair_product",
        negative_contribution="pair_sum",
        negative_reason="pair contribution x*y -> x+y",
    ),
    "sum_abs_all": Operation(
        "sum_abs_all",
        "sum_abs",
        "true",
        negative_kind="sum",
        negative_reason="absolute contribution abs(x) -> x",
    ),
}

REQUIRED_PROPOSAL_FIELDS = {
    "proposal_id",
    "operation",
    "why",
    "positive_spec",
    "negative_mutations",
    "transform_tags",
    "complexity_budget",
}
REQUIRED_BUDGET_FIELDS = {
    "max_lines",
    "max_branch_count",
    "max_primary_transforms",
    "max_secondary_transforms",
}


def snake_to_camel(name: str) -> str:
    parts = name.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


def snake_to_pascal(name: str) -> str:
    return "".join(p.title() for p in name.split("_"))


def js_script_wrap(surface: Surface, source: str) -> str:
    if surface.key == "vue":
        return f"<template><div></div></template>\n<script>\n{source}\n</script>\n"
    if surface.key == "svelte":
        return f"<script>\n{source}\n</script>\n<div></div>\n"
    if surface.key == "html":
        return f"<!doctype html>\n<script>\n{source}\n</script>\n"
    return source


def js_start_line(surface: Surface) -> int:
    if surface.key == "svelte":
        return 2
    if surface.key in {"vue", "html"}:
        return 3
    return 1


def load_capabilities(path: Path) -> dict:
    return json.loads(path.read_text())


def surface_capability(capabilities: dict, surface: Surface, axis: str) -> str:
    return capabilities["surfaces"].get(surface.key, {}).get(axis, "unsupported")


def capability_exact_supported(capabilities: dict, surface: Surface, axis: str) -> bool:
    return surface_capability(capabilities, surface, axis) in {"supported", "partial"}


def js_axis_source(surface: Surface, body: str, entrypoint: str = "axisCase") -> Variant:
    return Variant("axis", js_script_wrap(surface, body), entrypoint, js_start_line(surface))


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
    semantic_mutation = right and negative and not (wrong_threshold or wrong_receiver)

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
    if proposal_id.startswith("axis_membership_set_"):
        return surface.key in {"python", "javascript", "typescript", "go", "rust", "ruby"}
    if proposal_id.startswith("axis_membership_java_"):
        return surface.key == "java"
    if proposal_id.startswith("axis_membership_module_"):
        return surface.key in {"python", "ruby", "javascript", "typescript", "java"}
    if proposal_id.startswith("axis_membership_go_slices_"):
        return surface.key in {"python", "ruby", "go"}
    if proposal_id.startswith("axis_membership_rust_local_"):
        return surface.key in {"python", "ruby", "rust"}
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
    if proposal_id in {
        "axis_membership_rust_local_wrong_element_boundary",
        "axis_membership_rust_local_wrong_collection_boundary",
    }:
        form = "rust_local_array" if right else "membership"
    if proposal_id == "axis_membership_rust_local_mutated_boundary":
        form = "rust_local_mutated" if right else "membership"
    if proposal_id == "axis_membership_rust_local_custom_receiver_boundary":
        form = "rust_local_custom_receiver" if right else "membership"
    if right and negative and proposal_id in {
        "axis_membership_set_param_identity",
        "axis_membership_set_inline_identity",
        "axis_membership_set_local_identity",
        "axis_membership_java_list_of_identity",
        "axis_membership_java_set_of_identity",
        "axis_membership_java_arrays_aslist_identity",
        "axis_membership_module_js_set_identity",
        "axis_membership_module_ts_set_identity",
        "axis_membership_module_java_list_identity",
        "axis_membership_go_slices_package_identity",
        "axis_membership_go_slices_alias_package_identity",
        "axis_membership_go_slices_const_package_identity",
        "axis_membership_rust_local_array_identity",
        "axis_membership_rust_local_typed_array_identity",
        "axis_membership_rust_local_slice_ref_identity",
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
    if right and proposal_id.endswith("_shadowed_boundary"):
        form = f"{form}_shadowed"
    return element, items, form


def axis_membership_literal_variant(
    surface: Surface,
    proposal_id: str,
    negative: bool,
    right: bool,
) -> Variant:
    element, items, form = membership_axis_parts(proposal_id, negative, right)
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
        if form == "typed_membership":
            src = f"""def {name}(values: list[str], value: str, other: str) -> bool:
    return {element} in values
"""
            return Variant("axis", src, name)
        if form == "substring":
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
        if form == "rust_local_mutated":
            src = f"""pub fn {name}(value: &str, other: &str) -> bool {{
    let mut values = vec!["{left}", "{right_item}"];
    values.push("green");
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
        if form == "substring":
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
    if right and negative and proposal_id == "axis_map_key_membership_identity":
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
        expr = (
            f"{key} in {receiver}.values()"
            if form == "value"
            else (f"{receiver}.__contains__({key})" if right else f"{key} in {receiver}")
        )
        src = f"""def {name}(lookup, other_lookup, key, other):
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
    if proposal_id.startswith(("axis_map_default_js_map_", "axis_map_default_js_object_")):
        return surface.key in {"python", "ruby", "javascript", "typescript"}
    if proposal_id.startswith("axis_map_default_java_map_"):
        return surface.key == "java"
    if proposal_id.startswith("axis_map_default_rust_"):
        return surface.key in {"python", "ruby", "rust"}
    if proposal_id.startswith("axis_map_default_module_"):
        return surface.key in {"python", "ruby", "javascript", "typescript", "java"}
    return surface.key in {"python", "ruby"}


def map_default_lookup_axis_supported(surface: Surface, proposal_id: str) -> bool:
    if not proposal_id.startswith("axis_map_fallback_"):
        return False
    if proposal_id.startswith("axis_map_fallback_ts_"):
        return surface.key in {"go", "java", "rust", "typescript"}
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
    if proposal_id.startswith("axis_map_fallback_ts_wrong_"):
        form = "ts_nullish"
    if proposal_id == "axis_map_fallback_ts_untyped_boundary":
        form = "ts_untyped"
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
    if right and negative and proposal_id == "axis_map_fallback_identity":
        key = "other_key"
    if right and negative and proposal_id in {
        "axis_map_fallback_ts_nullish_identity",
        "axis_map_fallback_ts_has_get_identity",
        "axis_map_fallback_ts_temp_guard_identity",
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
        if right:
            expr = f"{receiver}.getOrDefault({key}, {default})"
        else:
            expr = f"{receiver}.containsKey({key}) ? {receiver}.get({key}) : {default}"
        src = f"""import java.util.Map;

class AxisCase {{
    static int {name}(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) {{
        return {expr};
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
        else:
            expr = f"{receiver}.get({key}) ?? {default}"
            body = f"return {expr};"
        src = f"""function {name}(lookup: {receiver_type}, other_lookup: {receiver_type}, key: string, other_key: string, fallback: number, other_default: number): number {{
  {body}
}}
"""
        return Variant("axis", src, name)

    raise ValueError(f"unsupported surface for dynamic map default axis: {surface.key}")


def map_default_axis_parts(
    proposal_id: str, negative: bool, right: bool
) -> tuple[str, tuple[tuple[str, int], tuple[str, int]], int, str]:
    key = "key"
    entries = (("red", 1), ("blue", 2))
    default = 0
    form = "literal_api"

    if right and proposal_id == "axis_map_default_wrong_key_boundary":
        key = "other"
    if right and proposal_id == "axis_map_default_wrong_default_boundary":
        default = 9
    if right and proposal_id == "axis_map_default_wrong_map_boundary":
        entries = (("red", 9), ("blue", 2))
    if right and negative and proposal_id == "axis_map_default_literal_identity":
        default = 9
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
        "axis_map_default_module_js_map_identity",
        "axis_map_default_module_ts_map_identity",
        "axis_map_default_module_java_map_identity",
    }:
        default = 9
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
    return {{"{k1}": {v1}, "{k2}": {v2}}}.get({key}, {default})
"""
        return Variant("axis", src, name)

    if surface.key == "ruby":
        src = f"""def {name}(key, other)
  {{"{k1}" => {v1}, "{k2}" => {v2}}}.fetch({key}, {default})
end
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
        if proposal_id == "axis_import_namespace_member_identity":
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


def render_predicate(predicate: str, var: str) -> str:
    if predicate == "gt0":
        return f"{var} > 0"
    if predicate == "ge0":
        return f"{var} >= 0"
    if predicate == "lt0":
        return f"{var} < 0"
    if predicate == "le0":
        return f"{var} <= 0"
    if predicate == "eq0":
        return f"{var} == 0"
    if predicate == "ne0":
        return f"{var} != 0"
    if predicate == "even":
        return f"{var} % 2 == 0"
    if predicate == "odd":
        return f"{var} % 2 != 0"
    if predicate == "lt3":
        return f"{var} < 3"
    if predicate == "le3":
        return f"{var} <= 3"
    if predicate == "true":
        return "1 == 1"
    raise ValueError(f"unknown predicate: {predicate}")


def render_contribution(contribution: str, var: str, other: str | None = None) -> str:
    if contribution == "identity":
        return var
    if contribution == "square":
        return f"{var} * {var}"
    if contribution == "pair_product" and other is not None:
        return f"{var} * {other}"
    if contribution == "pair_sum" and other is not None:
        return f"{var} + {other}"
    raise ValueError(f"unknown contribution: {contribution}")


def negate(expr: str, language: str) -> str:
    if language == "python":
        return f"not ({expr})"
    return f"!({expr})"


def eval_predicate(predicate: str, x: int) -> bool:
    if predicate == "gt0":
        return x > 0
    if predicate == "ge0":
        return x >= 0
    if predicate == "lt0":
        return x < 0
    if predicate == "le0":
        return x <= 0
    if predicate == "eq0":
        return x == 0
    if predicate == "ne0":
        return x != 0
    if predicate == "even":
        return x % 2 == 0
    if predicate == "odd":
        return x % 2 != 0
    if predicate == "lt3":
        return x < 3
    if predicate == "le3":
        return x <= 3
    if predicate == "true":
        return True
    raise ValueError(f"unknown predicate: {predicate}")


def eval_contribution(contribution: str, x: int, y: int | None = None) -> int:
    if contribution == "identity":
        return x
    if contribution == "square":
        return x * x
    if contribution == "pair_product" and y is not None:
        return x * y
    if contribution == "pair_sum" and y is not None:
        return x + y
    raise ValueError(f"unknown contribution: {contribution}")


def effective_operation(operation: Operation, negative: bool) -> tuple[str, int]:
    predicate = operation.positive_predicate
    init = operation.positive_init
    if negative:
        if operation.negative_predicate is not None:
            predicate = operation.negative_predicate
        if operation.negative_init is not None:
            init = operation.negative_init
    return predicate, init


def effective_components(operation: Operation, negative: bool) -> tuple[str, str, int, str]:
    kind = operation.kind
    predicate, init = effective_operation(operation, negative)
    contribution = operation.positive_contribution
    if negative and operation.negative_kind is not None:
        kind = operation.negative_kind
    if negative and operation.negative_contribution is not None:
        contribution = operation.negative_contribution
    return kind, predicate, init, contribution


def property_inputs(operation_key: str) -> list[dict]:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return PAIR_PROPERTY_INPUTS
    return [{"xs": xs} for xs in PROPERTY_INPUTS]


def spec_output(
    operation_key: str,
    inputs: dict | list[int],
    negative: bool = False,
    start: int = 0,
    step: int = 1,
) -> int | bool:
    op = OPERATIONS[operation_key]
    kind, predicate, init, contribution = effective_components(op, negative)
    if isinstance(inputs, list):
        inputs = {"xs": inputs}
    if op.arity == 2:
        pairs = list(zip(inputs["a"], inputs["b"]))[start::step]
        selected_values = [
            eval_contribution(contribution, x, y)
            for x, y in pairs
            if eval_predicate(predicate, x)
        ]
        if kind == "sum":
            return init + sum(selected_values)
        raise ValueError(f"unsupported arity-2 operation kind: {kind}")
    view = inputs["xs"][start::step]
    selected = [x for x in view if eval_predicate(predicate, x)]
    selected_values = [eval_contribution(contribution, x) for x in selected]
    if kind == "sum":
        return init + sum(selected_values)
    if kind == "sum_abs":
        return init + sum(abs(x) for x in selected)
    if kind == "count":
        return len(selected)
    if kind == "any":
        return any(eval_predicate(predicate, x) for x in view)
    if kind == "all":
        return all(eval_predicate(predicate, x) for x in view)
    if kind == "product":
        return math.prod(selected_values, start=init)
    if kind == "max":
        return max([init, *selected])
    if kind == "min":
        return min([init, *selected])
    raise ValueError(f"unknown operation kind: {kind}")


def counterexample(operation_key: str) -> dict:
    for inputs in property_inputs(operation_key):
        left = spec_output(operation_key, inputs, negative=False)
        right = spec_output(operation_key, inputs, negative=True)
        if left != right:
            return {"input": inputs, "left_output": left, "right_output": right}
    raise ValueError(f"no counterexample in property inputs for {operation_key}")


def evidence_positive(operation_key: str) -> dict:
    inputs = property_inputs(operation_key)
    return {
        "level": "E1",
        "kind": "same-spec-template+spec-interpreter",
        "property_inputs": inputs,
        "outputs": [spec_output(operation_key, inp, negative=False) for inp in inputs],
    }


def evidence_negative(operation_key: str) -> dict:
    return {
        "level": "E2",
        "kind": "counterexample",
        "counterexample": counterexample(operation_key),
    }


def contract_negative_window(representation: str) -> tuple[int, int]:
    if representation == "c_start_one":
        return 1, 1
    if representation == "c_stride_two":
        return 0, 2
    raise ValueError(f"unknown C contract negative representation: {representation}")


def contract_counterexample(operation_key: str, representation: str) -> dict:
    start, step = contract_negative_window(representation)
    for inputs in property_inputs(operation_key):
        left = spec_output(operation_key, inputs)
        right = spec_output(operation_key, inputs, start=start, step=step)
        if left != right:
            return {"input": inputs, "left_output": left, "right_output": right}
    raise ValueError(f"no C contract counterexample for {operation_key} {representation}")


def evidence_contract_negative(operation_key: str, representation: str) -> dict:
    return {
        "level": "E2",
        "kind": "counterexample",
        "counterexample": contract_counterexample(operation_key, representation),
    }


def python_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return python_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for x in xs:" if representation == "loop" else "for i in range(len(xs)):"
            loop_bind = "" if representation == "loop" else "\n        x = xs[i]"
            src = f"""def {operation_key}(xs):
    total = {init}
    {loop_head}{loop_bind}
        if x < 0:
            total += -x
        else:
            total += x
    return total
"""
        else:
            prefix = "" if init == 0 else f"{init} + "
            src = f"""def {operation_key}(xs):
    return {prefix}sum(abs(x) for x in xs)
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for x in xs:" if representation == "loop" else "for i in range(len(xs)):"
        loop_bind = "" if representation == "loop" else "\n        x = xs[i]"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {
                "sum": f"total += {term}",
                "count": "count += 1",
                "product": f"product *= {term}",
            }[kind]
            src = f"""def {operation_key}(xs):
    {var} = {init}
    {loop_head}{loop_bind}
        if {pred}:
            {update}
    return {var}
"""
        elif kind == "any":
            src = f"""def {operation_key}(xs):
    {loop_head}{loop_bind}
        if {pred}:
            return True
    return False
"""
        elif kind == "all":
            src = f"""def {operation_key}(xs):
    {loop_head}{loop_bind}
        if {negate(pred, "python")}:
            return False
    return True
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""def {operation_key}(xs):
    best = {init}
    {loop_head}{loop_bind}
        if x {cmp} best:
            best = x
    return best
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        prefix = "" if init == 0 else f"{init} + "
        src = f"""def {operation_key}(xs):
    return {prefix}sum({term} for x in xs if {pred})
"""
    elif kind == "count":
        src = f"""def {operation_key}(xs):
    return sum(1 for x in xs if {pred})
"""
    elif kind == "any":
        src = f"""def {operation_key}(xs):
    return any({pred} for x in xs)
"""
    elif kind == "all":
        src = f"""def {operation_key}(xs):
    return all({pred} for x in xs)
"""
    elif kind == "product":
        src = f"""import math

def {operation_key}(xs):
    return math.prod(({term} for x in xs if {pred}), start={init})
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""def {operation_key}(xs):
    return reduce(lambda best, x: x if x {cmp} best else best, xs, {init})
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def js_variant(surface: Surface, operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return js_pair_variant(surface, operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for (const x of xs)" if representation == "loop" else "for (let i = 0; i < xs.length; i += 1)"
            loop_bind = "" if representation == "loop" else "\n    const x = xs[i];"
            body = f"""function {name}(xs) {{
  let total = {init};
  {loop_head} {{{loop_bind}
    if (x < 0) {{
      total += -x;
    }} else {{
      total += x;
    }}
  }}
  return total;
}}
"""
        else:
            body = f"""function {name}(xs) {{
  return xs.reduce((total, x) => total + (x < 0 ? -x : x), {init});
}}
"""
        return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for (const x of xs)" if representation == "loop" else "for (let i = 0; i < xs.length; i += 1)"
        loop_bind = "" if representation == "loop" else "\n    const x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            body = f"""function {name}(xs) {{
  let {var} = {init};
  {loop_head} {{{loop_bind}
    if ({pred}) {{
      {update}
    }}
  }}
  return {var};
}}
"""
        elif kind == "any":
            body = f"""function {name}(xs) {{
  {loop_head} {{{loop_bind}
    if ({pred}) {{
      return true;
    }}
  }}
  return false;
}}
"""
        elif kind == "all":
            body = f"""function {name}(xs) {{
  {loop_head} {{{loop_bind}
    if ({negate(pred, "javascript")}) {{
      return false;
    }}
  }}
  return true;
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            body = f"""function {name}(xs) {{
  let best = {init};
  {loop_head} {{{loop_bind}
    if (x {cmp} best) {{
      best = x;
    }}
  }}
  return best;
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).reduce((total, x) => total + {term}, {init});
}}
"""
    elif kind == "count":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).length;
}}
"""
    elif kind == "any":
        body = f"""function {name}(xs) {{
  return xs.some((x) => {pred});
}}
"""
    elif kind == "all":
        body = f"""function {name}(xs) {{
  return xs.every((x) => {pred});
}}
"""
    elif kind == "product":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).reduce((product, x) => product * {term}, {init});
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""function {name}(xs) {{
  return xs.reduce((best, x) => x {cmp} best ? x : best, {init});
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))


def typescript_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return typescript_pair_variant(operation_key, representation, negative)
    v = js_variant(Surface("typescript", "typescript", "ts"), operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    kind, _, _, _ = effective_components(op, negative)
    ret = "boolean" if kind in {"any", "all"} else "number"
    src = v.source.replace(f"function {name}(xs)", f"function {name}(xs: number[]): {ret}")
    return Variant(representation, src, name)


def go_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return go_pair_variant(operation_key, representation, negative)
    name = snake_to_pascal(operation_key)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if representation == "loop":
        iter_head = "for _, x := range xs"
        index_line = ""
    elif representation in {"aggregate", "indexed_loop"}:
        iter_head = "for i := 0; i < len(xs); i++"
        index_line = "\n\t\tx := xs[i]"
    else:
        raise ValueError(f"unknown Go representation: {representation}")
    if kind == "sum_abs":
        body = f"""package main

func {name}(xs []int) int {{
	total := {init}
	{iter_head} {{{index_line}
		if x < 0 {{
			total += -x
		}} else {{
			total += x
		}}
	}}
	return total
}}
"""
        return Variant(representation, body, name)
    if kind in {"sum", "count", "product"}:
        var = {"sum": "total", "count": "count", "product": "product"}[kind]
        update = {"sum": f"total += {term}", "count": "count += 1", "product": f"product *= {term}"}[kind]
        body = f"""package main

func {name}(xs []int) int {{
	{var} := {init}
	{iter_head} {{{index_line}
		if {pred} {{
			{update}
		}}
	}}
	return {var}
}}
"""
    elif kind == "any":
        body = f"""package main

func {name}(xs []int) bool {{
	{iter_head} {{{index_line}
		if {pred} {{
			return true
		}}
	}}
	return false
}}
"""
    elif kind == "all":
        body = f"""package main

func {name}(xs []int) bool {{
	{iter_head} {{{index_line}
		if {negate(pred, "go")} {{
			return false
		}}
	}}
	return true
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""package main

func {name}(xs []int) int {{
	best := {init}
	{iter_head} {{{index_line}
		if x {cmp} best {{
			best = x
		}}
	}}
	return best
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, name)


def rust_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return rust_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred_val = render_predicate(predicate_key, "x")
    pred_ref = render_predicate(predicate_key, "*x")
    term = render_contribution(contribution, "x")
    ret = "bool" if kind in {"any", "all"} else "i32"
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for &x in xs" if representation == "loop" else "for i in 0..xs.len()"
            loop_bind = "" if representation == "loop" else "\n        let x = xs[i];"
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut total = {init};
    {loop_head} {{{loop_bind}
        if x < 0 {{
            total += -x;
        }} else {{
            total += x;
        }}
    }}
    total
}}
"""
        else:
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().fold({init}, |total, x| total + if x < 0 {{ -x }} else {{ x }})
}}
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for &x in xs" if representation == "loop" else "for i in 0..xs.len()"
        loop_bind = "" if representation == "loop" else "\n        let x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut {var} = {init};
    {loop_head} {{{loop_bind}
        if {pred_val} {{
            {update}
        }}
    }}
    {var}
}}
"""
        elif kind == "any":
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    {loop_head} {{{loop_bind}
        if {pred_val} {{
            return true;
        }}
    }}
    false
}}
"""
        elif kind == "all":
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    {loop_head} {{{loop_bind}
        if {negate(pred_val, "rust")} {{
            return false;
        }}
    }}
    true
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut best = {init};
    {loop_head} {{{loop_bind}
        if x {cmp} best {{
            best = x;
        }}
    }}
    best
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).fold({init}, |total, x| total + {term})
}}
"""
    elif kind == "count":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).count() as i32
}}
"""
    elif kind == "any":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().any(|x| {pred_val})
}}
"""
    elif kind == "all":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().all(|x| {pred_val})
}}
"""
    elif kind == "product":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).fold({init}, |product, x| product * {term})
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().fold({init}, |best, x| if x {cmp} best {{ x }} else {{ best }})
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def java_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return java_pair_variant(operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    class_name = f"Case{snake_to_pascal(operation_key)}{representation.title()}"
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    ret = "boolean" if kind in {"any", "all"} else "int"
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for (int x : xs)" if representation == "loop" else "for (int i = 0; i < xs.length; i++)"
            loop_bind = "" if representation == "loop" else "\n      int x = xs[i];"
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int total = {init};
    {loop_head} {{{loop_bind}
      if (x < 0) {{
        total += -x;
      }} else {{
        total += x;
      }}
    }}
    return total;
  }}
}}
"""
        else:
            body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).reduce({init}, (total, x) -> total + (x < 0 ? -x : x));
  }}
}}
"""
        return Variant(representation, body, name)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for (int x : xs)" if representation == "loop" else "for (int i = 0; i < xs.length; i++)"
        loop_bind = "" if representation == "loop" else "\n      int x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int {var} = {init};
    {loop_head} {{{loop_bind}
      if ({pred}) {{
        {update}
      }}
    }}
    return {var};
  }}
}}
"""
        elif kind == "any":
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    {loop_head} {{{loop_bind}
      if ({pred}) {{
        return true;
      }}
    }}
    return false;
  }}
}}
"""
        elif kind == "all":
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    {loop_head} {{{loop_bind}
      if ({negate(pred, "java")}) {{
        return false;
      }}
    }}
    return true;
  }}
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int best = {init};
    {loop_head} {{{loop_bind}
      if (x {cmp} best) {{
        best = x;
      }}
    }}
    return best;
  }}
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).filter(x -> {pred}).reduce({init}, (total, x) -> total + {term});
  }}
}}
"""
    elif kind == "count":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return (int) Arrays.stream(xs).filter(x -> {pred}).count();
  }}
}}
"""
    elif kind == "any":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).anyMatch(x -> {pred});
  }}
}}
"""
    elif kind == "all":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).allMatch(x -> {pred});
  }}
}}
"""
    elif kind == "product":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).filter(x -> {pred}).reduce({init}, (product, x) -> product * {term});
  }}
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).reduce({init}, (best, x) -> x {cmp} best ? x : best);
  }}
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, name)


def c_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return c_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "xs[i]")
    term = render_contribution(contribution, "xs[i]")
    if representation == "loop":
        iter_head = "for (int i = 0; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_start_one":
        iter_head = "for (int i = 1; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_stride_two":
        iter_head = "for (int i = 0; i < n; i += 2)"
        inc = ""
        prefix = ""
    elif representation in {"aggregate", "indexed_loop"}:
        iter_head = "while (i < n)"
        inc = "\n    i++;"
        prefix = "\n  int i = 0;"
    else:
        raise ValueError(f"unknown C representation: {representation}")
    if kind == "sum_abs":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int total = {init};
  {iter_head} {{
    if (xs[i] < 0) {{
      total += -xs[i];
    }} else {{
      total += xs[i];
    }}{inc}
  }}
  return total;
}}
"""
        return Variant(representation, body, operation_key)
    if kind in {"sum", "count", "product"}:
        var = {"sum": "total", "count": "count", "product": "product"}[kind]
        update = {
            "sum": f"total += {term};",
            "count": "count += 1;",
            "product": f"product *= {term};",
        }[kind]
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int {var} = {init};
  {iter_head} {{
    if ({pred}) {{
      {update}
    }}{inc}
  }}
  return {var};
}}
"""
    elif kind == "any":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  {iter_head} {{
    if ({pred}) {{
      return 1;
    }}{inc}
  }}
  return 0;
}}
"""
    elif kind == "all":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  {iter_head} {{
    if ({negate(pred, "c")}) {{
      return 0;
    }}{inc}
  }}
  return 1;
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int best = {init};
  {iter_head} {{
    if (xs[i] {cmp} best) {{
      best = xs[i];
    }}{inc}
  }}
  return best;
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, operation_key)


def ruby_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return ruby_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "xs.each do |x|" if representation == "loop" else "while i < xs.length"
            loop_prefix = "" if representation == "loop" else "  i = 0\n"
            loop_bind = "" if representation == "loop" else "\n    x = xs[i]"
            loop_inc = "" if representation == "loop" else "\n    i += 1"
            loop_end = "end"
            src = f"""def {operation_key}(xs)
  total = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if x < 0
      total += -x
    else
      total += x
    end{loop_inc}
  {loop_end}
  total
end
"""
        else:
            src = f"""def {operation_key}(xs)
  xs.reduce({init}) {{ |total, x| total + (x < 0 ? -x : x) }}
end
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "xs.each do |x|" if representation == "loop" else "while i < xs.length"
        loop_prefix = "" if representation == "loop" else "  i = 0\n"
        loop_bind = "" if representation == "loop" else "\n    x = xs[i]"
        loop_inc = "" if representation == "loop" else "\n    i += 1"
        loop_end = "end"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term}", "count": "count += 1", "product": f"product *= {term}"}[
                kind
            ]
            src = f"""def {operation_key}(xs)
  {var} = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if {pred}
      {update}
    end{loop_inc}
  {loop_end}
  {var}
end
"""
        elif kind == "any":
            src = f"""def {operation_key}(xs)
{loop_prefix}  {loop_head}{loop_bind}
    if {pred}
      return true
    end{loop_inc}
  {loop_end}
  false
end
"""
        elif kind == "all":
            src = f"""def {operation_key}(xs)
{loop_prefix}  {loop_head}{loop_bind}
    if {negate(pred, "ruby")}
      return false
    end{loop_inc}
  {loop_end}
  true
end
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""def {operation_key}(xs)
  best = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if x {cmp} best
      best = x
    end{loop_inc}
  {loop_end}
  best
end
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        src = f"""def {operation_key}(xs)
  xs.select {{ |x| {pred} }}.reduce({init}) {{ |total, x| total + {term} }}
end
"""
    elif kind == "count":
        src = f"""def {operation_key}(xs)
  xs.count {{ |x| {pred} }}
end
"""
    elif kind == "any":
        src = f"""def {operation_key}(xs)
  xs.any? {{ |x| {pred} }}
end
"""
    elif kind == "all":
        src = f"""def {operation_key}(xs)
  xs.all? {{ |x| {pred} }}
end
"""
    elif kind == "product":
        src = f"""def {operation_key}(xs)
  xs.select {{ |x| {pred} }}.reduce({init}) {{ |product, x| product * {term} }}
end
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""def {operation_key}(xs)
  xs.reduce({init}) {{ |best, x| x {cmp} best ? x : best }}
end
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def python_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    indexed_term = render_contribution(contribution, "a[i]", "b[i]")
    pair_term = render_contribution(contribution, "x", "y")
    if representation == "loop":
        src = f"""def {operation_key}(a, b):
    total = {init}
    for i in range(len(a)):
        total += {indexed_term}
    return total
"""
    else:
        prefix = "" if init == 0 else f"{init} + "
        src = f"""def {operation_key}(a, b):
    return {prefix}sum({pair_term} for x, y in zip(a, b))
"""
    return Variant(representation, src, operation_key)


def js_pair_variant(surface: Surface, operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_camel(operation_key)
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    indexed_term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        body = f"""function {name}(a, b) {{
  let total = {init};
  for (let i = 0; i < a.length; i += 1) {{
    total += {indexed_term};
  }}
  return total;
}}
"""
    else:
        body = f"""function {name}(a, b) {{
  let total = {init};
  let i = 0;
  while (i < a.length) {{
    total += {indexed_term};
    i += 1;
  }}
  return total;
}}
"""
    return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))


def typescript_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    v = js_pair_variant(Surface("typescript", "typescript", "ts"), operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    src = v.source.replace(f"function {name}(a, b)", f"function {name}(a: number[], b: number[]): number")
    return Variant(representation, src, name)


def go_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_pascal(operation_key)
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "x", "b[i]")
        body = f"""package main

func {name}(a []int, b []int) int {{
	total := {init}
	for i, x := range a {{
		total += {term}
	}}
	return total
}}
"""
    else:
        term = render_contribution(contribution, "a[i]", "b[i]")
        body = f"""package main

func {name}(a []int, b []int) int {{
	total := {init}
	for i := 0; i < len(a); i++ {{
		total += {term}
	}}
	return total
}}
"""
    return Variant(representation, body, name)


def rust_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "a[i]", "b[i]")
        src = f"""pub fn {operation_key}(a: &[i32], b: &[i32]) -> i32 {{
    let mut total = {init};
    for i in 0..a.len() {{
        total += {term};
    }}
    total
}}
"""
    else:
        term = render_contribution(contribution, "*x", "*y")
        src = f"""pub fn {operation_key}(a: &[i32], b: &[i32]) -> i32 {{
    a.iter().zip(b.iter()).fold({init}, |total, (x, y)| total + {term})
}}
"""
    return Variant(representation, src, operation_key)


def java_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_camel(operation_key)
    class_name = f"Case{snake_to_pascal(operation_key)}{representation.title()}"
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        body = f"""class {class_name} {{
  static int {name}(int[] a, int[] b) {{
    int total = {init};
    for (int i = 0; i < a.length; i++) {{
      total += {term};
    }}
    return total;
  }}
}}
"""
    else:
        body = f"""class {class_name} {{
  static int {name}(int[] a, int[] b) {{
    int total = {init};
    int i = 0;
    while (i < a.length) {{
      total += {term};
      i++;
    }}
    return total;
  }}
}}
"""
    return Variant(representation, body, name)


def c_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        iter_head = "for (int i = 0; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_start_one":
        iter_head = "for (int i = 1; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_stride_two":
        iter_head = "for (int i = 0; i < n; i += 2)"
        inc = ""
        prefix = ""
    elif representation == "aggregate":
        iter_head = "while (i < n)"
        inc = "\n    i++;"
        prefix = "\n  int i = 0;"
    else:
        raise ValueError(f"unknown C representation: {representation}")
    src = f"""int {operation_key}(int *a, int *b, int n) {{{prefix}
  int total = {init};
  {iter_head} {{
    total += {term};{inc}
  }}
  return total;
}}
"""
    return Variant(representation, src, operation_key)


def ruby_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "x", "b[i]")
        src = f"""def {operation_key}(a, b)
  total = {init}
  a.each_with_index do |x, i|
    total += {term}
  end
  total
end
"""
    else:
        term = render_contribution(contribution, "a[i]", "b[i]")
        src = f"""def {operation_key}(a, b)
  total = {init}
  i = 0
  while i < a.length
    total += {term}
    i += 1
  end
  total
end
"""
    return Variant(representation, src, operation_key)


EMITTERS = {
    "python": python_variant,
    "javascript": lambda op, rep, neg: js_variant(Surface("javascript", "javascript", "js"), op, rep, neg),
    "typescript": typescript_variant,
    "go": go_variant,
    "rust": rust_variant,
    "java": java_variant,
    "c": c_variant,
    "ruby": ruby_variant,
    "vue": lambda op, rep, neg: js_variant(Surface("vue", "javascript", "vue", "script"), op, rep, neg),
    "svelte": lambda op, rep, neg: js_variant(Surface("svelte", "javascript", "svelte", "script"), op, rep, neg),
    "html": lambda op, rep, neg: js_variant(Surface("html", "javascript", "html", "script"), op, rep, neg),
}


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
        "nullish_default": "nullable<int>+fallback",
        "numeric_minmax_abs": "scalar<int>+alternate",
        "projection_identity": "record<today:int,tomorrow:int>",
        "string_prefix_suffix": "string",
        "table_access": "map<string,int>",
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
        if axis == "string_prefix_suffix":
            return {
                "level": "E1",
                "kind": f"same-spec-{axis}",
                "property_inputs": ["prelude", "case-suf", "other"],
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
        }:
            counterexample = {
                "input": {"key": "green", "other": "red"},
                "left": 0,
                "right": 9,
            }
        elif proposal_id in {
            "axis_map_default_wrong_map_boundary",
            "axis_map_default_js_map_wrong_map_boundary",
            "axis_map_default_js_object_wrong_map_boundary",
            "axis_map_default_java_map_wrong_map_boundary",
            "axis_map_default_rust_wrong_map_boundary",
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
        if proposal_id == "axis_map_fallback_wrong_default_boundary":
            input_values["key"] = "green"
            input_values["other_key"] = "red"
            counterexample = {
                "input": input_values,
                "left": 0,
                "right": 9,
            }
        elif proposal_id == "axis_map_fallback_wrong_map_boundary":
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
            if proposal_id.startswith("axis_membership_set_"):
                continue
            if proposal_id.startswith("axis_membership_java_"):
                continue
            if proposal_id.startswith("axis_membership_module_"):
                continue
            if proposal_id.startswith("axis_membership_go_slices_"):
                continue
            if proposal_id.startswith("axis_membership_rust_local_"):
                continue
            if proposal_id.startswith("axis_map_key_") and not map_key_membership_axis_supported(
                surface, proposal_id
            ):
                continue
            if proposal_id.startswith(
                (
                    "axis_map_default_js_map_",
                    "axis_map_default_js_object_",
                    "axis_map_default_java_map_",
                    "axis_map_default_rust_",
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
            if proposal_id.startswith("axis_map_fallback_ts_"):
                continue
            if proposal_id in {
                "axis_collection_threshold_boundary",
                "axis_collection_wrong_receiver_boundary",
            }:
                items.append(
                    make_axis_item(
                        out_dir,
                        capabilities,
                        proposal_id,
                        surface,
                        "not_equivalent",
                        "heldout",
                        "collection-empty-boundary",
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
            if proposal_id.startswith("axis_projection_") and not projection_axis_supported(
                surface, proposal_id
            ):
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
    module_reference_surfaces = [surface_by_key["python"], surface_by_key["ruby"]]
    if cross_mode == "ring":
        module_reference_surfaces = [surface_by_key["python"]]
    elif cross_mode == "none":
        module_reference_surfaces = []
    module_right_surfaces_by_proposal = {
        "axis_membership_module_js_set_identity": [surface_by_key["javascript"]],
        "axis_membership_module_ts_set_identity": [surface_by_key["typescript"]],
        "axis_membership_module_java_list_identity": [surface_by_key["java"]],
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
