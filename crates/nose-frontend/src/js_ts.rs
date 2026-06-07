//! JavaScript / TypeScript → raw IL lowering.
//!
//! JS and TS share one walk; TypeScript type syntax is erased (`type_annotation`,
//! `as`/`satisfies`/`!` are stripped). Convergence-friendly lowering: `await e`
//! and `a as T` reduce to `e`/`a`; `i++` and `x += 1` desugar to assignments;
//! C-style `for`, `for...of`, `do/while` map to the unified `Loop`; `switch`
//! becomes an `if`/`else if` chain; ternary lowers to an expression `If`.

use crate::lower::Lowering;
use nose_il::{
    Builtin, FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload, Span,
    UnitKind,
};
use tree_sitter::Node as TsNode;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    lang: Lang,
    interner: &Interner,
) -> anyhow::Result<Il> {
    use crate::lower::grammar;
    // JS, TS, and TSX share a parse path but use distinct grammars; .tsx needs the
    // TSX dialect (JSX-aware), other TS files the plain TypeScript grammar.
    let key = if lang == Lang::JavaScript {
        grammar::JAVASCRIPT
    } else if path.ends_with(".tsx") {
        grammar::TSX
    } else {
        grammar::TYPESCRIPT
    };
    crate::lower::lower_file(
        file,
        path,
        src,
        interner,
        key,
        || match key {
            grammar::JAVASCRIPT => tree_sitter_javascript::LANGUAGE.into(),
            grammar::TSX => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        },
        lang,
        lower_program,
    )
}

fn lower_program(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, |lo, c| lower_stmt(lo, c, false))
}

fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, |lo, c| lower_stmt(lo, c, false))
}

fn lower_stmt(lo: &mut Lowering, node: TsNode, in_class: bool) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function" => Some(lower_func(lo, node, in_class)),
        // ambient declarations without bodies — no behavior to model
        "function_signature" => None,
        "method_definition" => Some(lower_func(lo, node, true)),
        "class_declaration" | "class" | "abstract_class_declaration" => Some(lower_class(lo, node)),
        "lexical_declaration" | "variable_declaration" => Some(lower_var_decl(lo, node)),
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for_c(lo, node)),
        "for_in_statement" => Some(lower_for_in(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "do_statement" => Some(lower_do(lo, node)),
        "switch_statement" => Some(lower_switch(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "throw_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Throw, Payload::None, span, &kids))
        }
        "try_statement" => Some(lower_try(lo, node)),
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        "statement_block" => Some(lower_block(lo, node)),
        // `label: stmt` (loop/block label) — lower the inner statement, drop the label.
        "labeled_statement" => Lowering::named_children(node)
            .into_iter()
            .next_back()
            .and_then(|s| lower_stmt(lo, s, in_class)),
        "expression_statement" => {
            let child = node.named_child(0)?;
            Some(match child.kind() {
                "assignment_expression" => crate::lower::assignment(lo, child, lower_expr),
                "augmented_assignment_expression" => lower_aug_assignment(lo, child),
                "update_expression" => lower_update(lo, child),
                _ => {
                    let e = lower_expr(lo, child);
                    lo.add(NodeKind::ExprStmt, Payload::None, span, &[e])
                }
            })
        }
        "empty_statement" => None,
        "import_statement" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        "export_statement" => {
            // Only an `export <decl>` carries behavior; re-exports
            // (`export {…} from "…"`, `export * …`) are erased.
            match node.named_child(0) {
                Some(d) if is_exportable_decl(d.kind()) => lower_stmt(lo, d, in_class),
                _ => None,
            }
        }
        // A type *declaration* is content worth deduplicating: copy-pasted type/
        // interface/enum definitions (e.g. generated `.gen.ts` files) are real
        // duplication. Lower to a structural unit (names + literals + shape). This is
        // distinct from erasing type *annotations* in code (behavioral convergence).
        "type_alias_declaration" | "interface_declaration" | "enum_declaration" => {
            Some(lower_type_decl(lo, node))
        }
        // TypeScript type-only / ambient constructs and bare specifiers: erase.
        "ambient_declaration"
        | "method_signature"
        | "abstract_method_signature"
        | "import_alias"
        | "export_specifier"
        | "export_clause"
        | "comment"
        // legacy `<!-- -->` HTML comments are valid JS tokens (common in <script> blocks)
        | "html_comment" => None,
        // class fields when walking a class body
        "field_definition" | "public_field_definition" => lower_field(lo, node),
        // Anything else in statement position: treat as an expression statement
        // (lower_expr has its own Raw fallback for genuinely unknown nodes).
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim().trim_end_matches(';').trim();
    let module = quoted_after_from(text)?;
    let mut assigns = Vec::new();

    if let Some(ns) = text
        .strip_prefix("import * as ")
        .and_then(|rest| rest.split(" from ").next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        assigns.push(crate::lower::import_namespace(lo, span, ns, module));
    } else if let Some((start, end)) = brace_range(text) {
        let inner = &text[start + 1..end];
        for part in inner.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (exported, local) = js_import_specifier(part)?;
            assigns.push(crate::lower::import_binding(
                lo, span, local, module, exported,
            ));
        }
    } else if let Some(default_part) = text
        .strip_prefix("import ")
        .and_then(|rest| rest.split(" from ").next())
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with(['{', '*']))
    {
        let local = default_part.split(',').next()?.trim();
        assigns.push(crate::lower::import_binding(
            lo, span, local, module, "default",
        ));
    }

    match assigns.len() {
        0 => None,
        1 => assigns.pop(),
        _ => Some(lo.add(NodeKind::Block, Payload::None, span, &assigns)),
    }
}

