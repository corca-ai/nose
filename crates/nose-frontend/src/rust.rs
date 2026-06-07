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
    Builtin, FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload, Span,
    Symbol, UnitKind,
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
        "mod_item" => Some(lower_mod_item(lo, node)),
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
    let name = rust_item_name(lo, node);
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
    let payload = name.map(Payload::Name).unwrap_or(Payload::None);
    let block = lo.add(NodeKind::Block, payload, span, &kids);
    // Only `impl`/`trait` blocks (which carry behavior) are detection units. Pure
    // data-type definitions (struct/enum/union) are NOT unit-ified: they have no
    // call/control/value signal, so any two same-arity types look "similar" and
    // flood the candidate report with false positives (see docs/dogfooding.md).
    if methods {
        lo.push_unit(block, UnitKind::Class, name);
    }
    block
}

fn lower_mod_item(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let payload = rust_item_name(lo, node)
        .map(Payload::Name)
        .unwrap_or(Payload::None);
    let kids = node
        .child_by_field_name("body")
        .map(|body| {
            Lowering::named_children(body)
                .into_iter()
                .filter_map(|child| lower_item(lo, child))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    lo.add(NodeKind::Module, payload, span, &kids)
}

fn rust_item_name(lo: &mut Lowering, node: TsNode) -> Option<Symbol> {
    node.child_by_field_name("name")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
        })
        .map(|n| lo.sym(lo.text(n)))
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
                    let semantic_text = p.child_by_field_name("type").map(|ty| lo.text(ty));
                    if let Some(semantic) = crate::lower::param_semantic_from_text(
                        semantic_text.unwrap_or_else(|| lo.text(p)),
                    ) {
                        if let Some(sym) = ident_of(lo, pat) {
                            let pspan = lo.span(pat);
                            lo.record_param_semantic(pspan, semantic);
                            out.push(lo.add(NodeKind::Param, Payload::Name(sym), pspan, &[]));
                            continue;
                        }
                    }
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
fn ident_of(lo: &Lowering, pat: TsNode) -> Option<Symbol> {
    match pat.kind() {
        "identifier" | "type_identifier" | "field_identifier" => Some(lo.sym(lo.text(pat))),
        // `mut x`, `ref x`, `&x`, `x: T` — descend to the inner identifier
        "mut_pattern" | "ref_pattern" | "reference_pattern" => {
            pat.named_child(0).and_then(|c| ident_of(lo, c))
        }
        _ => pat.named_child(0).and_then(|c| ident_of(lo, c)),
    }
}

fn lower_static_projection_pattern(
    lo: &mut Lowering,
    pattern: TsNode,
    base: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    if pattern.kind() != "struct_pattern" {
        return None;
    }
    let mut assigns = Vec::new();
    for child in Lowering::named_children(pattern) {
        match child.kind() {
            "type_identifier" | "scoped_type_identifier" | "qualified_type" => {}
            "remaining_field_pattern" => {}
            "field_pattern" => {
                let (field, local) = rust_field_projection(lo, child)?;
                assigns.push(rust_projection_assign(lo, base, &field, &local, span));
            }
            "shorthand_field_identifier_pattern" | "field_identifier" => {
                let name = lo.text(child).to_string();
                assigns.push(rust_projection_assign(lo, base, &name, &name, span));
            }
            _ => return rust_struct_pattern_text_projection(lo, pattern, base, span),
        }
    }
    if assigns.is_empty() {
        rust_struct_pattern_text_projection(lo, pattern, base, span)
    } else {
        Some(assigns)
    }
}

fn rust_field_projection(lo: &Lowering, node: TsNode) -> Option<(String, String)> {
    let kids = Lowering::named_children(node);
    let field = kids.first().and_then(|&k| rust_field_name(lo, k))?;
    let local = kids
        .iter()
        .skip(1)
        .find_map(|&k| rust_binding_name(lo, k))
        .unwrap_or_else(|| field.clone());
    Some((field, local))
}

fn rust_field_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "field_identifier" | "identifier" => Some(lo.text(node).to_string()),
        _ => None,
    }
}

fn rust_binding_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" => Some(lo.text(node).to_string()),
        "mut_pattern" | "ref_pattern" | "reference_pattern" => {
            node.named_child(0).and_then(|n| rust_binding_name(lo, n))
        }
        _ => None,
    }
}

