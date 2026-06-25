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
) {
    if let Some(domain) = result_domain {
        let _ = post_lower_find_or_push_evidence(
            il,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::Domain(domain),
            "library_api_result_domain",
            vec![api],
        );
    }
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
