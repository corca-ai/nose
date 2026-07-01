use super::*;

pub(super) fn lower_property(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut assigns = Vec::new();
    let is_async_let = swift_property_is_async_let(lo.text(node));
    let mut cursor = node.walk();
    let names: Vec<TsNode> = node.children_by_field_name("name", &mut cursor).collect();
    let values = field_children(node, "value");
    let types = field_children(node, "type");
    for (idx, name_node) in names.iter().enumerate() {
        if let Some(ty) = types.get(idx).or_else(|| types.first()) {
            record_property_binding_domain(lo, *name_node, *ty);
        } else {
            record_property_binding_domain_from_decl_text(lo, *name_node, node);
        }
        let lhs = binding_var(lo, *name_node, span);
        let rhs = values
            .get(idx)
            .or_else(|| values.first())
            .map(|value| lower_expr(lo, *value))
            .or_else(|| lower_computed_property(lo, node))
            .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
        let assign = lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        assigns.push(if is_async_let {
            lo.protocol_boundary(span, SourceProtocolKind::TaskSpawn, "task_spawn", &[assign])
        } else {
            assign
        });
    }
    if assigns.is_empty() {
        lower_computed_property(lo, node).unwrap_or_else(|| lo.empty_block(span))
    } else if assigns.len() == 1 {
        assigns[0]
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

fn swift_property_is_async_let(text: &str) -> bool {
    let text = text.trim_start();
    let Some(after_async) = consume_swift_keyword(text, "async") else {
        return false;
    };
    consume_swift_keyword(after_async.trim_start(), "let").is_some()
}
pub(super) fn record_property_binding_domain(
    lo: &mut Lowering,
    name_node: TsNode,
    type_node: TsNode,
) {
    let Some(name) = binding_name(lo, name_node) else {
        return;
    };
    if name == "_" {
        return;
    }
    let Some(domain) = lo.type_domain_from_text_with_dependencies(lo.text(type_node)) else {
        return;
    };
    lo.record_evidence_with_provenance_dependencies(
        EvidenceAnchor::binding(lo.span(name_node), stable_symbol_hash(&name)),
        EvidenceKind::Domain(domain.domain),
        domain.provenance.evidence_provenance,
        domain.dependencies,
    );
}
pub(super) fn record_property_binding_domain_from_decl_text(
    lo: &mut Lowering,
    name_node: TsNode,
    property_node: TsNode,
) {
    let Some(name) = binding_name(lo, name_node) else {
        return;
    };
    if name == "_" {
        return;
    }
    let decl = lo.text(property_node);
    let Some(name_start) = decl.find(&name) else {
        return;
    };
    let after_name = &decl[name_start + name.len()..];
    let Some((_, after_colon)) = after_name.split_once(':') else {
        return;
    };
    let ty = after_colon
        .split(['=', ','])
        .next()
        .unwrap_or(after_colon)
        .trim();
    if ty.is_empty() {
        return;
    }
    let annotated = format!("{name}: {ty}");
    let Some(domain) = lo.type_domain_from_text_with_dependencies(&annotated) else {
        return;
    };
    lo.record_evidence_with_provenance_dependencies(
        EvidenceAnchor::binding(lo.span(name_node), stable_symbol_hash(&name)),
        EvidenceKind::Domain(domain.domain),
        domain.provenance.evidence_provenance,
        domain.dependencies,
    );
}
pub(super) fn lower_computed_property(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    node.child_by_field_name("computed_value")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| child.kind() == "computed_property")
        })
        .map(|computed| lower_computed_property_body(lo, computed))
}
fn lower_computed_property_body(lo: &mut Lowering, computed: TsNode) -> NodeId {
    let span = lo.span(computed);
    let blocks: Vec<NodeId> = Lowering::named_children(computed)
        .into_iter()
        .filter_map(|child| match child.kind() {
            "statements" | "function_body" => Some(lower_block(lo, child)),
            "computed_getter" | "computed_setter" | "computed_modify" => {
                Some(lower_computed_accessor(lo, child))
            }
            _ => None,
        })
        .collect();
    match blocks.as_slice() {
        [only] => *only,
        [] => lo.empty_block(span),
        _ => lo.add(NodeKind::Block, Payload::None, span, &blocks),
    }
}
fn lower_computed_accessor(lo: &mut Lowering, accessor: TsNode) -> NodeId {
    Lowering::named_children(accessor)
        .into_iter()
        .find(|child| matches!(child.kind(), "statements" | "function_body"))
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(lo.span(accessor)))
}
pub(super) fn type_surface_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "user_type" | "optional_type" | "array_type" | "dictionary_type" => {
            Lowering::named_children(node)
                .into_iter()
                .find_map(|child| type_surface_name(lo, child))
        }
        "type_identifier" | "simple_identifier" | "identifier" => Some(lo.text(node).to_string()),
        _ => None,
    }
}
