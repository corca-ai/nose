//! Go → raw IL lowering.
//!
//! Convergence-friendly lowering: `x++`/`x op= y` desugar to assignments; the
//! several `for` forms map to the unified `Loop`; Go concurrency/channel
//! constructs stay as source-backed protocol boundaries; `switch` becomes an
//! `if`/`else if` chain; `true`/`false`/`nil` identifiers become literals.

use crate::lower::Lowering;
use nose_il::{
    Builtin, FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
    SourceProtocolKind, Span,
};
use tree_sitter::Node as TsNode;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
) -> anyhow::Result<Il> {
    crate::lower::lower_file(
        file,
        path,
        src,
        interner,
        crate::lower::grammar::GO,
        || tree_sitter_go::LANGUAGE.into(),
        Lang::Go,
        lower_source,
    )
}

fn lower_source(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_stmt)
}

fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_stmt)
}

fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
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
            // lower the inner statement, ignore the label
            Lowering::named_children(node)
                .into_iter()
                .rev()
                .find_map(|c| lower_stmt(lo, c))
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

fn lower_send_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|child| lower_expr(lo, child))
        .collect();
    let send = lo.protocol_boundary(span, SourceProtocolKind::ChannelSend, "channel_send", &kids);
    lo.add(NodeKind::ExprStmt, Payload::None, span, &[send])
}

fn lower_receive_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let receive_node = Lowering::named_children(node)
        .into_iter()
        .find(|child| is_channel_receive_expr(lo, *child));
    let receive = receive_node
        .map(|receive| lower_channel_receive_expr(lo, receive))
        .unwrap_or_else(|| lo.empty_block(span));
    let lefts = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "expression_list")
        .map(expr_list_items)
        .unwrap_or_default();
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

fn lower_select_statement(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|child| lower_select_child(lo, child))
        .collect();
    lo.protocol_boundary(span, SourceProtocolKind::ChannelSelect, "select", &kids)
}

fn lower_select_child(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "communication_case" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
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
            let kids: Vec<NodeId> = Lowering::named_children(node)
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

fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
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

fn is_expr_kind(k: &str) -> bool {
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

fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    crate::lower::function_unit(lo, node, method, lower_params, lower_block)
}

fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    for decl in Lowering::named_children(params) {
        // a parameter_declaration may bind several names sharing one type
        let mut cur = decl.walk();
        let names: Vec<TsNode> = decl.children_by_field_name("name", &mut cur).collect();
        if names.is_empty() {
            // unnamed parameter (type only) or variadic — emit one anonymous Param
            let span = lo.span(decl);
            out.push(lo.add(NodeKind::Param, Payload::None, span, &[]));
        } else {
            let semantic = crate::lower::param_semantic_from_text(lo.text(decl));
            for n in names {
                let span = lo.span(n);
                let sym = lo.sym(lo.text(n));
                if let Some(semantic) = semantic {
                    lo.record_param_semantic(span, semantic);
                }
                out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[]));
            }
        }
    }
}

