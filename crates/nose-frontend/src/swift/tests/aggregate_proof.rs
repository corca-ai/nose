use super::*;

fn has_all_satisfy_barrier(source: &str) -> bool {
    let (il, interner) = il_with_interner(source);
    il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if interner.resolve(name)
            == SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER)
    })
}

#[test]
fn all_satisfy_dispatch_and_namespace_risks_emit_a_barrier() {
    for source in [
        r#"extension Array where Element == Int {
  func allSatisfy(_ predicate: (Int) -> Bool) -> Bool { false }
}
"#,
        r#"extension Array where Element == Int {
  var `allSatisfy`: (((Int) -> Bool) -> Bool) { { _ in false } }
}
"#,
        "import Foundation\n",
        "typealias Values = [Int]\n",
        "macro makeAll() = #externalMacro(module: \"M\", type: \"T\")\n",
    ] {
        assert!(
            has_all_satisfy_barrier(source),
            "Swift allSatisfy proof risk must remain visible to corpus admission: {source}"
        );
    }
}

#[test]
fn ordinary_all_satisfy_use_does_not_emit_a_dispatch_barrier() {
    assert!(!has_all_satisfy_barrier(
        r#"func f(_ values: [Int]) -> Bool {
  values.allSatisfy { value in value >= 0 }
}
"#
    ));
}

#[test]
fn disjoint_callback_arity_overload_does_not_emit_a_dispatch_barrier() {
    assert!(!has_all_satisfy_barrier(
        r#"extension Array {
  func allSatisfy(_ predicate: (Element, Int) -> Bool) -> Bool { false }

  func standardUse() -> Bool {
    allSatisfy { value in true }
  }
}
"#
    ));
}

#[test]
fn any_unary_compatible_overload_emits_a_dispatch_barrier() {
    assert!(has_all_satisfy_barrier(
        r#"extension Array {
  func allSatisfy(_ predicate: (Element) -> Bool) -> Bool { false }
  func allSatisfy(_ predicate: (Element, Int) -> Bool) -> Bool { false }
}
"#
    ));
}
