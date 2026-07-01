use super::*;

pub(super) fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}
pub(super) fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => Some(lower_type(lo, node)),
        "method_declaration" | "constructor_declaration" => Some(lower_method(lo, node)),
        "compact_constructor_declaration" => Some(lower_compact_constructor(lo, node)),
        "field_declaration" | "constant_declaration" => Some(lower_field(lo, node)),
        "enum_body_declarations" => Some(lower_body_declarations(lo, node)),
        "static_initializer" => Some(lower_static_initializer(lo, node)),
        "import_declaration" => {
            Some(lower_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)))
        }
        "package_declaration" => Some(crate::lower::import_tokens(lo, node)),
        "module_declaration" => Some(lower_module_declaration(lo, node)),
        "line_comment" | "block_comment" => None,
        _ => lower_stmt(lo, node),
    }
}
pub(super) fn lower_module_declaration(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter_map(|child| match child.kind() {
            "identifier" | "scoped_identifier" => Some(lo.str_lit(lo.text(child), lo.span(child))),
            "module_body" => Some(lower_module_body(lo, child)),
            _ => None,
        })
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("java_module_declaration")),
        span,
        &kids,
    )
}
pub(super) fn lower_module_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter_map(|child| match child.kind() {
            "requires_module_directive"
            | "exports_module_directive"
            | "opens_module_directive"
            | "uses_module_directive"
            | "provides_module_directive" => Some(lower_module_directive(lo, child)),
            _ => None,
        })
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("java_module_body")),
        span,
        &kids,
    )
}
pub(super) fn lower_module_directive(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let tag = format!("java_{}", node.kind());
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter_map(|child| match child.kind() {
            "identifier" | "type_identifier" | "scoped_identifier" | "scoped_type_identifier" => {
                Some(lo.str_lit(lo.text(child), lo.span(child)))
            }
            "requires_modifier" => Some(lo.add(
                NodeKind::Seq,
                Payload::Name(lo.sym(&format!("java_requires_modifier_{}", lo.text(child)))),
                lo.span(child),
                &[],
            )),
            _ => None,
        })
        .collect();
    lo.add(NodeKind::Seq, Payload::Name(lo.sym(&tag)), span, &kids)
}
pub(super) fn lower_body_declarations(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter_map(|child| lower_item(lo, child))
        .collect();
    lo.add(NodeKind::Block, Payload::None, span, &kids)
}
pub(super) fn lower_static_initializer(lo: &mut Lowering, node: TsNode) -> NodeId {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "block")
        .map(|block| lower_block(lo, block))
        .unwrap_or_else(|| lo.empty_block(lo.span(node)))
}
pub(super) fn lower_import(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim().trim_end_matches(';').trim();
    if let Some(path) = text.strip_prefix("import static ") {
        let path = path.trim();
        if path.ends_with(".*") {
            return None;
        }
        let (module, exported) = path.rsplit_once('.')?;
        return Some(crate::lower::import_binding(
            lo,
            span,
            exported.trim(),
            module.trim(),
            exported.trim(),
        ));
    }
    let path = text.strip_prefix("import ")?.trim();
    if path.ends_with(".*") {
        let module = path.trim_end_matches(".*").trim();
        let wildcard_evidence = lo.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Import(ImportEvidenceKind::Wildcard {
                module_hash: stable_symbol_hash(module),
            }),
            "java_wildcard_import",
        );
        if module == "java.util.concurrent" {
            record_java_concurrent_wildcard_type_domain_aliases(lo, span, wildcard_evidence);
        }
        return None;
    }
    let (module, exported) = path.rsplit_once('.')?;
    let (assign, import_evidence) = crate::lower::import_binding_with_symbol_evidence(
        lo,
        span,
        exported.trim(),
        module.trim(),
        exported.trim(),
    );
    record_java_type_domain_alias(lo, module.trim(), exported.trim(), import_evidence);
    Some(assign)
}

