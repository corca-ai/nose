use super::*;

#[test]
fn closure_header_async_identifier_is_not_async_modifier() {
    assert_eq!(
        lambda_parameter_names_from_text("{ async in async + 1 }"),
        vec!["async".to_string()]
    );
    assert!(
        !swift_lambda_is_async("{ async in async + 1 }"),
        "a bare contextual `async` before `in` is a parameter name, not an async closure modifier"
    );
    assert_eq!(
        lambda_parameter_names_from_text("{ req async throws -> String in req.value }"),
        vec!["req".to_string()]
    );
    assert!(
        swift_lambda_is_async("{ req async throws -> String in req.value }"),
        "`async` after a closure parameter is the async closure modifier"
    );
    assert_eq!(
        lambda_parameter_names_from_text(
            "{ (closure: @escaping () async throws -> Void) in closure }"
        ),
        vec!["closure".to_string()]
    );
    assert!(
        !swift_lambda_is_async("{ (closure: @escaping () async throws -> Void) in closure }"),
        "`async` inside a function-typed closure parameter is not the closure modifier"
    );
}

#[test]
fn function_typed_async_parameter_does_not_create_async_closure_boundary() {
    let (il, interner) = il_with_interner(
        r#"
func install() {
  let handle: (@escaping () async throws -> Void) -> () async -> Void = { (closure: @escaping () async throws -> Void) in
    {
      try await closure()
    }
  }
}
"#,
    );
    let raw = raw_names(&il, &interner);

    assert!(
        !raw.iter().any(|name| name == "async_function"),
        "function-typed async parameters should not make the outer closure an async-function boundary: {raw:?}"
    );
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
fn async_closure_preserves_source_backed_async_boundary() {
    let (il, interner) = il_with_interner(
        r#"
func install(_ route: Route) {
  route.get("x") { req async throws -> String in
    return try await req.load()
  }
}
"#,
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_function",
        SourceProtocolKind::AsyncFunction,
    );
    let async_sym = interner.intern("async");
    assert!(
        !il.nodes.iter().any(|node| {
            node.kind == NodeKind::Param && node.payload == Payload::Name(async_sym)
        }),
        "async closure keyword should not lower as a lambda parameter"
    );
}

#[test]
fn no_argument_async_closure_does_not_create_async_parameter() {
    let (il, interner) = il_with_interner(
        r#"
func make() {
  let work: () async -> Int = { () async in 1 }
}
"#,
    );

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "async_function",
        SourceProtocolKind::AsyncFunction,
    );
    let async_sym = interner.intern("async");
    assert!(
        !il.nodes.iter().any(|node| {
            node.kind == NodeKind::Param && node.payload == Payload::Name(async_sym)
        }),
        "no-argument async closure should not invent an `async` parameter"
    );
}

#[test]
fn async_let_preserves_source_backed_task_spawn_boundary() {
    let (il, interner) = il_with_interner(
        r#"
func fetch() async throws -> Int {
  async let first: Int = try await compute()
  return try await first
}
"#,
    );

    let task_spawn = crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "task_spawn",
        SourceProtocolKind::TaskSpawn,
    );
    assert!(
        il.children(task_spawn)
            .iter()
            .any(|&child| il.kind(child) == NodeKind::Assign),
        "async let task-spawn boundary should retain the binding assignment"
    );
}

#[test]
fn async_let_keyword_does_not_match_identifier_prefixes() {
    let (il, interner) = il_with_interner(
        r#"
func fetch() {
  let asynclet = compute()
  let asynchronous = compute()
}
"#,
    );
    let raw = raw_names(&il, &interner);

    assert!(
        !raw.iter().any(|name| name == "task_spawn"),
        "ordinary identifiers prefixed with async should not create task-spawn boundaries: {raw:?}"
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
