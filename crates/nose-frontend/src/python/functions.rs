use super::*;

pub(super) fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    // Collect this function's `global` declarations BEFORE lowering its body, so an
    // assignment to a global-declared name can be recorded as a module rebind (#302).
    // The frame is scoped to this function — nested functions get their own.
    let is_async = crate::lower::node_has_child_kind(node, "async");
    let span = lo.span(node);
    let globals = collect_global_declarations(lo, node);
    lo.global_decls.push(globals);
    let func = crate::lower::function_unit(lo, node, method, lower_params, |lo, b| {
        let body = lower_docstring_block(lo, b, false);
        if is_async {
            lo.protocol_boundary(
                span,
                nose_il::SourceProtocolKind::AsyncFunction,
                "async_function",
                &[body],
            )
        } else {
            body
        }
    });
    record_global_rebinds(lo, func);
    lo.global_decls.pop();
    func
}
/// Mark every assignment in this function whose target rebinds a `global`-declared name
/// with a `ModuleRebind` fact, regardless of which lowering path built it — a plain
/// `helper = x`, a tuple unpack `helper, y = ...` (Seq target), an aug-assign
/// `helper += 1`, or a walrus `(helper := x)` all lower to an `Assign` with a `Var` or
/// `Seq`-of-`Var` target. Operating on the LOWERED IL (post-body) catches all forms
/// uniformly, where the original per-`lower_assignment` hook missed every form but the
/// first (coevo series 7, S2). Nested function/lambda scopes are skipped — they carry
/// their own `global` frame.
pub(super) fn record_global_rebinds(lo: &mut Lowering, func: NodeId) {
    let Some(frame) = lo.global_decls.last() else {
        return;
    };
    if frame.is_empty() {
        return;
    }
    let frame = frame.clone();
    let mut rebind_spans: Vec<Span> = Vec::new();
    let mut stack: Vec<NodeId> = lo.b.children(func).to_vec();
    while let Some(n) = stack.pop() {
        match lo.b.kind(n) {
            NodeKind::Func | NodeKind::Lambda => continue,
            NodeKind::Assign => {
                if let Some(&lhs) = lo.b.children(n).first() {
                    if assign_target_rebinds_global(lo, lhs, &frame) {
                        rebind_spans.push(lo.b.node(n).span);
                    }
                }
            }
            _ => {}
        }
        stack.extend(lo.b.children(n).iter().copied());
    }
    for span in rebind_spans {
        lo.record_source_fact(
            span,
            SourceFactKind::Binding(SourceBindingKind::ModuleRebind),
        );
    }
}
/// Whether a lowered assignment target (a `Var`, or a `Seq` of targets from a tuple/list
/// unpack) names a `global`-declared symbol. Attribute/index targets (`obj.x = `,
/// `d[k] = `) are NOT name rebinds and are ignored.
pub(super) fn assign_target_rebinds_global(
    lo: &Lowering,
    target: NodeId,
    frame: &rustc_hash::FxHashSet<Symbol>,
) -> bool {
    match lo.b.kind(target) {
        NodeKind::Var => {
            matches!(lo.b.node(target).payload, Payload::Name(s) if frame.contains(&s))
        }
        NodeKind::Seq => {
            lo.b.children(target)
                .to_vec()
                .iter()
                .any(|&c| assign_target_rebinds_global(lo, c, frame))
        }
        _ => false,
    }
}
/// The names declared `global` anywhere in a function body, excluding nested function /
/// lambda scopes (Python `global` is function-scoped). `nonlocal` rebinds an enclosing
/// *local*, never a module function, so it is not collected.
pub(super) fn collect_global_declarations(
    lo: &Lowering,
    func: TsNode,
) -> rustc_hash::FxHashSet<Symbol> {
    let mut out = rustc_hash::FxHashSet::default();
    let Some(body) = func.child_by_field_name("body") else {
        return out;
    };
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        match n.kind() {
            // Do not descend into nested function/lambda scopes — they have their own
            // `global` frame.
            "function_definition" | "lambda" => continue,
            "global_statement" => {
                for child in semantic_named_children(n) {
                    if child.kind() == "identifier" {
                        out.insert(lo.sym(lo.text(child)));
                    }
                }
            }
            _ => {}
        }
        for child in semantic_named_children(n) {
            stack.push(child);
        }
    }
    out
}
pub(super) fn lower_class(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body_block = match node.child_by_field_name("body") {
        Some(b) => lower_docstring_block(lo, b, true),
        None => lo.empty_block(span),
    };
    lo.push_unit(body_block, UnitKind::Class, name);
    body_block
}
pub(super) fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    for p in semantic_named_children(params) {
        let span = lo.span(p);
        let name = param_name(lo, p);
        let payload = match name {
            Some(s) => Payload::Name(lo.sym(s)),
            None => Payload::None,
        };
        if let Some(domain) = lo.type_domain_from_text_with_dependencies(lo.text(p)) {
            lo.record_param_domain_resolution(span, domain);
        }
        if !plain_parameter(p) {
            lo.record_parameter_shape(span, nose_il::ParameterShapeEvidenceKind::NonPlain);
        }
        out.push(lo.add(NodeKind::Param, payload, span, &[]));
    }
}

fn plain_parameter(param: TsNode) -> bool {
    param.kind() == "identifier"
        || param.kind() == "typed_parameter"
            && param
                .child_by_field_name("name")
                .or_else(|| param.named_child(0))
                .is_some_and(|name| name.kind() == "identifier")
}
/// Dig the identifier name out of the various Python parameter node shapes.
pub(super) fn param_name<'a>(lo: &Lowering<'a>, p: TsNode<'a>) -> Option<&'a str> {
    match p.kind() {
        "identifier" => Some(lo.text(p)),
        "typed_parameter" | "default_parameter" | "typed_default_parameter" => p
            .child_by_field_name("name")
            .or_else(|| p.named_child(0))
            .map(|n| lo.text(n)),
        "list_splat_pattern" | "dictionary_splat_pattern" => p.named_child(0).map(|n| lo.text(n)),
        _ => p.named_child(0).map(|n| lo.text(n)),
    }
}
