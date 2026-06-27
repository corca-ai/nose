use nose_il::{Interner, NodeId};

#[derive(Clone)]
pub(crate) struct ExactAdmissionRejectionDiagnostic {
    pub(crate) reason: &'static str,
    pub(crate) admission_gate: &'static str,
    pub(crate) capability_id: &'static str,
    pub(crate) pack_id: Option<&'static str>,
    pub(crate) missing_evidence: Vec<&'static str>,
}

pub(crate) fn exact_admission_rejection(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
    exact_safe: bool,
    value_len: usize,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    if exact_safe {
        return (!nose_detect::exact_claim_eligible_parts(true, value_len)).then(|| {
            ExactAdmissionRejectionDiagnostic {
                reason: "value-fingerprint-too-small",
                admission_gate: "exact-claim-value-fingerprint-floor",
                capability_id: "non-degenerate-value-fingerprint",
                pack_id: None,
                missing_evidence: vec!["non-degenerate-value-fingerprint"],
            }
        });
    }

    Some(strict_exact_rejection_reason(il, interner, root))
}

fn strict_exact_rejection_reason(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
) -> ExactAdmissionRejectionDiagnostic {
    if subtree_has(il, root, |il, node| {
        matches!(
            il.kind(node),
            nose_il::NodeKind::Raw
                | nose_il::NodeKind::Try
                | nose_il::NodeKind::Throw
                | nose_il::NodeKind::Splat
                | nose_il::NodeKind::KwArg
        )
    }) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "unsupported-runtime-boundary",
            admission_gate: "strict-exact-safety",
            capability_id: "runtime-boundary-model",
            pack_id: None,
            missing_evidence: vec!["lowered-runtime-boundary-contract"],
        };
    }

    if subtree_has(il, root, |il, node| il.kind(node) == nose_il::NodeKind::HoF) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "hof-demand-effect-proof-missing",
            admission_gate: "strict-exact-hof-demand-effect",
            capability_id: "hof-demand-effect-materialization",
            pack_id: None,
            missing_evidence: vec!["hof-demand-effect-profile"],
        };
    }

    if subtree_has(il, root, effect_boundary_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "mutation-effect-boundary",
            admission_gate: "strict-exact-effect-safety",
            capability_id: "effect-and-place-contract",
            pack_id: None,
            missing_evidence: vec!["effect-preserving-contract"],
        };
    }

    if subtree_has(il, root, builtin_call_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "library-api-occurrence-proof-missing",
            admission_gate: "strict-exact-library-api-occurrence",
            capability_id: "library-api-occurrence",
            pack_id: None,
            missing_evidence: vec!["library-api-occurrence-evidence"],
        };
    }

    if subtree_has(il, root, |il, node| {
        receiver_method_call(il, interner, node)
    }) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "receiver-domain-proof-missing",
            admission_gate: "strict-exact-receiver-domain",
            capability_id: "receiver-domain-evidence",
            pack_id: None,
            missing_evidence: vec!["receiver-domain-proof"],
        };
    }

    if subtree_has(il, root, rust_macro_invocation_call) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "source-surface-proof-missing",
            admission_gate: "strict-exact-source-surface",
            capability_id: "source-surface-evidence",
            pack_id: None,
            missing_evidence: vec!["rust-macro-expansion-contract"],
        };
    }

    if subtree_has(il, root, |il, node| {
        il.kind(node) == nose_il::NodeKind::Call
    }) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "import-symbol-callee-identity-proof-missing",
            admission_gate: "strict-exact-callee-identity",
            capability_id: "callee-identity-evidence",
            pack_id: None,
            missing_evidence: vec!["import-or-call-target-proof"],
        };
    }

    if subtree_has(il, root, source_surface_boundary_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "source-surface-proof-missing",
            admission_gate: "strict-exact-source-surface",
            capability_id: "source-surface-evidence",
            pack_id: None,
            missing_evidence: vec!["source-surface-contract"],
        };
    }

    ExactAdmissionRejectionDiagnostic {
        reason: "unattributed-strict-exact-unsafe",
        admission_gate: "strict-exact-safety",
        capability_id: "exact-semantic-merge",
        pack_id: None,
        missing_evidence: vec!["strict-exact-safe-tree"],
    }
}

fn subtree_has(
    il: &nose_il::Il,
    root: NodeId,
    pred: impl Fn(&nose_il::Il, NodeId) -> bool,
) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if pred(il, node) {
            return true;
        }
        stack.extend(il.children(node).iter().copied());
    }
    false
}

fn effect_boundary_node(il: &nose_il::Il, node: NodeId) -> bool {
    match il.node(node).payload {
        nose_il::Payload::Builtin(nose_il::Builtin::Append | nose_il::Builtin::Print) => true,
        _ => {
            il.kind(node) == nose_il::NodeKind::Assign
                && il.children(node).first().is_some_and(|&lhs| {
                    matches!(
                        il.kind(lhs),
                        nose_il::NodeKind::Field | nose_il::NodeKind::Index
                    )
                })
                || expression_statement_call(il, node)
        }
    }
}

fn builtin_call_node(il: &nose_il::Il, node: NodeId) -> bool {
    il.kind(node) == nose_il::NodeKind::Call
        && matches!(il.node(node).payload, nose_il::Payload::Builtin(_))
}

fn expression_statement_call(il: &nose_il::Il, node: NodeId) -> bool {
    il.kind(node) == nose_il::NodeKind::ExprStmt
        && il.children(node).first().is_some_and(|&expr| {
            subtree_has(il, expr, |il, node| {
                il.kind(node) == nose_il::NodeKind::Call
            })
        })
}

fn receiver_method_call(il: &nose_il::Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != nose_il::NodeKind::Call {
        return false;
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if il.kind(callee) != nose_il::NodeKind::Field {
        return false;
    }
    let nose_il::Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    matches!(
        interner.resolve(method),
        "and_then"
            | "any"
            | "all"
            | "collect"
            | "contains"
            | "end_with?"
            | "endsWith"
            | "filter"
            | "filter_map"
            | "flatMap"
            | "flat_map"
            | "get"
            | "getOrDefault"
            | "is_empty"
            | "isEmpty"
            | "map"
            | "max"
            | "min"
            | "reduce"
            | "reject"
            | "some"
            | "start_with?"
            | "startsWith"
            | "then"
    )
}

fn source_surface_boundary_node(il: &nose_il::Il, node: NodeId) -> bool {
    if rust_macro_invocation_call(il, node) {
        return true;
    }
    matches!(
        il.kind(node),
        nose_il::NodeKind::Seq
            | nose_il::NodeKind::Lambda
            | nose_il::NodeKind::Index
            | nose_il::NodeKind::BinOp
            | nose_il::NodeKind::UnOp
    )
}

fn rust_macro_invocation_call(il: &nose_il::Il, node: NodeId) -> bool {
    il.meta.lang == nose_il::Lang::Rust
        && il.kind(node) == nose_il::NodeKind::Call
        && il.evidence_anchored_at(il.node(node).span).any(|record| {
            matches!(
                record.kind,
                nose_il::EvidenceKind::Source(nose_il::SourceFactKind::Call(
                    nose_il::SourceCallKind::MacroInvocation
                ))
            )
        })
}
