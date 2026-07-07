use super::*;

// Post-lower Library API recognition lives in lower/library_api_post_lower.rs.

pub(super) fn post_lower_var_name<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        _ => None,
    }
}

pub(super) fn post_lower_unshadowed_symbol_evidence_id(
    il: &mut Il,
    node: NodeId,
    expected: &str,
) -> Option<EvidenceId> {
    let span = il.node(node).span;
    let anchor = EvidenceAnchor::node(span, NodeKind::Var);
    let kind = EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(expected),
    });
    post_lower_find_or_push_evidence(
        il,
        anchor,
        kind,
        "symbol_unshadowed_global_post_lower",
        vec![],
    )
}

pub(super) fn post_lower_qualified_global_symbol_evidence_id(
    il: &Il,
    node: NodeId,
    path: &str,
) -> Option<EvidenceId> {
    il.evidence.iter().find_map(|record| {
        (record.anchor == EvidenceAnchor::node(il.node(node).span, il.kind(node))
            && record.kind
                == EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                    path_hash: stable_symbol_hash(path),
                })
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

pub(super) fn post_lower_static_global_call_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    call: NodeId,
) -> Option<(NodeId, NodeId, &'a str, &'a str, usize)> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (&callee, args) = il.children(call).split_first()?;
    let (receiver_node, receiver, method) =
        post_lower_static_global_callee_parts(il, interner, callee)?;
    Some((callee, receiver_node, receiver, method, args.len()))
}

pub(super) fn post_lower_static_global_callee_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    callee: NodeId,
) -> Option<(NodeId, &'a str, &'a str)> {
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    let receiver_node = il.children(callee).first().copied()?;
    let receiver = post_lower_var_name(il, interner, receiver_node)?;
    Some((receiver_node, receiver, interner.resolve(method)))
}

pub(super) fn post_lower_imported_binding_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
    exported: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    let dependency = post_lower_binding_symbol_evidence_id(il, interner, node, expected)?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        "symbol_imported_binding_occurrence_post_lower",
        vec![dependency],
    )
}

pub(super) fn post_lower_imported_namespace_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    let dependency = post_lower_binding_symbol_evidence_id(il, interner, node, expected)?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        "symbol_imported_namespace_occurrence_post_lower",
        vec![dependency],
    )
}

pub(super) fn post_lower_binding_symbol_evidence_id(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(local) = il.node(node).payload else {
        return None;
    };
    let local_hash = stable_symbol_hash(interner.resolve(local));
    il.evidence.iter().find_map(|record| {
        (matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && record.kind == EvidenceKind::Symbol(expected)
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

pub(super) fn post_lower_java_wildcard_import_evidence_id(
    il: &Il,
    interner: &Interner,
    module: &str,
    simple_type: &str,
    use_span: Span,
) -> Option<EvidenceId> {
    if post_lower_explicit_import_conflicts(il, interner, module, simple_type) {
        return None;
    }
    let kind = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(module),
    });
    il.evidence.iter().find_map(|record| {
        (record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && matches!(
                record.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == use_span.file && span.end_byte <= use_span.start_byte
            ))
        .then_some(record.id)
    })
}

pub(super) fn post_lower_explicit_import_conflicts(
    il: &Il,
    _interner: &Interner,
    module: &str,
    simple_type: &str,
) -> bool {
    let local_hash = stable_symbol_hash(simple_type);
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(simple_type),
    };
    il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && record.status == EvidenceStatus::Asserted
            && matches!(record.kind, EvidenceKind::Symbol(actual) if actual != expected)
    })
}

pub(super) fn post_lower_required_module_evidence_id(
    il: &mut Il,
    interner: &Interner,
    module: &str,
    use_span: Span,
) -> Option<EvidenceId> {
    if il.meta.lang != Lang::Ruby {
        return None;
    }
    let module_hash = stable_symbol_hash(module);
    let (require_call, require_callee) =
        post_lower_top_level_statements(il)
            .into_iter()
            .find_map(|stmt| {
                let expr = if il.kind(stmt) == NodeKind::ExprStmt {
                    il.children(stmt).first().copied()
                } else {
                    Some(stmt)
                }?;
                let callee =
                    post_lower_require_call_callee_if_matches(il, interner, expr, module_hash)?;
                let require_span = il.node(expr).span;
                (require_span.file == use_span.file && require_span.end_byte <= use_span.start_byte)
                    .then_some((expr, callee))
            })?;
    if post_lower_file_defines_name_visible_at(
        il,
        interner,
        "require",
        il.node(require_callee).span,
    ) {
        return None;
    }
    let require_dependency =
        post_lower_unshadowed_symbol_evidence_id(il, require_callee, "require")?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::source_span(il.node(require_call).span),
        EvidenceKind::Import(ImportEvidenceKind::Require { module_hash }),
        "ruby_require_module",
        vec![require_dependency],
    )
}

