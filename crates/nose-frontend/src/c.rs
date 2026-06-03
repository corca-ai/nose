//! C → raw IL lowering.
//!
//! Convergence-friendly lowering: `x op= y` / `x++` desugar to assignments; `for`,
//! `while`, `do` map to the unified `Loop`; `switch` becomes an `if`/`else if`
//! chain; `function_definition` becomes a function unit. struct/union/enum are
//! data definitions (not unit-ified). `*p`, `&x`, casts peel to the operand.

use crate::lower::{common_bin_op, Lowering};
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
        crate::lower::grammar::C,
        || tree_sitter_c::LANGUAGE.into(),
        Lang::C,
        lower_items,
    )
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
            "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif"
            | "preproc_elifdef" | "linkage_specification" => collect_top_items(lo, c, out),
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
        "preproc_def"
        | "preproc_function_def"
        | "preproc_ifdef"
        | "preproc_if"
        | "type_definition"
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
        match lower_stmt(lo, node) {
            Some(s) => lo.add(NodeKind::Block, Payload::None, span, &[s]),
            None => lo.empty_block(span),
        }
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
        "identifier" | "field_identifier" | "type_identifier" => lo.var(lo.text(node), span),
        "number_literal" => {
            let t = lo.text(node);
            if t.contains('.') || t.contains('e') || t.contains('E') {
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
            let op = if lo.text(node).starts_with('!') {
                Op::Not
            } else {
                Op::Neg
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
            let op = if lo.text(node).contains("--") {
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
