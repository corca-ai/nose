//! Ruby → raw IL lowering.
//!
//! Convergence-friendly lowering: `def` → function unit, `class`/`module` →
//! class-like unit; `if`/`unless`/`while`/`until`/`case` map to `If`/`Loop`;
//! `for x in xs` maps to a `ForEach` loop; `x op= y` desugars to an assignment;
//! method calls → `Field`-call form. Ruby's implicit
//! last-expression return is wrapped in `Return` to converge with explicit returns.

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload, Span, Symbol,
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
        crate::lower::grammar::RUBY,
        || tree_sitter_ruby::LANGUAGE.into(),
        Lang::Ruby,
        |lo, root| crate::lower::collect_into(lo, root, NodeKind::Module, lower_stmt),
    )
}

fn block_of(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_stmt)
}

/// The statement body of a `{ |x| … }` / `do |x| … end` block — its `body`
/// field's statements only (excluding the `parameters` node).
fn block_body(lo: &mut Lowering, block: TsNode) -> NodeId {
    let span = lo.span(block);
    match block.child_by_field_name("body") {
        Some(b) => block_of(lo, b),
        // some grammars inline statements directly under the block
        None => {
            let stmts: Vec<NodeId> = Lowering::named_children(block)
                .into_iter()
                .filter(|c| c.kind() != "block_parameters")
                .filter_map(|c| lower_stmt(lo, c))
                .collect();
            lo.add(NodeKind::Block, Payload::None, span, &stmts)
        }
    }
}

/// A method/lambda body: wrap the trailing expression in `Return` (Ruby's implicit
/// return), so it converges with explicit-return languages.
fn body_with_return(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let children = Lowering::named_children(node);
    let n = children.len();
    let mut stmts = Vec::new();
    for (idx, c) in children.into_iter().enumerate() {
        if idx + 1 == n && is_tail_expr(c.kind()) {
            let e = lower_expr(lo, c);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(c), &[e]));
        } else if let Some(id) = lower_stmt(lo, c) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}

fn is_tail_expr(k: &str) -> bool {
    !matches!(
        k,
        "comment" | "return" | "if" | "unless" | "while" | "until" | "case" | "for"
    )
}

fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "method" | "singleton_method" => Some(lower_method(lo, node)),
        "class" | "module" | "singleton_class" => Some(lower_class(lo, node)),
        "comment" | "call" if node.kind() == "comment" => None,
        "if" | "unless" => Some(lower_if(lo, node)),
        "while" | "until" => Some(lower_while(lo, node)),
        "for" => Some(lower_for(lo, node)),
        "case" => Some(lower_case(lo, node)),
        "return" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_return_value(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "break" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "next" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        // `begin … rescue … ensure … end` → Try (body + handler/ensure blocks).
        "begin" | "do" => Some(lower_begin(lo, node)),
        // `alias new old` carries no behavior to dedupe.
        "alias" | "undef" => None,
        // Guard-clause modifiers: `stmt if cond` / `stmt unless cond` → `If` so they
        // converge with the block forms and other languages' guards.
        "if_modifier" | "unless_modifier" => Some(lower_modifier(lo, node)),
        "assignment" | "operator_assignment" => Some(lower_assign(lo, node)),
        "call" | "method_call" => {
            let e = lower_call(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

fn lower_method(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        for p in Lowering::named_children(params) {
            let pspan = lo.span(p);
            let sym = param_name(lo, p);
            kids.push(lo.add(
                NodeKind::Param,
                sym.map(Payload::Name).unwrap_or(Payload::None),
                pspan,
                &[],
            ));
        }
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| body_with_return(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(func, UnitKind::Method, name);
    func
}

fn param_name(lo: &Lowering, p: TsNode) -> Option<Symbol> {
    match p.kind() {
        "identifier" => Some(lo.sym(lo.text(p))),
        _ => p.named_child(0).map(|n| lo.sym(lo.text(n))),
    }
}

fn lower_return_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "argument_list" && node.named_child_count() == 1 {
        if let Some(v) = node.named_child(0) {
            return lower_expr(lo, v);
        }
    }
    lower_expr(lo, node)
}

fn lower_class(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node
        .child_by_field_name("body")
        .map(|b| block_of(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.push_unit(body, UnitKind::Class, name);
    body
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| block_of(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        // `else`/`elsif` clause
        let e = if alt.kind() == "else" {
            block_of(lo, alt)
        } else {
            lower_if(lo, alt)
        };
        kids.push(e);
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, block_of)
}

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("pattern")
        .map(|p| lower_expr(lo, p))
        .unwrap_or_else(|| lo.empty_block(span));
    // tree-sitter-ruby wraps the iterable in an `in` node (`for x in xs` →
    // value: (in (identifier))); lower the wrapped expression, not the wrapper,
    // or the iterable becomes an exact-unsafe `Raw("in")`.
    let iter = node
        .child_by_field_name("value")
        .map(|v| {
            let target = if v.kind() == "in" {
                Lowering::named_children(v).into_iter().next().unwrap_or(v)
            } else {
                v
            };
            lower_expr(lo, target)
        })
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| block_of(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}

/// `case x when a … when b …` → an `if`/`else` chain (the bodies are the signal).
fn lower_case(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node.child_by_field_name("value").map(|v| lower_expr(lo, v));
    let whens: Vec<TsNode> = Lowering::named_children(node)
        .into_iter()
        .filter(|c| c.kind() == "when" || c.kind() == "else")
        .collect();
    let mut acc = lo.empty_block(span);
    for w in whens.iter().rev() {
        let body = lower_case_arm_body(lo, *w, span);
        if w.kind() == "else" {
            acc = body;
        } else {
            // `when p1, p2 …` → `scrutinee == p1 || scrutinee == p2 || …`. Each `when`
            // carries one or more `pattern` fields wrapping the match expression; lower
            // those so the pattern's computation is in the IL (previously the condition
            // was `scrutinee == scrutinee`, dropping the patterns entirely).
            let cmps: Vec<NodeId> = Lowering::named_children(*w)
                .into_iter()
                .filter(|c| c.kind() == "pattern")
                .map(|p| {
                    let pv = p
                        .named_child(0)
                        .map(|e| lower_expr(lo, e))
                        .unwrap_or_else(|| lo.empty_block(span));
                    match scrutinee {
                        Some(subject) => {
                            lo.add(NodeKind::BinOp, Payload::Op(Op::Eq), span, &[subject, pv])
                        }
                        None => pv,
                    }
                })
                .collect();
            let cond = cmps
                .into_iter()
                .reduce(|a, b| lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[a, b]))
                .unwrap_or_else(|| match scrutinee {
                    Some(subject) => lo.add(
                        NodeKind::BinOp,
                        Payload::Op(Op::Eq),
                        span,
                        &[subject, subject],
                    ),
                    None => lo.empty_block(span),
                });
            acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
        }
    }
    acc
}

fn lower_case_arm_body(lo: &mut Lowering, arm: TsNode, span: Span) -> NodeId {
    arm.child_by_field_name("body")
        .map(|body| block_of(lo, body))
        .unwrap_or_else(|| {
            if arm.kind() == "else" {
                lower_clause_body(lo, arm)
            } else {
                lo.empty_block(span)
            }
        })
}

fn lower_assign(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    let r = node
        .child_by_field_name("right")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    if node.kind() == "operator_assignment" {
        let opt = node
            .child_by_field_name("operator")
            .map(|o| lo.text(o))
            .unwrap_or("+=");
        let l2 = node
            .child_by_field_name("left")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        // An unmapped compound operator keeps its own raw shape — dropping the
        // operator would merge it with `x = y`.
        let value = match common_bin_op(opt.trim_end_matches('=')) {
            Some(op) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l2, r]),
            None => lo.raw(&format!("compound_assignment {opt}"), span, &[l2, r]),
        };
        return lo.add(NodeKind::Assign, Payload::None, span, &[l, value]);
    }
    lo.add(NodeKind::Assign, Payload::None, span, &[l, r])
}

/// Lower a string. A plain string is a value-retaining `LitStr`; an interpolated
/// string (`"hi #{name}"`) lowers to a string-concat chain — a base `Str` literal
/// then `Add` of each `#{…}` expression — converging with a JS template / f-string.
fn lower_string(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let interps: Vec<TsNode> = Lowering::named_children(node)
        .into_iter()
        .filter(|c| c.kind() == "interpolation")
        .collect();
    if interps.is_empty() {
        let text = lo.text(node);
        if matches!(node.kind(), "symbol" | "simple_symbol" | "hash_key_symbol") {
            return lo.str_lit(text.trim_start_matches(':').trim_end_matches(':'), span);
        }
        return lo.str_lit(text, span);
    }
    let mut acc = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Str), span, &[]);
    for interp in interps {
        if let Some(e) = interp.named_child(0) {
            let sub = lower_expr(lo, e);
            acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Add), span, &[acc, sub]);
        }
    }
    acc
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "constant" | "instance_variable" | "global_variable" | "self" => {
            lo.var(lo.text(node), span)
        }
        "integer" => lo.int_lit(lo.text(node), span),
        "float" => lo.float_lit(lo.text(node), span),
        // Ruby symbols are atoms (`:foo`, and the `foo:` key in `{foo: 1}`); lower as
        // string literals so their value participates in matching like any constant.
        // Heredocs (`<<~SQL … SQL`) are multi-line strings: tree-sitter splits them
        // into a `heredoc_beginning` (in value position) and a dangling `heredoc_body`
        // (content + `#{…}` interpolations) — both lower like any interpolated string.
        "string" | "bare_string" | "symbol" | "simple_symbol" | "hash_key_symbol"
        | "string_array" | "symbol_array" | "heredoc_beginning" | "heredoc_body" => {
            lower_string(lo, node)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary" => lower_binary(lo, node),
        "unary" => lower_unary(lo, node),
        "assignment" | "operator_assignment" => lower_assign(lo, node),
        "method_call" | "call" => lower_call(lo, node),
        "element_reference" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "array" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym("array");
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "hash" => lower_hash(lo, node),
        "pair" => lower_hash_pair(lo, node),
        "block" | "do_block" => lower_block_lambda(lo, node),
        "parenthesized_statements" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "if" | "unless" => lower_if(lo, node),
        // A guard modifier in expression/tail position (Ruby's implicit return lowers
        // the last statement as an expr) lowers the same as in statement position.
        "if_modifier" | "unless_modifier" => lower_modifier(lo, node),
        // Ternary `c ? a : b` → `If` (converges with if-expressions elsewhere).
        "conditional" | "ternary" => {
            let kids: Vec<NodeId> = ["condition", "consequence", "alternative"]
                .iter()
                .filter_map(|f| node.child_by_field_name(f))
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::If, Payload::None, span, &kids)
        }
        // Adjacent string literals (`"a" "b"`) concatenate to one string.
        "chained_string" => lower_string(lo, node),
        // `*args` / `&blk` / `**kw` argument forms — lower the wrapped expression.
        "splat_argument" | "block_argument" | "hash_splat_argument" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `Foo::Bar` — a qualified name; treat as one Var atom (robust to nesting).
        "scope_resolution" => lo.var(lo.text(node), span),
        // A regex literal is a constant.
        "regex" => lo.str_lit(lo.text(node), span),
        // `begin … rescue … end` as an expression (e.g. RHS of an assignment).
        "begin" | "do" => lower_begin(lo, node),
        // Argument/assignment-target lists and ranges → a sequence of their elements.
        "argument_list" | "left_assignment_list" | "right_assignment_list" | "range" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `yield x` — the yielded values.
        "yield" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        "super" | "forward_argument" => lo.var(lo.text(node), span),
        _ => raw_kids(lo, node),
    }
}

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let l = node.child_by_field_name("left").map(|x| lower_expr(lo, x));
    let r = node.child_by_field_name("right").map(|x| lower_expr(lo, x));
    let op = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .and_then(|t| match t {
            // Ruby `%` is FLOORED (remainder takes the divisor's sign), unlike the
            // C-family truncated `%` in `common_bin_op` (#283-D).
            "%" => Some(Op::FloorMod),
            // Ruby integer `/` is FLOORED (`-7 / 2 == -4`), like Python `//` — distinct
            // from C-family truncated `Op::Div` and Python/JS true-float `Op::TrueDiv`
            // (#283-D).
            "/" => Some(Op::FloorDiv),
            other => common_bin_op(other),
        });
    match (l, r, op) {
        (Some(l), Some(r), Some(op)) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r]),
        _ => raw_kids(lo, node),
    }
}

fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .named_child(node.named_child_count().saturating_sub(1))
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    // Map by the operator token, not the leading byte: `+`→Pos, `-`→Neg,
    // `~`→BitNot, `!`/`not`→Not. Reading only the first byte collapsed `+5`
    // and `~5` onto `Neg`.
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("+") => Op::Pos,
        Some("~") => Op::BitNot,
        Some("!") | Some("not") => Op::Not,
        _ => Op::Neg,
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}

fn lower_block_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_block_lambda_with_unit(lo, node, None)
}

fn lower_block_lambda_with_unit(
    lo: &mut Lowering,
    node: TsNode,
    block_unit_name: Option<Symbol>,
) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        for p in Lowering::named_children(params) {
            let pspan = lo.span(p);
            let sym = param_name(lo, p);
            kids.push(lo.add(
                NodeKind::Param,
                sym.map(Payload::Name).unwrap_or(Payload::None),
                pspan,
                &[],
            ));
        }
    }
    let body = block_body(lo, node);
    if let Some(name) = block_unit_name {
        lo.push_unit(body, UnitKind::Block, Some(name));
    }
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}

fn is_test_dsl_method(method: &str) -> bool {
    matches!(
        method,
        "test"
            | "it"
            | "specify"
            | "example"
            | "describe"
            | "context"
            | "feature"
            | "scenario"
            | "shared_examples"
            | "shared_examples_for"
            | "shared_context"
            | "before"
            | "after"
            | "around"
            | "setup"
            | "teardown"
    )
}

