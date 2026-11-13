use super::*;

pub(super) fn lower_module(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, |lo, c| lower_stmt(lo, c, false))
}
/// Lower one statement. `in_class` tags nested `def`s as methods. Returns `None`
/// for statements that are pure noise for clone detection (imports, globals).
pub(super) fn lower_stmt(lo: &mut Lowering, node: TsNode, in_class: bool) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "function_definition" => {
            let out = lower_func(lo, node, in_class);
            clear_defined_param_alias(lo, node);
            Some(out)
        }
        "decorated_definition" => {
            // Lower only the wrapped definition — but RECORD the decoration as a
            // binding source fact: the runtime binding is `decorator(f)`, not `f`,
            // so call-target evidence / content-keyed seeding / inlining must not
            // attribute the bare body to the name (coevo series 6, S2-A: `@double`
            // vs `@triple` callers false-merged as "exact behavior match").
            let def = node.child_by_field_name("definition")?;
            let def_span = lo.span(def);
            let lowered = lower_stmt(lo, def, in_class)?;
            lo.record_source_fact(
                def_span,
                SourceFactKind::Binding(SourceBindingKind::DecoratedDefinition),
            );
            Some(lowered)
        }
        "class_definition" => {
            let out = lower_class(lo, node);
            clear_defined_param_alias(lo, node);
            Some(out)
        }
        "if_statement" => Some(lower_if(lo, node)),
        "match_statement" => Some(lower_match(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = first_semantic_named_child(node) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "raise_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = first_semantic_named_child(node) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Throw, Payload::None, span, &kids))
        }
        "try_statement" => Some(lower_try(lo, node)),
        "with_statement" => {
            // Keep the body available for structural matching, but never erase
            // context evaluation, binding, enter/exit, or exception suppression.
            let body = node.child_by_field_name("body");
            let lowered = match body {
                Some(b) => lower_block(lo, b, false),
                None => lo.empty_block(span),
            };
            Some(if crate::lower::node_has_child_kind(node, "async") {
                lo.protocol_boundary(
                    span,
                    nose_il::SourceProtocolKind::AsyncContext,
                    "async_with",
                    &[lowered],
                )
            } else {
                opaque_statement(lo, node, &[lowered])
            })
        }
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        "pass_statement" => Some(lo.empty_block(span)),
        "assert_statement" => {
            let kids: Vec<_> = semantic_named_children(node)
                .into_iter()
                .map(|child| lower_expr(lo, child))
                .collect();
            Some(opaque_statement(lo, node, &kids))
        }
        "delete_statement" => {
            let kids: Vec<_> = semantic_named_children(node)
                .into_iter()
                .map(|child| lower_expr(lo, child))
                .collect();
            Some(opaque_statement(lo, node, &kids))
        }
        "expression_statement" => {
            let child = node.named_child(0)?;
            match child.kind() {
                "assignment" => Some(lower_assignment(lo, child)),
                "augmented_assignment" => Some(lower_aug_assignment(lo, child)),
                _ => {
                    let e = lower_expr(lo, child);
                    Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
                }
            }
        }
        "import_statement" | "import_from_statement" | "future_import_statement" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        "global_statement" | "nonlocal_statement" | "comment" | "line_continuation" => None,
        // Anything else in statement position: treat as an expression statement
        // (lower_expr has its own Raw fallback for genuinely unknown nodes).
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

/// Unsupported statement semantics must remain source-keyed and exact-ineligible.
/// A surface-kind-only Raw would conflate different context managers/targets.
pub(super) fn opaque_statement(lo: &mut Lowering, node: TsNode, kids: &[NodeId]) -> NodeId {
    let key = stable_symbol_hash(lo.text(node));
    lo.raw(&format!("{}:{key:016x}", node.kind()), lo.span(node), kids)
}
pub(super) fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim();
    let mut assigns = Vec::new();

    if let Some(rest) = text.strip_prefix("from ") {
        let (module, names) = rest.split_once(" import ")?;
        if names.trim() == "*" {
            lo.record_evidence(
                EvidenceAnchor::source_span(span),
                EvidenceKind::Import(ImportEvidenceKind::Wildcard {
                    module_hash: stable_symbol_hash(module.trim()),
                }),
                "python_wildcard_import",
            );
            return Some(lo.raw("python_wildcard_import", span, &[]));
        }
        for part in names.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (exported, local) = py_import_specifier(part);
            let (assign, import_evidence) = crate::lower::import_binding_with_symbol_evidence(
                lo,
                span,
                local,
                module.trim(),
                exported,
            );
            if let Some(contract) =
                nose_semantics::python_stdlib_type_domain_contract(module.trim(), exported)
            {
                lo.record_type_domain_alias_with_pack_evidence(
                    local,
                    contract.domain,
                    import_evidence,
                    crate::type_domain_aliases::TypeDomainEvidenceProvenance {
                        evidence_provenance: crate::lower::builtin_evidence_provenance(
                            contract.pack_id,
                            contract.producer_id,
                        ),
                    },
                );
            } else {
                lo.clear_type_domain_alias(local);
            }
            assigns.push(assign);
        }
    } else if let Some(rest) = text.strip_prefix("import ") {
        for part in rest.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (module, local) = py_import_specifier(part);
            lo.clear_type_domain_alias(local);
            assigns.push(crate::lower::import_namespace(
                lo,
                span,
                local,
                module.trim(),
            ));
        }
    }

    match assigns.len() {
        0 => None,
        1 => assigns.pop(),
        _ => Some(lo.add(NodeKind::Block, Payload::None, span, &assigns)),
    }
}
pub(super) fn py_import_specifier(part: &str) -> (&str, &str) {
    if let Some((exported, local)) = part.split_once(" as ") {
        (exported.trim(), local.trim())
    } else {
        let local = part.rsplit('.').next().unwrap_or(part).trim();
        (part.trim(), local)
    }
}
pub(super) fn lower_block(lo: &mut Lowering, node: TsNode, in_class: bool) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Block, |lo, c| {
        lower_stmt(lo, c, in_class)
    })
}
pub(super) fn lower_docstring_block(lo: &mut Lowering, node: TsNode, in_class: bool) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for (idx, child) in semantic_named_children(node).into_iter().enumerate() {
        if idx == 0 && is_docstring_stmt(child) {
            continue;
        }
        if let Some(stmt) = lower_stmt(lo, child, in_class) {
            stmts.push(stmt);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}
pub(super) fn is_docstring_stmt(node: TsNode) -> bool {
    node.kind() == "expression_statement"
        && node.named_child(0).is_some_and(is_static_string_doc_expr)
}
pub(super) fn is_static_string_doc_expr(node: TsNode) -> bool {
    match node.kind() {
        "string" | "concatenated_string" => !contains_interpolation(node),
        "parenthesized_expression" => {
            let children = semantic_named_children(node);
            children.len() == 1 && is_static_string_doc_expr(children[0])
        }
        _ => false,
    }
}
pub(super) fn contains_interpolation(node: TsNode) -> bool {
    node.kind() == "interpolation"
        || semantic_named_children(node)
            .into_iter()
            .any(contains_interpolation)
}
