use super::*;

fn nodes(src: &str) -> Vec<nose_il::Node> {
    let interner = Interner::new();
    lower(FileId(0), "t.rb", src.as_bytes(), &interner)
        .expect("lower")
        .nodes
}

fn unit_names(src: &str) -> Vec<(UnitKind, String)> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.units
        .iter()
        .map(|unit| {
            (
                unit.kind,
                unit.name
                    .map(|name| interner.resolve(name).to_string())
                    .unwrap_or_else(|| "-".to_string()),
            )
        })
        .collect()
}

fn unary_ops(src: &str) -> Vec<Op> {
    nodes(src)
        .iter()
        .filter(|n| n.kind == NodeKind::UnOp)
        .filter_map(|n| match n.payload {
            Payload::Op(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn binary_ops(src: &str) -> Vec<Op> {
    nodes(src)
        .iter()
        .filter(|n| n.kind == NodeKind::BinOp)
        .filter_map(|n| match n.payload {
            Payload::Op(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn raw_name_set(src: &str) -> Vec<String> {
    let mut raw = raw_names(src);
    raw.sort();
    raw.dedup();
    raw
}

fn raw_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Raw)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn node_kinds(src: &str) -> Vec<NodeKind> {
    nodes(src).into_iter().map(|node| node.kind).collect()
}

fn field_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Field)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn has_post_test_loop_block(src: &str) -> bool {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Block {
            return false;
        }
        let children = il.children(NodeId(idx as u32));
        children.len() >= 2
            && children
                .last()
                .is_some_and(|last| il.node(*last).kind == NodeKind::Loop)
    })
}

fn func_body_kinds(src: &str) -> Vec<NodeKind> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::Func)
        .filter_map(|(idx, _)| il.children(NodeId(idx as u32)).last().copied())
        .map(|body| il.node(body).kind)
        .collect()
}

fn var_names(src: &str) -> Vec<String> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Var)
        .filter_map(|node| match node.payload {
            Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
            _ => None,
        })
        .collect()
}

fn raw_names_without_errors(src: &str) -> Vec<String> {
    raw_names(src)
        .into_iter()
        .filter(|name| name != "ERROR")
        .collect()
}

#[test]
fn yield_preserves_source_backed_protocol_boundary() {
    let interner = Interner::new();
    let il = lower(
        FileId(0),
        "t.rb",
        b"def render(value)\n  yield value, value + 1\nend\n",
        &interner,
    )
    .expect("lower");

    crate::test_helpers::expect_raw_protocol_boundary(
        &il,
        &interner,
        "yield",
        SourceProtocolKind::BlockYield,
    );
}

fn expr_stmt_ints(src: &str) -> Vec<i64> {
    let interner = Interner::new();
    let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
    il.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::ExprStmt)
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
fn method_body_rescue_and_ensure_lower_as_try_without_clause_raw() {
    let src = "def f\n  work\nrescue Error => e then recover(e)\nensure\n  cleanup\nend\n";
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "rescue" | "ensure" | "then" | "exceptions" | "exception_variable"
        )),
        "method-level exception clauses should lower without clause Raw nodes: {raw:?}"
    );
    assert!(
        node_kinds(src).contains(&NodeKind::Try),
        "method-level exception clauses should lower to Try"
    );
    assert_eq!(
        func_body_kinds(src),
        vec![NodeKind::Block],
        "method body should keep the Func(..., Block) shape"
    );
    let vars = var_names(src);
    assert!(
        vars.iter().any(|name| name == "recover") && vars.iter().any(|name| name == "cleanup"),
        "rescue/ensure handler bodies should be preserved: {vars:?}"
    );
}

#[test]
fn block_body_rescue_else_and_ensure_lower_as_try_without_clause_raw() {
    let src = "it 'defaults' do\n  res = options.instrumenter\nrescue NameError => e then recover(e)\nelse\n  verify(res)\nensure\n  cleanup\nend\n";
    let raw = raw_names(src);
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "rescue" | "ensure" | "then" | "else" | "exceptions" | "exception_variable"
        )),
        "block-level exception clauses should lower without clause Raw nodes: {raw:?}"
    );
    assert!(
        node_kinds(src).contains(&NodeKind::Try),
        "block-level exception clauses should lower to Try"
    );
    let vars = var_names(src);
    assert!(
        vars.iter().any(|name| name == "recover")
            && vars.iter().any(|name| name == "verify")
            && vars.iter().any(|name| name == "cleanup"),
        "rescue, else, and ensure block bodies should be preserved: {vars:?}"
    );
}

