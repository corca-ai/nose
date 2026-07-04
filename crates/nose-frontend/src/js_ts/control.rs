use super::declarations::lower_var_decl;
use super::expressions::lower_expr;
use super::operators::js_bin_op;
use super::{lower_block, lower_stmt};
use crate::lower::Lowering;
use nose_il::{Builtin, LitClass, LoopKind, NodeId, NodeKind, Op, Payload};
use tree_sitter::Node as TsNode;

pub(super) fn lower_aug_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node
        .child_by_field_name("operator")
        .is_some_and(|o| lo.text(o) == "??=")
    {
        let span = lo.span(node);
        let left = node.child_by_field_name("left");
        let lhs1 = left
            .map(|l| lower_expr(lo, l))
            .unwrap_or_else(|| lo.empty_block(span));
        let lhs2 = left
            .map(|l| lower_expr(lo, l))
            .unwrap_or_else(|| lo.empty_block(span));
        let rhs = node
            .child_by_field_name("right")
            .map(|r| lower_expr(lo, r))
            .unwrap_or_else(|| lo.empty_block(span));
        let null_lit = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]);
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[lhs2, null_lit],
        );
        let lhs3 = left
            .map(|l| lower_expr(lo, l))
            .unwrap_or_else(|| lo.empty_block(span));
        let value = lo.add(NodeKind::If, Payload::None, span, &[cond, rhs, lhs3]);
        return lo.add(NodeKind::Assign, Payload::None, span, &[lhs1, value]);
    }
    crate::lower::compound_assignment(lo, node, js_bin_op, lower_expr, lower_expr)
}

/// `x++` / `++x` / `x--`  →  `x = x +/- 1`.
pub(super) fn lower_update(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("--") => Op::Sub,
        _ => Op::Add,
    };
    let arg = node.child_by_field_name("argument");
    let target1 = arg
        .map(|a| lower_expr(lo, a))
        .unwrap_or_else(|| lo.empty_block(span));
    let target2 = arg
        .map(|a| lower_expr(lo, a))
        .unwrap_or_else(|| lo.empty_block(span));
    // `++`/`--` step by exactly 1 — emit a *concrete* `LitInt(1)` (like C does), not an
    // abstracted `Lit(Int)`, so `x++` converges with `x = x + 1` and the +1 step is
    // legible to induction-stride analysis in the value graph.
    let one = lo.int_lit("1", span);
    let binop = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[target2, one]);
    lo.add(NodeKind::Assign, Payload::None, span, &[target1, binop])
}

pub(super) fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::if_stmt(
        lo,
        node,
        |lo, c| lower_expr(lo, unwrap_paren(c)),
        lower_stmt_as_block,
        |lo, alt| {
            // else_clause wraps either a block/statement or another if (else-if).
            let inner = alt.named_child(0).unwrap_or(alt);
            if inner.kind() == "if_statement" {
                lower_if(lo, inner)
            } else {
                lower_stmt_as_block(lo, inner)
            }
        },
    )
}

/// Lower a statement that may or may not be a block into a `Block`.
pub(super) fn lower_stmt_as_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::stmt_as_block(lo, node, "statement_block", lower_block, |lo, n| {
        lower_stmt(lo, n, false)
    })
}

pub(super) fn lower_for_c(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::c_style_for(
        lo,
        node,
        "initializer",
        "increment",
        |lo, n| Some(lower_for_clause_stmt(lo, n)),
        |lo, c| lower_expr(lo, strip_expr_stmt(c)),
        lower_for_clause_stmt,
        lower_stmt_as_block,
    )
}

/// The init/update slots of a C-style for may be declarations, assignments, or
/// update expressions; normalize them to a single statement node.
fn lower_for_clause_stmt(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "lexical_declaration" | "variable_declaration" => lower_var_decl(lo, node),
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr, lower_expr),
        "augmented_assignment_expression" => lower_aug_assignment(lo, node),
        "update_expression" => lower_update(lo, node),
        "expression_statement" => {
            if let Some(c) = node.named_child(0) {
                lower_for_clause_stmt(lo, c)
            } else {
                lo.empty_block(lo.span(node))
            }
        }
        _ => {
            let span = lo.span(node);
            let e = lower_expr(lo, node);
            lo.add(NodeKind::ExprStmt, Payload::None, span, &[e])
        }
    }
}

