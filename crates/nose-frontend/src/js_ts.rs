//! JavaScript / TypeScript → raw IL lowering.
//!
//! JS and TS share one walk; TypeScript type syntax is erased (`type_annotation`,
//! `as`/`satisfies`/`!` are stripped). Convergence-friendly lowering:
//! `a as T` reduces to `a`; `i++` and `x += 1` desugar to assignments;
//! C-style `for`, `for...of`, `do/while` map to the unified `Loop`; `switch`
//! becomes an `if`/`else if` chain; ternary lowers to an expression `If`.

use crate::lower::Lowering;
use nose_il::{
    contains_js_identifier, is_js_identifier_continue, stable_symbol_hash, Builtin, EvidenceAnchor,
    EvidenceKind, FileId, GuardEvidenceKind, Il, Interner, JsRecordGuardComparison,
    JsRecordGuardNullCheck, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
    SourceCallKind, SourceFactKind, SourceLiteralKind, SourceOperatorKind, Span,
    SymbolEvidenceKind, UnitKind,
};
use nose_semantics::{
    js_array_is_array_contract, js_boolean_coercion_contract, qualified_global_symbol_contract,
    static_global_symbol_contract,
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
    if text.starts_with("import type ") {
        return None;
    }
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
            if part.starts_with("type ") {
                continue;
            }
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
        "undefined" => lower_js_static_global_or_var(lo, node),
        "regex" => {
            let lit = lo.str_lit(lo.text(node), span);
            lo.record_source_fact(span, SourceFactKind::Literal(SourceLiteralKind::Regex));
            lit
        }
        "call_expression" => lower_call(lo, node),
        "new_expression" => lower_new(lo, node),
        "binary_expression" => {
            lower_record_shape_guard(lo, node).unwrap_or_else(|| lower_binary(lo, node))
        }
        "unary_expression" => lower_unary(lo, node),
        "update_expression" => lower_update(lo, node),
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr),
        "augmented_assignment_expression" => lower_aug_assignment(lo, node),
        "member_expression" => lower_member_expr(lo, node),
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
        "await_expression" => {
            let value = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.await_boundary(span, value)
        }
        // TS-only wrappers have no runtime behavior.
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
        "yield_expression" => {
            let value = node.named_child(0).map(|c| lower_expr(lo, c));
            lo.yield_boundary(span, value)
        }
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
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("function") {
        Some(f) => kids.push(lower_callee_expr(lo, f)),
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

fn lower_own_property_guard_call(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let callee = node.child_by_field_name("function")?;
    let callee_text = compact_js_expr(lo.text(callee));
    let contract = qualified_global_symbol_contract(lo.lang, &callee_text)?;
    if !matches!(
        contract.path,
        "Object.hasOwn" | "Object.prototype.hasOwnProperty.call"
    ) {
        return None;
    }
    if contract.requires_unshadowed_root
        && (file_prefix_has_binding_ident(lo, node, contract.root)
            || enclosing_function_prefix_has_binding_ident(lo, node, contract.root))
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
    let guard = lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[receiver, key, own, present],
    );
    let api_dependency = lo.record_qualified_global_symbol(span, NodeKind::Seq, contract.path);
    lo.record_evidence_with_dependencies(
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash(contract.path),
        }),
        "own_property_guard_js_api",
        vec![api_dependency],
    );
    Some(guard)
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
                if contains_js_identifier(header, ident) || contains_js_binding_ident(prefix, ident)
                {
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
        if before.is_some_and(is_js_identifier_continue) {
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
        starts_with_js_ident(rest, ident) || destructuring_pattern_binds_ident(rest, ident)
    })
}

fn destructuring_pattern_binds_ident(text: &str, ident: &str) -> bool {
    if !matches!(text.chars().next(), Some('{') | Some('[')) {
        return false;
    }
    let pattern = text.split_once('=').map(|(lhs, _)| lhs).unwrap_or(text);
    contains_js_identifier(pattern, ident)
}