fn test_dsl_block_unit_name(lo: &Lowering, call: TsNode, method: &str) -> Symbol {
    let label = call
        .child_by_field_name("arguments")
        .and_then(|args| {
            Lowering::named_children(args)
                .into_iter()
                .find_map(|arg| test_dsl_literal_label(lo, arg))
        })
        .unwrap_or_else(|| lo.span(call).start_line.to_string());
    lo.sym(&format!("{method}:{label}"))
}

fn test_dsl_literal_label(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "string" | "bare_string" | "symbol" | "simple_symbol" | "hash_key_symbol" => {
            Some(trim_ruby_label_literal(lo.text(node)).to_string())
        }
        _ => None,
    }
}

fn trim_ruby_label_literal(text: &str) -> &str {
    let trimmed = text.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    if quoted && trimmed.len() >= 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed.trim_start_matches(':').trim_end_matches(':')
    }
}

fn lower_hash(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    for child in Lowering::named_children(node) {
        match child.kind() {
            "pair" => kids.push(lower_hash_pair(lo, child)),
            "hash_splat_argument" => {
                let inner: Vec<NodeId> = Lowering::named_children(child)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect();
                kids.push(lo.raw(child.kind(), lo.span(child), &inner));
            }
            _ => kids.push(lower_expr(lo, child)),
        }
    }
    let tag = lo.sym("hash");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}

