use super::*;
use crate::tree_sitter_ext::child_at;

pub(super) fn lower_block_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_block_lambda_with_unit(lo, node, None)
}
pub(super) fn lower_block_lambda_with_unit(
    lo: &mut Lowering,
    node: TsNode,
    block_unit_name: Option<Symbol>,
) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_block_params(lo, params, &mut kids);
    }
    let body = block_body(lo, node);
    if let Some(name) = block_unit_name {
        lo.push_unit(body, UnitKind::Block, Some(name));
    }
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}

fn lower_block_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    let mut has_block_locals = false;
    for index in 0..params.child_count() {
        let Some(param) = child_at(params, index) else {
            continue;
        };
        if !param.is_named() {
            continue;
        }
        if params.field_name_for_child(index as u32) == Some("locals") {
            has_block_locals = true;
        } else {
            out.push(lower_param_node(lo, param));
        }
    }
    if block_params_have_trailing_comma(lo.text(params)) {
        out.push(lo.raw("ruby_trailing_comma_parameter", lo.span(params), &[]));
    }
    if has_block_locals {
        out.push(lo.raw("ruby_block_local_parameters", lo.span(params), &[]));
    }
}

fn block_params_have_trailing_comma(source: &str) -> bool {
    let formals = source.split(';').next().unwrap_or(source).trim_end();
    formals
        .strip_suffix('|')
        .unwrap_or(formals)
        .trim_end()
        .ends_with(',')
}
pub(super) fn is_test_dsl_method(method: &str) -> bool {
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
pub(super) fn test_dsl_block_unit_name(lo: &Lowering, call: TsNode, method: &str) -> Symbol {
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
pub(super) fn test_dsl_literal_label(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "string" | "bare_string" | "symbol" | "simple_symbol" | "hash_key_symbol" => {
            Some(trim_ruby_label_literal(lo.text(node)).to_string())
        }
        _ => None,
    }
}
pub(super) fn trim_ruby_label_literal(text: &str) -> &str {
    let trimmed = text.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    if quoted && trimmed.len() >= 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed.trim_start_matches(':').trim_end_matches(':')
    }
}
pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let method_name = node
        .child_by_field_name("method")
        .map(|m| lo.text(m).to_string());
    if is_unqualified_raise_call(node, method_name.as_deref()) {
        return lower_raise_call(lo, node, span);
    }
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

pub(super) fn is_unqualified_raise_call(node: TsNode, method_name: Option<&str>) -> bool {
    node.child_by_field_name("receiver").is_none() && method_name == Some("raise")
}

pub(super) fn lower_raise_call(lo: &mut Lowering, node: TsNode, span: Span) -> NodeId {
    let mut args = Vec::new();
    if let Some(arguments) = node.child_by_field_name("arguments") {
        for arg in Lowering::named_children(arguments) {
            args.push(lower_expr(lo, arg));
        }
    }
    let kids = match args.len() {
        0 | 1 => args,
        _ => vec![lo.add(NodeKind::Seq, Payload::None, span, &args)],
    };
    lo.add(NodeKind::Throw, Payload::None, span, &kids)
}
