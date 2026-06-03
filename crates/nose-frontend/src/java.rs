//! Java → raw IL lowering.
//!
//! Convergence-friendly lowering: `x op= y` / `x++` desugar to assignments; the
//! `for`, enhanced-for, `while`, and `do` forms map to the unified `Loop`;
//! `switch` becomes an `if`/`else if` chain; `class`/`interface`/`enum` become
//! class-like units and `method`/`constructor` become function units. Type
//! annotations and generics are not modeled (Java is statically typed).

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
        "import_declaration" | "package_declaration" => Some(crate::lower::import_tokens(lo, node)),
        "line_comment" | "block_comment" => None,
        _ => lower_stmt(lo, node),
    }
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
        "class_declaration" | "interface_declaration" | "enum_declaration" | "method_declaration"
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
        match lower_stmt(lo, node) {
            Some(s) => lo.add(NodeKind::Block, Payload::None, span, &[s]),
            None => lo.empty_block(span),
        }
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
            let op = if lo.text(node).starts_with('!') {
                Op::Not
            } else {
                Op::Neg
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
            let op = if lo.text(node).contains("--") {
                Op::Sub
            } else {
                Op::Add
            };
            let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[operand2, one]);
            lo.add(NodeKind::Assign, Payload::None, span, &[operand, bin])
        }
        "method_invocation" => lower_call(lo, node),
        "object_creation_expression" => {
            let mut kids = Vec::new();
            if let Some(args) = node.child_by_field_name("arguments") {
                for a in Lowering::named_children(args) {
                    kids.push(lower_expr(lo, a));
                }
            }
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
            }
            let body = node
                .child_by_field_name("body")
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

fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, common_bin_op, lower_expr)
}

/// `recv.method(args)` → `Call(Field(method, recv), args...)`.
fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
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
