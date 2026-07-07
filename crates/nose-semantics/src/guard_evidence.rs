//! Guard sequence evidence admission.

use super::*;

pub(super) fn guard_evidence_at_sequence_span(
    il: &Il,
    span: Span,
) -> EvidenceResolution<GuardEvidenceKind> {
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !matches!(record.anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span)
        {
            continue;
        }
        let EvidenceKind::Guard(kind) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || !guard_evidence_dependencies_valid(il, record, kind, span)
        {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(kind),
            Some(existing) if existing == kind => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

pub(super) fn guard_evidence_dependencies_valid(
    il: &Il,
    record: &EvidenceRecord,
    kind: GuardEvidenceKind,
    span: Span,
) -> bool {
    match kind {
        GuardEvidenceKind::JsRecordShape { null_check, .. } => {
            js_record_shape_guard_dependencies_valid(il, record, null_check, span)
        }
        GuardEvidenceKind::JsOwnProperty { api_path_hash } => {
            js_own_property_guard_dependencies_valid(il, record, api_path_hash, span)
        }
        GuardEvidenceKind::BoundOrder { .. } => {
            record.status == EvidenceStatus::Asserted
                && language_core_record_has_provenance(il, record)
                && il.evidence_dependencies_asserted(record)
        }
    }
}

pub(super) fn js_record_shape_guard_dependencies_valid(
    il: &Il,
    record: &EvidenceRecord,
    null_check: nose_il::JsRecordGuardNullCheck,
    span: Span,
) -> bool {
    let mut has_array_is_array = false;
    let mut has_boolean = null_check != nose_il::JsRecordGuardNullCheck::BooleanGlobalTruthy;
    for id in &record.dependencies {
        let Some(dependency) = il.evidence_record_by_id(*id) else {
            return false;
        };
        if dependency.id != *id || !dependency.anchor.matches_span(span) {
            return false;
        }
        match dependency.kind {
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                if path_hash == stable_symbol_hash("Array.isArray")
                    && qualified_global_dependency_valid(il, dependency, span, "Array.isArray") =>
            {
                has_array_is_array = true;
            }
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                if null_check == nose_il::JsRecordGuardNullCheck::BooleanGlobalTruthy
                    && name_hash == stable_symbol_hash("Boolean")
                    && dependency.status == EvidenceStatus::Asserted
                    && il.evidence_dependencies_asserted(dependency) =>
            {
                has_boolean = true;
            }
            _ => return false,
        }
    }
    has_array_is_array && has_boolean
}

pub(super) fn js_own_property_guard_api_path(path_hash: u64) -> Option<&'static str> {
    if path_hash == stable_symbol_hash("Object.hasOwn") {
        Some("Object.hasOwn")
    } else if path_hash == stable_symbol_hash("Object.prototype.hasOwnProperty.call") {
        Some("Object.prototype.hasOwnProperty.call")
    } else {
        None
    }
}

pub(super) fn js_own_property_guard_dependencies_valid(
    il: &Il,
    record: &EvidenceRecord,
    api_path_hash: u64,
    span: Span,
) -> bool {
    let Some(api_path) = js_own_property_guard_api_path(api_path_hash) else {
        return false;
    };
    let mut has_api = false;
    for id in &record.dependencies {
        let Some(dependency) = il.evidence_record_by_id(*id) else {
            return false;
        };
        if dependency.id != *id || !dependency.anchor.matches_span(span) {
            return false;
        }
        match dependency.kind {
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                if path_hash == api_path_hash
                    && qualified_global_dependency_valid(il, dependency, span, api_path) =>
            {
                has_api = true;
            }
            _ => return false,
        }
    }
    has_api
}

/// Prove that a lowered `Seq("record_guard")` denotes the first-party JS-like
/// record-shape guard contract. The surface tag is not enough: the sequence must
/// carry both matching sequence-surface evidence and a dedicated guard evidence
/// record whose dependencies are asserted.
pub fn record_shape_guard_for_node(il: &Il, interner: &Interner, node: NodeId) -> bool {
    record_shape_guard_evidence_for_node(il, interner, node).is_some()
}

pub fn record_shape_guard_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GuardEvidenceKind> {
    if il.kind(node) != NodeKind::Seq || !js_like_lang(il.meta.lang) {
        return None;
    }
    let span = il.node(node).span;
    if !matches!(
        sequence_surface_evidence_at_sequence_span(il, span),
        EvidenceResolution::Found(SequenceSurfaceKind::RecordGuard)
    ) {
        return None;
    }
    match guard_evidence_at_sequence_span(il, span) {
        EvidenceResolution::Found(
            evidence @ GuardEvidenceKind::JsRecordShape { subject_hash, .. },
        ) if record_shape_guard_sequence_matches(il, interner, node, subject_hash) => {
            Some(evidence)
        }
        EvidenceResolution::Found(_)
        | EvidenceResolution::Ambiguous
        | EvidenceResolution::Missing => None,
    }
}

pub(super) fn record_shape_guard_sequence_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    subject_hash: u64,
) -> bool {
    let Payload::Name(tag) = il.node(node).payload else {
        return false;
    };
    if sequence_surface_kind_for_tag(il.meta.lang, Some(interner.resolve(tag)))
        != Some(SequenceSurfaceKind::RecordGuard)
    {
        return false;
    }
    let [subject, object, non_null, not_array] = il.children(node) else {
        return false;
    };
    record_shape_guard_subject_matches(il, interner, *subject, subject_hash)
        && literal_string_hash(il, *object) == Some(stable_symbol_hash("object"))
        && literal_string_hash(il, *non_null) == Some(stable_symbol_hash("non_null"))
        && literal_string_hash(il, *not_array) == Some(stable_symbol_hash("not_array"))
}

