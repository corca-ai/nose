use super::*;

pub(super) fn lower_assign(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    let r = node
        .child_by_field_name("right")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    if node.kind() == "operator_assignment" {
        let opt = node
            .child_by_field_name("operator")
            .map(|o| lo.text(o))
            .unwrap_or("+=");
        let l2 = node
            .child_by_field_name("left")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        // An unmapped compound operator keeps its own raw shape — dropping the
        // operator would merge it with `x = y`.
        let value = match common_bin_op(opt.trim_end_matches('=')) {
            Some(op) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l2, r]),
            None => lo.raw(&format!("compound_assignment {opt}"), span, &[l2, r]),
        };
        return lo.add(NodeKind::Assign, Payload::None, span, &[l, value]);
    }
    lo.add(NodeKind::Assign, Payload::None, span, &[l, r])
}
/// Lower a string. A plain string is a value-retaining `LitStr`; an interpolated
/// string (`"hi #{name}"`) lowers to a string-concat chain — a base `Str` literal
/// then `Add` of each `#{…}` expression — converging with a JS template / f-string.
pub(super) fn lower_string(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let children = Lowering::named_children(node);
    let has_string_parts = children
        .iter()
        .any(|c| matches!(c.kind(), "string_content" | "interpolation"));
    if !has_string_parts {
        let text = lo.text(node);
        if matches!(
            node.kind(),
            "symbol" | "simple_symbol" | "hash_key_symbol" | "delimited_symbol"
        ) {
            return lo.str_lit(text.trim_start_matches(':').trim_end_matches(':'), span);
        }
        return lo.str_lit(text, span);
    }
    let mut parts = Vec::new();
    for child in children {
        match child.kind() {
            "string_content" => parts.push(lo.str_lit(lo.text(child), lo.span(child))),
            "interpolation" => {
                if let Some(expr) = child.named_child(0) {
                    parts.push(lower_expr(lo, expr));
                }
            }
            _ => {}
        }
    }
    parts
        .into_iter()
        .reduce(|acc, part| lo.add(NodeKind::BinOp, Payload::Op(Op::Add), span, &[acc, part]))
        .unwrap_or_else(|| lo.str_lit("", span))
}
pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "constant" | "instance_variable" | "class_variable" | "global_variable"
        | "self" => lo.var(lo.text(node), span),
        "integer" => lo.int_lit(lo.text(node), span),
        "float" => lo.float_lit(lo.text(node), span),
        "character" => lo.str_lit(lo.text(node), span),
        // Ruby symbols are atoms (`:foo`, and the `foo:` key in `{foo: 1}`); lower as
        // string literals so their value participates in matching like any constant.
        // Heredocs (`<<~SQL … SQL`) are multi-line strings: tree-sitter splits them
        // into a `heredoc_beginning` (in value position) and a dangling `heredoc_body`
        // (content + `#{…}` interpolations) — both lower like any interpolated string.
        "string" | "bare_string" | "symbol" | "simple_symbol" | "hash_key_symbol"
        | "delimited_symbol" | "string_content" | "interpolation" | "string_array"
        | "symbol_array" | "heredoc_beginning" | "heredoc_body" => lower_string(lo, node),
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary" => lower_binary(lo, node),
        "unary" => lower_unary(lo, node),
        "assignment" | "operator_assignment" => lower_assign(lo, node),
        "method_call" | "call" => lower_call(lo, node),
        "element_reference" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "array" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym("array");
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "hash" => lower_hash(lo, node),
        "pair" => lower_hash_pair(lo, node),
        "block" | "do_block" => lower_block_lambda(lo, node),
        "lambda" => lower_arrow_lambda(lo, node),
        "parenthesized_statements" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "if" | "unless" => lower_if(lo, node),
        "case" => lower_case(lo, node),
        // A guard modifier in expression/tail position (Ruby's implicit return lowers
        // the last statement as an expr) lowers the same as in statement position.
        "if_modifier" | "unless_modifier" => lower_modifier(lo, node),
        // Ternary `c ? a : b` → `If` (converges with if-expressions elsewhere).
        "conditional" | "ternary" => {
            let kids: Vec<NodeId> = ["condition", "consequence", "alternative"]
                .iter()
                .filter_map(|f| node.child_by_field_name(f))
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::If, Payload::None, span, &kids)
        }
        // Adjacent string literals (`"a" "b"`) concatenate to one string.
        "chained_string" => lower_string(lo, node),
        // `*args` / `&blk` / `**kw` argument forms — lower the wrapped expression.
        "splat_argument" | "block_argument" | "hash_splat_argument" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `Foo::Bar` — a qualified name; treat as one Var atom (robust to nesting).
        "scope_resolution" => lo.var(lo.text(node), span),
        // A regex literal is a constant.
        "regex" => lo.str_lit(lo.text(node), span),
        // `begin … rescue … end` as an expression (e.g. RHS of an assignment).
        "begin" | "do" => lower_begin(lo, node),
        // `expr rescue fallback` is expression-level exception handling.
        "rescue_modifier" => lower_rescue_modifier(lo, node),
        // Backtick / `%x(...)` shell execution: preserve it as an opaque call keyed
        // by the source command rather than leaking parser-only string_content Raw.
        "subshell" => lower_subshell(lo, node),
        // Argument/assignment-target lists and ranges → a sequence of their elements.
        "argument_list" | "left_assignment_list" | "right_assignment_list" | "range" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `yield x` invokes the method's block with demand/effect semantics,
        // so keep it as a source protocol boundary instead of an ordinary value.
        "yield" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.protocol_boundary(span, SourceProtocolKind::BlockYield, "yield", &kids)
        }
        "super" | "forward_argument" => lo.var(lo.text(node), span),
        _ => raw_kids(lo, node),
    }
}
pub(super) fn lower_rescue_modifier(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids = Lowering::named_children(node);
    let body_expr = kids
        .first()
        .map(|child| lower_expr(lo, *child))
        .unwrap_or_else(|| lo.empty_block(span));
    let fallback_expr = kids
        .get(1)
        .map(|child| lower_expr(lo, *child))
        .unwrap_or_else(|| lo.empty_block(span));
    let body_stmt = lo.add(NodeKind::ExprStmt, Payload::None, span, &[body_expr]);
    let fallback_stmt = lo.add(NodeKind::ExprStmt, Payload::None, span, &[fallback_expr]);
    let body = lo.add(NodeKind::Block, Payload::None, span, &[body_stmt]);
    let fallback = lo.add(NodeKind::Block, Payload::None, span, &[fallback_stmt]);
    lo.add(NodeKind::Try, Payload::None, span, &[body, fallback])
}
pub(super) fn lower_arrow_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let children = Lowering::named_children(node);
    let mut kids = Vec::new();
    if let Some(params) = children
        .iter()
        .find(|child| child.kind() == "lambda_parameters")
    {
        for param in Lowering::named_children(*params) {
            let pspan = lo.span(param);
            let sym = param_name(lo, param);
            kids.push(lo.add(
                NodeKind::Param,
                sym.map(Payload::Name).unwrap_or(Payload::None),
                pspan,
                &[],
            ));
        }
    }
    let body = children
        .into_iter()
        .find(|child| child.kind() != "lambda_parameters")
        .map(|child| {
            if matches!(child.kind(), "block" | "do_block") {
                block_body(lo, child)
            } else {
                let stmt = lower_stmt(lo, child).unwrap_or_else(|| lower_expr(lo, child));
                lo.add(NodeKind::Block, Payload::None, lo.span(child), &[stmt])
            }
        })
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
pub(super) fn lower_subshell(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let callee = lo.var("__ruby_subshell__", span);
    let command = lo.str_lit(lo.text(node), span);
    lo.add(NodeKind::Call, Payload::None, span, &[callee, command])
}
pub(super) fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(operator) = node
        .child_by_field_name("operator")
        .map(|op| lo.text(op))
        .filter(|op| ruby_specific_binary_operator(op))
    {
        let left = node
            .child_by_field_name("left")
            .map(|child| lower_expr(lo, child))
            .unwrap_or_else(|| lo.empty_block(span));
        let right = node
            .child_by_field_name("right")
            .map(|child| lower_expr(lo, child))
            .unwrap_or_else(|| lo.empty_block(span));
        return ruby_operator_call(lo, span, operator, left, right);
    }
    crate::lower::binary(lo, node, ruby_bin_op, lower_expr)
}
pub(super) fn ruby_specific_binary_operator(text: &str) -> bool {
    matches!(text, "=~" | "!~" | "===" | "<=>")
}
pub(super) fn ruby_operator_call(
    lo: &mut Lowering,
    span: Span,
    operator: &str,
    receiver: NodeId,
    argument: NodeId,
) -> NodeId {
    let callee = lo.add(
        NodeKind::Field,
        Payload::Name(lo.sym(operator)),
        span,
        &[receiver],
    );
    lo.add(NodeKind::Call, Payload::None, span, &[callee, argument])
}
pub(super) fn ruby_bin_op(text: &str) -> Option<Op> {
    match text {
        // Ruby `%` is FLOORED (remainder takes the divisor's sign), unlike the
        // C-family truncated `%` in `common_bin_op` (#283-D).
        "%" => Some(Op::FloorMod),
        // Ruby integer `/` is FLOORED (`-7 / 2 == -4`), like Python `//` — distinct
        // from C-family truncated `Op::Div` and Python/JS true-float `Op::TrueDiv`
        // (#283-D).
        "/" => Some(Op::FloorDiv),
        "and" => Some(Op::And),
        "or" => Some(Op::Or),
        other => common_bin_op(other),
    }
}
pub(super) fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .named_child(node.named_child_count().saturating_sub(1))
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    // Map by the operator token, not the leading byte: `+`→Pos, `-`→Neg,
    // `~`→BitNot, `!`/`not`→Not. Reading only the first byte collapsed `+5`
    // and `~5` onto `Neg`.
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("+") => Op::Pos,
        Some("~") => Op::BitNot,
        Some("!") | Some("not") => Op::Not,
        _ => Op::Neg,
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}
pub(super) fn lower_hash(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    for child in Lowering::named_children(node) {
        match child.kind() {
            "pair" => kids.push(lower_hash_pair(lo, child)),
            "hash_splat_argument" => {
                let inner: Vec<NodeId> = Lowering::named_children(child)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect();
                kids.push(lo.raw(child.kind(), lo.span(child), &inner));
            }
            _ => kids.push(lower_expr(lo, child)),
        }
    }
    let tag = lo.sym("hash");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}
pub(super) fn lower_hash_pair(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    let tag = lo.sym("pair");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}
