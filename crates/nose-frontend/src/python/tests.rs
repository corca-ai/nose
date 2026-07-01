use super::*;
use nose_il::{SourceComprehensionKind, SourceProtocolKind};
use nose_semantics::source_comprehension_at_node;

fn raw_names(src: &[u8]) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.py", src, &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn seq_names(src: &[u8]) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.py", src, &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Seq)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn expect_python_protocol_boundary(src: &[u8], tag: &str, protocol: SourceProtocolKind) {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.py", src, &interner).expect("lower");
    crate::test_helpers::expect_raw_protocol_boundary(&il, &interner, tag, protocol);
}

#[test]
fn explicit_line_continuations_do_not_become_raw() {
    let raw = raw_names(
        b"def f(classes, i, expected):\n    assert \\\n        classes == expected\n    assert classes == \\\n        expected\n    return \\\n        tuple(c for c in classes[:i] if \\\n        c.__name__ == classes[0].__name__)\n\ndef g(error):\n    raise \\\n        error\n",
    );
    assert!(
        !raw.iter().any(|name| name == "line_continuation"),
        "line continuations are lexical noise and should not lower to Raw: {raw:?}"
    );
}

#[test]
fn comments_inside_parenthesized_expressions_are_lexical_noise() {
    let raw = raw_names(
        b"def f(x):\n    return (\n        x\n        # preserve formatting comment\n    )\n",
    );
    assert!(
        !raw.iter().any(|name| name == "comment"),
        "comments are lexical noise and should not lower to Raw: {raw:?}"
    );
}

#[test]
fn dictionary_unpack_lowers_to_fail_closed_surface_without_raw() {
    let src = b"def f(base, override):\n    return {**base, 'x': 1, **override}\n";
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| name == "dictionary_splat"),
        "dict unpack should not remain a lowering-gap Raw node: {raw:?}"
    );
    let seq = seq_names(src);
    assert_eq!(
        seq.iter()
            .filter(|name| name.as_str() == "python_dictionary_splat")
            .count(),
        2,
        "dict unpack should be preserved as exact-closed Python surfaces: {seq:?}"
    );
}

#[test]
fn match_guard_line_continuations_do_not_become_raw() {
    let raw = raw_names(
        b"def f(value):\n    match value:\n        case y if \\\n            y > 0:\n            return y\n        case _:\n            return 0\n",
    );
    assert!(
        !raw.iter().any(|name| name == "line_continuation"),
        "line continuations in match guards are lexical noise: {raw:?}"
    );
}

#[test]
fn dynamic_module_rebind_via_globals_and_setattr_marks_the_named_function() {
    // #307: `globals()['helper'] = …` and `setattr(<module>, 'other', …)` reassign a
    // module function with NO `global` declaration to key off. The named function's
    // runtime binding is no longer its `def`, so it must be excluded from inlining /
    // content-keying (else callers false-merge across files reassigning it differently).
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def helper(x):\n    return x + 1\ndef other(x):\n    return x * 100\nglobals()['helper'] = other\nsetattr(m, 'other', helper)\n",
        &interner,
    )
    .expect("lower");
    let rebound = nose_semantics::module_rebound_symbols(&il, &interner);
    let names: std::collections::HashSet<&str> =
        rebound.iter().map(|&s| interner.resolve(s)).collect();
    assert!(
        names.contains("helper"),
        "globals()['helper'] = … must mark helper as rebound: {names:?}"
    );
    assert!(
        names.contains("other"),
        "setattr(m, 'other', …) must mark other as rebound: {names:?}"
    );
}

#[test]
fn literal_match_lowers_without_raw_case_nodes() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case 0:\n            return 1\n        case _:\n            return x\n",
        &interner,
    )
    .expect("lower");

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(raw.is_empty(), "match should lower without Raw: {raw:?}");
    assert!(
        il.nodes.iter().any(|node| node.kind == NodeKind::If),
        "match should lower to an if-chain"
    );
}

#[test]
fn comprehension_surfaces_emit_source_evidence() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def a(xs):\n    return [x for x in xs]\ndef b(xs):\n    return {x for x in xs}\ndef c(xs):\n    return {x: x for x in xs}\ndef d(xs):\n    return (x for x in xs)\n",
        &interner,
    )
    .expect("lower");

    for kind in [
        SourceComprehensionKind::PythonListComprehension,
        SourceComprehensionKind::PythonSetComprehension,
        SourceComprehensionKind::PythonDictComprehension,
        SourceComprehensionKind::PythonGeneratorExpression,
    ] {
        let count = il
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::HoF)
            .filter(|(idx, _)| source_comprehension_at_node(&il, NodeId(*idx as u32)) == Some(kind))
            .count();
        assert_eq!(count, 1, "{kind:?} should have one source-backed HoF");
    }
}

#[test]
fn literal_or_match_lowers_to_or_condition_without_raw() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case 0 | 1:\n            return 1\n        case _:\n            return x\n",
        &interner,
    )
    .expect("lower");

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        raw.is_empty(),
        "or-pattern match should lower without Raw: {raw:?}"
    );
    assert!(
        il.nodes
            .iter()
            .any(|node| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::Or)),
        "or-pattern match should lower to an OR condition"
    );
}

#[test]
fn await_expression_preserves_source_backed_async_boundary() {
    expect_python_protocol_boundary(
        b"async def f(x):\n    return await x + 1\n",
        "await",
        SourceProtocolKind::Await,
    );
}

#[test]
fn async_function_preserves_source_backed_async_boundary() {
    expect_python_protocol_boundary(
        b"async def f(x):\n    return x + 1\n",
        "async_function",
        SourceProtocolKind::AsyncFunction,
    );
}

