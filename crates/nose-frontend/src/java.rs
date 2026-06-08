//! Java → raw IL lowering.
//!
//! Convergence-friendly lowering: `x op= y` / `x++` desugar to assignments; the
//! `for`, enhanced-for, `while`, and `do` forms map to the unified `Loop`;
//! `switch` becomes an `if`/`else if` chain; `class`/`interface`/`enum` become
//! class-like units and `method`/`constructor` become function units. Type
//! annotations and generics are not modeled (Java is statically typed).

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceKind, FileId, Il, ImportEvidenceKind, Interner,
    Lang, LitClass, LoopKind, NodeId, NodeKind, Op, ParamSemantic, Payload, SourceCallKind,
    SourceFactKind, Span, UnitKind,
};
use nose_semantics::{
    library_java_collection_constructor_contract, LibraryApiCalleeContract,
    LibraryCollectionFactoryResult,
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
        crate::lower::grammar::JAVA,
        || tree_sitter_java::LANGUAGE.into(),
        Lang::Java,
        lower_items,
    )
}

fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}

fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => Some(lower_type(lo, node)),
        "method_declaration" | "constructor_declaration" => Some(lower_method(lo, node)),
        "field_declaration" => Some(lower_field(lo, node)),
        "import_declaration" => {
            Some(lower_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)))
        }
        "package_declaration" => Some(crate::lower::import_tokens(lo, node)),
        "line_comment" | "block_comment" => None,
        _ => lower_stmt(lo, node),
    }
}

fn lower_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim().trim_end_matches(';').trim();
    if let Some(path) = text.strip_prefix("import static ") {
        let path = path.trim();
        if path.ends_with(".*") {
            return None;
        }
        let (module, exported) = path.rsplit_once('.')?;
        return Some(crate::lower::import_binding(
            lo,
            span,
            exported.trim(),
            module.trim(),
            exported.trim(),
        ));
    }
    let path = text.strip_prefix("import ")?.trim();
    if path.ends_with(".*") {
        let module = path.trim_end_matches(".*").trim();
        lo.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Import(ImportEvidenceKind::Wildcard {
                module_hash: stable_symbol_hash(module),
            }),
            "java_wildcard_import",
        );
        return None;
    }
    let (module, exported) = path.rsplit_once('.')?;
    Some(crate::lower::import_binding(
        lo,
        span,
        exported.trim(),
        module.trim(),
        exported.trim(),
    ))
}