pub(super) fn post_lower_require_call_callee_if_matches(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    module_hash: u64,
) -> Option<NodeId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let kids = il.children(call);
    if kids.len() != 2 {
        return None;
    }
    (matches!(post_lower_var_name(il, interner, kids[0]), Some("require"))
        && matches!(il.node(kids[1]).payload, Payload::LitStr(hash) if hash == module_hash))
    .then_some(kids[0])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn post_lower_library_api_evidence_with_pack_id(
    il: &mut Il,
    call: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    pack_id: &str,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        pack_id,
        rule,
        dependencies,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn post_lower_library_api_node_evidence_with_pack_id(
    il: &mut Il,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    pack_id: &str,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        pack_id,
        rule,
        dependencies,
    )
}

pub(super) fn post_lower_record_library_api_result_domain(
    il: &mut Il,
    call: NodeId,
    result_domain: Option<DomainEvidence>,
    api: EvidenceId,
) -> Option<EvidenceId> {
    if let Some(domain) = result_domain {
        return post_lower_find_or_push_evidence(
            il,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::Domain(domain),
            "library_api_result_domain",
            vec![api],
        );
    }
    None
}

pub(super) fn post_lower_record_library_api_node_result_domain(
    il: &mut Il,
    node: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    let _ = post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Domain(domain),
        "library_api_result_domain",
        vec![api],
    );
}

