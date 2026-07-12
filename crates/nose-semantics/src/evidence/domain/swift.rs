use super::*;

/// Whether a parameter has a unique, dependency-live language-core proof that
/// it is attribute/modifier-free and uses Swift bracket-array syntax (`[T]`).
pub fn swift_bracket_array_parameter_proven(il: &Il, param: NodeId) -> bool {
    if il.kind(param) != NodeKind::Param {
        return false;
    }
    let span = il.node(param).span;
    matches!(
        unique_asserted_record_evidence_at(
            il,
            span,
            |anchor| anchor == EvidenceAnchor::param(span),
            |record| match record.kind {
                EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter) => {
                    Some(language_core_record_has_provenance(il, record))
                }
                _ => None,
            },
        ),
        EvidenceResolution::Found(true)
    )
}

/// The immutable parameter denoted by a direct variable reference.
///
/// Canonical-id references use the same lexical scope and reassignment checks
/// as receiver-domain resolution. Raw-name references additionally require one
/// nearest parameter with no assignment in its owning scope. Callers use this
/// for proof coordinates whose evaluation must be a stable parameter read.
fn immutable_parameter_for_reference(il: &Il, reference: NodeId) -> Option<NodeId> {
    if il.kind(reference) != NodeKind::Var {
        return None;
    }
    match il.node(reference).payload {
        Payload::Cid(cid) => {
            let span = receiver_cid_param_span(il, reference, cid)?;
            unique_param_at_span(il, span)
        }
        Payload::Name(name) => {
            let (scope, param) = nearest_named_param_scope(il, reference, name)?;
            (!name_is_assigned_in_scope(il, name, scope)).then_some(param)
        }
        _ => None,
    }
}

/// The plain Swift parameter denoted by a direct immutable variable reference.
///
/// The Swift frontend gives attributed, modified, and parser-recovered
/// parameters a shape child. Requiring an empty parameter shape prevents
/// property wrappers and ownership/inout modifiers from borrowing a stable
/// source-coordinate proof.
pub(crate) fn swift_plain_parameter_for_reference(il: &Il, reference: NodeId) -> Option<NodeId> {
    if il.meta.lang != Lang::Swift {
        return None;
    }
    let param = immutable_parameter_for_reference(il, reference)?;
    il.children(param).is_empty().then_some(param)
}

fn unique_param_at_span(il: &Il, span: Span) -> Option<NodeId> {
    let mut found = None;
    for param in il
        .nodes
        .iter()
        .enumerate()
        .map(|(index, _)| NodeId(index as u32))
        .filter(|&node| il.kind(node) == NodeKind::Param && il.node(node).span == span)
    {
        if found.replace(param).is_some() {
            return None;
        }
    }
    found
}

/// Whether a Swift parameter has a live language-core proof for one of the
/// controlled Dictionary type spellings. Non-plain parameters never receive
/// this evidence, and corpus-visible shadow/dispatch analysis can tombstone it.
fn swift_dictionary_parameter_proven(il: &Il, interner: &Interner, param: NodeId) -> bool {
    if il.meta.lang != Lang::Swift || il.kind(param) != NodeKind::Param {
        return false;
    }
    if il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if interner.resolve(name)
            == SWIFT_DICTIONARY_DEFAULT_SUBSCRIPT_BARRIER_MARKER)
    }) {
        return false;
    }
    let span = il.node(param).span;
    let kind = match unique_asserted_record_evidence_at(
        il,
        span,
        |anchor| anchor == EvidenceAnchor::param(span),
        |record| match record.kind {
            EvidenceKind::Type(
                kind @ (TypeEvidenceKind::SwiftBracketDictionaryParameter
                | TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter
                | TypeEvidenceKind::SwiftQualifiedDictionaryParameter),
            ) if language_core_record_has_provenance(il, record) => Some(kind),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(kind) => kind,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => return false,
    };
    kind != TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter
        || !swift_dictionary_name_shadowed_in_file(il, interner)
}

fn swift_dictionary_name_shadowed_in_file(il: &Il, interner: &Interner) -> bool {
    il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|name| swift_identifier_matches(interner.resolve(name), "Dictionary"))
    }) || il.nodes.iter().enumerate().any(|(index, node)| {
        node.kind == NodeKind::Block
            && il.children(NodeId(index as u32)).is_empty()
            && matches!(node.payload, Payload::Name(name) if swift_identifier_matches(
                interner.resolve(name),
                "Dictionary",
            ))
    })
}

/// Whether a direct Swift variable reference denotes a proven, immutable
/// language-core Dictionary parameter.
pub(crate) fn swift_dictionary_parameter_reference_proven(
    il: &Il,
    interner: &Interner,
    reference: NodeId,
) -> bool {
    swift_plain_parameter_for_reference(il, reference)
        .is_some_and(|param| swift_dictionary_parameter_proven(il, interner, param))
}

/// Whether Swift source contains an Array/Collection-domain parameter that is
/// not backed by plain bracket-array syntax. Such a nominal, attributed, or
/// modified parameter can change source identity or route HOF selectors through
/// user-defined dispatch and therefore cannot participate in the controlled
/// compactMap or one-level flatMap equivalence perimeters.
pub fn swift_has_unproven_collection_parameter(il: &Il) -> bool {
    if il.meta.lang != Lang::Swift {
        return false;
    }
    il.nodes
        .iter()
        .enumerate()
        .map(|(index, _)| NodeId(index as u32))
        .filter(|&node| il.kind(node) == NodeKind::Param)
        .any(|param| {
            matches!(
                domain_evidence_for_param(il, param),
                Some(DomainEvidence::Array | DomainEvidence::Collection)
            ) && !swift_bracket_array_parameter_proven(il, param)
        })
}
