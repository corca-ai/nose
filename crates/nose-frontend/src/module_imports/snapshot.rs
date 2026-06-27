use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, ImportEvidenceKind, Node, NodeId,
    NodeKind, Payload, Span,
};
use nose_semantics::language_core_evidence_provenance;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone)]
pub(super) struct SnapshotNode {
    kind: NodeKind,
    payload: Payload,
    span: Span,
    children: Vec<usize>,
}

#[derive(Clone)]
pub(super) struct SubtreeSnapshot {
    nodes: Vec<SnapshotNode>,
    root: usize,
    evidence: Vec<SnapshotEvidence>,
}

#[derive(Clone)]
pub(super) struct SnapshotEvidence {
    source_id: EvidenceId,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: Vec<EvidenceId>,
    status: EvidenceStatus,
}

pub(super) struct AppendedSnapshot {
    pub(super) root: NodeId,
    pub(super) evidence: Vec<EvidenceId>,
}

pub(super) fn snapshot_subtree(il: &Il, root: NodeId) -> SubtreeSnapshot {
    fn snapshot_node(il: &Il, node: NodeId, out: &mut Vec<SnapshotNode>) -> usize {
        let children: Vec<usize> = il
            .children(node)
            .iter()
            .map(|&child| snapshot_node(il, child, out))
            .collect();
        let idx = out.len();
        let node_ref = il.node(node);
        out.push(SnapshotNode {
            kind: node_ref.kind,
            payload: node_ref.payload,
            span: node_ref.span,
            children,
        });
        idx
    }

    let mut nodes = Vec::new();
    let root = snapshot_node(il, root, &mut nodes);
    let evidence = snapshot_evidence(il, &nodes);
    SubtreeSnapshot {
        nodes,
        root,
        evidence,
    }
}

fn snapshot_evidence(il: &Il, nodes: &[SnapshotNode]) -> Vec<SnapshotEvidence> {
    let spans: FxHashSet<Span> = nodes.iter().map(|node| node.span).collect();
    let asserted: FxHashMap<EvidenceId, &EvidenceRecord> = il
        .evidence
        .iter()
        .filter(|record| record.status == EvidenceStatus::Asserted)
        .map(|record| (record.id, record))
        .collect();
    let mut kept: FxHashSet<EvidenceId> = asserted
        .values()
        .filter(|record| spans.contains(&evidence_anchor_span(record.anchor)))
        .map(|record| record.id)
        .collect();

    let mut stack: Vec<EvidenceId> = kept.iter().copied().collect();
    while let Some(id) = stack.pop() {
        let Some(record) = asserted.get(&id) else {
            continue;
        };
        for &dependency in &record.dependencies {
            if asserted.contains_key(&dependency) && kept.insert(dependency) {
                stack.push(dependency);
            }
        }
    }

    loop {
        let rejected: Vec<EvidenceId> = kept
            .iter()
            .copied()
            .filter(|id| {
                asserted
                    .get(id)
                    .is_some_and(|record| record.dependencies.iter().any(|dep| !kept.contains(dep)))
            })
            .collect();
        if rejected.is_empty() {
            break;
        }
        for id in rejected {
            kept.remove(&id);
        }
    }
    il.evidence
        .iter()
        .filter(|record| kept.contains(&record.id))
        .map(|record| SnapshotEvidence {
            source_id: record.id,
            anchor: record.anchor,
            kind: record.kind,
            provenance: record.provenance,
            dependencies: record.dependencies.clone(),
            status: record.status,
        })
        .collect()
}