fn quoted_after_from(text: &str) -> Option<&str> {
    let rest = text.split(" from ").nth(1)?.trim();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = rest[1..].find(quote)? + 1;
    Some(&rest[1..end])
}

fn brace_range(text: &str) -> Option<(usize, usize)> {
    let start = text.find('{')?;
    let end = text[start + 1..].find('}')? + start + 1;
    Some((start, end))
}

fn js_import_specifier(part: &str) -> Option<(&str, &str)> {
    let part = part.strip_prefix("type ").unwrap_or(part).trim();
    if let Some((exported, local)) = part.split_once(" as ") {
        Some((exported.trim(), local.trim()))
    } else {
        Some((part, part))
    }
}

fn is_exportable_decl(k: &str) -> bool {
    matches!(
        k,
        "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "class"
            | "lexical_declaration"
            | "variable_declaration"
            | "type_alias_declaration"
            | "interface_declaration"
            | "enum_declaration"
    )
}

/// A TypeScript `type X = …` / `interface X {…}` / `enum X {…}` declaration. Type
/// annotations in *code* are erased (behavioral convergence), but a type *declaration*
/// is content worth deduplicating — copy-pasted type files (e.g. generated `.gen.ts`)
/// are real duplication. Lower to a `Class` unit whose body skeletonizes the type
/// (its value/body, not its name → renamed copies still converge).
fn lower_type_decl(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let target = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("body"))
        .unwrap_or(node);
    let body = lower_type_skeleton(lo, target);
    let block = lo.add(NodeKind::Block, Payload::None, span, &[body]);
    lo.push_unit(block, UnitKind::Class, name);
    block
}

/// Recursively skeletonize a type node: identifiers / property names / type keywords →
/// `Var`, literal types → literals, composites → `Seq` of their parts. Captures the
/// type's textual structure (so identical definitions converge, different ones don't)
/// without modeling type semantics.
fn lower_type_skeleton(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "type_identifier"
        | "property_identifier"
        | "identifier"
        | "predefined_type"
        | "shorthand_property_identifier"
        | "this_type" => lo.var(lo.text(node), span),
        "string" => lo.str_lit(lo.text(node), span),
        "number" => lo.int_lit(lo.text(node).trim(), span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_type_skeleton(lo, c))
                .collect();
            if kids.is_empty() {
                lo.var(node.kind(), span) // keyword leaf (true/null/void/…)
            } else {
                lo.add(NodeKind::Seq, Payload::None, span, &kids)
            }
        }
    }
}

fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    crate::lower::function_unit(lo, node, method, lower_params, lower_func_body)
}

/// A function body is normally a `statement_block`, but arrow functions may have
/// an expression body.
fn lower_func_body(lo: &mut Lowering, body: TsNode) -> NodeId {
    if body.kind() == "statement_block" {
        lower_block(lo, body)
    } else {
        let span = lo.span(body);
        let e = lower_expr(lo, body);
        let ret = lo.add(NodeKind::Return, Payload::None, span, &[e]);
        lo.add(NodeKind::Block, Payload::None, span, &[ret])
    }
}

fn lower_class(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node.child_by_field_name("body");
    let block = match body {
        Some(b) => {
            let mut stmts = Vec::new();
            for child in Lowering::named_children(b) {
                if let Some(id) = lower_stmt(lo, child, true) {
                    stmts.push(id);
                }
            }
            lo.add(NodeKind::Block, Payload::None, lo.span(b), &stmts)
        }
        None => lo.empty_block(span),
    };
    lo.push_unit(block, UnitKind::Class, name);
    block
}

fn lower_field(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    // `name = value;` field initializer → Assign; bare declarations are dropped.
    let span = lo.span(node);
    let name = node.child_by_field_name("name")?;
    let value = node.child_by_field_name("value")?;
    let lhs = {
        let s = lo.span(name);
        lo.var(lo.text(name), s)
    };
    let rhs = lower_expr(lo, value);
    Some(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]))
}

fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    // A single-identifier arrow param arrives as the identifier itself.
    if matches!(params.kind(), "identifier" | "undefined") {
        let span = lo.span(params);
        let sym = lo.sym(lo.text(params));
        out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[]));
        return;
    }
    for p in Lowering::named_children(params) {
        let span = lo.span(p);
        let name = param_name(lo, p);
        let payload = match name {
            Some(s) => Payload::Name(lo.sym(s)),
            None => Payload::None,
        };
        if let Some(semantic) = crate::lower::param_semantic_from_text(lo.text(p)) {
            lo.record_param_semantic(span, semantic);
        }
        out.push(lo.add(NodeKind::Param, payload, span, &[]));
    }
}

