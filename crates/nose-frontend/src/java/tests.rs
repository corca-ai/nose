use super::*;

fn raw_kinds(src: &str) -> Vec<String> {
    let interner = Interner::new();
    lower(FileId(0), "T.java", src.as_bytes(), &interner)
        .expect("lower")
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Raw)
        .filter_map(|n| match n.payload {
            Payload::Name(s) => Some(interner.resolve(s).to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn local_record_and_annotation_declarations_do_not_fall_to_raw() {
    // Local type declarations are type metadata in this IL. They should follow the
    // same class-like lowering path as top-level declarations instead of surfacing
    // as opaque statement Raw nodes.
    let raw = raw_kinds(
        "class C { void f(){ record Pair(int a, int b) {} @interface Local { String value(); } } }",
    );
    assert!(
        raw.is_empty(),
        "local type declarations should be erased/lowered, got {raw:?}"
    );
}

#[test]
fn declaration_and_statement_surfaces_do_not_fall_to_raw() {
    let src = r#"
interface I {
  int LIMIT = 10;
  void run(String value);
}
enum Color {
  RED(1), BLUE(2);
  Color(int code) {}
}
class C {
  static { init(); }
  void f(Object lock, int x) {
    assert x > 0;
    synchronized (lock) {
      x++;
    }
  }
}
"#;
    let raw = raw_names(src);
    for name in [
        "assert_statement",
        "synchronized_statement",
        "constant_declaration",
        "method_declaration",
        "formal_parameters",
        "formal_parameter",
        "enum_body_declarations",
        "static_initializer",
        "constructor_declaration",
        "constructor_body",
    ] {
        assert!(
            !raw.iter().any(|raw_name| raw_name == name),
            "{name} should not lower to Raw: {raw:?}"
        );
    }
    let seq = seq_names(src);
    assert!(
        seq.iter().any(|name| name == "java_enum_constant"),
        "enum constant constructor arguments should be preserved exactly: {seq:?}"
    );
}

#[test]
fn type_surfaces_and_unsigned_shift_do_not_fall_to_raw() {
    let src = r#"
class C {
  Class<?> kind() { return java.util.Map.Entry.class; }
  int shift(int value, int bits) {
    value >>>= 1;
    return value >>> bits;
  }
}
"#;
    let raw = raw_names(src);
    for name in [
        "scoped_type_identifier",
        "binary_expression >>>",
        "compound_assignment >>>=",
    ] {
        assert!(
            !raw.iter().any(|raw_name| raw_name == name),
            "{name} should not lower to Raw: {raw:?}"
        );
    }
    let seq = seq_names(src);
    assert!(
        seq.iter()
            .filter(|name| name.as_str() == "java_unsigned_shift_right")
            .count()
            >= 2,
        "both >>> and >>>= should preserve unsigned-shift semantics: {seq:?}"
    );
}

#[test]
fn module_metadata_surfaces_do_not_fall_to_raw() {
    let src = r#"
module com.example.app {
  requires transitive java.sql;
  exports com.example.api;
  opens com.example.internal;
  uses com.example.Service;
  provides com.example.Service with com.example.impl.ServiceImpl;
}
"#;
    let raw = raw_names(src);
    for name in [
        "module_declaration",
        "module_body",
        "requires_module_directive",
        "requires_modifier",
        "exports_module_directive",
        "opens_module_directive",
        "uses_module_directive",
        "provides_module_directive",
    ] {
        assert!(
            !raw.iter().any(|raw_name| raw_name == name),
            "{name} should not lower to Raw: {raw:?}"
        );
    }
    let seq = seq_names(src);
    for name in [
        "java_module_declaration",
        "java_module_body",
        "java_requires_module_directive",
        "java_exports_module_directive",
        "java_opens_module_directive",
        "java_uses_module_directive",
        "java_provides_module_directive",
    ] {
        assert!(
            seq.iter().any(|seq_name| seq_name == name),
            "{name} should preserve module descriptor metadata: {seq:?}"
        );
    }
}

#[test]
fn labeled_statements_and_compact_constructors_do_not_fall_to_raw() {
    let src = r#"
record R(int x) {
  R {
    if (x < 0) throw new IllegalArgumentException();
  }
}
class C {
  void f(int limit) {
    outer: for (int i = 0; i < limit; i++) {
      if (i > 3) break outer;
      if (i == 2) continue outer;
    }
  }
}
"#;
    let raw = raw_names(src);
    for name in ["labeled_statement", "compact_constructor_declaration"] {
        assert!(
            !raw.iter().any(|raw_name| raw_name == name),
            "{name} should not lower to Raw: {raw:?}"
        );
    }
    let seq = seq_names(src);
    for name in [
        "java_labeled_statement",
        "java_labeled_break",
        "java_labeled_continue",
    ] {
        assert!(
            seq.iter().any(|seq_name| seq_name == name),
            "{name} should preserve labeled control-flow semantics: {seq:?}"
        );
    }
}

fn unary_ops(src: &str) -> Vec<Op> {
    let interner = Interner::new();
    lower(FileId(0), "T.java", src.as_bytes(), &interner)
        .expect("lower")
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::UnOp)
        .filter_map(|n| match n.payload {
            Payload::Op(op) => Some(op),
            _ => None,
        })
        .collect()
}

#[test]
fn unary_operators_lower_to_distinct_ops() {
    // `+x` must be Pos and `~x` BitNot, not both collapsed onto Neg.
    let ops = unary_ops(
        "class C { int f(int x){ return +x + -x + ~x; } boolean g(boolean b){ return !b; } }",
    );
    assert!(ops.contains(&Op::Pos), "unary + → Op::Pos, got {ops:?}");
    assert!(ops.contains(&Op::Neg), "unary - → Op::Neg, got {ops:?}");
    assert!(
        ops.contains(&Op::BitNot),
        "unary ~ → Op::BitNot, got {ops:?}"
    );
    assert!(ops.contains(&Op::Not), "unary ! → Op::Not, got {ops:?}");
}

fn binops(src: &str) -> Vec<Op> {
    let interner = Interner::new();
    lower(FileId(0), "T.java", src.as_bytes(), &interner)
        .expect("lower")
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::BinOp)
        .filter_map(|n| match n.payload {
            Payload::Op(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn switch_case_rhs_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::BinOp && n.payload == Payload::Op(Op::Eq))
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
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    crate::test_helpers::payload_names_for_kind(&il, &interner, NodeKind::Raw)
}

fn seq_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    crate::test_helpers::payload_names_for_kind(&il, &interner, NodeKind::Seq)
}

fn call_callee_paths(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::Call)
        .filter_map(|(idx, _)| {
            il.children(NodeId(idx as u32))
                .first()
                .and_then(|callee| callee_path_for_test(&il, &interner, *callee))
        })
        .collect()
}

fn callee_path_for_test(il: &Il, interner: &Interner, node: NodeId) -> Option<String> {
    match il.node(node).kind {
        NodeKind::Var => match il.node(node).payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        },
        NodeKind::Field => {
            let Payload::Name(field) = il.node(node).payload else {
                return None;
            };
            let receiver = il.children(node).first().copied()?;
            Some(format!(
                "{}.{}",
                callee_path_for_test(il, interner, receiver)?,
                interner.resolve(field)
            ))
        }
        _ => None,
    }
}

