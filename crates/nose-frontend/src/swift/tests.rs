use super::*;

mod surfaces;

fn il_with_interner(src: &str) -> (Il, Interner) {
    let interner = Interner::default();
    let il = lower(FileId(0), "t.swift", src.as_bytes(), &interner).expect("lower swift");
    (il, interner)
}

fn il(src: &str) -> Il {
    il_with_interner(src).0
}

fn raw_names(il: &Il, interner: &Interner) -> Vec<String> {
    il.nodes
        .iter()
        .filter_map(|node| {
            if node.kind != NodeKind::Raw {
                return None;
            }
            let Payload::Name(name) = node.payload else {
                return None;
            };
            Some(interner.resolve(name).to_string())
        })
        .collect()
}

fn seq_names(il: &Il, interner: &Interner) -> Vec<String> {
    il.nodes
        .iter()
        .filter_map(|node| {
            if node.kind != NodeKind::Seq {
                return None;
            }
            let Payload::Name(name) = node.payload else {
                return None;
            };
            Some(interner.resolve(name).to_string())
        })
        .collect()
}

fn seq_first_string_hashes(il: &Il, interner: &Interner, expected: &str) -> Vec<u64> {
    il.nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| {
            if node.kind != NodeKind::Seq {
                return None;
            }
            let Payload::Name(name) = node.payload else {
                return None;
            };
            if interner.resolve(name) != expected {
                return None;
            }
            let first = *il.children(NodeId(idx as u32)).first()?;
            let Payload::LitStr(hash) = il.node(first).payload else {
                return None;
            };
            Some(hash)
        })
        .collect()
}

fn has_assign_rhs_seq(il: &Il, interner: &Interner, expected: &str) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let children = il.children(NodeId(idx as u32));
        let [_, rhs] = children else {
            return false;
        };
        let rhs = il.node(*rhs);
        if rhs.kind != NodeKind::Seq {
            return false;
        }
        let Payload::Name(name) = rhs.payload else {
            return false;
        };
        interner.resolve(name) == expected
    })
}

fn raw_name_set(src: &str) -> Vec<String> {
    let (il, interner) = il_with_interner(src);
    let mut raw = raw_names(&il, &interner);
    raw.sort();
    raw.dedup();
    raw
}

fn op_count(src: &str, op: Op) -> usize {
    il(src)
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::BinOp && node.payload == Payload::Op(op))
        .count()
}

fn raw_names_for_src(src: &str) -> Vec<String> {
    let (il, interner) = il_with_interner(src);
    raw_names(&il, &interner)
}

#[test]
fn function_lowers_to_unit() {
    let il = il(r#"
func add(_ x: Int, _ y: Int) -> Int {
return x + y
}
"#);
    assert_eq!(il.units.len(), 1);
    assert_eq!(il.meta.lang, Lang::Swift);
}

#[test]
fn if_case_conditions_lower_to_pattern_tests_without_raw_case() {
    let src = r#"
func f(kind: Kind, update: Update) {
  if case (.positional, .nullary) = (kind, update) {
    fail()
  }
  if case .default = kind {
    ok()
  }
  if case .known = kind, ready {
    both()
  }
}
"#;
    let raw = raw_name_set(src);
    assert!(
        !raw.iter().any(|name| name == "case"),
        "if-case conditions should not lower to Raw(\"case\"): {raw:?}"
    );
    assert!(
        op_count(src, Op::Eq) >= 2,
        "if-case conditions should lower to equality-style pattern tests"
    );
    assert!(
        op_count(src, Op::And) >= 1,
        "compound if-case conditions should keep trailing boolean conditions"
    );
}

#[test]
fn unknown_default_switch_entry_is_default_not_raw_case() {
    let src = r#"
func f(value: Value) {
  switch value {
  case .known:
    ok()
  @unknown default:
    fallback()
  }
}
"#;
    let raw = raw_name_set(src);
    assert!(
        !raw.iter().any(|name| name == "switch_case"),
        "@unknown default should lower as a default arm, got {raw:?}"
    );
}

#[test]
fn nil_branch_ternary_lowers_to_if_without_raw() {
    let src = r#"
func f(flag: Bool, value: String) {
  let a = flag ? nil : value
  let b = flag ? value : nil
}
"#;
    let raw = raw_name_set(src);
    assert!(
        !raw.iter().any(|name| name == "ternary_expression"),
        "nil-branch ternary expressions should lower to If, got {raw:?}"
    );
    assert!(
        il(src).nodes.iter().any(|node| node.kind == NodeKind::If),
        "ternary expression should produce an If node"
    );
}

#[test]
fn availability_conditions_are_intentional_boundaries() {
    let raw = raw_name_set(
        r#"
func f() {
  if #available(macOS 13, *) {
    run()
  }
}
"#,
    );
    assert!(
        raw.iter().any(|name| name == "availability_condition"),
        "availability condition should remain fail-closed: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag(
        "availability_condition"
    ));
    assert!(!crate::is_protocol_boundary_tag("availability_condition"));
}

