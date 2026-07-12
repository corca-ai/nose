use super::support::{
    lower_js_with_interner, lower_ts_with_interner, raw_names, seq_names,
    sequence_surface_count_for_seq_name, source_binding_count, unit_root_seq_names,
};
use nose_il::{SequenceSurfaceKind, SourceBindingKind, UnitKind};

#[test]
fn callback_parameter_shapes_remain_visible_to_semantic_admission() {
    let (il, interner) = lower_ts_with_interner(
        r#"
declare function observe(): number;
function variants(xs: unknown[]) {
  const defaulted = xs.map((x = observe()) => x);
  const rest = xs.map((...values) => values);
  const destructured = xs.map(({ value }) => value);
  const typed = xs.map((x: number) => x);
  const functionTyped = xs.map((callback: (value: number) => number) => callback);
  const undefinedNamed = xs.map((undefined) => undefined);
  return [defaulted, rest, destructured, typed, functionTyped, undefinedNamed];
}
"#,
    );

    let raw = raw_names(&il, &interner);
    for expected in [
        "js_default_parameter",
        "js_rest_parameter",
        "js_destructured_parameter",
    ] {
        assert!(
            raw.iter().any(|name| name == expected),
            "{expected} must remain visible on its Param node: {raw:?}"
        );
        assert!(
            crate::is_intentional_raw_boundary_tag(expected),
            "{expected} is a deliberate fail-closed parameter boundary"
        );
    }
    assert_eq!(
        raw.iter()
            .filter(|name| name.starts_with("js_") && name.ends_with("_parameter"))
            .count(),
        3,
        "a plain typed parameter must stay childless while non-plain forms are marked: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag(
        "js_non_plain_parameter"
    ));
}

#[test]
fn ts_object_opaque_surfaces_do_not_cascade_raw_wrappers() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function build(rest: object, key: string, target: Record<string, number>) {
  return {
    ...rest,
    [key]: 1,
    run(value: number) {
      return void value;
    },
    drop() {
      return delete target[key];
    }
  };
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "spread_element",
        "method_definition",
        "formal_parameters",
        "statement_block",
        "return_statement",
        "computed_property_name",
        "void",
        "delete",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should lower to exact-closed structured IL, got {raw:?}"
        );
    }
}

#[test]
fn js_sparse_arrays_do_not_emit_exact_array_literal_surface() {
    let (il, interner) = lower_ts_with_interner(
        r#"
const dense = [1, 2,];
const sparse = [1, , 2];
const leading = [, 1];
const trailing = [1,];
"#,
    );

    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "array"),
        "dense arrays should keep the normal array surface: {seq:?}"
    );
    assert!(
        seq.iter().any(|name| name == "js_sparse_array"),
        "array elisions should lower to an exact-closed sparse-array surface: {seq:?}"
    );
    assert_eq!(
        sequence_surface_count_for_seq_name(
            &il,
            &interner,
            "js_sparse_array",
            SequenceSurfaceKind::Collection,
        ),
        0,
        "sparse JS arrays must not mint collection sequence-surface proof"
    );
    assert!(
        sequence_surface_count_for_seq_name(
            &il,
            &interner,
            "array",
            SequenceSurfaceKind::Collection
        ) >= 2,
        "dense arrays, including trailing-comma arrays, should still mint collection proof"
    );
}

#[test]
fn js_array_spread_remains_an_unproven_sequence_boundary() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function expand(xs: number[], source: number[]) {
  return xs.map((_value) => [...source]);
}
"#,
    );

    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "js_array_spread"),
        "array spread must remain visible inside the outer array literal: {seq:?}"
    );
    assert_eq!(
        sequence_surface_count_for_seq_name(
            &il,
            &interner,
            "js_array_spread",
            SequenceSurfaceKind::Collection,
        ),
        0,
        "an array-spread marker must not mint exact collection proof"
    );
}