pub(super) fn post_lower_sequence_surface_evidence_id(
    il: &Il,
    node: NodeId,
    surface: SequenceSurfaceKind,
) -> Option<EvidenceId> {
    let span = il.node(node).span;
    il.evidence.iter().find_map(|record| {
        (record.anchor == EvidenceAnchor::sequence(span)
            && record.kind == EvidenceKind::SequenceSurface(surface)
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

pub(super) fn record_post_lower_bound_order_guard_evidence(il: &mut Il, interner: &Interner) {
    let comparisons: Vec<NodeId> = (0..il.nodes.len() as u32)
        .map(NodeId)
        .filter(|&node| il.kind(node) == NodeKind::BinOp)
        .collect();
    for node in comparisons {
        for activation in [
            BoundOrderGuardActivation::WhenTrue,
            BoundOrderGuardActivation::WhenFalse,
        ] {
            let Some((lower, upper)) =
                post_lower_bound_order_operands_from_condition(il, node, activation)
            else {
                continue;
            };
            let Some(mut dependencies) =
                post_lower_bound_order_operand_dependencies(il, interner, lower)
            else {
                continue;
            };
            let Some(upper_dependencies) =
                post_lower_bound_order_operand_dependencies(il, interner, upper)
            else {
                continue;
            };
            dependencies.extend(upper_dependencies);
            dependencies.sort_unstable_by_key(|id| id.0);
            dependencies.dedup();
            let _ = post_lower_find_or_push_evidence(
                il,
                EvidenceAnchor::node(il.node(node).span, NodeKind::BinOp),
                EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
                    lower_span: il.node(lower).span,
                    upper_span: il.node(upper).span,
                    activation,
                }),
                "bound_order_guard_post_lower",
                dependencies,
            );
        }
    }
}

fn post_lower_bound_order_operands_from_condition(
    il: &Il,
    node: NodeId,
    activation: BoundOrderGuardActivation,
) -> Option<(NodeId, NodeId)> {
    let Payload::Op(op) = il.node(node).payload else {
        return None;
    };
    let [left, right] = il.children(node) else {
        return None;
    };
    match (activation, op) {
        (BoundOrderGuardActivation::WhenTrue, Op::Lt | Op::Le)
        | (BoundOrderGuardActivation::WhenFalse, Op::Gt | Op::Ge) => Some((*left, *right)),
        (BoundOrderGuardActivation::WhenTrue, Op::Gt | Op::Ge)
        | (BoundOrderGuardActivation::WhenFalse, Op::Lt | Op::Le) => Some((*right, *left)),
        _ => None,
    }
}

fn post_lower_bound_order_operand_dependencies(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<Vec<EvidenceId>> {
    match (il.kind(node), il.node(node).payload) {
        (NodeKind::Lit, Payload::LitInt(_)) => Some(Vec::new()),
        (NodeKind::Var, Payload::Cid(cid)) => {
            Some(vec![post_lower_integer_param_domain_dependency(
                il, interner, node, cid,
            )?])
        }
        (NodeKind::Var, Payload::Name(name)) => {
            Some(vec![post_lower_named_integer_param_domain_dependency(
                il, interner, node, name,
            )?])
        }
        _ => None,
    }
}

fn post_lower_integer_param_domain_dependency(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
    cid: u32,
) -> Option<EvidenceId> {
    if !receiver_satisfies_domain(il, interner, reference, DomainRequirement::INTEGER) {
        return None;
    }
    let scope = il.nearest_scope(reference)?;
    let mut found = None;
    for &param in il.children(scope) {
        if il.kind(param) != NodeKind::Param {
            continue;
        }
        if !matches!(il.node(param).payload, Payload::Cid(param_cid) if param_cid == cid) {
            continue;
        }
        let evidence = post_lower_integer_param_domain_evidence_id(il, il.node(param).span)?;
        match found {
            None => found = Some(evidence),
            Some(existing) if existing == evidence => {}
            Some(_) => return None,
        }
    }
    found
}

fn post_lower_named_integer_param_domain_dependency(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
    name: Symbol,
) -> Option<EvidenceId> {
    if !receiver_satisfies_domain(il, interner, reference, DomainRequirement::INTEGER) {
        return None;
    }
    let scope = il.nearest_scope(reference)?;
    let name_text = interner.resolve(name);
    let mut found = None;
    for &param in il.children(scope) {
        if il.kind(param) != NodeKind::Param {
            continue;
        }
        let same_param = match il.node(param).payload {
            Payload::Name(param_name) => interner.resolve(param_name) == name_text,
            Payload::Cid(param_cid) => il
                .cid_names
                .get(param_cid as usize)
                .is_some_and(|param_name| interner.resolve(*param_name) == name_text),
            _ => false,
        };
        if !same_param {
            continue;
        }
        let evidence = post_lower_integer_param_domain_evidence_id(il, il.node(param).span)?;
        match found {
            None => found = Some(evidence),
            Some(existing) if existing == evidence => {}
            Some(_) => return None,
        }
    }
    found
}

fn post_lower_integer_param_domain_evidence_id(il: &Il, span: Span) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::param(span);
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if domain != DomainEvidence::Integer
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            return None;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

pub(super) fn post_lower_find_or_push_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> Option<EvidenceId> {
    let _ = rule;
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    Some(il.find_or_push_builtin_evidence(anchor, kind, pack_id, producer_id, dependencies))
}

pub(super) fn post_lower_top_level_statements(il: &Il) -> Vec<NodeId> {
    let Some(root) = il.nodes.get(il.root.0 as usize) else {
        return Vec::new();
    };
    if root.kind != NodeKind::Module {
        return il.children(il.root).to_vec();
    }
    il.children(il.root).to_vec()
}

pub(super) fn post_lower_source_call_evidence_id(
    il: &Il,
    node: NodeId,
    call: SourceCallKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::source_span(il.node(node).span);
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::Source(SourceFactKind::Call(call))
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

pub(super) fn post_lower_has_python_wildcard_import_evidence(il: &Il) -> bool {
    il.evidence.iter().any(|record| {
        record.status == EvidenceStatus::Asserted
            && matches!(
                record.kind,
                EvidenceKind::Import(ImportEvidenceKind::Wildcard { .. })
            )
    })
}

pub(super) fn post_lower_file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    nose_semantics::file_defines_name_visible_at(il, interner, name, occurrence_span)
}

pub(super) fn post_lower_unit_defines_name(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    let name_hash = stable_symbol_hash(name);
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}
