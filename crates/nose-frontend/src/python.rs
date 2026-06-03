//! Python → raw IL lowering.
//!
//! Covers the constructs that matter for clone detection (functions, classes,
//! control flow, calls, operators, literals, comprehensions) and falls back to
//! `Raw` for the rest. A few convergence-friendly choices are made here because
//! they are language-specific: `await e` is stripped to `e` (async/sync clones),
//! compound assignment is desugared (no core node for it), and ternary lowers to
//! an expression-position `If`.

use crate::lower::Lowering;
use nose_il::{
    FileId, HoFKind, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
    UnitKind,
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
        crate::lower::grammar::PYTHON,
        || tree_sitter_python::LANGUAGE.into(),
        Lang::Python,
        lower_module,
    )
}

fn lower_module(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, |lo, c| lower_stmt(lo, c, false))
}

/// Lower one statement. `in_class` tags nested `def`s as methods. Returns `None`
/// for statements that are pure noise for clone detection (imports, globals).
fn lower_stmt(lo: &mut Lowering, node: TsNode, in_class: bool) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "function_definition" => Some(lower_func(lo, node, in_class)),
        "decorated_definition" => {
            // Ignore decorators; lower the wrapped definition.
            let def = node.child_by_field_name("definition")?;
            lower_stmt(lo, def, in_class)
        }
        "class_definition" => Some(lower_class(lo, node)),
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "raise_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Throw, Payload::None, span, &kids))
        }
        "try_statement" => Some(lower_try(lo, node)),
        "with_statement" => {
            // Treat `with ...: body` as its body block (the context manager is
            // mostly setup/teardown noise for structural matching).
            let body = node.child_by_field_name("body");
            Some(match body {
                Some(b) => lower_block(lo, b, false),
                None => lo.empty_block(span),
            })
        }
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        "pass_statement" => Some(lo.empty_block(span)),
        "assert_statement" => {
            // `assert cond[, msg]` → ExprStmt(cond) (msg is incidental)
            let cond = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[cond]))
        }
        "delete_statement" => None,
        "expression_statement" => {
            let child = node.named_child(0)?;
            match child.kind() {
                "assignment" => Some(lower_assignment(lo, child)),
                "augmented_assignment" => Some(lower_aug_assignment(lo, child)),
                _ => {
                    let e = lower_expr(lo, child);
                    Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
                }
            }
        }
        "import_statement" | "import_from_statement" | "future_import_statement" => {
            Some(crate::lower::import_tokens(lo, node))
        }
        "global_statement" | "nonlocal_statement" | "comment" => None,
        // Anything else in statement position: treat as an expression statement
        // (lower_expr has its own Raw fallback for genuinely unknown nodes).
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

fn lower_block(lo: &mut Lowering, node: TsNode, in_class: bool) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, |lo, c| {
        lower_stmt(lo, c, in_class)
    })
}

fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    crate::lower::function_unit(lo, node, method, lower_params, |lo, b| {
        lower_block(lo, b, false)
    })
}

fn lower_class(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body_block = match node.child_by_field_name("body") {
        Some(b) => lower_block(lo, b, true),
        None => lo.empty_block(span),
    };
    lo.push_unit(body_block, UnitKind::Class, name);
    body_block
}

fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    for p in Lowering::named_children(params) {
        let span = lo.span(p);
        let name = param_name(lo, p);
        let payload = match name {
            Some(s) => Payload::Name(lo.sym(s)),
            None => Payload::None,
        };
        out.push(lo.add(NodeKind::Param, payload, span, &[]));
    }
}

/// Dig the identifier name out of the various Python parameter node shapes.
fn param_name<'a>(lo: &Lowering<'a>, p: TsNode<'a>) -> Option<&'a str> {
    match p.kind() {
        "identifier" => Some(lo.text(p)),
        "typed_parameter" | "default_parameter" | "typed_default_parameter" => p
            .child_by_field_name("name")
            .or_else(|| p.named_child(0))
            .map(|n| lo.text(n)),
        "list_splat_pattern" | "dictionary_splat_pattern" => p.named_child(0).map(|n| lo.text(n)),
        _ => p.named_child(0).map(|n| lo.text(n)),
    }
}