fn rust_projection_assign(
    lo: &mut Lowering,
    base: NodeId,
    field: &str,
    local: &str,
    span: Span,
) -> NodeId {
    let lhs = lo.var(local, span);
    let sym = lo.sym(field);
    let rhs = lo.add(NodeKind::Field, Payload::Name(sym), span, &[base]);
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

fn rust_struct_pattern_text_projection(
    lo: &mut Lowering,
    pattern: TsNode,
    base: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    let text = lo.text(pattern);
    let open = text.find('{')?;
    let close = text.rfind('}')?;
    if close <= open {
        return None;
    }
    let mut assigns = Vec::new();
    for part in text[open + 1..close].split(',') {
        let part = part.trim();
        if part.is_empty() || part == ".." {
            continue;
        }
        let (field, local) = match part.split_once(':') {
            Some((field, local)) => {
                let field = field.trim();
                let local = local.trim();
                if !simple_rust_ident(field) || !simple_rust_ident(local) {
                    return None;
                }
                (field, local)
            }
            None => {
                if !simple_rust_ident(part) {
                    return None;
                }
                (part, part)
            }
        };
        assigns.push(rust_projection_assign(lo, base, field, local, span));
    }
    (!assigns.is_empty()).then_some(assigns)
}

fn simple_rust_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
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
        if idx + 1 == n && k == "expression_statement" && !lo.text(child).trim_end().ends_with(';')
        {
            let expr = child.named_child(0).unwrap_or(child);
            let e = lower_expr(lo, expr);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[e]));
        } else if idx + 1 == n && is_tail_expr(k) {
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
            let pattern = node.child_by_field_name("pattern");
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
                .map(|s| lo.add(NodeKind::Var, Payload::Name(s), span, &[]))
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
            lo.int_lit(strip_rust_decimal_int_suffix(t), span)
        }
        "float_literal" => lo.float_lit(lo.text(node), span),
        "negative_literal" => lower_negative_literal(lo, node),
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
        "reference_pattern" => node
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
            let (start, end, inclusive) = lower_range_bounds(lo, node);
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
        "tuple_pattern" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym("tuple_expression");
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "slice_pattern" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym("array_expression");
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
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
        // `async { … }` / `async move { … }` is a wrapper around the block; `.await`
        // is already peeled above.
        "async_block" => node
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
    let macro_name = node.child_by_field_name("macro").map(|m| lo.text(m));
    let name = macro_name.map(|name| lo.sym(name));
    let mut args = Vec::new();
    for c in Lowering::named_children(node) {
        if c.kind() == "token_tree" {
            collect_macro_atoms(lo, c, &mut args);
        }
    }
    if macro_name.is_some_and(|name| name.trim_end_matches('!') == "panic") {
        return lo.add(NodeKind::Throw, Payload::None, span, &args);
    }
    let callee = lo.add(
        NodeKind::Var,
        name.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[],
    );
    let mut kids = vec![callee];
    kids.extend(args);
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

fn lower_negative_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(child) = node.named_child(0) else {
        return lo.raw(node.kind(), span, &[]);
    };
    match child.kind() {
        "integer_literal" => {
            let text = lo.text(child);
            let magnitude = strip_rust_decimal_int_suffix(text);
            let signed = format!("-{magnitude}");
            lo.int_lit(&signed, span)
        }
        "float_literal" => {
            let signed = format!("-{}", lo.text(child));
            lo.float_lit(&signed, span)
        }
        _ => lo.raw(node.kind(), span, &[]),
    }
}

fn strip_rust_decimal_int_suffix(text: &str) -> &str {
    let trimmed = text.trim();
    if matches!(
        trimmed.get(..2),
        Some("0x" | "0X" | "0b" | "0B" | "0o" | "0O")
    ) {
        return trimmed;
    }
    let end = trimmed
        .char_indices()
        .find(|&(_, ch)| ch.is_ascii_alphabetic())
        .map(|(idx, _)| idx)
        .unwrap_or(trimmed.len());
    trimmed[..end].trim_end_matches('_')
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
        "let_condition" => lower_let_condition(lo, node),
        "let_chain" => node
            .child_by_field_name("value")
            .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)))
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lower_expr(lo, node)),
        _ => lower_expr(lo, node),
    }
}