pub(super) fn record_immutable_literal_export_evidence(
    il: &mut Il,
    rhs: NodeId,
    module_hash: u64,
    exported_hash: u64,
    deps: &[SubtreeSnapshot],
) -> EvidenceId {
    let spans = subtree_spans(il, rhs);
    let mut seen = FxHashSet::default();
    let mut dependencies = Vec::new();
    for id in il.evidence.iter().filter_map(|record| {
        (record.status == EvidenceStatus::Asserted
            && spans.contains(&evidence_anchor_span(record.anchor))
            && !matches!(
                record.kind,
                EvidenceKind::Import(
                    ImportEvidenceKind::ImmutableLiteralExport { .. }
                        | ImportEvidenceKind::ImportedLiteralSnapshot { .. }
                )
            ))
        .then_some(record.id)
    }) {
        if seen.insert(id) {
            dependencies.push(id);
        }
    }
    for id in deps
        .iter()
        .flat_map(|dep| dep.evidence.iter().map(|evidence| evidence.source_id))
    {
        if seen.insert(id) {
            dependencies.push(id);
        }
    }
    push_language_core_evidence_with_dependencies(
        il,
        EvidenceAnchor::node(il.node(rhs).span, il.kind(rhs)),
        EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
            module_hash,
            exported_hash,
            root_kind: il.kind(rhs),
        }),
        dependencies,
    )
}

fn subtree_spans(il: &Il, root: NodeId) -> FxHashSet<Span> {
    fn collect(il: &Il, node: NodeId, out: &mut FxHashSet<Span>) {
        out.insert(il.node(node).span);
        for &child in il.children(node) {
            collect(il, child, out);
        }
    }

    let mut spans = FxHashSet::default();
    collect(il, root, &mut spans);
    spans
}

fn evidence_anchor_span(anchor: EvidenceAnchor) -> Span {
    match anchor {
        EvidenceAnchor::SourceSpan(span)
        | EvidenceAnchor::Node { span, .. }
        | EvidenceAnchor::Param { span }
        | EvidenceAnchor::Binding { span, .. }
        | EvidenceAnchor::Sequence { span } => span,
    }
}

pub(super) fn append_snapshot(il: &mut Il, snapshot: &SubtreeSnapshot) -> AppendedSnapshot {
    let mut new_ids = vec![NodeId(0); snapshot.nodes.len()];
    for (idx, snapshot_node) in snapshot.nodes.iter().enumerate() {
        let children: Vec<NodeId> = snapshot_node
            .children
            .iter()
            .map(|&child_idx| new_ids[child_idx])
            .collect();
        let child_start = il.edges.len() as u32;
        il.edges.extend_from_slice(&children);
        let id = NodeId(il.nodes.len() as u32);
        il.nodes.push(Node {
            kind: snapshot_node.kind,
            payload: snapshot_node.payload,
            span: snapshot_node.span,
            child_start,
            child_len: children.len() as u32,
        });
        new_ids[idx] = id;
    }
    let source_evidence: FxHashMap<EvidenceId, &SnapshotEvidence> = snapshot
        .evidence
        .iter()
        .map(|evidence| (evidence.source_id, evidence))
        .collect();
    let mut evidence_id_map = FxHashMap::default();
    let mut copied_evidence = Vec::with_capacity(snapshot.evidence.len());
    for evidence in &snapshot.evidence {
        let id = append_snapshot_evidence(
            il,
            &source_evidence,
            &mut evidence_id_map,
            evidence.source_id,
        );
        if !copied_evidence.contains(&id) {
            copied_evidence.push(id);
        }
    }
    AppendedSnapshot {
        root: new_ids[snapshot.root],
        evidence: copied_evidence,
    }
}

fn append_snapshot_evidence(
    il: &mut Il,
    source_evidence: &FxHashMap<EvidenceId, &SnapshotEvidence>,
    evidence_id_map: &mut FxHashMap<EvidenceId, EvidenceId>,
    source_id: EvidenceId,
) -> EvidenceId {
    if let Some(id) = evidence_id_map.get(&source_id).copied() {
        return id;
    }
    let evidence = source_evidence
        .get(&source_id)
        .copied()
        .expect("snapshot evidence dependency must be closed");
    let dependencies: Vec<EvidenceId> = evidence
        .dependencies
        .iter()
        .map(|dependency| {
            append_snapshot_evidence(il, source_evidence, evidence_id_map, *dependency)
        })
        .collect();
    let anchor = evidence.anchor;
    let id = existing_snapshot_evidence_id(
        il,
        anchor,
        evidence.kind,
        evidence.provenance,
        &dependencies,
        evidence.status,
    )
    .unwrap_or_else(|| {
        let id = EvidenceId(il.evidence.len() as u32);
        il.evidence.push(EvidenceRecord {
            id,
            anchor,
            kind: evidence.kind,
            provenance: evidence.provenance,
            dependencies,
            status: evidence.status,
        });
        id
    });
    evidence_id_map.insert(source_id, id);
    id
}