/// `class`/`interface`/`enum` → a `Class` unit; its methods become units too.
fn lower_type(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for c in Lowering::named_children(body) {
            if let Some(id) = lower_item(lo, c) {
                kids.push(id);
            }
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit(block, UnitKind::Class, name);
    block
}

fn lower_method(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        for p in Lowering::named_children(params) {
            let pspan = lo.span(p);
            let sym = p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
            if let Some(semantic) = java_param_semantic_from_text(lo.text(p)) {
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
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(func, UnitKind::Method, name);
    func
}

fn java_param_semantic_from_text(text: &str) -> Option<ParamSemantic> {
    if text.contains("[]") || text.contains("...") {
        Some(ParamSemantic::Array)
    } else {
        crate::lower::param_semantic_from_text(text)
    }
}

fn lower_field(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // a field_declaration has one or more variable_declarators
    let mut assigns = Vec::new();
    for d in Lowering::named_children(node) {
        if d.kind() != "variable_declarator" {
            continue;
        }
        let dspan = lo.span(d);
        let lhs = d
            .child_by_field_name("name")
            .map(|n| lo.var(lo.text(n), dspan))
            .unwrap_or_else(|| lo.empty_block(dspan));
        let rhs = d
            .child_by_field_name("value")
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), dspan, &[]));
        assigns.push(lo.add(NodeKind::Assign, Payload::None, dspan, &[lhs, rhs]));
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
        "block" => Some(lower_block(lo, node)),
        "local_variable_declaration" => Some(lower_field(lo, node)),
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
        "enhanced_for_statement" => Some(lower_for_each(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "do_statement" => Some(lower_while(lo, node)),
        "switch_expression" | "switch_statement" => Some(lower_switch(lo, node)),
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
        "try_statement" | "try_with_resources_statement" => Some(lower_try(lo, node)),
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        ";" | "line_comment" | "block_comment" => None,
        k if is_type_decl(k) => lower_item(lo, node),
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

fn is_type_decl(k: &str) -> bool {
    matches!(
        k,
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "method_declaration"
            | "constructor_declaration"
    )
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
        kids.push(stmt_as_block(lo, alt));
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

fn stmt_as_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "block" {
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
        .child_by_field_name("init")
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

fn lower_for_each(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pat = node
        .child_by_field_name("name")
        .map(|n| lo.var(lo.text(n), span))
        .unwrap_or_else(|| lo.empty_block(span));
    let iter = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| stmt_as_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pat, iter, body],
    )
}

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::while_loop(lo, node, lower_expr, stmt_as_block)
}

/// `switch` → nested `if`/`else` chain over the switch value's groups.
fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::switch_to_if_chain(
        lo,
        node,
        |k| k.starts_with("switch_block_statement_group") || k == "switch_rule",
        lower_expr,
        lower_stmt,
    )
}

fn lower_switch_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = switch_expr_value(node)
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let rules: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|body| {
            Lowering::named_children(body)
                .into_iter()
                .filter(|child| child.kind() == "switch_rule")
                .collect()
        })
        .unwrap_or_default();
    let mut branches = Vec::new();
    let mut default_body = None;

    for rule in rules {
        let mut labels = Vec::new();
        let mut body = None;
        let mut saw_label = false;
        for child in Lowering::named_children(rule) {
            if child.kind() == "switch_label" {
                saw_label = true;
                labels.extend(
                    Lowering::named_children(child)
                        .into_iter()
                        .map(|label| lower_expr(lo, label)),
                );
                continue;
            }
            if saw_label {
                body = Some(lower_switch_rule_expr_body(lo, child));
                break;
            }
        }

        let body = body.unwrap_or_else(|| lo.empty_block(span));
        match fold_switch_expr_labels(lo, span, scrutinee, labels) {
            Some(cond) => branches.push((cond, body)),
            None => default_body = Some(body),
        }
    }

    let mut acc = default_body.unwrap_or_else(|| lo.empty_block(span));
    for (cond, body) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
    }
    acc
}

fn switch_expr_value(node: TsNode) -> Option<TsNode> {
    node.child_by_field_name("value").or_else(|| {
        Lowering::named_children(node)
            .into_iter()
            .find(|child| child.kind() != "switch_block")
    })
}

fn lower_switch_rule_expr_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node.kind() == "block" {
        lower_switch_yield_expr(lo, node).unwrap_or_else(|| lower_block(lo, node))
    } else {
        lower_expr(lo, node)
    }
}

fn lower_switch_yield_expr(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "yield_statement")
        .and_then(|child| {
            child
                .child_by_field_name("value")
                .or_else(|| child.named_child(0))
        })
        .map(|expr| lower_expr(lo, expr))
}