#[test]
fn completable_future_constructor_callee_requires_stdlib_type_identity() {
    let exact = call_callee_paths(
        "import java.util.concurrent.CompletableFuture;\nclass T { Object run() { return new CompletableFuture<String>(); } }\n",
    );
    assert!(
        exact.iter().any(|path| path == "CompletableFuture"),
        "exact import should preserve constructor callee: {exact:?}"
    );

    let wildcard = call_callee_paths(
        "import java.util.concurrent.*;\nclass T { Object run() { return new CompletableFuture<String>(); } }\n",
    );
    assert!(
        wildcard.iter().any(|path| path == "CompletableFuture"),
        "wildcard import should preserve constructor callee: {wildcard:?}"
    );

    let qualified = call_callee_paths(
        "class T { Object run() { return new java.util.concurrent.CompletableFuture<String>(); } }\n",
    );
    assert!(
        qualified
            .iter()
            .any(|path| path == "java.util.concurrent.CompletableFuture"),
        "qualified constructor should preserve constructor callee: {qualified:?}"
    );

    for (surface, src) in [
        (
            "unimported CompletableFuture",
            "class T { Object run() { return new CompletableFuture<String>(); } }\n",
        ),
        (
            "conflicting CompletableFuture import",
            "import java.util.concurrent.*;\nimport example.CompletableFuture;\nclass T { Object run() { return new CompletableFuture<String>(); } }\n",
        ),
        (
            "local CompletableFuture type",
            "import java.util.concurrent.CompletableFuture;\nclass CompletableFuture<T> {}\nclass T { Object run() { return new CompletableFuture<String>(); } }\n",
        ),
    ] {
        let paths = call_callee_paths(src);
        assert!(
            !paths.iter().any(|path| path.ends_with("CompletableFuture")),
            "{surface} should not preserve a stdlib constructor callee: {paths:?}"
        );
    }
}

