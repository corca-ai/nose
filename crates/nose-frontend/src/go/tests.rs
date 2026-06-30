use super::*;

fn ops(src: &str) -> Vec<Op> {
    let interner = Interner::new();
    lower(FileId(0), "t.go", src.as_bytes(), &interner)
        .expect("lower")
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::BinOp | NodeKind::UnOp))
        .filter_map(|n| match n.payload {
            Payload::Op(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn switch_case_rhs_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::Eq))
        .filter_map(|(idx, _)| {
            let kids = il.children(NodeId(idx as u32));
            match kids {
                [_, rhs] => match il.node(*rhs).payload {
                    Payload::LitInt(value) => Some(value),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

fn raw_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn seq_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Seq)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn node_kind_count(src: &str, kind: NodeKind) -> usize {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes.iter().filter(|node| node.kind == kind).count()
}

fn call_callee_index_count(src: &str) -> usize {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::Call)
        .filter(|(idx, _)| {
            il.children(NodeId(*idx as u32))
                .first()
                .is_some_and(|callee| il.node(*callee).kind == NodeKind::Index)
        })
        .count()
}

fn call_callee_shapes(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::Call)
        .filter_map(|(idx, _)| il.children(NodeId(idx as u32)).first().copied())
        .map(|callee| node_shape(&il, callee))
        .collect()
}

fn node_shape(il: &Il, id: NodeId) -> String {
    let node = il.node(id);
    let children = il.children(id);
    match node.kind {
        NodeKind::Var => "Var".to_string(),
        NodeKind::Field => {
            let base = children
                .first()
                .map(|child| node_shape(il, *child))
                .unwrap_or_else(|| "Empty".to_string());
            format!("Field({base})")
        }
        NodeKind::Index => {
            let parts: Vec<String> = children
                .iter()
                .map(|child| node_shape(il, *child))
                .collect();
            format!("Index({})", parts.join(","))
        }
        NodeKind::Seq => "Seq".to_string(),
        other => format!("{other:?}"),
    }
}

fn lit_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter_map(|node| match node.payload {
            Payload::LitInt(value) => Some(value),
            _ => None,
        })
        .collect()
}

#[test]
fn switch_cases_compare_scrutinee_to_all_case_labels() {
    let src =
        "package main\nfunc f(x int) int { switch x { case 1, 2: return 3; default: return 4 } }\n";
    assert_eq!(switch_case_rhs_ints(src), vec![1, 2]);
}

#[test]
fn const_iota_lowers_to_concrete_spec_ordinals() {
    let src = "package main\nconst (\n  A = iota\n  B\n  C = 1 << iota\n  D = MyInt(iota)\n  E = wrap(iota)\n)\n";
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| name == "iota"),
        "const iota should lower to concrete integers, got {raw:?}"
    );
    let ints = lit_ints(src);
    assert!(
        ints.contains(&0)
            && ints.contains(&1)
            && ints.contains(&2)
            && ints.contains(&3)
            && ints.contains(&4),
        "iota ordinals should appear in lowered const values, got {ints:?}"
    );
    assert!(
        ops(src).contains(&Op::Shl),
        "iota inside const expressions should preserve surrounding operators"
    );
}

#[test]
fn type_switch_case_types_do_not_leak_into_case_bodies() {
    let raw = raw_names(
        "package main\nfunc f(v any) int { switch v.(type) { case *Thing: return 1; case Other: return 2; default: return 3 } }\n",
    );
    assert!(
        raw.iter().any(|name| name == "type_case *Thing")
            && raw.iter().any(|name| name == "type_case Other"),
        "type switch tests should remain explicit fail-closed conditions: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag("type_case *Thing"));
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "pointer_type" | "type_identifier" | "qualified_type" | "slice_type" | "map_type"
        )),
        "type switch case body should not include type-only Raw nodes: {raw:?}"
    );
}

#[test]
fn type_switch_multi_label_types_do_not_leak_into_case_bodies() {
    let raw = raw_names(
        "package main\nfunc f(v any) int { switch v.(type) { case *Thing, pkg.Other, []string, map[string]int, nil: return 1; default: return 0 } }\n",
    );
    for expected in [
        "type_case *Thing",
        "type_case pkg.Other",
        "type_case []string",
        "type_case map[string]int",
        "type_case nil",
    ] {
        assert!(
            raw.iter().any(|name| name == expected),
            "type-switch labels should stay as explicit fail-closed tests: {raw:?}"
        );
        assert!(crate::is_intentional_raw_boundary_tag(expected));
    }
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "pointer_type"
                | "type_identifier"
                | "qualified_type"
                | "slice_type"
                | "map_type"
                | "parenthesized_type"
        )),
        "all type-switch labels should be consumed as tests, not body Raw: {raw:?}"
    );
}