fn lower_let_condition(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(value_node) = node
        .child_by_field_name("value")
        .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)))
    else {
        return lower_expr(lo, node);
    };
    let text = lo.text(node).trim();
    let op = if text.starts_with("let Some") && !rust_file_defines_name(lo, "Some") {
        Some(Builtin::IsNotNull)
    } else if text.starts_with("let None") && !rust_file_defines_name(lo, "None") {
        Some(Builtin::IsNull)
    } else {
        None
    };
    if let Some(op) = op {
        let value = lower_expr(lo, value_node);
        return lo.add(NodeKind::Call, Payload::Builtin(op), span, &[value]);
    }
    lower_expr(lo, value_node)
}

fn rust_file_defines_name(lo: &Lowering, name: &str) -> bool {
    let Ok(src) = std::str::from_utf8(lo.src) else {
        return false;
    };
    rust_item_declares_name(src, name)
}

fn rust_item_declares_name(src: &str, name: &str) -> bool {
    const ITEM_PREFIXES: &[&str] = &[
        "const", "static", "fn", "struct", "enum", "union", "type", "mod", "trait",
    ];
    src.lines()
        .map(strip_rust_line_comment)
        .map(rust_identifier_tokens)
        .any(|tokens| rust_tokens_declare_name(&tokens, ITEM_PREFIXES, name))
}

fn strip_rust_line_comment(line: &str) -> &str {
    line.split_once("//").map(|(code, _)| code).unwrap_or(line)
}

fn rust_identifier_tokens(line: &str) -> Vec<&str> {
    line.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .collect()
}

fn rust_tokens_declare_name(tokens: &[&str], item_prefixes: &[&str], name: &str) -> bool {
    tokens.iter().enumerate().any(|(idx, token)| {
        if !item_prefixes.contains(token) {
            return false;
        }
        tokens
            .get(idx + 1)
            .is_some_and(|candidate| !rust_item_qualifier_token(candidate) && *candidate == name)
    })
}

fn rust_item_qualifier_token(token: &str) -> bool {
    matches!(
        token,
        "pub" | "crate" | "super" | "self" | "unsafe" | "async" | "extern" | "const" | "fn"
    )
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
    let mut branches = Vec::new();
    let mut default_body = None;
    for arm in arms {
        let body_node = arm.child_by_field_name("value");
        let body_node_id = body_node.map(|v| v.id());
        let body = body_node
            .map(|v| {
                if v.kind() == "block" {
                    lower_block(lo, v)
                } else {
                    lower_expr(lo, v)
                }
            })
            .unwrap_or_else(|| lo.empty_block(span));
        let pattern = arm.child_by_field_name("pattern").or_else(|| {
            Lowering::named_children(arm)
                .into_iter()
                .find(|child| Some(child.id()) != body_node_id && !is_match_guard(*child))
        });
        let pattern_cond =
            pattern.and_then(|pattern| lower_match_pattern_condition(lo, scrutinee, pattern, span));
        let guard_cond = arm
            .child_by_field_name("condition")
            .or_else(|| arm.child_by_field_name("guard"))
            .or_else(|| {
                Lowering::named_children(arm)
                    .into_iter()
                    .find(|child| is_match_guard(*child))
                    .and_then(|guard| guard.named_child(0))
            })
            .map(|guard| lower_expr(lo, guard));
        let Some(cond) = combine_match_conditions(lo, span, pattern_cond, guard_cond) else {
            default_body = Some(body);
            continue;
        };
        branches.push((cond, body));
    }
    let mut acc = default_body.unwrap_or_else(|| lo.empty_block(span));
    for (cond, body) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
    }
    acc
}

fn is_match_guard(node: TsNode) -> bool {
    matches!(node.kind(), "match_arm_guard" | "match_guard")
}