fn strip_expr_stmt(node: TsNode) -> TsNode {
    if node.kind() == "expression_statement" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    }
}

pub(super) fn lower_for_in(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("left")
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut iter = node
        .child_by_field_name("right")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    // `for (x of it)` iterates VALUES; `for (x in it)` iterates KEYS/indices. Both are
    // tree-sitter `for_in_statement`, distinguished by the `of` keyword. They are
    // behaviorally different, so for-in iterates `Keys(it)` — without this, a for-in
    // (keys) and a for-of (values) over the same collection collapse to one fingerprint.
    let is_of = {
        let mut cur = node.walk();
        let mut found = false;
        for ch in node.children(&mut cur) {
            if ch.kind() == "of" {
                found = true;
                break;
            }
        }
        found
    };
    if !is_of {
        iter = lo.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Keys),
            span,
            &[iter],
        );
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}

// `while` and `do…while` lower identically — both are a `condition`/`body` While
// `Loop` (do-while's run-body-first semantics aren't modelled). One dispatch routes
// both `while_statement` and `do_statement` here.
pub(super) fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(
        lo,
        node,
        |lo, c| lower_expr(lo, unwrap_paren(c)),
        lower_stmt_as_block,
    )
}

/// `switch (v) { case t: ...; default: ... }`  →  nested `if (v == t) {...} else
/// ...`. Fallthrough is ignored (acceptable for fuzzy structural matching).
pub(super) fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let value = node.child_by_field_name("value").map(|v| unwrap_paren(v));
    let body = node.child_by_field_name("body");

    let scrutinee = value
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut pending_labels = Vec::new();
    let mut branches = Vec::new();
    let mut default_block = None;
    if let Some(b) = body {
        for c in Lowering::named_children(b) {
            match c.kind() {
                "switch_case" => {
                    let cspan = lo.span(c);
                    if let Some(test) = c.child_by_field_name("value").map(|t| lower_expr(lo, t)) {
                        pending_labels.push(test);
                    }
                    let stmts = lower_case_body_stmts(lo, c);
                    if stmts.is_empty() {
                        continue;
                    }
                    let block = lo.add(NodeKind::Block, Payload::None, cspan, &stmts);
                    if let Some(cond) = crate::lower::fold_switch_labels(
                        lo,
                        span,
                        scrutinee,
                        pending_labels.split_off(0),
                    ) {
                        branches.push((cond, block));
                    }
                }
                "switch_default" => {
                    pending_labels.clear();
                    let stmts = lower_case_body_stmts(lo, c);
                    default_block =
                        Some(lo.add(NodeKind::Block, Payload::None, lo.span(c), &stmts));
                }
                _ => {}
            }
        }
    }

    // Fold into nested ifs; default becomes the trailing else.
    let mut acc = default_block.unwrap_or_else(|| lo.empty_block(span));
    for (cond, block) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, block, acc]);
    }
    acc
}

fn lower_case_body_stmts(lo: &mut Lowering, case: TsNode) -> Vec<NodeId> {
    // The `value` field is the case test, not part of the body; skip it (and any
    // `break`, which is implicit once we drop fallthrough).
    let value_id = case.child_by_field_name("value").map(|v| v.id());
    let mut stmts = Vec::new();
    for c in Lowering::named_children(case) {
        if Some(c.id()) == value_id || c.kind() == "break_statement" {
            continue;
        }
        if let Some(id) = lower_stmt(lo, c, false) {
            stmts.push(id);
        }
    }
    stmts
}

pub(super) fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![body];
    let handler = node
        .child_by_field_name("handler")
        .and_then(|h| h.child_by_field_name("body").map(|b| lower_block(lo, b)));
    kids.push(handler.unwrap_or_else(|| lo.empty_block(span)));
    if let Some(fin) = node.child_by_field_name("finalizer") {
        let f = fin
            .named_child(0)
            .filter(|n| n.kind() == "statement_block")
            .map(|b| lower_block(lo, b))
            .unwrap_or_else(|| lo.empty_block(span));
        kids.push(f);
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}

fn unwrap_paren(node: TsNode) -> TsNode {
    if node.kind() == "parenthesized_expression" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    }
}