fn lower_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
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

/// `a OP= b`  →  `a = a OP b`. The lhs is lowered twice (two faithful subtrees).
fn lower_aug_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .and_then(py_aug_op)
        .unwrap_or(Op::Add);

    let lhs1 = match left {
        Some(l) => lower_expr(lo, l),
        None => lo.empty_block(span),
    };
    let lhs2 = match left {
        Some(l) => lower_expr(lo, l),
        None => lo.empty_block(span),
    };
    let rhs = match right {
        Some(r) => lower_expr(lo, r),
        None => lo.empty_block(span),
    };
    let binop = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[lhs2, rhs]);
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs1, binop])
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
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
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, |lo, b| lower_block(lo, b, false))
}

fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b, false))
        .unwrap_or_else(|| lo.empty_block(span));

    // Concatenate all except-clause bodies into one handler block.
    let mut handler_stmts = Vec::new();
    let mut finally_block = None;
    for child in Lowering::named_children(node) {
        match child.kind() {
            "except_clause" | "except_group_clause" => {
                if let Some(b) = child.child_by_field_name("body").or_else(|| {
                    // body is usually the last block child
                    Lowering::named_children(child)
                        .into_iter()
                        .rev()
                        .find(|n| n.kind() == "block")
                }) {
                    for s in Lowering::named_children(b) {
                        if let Some(id) = lower_stmt(lo, s, false) {
                            handler_stmts.push(id);
                        }
                    }
                }
            }
            "finally_clause" => {
                if let Some(b) = Lowering::named_children(child)
                    .into_iter()
                    .find(|n| n.kind() == "block")
                {
                    finally_block = Some(lower_block(lo, b, false));
                }
            }
            _ => {}
        }
    }

    let mut kids = vec![body];
    let handler = lo.add(NodeKind::Block, Payload::None, span, &handler_stmts);
    kids.push(handler);
    if let Some(f) = finally_block {
        kids.push(f);
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}

