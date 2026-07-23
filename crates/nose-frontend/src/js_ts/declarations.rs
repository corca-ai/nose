use super::expressions::lower_expr;
use super::globals::{enclosing_function_prefix_has_binding_ident, file_prefix_has_binding_ident};
use super::syntax::static_string_key;
use super::{lower_block, lower_stmt};
use crate::lower::Lowering;
use crate::tree_sitter_ext::child_at;
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceKind, LitClass, NodeId, NodeKind, Payload,
    RegionKind, SourceBindingKind, SourceFactKind, SourceGranularity, SourceProtocolKind, Span,
    SymbolEvidenceKind, UnitBodyKind, UnitDomain, UnitDomains, UnitEvidenceFlag, UnitKind,
    UnitOrigin, UnitSubkind,
};
use tree_sitter::Node as TsNode;

pub(super) fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    let span = lo.span(node);
    let is_async = node_has_async_modifier(node);
    crate::lower::function_unit(lo, node, method, lower_params, |lo, body| {
        let lowered = lower_func_body(lo, body);
        if is_async {
            lo.protocol_boundary(
                span,
                SourceProtocolKind::AsyncFunction,
                "async_function",
                &[lowered],
            )
        } else {
            lowered
        }
    })
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

pub(super) fn lower_class(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node.child_by_field_name("body");
    let block = match body {
        Some(b) => super::lower_stmt_list(lo, b, NodeKind::Block, true),
        None => lo.empty_block(span),
    };
    let mut class_children = Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "class_heritage")
        .flat_map(Lowering::named_children)
        .filter_map(|clause_or_value| match clause_or_value.kind() {
            // tree-sitter-typescript wraps the expression in `extends_clause`.
            "extends_clause" => clause_or_value.child_by_field_name("value"),
            // `implements` is type-only and deliberately absent from runtime IL.
            "implements_clause" => None,
            // tree-sitter-javascript puts the heritage expression directly
            // under `class_heritage`.
            _ => Some(clause_or_value),
        })
        .map(|value| lower_expr(lo, value))
        .collect::<Vec<_>>();
    let class_body = if class_children.is_empty() {
        block
    } else {
        class_children.push(block);
        lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("js_class_definition")),
            span,
            &class_children,
        )
    };
    let unit_root = lower_own_decorated_definition(lo, node, class_body);
    lo.push_unit_with_origin(
        unit_root,
        UnitKind::Class,
        name,
        UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::Class,
            UnitBodyKind::Implementation,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::HasReusableBody),
    );
    unit_root
}

pub(super) fn lower_decorator(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|child| lower_expr(lo, child))
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("js_decorator")),
        span,
        &kids,
    )
}

pub(super) fn lower_class_static_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter_map(|child| lower_stmt(lo, child, false))
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("js_class_static_block")),
        span,
        &kids,
    )
}

pub(super) fn lower_decorated_definition(
    lo: &mut Lowering,
    decorators: Vec<NodeId>,
    definition_span: Span,
    decorated_span: Span,
    lowered: NodeId,
) -> NodeId {
    lo.record_source_fact(
        definition_span,
        SourceFactKind::Binding(SourceBindingKind::DecoratedDefinition),
    );
    if decorated_span != definition_span {
        lo.record_source_fact(
            decorated_span,
            SourceFactKind::Binding(SourceBindingKind::DecoratedDefinition),
        );
    }
    let mut kids = decorators;
    kids.push(lowered);
    let wrapper = lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("js_decorated_definition")),
        decorated_span,
        &kids,
    );
    retarget_last_unit_root(lo, lowered, wrapper);
    wrapper
}

pub(super) fn lower_own_decorated_definition(
    lo: &mut Lowering,
    node: TsNode,
    lowered: NodeId,
) -> NodeId {
    let decorators: Vec<TsNode> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "decorator")
        .collect();
    if decorators.is_empty() {
        return lowered;
    }
    let kids: Vec<NodeId> = decorators
        .into_iter()
        .map(|decorator| lower_decorator(lo, decorator))
        .collect();
    lower_decorated_definition(lo, kids, lo.span(node), lo.span(node), lowered)
}

