//! Rust → raw IL lowering.
//!
//! Convergence-friendly lowering: `?` and `.await` are stripped to their operand;
//! `&x`/`&mut x`/`*x` references peel to the operand; `x op= y` desugars to an
//! assignment; `for`/`while`/`loop` map to the unified `Loop`; `match` becomes an
//! `if`/`else if` chain (arm pattern as the condition); `if let`/`while let` keep
//! their scrutinee as the condition. `fn` items become function units; `impl`,
//! `trait`, `struct`, `enum` become class-like units so similar types cluster.

use crate::lower::Lowering;
use nose_il::{
    FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload, UnitKind,
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
        crate::lower::grammar::RUST,
        || tree_sitter_rust::LANGUAGE.into(),
        Lang::Rust,
        lower_items,
    )
}

fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}

/// Lower a top-level / module-level item.
fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_item" => Some(lower_func(lo, node, false)),
        "impl_item" | "trait_item" => Some(lower_type_block(lo, node, true)),
        "struct_item" | "enum_item" | "union_item" => Some(lower_type_block(lo, node, false)),
        "mod_item" => node.child_by_field_name("body").map(|b| lower_items(lo, b)),
        "const_item" | "static_item" => Some(lower_value_item(lo, node)),
        "use_declaration" | "extern_crate_declaration" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        // type aliases, macro defs, attributes: no behavior to model
        "type_item"
        | "macro_definition"
        | "attribute_item"
        | "inner_attribute_item"
        // trait method/const declarations without a body: no behavior to model
        | "function_signature_item"
        | "associated_type"
        | "line_comment"
        | "block_comment" => None,
        _ => lower_stmt(lo, node),
    }
}

fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim().trim_end_matches(';').trim();
    let path = text.strip_prefix("use ")?.trim();
    if path.contains('*') || path.contains('{') {
        return None;
    }
    let (path, local) = if let Some((path, local)) = path.split_once(" as ") {
        (path.trim(), local.trim())
    } else {
        let local = path.rsplit("::").next()?.trim();
        (path, local)
    };
    let (module, exported) = path.rsplit_once("::")?;
    Some(crate::lower::import_binding(
        lo,
        span,
        local,
        module.trim(),
        exported.trim(),
    ))
}

/// `impl`/`trait`/`struct`/`enum` → a `Class` unit whose body holds methods (each
/// also a unit) or field declarations, so similar types/impls cluster.
fn lower_type_block(lo: &mut Lowering, node: TsNode, methods: bool) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node.child_by_field_name("body");
    let mut kids = Vec::new();
    if let Some(body) = body {
        for c in Lowering::named_children(body) {
            if methods {
                if let Some(id) = lower_item(lo, c) {
                    kids.push(id);
                }
            } else if let Some(id) = lower_field_decl(lo, c) {
                kids.push(id);
            }
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    // Only `impl`/`trait` blocks (which carry behavior) are detection units. Pure
    // data-type definitions (struct/enum/union) are NOT unit-ified: they have no
    // call/control/value signal, so any two same-arity types look "similar" and
    // flood the candidate report with false positives (see docs/Dogfooding.md).
    if methods {
        lo.push_unit(block, UnitKind::Class, name);
    }
    block
}

/// A struct/enum field or enum variant → an `Assign(name, type-as-literal)` so the
/// shape of the data structure is captured.
fn lower_field_decl(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "field_declaration" | "enum_variant" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| lo.var(lo.text(n), span))
                .unwrap_or_else(|| lo.empty_block(span));
            let ty = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[]);
            Some(lo.add(NodeKind::Assign, Payload::None, span, &[name, ty]))
        }
        _ => None,
    }
}

fn lower_value_item(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let lhs = node
        .child_by_field_name("name")
        .map(|n| lo.var(lo.text(n), span))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    crate::lower::function_unit(lo, node, method, lower_params, lower_fn_body)
}

fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    for p in Lowering::named_children(params) {
        let span = lo.span(p);
        match p.kind() {
            "self_parameter" => out.push(lo.add(NodeKind::Param, Payload::None, span, &[])),
            "parameter" => {
                if let Some(pat) = p.child_by_field_name("pattern") {
                    push_pattern_params(lo, pat, out);
                } else {
                    out.push(lo.add(NodeKind::Param, Payload::None, span, &[]));
                }
            }
            // Closure params (`|a, v|`) are bare identifiers/patterns, not `parameter`
            // nodes — name them so a closure's body binds them (else a `.fold` closure's
            // accumulator/element are free vars and the fold never converges with a loop).
            _ => push_pattern_params(lo, p, out),
        }
    }
}

