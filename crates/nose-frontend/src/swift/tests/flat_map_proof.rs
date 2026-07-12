use super::*;

fn has_flat_map_barrier(source: &str) -> bool {
    let (il, interner) = il_with_interner(source);
    il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if interner.resolve(name)
            == SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER)
    })
}

#[test]
fn flat_map_dispatch_and_namespace_risks_emit_a_barrier() {
    for source in [
        r#"extension Array where Element == [Bool] {
  func flatMap<T>(_ transform: ([Bool]) -> [T]) -> [T] { [] }
}
"#,
        r#"extension Array where Element == Bool {
  func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
"#,
        r#"extension Array where Element == Bool {
  func filter(_ predicate: (Bool) -> Bool) -> [Bool] { [] }
}
"#,
        r#"extension Array where Element == [Bool] {
  var `flatMap`: ((([Bool]) -> [Bool]) -> [Bool]) { { _ in [] } }
}
"#,
        "import Foundation\n",
        "typealias Rows = [[Bool]]\n",
        "macro makeFlatMap() = #externalMacro(module: \"M\", type: \"T\")\n",
    ] {
        assert!(
            has_flat_map_barrier(source),
            "Swift flatMap proof risk must remain visible to corpus admission: {source}"
        );
    }
}

#[test]
fn ordinary_flat_map_use_does_not_emit_a_dispatch_barrier() {
    assert!(!has_flat_map_barrier(
        r#"func f(_ groups: [[Bool]]) -> [Bool] {
  groups.flatMap { (group: [Bool]) in group.map { value in value } }
}
"#
    ));
}

#[test]
fn attributed_flat_map_callback_parameters_remain_non_plain() {
    let (il, interner) = il_with_interner(
        r#"@propertyWrapper
struct ForceCollection {
  var wrappedValue: [Bool]
  init(wrappedValue: [Bool]) { self.wrappedValue = [true] }
}
@propertyWrapper
struct ForceValue {
  var wrappedValue: Bool
  init(wrappedValue: Bool) { self.wrappedValue = true }
}
func outer(_ groups: [[Bool]]) -> [Bool] {
  groups.flatMap { (@ForceCollection group: [Bool]) in group }
}
func inner(_ groups: [[Bool]]) -> [Bool] {
  groups.flatMap { (row: [Bool]) in
    row.map { (@ForceValue value: Bool) in value }
  }
}
"#,
    );

    for name in ["group", "value"] {
        let params = il
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                matches!(node.payload, Payload::Name(symbol) if interner.resolve(symbol) == name)
                    .then_some(NodeId(index as u32))
            })
            .filter(|&node| il.kind(node) == NodeKind::Param)
            .filter(|&param| {
                il.nodes.iter().enumerate().any(|(index, node)| {
                    node.kind == NodeKind::Lambda
                        && il.children(NodeId(index as u32)).contains(&param)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            params.len(),
            1,
            "expected one attributed `{name}` parameter"
        );
        assert!(
            !il.children(params[0]).is_empty(),
            "attributed `{name}` must not look like a plain callback coordinate"
        );
    }

    let group = il
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            (node.kind == NodeKind::Param
                && matches!(node.payload, Payload::Name(symbol) if interner.resolve(symbol) == "group"))
            .then_some(NodeId(index as u32))
            .filter(|param| {
                il.nodes.iter().enumerate().any(|(scope_index, scope)| {
                    scope.kind == NodeKind::Lambda
                        && il
                            .children(NodeId(scope_index as u32))
                            .contains(param)
                })
            })
        })
        .expect("attributed collection parameter");
    assert!(
        !il.evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::param(il.node(group).span)
                && record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter)
        }),
        "a property-wrapped callback collection must not receive plain bracket proof"
    );
    assert!(raw_names(&il, &interner)
        .iter()
        .any(|name| name == "swift_non_plain_parameter"));
    assert!(crate::is_intentional_raw_boundary_tag(
        "swift_non_plain_parameter"
    ));
}
