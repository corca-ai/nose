use super::*;
use crate::tree_sitter_ext::last_named_child;

pub(super) fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let init = node
        .child_by_field_name("initializer")
        .and_then(|i| lower_stmt(lo, i));
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| lower_block(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        let else_node = if alt.kind() == "if_statement" {
            lower_if(lo, alt)
        } else {
            lower_block(lo, alt)
        };
        kids.push(else_node);
    }
    let if_node = lo.add(NodeKind::If, Payload::None, span, &kids);
    match init {
        Some(i) => lo.add(NodeKind::Block, Payload::None, span, &[i, if_node]),
        None => if_node,
    }
}
pub(super) fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));

    // Find the loop-control child (for_clause / range_clause / bare condition).
    let mut clause = None;
    for c in Lowering::named_children(node) {
        match c.kind() {
            "for_clause" | "range_clause" => {
                clause = Some(c);
                break;
            }
            _ if is_expr_kind(c.kind()) => {
                clause = Some(c);
                break;
            }
            _ => {}
        }
    }

    match clause {
        Some(c) if c.kind() == "for_clause" => {
            let init = c
                .child_by_field_name("initializer")
                .and_then(|i| lower_stmt(lo, i))
                .unwrap_or_else(|| lo.empty_block(span));
            let cond = c
                .child_by_field_name("condition")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            let update = c
                .child_by_field_name("update")
                .and_then(|u| lower_stmt(lo, u))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(
                NodeKind::Loop,
                Payload::Loop(LoopKind::CStyle),
                span,
                &[init, cond, update, body],
            )
        }
        Some(c) if c.kind() == "range_clause" => {
            let left = c.child_by_field_name("left");
            let mut iter = c
                .child_by_field_name("right")
                .map(|r| lower_expr(lo, r))
                .unwrap_or_else(|| lo.empty_block(span));
            let pat = match left {
                Some(l)
                    if range_bindings(l).len() >= 2
                        && range_bindings(l)
                            .first()
                            .is_some_and(|first| lo.text(*first) != "_") =>
                {
                    let vars: Vec<NodeId> = range_bindings(l)
                        .into_iter()
                        .map(|v| lower_range_binding(lo, v))
                        .collect();
                    iter = lo.add(
                        NodeKind::Call,
                        Payload::Builtin(Builtin::Enumerate),
                        span,
                        &[iter],
                    );
                    let tag = lo.sym("tuple");
                    lo.add(NodeKind::Seq, Payload::Name(tag), span, &vars)
                }
                Some(l) => lower_expr(lo, range_value_var(l)),
                None => lo.empty_block(span),
            };
            lo.add(
                NodeKind::Loop,
                Payload::Loop(LoopKind::ForEach),
                span,
                &[pat, iter, body],
            )
        }
        Some(c) => {
            // bare condition: `for cond { }`
            let cond = lower_expr(lo, c);
            lo.add(
                NodeKind::Loop,
                Payload::Loop(LoopKind::While),
                span,
                &[cond, body],
            )
        }
        None => {
            // infinite loop: `for { }`
            let cond = lo.empty_block(span);
            lo.add(
                NodeKind::Loop,
                Payload::Loop(LoopKind::While),
                span,
                &[cond, body],
            )
        }
    }
}
pub(super) fn range_bindings(node: TsNode) -> Vec<TsNode> {
    expr_list_items(node)
        .into_iter()
        .filter(|n| n.kind() == "identifier")
        .collect()
}
pub(super) fn lower_range_binding(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if lo.text(node) == "_" {
        lo.empty_block(span)
    } else {
        lower_expr(lo, node)
    }
}
/// The value binding of a Go `range` left-hand side: the last variable.
/// `_, x` → `x`; `i, v` → `v`; `x` → `x` (a lone var is the index, kept as-is).
pub(super) fn range_value_var(node: TsNode) -> TsNode {
    if node.kind() == "expression_list" {
        last_named_child(node).unwrap_or(node)
    } else {
        node
    }
}
pub(super) fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let value = node.child_by_field_name("value");
    let mut cases: Vec<(Option<NodeId>, NodeId)> = Vec::new();
    for c in Lowering::named_children(node) {
        match c.kind() {
            "expression_case" => {
                let cspan = lo.span(c);
                let test = c.child_by_field_name("value").map(|v| {
                    let val = value.map(|x| lower_expr(lo, x));
                    lower_switch_case_test(lo, cspan, val, v)
                });
                let blk = lower_case_body(lo, c);
                cases.push((test, blk));
            }
            "default_case" => {
                let blk = lower_case_body(lo, c);
                cases.push((None, blk));
            }
            // A type-switch case: the runtime type test is unmodeled semantics, but
            // the ARM BODIES are real code — keep the test as a raw shape keyed by
            // the case's type spelling so the bodies survive in the fingerprint.
            // (They used to fall through this match, lowering the whole type-switch
            // to an empty block — a recursive type-switch traversal fingerprinted
            // identically to a constant stub, #210.)
            "type_case" => {
                let cspan = lo.span(c);
                let test = lower_type_case_test(lo, cspan, &type_case_labels(c));
                let blk = lower_case_body(lo, c);
                cases.push((Some(test), blk));
            }
            _ => {}
        }
    }
    let mut else_node: Option<NodeId> = None;
    for (test, blk) in cases.into_iter().rev() {
        match test {
            None => else_node = Some(blk),
            Some(t) => {
                let mut kids = vec![t, blk];
                if let Some(e) = else_node {
                    kids.push(e);
                }
                else_node = Some(lo.add(NodeKind::If, Payload::None, span, &kids));
            }
        }
    }
    else_node.unwrap_or_else(|| lo.empty_block(span))
}
pub(super) fn type_case_labels(case: TsNode) -> Vec<TsNode> {
    Lowering::named_children(case)
        .into_iter()
        .filter(|child| is_type_surface_kind(child.kind()))
        .collect()
}
pub(super) fn lower_type_case_test(lo: &mut Lowering, span: Span, labels: &[TsNode]) -> NodeId {
    let mut tests: Vec<NodeId> = labels
        .iter()
        .map(|label| {
            lo.raw(
                &format!("type_case {}", lo.text(*label)),
                lo.span(*label),
                &[],
            )
        })
        .collect();
    if tests.is_empty() {
        return lo.raw("type_case", span, &[]);
    }
    let mut acc = tests.remove(0);
    for test in tests {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[acc, test]);
    }
    acc
}
pub(super) fn lower_switch_case_test(
    lo: &mut Lowering,
    span: Span,
    scrutinee: Option<NodeId>,
    value: TsNode,
) -> NodeId {
    let labels = if value.kind() == "expression_list" {
        let labels = Lowering::named_children(value);
        if labels.is_empty() {
            vec![value]
        } else {
            labels
        }
    } else {
        vec![value]
    };
    let mut conds: Vec<NodeId> = labels
        .into_iter()
        .map(|label| {
            let test = lower_expr(lo, label);
            match scrutinee {
                Some(subject) => {
                    lo.add(NodeKind::BinOp, Payload::Op(Op::Eq), span, &[subject, test])
                }
                None => test,
            }
        })
        .collect();
    let mut acc = conds.remove(0);
    for cond in conds {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[acc, cond]);
    }
    acc
}
pub(super) fn lower_case_body(lo: &mut Lowering, case: TsNode) -> NodeId {
    let span = lo.span(case);
    // The `value`/`type` field holds the case test expression(s); everything else
    // is the body. Skip the test so it doesn't land in the body block.
    let value_id = case.child_by_field_name("value").map(|v| v.id());
    let type_id = case.child_by_field_name("type").map(|v| v.id());
    let type_label_ids: Vec<_> = if case.kind() == "type_case" {
        type_case_labels(case)
            .into_iter()
            .map(|label| label.id())
            .collect()
    } else {
        Vec::new()
    };
    let mut stmts = Vec::new();
    for c in stmt_children(case) {
        if Some(c.id()) == value_id || Some(c.id()) == type_id || type_label_ids.contains(&c.id()) {
            continue;
        }
        if let Some(id) = lower_stmt(lo, c) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}
