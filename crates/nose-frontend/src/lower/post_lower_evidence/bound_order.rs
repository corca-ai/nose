use super::*;

pub(in crate::lower) fn record_post_lower_bound_order_guard_evidence(
    il: &mut Il,
    interner: &Interner,
) {
    let comparisons: Vec<NodeId> = (0..il.nodes.len() as u32)
        .map(NodeId)
        .filter(|&node| il.kind(node) == NodeKind::BinOp)
        .collect();
    for node in comparisons {
        for activation in [
            BoundOrderGuardActivation::WhenTrue,
            BoundOrderGuardActivation::WhenFalse,
        ] {
            let Some((lower, upper)) = operands_from_condition(il, node, activation) else {
                continue;
            };
            let Some(mut dependencies) = operand_dependencies(il, interner, lower) else {
                continue;
            };
            let Some(upper_dependencies) = operand_dependencies(il, interner, upper) else {
                continue;
            };
            dependencies.extend(upper_dependencies);
            dependencies.sort_unstable_by_key(|id| id.0);
            dependencies.dedup();
            let _ = post_lower_find_or_push_evidence(
                il,
                EvidenceAnchor::node(il.node(node).span, NodeKind::BinOp),
                EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
                    lower_span: il.node(lower).span,
                    upper_span: il.node(upper).span,
                    activation,
                }),
                "bound_order_guard_post_lower",
                dependencies,
            );
        }
    }
}

fn operands_from_condition(
    il: &Il,
    node: NodeId,
    activation: BoundOrderGuardActivation,
) -> Option<(NodeId, NodeId)> {
    let Payload::Op(op) = il.node(node).payload else {
        return None;
    };
    let [left, right] = il.children(node) else {
        return None;
    };
    match (activation, op) {
        (BoundOrderGuardActivation::WhenTrue, Op::Lt | Op::Le)
        | (BoundOrderGuardActivation::WhenFalse, Op::Gt | Op::Ge) => Some((*left, *right)),
        (BoundOrderGuardActivation::WhenTrue, Op::Gt | Op::Ge)
        | (BoundOrderGuardActivation::WhenFalse, Op::Lt | Op::Le) => Some((*right, *left)),
        _ => None,
    }
}

fn operand_dependencies(il: &Il, interner: &Interner, node: NodeId) -> Option<Vec<EvidenceId>> {
    match (il.kind(node), il.node(node).payload) {
        (NodeKind::Lit, Payload::LitInt(_)) => Some(Vec::new()),
        (NodeKind::Var, Payload::Cid(cid)) => {
            Some(vec![integer_param_dependency(il, interner, node, cid)?])
        }
        (NodeKind::Var, Payload::Name(name)) => Some(vec![named_integer_param_dependency(
            il, interner, node, name,
        )?]),
        _ => None,
    }
}

fn integer_param_dependency(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
    cid: u32,
) -> Option<EvidenceId> {
    if !receiver_satisfies_domain(il, interner, reference, DomainRequirement::INTEGER) {
        return None;
    }
    let scope = il.nearest_scope(reference)?;
    let mut found = None;
    for &param in il.children(scope) {
        if il.kind(param) != NodeKind::Param
            || !matches!(il.node(param).payload, Payload::Cid(param_cid) if param_cid == cid)
        {
            continue;
        }
        merge_unique_evidence(
            &mut found,
            integer_param_domain_evidence_id(il, il.node(param).span)?,
        )?;
    }
    found
}

fn named_integer_param_dependency(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
    name: Symbol,
) -> Option<EvidenceId> {
    if !receiver_satisfies_domain(il, interner, reference, DomainRequirement::INTEGER) {
        return None;
    }
    let scope = il.nearest_scope(reference)?;
    let name_text = interner.resolve(name);
    let mut found = None;
    for &param in il.children(scope) {
        if il.kind(param) != NodeKind::Param {
            continue;
        }
        let same_param = match il.node(param).payload {
            Payload::Name(param_name) => interner.resolve(param_name) == name_text,
            Payload::Cid(param_cid) => il
                .cid_names
                .get(param_cid as usize)
                .is_some_and(|param_name| interner.resolve(*param_name) == name_text),
            _ => false,
        };
        if same_param {
            merge_unique_evidence(
                &mut found,
                integer_param_domain_evidence_id(il, il.node(param).span)?,
            )?;
        }
    }
    found
}

fn integer_param_domain_evidence_id(il: &Il, span: Span) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::param(span);
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if domain != DomainEvidence::Integer
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            return None;
        }
        merge_unique_evidence(&mut found, record.id)?;
    }
    found
}

fn merge_unique_evidence(found: &mut Option<EvidenceId>, candidate: EvidenceId) -> Option<()> {
    match *found {
        None => *found = Some(candidate),
        Some(existing) if existing == candidate => {}
        Some(_) => return None,
    }
    Some(())
}