fn fold_switch_expr_labels(
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

fn lower_try(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(b) = node.child_by_field_name("body") {
        kids.push(lower_block(lo, b));
    }
    for c in Lowering::named_children(node) {
        if c.kind() == "catch_clause" || c.kind() == "finally_clause" {
            if let Some(b) = c.child_by_field_name("body").or_else(|| {
                c.named_children(&mut c.walk())
                    .find(|n| n.kind() == "block")
            }) {
                kids.push(lower_block(lo, b));
            }
        }
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "type_identifier" | "scoped_identifier" => lo.var(lo.text(node), span),
        "this" => lo.var("this", span),
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => {
            let t = lo.text(node);
            lo.int_lit(t.trim_end_matches(['L', 'l']), span)
        }
        "decimal_floating_point_literal" | "hex_floating_point_literal" => {
            lo.float_lit(lo.text(node), span)
        }
        "string_literal" | "character_literal" | "text_block" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "null_literal" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => {
            let operand = node
                .child_by_field_name("operand")
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            // Map by the operator token, not the leading byte: `+`→Pos, `-`→Neg,
            // `~`→BitNot, `!`→Not. Reading only the first byte collapsed `+x` and `~x`
            // onto `Neg` (same class of bug as the C/Ruby frontends).
            // Map by the operator token, not the leading byte: `+`→Pos, `-`→Neg,
            // `~`→BitNot, `!`→Not. Reading only the first byte collapsed `+x` and `~x`
            // onto `Neg` (same class of bug as the C/Ruby frontends).
            let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
                Some("+") => Op::Pos,
                Some("~") => Op::BitNot,
                Some("!") => Op::Not,
                _ => Op::Neg,
            };
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
        }
        "assignment_expression" => {
            let l = node
                .child_by_field_name("left")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            // compound `x += y` → `x = x op y`
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
            // x++ / ++x → x = x + 1
            let operand = node
                .named_child(0)
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let operand2 = node
                .named_child(0)
                .map(|o| lower_expr(lo, o))
                .unwrap_or_else(|| lo.empty_block(span));
            let one = lo.int_lit("1", span);
            // Decide by the operator TOKEN among this node's direct children: a substring
            // check on the whole text misreads a nested `--`/`++` in the operand (e.g.
            // `a[i--]++`, whose outer op is `++`).
            let op = if crate::lower::has_direct_token(node, "--") {
                Op::Sub
            } else {
                Op::Add
            };
            let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[operand2, one]);
            lo.add(NodeKind::Assign, Payload::None, span, &[operand, bin])
        }
        "method_invocation" => lower_call(lo, node),
        "switch_expression" => lower_switch_expr(lo, node),
        "object_creation_expression" => {
            let mut kids = Vec::new();
            if let Some(args) = node.child_by_field_name("arguments") {
                for a in Lowering::named_children(args) {
                    kids.push(lower_expr(lo, a));
                }
            }
            if let Some(list) = lower_empty_java_collection_constructor(lo, node, &kids, span) {
                return list;
            }
            lo.record_source_fact(span, SourceFactKind::Call(SourceCallKind::Construct));
            lo.add(NodeKind::Call, Payload::None, span, &kids)
        }
        "field_access" => {
            let base = node
                .child_by_field_name("object")
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
        "array_access" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "lambda_expression" => {
            let mut kids = Vec::new();
            let body_node = node.child_by_field_name("body");
            if let Some(params) = node.child_by_field_name("parameters") {
                for p in Lowering::named_children(params) {
                    let psym = if p.kind() == "identifier" {
                        Some(lo.sym(lo.text(p)))
                    } else {
                        p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)))
                    };
                    let pspan = lo.span(p);
                    kids.push(lo.add(
                        NodeKind::Param,
                        psym.map(Payload::Name).unwrap_or(Payload::None),
                        pspan,
                        &[],
                    ));
                }
            } else if let Some(p) = node.child_by_field_name("parameter") {
                let psym = if p.kind() == "identifier" {
                    Some(lo.sym(lo.text(p)))
                } else {
                    p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)))
                };
                kids.push(lo.add(
                    NodeKind::Param,
                    psym.map(Payload::Name).unwrap_or(Payload::None),
                    lo.span(p),
                    &[],
                ));
            } else if let Some(p) = node.named_child(0) {
                let body_start = body_node.map(|b| b.start_byte());
                if p.kind() == "identifier" && Some(p.start_byte()) != body_start {
                    kids.push(lo.add(
                        NodeKind::Param,
                        Payload::Name(lo.sym(lo.text(p))),
                        lo.span(p),
                        &[],
                    ));
                }
            }
            if kids.is_empty() {
                if let Some(name) = lambda_single_param_from_text(lo.text(node)) {
                    kids.push(lo.add(NodeKind::Param, Payload::Name(lo.sym(name)), span, &[]));
                }
            }
            let body = body_node
                .map(|b| {
                    if b.kind() == "block" {
                        lower_block(lo, b)
                    } else {
                        lower_expr(lo, b)
                    }
                })
                .unwrap_or_else(|| lo.empty_block(span));
            kids.push(body);
            lo.add(NodeKind::Lambda, Payload::None, span, &kids)
        }
        "parenthesized_expression" | "cast_expression" => node
            .named_child(node.named_child_count().saturating_sub(1))
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "ternary_expression" => {
            let kids: Vec<NodeId> = ["condition", "consequence", "alternative"]
                .iter()
                .filter_map(|f| node.child_by_field_name(f))
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::If, Payload::None, span, &kids)
        }
        "array_initializer" | "array_creation_expression" | "argument_list" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `Foo.class` → a field access named `class` over the (erased) type.
        "class_literal" => {
            let base = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            let f = lo.sym("class");
            lo.add(NodeKind::Field, Payload::Name(f), span, &[base])
        }
        "super" => lo.var("super", span),
        // `x instanceof T` → the runtime value being tested (type erased).
        "instanceof_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `this(...)` / `super(...)` constructor delegation → a Call over its args.
        "explicit_constructor_invocation" => {
            let mut kids = Vec::new();
            if let Some(args) = node.child_by_field_name("arguments") {
                for a in Lowering::named_children(args) {
                    kids.push(lower_expr(lo, a));
                }
            }
            lo.add(NodeKind::Call, Payload::None, span, &kids)
        }
        "method_reference" => lo.var(lo.text(node), span),
        // `case V:` label in an expression-position group → the matched value.
        "switch_label" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `new int[n]` size carries behavior — keep the inner expression.
        "dimensions_expr" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Enum constant `NAME(args)` → a Var (the constant), args carry no flow.
        "enum_constant" => node
            .child_by_field_name("name")
            .map(|n| lo.var(lo.text(n), span))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Statement nodes reaching expression position (switch-rule bodies, etc.).
        "block" => lower_block(lo, node),
        "expression_statement" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type-level nodes carry no behavior — erase rather than Raw.
        "integral_type"
        | "floating_point_type"
        | "boolean_type"
        | "void_type"
        | "generic_type"
        | "array_type"
        | "dimensions"
        | "type_arguments"
        | "wildcard"
        | "type_parameters"
        | "type_parameter"
        | "type_bound"
        | "annotation"
        | "marker_annotation"
        | "annotation_argument_list"
        | "annotation_type_element_declaration"
        | "annotation_type_body"
        | "super_interfaces"
        | "extends_interfaces"
        | "type_list"
        | "throws"
        | "modifiers" => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

