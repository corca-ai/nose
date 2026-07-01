use super::{
    import_conflicts, JavaExecutorKind, COMPLETABLE_FUTURE_TYPE, COMPLETION_STAGE_TYPE,
    EXECUTOR_SERVICE_TYPE, EXECUTOR_TYPE, FUTURE_TYPE, JAVA_CONCURRENT_MODULE,
    SCHEDULED_EXECUTOR_SERVICE_TYPE, SCHEDULED_FUTURE_TYPE,
};
use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceRecord, EvidenceStatus, Interner, NodeId, NodeKind, Payload, Span, Symbol,
    SymbolEvidenceKind, UnitKind,
};

pub(super) fn completion_stage_receiver_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    receiver_type_proven(
        il,
        interner,
        callee,
        context,
        |domain| domain == DomainEvidence::FutureLike,
        java_completion_stage_type_import_dependency,
    )
}

pub(super) fn future_handle_receiver_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    receiver_type_proven(
        il,
        interner,
        callee,
        context,
        |domain| domain == DomainEvidence::FutureLike,
        java_future_handle_type_import_dependency,
    )
}

pub(super) fn completable_future_receiver_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> bool {
    receiver_type_proven(
        il,
        interner,
        callee,
        context,
        |domain| domain == DomainEvidence::FutureLike,
        java_completable_future_type_import_dependency,
    )
}

pub(super) fn executor_receiver_kind_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &crate::verify_admission::AdmissionContext,
) -> Option<JavaExecutorKind> {
    let receiver = super::super::super::method_receiver(il, callee)?;
    let DomainEvidence::Nominal { type_hash } =
        nose_semantics::domain_evidence_for_receiver(il, interner, receiver)?
    else {
        return None;
    };
    let kind = java_executor_kind_from_type_hash(type_hash)?;
    let record = receiver_domain_record(
        il,
        interner,
        receiver,
        |domain| matches!(domain, DomainEvidence::Nominal { type_hash: actual } if actual == type_hash),
    )?;
    record
        .dependencies
        .iter()
        .copied()
        .any(|dependency| {
            java_executor_type_import_dependency(il, dependency, kind).is_some_and(
                |imported_type| {
                    java_concurrent_import_usable_at_span(
                        il,
                        interner,
                        record.anchor.span(),
                        imported_type,
                        context,
                        dependency,
                    )
                },
            )
        })
        .then_some(kind)
}

fn receiver_type_proven(
    il: &nose_il::Il,
    interner: &Interner,
    callee: NodeId,
    context: &crate::verify_admission::AdmissionContext,
    accepts: impl Fn(DomainEvidence) -> bool,
    import_dependency: impl Fn(&nose_il::Il, EvidenceId) -> Option<&'static str>,
) -> bool {
    let Some(receiver) = super::super::super::method_receiver(il, callee) else {
        return false;
    };
    if !nose_semantics::domain_evidence_for_receiver(il, interner, receiver).is_some_and(&accepts) {
        return false;
    }
    receiver_domain_record(il, interner, receiver, accepts).is_some_and(|record| {
        record.dependencies.iter().copied().any(|dependency| {
            import_dependency(il, dependency).is_some_and(|imported_type| {
                java_concurrent_import_usable_at_span(
                    il,
                    interner,
                    record.anchor.span(),
                    imported_type,
                    context,
                    dependency,
                )
            })
        })
    })
}

fn java_concurrent_import_usable_at_span(
    il: &nose_il::Il,
    interner: &Interner,
    span: Span,
    type_name: &str,
    context: &crate::verify_admission::AdmissionContext,
    import_dependency: EvidenceId,
) -> bool {
    let Some(import_record) = il.evidence.get(import_dependency.0 as usize) else {
        return false;
    };
    !java_type_name_shadowed_at_span(il, interner, span, type_name)
        && !import_conflicts::type_import_conflicted_at_span(il, span, type_name)
        && (!super::java_imported_binding_is_wildcard_backed(il, import_record)
            || !context.java_package_local_type_is_visible_in_file(il, interner, type_name))
}