fn contains_import_binding(text: &str, ident: &str) -> bool {
    text.match_indices("import").any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        if before.is_some_and(is_js_identifier_continue) {
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

fn starts_with_js_ident(text: &str, ident: &str) -> bool {
    text.starts_with(ident)
        && !text[ident.len()..]
            .chars()
            .next()
            .is_some_and(is_js_identifier_continue)
}

fn lower_callee_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

fn lower_member_object(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

fn lower_member_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let obj = node
        .child_by_field_name("object")
        .map(|o| lower_member_object(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let prop = node
        .child_by_field_name("property")
        .map(|p| lo.text(p))
        .unwrap_or("");
    let sym = lo.sym(prop);
    let field = lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj]);
    let path = compact_js_expr(lo.text(node));
    if let Some(contract) = qualified_global_symbol_contract(lo.lang, &path) {
        let root_unshadowed = !contract.requires_unshadowed_root
            || (!file_prefix_has_binding_ident(lo, node, contract.root)
                && !enclosing_function_prefix_has_binding_ident(lo, node, contract.root));
        if root_unshadowed {
            lo.record_qualified_global_symbol(span, NodeKind::Field, contract.path);
        }
    }
    field
}

fn lower_constructor_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

fn lower_js_static_global_or_var(lo: &mut Lowering, node: TsNode) -> NodeId {
    let name = lo.text(node);
    let span = lo.span(node);
    if js_static_global_unshadowed_at(lo, node, name) {
        lo.unshadowed_global_var(name, span)
    } else {
        lo.var(name, span)
    }
}

fn js_static_global_unshadowed_at(lo: &Lowering, node: TsNode, name: &str) -> bool {
    let Some(contract) = static_global_symbol_contract(lo.lang, name) else {
        return false;
    };
    if !contract.requires_unshadowed {
        return true;
    }
    !file_prefix_has_binding_ident(lo, node, contract.name)
        && !enclosing_function_prefix_has_binding_ident(lo, node, contract.name)
}

fn lower_new(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("constructor") {
        Some(c) => kids.push(lower_constructor_expr(lo, c)),
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
    lo.record_source_fact(span, SourceFactKind::Call(SourceCallKind::Construct));
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
    let source_operator = node
        .child_by_field_name("operator")
        .map(|op| lo.text(op))
        .and_then(js_source_operator);
    let lowered = crate::lower::binary(lo, node, js_bin_op, lower_expr);
    if let Some(source_operator) = source_operator {
        lo.record_source_fact(lo.span(node), SourceFactKind::Operator(source_operator));
    }
    lowered
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
    let mut null_check = None;
    let mut has_not_array = false;
    let mut comparison = JsRecordGuardComparison::StrictOnly;
    for clause in clauses {
        let parsed = record_guard_clause(&clause)?;
        let name = parsed.name;
        if !simple_js_ident(&name) {
            return None;
        }
        match &ident {
            Some(current) if current != &name => return None,
            None => ident = Some(name.clone()),
            _ => {}
        }
        comparison = merge_record_guard_comparison(comparison, parsed.comparison);
        match parsed.kind {
            RecordGuardClause::TypeofObject => has_typeof_object = true,
            RecordGuardClause::NonNullOrTruthy { kind } => null_check = Some(kind),
            RecordGuardClause::NotArray => has_not_array = true,
        }
    }

    if !(has_typeof_object && null_check.is_some() && has_not_array) {
        return None;
    }
    let array_contract = js_array_is_array_contract(lo.lang, "Array", "isArray", 1)?;
    if array_contract.requires_unshadowed_receiver
        && (file_prefix_has_binding_ident(lo, node, array_contract.receiver)
            || enclosing_function_prefix_has_binding_ident(lo, node, array_contract.receiver))
    {
        return None;
    }
    let null_check = null_check?;
    let requires_boolean_global = null_check == JsRecordGuardNullCheck::BooleanGlobalTruthy;
    let boolean_contract = requires_boolean_global
        .then(|| js_boolean_coercion_contract(lo.lang, "Boolean", 1))
        .flatten();
    if requires_boolean_global && boolean_contract.is_none() {
        return None;
    }
    if boolean_contract.is_some_and(|contract| {
        contract.requires_unshadowed_function
            && (file_prefix_has_binding_ident(lo, node, contract.function)
                || enclosing_function_prefix_has_binding_ident(lo, node, contract.function))
    }) {
        return None;
    }
    let ident = ident?;
    let span = lo.span(node);
    let value = lo.var(&ident, span);
    let object = lo.str_lit("object", span);
    let non_null = lo.str_lit("non_null", span);
    let not_array = lo.str_lit("not_array", span);
    let tag = lo.sym("record_guard");
    let guard = lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[value, object, non_null, not_array],
    );
    let array_dependency = lo.record_qualified_global_source_symbol(
        span,
        array_contract.qualified_path,
        "record_guard_array_is_array_api",
    );
    let mut dependencies = vec![array_dependency];
    if let Some(contract) = boolean_contract {
        dependencies.push(lo.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(contract.function),
            }),
            "record_guard_boolean_api",
        ));
    }
    lo.record_evidence_with_dependencies(
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
            subject_hash: stable_symbol_hash(&ident),
            null_check,
            comparison,
        }),
        "record_guard_js_shape",
        dependencies,
    );
    Some(guard)
}