fn param_name<'a>(lo: &Lowering<'a>, p: TsNode<'a>) -> Option<&'a str> {
    match p.kind() {
        "identifier" | "shorthand_property_identifier_pattern" | "undefined" => Some(lo.text(p)),
        "required_parameter" | "optional_parameter" => p
            .child_by_field_name("pattern")
            .or_else(|| p.named_child(0))
            .map(|n| lo.text(n)),
        "rest_pattern" | "assignment_pattern" => p.named_child(0).map(|n| lo.text(n)),
        _ => p.named_child(0).map(|n| lo.text(n)),
    }
}

fn lower_var_decl(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // One or more `variable_declarator`s → a Block of Assigns (or a single Assign).
    let mut assigns = Vec::new();
    for d in Lowering::named_children(node) {
        if d.kind() != "variable_declarator" {
            continue;
        }
        let dspan = lo.span(d);
        let name_node = d.child_by_field_name("name");
        let rhs = match d.child_by_field_name("value") {
            // `const f = (…) => {…}` / `const f = function(){…}` is a *named function*,
            // not an inline lambda — lower it to a `Func` unit so it is extracted and
            // matched like any function. (Modern JS/TS defines most functions this way;
            // without this, arrow-const-heavy files yield zero detection units.)
            Some(v) if is_func_value(v.kind()) => {
                let nsym = name_node
                    .filter(|n| n.kind() == "identifier")
                    .map(|n| lo.sym(lo.text(n)));
                lower_func_value(lo, v, nsym)
            }
            Some(v) => lower_expr(lo, v),
            None => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), dspan, &[]),
        };
        if let Some(name) = name_node {
            if let Some(mut projected) = lower_static_projection_pattern(lo, name, rhs, dspan) {
                assigns.append(&mut projected);
                continue;
            }
        }
        let lhs = match name_node {
            Some(n) => lower_expr(lo, n),
            None => lo.empty_block(dspan),
        };
        assigns.push(lo.add(NodeKind::Assign, Payload::None, dspan, &[lhs, rhs]));
    }
    if assigns.len() == 1 {
        assigns[0]
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

fn lower_static_projection_pattern(
    lo: &mut Lowering,
    pattern: TsNode,
    base: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    if pattern.kind() != "object_pattern" {
        return None;
    }
    let mut assigns = Vec::new();
    for child in Lowering::named_children(pattern) {
        let (field, local) = object_pattern_projection(lo, child)?;
        let lhs = lo.var(&local, lo.span(child));
        let key = lo.sym(&field);
        let rhs = lo.add(NodeKind::Field, Payload::Name(key), lo.span(child), &[base]);
        assigns.push(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]));
    }
    (!assigns.is_empty()).then_some(assigns)
}

fn object_pattern_projection(lo: &Lowering, node: TsNode) -> Option<(String, String)> {
    match node.kind() {
        "shorthand_property_identifier_pattern" => {
            let name = lo.text(node).to_string();
            Some((name.clone(), name))
        }
        "pair_pattern" => {
            let kids = Lowering::named_children(node);
            let key = kids.first().and_then(|&k| static_property_key(lo, k))?;
            let local = kids
                .iter()
                .skip(1)
                .find_map(|&k| binding_pattern_name(lo, k))?;
            Some((key, local))
        }
        _ => None,
    }
}

fn binding_pattern_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" | "property_identifier" => {
            Some(lo.text(node).to_string())
        }
        _ => None,
    }
}

fn static_property_key(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "property_identifier" | "shorthand_property_identifier_pattern" | "identifier" => {
            Some(lo.text(node).to_string())
        }
        "string" => static_string_key(lo, node),
        _ => None,
    }
}

fn static_string_key(lo: &Lowering, node: TsNode) -> Option<String> {
    let text = lo.text(node);
    let bytes = text.as_bytes();
    let quote = *bytes.first()?;
    if bytes.len() < 2 || bytes.last().copied()? != quote || !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let inner = &text[1..text.len() - 1];
    if inner.contains('\\') || inner.contains('\n') || inner.contains('\r') {
        return None;
    }
    Some(inner.to_string())
}

fn lower_aug_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .and_then(|t| js_bin_op(t.trim_end_matches('=')))
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

/// `x++` / `++x` / `x--`  →  `x = x +/- 1`.
fn lower_update(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, unwrap_paren(c)))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| lower_stmt_as_block(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        // else_clause wraps either a block/statement or another if (else-if).
        let inner = alt.named_child(0).unwrap_or(alt);
        let else_node = if inner.kind() == "if_statement" {
            lower_if(lo, inner)
        } else {
            lower_stmt_as_block(lo, inner)
        };
        kids.push(else_node);
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

/// Lower a statement that may or may not be a block into a `Block`.
fn lower_stmt_as_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "statement_block" {
        lower_block(lo, node)
    } else {
        let span = lo.span(node);
        let s = lower_stmt(lo, node, false);
        lo.block_of_stmt(span, s)
    }
}

