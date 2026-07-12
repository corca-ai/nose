use super::*;

#[test]
fn callback_sensitive_swift_prefix_and_unwrap_surfaces_are_not_erased() {
    let src = r#"
prefix operator +++
prefix func +++ (value: Int) -> Int { value }

func variants(_ xs: [Int], _ optional: Int?) {
    let positive = xs.map { value in +value }
    let forced = xs.map { _ in optional! }
    let custom = xs.map { value in +++value }
    let interpolated = xs.map { value in "\(value)" }
    _ = (positive, forced, custom, interpolated)
}

func observeCapture() -> Int { 0 }
func captured(_ xs: [Int]) -> [Int] {
    xs.map { [snapshot = observeCapture()] value in value }
}

func casts(_ xs: [Any]) {
    let forced = xs.map { value in value as! Int }
    let conditional = xs.map { value in value as? Int }
    let checked = xs.map { value in value is Int }
    _ = (forced, conditional, checked)
}
"#;
    let (il, interner) = il_with_interner(src);

    assert!(
        il.nodes
            .iter()
            .any(|node| node.kind == NodeKind::UnOp && node.payload == Payload::Op(Op::Pos)),
        "Swift prefix `+` must remain an explicit operator"
    );
    let raw = raw_names(&il, &interner);
    assert!(
        raw.iter().any(|name| name == "swift_force_unwrap"),
        "force unwrap must remain a trapping boundary: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag("swift_force_unwrap"));
    assert!(
        raw.iter().any(|name| name == "swift_capture_list"),
        "closure capture lists must remain an effect/lifetime boundary: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag("swift_capture_list"));
    assert!(
        raw.iter().any(|name| name == "swift_cast_or_type_check"),
        "Swift casts/type checks must remain trapping or dispatching boundaries: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag(
        "swift_cast_or_type_check"
    ));
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "swift_prefix_operator"),
        "custom prefix dispatch must remain a Swift-specific sequence surface: {seq:?}"
    );
    assert!(
        seq.iter().any(|name| name == "interpolated_string"),
        "string interpolation must remain distinct from a literal string: {seq:?}"
    );
}

#[test]
fn swift_pattern_and_keypath_surfaces_do_not_fall_to_generic_raw() {
    let (il, interner) = il_with_interner(
        r#"
enum Direction {
case north
case named(String)
}

func classify(_ value: Direction?, _ xs: [Int]) -> Int {
if let value = value {
    switch value {
    case .north:
        return xs[keyPath: \.count] > 0 ? 1 : 2
    case .named(let label):
        return label.count
    default:
        return 0
    }
}
return xs[0..<xs.count].count
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "switch_pattern",
        "value_binding_pattern",
        "enum_entry",
        "key_path_expression",
        "range_expression ..<",
        "ternary_expression",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should lower to structured IL, got {raw:?}"
        );
    }
}

#[test]
fn macro_invocations_do_not_cascade_raw_call_suffixes() {
    let (il, interner) = il_with_interner(
        r#"
func check(_ value: Int) {
#expect(value == 1)
#warning("generated fixture")
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "macro_invocation",
        "call_suffix",
        "value_arguments",
        "value_argument",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should not stay Raw in macro calls: {raw:?}"
        );
    }
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter().any(|name| name == "swift_macro_invocation"),
        "macro invocation should use the exact-closed Swift tag: {seq:?}"
    );
    assert!(
        seq.iter().any(|name| name == "swift_diagnostic_warning"),
        "diagnostic should preserve warning/error kind: {seq:?}"
    );
}

#[test]
fn selector_literals_and_local_typealiases_do_not_fall_to_raw() {
    let (il, interner) = il_with_interner(
        r#"
class C {
  @objc func foo(_ value: Int) {}
  func f() {
    typealias Callback = (Int) -> Void
    let a = #selector(foo(_:))
    let b = #selector(getter: C.description)
  }
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in ["selector_expression", "typealias_declaration"] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should not stay Raw: {raw:?}"
        );
    }
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter()
            .filter(|name| name.as_str() == "swift_selector_expression")
            .count()
            >= 2,
        "selector literals should preserve an exact-closed Swift selector tag: {seq:?}"
    );
}

#[test]
fn computed_property_accessors_do_not_fall_to_raw() {
    let (il, interner) = il_with_interner(
        r#"
struct Box {
var storage: Int
var value: Int {
    get { storage }
    set(newValue) { storage = newValue }
}
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "computed_getter",
        "getter_specifier",
        "computed_setter",
        "setter_specifier",
        "computed_property",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should lower through accessor bodies: {raw:?}"
        );
    }
}

