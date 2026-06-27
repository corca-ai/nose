use super::*;

/// A block's trailing expression (no semicolon, not a statement/item/comment).
pub(super) fn is_tail_expr(k: &str) -> bool {
    !matches!(
        k,
        "expression_statement"
            | "let_declaration"
            | "empty_statement"
            | "line_comment"
            | "block_comment"
    ) && !is_item(k)
}
pub(super) fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_item)
}
pub(super) fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "let_declaration" => {
            let pattern = node.child_by_field_name("pattern");
            let type_domain = node
                .child_by_field_name("type")
                .and_then(|ty| lo.type_domain_from_text_with_dependencies(lo.text(ty)));
            let rhs = node
                .child_by_field_name("value")
                .map(|v| lower_expr(lo, v))
                .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
            if let Some(pattern) = pattern {
                if let Some(assigns) = lower_static_projection_pattern(lo, pattern, rhs, span) {
                    let out = if assigns.len() == 1 {
                        assigns[0]
                    } else {
                        lo.add(NodeKind::Block, Payload::None, span, &assigns)
                    };
                    return Some(out);
                }
            }
            if let Some(assigns) = rust_struct_pattern_text_projection(lo, node, rhs, span) {
                let out = if assigns.len() == 1 {
                    assigns[0]
                } else {
                    lo.add(NodeKind::Block, Payload::None, span, &assigns)
                };
                return Some(out);
            }
            let lhs = pattern
                .and_then(|p| ident_of(lo, p))
                .map(|s| {
                    let local = lo.interner.resolve(s).to_string();
                    if let Some(domain) = type_domain.clone() {
                        lo.record_binding_domain_resolution(span, &local, domain);
                    }
                    lo.add(NodeKind::Var, Payload::Name(s), span, &[])
                })
                .unwrap_or_else(|| lo.empty_block(span));
            Some(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]))
        }
        "expression_statement" => {
            let inner = node.named_child(0)?;
            match inner.kind() {
                // assignments and control flow are statements, not expr-statements —
                // lower directly so they converge with other languages' forms.
                "assignment_expression" | "compound_assignment_expr" => Some(lower_expr(lo, inner)),
                k if is_control_expr(k) => Some(lower_expr(lo, inner)),
                _ => {
                    let e = lower_expr(lo, inner);
                    Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
                }
            }
        }
        "empty_statement" => None,
        // an item appearing in a block (nested fn, etc.)
        k if is_item(k) => lower_item(lo, node),
        // a bare expression as the block's tail
        _ => Some(lower_expr(lo, node)),
    }
}
pub(super) fn is_item(k: &str) -> bool {
    matches!(
        k,
        "function_item" | "impl_item" | "trait_item" | "struct_item" | "enum_item" | "mod_item"
    )
}
pub(super) fn is_control_expr(k: &str) -> bool {
    matches!(
        k,
        "if_expression"
            | "match_expression"
            | "for_expression"
            | "while_expression"
            | "loop_expression"
    )
}
pub(super) fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| lower_block(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        // `else` wraps a block or another if_expression
        let e = alt
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lower_expr(lo, alt));
        kids.push(e);
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}
/// `if let PAT = expr` / `let PAT = expr` condition → preserve the pattern test
/// as `expr == PAT`. Rust enum-variant/const pattern resolution is still carried
/// by post-lower occurrence evidence on the lowered pattern nodes.
pub(super) fn lower_cond(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "let_condition" => lower_let_condition(lo, node),
        "let_chain" => lower_let_chain(lo, node),
        _ => lower_expr(lo, node),
    }
}
pub(super) fn lower_let_chain(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut tests: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| !is_type_level(child.kind()))
        .map(|child| lower_cond(lo, child))
        .collect();
    if tests.is_empty() {
        return lo.empty_block(span);
    }
    let mut acc = tests.remove(0);
    for test in tests {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::And), span, &[acc, test]);
    }
    acc
}
pub(super) fn lower_let_condition(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(value_node) = node
        .child_by_field_name("value")
        .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)))
    else {
        return lower_expr(lo, node);
    };
    let value = lower_expr(lo, value_node);
    node.child_by_field_name("pattern")
        .and_then(|pattern| lower_match_pattern_condition(lo, value, pattern, span))
        .unwrap_or(value)
}
#[cfg(test)]
pub(super) fn rust_item_declares_name(src: &str, name: &str) -> bool {
    const ITEM_PREFIXES: &[&str] = &[
        "const", "static", "fn", "struct", "enum", "union", "type", "mod", "trait",
    ];
    src.lines()
        .map(strip_rust_line_comment)
        .map(rust_identifier_tokens)
        .any(|tokens| rust_tokens_declare_name(&tokens, ITEM_PREFIXES, name))
}
#[cfg(test)]
pub(super) fn strip_rust_line_comment(line: &str) -> &str {
    line.split_once("//").map(|(code, _)| code).unwrap_or(line)
}
#[cfg(test)]
pub(super) fn rust_identifier_tokens(line: &str) -> Vec<&str> {
    line.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .collect()
}
#[cfg(test)]
pub(super) fn rust_tokens_declare_name(
    tokens: &[&str],
    item_prefixes: &[&str],
    name: &str,
) -> bool {
    tokens.iter().enumerate().any(|(idx, token)| {
        if !item_prefixes.contains(token) {
            return false;
        }
        tokens
            .get(idx + 1)
            .is_some_and(|candidate| !rust_item_qualifier_token(candidate) && *candidate == name)
    })
}
#[cfg(test)]
pub(super) fn rust_item_qualifier_token(token: &str) -> bool {
    matches!(
        token,
        "pub" | "crate" | "super" | "self" | "unsafe" | "async" | "extern" | "const" | "fn"
    )
}
pub(super) fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("pattern")
        .and_then(|p| ident_of(lo, p))
        .map(|s| lo.add(NodeKind::Var, Payload::Name(s), span, &[]))
        .unwrap_or_else(|| lo.empty_block(span));
    let iter = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}
pub(super) fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_cond, lower_block)
}
pub(super) fn lower_loop(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]); // `loop` ≡ while true
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}