fn lower_var_decl(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut assigns = Vec::new();
    for spec in Lowering::named_children(node) {
        if !matches!(spec.kind(), "var_spec" | "const_spec" | "var_spec_list") {
            continue;
        }
        let sspan = lo.span(spec);
        let mut cur = spec.walk();
        let names: Vec<TsNode> = spec.children_by_field_name("name", &mut cur).collect();
        let value = spec.child_by_field_name("value");
        let rhs = match value {
            Some(v) => v.named_child(0).map(|x| lower_expr(lo, x)),
            None => None,
        };
        for n in names {
            let nspan = lo.span(n);
            let lhs = lo.var(lo.text(n), nspan);
            let r = rhs
                .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), nspan, &[]));
            assigns.push(lo.add(NodeKind::Assign, Payload::None, sspan, &[lhs, r]));
        }
    }
    if assigns.len() == 1 {
        assigns[0]
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

/// `a, b := f()` / `a = b` / `a += b`. Multiple targets become a Block of Assigns;
/// compound operators desugar to `a = a OP b`.
fn lower_assign_like(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op_text = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .unwrap_or("=");
    let compound =
        op_text.len() > 1 && op_text.ends_with('=') && op_text != ":=" && op_text != "==";

    let lefts: Vec<TsNode> = left.map(expr_list_items).unwrap_or_default();
    let rights: Vec<TsNode> = right.map(expr_list_items).unwrap_or_default();

    if !compound && lefts.len() == 2 && rights.len() == 1 {
        if let Some(ok_rhs) = lower_map_lookup_ok(lo, rights[0]) {
            if lo.text(lefts[1]) != "_" {
                let mut assigns = Vec::new();
                if lo.text(lefts[0]) != "_" {
                    let lhs = lower_expr(lo, lefts[0]);
                    let rhs = lower_expr(lo, rights[0]);
                    assigns.push(lo.add(
                        NodeKind::Assign,
                        Payload::None,
                        lo.span(lefts[0]),
                        &[lhs, rhs],
                    ));
                }
                let ok_lhs = lower_expr(lo, lefts[1]);
                assigns.push(lo.add(
                    NodeKind::Assign,
                    Payload::None,
                    lo.span(lefts[1]),
                    &[ok_lhs, ok_rhs],
                ));
                return if assigns.len() == 1 {
                    assigns[0]
                } else {
                    lo.add(NodeKind::Block, Payload::None, span, &assigns)
                };
            }
        }
        if is_channel_receive_expr(lo, rights[0]) {
            let mut assigns = Vec::new();
            let receive_span = lo.span(rights[0]);
            let channel = rights[0]
                .child_by_field_name("operand")
                .map(|operand| lower_expr(lo, operand))
                .unwrap_or_else(|| lo.empty_block(receive_span));
            if lo.text(lefts[0]) != "_" {
                let lhs = lower_expr(lo, lefts[0]);
                let value = lo.protocol_boundary(
                    receive_span,
                    SourceProtocolKind::ChannelReceive,
                    "channel_receive",
                    &[channel],
                );
                assigns.push(lo.add(
                    NodeKind::Assign,
                    Payload::None,
                    lo.span(lefts[0]),
                    &[lhs, value],
                ));
            }
            if lo.text(lefts[1]) != "_" {
                let ok_lhs = lower_expr(lo, lefts[1]);
                let ok = lo.protocol_boundary(
                    receive_span,
                    SourceProtocolKind::ChannelReceive,
                    "channel_receive_status",
                    &[channel],
                );
                assigns.push(lo.add(
                    NodeKind::Assign,
                    Payload::None,
                    lo.span(lefts[1]),
                    &[ok_lhs, ok],
                ));
            }
            return if assigns.len() == 1 {
                assigns[0]
            } else {
                lo.add(NodeKind::Block, Payload::None, span, &assigns)
            };
        }
    }

    let mut assigns = Vec::new();
    for (i, l) in lefts.iter().enumerate() {
        let lspan = lo.span(*l);
        let lhs = lower_expr(lo, *l);
        let rhs = if compound {
            let lhs2 = lower_expr(lo, *l);
            let r = rights
                .get(i)
                .map(|n| lower_expr(lo, *n))
                .unwrap_or_else(|| lo.empty_block(lspan));
            // `a &^= b` → `a = a & ^b`, same bit-clear desugar as the binary form.
            if op_text.trim_end_matches('=') == "&^" {
                go_bitclear(lo, lspan, lhs2, r)
            } else {
                let op = js_like_compound_op(op_text).unwrap_or(Op::Add);
                lo.add(NodeKind::BinOp, Payload::Op(op), lspan, &[lhs2, r])
            }
        } else {
            rights
                .get(i)
                .map(|n| lower_expr(lo, *n))
                .unwrap_or_else(|| lo.empty_block(lspan))
        };
        assigns.push(lo.add(NodeKind::Assign, Payload::None, lspan, &[lhs, rhs]));
    }
    if assigns.len() == 1 {
        assigns.pop().unwrap()
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

fn lower_map_lookup_ok(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    if node.kind() != "index_expression" {
        return None;
    }
    let map = node
        .child_by_field_name("operand")
        .map(|o| lower_expr(lo, o))?;
    let key = node
        .child_by_field_name("index")
        .map(|i| lower_expr(lo, i))?;
    Some(lo.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Contains),
        lo.span(node),
        &[key, map],
    ))
}

fn is_channel_receive_expr(lo: &Lowering, node: TsNode) -> bool {
    node.kind() == "unary_expression"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| lo.text(op) == "<-")
}