fn receiver_domain_record<'a>(
    il: &'a nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    accepts: impl Fn(DomainEvidence) -> bool,
) -> Option<&'a EvidenceRecord> {
    receiver_node_domain_record(il, receiver, &accepts)
        .or_else(|| receiver_binding_domain_record(il, interner, receiver, &accepts))
        .or_else(|| receiver_param_domain_record(il, receiver, &accepts))
}

fn receiver_node_domain_record<'a>(
    il: &'a nose_il::Il,
    receiver: NodeId,
    accepts: &impl Fn(DomainEvidence) -> bool,
) -> Option<&'a EvidenceRecord> {
    let span = il.node(receiver).span;
    let kind = il.kind(receiver);
    il.evidence_anchored_at(span).find(|record| {
        record.anchor == EvidenceAnchor::node(span, kind)
            && matches!(record.kind, EvidenceKind::Domain(domain) if accepts(domain))
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

fn receiver_binding_domain_record<'a>(
    il: &'a nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
    accepts: &impl Fn(DomainEvidence) -> bool,
) -> Option<&'a EvidenceRecord> {
    let lhs = receiver_same_scope_binding_lhs(il, interner, receiver)?;
    let span = il.node(lhs).span;
    let local_hash = node_name_hash(il, interner, lhs)?;
    il.evidence_anchored_at(span).find(|record| {
        record.anchor == EvidenceAnchor::binding(span, local_hash)
            && matches!(record.kind, EvidenceKind::Domain(domain) if accepts(domain))
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

fn receiver_same_scope_binding_lhs(
    il: &nose_il::Il,
    interner: &Interner,
    receiver: NodeId,
) -> Option<NodeId> {
    if il.kind(receiver) != NodeKind::Var {
        return None;
    }
    let receiver_hash = node_name_hash(il, interner, receiver)?;
    let scope = il.nearest_scope(receiver)?;
    let mut found = None;
    for &assign in il.assigns_in_scope(Some(scope)) {
        if il.node(assign).span.end_byte > il.node(receiver).span.start_byte {
            continue;
        }
        let Some(&lhs) = il.children(assign).first() else {
            continue;
        };
        if il.kind(lhs) != NodeKind::Var || node_name_hash(il, interner, lhs) != Some(receiver_hash)
        {
            continue;
        }
        match found {
            None => found = Some(lhs),
            Some(existing) if existing == lhs => {}
            Some(_) => return None,
        }
    }
    found
}

fn receiver_param_domain_record<'a>(
    il: &'a nose_il::Il,
    receiver: NodeId,
    accepts: &impl Fn(DomainEvidence) -> bool,
) -> Option<&'a EvidenceRecord> {
    let span = java_receiver_param_span(il, receiver)?;
    il.evidence_anchored_at(span).find(|record| {
        record.anchor == EvidenceAnchor::param(span)
            && matches!(record.kind, EvidenceKind::Domain(domain) if accepts(domain))
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}

fn node_name_hash(il: &nose_il::Il, interner: &Interner, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(stable_symbol_hash(interner.resolve(symbol))),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|symbol| stable_symbol_hash(interner.resolve(*symbol))),
        _ => None,
    }
}

fn java_type_name_shadowed_at_span(
    il: &nose_il::Il,
    interner: &Interner,
    span: Span,
    type_name: &str,
) -> bool {
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == span.file
            && unit.kind == UnitKind::Class
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == type_name)
    })
}

fn java_receiver_param_span(il: &nose_il::Il, receiver: NodeId) -> Option<Span> {
    match il.node(receiver).payload {
        Payload::Name(name) => java_nearest_named_param_span(il, receiver, name),
        _ => None,
    }
}