#[test]
fn async_for_preserves_source_backed_iteration_boundary() {
    expect_python_protocol_boundary(
        b"async def f(xs):\n    async for x in xs:\n        yield x\n",
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
}

#[test]
fn async_comprehension_preserves_source_backed_iteration_boundary() {
    expect_python_protocol_boundary(
        b"async def f(xs):\n    return [x async for x in xs]\n",
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
}

#[test]
fn multi_clause_async_comprehension_preserves_source_backed_iteration_boundary() {
    expect_python_protocol_boundary(
        b"async def f(xs, ys):\n    return [(x, y) async for x in xs for y in ys]\n",
        "async_for",
        SourceProtocolKind::AsyncIteration,
    );
}

#[test]
fn async_with_preserves_source_backed_context_boundary() {
    expect_python_protocol_boundary(
        b"async def f(cm):\n    async with cm:\n        return 1\n",
        "async_with",
        SourceProtocolKind::AsyncContext,
    );
}

#[test]
fn yield_expression_preserves_source_backed_protocol_boundary() {
    expect_python_protocol_boundary(
        b"def f(x):\n    yield x + 1\n",
        "yield",
        SourceProtocolKind::Yield,
    );
}

#[test]
fn guarded_match_lowers_guard_into_condition() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x, ok):\n    match x:\n        case 1 if ok:\n            return 1\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    assert!(
        il.nodes
            .iter()
            .any(|node| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::And)),
        "match guard should combine with the pattern condition"
    );
}

#[test]
fn capture_match_pattern_is_unconditional() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case y:\n            return 1\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    assert!(
        !il.nodes.iter().any(|node| node.kind == NodeKind::If),
        "a capture pattern should not lower to a scrutinee comparison"
    );
}

#[test]
fn qualified_match_pattern_lowers_without_raw_dotted_name() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case Color.RED:\n            return 1\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        !raw.contains(&"dotted_name"),
        "qualified match pattern should lower without Raw dotted_name: {raw:?}"
    );
}

#[test]
fn sequence_match_pattern_lowers_without_raw_case_pattern() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case [1, 2]:\n            return 1\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        !raw.contains(&"case_pattern"),
        "sequence match pattern should lower without Raw case_pattern: {raw:?}"
    );
}

#[test]
fn as_match_pattern_lowers_inner_value_without_raw() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x):\n    match x:\n        case 1 as y:\n            return y\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        !raw.contains(&"as_pattern"),
        "as-pattern match should lower without Raw as_pattern: {raw:?}"
    );
    assert!(
        il.nodes
            .iter()
            .any(|node| node.payload == Payload::LitInt(1)),
        "as-pattern should preserve its inner value pattern"
    );
}

#[test]
fn multi_clause_comprehension_binds_all_iterables() {
    // `[a + b for a in A for b in B]` is sugar for nested iteration: every
    // clause and target must survive lowering. Dropping the second clause
    // leaves `b` unbound and makes this comprehension collide with the
    // single-clause `[a + b for a in A]` (a false merge).
    // `xs`/`ys` are free module-level iterables (not params) so a reference
    // to `ys` can only come from the second clause actually being lowered.
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f():\n    return [a + b for a in xs for b in ys]\n",
        &interner,
    )
    .expect("lower");

    let names: Vec<_> = il
        .nodes
        .iter()
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"ys"),
        "second clause `for b in ys` was dropped; names = {names:?}"
    );
}

#[test]
fn multi_clause_comprehension_lowers_to_flat_map_without_raw() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(A, B):\n    return [a + b for a in A for b in B]\n",
        &interner,
    )
    .expect("lower");

    let hof_kinds: Vec<_> = il
        .nodes
        .iter()
        .filter_map(|node| match node.payload {
            Payload::HoF(kind) => Some(kind),
            _ => None,
        })
        .collect();
    assert!(
        hof_kinds.contains(&HoFKind::FlatMap),
        "multi-clause comprehension should contain a FlatMap HoF: {hof_kinds:?}"
    );
    assert!(
        hof_kinds.contains(&HoFKind::Map),
        "multi-clause comprehension should keep the innermost Map: {hof_kinds:?}"
    );

    let raw: Vec<_> = il
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym)),
            _ => None,
        })
        .collect();
    assert!(
        !raw.contains(&"comprehension"),
        "flat-map comprehension should no longer use Raw fallback: {raw:?}"
    );
}

#[test]
fn multi_clause_comprehension_differs_from_single_clause() {
    // The two-clause comprehension must not lower identically to the
    // one-clause version — otherwise the value graph false-merges them.
    let interner = Interner::new();
    let two = lower(
        FileId(0),
        "t.py",
        b"def f(A, B):\n    return [a + b for a in A for b in B]\n",
        &interner,
    )
    .expect("lower");
    let one = lower(
        FileId(0),
        "t.py",
        b"def f(A, B):\n    return [a + b for a in A]\n",
        &interner,
    )
    .expect("lower");
    assert_ne!(
        two.nodes.len(),
        one.nodes.len(),
        "two-clause and one-clause comprehensions lowered to the same shape"
    );
}

#[test]
fn wildcard_guard_match_is_not_unconditional() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.py",
        b"def f(x, ok):\n    match x:\n        case _ if ok:\n            return 1\n        case _:\n            return 0\n",
        &interner,
    )
    .expect("lower");

    assert!(
        il.nodes.iter().any(|node| node.kind == NodeKind::If),
        "a guarded wildcard case should still lower to a conditional branch"
    );
}
