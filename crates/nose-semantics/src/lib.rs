//! Semantic contracts for language and library facts used by exact matching.
//!
//! This crate is the first-party semantic-kernel facade. The initial migration is
//! deliberately behavior-preserving: it names the semantic assumptions that were
//! previously encoded as scattered `Lang` matches. Future pack loading should
//! extend this contract surface rather than letting packs mint fingerprints or
//! approve exact clone matches directly.

use nose_il::{
    contains_js_identifier, stable_symbol_hash, Builtin, CallTargetEvidenceKind,
    EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceRecord,
    EvidenceStatus, GuardEvidenceKind, HoFKind, Il, ImportEvidenceKind, Interner, Lang,
    LibraryApiEvidenceKind, LitClass, NodeId, NodeKind, Op, ParamSemantic, Payload,
    SequenceSurfaceKind, SourceCallKind, SourceCastKind, SourceComprehensionKind, SourceFactKind,
    SourceLiteralKind, SourceOperatorKind, SourceProtocolKind, Span, Symbol, SymbolEvidenceKind,
};
use rustc_hash::FxHashMap;

mod api_guards;
mod demand;
mod effects;
mod evidence;
mod library_api;
mod module_exports;
mod type_domain;

pub use api_guards::*;
pub use demand::*;
pub(crate) use effects::asserted_effect_at_node;
pub use effects::*;
pub use evidence::*;
use evidence::{
    assignment_is_visible_at_reference, nearest_named_param_scope, nearest_scope,
    strict_numeric_operand_operator, unique_asserted_evidence_at, unique_evidence_at,
    var_references_same_binding, EvidenceResolution,
};
pub use library_api::*;
use library_api::{
    imported_occurrence_symbol_dependencies_valid, language_core_builtin_at_call,
    library_api_dependency_id_for_normalized_hof, library_method_selector_name,
};
pub use module_exports::*;
pub use nose_il::DomainEvidence;
pub use type_domain::{python_stdlib_type_domain, type_domain_from_source_text};

/// Stable pack id for the first-party language/stdlib contracts compiled into nose.
pub const FIRST_PARTY_PACK_ID: &str = "nose.first_party";

/// Channel a semantic fact or contract is safe to influence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelEligibility {
    SyntaxOnly,
    NearOnly,
    ExactEmpirical,
    ExactProven,
}

/// Trust/provenance policy for a pack, separate from which analysis channel a fact may enter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackTrust {
    DefaultFirstParty,
    FirstPartyOptional,
    ExternalOptIn,
}

pub const SEQ_VALUE_UNTAGGED: u64 = 0;
pub const SEQ_VALUE_COLLECTION: u64 = 1;
pub const SEQ_VALUE_TUPLE: u64 = 2;
pub const SEQ_VALUE_MAP: u64 = 3;
pub const SEQ_VALUE_PAIR: u64 = 4;
pub const SEQ_VALUE_RECORD_GUARD: u64 = 7;
pub const SEQ_VALUE_OWN_PROPERTY_GUARD: u64 = 8;

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
        _ => None,
    }
}