fn lower_empty_java_collection_constructor(
    lo: &mut Lowering,
    node: TsNode,
    args: &[NodeId],
    span: Span,
) -> Option<NodeId> {
    let ty = node.child_by_field_name("type")?;
    let type_span = lo.span(ty);
    let type_name = java_constructor_type_name(lo.text(ty));
    let contract = library_java_collection_constructor_contract(lo.lang, &type_name, args.len())?;
    let LibraryApiCalleeContract::JavaUtilConstructor {
        simple_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
        ..
    } = contract.callee
    else {
        return None;
    };
    let uses_simple_type = type_name == simple_type;
    if uses_simple_type {
        let root = java_root(node);
        if requires_import_for_simple_type
            && !java_tree_resolves_simple_type(lo, root, module, simple_type)
        {
            return None;
        }
        if requires_no_local_type_shadow && java_tree_declares_type(lo, root, simple_type) {
            return None;
        }
    }
    match contract.result {
        LibraryCollectionFactoryResult::EmptySequence => {
            let callee = lo.add(
                NodeKind::Var,
                Payload::Name(lo.sym(&type_name)),
                type_span,
                &[],
            );
            let mut kids = Vec::with_capacity(args.len() + 1);
            kids.push(callee);
            kids.extend_from_slice(args);
            lo.record_source_fact(span, SourceFactKind::Call(SourceCallKind::Construct));
            Some(lo.add(NodeKind::Call, Payload::None, span, &kids))
        }
        _ => None,
    }
}