fn lower_match_pattern_condition(
    lo: &mut Lowering,
    scrutinee: NodeId,
    pattern: TsNode,
    span: Span,
) -> Option<NodeId> {
    if lo.text(pattern).trim() == "_" || pattern.kind() == "wildcard_pattern" {
        return None;
    }
    if pattern.kind() == "match_pattern" {
        let guard = pattern.child_by_field_name("condition");
        let guard_id = guard.map(|guard| guard.id());
        let pattern_cond = pattern
            .child_by_field_name("pattern")
            .or_else(|| {
                Lowering::named_children(pattern)
                    .into_iter()
                    .find(|child| Some(child.id()) != guard_id)
            })
            .and_then(|child| lower_match_pattern_condition(lo, scrutinee, child, span));
        let guard_cond = guard.map(|guard| lower_expr(lo, guard));
        return combine_match_conditions(lo, span, pattern_cond, guard_cond);
    }
    if pattern.kind() == "or_pattern" {
        let mut conditions = Vec::new();
        for child in Lowering::named_children(pattern) {
            let cond = lower_match_pattern_condition(lo, scrutinee, child, span)?;
            conditions.push(cond);
        }
        return fold_or(lo, span, conditions);
    }
    if pattern.kind() == "range_pattern" {
        return lower_range_pattern_condition(lo, scrutinee, pattern, span);
    }
    let pat = lower_expr(lo, pattern);
    Some(lo.add(
        NodeKind::BinOp,
        Payload::Op(Op::Eq),
        span,
        &[scrutinee, pat],
    ))
}

fn lower_range_bounds(lo: &mut Lowering, node: TsNode) -> (Option<NodeId>, Option<NodeId>, bool) {
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
    (start, end, inclusive)
}

fn lower_range_pattern_condition(
    lo: &mut Lowering,
    scrutinee: NodeId,
    pattern: TsNode,
    span: Span,
) -> Option<NodeId> {
    let (start, end, inclusive) = lower_range_bounds(lo, pattern);
    let lower = start.map(|start| {
        lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Ge),
            span,
            &[scrutinee, start],
        )
    });
    let upper = end.map(|end| {
        let op = if inclusive { Op::Le } else { Op::Lt };
        lo.add(NodeKind::BinOp, Payload::Op(op), span, &[scrutinee, end])
    });
    match (lower, upper) {
        (Some(lower), Some(upper)) => {
            Some(lo.add(NodeKind::BinOp, Payload::Op(Op::And), span, &[lower, upper]))
        }
        (Some(cond), None) | (None, Some(cond)) => Some(cond),
        (None, None) => None,
    }
}

fn fold_or(lo: &mut Lowering, span: Span, conditions: Vec<NodeId>) -> Option<NodeId> {
    let mut it = conditions.into_iter();
    let mut acc = it.next()?;
    for cond in it {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[acc, cond]);
    }
    Some(acc)
}