fn java_nearest_named_param_span(il: &nose_il::Il, receiver: NodeId, name: Symbol) -> Option<Span> {
    let target = il.node(receiver).span;
    let mut best: Option<(u32, Span)> = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if !matches!(candidate.kind, NodeKind::Func | NodeKind::Lambda) {
            continue;
        }
        if candidate.span.file != target.file
            || candidate.span.start_byte > target.start_byte
            || target.end_byte > candidate.span.end_byte
        {
            continue;
        }
        let scope = NodeId(idx as u32);
        let Some(param) = il.children(scope).iter().copied().find(|&child| {
            il.kind(child) == NodeKind::Param && il.node(child).payload == Payload::Name(name)
        }) else {
            continue;
        };
        let width = candidate
            .span
            .end_byte
            .saturating_sub(candidate.span.start_byte);
        let span = il.node(param).span;
        if best.is_none_or(|(best_width, _)| width < best_width) {
            best = Some((width, span));
        }
    }
    best.map(|(_, span)| span)
}

fn java_completion_stage_type_import_dependency(
    il: &nose_il::Il,
    dependency: EvidenceId,
) -> Option<&'static str> {
    java_concurrent_type_import_dependency(
        il,
        dependency,
        &[COMPLETABLE_FUTURE_TYPE, COMPLETION_STAGE_TYPE],
    )
}

fn java_future_handle_type_import_dependency(
    il: &nose_il::Il,
    dependency: EvidenceId,
) -> Option<&'static str> {
    java_concurrent_type_import_dependency(
        il,
        dependency,
        &[COMPLETABLE_FUTURE_TYPE, FUTURE_TYPE, SCHEDULED_FUTURE_TYPE],
    )
}

fn java_completable_future_type_import_dependency(
    il: &nose_il::Il,
    dependency: EvidenceId,
) -> Option<&'static str> {
    java_concurrent_type_import_dependency(il, dependency, &[COMPLETABLE_FUTURE_TYPE])
}

fn java_executor_type_import_dependency(
    il: &nose_il::Il,
    dependency: EvidenceId,
    kind: JavaExecutorKind,
) -> Option<&'static str> {
    let expected = match kind {
        JavaExecutorKind::Executor => EXECUTOR_TYPE,
        JavaExecutorKind::ExecutorService => EXECUTOR_SERVICE_TYPE,
        JavaExecutorKind::ScheduledExecutorService => SCHEDULED_EXECUTOR_SERVICE_TYPE,
    };
    java_concurrent_type_import_dependency(il, dependency, &[expected])
}

fn java_concurrent_type_import_dependency(
    il: &nose_il::Il,
    dependency: EvidenceId,
    supported_types: &[&'static str],
) -> Option<&'static str> {
    let record = il.evidence.get(dependency.0 as usize)?;
    let expected_module_hash = stable_symbol_hash(JAVA_CONCURRENT_MODULE);
    let EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash,
        exported_hash,
    }) = record.kind
    else {
        return None;
    };
    if module_hash != expected_module_hash
        || record.status != EvidenceStatus::Asserted
        || record.provenance.emitter != EvidenceEmitter::Builtin
        || !il.evidence_dependencies_asserted(record)
    {
        return None;
    }
    supported_types
        .iter()
        .copied()
        .find(|supported| exported_hash == stable_symbol_hash(supported))
}

fn java_executor_kind_from_type_hash(type_hash: u64) -> Option<JavaExecutorKind> {
    if type_hash == stable_symbol_hash("java.util.concurrent.Executor") {
        return Some(JavaExecutorKind::Executor);
    }
    if type_hash == stable_symbol_hash("java.util.concurrent.ExecutorService") {
        return Some(JavaExecutorKind::ExecutorService);
    }
    if type_hash == stable_symbol_hash("java.util.concurrent.ScheduledExecutorService") {
        return Some(JavaExecutorKind::ScheduledExecutorService);
    }
    None
}