#[test]
fn catch_blocks_do_not_leave_keyword_raw() {
    let src = r#"
func f() {
  do {
    try run()
  } catch {
  }
  do {
    try other()
  } catch {
    recover()
  }
}
"#;
    let raw = raw_name_set(src);
    assert!(
        !raw.iter().any(|name| name == "catch_keyword"),
        "catch keyword should not leak as Raw: {raw:?}"
    );
    assert!(
        il(src).nodes.iter().any(|node| node.kind == NodeKind::Try),
        "do/catch should lower to Try"
    );
}

#[test]
fn foreach_lowers_to_loop() {
    let il = il(r#"
func sumPositive(_ xs: [Int]) -> Int {
var total = 0
for x in xs {
    if x > 0 {
        total += x
    }
}
return total
}
"#);
    assert!(il.nodes.iter().any(|node| {
        node.kind == NodeKind::Loop && node.payload == Payload::Loop(LoopKind::ForEach)
    }));
}

#[test]
fn subscript_lowers_to_index() {
    let il = il(r#"
func get(_ xs: [Int], _ i: Int) -> Int {
return xs[i]
}
"#);
    assert!(il.nodes.iter().any(|node| node.kind == NodeKind::Index));
}

#[test]
fn closure_header_lowers_to_lambda_param() {
    let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
return xs.map { x in x + 1 }
}
"#);
    let lambda = il
        .nodes
        .iter()
        .position(|node| node.kind == NodeKind::Lambda)
        .map(|idx| NodeId(idx as u32))
        .expect("lambda");
    let first = il.children(lambda).first().copied().expect("lambda child");
    assert_eq!(il.kind(first), NodeKind::Param);
}

#[test]
fn closure_type_header_dedupes_lambda_params() {
    let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
return xs.map { (x: Int) -> Int in x + 1 }
}
"#);
    let lambda = il
        .nodes
        .iter()
        .position(|node| node.kind == NodeKind::Lambda)
        .map(|idx| NodeId(idx as u32))
        .expect("lambda");
    let params = il
        .children(lambda)
        .iter()
        .filter(|&&child| il.kind(child) == NodeKind::Param)
        .count();
    assert_eq!(params, 1);
}

#[test]
fn unparenthesized_comparison_conjunction_lowers_as_boolean_and() {
    let il = il(r#"
func ordered(_ x: Int, _ y: Int) -> Bool {
return x < y && x <= y
}
"#);
    assert!(il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::BinOp || node.payload != Payload::Op(Op::And) {
            return false;
        }
        let kids = il.children(NodeId(idx as u32));
        matches!(
            kids,
            [left, right]
                if il.kind(*left) == NodeKind::BinOp
                    && il.node(*left).payload == Payload::Op(Op::Lt)
                    && il.kind(*right) == NodeKind::BinOp
                    && il.node(*right).payload == Payload::Op(Op::Le)
        )
    }));
}

#[test]
fn dictionary_default_subscript_lowers_with_marker() {
    let (il, interner) = il_with_interner(
        r#"
func lookup(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int) -> Int {
return dict[key, default: fallback]
}
"#,
    );
    let marker = interner.intern("swift_subscript_default");
    assert!(il
        .nodes
        .iter()
        .any(|node| { node.kind == NodeKind::Seq && node.payload == Payload::Name(marker) }));
}

#[test]
fn parameter_type_annotation_records_domain() {
    let il = il(r#"
func lookup(_ dict: Dictionary<String, Int>, _ value: Any) -> Int {
return dict["red", default: 0]
}
"#);
    assert_eq!(
        il.evidence
            .iter()
            .filter(|record| record.kind == EvidenceKind::Domain(nose_il::DomainEvidence::Map))
            .count(),
        1,
        "only Dictionary parameters should record a Map domain"
    );
}

#[test]
fn property_type_annotation_records_binding_domain() {
    let il = il(r#"
func build(_ xs: [Int]) -> [Int] {
var out: [Int] = []
for x in xs {
    out.append(x)
}
return out
}
"#);
    assert!(il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding { local_hash, .. }
                if local_hash == stable_symbol_hash("out")
        ) && record.kind == EvidenceKind::Domain(nose_il::DomainEvidence::Collection)
    }));
}

