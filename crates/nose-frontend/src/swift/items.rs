use super::*;

pub(super) fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}
pub(super) fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_declaration" => Some(lower_function(lo, node, false)),
        "protocol_function_declaration" => Some(lower_function(lo, node, true)),
        "init_declaration" | "deinit_declaration" | "subscript_declaration" => {
            Some(lower_function(lo, node, true))
        }
        "actor_declaration"
        | "class_declaration"
        | "struct_declaration"
        | "enum_declaration"
        | "protocol_declaration" => Some(lower_type(lo, node)),
        "extension_declaration" => Some(lower_extension(lo, node)),
        "property_declaration"
        | "protocol_property_declaration"
        | "protocol_property_requirements" => Some(lower_property(lo, node)),
        "enum_entry" => Some(lower_enum_entry(lo, node)),
        "import_declaration" => Some(lower_import(lo, node)),
        "typealias_declaration" => {
            record_typealias_shadow(lo, node);
            None
        }
        "associatedtype_declaration"
        | "operator_declaration"
        | "precedence_group_declaration"
        | "macro_declaration"
        | "line_comment"
        | "multiline_comment" => None,
        _ => lower_stmt(lo, node),
    }
}
pub(super) fn lower_enum_entry(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(name) = node.child_by_field_name("name").or_else(|| {
        Lowering::named_children(node)
            .into_iter()
            .find(|child| matches!(child.kind(), "simple_identifier" | "identifier"))
    }) {
        kids.push(lo.var(lo.text(name), lo.span(name)));
    }
    for child in Lowering::named_children(node) {
        if matches!(
            child.kind(),
            "enum_type_parameters" | "parameter" | "tuple_type"
        ) {
            kids.push(lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym(&format!("swift_{}", child.kind()))),
                lo.span(child),
                &[],
            ));
        }
    }
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_enum_entry")),
        span,
        &kids,
    )
}
pub(super) fn lower_import(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let module = Lowering::named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "identifier" | "simple_identifier"))
        .map(|child| lo.text(child))
        .collect::<Vec<_>>()
        .join(".");
    if module.is_empty() {
        crate::lower::import_tokens(lo, node)
    } else {
        crate::lower::import_namespace(lo, span, &module, &module)
    }
}
pub(super) fn lower_type(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node.child_by_field_name("body");
    let mut kids = Vec::new();
    if let Some(body) = body {
        for child in Lowering::named_children(body) {
            if let Some(id) = lower_item(lo, child) {
                kids.push(id);
            }
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit_with_origin(block, UnitKind::Class, name, swift_type_origin(node));
    block
}
pub(super) fn lower_extension(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    for child in Lowering::named_children(node) {
        match child.kind() {
            "class_body" | "enum_class_body" => {
                for item in Lowering::named_children(child) {
                    if let Some(id) = lower_item(lo, item) {
                        kids.push(id);
                    }
                }
            }
            _ => {}
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit_with_origin(block, UnitKind::Class, None, swift_extension_origin(node));
    block
}
pub(super) fn record_typealias_shadow(lo: &mut Lowering, node: TsNode) {
    let span = lo.span(node);
    if let Some(name) = swift_decl_name(lo, node) {
        // Keep type-only syntax out of the structural tree while preserving the
        // declaration as a same-file shadow for stdlib free-name contracts.
        lo.add(NodeKind::Block, Payload::Name(name), span, &[]);
    }
}
pub(super) fn lower_function(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    let span = lo.span(node);
    let name = swift_decl_name(lo, node);
    let mut kids = Vec::new();
    for param in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "parameter")
    {
        lower_param(lo, param, &mut kids);
    }
    let body_node = node.child_by_field_name("body");
    let body = body_node
        .map(|body| lower_function_body(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    let kind = if method {
        UnitKind::Method
    } else {
        UnitKind::Function
    };
    let origin = swift_callable_origin(node, method, body_node.is_some());
    lo.push_unit_with_origin(func, kind, name, origin);
    func
}
pub(super) fn swift_callable_origin(node: TsNode, method: bool, has_body: bool) -> UnitOrigin {
    if node.kind().starts_with("protocol_") && !has_body {
        return UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
            UnitSubkind::FunctionPrototype,
            UnitBodyKind::DeclarationOnly,
            SourceGranularity::Member,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::ProtocolRequirement)
        .with_evidence(UnitEvidenceFlag::DeclarationOnly)
        .with_evidence(UnitEvidenceFlag::TypeOnly);
    }
    crate::lower::imperative_callable_origin(
        if method {
            UnitSubkind::Method
        } else {
            UnitSubkind::Function
        },
        has_body,
    )
}
pub(super) fn swift_type_origin(node: TsNode) -> UnitOrigin {
    match node.kind() {
        "protocol_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
            UnitSubkind::InterfaceTraitProtocol,
            UnitBodyKind::DeclarationOnly,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::ProtocolRequirement)
        .with_evidence(UnitEvidenceFlag::DeclarationOnly)
        .with_evidence(UnitEvidenceFlag::TypeOnly),
        "class_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::Class,
            if swift_node_has_reusable_body(node) {
                UnitBodyKind::Implementation
            } else {
                UnitBodyKind::DeclarationOnly
            },
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(if swift_node_has_reusable_body(node) {
            UnitEvidenceFlag::HasReusableBody
        } else {
            UnitEvidenceFlag::DeclarationOnly
        }),
        "actor_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::Actor,
            if swift_node_has_reusable_body(node) {
                UnitBodyKind::Implementation
            } else {
                UnitBodyKind::DeclarationOnly
            },
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::ActorIsolated)
        .with_evidence(if swift_node_has_reusable_body(node) {
            UnitEvidenceFlag::HasReusableBody
        } else {
            UnitEvidenceFlag::DeclarationOnly
        }),
        "enum_declaration" => {
            let body = if swift_node_has_reusable_body(node) {
                UnitBodyKind::Mixed
            } else {
                UnitBodyKind::DeclarativeDenotation
            };
            UnitOrigin::new(
                UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::Data),
                UnitSubkind::Enum,
                body,
                SourceGranularity::WholeUnit,
                RegionKind::Code,
            )
            .with_domain(if swift_node_has_reusable_body(node) {
                UnitDomain::ImplementationType
            } else {
                UnitDomain::Unknown
            })
            .with_evidence(if swift_node_has_reusable_body(node) {
                UnitEvidenceFlag::HasReusableBody
            } else {
                UnitEvidenceFlag::DataShapeOnly
            })
        }
        "struct_declaration" => {
            let body = if swift_node_has_reusable_body(node) {
                UnitBodyKind::Mixed
            } else {
                UnitBodyKind::DeclarativeDenotation
            };
            UnitOrigin::new(
                UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::Data),
                UnitSubkind::StructRecord,
                body,
                SourceGranularity::WholeUnit,
                RegionKind::Code,
            )
            .with_domain(if swift_node_has_reusable_body(node) {
                UnitDomain::ImplementationType
            } else {
                UnitDomain::Unknown
            })
            .with_evidence(if swift_node_has_reusable_body(node) {
                UnitEvidenceFlag::HasReusableBody
            } else {
                UnitEvidenceFlag::DataShapeOnly
            })
        }
        _ => UnitOrigin::unknown(),
    }
}
pub(super) fn swift_extension_origin(node: TsNode) -> UnitOrigin {
    let has_body = swift_node_has_reusable_body(node);
    UnitOrigin::new(
        UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::ImplementationType),
        UnitSubkind::ExtensionImpl,
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
}
pub(super) fn swift_node_has_reusable_body(node: TsNode) -> bool {
    Lowering::named_children(node).into_iter().any(|child| {
        if swift_is_nested_type_decl(child.kind()) {
            return false;
        }
        matches!(
            child.kind(),
            "function_body"
                | "getter_effects"
                | "setter_effects"
                | "code_block"
                | "computed_getter"
                | "computed_modify"
                | "computed_setter"
                | "computed_property"
        ) || child.child_by_field_name("body").is_some()
            || swift_node_has_reusable_body(child)
    })
}
pub(super) fn swift_is_nested_type_decl(kind: &str) -> bool {
    matches!(
        kind,
        "actor_declaration"
            | "class_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "protocol_declaration"
            | "extension_declaration"
    )
}
pub(super) fn swift_decl_name(lo: &mut Lowering, node: TsNode) -> Option<Symbol> {
    node.child_by_field_name("name")
        .or_else(|| {
            Lowering::named_children(node).into_iter().find(|child| {
                matches!(child.kind(), "simple_identifier" | "identifier")
                    || child.kind() == "custom_operator"
                    || is_swift_operator_token_kind(child.kind())
            })
        })
        .map(|name| lo.sym(lo.text(name)))
}
pub(super) fn lower_param(lo: &mut Lowering, param: TsNode, out: &mut Vec<NodeId>) {
    let span = lo.span(param);
    let name = parameter_binding_name(param);
    let payload = name
        .filter(|n| lo.text(*n) != "_")
        .map(|n| Payload::Name(lo.sym(lo.text(n))))
        .unwrap_or(Payload::None);
    if let Some(domain) = param
        .child_by_field_name("type")
        .and_then(|ty| lo.type_domain_from_text_with_dependencies(lo.text(ty)))
        .or_else(|| lo.type_domain_from_text_with_dependencies(lo.text(param)))
    {
        lo.record_param_domain_resolution(span, domain);
    }
    out.push(lo.add(NodeKind::Param, payload, span, &[]));
}
pub(super) fn parameter_binding_name(param: TsNode) -> Option<TsNode> {
    let mut cursor = param.walk();
    let named: Vec<TsNode> = param
        .children_by_field_name("name", &mut cursor)
        .filter(|child| matches!(child.kind(), "simple_identifier" | "self_expression"))
        .collect();
    named.last().copied().or_else(|| {
        Lowering::named_children(param)
            .into_iter()
            .rfind(|child| matches!(child.kind(), "simple_identifier" | "self_expression"))
    })
}
pub(super) fn lower_function_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let statements = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "statements")
        .unwrap_or(node);
    let children = Lowering::named_children(statements);
    let last_index = children.len().saturating_sub(1);
    let mut stmts = Vec::new();
    for (idx, child) in children.into_iter().enumerate() {
        if idx == last_index && is_tail_expr(child.kind()) {
            let expr = lower_expr(lo, child);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[expr]));
        } else if let Some(id) = lower_stmt(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}
