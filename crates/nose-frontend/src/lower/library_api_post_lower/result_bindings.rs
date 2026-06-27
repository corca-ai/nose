use super::super::post_lower_evidence::post_lower_find_or_push_evidence;
use super::super::*;

pub(super) fn post_lower_record_assignment_binding_domain_from_call_result(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    result_domain: Option<DomainEvidence>,
    dependency: EvidenceId,
) {
    let Some(domain) = result_domain else {
        return;
    };
    let assignments: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Assign).then_some(NodeId(idx as u32)))
        .filter_map(|assign| {
            let (lhs, rhs) = il.assignment_var_parts(assign)?;
            (rhs == call).then_some(lhs)
        })
        .collect();
    for lhs in assignments {
        let Some(local) = il.var_binding_name(lhs) else {
            continue;
        };
        let anchor = EvidenceAnchor::binding(
            il.node(lhs).span,
            stable_symbol_hash(interner.resolve(local)),
        );
        if post_lower_asserted_domain_evidence_exists(il, anchor, domain) {
            continue;
        }
        let _ = post_lower_find_or_push_evidence(
            il,
            anchor,
            EvidenceKind::Domain(domain),
            "binding_domain_from_value_result",
            vec![dependency],
        );
    }
}

fn post_lower_asserted_domain_evidence_exists(
    il: &Il,
    anchor: EvidenceAnchor,
    domain: DomainEvidence,
) -> bool {
    il.evidence_anchored_at(anchor.span()).any(|record| {
        record.anchor == anchor
            && record.kind == EvidenceKind::Domain(domain)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}