#[test]
fn ruby_value_surfaces_do_not_fall_to_raw() {
    let raw = raw_names_without_errors(
        "def f\n  @@count = ?x\n  path = %x(echo hi).chop rescue ''\n  tick while ready?\n  spin until done?\nend\n",
    );
    assert!(
        raw.is_empty(),
        "class variables, character literals, subshells, rescue modifiers, and loop modifiers should lower without Raw: {raw:?}"
    );
    let kinds = node_kinds(
        "def f\n  @@count = ?x\n  path = %x(echo hi).chop rescue ''\n  tick while ready?\n  spin until done?\nend\n",
    );
    assert!(
        kinds.contains(&NodeKind::Try),
        "rescue modifier should lower to Try"
    );
    assert!(
        kinds.contains(&NodeKind::Loop),
        "while/until modifiers should lower to Loop"
    );
}

#[test]
fn begin_end_loop_modifiers_keep_post_test_shape() {
    assert!(
        has_post_test_loop_block("def f\n  begin\n    tick\n  end while ready?\nend\n"),
        "begin/end while modifier should execute the body before the loop test"
    );
    assert!(
        has_post_test_loop_block("def f\n  begin\n    tick\n  end until done?\nend\n"),
        "begin/end until modifier should execute the body before the loop test"
    );
}

#[test]
fn interpolated_strings_keep_static_chunks_without_raw_wrappers() {
    let raw = raw_name_set("value = :\"authenticate_#{scope}!\"\nother = \"current_#{scope}\"\n");
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "delimited_symbol" | "string_content" | "interpolation"
        )),
        "interpolated strings and symbols should not leak parser wrappers: {raw:?}"
    );

    let str_count = nodes("value = :\"authenticate_#{scope}!\"\n")
        .iter()
        .filter(|node| matches!(node.payload, Payload::LitStr(_)))
        .count();
    assert!(
        str_count >= 2,
        "static interpolation chunks should be retained as string literals"
    );
}

#[test]
fn arrow_lambda_lowers_without_lambda_raw() {
    let raw = raw_name_set("handler = ->(env) { env }\n");
    assert!(
        !raw.iter()
            .any(|name| matches!(name.as_str(), "lambda" | "lambda_parameters")),
        "arrow lambda should lower to Lambda/Param/Block without Raw: {raw:?}"
    );
    let kinds = node_kinds("handler = ->(env) { env }\n");
    assert!(kinds.contains(&NodeKind::Lambda));
    assert!(kinds.contains(&NodeKind::Param));
}

#[test]
fn ruby_binary_keywords_lower_and_unmapped_ops_keep_operator_identity() {
    let ops = binary_ops("x = a and b\ny = a or b\n");
    assert!(
        ops.contains(&Op::And),
        "keyword and should lower to And: {ops:?}"
    );
    assert!(
        ops.contains(&Op::Or),
        "keyword or should lower to Or: {ops:?}"
    );

    let fields = field_names("x = a <=> b\ny = c === d\n");
    assert!(
        fields.contains(&"<=>".to_string()) && fields.contains(&"===".to_string()),
        "Ruby-specific operators should stay distinct as method-call fields: {fields:?}"
    );
}

#[test]
fn ruby_specific_binary_operators_lower_to_distinct_method_calls() {
    let raw = raw_name_set(
        "a = CopyrightRx =~ value\nb = CopyrightRx !~ value\nc = AbstractBlock === parent\nd = now <=> later\n",
    );
    assert!(
        !raw.iter().any(|name| matches!(
            name.as_str(),
            "binary =~" | "binary !~" | "binary ===" | "binary <=>"
        )),
        "Ruby-specific binary operators should no longer fall to Raw: {raw:?}"
    );

    let fields = field_names(
        "a = CopyrightRx =~ value\nb = CopyrightRx !~ value\nc = AbstractBlock === parent\nd = now <=> later\n",
    );
    for expected in ["=~", "!~", "===", "<=>"] {
        assert!(
            fields.iter().any(|name| name == expected),
            "{expected} method-call field should be preserved distinctly: {fields:?}"
        );
    }
    assert!(
        !binary_ops("c = AbstractBlock === parent\n").contains(&Op::Eq),
        "Ruby === must not lower to value equality"
    );
}

