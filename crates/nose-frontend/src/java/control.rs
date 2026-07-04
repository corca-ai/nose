use super::*;

pub(super) fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_stmt)
}
pub(super) fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "block" => Some(lower_block(lo, node)),
        "local_variable_declaration" => Some(lower_field(lo, node)),
        "expression_statement" => {
            let c = node.named_child(0)?;
            match c.kind() {
                "assignment_expression" | "update_expression" => Some(lower_expr(lo, c)),
                _ => {
                    let e = lower_expr(lo, c);
                    Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
                }
            }
        }
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "enhanced_for_statement" => Some(lower_for_each(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "do_statement" => Some(lower_while(lo, node)),
        "switch_expression" | "switch_statement" => Some(lower_switch(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "assert_statement" => {
            let cond = node
                .named_child(0)
                .map(|expr| lower_expr(lo, expr))
                .unwrap_or_else(|| lo.empty_block(span));
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[cond]))
        }
        "synchronized_statement" => node
            .child_by_field_name("body")
            .map(|body| lower_block(lo, body))
            .or_else(|| {
                Lowering::named_children(node)
                    .into_iter()
                    .find(|child| child.kind() == "block")
                    .map(|body| lower_block(lo, body))
            }),
        "throw_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Throw, Payload::None, span, &kids))
        }
        "try_statement" | "try_with_resources_statement" => Some(lower_try(lo, node)),
        "labeled_statement" => Some(lower_labeled_statement(lo, node)),
        "break_statement" => Some(lower_break_or_continue(lo, node, false)),
        "continue_statement" => Some(lower_break_or_continue(lo, node, true)),
        ";" | "line_comment" | "block_comment" => None,
        k if is_type_decl(k) => lower_item(lo, node),
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}
pub(super) fn lower_labeled_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let label = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "identifier")
        .map(|label| lo.str_lit(lo.text(label), lo.span(label)))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = Lowering::named_children(node)
        .into_iter()
        .rev()
        .find(|child| child.kind() != "identifier")
        .and_then(|body| lower_stmt(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("java_labeled_statement")),
        span,
        &[label, body],
    )
}
pub(super) fn lower_break_or_continue(
    lo: &mut Lowering,
    node: TsNode,
    is_continue: bool,
) -> NodeId {
    let span = lo.span(node);
    let label = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "identifier");
    match (is_continue, label) {
        (false, None) => lo.add(NodeKind::Break, Payload::None, span, &[]),
        (true, None) => lo.add(NodeKind::Continue, Payload::None, span, &[]),
        (false, Some(label)) => {
            let label = lo.str_lit(lo.text(label), lo.span(label));
            lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym("java_labeled_break")),
                span,
                &[label],
            )
        }
        (true, Some(label)) => {
            let label = lo.str_lit(lo.text(label), lo.span(label));
            lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym("java_labeled_continue")),
                span,
                &[label],
            )
        }
    }
}
pub(super) fn is_type_decl(k: &str) -> bool {
    matches!(
        k,
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "method_declaration"
            | "constructor_declaration"
    )
}
pub(super) fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::if_stmt(lo, node, lower_expr, stmt_as_block, stmt_as_block)
}
pub(super) fn stmt_as_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::stmt_as_block(lo, node, "block", lower_block, lower_stmt)
}
pub(super) fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::c_style_for(
        lo,
        node,
        "init",
        "update",
        lower_stmt,
        lower_expr,
        lower_expr,
        stmt_as_block,
    )
}
pub(super) fn lower_for_each(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("name")
        .map(|n| lo.var(lo.text(n), span))
        .unwrap_or_else(|| lo.empty_block(span));
    let iter = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}
pub(super) fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, stmt_as_block)
}
/// `switch` → nested `if`/`else` chain over the switch value's groups.
pub(super) fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::switch_to_if_chain(
        lo,
        node,
        |k| k.starts_with("switch_block_statement_group") || k == "switch_rule",
        lower_expr,
        lower_stmt,
    )
}
pub(super) fn lower_switch_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = switch_expr_value(node)
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let rules: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|body| {
            Lowering::named_children(body)
                .into_iter()
                .filter(|child| child.kind() == "switch_rule")
                .collect()
        })
        .unwrap_or_default();
    let mut branches = Vec::new();
    let mut default_body = None;

    for rule in rules {
        let mut labels = Vec::new();
        let mut body = None;
        let mut saw_label = false;
        for child in Lowering::named_children(rule) {
            if child.kind() == "switch_label" {
                saw_label = true;
                labels.extend(
                    Lowering::named_children(child)
                        .into_iter()
                        .map(|label| lower_expr(lo, label)),
                );
                continue;
            }
            if saw_label {
                body = Some(lower_switch_rule_expr_body(lo, child));
                break;
            }
        }

        let body = body.unwrap_or_else(|| lo.empty_block(span));
        match crate::lower::fold_switch_labels(lo, span, scrutinee, labels) {
            Some(cond) => branches.push((cond, body)),
            None => default_body = Some(body),
        }
    }

    let mut acc = default_body.unwrap_or_else(|| lo.empty_block(span));
    for (cond, body) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
    }
    acc
}
pub(super) fn switch_expr_value(node: TsNode) -> Option<TsNode> {
    node.child_by_field_name("value").or_else(|| {
        Lowering::named_children(node)
            .into_iter()
            .find(|child| child.kind() != "switch_block")
    })
}
pub(super) fn lower_switch_rule_expr_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "block" {
        lower_switch_yield_expr(lo, node).unwrap_or_else(|| lower_block(lo, node))
    } else {
        lower_expr(lo, node)
    }
}
pub(super) fn lower_switch_yield_expr(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "yield_statement")
        .and_then(|child| {
            child
                .child_by_field_name("value")
                .or_else(|| child.named_child(0))
        })
        .map(|expr| lower_expr(lo, expr))
}
pub(super) fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(b) = node.child_by_field_name("body") {
        kids.push(lower_block(lo, b));
    }
    for c in Lowering::named_children(node) {
        if c.kind() == "catch_clause" || c.kind() == "finally_clause" {
            if let Some(b) = c.child_by_field_name("body").or_else(|| {
                c.named_children(&mut c.walk())
                    .find(|n| n.kind() == "block")
            }) {
                kids.push(lower_block(lo, b));
            }
        }
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}