fn record_java_type_domain_alias(
    lo: &mut Lowering,
    module: &str,
    exported: &str,
    import_evidence: Option<nose_il::EvidenceId>,
) {
    if module == "java.util.concurrent" {
        let domain = match exported {
            "CompletableFuture" | "CompletionStage" | "Future" | "ScheduledFuture" => {
                Some(nose_il::DomainEvidence::FutureLike)
            }
            "Executor" | "ExecutorService" | "ScheduledExecutorService" => {
                Some(nose_il::DomainEvidence::Nominal {
                    type_hash: stable_symbol_hash(&format!("java.util.concurrent.{exported}")),
                })
            }
            _ => None,
        };
        if let Some(domain) = domain {
            lo.record_type_domain_alias_exact_with_evidence(exported, domain, import_evidence);
            return;
        }
    }
    lo.clear_type_domain_alias(exported);
}

fn record_java_concurrent_wildcard_type_domain_aliases(
    lo: &mut Lowering,
    span: Span,
    wildcard_evidence: nose_il::EvidenceId,
) {
    for exported in [
        "CompletableFuture",
        "CompletionStage",
        "Future",
        "ScheduledFuture",
        "Executor",
        "ExecutorService",
        "ScheduledExecutorService",
    ] {
        let import_evidence = crate::lower::import_binding_evidence_only_with_dependencies(
            lo,
            span,
            exported,
            "java.util.concurrent",
            exported,
            vec![wildcard_evidence],
        );
        record_java_type_domain_alias(lo, "java.util.concurrent", exported, import_evidence);
    }
}

/// `class`/`interface`/`enum` → a `Class` unit; its methods become units too.
pub(super) fn lower_type(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for c in Lowering::named_children(body) {
            let id = match c.kind() {
                "enum_constant" => Some(lower_expr(lo, c)),
                _ => lower_item(lo, c),
            };
            if let Some(id) = id {
                kids.push(id);
            }
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit_with_origin(block, UnitKind::Class, name, java_type_origin(node));
    block
}
pub(super) fn lower_method(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        for p in Lowering::named_children(params) {
            let pspan = lo.span(p);
            let sym = p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
            if let Some(domain) =
                lo.type_domain_from_text_with_dependencies(java_param_type_text(lo, p))
            {
                lo.record_param_domain_resolution(pspan, domain);
            }
            kids.push(lo.add(
                NodeKind::Param,
                sym.map(Payload::Name).unwrap_or(Payload::None),
                pspan,
                &[],
            ));
        }
    }
    let body_node = node.child_by_field_name("body");
    let body = body_node
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit_with_origin(
        func,
        UnitKind::Method,
        name,
        java_method_origin(node, body_node.is_some()),
    );
    func
}

