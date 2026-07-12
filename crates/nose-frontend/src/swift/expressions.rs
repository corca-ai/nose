use super::*;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "simple_identifier" | "type_identifier" => match lo.text(node) {
            "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
            "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
            other => lo.var(other, span),
        },
        "self_expression" => lo.var("self", span),
        "super_expression" => lo.var("super", span),
        "custom_operator" => lower_operator_ref(lo, node),
        k if is_swift_operator_token_kind(k) => lower_operator_ref(lo, node),
        "integer_literal" | "hex_literal" | "oct_literal" | "bin_literal" => {
            lo.int_lit(lo.text(node), span)
        }
        "real_literal" => lo.float_lit(lo.text(node), span),
        "line_string_literal"
        | "multi_line_string_literal"
        | "raw_string_literal"
        | "regex_literal" => lower_string(lo, node),
        "boolean_literal" => {
            let text = lo.text(node);
            lo.add(NodeKind::Lit, Payload::LitBool(text == "true"), span, &[])
        }
        "special_literal" => lower_special_literal(lo, node),
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "bang" => lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("swift_force_marker")),
            span,
            &[],
        ),
        "array_literal" => lower_seq(lo, node, "array"),
        "dictionary_literal" => lower_dictionary(lo, node),
        "tuple_expression" => lower_tuple(lo, node),
        "assignment" => lower_assignment(lo, node),
        "directly_assignable_expression" => peel_value_child(lo, node),
        "additive_expression"
        | "multiplicative_expression"
        | "comparison_expression"
        | "equality_expression"
        | "conjunction_expression"
        | "disjunction_expression"
        | "nil_coalescing_expression"
        | "infix_expression"
        | "range_expression"
        | "open_start_range_expression"
        | "open_end_range_expression"
        | "fully_open_range"
        | "bitwise_operation" => lower_binary_like(lo, node),
        "prefix_expression" => lower_prefix(lo, node),
        "postfix_expression" => lower_postfix(lo, node),
        "ternary_expression" => lower_ternary(lo, node),
        "key_path_expression" | "key_path_string_expression" => lower_key_path(lo, node),
        "value_binding_pattern" | "switch_pattern" | "pattern" => lower_pattern_value(lo, node),
        "if_statement" | "guard_statement" => lower_if(lo, node),
        "switch_statement" => lower_switch(lo, node),
        "call_expression" | "constructor_expression" => lower_call(lo, node),
        "macro_invocation" => lower_macro_invocation(lo, node),
        "diagnostic" => lower_diagnostic(lo, node),
        "directive" => lower_directive(lo, node),
        "navigation_expression" => lower_navigation(lo, node),
        "selector_expression" => lower_selector_expression(lo, node),
        "lambda_literal" => lower_lambda(lo, node),
        "as_expression" | "check_expression" => {
            let value = peel_value_child(lo, node);
            lo.raw("swift_cast_or_type_check", span, &[value])
        }
        "consume_expression" => {
            let value = peel_value_child(lo, node);
            lo.raw("swift_consume_expression", span, &[value])
        }
        "value_pack_expansion" => peel_value_child(lo, node),
        "try_expression" => {
            let value = first_expr_child(node)
                .map(|child| lower_expr(lo, child))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.protocol_boundary(span, SourceProtocolKind::TryPropagation, "try", &[value])
        }
        "await_expression" => {
            let value = first_expr_child(node)
                .map(|child| lower_expr(lo, child))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.await_boundary(span, value)
        }
        "control_transfer_statement" => lower_control_transfer(lo, node)
            .unwrap_or_else(|| lo.raw("control_transfer_statement", span, &[])),
        "statement_label" => lower_statement_label(lo, node),
        k if is_type_level(k) => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|child| lower_expr(lo, child))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}
