use super::*;

pub(in crate::library_api) fn domain_or_sequence_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    if let Some(id) = domain_dependency_id_for_receiver(il, interner, receiver, contract, cache) {
        return Some(vec![id]);
    }
    sequence_surface_dependency_id_for_receiver(il, interner, receiver, contract).map(|id| vec![id])
}

pub(in crate::library_api) fn domain_dependency_id_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    domain_dependency_id_for_receiver_requirement(il, interner, receiver, requirement, cache)
}

pub(in crate::library_api) fn domain_dependency_id_for_receiver_requirement(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    requirement: DomainRequirement,
    cache: &mut LibraryApiDependencyCache,
) -> Option<EvidenceId> {
    // A record can match the receiver only when anchored at one of three spans
    // (the receiver node itself, its unique binding LHS, or its declaring
    // param — see `domain_dependency_anchor_matches_receiver`), so query those
    // index buckets instead of walking the whole evidence table. Candidates
    // are visited in evidence order, exactly like the pass they replace.
    let mut indices = il.evidence_indices_anchored_at(il.node(receiver).span);
    if let EvidenceResolution::Found(lhs) =
        unique_binding_lhs_for_var_reference_cached(il, interner, receiver, cache)
    {
        indices.extend(il.evidence_indices_anchored_at(il.node(lhs).span));
    }
    if let Some(span) = receiver_param_span_cached(il, receiver, cache) {
        indices.extend(il.evidence_indices_anchored_at(span));
    }
    indices.sort_unstable();
    indices.dedup();
    let mut found = None;
    for idx in indices {
        let record = &il.evidence[idx as usize];
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
            || !requirement.accepts(domain)
            || !domain_dependency_anchor_matches_receiver(
                il,
                interner,
                receiver,
                record.anchor,
                cache,
            )
        {
            continue;
        }
        match found {
            None => found = Some((domain, record.id)),
            Some((existing, _)) if existing == domain => {}
            Some(_) => return None,
        }
    }
    found.map(|(_, id)| id)
}

pub(in crate::library_api) fn domain_dependency_anchor_matches_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    anchor: EvidenceAnchor,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    match anchor {
        EvidenceAnchor::Node { span, kind } => {
            span == il.node(receiver).span && kind == il.kind(receiver)
        }
        EvidenceAnchor::Binding { span, local_hash } => {
            matches!(
                unique_binding_lhs_for_var_reference_cached(il, interner, receiver, cache),
                EvidenceResolution::Found(lhs)
                    if il.node(lhs).span == span
                        && binding_lhs_matches_local_hash(il, interner, lhs, local_hash)
            )
        }
        EvidenceAnchor::Param { span } => {
            receiver_param_span_cached(il, receiver, cache) == Some(span)
        }
        _ => false,
    }
}

pub(in crate::library_api) fn unique_binding_lhs_for_var_reference_cached(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    if let Some(&cached) = cache.binding_lhs_by_reference.get(&node) {
        return cached;
    }
    let resolution = unique_binding_lhs_for_var_reference_with_cache(il, interner, node, cache);
    cache.binding_lhs_by_reference.insert(node, resolution);
    resolution
}

pub(in crate::library_api) fn unique_binding_lhs_for_var_reference_with_cache(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    let scope = nearest_scope_cached(il, node, cache);
    let reference_is_free_name = matches!(il.node(node).payload, Payload::Name(_));
    let mut found = None;
    // Same scope-bucketed candidate set as `evidence::unique_binding_lhs_for_var_reference`.
    let module_level: &[NodeId] = if reference_is_free_name && scope.is_some() {
        il.assigns_in_scope(None)
    } else {
        &[]
    };
    for &assign in il.assigns_in_scope(scope).iter().chain(module_level) {
        if !assignment_is_visible_at_reference(il, assign, node) {
            continue;
        }
        let Some(&lhs) = il.children(assign).first() else {
            continue;
        };
        if !var_references_same_binding(il, lhs, node)
            && !free_name_reference_matches_binding_domain(il, interner, lhs, node)
        {
            continue;
        }
        match found {
            None => found = Some(lhs),
            Some(existing) if existing == lhs => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn free_name_reference_matches_binding_domain(
    il: &Il,
    interner: &Interner,
    lhs: NodeId,
    reference: NodeId,
) -> bool {
    let Payload::Name(name) = il.node(reference).payload else {
        return false;
    };
    binding_lhs_matches_local_hash(
        il,
        interner,
        lhs,
        stable_symbol_hash(interner.resolve(name)),
    )
}

fn binding_lhs_matches_local_hash(
    il: &Il,
    interner: &Interner,
    lhs: NodeId,
    local_hash: u64,
) -> bool {
    node_name_hash(il, interner, lhs) == Some(local_hash)
        || binding_lhs_has_live_domain_hash(il, lhs, local_hash)
}

fn binding_lhs_has_live_domain_hash(il: &Il, lhs: NodeId, local_hash: u64) -> bool {
    let span = il.node(lhs).span;
    il.evidence_anchored_at(span).any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                span: anchor_span,
                local_hash: anchor_hash,
            } if anchor_span == span && anchor_hash == local_hash
        ) && matches!(record.kind, EvidenceKind::Domain(_))
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

pub(in crate::library_api) fn nearest_scope_cached(
    il: &Il,
    node: NodeId,
    _cache: &mut LibraryApiDependencyCache,
) -> Option<NodeId> {
    // `Il::nearest_scope` is already a whole-arena lazy index; no per-call cache needed.
    nearest_scope(il, node)
}

pub(in crate::library_api) fn receiver_param_span_cached(
    il: &Il,
    receiver: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    if let Some(cached) = cache
        .receiver_param_span_by_reference
        .get(&receiver)
        .copied()
    {
        return cached;
    }
    let span = receiver_var_payload(il, receiver).and_then(|payload| match payload {
        Payload::Cid(cid) => receiver_cid_param_span_with_cache(il, receiver, cid, cache),
        Payload::Name(name) => receiver_named_param_span_with_cache(il, receiver, name, cache),
        _ => None,
    });
    cache
        .receiver_param_span_by_reference
        .insert(receiver, span);
    span
}

pub(in crate::library_api) fn receiver_var_payload(il: &Il, receiver: NodeId) -> Option<Payload> {
    (il.kind(receiver) == NodeKind::Var).then_some(il.node(receiver).payload)
}

pub(in crate::library_api) fn receiver_cid_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    cid: u32,
    _cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    receiver_cid_param_span(il, receiver, cid)
}

pub(in crate::library_api) fn receiver_named_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    name: Symbol,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    let (scope, param) = nearest_named_param_scope(il, receiver, name)?;
    (!name_is_assigned_in_scope_cached(il, name, scope, cache)).then_some(il.node(param).span)
}

pub(in crate::library_api) fn name_is_assigned_in_scope_cached(
    il: &Il,
    name: Symbol,
    scope: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    if let Some(&assigned) = cache.name_assigned_in_scope.get(&(scope, name)) {
        return assigned;
    }
    let assigned = il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope_cached(il, id, cache) != Some(scope) {
            return false;
        }
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var && il.node(lhs).payload == Payload::Name(name)
    });
    cache.name_assigned_in_scope.insert((scope, name), assigned);
    assigned
}
