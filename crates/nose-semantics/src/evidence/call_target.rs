use super::*;

pub fn direct_function_call_target_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    target_root: NodeId,
) -> bool {
    if il.kind(target_root) != NodeKind::Func {
        return false;
    }
    direct_function_call_target_span_at_call(il, interner, call)
        .is_some_and(|proven_span| proven_span == il.node(target_root).span)
}

/// The proven `DirectFunction` target span at `call`, when the call carries a
/// unique admitted builtin language-core `CallTarget::DirectFunction` record. The
/// span-returning form lets a consumer with many possible targets resolve the
/// evidence once and look the target up, instead of re-resolving per target.
pub fn direct_function_call_target_span_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<Span> {
    match call_target_evidence_status_at_call(il, interner, call) {
        CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DirectFunction {
            target_span: proven_span,
            ..
        }) => Some(proven_span),
        CallTargetEvidenceStatus::Admitted(
            CallTargetEvidenceKind::DirectMethod { .. }
            | CallTargetEvidenceKind::ImportedFunction { .. }
            | CallTargetEvidenceKind::ImportedMember { .. }
            | CallTargetEvidenceKind::DynamicDispatch { .. },
        )
        | CallTargetEvidenceStatus::Missing
        | CallTargetEvidenceStatus::Rejected => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallTargetEvidenceStatus {
    Missing,
    Admitted(CallTargetEvidenceKind),
    Rejected,
}

pub fn call_target_evidence_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<CallTargetEvidenceKind> {
    match call_target_evidence_status_at_call(il, interner, call) {
        CallTargetEvidenceStatus::Admitted(target) => Some(target),
        CallTargetEvidenceStatus::Missing | CallTargetEvidenceStatus::Rejected => None,
    }
}

pub fn call_target_evidence_status_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> CallTargetEvidenceStatus {
    if il.kind(call) != NodeKind::Call {
        return CallTargetEvidenceStatus::Missing;
    }
    let target = match language_core_call_target_evidence_at_call(il, call) {
        EvidenceResolution::Found(target) => target,
        EvidenceResolution::Ambiguous => return CallTargetEvidenceStatus::Rejected,
        EvidenceResolution::Missing => return CallTargetEvidenceStatus::Missing,
    };
    if call_target_matches_call_shape(il, interner, call, target) {
        CallTargetEvidenceStatus::Admitted(target)
    } else {
        CallTargetEvidenceStatus::Rejected
    }
}

fn language_core_call_target_evidence_at_call(
    il: &Il,
    call: NodeId,
) -> EvidenceResolution<CallTargetEvidenceKind> {
    let call_span = il.node(call).span;
    let expected_provenance = language_core_call_target_provenance(il);
    let mut found = None;
    for record in il.evidence_anchored_at(call_span) {
        if !matches!(
            record.anchor,
            EvidenceAnchor::Node { span, kind } if span == call_span && kind == NodeKind::Call
        ) {
            continue;
        }
        let EvidenceKind::CallTarget(target) = record.kind else {
            continue;
        };
        if record.provenance != expected_provenance {
            continue;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(target),
            Some(existing) if existing == target => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn language_core_call_target_provenance(il: &Il) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

pub fn imported_function_call_target_at_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::ImportedFunction { .. })
    )
}

pub fn imported_member_call_target_at_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::ImportedMember { .. })
    )
}

pub fn direct_method_call_target_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    target_root: NodeId,
) -> bool {
    if il.kind(call) != NodeKind::Call || il.kind(target_root) != NodeKind::Func {
        return false;
    }
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::DirectMethod { target_span, .. })
            if target_span == il.node(target_root).span
    )
}

fn call_target_matches_call_shape(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    target: CallTargetEvidenceKind,
) -> bool {
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    match target {
        CallTargetEvidenceKind::DirectFunction { name_hash, .. }
        | CallTargetEvidenceKind::ImportedFunction {
            local_hash: name_hash,
            ..
        } => var_selector_matches_if_available(il, interner, callee, name_hash),
        CallTargetEvidenceKind::DirectMethod { method_hash, .. }
        | CallTargetEvidenceKind::DynamicDispatch { method_hash, .. } => {
            field_selector_matches(il, interner, callee, method_hash)
        }
        CallTargetEvidenceKind::ImportedMember { member_hash, .. } => {
            field_selector_matches(il, interner, callee, member_hash)
                || scoped_var_suffix_matches(il, interner, callee, member_hash)
        }
    }
}

fn var_selector_matches_if_available(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_hash: u64,
) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    match il.node(callee).payload {
        Payload::Name(name) => interner.symbol_hash(name) == expected_hash,
        Payload::Cid(_) => true,
        _ => false,
    }
}

fn field_selector_matches(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_hash: u64,
) -> bool {
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    match il.node(callee).payload {
        Payload::Name(name) => interner.symbol_hash(name) == expected_hash,
        _ => false,
    }
}

fn scoped_var_suffix_matches(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_hash: u64,
) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    match il.node(callee).payload {
        Payload::Name(name) => interner
            .resolve(name)
            .split_once("::")
            .is_some_and(|(_, suffix)| stable_symbol_hash(suffix) == expected_hash),
        _ => false,
    }
}