pub(super) fn lower_string(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let interpolations: Vec<NodeId> = field_children(node, "interpolation")
        .into_iter()
        .map(|interp| {
            let kids: Vec<NodeId> = Lowering::named_children(interp)
                .into_iter()
                .filter(|child| is_expr_kind(child.kind()))
                .map(|child| lower_expr(lo, child))
                .collect();
            if kids.len() == 1 {
                kids[0]
            } else {
                lo.raw("string_interpolation", lo.span(interp), &kids)
            }
        })
        .collect();
    if interpolations.is_empty() {
        lo.str_lit(lo.text(node), span)
    } else {
        let mut kids = Vec::with_capacity(interpolations.len() + 1);
        kids.push(lo.str_lit(lo.text(node), span));
        kids.extend(interpolations);
        lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("interpolated_string")),
            span,
            &kids,
        )
    }
}
pub(super) fn lower_seq(lo: &mut Lowering, node: TsNode, tag: &str) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = field_children(node, "element")
        .into_iter()
        .flat_map(|child| expr_list_children(child).into_iter())
        .map(|child| lower_expr(lo, child))
        .collect();
    lo.add(NodeKind::Seq, Payload::Name(lo.sym(tag)), span, &kids)
}
pub(super) fn lower_dictionary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let keys = field_children(node, "key");
    let values = field_children(node, "value");
    let mut entries = Vec::new();
    for (key, value) in keys.iter().zip(values.iter()) {
        let k = lower_expr(lo, *key);
        let v = lower_expr(lo, *value);
        entries.push(lo.add(NodeKind::Seq, Payload::Name(lo.sym("pair")), span, &[k, v]));
    }
    lo.add(NodeKind::Seq, Payload::Name(lo.sym("map")), span, &entries)
}
pub(super) fn lower_tuple(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()) || child.kind() == "value_argument")
        .map(|child| {
            if child.kind() == "value_argument" {
                lower_value_argument(lo, child)
            } else {
                lower_expr(lo, child)
            }
        })
        .collect();
    if kids.len() == 1 {
        return kids[0];
    }
    lo.add(NodeKind::Seq, Payload::Name(lo.sym("tuple")), span, &kids)
}
pub(super) fn lower_binary_like(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if node.kind() == "fully_open_range" {
        return lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("swift_range_fully_open")),
            span,
            &[],
        );
    }
    let lhs = node
        .child_by_field_name("lhs")
        .or_else(|| node.child_by_field_name("left"))
        .or_else(|| node.child_by_field_name("value"));
    let rhs = node
        .child_by_field_name("rhs")
        .or_else(|| node.child_by_field_name("right"))
        .or_else(|| node.child_by_field_name("if_nil"));
    let op_text = node
        .child_by_field_name("op")
        .or_else(|| node.child_by_field_name("operator"))
        .map(|op| lo.text(op));
    if node.kind() == "nil_coalescing_expression" {
        let left = lhs
            .map(|lhs| lower_expr(lo, lhs))
            .unwrap_or_else(|| lo.empty_block(span));
        let right = rhs
            .map(|rhs| lower_expr(lo, rhs))
            .unwrap_or_else(|| lo.empty_block(span));
        return lo.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::ValueOrDefault),
            span,
            &[left, right],
        );
    }
    if let (Some(lhs), Some(rhs), Some(op_text)) = (lhs, rhs, op_text) {
        let left = lower_expr(lo, lhs);
        let right = lower_expr(lo, rhs);
        if op_text == "??" {
            return lo.add(
                NodeKind::Call,
                Payload::Builtin(Builtin::ValueOrDefault),
                span,
                &[left, right],
            );
        }
        if let Some(rewritten) = lower_misnested_swift_boolean_rhs(lo, span, op_text, left, right) {
            return rewritten;
        }
        if let Some(base_op) = swift_compound_assignment_base(op_text) {
            let read_left = lower_expr(lo, lhs);
            let value = if let Some(op) = swift_bin_op(base_op) {
                lo.add(NodeKind::BinOp, Payload::Op(op), span, &[read_left, right])
            } else {
                lower_swift_specific_infix(lo, span, base_op, &[read_left, right])
            };
            return lo.add(NodeKind::Assign, Payload::None, span, &[left, value]);
        }
        if let Some(range) = lower_range_op(lo, span, op_text, left, right) {
            return range;
        }
        if let Some(op) = swift_bin_op(op_text) {
            return lo.add(NodeKind::BinOp, Payload::Op(op), span, &[left, right]);
        }
        return lower_swift_specific_infix(lo, span, op_text, &[left, right]);
    }
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()))
        .map(|child| lower_expr(lo, child))
        .collect();
    if kids.len() == 1 {
        kids[0]
    } else if let Some(op_text) = op_text {
        if let [left, right] = kids.as_slice() {
            if let Some(range) = lower_range_op(lo, span, op_text, *left, *right) {
                return range;
            }
        }
        lower_swift_specific_infix(lo, span, op_text, &kids)
    } else {
        lo.raw(node.kind(), span, &kids)
    }
}
pub(super) fn lower_special_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let literal = lo.str_lit(lo.text(node), span);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_special_literal")),
        span,
        &[literal],
    )
}
pub(super) fn lower_swift_specific_infix(
    lo: &mut Lowering,
    span: Span,
    op_text: &str,
    kids: &[NodeId],
) -> NodeId {
    let tag = match op_text {
        "===" => "swift_identity_eq".to_string(),
        "!==" => "swift_identity_ne".to_string(),
        "&+" => "swift_overflow_add".to_string(),
        "&-" => "swift_overflow_sub".to_string(),
        "&*" => "swift_overflow_mul".to_string(),
        "&<<" => "swift_overflow_shl".to_string(),
        "&>>" => "swift_overflow_shr".to_string(),
        "&+=" => "swift_overflow_add_assign".to_string(),
        "&-=" => "swift_overflow_sub_assign".to_string(),
        "&*=" => "swift_overflow_mul_assign".to_string(),
        "&<<=" => "swift_overflow_shl_assign".to_string(),
        "&>>=" => "swift_overflow_shr_assign".to_string(),
        other => format!("swift_infix_{other}"),
    };
    lo.add(NodeKind::Seq, Payload::Name(lo.sym(&tag)), span, kids)
}
pub(super) fn lower_misnested_swift_boolean_rhs(
    lo: &mut Lowering,
    span: Span,
    op_text: &str,
    left: NodeId,
    right: NodeId,
) -> Option<NodeId> {
    let cmp_op = swift_bin_op(op_text)?;
    if !matches!(cmp_op, Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne) {
        return None;
    }
    if lo.b.kind(right) != NodeKind::BinOp {
        return None;
    }
    let Payload::Op(bool_op @ (Op::And | Op::Or)) = lo.b.payload(right) else {
        return None;
    };
    let rhs_children = lo.b.children(right).to_vec();
    let [rhs_left, rhs_right] = rhs_children.as_slice() else {
        return None;
    };
    let fixed_left = lo.add(
        NodeKind::BinOp,
        Payload::Op(cmp_op),
        span,
        &[left, *rhs_left],
    );
    Some(lo.add(
        NodeKind::BinOp,
        Payload::Op(bool_op),
        span,
        &[fixed_left, *rhs_right],
    ))
}
pub(super) fn swift_bin_op(text: &str) -> Option<Op> {
    match text {
        "&&" => Some(Op::And),
        "||" => Some(Op::Or),
        "..<" | "..." => None,
        other => common_bin_op(other),
    }
}
pub(super) fn swift_compound_assignment_base(text: &str) -> Option<&str> {
    match text {
        "+=" => Some("+"),
        "-=" => Some("-"),
        "*=" => Some("*"),
        "/=" => Some("/"),
        "%=" => Some("%"),
        "&=" => Some("&"),
        "|=" => Some("|"),
        "^=" => Some("^"),
        "<<=" => Some("<<"),
        ">>=" => Some(">>"),
        "&+=" => Some("&+"),
        "&-=" => Some("&-"),
        "&*=" => Some("&*"),
        "&<<=" => Some("&<<"),
        "&>>=" => Some("&>>"),
        _ => None,
    }
}
pub(super) fn lower_range_op(
    lo: &mut Lowering,
    span: Span,
    op_text: &str,
    left: NodeId,
    right: NodeId,
) -> Option<NodeId> {
    let tag = match op_text {
        "..<" => "swift_range_half_open",
        "..." => "swift_range_closed",
        _ => return None,
    };
    Some(lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym(tag)),
        span,
        &[left, right],
    ))
}
pub(super) fn lower_prefix(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let target_node = node
        .child_by_field_name("target")
        .or_else(|| first_expr_child(node));
    let op_text = swift_prefix_operator_text(lo, node, target_node);
    let operand = target_node
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(span));
    if op_text == "!" {
        lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[operand])
    } else if op_text == "-" {
        lo.add(NodeKind::UnOp, Payload::Op(Op::Neg), span, &[operand])
    } else if op_text == "+" {
        lo.add(NodeKind::UnOp, Payload::Op(Op::Pos), span, &[operand])
    } else if op_text == "&" {
        operand
    } else if op_text == "." {
        lower_implicit_member(lo, span, operand)
    } else {
        lower_prefix_operator_surface(lo, span, &op_text, operand)
    }
}
pub(super) fn swift_prefix_operator_text(
    lo: &Lowering,
    node: TsNode,
    target: Option<TsNode>,
) -> String {
    if let Some(op) = node.child_by_field_name("operation") {
        return lo.text(op).trim().to_string();
    }
    if let Some(target) = target {
        let start = node.start_byte();
        let end = target.start_byte();
        if start <= end && end <= lo.src.len() {
            return std::str::from_utf8(&lo.src[start..end])
                .unwrap_or("")
                .trim()
                .to_string();
        }
    }
    lo.text(node).trim_start().to_string()
}
pub(super) fn lower_prefix_operator_surface(
    lo: &mut Lowering,
    span: Span,
    op_text: &str,
    operand: NodeId,
) -> NodeId {
    let op = lo.str_lit(op_text, span);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_prefix_operator")),
        span,
        &[op, operand],
    )
}
pub(super) fn lower_operator_ref(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op = lo.str_lit(lo.text(node), span);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_operator_ref")),
        span,
        &[op],
    )
}
pub(super) fn is_swift_operator_token_kind(kind: &str) -> bool {
    !kind.is_empty() && kind.chars().all(|ch| "+-*/%=!<>&|^~.?".contains(ch))
}
pub(super) fn lower_implicit_member(lo: &mut Lowering, span: Span, member: NodeId) -> NodeId {
    if lo.b.kind(member) == NodeKind::Var {
        if let Payload::Name(field) = lo.b.payload(member) {
            let base = lo.var("swift_implicit_member", span);
            return lo.add(NodeKind::Field, Payload::Name(field), span, &[base]);
        }
    }
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_implicit_member")),
        span,
        &[member],
    )
}
pub(super) fn lower_key_path(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| !is_type_level(child.kind()))
        .map(|child| lower_pattern_value(lo, child))
        .collect();
    if kids.is_empty() {
        return lo.str_lit(lo.text(node), span);
    }
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_key_path")),
        span,
        &kids,
    )
}
pub(super) fn lower_pattern_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "simple_identifier" | "identifier" | "type_identifier" => lo.var(lo.text(node), span),
        "self_expression" => lo.var("self", span),
        k if is_expr_kind(k)
            && !matches!(
                k,
                "value_binding_pattern" | "switch_pattern" | "pattern" | "key_path_expression"
            ) =>
        {
            lower_expr(lo, node)
        }
        k if is_type_level(k) => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .filter(|child| !is_type_level(child.kind()))
                .map(|child| lower_pattern_value(lo, child))
                .collect();
            match kids.as_slice() {
                [only]
                    if matches!(
                        node.kind(),
                        "pattern" | "switch_pattern" | "directly_assignable_expression"
                    ) =>
                {
                    *only
                }
                _ => lo.add(
                    NodeKind::Seq,
                    Payload::Name(lo.sym(&format!("swift_{}", node.kind()))),
                    span,
                    &kids,
                ),
            }
        }
    }
}
pub(super) fn lower_postfix(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operation")
        .map(|op| lo.text(op))
        .unwrap_or_else(|| lo.text(node).trim_end());
    let operand_node = node
        .child_by_field_name("target")
        .or_else(|| first_expr_child(node));
    let operand = operand_node
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(span));
    if op_text.ends_with('?') {
        operand
    } else if op_text.ends_with('!') {
        lo.raw("swift_force_unwrap", span, &[operand])
    } else {
        lo.raw("postfix_expression", span, &[operand])
    }
}
pub(super) fn lower_ternary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()))
        .map(|child| lower_expr(lo, child))
        .collect();
    match kids.as_slice() {
        [cond, yes, no] => {
            let then = lo.add(NodeKind::Block, Payload::None, span, &[*yes]);
            let els = lo.add(NodeKind::Block, Payload::None, span, &[*no]);
            lo.add(NodeKind::If, Payload::None, span, &[*cond, then, els])
        }
        [cond, branch] => lower_ternary_with_implicit_nil(lo, node, span, *cond, *branch)
            .unwrap_or_else(|| lo.raw("ternary_expression", span, &kids)),
        _ => lo.raw("ternary_expression", span, &kids),
    }
}
pub(super) fn lower_ternary_with_implicit_nil(
    lo: &mut Lowering,
    node: TsNode,
    span: Span,
    cond: NodeId,
    branch: NodeId,
) -> Option<NodeId> {
    let text = lo.text(node);
    let question = text.find('?')?;
    let colon = text.rfind(':')?;
    if question >= colon {
        return None;
    }
    let yes_text = text[question + 1..colon].trim();
    let no_text = text[colon + 1..].trim();
    let null = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]);
    let (yes, no) = if yes_text == "nil" {
        (null, branch)
    } else if no_text == "nil" {
        (branch, null)
    } else {
        return None;
    };
    let then = lo.add(NodeKind::Block, Payload::None, span, &[yes]);
    let els = lo.add(NodeKind::Block, Payload::None, span, &[no]);
    Some(lo.add(NodeKind::If, Payload::None, span, &[cond, then, els]))
}