#[test]
fn js_class_heritage_expression_remains_in_the_class_unit() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function observeBase(): typeof Object { return Object }
function variants(xs: number[]) {
  return xs.map((_value) => [class extends observeBase() {}]);
}
"#,
    );
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "js_class_definition"),
        "runtime extends expressions must remain attached to the class unit: {seq:?}"
    );
    assert!(
        il.nodes
            .iter()
            .any(|node| node.kind == nose_il::NodeKind::Call),
        "the heritage call must not disappear from lowered IL"
    );
}

#[test]
fn javascript_class_heritage_expression_remains_in_the_class_unit() {
    let (il, interner) = lower_js_with_interner(
        r#"
function observeBase() { return Object }
function variants(xs) {
  return xs.map((_value) => [class extends observeBase() {}]);
}
"#,
    );
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "js_class_definition"),
        "JavaScript's direct class_heritage child must remain attached: {seq:?}"
    );
    assert!(
        il.nodes
            .iter()
            .any(|node| node.kind == nose_il::NodeKind::Call),
        "the JavaScript heritage call must not disappear from lowered IL"
    );
}

#[test]
fn ts_decorators_lower_to_structured_surfaces_and_binding_facts() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function dec(value: unknown, ctx: unknown) { return value }

@dec
class Box {
  static { Box.ready = true }
  @dec value = 1
  @dec method() { return this.value }
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "decorator",
        "class_static_block",
        "statement_block",
        "expression_statement",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should not remain Raw after TS decorator lowering: {raw:?}"
        );
    }

    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter()
            .filter(|name| name.as_str() == "js_decorator")
            .count()
            >= 3,
        "decorator expressions should be preserved as exact-closed JS/TS surfaces: {seq:?}"
    );
    assert!(
        seq.iter().any(|name| name == "js_class_static_block"),
        "class static block should be an exact-closed surface: {seq:?}"
    );
    assert!(
        source_binding_count(&il, SourceBindingKind::DecoratedDefinition) >= 3,
        "decorated definitions should record binding source facts"
    );
    assert!(
        unit_root_seq_names(&il, &interner, UnitKind::Class)
            .iter()
            .any(|name| name == "js_decorated_definition"),
        "decorated class units must be rooted at the decorator wrapper"
    );
}

#[test]
fn ts_skipped_decorated_type_member_does_not_decorate_next_member() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function dec(value: unknown, ctx: unknown) { return value }

abstract class Base {
  @dec abstract missing(): number
  run() { return 1 }
}
"#,
    );

    let seq = seq_names(&il, &interner);
    assert!(
        !seq.iter().any(|name| name == "js_decorated_definition"),
        "decorators on erased type-only members must not attach to the next runtime member: {seq:?}"
    );
    assert_eq!(
        source_binding_count(&il, SourceBindingKind::DecoratedDefinition),
        0,
        "erased type-only members should not emit decorated runtime binding facts"
    );
}

#[test]
fn ts_decorated_class_expression_preserves_decorator_and_unit_boundary() {
    let (il, interner) = lower_ts_with_interner(
        r#"
function dec(value: unknown, ctx: unknown) { return value }
const Box = @dec class Box {
  value() { return 1 }
}
"#,
    );
    let raw = raw_names(&il, &interner);
    assert!(
        !raw.iter().any(|name| name == "decorator"),
        "class-expression decorators should not remain Raw: {raw:?}"
    );
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "js_decorated_definition"),
        "decorated class expressions should preserve their decorator surface: {seq:?}"
    );
    assert!(
        unit_root_seq_names(&il, &interner, UnitKind::Class)
            .iter()
            .any(|name| name == "js_decorated_definition"),
        "decorated class-expression units must be rooted at the decorator wrapper"
    );
    assert!(
        source_binding_count(&il, SourceBindingKind::DecoratedDefinition) >= 1,
        "decorated class expressions should record binding source facts"
    );
}
