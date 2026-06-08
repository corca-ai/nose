"""Axis proposal metadata for the Type-4 benchmark generator."""

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
    "axis_import_namespace_shadowed_param_identity": {
        "axis": "import_identity",
        "why": "A JS/TS namespace import remains a proven module binding when a different function parameter shadows the same local name.",
    },
    "axis_import_namespace_shadowed_param_template_identity": {
        "axis": "import_identity",
        "why": "A JS/TS namespace import call through a template literal should preserve static template fragments and match string concatenation.",
    },
    "axis_import_namespace_shadowed_param_unshadowed_mutation_boundary": {
        "axis": "import_identity",
        "why": "A mutation-like receiver call on the unshadowed namespace local must still block the module binding proof.",
    },
    "axis_import_namespace_shadowed_param_fake_receiver_boundary": {
        "axis": "import_identity",
        "why": "A same-named object receiver is not a proven namespace import coordinate.",
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
    "axis_numeric_clamp_guarded_minmax_identity": {
        "axis": "numeric_clamp",
        "why": "Guarded integer min(max(x, lo), hi) and max(min(x, hi), lo) should converge only when the source proves lo <= hi.",
    },
    "axis_numeric_clamp_unproven_boundary": {
        "axis": "numeric_clamp",
        "why": "Clamp min/max compositions over parameter bounds must not merge without a bound-order proof.",
    },
    "axis_numeric_clamp_swapped_bounds_boundary": {
        "axis": "numeric_clamp",
        "why": "Swapping lower and upper bounds changes clamp behavior even when the valid-order guard exists.",
    },
    "axis_numeric_clamp_float_boundary": {
        "axis": "numeric_clamp",
        "why": "Float/NaN-sensitive clamp surfaces need a separate domain proof and must not use the integer clamp canon.",
    },
    "axis_hof_filter_map_identity": {
        "axis": "hof_filter_map",
        "why": "Option-producing filter_map callbacks should prove the same filtered map as explicit filter+map and guarded builders.",
    },
    "axis_hof_filter_map_none_boundary": {
        "axis": "hof_filter_map",
        "why": "Dropping None is not the same as mapping None as an emitted value.",
    },
    "axis_hof_filter_map_value_boundary": {
        "axis": "hof_filter_map",
        "why": "The filter_map proof fixes the emitted Some value coordinate.",
    },
    "axis_hof_filter_map_falsey_boundary": {
        "axis": "hof_filter_map",
        "why": "Falsey Some-like values such as 0 are emitted; only None/Null absence drops an item.",
    },
    "axis_total_order_compare_guard_order_identity": {
        "axis": "total_order_compare",
        "why": "Two strict total-order guard returns commute when each branch exits and the fallback is equality.",
    },
    "axis_total_order_compare_ternary_identity": {
        "axis": "total_order_compare",
        "why": "A strict three-way comparator written as guard returns should prove the same sign result as the nested ternary form.",
    },
    "axis_total_order_compare_descending_boundary": {
        "axis": "total_order_compare",
        "why": "Ascending and descending comparators reverse negative/positive results and must not merge.",
    },
    "axis_total_order_compare_equal_boundary": {
        "axis": "total_order_compare",
        "why": "A comparator that treats equality as less changes the equality result and must not merge.",
    },
    "axis_total_order_compare_wrong_value_boundary": {
        "axis": "total_order_compare",
        "why": "A three-way comparator proof fixes the returned sign values -1, 0, and 1.",
    },
    "axis_java_dead_loop_guard_identity": {
        "axis": "java_statically_false_loop",
        "why": "A Java loop whose entry guard starts with a proven false short-circuit operand has an unreachable body.",
    },
    "axis_java_dead_loop_false_init_boundary": {
        "axis": "java_statically_false_loop",
        "why": "A Java loop guarded by `!found && ...` can execute when `found` is initialized false.",
    },
    "axis_java_dead_loop_positive_guard_boundary": {
        "axis": "java_statically_false_loop",
        "why": "A Java loop guarded by `found && ...` can execute when `found` is initialized true.",
    },
    "axis_java_dead_loop_reassigned_guard_boundary": {
        "axis": "java_statically_false_loop",
        "why": "A reassigned guard variable is not a proof that the loop entry guard is false.",
    },
    "axis_java_low_bit_toggle_even_identity": {
        "axis": "java_integer_low_bit_toggle",
        "why": "For Java primitive integers, the even/odd +/-1 reverse-edge idiom toggles the low bit exactly like `x ^ 1`.",
    },
    "axis_java_low_bit_toggle_odd_identity": {
        "axis": "java_integer_low_bit_toggle",
        "why": "The `% 2 != 0` branch order is the same Java primitive-integer low-bit toggle proof.",
    },
    "axis_java_low_bit_toggle_reversed_branch_boundary": {
        "axis": "java_integer_low_bit_toggle",
        "why": "Reversing the +/-1 branches changes the low-bit toggle direction and must not merge.",
    },
    "axis_java_low_bit_toggle_xor_two_boundary": {
        "axis": "java_integer_low_bit_toggle",
        "why": "Toggling bit 1 with `x ^ 2` is not the same as toggling the low bit.",
    },
    "axis_java_low_bit_toggle_positive_one_boundary": {
        "axis": "java_integer_low_bit_toggle",
        "why": "In Java, `x % 2 == 1` is not an oddness proof for negative odd integers.",
    },
    "axis_java_low_bit_toggle_wrong_delta_boundary": {
        "axis": "java_integer_low_bit_toggle",
        "why": "The low-bit toggle proof fixes both branch deltas to exactly +1 and -1.",
    },
    "axis_c_u16_be_byte_pack_unsigned_char_identity": {
        "axis": "c_u16_be_byte_pack",
        "why": "A proven C byte-buffer `u16` big-endian decode should treat disjoint byte-lane addition and bitwise-or as the same value.",
    },
    "axis_c_u16_be_byte_pack_uint8_identity": {
        "axis": "c_u16_be_byte_pack",
        "why": "A `uint8_t *` byte-buffer proof should support the same `u16` big-endian lane packing identity.",
    },
    "axis_c_u16_be_byte_pack_uncasted_add_identity": {
        "axis": "c_u16_be_byte_pack",
        "why": "A same-file `typedef unsigned char u8` proof should support uncasted C byte-lane addition in 16-bit big-endian decoders.",
    },
    "axis_c_u16_be_byte_pack_wrong_order_boundary": {
        "axis": "c_u16_be_byte_pack",
        "why": "Swapping the two decoded bytes changes the big-endian value and must not merge.",
    },
    "axis_c_u16_be_byte_pack_overlap_boundary": {
        "axis": "c_u16_be_byte_pack",
        "why": "Addition and bitwise-or are not equivalent when shifted lanes overlap.",
    },
    "axis_c_u16_be_byte_pack_wrong_byte_boundary": {
        "axis": "c_u16_be_byte_pack",
        "why": "The byte-pack proof fixes the low byte coordinate to index 1.",
    },
    "axis_c_u16_be_byte_pack_unproven_alias_boundary": {
        "axis": "c_u16_be_byte_pack",
        "why": "A C `u8` spelling is not a byte-buffer proof unless the same file proves it aliases unsigned char.",
    },
    "axis_c_u32_be_byte_pack_unsigned_alias_identity": {
        "axis": "c_u32_be_byte_pack",
        "why": "A proven C byte-buffer plus unsigned 32-bit cast can safely canonicalize 4-lane big-endian addition and bitwise-or forms.",
    },
    "axis_c_u32_be_byte_pack_unsigned_int_identity": {
        "axis": "c_u32_be_byte_pack",
        "why": "A direct `unsigned int` cast on byte lanes is an explicit proof for the high 32-bit big-endian lane shift.",
    },
    "axis_c_u32_be_byte_pack_uint8_identity": {
        "axis": "c_u32_be_byte_pack",
        "why": "A `uint8_t *` byte-buffer proof should support unsigned-cast 32-bit big-endian lane packing.",
    },
    "axis_c_u32_be_byte_pack_uncasted_high_boundary": {
        "axis": "c_u32_be_byte_pack",
        "why": "An uncasted C high byte lane shifted by 24 is not accepted because signed left shift can overflow or be undefined.",
    },
    "axis_c_u32_be_byte_pack_wrong_order_boundary": {
        "axis": "c_u32_be_byte_pack",
        "why": "Swapping decoded bytes changes the 32-bit big-endian value and must not merge.",
    },
    "axis_c_u32_be_byte_pack_wrong_byte_boundary": {
        "axis": "c_u32_be_byte_pack",
        "why": "The 32-bit byte-pack proof fixes all four byte coordinates.",
    },
    "axis_c_u32_be_byte_pack_wrong_alias_boundary": {
        "axis": "c_u32_be_byte_pack",
        "why": "A `u32` spelling is not an unsigned-cast proof unless the same file or direct include proves it aliases unsigned int.",
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
    "axis_python_docstring_guard_identity": {
        "axis": "python_docstring_noop",
        "why": "A leading static Python function docstring is metadata and must not affect equivalent guard-return behavior.",
    },
    "axis_python_docstring_return_identity": {
        "axis": "python_docstring_noop",
        "why": "A leading static Python function docstring is metadata and must not affect a returned expression.",
    },
    "axis_python_docstring_different_text_identity": {
        "axis": "python_docstring_noop",
        "why": "Different Python docstring text is documentation metadata, not callable behavior.",
    },
    "axis_python_docstring_returned_string_boundary": {
        "axis": "python_docstring_noop",
        "why": "Returned string literals are behavior-defining values and must not be treated as docstrings.",
    },
    "axis_python_docstring_assigned_string_boundary": {
        "axis": "python_docstring_noop",
        "why": "String literals assigned and returned through locals are behavior-defining values.",
    },
    "axis_python_docstring_fstring_boundary": {
        "axis": "python_docstring_noop",
        "why": "A leading f-string expression can evaluate dynamic formatting and is not a static docstring proof.",
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
    "axis_collection_typed_domain_array_boundary": {
        "axis": "collection_empty_check",
        "why": "A typed Java receiver collection empty check is not equivalent to a Java array length-empty check without an array receiver proof.",
    },
    "axis_collection_typed_domain_string_boundary": {
        "axis": "collection_empty_check",
        "why": "A typed Java receiver collection empty check is not equivalent to a Java string empty check without a string receiver proof.",
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
    "axis_membership_typefact_python_tuple_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python `tuple[T, ...]` parameter should be treated as a proven dynamic collection receiver for membership.",
    },
    "axis_membership_typefact_java_queue_identity": {
        "axis": "literal_collection_membership",
        "why": "A Java `Queue<T>` parameter should prove the same collection-membership predicate as other typed dynamic collections.",
    },
    "axis_membership_typefact_rust_vecdeque_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust `VecDeque<T>` parameter should prove the same collection-membership predicate as other typed dynamic collections.",
    },
    "axis_membership_python_alias_sequence_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python alias import of `typing.Sequence` used as a parameter annotation should prove typed dynamic collection membership.",
    },
    "axis_membership_python_alias_container_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python alias import of `collections.abc.Container` used as a parameter annotation should prove typed dynamic collection membership.",
    },
    "axis_membership_python_alias_set_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python alias import of `typing.Set` used as a parameter annotation should prove typed dynamic collection membership.",
    },
    "axis_membership_python_alias_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python alias-proven collection membership remains tied to a specific element coordinate.",
    },
    "axis_membership_python_alias_wrong_receiver_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python alias-proven collection membership remains tied to a specific receiver coordinate.",
    },
    "axis_membership_python_alias_unresolved_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Python collection annotation alias without a proven stdlib import is not strict collection-membership evidence.",
    },
    "axis_membership_python_alias_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Python collection annotation alias shadowed before use is not strict collection-membership evidence.",
    },
    "axis_membership_python_set_factory_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python builtin `set([...]).__contains__(value)` factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_tuple_factory_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python builtin `tuple([...]).__contains__(value)` factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_frozenset_factory_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python builtin `frozenset([...]).__contains__(value)` factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_deque_import_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python `collections.deque` imported factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_deque_alias_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python aliased `collections.deque` imported factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_deque_namespace_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python namespace-qualified `collections.deque` factory over static items should prove the same literal membership predicate.",
    },
    "axis_membership_python_deque_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python deque factory membership remains tied to a specific element coordinate.",
    },
    "axis_membership_python_deque_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python deque factory membership over different static items changes behavior.",
    },
    "axis_membership_python_deque_missing_import_boundary": {
        "axis": "literal_collection_membership",
        "why": "A free Python `deque` name is not strict stdlib factory evidence without a proven import.",
    },
    "axis_membership_python_deque_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Python `deque` binding shadowed after import is not proof of the stdlib factory.",
    },
    "axis_membership_python_deque_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Python deque binding mutated after construction is not the original static collection.",
    },
    "axis_membership_python_factory_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python builtin collection factory membership remains tied to a specific element coordinate.",
    },
    "axis_membership_python_factory_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Python builtin collection factory membership over different static items changes the collection coordinate.",
    },
    "axis_membership_python_factory_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A shadowed Python collection factory name is not proof of the builtin collection constructor.",
    },
    "axis_membership_local_go_slice_identity": {
        "axis": "literal_collection_membership",
        "why": "A Go function-local slice literal bound once and consumed by `slices.Contains` should prove the same static membership predicate.",
    },
    "axis_membership_local_java_list_identity": {
        "axis": "literal_collection_membership",
        "why": "A Java function-local `List.of(...)` binding consumed by `.contains` should prove the same static membership predicate.",
    },
    "axis_membership_local_rust_vec_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust function-local `vec![...]` binding consumed by `.contains` should prove the same static membership predicate.",
    },
    "axis_membership_local_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Function-local constructed collection membership remains tied to a specific element coordinate.",
    },
    "axis_membership_local_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Function-local constructed collection membership over different static items changes the collection coordinate.",
    },
    "axis_membership_local_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A function-local collection binding that is mutated before membership is not the original static collection.",
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
    "axis_membership_array_some_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.some(item => item === value)` existential predicate should prove the same literal collection-membership coordinate.",
    },
    "axis_membership_array_some_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.some` membership remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_some_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.some` membership over different static items changes the collection coordinate.",
    },
    "axis_membership_array_every_absence_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.every(item => item !== value)` absence predicate should prove the same negated literal collection-membership coordinate.",
    },
    "axis_membership_array_every_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.every` absence remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_every_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.every` absence over different static items changes the collection coordinate.",
    },
    "axis_membership_array_indexof_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.indexOf(value)` membership comparison should prove the same literal collection-membership coordinate.",
    },
    "axis_membership_array_indexof_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.indexOf` membership remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_indexof_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.indexOf` membership over different static items changes the collection coordinate.",
    },
    "axis_membership_array_findindex_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.findIndex(item => item === value)` membership comparison should prove the same literal collection-membership coordinate.",
    },
    "axis_membership_array_findindex_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.findIndex` membership remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_findindex_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.findIndex` membership over different static items changes the collection coordinate.",
    },
    "axis_membership_array_filter_length_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.filter(item => item === value).length` nonempty check should prove the same literal collection-membership coordinate.",
    },
    "axis_membership_array_filter_length_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.filter(...).length` membership remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_filter_length_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.filter(...).length` membership over different static items changes the collection coordinate.",
    },
    "axis_membership_array_filter_length_absence_identity": {
        "axis": "literal_collection_membership",
        "why": "A static array `.filter(item => item === value).length` zero check should prove the same negated literal collection-membership coordinate.",
    },
    "axis_membership_array_filter_length_absence_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.filter(...).length` absence remains a proof over a specific searched element coordinate.",
    },
    "axis_membership_array_filter_length_absence_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Array `.filter(...).length` absence over different static items changes the collection coordinate.",
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
    "axis_membership_module_python_tuple_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python module-level immutable tuple literal binding should prove literal collection membership when the binding is not mutated.",
    },
    "axis_membership_module_python_set_identity": {
        "axis": "literal_collection_membership",
        "why": "A Python module-level immutable set literal binding should prove literal collection membership when the binding is not mutated.",
    },
    "axis_membership_module_python_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Python module-level collection binding mutated after initialization is not a strict literal membership proof.",
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
    "axis_membership_rust_std_hashset_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust `std::collections::HashSet::from([...])` binding consumed by `.contains` should prove the same static membership predicate.",
    },
    "axis_membership_rust_std_btreeset_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust `std::collections::BTreeSet::from([...])` binding consumed by `.contains` should prove the same static membership predicate.",
    },
    "axis_membership_rust_std_vecdeque_identity": {
        "axis": "literal_collection_membership",
        "why": "A Rust `std::collections::VecDeque::from([...])` binding consumed by `.contains` should prove the same static membership predicate.",
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
    "axis_membership_rust_std_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Rust std collection factory membership remains tied to a specific element coordinate.",
    },
    "axis_membership_rust_std_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Rust std collection factory membership over different static items changes the collection coordinate.",
    },
    "axis_membership_rust_std_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Rust std collection factory binding mutated after construction is not the original static collection.",
    },
    "axis_membership_ruby_set_new_include_identity": {
        "axis": "literal_collection_membership",
        "why": "A Ruby `Set.new([...]).include?(value)` with a proven `require \"set\"` should prove the same static membership predicate.",
    },
    "axis_membership_ruby_set_new_member_identity": {
        "axis": "literal_collection_membership",
        "why": "A Ruby `Set.new([...]).member?(value)` with a proven `require \"set\"` should prove the same static membership predicate.",
    },
    "axis_membership_ruby_set_local_identity": {
        "axis": "literal_collection_membership",
        "why": "A local Ruby `Set.new([...])` binding consumed by `.include?` should prove the same static membership predicate when unmutated.",
    },
    "axis_membership_ruby_set_wrong_element_boundary": {
        "axis": "literal_collection_membership",
        "why": "Ruby Set membership remains tied to a specific element coordinate.",
    },
    "axis_membership_ruby_set_wrong_collection_boundary": {
        "axis": "literal_collection_membership",
        "why": "Ruby Set membership over a different static collection changes behavior.",
    },
    "axis_membership_ruby_set_missing_require_boundary": {
        "axis": "literal_collection_membership",
        "why": "Ruby `Set.new` is not strict stdlib Set evidence without a proven `require \"set\"`.",
    },
    "axis_membership_ruby_set_shadowed_boundary": {
        "axis": "literal_collection_membership",
        "why": "A locally defined Ruby `Set` constant is not proof of the standard Set factory.",
    },
    "axis_membership_ruby_set_mutated_boundary": {
        "axis": "literal_collection_membership",
        "why": "A Ruby Set binding mutated after construction no longer proves the original static collection membership predicate.",
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
    "axis_map_key_python_keys_in_identity": {
        "axis": "map_key_membership",
        "why": "Python typed `key in lookup.keys()` should prove the same key-in-map predicate as direct map membership.",
    },
    "axis_map_key_python_keys_contains_identity": {
        "axis": "map_key_membership",
        "why": "Python typed `lookup.keys().__contains__(key)` should prove the same key-in-map predicate as direct map membership.",
    },
    "axis_map_key_python_keys_wrong_key_boundary": {
        "axis": "map_key_membership",
        "why": "Python map key-view membership remains tied to a specific key coordinate.",
    },
    "axis_map_key_python_keys_wrong_map_boundary": {
        "axis": "map_key_membership",
        "why": "Python map key-view membership remains tied to a specific map receiver coordinate.",
    },
    "axis_map_key_python_keys_value_boundary": {
        "axis": "map_key_membership",
        "why": "Python map value-view membership is not the same predicate as key-view membership.",
    },
    "axis_map_key_ts_array_from_keys_identity": {
        "axis": "map_key_membership",
        "why": "TypeScript typed `Array.from(lookup.keys()).includes(key)` should prove the same key-in-map predicate as `Map.has`.",
    },
    "axis_map_key_ts_array_from_keys_wrong_key_boundary": {
        "axis": "map_key_membership",
        "why": "TypeScript `Array.from(lookup.keys()).includes(...)` remains tied to a specific key coordinate.",
    },
    "axis_map_key_ts_array_from_keys_wrong_map_boundary": {
        "axis": "map_key_membership",
        "why": "TypeScript `Array.from(lookup.keys()).includes(...)` remains tied to a specific map receiver coordinate.",
    },
    "axis_map_key_ts_array_from_keys_value_boundary": {
        "axis": "map_key_membership",
        "why": "TypeScript `Array.from(lookup.values()).includes(...)` is value membership, not key membership.",
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
    "axis_map_default_ruby_fetch_block_int_identity": {
        "axis": "literal_map_default_lookup",
        "why": "Ruby `Hash#fetch(key) { fallback }` with a pure zero-arg block should prove the same missing-key fallback as `fetch(key, fallback)`.",
    },
    "axis_map_default_ruby_fetch_block_string_identity": {
        "axis": "literal_map_default_lookup",
        "why": "Ruby `Hash#fetch(key) { fallback }` should preserve string fallback coordinates.",
    },
    "axis_map_default_ruby_fetch_block_bool_identity": {
        "axis": "literal_map_default_lookup",
        "why": "Ruby `Hash#fetch(key) { fallback }` should preserve boolean fallback coordinates.",
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
    "axis_map_default_go_map_inline_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Go inline `map[string]int{...}[key]` lookup should prove literal-map default lookup with the integer zero value fallback.",
    },
    "axis_map_default_go_map_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Go `map[string]int{...}` binding followed by index lookup should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_go_map_var_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Go `var lookup = map[string]int{...}` binding followed by index lookup should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_go_map_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Go literal map index default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_go_map_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Go literal map index default lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_go_zero_string_inline_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Go inline `map[string]string{...}[key]` lookup should prove literal-map default lookup with the string zero value fallback.",
    },
    "axis_map_default_go_zero_string_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Go `map[string]string{...}` binding followed by index lookup should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_go_zero_bool_inline_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Go inline `map[string]bool{...}[key]` lookup should prove literal-map default lookup with the boolean zero value fallback.",
    },
    "axis_map_default_go_zero_float_inline_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Go inline `map[string]float64{...}[key]` lookup should prove literal-map default lookup with the float zero value fallback.",
    },
    "axis_map_default_go_zero_float_local_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A local Go `map[string]float64{...}` binding followed by index lookup should preserve literal-map default proof coordinates.",
    },
    "axis_map_default_go_zero_nil_pointer_identity": {
        "axis": "literal_map_default_lookup",
        "why": "A Go inline `map[string]*Item{...}[key]` lookup with nil entries should prove literal-map default lookup with the nil zero value fallback.",
    },
    "axis_map_default_go_zero_wrong_key_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Go zero-value literal map lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_default_go_zero_wrong_map_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "Go zero-value literal map lookups over different static entries change present-key behavior.",
    },
    "axis_map_default_go_zero_mixed_value_boundary": {
        "axis": "literal_map_default_lookup",
        "why": "A Go literal map with mixed value literal kinds does not have one strict zero-value fallback proof.",
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
    "axis_map_fallback_ts_guard_return_identity": {
        "axis": "map_default_lookup",
        "why": "A typed TypeScript early-return `Map.has` guard should prove the same map/key/fallback behavior as existing map-default forms.",
    },
    "axis_map_fallback_java_guard_return_identity": {
        "axis": "map_default_lookup",
        "why": "A typed Java early-return `containsKey` guard should prove the same map/key/fallback behavior as existing map-default forms.",
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
    "axis_map_fallback_python_dict_get_identity": {
        "axis": "map_default_lookup",
        "why": "A typed Python `dict[str, int].get(key, fallback)` call should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_mapping_get_identity": {
        "axis": "map_default_lookup",
        "why": "A typed Python `Mapping[str, int].get(key, fallback)` call should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_mutable_mapping_get_identity": {
        "axis": "map_default_lookup",
        "why": "A typed Python `MutableMapping[str, int].get(key, fallback)` call should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_alias_mapping_identity": {
        "axis": "map_default_lookup",
        "why": "A Python alias import of `collections.abc.Mapping` used as a parameter annotation should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_alias_mutable_mapping_identity": {
        "axis": "map_default_lookup",
        "why": "A Python alias import of `collections.abc.MutableMapping` used as a parameter annotation should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_alias_dict_identity": {
        "axis": "map_default_lookup",
        "why": "A Python alias import of `typing.Dict` used as a parameter annotation should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_guard_return_identity": {
        "axis": "map_default_lookup",
        "why": "A typed Python `key in dict` early-return guard should prove dynamic map-default lookup.",
    },
    "axis_map_fallback_python_wrong_key_boundary": {
        "axis": "map_default_lookup",
        "why": "Typed Python map-default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_fallback_python_wrong_default_boundary": {
        "axis": "map_default_lookup",
        "why": "Typed Python map-default lookups with different fallback parameters change absent-key behavior.",
    },
    "axis_map_fallback_python_wrong_map_boundary": {
        "axis": "map_default_lookup",
        "why": "Typed Python map-default lookups over different receiver maps change present-key behavior.",
    },
    "axis_map_fallback_python_untyped_boundary": {
        "axis": "map_default_lookup",
        "why": "Untyped Python `.get(key, fallback)` cannot prove receiver/key/default semantics.",
    },
    "axis_map_fallback_python_alias_wrong_key_boundary": {
        "axis": "map_default_lookup",
        "why": "Python alias-proven map-default lookups over different key parameters are different proof coordinates.",
    },
    "axis_map_fallback_python_alias_wrong_default_boundary": {
        "axis": "map_default_lookup",
        "why": "Python alias-proven map-default lookups with different fallback parameters change absent-key behavior.",
    },
    "axis_map_fallback_python_alias_wrong_map_boundary": {
        "axis": "map_default_lookup",
        "why": "Python alias-proven map-default lookups over different receiver maps change present-key behavior.",
    },
    "axis_map_fallback_python_alias_unresolved_boundary": {
        "axis": "map_default_lookup",
        "why": "A Python annotation alias without a proven stdlib map import is not strict map-default evidence.",
    },
    "axis_map_fallback_python_alias_shadowed_boundary": {
        "axis": "map_default_lookup",
        "why": "A Python stdlib map annotation alias shadowed before use is not strict map-default evidence.",
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