fn retarget_last_unit_root(lo: &mut Lowering, from: NodeId, to: NodeId) {
    if let Some(unit) = lo.units.last_mut().filter(|unit| unit.root == from) {
        unit.root = to;
    }
}

pub(super) fn lower_field(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
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
    // A single arrow parameter can arrive as the parameter node itself rather than a
    // `formal_parameters` container. Preserve it as one coordinate even for patterns.
    if matches!(
        params.kind(),
        "identifier"
            | "undefined"
            | "required_parameter"
            | "optional_parameter"
            | "assignment_pattern"
            | "rest_pattern"
            | "object_pattern"
            | "array_pattern"
    ) {
        lower_param(lo, params, out);
        return;
    }
    for p in Lowering::named_children(params) {
        lower_param(lo, p, out);
    }
}

fn lower_param(lo: &mut Lowering, param: TsNode, out: &mut Vec<NodeId>) {
    let span = lo.span(param);
    let payload = param_name(lo, param)
        .map(|name| Payload::Name(lo.sym(name)))
        .unwrap_or(Payload::None);
    if let Some(domain) = lo.type_domain_from_text_with_dependencies(lo.text(param)) {
        lo.record_param_domain_resolution(span, domain);
    }
    let shape = non_plain_parameter_shape(lo, param);
    out.push(lo.add(NodeKind::Param, payload, span, &shape));
}

fn non_plain_parameter_shape(lo: &mut Lowering, param: TsNode) -> Vec<NodeId> {
    if plain_parameter_identifier(param) {
        return Vec::new();
    }

    let assignment = first_named_descendant_of_kind(param, &["assignment_pattern"]);
    let rest = first_named_descendant_of_kind(param, &["rest_pattern"]);
    let destructured = first_named_descendant_of_kind(param, &["object_pattern", "array_pattern"]);
    let has_default = assignment.is_some() || param.child_by_field_name("value").is_some();
    let surface = if has_default {
        "js_default_parameter"
    } else if rest.is_some() {
        "js_rest_parameter"
    } else if destructured.is_some() {
        "js_destructured_parameter"
    } else {
        "js_non_plain_parameter"
    };
    let default_value = assignment
        .and_then(|assignment| {
            assignment
                .child_by_field_name("right")
                .or_else(|| assignment.named_child(1))
        })
        .or_else(|| {
            (surface == "js_default_parameter")
                .then(|| param.child_by_field_name("value"))
                .flatten()
        })
        .map(|value| lower_expr(lo, value));
    let marker_children = default_value.into_iter().collect::<Vec<_>>();
    let marker = lo.raw(surface, lo.span(param), &marker_children);
    vec![marker]
}

fn plain_parameter_identifier(param: TsNode) -> bool {
    if matches!(param.kind(), "identifier" | "undefined") {
        return true;
    }
    param.kind() == "required_parameter"
        && param.child_by_field_name("value").is_none()
        && param
            .child_by_field_name("pattern")
            .or_else(|| param.named_child(0))
            .is_some_and(|pattern| matches!(pattern.kind(), "identifier" | "undefined"))
}

fn first_named_descendant_of_kind<'a>(root: TsNode<'a>, expected: &[&str]) -> Option<TsNode<'a>> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if expected.contains(&node.kind()) {
            return Some(node);
        }
        stack.extend(Lowering::named_children(node));
    }
    None
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

