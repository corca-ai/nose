use super::*;

pub(super) fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    crate::lower::function_unit(lo, node, method, lower_params, lower_fn_body)
}
pub(super) fn lower_params(lo: &mut Lowering, params: TsNode, out: &mut Vec<NodeId>) {
    for p in Lowering::named_children(params) {
        let span = lo.span(p);
        match p.kind() {
            "self_parameter" => out.push(lo.add(NodeKind::Param, Payload::None, span, &[])),
            "parameter" => {
                if let Some(pat) = p.child_by_field_name("pattern") {
                    let semantic_text = p.child_by_field_name("type").map(|ty| lo.text(ty));
                    let type_text = semantic_text.unwrap_or_else(|| lo.text(p));
                    if let Some(domain) = lo.type_domain_from_text_with_dependencies(type_text) {
                        if let Some(sym) = ident_of(lo, pat) {
                            let pspan = lo.span(pat);
                            if rust_param_domain_is_safe(lo, p, type_text, domain.domain) {
                                lo.record_param_domain_resolution(pspan, domain);
                                out.push(lo.add(NodeKind::Param, Payload::Name(sym), pspan, &[]));
                                continue;
                            }
                        }
                    }
                    push_pattern_params(lo, pat, out);
                } else {
                    out.push(lo.add(NodeKind::Param, Payload::None, span, &[]));
                }
            }
            // Closure params (`|a, v|`) are bare identifiers/patterns, not `parameter`
            // nodes — name them so a closure's body binds them (else a `.fold` closure's
            // accumulator/element are free vars and the fold never converges with a loop).
            _ => push_pattern_params(lo, p, out),
        }
    }
}
fn rust_param_domain_is_safe(
    lo: &Lowering,
    param: TsNode,
    type_text: &str,
    domain: DomainEvidence,
) -> bool {
    domain != DomainEvidence::Result || rust_result_type_reference_is_safe(lo, param, type_text)
}
fn rust_result_type_reference_is_safe(lo: &Lowering, param: TsNode, type_text: &str) -> bool {
    match rust_compact_type_path(type_text).as_deref() {
        Some("result") => !rust_module_scope_defines_type_name(lo, param, "Result"),
        Some("std::result::result" | "core::result::result") => true,
        Some(path) if path.ends_with("::result") => false,
        _ => false,
    }
}
fn rust_compact_type_path(type_text: &str) -> Option<String> {
    let compact: String = type_text
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    let mut ty = compact
        .split('=')
        .next()
        .unwrap_or(compact.as_str())
        .trim_start_matches("::");
    while let Some(rest) = ty.strip_prefix('&') {
        ty = rest.strip_prefix("mut").unwrap_or(rest);
    }
    let head = ty.split(['<', '[', '(']).next().unwrap_or(ty);
    (!head.is_empty()).then(|| head.to_string())
}
fn rust_module_scope_defines_type_name(lo: &Lowering, node: TsNode, name: &str) -> bool {
    rust_enclosing_module_scope(node).is_some_and(|scope| {
        Lowering::named_children(scope)
            .into_iter()
            .any(|child| rust_type_namespace_item_defines(lo, child, name))
    })
}
fn rust_enclosing_module_scope(mut node: TsNode) -> Option<TsNode> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "source_file"
            || (parent.kind() == "declaration_list"
                && parent.parent().is_some_and(|p| p.kind() == "mod_item"))
        {
            return Some(parent);
        }
        node = parent;
    }
    None
}
fn rust_type_namespace_item_defines(lo: &Lowering, node: TsNode, name: &str) -> bool {
    if !matches!(
        node.kind(),
        "struct_item" | "enum_item" | "union_item" | "type_item" | "trait_item" | "mod_item"
    ) {
        return false;
    }
    node.child_by_field_name("name")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
        })
        .is_some_and(|name_node| lo.text(name_node) == name)
}
pub(super) fn push_pattern_params(lo: &mut Lowering, pat: TsNode, out: &mut Vec<NodeId>) {
    match pat.kind() {
        "tuple_pattern" | "tuple_expression" => {
            for child in Lowering::named_children(pat) {
                push_pattern_params(lo, child, out);
            }
        }
        _ => {
            let span = lo.span(pat);
            match ident_of(lo, pat) {
                Some(sym) => out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[])),
                None => out.push(lo.add(NodeKind::Param, Payload::None, span, &[])),
            }
        }
    }
}
/// Extract the binding identifier from a (simple) pattern.
pub(super) fn ident_of(lo: &Lowering, pat: TsNode) -> Option<Symbol> {
    match pat.kind() {
        "identifier" | "type_identifier" | "field_identifier" => Some(lo.sym(lo.text(pat))),
        // `mut x`, `ref x`, `&x`, `x: T` — descend to the inner identifier
        "mut_pattern" | "ref_pattern" | "reference_pattern" => {
            pat.named_child(0).and_then(|c| ident_of(lo, c))
        }
        _ => pat.named_child(0).and_then(|c| ident_of(lo, c)),
    }
}
pub(super) fn lower_static_projection_pattern(
    lo: &mut Lowering,
    pattern: TsNode,
    base: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    if pattern.kind() != "struct_pattern" {
        return None;
    }
    let mut assigns = Vec::new();
    for child in Lowering::named_children(pattern) {
        match child.kind() {
            "type_identifier" | "scoped_type_identifier" | "qualified_type" => {}
            "remaining_field_pattern" => {}
            "field_pattern" => {
                let (field, local) = rust_field_projection(lo, child)?;
                assigns.push(rust_projection_assign(lo, base, &field, &local, span));
            }
            "shorthand_field_identifier_pattern" | "field_identifier" => {
                let name = lo.text(child).to_string();
                assigns.push(rust_projection_assign(lo, base, &name, &name, span));
            }
            _ => return rust_struct_pattern_text_projection(lo, pattern, base, span),
        }
    }
    if assigns.is_empty() {
        rust_struct_pattern_text_projection(lo, pattern, base, span)
    } else {
        Some(assigns)
    }
}
pub(super) fn rust_field_projection(lo: &Lowering, node: TsNode) -> Option<(String, String)> {
    let kids: Vec<TsNode> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() != "mutable_specifier")
        .collect();
    let field = kids.first().and_then(|&k| rust_field_name(lo, k))?;
    let local = kids
        .iter()
        .skip(1)
        .find_map(|&k| rust_binding_name(lo, k))
        .unwrap_or_else(|| field.clone());
    Some((field, local))
}
pub(super) fn rust_field_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "field_identifier" | "identifier" | "shorthand_field_identifier" => {
            Some(lo.text(node).to_string())
        }
        _ => None,
    }
}
pub(super) fn rust_binding_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "shorthand_field_identifier" => {
            Some(lo.text(node).to_string())
        }
        "mut_pattern" | "ref_pattern" | "reference_pattern" => {
            node.named_child(0).and_then(|n| rust_binding_name(lo, n))
        }
        _ => None,
    }
}
pub(super) fn rust_projection_assign(
    lo: &mut Lowering,
    base: NodeId,
    field: &str,
    local: &str,
    span: Span,
) -> NodeId {
    let lhs = lo.var(local, span);
    let sym = lo.sym(field);
    let rhs = lo.add(NodeKind::Field, Payload::Name(sym), span, &[base]);
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}
pub(super) fn rust_struct_pattern_text_projection(
    lo: &mut Lowering,
    pattern: TsNode,
    base: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    let text = lo.text(pattern);
    let open = text.find('{')?;
    let close = text.rfind('}')?;
    if close <= open {
        return None;
    }
    let mut assigns = Vec::new();
    for part in text[open + 1..close].split(',') {
        let part = part.trim();
        if part.is_empty() || part == ".." {
            continue;
        }
        let (field, local) = match part.split_once(':') {
            Some((field, local)) => {
                let field = field.trim();
                let local = local.trim();
                if !simple_rust_ident(field) || !simple_rust_ident(local) {
                    return None;
                }
                (field, local)
            }
            None => {
                if !simple_rust_ident(part) {
                    return None;
                }
                (part, part)
            }
        };
        assigns.push(rust_projection_assign(lo, base, field, local, span));
    }
    (!assigns.is_empty()).then_some(assigns)
}
pub(super) fn simple_rust_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
/// Lower a function body block, wrapping its tail expression in a `Return` — in
/// Rust the block's final expression *is* the return value, so this converges with
/// an explicit `return` (and with other languages' explicit returns).
pub(super) fn lower_fn_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let children = Lowering::named_children(node);
    let n = children.len();
    let mut stmts = Vec::new();
    for (idx, child) in children.into_iter().enumerate() {
        let k = child.kind();
        if idx + 1 == n && k == "expression_statement" && !lo.text(child).trim_end().ends_with(';')
        {
            let expr = child.named_child(0).unwrap_or(child);
            let e = lower_expr(lo, expr);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[e]));
        } else if idx + 1 == n && is_tail_expr(k) {
            let e = lower_expr(lo, child);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[e]));
        } else if let Some(id) = lower_item(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}