fn existing_snapshot_evidence_id(
    il: &Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: &[EvidenceId],
    status: EvidenceStatus,
) -> Option<EvidenceId> {
    il.evidence
        .iter()
        .find(|record| {
            record.anchor == anchor
                && record.kind == kind
                && record.provenance == provenance
                && record.dependencies == dependencies
                && record.status == status
        })
        .map(|record| record.id)
}

pub(super) fn replace_assignment_rhs(il: &mut Il, stmt: NodeId, rhs: NodeId) {
    let Some(node) = il.nodes.get(stmt.0 as usize) else {
        return;
    };
    if node.kind != NodeKind::Assign || node.child_len != 2 {
        return;
    }
    let rhs_slot = node.child_start as usize + 1;
    if let Some(slot) = il.edges.get_mut(rhs_slot) {
        *slot = rhs;
    }
}

pub(super) fn replace_node_references(il: &mut Il, old: NodeId, new: NodeId) {
    for edge in &mut il.edges {
        if *edge == old {
            *edge = new;
        }
    }
    if il.root == old {
        il.root = new;
    }
}

pub(super) fn record_imported_literal_snapshot_evidence(
    il: &mut Il,
    rhs: NodeId,
    module_hash: u64,
    exported_hash: u64,
    import_evidence: EvidenceId,
    copied_snapshot_evidence: Vec<EvidenceId>,
) {
    let mut dependencies = Vec::with_capacity(copied_snapshot_evidence.len() + 1);
    dependencies.push(import_evidence);
    for evidence in copied_snapshot_evidence {
        if !dependencies.contains(&evidence) {
            dependencies.push(evidence);
        }
    }
    push_language_core_evidence_with_dependencies(
        il,
        EvidenceAnchor::node(il.node(rhs).span, il.kind(rhs)),
        EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
            module_hash,
            exported_hash,
            root_kind: il.kind(rhs),
        }),
        dependencies,
    );
}

fn push_language_core_evidence_with_dependencies(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    push_evidence_with_provenance(
        il,
        anchor,
        kind,
        EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies,
    )
}

#[cfg(test)]
pub(super) fn push_builtin_evidence_with_dependencies(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    push_evidence_with_provenance(
        il,
        anchor,
        kind,
        EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(nose_semantics::BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash(rule)),
        },
        dependencies,
    )
}

fn push_evidence_with_provenance(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let id = EvidenceId(il.evidence.len() as u32);
    il.evidence.push(EvidenceRecord {
        id,
        anchor,
        kind,
        provenance,
        dependencies,
        status: EvidenceStatus::Asserted,
    });
    id
}

pub(super) fn prepend_root_statement(il: &mut Il, stmt: NodeId) {
    let old_root = il.root;
    let old_root_node = *il.node(old_root);
    let mut children = Vec::with_capacity(il.children(old_root).len() + 1);
    children.push(stmt);
    children.extend_from_slice(il.children(old_root));
    let child_start = il.edges.len() as u32;
    il.edges.extend_from_slice(&children);
    let new_root = NodeId(il.nodes.len() as u32);
    il.nodes.push(Node {
        kind: old_root_node.kind,
        payload: old_root_node.payload,
        span: old_root_node.span,
        child_start,
        child_len: children.len() as u32,
    });
    il.root = new_root;
}