pub(super) fn lower_var_decl(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // One or more `variable_declarator`s → a Block of Assigns (or a single Assign).
    let mut assigns = Vec::new();
    for d in Lowering::named_children(node) {
        if d.kind() != "variable_declarator" {
            continue;
        }
        let dspan = lo.span(d);
        let name_node = d.child_by_field_name("name");
        let value_node = d.child_by_field_name("value");
        let rhs = match value_node {
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
        if let (Some(name), Some(value)) = (name_node, value_node) {
            record_commonjs_node_timers_promise_import_bindings(lo, node, name, value, dspan);
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
    let projections = object_pattern_projections(lo, pattern)?;
    let mut assigns = Vec::new();
    for (child, field, local) in projections {
        let lhs = lo.var(&local, lo.span(child));
        let key = lo.sym(&field);
        let rhs = lo.add(NodeKind::Field, Payload::Name(key), lo.span(child), &[base]);
        assigns.push(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]));
    }
    Some(assigns)
}

fn record_commonjs_node_timers_promise_import_bindings(
    lo: &mut Lowering,
    declaration: TsNode,
    pattern: TsNode,
    value: TsNode,
    binding_span: Span,
) {
    if !is_const_lexical_declaration(declaration) {
        return;
    }
    let Some(projections) = object_pattern_projections(lo, pattern) else {
        return;
    };
    let Some((module, require_callee)) = static_require_module(lo, value) else {
        return;
    };
    if !is_node_timers_promise_module(&module) || js_require_shadowed_at(lo, require_callee) {
        return;
    }

    let require_dependency = lo.record_evidence(
        EvidenceAnchor::node(lo.span(require_callee), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        }),
        "symbol_commonjs_require_unshadowed",
    );
    for (_, exported, local) in projections {
        if !is_node_timers_promise_export(&exported) {
            continue;
        }
        let _ = crate::lower::import_binding_evidence_only_with_dependencies(
            lo,
            binding_span,
            &local,
            &module,
            &exported,
            vec![require_dependency],
        );
    }
}

fn object_pattern_projections<'a>(
    lo: &Lowering<'a>,
    pattern: TsNode<'a>,
) -> Option<Vec<(TsNode<'a>, String, String)>> {
    if pattern.kind() != "object_pattern" {
        return None;
    }
    let projections: Option<Vec<_>> = Lowering::named_children(pattern)
        .into_iter()
        .map(|child| {
            object_pattern_projection(lo, child).map(|(field, local)| (child, field, local))
        })
        .collect();
    let projections = projections?;
    (!projections.is_empty()).then_some(projections)
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

fn is_const_lexical_declaration(node: TsNode) -> bool {
    node.kind() == "lexical_declaration"
        && (0..node.child_count())
            .any(|index| child_at(node, index).is_some_and(|child| child.kind() == "const"))
}

fn static_require_module<'a>(lo: &Lowering<'a>, value: TsNode<'a>) -> Option<(String, TsNode<'a>)> {
    if value.kind() != "call_expression" {
        return None;
    }
    let callee = value.child_by_field_name("function")?;
    if callee.kind() != "identifier" || lo.text(callee) != "require" {
        return None;
    }
    let args = value.child_by_field_name("arguments")?;
    let args = Lowering::named_children(args);
    let [arg] = args.as_slice() else {
        return None;
    };
    let module = static_string_key(lo, *arg)?;
    Some((module, callee))
}

fn js_require_shadowed_at(lo: &Lowering, require_callee: TsNode) -> bool {
    file_prefix_has_binding_ident(lo, require_callee, "require")
        || enclosing_function_prefix_has_binding_ident(lo, require_callee, "require")
}

fn is_node_timers_promise_module(module: &str) -> bool {
    matches!(module, "node:timers/promises" | "timers/promises")
}

fn is_node_timers_promise_export(exported: &str) -> bool {
    matches!(exported, "setTimeout" | "setImmediate")
}

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

fn node_has_async_modifier(node: TsNode) -> bool {
    crate::lower::node_has_child_kind(node, "async")
}

/// Lower a function-valued expression as a named `Func` unit (params + body),
/// registering it for detection. Mirrors `lower_func`/`lower_arrow` but takes the
/// binding name explicitly (arrow/function expressions have no own name).
pub(super) fn lower_func_value(
    lo: &mut Lowering,
    node: TsNode,
    name: Option<nose_il::Symbol>,
) -> NodeId {
    let span = lo.span(node);
    let is_async = node_has_async_modifier(node);
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
    if is_async {
        kids.push(lo.protocol_boundary(
            span,
            SourceProtocolKind::AsyncFunction,
            "async_function",
            &[body],
        ));
    } else {
        kids.push(body);
    }
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(func, UnitKind::Function, name);
    func
}

pub(super) fn lower_arrow(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let is_async = node_has_async_modifier(node);
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
    if is_async {
        kids.push(lo.protocol_boundary(
            span,
            SourceProtocolKind::AsyncFunction,
            "async_function",
            &[body],
        ));
    } else {
        kids.push(body);
    }
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
