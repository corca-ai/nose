//! Symbol identity evidence helpers.

use super::*;

pub(super) fn symbol_evidence_at_node_anchor(
    il: &Il,
    span: Span,
    kind: NodeKind,
) -> EvidenceResolution<SymbolEvidenceKind> {
    unique_asserted_evidence_at(
        il,
        span,
        |anchor| {
            matches!(
                anchor,
                EvidenceAnchor::Node {
                    span: anchor_span,
                    kind: anchor_kind,
                } if anchor_span == span && anchor_kind == kind
            )
        },
        |evidence| match evidence {
            EvidenceKind::Symbol(symbol) => Some(symbol),
            _ => None,
        },
    )
}

pub(super) fn symbol_evidence_for_binding(
    il: &Il,
    local_hash: u64,
    span: Span,
) -> EvidenceResolution<SymbolEvidenceKind> {
    unique_evidence_at(
        il,
        span,
        |anchor| {
            matches!(
                anchor,
                EvidenceAnchor::Binding {
                    span: anchor_span,
                    local_hash: anchor_hash,
                } if anchor_hash == local_hash && anchor_span == span
            )
        },
        |evidence| match evidence {
            EvidenceKind::Symbol(symbol) => Some(symbol),
            _ => None,
        },
    )
}

pub(super) fn language_core_symbol_evidence_at_anchor(
    il: &Il,
    span: Span,
    kind: NodeKind,
) -> EvidenceResolution<SymbolEvidenceKind> {
    let anchor = EvidenceAnchor::node(span, kind);
    let mut found = None;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if !symbol_record_has_admitted_provenance(il, record) {
            continue;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(symbol),
            Some(existing) if existing == symbol => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

pub(super) fn language_core_symbol_identity_at_anchor_matches(
    il: &Il,
    span: Span,
    kind: NodeKind,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    match language_core_symbol_evidence_at_anchor(il, span, kind) {
        EvidenceResolution::Found(actual) => EvidenceResolution::Found(actual == expected),
        EvidenceResolution::Ambiguous => EvidenceResolution::Ambiguous,
        EvidenceResolution::Missing => EvidenceResolution::Missing,
    }
}

fn language_core_symbol_identity_at_node_matches(
    il: &Il,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    language_core_symbol_identity_at_anchor_matches(il, il.node(node).span, il.kind(node), expected)
}

pub(super) fn imported_symbol_identity_at_node_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    let span = il.node(node).span;
    let kind = il.kind(node);
    let mut found = None;
    let mut dependencies_valid = true;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != EvidenceAnchor::node(span, kind) {
            continue;
        }
        let EvidenceKind::Symbol(actual) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(actual),
            Some(existing) if existing == actual => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
        if actual == expected
            && !imported_occurrence_symbol_dependencies_valid(il, interner, record, expected)
        {
            dependencies_valid = false;
        }
    }
    let Some(actual) = found else {
        return EvidenceResolution::Missing;
    };
    EvidenceResolution::Found(actual == expected && dependencies_valid)
}

pub(super) fn language_core_imported_namespace_identity_at_anchor_matches(
    il: &Il,
    interner: &Interner,
    span: Span,
    kind: NodeKind,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    let anchor = EvidenceAnchor::node(span, kind);
    let mut found = None;
    let mut dependencies_valid = true;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(actual) = record.kind else {
            continue;
        };
        if !matches!(actual, SymbolEvidenceKind::ImportedNamespace { .. }) {
            if record.status != EvidenceStatus::Asserted
                || !il.evidence_dependencies_asserted(record)
            {
                return EvidenceResolution::Ambiguous;
            }
            match found {
                None => found = Some(actual),
                Some(existing) if existing == actual => {}
                Some(_) => return EvidenceResolution::Ambiguous,
            }
            continue;
        }
        if !symbol_record_has_admitted_provenance(il, record) {
            continue;
        }
        if record.status != EvidenceStatus::Asserted {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(actual),
            Some(existing) if existing == actual => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
        if actual == expected
            && !imported_occurrence_symbol_dependencies_valid(il, interner, record, expected)
        {
            dependencies_valid = false;
        }
    }
    let Some(actual) = found else {
        return EvidenceResolution::Missing;
    };
    EvidenceResolution::Found(actual == expected && dependencies_valid)
}

fn language_core_imported_namespace_identity_at_node_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    language_core_imported_namespace_identity_at_anchor_matches(
        il,
        interner,
        il.node(node).span,
        il.kind(node),
        expected,
    )
}

