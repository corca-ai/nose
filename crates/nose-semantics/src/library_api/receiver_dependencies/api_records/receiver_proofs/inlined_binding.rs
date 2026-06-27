use super::*;

pub(super) fn domain_dependency_record_matches_canonical_receiver(
    il: &Il,
    interner: Option<&Interner>,
    dependency: &EvidenceRecord,
    receiver: NodeId,
) -> bool {
    if domain_dependency_matches_canonical_receiver(il, interner, dependency.anchor, receiver) {
        return true;
    }
    binding_domain_dependency_matches_inlined_receiver(il, dependency, receiver)
}

fn binding_domain_dependency_matches_inlined_receiver(
    il: &Il,
    dependency: &EvidenceRecord,
    receiver: NodeId,
) -> bool {
    let EvidenceAnchor::Binding { .. } = dependency.anchor else {
        return false;
    };
    binding_domain_record_dependencies_match_inlined_receiver(il, dependency, receiver)
        || il
            .evidence_anchored_at(dependency.anchor.span())
            .any(|record| {
                record.id != dependency.id
                    && record.anchor == dependency.anchor
                    && record.kind == dependency.kind
                    && record.status == EvidenceStatus::Asserted
                    && il.evidence_dependencies_asserted(record)
                    && binding_domain_record_dependencies_match_inlined_receiver(
                        il, record, receiver,
                    )
            })
}

fn binding_domain_record_dependencies_match_inlined_receiver(
    il: &Il,
    dependency: &EvidenceRecord,
    receiver: NodeId,
) -> bool {
    let EvidenceKind::Domain(domain) = dependency.kind else {
        return false;
    };
    dependency.dependencies.iter().any(|&id| {
        let Some(upstream) = il.evidence_record_by_id(id) else {
            return false;
        };
        upstream.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(upstream)
            && upstream.kind == EvidenceKind::Domain(domain)
            && matches!(
                upstream.anchor,
                EvidenceAnchor::Node { span, kind }
                    if span == il.node(receiver).span && kind == il.kind(receiver)
            )
    })
}