/// Lower a string. A plain string is a value-retaining `LitStr`; an f-string
/// (one with `{expr}` interpolations) lowers to a string-concat chain — a base
/// `Str` literal then `Add` of each interpolated expression — so it converges with
/// a JS template literal and a `"…" + x` concatenation.
fn lower_string(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let interps: Vec<TsNode> = Lowering::named_children(node)
        .into_iter()
        .filter(|c| c.kind() == "interpolation")
        .collect();
    if interps.is_empty() {
        return lo.str_lit(lo.text(node), span);
    }
    let mut acc = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Str), span, &[]);
    for interp in interps {
        // `interpolation` wraps the expression as its first named child.
        if let Some(e) = interp
            .child_by_field_name("expression")
            .or_else(|| interp.named_child(0))
        {
            let sub = lower_expr(lo, e);
            acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Add), span, &[acc, sub]);
        }
    }
    acc
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" => lo.var(lo.text(node), span),
        "integer" => {
            let t = lo.text(node);
            lo.int_lit(t, span)
        }
        "float" => lo.float_lit(lo.text(node), span),
        "string" | "concatenated_string" | "string_content" => lower_string(lo, node),
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "none" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "ellipsis" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[]),
        "call" => lower_call(lo, node),
        "binary_operator" => lower_binary(lo, node),
        "boolean_operator" => lower_boolop(lo, node),
        "comparison_operator" => lower_comparison(lo, node),
        "unary_operator" => {
            let op = node
                .child_by_field_name("operator")
                .map(|o| lo.text(o))
                .unwrap_or("-");
            let il_op = match op {
                "-" => Op::Neg,
                "+" => Op::Pos,
                "~" => Op::BitNot,
                _ => Op::Neg,
            };
            let arg = node
                .child_by_field_name("argument")
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(il_op), span, &[arg])
        }
        "not_operator" => {
            let arg = node
                .child_by_field_name("argument")
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[arg])
        }
        "attribute" => {
            let obj = node
                .child_by_field_name("object")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let attr = node
                .child_by_field_name("attribute")
                .map(|a| lo.text(a))
                .unwrap_or("");
            let sym = lo.sym(attr);
            lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj])
        }
        "subscript" => {
            let base = node
                .child_by_field_name("value")
                .map(|v| lower_expr(lo, v))
                .unwrap_or_else(|| lo.empty_block(span));
            let idx = node
                .child_by_field_name("subscript")
                .map(|s| lower_expr(lo, s))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Index, Payload::None, span, &[base, idx])
        }
        "lambda" => {
            let mut kids = Vec::new();
            if let Some(params) = node.child_by_field_name("parameters") {
                lower_params(lo, params, &mut kids);
            }
            // Wrap the single-expression body in `Block(Return(expr))` so a
            // `lambda x: e` converges with a JS arrow `x => e` (and `x => { return e }`)
            // and a one-line function — all single-expression callables share a shape.
            let body = match node.child_by_field_name("body") {
                Some(b) => {
                    let bspan = lo.span(b);
                    let e = lower_expr(lo, b);
                    let ret = lo.add(NodeKind::Return, Payload::None, bspan, &[e]);
                    lo.add(NodeKind::Block, Payload::None, bspan, &[ret])
                }
                None => lo.empty_block(span),
            };
            kids.push(body);
            lo.add(NodeKind::Lambda, Payload::None, span, &kids)
        }
        "slice" => {
            // Preserve start/stop/step POSITIONS: `a[1:]` (start=1) and `a[:1]` (stop=1)
            // are different slices and must not collapse. tree-sitter omits empty bounds
            // and the `:` separators are anonymous, so collecting only named children
            // loses which slot the bound occupies. Walk children in order, split on `:`,
            // and emit an explicit `None` placeholder for each empty slot so the `Seq` is
            // positional.
            let mut slots: Vec<NodeId> = Vec::new();
            let mut cur: Option<NodeId> = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == ":" {
                    slots.push(cur.take().unwrap_or_else(|| {
                        lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[])
                    }));
                } else if child.is_named() {
                    cur = Some(lower_expr(lo, child));
                }
            }
            slots.push(
                cur.take().unwrap_or_else(|| {
                    lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[])
                }),
            );
            lo.add(NodeKind::Seq, Payload::None, span, &slots)
        }
        "list" | "tuple" | "set" | "dictionary" | "pattern_list" | "expression_list"
        | "list_pattern" | "tuple_pattern" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // splats / unpacking: strip to the inner expression
        "list_splat" | "dictionary_splat" | "list_splat_pattern" | "dictionary_splat_pattern" => {
            node.named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span))
        }
        "pair" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        "list_comprehension"
        | "set_comprehension"
        | "generator_expression"
        | "dictionary_comprehension" => lower_comprehension(lo, node),
        "conditional_expression" => lower_ternary(lo, node),
        "parenthesized_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Strip `await` so sync and async variants converge.
        "await" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "named_expression" => {
            // walrus `name := value` → Assign in expression position
            let lhs = node
                .child_by_field_name("name")
                .map(|n| lower_expr(lo, n))
                .unwrap_or_else(|| lo.empty_block(span));
            let rhs = node
                .child_by_field_name("value")
                .map(|v| lower_expr(lo, v))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
        }
        "keyword_argument" => node
            .child_by_field_name("value")
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lo.empty_block(span)),
        "assignment" => lower_assignment(lo, node),
        "augmented_assignment" => lower_aug_assignment(lo, node),
        // comprehension clauses, if ever reached directly: lower the meaningful part
        "for_in_clause" => node
            .child_by_field_name("right")
            .or_else(|| node.named_child(1))
            .map(|r| lower_expr(lo, r))
            .unwrap_or_else(|| lo.empty_block(span)),
        "if_clause" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "yield" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
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
    if let Some(f) = node.child_by_field_name("function") {
        kids.push(lower_expr(lo, f));
    } else {
        let e = lo.empty_block(span);
        kids.push(e);
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        // `f(x for x in xs)` — a bare generator argument: tree-sitter makes the
        // `generator_expression` the `arguments` node itself, so iterating its named
        // children would flatten the generator into separate args and drop the `for`
        // binding. Lower it as one comprehension argument (→ `HoF(Map)`).
        if args.kind() == "generator_expression" {
            kids.push(lower_comprehension(lo, args));
        } else {
            for a in Lowering::named_children(args) {
                kids.push(lower_expr(lo, a));
            }
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, py_bin_op, lower_expr)
}