fn seq_surface_contract_for_tag(
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

/// Backward-compatible name for the evidence-only `Seq` surface resolver.
pub fn seq_surface_contract_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<SeqSurfaceContract> {
    seq_surface_contract_for_node(il, interner, node)
}

fn sequence_surface_evidence_at_sequence_span(
    il: &Il,
    span: Span,
) -> EvidenceResolution<SequenceSurfaceKind> {
    unique_evidence_at(
        il,
        |anchor| matches!(anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span),
        |evidence| match evidence {
            EvidenceKind::SequenceSurface(kind) => Some(kind),
            _ => None,
        },
    )
}

fn sequence_surface_evidence_matches_node(
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

fn guard_evidence_at_sequence_span(il: &Il, span: Span) -> EvidenceResolution<GuardEvidenceKind> {
    let mut found = None;
    for record in &il.evidence {
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

fn guard_evidence_dependencies_valid(
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
    }
}

fn js_record_shape_guard_dependencies_valid(
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

fn js_own_property_guard_api_path(path_hash: u64) -> Option<&'static str> {
    if path_hash == stable_symbol_hash("Object.hasOwn") {
        Some("Object.hasOwn")
    } else if path_hash == stable_symbol_hash("Object.prototype.hasOwnProperty.call") {
        Some("Object.prototype.hasOwnProperty.call")
    } else {
        None
    }
}

fn js_own_property_guard_dependencies_valid(
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

fn record_shape_guard_sequence_matches(
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

fn record_shape_guard_subject_matches(
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

fn own_property_guard_sequence_matches(il: &Il, interner: &Interner, node: NodeId) -> bool {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportFactKind {
    Binding,
    Namespace,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFactContract {
    pub kind: ImportFactKind,
    pub channel: ChannelEligibility,
}

pub fn import_fact_contract(kind: ImportFactKind) -> ImportFactContract {
    match kind {
        ImportFactKind::Binding => ImportFactContract {
            kind,
            channel: ChannelEligibility::ExactProven,
        },
        ImportFactKind::Namespace => ImportFactContract {
            kind,
            channel: ChannelEligibility::ExactProven,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFact {
    pub kind: ImportFactKind,
    pub module_hash: u64,
    pub exported_hash: Option<u64>,
}

fn import_fact_evidence_at_sequence_span(il: &Il, span: Span) -> EvidenceResolution<ImportFact> {
    unique_evidence_at(
        il,
        |anchor| matches!(anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span),
        |evidence| match evidence {
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash,
                exported_hash,
            }) => Some(ImportFact {
                kind: ImportFactKind::Binding,
                module_hash,
                exported_hash: Some(exported_hash),
            }),
            EvidenceKind::Import(ImportEvidenceKind::Namespace { module_hash }) => {
                Some(ImportFact {
                    kind: ImportFactKind::Namespace,
                    module_hash,
                    exported_hash: None,
                })
            }
            _ => None,
        },
    )
}

/// Evidence-only import fact resolution for semantic consumers. Import proof is
/// intentionally not encoded in the lowered `Seq` payload; callers rely on a
/// provider-owned evidence record, not on tag spelling.
pub fn import_fact_evidence_rhs(il: &Il, rhs: NodeId) -> Option<ImportFact> {
    if il.kind(rhs) != NodeKind::Seq {
        return None;
    }
    match import_fact_evidence_at_sequence_span(il, il.node(rhs).span) {
        EvidenceResolution::Found(fact) => Some(fact),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

/// Prove that `span/kind` is a first-party imported-literal producer or copied
/// snapshot whose recorded dependencies are all asserted. This proof preserves a
/// provider-scope literal producer after cross-file replacement; consumers must
/// still check the expression shape/result contract they are about to build.
pub fn imported_literal_producer_evidence_at_span(il: &Il, span: Span, kind: NodeKind) -> bool {
    il.evidence.iter().any(|record| {
        record.status == EvidenceStatus::Asserted
            && first_party_record(record)
            && record.anchor == EvidenceAnchor::node(span, kind)
            && matches!(
                record.kind,
                EvidenceKind::Import(
                    ImportEvidenceKind::ImmutableLiteralExport {
                        root_kind,
                        ..
                    } | ImportEvidenceKind::ImportedLiteralSnapshot {
                        root_kind,
                        ..
                    }
                ) if root_kind == kind
            )
            && il.evidence_dependencies_asserted(record)
    })
}

pub fn imported_literal_snapshot_evidence_at_span(il: &Il, span: Span, kind: NodeKind) -> bool {
    il.evidence.iter().any(|record| {
        record.status == EvidenceStatus::Asserted
            && first_party_record(record)
            && record.anchor == EvidenceAnchor::node(span, kind)
            && matches!(
                record.kind,
                EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot {
                    root_kind,
                    ..
                }) if root_kind == kind
            )
            && il.evidence_dependencies_asserted(record)
    })
}

pub fn imported_literal_producer_evidence_for_node(il: &Il, node: NodeId) -> bool {
    imported_literal_producer_evidence_at_span(il, il.node(node).span, il.kind(node))
}

fn first_party_record(record: &EvidenceRecord) -> bool {
    record.provenance.emitter == EvidenceEmitter::FirstParty
        && record.provenance.pack_hash == Some(stable_symbol_hash(FIRST_PARTY_PACK_ID))
}

fn symbol_evidence_at_node(il: &Il, node: NodeId) -> EvidenceResolution<SymbolEvidenceKind> {
    let span = il.node(node).span;
    let kind = il.kind(node);
    symbol_evidence_at_node_anchor(il, span, kind)
}

fn symbol_evidence_at_node_anchor(
    il: &Il,
    span: Span,
    kind: NodeKind,
) -> EvidenceResolution<SymbolEvidenceKind> {
    unique_asserted_evidence_at(
        il,
        |anchor| {
            matches!(
                anchor,
                EvidenceAnchor::Node {
                    span: anchor_span,
                    kind: anchor_kind,
                } if anchor_span == span && anchor_kind == kind
            )
        },
        |evidence| match evidence {
            EvidenceKind::Symbol(symbol) => Some(symbol),
            _ => None,
        },
    )
}

fn symbol_evidence_for_binding(
    il: &Il,
    local_hash: u64,
    span: Span,
) -> EvidenceResolution<SymbolEvidenceKind> {
    unique_evidence_at(
        il,
        |anchor| {
            matches!(
                anchor,
                EvidenceAnchor::Binding {
                    span: anchor_span,
                    local_hash: anchor_hash,
                } if anchor_hash == local_hash && anchor_span == span
            )
        },
        |evidence| match evidence {
            EvidenceKind::Symbol(symbol) => Some(symbol),
            _ => None,
        },
    )
}

fn symbol_identity_at_node_matches(
    il: &Il,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    match symbol_evidence_at_node(il, node) {
        EvidenceResolution::Found(actual) => EvidenceResolution::Found(actual == expected),
        EvidenceResolution::Ambiguous => EvidenceResolution::Ambiguous,
        EvidenceResolution::Missing => EvidenceResolution::Missing,
    }
}

fn imported_symbol_identity_at_node_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    let span = il.node(node).span;
    let kind = il.kind(node);
    let mut found = None;
    let mut dependencies_valid = true;
    for record in &il.evidence {
        if record.anchor != EvidenceAnchor::node(span, kind) {
            continue;
        }
        let EvidenceKind::Symbol(actual) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(actual),
            Some(existing) if existing == actual => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
        if actual == expected
            && !imported_occurrence_symbol_dependencies_valid(il, interner, record, expected)
        {
            dependencies_valid = false;
        }
    }
    let Some(actual) = found else {
        return EvidenceResolution::Missing;
    };
    EvidenceResolution::Found(actual == expected && dependencies_valid)
}

fn binding_identity_matches(
    il: &Il,
    local_hash: u64,
    span: Span,
    expected: SymbolEvidenceKind,
) -> EvidenceResolution<bool> {
    match symbol_evidence_for_binding(il, local_hash, span) {
        EvidenceResolution::Found(actual) => EvidenceResolution::Found(actual == expected),
        EvidenceResolution::Ambiguous => EvidenceResolution::Ambiguous,
        EvidenceResolution::Missing => EvidenceResolution::Missing,
    }
}

/// Prove that `node` denotes a language-defined unshadowed global with the exact
/// requested name. The raw spelling is not enough: when symbol evidence exists it
/// is authoritative, and ambiguous/conflicting evidence keeps the exact path
/// closed instead of falling back to spelling checks.
pub fn unshadowed_global_symbol(il: &Il, interner: &Interner, node: NodeId, name: &str) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    let expected = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(name),
    };
    match symbol_identity_at_node_matches(il, node, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    node_name(il, interner, node) == Some(name) && !file_defines_name(il, interner, name)
}

/// Evidence-only proof that `node` denotes a language-defined unshadowed global.
///
/// This is the consumer-side API for exact value semantics. Producer-side scans may
/// still use `unshadowed_global_symbol` as a compatibility bridge while migrating
/// old frontend paths onto explicit `Symbol` evidence.
pub fn asserted_unshadowed_global_symbol(il: &Il, node: NodeId, name: &str) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    let expected = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(name),
    };
    match symbol_identity_at_node_matches(il, node, expected) {
        EvidenceResolution::Found(matches) => matches,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => false,
    }
}

/// Prove that `node` denotes a static imported namespace for `module`.
pub fn imported_namespace_symbol(il: &Il, interner: &Interner, node: NodeId, module: &str) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    imported_symbol(il, interner, node, expected)
}

/// Prove that `node` denotes a static imported binding for `module.exported`.
pub fn imported_binding_symbol(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    imported_symbol(il, interner, node, expected)
}

/// Prove either `from module import exported as local; local(...)` or
/// `import module as ns; ns.exported(...)`.
pub fn imported_member_symbol(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    match il.kind(callee) {
        NodeKind::Var => imported_binding_symbol(il, interner, callee, module, exported),
        NodeKind::Field => {
            let Payload::Name(method) = il.node(callee).payload else {
                return false;
            };
            if interner.resolve(method) != exported {
                return false;
            }
            il.children(callee)
                .first()
                .copied()
                .is_some_and(|receiver| imported_namespace_symbol(il, interner, receiver, module))
        }
        _ => false,
    }
}

/// Prove that `node` denotes an exact language-defined qualified global path,
/// such as `Array.from` or `Object.hasOwn`. This is intentionally evidence-only:
/// unlike legacy import/global helpers, a matching selector spelling cannot prove
/// a qualified API identity by itself.
pub fn qualified_global_symbol(il: &Il, node: NodeId, path: &str) -> bool {
    qualified_global_symbol_at_anchor(il, il.node(node).span, il.kind(node), path)
}

/// Prove a qualified global identity at a preserved span/kind anchor. This is
/// used by value-graph consumers after IL node ids have been erased but source
/// spans remain attached to value nodes.
pub fn qualified_global_symbol_at_span(
    il: &Il,
    span: Option<Span>,
    kind: NodeKind,
    path: &str,
) -> bool {
    let Some(span) = span else {
        return false;
    };
    qualified_global_symbol_at_anchor(il, span, kind, path)
}

fn qualified_global_symbol_at_anchor(il: &Il, span: Span, kind: NodeKind, path: &str) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, path) else {
        return false;
    };
    matches!(
        qualified_global_symbol_at_evidence_anchor(il, EvidenceAnchor::node(span, kind), contract),
        EvidenceResolution::Found(())
    )
}

fn qualified_global_dependency_valid(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    path: &str,
) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, path) else {
        return false;
    };
    record.anchor.matches_span(span) && qualified_global_symbol_record_valid(il, record, contract)
}

fn qualified_global_symbol_at_evidence_anchor(
    il: &Il,
    anchor: EvidenceAnchor,
    contract: QualifiedGlobalSymbolContract,
) -> EvidenceResolution<()> {
    let mut found = false;
    for record in &il.evidence {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(_) = record.kind else {
            continue;
        };
        if !qualified_global_symbol_record_valid(il, record, contract) {
            return EvidenceResolution::Ambiguous;
        }
        found = true;
    }
    if found {
        EvidenceResolution::Found(())
    } else {
        EvidenceResolution::Missing
    }
}

fn qualified_global_symbol_record_valid(
    il: &Il,
    record: &EvidenceRecord,
    contract: QualifiedGlobalSymbolContract,
) -> bool {
    let expected = SymbolEvidenceKind::QualifiedGlobal {
        path_hash: stable_symbol_hash(contract.path),
    };
    if record.status != EvidenceStatus::Asserted
        || record.kind != EvidenceKind::Symbol(expected)
        || !il.evidence_dependencies_asserted(record)
    {
        return false;
    }
    !contract.requires_unshadowed_root
        || evidence_record_has_unshadowed_root_dependency(il, record, contract.root)
}

fn evidence_record_has_unshadowed_root_dependency(
    il: &Il,
    record: &EvidenceRecord,
    root: &str,
) -> bool {
    let span = evidence_anchor_span(record.anchor);
    let expected = EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(root),
    });
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.status == EvidenceStatus::Asserted
                && dependency.anchor == EvidenceAnchor::source_span(span)
                && dependency.kind == expected
                && il.evidence_dependencies_asserted(dependency)
        })
    })
}

fn evidence_anchor_span(anchor: EvidenceAnchor) -> Span {
    match anchor {
        EvidenceAnchor::SourceSpan(span)
        | EvidenceAnchor::Node { span, .. }
        | EvidenceAnchor::Param { span }
        | EvidenceAnchor::Binding { span, .. }
        | EvidenceAnchor::Sequence { span } => span,
    }
}

fn imported_symbol(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    match imported_symbol_identity_at_node_matches(il, interner, node, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    let Some(local_hash) = node_name_hash(il, interner, node) else {
        return false;
    };
    if unit_defines_hash_visible_at(il, interner, local_hash, il.node(node).span) {
        return false;
    }
    let statements = top_level_statements(il);
    let matching_assignments = statements
        .iter()
        .copied()
        .filter(|&stmt| assignment_alias_hash(il, interner, stmt) == Some(local_hash))
        .collect::<Vec<_>>();
    let [assignment] = matching_assignments.as_slice() else {
        return false;
    };
    match binding_identity_matches(il, local_hash, il.node(*assignment).span, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    false
}

fn top_level_statements(il: &Il) -> Vec<NodeId> {
    il.children(il.root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            if il.kind(node) == NodeKind::Block {
                statements.extend_from_slice(il.children(node));
            } else {
                statements.push(node);
            }
            statements
        })
}

fn assignment_alias_hash(il: &Il, interner: &Interner, stmt: NodeId) -> Option<u64> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    (il.kind(lhs) == NodeKind::Var)
        .then(|| node_name_hash(il, interner, lhs))
        .flatten()
}

fn assignment_parts(il: &Il, stmt: NodeId) -> Option<(NodeId, NodeId)> {
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let [lhs, rhs] = il.children(stmt) else {
        return None;
    };
    Some((*lhs, *rhs))
}

fn node_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.resolve(symbol)),
        _ => None,
    }
}

fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    node_name(il, interner, node).map(stable_symbol_hash)
}

fn unit_defines_hash(il: &Il, interner: &Interner, name_hash: u64) -> bool {
    il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}

fn unit_defines_hash_visible_at(
    il: &Il,
    interner: &Interner,
    name_hash: u64,
    occurrence_span: Span,
) -> bool {
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}

fn file_defines_name(il: &Il, interner: &Interner, name: &str) -> bool {
    let name_hash = stable_symbol_hash(name);
    il.units.iter().any(|unit| {
        unit.name.is_some_and(|symbol| {
            symbol_defines_name(il.meta.lang, interner.resolve(symbol), name, name_hash)
        })
    }) || il
        .nodes
        .iter()
        .enumerate()
        .any(|(idx, node)| match node.kind {
            NodeKind::Module | NodeKind::Block | NodeKind::Param => {
                node_defines_name(il, interner, NodeId(idx as u32), name, name_hash)
            }
            NodeKind::Assign => il
                .children(NodeId(idx as u32))
                .first()
                .copied()
                .is_some_and(|lhs| node_defines_name(il, interner, lhs, name, name_hash)),
            _ => false,
        })
}

