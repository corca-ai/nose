use super::*;

pub(super) fn strict_exact_call_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node)
        .iter()
        .skip(1)
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

pub(super) fn strict_exact_callee_identity(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    call: NodeId,
    callee: NodeId,
) -> bool {
    let target_status = call_target_evidence_status_at_call(il, interner, call);
    match il.kind(callee) {
        NodeKind::Var => match target_status {
            CallTargetEvidenceStatus::Rejected => false,
            CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::ImportedFunction {
                ..
            })
            | CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::ImportedMember {
                ..
            }) => true,
            CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DirectFunction {
                ..
            }) => facts.direct_function_target_at_call(il, interner, call),
            CallTargetEvidenceStatus::Admitted(
                CallTargetEvidenceKind::DirectMethod { .. }
                | CallTargetEvidenceKind::DynamicDispatch { .. },
            ) => false,
            CallTargetEvidenceStatus::Missing => {
                strict_exact_safe_var(il, facts, callee)
                    || facts.direct_function_target_at_call(il, interner, call)
            }
        },
        NodeKind::Field => {
            let exact_receiver = il.children(callee).first().is_some_and(|&receiver| {
                strict_exact_callee_receiver_identity(il, facts, receiver)
            });
            if !matches!(il.node(callee).payload, Payload::Name(_)) {
                return false;
            }
            match target_status {
                CallTargetEvidenceStatus::Rejected => false,
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::ImportedMember {
                    ..
                }) => true,
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DirectMethod {
                    ..
                }) => exact_receiver && facts.direct_method_target_at_call(il, interner, call),
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DynamicDispatch {
                    ..
                }) => exact_receiver,
                CallTargetEvidenceStatus::Admitted(
                    CallTargetEvidenceKind::DirectFunction { .. }
                    | CallTargetEvidenceKind::ImportedFunction { .. },
                ) => false,
                CallTargetEvidenceStatus::Missing => exact_receiver,
            }
        }
        _ => false,
    }
}

fn strict_exact_callee_receiver_identity(il: &Il, facts: &StrictFacts, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        NodeKind::Field => {
            matches!(il.node(node).payload, Payload::Name(_))
                && il.children(node).first().is_some_and(|&receiver| {
                    strict_exact_callee_receiver_identity(il, facts, receiver)
                })
        }
        _ => false,
    }
}