fn combine_match_conditions(
    lo: &mut Lowering,
    span: Span,
    pattern_cond: Option<NodeId>,
    guard_cond: Option<NodeId>,
) -> Option<NodeId> {
    match (pattern_cond, guard_cond) {
        (Some(pattern), Some(guard)) => Some(lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::And),
            span,
            &[pattern, guard],
        )),
        (Some(cond), None) | (None, Some(cond)) => Some(cond),
        (None, None) => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn match_case_rhs_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.rs", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::BinOp && n.payload == Payload::Op(Op::Eq))
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

    fn lower_rust(src: &str) -> (Interner, Il) {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.rs", src.as_bytes(), &interner).expect("lower");
        (interner, il)
    }

    fn raw_names(il: &Il, interner: &Interner) -> Vec<String> {
        il.nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Raw)
            .filter_map(|node| match node.payload {
                Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
                _ => None,
            })
            .collect()
    }

    fn binop_ops(il: &Il) -> Vec<Op> {
        il.nodes
            .iter()
            .filter(|node| node.kind == NodeKind::BinOp)
            .filter_map(|node| match node.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn match_cases_compare_scrutinee_to_literal_patterns() {
        let src = "fn f(x: i32) -> i32 { match x { 7 => 1, 8 => 2, _ => 3 } }";
        assert_eq!(match_case_rhs_ints(src), vec![7, 8]);
    }

    #[test]
    fn guarded_match_combines_pattern_and_guard() {
        let src = "fn f(x: i32, ok: bool) -> i32 { match x { 7 | 8 if ok => 1, _ => 2 } }";
        let (interner, il) = lower_rust(src);

        assert_eq!(match_case_rhs_ints(src), vec![7, 8]);
        assert!(il
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::And)));
        assert!(il.nodes.iter().any(|node| match node.payload {
            Payload::Name(sym) => interner.resolve(sym) == "ok",
            _ => false,
        }));
    }

    #[test]
    fn panic_macro_lowers_to_throw() {
        let src = "fn f(x: i32) -> i32 { if x < 0 { panic!(); } x }";
        let (_, il) = lower_rust(src);

        assert!(
            il.nodes.iter().any(|node| node.kind == NodeKind::Throw),
            "panic! should lower to a Throw node so guard clauses are path-narrowing exits"
        );
    }

    #[test]
    fn rust_item_shadow_scan_handles_visibility_and_qualifiers() {
        assert!(rust_item_declares_name(
            "pub(crate) struct Some<T>(T);",
            "Some"
        ));
        assert!(rust_item_declares_name(
            "pub const None: Option<i32> = Some(0);",
            "None"
        ));
        assert!(rust_item_declares_name(
            "pub const fn Some(value: i32) -> Option<i32> { None }",
            "Some"
        ));
        assert!(!rust_item_declares_name(
            "if let Some(_) = value { true } else { false }",
            "Some"
        ));
    }

    #[test]
    fn match_range_pattern_lowers_to_bounds() {
        let src = "fn f(x: i32) -> i32 { match x { 1..=3 => 7, _ => 0 } }";
        let (interner, il) = lower_rust(src);

        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| name == "range_pattern"),
            "range match pattern should lower without Raw range_pattern: {raw:?}"
        );
        let ops = binop_ops(&il);
        assert!(
            ops.contains(&Op::Ge) && ops.contains(&Op::Le) && ops.contains(&Op::And),
            "inclusive range pattern should lower to lower/upper bound checks, got {ops:?}"
        );
    }

    #[test]
    fn match_tuple_pattern_lowers_without_raw() {
        let src = "fn f(x: (i32, i32)) -> i32 { match x { (1, 2) => 7, _ => 0 } }";
        let (interner, il) = lower_rust(src);

        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| name == "tuple_pattern"),
            "tuple match pattern should lower without Raw tuple_pattern: {raw:?}"
        );
    }

    #[test]
    fn match_slice_pattern_lowers_without_raw() {
        let src = "fn f(x: [i32; 2]) -> i32 { match x { [1, 2] => 7, _ => 0 } }";
        let (interner, il) = lower_rust(src);

        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| name == "slice_pattern"),
            "slice match pattern should lower without Raw slice_pattern: {raw:?}"
        );
    }

    #[test]
    fn match_reference_pattern_lowers_without_raw() {
        let src = "fn f(x: &i32) -> i32 { match x { &1 => 7, _ => 0 } }";
        let (interner, il) = lower_rust(src);

        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| name == "reference_pattern"),
            "reference match pattern should lower without Raw reference_pattern: {raw:?}"
        );
    }

    #[test]
    fn match_negative_literal_pattern_lowers_without_raw() {
        let src = "fn f(x: i32) -> i32 { match x { -1 => 7, _ => 0 } }";
        let (interner, il) = lower_rust(src);

        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| name == "negative_literal"),
            "negative literal match pattern should lower without Raw negative_literal: {raw:?}"
        );
        assert!(match_case_rhs_ints(src).contains(&-1));
    }

    #[test]
    fn match_typed_integer_literal_pattern_retains_value() {
        let src = "fn f(x: i32) -> i32 { match x { 1i32 => 7, _ => 0 } }";
        assert!(
            match_case_rhs_ints(src).contains(&1),
            "typed integer match patterns should retain their numeric value"
        );
    }

    #[test]
    fn async_blocks_lower_to_body_not_raw() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.rs",
            b"pub async fn f(x: i32) -> i32 { async move { x + 1 }.await }\n",
            &interner,
        )
        .expect("lower");

        let raw: Vec<_> = il
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Raw)
            .filter_map(|node| match node.payload {
                Payload::Name(sym) => Some(interner.resolve(sym)),
                _ => None,
            })
            .collect();
        assert!(
            raw.is_empty(),
            "async block should lower without Raw: {raw:?}"
        );

        let ops: Vec<_> = il
            .nodes
            .iter()
            .filter_map(|node| match node.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect();
        assert!(ops.contains(&Op::Add), "async block body was lost: {ops:?}");
    }
}
