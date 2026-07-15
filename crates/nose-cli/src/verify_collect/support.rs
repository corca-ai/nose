use super::*;

pub(super) fn unit_value_fingerprint_and_contracts(
    value_context: Option<&nose_normalize::ValueFingerprintContext>,
    il: &nose_il::Il,
    root: nose_il::NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<(u32, u32)>) {
    match value_context {
        Some(context) => nose_normalize::value_fingerprint_and_contracts_with_context(
            il, root, interner, context,
        ),
        None => nose_normalize::value_fingerprint_and_contracts(il, root, interner),
    }
}

pub(super) fn admission_rejection_for_rec(
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    exact_safe: bool,
    fingerprint_len: usize,
    admission_context: &AdmissionContext,
    raw_source: RuntimeDiagnosticSource<'_>,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    if !exact_safe {
        if let Some(diagnostic) =
            runtime_boundary_diagnostic_from_source(raw_source, interner, admission_context)
        {
            return Some(diagnostic);
        }
    }
    exact_admission_rejection_with_context(
        il,
        interner,
        root,
        exact_safe,
        fingerprint_len,
        admission_context,
    )
}

pub(super) fn oracle_exclusion_diagnostic(
    reason: VerifyExclusionReason,
    raw_source: RuntimeDiagnosticSource<'_>,
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    admission_context: &AdmissionContext,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    match reason {
        VerifyExclusionReason::Uninterpretable => runtime_boundary_diagnostic_with_fallback(
            raw_source,
            il,
            root,
            interner,
            admission_context,
        ),
        VerifyExclusionReason::CoreMissing
        | VerifyExclusionReason::BatteryBail
        | VerifyExclusionReason::EmptyFingerprint
        | VerifyExclusionReason::PathBail => None,
    }
}

fn runtime_boundary_diagnostic_with_fallback(
    raw_source: RuntimeDiagnosticSource<'_>,
    normalized: &nose_il::Il,
    normalized_root: nose_il::NodeId,
    interner: &Interner,
    admission_context: &AdmissionContext,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    runtime_boundary_diagnostic_from_source(raw_source, interner, admission_context).or_else(|| {
        runtime_boundary_rejection_diagnostic_with_context(
            normalized,
            interner,
            normalized_root,
            admission_context,
        )
    })
}

fn runtime_boundary_diagnostic_from_source(
    raw_source: RuntimeDiagnosticSource<'_>,
    interner: &Interner,
    admission_context: &AdmissionContext,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    if !matches!(raw_source.il.meta.lang, Lang::Python | Lang::Rust) {
        return None;
    }
    // Python/Rust async-runtime labels rely on source-level import/shadow facts.
    // Compute them from the raw span-matched unit first so alpha normalization
    // cannot erase alias shadowing evidence.
    raw_source.root.and_then(|root| {
        runtime_boundary_rejection_diagnostic_with_context(
            raw_source.il,
            interner,
            root,
            admission_context,
        )
    })
}

/// A unit's declared parameter domains in source order. Units whose declarations differ are
/// interpreted under different battery coercions and are not behavior-comparable row-for-row.
/// Keep this exact representation for hard-gate decisions: the stable hash below is a compact
/// reporting identifier, not a proof of equality.
pub(super) fn param_domains(
    il: &nose_il::Il,
    root: nose_il::NodeId,
) -> Vec<Option<nose_il::DomainEvidence>> {
    il.children(root)
        .iter()
        .filter(|&&k| il.kind(k) == nose_il::NodeKind::Param)
        .map(|&k| nose_semantics::domain_evidence_for_param(il, k))
        .collect()
}

/// Stable hash of a unit's declared parameter domains (position-sensitive), retained in JSON
/// reports as a compact compatibility identifier. Hard-gate comparisons use [`param_domains`].
pub(super) fn param_domain_signature(domains: &[Option<nose_il::DomainEvidence>]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for domain in domains {
        match domain {
            Some(d) => d.hash(&mut h),
            None => 0xD07Fu16.hash(&mut h),
        }
    }
    h.finish()
}

/// Subtree node count — the same size signal the detector gates on, so the
/// value-add evaluator can restrict its gold to meaningful-size units.
pub(super) fn subtree_node_count(il: &nose_il::Il, root: nose_il::NodeId) -> usize {
    let mut tokens = 0usize;
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        tokens += 1;
        stack.extend(il.children(x).iter().copied());
    }
    tokens
}