fn lower_boolop(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("or") => Op::Or,
        _ => Op::And,
    };
    let l = node
        .child_by_field_name("left")
        .map(|n| lower_expr(lo, n))
        .unwrap_or_else(|| lo.empty_block(span));
    let r = node
        .child_by_field_name("right")
        .map(|n| lower_expr(lo, n))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r])
}

/// Python comparison can chain (`a < b < c`). Two operands → one `BinOp`;
/// longer chains fold into `And` of pairwise comparisons.
fn lower_comparison(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // Walk children in order, separating operand expressions from operator tokens.
    // Operator keywords (`<`, `==`, `in`, `not`, `is`, …) may be anonymous or named,
    // combined (`not in`) or split (`not` + `in`); the operator between two operands is
    // the space-joined run of operator tokens seen between them. This keeps `not in` /
    // `is not` NEGATED — previously the negation was dropped (`x is not None` collapsed
    // with `x is None`) and `not in` mis-lowered to `==`.
    fn is_op_tok(t: &str) -> bool {
        matches!(
            t,
            "<" | "<="
                | ">"
                | ">="
                | "=="
                | "!="
                | "<>"
                | "in"
                | "not"
                | "is"
                | "not in"
                | "is not"
        )
    }
    let mut operand_nodes: Vec<TsNode> = Vec::new();
    let mut ops: Vec<(Op, bool)> = Vec::new(); // (op, negated)
    let mut pending: Vec<String> = Vec::new();
    let mut cur = node.walk();
    for c in node.children(&mut cur) {
        let t = lo.text(c).trim();
        if is_op_tok(t) {
            pending.push(t.to_string());
        } else if c.is_named() {
            operand_nodes.push(c);
            if operand_nodes.len() >= 2 {
                let key = pending.join(" ");
                ops.push(py_cmp_op(&key).unwrap_or((Op::Eq, false)));
                pending.clear();
            }
        }
    }
    if operand_nodes.len() < 2 {
        return operand_nodes
            .first()
            .map(|n| lower_expr(lo, *n))
            .unwrap_or_else(|| lo.empty_block(span));
    }
    let mut acc: Option<NodeId> = None;
    for i in 0..operand_nodes.len() - 1 {
        // Lower each operand fresh per use so a chained `a<b<c` keeps `b` as two
        // independent subtrees (a tree, not a shared-child DAG).
        let l = lower_expr(lo, operand_nodes[i]);
        let r = lower_expr(lo, operand_nodes[i + 1]);
        let (op, neg) = ops.get(i).copied().unwrap_or((Op::Eq, false));
        let cmp = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r]);
        let cmp = if neg {
            lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[cmp])
        } else {
            cmp
        };
        acc = Some(match acc {
            None => cmp,
            Some(prev) => lo.add(NodeKind::BinOp, Payload::Op(Op::And), span, &[prev, cmp]),
        });
    }
    acc.unwrap_or_else(|| lo.empty_block(span))
}