fn lower_for_c(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let init = match node.child_by_field_name("initializer") {
        Some(i) => lower_for_clause_stmt(lo, i),
        None => lo.empty_block(span),
    };
    let cond = match node.child_by_field_name("condition") {
        Some(c) => lower_expr(lo, strip_expr_stmt(c)),
        None => lo.empty_block(span),
    };
    let update = match node.child_by_field_name("increment") {
        Some(u) => lower_for_clause_stmt(lo, u),
        None => lo.empty_block(span),
    };
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::CStyle),
        span,
        &[init, cond, update, body],
    )
}

/// The init/update slots of a C-style for may be declarations, assignments, or
/// update expressions; normalize them to a single statement node.
fn lower_for_clause_stmt(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "lexical_declaration" | "variable_declaration" => lower_var_decl(lo, node),
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr),
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

fn lower_for_in(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, unwrap_paren(c)))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

fn lower_do(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, unwrap_paren(c)))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

/// `switch (v) { case t: ...; default: ... }`  →  nested `if (v == t) {...} else
/// ...`. Fallthrough is ignored (acceptable for fuzzy structural matching).
fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
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
                    if let Some(cond) =
                        fold_js_switch_labels(lo, span, scrutinee, pending_labels.split_off(0))
                    {
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

fn fold_js_switch_labels(
    lo: &mut Lowering,
    span: Span,
    scrutinee: NodeId,
    labels: Vec<NodeId>,
) -> Option<NodeId> {
    let mut acc = None;
    for label in labels {
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[scrutinee, label],
        );
        acc = Some(match acc {
            None => cond,
            Some(prev) => lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[prev, cond]),
        });
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

fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
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

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "shorthand_property_identifier" | "this" | "super" => {
            lo.var(lo.text(node), span)
        }
        "number" => {
            let t = lo.text(node);
            lo.int_lit(t, span)
        }
        "string" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        // Template literal → string-concat chain, converging with `"a" + x`.
        "template_string" => lower_template(lo, node),
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "null" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "undefined" => lo.var("undefined", span),
        "regex" => lo.str_lit(lo.text(node), span),
        "call_expression" => lower_call(lo, node),
        "new_expression" => lower_new(lo, node),
        "binary_expression" => {
            lower_record_shape_guard(lo, node).unwrap_or_else(|| lower_binary(lo, node))
        }
        "unary_expression" => lower_unary(lo, node),
        "update_expression" => lower_update(lo, node),
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr),
        "augmented_assignment_expression" => lower_aug_assignment(lo, node),
        "member_expression" => {
            let obj = node
                .child_by_field_name("object")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let prop = node
                .child_by_field_name("property")
                .map(|p| lo.text(p))
                .unwrap_or("");
            let sym = lo.sym(prop);
            lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj])
        }
        "subscript_expression" => {
            let base = node
                .child_by_field_name("object")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let index_node = node.child_by_field_name("index");
            if let Some(key) = index_node.and_then(|i| static_string_key(lo, i)) {
                let sym = lo.sym(&key);
                return lo.add(NodeKind::Field, Payload::Name(sym), span, &[base]);
            }
            let idx = index_node
                .map(|i| lower_expr(lo, i))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Index, Payload::None, span, &[base, idx])
        }
        "arrow_function"
        | "function_expression"
        | "function"
        | "generator_function"
        | "generator_function_declaration" => lower_arrow(lo, node),
        "array" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            let tag = lo.sym("array");
            lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
        }
        "object" => lower_object(lo, node),
        "pair" => lower_object_pair(lo, node),
        "ternary_expression" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            let then = node
                .child_by_field_name("consequence")
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            let alt = node
                .child_by_field_name("alternative")
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::If, Payload::None, span, &[cond, then, alt])
        }
        "parenthesized_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Strip await / TS-only wrappers so variants converge.
        "await_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "as_expression" | "satisfies_expression" | "non_null_expression" | "type_assertion" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "spread_element" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "sequence_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // Object keys / property names used as expressions (object literals).
        "property_identifier" => lo.var(lo.text(node), span),
        // Method shorthand inside an object literal.
        "method_definition" => lower_func(lo, node, true),
        // Destructuring patterns → a Seq of the bound targets.
        "object_pattern" | "array_pattern" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        "shorthand_property_identifier_pattern" => lo.var(lo.text(node), span),
        "pair_pattern" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        "rest_pattern" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "assignment_pattern" | "object_assignment_pattern" => node
            .child_by_field_name("left")
            .or_else(|| node.named_child(0))
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "required_parameter" | "optional_parameter" => node
            .child_by_field_name("pattern")
            .or_else(|| node.named_child(0))
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "instantiation_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "empty_statement" => lo.empty_block(span),
        // JSX → Call(tag, attrs…, children…); text → string literal.
        "jsx_element" | "jsx_self_closing_element" | "jsx_fragment" => lower_jsx(lo, node),
        "jsx_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "jsx_text" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Str), span, &[]),
        // String/template internals, if ever reached directly (safety net).
        "string_fragment" | "escape_sequence" => {
            lo.add(NodeKind::Lit, Payload::Lit(LitClass::Str), span, &[])
        }
        "template_substitution" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "yield_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "computed_property_name" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "meta_property" => lo.var(lo.text(node), span),
        "import" => lo.var("import", span),
        // TypeScript type syntax in a value position: erase to a neutral literal.
        k if is_ts_type(k) => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[]),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