fn lower_channel_receive_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let arg = node
        .child_by_field_name("operand")
        .map(|a| lower_expr(lo, a))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.protocol_boundary(
        span,
        SourceProtocolKind::ChannelReceive,
        "channel_receive",
        &[arg],
    )
}

fn expr_list_items(node: TsNode) -> Vec<TsNode> {
    if node.kind() == "expression_list" {
        Lowering::named_children(node)
    } else {
        vec![node]
    }
}

fn lower_inc_dec(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op = if node.kind() == "dec_statement" {
        Op::Sub
    } else {
        Op::Add
    };
    let target = node.named_child(0);
    let t1 = target
        .map(|t| lower_expr(lo, t))
        .unwrap_or_else(|| lo.empty_block(span));
    let t2 = target
        .map(|t| lower_expr(lo, t))
        .unwrap_or_else(|| lo.empty_block(span));
    // `++`/`--` step by exactly 1 — emit a *concrete* `LitInt(1)` (like C does), not an
    // abstracted `Lit(Int)`, so `x++` converges with `x = x + 1` and the +1 step is
    // legible to induction-stride analysis in the value graph.
    let one = lo.int_lit("1", span);
    let binop = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[t2, one]);
    lo.add(NodeKind::Assign, Payload::None, span, &[t1, binop])
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn range_bindings(node: TsNode) -> Vec<TsNode> {
    expr_list_items(node)
        .into_iter()
        .filter(|n| n.kind() == "identifier")
        .collect()
}

fn lower_range_binding(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if lo.text(node) == "_" {
        lo.empty_block(span)
    } else {
        lower_expr(lo, node)
    }
}

/// The value binding of a Go `range` left-hand side: the last variable.
/// `_, x` → `x`; `i, v` → `v`; `x` → `x` (a lone var is the index, kept as-is).
fn range_value_var(node: TsNode) -> TsNode {
    if node.kind() == "expression_list" {
        node.named_child(node.named_child_count().saturating_sub(1))
            .unwrap_or(node)
    } else {
        node
    }
}

fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_switch_case_test(
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

fn lower_case_body(lo: &mut Lowering, case: TsNode) -> NodeId {
    let span = lo.span(case);
    // The `value` field holds the case test expression(s); everything else is the
    // body. Skip the test so it doesn't land in the body block.
    let value_id = case.child_by_field_name("value").map(|v| v.id());
    let mut stmts = Vec::new();
    for c in Lowering::named_children(case) {
        if Some(c.id()) == value_id {
            continue;
        }
        if let Some(id) = lower_stmt(lo, c) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "field_identifier" | "package_identifier" | "type_identifier" => {
            match lo.text(node) {
                "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
                "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
                "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
                other => lo.var(other, span),
            }
        }
        "int_literal" => {
            let t = lo.text(node);
            lo.int_lit(t, span)
        }
        "float_literal" | "imaginary_literal" => lo.float_lit(lo.text(node), span),
        "interpreted_string_literal" | "raw_string_literal" | "rune_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "call_expression" => lower_call(lo, node),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => lower_unary(lo, node),
        "selector_expression" => {
            let obj = node
                .child_by_field_name("operand")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let field = node
                .child_by_field_name("field")
                .map(|f| lo.text(f))
                .unwrap_or("");
            let sym = lo.sym(field);
            lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj])
        }
        "index_expression" => {
            let base = node
                .child_by_field_name("operand")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let idx = node
                .child_by_field_name("index")
                .map(|i| lower_expr(lo, i))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Index, Payload::None, span, &[base, idx])
        }
        "func_literal" => {
            let mut kids = Vec::new();
            if let Some(params) = node.child_by_field_name("parameters") {
                lower_params(lo, params, &mut kids);
            }
            let body = node
                .child_by_field_name("body")
                .map(|b| lower_block(lo, b))
                .unwrap_or_else(|| lo.empty_block(span));
            kids.push(body);
            lo.add(NodeKind::Lambda, Payload::None, span, &kids)
        }
        "composite_literal" => {
            let body = node.child_by_field_name("body");
            let kids: Vec<NodeId> = match body {
                Some(b) => Lowering::named_children(b)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect(),
                None => Vec::new(),
            };
            // Tag the literal by its TYPE so the kinds stay semantically distinct (the old
            // blanket `composite_literal` tag collapsed slice ≡ map ≡ struct to one value):
            //   • slice/array  → `array`  — an ordered sequence; converges with `[1,2]` / JS.
            //   • map          → `composite_literal` — preserves go-map handling
            //                    (`proven_go_literal_zero_map_seq`, which keys on this tag).
            //   • struct/named → `go_struct` — a record, NOT a collection; a distinct value so
            //                    `Point{1,2}` no longer value-merges with `[1,2]`.
            // Named-type aliases default to `go_struct` (conservative: distinct, never a false
            // sequence merge — at worst a missed convergence for a named slice alias).
            let tag_str = match node.child_by_field_name("type").map(|t| t.kind()) {
                Some("slice_type" | "array_type") => "array",
                Some("map_type") => "composite_literal",
                _ => "go_struct",
            };
            let tag = lo.sym(tag_str);
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "literal_element" | "keyed_element" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            if kids.len() == 1 {
                kids[0]
            } else {
                let tag = lo.sym(node.kind());
                lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
            }
        }
        "parenthesized_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "type_conversion_expression" => node
            .child_by_field_name("operand")
            .map(|o| lower_expr(lo, o))
            .unwrap_or_else(|| lo.empty_block(span)),
        // A bare `{ … }` initializer (a nested composite, or a body reached
        // directly) → a `Seq` of its elements.
        "literal_value" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `a[lo:hi:cap]` → indexing shape: base plus whatever bounds are present.
        "slice_expression" => {
            let base = node
                .child_by_field_name("operand")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            // Preserve slot POSITIONS: `a[1:]` (start) and `a[:1]` (end) are different
            // slices. A missing bound emits an explicit `None` placeholder so the
            // operands stay positional rather than both collapsing to `Index(a, 1)`.
            let mut kids = vec![base];
            for field in ["start", "end", "capacity"] {
                let v = node
                    .child_by_field_name(field)
                    .map(|n| lower_expr(lo, n))
                    .unwrap_or_else(|| {
                        lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[])
                    });
                kids.push(v);
            }
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        // `x.(T)` is a type-level check; keep the operand `x`, drop the assertion.
        "type_assertion_expression" => node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))
            .map(|o| lower_expr(lo, o))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `args...` → the spread operand.
        "variadic_argument" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type expressions in value position (`[]int`, `map[K]V`, `pkg.T`, …) carry
        // no behavior — collapse to an abstract literal so they aren't `Raw` noise.
        "slice_type" | "map_type" | "array_type" | "pointer_type" | "qualified_type"
        | "interface_type" | "struct_type" | "channel_type" | "function_type" | "generic_type" => {
            lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[])
        }
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("function") {
        Some(f) => kids.push(lower_expr(lo, f)),
        None => {
            let e = lo.empty_block(span);
            kids.push(e);
        }
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    // `a &^ b` (bit-clear / AND-NOT) desugars to `a & ^b`; the generic op-map can only
    // yield a single BinOp, so it is built here rather than in `go_bin_op`.
    if node.child_by_field_name("operator").map(|o| lo.text(o)) == Some("&^") {
        let span = lo.span(node);
        let l = node
            .child_by_field_name("left")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        let r = node
            .child_by_field_name("right")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        return go_bitclear(lo, span, l, r);
    }
    crate::lower::binary(lo, node, go_bin_op, lower_expr)
}

/// Go's bit-clear `a &^ b` ≡ `a & ^b` (AND-NOT) — it clears the bits of `a` that are set
/// in `b`, which is NOT the same as `a & b`. Desugar to that two-node form so the two
/// operators don't collapse to one fingerprint.
fn go_bitclear(lo: &mut Lowering, span: Span, l: NodeId, r: NodeId) -> NodeId {
    let not_r = lo.add(NodeKind::UnOp, Payload::Op(Op::BitNot), span, &[r]);
    lo.add(NodeKind::BinOp, Payload::Op(Op::BitAnd), span, &[l, not_r])
}

fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .unwrap_or("-");
    let operand = node.child_by_field_name("operand");
    match op_text {
        "-" | "+" | "!" | "^" => {
            let op = match op_text {
                "-" => Op::Neg,
                "+" => Op::Pos,
                "!" => Op::Not,
                _ => Op::BitNot,
            };
            let arg = operand
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[arg])
        }
        "<-" => lower_channel_receive_expr(lo, node),
        // `*p`, `&x`: strip to operand. Pointer/place semantics need a separate
        // place-evidence slice; channel receive is preserved above because it has
        // observable synchronization behavior.
        _ => operand
            .map(|a| lower_expr(lo, a))
            .unwrap_or_else(|| lo.empty_block(span)),
    }
}

fn go_bin_op(text: &str) -> Option<Op> {
    // `&^` (bit-clear) is handled by desugaring in `lower_binary` / the compound path,
    // not here, since it expands to two nodes rather than a single BinOp.
    crate::lower::common_bin_op(text)
}

fn js_like_compound_op(text: &str) -> Option<Op> {
    go_bin_op(text.trim_end_matches('='))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        lower(FileId(0), "t.go", src.as_bytes(), &interner)
            .expect("lower")
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::BinOp | NodeKind::UnOp))
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn switch_case_rhs_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.go", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::Eq))
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

    #[test]
    fn switch_cases_compare_scrutinee_to_all_case_labels() {
        let src = "package main\nfunc f(x int) int { switch x { case 1, 2: return 3; default: return 4 } }\n";
        assert_eq!(switch_case_rhs_ints(src), vec![1, 2]);
    }

    #[test]
    fn bit_clear_is_not_plain_bitand() {
        // Go's `a &^ b` is AND-NOT (`a & ^b`): it must desugar to a `BitAnd` over a
        // `BitNot` of the right operand, NOT collapse to a plain `a & b` (different bits,
        // and a false merge with real `&`).
        let clear = ops("package main\nfunc f(a int, b int) int { return a &^ b }\n");
        assert!(
            clear.contains(&Op::BitNot),
            "`a &^ b` must introduce BitNot, got {clear:?}"
        );
        assert!(
            clear.contains(&Op::BitAnd),
            "`a &^ b` must keep BitAnd, got {clear:?}"
        );

        // Plain `a & b` must NOT introduce a BitNot — the two operators stay distinct.
        let and = ops("package main\nfunc f(a int, b int) int { return a & b }\n");
        assert!(
            !and.contains(&Op::BitNot),
            "`a & b` must not introduce BitNot, got {and:?}"
        );
    }

    #[test]
    fn go_defer_and_channel_operations_preserve_source_backed_protocol_boundaries() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.go",
            b"package main\nfunc f(ch chan int, x int) int { go record(x); defer record(x); ch <- x; return <-ch }\n",
            &interner,
        )
        .expect("lower");

        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "go",
            SourceProtocolKind::GoRoutine,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "defer",
            SourceProtocolKind::Defer,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "channel_send",
            SourceProtocolKind::ChannelSend,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "channel_receive",
            SourceProtocolKind::ChannelReceive,
        );
    }

    #[test]
    fn select_statement_preserves_source_backed_protocol_boundary() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.go",
            b"package main\nfunc f(ch chan int) { select { case v := <-ch: _ = v; default: return } }\n",
            &interner,
        )
        .expect("lower");

        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "select",
            SourceProtocolKind::ChannelSelect,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "select_case",
            SourceProtocolKind::ChannelSelectCase,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "select_default",
            SourceProtocolKind::ChannelSelectDefault,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "channel_receive",
            SourceProtocolKind::ChannelReceive,
        );
    }

    #[test]
    fn comma_ok_receive_preserves_value_and_status_protocol_boundaries() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.go",
            b"package main\nfunc f(ch chan int) bool { v, ok := <-ch; _ = v; return ok }\n",
            &interner,
        )
        .expect("lower");

        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "channel_receive",
            SourceProtocolKind::ChannelReceive,
        );
        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "channel_receive_status",
            SourceProtocolKind::ChannelReceive,
        );
    }
}