pub fn file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    let name_hash = stable_symbol_hash(name);
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit.name.is_some_and(|symbol| {
                symbol_defines_name(il.meta.lang, interner.resolve(symbol), name, name_hash)
            })
    }) || il.nodes.iter().enumerate().any(|(idx, node)| {
        node.span.file == occurrence_span.file
            && match node.kind {
                NodeKind::Module | NodeKind::Block | NodeKind::Param => {
                    node_defines_name(il, interner, NodeId(idx as u32), name, name_hash)
                }
                NodeKind::Assign => il
                    .children(NodeId(idx as u32))
                    .first()
                    .copied()
                    .is_some_and(|lhs| node_defines_name(il, interner, lhs, name, name_hash)),
                _ => false,
            }
    })
}

fn node_defines_name(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    name: &str,
    name_hash: u64,
) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => {
            symbol_defines_name(il.meta.lang, interner.resolve(symbol), name, name_hash)
        }
        Payload::Cid(cid) => il.cid_names.get(cid as usize).is_some_and(|symbol| {
            symbol_defines_name(il.meta.lang, interner.resolve(*symbol), name, name_hash)
        }),
        _ => false,
    }
}

fn symbol_defines_name(lang: Lang, text: &str, name: &str, name_hash: u64) -> bool {
    stable_symbol_hash(text) == name_hash
        || (js_like_lang(lang) && contains_js_identifier(text, name))
}

fn literal_string_hash(il: &Il, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::LitStr(hash) => Some(hash),
        _ => None,
    }
}

/// A first-party language profile. Keep this cheap and copyable; callers use it as a
/// named semantic boundary around currently-supported language behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LanguageProfile {
    lang: Lang,
}

pub fn semantics(lang: Lang) -> LanguageProfile {
    LanguageProfile { lang }
}

impl LanguageProfile {
    pub fn lang(self) -> Lang {
        self.lang
    }

