use super::*;

pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let callee = lower_callee(lo, node).unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![callee];
    for suffix in Lowering::named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "call_suffix" | "constructor_suffix"))
    {
        for child in Lowering::named_children(suffix) {
            match child.kind() {
                "value_arguments" => {
                    let mut args = Vec::new();
                    for arg in Lowering::named_children(child) {
                        if arg.kind() == "value_argument" {
                            args.push(lower_value_argument(lo, arg));
                        }
                    }
                    if lo.text(child).trim_start().starts_with('[') {
                        let index = match args.as_slice() {
                            [] => lo.empty_block(lo.span(child)),
                            [only] => *only,
                            [key, default] if kwarg_name(lo, *default) == Some("default") => {
                                let default_value =
                                    lo.b.children(*default).first().copied().unwrap_or(*default);
                                lo.add(
                                    NodeKind::Seq,
                                    Payload::Name(lo.sym("swift_subscript_default")),
                                    lo.span(child),
                                    &[*key, default_value],
                                )
                            }
                            _ => lo.add(
                                NodeKind::Seq,
                                Payload::Name(lo.sym("tuple")),
                                lo.span(child),
                                &args,
                            ),
                        };
                        return lo.add(NodeKind::Index, Payload::None, span, &[kids[0], index]);
                    }
                    kids.extend(args);
                }
                "lambda_literal" => kids.push(lower_lambda(lo, child)),
                _ => {}
            }
        }
    }
    if lo.text(node).trim_start().starts_with('!')
        && kids.len() == 2
        && is_swift_force_marker(lo, kids[0])
    {
        return lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[kids[1]]);
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn is_swift_force_marker(lo: &Lowering, node: NodeId) -> bool {
    if lo.b.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(name) = lo.b.payload(node) else {
        return false;
    };
    lo.interner.resolve(name) == "swift_force_marker"
}
pub(super) fn lower_macro_invocation(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let call = lower_call(lo, node);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_macro_invocation")),
        span,
        &[call],
    )
}
pub(super) fn lower_diagnostic(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = vec![lo.str_lit(lo.text(node), span)];
    kids.extend(
        Lowering::named_children(node)
            .into_iter()
            .filter(|child| is_expr_kind(child.kind()))
            .map(|child| lower_expr(lo, child)),
    );
    let tag = if lo.text(node).trim_start().starts_with("#error") {
        "swift_diagnostic_error"
    } else if lo.text(node).trim_start().starts_with("#warning") {
        "swift_diagnostic_warning"
    } else {
        "swift_diagnostic"
    };
    lo.add(NodeKind::Seq, Payload::Name(lo.sym(tag)), span, &kids)
}
pub(super) fn kwarg_name<'a>(lo: &'a Lowering, node: NodeId) -> Option<&'a str> {
    if lo.b.kind(node) != NodeKind::KwArg {
        return None;
    }
    let Payload::Name(name) = lo.b.payload(node) else {
        return None;
    };
    Some(lo.interner.resolve(name))
}
pub(super) fn lower_callee(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    if node.kind() == "constructor_expression" {
        let ty = node.child_by_field_name("constructed_type")?;
        let name = type_surface_name(lo, ty).unwrap_or_else(|| lo.text(ty).to_string());
        return Some(lo.var(&name, lo.span(ty)));
    }
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() != "call_suffix")
        .map(|child| lower_expr(lo, child))
}
pub(super) fn lower_value_argument(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let value = node
        .child_by_field_name("value")
        .or_else(|| first_expr_child(node))
        .map(|value| lower_expr(lo, value))
        .unwrap_or_else(|| lo.empty_block(span));
    if let Some(name) = node.child_by_field_name("name") {
        lo.add(
            NodeKind::KwArg,
            Payload::Name(lo.sym(lo.text(name).trim_end_matches(':'))),
            span,
            &[value],
        )
    } else {
        value
    }
}
pub(super) fn lower_navigation(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if lo.text(node).trim_start().starts_with('/') {
        return lower_case_path(lo, node);
    }
    let Some(target) = node.child_by_field_name("target") else {
        return lo.raw(node.kind(), span, &[]);
    };
    let Some(suffix) = node.child_by_field_name("suffix") else {
        return lower_expr(lo, target);
    };
    if let Some(inner_target) = logical_not_prefix_target(lo, target) {
        let base = lower_expr(lo, inner_target);
        let value = lower_navigation_suffix(lo, span, base, suffix);
        return lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[value]);
    }
    let base = lower_expr(lo, target);
    lower_navigation_suffix(lo, span, base, suffix)
}

fn logical_not_prefix_target<'tree>(
    lo: &Lowering<'_>,
    node: TsNode<'tree>,
) -> Option<TsNode<'tree>> {
    if node.kind() != "prefix_expression" {
        return None;
    }
    let target = node
        .child_by_field_name("target")
        .or_else(|| first_expr_child(node))?;
    (swift_prefix_operator_text(lo, node, Some(target)) == "!").then_some(target)
}

fn lower_navigation_suffix(
    lo: &mut Lowering,
    span: Span,
    mut base: NodeId,
    suffix: TsNode,
) -> NodeId {
    let suffix_value = suffix
        .child_by_field_name("suffix")
        .or_else(|| Lowering::named_children(suffix).into_iter().next());
    if let Some(value) = suffix_value {
        match value.kind() {
            "simple_identifier" | "identifier" => {
                return lo.add(
                    NodeKind::Field,
                    Payload::Name(lo.sym(lo.text(value))),
                    span,
                    &[base],
                );
            }
            "integer_literal" => {
                let index = lower_expr(lo, value);
                base = lo.add(NodeKind::Index, Payload::None, span, &[base, index]);
            }
            _ if is_expr_kind(value.kind()) => {
                let index = lower_expr(lo, value);
                base = lo.add(NodeKind::Index, Payload::None, span, &[base, index]);
            }
            _ => {}
        }
    }
    base
}
fn lower_case_path(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let source = lo.str_lit(lo.text(node), span);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_case_path")),
        span,
        &[source],
    )
}
pub(super) fn lower_selector_expression(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let source = lo.str_lit(lo.text(node), span);
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_selector_expression")),
        span,
        &[source],
    )
}