fn java_param_type_text<'a>(lo: &'a Lowering<'a>, param: TsNode) -> &'a str {
    let text = lo.text(param);
    let Some(name_node) = param.child_by_field_name("name") else {
        return text;
    };
    let name = lo.text(name_node);
    text.rsplit_once(name)
        .map(|(ty, _)| ty.trim())
        .filter(|ty| !ty.is_empty())
        .unwrap_or(text)
}
pub(super) fn lower_compact_constructor(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body_node = node.child_by_field_name("body").or_else(|| {
        Lowering::named_children(node)
            .into_iter()
            .find(|child| child.kind() == "block" || child.kind() == "constructor_body")
    });
    let body = body_node
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    let func = lo.add(NodeKind::Func, Payload::None, span, &[body]);
    lo.push_unit_with_origin(
        func,
        UnitKind::Method,
        None,
        crate::lower::imperative_callable_origin(UnitSubkind::Constructor, true),
    );
    func
}
pub(super) fn java_type_origin(node: TsNode) -> UnitOrigin {
    let has_body = java_node_has_method_body(node);
    match node.kind() {
        "interface_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
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
            UnitEvidenceFlag::InterfaceDefaultMethod
        } else {
            UnitEvidenceFlag::DeclarationOnly
        })
        .with_evidence(UnitEvidenceFlag::TypeOnly),
        // An annotation type (`@interface`) is a declaration-only type contract, not an
        // implementation-inheritance candidate — it must not read as `extract-base-class`.
        "annotation_type_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
            UnitSubkind::DefinedType,
            UnitBodyKind::DeclarationOnly,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::DeclarationOnly)
        .with_evidence(UnitEvidenceFlag::TypeOnly),
        // A `record` is a data/struct contract (its canonical body is the component header);
        // it gains an implementation facet only when it carries real method bodies.
        "record_declaration" => UnitOrigin::new(
            if has_body {
                UnitDomains::of(UnitDomain::TypeContract)
                    .with(UnitDomain::Data)
                    .with(UnitDomain::ImplementationType)
            } else {
                UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::Data)
            },
            UnitSubkind::StructRecord,
            if has_body {
                UnitBodyKind::Mixed
            } else {
                UnitBodyKind::DeclarativeDenotation
            },
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::RecordHeader)
        .with_evidence(if has_body {
            UnitEvidenceFlag::HasReusableBody
        } else {
            UnitEvidenceFlag::DataShapeOnly
        }),
        "enum_declaration" => UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::Data),
            UnitSubkind::Enum,
            if has_body {
                UnitBodyKind::Mixed
            } else {
                UnitBodyKind::DeclarativeDenotation
            },
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_domain(if has_body {
            UnitDomain::ImplementationType
        } else {
            UnitDomain::Unknown
        })
        .with_evidence(if has_body {
            UnitEvidenceFlag::HasReusableBody
        } else {
            UnitEvidenceFlag::DataShapeOnly
        }),
        // class_declaration — the only remaining type construct routed here.
        _ => UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::Class,
            if has_body {
                UnitBodyKind::Implementation
            } else {
                UnitBodyKind::DeclarationOnly
            },
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        )
        .with_evidence(if has_body {
            UnitEvidenceFlag::HasReusableBody
        } else {
            UnitEvidenceFlag::DeclarationOnly
        }),
    }
}
pub(super) fn java_method_origin(node: TsNode, has_body: bool) -> UnitOrigin {
    if !has_body {
        return UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
            UnitSubkind::FunctionPrototype,
            UnitBodyKind::DeclarationOnly,
            SourceGranularity::Member,
            RegionKind::Code,
        )
        .with_evidence(UnitEvidenceFlag::DeclarationOnly)
        .with_evidence(UnitEvidenceFlag::TypeOnly);
    }
    crate::lower::imperative_callable_origin(
        if node.kind() == "constructor_declaration" {
            UnitSubkind::Constructor
        } else {
            UnitSubkind::Method
        },
        true,
    )
}
pub(super) fn java_node_has_method_body(node: TsNode) -> bool {
    Lowering::named_children(node).into_iter().any(|child| {
        if java_is_nested_type_decl(child.kind()) {
            return false;
        }
        matches!(
            child.kind(),
            "method_declaration" | "constructor_declaration"
        ) && child.child_by_field_name("body").is_some()
            || java_node_has_method_body(child)
    })
}
pub(super) fn java_is_nested_type_decl(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
    )
}
pub(super) fn lower_field(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let type_domain = java_receiver_declaration_domain(lo, node);
    // a field_declaration has one or more variable_declarators
    let mut assigns = Vec::new();
    for d in Lowering::named_children(node) {
        if d.kind() != "variable_declarator" {
            continue;
        }
        let dspan = lo.span(d);
        let lhs = d
            .child_by_field_name("name")
            .map(|n| {
                let local = lo.text(n);
                if let Some(domain) = type_domain.clone() {
                    lo.record_binding_domain_resolution(dspan, local, domain);
                }
                lo.var(local, dspan)
            })
            .unwrap_or_else(|| lo.empty_block(dspan));
        let rhs = d
            .child_by_field_name("value")
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), dspan, &[]));
        assigns.push(lo.add(NodeKind::Assign, Payload::None, dspan, &[lhs, rhs]));
    }
    if assigns.len() == 1 {
        assigns.pop().unwrap()
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

pub(super) fn java_receiver_declaration_domain(
    lo: &Lowering,
    node: TsNode,
) -> Option<crate::type_domain_aliases::ResolvedTypeDomain> {
    let type_node = node.child_by_field_name("type")?;
    let domain = lo.type_domain_from_text_with_dependencies(lo.text(type_node))?;
    java_runtime_receiver_domain_supported(domain.domain).then_some(domain)
}

pub(super) fn java_runtime_receiver_domain_supported(domain: nose_il::DomainEvidence) -> bool {
    match domain {
        nose_il::DomainEvidence::FutureLike => true,
        nose_il::DomainEvidence::Nominal { type_hash } => [
            "java.util.concurrent.Executor",
            "java.util.concurrent.ExecutorService",
            "java.util.concurrent.ScheduledExecutorService",
        ]
        .into_iter()
        .any(|name| type_hash == stable_symbol_hash(name)),
        _ => false,
    }
}