/// Function-valued initializers that, when bound to a name, become function units.
fn is_func_value(kind: &str) -> bool {
    matches!(
        kind,
        "arrow_function"
            | "function_expression"
            | "function"
            | "generator_function"
            | "generator_function_declaration"
    )
}

/// Lower a function-valued expression as a named `Func` unit (params + body),
/// registering it for detection. Mirrors `lower_func`/`lower_arrow` but takes the
/// binding name explicitly (arrow/function expressions have no own name).
fn lower_func_value(lo: &mut Lowering, node: TsNode, name: Option<nose_il::Symbol>) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
    {
        lower_params(lo, params, &mut kids);
    }
    let body = match node.child_by_field_name("body") {
        Some(b) => lower_func_body(lo, b),
        None => lo.empty_block(span),
    };
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(func, UnitKind::Function, name);
    func
}

fn lower_arrow(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
    {
        lower_params(lo, params, &mut kids);
    }
    let body = match node.child_by_field_name("body") {
        Some(b) => lower_func_body(lo, b),
        None => lo.empty_block(span),
    };
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}

fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    if let Some(guard) = lower_own_property_guard_call(lo, node) {
        return guard;
    }
    if let Some(math_builtin) = lower_math_call(lo, node) {
        return math_builtin;
    }
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
        if args.kind() == "template_string" {
            // tagged template: `tag`…`` — lower the template as a single arg
            kids.push(lower_template(lo, args));
        } else {
            for a in Lowering::named_children(args) {
                kids.push(lower_expr(lo, a));
            }
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn lower_math_call(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let callee = node.child_by_field_name("function")?;
    let (builtin, arity) = match compact_js_expr(lo.text(callee)).as_str() {
        "Math.abs" => (Builtin::Abs, 1),
        "Math.min" => (Builtin::Min, 2),
        "Math.max" => (Builtin::Max, 2),
        _ => return None,
    };
    if file_prefix_has_binding_ident(lo, node, "Math")
        || enclosing_function_prefix_has_binding_ident(lo, node, "Math")
    {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let args: Vec<TsNode> = Lowering::named_children(args);
    if args.len() != arity || args.iter().any(|arg| arg.kind() == "spread_element") {
        return None;
    }
    let lowered: Vec<NodeId> = args.into_iter().map(|arg| lower_expr(lo, arg)).collect();
    Some(lo.add(
        NodeKind::Call,
        Payload::Builtin(builtin),
        lo.span(node),
        &lowered,
    ))
}

fn lower_own_property_guard_call(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let callee = node.child_by_field_name("function")?;
    let callee_text = compact_js_expr(lo.text(callee));
    if !matches!(
        callee_text.as_str(),
        "Object.hasOwn" | "Object.prototype.hasOwnProperty.call"
    ) {
        return None;
    }
    if file_prefix_has_binding_ident(lo, node, "Object")
        || enclosing_function_prefix_has_binding_ident(lo, node, "Object")
    {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let args: Vec<TsNode> = Lowering::named_children(args);
    if args.len() != 2 || args.iter().any(|arg| arg.kind() == "spread_element") {
        return None;
    }
    let span = lo.span(node);
    let receiver = lower_expr(lo, args[0]);
    let key = lower_expr(lo, args[1]);
    let own = lo.str_lit("own", span);
    let present = lo.str_lit("present", span);
    let tag = lo.sym("own_property_guard");
    Some(lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[receiver, key, own, present],
    ))
}

fn file_prefix_has_binding_ident(lo: &Lowering, node: TsNode, ident: &str) -> bool {
    let end = node.start_byte();
    if end > lo.src.len() {
        return false;
    }
    let prefix = std::str::from_utf8(&lo.src[..end]).unwrap_or("");
    contains_js_binding_ident(prefix, ident)
}

fn enclosing_function_prefix_has_binding_ident(lo: &Lowering, node: TsNode, ident: &str) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if matches!(
            parent.kind(),
            "function_declaration" | "function" | "function_expression" | "arrow_function"
        ) {
            let start = parent.start_byte();
            let end = node.start_byte();
            if end <= lo.src.len() && start <= end {
                let prefix = std::str::from_utf8(&lo.src[start..end]).unwrap_or("");
                let header = prefix.find('{').map(|idx| &prefix[..idx]).unwrap_or(prefix);
                if contains_js_ident(header, ident) || contains_js_binding_ident(prefix, ident) {
                    return true;
                }
            }
        }
        current = parent;
    }
    false
}

fn contains_js_binding_ident(text: &str, ident: &str) -> bool {
    ["const", "let", "var", "function", "class"]
        .iter()
        .any(|kw| contains_keyword_binding(text, kw, ident))
        || contains_import_binding(text, ident)
}

fn contains_keyword_binding(text: &str, keyword: &str, ident: &str) -> bool {
    text.match_indices(keyword).any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        if before.is_some_and(is_js_ident_continue) {
            return false;
        }
        let mut rest = &text[idx + keyword.len()..];
        let Some(next) = rest.chars().next() else {
            return false;
        };
        if !next.is_whitespace() {
            return false;
        }
        rest = rest.trim_start();
        starts_with_js_ident(rest, ident)
    })
}

