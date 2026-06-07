//! Semantic contracts for language and library facts used by exact matching.
//!
//! This crate is the first-party semantic-kernel facade. The initial migration is
//! deliberately behavior-preserving: it names the semantic assumptions that were
//! previously encoded as scattered `Lang` matches. Future pack loading should
//! extend this contract surface rather than letting packs mint fingerprints or
//! approve exact clone matches directly.

use nose_il::{
    contains_js_identifier, stable_symbol_hash, Builtin, EvidenceAnchor, EvidenceKind,
    EvidenceStatus, HoFKind, Il, ImportEvidenceKind, Interner, Lang, LitClass, NodeId, NodeKind,
    Op, ParamSemantic, Payload, SequenceSurfaceKind, SourceCallKind, SourceFactKind,
    SourceLiteralKind, SourceOperatorKind, Span, SymbolEvidenceKind,
};

pub use nose_il::DomainEvidence;

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

/// Source facts are evidence records emitted by a language frontend or future
/// pack. They preserve source distinctions that the shared IL intentionally
/// abstracts away; a fact only matters when a semantic contract consumes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceFactContract {
    pub kind: SourceFactKind,
    pub channel: ChannelEligibility,
}

pub fn source_fact_contract(kind: SourceFactKind) -> SourceFactContract {
    SourceFactContract {
        kind,
        channel: ChannelEligibility::ExactProven,
    }
}

enum EvidenceResolution<T> {
    Missing,
    Found(T),
    Ambiguous,
}

fn unique_evidence_at<T: Copy + Eq>(
    il: &Il,
    anchor_matches: impl Fn(EvidenceAnchor) -> bool,
    project: impl Fn(EvidenceKind) -> Option<T>,
) -> EvidenceResolution<T> {
    let mut found = None;
    for record in &il.evidence {
        if !anchor_matches(record.anchor) {
            continue;
        }
        let Some(value) = project(record.kind) else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(value),
            Some(existing) if existing == value => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn evidence_at_span<T: Copy + Eq>(
    il: &Il,
    span: Span,
    project: impl Fn(EvidenceKind) -> Option<T>,
) -> EvidenceResolution<T> {
    unique_evidence_at(il, |anchor| anchor.matches_span(span), project)
}

pub fn source_fact_at_node(il: &Il, node: NodeId, kind: SourceFactKind) -> bool {
    match kind {
        SourceFactKind::Operator(operator) => source_operator_at_node(il, node) == Some(operator),
        SourceFactKind::Call(call) => source_call_at_node(il, node) == Some(call),
        SourceFactKind::Literal(literal) => source_literal_at_node(il, node) == Some(literal),
    }
}

pub fn source_operator_at_node(il: &Il, node: NodeId) -> Option<SourceOperatorKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Operator(operator)) => Some(operator),
        _ => None,
    }) {
        EvidenceResolution::Found(operator) => Some(operator),
        EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => {
            unique_legacy_source_fact(il.source_facts.iter().filter_map(|fact| {
                (fact.span == span)
                    .then_some(fact.kind)
                    .and_then(|kind| match kind {
                        SourceFactKind::Operator(operator) => Some(operator),
                        SourceFactKind::Call(_) | SourceFactKind::Literal(_) => None,
                    })
            }))
        }
    }
}

pub fn source_call_at_node(il: &Il, node: NodeId) -> Option<SourceCallKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
        _ => None,
    }) {
        EvidenceResolution::Found(call) => Some(call),
        EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => {
            unique_legacy_source_fact(il.source_facts.iter().filter_map(|fact| {
                (fact.span == span)
                    .then_some(fact.kind)
                    .and_then(|kind| match kind {
                        SourceFactKind::Call(call) => Some(call),
                        SourceFactKind::Operator(_) | SourceFactKind::Literal(_) => None,
                    })
            }))
        }
    }
}

pub fn source_literal_at_node(il: &Il, node: NodeId) -> Option<SourceLiteralKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Literal(literal)) => Some(literal),
        _ => None,
    }) {
        EvidenceResolution::Found(literal) => Some(literal),
        EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => {
            unique_legacy_source_fact(il.source_facts.iter().filter_map(|fact| {
                (fact.span == span)
                    .then_some(fact.kind)
                    .and_then(|kind| match kind {
                        SourceFactKind::Literal(literal) => Some(literal),
                        SourceFactKind::Operator(_) | SourceFactKind::Call(_) => None,
                    })
            }))
        }
    }
}

fn unique_legacy_source_fact<T: Copy + Eq>(facts: impl Iterator<Item = T>) -> Option<T> {
    let mut found = None;
    for fact in facts {
        match found {
            None => found = Some(fact),
            Some(existing) if existing == fact => {}
            Some(_) => return None,
        }
    }
    found
}

pub fn construct_syntax_proof(il: &Il, node: NodeId) -> bool {
    source_call_at_node(il, node) == Some(SourceCallKind::Construct)
}

pub fn regex_literal_proof(il: &Il, node: NodeId) -> bool {
    source_literal_at_node(il, node) == Some(SourceLiteralKind::Regex)
}

pub fn exact_static_membership_predicate_operator(
    lang: Lang,
    op: Op,
    source: SourceOperatorKind,
) -> bool {
    js_like_lang(lang)
        && matches!(
            (op, source),
            (Op::Eq, SourceOperatorKind::StrictEquality)
                | (Op::Ne, SourceOperatorKind::StrictInequality)
        )
}

pub fn domain_evidence_from_param_semantic(semantic: ParamSemantic) -> DomainEvidence {
    DomainEvidence::from_param_semantic(semantic)
}

pub fn domain_evidence_at_span(il: &Il, span: Span) -> Option<DomainEvidence> {
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Domain(domain) => Some(domain),
        _ => None,
    }) {
        EvidenceResolution::Found(domain) => Some(domain),
        EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => {
            let mut found = None;
            for fact in il.param_type_facts.iter().filter(|fact| fact.span == span) {
                let domain = domain_evidence_from_param_semantic(fact.semantic);
                match found {
                    None => found = Some(domain),
                    Some(existing) if existing == domain => {}
                    Some(_) => return None,
                }
            }
            found
        }
    }
}

pub fn domain_evidence_for_param(il: &Il, param: NodeId) -> Option<DomainEvidence> {
    (il.kind(param) == NodeKind::Param)
        .then_some(il.node(param).span)
        .and_then(|span| domain_evidence_at_span(il, span))
}

pub const SEQ_VALUE_UNTAGGED: u64 = 0;
pub const SEQ_VALUE_COLLECTION: u64 = 1;
pub const SEQ_VALUE_TUPLE: u64 = 2;
pub const SEQ_VALUE_MAP: u64 = 3;
pub const SEQ_VALUE_PAIR: u64 = 4;
pub const SEQ_VALUE_IMPORT_BINDING: u64 = 5;
pub const SEQ_VALUE_IMPORT_NAMESPACE: u64 = 6;
pub const SEQ_VALUE_RECORD_GUARD: u64 = 7;
pub const SEQ_VALUE_OWN_PROPERTY_GUARD: u64 = 8;

