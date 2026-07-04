use super::*;

pub(super) fn lower_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(left) = node.child_by_field_name("left") {
        clear_assigned_param_alias(lo, left);
    }
    let lhs = match node.child_by_field_name("left") {
        Some(l) => lower_expr(lo, l),
        None => lo.empty_block(span),
    };
    let rhs = match node.child_by_field_name("right") {
        Some(r) => lower_expr(lo, r),
        None => lo.empty_block(span),
    };
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}
pub(super) fn lower_aug_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    if let Some(l) = node.child_by_field_name("left") {
        clear_assigned_param_alias(lo, l);
    }
    crate::lower::compound_assignment(lo, node, py_bin_op, lower_expr, lower_expr)
}
pub(super) fn clear_assigned_param_alias(lo: &mut Lowering, node: TsNode) {
    if node.kind() == "identifier" {
        let name = lo.text(node).to_string();
        lo.clear_type_domain_alias(&name);
    }
}
pub(super) fn clear_defined_param_alias(lo: &mut Lowering, node: TsNode) {
    if let Some(name) = node.child_by_field_name("name") {
        let name = lo.text(name).to_string();
        lo.clear_type_domain_alias(&name);
    }
}
pub(super) fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| lower_block(lo, c, false))
        .unwrap_or_else(|| lo.empty_block(span));

    // Collect elif/else alternatives in source order.
    let mut else_node: Option<NodeId> = None;
    let alternatives: Vec<TsNode> = {
        let mut cur = node.walk();
        node.children_by_field_name("alternative", &mut cur)
            .collect()
    };
    // Fold from the end so elifs nest into the else slot.
    for alt in alternatives.into_iter().rev() {
        match alt.kind() {
            "else_clause" => {
                let b = alt
                    .child_by_field_name("body")
                    .or_else(|| alt.named_child(0))
                    .map(|b| lower_block(lo, b, false))
                    .unwrap_or_else(|| lo.empty_block(lo.span(alt)));
                else_node = Some(b);
            }
            "elif_clause" => {
                let aspan = lo.span(alt);
                let ec = alt
                    .child_by_field_name("condition")
                    .map(|c| lower_expr(lo, c))
                    .unwrap_or_else(|| lo.empty_block(aspan));
                let eb = alt
                    .child_by_field_name("consequence")
                    .map(|c| lower_block(lo, c, false))
                    .unwrap_or_else(|| lo.empty_block(aspan));
                let mut kids = vec![ec, eb];
                if let Some(e) = else_node {
                    kids.push(e);
                }
                else_node = Some(lo.add(NodeKind::If, Payload::None, aspan, &kids));
            }
            _ => {}
        }
    }

    let mut kids = vec![cond, then];
    if let Some(e) = else_node {
        kids.push(e);
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}
pub(super) fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("left")
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let iter = node
        .child_by_field_name("right")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b, false))
        .unwrap_or_else(|| lo.empty_block(span));
    let loop_id = lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    );
    if crate::lower::node_has_child_kind(node, "async") {
        lo.protocol_boundary(
            span,
            nose_il::SourceProtocolKind::AsyncIteration,
            "async_for",
            &[loop_id],
        )
    } else {
        loop_id
    }
}
pub(super) fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, |lo, b| lower_block(lo, b, false))
}
pub(super) fn lower_match(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let subject = semantic_named_children(node)
        .into_iter()
        .find(|child| child.kind() != "block")
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(span));
    let clauses: Vec<TsNode> = semantic_named_children(node)
        .into_iter()
        .flat_map(|child| semantic_named_children(child).into_iter())
        .filter(|child| child.kind() == "case_clause")
        .collect();

    let mut acc = lo.empty_block(span);
    for clause in clauses.into_iter().rev() {
        let cspan = lo.span(clause);
        let body = semantic_named_children(clause)
            .into_iter()
            .rev()
            .find(|child| child.kind() == "block")
            .map(|body| lower_block(lo, body, false))
            .unwrap_or_else(|| lo.empty_block(cspan));
        let Some(pattern) = semantic_named_children(clause)
            .into_iter()
            .find(|child| child.kind() == "case_pattern")
        else {
            acc = body;
            continue;
        };
        let pattern_cond = semantic_named_children(pattern)
            .first()
            .and_then(|&child| lower_match_pattern_condition(lo, subject, child, cspan));
        let guard_cond = semantic_named_children(clause)
            .into_iter()
            .find(|child| child.kind() == "if_clause")
            .and_then(first_semantic_named_child)
            .map(|guard| lower_expr(lo, guard));
        let Some(cond) =
            crate::lower::combine_optional_conditions(lo, cspan, pattern_cond, guard_cond)
        else {
            acc = body;
            continue;
        };
        acc = lo.add(NodeKind::If, Payload::None, cspan, &[cond, body, acc]);
    }
    acc
}
pub(super) fn lower_match_pattern_condition(
    lo: &mut Lowering,
    subject: NodeId,
    pattern: TsNode,
    span: Span,
) -> Option<NodeId> {
    // In Python structural pattern matching, a bare identifier is a capture pattern
    // (including `_`, the wildcard) rather than a value comparison. tree-sitter wraps
    // bare captures as either `identifier` or a one-segment `dotted_name`; qualified
    // dotted names like `Color.RED` remain value patterns.
    if pattern.kind() == "identifier"
        || (pattern.kind() == "dotted_name" && !lo.text(pattern).contains('.'))
    {
        return None;
    }
    if pattern.kind() == "union_pattern" {
        let mut conditions = Vec::new();
        for child in semantic_named_children(pattern) {
            let cond = lower_match_pattern_condition(lo, subject, child, span)?;
            conditions.push(cond);
        }
        return crate::lower::fold_or_conditions(lo, span, conditions);
    }
    if pattern.kind() == "as_pattern" {
        return semantic_named_children(pattern)
            .into_iter()
            .find(|child| child.kind() != "as_pattern_target")
            .and_then(|child| lower_match_pattern_condition(lo, subject, child, span));
    }
    let pat = lower_expr(lo, pattern);
    Some(lo.add(NodeKind::BinOp, Payload::Op(Op::Eq), span, &[subject, pat]))
}
pub(super) fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // Body statements, with the `else` clause's statements appended: Python's
    // `else` runs exactly when the body completes without raising, so under the
    // IL's `(body, handler[, finally])` try model the else IS the tail of the
    // success path. (Accepted nuance: an exception raised inside `else` is
    // modeled as catchable, where real Python would not catch it — far closer
    // than dropping the clause, which erased the success path from the value
    // fingerprint and merged try/import/else wrappers with their bare except
    // arm, #210.)
    let mut body_stmts = Vec::new();
    if let Some(b) = node.child_by_field_name("body") {
        for s in semantic_named_children(b) {
            if let Some(id) = lower_stmt(lo, s, false) {
                body_stmts.push(id);
            }
        }
    }

    // Concatenate all except-clause bodies into one handler block.
    let mut handler_stmts = Vec::new();
    let mut finally_block = None;
    for child in semantic_named_children(node) {
        match child.kind() {
            "else_clause" => {
                if let Some(b) = semantic_named_children(child)
                    .into_iter()
                    .find(|n| n.kind() == "block")
                {
                    for s in semantic_named_children(b) {
                        if let Some(id) = lower_stmt(lo, s, false) {
                            body_stmts.push(id);
                        }
                    }
                }
            }
            "except_clause" | "except_group_clause" => {
                if let Some(b) = child.child_by_field_name("body").or_else(|| {
                    // body is usually the last block child
                    semantic_named_children(child)
                        .into_iter()
                        .rev()
                        .find(|n| n.kind() == "block")
                }) {
                    for s in semantic_named_children(b) {
                        if let Some(id) = lower_stmt(lo, s, false) {
                            handler_stmts.push(id);
                        }
                    }
                }
            }
            "finally_clause" => {
                if let Some(b) = semantic_named_children(child)
                    .into_iter()
                    .find(|n| n.kind() == "block")
                {
                    finally_block = Some(lower_block(lo, b, false));
                }
            }
            _ => {}
        }
    }

    let body = lo.add(NodeKind::Block, Payload::None, span, &body_stmts);
    let mut kids = vec![body];
    let handler = lo.add(NodeKind::Block, Payload::None, span, &handler_stmts);
    kids.push(handler);
    if let Some(f) = finally_block {
        kids.push(f);
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}
