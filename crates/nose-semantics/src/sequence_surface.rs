//! Sequence surface contracts and evidence admission.

use super::*;

pub const SEQ_VALUE_UNTAGGED: u64 = 0;
pub const SEQ_VALUE_COLLECTION: u64 = 1;
pub const SEQ_VALUE_TUPLE: u64 = 2;
pub const SEQ_VALUE_MAP: u64 = 3;
pub const SEQ_VALUE_PAIR: u64 = 4;
pub const SEQ_VALUE_RECORD_GUARD: u64 = 7;
pub const SEQ_VALUE_OWN_PROPERTY_GUARD: u64 = 8;
pub const SEQ_VALUE_RUST_STRUCT_EXPRESSION: u64 = stable_symbol_hash("rust_struct_expression");

/// Kernel contract for a lowered `Seq` surface tag.
///
/// This is deliberately not just a value-graph tag table. The same surface may be
/// exact-safe as a literal, admissible as a membership collection, exportable as an
/// immutable module literal, or none of those. Keeping the axes separate prevents a
/// frontend tag such as Go's `composite_literal` from silently becoming a collection
/// merely because it is represented as `Seq` in IL.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SeqSurfaceContract {
    pub value_tag: u64,
    pub exact_tree_safe: bool,
    pub membership_collection: bool,
    pub map_entry_list: bool,
    pub imported_literal: bool,
}

pub fn sequence_surface_kind_for_tag(lang: Lang, tag: Option<&str>) -> Option<SequenceSurfaceKind> {
    match tag {
        None => Some(SequenceSurfaceKind::Untagged),
        Some("array" | "array_expression" | "list" | "set") => {
            Some(SequenceSurfaceKind::Collection)
        }
        Some("tuple" | "tuple_expression") => Some(SequenceSurfaceKind::Tuple),
        Some("dictionary" | "object" | "hash") => Some(SequenceSurfaceKind::Map),
        Some("pair") => Some(SequenceSurfaceKind::Pair),
        Some("record_guard") => Some(SequenceSurfaceKind::RecordGuard),
        Some("own_property_guard") => Some(SequenceSurfaceKind::OwnPropertyGuard),
        Some("composite_literal") if lang == Lang::Go => {
            Some(SequenceSurfaceKind::GoCompositeMapLiteral)
        }
        Some("keyed_element") if lang == Lang::Go => Some(SequenceSurfaceKind::GoMapEntry),
        Some("rust_struct_expression") if lang == Lang::Rust => {
            Some(SequenceSurfaceKind::RustStructExpression)
        }
        _ => None,
    }
}

pub(super) fn seq_surface_contract_for_tag(
    lang: Lang,
    tag: Option<&str>,
) -> Option<(SequenceSurfaceKind, SeqSurfaceContract)> {
    let kind = sequence_surface_kind_for_tag(lang, tag)?;
    let contract = match kind {
        SequenceSurfaceKind::Untagged => SeqSurfaceContract {
            value_tag: SEQ_VALUE_UNTAGGED,
            exact_tree_safe: false,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::Collection => SeqSurfaceContract {
            value_tag: SEQ_VALUE_COLLECTION,
            exact_tree_safe: true,
            membership_collection: true,
            map_entry_list: true,
            imported_literal: matches!(tag, Some("array" | "array_expression")),
        },
        SequenceSurfaceKind::Tuple => SeqSurfaceContract {
            value_tag: SEQ_VALUE_TUPLE,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: matches!(tag, Some("tuple_expression")),
        },
        SequenceSurfaceKind::Map => SeqSurfaceContract {
            value_tag: SEQ_VALUE_MAP,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: matches!(tag, Some("dictionary" | "object")),
        },
        SequenceSurfaceKind::Pair => SeqSurfaceContract {
            value_tag: SEQ_VALUE_PAIR,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::RecordGuard => SeqSurfaceContract {
            value_tag: SEQ_VALUE_RECORD_GUARD,
            exact_tree_safe: false,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::OwnPropertyGuard => SeqSurfaceContract {
            value_tag: SEQ_VALUE_OWN_PROPERTY_GUARD,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::GoCompositeMapLiteral => SeqSurfaceContract {
            value_tag: stable_symbol_hash("go_composite_map_literal"),
            exact_tree_safe: false,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::GoMapEntry => SeqSurfaceContract {
            value_tag: stable_symbol_hash("keyed_element"),
            exact_tree_safe: false,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::RustStructExpression => SeqSurfaceContract {
            value_tag: SEQ_VALUE_RUST_STRUCT_EXPRESSION,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
    };
    Some((kind, contract))
}

pub fn seq_surface_contract(lang: Lang, tag: Option<&str>) -> Option<SeqSurfaceContract> {
    seq_surface_contract_for_tag(lang, tag).map(|(_, contract)| contract)
}

pub fn seq_surface_contract_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<SeqSurfaceContract> {
    if il.kind(node) != NodeKind::Seq {
        return None;
    }
    let raw_tag = match il.node(node).payload {
        Payload::None => None,
        Payload::Name(name) => Some(interner.resolve(name)),
        _ => return None,
    };
    let (raw_kind, raw_contract) = seq_surface_contract_for_tag(il.meta.lang, raw_tag)?;
    match sequence_surface_evidence_at_sequence_span(il, il.node(node).span) {
        EvidenceResolution::Found(kind) if kind == raw_kind => Some(raw_contract),
        EvidenceResolution::Found(_)
        | EvidenceResolution::Ambiguous
        | EvidenceResolution::Missing => None,
    }
}

pub(super) fn sequence_surface_evidence_at_sequence_span(
    il: &Il,
    span: Span,
) -> EvidenceResolution<SequenceSurfaceKind> {
    match sequence_surface_evidence_record_at_sequence_span(il, span) {
        EvidenceResolution::Found((kind, _)) => EvidenceResolution::Found(kind),
        EvidenceResolution::Missing => EvidenceResolution::Missing,
        EvidenceResolution::Ambiguous => EvidenceResolution::Ambiguous,
    }
}

pub(crate) fn sequence_surface_evidence_record_at_sequence_span(
    il: &Il,
    span: Span,
) -> EvidenceResolution<(SequenceSurfaceKind, EvidenceId)> {
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !matches!(record.anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span)
        {
            continue;
        }
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if !language_core_sequence_surface_record(il, record) {
            continue;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn language_core_sequence_surface_record(il: &Il, record: &EvidenceRecord) -> bool {
    if record.provenance.emitter != EvidenceEmitter::Builtin {
        return false;
    }
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

pub(super) fn sequence_surface_evidence_matches_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SequenceSurfaceKind,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let raw_tag = match il.node(node).payload {
        Payload::None => None,
        Payload::Name(name) => Some(interner.resolve(name)),
        _ => return false,
    };
    if sequence_surface_kind_for_tag(il.meta.lang, raw_tag) != Some(expected) {
        return false;
    }
    matches!(
        sequence_surface_evidence_at_sequence_span(il, il.node(node).span),
        EvidenceResolution::Found(kind) if kind == expected
    )
}