fn lower_hash_pair(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    let tag = lo.sym("pair");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}

/// `begin … rescue … ensure … else … end` → `Try(body, handler-blocks…)`, converging
/// with try/catch in other languages. Exception-type lists are skipped (data, not behavior).
fn lower_begin(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut body = Vec::new();
    let mut handlers = Vec::new();
    for c in Lowering::named_children(node) {
        match c.kind() {
            "rescue" | "ensure" => handlers.push(lower_clause_body(lo, c)),
            // `else` runs exactly when the body completes without raising — the
            // success path's tail, not a handler. Handler position let the
            // no-throw fingerprint convention erase it entirely (#210).
            "else" => {
                for s in Lowering::named_children(c) {
                    if matches!(s.kind(), "exceptions" | "exception_variable" | "then") {
                        continue;
                    }
                    if let Some(id) = lower_stmt(lo, s) {
                        body.push(id);
                    }
                }
            }
            _ => {
                if let Some(s) = lower_stmt(lo, c) {
                    body.push(s);
                }
            }
        }
    }
    let body_block = lo.add(NodeKind::Block, Payload::None, span, &body);
    let mut kids = vec![body_block];
    kids.extend(handlers);
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}

/// A `rescue`/`ensure`/`else` clause → a `Block` of its statements (its exception-type
/// list and `then` keyword carry no behavior and are skipped).
fn lower_clause_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for c in Lowering::named_children(node) {
        if matches!(c.kind(), "exceptions" | "exception_variable" | "then") {
            continue;
        }
        if let Some(s) = lower_stmt(lo, c) {
            stmts.push(s);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}

/// `body if cond` / `body unless cond` → `If(cond, Block[body])`, matching the block
/// `if`/`unless` form. Used from both statement and expression (tail) position.
fn lower_modifier(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = node
        .child_by_field_name("body")
        .and_then(|b| lower_stmt(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = lo.add(NodeKind::Block, Payload::None, span, &[body]);
    let mut cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    if node.kind() == "unless_modifier" {
        cond = lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[cond]);
    }
    lo.add(NodeKind::If, Payload::None, span, &[cond, then])
}

fn raw_kids(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    lo.raw(node.kind(), span, &kids)
}

fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let method_name = node
        .child_by_field_name("method")
        .map(|m| lo.text(m).to_string());
    let method = method_name.as_deref().map(|m| lo.sym(m));
    let recv = node.child_by_field_name("receiver");
    let block = Lowering::named_children(node)
        .into_iter()
        .find(|c| matches!(c.kind(), "block" | "do_block"));

    let callee = match recv {
        Some(r) => {
            let base = lower_expr(lo, r);
            lo.add(
                NodeKind::Field,
                method.map(Payload::Name).unwrap_or(Payload::None),
                span,
                &[base],
            )
        }
        None => lo.add(
            NodeKind::Var,
            method.map(Payload::Name).unwrap_or(Payload::None),
            span,
            &[],
        ),
    };
    let mut kids = vec![callee];
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    if let Some(b) = block {
        let block_expr = if method_name.as_deref().is_some_and(is_test_dsl_method) {
            let name = test_dsl_block_unit_name(lo, node, method_name.as_deref().unwrap());
            lower_block_lambda_with_unit(lo, b, Some(name))
        } else {
            lower_block_lambda(lo, b)
        };
        kids.push(block_expr);
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(src: &str) -> Vec<nose_il::Node> {
        let interner = Interner::new();
        lower(FileId(0), "t.rb", src.as_bytes(), &interner)
            .expect("lower")
            .nodes
    }

    fn unit_names(src: &str) -> Vec<(UnitKind, String)> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
        il.units
            .iter()
            .map(|unit| {
                (
                    unit.kind,
                    unit.name
                        .map(|name| interner.resolve(name).to_string())
                        .unwrap_or_else(|| "-".to_string()),
                )
            })
            .collect()
    }

    fn unary_ops(src: &str) -> Vec<Op> {
        nodes(src)
            .iter()
            .filter(|n| n.kind == NodeKind::UnOp)
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn binary_ops(src: &str) -> Vec<Op> {
        nodes(src)
            .iter()
            .filter(|n| n.kind == NodeKind::BinOp)
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn expr_stmt_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.rb", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::ExprStmt)
            .filter_map(|(idx, _)| {
                let kids = il.children(NodeId(idx as u32));
                match kids {
                    [expr] => match il.node(*expr).payload {
                        Payload::LitInt(value) => Some(value),
                        _ => None,
                    },
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn unary_operators_lower_to_distinct_ops() {
        let ops = unary_ops("x = +5\ny = -5\nz = !a\nw = ~5\n");
        assert!(ops.contains(&Op::Pos), "unary + → Op::Pos, got {ops:?}");
        assert!(ops.contains(&Op::Neg), "unary - → Op::Neg, got {ops:?}");
        assert!(ops.contains(&Op::Not), "unary ! → Op::Not, got {ops:?}");
        assert!(
            ops.contains(&Op::BitNot),
            "unary ~ → Op::BitNot, got {ops:?}"
        );
    }

    #[test]
    fn keyword_not_lowers_to_not() {
        assert_eq!(unary_ops("y = not a\n"), vec![Op::Not]);
    }

    #[test]
    fn case_when_compares_scrutinee_against_pattern() {
        // `case x when 7 ...` must lower a comparison of the scrutinee against the
        // pattern `7`; previously the pattern was dropped (cond was `x == x`), so the
        // literal 7 never appeared in the IL.
        let has_seven = nodes("case x\nwhen 7\n  y\nend\n")
            .iter()
            .any(|n| matches!(n.payload, Payload::LitInt(7)));
        assert!(
            has_seven,
            "the `when 7` pattern literal must appear in the lowered IL"
        );
    }

    #[test]
    fn scrutinee_less_case_uses_when_condition_directly() {
        let ops = binary_ops("case\nwhen x > 0\n  y\nelse\n  z\nend\n");
        assert!(
            ops.contains(&Op::Gt),
            "scrutinee-less case should keep the when predicate, got {ops:?}"
        );
        assert!(
            !ops.contains(&Op::Eq),
            "scrutinee-less case should not compare an empty scrutinee, got {ops:?}"
        );
    }

    #[test]
    fn case_else_body_is_preserved() {
        let mut ints = expr_stmt_ints("case\nwhen x > 0\n  1\nelse\n  2\nend\n");
        ints.sort_unstable();
        assert_eq!(ints, vec![1, 2]);
    }

    #[test]
    fn test_dsl_block_calls_are_units() {
        let units = unit_names(
            r#"
test 'renders table' do
  assert_equal 1, result
end

RSpec.describe 'Widget' do
  it 'renders value' do
    expect(result).to eq(1)
  end
end

items.each do |item|
  puts item
end
"#,
        );
        assert!(
            units.contains(&(UnitKind::Block, "test:renders table".to_string())),
            "Minitest-style test blocks should be block units: {units:?}"
        );
        assert!(
            units.contains(&(UnitKind::Block, "describe:Widget".to_string())),
            "RSpec describe blocks should be block units: {units:?}"
        );
        assert!(
            units.contains(&(UnitKind::Block, "it:renders value".to_string())),
            "RSpec it blocks should be block units: {units:?}"
        );
        assert!(
            !units
                .iter()
                .any(|(kind, name)| *kind == UnitKind::Block && name.starts_with("each:")),
            "generic Ruby block calls must not become DSL units: {units:?}"
        );
    }
}