#[test]
fn swift_operator_literal_and_range_surfaces_do_not_fall_to_raw() {
    let (il, interner) = il_with_interner(
        r#"
final class Token {}

func risky() throws -> Int { 1 }

func ops(_ a: Int, _ b: Int, _ left: Token, _ right: Token) -> Any {
var x = a
x &+= b
let y = a &+ b
let z = a &- b
let w = a &* b
let id = left === right
let notId = left !== right
let slice = [a, b][...]
let forced = try! risky()
let info = (#fileID, #line, #function)
return (x, y, z, w, id, notId, slice, forced, info)
}
"#,
    );
    let raw = raw_names(&il, &interner);
    for unexpected in [
        "infix_expression &+=",
        "infix_expression &+",
        "infix_expression &-",
        "infix_expression &*",
        "equality_expression ===",
        "infix_expression !==",
        "fully_open_range",
        "bang",
        "special_literal",
    ] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should lower to Swift-specific structured IL: {raw:?}"
        );
    }
    let seq = seq_names(&il, &interner);
    for expected in [
        "swift_overflow_add",
        "swift_overflow_sub",
        "swift_overflow_mul",
        "swift_identity_eq",
        "swift_identity_ne",
        "swift_range_fully_open",
        "swift_special_literal",
    ] {
        assert!(
            seq.iter().any(|name| name == expected),
            "{expected} should be preserved as an exact-closed Swift tag: {seq:?}"
        );
    }
    assert!(
        has_assign_rhs_seq(&il, &interner, "swift_overflow_add"),
        "&+= should lower as mutation with a Swift overflow-add RHS"
    );
}

#[test]
fn swift_operator_references_are_exact_closed_not_binary_ops() {
    let src = r#"
func equal(_ expected: Int, by cmp: (Int, Int) -> Bool) -> Bool { cmp(expected, expected) }
func refs(_ values: [Int]) -> Any {
let total = values.reduce(0, +)
let same = equal(1, by: ==)
return (total, same)
}
"#;
    let (il, interner) = il_with_interner(src);
    let raw = raw_names(&il, &interner);
    for unexpected in ["+", "=="] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "operator reference {unexpected} should not remain Raw: {raw:?}"
        );
    }
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter()
            .filter(|name| name.as_str() == "swift_operator_ref")
            .count()
            >= 2,
        "operator references should use Swift exact-closed surfaces: {seq:?}"
    );
    assert_eq!(
        op_count(src, Op::Add),
        0,
        "passing `+` as a function value must not become BinOp::Add"
    );
    assert_eq!(
        op_count(src, Op::Eq),
        0,
        "passing `==` as a function value must not become BinOp::Eq"
    );
}

#[test]
fn swift_prefix_operator_surfaces_do_not_impersonate_common_ops() {
    let src = r#"
enum Action { case view(Int) }
func route(_ value: Any) {}
func refs(_ x: Int) {
route(/Action.view)
route(/Other.view)
route(~x)
}
"#;
    let (il, interner) = il_with_interner(src);
    let raw = raw_names(&il, &interner);
    for unexpected in ["/", "prefix_expression"] {
        assert!(
            !raw.iter().any(|name| name == unexpected),
            "{unexpected} should lower as a Swift prefix/operator surface: {raw:?}"
        );
    }
    let seq = seq_names(&il, &interner);
    assert!(
        seq.iter()
            .filter(|name| name.as_str() == "swift_prefix_operator")
            .count()
            >= 1,
        "custom prefix operators should use exact-closed Swift surfaces: {seq:?}"
    );
    assert!(
        seq.iter().any(|name| name == "swift_case_path"),
        "case-path operator references should stay Swift-specific: {seq:?}"
    );
    let case_path_hashes = seq_first_string_hashes(&il, &interner, "swift_case_path");
    assert_eq!(
        case_path_hashes.len(),
        2,
        "both case paths should be preserved as exact-closed source surfaces"
    );
    assert_ne!(
        case_path_hashes[0], case_path_hashes[1],
        "`/Action.view` and `/Other.view` must not collapse to the same field access"
    );
    assert_eq!(
        op_count(src, Op::Div),
        0,
        "`/Action.view` is a case-path prefix operator, not division"
    );
}

#[test]
fn swift_labeled_control_flow_preserves_target_as_boundary() {
    let src = r#"
func f(_ xs: [Int]) {
outer: for x in xs {
    if x > 0 { break outer }
}
}
"#;
    let raw = raw_names_for_src(src);
    assert!(
        !raw.iter().any(|name| name == "statement_label"),
        "statement labels should preserve their target spelling, got {raw:?}"
    );
    assert!(
        raw.iter().any(|name| name == "swift_labeled_break outer"),
        "labeled break must stay fail-closed with its target label: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag(
        "swift_labeled_break outer"
    ));
}

#[test]
fn swift_trailing_closure_labels_do_not_count_as_statement_label_gaps() {
    let src = r#"
func withHandlers() {
withUnsafeTemporaryAllocation(capacity: 1) { buffer in
    _ = buffer
}
errorHandler: { error in
    _ = error
}
}
"#;
    let raw = raw_names_for_src(src);
    assert!(
        !raw.iter().any(|name| name == "statement_label"),
        "trailing closure labels should preserve spelling instead of generic statement_label Raw: {raw:?}"
    );
}

#[test]
fn conditional_compilation_directives_do_not_fall_to_raw() {
    let (il, interner) = il_with_interner(
        r#"
#if os(macOS)
let platform = 1
#elseif canImport(Glibc)
let platform = 2
#else
let platform = 3
#endif
"#,
    );
    let raw = raw_names(&il, &interner);
    assert!(
        !raw.iter().any(|name| name == "directive"),
        "conditional compilation directives should lower to Swift-specific markers: {raw:?}"
    );
    let seq = seq_names(&il, &interner);
    for expected in [
        "swift_directive_if",
        "swift_directive_elseif",
        "swift_directive_else",
        "swift_directive_endif",
    ] {
        assert!(
            seq.iter().any(|name| name == expected),
            "{expected} should preserve conditional-compilation shape: {seq:?}"
        );
    }
}
