//! C → raw IL lowering.
//!
//! Convergence-friendly lowering: `x op= y` / `x++` desugar to assignments; `for`,
//! `while`, `do` map to the unified `Loop`; `switch` becomes an `if`/`else if`
//! chain; `function_definition` becomes a function unit. struct/union/enum are
//! data definitions (not unit-ified). `*p`, `&x`, casts peel to the operand.

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, ParamSemantic, Payload,
    UnitKind,
};
use std::{fs, path::Path};
use tree_sitter::Node as TsNode;

const C_INCLUDE_ALIAS_READ_LIMIT: u64 = 256 * 1024;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
) -> anyhow::Result<Il> {
    crate::lower::lower_file_with_setup(
        file,
        path,
        src,
        interner,
        crate::lower::grammar::C,
        || tree_sitter_c::LANGUAGE.into(),
        Lang::C,
        |lo| record_c_direct_include_type_aliases(path, src, lo),
        lower_items,
    )
}

fn record_c_direct_include_type_aliases(path: &str, src: &[u8], lo: &mut Lowering) {
    let Ok(source) = std::str::from_utf8(src) else {
        return;
    };
    if !contains_c_identifier(source, "u8") || !c_source_may_contain_u16_byte_pack(source) {
        return;
    }
    let Some(dir) = Path::new(path).parent() else {
        return;
    };
    for line in source.lines() {
        let Some(include) = c_direct_quote_include_name(line) else {
            continue;
        };
        if include.is_empty() || include.contains('/') || include.contains('\\') {
            continue;
        }
        let header = dir.join(include);
        let Ok(meta) = fs::metadata(&header) else {
            continue;
        };
        if !meta.is_file() || meta.len() > C_INCLUDE_ALIAS_READ_LIMIT {
            continue;
        }
        let Ok(header_text) = fs::read_to_string(&header) else {
            continue;
        };
        for header_line in header_text.lines() {
            if let Some(alias) = c_unsigned_char_typedef_alias(header_line) {
                if contains_c_identifier(source, &alias) {
                    lo.record_param_semantic_alias(&alias, ParamSemantic::ByteArray);
                }
            }
        }
    }
}

fn c_direct_quote_include_name(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let rest = line.strip_prefix('#')?.trim_start();
    let rest = rest.strip_prefix("include")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn contains_c_identifier(text: &str, ident: &str) -> bool {
    text.match_indices(ident).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + ident.len()..].chars().next();
        !before.is_some_and(is_c_identifier_char) && !after.is_some_and(is_c_identifier_char)
    })
}

fn is_c_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn c_source_may_contain_u16_byte_pack(source: &str) -> bool {
    source.contains("[0]")
        && source.contains("[1]")
        && (source.contains("<<8") || source.contains("<< 8"))
}

fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    collect_top_items(lo, node, &mut kids);
    lo.add(NodeKind::Module, Payload::None, span, &kids)
}

/// Collect top-level items, descending through preprocessor conditionals. A function
/// guarded by `#if PLATFORM … #endif` (ubiquitous in C: nginx/curl per-OS code) lives
/// *inside* a `preproc_if`/`preproc_ifdef` node, so a flat scan of the translation
/// unit would discard it entirely — the file would lower to an empty module and its
/// functions become invisible to detection. Recurse into the conditional's body
/// (skipping the condition/macro-name field, which is not an item).
fn collect_top_items(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    let skip = node
        .child_by_field_name("condition")
        .or_else(|| node.child_by_field_name("name"))
        .map(|n| n.id());
    for c in Lowering::named_children(node) {
        if Some(c.id()) == skip {
            continue; // the `#if COND` / `#ifdef NAME` test, not an item
        }
        match c.kind() {
            "preproc_if"
            | "preproc_ifdef"
            | "preproc_else"
            | "preproc_elif"
            | "preproc_elifdef"
            | "linkage_specification" => collect_top_items(lo, c, out),
            _ => {
                if let Some(n) = lower_item(lo, c) {
                    out.push(n);
                }
            }
        }
    }
}

fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_definition" => Some(lower_func(lo, node)),
        "declaration" => Some(lower_decl(lo, node)),
        "preproc_include" => Some(crate::lower::import_tokens(lo, node)),
        "type_definition" => {
            record_c_type_definition(lo, node);
            None
        }
        "preproc_def"
        | "preproc_function_def"
        | "preproc_ifdef"
        | "preproc_if"
        | "struct_specifier"
        | "union_specifier"
        | "enum_specifier"
        | "comment" => None,
        _ => lower_stmt(lo, node),
    }
}

/// Find the binding identifier inside a (possibly pointer/array) C declarator.
fn declarator_name(lo: &Lowering, node: TsNode) -> Option<nose_il::Symbol> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => Some(lo.sym(lo.text(node))),
        _ => node
            .child_by_field_name("declarator")
            .or_else(|| node.named_child(0))
            .and_then(|c| declarator_name(lo, c)),
    }
}

