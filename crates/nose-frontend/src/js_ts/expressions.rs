use super::control::{lower_aug_assignment, lower_update};
use super::declarations::{lower_arrow, lower_class, lower_field, lower_func};
use super::globals::{
    lower_callee_expr, lower_constructor_expr, lower_js_static_global_or_var, lower_member_expr,
    lower_own_property_guard_call,
};
use super::jsx::lower_jsx;
use super::operators::{js_bin_op, js_source_operator};
use super::record_guard::lower_record_shape_guard;
use super::syntax::static_string_key;
use super::types::is_ts_type;
use crate::lower::Lowering;
use nose_il::{
    LitClass, NodeId, NodeKind, Op, Payload, SourceCallKind, SourceFactKind, SourceLiteralKind,
    SourceOperatorKind, Span,
};
use tree_sitter::Node as TsNode;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
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
        "assignment_expression" => crate::lower::assignment(lo, node, lower_expr, lower_expr),
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
        _ => lower_expr_rest(lo, node),
    }
}

/// Tail of [`lower_expr`]'s dispatch: destructuring patterns, parameters,
/// JSX, template internals, and TS-only kinds that reach expression position.
fn lower_expr_rest(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
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
        "jsx_opening_element" | "jsx_closing_element" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym(node.kind())),
                span,
                &kids,
            )
        }
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
        _ => lower_expr_tail(lo, node, span),
    }
}

fn lower_expr_tail(lo: &mut Lowering, node: TsNode, span: Span) -> NodeId {
    match node.kind() {
        "class" | "class_declaration" | "abstract_class_declaration" => lower_class(lo, node),
        "class_body" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .filter_map(|child| {
                    if matches!(child.kind(), "field_definition" | "public_field_definition") {
                        lower_field(lo, child)
                    } else {
                        Some(lower_expr(lo, child))
                    }
                })
                .collect();
            lo.add(NodeKind::Block, Payload::None, span, &kids)
        }
        "comment" => lo.empty_block(span),
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
        // exact mode does not prove yet. Keep operand structure for near/diagnostics, but
        // use a non-admitted Seq tag so strict exact stays closed.
        _ => {
            let inner: Vec<NodeId> = arg_node.into_iter().map(|a| lower_expr(lo, a)).collect();
            lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym(&format!("js_{op_text}"))),
                span,
                &inner,
            )
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
            // Spread depends on object/runtime semantics that strict exact does not prove
            // yet. Keep the source shape for near mode while using a non-admitted Seq tag
            // so the containing object remains exact-closed without cascading Raw wrappers.
            "spread_element" => {
                let inner: Vec<NodeId> = Lowering::named_children(child)
                    .into_iter()
                    .map(|c| lower_expr(lo, c))
                    .collect();
                kids.push(lo.add(
                    NodeKind::Seq,
                    Payload::Name(lo.sym("js_object_spread")),
                    lo.span(child),
                    &inner,
                ));
            }
            "method_definition" => {
                kids.push(lower_func(lo, child, true));
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
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("js_computed_property_name")),
        span,
        &inner,
    )
}

fn static_object_property_key(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "property_identifier" | "identifier" => static_identifier_property_key(lo, node),
        "string" => static_string_key(lo, node),
        _ => None,
    }
}

fn static_identifier_property_key(lo: &Lowering, node: TsNode) -> Option<String> {
    let text = lo.text(node);
    (!text.contains('\\')).then(|| text.to_string())
}

fn lower_object_shorthand(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = lo.text(node).to_string();
    let key = if name.contains('\\') {
        let tag = lo.sym("js_escaped_property_identifier");
        lo.add(NodeKind::Seq, Payload::Name(tag), span, &[])
    } else {
        lo.str_lit(&name, span)
    };
    let value = lo.var(&name, span);
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