    pub fn pack_id(self) -> &'static str {
        FIRST_PARTY_PACK_ID
    }

    pub fn trust(self) -> PackTrust {
        PackTrust::DefaultFirstParty
    }

    pub fn operators(self) -> OperatorSemantics {
        OperatorSemantics { lang: self.lang }
    }

    pub fn effects(self) -> EffectSemantics {
        EffectSemantics { lang: self.lang }
    }

    pub fn modules(self) -> ModuleSemantics {
        ModuleSemantics { lang: self.lang }
    }

    pub fn stdlib(self) -> StdlibSemantics {
        StdlibSemantics { lang: self.lang }
    }

    pub fn collections(self) -> CollectionSemantics {
        CollectionSemantics { lang: self.lang }
    }

    pub fn exact_fragments(self) -> FragmentSemantics {
        FragmentSemantics { lang: self.lang }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperatorSemantics {
    lang: Lang,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComparisonLaw {
    DirectionCanon,
    Negation,
    EqualityCommutativity,
    LatticeLeNeToLt,
    LatticeLtEqToLe,
    LatticeStrictAbsorbsNonstrict,
    AbsSignTernary,
    MinMaxTernary,
    SelectionReductionGuard,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperatorEvidence {
    ModeledIlOperator,
    PrimitiveTotalOrder,
    StaticCardinalityThreshold,
    JsLikeStaticIndexMembershipThreshold,
    CIntegerBytePack,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperatorLawContract {
    pub law: ComparisonLaw,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ComparisonTransformContract {
    pub law: ComparisonLaw,
    pub input: Op,
    pub output: Op,
    pub swap_operands: bool,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardinalityThreshold {
    Zero,
    One,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardinalityPredicate {
    Empty,
    NonEmpty,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CardinalityThresholdContract {
    pub threshold: CardinalityThreshold,
    pub predicate: CardinalityPredicate,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticIndexMembershipThresholdContract {
    pub threshold: IndexMembershipThreshold,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MembershipOperatorReceiverContract {
    ExactCollectionOrMap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MembershipOperatorContract {
    pub operator: Op,
    pub receiver: MembershipOperatorReceiverContract,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CBytePackWidth {
    U16,
    U32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CIntegerBytePackContract {
    pub width: CBytePackWidth,
    pub base_domain: DomainRequirement,
    pub required_high_lane_cast: Option<SourceFactKind>,
    pub channel: ChannelEligibility,
    pub evidence: OperatorEvidence,
}

impl OperatorSemantics {
    pub fn value_law(self, law: ValueLaw) -> Option<ValueLawContract> {
        let requirement = match law {
            ValueLaw::AddCommutativity | ValueLaw::AddAssociativity => {
                ValueDomainRequirement::NoConcatOperands
            }
            ValueLaw::NumericNegationInvolution
            | ValueLaw::NumericBitwiseIdempotence
            | ValueLaw::NumericFactorDistribution
            | ValueLaw::StructuralNumericFold => ValueDomainRequirement::NumericOperands,
            ValueLaw::BooleanIdempotence
            | ValueLaw::BooleanCommutativity
            | ValueLaw::BooleanAssociativity => ValueDomainRequirement::BooleanOperands,
        };
        Some(ValueLawContract {
            law,
            requirement,
            channel: ChannelEligibility::ExactProven,
            evidence: ValueDomainEvidence::ModeledOperatorResult,
        })
    }

    pub fn strict_operand_domain(self, op: Op) -> Option<ValueDomain> {
        if strict_numeric_operand_operator(op) {
            Some(ValueDomain::Number)
        } else {
            None
        }
    }

    pub fn unary_operand_domain(self, op: Op) -> Option<ValueDomain> {
        match op {
            Op::Neg | Op::Pos | Op::BitNot => Some(ValueDomain::Number),
            _ => None,
        }
    }

    pub fn unary_result_domain(self, op: Op) -> ValueDomain {
        match op {
            Op::Neg | Op::Pos | Op::BitNot => ValueDomain::Number,
            Op::Not => ValueDomain::Boolean,
            _ => ValueDomain::Unknown,
        }
    }

    pub fn binary_result_domain(
        self,
        op: Op,
        left: ValueDomain,
        right: ValueDomain,
    ) -> ValueDomain {
        if op == Op::Mul && (left == ValueDomain::String || right == ValueDomain::String) {
            ValueDomain::String
        } else if strict_numeric_operand_operator(op) {
            if left.is_known() || right.is_known() {
                if left == ValueDomain::Number && right == ValueDomain::Number {
                    ValueDomain::Number
                } else {
                    ValueDomain::Unknown
                }
            } else {
                ValueDomain::Number
            }
        } else if matches!(
            op,
            Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne | Op::In
        ) {
            ValueDomain::Boolean
        } else if op == Op::Add {
            if left == ValueDomain::Number && right == ValueDomain::Number {
                ValueDomain::Number
            } else if left == ValueDomain::String || right == ValueDomain::String {
                ValueDomain::String
            } else if left == ValueDomain::Sequence || right == ValueDomain::Sequence {
                ValueDomain::Sequence
            } else {
                ValueDomain::Unknown
            }
        } else if matches!(op, Op::And | Op::Or)
            && left == ValueDomain::Boolean
            && right == ValueDomain::Boolean
        {
            ValueDomain::Boolean
        } else {
            ValueDomain::Unknown
        }
    }

    pub fn builtin_result_domain(self, builtin: Builtin) -> ValueDomain {
        match builtin {
            Builtin::Len | Builtin::UnsignedCast32 => ValueDomain::Number,
            Builtin::IsEmpty
            | Builtin::IsNull
            | Builtin::IsNotNull
            | Builtin::StartsWith
            | Builtin::EndsWith
            | Builtin::Contains => ValueDomain::Boolean,
            Builtin::Join => ValueDomain::String,
            _ => ValueDomain::Unknown,
        }
    }

    pub fn literal_value_domain(self, payload: Payload) -> Option<ValueDomain> {
        match payload {
            Payload::LitInt(_) | Payload::LitFloat(_) => Some(ValueDomain::Number),
            Payload::LitStr(_) => Some(ValueDomain::String),
            Payload::LitBool(_) => Some(ValueDomain::Boolean),
            Payload::Lit(LitClass::Int) | Payload::Lit(LitClass::Float) => {
                Some(ValueDomain::Number)
            }
            Payload::Lit(LitClass::Str) => Some(ValueDomain::String),
            Payload::Lit(LitClass::Bool) => Some(ValueDomain::Boolean),
            _ => None,
        }
    }

    pub fn expression_value_domain<F>(self, il: &Il, node: NodeId, param_domain: &F) -> ValueDomain
    where
        F: Fn(u32) -> ValueDomain,
    {
        match il.node(node).kind {
            NodeKind::Lit => self
                .literal_value_domain(il.node(node).payload)
                .unwrap_or(ValueDomain::Unknown),
            NodeKind::Var => match il.node(node).payload {
                Payload::Cid(cid) => param_domain(cid),
                _ => ValueDomain::Unknown,
            },
            NodeKind::Seq => ValueDomain::Sequence,
            NodeKind::UnOp => match il.node(node).payload {
                Payload::Op(op) => self.unary_result_domain(op),
                _ => ValueDomain::Unknown,
            },
            NodeKind::BinOp => {
                let kids = il.children(node);
                let Payload::Op(op) = il.node(node).payload else {
                    return ValueDomain::Unknown;
                };
                if kids.len() == 2 {
                    let left = self.expression_value_domain(il, kids[0], param_domain);
                    let right = self.expression_value_domain(il, kids[1], param_domain);
                    self.binary_result_domain(op, left, right)
                } else {
                    self.binary_result_domain(op, ValueDomain::Unknown, ValueDomain::Unknown)
                }
            }
            NodeKind::Call => match il.node(node).payload {
                Payload::Builtin(builtin)
                    if admitted_builtin_semantics_at_call(il, node, builtin) =>
                {
                    self.builtin_result_domain(builtin)
                }
                _ => ValueDomain::Unknown,
            },
            _ => ValueDomain::Unknown,
        }
    }

    pub fn infer_param_value_domains(self, il: &Il, root: NodeId) -> Vec<ValueDomain> {
        if il.kind(root) != NodeKind::Func {
            return Vec::new();
        }
        let mut params: Vec<u32> = Vec::new();
        for &child in il.children(root) {
            if il.kind(child) == NodeKind::Param {
                if let Payload::Cid(cid) = il.node(child).payload {
                    params.push(cid);
                }
            }
        }
        let cid_of = |node: NodeId, il: &Il| -> Option<u32> {
            if il.kind(node) == NodeKind::Var {
                if let Payload::Cid(cid) = il.node(node).payload {
                    return Some(cid);
                }
            }
            None
        };
        let mut evidence: FxHashMap<u32, ValueDomain> = FxHashMap::default();
        for _ in 0..params.len() + 1 {
            let mut next = evidence.clone();
            let add = |cid: u32, domain: ValueDomain, ev: &mut FxHashMap<u32, ValueDomain>| {
                ev.entry(cid)
                    .and_modify(|existing| *existing = existing.join(domain))
                    .or_insert(domain);
            };
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                let kids = il.children(node).to_vec();
                match il.node(node).kind {
                    NodeKind::BinOp => {
                        if let Payload::Op(op) = il.node(node).payload {
                            if self.strict_operand_domain(op).is_some() && kids.len() == 2 {
                                for &kid in &kids {
                                    if let Some(cid) = cid_of(kid, il) {
                                        add(cid, ValueDomain::Number, &mut next);
                                    }
                                }
                            } else if op == Op::Add && kids.len() == 2 {
                                let lookup = |cid| {
                                    evidence.get(&cid).copied().unwrap_or(ValueDomain::Unknown)
                                };
                                let domains = [
                                    self.expression_value_domain(il, kids[0], &lookup),
                                    self.expression_value_domain(il, kids[1], &lookup),
                                ];
                                for i in 0..2 {
                                    if let Some(cid) = cid_of(kids[i], il) {
                                        if matches!(
                                            domains[1 - i],
                                            ValueDomain::Number | ValueDomain::String
                                        ) {
                                            add(cid, domains[1 - i], &mut next);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    NodeKind::UnOp => {
                        if let Payload::Op(op) = il.node(node).payload {
                            if self.unary_operand_domain(op).is_some() {
                                if let Some(cid) = kids.first().and_then(|&kid| cid_of(kid, il)) {
                                    add(cid, ValueDomain::Number, &mut next);
                                }
                            }
                        }
                    }
                    NodeKind::Index => {
                        if let Some(cid) = kids.get(1).and_then(|&kid| cid_of(kid, il)) {
                            add(cid, ValueDomain::Number, &mut next);
                        }
                    }
                    _ => {}
                }
                stack.extend(kids);
            }
            if next == evidence {
                break;
            }
            evidence = next;
        }
        params
            .iter()
            .map(|cid| evidence.get(cid).copied().unwrap_or(ValueDomain::Unknown))
            .collect()
    }

    pub fn comparison_law(self, law: ComparisonLaw) -> Option<OperatorLawContract> {
        let evidence = match law {
            ComparisonLaw::LatticeStrictAbsorbsNonstrict => {
                if !matches!(self.lang, Lang::C | Lang::Go | Lang::Java) {
                    return None;
                }
                OperatorEvidence::PrimitiveTotalOrder
            }
            ComparisonLaw::DirectionCanon
            | ComparisonLaw::Negation
            | ComparisonLaw::EqualityCommutativity
            | ComparisonLaw::LatticeLeNeToLt
            | ComparisonLaw::LatticeLtEqToLe
            | ComparisonLaw::AbsSignTernary
            | ComparisonLaw::MinMaxTernary
            | ComparisonLaw::SelectionReductionGuard => OperatorEvidence::ModeledIlOperator,
        };
        Some(OperatorLawContract {
            law,
            channel: ChannelEligibility::ExactProven,
            evidence,
        })
    }

    pub fn comparison_direction(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Gt => Op::Lt,
            Op::Ge => Op::Le,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::DirectionCanon)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: true,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn comparison_reverse(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Lt => Op::Gt,
            Op::Le => Op::Ge,
            Op::Gt => Op::Lt,
            Op::Ge => Op::Le,
            Op::Eq => Op::Eq,
            Op::Ne => Op::Ne,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::DirectionCanon)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: true,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn comparison_complement(self, op: Op) -> Option<ComparisonTransformContract> {
        let output = match op {
            Op::Lt => Op::Ge,
            Op::Le => Op::Gt,
            Op::Gt => Op::Le,
            Op::Ge => Op::Lt,
            Op::Eq => Op::Ne,
            Op::Ne => Op::Eq,
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::Negation)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands: false,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    pub fn canonical_negated_comparison(self, op: Op) -> Option<ComparisonTransformContract> {
        let (output, swap_operands) = match op {
            Op::Eq => (Op::Ne, false),
            Op::Ne => (Op::Eq, false),
            Op::Lt => (Op::Le, true),
            Op::Le => (Op::Lt, true),
            Op::Gt => (Op::Le, false),
            Op::Ge => (Op::Lt, false),
            _ => return None,
        };
        let law = self.comparison_law(ComparisonLaw::Negation)?;
        Some(ComparisonTransformContract {
            law: law.law,
            input: op,
            output,
            swap_operands,
            channel: law.channel,
            evidence: law.evidence,
        })
    }

    /// Source comparison operators are primitive total-order comparisons rather
    /// than receiver-overloadable/user-dispatched comparisons. This gates lattice
    /// comparison absorption rules.
    pub fn primitive_order_comparisons(self) -> bool {
        self.comparison_law(ComparisonLaw::LatticeStrictAbsorbsNonstrict)
            .is_some()
    }

    pub fn zero_cardinality_equality(self, op: Op) -> Option<CardinalityThresholdContract> {
        let predicate = match op {
            Op::Eq => CardinalityPredicate::Empty,
            Op::Ne => CardinalityPredicate::NonEmpty,
            _ => return None,
        };
        Some(CardinalityThresholdContract {
            threshold: CardinalityThreshold::Zero,
            predicate,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    }

    pub fn cardinality_threshold(
        self,
        op: Op,
        count_on_right: bool,
        threshold: CardinalityThreshold,
        predicate: CardinalityPredicate,
    ) -> Option<CardinalityThresholdContract> {
        let matches = match (predicate, threshold) {
            (CardinalityPredicate::NonEmpty, CardinalityThreshold::Zero) => {
                threshold_excludes_floor(op, count_on_right)
            }
            (CardinalityPredicate::NonEmpty, CardinalityThreshold::One) => {
                threshold_reaches_floor(op, count_on_right)
            }
            (CardinalityPredicate::Empty, CardinalityThreshold::Zero) => {
                threshold_at_or_below_floor(op, count_on_right)
            }
            (CardinalityPredicate::Empty, CardinalityThreshold::One) => {
                threshold_below_floor(op, count_on_right)
            }
        };
        matches.then_some(CardinalityThresholdContract {
            threshold,
            predicate,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::StaticCardinalityThreshold,
        })
    }

    pub fn static_index_membership_threshold(
        self,
        op: Op,
        index_call_on_right: bool,
        threshold: IndexMembershipThreshold,
    ) -> Option<StaticIndexMembershipThresholdContract> {
        if !js_like_lang(self.lang) {
            return None;
        }
        index_membership_threshold_matches(op, index_call_on_right, threshold).then_some(
            StaticIndexMembershipThresholdContract {
                threshold,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::JsLikeStaticIndexMembershipThreshold,
            },
        )
    }

    pub fn membership_operator(self, op: Op) -> Option<MembershipOperatorContract> {
        (self.lang == Lang::Python && op == Op::In).then_some(MembershipOperatorContract {
            operator: op,
            receiver: MembershipOperatorReceiverContract::ExactCollectionOrMap,
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::ModeledIlOperator,
        })
    }

    /// C unsigned byte/word packing contracts are currently first-party only for
    /// the C lowering, where explicit byte-buffer and unsigned-cast facts are
    /// recovered by the frontend.
    pub fn c_integer_byte_pack_contract(
        self,
        width: CBytePackWidth,
    ) -> Option<CIntegerBytePackContract> {
        (self.lang == Lang::C).then_some(CIntegerBytePackContract {
            width,
            base_domain: DomainRequirement::ByteArray,
            required_high_lane_cast: match width {
                CBytePackWidth::U16 => None,
                CBytePackWidth::U32 => Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
            },
            channel: ChannelEligibility::ExactProven,
            evidence: OperatorEvidence::CIntegerBytePack,
        })
    }
}

fn threshold_excludes_floor(op: Op, value_on_right: bool) -> bool {
    op == Op::Ne || (!value_on_right && op == Op::Gt) || (value_on_right && op == Op::Lt)
}

fn threshold_reaches_floor(op: Op, value_on_right: bool) -> bool {
    (!value_on_right && op == Op::Ge) || (value_on_right && op == Op::Le)
}

fn threshold_at_or_below_floor(op: Op, value_on_right: bool) -> bool {
    op == Op::Eq || (!value_on_right && op == Op::Le) || (value_on_right && op == Op::Ge)
}

fn threshold_below_floor(op: Op, value_on_right: bool) -> bool {
    (!value_on_right && op == Op::Lt) || (value_on_right && op == Op::Gt)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModuleSemantics {
    lang: Lang,
}

impl ModuleSemantics {
    /// JavaScript-like lexical scopes can shadow imported module bindings with a
    /// local definition of the same name.
    pub fn js_like_shadowed_module_bindings(self) -> bool {
        matches!(
            self.lang,
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
        )
    }

    /// Sibling-module immutable literal export resolution is modeled for these
    /// first-party module systems.
    pub fn sibling_literal_exports(self) -> bool {
        self.path_spec().is_some()
    }

    /// Java class bodies also contribute static literal bindings keyed by class
    /// names and path-derived class module names.
    pub fn java_class_literal_exports(self) -> bool {
        self.lang == Lang::Java
    }

    /// Java class/type declarations can shadow standard type names such as
    /// `Map`, `List`, `Set`, and `Arrays` in first-party stdlib contracts.
    pub fn java_type_declarations_shadow_stdlib(self) -> bool {
        self.lang == Lang::Java
    }

    /// Go static imports are lowered as namespace facts that can prove package
    /// aliases for selected stdlib-style recognizers.
    pub fn go_import_namespace_facts(self) -> bool {
        self.lang == Lang::Go
    }

    pub fn path_spec(self) -> Option<ModulePathSpec> {
        match self.lang {
            Lang::Python => Some(ModulePathSpec {
                extensions: &["py"],
                separator: ".",
                include_relative_dot: false,
                drop_init_file: true,
                rust_crate_self_aliases: false,
            }),
            Lang::JavaScript | Lang::TypeScript => Some(ModulePathSpec {
                extensions: &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"],
                separator: "/",
                include_relative_dot: true,
                drop_init_file: false,
                rust_crate_self_aliases: false,
            }),
            Lang::Java => Some(ModulePathSpec {
                extensions: &["java"],
                separator: ".",
                include_relative_dot: false,
                drop_init_file: false,
                rust_crate_self_aliases: false,
            }),
            Lang::Rust => Some(ModulePathSpec {
                extensions: &["rs"],
                separator: "::",
                include_relative_dot: false,
                drop_init_file: false,
                rust_crate_self_aliases: true,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModulePathSpec {
    pub extensions: &'static [&'static str],
    pub separator: &'static str,
    pub include_relative_dot: bool,
    pub drop_init_file: bool,
    pub rust_crate_self_aliases: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StdlibSemantics {
    lang: Lang,
}

impl StdlibSemantics {
    pub fn python_collection_factories(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn python_deque_factory(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn java_collection_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_map_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_primitive_integer_ops(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn ruby_set_factory(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn rust_vec_macro_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_vec_new_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_collection_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_map_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn go_literal_zero_map_lookup(self) -> bool {
        self.lang == Lang::Go
    }

    pub fn rust_filter_map_option_contract(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn imported_map_factory(self) -> Option<ImportedMapFactoryContract> {
        match self.lang {
            Lang::Java => Some(ImportedMapFactoryContract::JavaMap),
            Lang::Rust => Some(ImportedMapFactoryContract::RustStdMap),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportedMapFactoryContract {
    JavaMap,
    RustStdMap,
}

/// The value-graph call tag for a canonical builtin. Tag `0` is reserved for
/// opaque calls, so kernel-owned builtin contracts start at `1`.
pub fn builtin_tag(builtin: Builtin) -> u32 {
    builtin as u32 + 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinArgContract {
    First,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeFunctionBuiltinContract {
    pub name: &'static str,
    pub builtin: Builtin,
    pub args: BuiltinArgContract,
    pub requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FreeFunctionBuiltinArity {
    Exact(usize),
    AtLeast(usize),
    OneOf(&'static [usize]),
}

impl FreeFunctionBuiltinArity {
    fn accepts(self, arg_count: usize) -> bool {
        match self {
            FreeFunctionBuiltinArity::Exact(expected) => arg_count == expected,
            FreeFunctionBuiltinArity::AtLeast(minimum) => arg_count >= minimum,
            FreeFunctionBuiltinArity::OneOf(expected) => expected.contains(&arg_count),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FreeFunctionBuiltinRow {
    lang: Lang,
    name: &'static str,
    builtin: Builtin,
    args: BuiltinArgContract,
    arity: FreeFunctionBuiltinArity,
    requires_unshadowed: bool,
}

const ONE_OR_TWO_ARGS: &[usize] = &[1, 2];
const ONE_TO_THREE_ARGS: &[usize] = &[1, 2, 3];
const PY: Lang = Lang::Python;
const GO: Lang = Lang::Go;
const FIRST_ARG: BuiltinArgContract = BuiltinArgContract::First;
const ALL_ARGS: BuiltinArgContract = BuiltinArgContract::All;
const ARITY_ANY: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::AtLeast(0);
const ARITY_ONE: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::Exact(1);
const ARITY_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::Exact(2);
const ARITY_AT_LEAST_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::AtLeast(2);
const ARITY_ONE_OR_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::OneOf(ONE_OR_TWO_ARGS);
const ARITY_ONE_TO_THREE: FreeFunctionBuiltinArity =
    FreeFunctionBuiltinArity::OneOf(ONE_TO_THREE_ARGS);

const fn free_function_builtin_row(
    lang: Lang,
    name: &'static str,
    builtin: Builtin,
    args: BuiltinArgContract,
    arity: FreeFunctionBuiltinArity,
) -> FreeFunctionBuiltinRow {
    FreeFunctionBuiltinRow {
        lang,
        name,
        builtin,
        args,
        arity,
        requires_unshadowed: true,
    }
}

const FREE_FUNCTION_BUILTINS: &[FreeFunctionBuiltinRow] = &[
    free_function_builtin_row(PY, "len", Builtin::Len, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(GO, "len", Builtin::Len, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(GO, "append", Builtin::Append, ALL_ARGS, ARITY_AT_LEAST_TWO),
    free_function_builtin_row(PY, "print", Builtin::Print, ALL_ARGS, ARITY_ANY),
    free_function_builtin_row(PY, "range", Builtin::Range, ALL_ARGS, ARITY_ONE_TO_THREE),
    free_function_builtin_row(PY, "sum", Builtin::Sum, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "min", Builtin::Min, ALL_ARGS, ARITY_ONE_OR_TWO),
    free_function_builtin_row(PY, "max", Builtin::Max, ALL_ARGS, ARITY_ONE_OR_TWO),
    free_function_builtin_row(PY, "abs", Builtin::Abs, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "zip", Builtin::Zip, ALL_ARGS, ARITY_TWO),
    free_function_builtin_row(PY, "enumerate", Builtin::Enumerate, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "any", Builtin::Any, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "all", Builtin::All, FIRST_ARG, ARITY_ONE),
];

fn free_function_builtin_contract_from_row(
    row: &FreeFunctionBuiltinRow,
) -> FreeFunctionBuiltinContract {
    FreeFunctionBuiltinContract {
        name: row.name,
        builtin: row.builtin,
        args: row.args,
        requires_unshadowed: row.requires_unshadowed,
    }
}

pub fn free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<FreeFunctionBuiltinContract> {
    FREE_FUNCTION_BUILTINS
        .iter()
        .find(|row| row.lang == lang && row.name == name && row.arity.accepts(arg_count))
        .map(free_function_builtin_contract_from_row)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodReceiverContract {
    ExactCollection,
    ExactProtocol,
    ExactProtocolPairArgument,
    ExactOption,
    ExactString,
    ExactInteger,
    ExactMap,
    ExactMapLiteral,
    ExactCollectionOrMap,
    ExactCollectionOrMapLiteral,
    ExactCollectionOrJavaKeySet,
    ExactSetOrMap,
    LiteralString,
    UnshadowedGlobal(&'static str),
    ImportedNamespace(&'static str),
    RustMapGetOrExactOption,
}

pub fn method_receiver_domain_requirement(
    receiver: MethodReceiverContract,
) -> Option<DomainRequirement> {
    match receiver {
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactProtocol
        | MethodReceiverContract::ExactProtocolPairArgument
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            Some(DomainRequirement::ArrayCollectionOrSet)
        }
        MethodReceiverContract::ExactOption | MethodReceiverContract::RustMapGetOrExactOption => {
            Some(DomainRequirement::Option)
        }
        MethodReceiverContract::ExactString | MethodReceiverContract::LiteralString => {
            Some(DomainRequirement::String)
        }
        MethodReceiverContract::ExactInteger => Some(DomainRequirement::Integer),
        MethodReceiverContract::ExactMap => Some(DomainRequirement::Map),
        MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrMapLiteral => {
            Some(DomainRequirement::CollectionOrMap)
        }
        MethodReceiverContract::ExactSetOrMap => Some(DomainRequirement::SetOrMap),
        MethodReceiverContract::ExactMapLiteral
        | MethodReceiverContract::UnshadowedGlobal(_)
        | MethodReceiverContract::ImportedNamespace(_) => None,
    }
}

pub fn receiver_satisfies_method_domain(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> bool {
    method_receiver_domain_requirement(contract)
        .is_some_and(|requirement| receiver_satisfies_domain(il, interner, receiver, requirement))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodBuiltinArgs {
    All,
    First,
    ReceiverOnly,
    ReceiverThenAll,
    ReceiverAndFirst,
    FirstThenReceiver,
    GoSliceContains,
    MapGetDefault,
    MapGetDefaultOrZeroArgLambda,
    RustMapGetOrOptionDefault,
    RustOptionDefaultLambda,
    RustOptionMapOrIdentity,
    RustZip,
    Fold,
    BoolReduction,
    Hof,
    CollectionReduction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodSemanticContract {
    Builtin(Builtin),
    HoF(HoFKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodCallContract {
    pub semantic: MethodSemanticContract,
    pub receiver: MethodReceiverContract,
    pub args: MethodBuiltinArgs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalarIntegerMethod {
    Abs,
    Min,
    Max,
    Clamp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalarIntegerMethodContract {
    pub semantic: ScalarIntegerMethod,
    pub receiver: MethodReceiverContract,
}

fn scalar_integer_method_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<ScalarIntegerMethodContract> {
    use ScalarIntegerMethod as Method;

    let semantic = match (lang, name, arg_count) {
        (Lang::Rust, "abs", 0) => Method::Abs,
        (Lang::Rust, "min", 1) => Method::Min,
        (Lang::Rust, "max", 1) => Method::Max,
        (Lang::Rust, "clamp", 2) => Method::Clamp,
        _ => return None,
    };
    Some(ScalarIntegerMethodContract {
        semantic,
        receiver: MethodReceiverContract::ExactInteger,
    })
}

pub fn scalar_integer_method_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<ScalarIntegerMethodContract> {
    library_scalar_integer_method_contract(lang, name, arg_count).map(|contract| contract.result)
}

pub fn method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodCallContract> {
    library_method_call_contract(lang, name, arg_count).map(|contract| contract.result)
}

fn method_call_contract_shape(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodCallContract> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;
    use MethodSemanticContract as Semantic;

    let contract = match (lang, name, arg_count) {
        (Lang::Python, "append", 1) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "push",
            1..,
        ) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),

        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "log" | "info" | "debug",
            _,
        ) => (
            Builtin::Print,
            Receiver::UnshadowedGlobal("console"),
            Args::All,
        ),
        (Lang::Go, "Println" | "Printf" | "Print", _) => (
            Builtin::Print,
            Receiver::ImportedNamespace("fmt"),
            Args::All,
        ),
        (Lang::Go, "Abs", 1) => (
            Builtin::Abs,
            Receiver::ImportedNamespace("math"),
            Args::First,
        ),
        (Lang::Go, "HasPrefix", 2) => (
            Builtin::StartsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "HasSuffix", 2) => (
            Builtin::EndsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "Contains", 2) => (
            Builtin::Contains,
            Receiver::ImportedNamespace("slices"),
            Args::GoSliceContains,
        ),

        (Lang::Rust, "len", 0) | (Lang::Java, "size", 0) => {
            (Builtin::Len, Receiver::ExactCollection, Args::ReceiverOnly)
        }
        (Lang::Rust, "is_empty", 0) | (Lang::Java, "isEmpty", 0) | (Lang::Ruby, "empty?", 0) => (
            Builtin::IsEmpty,
            Receiver::ExactCollection,
            Args::ReceiverOnly,
        ),
        (Lang::Ruby, "nil?", 0) | (Lang::Rust, "is_none", 0) => {
            (Builtin::IsNull, Receiver::ExactOption, Args::ReceiverOnly)
        }
        (Lang::Rust, "is_some", 0) => (
            Builtin::IsNotNull,
            Receiver::RustMapGetOrExactOption,
            Args::ReceiverOnly,
        ),

        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "startsWith",
            1,
        )
        | (Lang::Python, "startswith", 1)
        | (Lang::Rust, "starts_with", 1)
        | (Lang::Ruby, "start_with?", 1) => (
            Builtin::StartsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),
        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "endsWith",
            1,
        )
        | (Lang::Python, "endswith", 1)
        | (Lang::Rust, "ends_with", 1)
        | (Lang::Ruby, "end_with?", 1) => (
            Builtin::EndsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),

        (Lang::Java, "containsKey", 1)
        | (Lang::Rust, "contains_key", 1)
        | (Lang::Ruby, "key?" | "has_key?", 1) => (
            Builtin::Contains,
            Receiver::ExactMap,
            Args::FirstThenReceiver,
        ),
        (Lang::Python, "__contains__", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrMap,
            Args::FirstThenReceiver,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "includes",
            1,
        )
        | (Lang::Ruby, "include?" | "member?", 1)
        | (Lang::Java | Lang::Rust, "contains", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrJavaKeySet,
            Args::FirstThenReceiver,
        ),
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "has", 1) => {
            (
                Builtin::Contains,
                Receiver::ExactSetOrMap,
                Args::FirstThenReceiver,
            )
        }

        (Lang::Python, "join", 1) => (
            Builtin::Join,
            Receiver::LiteralString,
            Args::ReceiverAndFirst,
        ),
        (Lang::Python, "get", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Ruby, "fetch", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefaultOrZeroArgLambda,
        ),
        (Lang::Java, "getOrDefault", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Rust, "unwrap_or", 1) => (
            Builtin::ValueOrDefault,
            Receiver::RustMapGetOrExactOption,
            Args::RustMapGetOrOptionDefault,
        ),
        (Lang::Rust, "unwrap_or_else", 1) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionDefaultLambda,
        ),
        (Lang::Rust, "map_or", 2) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionMapOrIdentity,
        ),

        (Lang::Python, "reduce", 2..) => (
            Builtin::Reduce,
            Receiver::ImportedNamespace("functools"),
            Args::All,
        ),
        (Lang::Go, "Min", 2) => (Builtin::Min, Receiver::ImportedNamespace("math"), Args::All),
        (Lang::Go, "Max", 2) => (Builtin::Max, Receiver::ImportedNamespace("math"), Args::All),
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "abs", 1) => {
            (
                Builtin::Abs,
                Receiver::UnshadowedGlobal("Math"),
                Args::First,
            )
        }
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "min", 2) => {
            (Builtin::Min, Receiver::UnshadowedGlobal("Math"), Args::All)
        }
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "max", 2) => {
            (Builtin::Max, Receiver::UnshadowedGlobal("Math"), Args::All)
        }
        (Lang::Java, "abs", 1) => (
            Builtin::Abs,
            Receiver::UnshadowedGlobal("Math"),
            Args::First,
        ),
        (Lang::Java, "min", 2) => (Builtin::Min, Receiver::UnshadowedGlobal("Math"), Args::All),
        (Lang::Java, "max", 2) => (Builtin::Max, Receiver::UnshadowedGlobal("Math"), Args::All),
        (Lang::Rust, "zip", 1) => (
            Builtin::Zip,
            Receiver::ExactProtocolPairArgument,
            Args::RustZip,
        ),

        _ if method_fold_name(lang, name) && arg_count > 0 => {
            (Builtin::Reduce, Receiver::ExactProtocol, Args::Fold)
        }
        _ if method_bool_reduction_builtin(lang, name).is_some() && arg_count > 0 => (
            method_bool_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::BoolReduction,
        ),
        _ if method_collection_reduction_builtin(lang, name).is_some() && arg_count == 0 => (
            method_collection_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::CollectionReduction,
        ),
        _ if method_hof_contract(lang, name).is_some() && arg_count > 0 => {
            return Some(MethodCallContract {
                semantic: Semantic::HoF(method_hof_contract(lang, name).unwrap()),
                receiver: Receiver::ExactProtocol,
                args: Args::Hof,
            });
        }
        (Lang::Rust, "abs", 0) => (Builtin::Abs, Receiver::ExactInteger, Args::ReceiverOnly),
        _ => return None,
    };

    Some(MethodCallContract {
        semantic: Semantic::Builtin(contract.0),
        receiver: contract.1,
        args: contract.2,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsyncReceiverContract {
    ExactPromiseLike,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PromiseThenContract {
    pub receiver: AsyncReceiverContract,
}

pub fn promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<PromiseThenContract> {
    library_promise_then_contract(lang, method, arg_count).map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IteratorAdapterReceiverContract {
    ExactIterableValue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IteratorIdentityAdapterContract {
    pub receiver: IteratorAdapterReceiverContract,
}

pub fn iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<IteratorIdentityAdapterContract> {
    library_iterator_identity_adapter_contract(lang, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticCollectionAdapterContract {
    pub module: &'static str,
    pub exported: &'static str,
}

pub fn static_collection_adapter_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<StaticCollectionAdapterContract> {
    library_static_collection_adapter_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowedPathContract {
    pub shadow_root: &'static str,
}

fn rust_option_some_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "Some" => "Some",
        "Option::Some" => "Option::Some",
        "std::option::Option::Some" => "std::option::Option::Some",
        "core::option::Option::Some" => "core::option::Option::Some",
        _ => return None,
    })
}

fn rust_option_none_selector_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Rust {
        return None;
    }
    Some(match name {
        "None" => "None",
        "Option::None" => "Option::None",
        "std::option::Option::None" => "std::option::Option::None",
        "core::option::Option::None" => "core::option::Option::None",
        _ => return None,
    })
}

pub fn rust_option_some_constructor_contract(
    lang: Lang,
    name: &str,
) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Some" => "Some",
        "Option::Some" => "Option",
        "std::option::Option::Some" => "std",
        "core::option::Option::Some" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_option_none_sentinel_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "None" => "None",
        "Option::None" => "Option",
        "std::option::Option::None" => "std",
        "core::option::Option::None" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_vec_new_factory_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Vec::new" => "Vec",
        "std::vec::Vec::new" => "std",
        "alloc::vec::Vec::new" => "alloc",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_option_and_then_contract(lang: Lang, method: &str, arg_count: usize) -> bool {
    library_rust_option_and_then_contract(lang, method, arg_count).is_some()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaCollectionFactoryKind {
    ListOf,
    SetOf,
    ArraysAsList,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaCollectionFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaCollectionFactoryKind,
    pub single_arg_spreads_array: bool,
}

pub fn java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaCollectionFactoryContract> {
    if lang != Lang::Java {
        return None;
    }
    Some(match (receiver, method) {
        ("List", "of") => JavaCollectionFactoryContract {
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        },
        ("Set", "of") => JavaCollectionFactoryContract {
            receiver: "Set",
            method: "of",
            kind: JavaCollectionFactoryKind::SetOf,
            single_arg_spreads_array: false,
        },
        ("Arrays", "asList") => JavaCollectionFactoryContract {
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        },
        _ => return None,
    })
}

pub fn java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaCollectionConstructorKind {
    EmptyList,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaCollectionConstructorContract {
    pub simple_type: &'static str,
    pub qualified_type: &'static str,
    pub module: &'static str,
    pub kind: JavaCollectionConstructorKind,
    pub requires_import_for_simple_type: bool,
    pub requires_no_local_type_shadow: bool,
}

pub fn java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<JavaCollectionConstructorContract> {
    if lang != Lang::Java || arg_count != 0 {
        return None;
    }
    let simple_type = match type_name {
        "ArrayList" | "java.util.ArrayList" => "ArrayList",
        "LinkedList" | "java.util.LinkedList" => "LinkedList",
        _ => return None,
    };
    Some(JavaCollectionConstructorContract {
        simple_type,
        qualified_type: match simple_type {
            "ArrayList" => "java.util.ArrayList",
            "LinkedList" => "java.util.LinkedList",
            _ => return None,
        },
        module: "java.util",
        kind: JavaCollectionConstructorKind::EmptyList,
        requires_import_for_simple_type: true,
        requires_no_local_type_shadow: true,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaMapFactoryKind {
    Of,
    OfEntries,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaMapFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaMapFactoryKind,
}

pub fn java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaMapFactoryContract> {
    if lang != Lang::Java || receiver != "Map" {
        return None;
    }
    Some(match method {
        "of" => JavaMapFactoryContract {
            receiver: "Map",
            method: "of",
            kind: JavaMapFactoryKind::Of,
        },
        "ofEntries" => JavaMapFactoryContract {
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        },
        _ => return None,
    })
}

pub fn java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn java_map_entry_contract(lang: Lang, receiver: &str, method: &str) -> bool {
    lang == Lang::Java && receiver == "Map" && method == "entry"
}

pub fn java_map_entry_contract_by_hash(lang: Lang, receiver: &str, method_hash: u64) -> bool {
    java_map_entry_contract(lang, receiver, "entry") && method_hash == stable_symbol_hash("entry")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RubySetFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub required_module: &'static str,
    pub shadow_root: &'static str,
}

pub fn ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (lang == Lang::Ruby && receiver == "Set" && method == "new" && arg_count == 1).then_some(
        RubySetFactoryContract {
            receiver: "Set",
            method: "new",
            required_module: "set",
            shadow_root: "Set",
        },
    )
}

pub fn ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstructorProofRequirement {
    ConstructSyntax,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClosedConstructorContract {
    pub receiver: &'static str,
    pub required_proof: ConstructorProofRequirement,
    pub requires_unshadowed_global: bool,
    pub entry_seq_tag: Option<u64>,
}

pub fn js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Set").then_some(ClosedConstructorContract {
        receiver: "Set",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
        requires_unshadowed_global: true,
        entry_seq_tag: None,
    })
}

pub fn js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Map").then_some(ClosedConstructorContract {
        receiver: "Map",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
        requires_unshadowed_global: true,
        entry_seq_tag: Some(SEQ_VALUE_COLLECTION),
    })
}

pub(crate) fn js_like_lang(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKeyViewKind {
    Collection,
    Iterator,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewContract {
    pub method: &'static str,
    pub kind: MapKeyViewKind,
}

pub fn map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    library_map_key_view_contract(lang, method, arg_count).map(|contract| contract.result)
}

pub fn map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewWrapperContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub qualified_path: &'static str,
}

pub fn map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    library_map_key_view_wrapper_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

pub fn map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GoZeroMapLookupContract {
    pub map_literal_tag: &'static str,
    pub entry_tag: &'static str,
    pub canonical_value_tag: &'static str,
}

pub fn go_zero_map_lookup_contract(lang: Lang) -> Option<GoZeroMapLookupContract> {
    (lang == Lang::Go).then_some(GoZeroMapLookupContract {
        map_literal_tag: "composite_literal",
        entry_tag: "keyed_element",
        canonical_value_tag: "go_literal_zero_map",
    })
}

pub fn go_zero_map_literal_contract_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GoZeroMapLookupContract> {
    let contract = go_zero_map_lookup_contract(il.meta.lang)?;
    sequence_surface_evidence_matches_node(
        il,
        interner,
        node,
        SequenceSurfaceKind::GoCompositeMapLiteral,
    )
    .then_some(contract)
}

pub fn go_zero_map_entry_contract_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<GoZeroMapLookupContract> {
    let contract = go_zero_map_lookup_contract(il.meta.lang)?;
    sequence_surface_evidence_matches_node(il, interner, node, SequenceSurfaceKind::GoMapEntry)
        .then_some(contract)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GoZeroMapDefaultKind {
    Int,
    String,
    Bool,
    Float,
    Null,
}

pub fn go_zero_map_default_kind(lang: Lang, payload: Payload) -> Option<GoZeroMapDefaultKind> {
    if lang != Lang::Go {
        return None;
    }
    Some(match payload {
        Payload::LitInt(_) => GoZeroMapDefaultKind::Int,
        Payload::LitStr(_) => GoZeroMapDefaultKind::String,
        Payload::LitBool(_) => GoZeroMapDefaultKind::Bool,
        Payload::LitFloat(_) => GoZeroMapDefaultKind::Float,
        Payload::Lit(LitClass::Null) => GoZeroMapDefaultKind::Null,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapGetContract {
    pub method: &'static str,
    pub receiver: MethodReceiverContract,
}

pub fn map_get_contract(lang: Lang, method: &str, arg_count: usize) -> Option<MapGetContract> {
    library_map_get_contract(lang, method, arg_count).map(|contract| contract.result)
}

pub fn map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| map_get_contract(lang, "get", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeofOperatorContract {
    pub name: &'static str,
    pub required_source_fact: SourceFactKind,
}

pub fn typeof_operator_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<TypeofOperatorContract> {
    (js_like_lang(lang) && name == "typeof" && arg_count == 1).then_some(TypeofOperatorContract {
        name: "typeof",
        required_source_fact: SourceFactKind::Operator(SourceOperatorKind::Typeof),
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalMethodContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub qualified_path: &'static str,
    pub requires_unshadowed_receiver: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalFunctionContract {
    pub function: &'static str,
    pub requires_unshadowed_function: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticGlobalSymbolContract {
    pub name: &'static str,
    pub requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QualifiedGlobalSymbolContract {
    pub path: &'static str,
    pub root: &'static str,
    pub requires_unshadowed_root: bool,
}

pub fn static_global_symbol_contract(lang: Lang, name: &str) -> Option<StaticGlobalSymbolContract> {
    if !js_like_lang(lang) {
        return None;
    }
    let name = match name {
        "Array" => "Array",
        "Boolean" => "Boolean",
        "Map" => "Map",
        "Math" => "Math",
        "Object" => "Object",
        "Set" => "Set",
        "console" => "console",
        "undefined" => "undefined",
        _ => return None,
    };
    Some(StaticGlobalSymbolContract {
        name,
        requires_unshadowed: true,
    })
}

pub fn qualified_global_symbol_contract(
    lang: Lang,
    path: &str,
) -> Option<QualifiedGlobalSymbolContract> {
    if !js_like_lang(lang) {
        return None;
    }
    let (path, root) = match path {
        "Array.from" => ("Array.from", "Array"),
        "Array.isArray" => ("Array.isArray", "Array"),
        "Object.hasOwn" => ("Object.hasOwn", "Object"),
        "Object.prototype.hasOwnProperty.call" => {
            ("Object.prototype.hasOwnProperty.call", "Object")
        }
        _ => return None,
    };
    Some(QualifiedGlobalSymbolContract {
        path,
        root,
        requires_unshadowed_root: true,
    })
}

pub fn js_boolean_coercion_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<StaticGlobalFunctionContract> {
    library_js_boolean_coercion_contract(lang, function, arg_count).map(|contract| contract.result)
}

pub fn js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<StaticGlobalMethodContract> {
    library_js_array_is_array_contract(lang, receiver, method, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegexTestContract {
    pub method: &'static str,
    pub required_receiver_fact: SourceFactKind,
}

pub fn regex_test_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<RegexTestContract> {
    library_regex_test_contract(lang, method, arg_count).map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaticIndexMembershipKind {
    IndexOf,
    FindIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaticIndexMembershipReceiverContract {
    StaticNonFloatLiteralCollection,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticIndexMembershipContract {
    pub method: &'static str,
    pub kind: StaticIndexMembershipKind,
    pub receiver: StaticIndexMembershipReceiverContract,
}

pub fn static_index_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<StaticIndexMembershipContract> {
    if !js_like_lang(lang) || arg_count != 1 {
        return None;
    }
    Some(match method {
        "indexOf" => StaticIndexMembershipContract {
            method: "indexOf",
            kind: StaticIndexMembershipKind::IndexOf,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        },
        "findIndex" => StaticIndexMembershipContract {
            method: "findIndex",
            kind: StaticIndexMembershipKind::FindIndex,
            receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
        },
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexMembershipThreshold {
    MinusOne,
    Zero,
}

fn index_membership_threshold_matches(
    op: Op,
    index_call_on_right: bool,
    threshold: IndexMembershipThreshold,
) -> bool {
    match threshold {
        IndexMembershipThreshold::MinusOne => threshold_excludes_floor(op, index_call_on_right),
        IndexMembershipThreshold::Zero => threshold_reaches_floor(op, index_call_on_right),
    }
}

pub fn index_membership_threshold_contract(
    op: Op,
    index_call_on_right: bool,
    threshold: IndexMembershipThreshold,
) -> bool {
    index_membership_threshold_matches(op, index_call_on_right, threshold)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportedNamespaceFunctionSemantic {
    ProductReduction { op: Op, identity: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportedNamespaceFunctionContract {
    pub module: &'static str,
    pub function: &'static str,
    pub receiver: MethodReceiverContract,
    pub semantic: ImportedNamespaceFunctionSemantic,
}

pub fn imported_namespace_function_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<ImportedNamespaceFunctionContract> {
    library_imported_namespace_function_contract(lang, function, arg_count)
        .map(|contract| contract.result)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NullishGlobalContract {
    pub name: &'static str,
    pub requires_unshadowed: bool,
}

pub fn nullish_global_contract(lang: Lang, name: &str) -> Option<NullishGlobalContract> {
    (js_like_lang(lang) && name == "undefined").then_some(NullishGlobalContract {
        name: "undefined",
        requires_unshadowed: true,
    })
}

pub fn method_fold_name(lang: Lang, name: &str) -> bool {
    matches!(
        (lang, name),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "reduce"
        ) | (Lang::Ruby, "inject" | "reduce")
            | (Lang::Rust, "fold")
            | (Lang::Java, "reduce")
    )
}

pub fn method_bool_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "some") => {
            Builtin::Any
        }
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "every") => {
            Builtin::All
        }
        (Lang::Rust, "any") | (Lang::Ruby, "any?") | (Lang::Java, "anyMatch") => Builtin::Any,
        (Lang::Rust, "all") | (Lang::Ruby, "all?") | (Lang::Java, "allMatch") => Builtin::All,
        _ => return None,
    })
}

pub fn method_hof_contract(lang: Lang, name: &str) -> Option<HoFKind> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "map")
        | (Lang::Rust, "map")
        | (Lang::Java, "map")
        | (Lang::Ruby, "map" | "collect") => HoFKind::Map,
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "flatMap",
        )
        | (Lang::Rust, "flat_map")
        | (Lang::Java, "flatMap") => HoFKind::FlatMap,
        (Lang::Rust, "filter_map") => HoFKind::FilterMap,
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "filter")
        | (Lang::Rust, "filter")
        | (Lang::Java, "filter")
        | (Lang::Ruby, "filter" | "select") => HoFKind::Filter,
        _ => return None,
    })
}

pub fn method_collection_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::Rust, "sum") => Builtin::Sum,
        (Lang::Rust, "min") => Builtin::Min,
        (Lang::Rust, "max") => Builtin::Max,
        (Lang::Rust, "count") => Builtin::Len,
        (Lang::Java, "count") => Builtin::Len,
        _ => return None,
    })
}

pub fn property_builtin_contract(lang: Lang, name: &str) -> Option<Builtin> {
    library_property_builtin_contract(lang, name).map(|contract| contract.result)
}

fn property_builtin_contract_shape(
    lang: Lang,
    name: &str,
) -> Option<(Builtin, MethodReceiverContract)> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "length") => {
            (Builtin::Len, MethodReceiverContract::ExactCollection)
        }
        (Lang::Java, "length") => (Builtin::Len, MethodReceiverContract::ExactCollection),
        _ => return None,
    })
}

pub fn library_property_builtin_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryPropertyBuiltinContract> {
    let (result, receiver) = property_builtin_contract_shape(lang, name)?;
    let property = library_property_selector_name(name)?;
    Some(LibraryPropertyBuiltinContract {
        id: LibraryApiContractId::PropertyBuiltin(result),
        callee: LibraryApiCalleeContract::Property { property, receiver },
        result,
    })
}

fn library_property_selector_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "length" => "length",
        _ => return None,
    })
}

pub fn library_scalar_integer_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryScalarIntegerMethodContract> {
    let result = scalar_integer_method_contract_shape(lang, method, arg_count)?;
    let method = library_method_selector_name(method)?;
    Some(LibraryScalarIntegerMethodContract {
        id: LibraryApiContractId::ScalarIntegerMethod(result.semantic),
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_rust_option_some_constructor_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryRustOptionConstructorContract> {
    if arg_count != 1 {
        return None;
    }
    let name = rust_option_some_selector_name(lang, name)?;
    let shadow = rust_option_some_constructor_contract(lang, name)?;
    Some(LibraryRustOptionConstructorContract {
        id: LibraryApiContractId::RustOptionSomeConstructor,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Option,
    })
}

pub fn library_rust_option_none_sentinel_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryRustOptionSentinelContract> {
    let name = rust_option_none_selector_name(lang, name)?;
    let shadow = rust_option_none_sentinel_contract(lang, name)?;
    Some(LibraryRustOptionSentinelContract {
        id: LibraryApiContractId::RustOptionNoneSentinel,
        callee: LibraryApiCalleeContract::FreeName {
            name,
            shadow: LibraryApiShadowPolicy::ExplicitRoot(shadow.shadow_root),
        },
        result_domain: DomainEvidence::Option,
    })
}

pub fn library_rust_option_and_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryRustOptionAndThenContract> {
    if lang != Lang::Rust || method != "and_then" || arg_count != 1 {
        return None;
    }
    Some(LibraryRustOptionAndThenContract {
        id: LibraryApiContractId::RustOptionAndThen,
        callee: LibraryApiCalleeContract::Method {
            method: "and_then",
            receiver: MethodReceiverContract::RustMapGetOrExactOption,
        },
        result: RustOptionAndThenContract {
            receiver: MethodReceiverContract::RustMapGetOrExactOption,
        },
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CollectionSemantics {
    lang: Lang,
}

impl CollectionSemantics {
    /// Python's empty `Seq(0)` literal is a collection value for first-party exact
    /// collection contracts.
    pub fn empty_sequence_is_collection(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn ruby_shovel_list_append(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn free_name_collection_factories(self) -> impl Iterator<Item = FreeNameCollectionFactory> {
        FREE_NAME_COLLECTION_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }

    pub fn free_name_map_factories(self) -> impl Iterator<Item = FreeNameMapFactory> {
        FREE_NAME_MAP_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }

    pub fn imported_collection_factories(self) -> impl Iterator<Item = ImportedCollectionFactory> {
        IMPORTED_COLLECTION_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeNameCollectionFactory {
    pub lang: Option<Lang>,
    pub names: &'static [&'static str],
    pub shadow_guard: bool,
}

const FREE_NAME_COLLECTION_FACTORIES: &[FreeNameCollectionFactory] = &[
    FreeNameCollectionFactory {
        lang: Some(Lang::Python),
        names: &["list", "set", "frozenset", "tuple"],
        shadow_guard: true,
    },
    FreeNameCollectionFactory {
        lang: Some(Lang::Rust),
        names: &[
            "std::collections::HashSet::from",
            "std::collections::BTreeSet::from",
            "std::collections::VecDeque::from",
        ],
        shadow_guard: false,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeNameMapFactory {
    pub lang: Option<Lang>,
    pub names: &'static [&'static str],
    pub entry_seq_tag: u64,
}

const FREE_NAME_MAP_FACTORIES: &[FreeNameMapFactory] = &[FreeNameMapFactory {
    lang: Some(Lang::Rust),
    names: &[
        "std::collections::HashMap::from",
        "std::collections::BTreeMap::from",
    ],
    entry_seq_tag: SEQ_VALUE_TUPLE,
}];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportedCollectionFactory {
    pub lang: Option<Lang>,
    pub module: &'static str,
    pub exported: &'static str,
}

const IMPORTED_COLLECTION_FACTORIES: &[ImportedCollectionFactory] = &[ImportedCollectionFactory {
    lang: Some(Lang::Python),
    module: "collections",
    exported: "deque",
}];

pub fn imported_literal_seq_tag_safe(lang: Lang, tag: &str) -> bool {
    seq_surface_contract(lang, Some(tag)).is_some_and(|contract| contract.imported_literal)
}

#[cfg(test)]
mod tests;