fn contains_import_binding(text: &str, ident: &str) -> bool {
    text.match_indices("import").any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        if before.is_some_and(is_js_ident_continue) {
            return false;
        }
        let rest = text[idx + "import".len()..].trim_start();
        starts_with_js_ident(rest, ident)
            || rest.contains(&format!("{{ {ident}"))
            || rest.contains(&format!("{{{ident}"))
            || rest.contains(&format!(", {ident}"))
            || rest.contains(&format!(" as {ident}"))
    })
}

fn contains_js_ident(text: &str, ident: &str) -> bool {
    text.match_indices(ident).any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        let after = text[idx + ident.len()..].chars().next();
        !before.is_some_and(is_js_ident_continue) && !after.is_some_and(is_js_ident_continue)
    })
}

fn starts_with_js_ident(text: &str, ident: &str) -> bool {
    text.starts_with(ident)
        && !text[ident.len()..]
            .chars()
            .next()
            .is_some_and(is_js_ident_continue)
}

fn is_js_ident_continue(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphanumeric()
}

fn lower_new(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("constructor") {
        Some(c) => kids.push(lower_expr(lo, c)),
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
    if node
        .child_by_field_name("operator")
        .is_some_and(|op| lo.text(op) == "??")
    {
        let span = lo.span(node);
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");
        let value_for_cond = left
            .map(|l| lower_expr(lo, l))
            .unwrap_or_else(|| lo.empty_block(span));
        let null_lit = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]);
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[value_for_cond, null_lit],
        );
        let fallback = right
            .map(|r| lower_expr(lo, r))
            .unwrap_or_else(|| lo.empty_block(span));
        let value = left
            .map(|l| lower_expr(lo, l))
            .unwrap_or_else(|| lo.empty_block(span));
        return lo.add(NodeKind::If, Payload::None, span, &[cond, fallback, value]);
    }
    crate::lower::binary(lo, node, js_bin_op, lower_expr)
}

fn lower_record_shape_guard(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let text = compact_js_expr(lo.text(node));
    let clauses: Vec<String> = text
        .split("&&")
        .map(strip_outer_parens_owned)
        .filter(|s| !s.is_empty())
        .collect();
    if clauses.len() != 3 {
        return None;
    }

    let mut ident: Option<String> = None;
    let mut has_typeof_object = false;
    let mut has_non_null_or_truthy = false;
    let mut has_not_array = false;
    for clause in clauses {
        let (kind, name) = record_guard_clause(&clause)?;
        if !simple_js_ident(&name) {
            return None;
        }
        match &ident {
            Some(current) if current != &name => return None,
            None => ident = Some(name.clone()),
            _ => {}
        }
        match kind {
            RecordGuardClause::TypeofObject => has_typeof_object = true,
            RecordGuardClause::NonNullOrTruthy => has_non_null_or_truthy = true,
            RecordGuardClause::NotArray => has_not_array = true,
        }
    }

    if !(has_typeof_object && has_non_null_or_truthy && has_not_array) {
        return None;
    }
    if file_prefix_has_binding_ident(lo, node, "Array")
        || enclosing_function_prefix_has_binding_ident(lo, node, "Array")
    {
        return None;
    }
    let span = lo.span(node);
    let value = lo.var(&ident?, span);
    let object = lo.str_lit("object", span);
    let non_null = lo.str_lit("non_null", span);
    let not_array = lo.str_lit("not_array", span);
    let tag = lo.sym("record_guard");
    Some(lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[value, object, non_null, not_array],
    ))
}

#[derive(Clone, Copy)]
enum RecordGuardClause {
    TypeofObject,
    NonNullOrTruthy,
    NotArray,
}

fn record_guard_clause(clause: &str) -> Option<(RecordGuardClause, String)> {
    parse_typeof_object_clause(clause)
        .map(|name| (RecordGuardClause::TypeofObject, name))
        .or_else(|| {
            parse_non_null_clause(clause).map(|name| (RecordGuardClause::NonNullOrTruthy, name))
        })
        .or_else(|| {
            parse_truthy_clause(clause).map(|name| (RecordGuardClause::NonNullOrTruthy, name))
        })
        .or_else(|| parse_not_array_clause(clause).map(|name| (RecordGuardClause::NotArray, name)))
}