pub(super) fn symbol_record_has_admitted_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    match record.kind {
        EvidenceKind::Symbol(
            SymbolEvidenceKind::UnshadowedGlobal { .. }
            | SymbolEvidenceKind::ImportedNamespace { .. },
        ) => language_core_record_has_provenance(il, record),
        EvidenceKind::Symbol(_) => true,
        _ => false,
    }
}

pub(super) fn binding_identity_matches(
    il: &Il,
    local_hash: u64,
    span: Span,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    match symbol_evidence_for_binding(il, local_hash, span) {
        EvidenceResolution::Found(actual) => EvidenceResolution::Found(actual == expected),
        EvidenceResolution::Ambiguous => EvidenceResolution::Ambiguous,
        EvidenceResolution::Missing => EvidenceResolution::Missing,
    }
}

/// Evidence-only proof that `node` denotes a language-defined unshadowed global
/// with the exact requested name. The raw spelling is never enough: only explicit
/// `Symbol` evidence opens the exact path, and ambiguous or conflicting evidence
/// keeps it closed.
pub fn asserted_unshadowed_global_symbol(il: &Il, node: NodeId, name: &str) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    let expected = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(name),
    };
    match language_core_symbol_identity_at_node_matches(il, node, expected) {
        EvidenceResolution::Found(matches) => matches,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => false,
    }
}

/// Prove that `node` denotes a static imported namespace for `module`.
pub fn imported_namespace_symbol(il: &Il, interner: &Interner, node: NodeId, module: &str) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    match language_core_imported_namespace_identity_at_node_matches(il, interner, node, expected) {
        EvidenceResolution::Found(matches) => matches,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => false,
    }
}

/// Prove that `node` denotes a static imported binding for `module.exported`.
pub fn imported_binding_symbol(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    imported_symbol(il, interner, node, expected)
}

/// Prove either `from module import exported as local; local(...)` or
/// `import module as ns; ns.exported(...)`.
pub fn imported_member_symbol(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    match il.kind(callee) {
        NodeKind::Var => imported_binding_symbol(il, interner, callee, module, exported),
        NodeKind::Field => {
            let Payload::Name(method) = il.node(callee).payload else {
                return false;
            };
            if interner.resolve(method) != exported {
                return false;
            }
            il.children(callee)
                .first()
                .copied()
                .is_some_and(|receiver| imported_namespace_symbol(il, interner, receiver, module))
        }
        _ => false,
    }
}

/// Prove that `node` denotes an exact language-defined qualified global path,
/// such as `Array.from` or `Object.hasOwn`. This is intentionally evidence-only:
/// unlike legacy import/global helpers, a matching selector spelling cannot prove
/// a qualified API identity by itself.
pub fn qualified_global_symbol(il: &Il, node: NodeId, path: &str) -> bool {
    qualified_global_symbol_at_anchor(il, il.node(node).span, il.kind(node), path)
}

/// Prove a qualified global identity at a preserved span/kind anchor. This is
/// used by value-graph consumers after IL node ids have been erased but source
/// spans remain attached to value nodes.
pub fn qualified_global_symbol_at_span(
    il: &Il,
    span: Option<Span>,
    kind: NodeKind,
    path: &str,
) -> bool {
    let Some(span) = span else {
        return false;
    };
    qualified_global_symbol_at_anchor(il, span, kind, path)
}

pub(super) fn qualified_global_symbol_at_anchor(
    il: &Il,
    span: Span,
    kind: NodeKind,
    path: &str,
) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, path) else {
        return false;
    };
    matches!(
        qualified_global_symbol_at_evidence_anchor(il, EvidenceAnchor::node(span, kind), contract),
        EvidenceResolution::Found(())
    )
}

pub(super) fn qualified_global_dependency_valid(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    path: &str,
) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, path) else {
        return false;
    };
    record.anchor.matches_span(span) && qualified_global_symbol_record_valid(il, record, contract)
}

pub(super) fn qualified_global_symbol_at_evidence_anchor(
    il: &Il,
    anchor: EvidenceAnchor,
    contract: QualifiedGlobalSymbolContract,
) -> EvidenceResolution<()> {
    let mut found = false;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(_) = record.kind else {
            continue;
        };
        if !qualified_global_symbol_record_valid(il, record, contract) {
            return EvidenceResolution::Ambiguous;
        }
        found = true;
    }
    if found {
        EvidenceResolution::Found(())
    } else {
        EvidenceResolution::Missing
    }
}

