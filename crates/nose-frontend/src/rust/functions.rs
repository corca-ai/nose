use super::*;

pub(super) fn lower_func(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    let is_async = rust_function_has_async_modifier(node);
    let span = lo.span(node);
    crate::lower::function_unit(lo, node, method, lower_params, |lo, body| {
        let body = lower_fn_body(lo, body);
        if is_async {
            lo.protocol_boundary(
                span,
                SourceProtocolKind::AsyncFunction,
                "async_function",
                &[body],
            )
        } else {
            body
        }
    })
}
fn rust_function_has_async_modifier(node: TsNode) -> bool {
    (0..node.child_count()).any(|index| {
        node.child(index).is_some_and(|child| {
            child.kind() == "async"
                || (child.kind() == "function_modifiers"
                    && crate::lower::node_has_child_kind(child, "async"))
        })
    })
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
                    if let Some((domain, dependencies)) =
                        rust_tokio_runtime_param_nominal_domain(lo, p, type_text)
                    {
                        if let Some(sym) = ident_of(lo, pat) {
                            let pspan = lo.span(pat);
                            lo.record_param_domain_with_dependencies(pspan, domain, dependencies);
                            out.push(lo.add(NodeKind::Param, Payload::Name(sym), pspan, &[]));
                            continue;
                        }
                    }
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
fn rust_tokio_runtime_param_nominal_domain(
    lo: &Lowering,
    param: TsNode,
    type_text: &str,
) -> Option<(DomainEvidence, Vec<nose_il::EvidenceId>)> {
    rust_tokio_runtime_nominal_type_domain(lo, param, type_text)
}
pub(super) fn rust_tokio_runtime_nominal_type_domain(
    lo: &Lowering,
    node: TsNode,
    type_text: &str,
) -> Option<(DomainEvidence, Vec<nose_il::EvidenceId>)> {
    let head = rust_type_head_preserving_case(type_text)?;
    match head.as_str() {
        "tokio::runtime::Runtime" => {
            if rust_type_reference_scope_shadows_qualified_root(lo, node, "tokio") {
                return None;
            }
            Some((rust_tokio_runtime_nominal_domain("Runtime"), Vec::new()))
        }
        "tokio::runtime::Handle" => {
            if rust_type_reference_scope_shadows_qualified_root(lo, node, "tokio") {
                return None;
            }
            Some((rust_tokio_runtime_nominal_domain("Handle"), Vec::new()))
        }
        _ if !head.contains("::") => {
            if rust_type_reference_scope_defines_type_name(lo, node, &head) {
                return None;
            }
            let mut matches = Vec::new();
            for exported in ["Runtime", "Handle"] {
                if let Some(dependencies) =
                    rust_imported_runtime_type_dependencies(lo, node, &head, exported)
                {
                    matches.push((rust_tokio_runtime_nominal_domain(exported), dependencies));
                }
            }
            let [(domain, dependencies)] = matches.as_slice() else {
                return None;
            };
            Some((*domain, dependencies.clone()))
        }
        _ => None,
    }
}
fn rust_tokio_runtime_nominal_domain(exported: &str) -> DomainEvidence {
    DomainEvidence::Nominal {
        type_hash: nose_il::stable_symbol_hash(&format!("tokio::runtime::{exported}")),
    }
}
fn rust_imported_runtime_type_dependencies(
    lo: &Lowering,
    param: TsNode,
    local: &str,
    exported: &str,
) -> Option<Vec<nose_il::EvidenceId>> {
    if rust_type_reference_scope_shadows_qualified_root(lo, param, "tokio") {
        return None;
    }
    let local_hash = nose_il::stable_symbol_hash(local);
    let module_hash = nose_il::stable_symbol_hash("tokio::runtime");
    let exported_hash = nose_il::stable_symbol_hash(exported);

    for scope in rust_type_reference_visible_scopes(param) {
        let mut dependencies = Vec::new();
        let mut saw_local_import = false;
        for record in &lo.evidence {
            let nose_il::EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                span,
                ..
            } = record.anchor
            else {
                continue;
            };
            if anchor_hash != local_hash {
                continue;
            }
            if !rust_import_span_is_direct_child_of_scope(lo, scope, span) {
                continue;
            }
            saw_local_import = true;
            let nose_il::EvidenceKind::Symbol(nose_il::SymbolEvidenceKind::ImportedBinding {
                module_hash: actual_module,
                exported_hash: actual_exported,
            }) = record.kind
            else {
                continue;
            };
            if record.status != nose_il::EvidenceStatus::Asserted
                || actual_module != module_hash
                || actual_exported != exported_hash
            {
                return None;
            }
            dependencies.push(record.id);
        }
        if saw_local_import {
            return (!dependencies.is_empty()).then_some(dependencies);
        }
    }
    None
}
fn rust_import_span_is_direct_child_of_scope(lo: &Lowering, scope: TsNode, span: Span) -> bool {
    Lowering::named_children(scope).into_iter().any(|child| {
        let child_span = lo.span(child);
        matches!(child.kind(), "use_declaration" | "extern_crate_declaration")
            && child_span.file == span.file
            && child_span.start_byte <= span.start_byte
            && span.end_byte <= child_span.end_byte
    })
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
pub(super) fn rust_type_head_preserving_case(type_text: &str) -> Option<String> {
    let compact: String = type_text.chars().filter(|c| !c.is_whitespace()).collect();
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
    rust_enclosing_module_scope(node)
        .is_some_and(|scope| rust_scope_defines_type_name(lo, scope, name))
}
fn rust_type_reference_scope_defines_type_name(lo: &Lowering, node: TsNode, name: &str) -> bool {
    rust_type_reference_visible_scopes(node)
        .into_iter()
        .any(|scope| rust_scope_defines_type_name(lo, scope, name))
}
fn rust_scope_defines_type_name(lo: &Lowering, scope: TsNode, name: &str) -> bool {
    Lowering::named_children(scope)
        .into_iter()
        .any(|child| rust_type_namespace_item_defines(lo, child, name))
}
fn rust_type_reference_scope_shadows_qualified_root(
    lo: &Lowering,
    node: TsNode,
    root: &str,
) -> bool {
    rust_type_reference_visible_scopes(node)
        .into_iter()
        .any(|scope| rust_scope_shadows_qualified_root(lo, scope, root))
}
fn rust_scope_shadows_qualified_root(lo: &Lowering, scope: TsNode, root: &str) -> bool {
    Lowering::named_children(scope).into_iter().any(|child| {
        rust_type_namespace_item_defines(lo, child, root)
            || rust_import_item_shadows_qualified_root(lo, child, root)
    }) || rust_import_evidence_shadows_qualified_root(lo, scope, root)
}
fn rust_import_evidence_shadows_qualified_root(lo: &Lowering, scope: TsNode, root: &str) -> bool {
    let local_hash = nose_il::stable_symbol_hash(root);
    let raw_local_hash = nose_il::stable_symbol_hash(&format!("r#{root}"));
    let root_hash = nose_il::stable_symbol_hash(root);
    lo.evidence.iter().any(|record| {
        let nose_il::EvidenceAnchor::Binding {
            local_hash: anchor_hash,
            span,
            ..
        } = record.anchor
        else {
            return false;
        };
        if record.status != nose_il::EvidenceStatus::Asserted
            || (anchor_hash != local_hash && anchor_hash != raw_local_hash)
            || !rust_import_span_is_direct_child_of_scope(lo, scope, span)
        {
            return false;
        }
        match record.kind {
            nose_il::EvidenceKind::Symbol(nose_il::SymbolEvidenceKind::ImportedNamespace {
                module_hash,
            }) => module_hash != root_hash,
            nose_il::EvidenceKind::Symbol(nose_il::SymbolEvidenceKind::ImportedBinding {
                ..
            }) => true,
            _ => false,
        }
    })
}
fn rust_import_item_shadows_qualified_root(lo: &Lowering, node: TsNode, root: &str) -> bool {
    match node.kind() {
        "use_declaration" => rust_use_alias_shadows_qualified_root(lo.text(node), root),
        "extern_crate_declaration" => rust_extern_crate_shadows_qualified_root(lo.text(node), root),
        _ => false,
    }
}
fn rust_use_alias_shadows_qualified_root(text: &str, root: &str) -> bool {
    if text.contains('{') || text.contains('}') || text.contains('*') {
        return false;
    }
    let words = text.split_whitespace().collect::<Vec<_>>();
    let Some(use_index) = words.iter().position(|word| *word == "use") else {
        return false;
    };
    rust_alias_words_shadow_qualified_root(&words[use_index + 1..], root)
}
fn rust_extern_crate_shadows_qualified_root(text: &str, root: &str) -> bool {
    let words = text.split_whitespace().collect::<Vec<_>>();
    let Some(crate_index) = words
        .windows(2)
        .position(|window| window[0] == "extern" && window[1] == "crate")
    else {
        return false;
    };
    rust_alias_words_shadow_qualified_root(&words[crate_index + 2..], root)
}
fn rust_alias_words_shadow_qualified_root(words: &[&str], root: &str) -> bool {
    let Some(as_index) = words.iter().position(|word| *word == "as") else {
        return false;
    };
    let Some(local) = words.get(as_index + 1) else {
        return false;
    };
    let local = rust_normalize_import_identifier(rust_trim_import_token(local));
    let aliased_path = rust_normalize_import_path(&rust_import_path_words(&words[..as_index]));
    local == root && aliased_path.trim_start_matches("::") != root
}
fn rust_import_path_words(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| rust_trim_import_token(word))
        .collect::<String>()
}
fn rust_trim_import_token(token: &str) -> &str {
    token.trim_matches(';')
}
fn rust_normalize_import_identifier(identifier: &str) -> &str {
    identifier.strip_prefix("r#").unwrap_or(identifier)
}
fn rust_normalize_import_path(path: &str) -> String {
    path.split("::")
        .map(rust_normalize_import_identifier)
        .collect::<Vec<_>>()
        .join("::")
}
pub(super) fn rust_enclosing_module_scope(mut node: TsNode) -> Option<TsNode> {
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
fn rust_type_reference_visible_scopes(mut node: TsNode) -> Vec<TsNode> {
    let mut scopes = Vec::new();
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "block" | "source_file" => {
                push_unique_ts_node(&mut scopes, parent);
                if parent.kind() == "source_file" {
                    break;
                }
            }
            "declaration_list" if parent.parent().is_some_and(|p| p.kind() == "mod_item") => {
                push_unique_ts_node(&mut scopes, parent);
                break;
            }
            _ => {}
        }
        node = parent;
    }
    scopes
}
fn push_unique_ts_node<'tree>(nodes: &mut Vec<TsNode<'tree>>, node: TsNode<'tree>) {
    if nodes.iter().all(|existing| existing.id() != node.id()) {
        nodes.push(node);
    }
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
        .is_some_and(|name_node| rust_normalize_import_identifier(lo.text(name_node)) == name)
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