fn push_pattern_params(lo: &mut Lowering, pat: TsNode, out: &mut Vec<NodeId>) {
    match pat.kind() {
        "tuple_pattern" | "tuple_expression" => {
            for child in Lowering::named_children(pat) {
                push_pattern_params(lo, child, out);
            }
        }
        _ => {
            let span = lo.span(pat);
            match ident_of(lo, pat) {
                Some(sym) => out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[])),
                None => out.push(lo.add(NodeKind::Param, Payload::None, span, &[])),
            }
        }
    }
}

/// Extract the binding identifier from a (simple) pattern.
fn ident_of(lo: &Lowering, pat: TsNode) -> Option<nose_il::Symbol> {
    match pat.kind() {
        "identifier" | "type_identifier" | "field_identifier" => Some(lo.sym(lo.text(pat))),
        // `mut x`, `ref x`, `&x`, `x: T` — descend to the inner identifier
        "mut_pattern" | "ref_pattern" | "reference_pattern" => {
            pat.named_child(0).and_then(|c| ident_of(lo, c))
        }
        _ => pat.named_child(0).and_then(|c| ident_of(lo, c)),
    }
}

/// Lower a function body block, wrapping its tail expression in a `Return` — in
/// Rust the block's final expression *is* the return value, so this converges with
/// an explicit `return` (and with other languages' explicit returns).
fn lower_fn_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let children = Lowering::named_children(node);
    let n = children.len();
    let mut stmts = Vec::new();
    for (idx, child) in children.into_iter().enumerate() {
        let k = child.kind();
        if idx + 1 == n && is_tail_expr(k) {
            let e = lower_expr(lo, child);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[e]));
        } else if let Some(id) = lower_item(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}

/// A block's trailing expression (no semicolon, not a statement/item/comment).
fn is_tail_expr(k: &str) -> bool {
    !matches!(
        k,
        "expression_statement"
            | "let_declaration"
            | "empty_statement"
            | "line_comment"
            | "block_comment"
    ) && !is_item(k)
}

fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_item)
}

fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "let_declaration" => {
            let lhs = node
                .child_by_field_name("pattern")
                .and_then(|p| ident_of(lo, p))
                .map(|s| lo.add(NodeKind::Var, Payload::Name(s), span, &[]))
                .unwrap_or_else(|| lo.empty_block(span));
            let rhs = node
                .child_by_field_name("value")
                .map(|v| lower_expr(lo, v))
                .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
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

fn is_item(k: &str) -> bool {
    matches!(
        k,
        "function_item" | "impl_item" | "trait_item" | "struct_item" | "enum_item" | "mod_item"
    )
}

fn is_control_expr(k: &str) -> bool {
    matches!(
        k,
        "if_expression"
            | "match_expression"
            | "for_expression"
            | "while_expression"
            | "loop_expression"
    )
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "type_identifier" | "field_identifier" | "scoped_identifier" => {
            lo.var(lo.text(node), span)
        }
        "self" => lo.var("self", span),
        "integer_literal" => {
            let t = lo.text(node);
            lo.int_lit(t.trim_end_matches(|c: char| c.is_alphabetic()), span)
        }
        "float_literal" => lo.float_lit(lo.text(node), span),
        "string_literal" | "raw_string_literal" | "char_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "boolean_literal" => {
            let b = lo.text(node) == "true";
            lo.add(NodeKind::Lit, Payload::LitBool(b), span, &[])
        }
        "unit_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "block" => lower_block(lo, node),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => lower_unary(lo, node),
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr),
        "compound_assignment_expr" => lower_compound_assign(lo, node),
        // peel try / await / paren wrappers to their operand
        "try_expression" | "await_expression" | "parenthesized_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `&x` / `&mut x` → the referenced value (skip the mutable_specifier)
        "reference_expression" => node
            .child_by_field_name("value")
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `x as T` → `x` (a cast is type-level; erase it, like TS `as`)
        "type_cast_expression" => node
            .child_by_field_name("value")
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `a..b` / `a..=b` / `a..` → a sequence of its endpoints
        "range_expression" => {
            // Preserve start/end POSITIONS and inclusivity: `1..`, `..1`, `1..2`,
            // `1..=2` are all different. tree-sitter omits empty bounds and the
            // `..`/`..=` operator is anonymous, so collecting named children collapsed
            // `1..` and `..1` to `Seq(1)`. Split on the operator; emit a `None`
            // placeholder for each empty slot and a trailing `0`/`1` inclusivity flag.
            let mut start: Option<NodeId> = None;
            let mut end: Option<NodeId> = None;
            let mut inclusive = false;
            let mut seen_op = false;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    ".." => seen_op = true,
                    "..=" | "..." => {
                        seen_op = true;
                        inclusive = true;
                    }
                    _ if child.is_named() => {
                        let v = lower_expr(lo, child);
                        if seen_op {
                            end = Some(v);
                        } else {
                            start = Some(v);
                        }
                    }
                    _ => {}
                }
            }
            let none =
                |lo: &mut Lowering| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]);
            let s = start.unwrap_or_else(|| none(lo));
            let e = end.unwrap_or_else(|| none(lo));
            let flag = lo.int_lit(if inclusive { "1" } else { "0" }, span);
            lo.add(NodeKind::Seq, Payload::None, span, &[s, e, flag])
        }
        "call_expression" => lower_call(lo, node),
        "macro_invocation" => lower_macro(lo, node),
        "method_call_expression" => lower_method_call(lo, node),
        "field_expression" => lower_field(lo, node),
        "index_expression" => lower_index(lo, node),
        "closure_expression" => lower_closure(lo, node),
        "if_expression" => lower_if(lo, node),
        "match_expression" => lower_match(lo, node),
        "for_expression" => lower_for(lo, node),
        "while_expression" => lower_while(lo, node),
        "loop_expression" => lower_loop(lo, node),
        "return_expression" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            lo.add(NodeKind::Return, Payload::None, span, &kids)
        }
        "break_expression" => lo.add(NodeKind::Break, Payload::None, span, &[]),
        "continue_expression" => lo.add(NodeKind::Continue, Payload::None, span, &[]),
        "array_expression" | "tuple_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym(node.kind());
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "struct_expression" => lower_struct_expr(lo, node),
        // `foo::<T>` / `Vec::<T>::new` — lower the function/value, drop the turbofish.
        "generic_function" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `unsafe { … }` is just its block.
        "unsafe_block" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type-level nodes carry no runtime behavior — erase (don't Raw). These reach
        // expression position via turbofish, casts, and closure/fn param subtrees.
        "type_arguments"
        | "type_parameters"
        | "reference_type"
        | "primitive_type"
        | "generic_type"
        | "scoped_type_identifier"
        | "array_type"
        | "tuple_type"
        | "pointer_type"
        | "dynamic_type"
        | "lifetime"
        | "parameters"
        | "parameter"
        | "self_parameter"
        | "function_signature_item"
        | "where_clause"
        | "type_arguments_list"
        | "trait_bounds"
        | "type_binding"
        | "constrained_type_parameter" => lo.empty_block(span),
        "macro_definition" | "line_comment" | "block_comment" | "attribute_item" => {
            lo.empty_block(span)
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

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, rust_bin_op, lower_expr)
}

fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .named_child(0)
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    // tree-sitter-rust unary_expression: the operator is an anonymous child
    // (`-`, `!`, or `*`). A dereference `*x` is reference-level — other languages
    // don't have it — so peel it to its operand, like `&x` (reference_expression),
    // so `*x > 0` converges with a plain `x > 0`.
    let txt = lo.text(node);
    if txt.starts_with('*') {
        return operand;
    }
    let op = if txt.starts_with('!') {
        Op::Not
    } else {
        Op::Neg
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}

/// `x op= y` → `x = x op y`.
fn lower_compound_assign(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o).trim_end_matches('='))
        .and_then(rust_bin_op)
        .unwrap_or(Op::Add);
    let lhs1 = left
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let lhs2 = left
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = right
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    let binop = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[lhs2, rhs]);
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs1, binop])
}

fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("function") {
        Some(f) => kids.push(lower_expr(lo, f)),
        None => kids.push(lo.empty_block(span)),
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

/// `recv.method(args)` → `Call(Field(method, recv), args...)`, matching how the
/// JS/Python frontends model method calls (so `.map`/`.filter` etc. canonicalize).
fn lower_method_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let recv = node
        .child_by_field_name("receiver")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    let method = node
        .child_by_field_name("method")
        .map(|m| lo.sym(lo.text(m)));
    let callee = lo.add(
        NodeKind::Field,
        method.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[recv],
    );
    let mut kids = vec![callee];
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

/// `name!(args)` → `Call(name, args...)` (best-effort; macro tokens that don't
/// parse as expressions fall back to Raw children).
fn lower_macro(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node
        .child_by_field_name("macro")
        .map(|m| lo.sym(lo.text(m)));
    let callee = lo.add(
        NodeKind::Var,
        name.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[],
    );
    let mut kids = vec![callee];
    for c in Lowering::named_children(node) {
        if c.kind() == "token_tree" {
            collect_macro_atoms(lo, c, &mut kids);
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

/// Macro arguments are an unparsed token stream: nested `()`/`[]`/`{}` are sub-
/// `token_tree`s, not expressions. Recurse through them collecting only real atoms
/// (names, literals) as call args — skipping delimiters/punctuation — so a macro
/// never leaves `Raw` token_tree nodes that would corrupt the value graph.
fn collect_macro_atoms(lo: &mut Lowering, tt: TsNode, kids: &mut Vec<NodeId>) {
    for t in Lowering::named_children(tt) {
        if t.kind() == "token_tree" {
            collect_macro_atoms(lo, t, kids);
        } else if is_macro_atom(t.kind()) {
            kids.push(lower_expr(lo, t));
        }
        // else: an unparsed token with no behavioral signal — drop it.
    }
}

fn is_macro_atom(k: &str) -> bool {
    matches!(
        k,
        "identifier"
            | "scoped_identifier"
            | "field_identifier"
            | "self"
            | "integer_literal"
            | "float_literal"
            | "string_literal"
            | "raw_string_literal"
            | "char_literal"
            | "boolean_literal"
    )
}

fn lower_field(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let base = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let field = node
        .child_by_field_name("field")
        .map(|f| lo.sym(lo.text(f)));
    lo.add(
        NodeKind::Field,
        field.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[base],
    )
}

fn lower_index(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    lo.add(NodeKind::Index, Payload::None, span, &kids)
}

fn lower_closure(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_expr(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
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

/// `if let PAT = expr` / `let PAT = expr` condition → use the scrutinee expression
/// as the condition (the pattern binding is irrelevant to behavioral shape).
fn lower_cond(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "let_condition" | "let_chain" => node
            .child_by_field_name("value")
            .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)))
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lower_expr(lo, node)),
        _ => lower_expr(lo, node),
    }
}

/// `match e { p1 => b1, p2 => b2, … }` → nested `if`/`else` chain, each arm's
/// pattern lowered as a comparison-ish condition (approximate, but converges with
/// equivalent switch/if-chains in other languages).
fn lower_match(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let arms: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter(|c| c.kind() == "match_arm")
                .collect()
        })
        .unwrap_or_default();
    // build from the last arm backwards into nested Ifs
    let mut acc = lo.empty_block(span);
    for arm in arms.iter().rev() {
        let body = arm
            .child_by_field_name("value")
            .map(|v| {
                if v.kind() == "block" {
                    lower_block(lo, v)
                } else {
                    lower_expr(lo, v)
                }
            })
            .unwrap_or_else(|| lo.empty_block(span));
        // condition: scrutinee compared against the arm pattern (use scrutinee value
        // as a proxy — the discriminating structure is the arm bodies).
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[scrutinee, scrutinee],
        );
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
    }
    acc
}

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_cond, lower_block)
}

fn lower_loop(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_struct_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter_map(|f| f.child_by_field_name("value").map(|v| lower_expr(lo, v)))
                .collect()
        })
        .unwrap_or_default();
    lo.add(NodeKind::Seq, Payload::None, span, &kids)
}

fn rust_bin_op(text: &str) -> Option<Op> {
    // Rust's binary operators are exactly the shared C-family set.
    crate::lower::common_bin_op(text)
}