fn java_constructor_type_name(text: &str) -> String {
    text.split('<')
        .next()
        .unwrap_or(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn java_root(mut node: TsNode) -> TsNode {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

fn java_tree_resolves_simple_type(
    lo: &Lowering,
    node: TsNode,
    module: &str,
    simple_type: &str,
) -> bool {
    let mut saw_wildcard = false;
    let mut saw_exact = false;
    let mut saw_conflict = false;
    java_tree_import_resolution(
        lo,
        node,
        module,
        simple_type,
        &mut saw_wildcard,
        &mut saw_exact,
        &mut saw_conflict,
    );
    (saw_exact || saw_wildcard) && !saw_conflict
}

fn java_tree_import_resolution(
    lo: &Lowering,
    node: TsNode,
    module: &str,
    simple_type: &str,
    saw_wildcard: &mut bool,
    saw_exact: &mut bool,
    saw_conflict: &mut bool,
) {
    if node.kind() == "import_declaration" {
        let compact: String = lo
            .text(node)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let exact = format!("import{module}.{simple_type};");
        let wildcard = format!("import{module}.*;");
        if compact == exact {
            *saw_exact = true;
        } else if compact == wildcard {
            *saw_wildcard = true;
        } else if compact.starts_with("import") && compact.ends_with(&format!(".{simple_type};")) {
            *saw_conflict = true;
        }
        return;
    }
    for child in Lowering::named_children(node) {
        java_tree_import_resolution(
            lo,
            child,
            module,
            simple_type,
            saw_wildcard,
            saw_exact,
            saw_conflict,
        );
    }
}

fn java_tree_declares_type(lo: &Lowering, node: TsNode, simple_type: &str) -> bool {
    if matches!(
        node.kind(),
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
    ) && node
        .child_by_field_name("name")
        .is_some_and(|name| lo.text(name) == simple_type)
    {
        return true;
    }
    Lowering::named_children(node)
        .into_iter()
        .any(|child| java_tree_declares_type(lo, child, simple_type))
}

fn lambda_single_param_from_text(text: &str) -> Option<&str> {
    let (head, _) = text.split_once("->")?;
    let head = head
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    if head.is_empty() || head.contains(',') {
        return None;
    }
    let name = head.split_whitespace().last()?;
    if is_java_identifier(name) {
        Some(name)
    } else {
        None
    }
}

fn is_java_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, common_bin_op, lower_expr)
}

/// `recv.method(args)` → `Call(Field(method, recv), args...)`.
fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name_node = node.child_by_field_name("name");
    let name = name_node.map(|n| lo.sym(lo.text(n)));
    let callee = match node.child_by_field_name("object") {
        Some(o) => {
            let recv = lower_expr(lo, o);
            lo.add(
                NodeKind::Field,
                name.map(Payload::Name).unwrap_or(Payload::None),
                span,
                &[recv],
            )
        }
        None => lo.add(
            NodeKind::Var,
            name.map(Payload::Name).unwrap_or(Payload::None),
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
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_kinds(src: &str) -> Vec<String> {
        let interner = Interner::new();
        lower(FileId(0), "T.java", src.as_bytes(), &interner)
            .expect("lower")
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Raw)
            .filter_map(|n| match n.payload {
                Payload::Name(s) => Some(interner.resolve(s).to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn local_record_and_annotation_declarations_do_not_fall_to_raw() {
        // Local type declarations are type metadata in this IL. They should follow the
        // same class-like lowering path as top-level declarations instead of surfacing
        // as opaque statement Raw nodes.
        let raw = raw_kinds(
            "class C { void f(){ record Pair(int a, int b) {} @interface Local { String value(); } } }",
        );
        assert!(
            raw.is_empty(),
            "local type declarations should be erased/lowered, got {raw:?}"
        );
    }

    fn unary_ops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        lower(FileId(0), "T.java", src.as_bytes(), &interner)
            .expect("lower")
            .nodes
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
        // `+x` must be Pos and `~x` BitNot, not both collapsed onto Neg.
        let ops = unary_ops(
            "class C { int f(int x){ return +x + -x + ~x; } boolean g(boolean b){ return !b; } }",
        );
        assert!(ops.contains(&Op::Pos), "unary + → Op::Pos, got {ops:?}");
        assert!(ops.contains(&Op::Neg), "unary - → Op::Neg, got {ops:?}");
        assert!(
            ops.contains(&Op::BitNot),
            "unary ~ → Op::BitNot, got {ops:?}"
        );
        assert!(ops.contains(&Op::Not), "unary ! → Op::Not, got {ops:?}");
    }

    fn binops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        lower(FileId(0), "T.java", src.as_bytes(), &interner)
            .expect("lower")
            .nodes
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
        let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
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

    fn raw_names(src: &str) -> Vec<String> {
        let interner = Interner::new();
        let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Raw)
            .filter_map(|node| match node.payload {
                Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
                _ => None,
            })
            .collect()
    }

    fn switch_expression_branch_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::If)
            .find_map(|(idx, _)| {
                let kids = il.children(NodeId(idx as u32));
                match kids {
                    [_, then_expr, else_expr] => {
                        match (il.node(*then_expr).payload, il.node(*else_expr).payload) {
                            (Payload::LitInt(then_value), Payload::LitInt(else_value)) => {
                                Some(vec![then_value, else_value])
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            })
            .unwrap_or_default()
    }

    fn switch_case_lhs_names(src: &str) -> Vec<String> {
        let interner = Interner::new();
        let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
        il.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::BinOp && node.payload == Payload::Op(Op::Eq))
            .filter_map(|(idx, _)| {
                let kids = il.children(NodeId(idx as u32));
                match kids {
                    [lhs, _] => match il.node(*lhs).payload {
                        Payload::Name(sym) => Some(interner.resolve(sym).to_string()),
                        _ => None,
                    },
                    _ => None,
                }
            })
            .collect()
    }

    fn expr_stmt_ints(src: &str) -> Vec<i64> {
        let interner = Interner::new();
        let il = lower(FileId(0), "T.java", src.as_bytes(), &interner).expect("lower");
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
        let src = "class C { int f(int x){ switch(x){ case 7: return 1; case 8: return 2; default: return 3; } } }";
        assert_eq!(switch_case_rhs_ints(src), vec![7, 8]);
        assert!(
            expr_stmt_ints(src).is_empty(),
            "case labels should not remain as stray expression statements"
        );
    }

    #[test]
    fn switch_expression_rules_lower_to_expression_if_chain() {
        let src = "class C { int f(int x){ return switch (x) { case 1 -> 2; default -> 3; }; } }";
        assert_eq!(switch_case_rhs_ints(src), vec![1]);
        assert_eq!(switch_case_lhs_names(src), vec!["x"]);
        assert_eq!(switch_expression_branch_ints(src), vec![2, 3]);
        let raw = raw_names(src);
        assert!(
            !raw.iter()
                .any(|name| matches!(name.as_str(), "switch_expression" | "switch_rule")),
            "switch expression rules should lower without Raw nodes: {raw:?}"
        );
    }

    #[test]
    fn switch_expression_yield_blocks_lower_to_branch_values() {
        let src = "class C { int f(int x){ return switch (x) { case 1 -> { yield 2; } default -> { yield 3; } }; } }";
        assert_eq!(switch_case_rhs_ints(src), vec![1]);
        assert_eq!(switch_expression_branch_ints(src), vec![2, 3]);
        let raw = raw_names(src);
        assert!(
            !raw.iter().any(|name| name == "yield_statement"),
            "switch expression yield blocks should lower without Raw yield_statement: {raw:?}"
        );
    }

    #[test]
    fn postfix_increment_with_nested_decrement_in_operand() {
        // `a[i--]++` desugars with the OUTER op being increment (`+ 1`); a substring
        // `--` check misread the nested `i--` and flipped it to decrement.
        let ops = binops("class C { void f(){ int[] a = new int[10]; int i = 0; a[i--]++; } }");
        assert!(
            ops.contains(&Op::Add),
            "outer `++` must lower to Op::Add despite the nested `i--`, got {ops:?}"
        );
    }
}