#[test]
fn parenthesized_single_expression_does_not_become_tuple() {
    let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
return xs.map { x in (x + 1) * 2 }
}
"#);
    assert!(!il
        .nodes
        .iter()
        .any(|node| { node.kind == NodeKind::Seq && matches!(node.payload, Payload::Name(_)) }));
}

#[test]
fn implicit_member_shorthand_lowers_without_raw_prefix() {
    let (il, interner) = il_with_interner(
        r#"
func axis() -> Any {
return .vertical
}

func space() -> Any {
return .named("scroll")
}
"#,
    );
    assert!(
        !raw_names(&il, &interner)
            .iter()
            .any(|name| name == "prefix_expression"),
        "implicit member syntax should not stay as a generic Raw prefix"
    );
    let implicit = interner.intern("swift_implicit_member");
    assert!(il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Field {
            return false;
        }
        let children = il.children(NodeId(idx as u32));
        matches!(
            children,
            [receiver]
                if il.kind(*receiver) == NodeKind::Var
                    && il.node(*receiver).payload == Payload::Name(implicit)
        )
    }));
}

#[test]
fn protocol_requirements_lower_as_signature_units() {
    let (il, interner) = il_with_interner(
        r#"
protocol Store {
var count: Int { get }
func fetch(_ key: String) async throws -> Int
}
"#,
    );
    let raw = raw_names(&il, &interner);
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "protocol_function_declaration"
                | "protocol_property_declaration"
                | "protocol_property_requirements"
        )),
        "protocol requirements should lower as declaration/signature structure, got {raw:?}"
    );
    assert!(
        il.units.iter().any(
            |unit| unit.kind == UnitKind::Method && unit.name == Some(interner.intern("fetch"))
        ),
        "protocol function requirement should be a method-like unit"
    );
    assert!(il.nodes.iter().any(|node| node.kind == NodeKind::Param));
}

#[test]
fn async_function_preserves_source_backed_async_boundary() {
    let (il, interner) = il_with_interner(
        r#"
func fetch(_ key: String) async -> Int {
  return key.count
}
"#,
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_function",
        SourceProtocolKind::AsyncFunction,
    );
}

#[test]
fn async_for_preserves_source_backed_async_iteration_boundary() {
    let (il, interner) = il_with_interner(
        r#"
func read(_ stream: AsyncStream<Int>) async {
  for await value in stream {
    print(value)
  }
}
"#,
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
    assert!(
        !raw_names(&il, &interner).iter().any(|name| name == "try"),
        "plain for-await should not introduce a try-propagation boundary"
    );
}

#[test]
fn throwing_async_for_preserves_iteration_and_try_boundaries() {
    let (il, interner) = il_with_interner(
        r#"
func read(_ stream: AsyncThrowingStream<Int, Error>) async throws {
  for try await value in stream {
    print(value)
  }
}
"#,
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "try",
        SourceProtocolKind::TryPropagation,
    );
}

#[test]
fn async_for_keywords_do_not_match_identifier_prefixes() {
    let (il, interner) = il_with_interner(
        r#"
func read(_ awaiters: [Int], _ values: [Int]) {
  for awaiter in awaiters {
    print(awaiter)
  }
  for tryawait in values {
    print(tryawait)
  }
}
"#,
    );
    let raw = raw_names(&il, &interner);

    assert!(
        !raw.iter().any(|name| name == "async_for"),
        "ordinary identifiers prefixed with await/try-await should not create async iteration boundaries: {raw:?}"
    );
    assert!(
        !raw.iter().any(|name| name == "try"),
        "ordinary identifiers prefixed with try should not create try-propagation boundaries: {raw:?}"
    );
}

#[test]
fn multiline_throwing_async_for_anchors_try_to_keyword_line() {
    let (il, interner) = il_with_interner(
        "func read(_ stream: AsyncThrowingStream<Int, Error>) async throws {\n  for\n    try await value in stream {\n      print(value)\n    }\n}\n",
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
    let try_node = crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "try",
        SourceProtocolKind::TryPropagation,
    );
    assert_eq!(il.node(try_node).span.start_line, 3);
    assert_eq!(il.node(try_node).span.end_line, 3);
}

#[test]
fn extension_declaration_preserves_extended_type_name() {
    let (il, interner) = il_with_interner(
        r#"
extension Task {
  static func sleep(nanoseconds: Int) {}
}
"#,
    );
    assert!(
        il.units
            .iter()
            .any(|unit| unit.kind == UnitKind::Class && unit.name == Some(interner.intern("Task"))),
        "extension Task should preserve the extended type name as a visible unit"
    );
}