pub(super) fn record_shape_guard_subject_matches(
    il: &Il,
    interner: &Interner,
    subject: NodeId,
    subject_hash: u64,
) -> bool {
    if il.kind(subject) != NodeKind::Var {
        return false;
    }
    match il.node(subject).payload {
        Payload::Name(_) => node_name_hash(il, interner, subject) == Some(subject_hash),
        Payload::Cid(_) => true,
        _ => false,
    }
}

/// Prove that a lowered `Seq("own_property_guard")` denotes a first-party
/// JS-like own-property test such as `Object.hasOwn(obj, key)`. The surface tag
/// is not enough: exact consumers require matching sequence evidence, dedicated
/// guard evidence, and a supported qualified-global API dependency.
pub fn own_property_guard_for_node(il: &Il, interner: &Interner, node: NodeId) -> bool {
    own_property_guard_evidence_for_node(il, interner, node).is_some()
}

pub fn own_property_guard_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GuardEvidenceKind> {
    if il.kind(node) != NodeKind::Seq || !js_like_lang(il.meta.lang) {
        return None;
    }
    let span = il.node(node).span;
    if !matches!(
        sequence_surface_evidence_at_sequence_span(il, span),
        EvidenceResolution::Found(SequenceSurfaceKind::OwnPropertyGuard)
    ) {
        return None;
    }
    match guard_evidence_at_sequence_span(il, span) {
        EvidenceResolution::Found(evidence @ GuardEvidenceKind::JsOwnProperty { .. })
            if own_property_guard_sequence_matches(il, interner, node) =>
        {
            Some(evidence)
        }
        EvidenceResolution::Found(_)
        | EvidenceResolution::Ambiguous
        | EvidenceResolution::Missing => None,
    }
}

pub fn own_property_guard_evidence_at_span(il: &Il, span: Span) -> bool {
    if !js_like_lang(il.meta.lang)
        || !matches!(
            sequence_surface_evidence_at_sequence_span(il, span),
            EvidenceResolution::Found(SequenceSurfaceKind::OwnPropertyGuard)
        )
    {
        return false;
    }
    matches!(
        guard_evidence_at_sequence_span(il, span),
        EvidenceResolution::Found(GuardEvidenceKind::JsOwnProperty { .. })
    )
}

fn bound_order_guard_evidence_at_node(
    il: &Il,
    node: NodeId,
    activation: BoundOrderGuardActivation,
) -> EvidenceResolution<GuardEvidenceKind> {
    if il.kind(node) != NodeKind::BinOp {
        return EvidenceResolution::Missing;
    }
    let span = il.node(node).span;
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !matches!(
            record.anchor,
            EvidenceAnchor::Node {
                span: anchor_span,
                kind: NodeKind::BinOp,
            } if anchor_span == span
        ) {
            continue;
        }
        let EvidenceKind::Guard(
            evidence @ GuardEvidenceKind::BoundOrder {
                activation: evidence_activation,
                ..
            },
        ) = record.kind
        else {
            continue;
        };
        if evidence_activation != activation {
            continue;
        }
        if !guard_evidence_dependencies_valid(il, record, evidence, span) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(evidence),
            Some(existing) if existing == evidence => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

/// Prove that the branch selected by `activation` establishes a non-strict
/// lower-before-upper ordering. The evidence must be an asserted language-core
/// guard record anchored on the exact comparison node and must name the exact
/// operand spans; parameter names and unrelated guards are not proof.
pub fn bound_order_guard_for_node(
    il: &Il,
    node: NodeId,
    activation: BoundOrderGuardActivation,
) -> Option<(NodeId, NodeId)> {
    let (lower, upper) = bound_order_operands_from_condition(il, node, activation)?;
    if !bound_order_operand_admitted(il, lower) || !bound_order_operand_admitted(il, upper) {
        return None;
    }
    match bound_order_guard_evidence_at_node(il, node, activation) {
        EvidenceResolution::Found(GuardEvidenceKind::BoundOrder {
            lower_span,
            upper_span,
            activation: evidence_activation,
        }) if evidence_activation == activation
            && il.node(lower).span == lower_span
            && il.node(upper).span == upper_span =>
        {
            Some((lower, upper))
        }
        EvidenceResolution::Found(_)
        | EvidenceResolution::Ambiguous
        | EvidenceResolution::Missing => None,
    }
}

fn bound_order_operands_from_condition(
    il: &Il,
    node: NodeId,
    activation: BoundOrderGuardActivation,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::BinOp {
        return None;
    }
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

fn bound_order_operand_admitted(il: &Il, node: NodeId) -> bool {
    matches!(il.kind(node), NodeKind::Var)
        || matches!(
            (il.kind(node), il.node(node).payload),
            (NodeKind::Lit, Payload::LitInt(_))
        )
}

pub(super) fn own_property_guard_sequence_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    let Payload::Name(tag) = il.node(node).payload else {
        return false;
    };
    if sequence_surface_kind_for_tag(il.meta.lang, Some(interner.resolve(tag)))
        != Some(SequenceSurfaceKind::OwnPropertyGuard)
    {
        return false;
    }
    let [_, _, own, present] = il.children(node) else {
        return false;
    };
    literal_string_hash(il, *own) == Some(stable_symbol_hash("own"))
        && literal_string_hash(il, *present) == Some(stable_symbol_hash("present"))
}
