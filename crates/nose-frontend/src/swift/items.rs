use super::*;

pub(super) fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}
pub(super) fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_declaration" => Some(lower_function(lo, node, false)),
        "protocol_function_declaration" => Some(lower_function(lo, node, true)),
        "init_declaration" | "deinit_declaration" => Some(lower_function(lo, node, true)),
        "subscript_declaration" => {
            // The enclosing extension/type surface can be parser-recovered or
            // comment-split in ways that hide its full body text. A labeled
            // default subscript declaration is itself sufficient to close the
            // controlled stdlib dispatch slice, regardless of target spelling.
            if lo.text(node).contains("default") {
                record_dictionary_default_subscript_proof_barrier(lo, lo.span(node));
            }
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
        "macro_declaration" => {
            let span = lo.span(node);
            record_all_satisfy_dispatch_proof_barrier(lo, span);
            record_nil_literal_proof_barrier(lo, span);
            record_flat_map_dispatch_proof_barrier(lo, span);
            record_dictionary_default_subscript_proof_barrier(lo, span);
            None
        }
        "associatedtype_declaration"
        | "operator_declaration"
        | "precedence_group_declaration"
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
    record_all_satisfy_dispatch_proof_barrier(lo, span);
    record_nil_literal_proof_barrier(lo, span);
    record_flat_map_dispatch_proof_barrier(lo, span);
    record_dictionary_default_subscript_proof_barrier(lo, span);
    record_selective_import_shadow(lo, node);
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
    record_all_satisfy_dispatch_barrier(lo, node);
    record_compact_map_dispatch_barrier(lo, node);
    record_flat_map_dispatch_barrier(lo, node);
    record_nil_literal_conformance(lo, node, &mut kids);
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
    let name = node
        .child_by_field_name("type")
        .and_then(|ty| type_surface_name(lo, ty))
        .map(|name| lo.sym(&name));
    let mut kids = Vec::new();
    record_all_satisfy_dispatch_barrier(lo, node);
    record_compact_map_dispatch_barrier(lo, node);
    record_flat_map_dispatch_barrier(lo, node);
    record_dictionary_default_subscript_extension_barrier(lo, node);
    record_nil_literal_conformance(lo, node, &mut kids);
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
    lo.push_unit_with_origin(block, UnitKind::Class, name, swift_extension_origin(node));
    block
}

