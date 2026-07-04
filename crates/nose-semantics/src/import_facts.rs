//! Import evidence facts and imported literal provenance.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportFactKind {
    Binding,
    Namespace,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFactContract {
    pub kind: ImportFactKind,
    pub channel: ChannelEligibility,
}

pub fn import_fact_contract(kind: ImportFactKind) -> ImportFactContract {
    match kind {
        ImportFactKind::Binding => ImportFactContract {
            kind,
            channel: ChannelEligibility::ExactProven,
        },
        ImportFactKind::Namespace => ImportFactContract {
            kind,
            channel: ChannelEligibility::ExactProven,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFact {
    pub kind: ImportFactKind,
    pub module_hash: u64,
    pub exported_hash: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFactProof {
    pub fact: ImportFact,
    pub evidence: EvidenceId,
}

pub(super) fn import_fact_evidence_at_sequence_span(
    il: &Il,
    span: Span,
) -> EvidenceResolution<ImportFactProof> {
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !matches!(record.anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span)
        {
            continue;
        }
        let Some(value) = (match record.kind {
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash,
                exported_hash,
            }) => Some(ImportFact {
                kind: ImportFactKind::Binding,
                module_hash,
                exported_hash: Some(exported_hash),
            }),
            EvidenceKind::Import(ImportEvidenceKind::Namespace { module_hash }) => {
                Some(ImportFact {
                    kind: ImportFactKind::Namespace,
                    module_hash,
                    exported_hash: None,
                })
            }
            _ => None,
        }) else {
            continue;
        };
        if !asserted_language_core_record(il, record) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => {
                found = Some(ImportFactProof {
                    fact: value,
                    evidence: record.id,
                })
            }
            Some(existing) if existing.fact == value => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

/// Evidence-only import fact resolution for semantic consumers. Import proof is
/// intentionally not encoded in the lowered `Seq` payload; callers rely on a
/// provider-owned evidence record, not on tag spelling.
pub fn import_fact_evidence_rhs(il: &Il, rhs: NodeId) -> Option<ImportFact> {
    import_fact_proof_rhs(il, rhs).map(|proof| proof.fact)
}

pub fn import_fact_proof_rhs(il: &Il, rhs: NodeId) -> Option<ImportFactProof> {
    if il.kind(rhs) != NodeKind::Seq {
        return None;
    }
    match import_fact_evidence_at_sequence_span(il, il.node(rhs).span) {
        EvidenceResolution::Found(proof) => Some(proof),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

/// Prove that `span/kind` is a builtin language-core imported-literal producer
/// or copied snapshot whose recorded dependencies are all asserted. This proof
/// preserves a provider-scope literal producer after cross-file replacement;
/// consumers must still check the expression shape/result contract they are
/// about to build.
pub fn imported_literal_producer_evidence_at_span(il: &Il, span: Span, kind: NodeKind) -> bool {
    matches!(
        imported_literal_evidence_at_span(il, span, kind, ImportedLiteralAdmission::Producer),
        EvidenceResolution::Found(())
    )
}

pub fn imported_literal_snapshot_evidence_at_span(il: &Il, span: Span, kind: NodeKind) -> bool {
    matches!(
        imported_literal_evidence_at_span(il, span, kind, ImportedLiteralAdmission::SnapshotOnly),
        EvidenceResolution::Found(())
    )
}

pub fn imported_literal_producer_evidence_for_node(il: &Il, node: NodeId) -> bool {
    imported_literal_producer_evidence_at_span(il, il.node(node).span, il.kind(node))
}

#[derive(Clone, Copy)]
enum ImportedLiteralAdmission {
    Producer,
    SnapshotOnly,
}

fn imported_literal_evidence_at_span(
    il: &Il,
    span: Span,
    kind: NodeKind,
    admission: ImportedLiteralAdmission,
) -> EvidenceResolution<()> {
    let mut found = false;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != EvidenceAnchor::node(span, kind) {
            continue;
        }
        if !imported_literal_record_matches(record.kind, kind, admission) {
            continue;
        }
        if !asserted_language_core_record(il, record) {
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

fn imported_literal_record_matches(
    evidence: EvidenceKind,
    kind: NodeKind,
    admission: ImportedLiteralAdmission,
) -> bool {
    match (admission, evidence) {
        (
            ImportedLiteralAdmission::Producer,
            EvidenceKind::Import(
                ImportEvidenceKind::ImmutableLiteralExport { root_kind, .. }
                | ImportEvidenceKind::ImportedLiteralSnapshot { root_kind, .. },
            ),
        ) => root_kind == kind,
        (
            ImportedLiteralAdmission::SnapshotOnly,
            EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot { root_kind, .. }),
        ) => root_kind == kind,
        _ => false,
    }
}
