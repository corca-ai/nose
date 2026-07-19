use super::*;

/// Admit the one external exact operation exposed by semantic-pack v1.
///
/// The product lock/receipt/evidence pipeline is the only producer of this
/// record. Consumers nevertheless re-check its query-local provenance,
/// dependency closure, anchor, and arity against the current IL so a stale or
/// transplanted record cannot affect normalization.
pub fn admitted_external_collection_factory_at_call(
    il: &Il,
    call: NodeId,
) -> Option<&EvidenceRecord> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let span = il.node(call).span;
    let actual_arity = il.children(call).len().saturating_sub(1);
    let mut admitted = il.evidence_anchored_at(span).filter(|record| {
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::ExternalCollectionFactory { arity }) =
            record.kind
        else {
            return false;
        };
        record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::External
            && record.provenance.pack_hash.is_some()
            && record.provenance.rule_hash.is_some()
            && usize::from(arity) == actual_arity
            && il.evidence_dependencies_asserted(record)
    });
    let first = admitted.next()?;
    admitted.next().is_none().then_some(first)
}
