use super::*;

pub(super) fn collect_call_nodes(il: &Il, node: NodeId, out: &mut Vec<NodeId>) {
    if il.kind(node) == NodeKind::Call {
        out.push(node);
    }
    for &child in il.children(node) {
        collect_call_nodes(il, child, out);
    }
}

pub(super) fn record_imported_call_target(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    provenance: CallTargetEvidenceProvenance,
    cache: &mut ImportedOccurrenceValidationCache,
) {
    if il.kind(call) != NodeKind::Call || !matches!(il.node(call).payload, Payload::None) {
        return;
    }
    let Some(&callee) = il.children(call).first() else {
        return;
    };
    match il.kind(callee) {
        NodeKind::Var => {
            if let Some(target) = imported_function_target(il, interner, callee, provenance, cache)
            {
                upsert(
                    il,
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedFunction {
                        module_hash: target.module_hash,
                        exported_hash: target.exported_hash,
                        local_hash: target.local_hash,
                    }),
                    IMPORTED_FUNCTION_RULE,
                    provenance,
                    vec![target.dependency],
                );
            }
            if let Some(target) =
                imported_scoped_member_target(il, interner, callee, provenance, cache)
            {
                upsert(
                    il,
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedMember {
                        module_hash: target.module_hash,
                        exported_hash: target.exported_hash,
                        member_hash: target.member_hash,
                    }),
                    IMPORTED_MEMBER_RULE,
                    provenance,
                    vec![target.dependency],
                );
            }
        }
        NodeKind::Field => {
            if let Some(target) = imported_member_target(il, interner, callee, provenance, cache) {
                upsert(
                    il,
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedMember {
                        module_hash: target.module_hash,
                        exported_hash: target.exported_hash,
                        member_hash: target.member_hash,
                    }),
                    IMPORTED_MEMBER_RULE,
                    provenance,
                    vec![target.dependency],
                );
            }
        }
        _ => {}
    }
}

fn imported_function_target(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    provenance: CallTargetEvidenceProvenance,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<ImportedFunctionTarget> {
    let local_hash = node_name_hash(il, interner, callee)?;
    let (symbol, binding_dependency) =
        unique_binding_symbol_for_var(il, interner, callee, ImportedBindingUse::FunctionCallee)?;
    let SymbolEvidenceKind::ImportedBinding {
        module_hash,
        exported_hash,
    } = symbol
    else {
        return None;
    };
    let dependency = upsert_valid_imported_symbol_occurrence(
        il,
        interner,
        callee,
        symbol,
        binding_dependency,
        provenance,
        cache,
    )?;
    Some(ImportedFunctionTarget {
        module_hash,
        exported_hash,
        local_hash,
        dependency,
    })
}

fn imported_scoped_member_target(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    provenance: CallTargetEvidenceProvenance,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<ImportedMemberTarget> {
    let (root, suffix) = scoped_var_root_and_suffix(il, interner, callee)?;
    let root_hash = stable_symbol_hash(root);
    let (symbol, binding_dependency) =
        unique_binding_symbol_for_hash(il, root_hash, ImportedBindingUse::MemberReceiver)?;
    let dependency = upsert_valid_imported_symbol_occurrence(
        il,
        interner,
        callee,
        symbol,
        binding_dependency,
        provenance,
        cache,
    )?;
    let member_hash = stable_symbol_hash(suffix);
    match symbol {
        SymbolEvidenceKind::ImportedBinding {
            module_hash,
            exported_hash,
        } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash,
            member_hash,
            dependency,
        }),
        SymbolEvidenceKind::ImportedNamespace { module_hash } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash: member_hash,
            member_hash,
            dependency,
        }),
        _ => None,
    }
}

fn imported_member_target(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    provenance: CallTargetEvidenceProvenance,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<ImportedMemberTarget> {
    let Payload::Name(member) = il.node(callee).payload else {
        return None;
    };
    let member_hash = interner.symbol_hash(member);
    let receiver = *il.children(callee).first()?;
    if il.kind(receiver) != NodeKind::Var {
        return None;
    }
    let (symbol, binding_dependency) =
        unique_binding_symbol_for_var(il, interner, receiver, ImportedBindingUse::MemberReceiver)?;
    let dependency = upsert_valid_imported_symbol_occurrence(
        il,
        interner,
        receiver,
        symbol,
        binding_dependency,
        provenance,
        cache,
    )?;
    match symbol {
        SymbolEvidenceKind::ImportedBinding {
            module_hash,
            exported_hash,
        } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash,
            member_hash,
            dependency,
        }),
        SymbolEvidenceKind::ImportedNamespace { module_hash } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash: member_hash,
            member_hash,
            dependency,
        }),
        _ => None,
    }
}

