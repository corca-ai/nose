use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceKind, EvidenceStatus, Interner,
    NodeId, NodeKind, SymbolEvidenceKind,
};

pub(super) fn rust_imported_binding_evidence_only_symbol(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    let Some(local_name) = super::super::super::node_exact_name(il, interner, callee) else {
        return false;
    };
    rust_imported_binding_evidence_only_symbol_for_local(il, local_name, callee, module, exported)
}

pub(super) fn rust_imported_binding_evidence_only_symbol_for_local(
    il: &nose_il::Il,
    local_name: &str,
    occurrence: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let local_hash = stable_symbol_hash(local_name);
    let module_hash = stable_symbol_hash(module);
    let exported_hash = stable_symbol_hash(exported);
    let occurrence_span = il.node(occurrence).span;
    let mut found_matching = false;
    for record in il.evidence_binding_anchored(local_hash) {
        let EvidenceAnchor::Binding {
            span,
            local_hash: anchor_hash,
        } = record.anchor
        else {
            continue;
        };
        if span.file != occurrence_span.file || anchor_hash != local_hash {
            continue;
        }
        if !rust_import_binding_span_visible_at_call(il, span, occurrence_span) {
            continue;
        }
        let EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: actual_module,
            exported_hash: actual_exported,
        }) = record.kind
        else {
            continue;
        };
        if record.provenance.emitter != EvidenceEmitter::Builtin
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            return false;
        }
        if actual_module == module_hash && actual_exported == exported_hash {
            found_matching = true;
        } else {
            return false;
        }
    }
    found_matching
}

fn rust_import_binding_span_visible_at_call(
    il: &nose_il::Il,
    binding_span: nose_il::Span,
    occurrence_span: nose_il::Span,
) -> bool {
    if binding_span.file != occurrence_span.file {
        return false;
    }
    if il.span_inside_local_scope(binding_span) {
        return false;
    }
    il.nearest_module_scope_containing_span(binding_span)
        == il.nearest_module_scope_containing_span(occurrence_span)
}