#[derive(Clone, Copy)]
enum RecordGuardClause {
    TypeofObject,
    NonNullOrTruthy { kind: JsRecordGuardNullCheck },
    NotArray,
}

struct ParsedRecordGuardClause {
    kind: RecordGuardClause,
    name: String,
    comparison: JsRecordGuardComparison,
}

fn record_guard_clause(clause: &str) -> Option<ParsedRecordGuardClause> {
    parse_typeof_object_clause(clause)
        .map(|(name, comparison)| ParsedRecordGuardClause {
            kind: RecordGuardClause::TypeofObject,
            name,
            comparison,
        })
        .or_else(|| {
            parse_non_null_clause(clause).map(|(name, kind, comparison)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NonNullOrTruthy { kind },
                name,
                comparison,
            })
        })
        .or_else(|| {
            parse_truthy_clause(clause).map(|(name, kind)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NonNullOrTruthy { kind },
                name,
                comparison: JsRecordGuardComparison::StrictOnly,
            })
        })
        .or_else(|| {
            parse_not_array_clause(clause).map(|(name, comparison)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NotArray,
                name,
                comparison,
            })
        })
}

fn parse_typeof_object_clause(clause: &str) -> Option<(String, JsRecordGuardComparison)> {
    for op in ["===", "=="] {
        let comparison = record_guard_comparison_for_op(op);
        if let Some(rest) = clause.strip_prefix("typeof ") {
            let (name, value) = rest.split_once(op)?;
            if is_object_literal(value) {
                return Some((name.to_string(), comparison));
            }
        }
        for object_lit in ["'object'", "\"object\""] {
            let prefix = format!("{object_lit}{op}typeof ");
            if let Some(name) = clause.strip_prefix(&prefix) {
                return Some((name.to_string(), comparison));
            }
        }
    }
    None
}

fn parse_non_null_clause(
    clause: &str,
) -> Option<(String, JsRecordGuardNullCheck, JsRecordGuardComparison)> {
    for op in ["!==", "!="] {
        let null_check = match op {
            "!==" => JsRecordGuardNullCheck::StrictNonNull,
            "!=" => JsRecordGuardNullCheck::LooseNonNull,
            _ => unreachable!(),
        };
        let comparison = record_guard_comparison_for_op(op);
        if let Some((name, "null")) = clause.split_once(op) {
            return Some((name.to_string(), null_check, comparison));
        }
        let prefix = format!("null{op}");
        if let Some(name) = clause.strip_prefix(&prefix) {
            return Some((name.to_string(), null_check, comparison));
        }
    }
    None
}

fn parse_truthy_clause(clause: &str) -> Option<(String, JsRecordGuardNullCheck)> {
    if let Some(name) = clause.strip_prefix("!!") {
        return Some((
            name.to_string(),
            JsRecordGuardNullCheck::DoubleNegationTruthy,
        ));
    }
    clause
        .strip_prefix("Boolean(")
        .and_then(|inner| inner.strip_suffix(')'))
        .map(|name| {
            (
                name.to_string(),
                JsRecordGuardNullCheck::BooleanGlobalTruthy,
            )
        })
}

fn parse_not_array_clause(clause: &str) -> Option<(String, JsRecordGuardComparison)> {
    if let Some(name) = clause
        .strip_prefix("!Array.isArray(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        return Some((name.to_string(), JsRecordGuardComparison::StrictOnly));
    }
    for op in ["===", "=="] {
        let comparison = record_guard_comparison_for_op(op);
        if let Some(call) = clause.strip_suffix(&format!("{op}false")) {
            if let Some(name) = call
                .strip_prefix("Array.isArray(")
                .and_then(|inner| inner.strip_suffix(')'))
            {
                return Some((name.to_string(), comparison));
            }
        }
        let prefix = format!("false{op}Array.isArray(");
        if let Some(name) = clause
            .strip_prefix(&prefix)
            .and_then(|inner| inner.strip_suffix(')'))
        {
            return Some((name.to_string(), comparison));
        }
    }
    None
}