#[test]
fn unary_operators_lower_to_distinct_ops() {
    let ops = unary_ops("x = +5\ny = -5\nz = !a\nw = ~5\n");
    assert!(ops.contains(&Op::Pos), "unary + → Op::Pos, got {ops:?}");
    assert!(ops.contains(&Op::Neg), "unary - → Op::Neg, got {ops:?}");
    assert!(ops.contains(&Op::Not), "unary ! → Op::Not, got {ops:?}");
    assert!(
        ops.contains(&Op::BitNot),
        "unary ~ → Op::BitNot, got {ops:?}"
    );
}

#[test]
fn keyword_not_lowers_to_not() {
    assert_eq!(unary_ops("y = not a\n"), vec![Op::Not]);
}

#[test]
fn case_when_uses_case_equality_against_scrutinee() {
    // `case x when 7 ...` is Ruby's `7 === x`, not value equality. The pattern
    // literal must appear and the predicate must stay a Ruby method call.
    let has_seven = nodes("case x\nwhen 7\n  y\nend\n")
        .iter()
        .any(|n| matches!(n.payload, Payload::LitInt(7)));
    assert!(
        has_seven,
        "the `when 7` pattern literal must appear in the lowered IL"
    );
    let fields = field_names("case x\nwhen 7\n  y\nend\n");
    assert!(
        fields.iter().any(|name| name == "==="),
        "scrutinee case should use Ruby case-equality method call: {fields:?}"
    );
    assert!(
        !binary_ops("case x\nwhen 7\n  y\nend\n").contains(&Op::Eq),
        "Ruby case should not lower to value equality"
    );
}

#[test]
fn scrutinee_less_case_uses_when_condition_directly() {
    let ops = binary_ops("case\nwhen x > 0\n  y\nelse\n  z\nend\n");
    assert!(
        ops.contains(&Op::Gt),
        "scrutinee-less case should keep the when predicate, got {ops:?}"
    );
    assert!(
        !ops.contains(&Op::Eq),
        "scrutinee-less case should not compare an empty scrutinee, got {ops:?}"
    );
}

#[test]
fn case_else_body_is_preserved() {
    let mut ints = expr_stmt_ints("case\nwhen x > 0\n  1\nelse\n  2\nend\n");
    ints.sort_unstable();
    assert_eq!(ints, vec![1, 2]);
}

#[test]
fn expression_case_with_then_lowers_without_case_wrapper_raw() {
    let src = "def f(authentication_keys)\n  mapping.to.http_authentication_key || case authentication_keys\n  when Array then authentication_keys.first\n  when Hash then authentication_keys.keys.first\n  end\nend\n";
    let raw = raw_name_set(src);
    assert!(
        !raw.iter()
            .any(|name| matches!(name.as_str(), "case" | "when" | "pattern" | "then")),
        "case expressions and one-line then bodies should lower without Raw wrappers: {raw:?}"
    );
    let ops = binary_ops(src);
    assert!(
        ops.contains(&Op::Or) && !ops.contains(&Op::Eq),
        "case expression should preserve outer || without treating === as Eq: {ops:?}"
    );
    let fields = field_names(src);
    assert!(
        fields.iter().filter(|name| name.as_str() == "===").count() >= 2,
        "case expression should compare patterns to the scrutinee with === calls: {fields:?}"
    );
}

#[test]
fn test_dsl_block_calls_are_units() {
    let units = unit_names(
        r#"
test 'renders table' do
  assert_equal 1, result
end

RSpec.describe 'Widget' do
  it 'renders value' do
expect(result).to eq(1)
  end
end

items.each do |item|
  puts item
end
"#,
    );
    assert!(
        units.contains(&(UnitKind::Block, "test:renders table".to_string())),
        "Minitest-style test blocks should be block units: {units:?}"
    );
    assert!(
        units.contains(&(UnitKind::Block, "describe:Widget".to_string())),
        "RSpec describe blocks should be block units: {units:?}"
    );
    assert!(
        units.contains(&(UnitKind::Block, "it:renders value".to_string())),
        "RSpec it blocks should be block units: {units:?}"
    );
    assert!(
        !units
            .iter()
            .any(|(kind, name)| *kind == UnitKind::Block && name.starts_with("each:")),
        "generic Ruby block calls must not become DSL units: {units:?}"
    );
}