fn parse_typeof_object_clause(clause: &str) -> Option<String> {
    for op in ["===", "=="] {
        if let Some(rest) = clause.strip_prefix("typeof") {
            let (name, value) = rest.split_once(op)?;
            if is_object_literal(value) {
                return Some(name.to_string());
            }
        }
        for object_lit in ["'object'", "\"object\""] {
            let prefix = format!("{object_lit}{op}typeof");
            if let Some(name) = clause.strip_prefix(&prefix) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn parse_non_null_clause(clause: &str) -> Option<String> {
    for op in ["!==", "!="] {
        if let Some((name, "null")) = clause.split_once(op) {
            return Some(name.to_string());
        }
        let prefix = format!("null{op}");
        if let Some(name) = clause.strip_prefix(&prefix) {
            return Some(name.to_string());
        }
    }
    None
}

fn parse_truthy_clause(clause: &str) -> Option<String> {
    if let Some(name) = clause.strip_prefix("!!") {
        return Some(name.to_string());
    }
    clause
        .strip_prefix("Boolean(")
        .and_then(|inner| inner.strip_suffix(')'))
        .map(str::to_string)
}

fn parse_not_array_clause(clause: &str) -> Option<String> {
    if let Some(name) = clause
        .strip_prefix("!Array.isArray(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        return Some(name.to_string());
    }
    for op in ["===", "=="] {
        if let Some(call) = clause.strip_suffix(&format!("{op}false")) {
            if let Some(name) = call
                .strip_prefix("Array.isArray(")
                .and_then(|inner| inner.strip_suffix(')'))
            {
                return Some(name.to_string());
            }
        }
        let prefix = format!("false{op}Array.isArray(");
        if let Some(name) = clause
            .strip_prefix(&prefix)
            .and_then(|inner| inner.strip_suffix(')'))
        {
            return Some(name.to_string());
        }
    }
    None
}

fn is_object_literal(value: &str) -> bool {
    matches!(value, "'object'" | "\"object\"")
}

fn compact_js_expr(text: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut escaped = false;
    for c in text.chars() {
        if let Some(q) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

fn strip_outer_parens_owned(mut text: &str) -> String {
    loop {
        let Some(inner) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
            return text.to_string();
        };
        if !balanced_parens(inner) {
            return text.to_string();
        }
        text = inner;
    }
}

fn balanced_parens(text: &str) -> bool {
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn simple_js_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .unwrap_or("-");
    let arg_node = node.child_by_field_name("argument");
    match op_text {
        "!" | "-" | "+" | "~" => {
            let op = match op_text {
                "!" => Op::Not,
                "-" => Op::Neg,
                "+" => Op::Pos,
                _ => Op::BitNot,
            };
            let arg = arg_node
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[arg])
        }
        "typeof" => {
            let callee = lo.var("typeof", span);
            let arg = arg_node
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Call, Payload::None, span, &[callee, arg])
        }
        // `void` and `delete` have JS-specific side-effect/value semantics that strict
        // exact mode does not prove yet.
        _ => {
            let inner: Vec<NodeId> = arg_node.into_iter().map(|a| lower_expr(lo, a)).collect();
            lo.raw(op_text, span, &inner)
        }
    }
}

fn lower_object(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    for child in Lowering::named_children(node) {
        match child.kind() {
            "pair" => kids.push(lower_object_pair(lo, child)),
            "shorthand_property_identifier" => kids.push(lower_object_shorthand(lo, child)),
            // Spread and methods depend on object/runtime semantics that the strict
            // value graph does not prove yet. Keep the source shape for near mode and
            // make the containing unit ineligible for exact semantic reporting.
            "spread_element" | "method_definition" => {
                let inner: Vec<NodeId> = Lowering::named_children(child)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect();
                kids.push(lo.raw(child.kind(), lo.span(child), &inner));
            }
            _ => kids.push(lower_expr(lo, child)),
        }
    }
    let tag = lo.sym("object");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}

fn lower_object_pair(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let key = node
        .child_by_field_name("key")
        .map(|x| lo.str_lit(lo.text(x), lo.span(x)))
        .unwrap_or_else(|| lo.empty_block(span));
    let value = node
        .child_by_field_name("value")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    let tag = lo.sym("pair");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &[key, value])
}

fn lower_object_shorthand(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = lo.text(node);
    let key = lo.str_lit(name, span);
    let value = lo.var(name, span);
    let tag = lo.sym("pair");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &[key, value])
}

/// Lower a template literal to a string-concat chain. Static fragments keep their
/// string content, and a leading substitution keeps the previous empty-string
/// coercion shape. `` `a${x}b` `` thus converges with `"a" + x + "b"`.
fn lower_template(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut acc = None;
    for c in Lowering::named_children(node) {
        match c.kind() {
            "string_fragment" | "escape_sequence" => {
                let lit = lo.str_lit(lo.text(c), lo.span(c));
                append_template_part(lo, &mut acc, span, lit);
            }
            "template_substitution" => {
                if let Some(e) = c.named_child(0) {
                    if acc.is_none() {
                        acc = Some(lo.str_lit("", span));
                    }
                    let sub = lower_expr(lo, e);
                    append_template_part(lo, &mut acc, span, sub);
                }
            }
            _ => {}
        }
    }
    acc.unwrap_or_else(|| lo.str_lit("", span))
}

fn append_template_part(lo: &mut Lowering, acc: &mut Option<NodeId>, span: Span, part: NodeId) {
    *acc = Some(match *acc {
        Some(left) => lo.add(NodeKind::BinOp, Payload::Op(Op::Add), span, &[left, part]),
        None => part,
    });
}