#[test]
fn ambiguous_type_instantiation_expression_preserves_index_reads_without_raw() {
    let src = "package main\ntype SymbolMap struct { SymbolsForSource [][]Symbol }\ntype Ref struct { SourceIndex int; InnerIndex int }\nfunc (sm SymbolMap) Get(ref Ref) *Symbol { return &sm.SymbolsForSource[ref.SourceIndex][ref.InnerIndex] }\n";
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "type_instantiation_expression" | "type_arguments" | "type_elem"
        )),
        "type-instantiation/parser-ambiguous index surfaces should not remain Raw: {raw:?}"
    );
    assert!(
        node_kind_count(src, NodeKind::Index) >= 2,
        "parser-ambiguous nested index expressions must preserve both index reads"
    );
}

#[test]
fn call_position_type_instantiation_preserves_ambiguous_index_callee() {
    for src in [
        "package main\nfunc f(x int) int { return pkg.Max[pkg.Type](x, x) }\n",
        "package main\nfunc f(xs []int, ys []int) []int { return Max[[]int](xs, ys) }\n",
    ] {
        assert_eq!(
            call_callee_index_count(src),
            1,
            "call-position type instantiation is ambiguous without type facts and should preserve the indexed callee: {:?}",
            call_callee_shapes(src)
        );
        assert!(
            !raw_names(src).iter().any(|name| matches!(
                name.as_str(),
                "type_instantiation_expression" | "type_arguments" | "type_elem"
            )),
            "type-instantiation wrappers should not remain Raw"
        );
    }
}

#[test]
fn indexed_function_call_callee_spelling_does_not_trigger_generic_erasure() {
    for src in [
        "package main\nfunc f(Max []func(int, int) int, local int, x int, y int) int { return Max[local](x, y) }\n",
        "package main\nfunc f(Max []func(int, int) int, I int, x int, y int) int { return Max[I](x, y) }\n",
        "package main\nfunc f(Max []func(int, int) int, int int, x int, y int) int { return Max[int](x, y) }\n",
        "package main\ntype P struct { Type int }\nfunc f(Max []func(int, int) int, pkg P, x int, y int) int { return Max[pkg.Type](x, y) }\n",
    ] {
        assert_eq!(
            call_callee_index_count(src),
            1,
            "value indexed function calls must preserve the indexed callee even when names resemble generic syntax: {:?}",
            call_callee_shapes(src)
        );
    }
}

#[test]
fn indexed_function_call_ambiguity_does_not_collapse_to_argument() {
    let src = "package main\nfunc f(fs []func(int) int, i int, x int) int { return fs[i](x) }\n";
    let seq = seq_names(src);
    let callee_shapes = call_callee_shapes(src);
    assert!(
        node_kind_count(src, NodeKind::Index) >= 1,
        "ambiguous indexed function call should preserve the indexed callee read"
    );
    assert!(
        seq.iter()
            .any(|name| name == "go_type_conversion_or_index_call")
            || callee_shapes.iter().any(|shape| shape.starts_with("Index(")),
        "ambiguous indexed function call should fail closed or lower as Call(Index(...), ...), got seq={seq:?} call_callees={callee_shapes:?}"
    );
}

#[test]
fn fallthrough_and_goto_preserve_fail_closed_control_flow_boundaries() {
    let raw = raw_names(
        "package main\nfunc f(x int) int { switch x { case 3: fallthrough; case 2: return 2 }; goto done; done: return 1 }\n",
    );
    assert!(
        raw.iter().any(|name| name == "fallthrough_statement"),
        "fallthrough should remain an explicit CFG boundary: {raw:?}"
    );
    assert!(
        raw.iter().any(|name| name == "go_goto done"),
        "goto should preserve its target label spelling: {raw:?}"
    );
    assert!(
        raw.iter().any(|name| name == "go_label done"),
        "label definitions should preserve their spelling: {raw:?}"
    );
    assert!(
        !raw.iter()
            .any(|name| name == "label_name" || name == "goto_statement"),
        "goto lowering should not leak parser-only label nodes: {raw:?}"
    );
    assert!(crate::is_intentional_raw_boundary_tag(
        "fallthrough_statement"
    ));
    assert!(crate::is_intentional_raw_boundary_tag("go_goto done"));
    assert!(crate::is_intentional_raw_boundary_tag("go_label done"));
}

