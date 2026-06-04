//! Go → raw IL lowering.
//!
//! Convergence-friendly lowering: `x++`/`x op= y` desugar to assignments; the
//! several `for` forms map to the unified `Loop`; `*p`, `&x`, `<-ch` unary
//! wrappers and `go`/`defer` are stripped to their operand; `switch` becomes an
//! `if`/`else if` chain; `true`/`false`/`nil` identifiers become literals.

use crate::lower::Lowering;
use nose_il::{
    Builtin, FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
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
        // strip `go` / `defer` to the called expression
        "go_statement" | "defer_statement" => node.named_child(0).map(|c| {
            let e = lower_expr(lo, c);
            lo.add(NodeKind::ExprStmt, Payload::None, span, &[e])
        }),
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
        "import_declaration" | "package_clause" => Some(crate::lower::import_tokens(lo, node)),
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
            for n in names {
                let span = lo.span(n);
                let sym = lo.sym(lo.text(n));
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

    let mut assigns = Vec::new();
    for (i, l) in lefts.iter().enumerate() {
        let lspan = lo.span(*l);
        let lhs = lower_expr(lo, *l);
        let rhs = if compound {
            let op = js_like_compound_op(op_text).unwrap_or(Op::Add);
            let lhs2 = lower_expr(lo, *l);
            let r = rights
                .get(i)
                .map(|n| lower_expr(lo, *n))
                .unwrap_or_else(|| lo.empty_block(lspan));
            lo.add(NodeKind::BinOp, Payload::Op(op), lspan, &[lhs2, r])
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
                    lo.add(NodeKind::Seq, Payload::None, span, &vars)
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

fn first_of_list(node: TsNode) -> TsNode {
    if node.kind() == "expression_list" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
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
                    let t = lower_expr(lo, first_of_list(v));
                    match val {
                        Some(vv) => lo.add(NodeKind::BinOp, Payload::Op(Op::Eq), cspan, &[vv, t]),
                        None => t,
                    }
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
            let tag = lo.sym("composite_literal");
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
    crate::lower::binary(lo, node, go_bin_op, lower_expr)
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
        // `*p`, `&x`, `<-ch`: strip to operand.
        _ => operand
            .map(|a| lower_expr(lo, a))
            .unwrap_or_else(|| lo.empty_block(span)),
    }
}

fn go_bin_op(text: &str) -> Option<Op> {
    // shared C-family set, plus Go's bit-clear `&^`
    crate::lower::common_bin_op(text).or(match text {
        "&^" => Some(Op::BitAnd),
        _ => None,
    })
}

fn js_like_compound_op(text: &str) -> Option<Op> {
    go_bin_op(text.trim_end_matches('='))
}
