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

/// Record whether each source parameter has plain binding syntax. The source-shape evidence stays
/// outside the node tree so an oracle-only arity proof cannot alter product fingerprints.
pub(super) fn plain_source_parameters(il: &nose_il::Il, root: nose_il::NodeId) -> Vec<bool> {
    il.children(root)
        .iter()
        .copied()
        .filter(|&node| il.kind(node) == nose_il::NodeKind::Param)
        .map(|node| {
            !il.evidence_anchored_at(il.node(node).span).any(|record| {
                record.status == nose_il::EvidenceStatus::Asserted
                    && record.kind
                        == nose_il::EvidenceKind::ParameterShape(
                            nose_il::ParameterShapeEvidenceKind::NonPlain,
                        )
            })
        })
        .collect()
}

/// Prove the only arity relaxation used by the whole-function oracle: an unread suffix of plain
/// parameters. The walk includes nested functions, so a closure capture counts as a read. A
/// leading or interior unused parameter is retained to preserve positional binding. A missing or
/// mismatched source-shape contract fails closed.
pub(super) fn trailing_unused_input_projections(
    il: &nose_il::Il,
    root: nose_il::NodeId,
    plain_source: &[bool],
) -> Vec<nose_detect::OracleInputProjection> {
    let params: Vec<_> = il
        .children(root)
        .iter()
        .copied()
        .filter(|&node| il.kind(node) == nose_il::NodeKind::Param)
        .collect();
    let mut read_cids = std::collections::HashSet::new();
    let mut read_names = std::collections::HashSet::new();
    let mut visited = vec![false; il.nodes.len()];
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if std::mem::replace(&mut visited[node.0 as usize], true) {
            continue;
        }
        if il.kind(node) == nose_il::NodeKind::Var {
            match il.node(node).payload {
                nose_il::Payload::Cid(cid) => {
                    read_cids.insert(cid);
                }
                nose_il::Payload::Name(name) => {
                    read_names.insert(name);
                }
                _ => {}
            }
        }
        stack.extend(il.children(node).iter().copied());
    }

    let mut projections = vec![nose_detect::OracleInputProjection::Declared; params.len()];
    if plain_source.len() != params.len() {
        return projections;
    }
    for (index, param) in params.iter().enumerate().rev() {
        if !plain_source[index] || !il.children(*param).is_empty() {
            break;
        }
        let nose_il::Payload::Cid(cid) = il.node(*param).payload else {
            break;
        };
        let captured_by_name = il
            .cid_names
            .get(cid as usize)
            .is_some_and(|name| read_names.contains(name));
        if read_cids.contains(&cid) || captured_by_name {
            break;
        }
        projections[index] = nose_detect::OracleInputProjection::UnusedTrailing;
    }
    projections
}

