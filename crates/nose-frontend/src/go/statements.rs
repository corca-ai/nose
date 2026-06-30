use super::*;

pub(super) fn lower_source(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_stmt)
}
/// The statements of a `block` / switch-case / select-case. go 0.25 wraps them in
/// a single `statement_list` node (go 0.23 listed them directly under the parent);
/// flatten that wrapper — without an extra block level — so both grammar shapes
/// lower identically. Non-`statement_list` children (e.g. a case's `value` field)
/// pass through untouched for the caller to handle.
pub(super) fn stmt_children(node: TsNode) -> Vec<TsNode> {
    let mut out = Vec::new();
    for c in Lowering::named_children(node) {
        if c.kind() == "statement_list" {
            out.extend(Lowering::named_children(c));
        } else {
            out.push(c);
        }
    }
    out
}
pub(super) fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for c in stmt_children(node) {
        if let Some(id) = lower_stmt(lo, c) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}
pub(super) fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "function_declaration" => Some(lower_func(lo, node, false)),
        "method_declaration" => Some(lower_func(lo, node, true)),
        "block" => Some(lower_block(lo, node)),
        "short_var_declaration" | "assignment_statement" => Some(lower_assign_like(lo, node)),
        "var_declaration" | "const_declaration" => Some(lower_var_decl(lo, node)),
        "inc_statement" | "dec_statement" => Some(lower_inc_dec(lo, node)),
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "expression_switch_statement" | "type_switch_statement" => Some(lower_switch(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            for c in Lowering::named_children(node) {
                for item in expr_list_items(c) {
                    kids.push(lower_expr(lo, item));
                }
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        "fallthrough_statement" => Some(lo.raw("fallthrough_statement", span, &[])),
        "goto_statement" => Some(lower_goto_statement(lo, node)),
        "receive_statement" => Some(lower_receive_statement(lo, node)),
        "send_statement" => Some(lower_send_statement(lo, node)),
        "go_statement" => node.named_child(0).map(|c| {
            let call = lower_expr(lo, c);
            let boundary = lo.protocol_boundary(span, SourceProtocolKind::GoRoutine, "go", &[call]);
            lo.add(NodeKind::ExprStmt, Payload::None, span, &[boundary])
        }),
        "defer_statement" => node.named_child(0).map(|c| {
            let call = lower_expr(lo, c);
            let boundary = lo.protocol_boundary(span, SourceProtocolKind::Defer, "defer", &[call]);
            lo.add(NodeKind::ExprStmt, Payload::None, span, &[boundary])
        }),
        "select_statement" => Some(lower_select_statement(lo, node)),
        "labeled_statement" => {
            let children = Lowering::named_children(node);
            let label = children.iter().find(|child| child.kind() == "label_name");
            let inner = children
                .iter()
                .rev()
                .find(|child| child.kind() != "label_name")
                .and_then(|child| lower_stmt(lo, *child));
            match (label, inner) {
                (Some(label), Some(stmt)) => {
                    let marker = lo.raw(
                        &format!("go_label {}", lo.text(*label)),
                        lo.span(*label),
                        &[],
                    );
                    Some(lo.add(NodeKind::Block, Payload::None, span, &[marker, stmt]))
                }
                (Some(label), None) => Some(lo.raw(
                    &format!("go_label {}", lo.text(*label)),
                    lo.span(*label),
                    &[],
                )),
                (None, Some(stmt)) => Some(stmt),
                (None, None) => None,
            }
        }
        "expression_statement" => {
            let c = node.named_child(0)?;
            let e = lower_expr(lo, c);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
        "import_declaration" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        "package_clause" => Some(crate::lower::import_tokens(lo, node)),
        "comment" | "type_declaration" => None,
        _ => {
            // call expressions etc. can appear directly as statements
            if is_expr_kind(node.kind()) {
                let e = lower_expr(lo, node);
                Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
            } else {
                let kids: Vec<NodeId> = Lowering::named_children(node)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect();
                Some(lo.raw(node.kind(), span, &kids))
            }
        }
    }
}
pub(super) fn lower_goto_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let label = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "label_name")
        .map(|child| lo.text(child).to_string())
        .unwrap_or_default();
    if label.is_empty() {
        lo.raw("goto_statement", span, &[])
    } else {
        lo.raw(&format!("go_goto {label}"), span, &[])
    }
}
pub(super) fn lower_send_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|child| lower_expr(lo, child))
        .collect();
    let send = lo.protocol_boundary(span, SourceProtocolKind::ChannelSend, "channel_send", &kids);
    lo.add(NodeKind::ExprStmt, Payload::None, span, &[send])
}
pub(super) fn lower_receive_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let receive_node = Lowering::named_children(node)
        .into_iter()
        .find(|child| is_channel_receive_expr(lo, *child));
    let lefts = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "expression_list")
        .map(expr_list_items)
        .unwrap_or_default();
    if let Some(receive_node) = receive_node {
        if lefts.len() == 2 {
            return lower_two_value_channel_receive(lo, span, &lefts, &[receive_node])
                .unwrap_or_else(|| lo.empty_block(span));
        }
    }
    let receive = receive_node
        .map(|receive| lower_channel_receive_expr(lo, receive))
        .unwrap_or_else(|| lo.empty_block(span));
    if let Some(lhs_node) = lefts.into_iter().find(|lhs| lo.text(*lhs) != "_") {
        let lhs = lower_expr(lo, lhs_node);
        lo.add(
            NodeKind::Assign,
            Payload::None,
            lo.span(lhs_node),
            &[lhs, receive],
        )
    } else {
        lo.add(NodeKind::ExprStmt, Payload::None, span, &[receive])
    }
}
pub(super) fn lower_select_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|child| lower_select_child(lo, child))
        .collect();
    lo.protocol_boundary(span, SourceProtocolKind::ChannelSelect, "select", &kids)
}
pub(super) fn lower_select_child(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "communication_case" => {
            let kids: Vec<NodeId> = stmt_children(node)
                .into_iter()
                .filter_map(|child| lower_stmt(lo, child))
                .collect();
            lo.protocol_boundary(
                span,
                SourceProtocolKind::ChannelSelectCase,
                "select_case",
                &kids,
            )
        }
        "default_case" => {
            let kids: Vec<NodeId> = stmt_children(node)
                .into_iter()
                .filter_map(|child| lower_stmt(lo, child))
                .collect();
            lo.protocol_boundary(
                span,
                SourceProtocolKind::ChannelSelectDefault,
                "select_default",
                &kids,
            )
        }
        _ => lower_stmt(lo, node).unwrap_or_else(|| lower_expr(lo, node)),
    }
}
pub(super) fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim();
    let rest = text.strip_prefix("import ")?.trim();
    if rest.starts_with('(') {
        return None;
    }
    let quote_pos = rest.find('"')?;
    let local = rest[..quote_pos].trim();
    if local == "." || local == "_" {
        return None;
    }
    let module_rest = &rest[quote_pos + 1..];
    let end = module_rest.find('"')?;
    let module = &module_rest[..end];
    let local = if local.is_empty() {
        module.rsplit('/').next().unwrap_or(module)
    } else {
        local
    };
    Some(crate::lower::import_namespace(lo, span, local, module))
}
pub(super) fn is_expr_kind(k: &str) -> bool {
    matches!(
        k,
        "call_expression"
            | "binary_expression"
            | "unary_expression"
            | "selector_expression"
            | "index_expression"
            | "identifier"
            | "parenthesized_expression"
    )
}