pub(super) fn record_typealias_shadow(lo: &mut Lowering, node: TsNode) {
    let span = lo.span(node);
    record_all_satisfy_dispatch_proof_barrier(lo, span);
    record_nil_literal_proof_barrier(lo, span);
    record_flat_map_dispatch_proof_barrier(lo, span);
    // An alias with any name can later be used as a Dictionary extension
    // target, including from another file. Alias resolution is deliberately
    // outside this controlled default-subscript slice.
    record_dictionary_default_subscript_proof_barrier(lo, span);
    if let Some(name) = swift_decl_name(lo, node) {
        // Keep type-only syntax out of the structural tree while preserving the
        // declaration as a same-file shadow for stdlib free-name contracts.
        lo.add(NodeKind::Block, Payload::Name(name), span, &[]);
    }
}
pub(super) fn lower_function(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    let span = lo.span(node);
    let name = swift_decl_name(lo, node);
    let protocol_modifiers = swift_function_protocol_modifiers(node);
    let mut kids = Vec::new();
    let mut previous_was_attribute = false;
    for child in Lowering::named_children(node) {
        match child.kind() {
            "attribute" => previous_was_attribute = true,
            "parameter" => {
                lower_param(lo, child, previous_was_attribute, &mut kids);
                previous_was_attribute = false;
            }
            _ => previous_was_attribute = false,
        }
    }
    let body_node = node.child_by_field_name("body");
    let body = body_node
        .map(|body| {
            let body_span = lo.span(body);
            let body = lower_function_body(lo, body);
            wrap_swift_callable_protocols(
                lo,
                span,
                body_span,
                body,
                protocol_modifiers.is_async,
                protocol_modifiers.is_throwing,
                "throwing_function",
            )
        })
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

struct SwiftFunctionProtocolModifiers {
    is_async: bool,
    is_throwing: bool,
}

fn swift_function_protocol_modifiers(node: TsNode) -> SwiftFunctionProtocolModifiers {
    let mut modifiers = SwiftFunctionProtocolModifiers {
        is_async: false,
        is_throwing: false,
    };
    for index in 0..node.child_count() {
        let Some(child) = node.child(index) else {
            continue;
        };
        match child.kind() {
            "async" => modifiers.is_async = true,
            "throws" | "rethrows" | "throws_clause" => modifiers.is_throwing = true,
            _ => {}
        }
        if modifiers.is_async && modifiers.is_throwing {
            break;
        }
    }
    modifiers
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
        "protocol_declaration" => swift_protocol_origin(node),
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
fn swift_protocol_origin(node: TsNode) -> UnitOrigin {
    let has_body = swift_node_has_reusable_body(node);
    let origin = UnitOrigin::new(
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
        UnitEvidenceFlag::ProtocolRequirement
    })
    .with_evidence(UnitEvidenceFlag::TypeOnly);
    if has_body {
        origin.with_evidence(UnitEvidenceFlag::HasReusableBody)
    } else {
        origin.with_evidence(UnitEvidenceFlag::DeclarationOnly)
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
pub(super) fn lower_param(
    lo: &mut Lowering,
    param: TsNode,
    has_attribute: bool,
    out: &mut Vec<NodeId>,
) {
    let span = lo.span(param);
    let name = parameter_binding_name(param);
    let payload = name
        .filter(|n| lo.text(*n) != "_")
        .map(|n| Payload::Name(lo.sym(lo.text(n))))
        .unwrap_or(Payload::None);
    let type_node = param.child_by_field_name("type");
    let has_parameter_modifiers = Lowering::named_children(param)
        .into_iter()
        .any(|child| child.kind() == "parameter_modifiers");
    let plain_parameter = !has_attribute && !has_parameter_modifiers && !param.has_error();
    let string_evidence = plain_parameter
        .then(|| type_node.and_then(|ty| swift_string_parameter_evidence(lo, param, ty)))
        .flatten();
    if type_node.is_some_and(|ty| ty.kind() == "array_type") && plain_parameter {
        // Swift's bracket type is the one source surface that proves a builtin
        // Array independently of nominal `Array`/`Collection` declarations.
        // Parameter attributes can denote property wrappers that replace the
        // caller's value before the function body observes it. Other parameter
        // modifiers and parser-recovered parameter syntax also stay outside
        // this deliberately plain source proof.
        lo.record_evidence(
            EvidenceAnchor::param(span),
            EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter),
            "swift_bracket_array_parameter",
        );
    }
    if plain_parameter {
        if let Some(kind) = type_node.and_then(|ty| swift_dictionary_parameter_evidence(lo, ty)) {
            lo.record_evidence(
                EvidenceAnchor::param(span),
                EvidenceKind::Type(kind),
                "swift_dictionary_parameter",
            );
        }
        if let Some(kind) = string_evidence {
            lo.record_evidence(
                EvidenceAnchor::param(span),
                EvidenceKind::Type(kind),
                "swift_string_parameter",
            );
        }
    }
    if let Some(domain) = type_node
        .and_then(|ty| lo.type_domain_from_text_with_dependencies(lo.text(ty)))
        .or_else(|| lo.type_domain_from_text_with_dependencies(lo.text(param)))
    {
        if domain.domain != nose_il::DomainEvidence::String || string_evidence.is_some() {
            lo.record_param_domain_resolution(span, domain);
        }
    }
    let shape = if plain_parameter {
        Vec::new()
    } else {
        vec![lo.raw("swift_non_plain_parameter", span, &[])]
    };
    out.push(lo.add(NodeKind::Param, payload, span, &shape));
}

fn swift_dictionary_parameter_evidence(
    lo: &Lowering,
    type_node: TsNode,
) -> Option<TypeEvidenceKind> {
    if type_node.kind() == "dictionary_type" {
        return Some(TypeEvidenceKind::SwiftBracketDictionaryParameter);
    }
    let ty = lo
        .text(type_node)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if ty.starts_with("Dictionary<") && ty.ends_with('>') {
        return Some(TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter);
    }
    if ty.starts_with("Swift.Dictionary<") && ty.ends_with('>') {
        return Some(TypeEvidenceKind::SwiftQualifiedDictionaryParameter);
    }
    None
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
