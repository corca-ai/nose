use super::*;

struct JavaConstructorOccurrence<'a> {
    actual: &'a str,
    callee_node: Option<NodeId>,
    callee_span: Span,
    call_span: Span,
}

pub(in crate::library_api) fn java_constructor_dependencies_match(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_node: NodeId,
    call_span: Span,
    type_ref: JavaTypeReference,
) -> bool {
    let Some(actual) = node_name(il, interner, callee_node) else {
        return false;
    };
    java_constructor_dependencies_match_for_name(
        il,
        interner,
        record,
        JavaConstructorOccurrence {
            actual,
            callee_node: Some(callee_node),
            callee_span: il.node(callee_node).span,
            call_span,
        },
        type_ref,
    )
}

pub(in crate::library_api) fn java_constructor_dependencies_match_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_span: Span,
    call_span: Span,
    type_ref: JavaTypeReference,
) -> bool {
    let Some(callee_node) = node_at_span_with_kind(il, callee_span, NodeKind::Var) else {
        return false;
    };
    java_constructor_dependencies_match(il, interner, record, callee_node, call_span, type_ref)
}

fn java_constructor_dependencies_match_for_name(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    occurrence: JavaConstructorOccurrence<'_>,
    type_ref: JavaTypeReference,
) -> bool {
    if type_ref.matches_qualified_name(occurrence.actual) {
        return true;
    }
    if occurrence.actual != type_ref.simple_type || !type_ref.simple_name_is_allowed() {
        return false;
    }
    if type_ref.simple_name_rejects_local_shadow()
        && unit_defines_hash_visible_at(
            il,
            interner,
            stable_symbol_hash(type_ref.simple_type),
            occurrence.callee_span,
        )
    {
        return false;
    }
    if !type_ref.simple_name_requires_import() {
        return true;
    }
    let explicit_import = occurrence.callee_node.is_some_and(|node| {
        dependency_has_imported_binding_node(
            il,
            interner,
            record,
            node,
            type_ref.module,
            type_ref.simple_type,
        )
    });
    explicit_import
        || dependency_has_java_wildcard_import_before(
            il,
            interner,
            record,
            type_ref.module,
            type_ref.simple_type,
            occurrence.call_span,
        )
}

pub(in crate::library_api) fn dependency_has_java_wildcard_import_before(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    simple_type: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == expected
            && matches!(
                dependency.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == call_span.file && span.end_byte <= call_span.start_byte
            )
            && !java_explicit_import_conflicts(il, interner, module, simple_type)
    })
}

pub(in crate::library_api) fn java_explicit_import_conflicts(
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
    il.evidence_binding_anchored(local_hash).any(|record| {
        matches!(record.kind, EvidenceKind::Symbol(actual) if actual != expected)
            && record.status == EvidenceStatus::Asserted
    })
}