/// A comprehension `[body for x in xs]` lowers to `HoF(Map)[xs, λx. body]`, with
/// the body wrapped as `Block[Return[body]]` so it converges with a JS
/// `xs.map(x => body)` arrow (whose expression body lowers the same way). A filter
/// `… if cond` wraps the collection in `HoF(Filter)[xs, λx. cond]`, so a filtered
/// comprehension converges with a guarded loop (`if cond: …`) — see §AI.
fn lower_comprehension(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body_node = node.named_child(0);

    let clause = Lowering::named_children(node)
        .into_iter()
        .find(|c| c.kind() == "for_in_clause");
    let pattern = clause.and_then(|c| c.child_by_field_name("left").or_else(|| c.named_child(0)));
    let mut collection = clause
        .and_then(|c| c.child_by_field_name("right").or_else(|| c.named_child(1)))
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));

    // Build a lambda `λ<pattern>. <Block[Return[body]]>` over the iteration pattern.
    let lambda = |lo: &mut Lowering, body: NodeId, bspan| {
        let mut kids = Vec::new();
        if let Some(p) = pattern {
            push_pattern_params(lo, p, &mut kids);
        }
        let ret = lo.add(NodeKind::Return, Payload::None, bspan, &[body]);
        let block = lo.add(NodeKind::Block, Payload::None, bspan, &[ret]);
        kids.push(block);
        lo.add(NodeKind::Lambda, Payload::None, bspan, &kids)
    };

    // Each `if cond` clause wraps the collection in a `HoF(Filter)`.
    for f in Lowering::named_children(node) {
        if f.kind() != "if_clause" {
            continue;
        }
        if let Some(cn) = f.named_child(0) {
            let fspan = lo.span(f);
            let cond = lower_expr(lo, cn);
            let flam = lambda(lo, cond, fspan);
            collection = lo.add(
                NodeKind::HoF,
                Payload::HoF(HoFKind::Filter),
                fspan,
                &[collection, flam],
            );
        }
    }

    let body = body_node
        .map(|b| lower_expr(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    let map_lam = lambda(lo, body, span);
    lo.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        span,
        &[collection, map_lam],
    )
}

/// Emit `Param` nodes for a comprehension/loop target (identifier or tuple).
fn push_pattern_params(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    match node.kind() {
        "tuple_pattern" | "pattern_list" | "tuple" | "list_pattern" => {
            for c in Lowering::named_children(node) {
                push_pattern_params(lo, c, out);
            }
        }
        _ => {
            let span = lo.span(node);
            let sym = lo.sym(lo.text(node));
            out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[]));
        }
    }
}

fn lower_ternary(lo: &mut Lowering, node: TsNode) -> NodeId {
    // Python: `then if cond else alt`. Named children order: [then, cond, alt].
    let span = lo.span(node);
    let kids = Lowering::named_children(node);
    let then = kids
        .first()
        .map(|n| lower_expr(lo, *n))
        .unwrap_or_else(|| lo.empty_block(span));
    let cond = kids
        .get(1)
        .map(|n| lower_expr(lo, *n))
        .unwrap_or_else(|| lo.empty_block(span));
    let alt = kids
        .get(2)
        .map(|n| lower_expr(lo, *n))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(NodeKind::If, Payload::None, span, &[cond, then, alt])
}

fn py_bin_op(text: &str) -> Option<Op> {
    Some(match text {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" | "@" => Op::Mul,
        "/" | "//" => Op::Div,
        "%" => Op::Mod,
        "**" => Op::Pow,
        "&" => Op::BitAnd,
        "|" => Op::BitOr,
        "^" => Op::BitXor,
        "<<" => Op::Shl,
        ">>" => Op::Shr,
        _ => return None,
    })
}

fn py_aug_op(text: &str) -> Option<Op> {
    py_bin_op(text.trim_end_matches('='))
}

/// Map a comparison operator string to `(op, negated)`. `not in` / `is not` carry the
/// negation (the caller wraps the comparison in `Not`).
fn py_cmp_op(text: &str) -> Option<(Op, bool)> {
    Some(match text {
        "==" => (Op::Eq, false),
        "!=" | "<>" => (Op::Ne, false),
        "<" => (Op::Lt, false),
        "<=" => (Op::Le, false),
        ">" => (Op::Gt, false),
        ">=" => (Op::Ge, false),
        // Membership is directional and non-commutative — its own op, so `a in b` ≠
        // `b in a` ≠ `a == b`. Identity (`is`) stays equality-shaped (identity ≈ equality
        // in a value model). `not in` / `is not` negate.
        "in" => (Op::In, false),
        "not in" => (Op::In, true),
        "is" => (Op::Eq, false),
        "is not" => (Op::Eq, true),
        _ => return None,
    })
}