#[test]
fn block_switch_select_bodies_survive_statement_list_wrapper() {
    // go 0.25 wraps block / switch-case / select-case statements in a single
    // `statement_list` node (go 0.23 listed them directly). `stmt_children` must
    // flatten that wrapper, else every nested statement is orphaned to Raw — the
    // go-0.25 Raw-ratio blowup (0.7% → 29%) this migration fixes. Each op below
    // lives inside one of the three wrapper sites.
    let block = ops("package main\nfunc f(a int, b int) int { c := a + b; return c }\n");
    assert!(
        block.contains(&Op::Add),
        "block-body op orphaned, got {block:?}"
    );

    let sw = ops(
        "package main\nfunc f(a int, b int, x int) int { switch x { case 1: return a - b; default: return a * b } }\n",
    );
    assert!(
        sw.contains(&Op::Sub) && sw.contains(&Op::Mul),
        "switch-case-body ops orphaned, got {sw:?}"
    );

    let sel = ops(
        "package main\nfunc f(ch chan int, a int, b int) int { select { case <-ch: return a / b; default: return a + b } }\n",
    );
    assert!(
        sel.contains(&Op::Div) && sel.contains(&Op::Add),
        "select-case-body ops orphaned, got {sel:?}"
    );
}

#[test]
fn bit_clear_is_not_plain_bitand() {
    // Go's `a &^ b` is AND-NOT (`a & ^b`): it must desugar to a `BitAnd` over a
    // `BitNot` of the right operand, NOT collapse to a plain `a & b` (different bits,
    // and a false merge with real `&`).
    let clear = ops("package main\nfunc f(a int, b int) int { return a &^ b }\n");
    assert!(
        clear.contains(&Op::BitNot),
        "`a &^ b` must introduce BitNot, got {clear:?}"
    );
    assert!(
        clear.contains(&Op::BitAnd),
        "`a &^ b` must keep BitAnd, got {clear:?}"
    );

    // Plain `a & b` must NOT introduce a BitNot — the two operators stay distinct.
    let and = ops("package main\nfunc f(a int, b int) int { return a & b }\n");
    assert!(
        !and.contains(&Op::BitNot),
        "`a & b` must not introduce BitNot, got {and:?}"
    );
}

#[test]
fn go_defer_and_channel_operations_preserve_source_backed_protocol_boundaries() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.go",
        b"package main\nfunc f(ch chan int, x int) int { go record(x); defer record(x); ch <- x; return <-ch }\n",
        &interner,
    )
    .expect("lower");

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "go",
        SourceProtocolKind::GoRoutine,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "defer",
        SourceProtocolKind::Defer,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_send",
        SourceProtocolKind::ChannelSend,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_receive",
        SourceProtocolKind::ChannelReceive,
    );
}

#[test]
fn select_statement_preserves_source_backed_protocol_boundary() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.go",
        b"package main\nfunc f(ch chan int) { select { case v := <-ch: _ = v; default: return } }\n",
        &interner,
    )
    .expect("lower");

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "select",
        SourceProtocolKind::ChannelSelect,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "select_case",
        SourceProtocolKind::ChannelSelectCase,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "select_default",
        SourceProtocolKind::ChannelSelectDefault,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_receive",
        SourceProtocolKind::ChannelReceive,
    );
}

#[test]
fn select_comma_ok_receive_preserves_status_protocol_boundary() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.go",
        b"package main\nfunc f(ch chan int) bool { select { case _, ok := <-ch: return ok; default: return false } }\n",
        &interner,
    )
    .expect("lower");

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "select",
        SourceProtocolKind::ChannelSelect,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "select_case",
        SourceProtocolKind::ChannelSelectCase,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_receive_status",
        SourceProtocolKind::ChannelReceive,
    );
}

#[test]
fn comma_ok_receive_preserves_value_and_status_protocol_boundaries() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.go",
        b"package main\nfunc f(ch chan int) bool { v, ok := <-ch; _ = v; return ok }\n",
        &interner,
    )
    .expect("lower");

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_receive",
        SourceProtocolKind::ChannelReceive,
    );
    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "channel_receive_status",
        SourceProtocolKind::ChannelReceive,
    );
}