fn record_guard_comparison_for_op(op: &str) -> JsRecordGuardComparison {
    match op {
        "==" | "!=" => JsRecordGuardComparison::LooseEqualityAllowed,
        _ => JsRecordGuardComparison::StrictOnly,
    }
}

fn merge_record_guard_comparison(
    left: JsRecordGuardComparison,
    right: JsRecordGuardComparison,
) -> JsRecordGuardComparison {
    if matches!(
        (left, right),
        (JsRecordGuardComparison::LooseEqualityAllowed, _)
            | (_, JsRecordGuardComparison::LooseEqualityAllowed)
    ) {
        JsRecordGuardComparison::LooseEqualityAllowed
    } else {
        JsRecordGuardComparison::StrictOnly
    }
}

fn is_object_literal(value: &str) -> bool {
    matches!(value, "'object'" | "\"object\"")
}

fn compact_js_expr(text: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
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
        } else if c.is_whitespace() {
            let next = chars.clone().find(|next| !next.is_whitespace());
            if out
                .chars()
                .next_back()
                .is_some_and(is_js_identifier_continue)
                && next.is_some_and(is_js_identifier_continue)
            {
                out.push(' ');
            }
        } else {
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
            lo.record_source_fact(span, SourceFactKind::Operator(SourceOperatorKind::Typeof));
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
        .map(|x| lower_object_pair_key(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    let value = node
        .child_by_field_name("value")
        .map(|x| lower_expr(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
    let tag = lo.sym("pair");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &[key, value])
}

fn lower_object_pair_key(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(key) = static_object_property_key(lo, node) {
        return lo.str_lit(&key, span);
    }
    let inner: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    lo.raw(node.kind(), span, &inner)
}

fn static_object_property_key(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "property_identifier" | "identifier" => Some(lo.text(node).to_string()),
        "string" => static_string_key(lo, node),
        "number" => Some(lo.text(node).to_string()),
        _ => None,
    }
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

fn js_source_operator(text: &str) -> Option<SourceOperatorKind> {
    match text {
        "===" => Some(SourceOperatorKind::StrictEquality),
        "!==" => Some(SourceOperatorKind::StrictInequality),
        "==" => Some(SourceOperatorKind::LooseEquality),
        "!=" => Some(SourceOperatorKind::LooseInequality),
        "instanceof" => Some(SourceOperatorKind::TypeMembership),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, EvidenceAnchor, EvidenceKind, FileId, GuardEvidenceKind, Interner,
        JsRecordGuardComparison, JsRecordGuardNullCheck, LibraryApiEvidenceKind,
        SourceProtocolKind, SymbolEvidenceKind,
    };
    use nose_semantics::{
        library_api_callee_contract_hash, library_api_contract_id_hash,
        library_js_array_is_array_contract, library_js_like_map_constructor_contract,
        library_js_like_set_constructor_contract, library_map_key_view_wrapper_contract,
    };

    fn lower_js(src: &str) -> Il {
        let interner = Interner::new();
        crate::lower_source(
            FileId(0),
            "t.js",
            src.as_bytes(),
            Lang::JavaScript,
            &interner,
        )
        .expect("lower js")
    }

    fn unshadowed_global_evidence_count(il: &Il, name: &str) -> usize {
        let expected = stable_symbol_hash(name);
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                        if name_hash == expected
                )
            })
            .count()
    }

    fn qualified_global_evidence_count(il: &Il, path: &str, kind: NodeKind) -> usize {
        qualified_global_evidence_records(il, path, kind).len()
    }

    fn qualified_global_evidence_records<'a>(
        il: &'a Il,
        path: &str,
        kind: NodeKind,
    ) -> Vec<&'a nose_il::EvidenceRecord> {
        let expected = stable_symbol_hash(path);
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.anchor,
                    EvidenceAnchor::Node {
                        kind: anchor_kind,
                        ..
                    } if anchor_kind == kind
                ) && matches!(
                    record.kind,
                    EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                        if path_hash == expected
                )
            })
            .collect()
    }

    fn evidence_anchor_span(anchor: EvidenceAnchor) -> Span {
        match anchor {
            EvidenceAnchor::SourceSpan(span)
            | EvidenceAnchor::Node { span, .. }
            | EvidenceAnchor::Param { span }
            | EvidenceAnchor::Binding { span, .. }
            | EvidenceAnchor::Sequence { span } => span,
        }
    }

    fn evidence_by_id(il: &Il, id: nose_il::EvidenceId) -> Option<&nose_il::EvidenceRecord> {
        il.evidence
            .get(id.0 as usize)
            .filter(|record| record.id == id)
            .or_else(|| il.evidence.iter().find(|record| record.id == id))
    }

    fn record_has_source_unshadowed_dependency(
        il: &Il,
        record: &nose_il::EvidenceRecord,
        root: &str,
    ) -> bool {
        let span = evidence_anchor_span(record.anchor);
        let expected = stable_symbol_hash(root);
        record.dependencies.iter().any(|&id| {
            evidence_by_id(il, id).is_some_and(|dependency| {
                dependency.anchor == EvidenceAnchor::source_span(span)
                    && matches!(
                        dependency.kind,
                        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                            if name_hash == expected
                    )
            })
        })
    }

    fn record_has_qualified_global_dependency_with_root(
        il: &Il,
        record: &nose_il::EvidenceRecord,
        path: &str,
        root: &str,
    ) -> bool {
        let expected = stable_symbol_hash(path);
        record.dependencies.iter().any(|&id| {
            evidence_by_id(il, id).is_some_and(|dependency| {
                matches!(
                    dependency.kind,
                    EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                        if path_hash == expected
                ) && record_has_source_unshadowed_dependency(il, dependency, root)
            })
        })
    }

    fn source_operator_evidence_count(il: &Il, operator: SourceOperatorKind) -> usize {
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Source(SourceFactKind::Operator(actual)) if actual == operator
                )
            })
            .count()
    }

    fn library_api_evidence_count(
        il: &Il,
        contract_hash: u64,
        callee_hash: u64,
        arity: u16,
    ) -> usize {
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        arity: actual_arity,
                    }) if actual_contract == contract_hash
                        && actual_callee == callee_hash
                        && actual_arity == arity
                )
            })
            .count()
    }

    fn library_api_dependency_counts(il: &Il) -> Vec<usize> {
        il.evidence
            .iter()
            .filter_map(|record| {
                matches!(record.kind, EvidenceKind::LibraryApi(_))
                    .then_some(record.dependencies.len())
            })
            .collect()
    }

    fn library_api_dependency_counts_for(
        il: &Il,
        contract_hash: u64,
        callee_hash: u64,
        arity: u16,
    ) -> Vec<usize> {
        il.evidence
            .iter()
            .filter_map(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        arity: actual_arity,
                    }) if actual_contract == contract_hash
                        && actual_callee == callee_hash
                        && actual_arity == arity
                )
                .then_some(record.dependencies.len())
            })
            .collect()
    }

    fn js_record_shape_guard_evidence(il: &Il) -> Vec<&nose_il::EvidenceRecord> {
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape { .. })
                )
            })
            .collect()
    }

    #[test]
    fn await_expression_preserves_source_backed_async_boundary() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.js",
            b"async function f(x) { return await x + 1; }",
            Lang::JavaScript,
            &interner,
        )
        .expect("lower js");

        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "await",
            SourceProtocolKind::Await,
        );
    }

    #[test]
    fn yield_expression_preserves_source_backed_protocol_boundary() {
        let interner = Interner::new();
        let il = lower(
            FileId(0),
            "t.js",
            b"function* f(x) { yield x + 1; }",
            Lang::JavaScript,
            &interner,
        )
        .expect("lower js");

        crate::test_helpers::expect_raw_protocol_boundary(
            &il,
            &interner,
            "yield",
            SourceProtocolKind::Yield,
        );
    }

    fn js_own_property_guard_evidence(il: &Il) -> Vec<&nose_il::EvidenceRecord> {
        il.evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty { .. })
                )
            })
            .collect()
    }

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

    #[test]
    fn js_static_global_value_occurrences_emit_symbol_evidence() {
        let il = lower_js(
            "function f(value) {
                console.log(Math.abs(value));
                const picked = new Map([[\"x\", 1]]).get(\"x\") ?? undefined;
                return Array.isArray(value) || new Set([value]).has(value) || picked;
            }",
        );

        for name in ["console", "Math", "Map", "undefined", "Array", "Set"] {
            assert!(
                unshadowed_global_evidence_count(&il, name) >= 1,
                "missing global evidence for {name}"
            );
        }
        let map = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
        assert!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(map.id),
                library_api_callee_contract_hash(map.callee),
                1,
            ) >= 1
        );
        let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
        assert!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(set.id),
                library_api_callee_contract_hash(set.callee),
                1,
            ) >= 1
        );
        let is_array =
            library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1).unwrap();
        assert!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(is_array.id),
                library_api_callee_contract_hash(is_array.callee),
                1,
            ) >= 1
        );
        assert!(
            library_api_dependency_counts(&il)
                .into_iter()
                .all(|count| count >= 1),
            "selected JS API evidence should depend on explicit proof"
        );
        for (id, callee, arity) in [
            (map.id, map.callee, 1),
            (set.id, set.callee, 1),
            (is_array.id, is_array.callee, 1),
        ] {
            assert!(
                library_api_dependency_counts_for(
                    &il,
                    library_api_contract_id_hash(id),
                    library_api_callee_contract_hash(callee),
                    arity,
                )
                .into_iter()
                .all(|count| count >= 2),
                "selected JS static/global API evidence should depend on source/symbol proof"
            );
        }
        assert!(
            !il.nodes
                .iter()
                .any(|node| matches!(node.payload, Payload::Builtin(Builtin::Abs))),
            "Math.abs should stay as Field(Var(Math), abs) for semantic consumers"
        );
    }

    #[test]
    fn js_static_global_evidence_respects_local_and_destructured_shadows() {
        let il = lower_js(
            "function f(Math, value) { return Math.abs(value); }
             function g(scope) { const { Map } = scope; return new Map([]); }
             function h(value, undefined) { return value === undefined; }
             function i(value) { const Array = { isArray() { return false; } }; return Array.isArray(value); }",
        );

        for name in ["Math", "Map", "undefined", "Array"] {
            assert_eq!(
                unshadowed_global_evidence_count(&il, name),
                0,
                "shadowed {name} should not get global evidence"
            );
        }
    }

    #[test]
    fn js_typeof_unary_emits_source_operator_evidence() {
        let il = lower_js("function real(value) { return typeof value === \"string\"; }");
        assert_eq!(
            source_operator_evidence_count(&il, SourceOperatorKind::Typeof),
            1
        );
    }

    #[test]
    fn js_qualified_global_paths_emit_symbol_evidence() {
        let il = lower_js(
            "function hasOwn(value) { return Object.hasOwn(value, \"ready\"); }
             function hasOwnCall(value) { return Object.prototype.hasOwnProperty.call(value, \"ready\"); }
             function fromKeys(lookup) { return Array.from(lookup.keys()).includes(\"ready\"); }
             function isArray(value) { return Array.isArray(value); }",
        );

        assert_eq!(
            qualified_global_evidence_count(&il, "Object.hasOwn", NodeKind::Seq),
            1
        );
        assert_eq!(
            qualified_global_evidence_count(
                &il,
                "Object.prototype.hasOwnProperty.call",
                NodeKind::Seq
            ),
            1
        );
        assert_eq!(
            qualified_global_evidence_count(&il, "Array.from", NodeKind::Field),
            1
        );
        assert_eq!(
            qualified_global_evidence_count(&il, "Array.isArray", NodeKind::Field),
            1
        );
        for (path, kind, root) in [
            ("Object.hasOwn", NodeKind::Seq, "Object"),
            (
                "Object.prototype.hasOwnProperty.call",
                NodeKind::Seq,
                "Object",
            ),
            ("Array.from", NodeKind::Field, "Array"),
            ("Array.isArray", NodeKind::Field, "Array"),
        ] {
            for record in qualified_global_evidence_records(&il, path, kind) {
                assert!(
                    record_has_source_unshadowed_dependency(&il, record, root),
                    "{path} evidence should depend on an unshadowed {root} proof"
                );
            }
        }
        let from =
            library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap();
        let is_array =
            library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1).unwrap();
        assert_eq!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(from.id),
                library_api_callee_contract_hash(from.callee),
                1,
            ),
            1
        );
        assert_eq!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(is_array.id),
                library_api_callee_contract_hash(is_array.callee),
                1,
            ),
            1
        );
    }

    #[test]
    fn js_own_property_guards_emit_guard_evidence_with_api_dependencies() {
        let il = lower_js(
            "function hasOwn(value) { return Object.hasOwn(value, \"ready\"); }
             function hasOwnCall(value) { return Object.prototype.hasOwnProperty.call(value, \"ready\"); }",
        );

        let guards = js_own_property_guard_evidence(&il);
        assert_eq!(guards.len(), 2);
        assert!(guards.iter().any(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty { api_path_hash })
                    if api_path_hash == stable_symbol_hash("Object.hasOwn")
            ) && record.dependencies.len() == 1
                && record_has_qualified_global_dependency_with_root(
                    &il,
                    record,
                    "Object.hasOwn",
                    "Object",
                )
        }));
        assert!(guards.iter().any(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty { api_path_hash })
                    if api_path_hash == stable_symbol_hash("Object.prototype.hasOwnProperty.call")
            ) && record.dependencies.len() == 1
                && record_has_qualified_global_dependency_with_root(
                    &il,
                    record,
                    "Object.prototype.hasOwnProperty.call",
                    "Object",
                )
        }));
    }

    #[test]
    fn js_record_shape_guards_emit_guard_evidence_with_api_dependencies() {
        let il = lower_js(
            "function direct(value) {
                return typeof value === \"object\" && value !== null && !Array.isArray(value);
             }
             function truthy(input) {
                return Boolean(input) && typeof input === \"object\" && !Array.isArray(input);
             }",
        );

        let guards = js_record_shape_guard_evidence(&il);
        assert_eq!(guards.len(), 2);
        assert!(guards.iter().any(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
                    subject_hash,
                    null_check: JsRecordGuardNullCheck::StrictNonNull,
                    comparison: JsRecordGuardComparison::StrictOnly,
                }) if subject_hash == stable_symbol_hash("value")
            ) && record.dependencies.len() == 1
                && record_has_qualified_global_dependency_with_root(
                    &il,
                    record,
                    "Array.isArray",
                    "Array",
                )
        }));
        assert!(guards.iter().any(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
                    subject_hash,
                    null_check: JsRecordGuardNullCheck::BooleanGlobalTruthy,
                    comparison: JsRecordGuardComparison::StrictOnly,
                }) if subject_hash == stable_symbol_hash("input")
            ) && record.dependencies.len() == 2
                && record_has_qualified_global_dependency_with_root(
                    &il,
                    record,
                    "Array.isArray",
                    "Array",
                )
        }));
    }

    #[test]
    fn js_record_shape_guard_evidence_respects_array_shadowing() {
        let il = lower_js(
            "function f(Array, value) {
                return typeof value === \"object\" && value !== null && !Array.isArray(value);
             }
             function g(scope, value) {
                const { Array } = scope;
                return typeof value === \"object\" && value !== null && !Array.isArray(value);
             }",
        );

        assert!(js_record_shape_guard_evidence(&il).is_empty());
    }

    #[test]
    fn js_record_shape_guard_evidence_requires_typeof_keyword_boundary() {
        let il = lower_js(
            "function f(value) {
                return typeofvalue === \"object\" && value !== null && !Array.isArray(value);
             }
             function g(value) {
                return \"object\" === typeofvalue && value !== null && !Array.isArray(value);
             }",
        );

        assert!(js_record_shape_guard_evidence(&il).is_empty());
    }

    #[test]
    fn js_qualified_global_evidence_respects_shadowed_roots() {
        let il = lower_js(
            "function a(Object, value) { return Object.hasOwn(value, \"ready\"); }
             const Object = { prototype: { hasOwnProperty: { call() { return true; } } } };
             function b(value) { return Object.prototype.hasOwnProperty.call(value, \"ready\"); }
             function c(Array, lookup) { return Array.from(lookup.keys()).includes(\"ready\"); }
             function d(scope, lookup) { const { Array } = scope; return Array.from(lookup.keys()).includes(\"ready\"); }",
        );

        assert_eq!(
            qualified_global_evidence_count(&il, "Object.hasOwn", NodeKind::Seq),
            0
        );
        assert_eq!(
            qualified_global_evidence_count(
                &il,
                "Object.prototype.hasOwnProperty.call",
                NodeKind::Seq
            ),
            0
        );
        assert_eq!(
            qualified_global_evidence_count(&il, "Array.from", NodeKind::Field),
            0
        );
        assert!(
            il.evidence
                .iter()
                .all(|record| !matches!(record.kind, EvidenceKind::LibraryApi(_))),
            "shadowed JS static globals must not emit API contract evidence"
        );
        assert!(js_own_property_guard_evidence(&il).is_empty());
    }
}
