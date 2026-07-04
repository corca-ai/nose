use super::*;

pub(in crate::library_api) fn dependency_ids_are_present(
    record: &EvidenceRecord,
    dependencies: &[EvidenceId],
) -> bool {
    dependencies
        .iter()
        .all(|dependency| record.dependencies.contains(dependency))
}

pub(in crate::library_api) fn dependency_has_source_call(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceCallKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    let kind = EvidenceKind::Source(SourceFactKind::Call(expected));
    matches!(
        unique_evidence_at(
            il,
            anchor.span(),
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
                _ => None,
            },
        ),
        EvidenceResolution::Found(call) if call == expected
    ) && dependency_has_asserted_record(il, record, anchor, kind)
}

pub(in crate::library_api) fn dependency_has_source_fact_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceFactKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    matches!(
        unique_evidence_at(
            il,
            anchor.span(),
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(fact) => Some(fact),
                _ => None,
            },
        ),
        EvidenceResolution::Found(fact) if fact == expected
    ) && dependency_has_asserted_record(il, record, anchor, EvidenceKind::Source(expected))
}

pub(in crate::library_api) fn dependency_has_required_module_before(
    record: &EvidenceRecord,
    il: &Il,
    interner: &Interner,
    module: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Require {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|id| {
        il.evidence.get(id.0 as usize).is_some_and(|dependency| {
            dependency.id == *id
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == expected
                && require_dependency_is_before_call(dependency, call_span)
                && require_dependency_has_unshadowed_require(il, interner, dependency)
        })
    })
}

pub(in crate::library_api) fn require_dependency_is_before_call(
    require_record: &EvidenceRecord,
    call_span: Span,
) -> bool {
    matches!(
        require_record.anchor,
        EvidenceAnchor::SourceSpan(span)
            if span.file == call_span.file && span.end_byte <= call_span.start_byte
    )
}

pub(in crate::library_api) fn require_dependency_has_unshadowed_require(
    il: &Il,
    interner: &Interner,
    require_record: &EvidenceRecord,
) -> bool {
    let require_span = match require_record.anchor {
        EvidenceAnchor::SourceSpan(span) => span,
        _ => return false,
    };
    require_record.dependencies.iter().any(|id| {
        let Some(dependency) = il.evidence.get(id.0 as usize) else {
            return false;
        };
        let expected = SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        dependency.id == *id
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && symbol_record_has_admitted_provenance(il, dependency)
            && il.evidence_dependencies_asserted(dependency)
            && span.file == require_span.file
            && span.start_byte >= require_span.start_byte
            && span.end_byte <= require_span.end_byte
            && !file_defines_name_visible_at(il, interner, "require", span)
            && matches!(
                language_core_symbol_identity_at_anchor_matches(il, span, NodeKind::Var, expected),
                EvidenceResolution::Found(true)
            )
    })
}

pub(in crate::library_api) fn dependency_has_unshadowed_global_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: &str,
) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    dependency_has_unshadowed_global_anchor(il, record, span, kind, expected)
}

pub(in crate::library_api) fn dependency_has_unshadowed_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let expected_kind = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(expected),
    };
    if !matches!(
        language_core_symbol_identity_at_anchor_matches(il, span, kind, expected_kind),
        EvidenceResolution::Found(true)
    ) {
        return false;
    }
    dependency_has_admitted_symbol_record(
        il,
        record,
        EvidenceAnchor::node(span, kind),
        expected_kind,
    )
}

pub(in crate::library_api) fn dependency_has_qualified_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, expected) else {
        return false;
    };
    let anchor = EvidenceAnchor::node(span, kind);
    if !matches!(
        qualified_global_symbol_at_evidence_anchor(il, anchor, contract),
        EvidenceResolution::Found(())
    ) {
        return false;
    }
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && qualified_global_symbol_record_valid(il, dependency, contract)
        })
    })
}

pub(in crate::library_api) fn dependency_has_imported_binding_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    dependency_has_imported_binding_anchor(
        il,
        interner,
        record,
        il.node(node).span,
        il.kind(node),
        module,
        exported,
    )
}

pub(in crate::library_api) fn dependency_has_imported_binding_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

pub(in crate::library_api) fn dependency_has_imported_namespace_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

pub(in crate::library_api) fn dependency_has_imported_binding_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

pub(in crate::library_api) fn dependency_has_imported_namespace_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

pub(in crate::library_api) fn dependency_has_imported_symbol_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    let requires_admitted_record = matches!(expected, SymbolEvidenceKind::ImportedNamespace { .. });
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        let dependency_anchor_matches = !requires_admitted_record
            || matches!(
                language_core_symbol_identity_at_anchor_matches(il, span, NodeKind::Var, expected),
                EvidenceResolution::Found(true)
            );
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && (!requires_admitted_record || symbol_record_has_admitted_provenance(il, dependency))
            && (!requires_admitted_record || il.evidence_dependencies_asserted(dependency))
            && dependency_anchor_matches
            && imported_occurrence_symbol_dependencies_valid(il, interner, dependency, expected)
    })
}

pub(in crate::library_api) fn dependency_has_imported_symbol_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: SymbolEvidenceKind,
) -> bool {
    if kind != NodeKind::Var {
        return false;
    }
    let anchor = EvidenceAnchor::node(span, kind);
    let requires_admitted_record = matches!(expected, SymbolEvidenceKind::ImportedNamespace { .. });
    if requires_admitted_record {
        if !matches!(
            language_core_symbol_identity_at_anchor_matches(il, span, kind, expected),
            EvidenceResolution::Found(true)
        ) {
            return false;
        }
    } else if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected
    ) {
        return false;
    }
    let Some(symbol_record) = record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.anchor == anchor
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && (!requires_admitted_record || symbol_record_has_admitted_provenance(il, dependency))
            && (!requires_admitted_record || il.evidence_dependencies_asserted(dependency)))
        .then_some(dependency)
    }) else {
        return false;
    };
    imported_occurrence_symbol_dependencies_valid(il, interner, symbol_record, expected)
}

fn dependency_has_admitted_symbol_record(
    il: &Il,
    record: &EvidenceRecord,
    anchor: EvidenceAnchor,
    symbol: SymbolEvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == EvidenceKind::Symbol(symbol)
                && symbol_record_has_admitted_provenance(il, dependency)
                && il.evidence_dependencies_asserted(dependency)
        })
    })
}

pub(in crate::library_api) fn dependency_has_asserted_record(
    il: &Il,
    record: &EvidenceRecord,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == kind
        })
    })
}
