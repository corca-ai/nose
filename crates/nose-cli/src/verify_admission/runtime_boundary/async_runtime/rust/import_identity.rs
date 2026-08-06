use super::super::{node_defines_name, rust_imports};
use super::api_paths::module_root;
use crate::verify_admission::AdmissionContext;
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceKind, EvidenceStatus, Interner,
    NodeId, NodeKind, SymbolEvidenceKind,
};

pub(super) fn imported_runtime_type_visible(
    il: &nose_il::Il,
    interner: &Interner,
    occurrence: NodeId,
    exported: &str,
    context: &AdmissionContext,
) -> bool {
    let module = "tokio::runtime";
    if context.rust_runtime_root_is_local_for_file(module_root(module), &il.meta.path) {
        return false;
    }
    rust_imports::rust_imported_binding_evidence_only_symbol_for_local(
        il, exported, occurrence, module, exported,
    ) && !imported_local_shadowed(il, interner, occurrence, exported, module, exported)
}

pub(super) fn imported_async_spawn_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    imported_runtime_member(il, interner, callee, "tokio", "spawn", context)
        || imported_runtime_member(il, interner, callee, "tokio::task", "spawn", context)
        || imported_runtime_member(
            il,
            interner,
            callee,
            "tokio::task",
            "spawn_blocking",
            context,
        )
        || imported_runtime_member(il, interner, callee, "async_std::task", "spawn", context)
        || imported_runtime_member(
            il,
            interner,
            callee,
            "async_std::task",
            "spawn_blocking",
            context,
        )
}

pub(super) fn imported_async_join_macro_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    imported_runtime_member(il, interner, callee, "tokio", "join", context)
        || imported_runtime_member(il, interner, callee, "tokio", "try_join", context)
        || imported_runtime_member(il, interner, callee, "futures", "join", context)
        || imported_runtime_member(il, interner, callee, "futures", "try_join", context)
        || imported_runtime_member(il, interner, callee, "futures_util", "join", context)
        || imported_runtime_member(il, interner, callee, "futures_util", "try_join", context)
}

pub(super) fn imported_async_select_macro_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &AdmissionContext,
) -> bool {
    imported_runtime_member(il, interner, callee, "tokio", "select", context)
        || imported_runtime_member(il, interner, callee, "futures", "select", context)
        || imported_runtime_member(il, interner, callee, "futures_util", "select", context)
}

pub(super) fn imported_runtime_member(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
    context: &AdmissionContext,
) -> bool {
    if context.rust_runtime_root_is_local_for_file(module_root(module), &il.meta.path) {
        return false;
    }
    (nose_semantics::imported_member_symbol(il, interner, callee, module, exported)
        || rust_imports::rust_imported_binding_evidence_only_symbol(
            il, interner, callee, module, exported,
        ))
        && !imported_member_shadowed(il, interner, callee, module, exported)
}

fn imported_member_shadowed(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let Some(local_name) = super::super::super::super::node_exact_name(il, interner, callee) else {
        return false;
    };
    imported_local_shadowed(il, interner, callee, local_name, module, exported)
}

fn imported_local_shadowed(
    il: &nose_il::Il,
    interner: &Interner,
    occurrence: NodeId,
    local_name: &str,
    module: &str,
    exported: &str,
) -> bool {
    let occurrence_span = il.node(occurrence).span;
    for unit in &il.units {
        if il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == local_name)
        {
            return true;
        }
    }
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span.file != occurrence_span.file {
            continue;
        }
        let node_id = NodeId(idx as u32);
        match node.kind {
            NodeKind::Assign => {
                let Some(lhs) = il.children(node_id).first().copied() else {
                    continue;
                };
                if !node_defines_name(il, interner, lhs, local_name) {
                    continue;
                }
                if !imported_binding_at_span(il, node.span, local_name, module, exported)
                    && super::super::definition_shadows_occurrence(il, node_id, occurrence)
                {
                    return true;
                }
            }
            NodeKind::Block | NodeKind::Module | NodeKind::Param
                if node_defines_name(il, interner, node_id, local_name)
                    && super::super::definition_shadows_occurrence(il, node_id, occurrence) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn imported_binding_at_span(
    il: &nose_il::Il,
    span: nose_il::Span,
    local: &str,
    module: &str,
    exported: &str,
) -> bool {
    let local_hash = stable_symbol_hash(local);
    let module_hash = stable_symbol_hash(module);
    let exported_hash = stable_symbol_hash(exported);
    il.evidence_anchored_at(span).any(|record| {
        record.anchor == EvidenceAnchor::binding(span, local_hash)
            && record.kind
                == EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                    module_hash,
                    exported_hash,
                })
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}