fn unique_binding_symbol_for_var(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    imported_use: ImportedBindingUse,
) -> Option<(SymbolEvidenceKind, EvidenceId)> {
    let local_hash = node_name_hash(il, interner, node)?;
    unique_binding_symbol_for_hash(il, local_hash, imported_use)
}

fn unique_binding_symbol_for_hash(
    il: &Il,
    local_hash: u64,
    imported_use: ImportedBindingUse,
) -> Option<(SymbolEvidenceKind, EvidenceId)> {
    let mut found = None;
    for record in il.evidence_binding_anchored(local_hash) {
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return None;
        }
        if !imported_symbol_allowed_for_use(symbol, imported_use) {
            return None;
        }
        match found {
            None => found = Some((symbol, record.id)),
            Some((existing, _)) if existing == symbol => {}
            Some(_) => return None,
        }
    }
    found
}

fn scoped_var_root_and_suffix<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<(&'a str, &'a str)> {
    if il.meta.lang != nose_il::Lang::Rust || il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(symbol) = il.node(node).payload else {
        return None;
    };
    let path = interner.resolve(symbol);
    let (root, suffix) = path.split_once("::")?;
    if root.is_empty()
        || suffix.is_empty()
        || suffix.split("::").any(str::is_empty)
        || matches!(root, "crate" | "self" | "super" | "std" | "core" | "alloc")
    {
        return None;
    }
    Some((root, suffix))
}

fn imported_symbol_allowed_for_use(
    symbol: SymbolEvidenceKind,
    imported_use: ImportedBindingUse,
) -> bool {
    match imported_use {
        ImportedBindingUse::FunctionCallee => {
            matches!(symbol, SymbolEvidenceKind::ImportedBinding { .. })
        }
        ImportedBindingUse::MemberReceiver => matches!(
            symbol,
            SymbolEvidenceKind::ImportedBinding { .. }
                | SymbolEvidenceKind::ImportedNamespace { .. }
        ),
    }
}

fn upsert_valid_imported_symbol_occurrence(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    symbol: SymbolEvidenceKind,
    binding_dependency: EvidenceId,
    provenance: CallTargetEvidenceProvenance,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<EvidenceId> {
    if !imported_symbol_occurrence_can_be_upserted(il, interner, node, symbol, cache) {
        return None;
    }
    let rule = match symbol {
        SymbolEvidenceKind::ImportedBinding { .. } => IMPORTED_BINDING_OCCURRENCE_RULE,
        SymbolEvidenceKind::ImportedNamespace { .. } => IMPORTED_NAMESPACE_OCCURRENCE_RULE,
        _ => return None,
    };
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    let kind = EvidenceKind::Symbol(symbol);
    let dependencies = vec![binding_dependency];
    let candidate = EvidenceRecord {
        id: EvidenceId(u32::MAX),
        anchor,
        kind,
        provenance: provenance.current,
        dependencies: dependencies.clone(),
        status: EvidenceStatus::Asserted,
    };
    if !imported_occurrence_symbol_dependencies_valid_with_cache(
        il, interner, &candidate, symbol, cache,
    ) {
        return None;
    }
    let id = upsert(il, anchor, kind, rule, provenance, dependencies);
    Some(id)
}

fn imported_symbol_occurrence_can_be_upserted(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
    cache: &mut ImportedOccurrenceValidationCache,
) -> bool {
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(actual) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || actual != expected
            || !il.evidence_dependencies_asserted(record)
            || !imported_occurrence_symbol_dependencies_valid_with_cache(
                il, interner, record, expected, cache,
            )
        {
            return false;
        }
    }
    true
}

pub(super) fn var_has_symbol_identity_evidence(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    if il
        .evidence_anchored_at(anchor.span())
        .any(|record| record.anchor == anchor && matches!(record.kind, EvidenceKind::Symbol(_)))
    {
        return true;
    }
    let Some(local_hash) = node_name_hash(il, interner, node) else {
        return false;
    };
    il.evidence_binding_anchored(local_hash)
        .any(|record| matches!(record.kind, EvidenceKind::Symbol(_)))
}

fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.symbol_hash(symbol)),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.symbol_hash(symbol)),
        _ => None,
    }
}