fn switch_expression_branch_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::If)
        .find_map(|(idx, _)| {
            let kids = il.children(NodeId(idx as u32));
            match kids {
                [_, then_expr, else_expr] => {
                    match (il.node(*then_expr).payload, il.node(*else_expr).payload) {
                        (Payload::LitInt(then_value), Payload::LitInt(else_value)) => {
                            Some(vec![then_value, else_value])
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        })
        .unwrap_or_default()
}

fn switch_case_lhs_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::Eq))
        .filter_map(|(idx, _)| {
            let kids = il.children(NodeId(idx as u32));
            match kids {
                [lhs, _] => match il.node(*lhs).payload {
                    Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

fn expr_stmt_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::ExprStmt)
        .filter_map(|(idx, _)| {
            let kids = il.children(NodeId(idx as u32));
            match kids {
                [expr] => match il.node(*expr).payload {
                    Payload::LitInt(value) => Some(value),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

#[test]
fn switch_cases_compare_scrutinee_to_case_literals() {
    let src = "class C { int f(int x){ switch(x){ case 7: return 1; case 8: return 2; default: return 3; } } }";
    assert_eq!(switch_case_rhs_ints(src), vec![7, 8]);
    assert!(
        expr_stmt_ints(src).is_empty(),
        "case labels should not remain as stray expression statements"
    );
}

#[test]
fn switch_expression_rules_lower_to_expression_if_chain() {
    let src = "class C { int f(int x){ return switch (x) { case 1 -> 2; default -> 3; }; } }";
    assert_eq!(switch_case_rhs_ints(src), vec![1]);
    assert_eq!(switch_case_lhs_names(src), vec!["x"]);
    assert_eq!(switch_expression_branch_ints(src), vec![2, 3]);
    let raw = raw_names(src);
    assert!(
        !raw.iter()
            .any(|name| matches!(name.as_str(), "switch_expression" | "switch_rule")),
        "switch expression rules should lower without Raw nodes: {raw:?}"
    );
}

#[test]
fn switch_expression_yield_blocks_lower_to_branch_values() {
    let src = "class C { int f(int x){ return switch (x) { case 1 -> { yield 2; } default -> { yield 3; } }; } }";
    assert_eq!(switch_case_rhs_ints(src), vec![1]);
    assert_eq!(switch_expression_branch_ints(src), vec![2, 3]);
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| name == "yield_statement"),
        "switch expression yield blocks should lower without Raw yield_statement: {raw:?}"
    );
}

#[test]
fn postfix_increment_with_nested_decrement_in_operand() {
    // `a[i--]++` desugars with the OUTER op being increment (`+ 1`); a substring
    // `--` check misread the nested `i--` and flipped it to decrement.
    let ops = binops("class C { void f(){ int[] a = new int[10]; int i = 0; a[i--]++; } }");
    assert!(
        ops.contains(&Op::Add),
        "outer `++` must lower to Op::Add despite the nested `i--`, got {ops:?}"
    );
}