pub const IMPORT_BINDING_TAG: &str = "import_binding";
pub const IMPORT_NAMESPACE_TAG: &str = "import_namespace";

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
        Some("array" | "array_expression" | "list") => Some(SequenceSurfaceKind::Collection),
        Some("tuple" | "tuple_expression") => Some(SequenceSurfaceKind::Tuple),
        Some("dictionary" | "object" | "hash") => Some(SequenceSurfaceKind::Map),
        Some("pair") => Some(SequenceSurfaceKind::Pair),
        Some(IMPORT_BINDING_TAG) => Some(SequenceSurfaceKind::ImportBinding),
        Some(IMPORT_NAMESPACE_TAG) => Some(SequenceSurfaceKind::ImportNamespace),
        Some("record_guard") => Some(SequenceSurfaceKind::RecordGuard),
        Some("own_property_guard") => Some(SequenceSurfaceKind::OwnPropertyGuard),
        Some("composite_literal") if lang == Lang::Go => {
            Some(SequenceSurfaceKind::GoCompositeMapLiteral)
        }
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
        SequenceSurfaceKind::ImportBinding => SeqSurfaceContract {
            value_tag: SEQ_VALUE_IMPORT_BINDING,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::ImportNamespace => SeqSurfaceContract {
            value_tag: SEQ_VALUE_IMPORT_NAMESPACE,
            exact_tree_safe: true,
            membership_collection: false,
            map_entry_list: false,
            imported_literal: false,
        },
        SequenceSurfaceKind::RecordGuard => SeqSurfaceContract {
            value_tag: SEQ_VALUE_RECORD_GUARD,
            exact_tree_safe: true,
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
    let span = il.node(node).span;
    match unique_evidence_at(
        il,
        |anchor| matches!(anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span),
        |evidence| match evidence {
            EvidenceKind::SequenceSurface(kind) => Some(kind),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(kind) => {
            let (raw_kind, raw_contract) = seq_surface_contract_for_tag(il.meta.lang, raw_tag)?;
            (kind == raw_kind).then_some(raw_contract)
        }
        EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => seq_surface_contract(il.meta.lang, raw_tag),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportFactKind {
    Binding,
    Namespace,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportFactContract {
    pub kind: ImportFactKind,
    pub tag: &'static str,
    pub coordinate_count: usize,
    pub value_tag: u64,
    pub channel: ChannelEligibility,
}

pub fn import_fact_contract(kind: ImportFactKind) -> ImportFactContract {
    match kind {
        ImportFactKind::Binding => ImportFactContract {
            kind,
            tag: IMPORT_BINDING_TAG,
            coordinate_count: 2,
            value_tag: SEQ_VALUE_IMPORT_BINDING,
            channel: ChannelEligibility::ExactProven,
        },
        ImportFactKind::Namespace => ImportFactContract {
            kind,
            tag: IMPORT_NAMESPACE_TAG,
            coordinate_count: 1,
            value_tag: SEQ_VALUE_IMPORT_NAMESPACE,
            channel: ChannelEligibility::ExactProven,
        },
    }
}

pub fn import_fact_contract_for_tag(tag: &str) -> Option<ImportFactContract> {
    match tag {
        IMPORT_BINDING_TAG => Some(import_fact_contract(ImportFactKind::Binding)),
        IMPORT_NAMESPACE_TAG => Some(import_fact_contract(ImportFactKind::Namespace)),
        _ => None,
    }
}

pub fn import_fact_tag(kind: ImportFactKind) -> &'static str {
    import_fact_contract(kind).tag
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

/// Evidence-only import fact resolution for semantic consumers. This intentionally
/// does not parse the legacy raw `Seq("import_*")` payload: callers that use this
/// helper are relying on a provider-owned evidence record, not on tag spelling.
pub fn import_fact_evidence_rhs(il: &Il, rhs: NodeId) -> Option<ImportFact> {
    if il.kind(rhs) != NodeKind::Seq {
        return None;
    }
    match import_fact_evidence_at_sequence_span(il, il.node(rhs).span) {
        EvidenceResolution::Found(fact) => Some(fact),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn import_fact_rhs(il: &Il, interner: &Interner, rhs: NodeId) -> Option<ImportFact> {
    if il.kind(rhs) != NodeKind::Seq {
        return None;
    }
    match import_fact_evidence_at_sequence_span(il, il.node(rhs).span) {
        EvidenceResolution::Found(fact) => return Some(fact),
        EvidenceResolution::Ambiguous => return None,
        EvidenceResolution::Missing => {}
    }
    let Payload::Name(tag) = il.node(rhs).payload else {
        return None;
    };
    let contract = import_fact_contract_for_tag(interner.resolve(tag))?;
    let coords = il.children(rhs);
    if coords.len() != contract.coordinate_count {
        return None;
    }
    let module_hash = literal_string_hash(il, coords[0])?;
    let exported_hash = match contract.kind {
        ImportFactKind::Binding => Some(literal_string_hash(il, coords[1])?),
        ImportFactKind::Namespace => None,
    };
    Some(ImportFact {
        kind: contract.kind,
        module_hash,
        exported_hash,
    })
}

pub fn import_binding_rhs_matches(
    il: &Il,
    interner: &Interner,
    rhs: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    import_fact_rhs(il, interner, rhs).is_some_and(|fact| {
        fact.kind == ImportFactKind::Binding
            && fact.module_hash == stable_symbol_hash(module)
            && fact.exported_hash == Some(stable_symbol_hash(exported))
    })
}

pub fn import_namespace_rhs_matches(
    il: &Il,
    interner: &Interner,
    rhs: NodeId,
    module: &str,
) -> bool {
    import_fact_rhs(il, interner, rhs).is_some_and(|fact| {
        fact.kind == ImportFactKind::Namespace
            && fact.module_hash == stable_symbol_hash(module)
            && fact.exported_hash.is_none()
    })
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
    unique_evidence_at(
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
    if qualified_global_symbol_contract(il.meta.lang, path).is_none() {
        return false;
    }
    let expected = SymbolEvidenceKind::QualifiedGlobal {
        path_hash: stable_symbol_hash(path),
    };
    match symbol_evidence_at_node_anchor(il, span, kind) {
        EvidenceResolution::Found(actual) => actual == expected,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => false,
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
    match symbol_identity_at_node_matches(il, node, expected) {
        EvidenceResolution::Found(matches) => return matches,
        EvidenceResolution::Ambiguous => return false,
        EvidenceResolution::Missing => {}
    }
    let Some(local_hash) = node_name_hash(il, interner, node) else {
        return false;
    };
    if unit_defines_hash(il, interner, local_hash) {
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

impl OperatorSemantics {
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
    /// the C lowering, where explicit unsigned facts are recovered by the frontend.
    pub fn c_integer_byte_pack_contracts(self) -> bool {
        self.lang == Lang::C
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
pub struct EffectSemantics {
    lang: Lang,
}

impl EffectSemantics {
    /// `target[key] = value` is modeled as a non-overloadable observable index
    /// write. Languages with user-dispatched index assignment must stay fail-closed
    /// unless a future pack emits a stronger receiver proof.
    pub fn non_overloadable_index_assignment(self) -> bool {
        matches!(self.lang, Lang::C | Lang::Go | Lang::Java)
    }

    /// Exact field-write fragments currently require Java's fixed `this.field`
    /// receiver proof.
    pub fn java_this_field_place(self) -> bool {
        self.lang == Lang::Java
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FragmentSemantics {
    lang: Lang,
}

impl FragmentSemantics {
    pub fn non_overloadable_index_assignment(self) -> bool {
        EffectSemantics { lang: self.lang }.non_overloadable_index_assignment()
    }

    pub fn java_this_field_place(self) -> bool {
        EffectSemantics { lang: self.lang }.java_this_field_place()
    }
}

/// Exact Java `this` receiver proof for first-party self-field fragments.
pub fn exact_java_this_var(il: &Il, interner: &Interner, node: NodeId) -> bool {
    semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        && il.kind(node) == NodeKind::Var
        && matches!(il.node(node).payload, Payload::Name(name) if interner.resolve(name) == "this")
}

/// Exact Java `this.field` place proof for receiver-aware field-write fingerprints.
pub fn exact_java_this_field(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Field
    {
        return false;
    }
    if !matches!(il.node(node).payload, Payload::Name(_)) {
        return false;
    }
    il.children(node)
        .first()
        .is_some_and(|&receiver| exact_java_this_var(il, interner, receiver))
}

/// Exact Java `return this` proof used by self-field body fragments.
pub fn exact_java_return_this(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Return
    {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 1 && exact_java_this_var(il, interner, kids[0])
}

/// `(receiver, key, value)` of a first-party exact-safe index assignment.
///
/// This is intentionally language-gated: languages with overloadable/user-dispatched index
/// assignment remain fail-closed until a pack supplies a stronger receiver proof.
pub fn exact_non_overloadable_index_assignment_parts(
    il: &Il,
    node: NodeId,
) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .non_overloadable_index_assignment()
    {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Index {
        return None;
    }
    let target = il.children(kids[0]);
    Some((*target.first()?, target.get(1).copied(), kids[1]))
}

pub fn exact_non_overloadable_index_assignment(il: &Il, node: NodeId) -> bool {
    exact_non_overloadable_index_assignment_parts(il, node).is_some()
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinDemand {
    Eager,
    Reduce,
    AnyAll { all: bool },
    Append,
    ValueOrDefault,
}

pub fn builtin_demand(builtin: Builtin) -> BuiltinDemand {
    match builtin {
        Builtin::Reduce => BuiltinDemand::Reduce,
        Builtin::Any => BuiltinDemand::AnyAll { all: false },
        Builtin::All => BuiltinDemand::AnyAll { all: true },
        Builtin::Append => BuiltinDemand::Append,
        Builtin::ValueOrDefault => BuiltinDemand::ValueOrDefault,
        _ => BuiltinDemand::Eager,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EagerBuiltinContract {
    Len,
    IsEmpty,
    IsNull,
    IsNotNull,
    StartsWith,
    EndsWith,
    Contains,
    Join,
    Abs,
    UnsignedCast32,
    Sum,
    Min,
    Max,
    Range,
    Zip,
    Enumerate,
    Keys,
    Print,
    DictEntry,
    GetOrDefault,
}

pub fn eager_builtin_contract(builtin: Builtin) -> Option<EagerBuiltinContract> {
    Some(match builtin {
        Builtin::Len => EagerBuiltinContract::Len,
        Builtin::IsEmpty => EagerBuiltinContract::IsEmpty,
        Builtin::IsNull => EagerBuiltinContract::IsNull,
        Builtin::IsNotNull => EagerBuiltinContract::IsNotNull,
        Builtin::StartsWith => EagerBuiltinContract::StartsWith,
        Builtin::EndsWith => EagerBuiltinContract::EndsWith,
        Builtin::Contains => EagerBuiltinContract::Contains,
        Builtin::Join => EagerBuiltinContract::Join,
        Builtin::Abs => EagerBuiltinContract::Abs,
        Builtin::UnsignedCast32 => EagerBuiltinContract::UnsignedCast32,
        Builtin::Sum => EagerBuiltinContract::Sum,
        Builtin::Min => EagerBuiltinContract::Min,
        Builtin::Max => EagerBuiltinContract::Max,
        Builtin::Range => EagerBuiltinContract::Range,
        Builtin::Zip => EagerBuiltinContract::Zip,
        Builtin::Enumerate => EagerBuiltinContract::Enumerate,
        Builtin::Keys => EagerBuiltinContract::Keys,
        Builtin::Print => EagerBuiltinContract::Print,
        Builtin::DictEntry => EagerBuiltinContract::DictEntry,
        Builtin::GetOrDefault => EagerBuiltinContract::GetOrDefault,
        Builtin::Reduce
        | Builtin::Any
        | Builtin::All
        | Builtin::Append
        | Builtin::ValueOrDefault => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReductionBuiltinContract {
    Len,
    Sum,
    ExplicitFold,
    Selection { max: bool },
    Bool { all: bool },
    Join,
}

pub fn reduction_builtin_contract(builtin: Builtin) -> Option<ReductionBuiltinContract> {
    Some(match builtin {
        Builtin::Len => ReductionBuiltinContract::Len,
        Builtin::Sum => ReductionBuiltinContract::Sum,
        Builtin::Reduce => ReductionBuiltinContract::ExplicitFold,
        Builtin::Min => ReductionBuiltinContract::Selection { max: false },
        Builtin::Max => ReductionBuiltinContract::Selection { max: true },
        Builtin::Any => ReductionBuiltinContract::Bool { all: false },
        Builtin::All => ReductionBuiltinContract::Bool { all: true },
        Builtin::Join => ReductionBuiltinContract::Join,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HofContract {
    Map,
    FlatMap,
    FilterMap,
    Filter,
    Reduce,
}

pub fn hof_contract(kind: HoFKind) -> HofContract {
    match kind {
        HoFKind::Map => HofContract::Map,
        HoFKind::FlatMap => HofContract::FlatMap,
        HoFKind::FilterMap => HofContract::FilterMap,
        HoFKind::Filter => HofContract::Filter,
        HoFKind::Reduce => HofContract::Reduce,
    }
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
    pub builtin: Builtin,
    pub args: BuiltinArgContract,
    pub requires_unshadowed: bool,
}

pub fn free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<FreeFunctionBuiltinContract> {
    let contract = match name {
        "len" if matches!(lang, Lang::Python | Lang::Go) && arg_count == 1 => {
            (Builtin::Len, BuiltinArgContract::First)
        }
        "append" if lang == Lang::Go && arg_count >= 2 => {
            (Builtin::Append, BuiltinArgContract::All)
        }
        "print" if lang == Lang::Python => (Builtin::Print, BuiltinArgContract::All),
        "range" if lang == Lang::Python => (Builtin::Range, BuiltinArgContract::All),
        "sum" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Sum, BuiltinArgContract::First)
        }
        "min" if lang == Lang::Python && (arg_count == 1 || arg_count == 2) => {
            (Builtin::Min, BuiltinArgContract::All)
        }
        "max" if lang == Lang::Python && (arg_count == 1 || arg_count == 2) => {
            (Builtin::Max, BuiltinArgContract::All)
        }
        "abs" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Abs, BuiltinArgContract::First)
        }
        "zip" if lang == Lang::Python && arg_count == 2 => (Builtin::Zip, BuiltinArgContract::All),
        "enumerate" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Enumerate, BuiltinArgContract::First)
        }
        "any" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Any, BuiltinArgContract::First)
        }
        "all" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::All, BuiltinArgContract::First)
        }
        _ => return None,
    };
    Some(FreeFunctionBuiltinContract {
        builtin: contract.0,
        args: contract.1,
        requires_unshadowed: true,
    })
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

pub fn scalar_integer_method_contract(
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

pub fn method_call_contract(
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
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "length",
            0,
        ) => (Builtin::Len, Receiver::ExactCollection, Args::ReceiverOnly),
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
    if matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    ) && method == "then"
        && arg_count == 1
    {
        Some(PromiseThenContract {
            receiver: AsyncReceiverContract::ExactPromiseLike,
        })
    } else {
        None
    }
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
    if lang == Lang::Rust
        && arg_count == 0
        && matches!(
            method,
            "iter" | "into_iter" | "iter_mut" | "collect" | "to_vec" | "copied" | "cloned"
        )
        || lang == Lang::Java && method == "stream" && arg_count == 0
    {
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    } else {
        None
    }
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
    (lang == Lang::Java && receiver == "Arrays" && method == "stream" && arg_count == 1).then_some(
        StaticCollectionAdapterContract {
            module: "java.util",
            exported: "Arrays",
        },
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowedPathContract {
    pub shadow_root: &'static str,
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
    lang == Lang::Rust && method == "and_then" && arg_count == 1
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

fn js_like_lang(lang: Lang) -> bool {
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
    if arg_count != 0 {
        return None;
    }
    Some(match (lang, method) {
        (Lang::Python | Lang::Ruby, "keys") => MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::Java, "keySet") => MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "keys") => {
            MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            }
        }
        _ => return None,
    })
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
    (js_like_lang(lang) && receiver == "Array" && method == "from" && arg_count == 1).then_some(
        MapKeyViewWrapperContract {
            receiver: "Array",
            method: "from",
            qualified_path: "Array.from",
        },
    )
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
    matches!(
        lang,
        Lang::Java
            | Lang::Rust
            | Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
    )
    .then_some(())
    .filter(|_| method == "get" && arg_count == 1)
    .map(|_| MapGetContract {
        method: "get",
        receiver: MethodReceiverContract::ExactMap,
    })
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
}

pub fn typeof_operator_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<TypeofOperatorContract> {
    (js_like_lang(lang) && name == "typeof" && arg_count == 1)
        .then_some(TypeofOperatorContract { name: "typeof" })
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
    (js_like_lang(lang) && function == "Boolean" && arg_count == 1).then_some(
        StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        },
    )
}

pub fn js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<StaticGlobalMethodContract> {
    (js_like_lang(lang) && receiver == "Array" && method == "isArray" && arg_count == 1).then_some(
        StaticGlobalMethodContract {
            receiver: "Array",
            method: "isArray",
            qualified_path: "Array.isArray",
            requires_unshadowed_receiver: true,
        },
    )
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
    (js_like_lang(lang) && method == "test" && arg_count == 1).then_some(RegexTestContract {
        method: "test",
        required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
    })
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
    Some(match (lang, function, arg_count) {
        (Lang::Python, "prod", 1 | 2) => ImportedNamespaceFunctionContract {
            module: "math",
            function: "prod",
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        },
        _ => return None,
    })
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

pub fn builder_append_method_contract(lang: Lang, method: &str, arg_count: usize) -> bool {
    matches!(
        (lang, method, arg_count),
        (Lang::Python, "append", 1)
            | (
                Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
                "push",
                1
            )
            | (Lang::Java, "add", 1)
            | (Lang::Rust, "push", 1)
    )
}

/// `(receiver, value)` of a single-item append-like builder call admitted by first-party
/// language/library contracts.
///
/// This intentionally reads only the canonical `Builtin::Append` surface. Raw method
/// selectors such as `push`, `append`, or `add` are not proof by themselves; callers that
/// see those selectors must first prove the receiver/builder contract and lower the call to
/// the canonical builtin.
pub fn builder_append_call_args(
    il: &Il,
    _interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return (kids.len() == 2).then(|| (kids[0], kids[1]));
    }
    None
}

pub fn builder_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    builder_append_call_args(il, interner, node).is_some()
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
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "length") => {
            Builtin::Len
        }
        (Lang::Java, "length") => Builtin::Len,
        _ => return None,
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

pub fn mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "clear"
            | "delete"
            | "insert"
            | "pop"
            | "popitem"
            | "put"
            | "putAll"
            | "remove"
            | "set"
            | "setdefault"
            | "update"
    )
}

pub fn module_binding_mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "add"
            | "addAll"
            | "append"
            | "delete"
            | "clear"
            | "compute"
            | "computeIfAbsent"
            | "computeIfPresent"
            | "merge"
            | "pop"
            | "push"
            | "put"
            | "putAll"
            | "remove"
            | "removeAll"
            | "removeIf"
            | "replace"
            | "replaceAll"
            | "retainAll"
            | "shift"
            | "sort"
            | "splice"
            | "unshift"
            | "set"
    )
}

pub fn async_to_sync_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Python {
        return None;
    }
    Some(match name {
        "__aenter__" => "__enter__",
        "__aexit__" => "__exit__",
        "__anext__" => "__next__",
        "__aiter__" => "__iter__",
        "aread" => "read",
        "areadline" => "readline",
        "areadlines" => "readlines",
        "awrite" => "write",
        "aclose" => "close",
        "asend" => "send",
        "areceive" => "receive",
        "aconnect" => "connect",
        "adrain" => "drain",
        "aflush" => "flush",
        "AsyncIterable" => "Iterable",
        "AsyncIterator" => "Iterator",
        "AsyncGenerator" => "Generator",
        "AsyncContextManager" => "ContextManager",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance,
        EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, ImportEvidenceKind,
        ParamSemantic, ParamTypeFact, SequenceSurfaceKind, SourceFact, Span, SymbolEvidenceKind,
        Unit, UnitKind,
    };

    const ALL_LANGS: &[Lang] = &[
        Lang::Python,
        Lang::JavaScript,
        Lang::TypeScript,
        Lang::Go,
        Lang::Rust,
        Lang::Java,
        Lang::C,
        Lang::Ruby,
        Lang::Vue,
        Lang::Svelte,
        Lang::Html,
    ];

    const ALL_BUILTINS: &[Builtin] = &[
        Builtin::Len,
        Builtin::Print,
        Builtin::Append,
        Builtin::Range,
        Builtin::Sum,
        Builtin::Reduce,
        Builtin::Min,
        Builtin::Max,
        Builtin::Abs,
        Builtin::Zip,
        Builtin::Enumerate,
        Builtin::Keys,
        Builtin::Any,
        Builtin::All,
        Builtin::DictEntry,
        Builtin::IsEmpty,
        Builtin::StartsWith,
        Builtin::EndsWith,
        Builtin::Contains,
        Builtin::GetOrDefault,
        Builtin::ValueOrDefault,
        Builtin::IsNull,
        Builtin::IsNotNull,
        Builtin::Join,
        Builtin::UnsignedCast32,
    ];

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, 1, 1)
    }

    fn finish_il(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
        builder.finish(
            root,
            FileMeta {
                path: "t".into(),
                lang,
            },
            vec![Unit {
                root,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        )
    }

    fn evidence(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("test")),
            },
            dependencies: Vec::new(),
            status,
        }
    }

    #[test]
    fn first_party_profile_wraps_each_language() {
        for &lang in ALL_LANGS {
            let profile = semantics(lang);
            assert_eq!(profile.lang(), lang);
            assert_eq!(profile.pack_id(), FIRST_PARTY_PACK_ID);
            assert_eq!(profile.trust(), PackTrust::DefaultFirstParty);
        }
    }

    #[test]
    fn domain_evidence_preserves_param_semantic_boundaries() {
        assert_eq!(
            domain_evidence_from_param_semantic(ParamSemantic::Array),
            DomainEvidence::Array
        );
        assert!(DomainEvidence::Array.is_array_collection_or_set());
        assert!(DomainEvidence::Collection.is_array_or_collection());
        assert!(DomainEvidence::Set.is_collection_or_set());
        assert!(DomainEvidence::Map.is_map());
        assert!(DomainEvidence::Option.is_option());
        assert!(DomainEvidence::String.is_string());
        assert!(DomainEvidence::ByteArray.is_byte_array());
        assert!(DomainEvidence::Integer.is_integer());
        assert!(DomainEvidence::Number.is_integer_or_number());
        assert!(DomainEvidence::Integer.is_integer_or_number());
        assert!(!DomainEvidence::Number.is_integer());
        assert!(!DomainEvidence::Array.is_collection_or_set());
        assert!(!DomainEvidence::Set.is_array_or_collection());
    }

    #[test]
    fn domain_evidence_records_are_preferred_over_legacy_param_facts() {
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::None, sp(3), &[]);
        let root = b.add(NodeKind::Func, Payload::None, sp(3), &[param]);
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.param_type_facts.push(ParamTypeFact {
            span: sp(3),
            semantic: ParamSemantic::Set,
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(sp(3)),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_param(&il, param),
            Some(DomainEvidence::Map)
        );
    }

    #[test]
    fn ambiguous_domain_evidence_blocks_legacy_fallback() {
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::None, sp(4), &[]);
        let root = b.add(NodeKind::Func, Payload::None, sp(4), &[param]);
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.param_type_facts.push(ParamTypeFact {
            span: sp(4),
            semantic: ParamSemantic::Set,
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(sp(4)),
            EvidenceKind::Domain(DomainEvidence::Set),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::param(sp(4)),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(domain_evidence_for_param(&il, param), None);
    }

    #[test]
    fn sequence_surface_contracts_keep_value_and_exact_axes_separate() {
        let array = seq_surface_contract(Lang::JavaScript, Some("array")).unwrap();
        assert_eq!(array.value_tag, SEQ_VALUE_COLLECTION);
        assert!(array.exact_tree_safe);
        assert!(array.membership_collection);

        let untagged = seq_surface_contract(Lang::JavaScript, None).unwrap();
        assert_eq!(untagged.value_tag, SEQ_VALUE_UNTAGGED);
        assert!(!untagged.exact_tree_safe);
        assert!(!untagged.membership_collection);

        let object = seq_surface_contract(Lang::JavaScript, Some("object")).unwrap();
        assert_eq!(object.value_tag, SEQ_VALUE_MAP);
        assert!(object.exact_tree_safe);
        assert!(!object.membership_collection);
        assert!(object.imported_literal);

        let go_map = seq_surface_contract(Lang::Go, Some("composite_literal")).unwrap();
        assert_eq!(
            go_map.value_tag,
            stable_symbol_hash("go_composite_map_literal")
        );
        assert!(!go_map.exact_tree_safe);
        assert!(!go_map.membership_collection);
        assert!(!go_map.imported_literal);

        assert!(seq_surface_contract(Lang::Python, Some("composite_literal")).is_none());
        assert!(imported_literal_seq_tag_safe(Lang::Python, "dictionary"));
        assert!(!imported_literal_seq_tag_safe(Lang::Ruby, "hash"));
    }

    #[test]
    fn sequence_surface_evidence_must_match_the_lowered_surface() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let array = interner.intern("array");
        let seq = b.add(NodeKind::Seq, Payload::Name(array), sp(5), &[]);
        let root = b.add(NodeKind::Block, Payload::None, sp(5), &[seq]);
        let mut il = finish_il(b, root, Lang::JavaScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(5)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            EvidenceStatus::Asserted,
        ));
        assert!(seq_surface_contract_for_node(&il, &interner, seq)
            .is_some_and(|contract| contract.membership_collection));

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(sp(5)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(seq_surface_contract_for_node(&il, &interner, seq), None);
    }

    #[test]
    fn import_fact_contracts_parse_typed_binding_and_namespace_proofs() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let binding_tag = interner.intern(import_fact_tag(ImportFactKind::Binding));
        let namespace_tag = interner.intern(import_fact_tag(ImportFactKind::Namespace));
        let collections = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("collections")),
            sp(1),
            &[],
        );
        let deque = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("deque")),
            sp(1),
            &[],
        );
        let binding = b.add(
            NodeKind::Seq,
            Payload::Name(binding_tag),
            sp(1),
            &[collections, deque],
        );
        let math = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("math")),
            sp(2),
            &[],
        );
        let namespace = b.add(NodeKind::Seq, Payload::Name(namespace_tag), sp(2), &[math]);
        let malformed_binding = b.add(NodeKind::Seq, Payload::Name(binding_tag), sp(3), &[math]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(1),
            &[binding, namespace, malformed_binding],
        );
        let il = finish_il(b, root, Lang::Python);

        assert_eq!(
            import_fact_contract(ImportFactKind::Binding).channel,
            ChannelEligibility::ExactProven
        );
        assert!(import_binding_rhs_matches(
            &il,
            &interner,
            binding,
            "collections",
            "deque"
        ));
        assert_eq!(import_fact_evidence_rhs(&il, binding), None);
        assert!(!import_binding_rhs_matches(
            &il,
            &interner,
            namespace,
            "collections",
            "deque"
        ));
        assert!(!import_binding_rhs_matches(
            &il,
            &interner,
            malformed_binding,
            "math",
            "prod"
        ));
        assert!(import_namespace_rhs_matches(
            &il, &interner, namespace, "math"
        ));
        assert!(!import_namespace_rhs_matches(
            &il,
            &interner,
            binding,
            "collections"
        ));
    }

    #[test]
    fn import_evidence_records_are_fail_closed_before_raw_seq_fallback() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let binding_tag = interner.intern(import_fact_tag(ImportFactKind::Binding));
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("collections")),
            sp(10),
            &[],
        );
        let exported = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("deque")),
            sp(10),
            &[],
        );
        let binding = b.add(
            NodeKind::Seq,
            Payload::Name(binding_tag),
            sp(10),
            &[module, exported],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(10), &[binding]);
        let mut il = finish_il(b, root, Lang::Python);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(10)),
            EvidenceKind::Import(ImportEvidenceKind::Namespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));

        assert!(!import_binding_rhs_matches(
            &il,
            &interner,
            binding,
            "collections",
            "deque"
        ));
        assert!(import_namespace_rhs_matches(
            &il, &interner, binding, "math"
        ));

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(sp(10)),
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash("collections"),
                exported_hash: stable_symbol_hash("deque"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(import_fact_rhs(&il, &interner, binding), None);
        assert_eq!(import_fact_evidence_rhs(&il, binding), None);
    }

    #[test]
    fn imported_symbol_identity_does_not_fall_back_to_raw_import_seq() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("deque");
        let binding_tag = interner.intern(import_fact_tag(ImportFactKind::Binding));
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("collections")),
            sp(30),
            &[],
        );
        let exported = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("deque")),
            sp(30),
            &[],
        );
        let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
        let rhs = b.add(
            NodeKind::Seq,
            Payload::Name(binding_tag),
            sp(30),
            &[module, exported],
        );
        let assignment = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
        let use_site = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(30),
            &[assignment, use_site],
        );
        let mut il = finish_il(b, root, Lang::Python);

        assert!(import_binding_rhs_matches(
            &il,
            &interner,
            rhs,
            "collections",
            "deque"
        ));
        assert!(!imported_binding_symbol(
            &il,
            &interner,
            use_site,
            "collections",
            "deque"
        ));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(30), stable_symbol_hash("deque")),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("collections"),
                exported_hash: stable_symbol_hash("deque"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert!(imported_binding_symbol(
            &il,
            &interner,
            use_site,
            "collections",
            "deque"
        ));
    }

    #[test]
    fn direct_symbol_evidence_proves_imported_namespace_without_raw_assignment() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("m")),
            sp(20),
            &[],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(20), &[receiver]);
        let mut il = finish_il(b, root, Lang::Python);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(20), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));

        assert!(imported_namespace_symbol(&il, &interner, receiver, "math"));
        assert!(!imported_namespace_symbol(
            &il,
            &interner,
            receiver,
            "collections"
        ));
    }

    #[test]
    fn symbol_evidence_blocks_import_assignment_fallback() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("math");
        let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(21), &[]);
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("math")),
            sp(21),
            &[],
        );
        let namespace_tag = interner.intern(import_fact_tag(ImportFactKind::Namespace));
        let rhs = b.add(
            NodeKind::Seq,
            Payload::Name(namespace_tag),
            sp(21),
            &[module],
        );
        let assign = b.add(NodeKind::Assign, Payload::None, sp(21), &[lhs, rhs]);
        let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(22), &[]);
        let root = b.add(NodeKind::Module, Payload::None, sp(21), &[assign, receiver]);
        let mut il = finish_il(b, root, Lang::Python);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(21), stable_symbol_hash("math")),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("other"),
            }),
            EvidenceStatus::Asserted,
        ));

        assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
    }

    #[test]
    fn binding_symbol_evidence_does_not_prove_rebound_alias_uses() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("math");
        let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(24), &[]);
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("math")),
            sp(24),
            &[],
        );
        let namespace_tag = interner.intern(import_fact_tag(ImportFactKind::Namespace));
        let rhs = b.add(
            NodeKind::Seq,
            Payload::Name(namespace_tag),
            sp(24),
            &[module],
        );
        let import_assign = b.add(NodeKind::Assign, Payload::None, sp(24), &[lhs, rhs]);
        let rebound_lhs = b.add(NodeKind::Var, Payload::Name(local), sp(25), &[]);
        let rebound_rhs = b.add(NodeKind::Lit, Payload::LitInt(0), sp(25), &[]);
        let rebound = b.add(
            NodeKind::Assign,
            Payload::None,
            sp(25),
            &[rebound_lhs, rebound_rhs],
        );
        let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(26), &[]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(24),
            &[import_assign, rebound, receiver],
        );
        let mut il = finish_il(b, root, Lang::Python);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(24), stable_symbol_hash("math")),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));

        assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
    }

    #[test]
    fn ambiguous_global_symbol_evidence_blocks_name_fallback() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let math = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Math")),
            sp(23),
            &[],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(23), &[math]);
        let mut il = finish_il(b, root, Lang::JavaScript);

        assert!(unshadowed_global_symbol(&il, &interner, math, "Math"));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(23), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Math"),
            }),
            EvidenceStatus::Ambiguous,
        ));
        assert!(!unshadowed_global_symbol(&il, &interner, math, "Math"));
    }

    #[test]
    fn qualified_global_symbol_contracts_are_language_and_path_scoped() {
        assert_eq!(
            qualified_global_symbol_contract(Lang::JavaScript, "Object.hasOwn"),
            Some(QualifiedGlobalSymbolContract {
                path: "Object.hasOwn",
                root: "Object",
                requires_unshadowed_root: true,
            })
        );
        assert_eq!(
            qualified_global_symbol_contract(Lang::TypeScript, "Array.from"),
            Some(QualifiedGlobalSymbolContract {
                path: "Array.from",
                root: "Array",
                requires_unshadowed_root: true,
            })
        );
        assert!(qualified_global_symbol_contract(
            Lang::JavaScript,
            "Object.prototype.hasOwnProperty.call"
        )
        .is_some());
        assert!(qualified_global_symbol_contract(Lang::Python, "Array.from").is_none());
        assert!(
            qualified_global_symbol_contract(Lang::JavaScript, "value.hasOwnProperty").is_none()
        );
        assert!(qualified_global_symbol_contract(Lang::JavaScript, "Array.fromAsync").is_none());
    }

    #[test]
    fn qualified_global_symbol_requires_matching_node_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let array = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Array")),
            sp(27),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("from")),
            sp(27),
            &[array],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(27), &[field]);
        let mut il = finish_il(b, root, Lang::JavaScript);

        assert!(!qualified_global_symbol(&il, field, "Array.from"));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(27), NodeKind::Field),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.from"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert!(qualified_global_symbol(&il, field, "Array.from"));
        assert!(qualified_global_symbol_at_span(
            &il,
            Some(sp(27)),
            NodeKind::Field,
            "Array.from"
        ));
        assert!(!qualified_global_symbol(&il, field, "Array.fromAsync"));

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(27), NodeKind::Field),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.isArray"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert!(!qualified_global_symbol(&il, field, "Array.from"));
    }

    #[test]
    fn language_predicates_preserve_existing_gates() {
        for &lang in ALL_LANGS {
            let profile = semantics(lang);
            assert_eq!(
                profile.operators().primitive_order_comparisons(),
                matches!(lang, Lang::C | Lang::Go | Lang::Java)
            );
            assert_eq!(
                profile.operators().c_integer_byte_pack_contracts(),
                lang == Lang::C
            );
            assert_eq!(
                profile.effects().non_overloadable_index_assignment(),
                matches!(lang, Lang::C | Lang::Go | Lang::Java)
            );
            assert_eq!(
                profile.effects().java_this_field_place(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().js_like_shadowed_module_bindings(),
                matches!(
                    lang,
                    Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
                )
            );
            assert_eq!(
                profile.modules().java_class_literal_exports(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().java_type_declarations_shadow_stdlib(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().go_import_namespace_facts(),
                lang == Lang::Go
            );
        }
    }

    #[test]
    fn stdlib_predicates_preserve_existing_gates() {
        for &lang in ALL_LANGS {
            let stdlib = semantics(lang).stdlib();
            assert_eq!(stdlib.python_collection_factories(), lang == Lang::Python);
            assert_eq!(stdlib.python_deque_factory(), lang == Lang::Python);
            assert_eq!(stdlib.java_collection_factories(), lang == Lang::Java);
            assert_eq!(stdlib.java_map_factories(), lang == Lang::Java);
            assert_eq!(stdlib.java_primitive_integer_ops(), lang == Lang::Java);
            assert_eq!(stdlib.ruby_set_factory(), lang == Lang::Ruby);
            assert_eq!(stdlib.rust_vec_macro_factory(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_vec_new_factory(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_std_collection_factories(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_std_map_factories(), lang == Lang::Rust);
            assert_eq!(stdlib.go_literal_zero_map_lookup(), lang == Lang::Go);
            assert_eq!(stdlib.rust_filter_map_option_contract(), lang == Lang::Rust);
        }
    }

    #[test]
    fn free_name_contracts_are_behavior_equivalent_tables() {
        let py_names: Vec<_> = semantics(Lang::Python)
            .collections()
            .free_name_collection_factories()
            .flat_map(|factory| factory.names.iter().copied())
            .collect();
        assert!(py_names.contains(&"list"));
        assert!(py_names.contains(&"frozenset"));
        assert!(!py_names.contains(&"Set"));

        let imported_py_names: Vec<_> = semantics(Lang::Python)
            .collections()
            .imported_collection_factories()
            .map(|factory| (factory.module, factory.exported))
            .collect();
        assert_eq!(imported_py_names, vec![("collections", "deque")]);

        let rust_map_tags: Vec<_> = semantics(Lang::Rust)
            .collections()
            .free_name_map_factories()
            .map(|factory| factory.entry_seq_tag)
            .collect();
        assert_eq!(rust_map_tags, vec![2]);

        let js_map_tags: Vec<_> = semantics(Lang::JavaScript)
            .collections()
            .free_name_map_factories()
            .map(|factory| factory.entry_seq_tag)
            .collect();
        assert!(js_map_tags.is_empty());
    }

    #[test]
    fn mutating_method_sets_stay_distinct() {
        assert!(mutating_method_name("put"));
        assert!(!mutating_method_name("push"));
        assert!(module_binding_mutating_method_name("push"));
        assert!(module_binding_mutating_method_name("addAll"));
    }

    #[test]
    fn builtin_contracts_preserve_current_special_demand_split() {
        for &builtin in ALL_BUILTINS {
            assert_eq!(builtin_tag(builtin), builtin as u32 + 1);
        }
        assert_eq!(builtin_demand(Builtin::Reduce), BuiltinDemand::Reduce);
        assert_eq!(
            builtin_demand(Builtin::Any),
            BuiltinDemand::AnyAll { all: false }
        );
        assert_eq!(
            builtin_demand(Builtin::All),
            BuiltinDemand::AnyAll { all: true }
        );
        assert_eq!(builtin_demand(Builtin::Append), BuiltinDemand::Append);
        assert_eq!(
            builtin_demand(Builtin::ValueOrDefault),
            BuiltinDemand::ValueOrDefault
        );
        assert_eq!(builtin_demand(Builtin::Len), BuiltinDemand::Eager);
        assert_eq!(
            eager_builtin_contract(Builtin::Len),
            Some(EagerBuiltinContract::Len)
        );
        assert_eq!(eager_builtin_contract(Builtin::Append), None);
        assert_eq!(
            reduction_builtin_contract(Builtin::Max),
            Some(ReductionBuiltinContract::Selection { max: true })
        );
        assert_eq!(
            reduction_builtin_contract(Builtin::Any),
            Some(ReductionBuiltinContract::Bool { all: false })
        );
        assert_eq!(reduction_builtin_contract(Builtin::Print), None);
        assert_eq!(hof_contract(HoFKind::FilterMap), HofContract::FilterMap);
    }

    #[test]
    fn free_function_builtin_contracts_are_language_and_shadow_constrained() {
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "len", 1),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Len,
                args: BuiltinArgContract::First,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Python, "len", 2), None);
        assert_eq!(
            free_function_builtin_contract(Lang::JavaScript, "len", 1),
            None
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "print", 3),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Print,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Go, "append", 2),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Append,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Go, "append", 1), None);
        assert_eq!(free_function_builtin_contract(Lang::C, "fmaxf", 2), None);
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "fmaxf", 2),
            None
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "max", 2),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Max,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Python, "any", 2), None);
    }

    #[test]
    fn method_protocol_contracts_are_language_constrained() {
        assert!(method_fold_name(Lang::Ruby, "inject"));
        assert!(!method_fold_name(Lang::Python, "inject"));
        assert!(!method_fold_name(Lang::Ruby, "map"));
        assert_eq!(
            method_bool_reduction_builtin(Lang::Java, "anyMatch"),
            Some(Builtin::Any)
        );
        assert_eq!(
            method_bool_reduction_builtin(Lang::JavaScript, "every"),
            Some(Builtin::All)
        );
        assert_eq!(method_bool_reduction_builtin(Lang::Python, "every"), None);
        assert_eq!(
            method_hof_contract(Lang::Ruby, "collect"),
            Some(HoFKind::Map)
        );
        assert_eq!(
            method_hof_contract(Lang::Rust, "flat_map"),
            Some(HoFKind::FlatMap)
        );
        assert_eq!(
            method_hof_contract(Lang::Ruby, "select"),
            Some(HoFKind::Filter)
        );
        assert_eq!(method_hof_contract(Lang::Python, "select"), None);
        assert_eq!(
            method_collection_reduction_builtin(Lang::Rust, "count"),
            Some(Builtin::Len)
        );
        assert_eq!(
            method_collection_reduction_builtin(Lang::Java, "count"),
            Some(Builtin::Len)
        );
        assert_eq!(
            method_collection_reduction_builtin(Lang::JavaScript, "count"),
            None
        );
        assert_eq!(
            property_builtin_contract(Lang::JavaScript, "length"),
            Some(Builtin::Len)
        );
        assert_eq!(property_builtin_contract(Lang::Python, "length"), None);
    }

    #[test]
    fn method_call_contracts_carry_receiver_and_resolution_obligations() {
        assert_eq!(
            method_call_contract(Lang::Python, "append", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Append),
                receiver: MethodReceiverContract::ExactCollection,
                args: MethodBuiltinArgs::ReceiverThenAll,
            })
        );
        assert_eq!(method_call_contract(Lang::Python, "append", 0), None);
        assert_eq!(
            method_call_contract(Lang::JavaScript, "log", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Print),
                receiver: MethodReceiverContract::UnshadowedGlobal("console"),
                args: MethodBuiltinArgs::All,
            })
        );
        assert_eq!(
            method_call_contract(Lang::JavaScript, "min", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Min),
                receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                args: MethodBuiltinArgs::All,
            })
        );
        assert_eq!(method_call_contract(Lang::JavaScript, "min", 1), None);
        assert_eq!(method_call_contract(Lang::Python, "min", 2), None);
        assert_eq!(
            method_call_contract(Lang::Go, "Abs", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Abs),
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                args: MethodBuiltinArgs::First,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Go, "Contains", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
                args: MethodBuiltinArgs::GoSliceContains,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Java, "abs", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Abs),
                receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                args: MethodBuiltinArgs::First,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Java, "min", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Min),
                receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                args: MethodBuiltinArgs::All,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Python, "__contains__", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactCollectionOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(
            method_call_contract(Lang::TypeScript, "has", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactSetOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Ruby, "member?", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(method_call_contract(Lang::JavaScript, "contains", 1), None);
        assert_eq!(
            method_call_contract(Lang::Java, "getOrDefault", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
                receiver: MethodReceiverContract::ExactMap,
                args: MethodBuiltinArgs::MapGetDefault,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Python, "get", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
                receiver: MethodReceiverContract::ExactMap,
                args: MethodBuiltinArgs::MapGetDefault,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Ruby, "fetch", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
                receiver: MethodReceiverContract::ExactMap,
                args: MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda,
            })
        );
        assert_eq!(method_call_contract(Lang::JavaScript, "abs", 0), None);
    }

    #[test]
    fn scalar_integer_methods_are_language_and_signature_constrained() {
        assert_eq!(
            scalar_integer_method_contract(Lang::Rust, "clamp", 2),
            Some(ScalarIntegerMethodContract {
                semantic: ScalarIntegerMethod::Clamp,
                receiver: MethodReceiverContract::ExactInteger,
            })
        );
        assert_eq!(
            scalar_integer_method_contract(Lang::Rust, "min", 1),
            Some(ScalarIntegerMethodContract {
                semantic: ScalarIntegerMethod::Min,
                receiver: MethodReceiverContract::ExactInteger,
            })
        );
        assert_eq!(scalar_integer_method_contract(Lang::Rust, "clamp", 1), None);
        assert_eq!(
            scalar_integer_method_contract(Lang::TypeScript, "clamp", 2),
            None
        );
        assert_eq!(
            scalar_integer_method_contract(Lang::JavaScript, "abs", 0),
            None
        );
    }

    #[test]
    fn async_to_sync_contracts_are_python_constrained() {
        assert_eq!(
            async_to_sync_name(Lang::Python, "__aenter__"),
            Some("__enter__")
        );
        assert_eq!(async_to_sync_name(Lang::Python, "aread"), Some("read"));
        assert_eq!(
            async_to_sync_name(Lang::Python, "AsyncIterator"),
            Some("Iterator")
        );
        assert_eq!(async_to_sync_name(Lang::JavaScript, "aread"), None);
        assert_eq!(async_to_sync_name(Lang::Python, "append"), None);
    }

    #[test]
    fn promise_then_contract_requires_js_like_surface_and_receiver_proof() {
        assert_eq!(
            promise_then_contract(Lang::TypeScript, "then", 1),
            Some(PromiseThenContract {
                receiver: AsyncReceiverContract::ExactPromiseLike,
            })
        );
        assert_eq!(promise_then_contract(Lang::TypeScript, "then", 2), None);
        assert_eq!(promise_then_contract(Lang::Python, "then", 1), None);
    }

    #[test]
    fn iterator_identity_adapters_are_rust_and_receiver_proof_constrained() {
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "iter", 0),
            Some(IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            })
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
            Some(IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            })
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Java, "stream", 0),
            Some(IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            })
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
            None
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
            None
        );
    }

    #[test]
    fn static_collection_adapters_are_import_binding_constrained() {
        assert_eq!(
            static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
            Some(StaticCollectionAdapterContract {
                module: "java.util",
                exported: "Arrays",
            })
        );
        assert_eq!(
            static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
            None
        );
        assert_eq!(
            static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
            None
        );
    }

    #[test]
    fn rust_std_path_contracts_carry_shadow_roots() {
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Rust, "Option::Some"),
            Some(ShadowedPathContract {
                shadow_root: "Option",
            })
        );
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Rust, "std::option::Option::Some"),
            Some(ShadowedPathContract { shadow_root: "std" })
        );
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Python, "Some"),
            None
        );
        assert_eq!(
            rust_option_none_sentinel_contract(Lang::Rust, "None"),
            Some(ShadowedPathContract {
                shadow_root: "None",
            })
        );
        assert_eq!(
            rust_option_none_sentinel_contract(Lang::Rust, "core::option::Option::None"),
            Some(ShadowedPathContract {
                shadow_root: "core",
            })
        );
        assert_eq!(
            rust_option_none_sentinel_contract(Lang::JavaScript, "None"),
            None
        );
        assert_eq!(
            rust_vec_new_factory_contract(Lang::Rust, "alloc::vec::Vec::new"),
            Some(ShadowedPathContract {
                shadow_root: "alloc",
            })
        );
        assert_eq!(
            rust_vec_new_factory_contract(Lang::Rust, "Vec::with_capacity"),
            None
        );
        assert!(rust_option_and_then_contract(Lang::Rust, "and_then", 1));
        assert!(!rust_option_and_then_contract(Lang::Rust, "and_then", 0));
        assert!(!rust_option_and_then_contract(
            Lang::JavaScript,
            "and_then",
            1
        ));
    }

    #[test]
    fn java_factory_contracts_are_language_receiver_and_selector_constrained() {
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "List", "of"),
            Some(JavaCollectionFactoryContract {
                receiver: "List",
                method: "of",
                kind: JavaCollectionFactoryKind::ListOf,
                single_arg_spreads_array: false,
            })
        );
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
            Some(JavaCollectionFactoryContract {
                receiver: "Arrays",
                method: "asList",
                kind: JavaCollectionFactoryKind::ArraysAsList,
                single_arg_spreads_array: true,
            })
        );
        assert_eq!(
            java_collection_factory_contract(Lang::JavaScript, "List", "of"),
            None
        );
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "Map", "of"),
            None
        );
        assert_eq!(
            java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
            Some(JavaCollectionConstructorContract {
                simple_type: "ArrayList",
                qualified_type: "java.util.ArrayList",
                module: "java.util",
                kind: JavaCollectionConstructorKind::EmptyList,
                requires_import_for_simple_type: true,
                requires_no_local_type_shadow: true,
            })
        );
        assert_eq!(
            java_collection_constructor_contract(Lang::Java, "java.util.LinkedList", 0)
                .map(|contract| contract.kind),
            Some(JavaCollectionConstructorKind::EmptyList)
        );
        assert_eq!(
            java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
            None
        );
        assert_eq!(
            java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
            None
        );
        assert_eq!(
            java_map_factory_contract(Lang::Java, "Map", "ofEntries"),
            Some(JavaMapFactoryContract {
                receiver: "Map",
                method: "ofEntries",
                kind: JavaMapFactoryKind::OfEntries,
            })
        );
        assert_eq!(java_map_factory_contract(Lang::Java, "List", "of"), None);
        assert!(java_map_entry_contract(Lang::Java, "Map", "entry"));
        assert!(!java_map_entry_contract(Lang::Java, "Entry", "entry"));
        assert_eq!(
            java_collection_factory_contract_by_hash(Lang::Java, "Set", stable_symbol_hash("of"))
                .map(|contract| contract.kind),
            Some(JavaCollectionFactoryKind::SetOf)
        );
        assert_eq!(
            java_map_factory_contract_by_hash(Lang::Java, "Map", stable_symbol_hash("of"))
                .map(|contract| contract.kind),
            Some(JavaMapFactoryKind::Of)
        );
        assert!(java_map_entry_contract_by_hash(
            Lang::Java,
            "Map",
            stable_symbol_hash("entry")
        ));
    }

    #[test]
    fn ruby_and_closed_js_like_factory_contracts_keep_proof_obligations_explicit() {
        assert_eq!(
            ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
            Some(RubySetFactoryContract {
                receiver: "Set",
                method: "new",
                required_module: "set",
                shadow_root: "Set",
            })
        );
        assert_eq!(ruby_set_factory_contract(Lang::Ruby, "Set", "new", 2), None);
        assert_eq!(
            ruby_set_factory_contract(Lang::Python, "Set", "new", 1),
            None
        );
        assert!(
            ruby_set_factory_contract_by_hash(Lang::Ruby, "Set", stable_symbol_hash("new"), 1)
                .is_some()
        );

        assert_eq!(
            js_like_set_constructor_contract(Lang::TypeScript, "Set"),
            Some(ClosedConstructorContract {
                receiver: "Set",
                required_proof: ConstructorProofRequirement::ConstructSyntax,
                requires_unshadowed_global: true,
                entry_seq_tag: None,
            })
        );
        assert_eq!(
            js_like_map_constructor_contract(Lang::JavaScript, "Map"),
            Some(ClosedConstructorContract {
                receiver: "Map",
                required_proof: ConstructorProofRequirement::ConstructSyntax,
                requires_unshadowed_global: true,
                entry_seq_tag: Some(SEQ_VALUE_COLLECTION),
            })
        );
        assert_eq!(js_like_map_constructor_contract(Lang::Java, "Map"), None);
        assert_eq!(
            js_like_set_constructor_contract(Lang::JavaScript, "WeakSet"),
            None
        );
    }

    #[test]
    fn map_key_view_contracts_distinguish_collection_and_iterator_views() {
        assert_eq!(
            map_key_view_contract(Lang::Python, "keys", 0),
            Some(MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Collection,
            })
        );
        assert_eq!(
            map_key_view_contract(Lang::Java, "keySet", 0),
            Some(MapKeyViewContract {
                method: "keySet",
                kind: MapKeyViewKind::Collection,
            })
        );
        assert_eq!(
            map_key_view_contract(Lang::TypeScript, "keys", 0),
            Some(MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            })
        );
        assert_eq!(map_key_view_contract(Lang::JavaScript, "keySet", 0), None);
        assert_eq!(map_key_view_contract(Lang::Python, "keys", 1), None);
        assert_eq!(
            map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
            Some(MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
                qualified_path: "Array.from",
            })
        );
        assert_eq!(
            map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
            None
        );
        assert_eq!(
            map_key_view_contract_by_hash(Lang::Java, stable_symbol_hash("keySet"), 0)
                .map(|contract| contract.kind),
            Some(MapKeyViewKind::Collection)
        );
        assert!(map_key_view_wrapper_contract_by_hash(
            Lang::TypeScript,
            "Array",
            stable_symbol_hash("from"),
            1,
        )
        .is_some());
    }

    #[test]
    fn go_zero_map_contracts_are_go_surface_and_default_constrained() {
        assert_eq!(
            go_zero_map_lookup_contract(Lang::Go),
            Some(GoZeroMapLookupContract {
                map_literal_tag: "composite_literal",
                entry_tag: "keyed_element",
                canonical_value_tag: "go_literal_zero_map",
            })
        );
        assert_eq!(go_zero_map_lookup_contract(Lang::Python), None);
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::LitInt(1)),
            Some(GoZeroMapDefaultKind::Int)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::LitStr(stable_symbol_hash("x"))),
            Some(GoZeroMapDefaultKind::String)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::Lit(LitClass::Null)),
            Some(GoZeroMapDefaultKind::Null)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::JavaScript, Payload::LitInt(1)),
            None
        );
        assert_eq!(go_zero_map_default_kind(Lang::Go, Payload::None), None);
    }

    #[test]
    fn map_get_contracts_are_language_and_arity_constrained() {
        assert_eq!(
            map_get_contract(Lang::Rust, "get", 1),
            Some(MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            })
        );
        assert_eq!(
            map_get_contract_by_hash(Lang::Java, stable_symbol_hash("get"), 1),
            Some(MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            })
        );
        assert_eq!(
            map_get_contract(Lang::TypeScript, "get", 1),
            Some(MapGetContract {
                method: "get",
                receiver: MethodReceiverContract::ExactMap,
            })
        );
        assert_eq!(map_get_contract(Lang::Python, "get", 1), None);
        assert_eq!(map_get_contract(Lang::Rust, "get", 2), None);
        assert_eq!(map_get_contract(Lang::Java, "getOrDefault", 1), None);
    }

    #[test]
    fn js_static_builtin_contracts_are_language_and_arity_constrained() {
        assert_eq!(
            static_global_symbol_contract(Lang::JavaScript, "Math"),
            Some(StaticGlobalSymbolContract {
                name: "Math",
                requires_unshadowed: true,
            })
        );
        assert_eq!(
            static_global_symbol_contract(Lang::TypeScript, "undefined"),
            Some(StaticGlobalSymbolContract {
                name: "undefined",
                requires_unshadowed: true,
            })
        );
        assert_eq!(static_global_symbol_contract(Lang::Python, "Math"), None);
        assert_eq!(
            static_global_symbol_contract(Lang::JavaScript, "WeakMap"),
            None
        );
        assert_eq!(
            typeof_operator_contract(Lang::TypeScript, "typeof", 1),
            Some(TypeofOperatorContract { name: "typeof" })
        );
        assert_eq!(typeof_operator_contract(Lang::Python, "typeof", 1), None);
        assert_eq!(
            typeof_operator_contract(Lang::JavaScript, "typeof", 2),
            None
        );
        assert_eq!(
            js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
            Some(StaticGlobalMethodContract {
                receiver: "Array",
                method: "isArray",
                qualified_path: "Array.isArray",
                requires_unshadowed_receiver: true,
            })
        );
        assert_eq!(
            js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
            None
        );
        assert_eq!(
            js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
            None
        );
        assert_eq!(
            js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1),
            Some(StaticGlobalFunctionContract {
                function: "Boolean",
                requires_unshadowed_function: true,
            })
        );
        assert_eq!(
            js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
            Some(StaticGlobalFunctionContract {
                function: "Boolean",
                requires_unshadowed_function: true,
            })
        );
        assert_eq!(
            js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
            None
        );
        assert_eq!(
            js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
            None
        );
        assert_eq!(
            regex_test_contract(Lang::JavaScript, "test", 1),
            Some(RegexTestContract {
                method: "test",
                required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
            })
        );
        assert_eq!(regex_test_contract(Lang::Ruby, "test", 1), None);
    }

    #[test]
    fn operator_law_contracts_preserve_comparison_gates() {
        for &lang in ALL_LANGS {
            let profile = semantics(lang);
            assert_eq!(
                profile
                    .operators()
                    .comparison_law(ComparisonLaw::LatticeStrictAbsorbsNonstrict)
                    .is_some(),
                matches!(lang, Lang::C | Lang::Go | Lang::Java)
            );
            assert_eq!(
                profile
                    .operators()
                    .comparison_law(ComparisonLaw::LatticeLeNeToLt),
                Some(OperatorLawContract {
                    law: ComparisonLaw::LatticeLeNeToLt,
                    channel: ChannelEligibility::ExactProven,
                    evidence: OperatorEvidence::ModeledIlOperator,
                })
            );
        }
    }

    #[test]
    fn comparison_transform_contracts_carry_outputs_and_operand_swaps() {
        let ops = semantics(Lang::Python).operators();
        assert_eq!(
            ops.comparison_direction(Op::Gt),
            Some(ComparisonTransformContract {
                law: ComparisonLaw::DirectionCanon,
                input: Op::Gt,
                output: Op::Lt,
                swap_operands: true,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::ModeledIlOperator,
            })
        );
        assert_eq!(
            ops.comparison_complement(Op::Lt)
                .map(|contract| (contract.output, contract.swap_operands)),
            Some((Op::Ge, false))
        );
        assert_eq!(
            ops.canonical_negated_comparison(Op::Lt)
                .map(|contract| (contract.output, contract.swap_operands)),
            Some((Op::Le, true))
        );
        assert_eq!(ops.comparison_direction(Op::Eq), None);
    }

    #[test]
    fn cardinality_threshold_contracts_name_existing_operator_shapes() {
        let ops = semantics(Lang::JavaScript).operators();
        assert_eq!(
            ops.zero_cardinality_equality(Op::Eq),
            Some(CardinalityThresholdContract {
                threshold: CardinalityThreshold::Zero,
                predicate: CardinalityPredicate::Empty,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::StaticCardinalityThreshold,
            })
        );
        assert_eq!(ops.zero_cardinality_equality(Op::Gt), None);
        assert_eq!(
            ops.cardinality_threshold(
                Op::Gt,
                false,
                CardinalityThreshold::Zero,
                CardinalityPredicate::NonEmpty,
            )
            .map(|contract| contract.predicate),
            Some(CardinalityPredicate::NonEmpty)
        );
        assert_eq!(
            ops.cardinality_threshold(
                Op::Eq,
                false,
                CardinalityThreshold::One,
                CardinalityPredicate::NonEmpty,
            ),
            None
        );
    }

    #[test]
    fn membership_operator_contract_is_language_scoped() {
        assert_eq!(
            semantics(Lang::Python)
                .operators()
                .membership_operator(Op::In),
            Some(MembershipOperatorContract {
                operator: Op::In,
                receiver: MembershipOperatorReceiverContract::ExactCollectionOrMap,
                channel: ChannelEligibility::ExactProven,
                evidence: OperatorEvidence::ModeledIlOperator,
            })
        );
        assert_eq!(
            semantics(Lang::JavaScript)
                .operators()
                .membership_operator(Op::In),
            None
        );
        assert_eq!(
            semantics(Lang::Python)
                .operators()
                .membership_operator(Op::Eq),
            None
        );
    }

    #[test]
    fn static_index_membership_contracts_are_js_like_and_threshold_constrained() {
        assert_eq!(
            static_index_membership_contract(Lang::JavaScript, "indexOf", 1),
            Some(StaticIndexMembershipContract {
                method: "indexOf",
                kind: StaticIndexMembershipKind::IndexOf,
                receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
            })
        );
        assert_eq!(
            static_index_membership_contract(Lang::TypeScript, "findIndex", 1),
            Some(StaticIndexMembershipContract {
                method: "findIndex",
                kind: StaticIndexMembershipKind::FindIndex,
                receiver: StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection,
            })
        );
        assert_eq!(
            static_index_membership_contract(Lang::Python, "indexOf", 1),
            None
        );
        assert_eq!(
            static_index_membership_contract(Lang::JavaScript, "indexOf", 2),
            None
        );
        assert_eq!(
            static_index_membership_contract(Lang::JavaScript, "includes", 1),
            None
        );
        assert_eq!(
            semantics(Lang::JavaScript)
                .operators()
                .static_index_membership_threshold(
                    Op::Ne,
                    false,
                    IndexMembershipThreshold::MinusOne
                )
                .map(|contract| contract.evidence),
            Some(OperatorEvidence::JsLikeStaticIndexMembershipThreshold)
        );
        assert!(semantics(Lang::TypeScript)
            .operators()
            .static_index_membership_threshold(Op::Le, true, IndexMembershipThreshold::Zero)
            .is_some());
        assert!(semantics(Lang::Python)
            .operators()
            .static_index_membership_threshold(Op::Ne, false, IndexMembershipThreshold::MinusOne)
            .is_none());
        assert!(semantics(Lang::JavaScript)
            .operators()
            .static_index_membership_threshold(Op::Eq, false, IndexMembershipThreshold::MinusOne)
            .is_none());
    }

    #[test]
    fn imported_namespace_function_contracts_carry_module_and_receiver_proof() {
        assert_eq!(
            imported_namespace_function_contract(Lang::Python, "prod", 1),
            Some(ImportedNamespaceFunctionContract {
                module: "math",
                function: "prod",
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                    op: Op::Mul,
                    identity: 1,
                },
            })
        );
        assert_eq!(
            imported_namespace_function_contract(Lang::Python, "prod", 2)
                .map(|contract| contract.semantic),
            Some(ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            })
        );
        assert_eq!(
            imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
            None
        );
        assert_eq!(
            imported_namespace_function_contract(Lang::Python, "prod", 3),
            None
        );
        assert_eq!(
            imported_namespace_function_contract(Lang::Python, "sum", 1),
            None
        );
    }

    #[test]
    fn nullish_global_contracts_are_js_like_and_unshadowed() {
        assert_eq!(
            nullish_global_contract(Lang::JavaScript, "undefined"),
            Some(NullishGlobalContract {
                name: "undefined",
                requires_unshadowed: true,
            })
        );
        assert_eq!(
            nullish_global_contract(Lang::TypeScript, "undefined"),
            Some(NullishGlobalContract {
                name: "undefined",
                requires_unshadowed: true,
            })
        );
        assert_eq!(nullish_global_contract(Lang::Python, "undefined"), None);
        assert_eq!(nullish_global_contract(Lang::JavaScript, "null"), None);
    }

    #[test]
    fn builder_append_contracts_are_language_and_arity_constrained() {
        assert!(builder_append_method_contract(Lang::Rust, "push", 1));
        assert!(!builder_append_method_contract(Lang::Rust, "push", 2));
        assert!(builder_append_method_contract(Lang::Java, "add", 1));
        assert!(builder_append_method_contract(Lang::JavaScript, "push", 1));
        assert!(builder_append_method_contract(Lang::Python, "append", 1));
        assert!(!builder_append_method_contract(Lang::Ruby, "push", 1));
    }

    #[test]
    fn exact_java_this_helpers_are_language_and_shape_constrained() {
        let interner = Interner::default();
        let this = interner.intern("this");
        let field_name = interner.intern("value");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(this), sp(1), &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp(1),
            &[receiver],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp(2), &[receiver]);
        let root = b.add(NodeKind::Block, Payload::None, sp(1), &[field, ret]);
        let il = finish_il(b, root, Lang::Java);

        assert!(exact_java_this_var(&il, &interner, receiver));
        assert!(exact_java_this_field(&il, &interner, field));
        assert!(exact_java_return_this(&il, &interner, ret));

        let mut js_il = il.clone();
        js_il.meta.lang = Lang::JavaScript;
        assert!(!exact_java_this_var(&js_il, &interner, receiver));
        assert!(!exact_java_this_field(&js_il, &interner, field));
        assert!(!exact_java_return_this(&js_il, &interner, ret));
    }

    #[test]
    fn exact_index_assignment_parts_are_language_constrained() {
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
        let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(1), &[target, value]);
        let il = finish_il(b, assign, Lang::Go);

        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            Some((receiver, Some(key), value))
        );
        assert!(exact_non_overloadable_index_assignment(&il, assign));

        let mut ruby_il = il.clone();
        ruby_il.meta.lang = Lang::Ruby;
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&ruby_il, assign),
            None
        );
        assert!(!exact_non_overloadable_index_assignment(&ruby_il, assign));
    }

    #[test]
    fn builder_append_call_args_require_canonical_append_proof() {
        let interner = Interner::default();
        let append = interner.intern("append");
        let push = interner.intern("push");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let builtin = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp(1),
            &[receiver, value],
        );
        let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
        let call = b.add(NodeKind::Call, Payload::None, sp(2), &[method, value]);
        let push_method = b.add(NodeKind::Field, Payload::Name(push), sp(3), &[receiver]);
        let push_call = b.add(NodeKind::Call, Payload::None, sp(3), &[push_method, value]);
        let root = b.add(
            NodeKind::Block,
            Payload::None,
            sp(1),
            &[builtin, call, push_call],
        );
        let il = finish_il(b, root, Lang::Python);

        assert_eq!(
            builder_append_call_args(&il, &interner, builtin),
            Some((receiver, value))
        );
        assert_eq!(builder_append_call_args(&il, &interner, call), None);
        assert_eq!(builder_append_call_args(&il, &interner, push_call), None);

        let mut rust_il = il.clone();
        rust_il.meta.lang = Lang::Rust;
        assert_eq!(
            builder_append_call_args(&rust_il, &interner, push_call),
            None
        );
    }

    #[test]
    fn source_fact_contracts_are_span_keyed_evidence() {
        let mut b = IlBuilder::new(FileId(0));
        let call = b.add(NodeKind::Call, Payload::None, sp(7), &[]);
        let regex = b.add(NodeKind::Lit, Payload::LitStr(42), sp(8), &[]);
        let root = b.add(NodeKind::Block, Payload::None, sp(7), &[call, regex]);
        let mut il = finish_il(b, root, Lang::JavaScript);
        il.source_facts.push(SourceFact {
            span: sp(7),
            kind: SourceFactKind::Call(SourceCallKind::Construct),
        });
        il.source_facts.push(SourceFact {
            span: sp(8),
            kind: SourceFactKind::Literal(SourceLiteralKind::Regex),
        });

        assert!(construct_syntax_proof(&il, call));
        assert!(regex_literal_proof(&il, regex));
        assert!(!construct_syntax_proof(&il, regex));
        assert_eq!(
            source_fact_contract(SourceFactKind::Call(SourceCallKind::Construct)).channel,
            ChannelEligibility::ExactProven
        );
    }

    #[test]
    fn source_fact_evidence_conflicts_fail_closed() {
        let mut b = IlBuilder::new(FileId(0));
        let op = b.add(NodeKind::BinOp, Payload::Op(Op::Eq), sp(9), &[]);
        let root = b.add(NodeKind::Block, Payload::None, sp(9), &[op]);
        let mut il = finish_il(b, root, Lang::JavaScript);
        il.source_facts.push(SourceFact {
            span: sp(9),
            kind: SourceFactKind::Operator(SourceOperatorKind::StrictEquality),
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::source_span(sp(9)),
            EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::StrictEquality)),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(
            source_operator_at_node(&il, op),
            Some(SourceOperatorKind::StrictEquality)
        );

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::source_span(sp(9)),
            EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::LooseEquality)),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(source_operator_at_node(&il, op), None);
    }

    #[test]
    fn static_membership_predicate_operator_requires_js_strict_equality() {
        assert!(exact_static_membership_predicate_operator(
            Lang::JavaScript,
            Op::Eq,
            SourceOperatorKind::StrictEquality
        ));
        assert!(exact_static_membership_predicate_operator(
            Lang::TypeScript,
            Op::Ne,
            SourceOperatorKind::StrictInequality
        ));
        assert!(!exact_static_membership_predicate_operator(
            Lang::JavaScript,
            Op::Eq,
            SourceOperatorKind::LooseEquality
        ));
        assert!(!exact_static_membership_predicate_operator(
            Lang::Python,
            Op::Eq,
            SourceOperatorKind::ValueEquality
        ));
        assert!(!exact_static_membership_predicate_operator(
            Lang::JavaScript,
            Op::Eq,
            SourceOperatorKind::TypeMembership
        ));
    }
}