/// Stable hash of a unit's effective parameter-domain contract (position-sensitive), retained in
/// JSON reports as a compact compatibility identifier. Hard-gate comparisons use the full
/// validated contract rather than this hash.
pub(super) fn param_domain_signature(
    domains: &[Option<nose_il::DomainEvidence>],
    projections: &[nose_detect::OracleInputProjection],
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let Some((domains, projections)) =
        crate::falsify::effective_domain_contract(domains, projections)
    else {
        0x494e_5641_4c49_4401u64.hash(&mut h);
        domains.hash(&mut h);
        projections.hash(&mut h);
        return h.finish();
    };
    for domain in domains {
        match domain {
            Some(d) => d.hash(&mut h),
            None => 0xD07Fu16.hash(&mut h),
        }
    }
    if projections
        .iter()
        .any(|projection| *projection != nose_detect::OracleInputProjection::Declared)
    {
        0x5052_4f4a_4543_5401u64.hash(&mut h);
        projections.hash(&mut h);
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

#[cfg(test)]
mod tests {
    use super::{plain_source_parameters, trailing_unused_input_projections};
    use nose_detect::OracleInputProjection::{Declared, UnusedTrailing};
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, NodeKind, Payload, Span};

    fn projections(reads: &[u32], nested: bool) -> Vec<nose_detect::OracleInputProjection> {
        let span = Span::synthetic(FileId(0));
        let mut builder = IlBuilder::new(FileId(0));
        let first = builder.add(NodeKind::Param, Payload::Cid(0), span, &[]);
        let second = builder.add(NodeKind::Param, Payload::Cid(1), span, &[]);
        let vars: Vec<_> = reads
            .iter()
            .map(|&cid| builder.add(NodeKind::Var, Payload::Cid(cid), span, &[]))
            .collect();
        let body = builder.add(NodeKind::Block, Payload::None, span, &vars);
        let body = if nested {
            builder.add(NodeKind::Func, Payload::None, span, &[body])
        } else {
            body
        };
        let root = builder.add(NodeKind::Func, Payload::None, span, &[first, second, body]);
        let il = builder.finish(
            root,
            FileMeta {
                path: "unused-tail.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        trailing_unused_input_projections(&il, root, &[true, true])
    }

    #[test]
    fn only_an_unread_trailing_suffix_is_erased() {
        assert_eq!(projections(&[0], false), vec![Declared, UnusedTrailing]);
        assert_eq!(projections(&[1], false), vec![Declared, Declared]);
        assert_eq!(projections(&[0, 1], false), vec![Declared, Declared]);
    }

    #[test]
    fn a_nested_capture_keeps_the_trailing_parameter() {
        assert_eq!(projections(&[1], true), vec![Declared, Declared]);
    }

    #[test]
    fn non_plain_parameter_syntax_keeps_an_unread_trailing_parameter() {
        let span = Span::synthetic(FileId(0));
        let mut builder = IlBuilder::new(FileId(0));
        let first = builder.add(NodeKind::Param, Payload::Cid(0), span, &[]);
        let marker = builder.add(NodeKind::Raw, Payload::None, span, &[]);
        let second = builder.add(NodeKind::Param, Payload::Cid(1), span, &[marker]);
        let read = builder.add(NodeKind::Var, Payload::Cid(0), span, &[]);
        let body = builder.add(NodeKind::Return, Payload::None, span, &[read]);
        let root = builder.add(NodeKind::Func, Payload::None, span, &[first, second, body]);
        let il = builder.finish(
            root,
            FileMeta {
                path: "default.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            trailing_unused_input_projections(&il, root, &[true, false]),
            vec![Declared, Declared]
        );
    }

    #[test]
    fn a_lowered_nested_capture_keeps_the_trailing_parameter() {
        let interner = nose_il::Interner::default();
        let raw = nose_frontend::lower_source(
            FileId(0),
            "nested.py",
            b"def outer(value, trailing):\n    def inner():\n        return trailing\n    return value\n",
            Lang::Python,
            &interner,
        )
        .expect("lower Python nested capture");
        let normalized = nose_normalize::normalize(
            &raw,
            &interner,
            &nose_normalize::NormalizeOptions {
                oracle: true,
                ..nose_normalize::NormalizeOptions::default()
            },
        );
        let outer = normalized
            .units
            .iter()
            .find(|unit| {
                unit.name
                    .is_some_and(|name| interner.resolve(name) == "outer")
            })
            .expect("outer function unit");
        assert_eq!(
            trailing_unused_input_projections(&normalized, outer.root, &[true, true]),
            vec![Declared, Declared]
        );
    }

    #[test]
    fn source_parameter_shapes_survive_the_projection_boundary() {
        let cases = [
            (
                "default.py",
                Lang::Python,
                "long",
                "def long(value, trailing=side_effect()):\n    return value\n",
            ),
            (
                "modified.swift",
                Lang::Swift,
                "long",
                "func long(_ value: String, _ trailing: inout String) -> String { value }\n",
            ),
        ];
        for (path, lang, name, source) in cases {
            let interner = nose_il::Interner::default();
            let raw =
                nose_frontend::lower_source(FileId(0), path, source.as_bytes(), lang, &interner)
                    .expect("lower non-plain parameter");
            let raw_unit = raw
                .units
                .iter()
                .find(|unit| {
                    unit.name
                        .is_some_and(|symbol| interner.resolve(symbol) == name)
                })
                .expect("raw named function unit");
            let plain_source = plain_source_parameters(&raw, raw_unit.root);
            let normalized = nose_normalize::normalize(
                &raw,
                &interner,
                &nose_normalize::NormalizeOptions {
                    oracle: true,
                    ..nose_normalize::NormalizeOptions::default()
                },
            );
            let unit = normalized
                .units
                .iter()
                .find(|unit| {
                    unit.name
                        .is_some_and(|symbol| interner.resolve(symbol) == name)
                })
                .expect("named function unit");
            assert_eq!(
                trailing_unused_input_projections(&normalized, unit.root, &plain_source),
                vec![Declared, Declared],
                "{path} must retain its source-level parameter shape"
            );
            if lang == Lang::Python {
                assert!(
                    normalized
                        .children(unit.root)
                        .iter()
                        .copied()
                        .filter(|&node| normalized.kind(node) == NodeKind::Param)
                        .all(|node| normalized.children(node).is_empty()),
                    "Python source-shape evidence must stay outside the product node tree"
                );
            }
        }
    }
}