/// Lower JSX to `Call(tag, attrs…, children…)` — structurally close to the
/// `createElement(tag, props, ...children)` it compiles to.
fn lower_jsx(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    let callee = match jsx_tag(node) {
        Some(t) => {
            let s = lo.span(t);
            lo.var(lo.text(t), s)
        }
        None => lo.empty_block(span),
    };
    kids.push(callee);
    for attr in jsx_attributes(node) {
        kids.push(lower_jsx_attr(lo, attr));
    }
    for c in Lowering::named_children(node) {
        if matches!(
            c.kind(),
            "jsx_element"
                | "jsx_self_closing_element"
                | "jsx_fragment"
                | "jsx_expression"
                | "jsx_text"
        ) {
            kids.push(lower_expr(lo, c));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn jsx_tag(node: TsNode) -> Option<TsNode> {
    if let Some(n) = node.child_by_field_name("name") {
        return Some(n);
    }
    Lowering::named_children(node)
        .into_iter()
        .find(|c| c.kind() == "jsx_opening_element")
        .and_then(|o| o.child_by_field_name("name"))
}

fn jsx_attributes(node: TsNode) -> Vec<TsNode> {
    let host = if node.kind() == "jsx_element" {
        Lowering::named_children(node)
            .into_iter()
            .find(|c| c.kind() == "jsx_opening_element")
    } else {
        Some(node)
    };
    match host {
        Some(h) => Lowering::named_children(h)
            .into_iter()
            .filter(|c| c.kind() == "jsx_attribute")
            .collect(),
        None => Vec::new(),
    }
}

fn lower_jsx_attr(lo: &mut Lowering, attr: TsNode) -> NodeId {
    let span = lo.span(attr);
    let kids = Lowering::named_children(attr);
    if kids.len() >= 2 {
        lower_expr(lo, kids[kids.len() - 1]) // the value (name is first)
    } else {
        lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]) // boolean attr
    }
}

/// TypeScript type-syntax node kinds (erased in value positions).
fn is_ts_type(k: &str) -> bool {
    matches!(
        k,
        "type_identifier"
            | "predefined_type"
            | "generic_type"
            | "type_annotation"
            | "opting_type_annotation"
            | "omitting_type_annotation"
            | "type_arguments"
            | "type_parameter"
            | "type_parameters"
            | "function_type"
            | "constructor_type"
            | "property_signature"
            | "call_signature"
            | "construct_signature"
            | "index_signature"
            | "method_signature"
            | "abstract_method_signature"
            | "union_type"
            | "intersection_type"
            | "type_predicate"
            | "type_query"
            | "index_type_query"
            | "lookup_type"
            | "literal_type"
            | "tuple_type"
            | "array_type"
            | "object_type"
            | "parenthesized_type"
            | "conditional_type"
            | "mapped_type"
            | "nested_type_identifier"
            | "readonly_type"
            | "infer_type"
            | "template_literal_type"
            | "existential_type"
    )
}

fn js_bin_op(text: &str) -> Option<Op> {
    // shared C-family set, plus JS's strict-equality, exponent, unsigned
    // shift, and the type-test operators (the last two collapse lossily).
    crate::lower::common_bin_op(text).or(match text {
        "**" => Some(Op::Pow),
        "===" => Some(Op::Eq),
        "!==" => Some(Op::Ne),
        ">>>" => Some(Op::Shr),
        // `x in obj` is a directional membership/key test — its own non-commutative op.
        // `instanceof` is a type-identity check; equality-shaped is an acceptable approx.
        "in" => Some(Op::In),
        "instanceof" => Some(Op::Eq),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, Interner};

    fn switch_labels_for_return(src: &str, expected_return: i64) -> Vec<i64> {
        let interner = Interner::new();
        let il = crate::lower_source(
            FileId(0),
            "t.js",
            src.as_bytes(),
            Lang::JavaScript,
            &interner,
        )
        .expect("lower js");
        let mut out = Vec::new();
        for (idx, node) in il.nodes.iter().enumerate() {
            if node.kind != NodeKind::If {
                continue;
            }
            let kids = il.children(NodeId(idx as u32));
            if kids.len() >= 2 && block_contains_return_int(&il, kids[1], expected_return) {
                collect_eq_rhs_ints(&il, kids[0], &mut out);
            }
        }
        out.sort_unstable();
        out
    }

    fn block_contains_return_int(il: &Il, node: NodeId, expected: i64) -> bool {
        match il.kind(node) {
            NodeKind::Block => il
                .children(node)
                .iter()
                .any(|&child| block_contains_return_int(il, child, expected)),
            NodeKind::Return => il.children(node).first().is_some_and(
                |&expr| matches!(il.node(expr).payload, Payload::LitInt(v) if v == expected),
            ),
            _ => false,
        }
    }

    fn collect_eq_rhs_ints(il: &Il, node: NodeId, out: &mut Vec<i64>) {
        if il.kind(node) != NodeKind::BinOp {
            return;
        }
        let kids = il.children(node);
        match il.node(node).payload {
            Payload::Op(Op::Or) if kids.len() == 2 => {
                collect_eq_rhs_ints(il, kids[0], out);
                collect_eq_rhs_ints(il, kids[1], out);
            }
            Payload::Op(Op::Eq) if kids.len() == 2 => {
                if let Payload::LitInt(value) = il.node(kids[1]).payload {
                    out.push(value);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn stacked_switch_cases_share_the_following_body() {
        let src = "function f(x) { switch (x) { case 1: case 2: return 7; default: return 0; } }";
        assert_eq!(switch_labels_for_return(src, 7), vec![1, 2]);
    }
}