fn lower_func(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let decl = node.child_by_field_name("declarator");
    let name = decl.and_then(|d| declarator_name(lo, d));
    let mut kids = Vec::new();
    // parameters live under the function_declarator's parameter_list
    if let Some(d) = decl {
        if let Some(params) = find_param_list(d) {
            for p in Lowering::named_children(params) {
                let pspan = lo.span(p);
                let sym = p
                    .child_by_field_name("declarator")
                    .and_then(|x| declarator_name(lo, x));
                if let Some(semantic) = c_param_semantic_from_text(lo, lo.text(p)) {
                    lo.record_param_semantic(pspan, semantic);
                }
                kids.push(lo.add(
                    NodeKind::Param,
                    sym.map(Payload::Name).unwrap_or(Payload::None),
                    pspan,
                    &[],
                ));
            }
        }
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(func, UnitKind::Function, name);
    func
}

fn record_c_type_definition(lo: &mut Lowering, node: TsNode) {
    if let Some(alias) = c_unsigned_char_typedef_alias(lo.text(node)) {
        lo.record_param_semantic_alias(&alias, ParamSemantic::ByteArray);
    }
}

fn c_unsigned_char_typedef_alias(text: &str) -> Option<String> {
    let compact = compact_c_type_text(text);
    let rest = compact.strip_prefix("typedefunsignedchar")?;
    let alias = rest.strip_suffix(';').unwrap_or(rest);
    if is_c_identifier(alias) {
        Some(alias.to_string())
    } else {
        None
    }
}

fn c_param_semantic_from_text(lo: &Lowering, text: &str) -> Option<ParamSemantic> {
    if c_byte_buffer_param(lo, text) {
        Some(ParamSemantic::ByteArray)
    } else {
        crate::lower::param_semantic_from_text(text)
    }
}

fn c_byte_buffer_param(lo: &Lowering, text: &str) -> bool {
    let compact = compact_c_type_text(text);
    if !(compact.contains('*') || compact.contains('[')) {
        return false;
    }
    let tokens = c_identifier_tokens(text);
    if tokens.iter().any(|token| token == "uint8_t")
        || (tokens.iter().any(|token| token == "unsigned")
            && tokens.iter().any(|token| token == "char"))
    {
        return true;
    }
    lo.param_semantic_aliases.iter().any(|(alias, semantic)| {
        *semantic == ParamSemantic::ByteArray && tokens.iter().any(|token| token == alias)
    })
}

fn compact_c_type_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn c_identifier_tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn is_c_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(ch) if ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn find_param_list(decl: TsNode) -> Option<TsNode> {
    if decl.kind() == "parameter_list" {
        return Some(decl);
    }
    Lowering::named_children(decl).into_iter().find_map(|c| {
        if c.kind() == "parameter_list" {
            Some(c)
        } else {
            find_param_list(c)
        }
    })
}

fn lower_decl(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut assigns = Vec::new();
    for d in Lowering::named_children(node) {
        let (name, value) = match d.kind() {
            "init_declarator" => (
                d.child_by_field_name("declarator")
                    .and_then(|x| declarator_name(lo, x)),
                d.child_by_field_name("value"),
            ),
            "identifier" => (Some(lo.sym(lo.text(d))), None),
            _ => (declarator_name(lo, d), None),
        };
        if let Some(sym) = name {
            let lhs = lo.add(NodeKind::Var, Payload::Name(sym), span, &[]);
            let rhs = value
                .map(|v| lower_expr(lo, v))
                .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
            assigns.push(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]));
        }
    }
    if assigns.len() == 1 {
        assigns.pop().unwrap()
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, lower_stmt)
}

fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "compound_statement" => Some(lower_block(lo, node)),
        "declaration" => Some(lower_decl(lo, node)),
        "expression_statement" => {
            let c = node.named_child(0)?;
            match c.kind() {
                "assignment_expression" | "update_expression" => Some(lower_expr(lo, c)),
                _ => {
                    let e = lower_expr(lo, c);
                    Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
                }
            }
        }
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "while_statement" | "do_statement" => Some(lower_while(lo, node)),
        "switch_statement" => Some(lower_switch(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        // `label: stmt` (goto target) — lower the inner statement, drop the label.
        "labeled_statement" => Lowering::named_children(node)
            .into_iter()
            .next_back()
            .and_then(|s| lower_stmt(lo, s)),
        // `goto label` — a jump; model as Break (drop the label so it doesn't leak).
        "goto_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        // `#if`/`#ifdef`/… conditional compilation: lower the guarded statements as a
        // Block (skip the condition), so the code inside doesn't fall through to Raw.
        "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => {
            Some(lower_preproc(lo, node))
        }
        ";"
        | "comment"
        | "preproc_call"
        | "preproc_def"
        | "preproc_function_def"
        | "preproc_include" => None,
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

/// `#if COND … #else … #endif` and friends → a `Block` of the guarded statements,
/// skipping the condition/macro name (which carry no runtime behavior).
fn lower_preproc(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .or_else(|| node.child_by_field_name("name"));
    let mut kids = Vec::new();
    for c in Lowering::named_children(node) {
        if Some(c) == cond {
            continue;
        }
        if let Some(s) = lower_stmt(lo, c) {
            kids.push(s);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &kids)
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| stmt_as_block(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        // `else` clause wraps the alternative statement
        let inner = alt.named_child(0).unwrap_or(alt);
        kids.push(stmt_as_block(lo, inner));
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

fn stmt_as_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "compound_statement" {
        lower_block(lo, node)
    } else {
        let span = lo.span(node);
        let s = lower_stmt(lo, node);
        lo.block_of_stmt(span, s)
    }
}

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let init = node
        .child_by_field_name("initializer")
        .and_then(|n| lower_stmt(lo, n))
        .unwrap_or_else(|| lo.empty_block(span));
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let update = node
        .child_by_field_name("update")
        .map(|u| lower_expr(lo, u))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::CStyle),
        span,
        &[init, cond, update, body],
    )
}

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, stmt_as_block)
}

fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::switch_to_if_chain(lo, node, |k| k == "case_statement", lower_expr, lower_stmt)
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        // GCC statement-expression `({ stmt; …; expr; })` reaches here via
        // `parenthesized_expression`; lower its body as a Block so the inner
        // statements route through `lower_stmt` instead of falling to Raw.
        "compound_statement" => lower_block(lo, node),
        // `sizeof x` / `sizeof(T)` is a compile-time integer constant; the operand is
        // often a type (which would itself be Raw), so lower to an int literal.
        "sizeof_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        "identifier" | "field_identifier" | "type_identifier" => {
            if lo.text(node) == "NULL" {
                lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[])
            } else {
                lo.var(lo.text(node), span)
            }
        }
        "number_literal" => {
            let t = lo.text(node);
            let lower = t.to_ascii_lowercase();
            // In a hex literal (`0x…`) the digits e/E are hex digits, not a float exponent;
            // a hex float instead uses a `.` or a binary `p`/`P` exponent. A decimal literal
            // is a float if it has a `.` or an `e`/`E` exponent.
            let is_float = if lower.starts_with("0x") {
                lower.contains('.') || lower.contains('p')
            } else {
                lower.contains('.') || lower.contains('e')
            };
            if is_float {
                lo.float_lit(t, span)
            } else {
                lo.int_lit(t.trim_end_matches(['u', 'U', 'l', 'L']), span)
            }
        }
        "string_literal" | "concatenated_string" | "char_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "null" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => {
            let operand = node
                .child_by_field_name("argument")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            // Map by the operator token, not the whole node's text: `+` is `Pos`,
            // `-` is `Neg`, `!` is `Not`, `~` is `BitNot`. Reading only the leading
            // byte once collapsed `+x` and `~x` onto `Neg`.
            let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
                Some("+") => Op::Pos,
                Some("!") => Op::Not,
                Some("~") => Op::BitNot,
                _ => Op::Neg,
            };
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
        }
        // `*p`, `&x` pointer ops, and casts peel to the operand
        "pointer_expression" | "cast_expression" | "parenthesized_expression" => node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)))
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "assignment_expression" => {
            let l = node
                .child_by_field_name("left")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            let opt = node
                .child_by_field_name("operator")
                .map(|o| lo.text(o))
                .unwrap_or("=");
            let r = node
                .child_by_field_name("right")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            if opt.len() > 1 {
                if let Some(op) = common_bin_op(opt.trim_end_matches('=')) {
                    let l2 = node
                        .child_by_field_name("left")
                        .map(|x| lower_expr(lo, x))
                        .unwrap_or_else(|| lo.empty_block(span));
                    let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l2, r]);
                    return lo.add(NodeKind::Assign, Payload::None, span, &[l, bin]);
                }
            }
            lo.add(NodeKind::Assign, Payload::None, span, &[l, r])
        }
        "update_expression" => {
            let arg = node.child_by_field_name("argument");
            let operand = arg
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let operand2 = arg
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let one = lo.int_lit("1", span);
            // Decide by the operator TOKEN, scanning only this node's direct children:
            // a substring check on the whole text misreads a nested `--`/`++` in the
            // operand (e.g. `a[i--]++`, whose outer op is `++`).
            let op = if crate::lower::has_direct_token(node, "--") {
                Op::Sub
            } else {
                Op::Add
            };
            let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[operand2, one]);
            lo.add(NodeKind::Assign, Payload::None, span, &[operand, bin])
        }
        "call_expression" => {
            let mut kids = Vec::new();
            if let Some(f) = node.child_by_field_name("function") {
                kids.push(lower_expr(lo, f));
            }
            if let Some(args) = node.child_by_field_name("arguments") {
                for a in Lowering::named_children(args) {
                    kids.push(lower_expr(lo, a));
                }
            }
            lo.add(NodeKind::Call, Payload::None, span, &kids)
        }
        "field_expression" => {
            let base = node
                .child_by_field_name("argument")
                .map(|o| lower_expr(lo, o))
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
        "subscript_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "conditional_expression" => {
            let kids: Vec<NodeId> = ["condition", "consequence", "alternative"]
                .iter()
                .filter_map(|f| node.child_by_field_name(f))
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::If, Payload::None, span, &kids)
        }
        "initializer_list" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // Designated initializer `.field = v` / `[i] = v` → the value (the designator
        // is a field/index name, not behavior).
        "initializer_pair" => node
            .child_by_field_name("value")
            .or_else(|| Lowering::named_children(node).into_iter().next_back())
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lo.empty_block(span)),
        "field_designator" | "subscript_designator" => lo.var(lo.text(node), span),
        // `offsetof(T, m)` is a compile-time integer constant (like sizeof).
        "offsetof_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        // `a, b` comma expression → a sequence of its operands.
        "comma_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `NAME = value` enum constant → its value (or the name).
        "enumerator" => node
            .child_by_field_name("value")
            .map(|v| lower_expr(lo, v))
            .or_else(|| node.named_child(0).map(|n| lo.var(lo.text(n), span)))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type-level / declarator nodes reaching expression position (sizeof/casts/
        // compound literals, K&R decls, macro bodies) carry no behavior — erase.
        "primitive_type"
        | "sized_type_specifier"
        | "type_descriptor"
        | "parameter_declaration"
        | "parameter_list"
        | "abstract_pointer_declarator"
        | "function_declarator"
        | "storage_class_specifier"
        | "type_qualifier"
        | "ms_call_modifier"
        | "preproc_arg"
        | "preproc_defined" => lo.empty_block(span),
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
    crate::lower::binary(lo, node, common_bin_op, lower_expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect every `Op` carried by a `UnOp` node in the lowered IL.
    fn unary_ops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.c", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::UnOp)
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn unary_operators_lower_to_distinct_ops() {
        // Each C unary operator must lower to its own `Op`; in particular unary
        // plus is `Pos`, not `Neg` (the two were once indistinguishable).
        let ops = unary_ops("int f(int x){ int a=+x; int b=-x; int c=!x; int d=~x; return 0; }");
        assert!(
            ops.contains(&Op::Pos),
            "unary + should lower to Op::Pos, got {ops:?}"
        );
        assert!(
            ops.contains(&Op::Neg),
            "unary - should lower to Op::Neg, got {ops:?}"
        );
        assert!(
            ops.contains(&Op::Not),
            "unary ! should lower to Op::Not, got {ops:?}"
        );
        assert!(
            ops.contains(&Op::BitNot),
            "unary ~ should lower to Op::BitNot, got {ops:?}"
        );
    }

    #[test]
    fn unary_plus_and_minus_are_not_aliased() {
        // `+x` and `-x` must not collapse to the same operator.
        assert_eq!(unary_ops("int f(int x){ return +x; }"), vec![Op::Pos]);
        assert_eq!(unary_ops("int f(int x){ return -x; }"), vec![Op::Neg]);
    }

    /// Collect every `Op` carried by a `BinOp` node in the lowered IL.
    fn binops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.c", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::BinOp)
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn switch_case_rhs_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.c", src.as_bytes(), &interner).expect("lower");
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

    fn expr_stmt_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "t.c", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::ExprStmt)
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
    fn switch_cases_compare_scrutinee_to_case_literals() {
        let src =
            "int f(int x){ switch(x){ case 7: return 1; case 8: return 2; default: return 3; } }";
        assert_eq!(switch_case_rhs_ints(src), vec![7, 8]);
        assert!(
            expr_stmt_ints(src).is_empty(),
            "case labels should not remain as stray expression statements"
        );
    }

    #[test]
    fn postfix_increment_with_nested_decrement_in_operand() {
        // `a[i--]++` desugars to `a[i--] = a[i--] + 1`: the OUTER op is increment
        // (`+ 1`). Detecting `--` anywhere in the node text misread the nested `i--`
        // and flipped the outer op to decrement; the operator token, not the text,
        // decides it.
        let ops = binops("int f(){ int a[10]; int i=0; a[i--]++; return 0; }");
        assert!(
            ops.contains(&Op::Add),
            "outer `++` must lower to Op::Add despite the nested `i--`, got {ops:?}"
        );
    }
}
