use super::*;

pub(super) fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}
/// Lower a top-level / module-level item.
pub(super) fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_item" => Some(lower_func(lo, node, false)),
        "impl_item" | "trait_item" => Some(lower_type_block(lo, node, true)),
        "struct_item" | "enum_item" | "union_item" => Some(lower_type_block(lo, node, false)),
        "mod_item" => Some(lower_mod_item(lo, node)),
        "const_item" | "static_item" => Some(lower_value_item(lo, node)),
        "use_declaration" | "extern_crate_declaration" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        "macro_definition" => Some(lower_macro_definition_shadow(lo, node)),
        "type_item" => Some(lower_type_alias_shadow(lo, node)),
        // attributes: no behavior to model
        "attribute_item"
        | "inner_attribute_item"
        // trait method/const declarations without a body: no behavior to model
        | "function_signature_item"
        | "associated_type"
        | "line_comment"
        | "block_comment" => None,
        _ => lower_stmt(lo, node),
    }
}
pub(super) fn lower_static_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node);
    let brace_import = text.contains('{') || text.contains('}');
    let public_re_export = rust_public_use(text);
    let imports = rust_static_imports(text, span)?;
    if brace_import {
        for import in imports {
            match import.kind {
                RustStaticImportKind::Binding { exported } => {
                    crate::lower::import_binding_evidence_only(
                        lo,
                        import.span,
                        &import.local,
                        &import.module,
                        &exported,
                    );
                    if public_re_export {
                        crate::lower::re_export_binding_evidence_only(
                            lo,
                            import.span,
                            &import.local,
                            &import.module,
                            &exported,
                        );
                    }
                }
                RustStaticImportKind::Namespace => {
                    crate::lower::import_namespace_evidence_only(
                        lo,
                        import.span,
                        &import.local,
                        &import.module,
                    );
                }
            }
        }
        return Some(crate::lower::import_tokens(lo, node));
    }
    let ids = imports
        .into_iter()
        .map(|import| match import.kind {
            RustStaticImportKind::Binding { exported } => {
                if public_re_export {
                    crate::lower::re_export_binding_evidence_only(
                        lo,
                        import.span,
                        &import.local,
                        &import.module,
                        &exported,
                    );
                }
                crate::lower::import_binding(
                    lo,
                    import.span,
                    &import.local,
                    &import.module,
                    &exported,
                )
            }
            RustStaticImportKind::Namespace => {
                crate::lower::import_namespace(lo, import.span, &import.local, &import.module)
            }
        })
        .collect::<Vec<_>>();
    match ids.as_slice() {
        [] => None,
        [id] => Some(*id),
        _ => Some(lo.add(NodeKind::Seq, Payload::None, span, &ids)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RustStaticImport {
    span: Span,
    local: String,
    module: String,
    kind: RustStaticImportKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RustStaticImportKind {
    Binding { exported: String },
    Namespace,
}

fn rust_static_imports(text: &str, span: Span) -> Option<Vec<RustStaticImport>> {
    let (body, body_offset) = rust_use_body(text)?;
    if body.contains('*') {
        return None;
    }
    if body.contains('{') || body.contains('}') {
        return rust_brace_static_imports(text, span, body, body_offset);
    }
    let item = rust_single_static_import(body, body_offset, text, span)?;
    Some(vec![item])
}

fn rust_use_body(text: &str) -> Option<(&str, usize)> {
    let start = text.find("use ")? + "use ".len();
    let semi = text.rfind(';').unwrap_or(text.len());
    let raw = text.get(start..semi)?;
    let (body, trim_start) = trim_with_offset(raw);
    let (body, _) = trim_end_with_len(body);
    (!body.is_empty()).then_some((body, start + trim_start))
}

fn rust_public_use(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.starts_with("pub use ") {
        return true;
    }
    let Some(rest) = trimmed.strip_prefix("pub(") else {
        return false;
    };
    let Some((visibility, after)) = rest.split_once(')') else {
        return false;
    };
    visibility.trim() == "crate" && after.trim_start().starts_with("use ")
}

fn rust_single_static_import(
    body: &str,
    body_offset: usize,
    full_text: &str,
    declaration_span: Span,
) -> Option<RustStaticImport> {
    let parsed = parse_rust_use_item(body)?;
    let (module, exported) = split_rust_path_for_binding(&parsed.path)?;
    let local = parsed.local.unwrap_or_else(|| exported.clone());
    Some(RustStaticImport {
        span: span_for_text_range(
            declaration_span,
            full_text,
            body_offset,
            body_offset + body.len(),
        ),
        local,
        module,
        kind: RustStaticImportKind::Binding { exported },
    })
}

fn rust_brace_static_imports(
    full_text: &str,
    declaration_span: Span,
    body: &str,
    body_offset: usize,
) -> Option<Vec<RustStaticImport>> {
    let open = body.find('{')?;
    let close = matching_brace(body, open)?;
    if open >= close || !body[close + 1..].trim().is_empty() {
        return None;
    }
    let prefix = body[..open].trim().trim_end_matches("::").trim();
    if prefix.is_empty() || relative_rust_use_prefix(prefix) {
        return None;
    }
    let items = &body[open + 1..close];
    let items_offset = body_offset + open + 1;
    let mut imports = Vec::new();
    collect_rust_brace_static_imports(
        full_text,
        declaration_span,
        prefix,
        items,
        items_offset,
        &mut imports,
    )?;
    (!imports.is_empty()).then_some(imports)
}

fn collect_rust_brace_static_imports(
    full_text: &str,
    declaration_span: Span,
    prefix: &str,
    items: &str,
    items_offset: usize,
    imports: &mut Vec<RustStaticImport>,
) -> Option<()> {
    for (item, item_offset) in comma_separated_use_items(items, items_offset)? {
        if let Some(open) = item.find('{') {
            let close = matching_brace(item, open)?;
            if !item[close + 1..].trim().is_empty() {
                return None;
            }
            let nested_prefix = item[..open].trim().trim_end_matches("::").trim();
            if nested_prefix.is_empty() || relative_rust_use_prefix(nested_prefix) {
                return None;
            }
            let nested = format!("{prefix}::{nested_prefix}");
            collect_rust_brace_static_imports(
                full_text,
                declaration_span,
                &nested,
                &item[open + 1..close],
                item_offset + open + 1,
                imports,
            )?;
            continue;
        }
        let parsed = parse_rust_use_item(item)?;
        let item_span = span_for_text_range(
            declaration_span,
            full_text,
            item_offset,
            item_offset + item.len(),
        );
        if parsed.path == "self" {
            let local = parsed
                .local
                .unwrap_or_else(|| prefix.rsplit("::").next().unwrap_or(prefix).to_string());
            imports.push(RustStaticImport {
                span: item_span,
                local,
                module: prefix.to_string(),
                kind: RustStaticImportKind::Namespace,
            });
            continue;
        }
        let (path_prefix, exported) = split_rust_path_tail(&parsed.path)?;
        let module = path_prefix
            .map(|path_prefix| format!("{prefix}::{path_prefix}"))
            .unwrap_or_else(|| prefix.to_string());
        let local = parsed.local.unwrap_or_else(|| exported.clone());
        imports.push(RustStaticImport {
            span: item_span,
            local,
            module,
            kind: RustStaticImportKind::Binding { exported },
        });
    }
    Some(())
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedRustUseItem {
    path: String,
    local: Option<String>,
}

fn parse_rust_use_item(item: &str) -> Option<ParsedRustUseItem> {
    let item = item.trim();
    if item.is_empty() || item.contains('{') || item.contains('}') || item.contains('*') {
        return None;
    }
    let (path, local) = if let Some((path, local)) = item.split_once(" as ") {
        (path.trim(), Some(local.trim().to_string()))
    } else {
        (item, None)
    };
    if path.is_empty() || local.as_deref().is_some_and(str::is_empty) {
        return None;
    }
    Some(ParsedRustUseItem {
        path: path.to_string(),
        local,
    })
}

fn split_rust_path_for_binding(path: &str) -> Option<(String, String)> {
    let (module, exported) = path.rsplit_once("::")?;
    let module = module.trim();
    let exported = exported.trim();
    (!module.is_empty() && !exported.is_empty()).then(|| (module.to_string(), exported.to_string()))
}

fn split_rust_path_tail(path: &str) -> Option<(Option<String>, String)> {
    if let Some((prefix, tail)) = path.rsplit_once("::") {
        let prefix = prefix.trim();
        let tail = tail.trim();
        (!prefix.is_empty() && !tail.is_empty())
            .then(|| (Some(prefix.to_string()), tail.to_string()))
    } else {
        let tail = path.trim();
        (!tail.is_empty()).then(|| (None, tail.to_string()))
    }
}

fn relative_rust_use_prefix(prefix: &str) -> bool {
    prefix == "self"
        || prefix == "super"
        || prefix.starts_with("self::")
        || prefix.starts_with("super::")
}

fn comma_separated_use_items(items: &str, items_offset: usize) -> Option<Vec<(&str, usize)>> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, byte) in items.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 => {
                push_trimmed_use_item(items, items_offset, start, index, &mut parts);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    push_trimmed_use_item(items, items_offset, start, items.len(), &mut parts);
    Some(parts)
}

fn push_trimmed_use_item<'a>(
    items: &'a str,
    items_offset: usize,
    start: usize,
    end: usize,
    parts: &mut Vec<(&'a str, usize)>,
) {
    if let Some(part) = items.get(start..end) {
        let (trimmed, leading) = trim_with_offset(part);
        let (trimmed, _) = trim_end_with_len(trimmed);
        if !trimmed.is_empty() {
            parts.push((trimmed, items_offset + start + leading));
        }
    }
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in text.bytes().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn trim_with_offset(value: &str) -> (&str, usize) {
    let trimmed = value.trim_start();
    (trimmed, value.len() - trimmed.len())
}

fn trim_end_with_len(value: &str) -> (&str, usize) {
    let trimmed = value.trim_end();
    (trimmed, trimmed.len())
}

fn span_for_text_range(base: Span, text: &str, start: usize, end: usize) -> Span {
    let before = &text[..start.min(text.len())];
    let inside = &text[start.min(text.len())..end.min(text.len())];
    let start_line = base.start_line + before.bytes().filter(|&b| b == b'\n').count() as u32;
    let end_line = start_line + inside.bytes().filter(|&b| b == b'\n').count() as u32;
    Span::new(
        base.file,
        base.start_byte + start as u32,
        base.start_byte + end as u32,
        start_line,
        end_line,
    )
}
/// `impl`/`trait`/`struct`/`enum` → a `Class` unit whose body holds methods (each
/// also a unit) or field declarations, so similar types/impls cluster.
pub(super) fn lower_type_block(lo: &mut Lowering, node: TsNode, methods: bool) -> NodeId {
    let span = lo.span(node);
    let name = rust_item_name(lo, node);
    let body = node.child_by_field_name("body");
    let mut kids = Vec::new();
    if let Some(body) = body {
        for c in Lowering::named_children(body) {
            if methods {
                if let Some(id) = lower_item(lo, c) {
                    kids.push(id);
                }
            } else if let Some(id) = lower_field_decl(lo, c) {
                kids.push(id);
            }
        }
    }
    let payload = name.map(Payload::Name).unwrap_or(Payload::None);
    let block = lo.add(NodeKind::Block, payload, span, &kids);
    // Only `impl`/`trait` blocks (which carry behavior) are detection units. Pure
    // data-type definitions (struct/enum/union) are NOT unit-ified: they have no
    // call/control/value signal, so any two same-arity types look "similar" and
    // flood the candidate report with false positives (see docs/dogfooding.md).
    if methods {
        lo.push_unit_with_origin(block, UnitKind::Class, name, rust_type_origin(node));
    }
    block
}
pub(super) fn rust_type_origin(node: TsNode) -> UnitOrigin {
    match node.kind() {
        "trait_item" => {
            let has_body = rust_node_has_function_body(node);
            UnitOrigin::new(
                UnitDomains::of(UnitDomain::TypeContract).union(if has_body {
                    UnitDomains::of(UnitDomain::ImplementationType)
                } else {
                    UnitDomains::empty()
                }),
                UnitSubkind::InterfaceTraitProtocol,
                if has_body {
                    UnitBodyKind::Mixed
                } else {
                    UnitBodyKind::DeclarationOnly
                },
                SourceGranularity::WholeUnit,
                RegionKind::Code,
            )
            .with_evidence(if has_body {
                UnitEvidenceFlag::HasDefaultBody
            } else {
                UnitEvidenceFlag::DeclarationOnly
            })
            .with_evidence(UnitEvidenceFlag::TypeOnly)
        }
        "impl_item" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::ImplBlock,
            UnitBodyKind::Implementation,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::HasReusableBody),
        _ => UnitOrigin::unknown(),
    }
}
pub(super) fn rust_node_has_function_body(node: TsNode) -> bool {
    Lowering::named_children(node).into_iter().any(|child| {
        if rust_is_nested_item_boundary(child.kind()) {
            return false;
        }
        child.kind() == "function_item" && child.child_by_field_name("body").is_some()
            || rust_node_has_function_body(child)
    })
}
pub(super) fn rust_is_nested_item_boundary(kind: &str) -> bool {
    matches!(
        kind,
        "impl_item" | "trait_item" | "struct_item" | "enum_item" | "union_item" | "mod_item"
    )
}
pub(super) fn lower_mod_item(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let payload = rust_item_name(lo, node)
        .map(Payload::Name)
        .unwrap_or(Payload::None);
    let kids = node
        .child_by_field_name("body")
        .map(|body| {
            Lowering::named_children(body)
                .into_iter()
                .filter_map(|child| lower_item(lo, child))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    lo.add(NodeKind::Module, payload, span, &kids)
}
pub(super) fn rust_item_name(lo: &mut Lowering, node: TsNode) -> Option<Symbol> {
    node.child_by_field_name("name")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
        })
        .map(|n| lo.sym(lo.text(n)))
}
pub(super) fn lower_type_alias_shadow(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let payload = rust_item_name(lo, node)
        .map(Payload::Name)
        .unwrap_or(Payload::None);
    lo.add(NodeKind::Block, payload, span, &[])
}
/// A struct/enum field or enum variant → an `Assign(name, type-as-literal)` so the
/// shape of the data structure is captured.
pub(super) fn lower_field_decl(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "field_declaration" | "enum_variant" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| lo.var(lo.text(n), span))
                .unwrap_or_else(|| lo.empty_block(span));
            let ty = lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[]);
            Some(lo.add(NodeKind::Assign, Payload::None, span, &[name, ty]))
        }
        _ => None,
    }
}
pub(super) fn lower_value_item(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name");
    let lhs = name
        .map(|n| lo.var(lo.text(n), span))
        .unwrap_or_else(|| lo.empty_block(span));
    if let (Some(name), Some(type_node)) = (name, node.child_by_field_name("type")) {
        if let Some(domain) = lo.type_domain_from_text_with_dependencies(lo.text(type_node)) {
            lo.record_binding_domain_resolution(span, lo.text(name), domain);
        }
    }
    let rhs = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}