pub(super) fn qualified_global_symbol_record_valid(
    il: &Il,
    record: &EvidenceRecord,
    contract: QualifiedGlobalSymbolContract,
) -> bool {
    let expected = SymbolEvidenceKind::QualifiedGlobal {
        path_hash: stable_symbol_hash(contract.path),
    };
    if record.status != EvidenceStatus::Asserted
        || record.kind != EvidenceKind::Symbol(expected)
        || !il.evidence_dependencies_asserted(record)
    {
        return false;
    }
    !contract.requires_unshadowed_root
        || evidence_record_has_unshadowed_root_dependency(il, record, contract.root)
}

pub(super) fn evidence_record_has_unshadowed_root_dependency(
    il: &Il,
    record: &EvidenceRecord,
    root: &str,
) -> bool {
    let span = record.anchor.span();
    let expected = EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(root),
    });
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.status == EvidenceStatus::Asserted
                && dependency.anchor == EvidenceAnchor::source_span(span)
                && dependency.kind == expected
                && il.evidence_dependencies_asserted(dependency)
        })
    })
}

pub(super) fn imported_symbol(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    match imported_symbol_identity_at_node_matches(il, interner, node, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    let Some(local_hash) = node_name_hash(il, interner, node) else {
        return false;
    };
    if unit_defines_hash_visible_at(il, interner, local_hash, il.node(node).span) {
        return false;
    }
    let statements = top_level_statements(il);
    let matching_assignments = statements
        .iter()
        .copied()
        .filter(|&stmt| assignment_alias_hash(il, interner, stmt) == Some(local_hash))
        .collect::<Vec<_>>();
    let [assignment] = matching_assignments.as_slice() else {
        return false;
    };
    match binding_identity_matches(il, local_hash, il.node(*assignment).span, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    false
}

pub(super) fn top_level_statements(il: &Il) -> Vec<NodeId> {
    il.children(il.root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            if il.kind(node) == NodeKind::Block {
                statements.extend_from_slice(il.children(node));
            } else {
                statements.push(node);
            }
            statements
        })
}

pub(super) fn assignment_alias_hash(il: &Il, interner: &Interner, stmt: NodeId) -> Option<u64> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    (il.kind(lhs) == NodeKind::Var)
        .then(|| node_name_hash(il, interner, lhs))
        .flatten()
}

pub(super) fn assignment_parts(il: &Il, stmt: NodeId) -> Option<(NodeId, NodeId)> {
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let [lhs, rhs] = il.children(stmt) else {
        return None;
    };
    Some((*lhs, *rhs))
}

pub(super) fn node_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    il.var_binding_name(node)
        .map(|symbol| interner.resolve(symbol))
}

pub(super) fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    node_name(il, interner, node).map(stable_symbol_hash)
}

pub(super) fn unit_defines_hash(il: &Il, interner: &Interner, name_hash: u64) -> bool {
    il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}

pub(super) fn unit_defines_hash_visible_at(
    il: &Il,
    interner: &Interner,
    name_hash: u64,
    occurrence_span: Span,
) -> bool {
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}

pub fn file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    let name_hash = stable_symbol_hash(name);
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit.name.is_some_and(|symbol| {
                symbol_defines_name(il.meta.lang, interner.resolve(symbol), name, name_hash)
            })
    }) || il.nodes.iter().enumerate().any(|(idx, node)| {
        node.span.file == occurrence_span.file
            && match node.kind {
                NodeKind::Module | NodeKind::Block | NodeKind::Param => {
                    node_defines_name(il, interner, NodeId(idx as u32), name, name_hash)
                }
                NodeKind::Assign => il
                    .children(NodeId(idx as u32))
                    .first()
                    .copied()
                    .is_some_and(|lhs| node_defines_name(il, interner, lhs, name, name_hash)),
                _ => false,
            }
    })
}

pub(super) fn node_defines_name(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    name: &str,
    name_hash: u64,
) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => {
            symbol_defines_name(il.meta.lang, interner.resolve(symbol), name, name_hash)
        }
        Payload::Cid(cid) => il.cid_names.get(cid as usize).is_some_and(|symbol| {
            symbol_defines_name(il.meta.lang, interner.resolve(*symbol), name, name_hash)
        }),
        _ => false,
    }
}

pub(super) fn symbol_defines_name(lang: Lang, text: &str, name: &str, name_hash: u64) -> bool {
    stable_symbol_hash(text) == name_hash
        || (js_like_lang(lang) && contains_js_identifier(text, name))
}

pub(super) fn literal_string_hash(il: &Il, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::LitStr(hash) => Some(hash),
        _ => None,
    }
}
