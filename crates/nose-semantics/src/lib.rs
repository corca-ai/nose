//! Semantic contracts for language and library facts used by exact matching.
//!
//! This crate is the first-party semantic-kernel facade. The initial migration is
//! deliberately behavior-preserving: it names the semantic assumptions that were
//! previously encoded as scattered `Lang` matches. Future pack loading should
//! extend this contract surface rather than letting packs mint fingerprints or
//! approve exact clone matches directly.

use nose_il::{
    contains_js_identifier, stable_symbol_hash, Builtin, EffectEvidenceKind, EvidenceAnchor,
    EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceRecord, EvidenceStatus, GuardEvidenceKind,
    HoFKind, Il, ImportEvidenceKind, Interner, Lang, LibraryApiEvidenceKind, LitClass, NodeId,
    NodeKind, Op, ParamSemantic, Payload, PlaceEvidenceKind, SequenceSurfaceKind, SourceCallKind,
    SourceComprehensionKind, SourceFactKind, SourceLiteralKind, SourceOperatorKind,
    SourceProtocolKind, Span, Symbol, SymbolEvidenceKind,
};
use rustc_hash::FxHashMap;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

fn unique_asserted_evidence_at<T: Copy + Eq>(
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
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
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
    unique_asserted_evidence_at(il, |anchor| anchor.matches_span(span), project)
}

pub fn source_fact_at_node(il: &Il, node: NodeId, kind: SourceFactKind) -> bool {
    match kind {
        SourceFactKind::Operator(operator) => source_operator_at_node(il, node) == Some(operator),
        SourceFactKind::Call(call) => source_call_at_node(il, node) == Some(call),
        SourceFactKind::Protocol(protocol) => source_protocol_at_node(il, node) == Some(protocol),
        SourceFactKind::Literal(literal) => source_literal_at_node(il, node) == Some(literal),
        SourceFactKind::Comprehension(comprehension) => {
            source_comprehension_at_node(il, node) == Some(comprehension)
        }
    }
}

pub fn source_operator_at_node(il: &Il, node: NodeId) -> Option<SourceOperatorKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Operator(operator)) => Some(operator),
        _ => None,
    }) {
        EvidenceResolution::Found(operator) => Some(operator),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_call_at_node(il: &Il, node: NodeId) -> Option<SourceCallKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
        _ => None,
    }) {
        EvidenceResolution::Found(call) => Some(call),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_protocol_at_node(il: &Il, node: NodeId) -> Option<SourceProtocolKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Protocol(protocol)) => Some(protocol),
        _ => None,
    }) {
        EvidenceResolution::Found(protocol) => Some(protocol),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_literal_at_node(il: &Il, node: NodeId) -> Option<SourceLiteralKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Literal(literal)) => Some(literal),
        _ => None,
    }) {
        EvidenceResolution::Found(literal) => Some(literal),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_comprehension_at_node(il: &Il, node: NodeId) -> Option<SourceComprehensionKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Comprehension(comprehension)) => Some(comprehension),
        _ => None,
    }) {
        EvidenceResolution::Found(comprehension) => Some(comprehension),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn admitted_hof_api_at_node(il: &Il, node: NodeId, kind: HoFKind) -> bool {
    if il.kind(node) != NodeKind::HoF || il.node(node).payload != Payload::HoF(kind) {
        return false;
    }
    library_api_dependency_id_for_normalized_hof(il, node).is_some()
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

/// Coarse value domain used by proof-gated value-graph and recursion laws.
///
/// This is deliberately not a general type system. It records only the semantic
/// domains that current first-party laws need in order to avoid known false
/// merges, such as numeric arithmetic versus string/list concatenation and
/// boolean logic versus short-circuit value selection. Unknown is fail-closed
/// for laws that require a positive proof.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueDomain {
    Number,
    Boolean,
    String,
    Sequence,
    Unknown,
}

impl ValueDomain {
    pub fn join(self, other: ValueDomain) -> ValueDomain {
        if self == other {
            self
        } else {
            ValueDomain::Unknown
        }
    }

    pub fn is_concat_like(self) -> bool {
        matches!(self, ValueDomain::String | ValueDomain::Sequence)
    }

    fn is_known(self) -> bool {
        self != ValueDomain::Unknown
    }

    pub fn from_domain_evidence(domain: DomainEvidence) -> Option<ValueDomain> {
        if domain.is_integer_or_number() {
            Some(ValueDomain::Number)
        } else if domain.is_string() {
            Some(ValueDomain::String)
        } else if domain.is_array_collection_or_set() {
            Some(ValueDomain::Sequence)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueLaw {
    AddCommutativity,
    AddAssociativity,
    NumericNegationInvolution,
    NumericBitwiseIdempotence,
    BooleanIdempotence,
    BooleanCommutativity,
    BooleanAssociativity,
    NumericFactorDistribution,
    StructuralNumericFold,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueDomainRequirement {
    NumericOperands,
    BooleanOperands,
    NoConcatOperands,
}

impl ValueDomainRequirement {
    pub fn accepts(self, domains: impl IntoIterator<Item = ValueDomain>) -> bool {
        match self {
            ValueDomainRequirement::NumericOperands => domains
                .into_iter()
                .all(|domain| domain == ValueDomain::Number),
            ValueDomainRequirement::BooleanOperands => domains
                .into_iter()
                .all(|domain| domain == ValueDomain::Boolean),
            ValueDomainRequirement::NoConcatOperands => {
                domains.into_iter().all(|domain| !domain.is_concat_like())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueDomainEvidence {
    Literal,
    SequenceSurface,
    DomainRecord,
    StrictOperatorUse,
    ModeledOperatorResult,
    ModeledBuiltinResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValueLawContract {
    pub law: ValueLaw,
    pub requirement: ValueDomainRequirement,
    pub channel: ChannelEligibility,
    pub evidence: ValueDomainEvidence,
}

fn strict_numeric_operand_operator(op: Op) -> bool {
    matches!(
        op,
        Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
    )
}

pub fn domain_evidence_at_span(il: &Il, span: Span) -> Option<DomainEvidence> {
    match unique_asserted_evidence_at(
        il,
        |anchor| anchor.matches_span(span),
        |evidence| match evidence {
            EvidenceKind::Domain(domain) => Some(domain),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(domain) => Some(domain),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn domain_evidence_for_param(il: &Il, param: NodeId) -> Option<DomainEvidence> {
    (il.kind(param) == NodeKind::Param)
        .then_some(il.node(param).span)
        .and_then(|span| domain_evidence_at_span(il, span))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DomainRequirement {
    Array,
    ByteArray,
    Collection,
    CollectionOrSet,
    CollectionOrMap,
    ArrayOrCollection,
    ArrayCollectionOrSet,
    Set,
    SetOrMap,
    Map,
    Option,
    String,
    Integer,
    IntegerOrNumber,
}

impl DomainRequirement {
    pub fn accepts(self, domain: DomainEvidence) -> bool {
        match self {
            DomainRequirement::Array => domain.is_array(),
            DomainRequirement::ByteArray => domain.is_byte_array(),
            DomainRequirement::Collection => domain == DomainEvidence::Collection,
            DomainRequirement::CollectionOrSet => domain.is_collection_or_set(),
            DomainRequirement::CollectionOrMap => {
                domain.is_array_collection_or_set() || domain.is_map()
            }
            DomainRequirement::ArrayOrCollection => domain.is_array_or_collection(),
            DomainRequirement::ArrayCollectionOrSet => domain.is_array_collection_or_set(),
            DomainRequirement::Set => domain.is_set(),
            DomainRequirement::SetOrMap => domain.is_set() || domain.is_map(),
            DomainRequirement::Map => domain.is_map(),
            DomainRequirement::Option => domain.is_option(),
            DomainRequirement::String => domain.is_string(),
            DomainRequirement::Integer => domain.is_integer(),
            DomainRequirement::IntegerOrNumber => domain.is_integer_or_number(),
        }
    }
}

fn domain_evidence_at_exact_anchor(
    il: &Il,
    expected: EvidenceAnchor,
) -> EvidenceResolution<DomainEvidence> {
    unique_asserted_evidence_at(
        il,
        |anchor| anchor == expected,
        |evidence| match evidence {
            EvidenceKind::Domain(domain) => Some(domain),
            _ => None,
        },
    )
}

pub fn domain_evidence_for_node(il: &Il, node: NodeId) -> Option<DomainEvidence> {
    match domain_evidence_at_exact_anchor(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
    ) {
        EvidenceResolution::Found(domain) => Some(domain),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn domain_evidence_for_binding_lhs(
    il: &Il,
    interner: &Interner,
    lhs: NodeId,
) -> Option<DomainEvidence> {
    match domain_evidence_at_binding_lhs(il, interner, lhs) {
        EvidenceResolution::Found(domain) => Some(domain),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn domain_evidence_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
) -> Option<DomainEvidence> {
    match domain_evidence_at_exact_anchor(
        il,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
    ) {
        EvidenceResolution::Found(domain) => return Some(domain),
        EvidenceResolution::Ambiguous => return None,
        EvidenceResolution::Missing => {}
    }
    match domain_evidence_for_binding_reference(il, interner, receiver) {
        EvidenceResolution::Found(domain) => return Some(domain),
        EvidenceResolution::Ambiguous => return None,
        EvidenceResolution::Missing => {}
    }
    domain_evidence_for_var_reference(il, receiver)
}

pub fn domain_evidence_for_var(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<DomainEvidence> {
    (il.kind(node) == NodeKind::Var)
        .then(|| domain_evidence_for_receiver(il, interner, node))
        .flatten()
}

pub fn receiver_satisfies_domain(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    requirement: DomainRequirement,
) -> bool {
    domain_evidence_for_receiver(il, interner, receiver)
        .is_some_and(|domain| requirement.accepts(domain))
}

fn domain_evidence_for_var_reference(il: &Il, node: NodeId) -> Option<DomainEvidence> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Cid(cid) => match nearest_scope(il, node) {
            // A reassigned binding no longer proves its parameter's declared domain: the
            // current value may have a different domain (e.g. a `list[int]` parameter
            // rebound to a `str`). This mirrors the `Payload::Name` reassignment guard
            // below; without it the alpha-renamed (Cid) form — the form that actually runs
            // on the normalized IL value-graph/idiom consumers see — would fail open and
            // admit, for instance, substring membership as collection membership.
            Some(scope) if cid_is_assigned_in_scope(il, cid, scope) => None,
            Some(scope) => unique_domain_evidence_for_params(
                il,
                il.children(scope).iter().copied().filter(move |&child| {
                    il.kind(child) == NodeKind::Param
                        && matches!(il.node(child).payload, Payload::Cid(param_cid) if param_cid == cid)
                }),
            ),
            None => unique_domain_evidence_for_params(
                il,
                il.nodes.iter().enumerate().filter_map(move |(idx, candidate)| {
                    (candidate.kind == NodeKind::Param
                        && matches!(candidate.payload, Payload::Cid(param_cid) if param_cid == cid))
                    .then_some(NodeId(idx as u32))
                }),
            ),
        },
        Payload::Name(name) => {
            let (scope, param) = nearest_named_param_scope(il, node, name)?;
            if name_is_assigned_in_scope(il, name, scope) {
                return None;
            }
            domain_evidence_for_param(il, param)
        }
        _ => None,
    }
}

fn domain_evidence_for_binding_reference(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> EvidenceResolution<DomainEvidence> {
    if il.kind(node) != NodeKind::Var {
        return EvidenceResolution::Missing;
    }
    let lhs = match unique_binding_lhs_for_var_reference(il, node) {
        EvidenceResolution::Found(lhs) => lhs,
        EvidenceResolution::Ambiguous => return EvidenceResolution::Ambiguous,
        EvidenceResolution::Missing => return EvidenceResolution::Missing,
    };
    domain_evidence_at_binding_lhs(il, interner, lhs)
}

fn domain_evidence_at_binding_lhs(
    il: &Il,
    interner: &Interner,
    lhs: NodeId,
) -> EvidenceResolution<DomainEvidence> {
    let span = il.node(lhs).span;
    let Some(local_hash) = node_name_hash(il, interner, lhs) else {
        return EvidenceResolution::Missing;
    };
    unique_asserted_evidence_at(
        il,
        |anchor| {
            matches!(
                anchor,
                EvidenceAnchor::Binding {
                    span: anchor_span,
                    local_hash: anchor_hash,
                } if anchor_span == span && anchor_hash == local_hash
            )
        },
        |evidence| match evidence {
            EvidenceKind::Domain(domain) => Some(domain),
            _ => None,
        },
    )
}

fn unique_binding_lhs_for_var_reference(il: &Il, node: NodeId) -> EvidenceResolution<NodeId> {
    let scope = nearest_scope(il, node);
    let reference_is_free_name = matches!(il.node(node).payload, Payload::Name(_));
    let mut found = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if candidate.kind != NodeKind::Assign {
            continue;
        }
        let assign = NodeId(idx as u32);
        let assignment_scope = nearest_scope(il, assign);
        if assignment_scope != scope && !(reference_is_free_name && assignment_scope.is_none()) {
            continue;
        }
        if !assignment_is_visible_at_reference(il, assign, node) {
            continue;
        }
        let Some(&lhs) = il.children(assign).first() else {
            continue;
        };
        if !var_references_same_binding(il, lhs, node) {
            continue;
        }
        match found {
            None => found = Some(lhs),
            Some(existing) if existing == lhs => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn assignment_is_visible_at_reference(il: &Il, assign: NodeId, reference: NodeId) -> bool {
    il.node(assign).span.end_byte <= il.node(reference).span.start_byte
}

fn var_references_same_binding(il: &Il, lhs: NodeId, reference: NodeId) -> bool {
    if il.kind(lhs) != NodeKind::Var || il.kind(reference) != NodeKind::Var {
        return false;
    }
    match (il.node(lhs).payload, il.node(reference).payload) {
        (Payload::Cid(lhs_cid), Payload::Cid(reference_cid)) => lhs_cid == reference_cid,
        (Payload::Name(lhs_name), Payload::Name(reference_name)) => lhs_name == reference_name,
        (Payload::Cid(lhs_cid), Payload::Name(reference_name))
        | (Payload::Name(reference_name), Payload::Cid(lhs_cid)) => il
            .cid_names
            .get(lhs_cid as usize)
            .is_some_and(|&lhs_name| lhs_name == reference_name),
        _ => false,
    }
}

fn unique_domain_evidence_for_params(
    il: &Il,
    params: impl Iterator<Item = NodeId>,
) -> Option<DomainEvidence> {
    let mut found = None;
    for param in params {
        let Some(domain) = domain_evidence_for_param(il, param) else {
            continue;
        };
        match found {
            None => found = Some(domain),
            Some(existing) if existing == domain => {}
            Some(_) => return None,
        }
    }
    found
}

fn nearest_named_param_scope(il: &Il, node: NodeId, name: Symbol) -> Option<(NodeId, NodeId)> {
    let target = il.node(node).span;
    let mut best: Option<(u32, NodeId, NodeId)> = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if !matches!(candidate.kind, NodeKind::Func | NodeKind::Lambda) {
            continue;
        }
        if !span_contains(candidate.span, target) {
            continue;
        }
        let scope = NodeId(idx as u32);
        let Some(param) = il.children(scope).iter().copied().find(|&child| {
            il.kind(child) == NodeKind::Param && il.node(child).payload == Payload::Name(name)
        }) else {
            continue;
        };
        let width = candidate
            .span
            .end_byte
            .saturating_sub(candidate.span.start_byte);
        if best.is_none_or(|(best_width, _, _)| width < best_width) {
            best = Some((width, scope, param));
        }
    }
    best.map(|(_, scope, param)| (scope, param))
}

fn name_is_assigned_in_scope(il: &Il, name: Symbol, scope: NodeId) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope(il, id) != Some(scope) {
            return false;
        }
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var && il.node(lhs).payload == Payload::Name(name)
    })
}

/// Cid-keyed counterpart of [`name_is_assigned_in_scope`]: is the alpha-renamed binding
/// `cid` the target of a reassignment inside `scope`? Used to keep a reassigned
/// parameter from proving its declared domain on the normalized (Cid) IL.
fn cid_is_assigned_in_scope(il: &Il, cid: u32, scope: NodeId) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope(il, id) != Some(scope) {
            return false;
        }
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var
            && matches!(il.node(lhs).payload, Payload::Cid(lhs_cid) if lhs_cid == cid)
    })
}

fn nearest_scope(il: &Il, node: NodeId) -> Option<NodeId> {
    let target = il.node(node).span;
    let mut best: Option<(u32, NodeId)> = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if !matches!(candidate.kind, NodeKind::Func | NodeKind::Lambda) {
            continue;
        }
        if !span_contains(candidate.span, target) {
            continue;
        }
        let width = candidate
            .span
            .end_byte
            .saturating_sub(candidate.span.start_byte);
        if best.is_none_or(|(best_width, _)| width < best_width) {
            best = Some((width, NodeId(idx as u32)));
        }
    }
    best.map(|(_, scope)| scope)
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
        let Some(dependency) = il.evidence.get(id.0 as usize) else {
            return false;
        };
        if dependency.id != *id || dependency.status != EvidenceStatus::Asserted {
            return false;
        }
        if !dependency.anchor.matches_span(span) {
            return false;
        }
        match dependency.kind {
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                if path_hash == stable_symbol_hash("Array.isArray") =>
            {
                has_array_is_array = true;
            }
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                if null_check == nose_il::JsRecordGuardNullCheck::BooleanGlobalTruthy
                    && name_hash == stable_symbol_hash("Boolean") =>
            {
                has_boolean = true;
            }
            _ => return false,
        }
    }
    has_array_is_array && has_boolean
}

fn js_own_property_guard_api_path_hash_supported(path_hash: u64) -> bool {
    path_hash == stable_symbol_hash("Object.hasOwn")
        || path_hash == stable_symbol_hash("Object.prototype.hasOwnProperty.call")
}

fn js_own_property_guard_dependencies_valid(
    il: &Il,
    record: &EvidenceRecord,
    api_path_hash: u64,
    span: Span,
) -> bool {
    if !js_own_property_guard_api_path_hash_supported(api_path_hash) {
        return false;
    }
    let mut has_api = false;
    for id in &record.dependencies {
        let Some(dependency) = il.evidence.get(id.0 as usize) else {
            return false;
        };
        if dependency.id != *id
            || dependency.status != EvidenceStatus::Asserted
            || !dependency.anchor.matches_span(span)
        {
            return false;
        }
        match dependency.kind {
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                if path_hash == api_path_hash =>
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
                Payload::Builtin(builtin) => self.builtin_result_domain(builtin),
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodEffectContractId {
    ExactBuilderAppendCall,
    ReceiverMutationRisk,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodEffectArity {
    Any,
    Exact(usize),
}

impl MethodEffectArity {
    pub fn matches(self, arg_count: usize) -> bool {
        match self {
            MethodEffectArity::Any => true,
            MethodEffectArity::Exact(expected) => arg_count == expected,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodEffectReceiverContract {
    ActiveCollectionBuilder,
    PotentiallyMutableReceiver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodEffectContract {
    pub pack_id: &'static str,
    pub id: MethodEffectContractId,
    pub lang: Lang,
    pub method: &'static str,
    pub arity: MethodEffectArity,
    pub receiver: MethodEffectReceiverContract,
    pub effect: EffectEvidenceKind,
    pub channel: ChannelEligibility,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexWriteContractId {
    MapBuilderEntryWrite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexWriteReceiverContract {
    ActiveMapBuilder,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexWriteContract {
    pub pack_id: &'static str,
    pub id: IndexWriteContractId,
    pub lang: Lang,
    pub receiver: IndexWriteReceiverContract,
    pub required_effect: EffectEvidenceKind,
    pub channel: ChannelEligibility,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MethodEffectContractSet {
    id: MethodEffectContractId,
    lang: Lang,
    methods: &'static [&'static str],
    arity: MethodEffectArity,
    receiver: MethodEffectReceiverContract,
    effect: EffectEvidenceKind,
    channel: ChannelEligibility,
}

const BUILDER_APPEND_METHOD_EFFECTS: &[MethodEffectContractSet] = &[
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Python,
        methods: &["append"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::JavaScript,
        methods: &["push"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Java,
        methods: &["add"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ExactBuilderAppendCall,
        lang: Lang::Rust,
        methods: &["push"],
        arity: MethodEffectArity::Exact(1),
        receiver: MethodEffectReceiverContract::ActiveCollectionBuilder,
        effect: EffectEvidenceKind::BuilderAppendCall,
        channel: ChannelEligibility::ExactProven,
    },
];

const RECEIVER_MUTATION_METHOD_EFFECTS: &[MethodEffectContractSet] = &[
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::JavaScript,
        methods: &[
            "add",
            "clear",
            "copyWithin",
            "delete",
            "fill",
            "pop",
            "push",
            "reverse",
            "set",
            "shift",
            "sort",
            "splice",
            "unshift",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Python,
        methods: &[
            "add",
            "append",
            "clear",
            "extend",
            "insert",
            "pop",
            "remove",
            "reverse",
            "setdefault",
            "sort",
            "update",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Ruby,
        methods: &[
            "add", "append", "clear", "delete", "merge!", "pop", "push", "reverse!", "shift",
            "sort!", "store", "unshift", "update",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Java,
        methods: &[
            "add",
            "addAll",
            "clear",
            "compute",
            "computeIfAbsent",
            "computeIfPresent",
            "merge",
            "put",
            "putAll",
            "remove",
            "removeAll",
            "removeIf",
            "replace",
            "replaceAll",
            "retainAll",
            "set",
            "sort",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
    MethodEffectContractSet {
        id: MethodEffectContractId::ReceiverMutationRisk,
        lang: Lang::Rust,
        methods: &[
            "clear",
            "extend",
            "insert",
            "pop",
            "push",
            "remove",
            "retain",
            "reverse",
            "sort",
            "sort_by",
            "sort_unstable",
        ],
        arity: MethodEffectArity::Any,
        receiver: MethodEffectReceiverContract::PotentiallyMutableReceiver,
        effect: EffectEvidenceKind::ReceiverMutation,
        channel: ChannelEligibility::ExactProven,
    },
];

const MAP_BUILDER_INDEX_WRITE_CONTRACTS: &[IndexWriteContract] = &[IndexWriteContract {
    pack_id: FIRST_PARTY_PACK_ID,
    id: IndexWriteContractId::MapBuilderEntryWrite,
    lang: Lang::Python,
    receiver: IndexWriteReceiverContract::ActiveMapBuilder,
    required_effect: EffectEvidenceKind::BindingWrite,
    channel: ChannelEligibility::ExactProven,
}];

fn method_effect_contract_lang(requested: Lang, contract_lang: Lang) -> Option<Lang> {
    if requested == contract_lang || (js_like_lang(requested) && contract_lang == Lang::JavaScript)
    {
        Some(requested)
    } else {
        None
    }
}

impl EffectSemantics {
    pub fn method_effect_contracts(self) -> impl Iterator<Item = MethodEffectContract> {
        BUILDER_APPEND_METHOD_EFFECTS
            .iter()
            .chain(RECEIVER_MUTATION_METHOD_EFFECTS.iter())
            .copied()
            .filter_map(move |set| {
                let lang = method_effect_contract_lang(self.lang, set.lang)?;
                Some((lang, set))
            })
            .flat_map(|(lang, set)| {
                set.methods
                    .iter()
                    .copied()
                    .map(move |method| MethodEffectContract {
                        pack_id: FIRST_PARTY_PACK_ID,
                        id: set.id,
                        lang,
                        method,
                        arity: set.arity,
                        receiver: set.receiver,
                        effect: set.effect,
                        channel: set.channel,
                    })
            })
    }

    pub fn method_effect_contract(
        self,
        id: MethodEffectContractId,
        method: &str,
        arg_count: usize,
    ) -> Option<MethodEffectContract> {
        self.method_effect_contracts().find(|contract| {
            contract.id == id && contract.method == method && contract.arity.matches(arg_count)
        })
    }

    pub fn builder_append_method_contract(
        self,
        method: &str,
        arg_count: usize,
    ) -> Option<MethodEffectContract> {
        self.method_effect_contract(
            MethodEffectContractId::ExactBuilderAppendCall,
            method,
            arg_count,
        )
    }

    pub fn receiver_mutation_method_contract(
        self,
        method: &str,
        arg_count: usize,
    ) -> Option<MethodEffectContract> {
        self.method_effect_contract(
            MethodEffectContractId::ReceiverMutationRisk,
            method,
            arg_count,
        )
    }

    pub fn map_builder_index_write_contract(self) -> Option<IndexWriteContract> {
        MAP_BUILDER_INDEX_WRITE_CONTRACTS
            .iter()
            .copied()
            .find(|contract| contract.lang == self.lang)
    }

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

fn exact_effect_evidence_for_node(il: &Il, node: NodeId) -> EvidenceResolution<EffectEvidenceKind> {
    let span = il.node(node).span;
    let kind = il.kind(node);
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
            EvidenceKind::Effect(
                effect @ (EffectEvidenceKind::BuilderAppendCall
                | EffectEvidenceKind::NonOverloadableIndexWrite
                | EffectEvidenceKind::SelfFieldWrite { .. }),
            ) => Some(effect),
            _ => None,
        },
    )
}

fn asserted_effect_at_node(il: &Il, node: NodeId, wanted: EffectEvidenceKind) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    il.evidence.iter().any(|record| {
        record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
            && record.kind == EvidenceKind::Effect(wanted)
            && matches!(
                record.anchor,
                EvidenceAnchor::Node {
                    span: anchor_span,
                    kind: anchor_kind,
                } if anchor_span == span && anchor_kind == kind
            )
    })
}

fn asserted_library_api_at_node(il: &Il, node: NodeId) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    il.evidence.iter().any(|record| {
        record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
            && matches!(record.kind, EvidenceKind::LibraryApi(_))
            && matches!(
                record.anchor,
                EvidenceAnchor::Node {
                    span: anchor_span,
                    kind: anchor_kind,
                } if anchor_span == span && anchor_kind == kind
            )
    })
}

fn place_evidence_for_node(il: &Il, node: NodeId) -> EvidenceResolution<PlaceEvidenceKind> {
    let span = il.node(node).span;
    let kind = il.kind(node);
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
            EvidenceKind::Place(place) => Some(place),
            _ => None,
        },
    )
}

/// Exact self receiver proof for first-party self-field fragments.
pub fn exact_java_this_var(il: &Il, _interner: &Interner, node: NodeId) -> bool {
    match place_evidence_for_node(il, node) {
        EvidenceResolution::Found(PlaceEvidenceKind::SelfReceiver) => {
            il.kind(node) == NodeKind::Var
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => false,
        EvidenceResolution::Missing => false,
    }
}

/// Exact self-field place proof for receiver-aware field-write fingerprints.
pub fn exact_java_this_field(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match place_evidence_for_node(il, node) {
        EvidenceResolution::Found(PlaceEvidenceKind::SelfField { field_hash }) => {
            if il.kind(node) != NodeKind::Field {
                return false;
            }
            let Payload::Name(field) = il.node(node).payload else {
                return false;
            };
            if stable_symbol_hash(interner.resolve(field)) != field_hash {
                return false;
            }
            il.children(node)
                .first()
                .is_some_and(|&receiver| exact_java_this_var(il, interner, receiver))
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => false,
        EvidenceResolution::Missing => false,
    }
}

/// Exact self-return proof used by self-field body fragments.
pub fn exact_java_return_this(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Return {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 1 && exact_java_this_var(il, interner, kids[0])
}

/// `(receiver, key, value)` of a first-party exact-safe index assignment.
///
/// This is intentionally evidence-gated: languages with overloadable/user-dispatched index
/// assignment remain fail-closed unless a frontend or pack supplies effect proof.
pub fn exact_non_overloadable_index_assignment_parts(
    il: &Il,
    node: NodeId,
) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    match exact_effect_evidence_for_node(il, node) {
        EvidenceResolution::Found(EffectEvidenceKind::NonOverloadableIndexWrite) => {
            syntactic_index_assignment_parts(il, node)
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => None,
    }
}

fn syntactic_index_assignment_parts(
    il: &Il,
    node: NodeId,
) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    if il.kind(node) != NodeKind::Assign {
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

pub fn exact_self_field_write_assignment(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match exact_effect_evidence_for_node(il, node) {
        EvidenceResolution::Found(EffectEvidenceKind::SelfFieldWrite { field_hash }) => {
            syntactic_self_field_write_assignment(il, interner, node, Some(field_hash))
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => false,
        EvidenceResolution::Missing => false,
    }
}

fn syntactic_self_field_write_assignment(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected_field_hash: Option<u64>,
) -> bool {
    if il.kind(node) != NodeKind::Assign {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    if let Some(expected) = expected_field_hash {
        let Payload::Name(field) = il.node(kids[0]).payload else {
            return false;
        };
        if stable_symbol_hash(interner.resolve(field)) != expected {
            return false;
        }
    }
    exact_java_this_field(il, interner, kids[0])
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

pub fn builder_append_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MethodEffectContract> {
    semantics(lang)
        .effects()
        .builder_append_method_contract(method, arg_count)
}

pub fn map_builder_index_write_contract(lang: Lang) -> Option<IndexWriteContract> {
    semantics(lang).effects().map_builder_index_write_contract()
}

/// `(receiver, value)` of a single-item append-like builder call admitted by first-party
/// language/library contracts.
///
/// Raw method selectors such as `push`, `append`, or `add` are not proof by themselves;
/// callers that see those selectors must first prove the receiver/builder contract, lower
/// the call to the canonical builtin, and attach append-effect evidence.
pub fn builder_append_call_args(
    il: &Il,
    _interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    match exact_effect_evidence_for_node(il, node) {
        EvidenceResolution::Found(EffectEvidenceKind::BuilderAppendCall) => {
            syntactic_append_call_args(il, node)
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => None,
    }
}

fn canonical_append_call_args(il: &Il, node: NodeId) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return (kids.len() == 2).then(|| (kids[0], kids[1]));
    }
    None
}

fn syntactic_append_call_args(il: &Il, node: NodeId) -> Option<(NodeId, NodeId)> {
    if let Some(parts) = canonical_append_call_args(il, node) {
        return Some(parts);
    }
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let receiver = il.children(kids[0]).first().copied()?;
    Some((receiver, kids[1]))
}

pub fn builder_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    builder_append_call_args(il, interner, node).is_some()
}

pub fn binding_write_target(il: &Il, node: NodeId) -> Option<NodeId> {
    if !asserted_effect_at_node(il, node, EffectEvidenceKind::BindingWrite) {
        return None;
    }
    if il.kind(node) != NodeKind::Assign {
        return None;
    }
    il.children(node).first().copied()
}

pub fn receiver_mutation_call_receiver(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<NodeId> {
    if let Some((receiver, _)) = builder_append_call_args(il, interner, node) {
        return Some(receiver);
    }
    if !asserted_effect_at_node(il, node, EffectEvidenceKind::ReceiverMutation) {
        return None;
    }
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let callee = *il.children(node).first()?;
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    il.children(callee).first().copied()
}

pub fn opaque_argument_escape_args(il: &Il, node: NodeId) -> Option<&[NodeId]> {
    if !asserted_effect_at_node(il, node, EffectEvidenceKind::OpaqueArgumentEscape) {
        return None;
    }
    if asserted_library_api_at_node(il, node) {
        return None;
    }
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    Some(il.children(node).get(1..).unwrap_or(&[]))
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiContractId {
    PropertyBuiltin(Builtin),
    PythonBuiltinCollectionFactory,
    PythonImportedCollectionFactory,
    FreeFunctionBuiltin(Builtin),
    RustOptionSomeConstructor,
    RustOptionNoneSentinel,
    RustOptionAndThen,
    ScalarIntegerMethod(ScalarIntegerMethod),
    RustStdCollectionFactory,
    RustStdMapFactory,
    RustVecMacroFactory,
    RustVecNewFactory,
    JavaCollectionFactory(JavaCollectionFactoryKind),
    JavaCollectionConstructor(JavaCollectionConstructorKind),
    JavaMapFactory(JavaMapFactoryKind),
    JavaMapEntryFactory,
    RubySetFactory,
    JsLikeSetConstructor,
    JsLikeMapConstructor,
    MapKeyView(MapKeyViewKind),
    MapKeyViewWrapper,
    MapGet,
    JsArrayIsArray,
    JsBooleanCoercion,
    RegexTest,
    JsLikeStaticIndexMembership(StaticIndexMembershipKind),
    ImportedNamespaceFunction(ImportedNamespaceFunctionSemantic),
    PromiseThen,
    IteratorIdentityAdapter,
    StaticCollectionAdapter,
    MethodCall(MethodSemanticContract),
}

pub fn library_api_contract_id_hash(id: LibraryApiContractId) -> u64 {
    stable_symbol_hash(&library_api_contract_id_key(id))
}

fn library_api_contract_id_key(id: LibraryApiContractId) -> String {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin) => {
            format!("property_builtin.{}", builtin as u32)
        }
        LibraryApiContractId::PythonBuiltinCollectionFactory => {
            "python.builtin.collection_factory".into()
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            "python.imported.collection_factory".into()
        }
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            format!("free_function_builtin.{}", builtin as u32)
        }
        LibraryApiContractId::RustOptionSomeConstructor => "rust.option.some.constructor".into(),
        LibraryApiContractId::RustOptionNoneSentinel => "rust.option.none.sentinel".into(),
        LibraryApiContractId::RustOptionAndThen => "rust.option.and_then".into(),
        LibraryApiContractId::ScalarIntegerMethod(method) => {
            format!(
                "scalar_integer_method.{}",
                scalar_integer_method_key(method)
            )
        }
        LibraryApiContractId::RustStdCollectionFactory => "rust.std.collection_factory".into(),
        LibraryApiContractId::RustStdMapFactory => "rust.std.map_factory".into(),
        LibraryApiContractId::RustVecMacroFactory => "rust.vec.macro_factory".into(),
        LibraryApiContractId::RustVecNewFactory => "rust.vec.new_factory".into(),
        LibraryApiContractId::JavaCollectionFactory(kind) => {
            format!(
                "java.collection_factory.{}",
                java_collection_factory_kind_key(kind)
            )
        }
        LibraryApiContractId::JavaCollectionConstructor(kind) => {
            format!(
                "java.collection_constructor.{}",
                java_collection_constructor_kind_key(kind)
            )
        }
        LibraryApiContractId::JavaMapFactory(kind) => {
            format!("java.map_factory.{}", java_map_factory_kind_key(kind))
        }
        LibraryApiContractId::JavaMapEntryFactory => "java.map_entry_factory".into(),
        LibraryApiContractId::RubySetFactory => "ruby.set_factory".into(),
        LibraryApiContractId::JsLikeSetConstructor => "js_like.set.constructor".into(),
        LibraryApiContractId::JsLikeMapConstructor => "js_like.map.constructor".into(),
        LibraryApiContractId::MapKeyView(kind) => {
            format!("map_key_view.{}", map_key_view_kind_key(kind))
        }
        LibraryApiContractId::MapKeyViewWrapper => "map_key_view.wrapper".into(),
        LibraryApiContractId::MapGet => "map.get".into(),
        LibraryApiContractId::JsArrayIsArray => "js_like.array.is_array".into(),
        LibraryApiContractId::JsBooleanCoercion => "js_like.boolean.coercion".into(),
        LibraryApiContractId::RegexTest => "js_like.regex.test".into(),
        LibraryApiContractId::JsLikeStaticIndexMembership(kind) => {
            format!(
                "js_like.static_index_membership.{}",
                static_index_membership_kind_key(kind)
            )
        }
        LibraryApiContractId::ImportedNamespaceFunction(semantic) => {
            format!(
                "imported_namespace_function.{}",
                imported_namespace_function_semantic_key(semantic)
            )
        }
        LibraryApiContractId::PromiseThen => "js_like.promise.then".into(),
        LibraryApiContractId::IteratorIdentityAdapter => "iterator.identity_adapter".into(),
        LibraryApiContractId::StaticCollectionAdapter => "static.collection_adapter".into(),
        LibraryApiContractId::MethodCall(semantic) => {
            format!("method_call.{}", method_semantic_contract_key(semantic))
        }
    }
}

fn scalar_integer_method_key(method: ScalarIntegerMethod) -> &'static str {
    match method {
        ScalarIntegerMethod::Abs => "abs",
        ScalarIntegerMethod::Min => "min",
        ScalarIntegerMethod::Max => "max",
        ScalarIntegerMethod::Clamp => "clamp",
    }
}

fn java_collection_factory_kind_key(kind: JavaCollectionFactoryKind) -> &'static str {
    match kind {
        JavaCollectionFactoryKind::ListOf => "list_of",
        JavaCollectionFactoryKind::SetOf => "set_of",
        JavaCollectionFactoryKind::ArraysAsList => "arrays_as_list",
    }
}

fn java_collection_constructor_kind_key(kind: JavaCollectionConstructorKind) -> &'static str {
    match kind {
        JavaCollectionConstructorKind::EmptyList => "empty_list",
    }
}

fn java_map_factory_kind_key(kind: JavaMapFactoryKind) -> &'static str {
    match kind {
        JavaMapFactoryKind::Of => "of",
        JavaMapFactoryKind::OfEntries => "of_entries",
    }
}

fn map_key_view_kind_key(kind: MapKeyViewKind) -> &'static str {
    match kind {
        MapKeyViewKind::Collection => "collection",
        MapKeyViewKind::Iterator => "iterator",
    }
}

fn static_index_membership_kind_key(kind: StaticIndexMembershipKind) -> &'static str {
    match kind {
        StaticIndexMembershipKind::IndexOf => "index_of",
        StaticIndexMembershipKind::FindIndex => "find_index",
    }
}

fn imported_namespace_function_semantic_key(semantic: ImportedNamespaceFunctionSemantic) -> String {
    match semantic {
        ImportedNamespaceFunctionSemantic::ProductReduction { op, identity } => {
            format!("product_reduction.{}.{}", op as u32, identity)
        }
    }
}

fn method_semantic_contract_key(semantic: MethodSemanticContract) -> String {
    match semantic {
        MethodSemanticContract::Builtin(builtin) => format!("builtin.{}", builtin as u32),
        MethodSemanticContract::HoF(hof) => format!("hof.{}", hof as u32),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiShadowPolicy {
    None,
    SameName,
    RustStdRootForStdPath,
    ExplicitRoot(&'static str),
}

pub fn library_api_free_name_shadow_safe(
    lang: Lang,
    name: &str,
    policy: LibraryApiShadowPolicy,
    defines_name: impl Fn(&str) -> bool,
) -> bool {
    match policy {
        LibraryApiShadowPolicy::None => true,
        LibraryApiShadowPolicy::SameName => !defines_name(name),
        LibraryApiShadowPolicy::RustStdRootForStdPath => {
            !(lang == Lang::Rust && name.starts_with("std::") && defines_name("std"))
        }
        LibraryApiShadowPolicy::ExplicitRoot(root) => !defines_name(root),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiCalleeContract {
    FreeName {
        name: &'static str,
        shadow: LibraryApiShadowPolicy,
    },
    RustMacro {
        name: &'static str,
        shadow: LibraryApiShadowPolicy,
    },
    ImportedBinding {
        module: &'static str,
        exported: &'static str,
    },
    JavaUtilStaticMember {
        receiver: &'static str,
        method: &'static str,
    },
    JavaUtilConstructor {
        simple_type: &'static str,
        qualified_type: &'static str,
        module: &'static str,
        requires_import_for_simple_type: bool,
        requires_no_local_type_shadow: bool,
    },
    RubyRequireStaticMember {
        receiver: &'static str,
        method: &'static str,
        required_module: &'static str,
        shadow_root: &'static str,
    },
    JsGlobalConstructor {
        receiver: &'static str,
        requires_unshadowed_global: bool,
    },
    Method {
        method: &'static str,
        receiver: MethodReceiverContract,
    },
    StaticGlobalMethod {
        receiver: &'static str,
        method: &'static str,
        qualified_path: &'static str,
        requires_unshadowed_receiver: bool,
    },
    StaticGlobalFunction {
        function: &'static str,
        requires_unshadowed_function: bool,
    },
    RegexLiteralMethod {
        method: &'static str,
        required_receiver_fact: SourceFactKind,
    },
    Property {
        property: &'static str,
        receiver: MethodReceiverContract,
    },
    StaticIndexMembershipMethod {
        method: &'static str,
        receiver: StaticIndexMembershipReceiverContract,
    },
    ImportedNamespaceFunction {
        module: &'static str,
        function: &'static str,
    },
    AsyncMethod {
        method: &'static str,
        receiver: AsyncReceiverContract,
    },
    IteratorAdapterMethod {
        method: &'static str,
        receiver: IteratorAdapterReceiverContract,
    },
}

pub fn library_api_callee_contract_hash(callee: LibraryApiCalleeContract) -> u64 {
    stable_symbol_hash(&library_api_callee_contract_key(callee))
}

fn library_api_callee_contract_key(callee: LibraryApiCalleeContract) -> String {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => format!("free_name:{name}"),
        LibraryApiCalleeContract::RustMacro { name, .. } => format!("rust_macro:{name}"),
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            format!("imported_binding:{module}:{exported}")
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method } => {
            format!("java_util_static_member:{receiver}:{method}")
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            ..
        } => format!("java_util_constructor:{module}:{simple_type}:{qualified_type}"),
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            method,
            required_module,
            ..
        } => format!("ruby_require_static_member:{required_module}:{receiver}:{method}"),
        LibraryApiCalleeContract::JsGlobalConstructor { receiver, .. } => {
            format!("js_global_constructor:{receiver}")
        }
        LibraryApiCalleeContract::Method { method, receiver } => {
            format!("method:{method}:{}", method_receiver_contract_key(receiver))
        }
        LibraryApiCalleeContract::StaticGlobalMethod { qualified_path, .. } => {
            format!("static_global_method:{qualified_path}")
        }
        LibraryApiCalleeContract::StaticGlobalFunction { function, .. } => {
            format!("static_global_function:{function}")
        }
        LibraryApiCalleeContract::RegexLiteralMethod { method, .. } => {
            format!("regex_literal_method:{method}")
        }
        LibraryApiCalleeContract::Property { property, receiver } => {
            format!(
                "property:{property}:{}",
                method_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            format!(
                "static_index_membership_method:{method}:{}",
                static_index_membership_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, function } => {
            format!("imported_namespace_function:{module}:{function}")
        }
        LibraryApiCalleeContract::AsyncMethod { method, receiver } => {
            format!(
                "async_method:{method}:{}",
                async_receiver_contract_key(receiver)
            )
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            format!(
                "iterator_adapter_method:{method}:{}",
                iterator_adapter_receiver_contract_key(receiver)
            )
        }
    }
}

fn method_receiver_contract_key(receiver: MethodReceiverContract) -> String {
    match receiver {
        MethodReceiverContract::ExactCollection => "exact_collection".into(),
        MethodReceiverContract::ExactProtocol => "exact_protocol".into(),
        MethodReceiverContract::ExactProtocolPairArgument => "exact_protocol_pair_argument".into(),
        MethodReceiverContract::ExactOption => "exact_option".into(),
        MethodReceiverContract::ExactString => "exact_string".into(),
        MethodReceiverContract::ExactInteger => "exact_integer".into(),
        MethodReceiverContract::ExactMap => "exact_map".into(),
        MethodReceiverContract::ExactMapLiteral => "exact_map_literal".into(),
        MethodReceiverContract::ExactCollectionOrMap => "exact_collection_or_map".into(),
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            "exact_collection_or_map_literal".into()
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            "exact_collection_or_java_key_set".into()
        }
        MethodReceiverContract::ExactSetOrMap => "exact_set_or_map".into(),
        MethodReceiverContract::LiteralString => "literal_string".into(),
        MethodReceiverContract::UnshadowedGlobal(name) => format!("unshadowed_global:{name}"),
        MethodReceiverContract::ImportedNamespace(module) => {
            format!("imported_namespace:{module}")
        }
        MethodReceiverContract::RustMapGetOrExactOption => "rust_map_get_or_exact_option".into(),
    }
}

fn async_receiver_contract_key(receiver: AsyncReceiverContract) -> &'static str {
    match receiver {
        AsyncReceiverContract::ExactPromiseLike => "exact_promise_like",
    }
}

fn iterator_adapter_receiver_contract_key(
    receiver: IteratorAdapterReceiverContract,
) -> &'static str {
    match receiver {
        IteratorAdapterReceiverContract::ExactIterableValue => "exact_iterable_value",
    }
}

fn static_index_membership_receiver_contract_key(
    receiver: StaticIndexMembershipReceiverContract,
) -> &'static str {
    match receiver {
        StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection => {
            "static_non_float_literal_collection"
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryCollectionFactoryResult {
    SequenceArgument,
    VariadicElements { single_arg_spreads_array: bool },
    StaticNonFloatSequenceArgument,
    EmptySequence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryCollectionFactoryContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: LibraryCollectionFactoryResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryMapFactoryResult {
    EntrySequence { entry_seq_tag: u64 },
    JavaFactory { kind: JavaMapFactoryKind },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapFactoryContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: LibraryMapFactoryResult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapEntryFactoryContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapKeyViewContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapKeyViewWrapperContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapKeyViewWrapperContract,
}

pub fn library_collection_factory_result_domain(
    contract: LibraryCollectionFactoryContract,
) -> DomainEvidence {
    match contract.id {
        LibraryApiContractId::PythonBuiltinCollectionFactory => match contract.callee {
            LibraryApiCalleeContract::FreeName {
                name: "set" | "frozenset",
                ..
            } => DomainEvidence::Set,
            _ => DomainEvidence::Collection,
        },
        LibraryApiContractId::RustStdCollectionFactory => match contract.callee {
            LibraryApiCalleeContract::FreeName {
                name: "std::collections::HashSet::from" | "std::collections::BTreeSet::from",
                ..
            } => DomainEvidence::Set,
            _ => DomainEvidence::Collection,
        },
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::SetOf)
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => DomainEvidence::Set,
        _ => DomainEvidence::Collection,
    }
}

pub fn library_collection_factory_result_domain_for_arity(
    contract: LibraryCollectionFactoryContract,
    arg_count: usize,
) -> Option<DomainEvidence> {
    match contract.id {
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ArraysAsList)
            if arg_count == 1 =>
        {
            None
        }
        _ => Some(library_collection_factory_result_domain(contract)),
    }
}

pub fn library_map_factory_result_domain(_contract: LibraryMapFactoryContract) -> DomainEvidence {
    DomainEvidence::Map
}

pub fn library_map_key_view_wrapper_result_domain(
    _contract: LibraryMapKeyViewWrapperContract,
) -> DomainEvidence {
    DomainEvidence::Array
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMapGetContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MapGetContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalMethodContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticGlobalFunctionContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticGlobalFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRegexTestContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: RegexTestContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticIndexMembershipContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticIndexMembershipContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryImportedNamespaceFunctionContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ImportedNamespaceFunctionContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPromiseThenContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: PromiseThenContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryIteratorIdentityAdapterContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: IteratorIdentityAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryStaticCollectionAdapterContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: StaticCollectionAdapterContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryMethodCallContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: MethodCallContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryPropertyBuiltinContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: Builtin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryScalarIntegerMethodContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: ScalarIntegerMethodContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionConstructorContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionSentinelContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result_domain: DomainEvidence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RustOptionAndThenContract {
    pub receiver: MethodReceiverContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryRustOptionAndThenContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: RustOptionAndThenContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryFreeFunctionBuiltinContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub result: FreeFunctionBuiltinContract,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryReceiverMethodApiContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub rule: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryApiEvidenceStatus {
    Missing,
    Admitted,
    Rejected,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LibraryApiSpanEvidenceQuery {
    pub call_span: Option<Span>,
    pub callee_span: Option<Span>,
    pub receiver_span: Option<Span>,
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub arg_count: usize,
}

pub fn library_api_contract_evidence_for_call(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if il.kind(node) != NodeKind::Call || arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let span = il.node(node).span;
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || !library_api_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || !library_api_node_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee_node(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_at_call_span(
    il: &Il,
    interner: &Interner,
    query: LibraryApiSpanEvidenceQuery,
) -> LibraryApiEvidenceStatus {
    let Some(span) = query.call_span else {
        return LibraryApiEvidenceStatus::Missing;
    };
    if query.arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(query.id),
        callee_hash: library_api_callee_contract_hash(query.callee),
        arity: query.arg_count as u16,
    };
    let source_call = node_at_span_with_kind(il, span, NodeKind::Call);
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        let source_call_matches = source_call.is_some_and(|node| {
            library_api_source_call_spans_match_query(
                il,
                node,
                query.callee_span,
                query.receiver_span,
            ) && library_api_callee_shape_matches(il, interner, node, query.callee)
                && library_api_dependencies_match_callee(il, interner, node, query.callee, record)
        });
        let span_query_matches = library_api_dependencies_match_callee_at_span(
            il,
            interner,
            span,
            query.callee_span,
            query.receiver_span,
            query.callee,
            record,
        );
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || (!source_call_matches && !span_query_matches)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

fn library_api_source_call_spans_match_query(
    il: &Il,
    source_call: NodeId,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
) -> bool {
    let Some(&callee) = il.children(source_call).first() else {
        return false;
    };
    if callee_span.is_some_and(|span| il.node(callee).span != span) {
        return false;
    }
    if let Some(span) = receiver_span {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        if il.node(receiver).span != span {
            return false;
        }
    }
    true
}

pub fn library_api_receiver_dependencies_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
) -> Option<Vec<EvidenceId>> {
    let mut cache = LibraryApiDependencyCache::default();
    library_api_receiver_dependencies_for_call_with_cache(il, interner, call, callee, &mut cache)
}

#[derive(Default)]
pub struct LibraryApiDependencyCache {
    nearest_scope_by_node: FxHashMap<NodeId, Option<NodeId>>,
    binding_lhs_by_reference: FxHashMap<NodeId, EvidenceResolution<NodeId>>,
    receiver_param_span_by_reference: FxHashMap<NodeId, Option<Span>>,
    name_assigned_in_scope: FxHashMap<(NodeId, Symbol), bool>,
}

pub fn library_api_receiver_dependencies_for_call_with_cache(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let (&callee_node, args) = il.children(call).split_first()?;
    match callee {
        LibraryApiCalleeContract::Method { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            method_receiver_dependency_ids(il, interner, receiver_node, receiver, args, cache)
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            iterator_adapter_receiver_dependency_ids(il, interner, receiver_node, receiver, cache)
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => None,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            static_index_membership_receiver_dependency_id(il, interner, receiver_node, receiver)
                .map(|dependency| vec![dependency])
        }
        _ => Some(Vec::new()),
    }
}

pub fn library_api_property_dependencies_for_field_with_cache(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let LibraryApiCalleeContract::Property { property, receiver } = callee else {
        return None;
    };
    if !field_method_matches(il, interner, field, property) {
        return None;
    }
    let receiver_node = il.children(field).first().copied()?;
    method_receiver_dependency_ids(il, interner, receiver_node, receiver, &[], cache)
}

fn library_api_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
) -> bool {
    let Some(&callee_node) = il.children(node).first() else {
        return false;
    };
    match callee {
        LibraryApiCalleeContract::FreeName { .. } | LibraryApiCalleeContract::RustMacro { .. } => {
            il.kind(callee_node) == NodeKind::Var
        }
        LibraryApiCalleeContract::JsGlobalConstructor { receiver, .. } => {
            var_name_matches(il, interner, callee_node, receiver)
        }
        LibraryApiCalleeContract::ImportedBinding { exported, .. } => {
            imported_member_callee_shape_matches(il, interner, callee_node, exported)
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method } => {
            let Some((actual_receiver, actual_method)) =
                static_member_callee_parts(il, interner, callee_node)
            else {
                return false;
            };
            actual_receiver == receiver && actual_method == method
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            ..
        } => {
            var_name_matches(il, interner, callee_node, simple_type)
                || var_name_matches(il, interner, callee_node, qualified_type)
        }
        LibraryApiCalleeContract::RubyRequireStaticMember { method, .. } => {
            if il.kind(callee_node) != NodeKind::Field {
                return false;
            }
            let Some(&receiver) = il.children(callee_node).first() else {
                return false;
            };
            il.kind(receiver) == NodeKind::Var
                && field_method_matches(il, interner, callee_node, method)
        }
        LibraryApiCalleeContract::RegexLiteralMethod { method, .. } => {
            field_method_matches(il, interner, callee_node, method)
        }
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, .. } => {
            method_callee_receiver(il, interner, callee_node, method).is_some()
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { function, .. } => {
            field_method_matches(il, interner, callee_node, function)
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver, method, ..
        } => {
            let Some((actual_receiver, actual_method)) =
                static_member_callee_parts(il, interner, callee_node)
            else {
                return false;
            };
            actual_receiver == receiver && actual_method == method
        }
        LibraryApiCalleeContract::StaticGlobalFunction { function, .. } => {
            var_name_matches(il, interner, callee_node, function)
        }
        LibraryApiCalleeContract::Method { method, .. }
        | LibraryApiCalleeContract::AsyncMethod { method, .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { method, .. } => {
            method_callee_receiver(il, interner, callee_node, method).is_some()
        }
    }
}

fn library_api_dependencies_match_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    let Some(&callee_node) = il.children(node).first() else {
        return false;
    };
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            dependency_has_unshadowed_global_node(il, record, callee_node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(callee_node).span)
                })
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            dependency_has_source_call(
                il,
                record,
                il.node(node).span,
                SourceCallKind::MacroInvocation,
            ) && dependency_has_unshadowed_global_node(il, record, callee_node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(callee_node).span)
                })
        }
        LibraryApiCalleeContract::JsGlobalConstructor {
            receiver,
            requires_unshadowed_global,
        } => {
            dependency_has_source_call(il, record, il.node(node).span, SourceCallKind::Construct)
                && (!requires_unshadowed_global
                    || dependency_has_unshadowed_global_node(il, record, callee_node, receiver))
        }
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            dependency_has_imported_member_node(il, interner, record, callee_node, module, exported)
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_imported_binding_node(
                il,
                interner,
                record,
                receiver_node,
                "java.util",
                receiver,
            ) && !unit_defines_hash_visible_at(
                il,
                interner,
                stable_symbol_hash(receiver),
                il.node(receiver_node).span,
            )
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            requires_import_for_simple_type,
            requires_no_local_type_shadow,
        } => {
            dependency_has_source_call(il, record, il.node(node).span, SourceCallKind::Construct)
                && java_constructor_dependencies_match(
                    il,
                    interner,
                    record,
                    callee_node,
                    il.node(node).span,
                    simple_type,
                    qualified_type,
                    module,
                    requires_import_for_simple_type,
                    requires_no_local_type_shadow,
                )
        }
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            required_module,
            shadow_root,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_unshadowed_global_node(il, record, receiver_node, receiver)
                && dependency_has_required_module_before(
                    record,
                    il,
                    interner,
                    required_module,
                    il.node(node).span,
                )
                && !file_defines_name_visible_at(
                    il,
                    interner,
                    shadow_root,
                    il.node(receiver_node).span,
                )
        }
        LibraryApiCalleeContract::RegexLiteralMethod {
            required_receiver_fact,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_source_fact_node(il, record, receiver_node, required_receiver_fact)
        }
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            let Some(receiver_node) = method_callee_receiver(il, interner, callee_node, method)
            else {
                return false;
            };
            static_index_membership_receiver_dependency_id(il, interner, receiver_node, receiver)
                .is_some_and(|dependency| dependency_ids_are_present(record, &[dependency]))
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, .. } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_imported_namespace_node(il, interner, record, receiver_node, module)
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver,
            qualified_path,
            requires_unshadowed_receiver,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_qualified_global_node(il, record, callee_node, qualified_path)
                && (!requires_unshadowed_receiver
                    || dependency_has_unshadowed_global_node(il, record, receiver_node, receiver))
        }
        LibraryApiCalleeContract::StaticGlobalFunction {
            function,
            requires_unshadowed_function,
        } => {
            !requires_unshadowed_function
                || dependency_has_unshadowed_global_node(il, record, callee_node, function)
        }
        LibraryApiCalleeContract::Method { .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { .. } => {
            library_api_receiver_dependencies_for_call(il, interner, node, callee)
                .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => false,
    }
}

fn library_api_node_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => {
            var_name_matches(il, interner, node, name)
        }
        LibraryApiCalleeContract::Property { property, .. } => {
            field_method_matches(il, interner, node, property)
        }
        _ => false,
    }
}

fn library_api_dependencies_match_callee_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            dependency_has_unshadowed_global_node(il, record, node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(node).span)
                })
        }
        LibraryApiCalleeContract::Property { .. } => {
            let mut cache = LibraryApiDependencyCache::default();
            library_api_property_dependencies_for_field_with_cache(
                il, interner, node, callee, &mut cache,
            )
            .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_node: NodeId,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    let Some(actual) = node_name(il, interner, callee_node) else {
        return false;
    };
    java_constructor_dependencies_match_for_name(
        il,
        interner,
        record,
        actual,
        Some(callee_node),
        il.node(callee_node).span,
        call_span,
        simple_type,
        qualified_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
    )
}

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_span: Span,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    let Some(callee_node) = node_at_span_with_kind(il, callee_span, NodeKind::Var) else {
        return false;
    };
    java_constructor_dependencies_match(
        il,
        interner,
        record,
        callee_node,
        call_span,
        simple_type,
        qualified_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
    )
}

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match_for_name(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    actual: &str,
    callee_node: Option<NodeId>,
    callee_span: Span,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    if actual == qualified_type {
        return true;
    }
    if actual != simple_type {
        return false;
    }
    if requires_no_local_type_shadow
        && unit_defines_hash_visible_at(il, interner, stable_symbol_hash(simple_type), callee_span)
    {
        return false;
    }
    if !requires_import_for_simple_type {
        return true;
    }
    let explicit_import = callee_node.is_some_and(|node| {
        dependency_has_imported_binding_node(il, interner, record, node, module, simple_type)
    });
    explicit_import
        || dependency_has_java_wildcard_import_before(
            il,
            interner,
            record,
            module,
            simple_type,
            call_span,
        )
}

fn dependency_has_java_wildcard_import_before(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    simple_type: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == expected
            && matches!(
                dependency.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == call_span.file && span.end_byte <= call_span.start_byte
            )
            && !java_explicit_import_conflicts(il, interner, module, simple_type)
    })
}

fn java_explicit_import_conflicts(
    il: &Il,
    _interner: &Interner,
    module: &str,
    simple_type: &str,
) -> bool {
    let local_hash = stable_symbol_hash(simple_type);
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(simple_type),
    };
    il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && matches!(record.kind, EvidenceKind::Symbol(actual) if actual != expected)
            && record.status == EvidenceStatus::Asserted
    })
}

fn library_api_dependencies_match_callee_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            callee_span.is_some_and(|span| {
                dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
            }) && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                callee_span
                    .is_some_and(|span| file_defines_name_visible_at(il, interner, candidate, span))
            })
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::MacroInvocation)
                && callee_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
                })
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    callee_span.is_some_and(|span| {
                        file_defines_name_visible_at(il, interner, candidate, span)
                    })
                })
        }
        LibraryApiCalleeContract::JsGlobalConstructor {
            receiver,
            requires_unshadowed_global,
        } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::Construct)
                && (!requires_unshadowed_global
                    || callee_span.is_some_and(|span| {
                        dependency_has_unshadowed_global_anchor(
                            il,
                            record,
                            span,
                            NodeKind::Var,
                            receiver,
                        )
                    }))
        }
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            if let Some(span) = receiver_span {
                dependency_has_imported_namespace_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                )
            } else if let Some(span) = callee_span {
                dependency_has_imported_binding_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                    exported,
                ) || dependency_has_imported_namespace_dependency(il, interner, record, module)
            } else {
                dependency_has_imported_binding_dependency(il, interner, record, module, exported)
                    || dependency_has_imported_namespace_dependency(il, interner, record, module)
            }
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
            let receiver_proven = if let Some(span) = receiver_span {
                dependency_has_imported_binding_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    "java.util",
                    receiver,
                )
            } else {
                dependency_has_imported_binding_dependency(
                    il,
                    interner,
                    record,
                    "java.util",
                    receiver,
                )
            };
            receiver_proven
                && if let Some(span) = receiver_span {
                    !unit_defines_hash_visible_at(il, interner, stable_symbol_hash(receiver), span)
                } else {
                    !unit_defines_hash(il, interner, stable_symbol_hash(receiver))
                }
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            requires_import_for_simple_type,
            requires_no_local_type_shadow,
        } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::Construct)
                && callee_span.is_some_and(|span| {
                    java_constructor_dependencies_match_at_span(
                        il,
                        interner,
                        record,
                        span,
                        call_span,
                        simple_type,
                        qualified_type,
                        module,
                        requires_import_for_simple_type,
                        requires_no_local_type_shadow,
                    )
                })
        }
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            required_module,
            shadow_root,
            ..
        } => {
            receiver_span.is_some_and(|span| {
                dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, receiver)
            }) && dependency_has_required_module_before(
                record,
                il,
                interner,
                required_module,
                call_span,
            ) && receiver_span
                .is_some_and(|span| !file_defines_name_visible_at(il, interner, shadow_root, span))
        }
        LibraryApiCalleeContract::RegexLiteralMethod {
            required_receiver_fact,
            ..
        } => receiver_span.is_some_and(|span| {
            dependency_has_source_fact_anchor(il, record, span, required_receiver_fact)
        }),
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    static_index_membership_receiver_dependency_id_at_span(
                        il, interner, span, receiver,
                    )
                    .is_some_and(|dependency| dependency_ids_are_present(record, &[dependency]))
                })
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, .. } => {
            if let Some(span) = receiver_span {
                dependency_has_imported_namespace_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                )
            } else {
                dependency_has_imported_namespace_dependency(il, interner, record, module)
            }
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver,
            qualified_path,
            requires_unshadowed_receiver,
            ..
        } => {
            callee_span.is_some_and(|span| {
                dependency_has_qualified_global_anchor(
                    il,
                    record,
                    span,
                    NodeKind::Field,
                    qualified_path,
                )
            }) && (!requires_unshadowed_receiver
                || receiver_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(
                        il,
                        record,
                        span,
                        NodeKind::Var,
                        receiver,
                    )
                }))
        }
        LibraryApiCalleeContract::StaticGlobalFunction {
            function,
            requires_unshadowed_function,
        } => {
            !requires_unshadowed_function
                || callee_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(
                        il,
                        record,
                        span,
                        NodeKind::Var,
                        function,
                    )
                })
        }
        LibraryApiCalleeContract::Method { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    method_receiver_dependencies_at_span(il, interner, span, receiver).is_some_and(
                        |dependencies| dependency_ids_are_present(record, &dependencies),
                    )
                })
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    iterator_adapter_receiver_dependencies_at_span(il, interner, span, receiver)
                        .is_some_and(|dependencies| {
                            dependency_ids_are_present(record, &dependencies)
                        })
                })
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => false,
    }
}

fn method_callee_receiver(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_method: &str,
) -> Option<NodeId> {
    if !field_method_matches(il, interner, callee, expected_method) {
        return None;
    }
    il.children(callee).first().copied()
}

fn field_method_at_span(il: &Il, interner: &Interner, span: Span, expected: &str) -> bool {
    il.nodes.iter().any(|node| {
        node.span == span
            && node.kind == NodeKind::Field
            && matches!(node.payload, Payload::Name(method) if interner.resolve(method) == expected)
    })
}

fn method_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    args: &[NodeId],
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let mut dependencies = receiver_dependency_ids(il, interner, receiver, contract, cache)?;
    if contract == MethodReceiverContract::ExactProtocolPairArgument {
        let pair = *args.first()?;
        dependencies.extend(receiver_dependency_ids(
            il,
            interner,
            pair,
            MethodReceiverContract::ExactProtocol,
            cache,
        )?);
    }
    Some(dependencies)
}

fn iterator_adapter_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: IteratorAdapterReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        IteratorAdapterReceiverContract::ExactIterableValue => receiver_dependency_ids(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
            cache,
        ),
    }
}

fn method_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: MethodReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

fn iterator_adapter_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: IteratorAdapterReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    iterator_adapter_receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

fn node_at_span(il: &Il, span: Span) -> Option<NodeId> {
    let mut found = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span != span {
            continue;
        }
        let id = NodeId(idx as u32);
        match found {
            None => found = Some(id),
            Some(existing)
                if il.kind(existing) == node.kind && il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

fn node_at_span_with_kind(il: &Il, span: Span, kind: NodeKind) -> Option<NodeId> {
    let mut found = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span != span || node.kind != kind {
            continue;
        }
        let id = NodeId(idx as u32);
        match found {
            None => found = Some(id),
            Some(existing) if il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

fn receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        MethodReceiverContract::LiteralString => {
            matches!(il.node(receiver).payload, Payload::LitStr(_)).then_some(Vec::new())
        }
        MethodReceiverContract::UnshadowedGlobal(global) => {
            Some(vec![symbol_dependency_id_for_node(
                il,
                receiver,
                SymbolEvidenceKind::UnshadowedGlobal {
                    name_hash: stable_symbol_hash(global),
                },
            )?])
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            Some(vec![imported_symbol_dependency_id_for_node(
                il,
                interner,
                receiver,
                SymbolEvidenceKind::ImportedNamespace {
                    module_hash: stable_symbol_hash(module),
                },
            )?])
        }
        MethodReceiverContract::ExactMapLiteral => {
            Some(vec![sequence_surface_dependency_id_for_receiver(
                il, interner, receiver, contract,
            )?])
        }
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
        }
        MethodReceiverContract::ExactCollection | MethodReceiverContract::ExactCollectionOrMap => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) =
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
            {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection],
            )
            .map(|id| vec![id])
        }
        MethodReceiverContract::RustMapGetOrExactOption => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            library_api_dependency_id_for_call(il, interner, receiver, LibraryApiContractId::MapGet)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) = library_api_dependency_id_for_call(
                il,
                interner,
                receiver,
                LibraryApiContractId::MapKeyView(MapKeyViewKind::Collection),
            ) {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactProtocol => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) = library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
            ) {
                return Some(vec![id]);
            }
            if let Some(id) =
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
            {
                return Some(vec![id]);
            }
            if let Some(id) = library_api_dependency_id_for_normalized_hof(il, receiver) {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
        }
        MethodReceiverContract::ExactProtocolPairArgument => domain_or_sequence_dependency_ids(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
            cache,
        )
        .or_else(|| {
            library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
            )
            .map(|id| vec![id])
        })
        .or_else(|| {
            library_api_dependency_id_for_receiver_domain_call(
                il,
                interner,
                receiver,
                MethodReceiverContract::ExactProtocol,
            )
            .map(|id| vec![id])
        })
        .or_else(|| library_api_dependency_id_for_normalized_hof(il, receiver).map(|id| vec![id]))
        .or_else(|| {
            library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
        }),
        _ => domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache).or_else(
            || {
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                    .map(|id| vec![id])
            },
        ),
    }
}

fn domain_or_sequence_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    if let Some(id) = domain_dependency_id_for_receiver(il, interner, receiver, contract, cache) {
        return Some(vec![id]);
    }
    sequence_surface_dependency_id_for_receiver(il, interner, receiver, contract).map(|id| vec![id])
}

fn domain_dependency_id_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
            || !requirement.accepts(domain)
            || !domain_dependency_anchor_matches_receiver(
                il,
                interner,
                receiver,
                record.anchor,
                cache,
            )
        {
            continue;
        }
        match found {
            None => found = Some((domain, record.id)),
            Some((existing, _)) if existing == domain => {}
            Some(_) => return None,
        }
    }
    found.map(|(_, id)| id)
}

fn domain_dependency_anchor_matches_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    anchor: EvidenceAnchor,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    match anchor {
        EvidenceAnchor::Node { span, kind } => {
            span == il.node(receiver).span && kind == il.kind(receiver)
        }
        EvidenceAnchor::Binding { span, local_hash } => {
            matches!(
                unique_binding_lhs_for_var_reference_cached(il, receiver, cache),
                EvidenceResolution::Found(lhs)
                    if il.node(lhs).span == span
                        && node_name_hash(il, interner, lhs) == Some(local_hash)
            )
        }
        EvidenceAnchor::Param { span } => {
            receiver_param_span_cached(il, receiver, cache) == Some(span)
        }
        _ => false,
    }
}

fn unique_binding_lhs_for_var_reference_cached(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    if let Some(&cached) = cache.binding_lhs_by_reference.get(&node) {
        return cached;
    }
    let resolution = unique_binding_lhs_for_var_reference_with_cache(il, node, cache);
    cache.binding_lhs_by_reference.insert(node, resolution);
    resolution
}

fn unique_binding_lhs_for_var_reference_with_cache(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    let scope = nearest_scope_cached(il, node, cache);
    let reference_is_free_name = matches!(il.node(node).payload, Payload::Name(_));
    let mut found = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if candidate.kind != NodeKind::Assign {
            continue;
        }
        let assign = NodeId(idx as u32);
        let assignment_scope = nearest_scope_cached(il, assign, cache);
        if assignment_scope != scope && !(reference_is_free_name && assignment_scope.is_none()) {
            continue;
        }
        if !assignment_is_visible_at_reference(il, assign, node) {
            continue;
        }
        let Some(&lhs) = il.children(assign).first() else {
            continue;
        };
        if !var_references_same_binding(il, lhs, node) {
            continue;
        }
        match found {
            None => found = Some(lhs),
            Some(existing) if existing == lhs => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn nearest_scope_cached(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<NodeId> {
    if let Some(cached) = cache.nearest_scope_by_node.get(&node).copied() {
        return cached;
    }
    let scope = nearest_scope(il, node);
    cache.nearest_scope_by_node.insert(node, scope);
    scope
}

fn receiver_param_span_cached(
    il: &Il,
    receiver: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    if let Some(cached) = cache
        .receiver_param_span_by_reference
        .get(&receiver)
        .copied()
    {
        return cached;
    }
    let span = receiver_var_payload(il, receiver).and_then(|payload| match payload {
        Payload::Cid(cid) => receiver_cid_param_span_with_cache(il, receiver, cid, cache),
        Payload::Name(name) => receiver_named_param_span_with_cache(il, receiver, name, cache),
        _ => None,
    });
    cache
        .receiver_param_span_by_reference
        .insert(receiver, span);
    span
}

fn receiver_var_payload(il: &Il, receiver: NodeId) -> Option<Payload> {
    (il.kind(receiver) == NodeKind::Var).then_some(il.node(receiver).payload)
}

fn receiver_cid_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    cid: u32,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    let scope = nearest_scope_cached(il, receiver, cache);
    let mut found = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if candidate.kind != NodeKind::Param {
            continue;
        }
        let id = NodeId(idx as u32);
        if nearest_scope_cached(il, id, cache) != scope {
            continue;
        }
        if !matches!(candidate.payload, Payload::Cid(param_cid) if param_cid == cid) {
            continue;
        }
        match found {
            None => found = Some(candidate.span),
            Some(existing) if existing == candidate.span => {}
            Some(_) => return None,
        }
    }
    found
}

fn receiver_named_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    name: Symbol,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    let (scope, param) = nearest_named_param_scope(il, receiver, name)?;
    (!name_is_assigned_in_scope_cached(il, name, scope, cache)).then_some(il.node(param).span)
}

fn name_is_assigned_in_scope_cached(
    il: &Il,
    name: Symbol,
    scope: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    if let Some(&assigned) = cache.name_assigned_in_scope.get(&(scope, name)) {
        return assigned;
    }
    let assigned = il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope_cached(il, id, cache) != Some(scope) {
            return false;
        }
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var && il.node(lhs).payload == Payload::Name(name)
    });
    cache.name_assigned_in_scope.insert((scope, name), assigned);
    assigned
}

fn sequence_surface_dependency_id_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> Option<EvidenceId> {
    if il.kind(receiver) != NodeKind::Seq {
        return None;
    }
    let surface = seq_surface_contract_for_node(il, interner, receiver)?;
    if !sequence_surface_satisfies_method_receiver(surface, contract) {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(il.node(receiver).span);
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return None,
        }
    }
    found.map(|(_, id)| id)
}

fn static_index_membership_receiver_dependency_id(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: StaticIndexMembershipReceiverContract,
) -> Option<EvidenceId> {
    static_index_membership_receiver_dependency_id_at_span(
        il,
        interner,
        il.node(receiver).span,
        contract,
    )
    .filter(|_| static_index_membership_receiver_shape_matches(il, interner, receiver, contract))
}

fn static_index_membership_receiver_dependency_id_at_span(
    il: &Il,
    interner: &Interner,
    span: Span,
    contract: StaticIndexMembershipReceiverContract,
) -> Option<EvidenceId> {
    let receiver = node_at_span_with_kind(il, span, NodeKind::Seq)?;
    if !static_index_membership_receiver_shape_matches(il, interner, receiver, contract) {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(span);
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return None,
        }
    }
    found.and_then(|(kind, id)| (kind == SequenceSurfaceKind::Collection).then_some(id))
}

fn static_index_membership_receiver_shape_matches(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: StaticIndexMembershipReceiverContract,
) -> bool {
    match contract {
        StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection => {
            if il.kind(receiver) != NodeKind::Seq {
                return false;
            }
            if !seq_surface_contract_for_node(il, interner, receiver)
                .is_some_and(|surface| surface.membership_collection)
            {
                return false;
            }
            let kids = il.children(receiver);
            !kids.is_empty()
                && kids.iter().all(|&kid| {
                    il.kind(kid) == NodeKind::Lit
                        && matches!(
                            il.node(kid).payload,
                            Payload::LitInt(_)
                                | Payload::LitBool(_)
                                | Payload::LitStr(_)
                                | Payload::Lit(LitClass::Null)
                        )
                })
        }
    }
}

fn sequence_surface_satisfies_method_receiver(
    surface: SeqSurfaceContract,
    contract: MethodReceiverContract,
) -> bool {
    match contract {
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactProtocol
        | MethodReceiverContract::ExactProtocolPairArgument
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => surface.membership_collection,
        MethodReceiverContract::ExactMap | MethodReceiverContract::ExactMapLiteral => {
            surface.value_tag == SEQ_VALUE_MAP
        }
        MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrMapLiteral => {
            surface.membership_collection || surface.value_tag == SEQ_VALUE_MAP
        }
        MethodReceiverContract::ExactSetOrMap => surface.value_tag == SEQ_VALUE_MAP,
        _ => false,
    }
}

fn symbol_dependency_id_for_node(
    il: &Il,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn imported_symbol_dependency_id_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && imported_occurrence_symbol_dependencies_valid(il, interner, record, expected))
        .then_some(record.id)
    })
}

fn library_api_dependency_id_for_normalized_hof(il: &Il, receiver: NodeId) -> Option<EvidenceId> {
    let Payload::HoF(kind) = il.node(receiver).payload else {
        return None;
    };
    let expected_id = LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(kind));
    let expected_contract_hash = library_api_contract_id_hash(expected_id);
    let anchor = EvidenceAnchor::node(il.node(receiver).span, NodeKind::Call);
    let mut found = None;
    for record in &il.evidence {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            ..
        }) = record.kind
        else {
            continue;
        };
        if contract_hash != expected_contract_hash {
            continue;
        }
        if library_api_callee_contract_for_hash(il.meta.lang, expected_id, callee_hash).is_none() {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn library_api_dependency_id_for_protocol_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<EvidenceId> {
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::IteratorIdentityAdapter,
    ) {
        return Some(id);
    }
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::StaticCollectionAdapter,
    ) {
        return Some(id);
    }
    library_api_dependency_id_for_call_predicate(il, interner, call, |id| {
        matches!(
            id,
            LibraryApiContractId::MethodCall(
                MethodSemanticContract::HoF(_) | MethodSemanticContract::Builtin(Builtin::Zip)
            )
        )
    })
}

fn library_api_dependency_id_for_receiver_domain_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract: MethodReceiverContract,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    library_api_dependency_id_for_call_contract(il, interner, call, |id, callee, arity| {
        library_api_contract_result_domain_for_arity(id, callee, arity)
            .is_some_and(|domain| requirement.accepts(domain))
    })
}

fn library_api_dependency_id_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    id: LibraryApiContractId,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(il, interner, call, |actual| actual == id)
}

fn library_api_dependency_id_for_map_key_view_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    allowed: &[MapKeyViewKind],
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(
        il,
        interner,
        call,
        |id| matches!(id, LibraryApiContractId::MapKeyView(kind) if allowed.contains(&kind)),
    )
}

fn library_api_dependency_id_for_call_predicate(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId) -> bool,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_contract(il, interner, call, |id, _, _| accepts(id))
}

fn library_api_dependency_id_for_call_contract(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId, LibraryApiCalleeContract, u16) -> bool,
) -> Option<EvidenceId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    let mut found = None;
    for record in &il.evidence {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = record.kind
        else {
            continue;
        };
        let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
            continue;
        };
        let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
        else {
            continue;
        };
        if !accepts(id, callee, arity) {
            continue;
        }
        if !library_api_record_admitted_for_current_shape(il, interner, call, record) {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn library_api_contract_result_domain_for_arity(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<DomainEvidence> {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::PythonImportedCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory
        | LibraryApiContractId::RustVecMacroFactory
        | LibraryApiContractId::RustVecNewFactory
        | LibraryApiContractId::JavaCollectionFactory(_)
        | LibraryApiContractId::JavaCollectionConstructor(_)
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => {
            library_collection_factory_result_domain_for_arity(
                LibraryCollectionFactoryContract {
                    id,
                    callee,
                    result: LibraryCollectionFactoryResult::SequenceArgument,
                },
                arity as usize,
            )
        }
        LibraryApiContractId::RustStdMapFactory
        | LibraryApiContractId::JavaMapFactory(_)
        | LibraryApiContractId::JsLikeMapConstructor => Some(library_map_factory_result_domain(
            LibraryMapFactoryContract {
                id,
                callee,
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_COLLECTION,
                },
            },
        )),
        LibraryApiContractId::MapKeyViewWrapper => Some(
            library_map_key_view_wrapper_result_domain(LibraryMapKeyViewWrapperContract {
                id,
                callee,
                result: MapKeyViewWrapperContract {
                    receiver: "Array",
                    method: "from",
                    qualified_path: "Array.from",
                },
            }),
        ),
        LibraryApiContractId::RustOptionSomeConstructor => Some(DomainEvidence::Option),
        LibraryApiContractId::ScalarIntegerMethod(_) => Some(DomainEvidence::Integer),
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(_)) => {
            Some(DomainEvidence::Collection)
        }
        _ => None,
    }
}

fn library_api_contract_id_from_hash(hash: u64) -> Option<LibraryApiContractId> {
    library_api_contract_ids()
        .into_iter()
        .find(|id| library_api_contract_id_hash(*id) == hash)
}

fn library_api_contract_ids() -> Vec<LibraryApiContractId> {
    let mut ids = vec![
        LibraryApiContractId::PropertyBuiltin(Builtin::Len),
        LibraryApiContractId::PythonBuiltinCollectionFactory,
        LibraryApiContractId::PythonImportedCollectionFactory,
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Append),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Print),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Range),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Sum),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Min),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Max),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Abs),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Enumerate),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::All),
        LibraryApiContractId::RustOptionSomeConstructor,
        LibraryApiContractId::RustOptionNoneSentinel,
        LibraryApiContractId::RustOptionAndThen,
        LibraryApiContractId::RustStdCollectionFactory,
        LibraryApiContractId::RustStdMapFactory,
        LibraryApiContractId::RustVecMacroFactory,
        LibraryApiContractId::RustVecNewFactory,
        LibraryApiContractId::JavaMapEntryFactory,
        LibraryApiContractId::RubySetFactory,
        LibraryApiContractId::JsLikeSetConstructor,
        LibraryApiContractId::JsLikeMapConstructor,
        LibraryApiContractId::MapKeyViewWrapper,
        LibraryApiContractId::MapGet,
        LibraryApiContractId::JsArrayIsArray,
        LibraryApiContractId::JsBooleanCoercion,
        LibraryApiContractId::RegexTest,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::IndexOf),
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::FindIndex),
        LibraryApiContractId::PromiseThen,
        LibraryApiContractId::IteratorIdentityAdapter,
        LibraryApiContractId::StaticCollectionAdapter,
    ];
    ids.extend(
        [
            ScalarIntegerMethod::Abs,
            ScalarIntegerMethod::Min,
            ScalarIntegerMethod::Max,
            ScalarIntegerMethod::Clamp,
        ]
        .into_iter()
        .map(LibraryApiContractId::ScalarIntegerMethod),
    );
    ids.extend(
        [
            JavaCollectionFactoryKind::ListOf,
            JavaCollectionFactoryKind::SetOf,
            JavaCollectionFactoryKind::ArraysAsList,
        ]
        .into_iter()
        .map(LibraryApiContractId::JavaCollectionFactory),
    );
    ids.push(LibraryApiContractId::JavaCollectionConstructor(
        JavaCollectionConstructorKind::EmptyList,
    ));
    ids.extend(
        [JavaMapFactoryKind::Of, JavaMapFactoryKind::OfEntries]
            .into_iter()
            .map(LibraryApiContractId::JavaMapFactory),
    );
    ids.extend(
        [MapKeyViewKind::Collection, MapKeyViewKind::Iterator]
            .into_iter()
            .map(LibraryApiContractId::MapKeyView),
    );
    ids.extend(
        [ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        }]
        .into_iter()
        .map(LibraryApiContractId::ImportedNamespaceFunction),
    );
    ids.extend(
        [
            MethodSemanticContract::Builtin(Builtin::Append),
            MethodSemanticContract::Builtin(Builtin::Print),
            MethodSemanticContract::Builtin(Builtin::Len),
            MethodSemanticContract::Builtin(Builtin::IsEmpty),
            MethodSemanticContract::Builtin(Builtin::IsNull),
            MethodSemanticContract::Builtin(Builtin::IsNotNull),
            MethodSemanticContract::Builtin(Builtin::StartsWith),
            MethodSemanticContract::Builtin(Builtin::EndsWith),
            MethodSemanticContract::Builtin(Builtin::Contains),
            MethodSemanticContract::Builtin(Builtin::Join),
            MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            MethodSemanticContract::Builtin(Builtin::ValueOrDefault),
            MethodSemanticContract::Builtin(Builtin::Reduce),
            MethodSemanticContract::Builtin(Builtin::Sum),
            MethodSemanticContract::Builtin(Builtin::Abs),
            MethodSemanticContract::Builtin(Builtin::Min),
            MethodSemanticContract::Builtin(Builtin::Max),
            MethodSemanticContract::Builtin(Builtin::Zip),
            MethodSemanticContract::Builtin(Builtin::Any),
            MethodSemanticContract::Builtin(Builtin::All),
            MethodSemanticContract::HoF(HoFKind::Map),
            MethodSemanticContract::HoF(HoFKind::Filter),
            MethodSemanticContract::HoF(HoFKind::FlatMap),
            MethodSemanticContract::HoF(HoFKind::FilterMap),
        ]
        .into_iter()
        .map(LibraryApiContractId::MethodCall),
    );
    ids
}

fn library_api_record_admitted_for_current_shape(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    record: &EvidenceRecord,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash,
        callee_hash,
        arity,
    }) = record.kind
    else {
        return false;
    };
    let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
        return false;
    };
    let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash) else {
        return false;
    };
    matches!(
        library_api_contract_evidence_for_call(il, interner, call, id, callee, arity as usize),
        LibraryApiEvidenceStatus::Admitted
    )
}

fn library_api_callee_contract_for_hash(
    lang: Lang,
    id: LibraryApiContractId,
    hash: u64,
) -> Option<LibraryApiCalleeContract> {
    library_api_callee_contracts_for_id(lang, id)
        .into_iter()
        .find(|callee| library_api_callee_contract_hash(*callee) == hash)
}

fn library_api_callee_contracts_for_id(
    lang: Lang,
    id: LibraryApiContractId,
) -> Vec<LibraryApiCalleeContract> {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin) => ["length"]
            .into_iter()
            .filter_map(|property| library_property_builtin_contract(lang, property))
            .filter(|contract| contract.id == LibraryApiContractId::PropertyBuiltin(builtin))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory => {
            library_free_name_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            library_imported_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            library_free_function_builtin_callee_contracts_for_id(lang, builtin)
        }
        LibraryApiContractId::RustOptionSomeConstructor => [
            "Some",
            "Option::Some",
            "std::option::Option::Some",
            "core::option::Option::Some",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_some_constructor_contract(lang, name, 1))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionNoneSentinel => [
            "None",
            "Option::None",
            "std::option::Option::None",
            "core::option::Option::None",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_none_sentinel_contract(lang, name))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionAndThen => {
            library_rust_option_and_then_contract(lang, "and_then", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::ScalarIntegerMethod(method) => ["abs", "min", "max", "clamp"]
            .into_iter()
            .filter_map(|name| library_scalar_integer_method_contract(lang, name, 0))
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 1)),
            )
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 2)),
            )
            .filter(|contract| contract.id == LibraryApiContractId::ScalarIntegerMethod(method))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustStdMapFactory => library_free_name_map_factory_contracts(lang)
            .filter(|contract| contract.id == id)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustVecMacroFactory => {
            library_rust_vec_macro_factory_contract(lang, "vec")
                .filter(|contract| contract.id == id)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RustVecNewFactory => {
            ["Vec::new", "std::vec::Vec::new", "alloc::vec::Vec::new"]
                .into_iter()
                .filter_map(|name| library_rust_vec_new_factory_contract(lang, name))
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionFactory(kind) => {
            [("List", "of"), ("Set", "of"), ("Arrays", "asList")]
                .into_iter()
                .filter_map(|(receiver, method)| {
                    library_java_collection_factory_contract(lang, receiver, method)
                })
                .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionFactory(kind))
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionConstructor(kind) => [
            "ArrayList",
            "java.util.ArrayList",
            "LinkedList",
            "java.util.LinkedList",
        ]
        .into_iter()
        .filter_map(|type_name| library_java_collection_constructor_contract(lang, type_name, 0))
        .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionConstructor(kind))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::JavaMapFactory(kind) => ["of", "ofEntries"]
            .into_iter()
            .filter_map(|method| library_java_map_factory_contract(lang, "Map", method))
            .filter(|contract| contract.id == LibraryApiContractId::JavaMapFactory(kind))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::JavaMapEntryFactory => {
            library_java_map_entry_contract(lang, "Map", "entry")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RubySetFactory => {
            library_ruby_set_factory_contract(lang, "Set", "new", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeSetConstructor => {
            library_js_like_set_constructor_contract(lang, "Set")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeMapConstructor => {
            library_js_like_map_constructor_contract(lang, "Map")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MapKeyViewWrapper => {
            library_map_key_view_wrapper_contract(lang, "Array", "from", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeStaticIndexMembership(kind) => ["indexOf", "findIndex"]
            .into_iter()
            .filter_map(|method| library_static_index_membership_contract(lang, method, 1))
            .filter(|contract| {
                contract.id == LibraryApiContractId::JsLikeStaticIndexMembership(kind)
            })
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::MapGet => ["get"]
            .into_iter()
            .filter_map(|method| {
                library_map_get_contract(lang, method, 1).map(|contract| contract.callee)
            })
            .collect(),
        LibraryApiContractId::MapKeyView(kind) => ["keys", "keySet"]
            .into_iter()
            .filter_map(|method| library_map_key_view_contract(lang, method, 0))
            .filter(|contract| contract.result.kind == kind)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::IteratorIdentityAdapter => {
            let methods = [
                "iter",
                "into_iter",
                "iter_mut",
                "collect",
                "to_vec",
                "copied",
                "cloned",
                "stream",
            ];
            methods
                .into_iter()
                .filter_map(|method| {
                    library_iterator_identity_adapter_contract(lang, method, 0)
                        .map(|contract| contract.callee)
                })
                .collect()
        }
        LibraryApiContractId::StaticCollectionAdapter => {
            library_static_collection_adapter_contract(lang, "Arrays", "stream", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MethodCall(semantic) => {
            method_call_contract_callees_for_semantic(lang, semantic)
        }
        _ => Vec::new(),
    }
}

fn library_free_function_builtin_callee_contracts_for_id(
    lang: Lang,
    builtin: Builtin,
) -> Vec<LibraryApiCalleeContract> {
    let candidate = match (lang, builtin) {
        (Lang::Python, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Append) => Some(("append", 2)),
        (Lang::Python, Builtin::Print) => Some(("print", 0)),
        (Lang::Python, Builtin::Range) => Some(("range", 1)),
        (Lang::Python, Builtin::Sum) => Some(("sum", 1)),
        (Lang::Python, Builtin::Min) => Some(("min", 1)),
        (Lang::Python, Builtin::Max) => Some(("max", 1)),
        (Lang::Python, Builtin::Abs) => Some(("abs", 1)),
        (Lang::Python, Builtin::Zip) => Some(("zip", 2)),
        (Lang::Python, Builtin::Enumerate) => Some(("enumerate", 1)),
        (Lang::Python, Builtin::Any) => Some(("any", 1)),
        (Lang::Python, Builtin::All) => Some(("all", 1)),
        _ => None,
    };
    candidate
        .and_then(|(name, arg_count)| library_free_function_builtin_contract(lang, name, arg_count))
        .map(|contract| vec![contract.callee])
        .unwrap_or_default()
}

fn method_call_contract_callees_for_semantic(
    lang: Lang,
    semantic: MethodSemanticContract,
) -> Vec<LibraryApiCalleeContract> {
    let methods = [
        "append",
        "push",
        "log",
        "info",
        "debug",
        "Println",
        "Printf",
        "Print",
        "Abs",
        "HasPrefix",
        "HasSuffix",
        "Contains",
        "len",
        "size",
        "length",
        "is_empty",
        "isEmpty",
        "empty?",
        "nil?",
        "is_none",
        "is_some",
        "startsWith",
        "startswith",
        "starts_with",
        "start_with?",
        "endsWith",
        "endswith",
        "ends_with",
        "end_with?",
        "containsKey",
        "contains_key",
        "key?",
        "has_key?",
        "__contains__",
        "includes",
        "include?",
        "member?",
        "contains",
        "has",
        "join",
        "get",
        "fetch",
        "getOrDefault",
        "unwrap_or",
        "unwrap_or_else",
        "map_or",
        "reduce",
        "Min",
        "Max",
        "abs",
        "min",
        "max",
        "zip",
        "fold",
        "inject",
        "map",
        "collect",
        "filter",
        "select",
        "flatMap",
        "flat_map",
        "filter_map",
        "some",
        "every",
        "all",
        "any",
        "all?",
        "any?",
        "allMatch",
        "anyMatch",
        "sum",
        "count",
    ];
    methods
        .into_iter()
        .flat_map(|method| {
            (0..=3).filter_map(move |arity| library_method_call_contract(lang, method, arity))
        })
        .filter(|contract| contract.result.semantic == semantic)
        .map(|contract| contract.callee)
        .collect()
}

fn dependency_ids_are_present(record: &EvidenceRecord, dependencies: &[EvidenceId]) -> bool {
    dependencies
        .iter()
        .all(|dependency| record.dependencies.contains(dependency))
}

fn var_name_matches(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Var, Payload::Name(name)) if interner.resolve(name) == expected
    )
}

fn static_member_callee_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<(&'a str, &'a str)> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    let receiver = il.children(node).first().copied()?;
    if il.kind(receiver) != NodeKind::Var {
        return None;
    }
    let receiver_name = node_name(il, interner, receiver)?;
    Some((receiver_name, interner.resolve(method)))
}

fn imported_member_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    exported: &str,
) -> bool {
    match il.kind(node) {
        // Aliased imports are proven by the imported-binding dependency, not by
        // comparing the local callee spelling to the exported API name.
        NodeKind::Var => true,
        NodeKind::Field => field_method_matches(il, interner, node, exported),
        _ => false,
    }
}

fn field_method_matches(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Field, Payload::Name(method)) if interner.resolve(method) == expected
    )
}

fn dependency_has_source_call(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceCallKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    let kind = EvidenceKind::Source(SourceFactKind::Call(expected));
    matches!(
        unique_evidence_at(
            il,
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
                _ => None,
            },
        ),
        EvidenceResolution::Found(call) if call == expected
    ) && dependency_has_asserted_record(il, record, anchor, kind)
}

fn dependency_has_source_fact_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: SourceFactKind,
) -> bool {
    dependency_has_source_fact_anchor(il, record, il.node(node).span, expected)
}

fn dependency_has_source_fact_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceFactKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    matches!(
        unique_evidence_at(
            il,
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(fact) => Some(fact),
                _ => None,
            },
        ),
        EvidenceResolution::Found(fact) if fact == expected
    ) && dependency_has_asserted_record(il, record, anchor, EvidenceKind::Source(expected))
}

fn dependency_has_required_module_before(
    record: &EvidenceRecord,
    il: &Il,
    interner: &Interner,
    module: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Require {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|id| {
        il.evidence.get(id.0 as usize).is_some_and(|dependency| {
            dependency.id == *id
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == expected
                && require_dependency_is_before_call(dependency, call_span)
                && require_dependency_has_unshadowed_require(il, interner, dependency)
        })
    })
}

fn require_dependency_is_before_call(require_record: &EvidenceRecord, call_span: Span) -> bool {
    matches!(
        require_record.anchor,
        EvidenceAnchor::SourceSpan(span)
            if span.file == call_span.file && span.end_byte <= call_span.start_byte
    )
}

fn require_dependency_has_unshadowed_require(
    il: &Il,
    interner: &Interner,
    require_record: &EvidenceRecord,
) -> bool {
    let require_span = match require_record.anchor {
        EvidenceAnchor::SourceSpan(span) => span,
        _ => return false,
    };
    require_record.dependencies.iter().any(|id| {
        let Some(dependency) = il.evidence.get(id.0 as usize) else {
            return false;
        };
        let expected = SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        dependency.id == *id
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && span.file == require_span.file
            && span.start_byte >= require_span.start_byte
            && span.end_byte <= require_span.end_byte
            && !file_defines_name_visible_at(il, interner, "require", span)
            && matches!(
                symbol_evidence_at_node_anchor(il, span, NodeKind::Var),
                EvidenceResolution::Found(actual) if actual == expected
            )
    })
}

fn dependency_has_unshadowed_global_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: &str,
) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    dependency_has_unshadowed_global_anchor(il, record, span, kind, expected)
}

fn dependency_has_unshadowed_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let expected_kind = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(expected),
    };
    if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected_kind
    ) {
        return false;
    }
    dependency_has_asserted_record(
        il,
        record,
        EvidenceAnchor::node(span, kind),
        EvidenceKind::Symbol(expected_kind),
    )
}

fn dependency_has_qualified_global_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: &str,
) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    dependency_has_qualified_global_anchor(il, record, span, kind, expected)
}

fn dependency_has_qualified_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let expected_kind = SymbolEvidenceKind::QualifiedGlobal {
        path_hash: stable_symbol_hash(expected),
    };
    if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected_kind
    ) {
        return false;
    }
    dependency_has_asserted_record(
        il,
        record,
        EvidenceAnchor::node(span, kind),
        EvidenceKind::Symbol(expected_kind),
    )
}

fn dependency_has_imported_member_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    match il.kind(node) {
        NodeKind::Var => {
            dependency_has_imported_binding_node(il, interner, record, node, module, exported)
        }
        NodeKind::Field => {
            let Some(receiver) = il.children(node).first().copied() else {
                return false;
            };
            dependency_has_imported_namespace_node(il, interner, record, receiver, module)
        }
        _ => false,
    }
}

fn dependency_has_imported_binding_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    dependency_has_imported_binding_anchor(
        il,
        interner,
        record,
        il.node(node).span,
        il.kind(node),
        module,
        exported,
    )
}

fn dependency_has_imported_binding_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

fn dependency_has_imported_namespace_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
) -> bool {
    dependency_has_imported_namespace_anchor(
        il,
        interner,
        record,
        il.node(node).span,
        il.kind(node),
        module,
    )
}

fn dependency_has_imported_namespace_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

fn dependency_has_imported_binding_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

fn dependency_has_imported_namespace_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

fn dependency_has_imported_symbol_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && matches!(
                dependency.anchor,
                EvidenceAnchor::Node {
                    kind: NodeKind::Var,
                    ..
                }
            )
            && imported_occurrence_symbol_dependencies_valid(il, interner, dependency, expected)
    })
}

fn dependency_has_imported_symbol_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: SymbolEvidenceKind,
) -> bool {
    if kind != NodeKind::Var {
        return false;
    }
    if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected
    ) {
        return false;
    }
    let Some(symbol_record) = record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.anchor == EvidenceAnchor::node(span, kind)
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected))
        .then_some(dependency)
    }) else {
        return false;
    };
    imported_occurrence_symbol_dependencies_valid(il, interner, symbol_record, expected)
}

fn imported_occurrence_symbol_dependencies_valid(
    il: &Il,
    interner: &Interner,
    symbol_record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    let EvidenceAnchor::Node {
        span: occurrence_span,
        kind: NodeKind::Var,
    } = symbol_record.anchor
    else {
        return false;
    };
    let Some(binding_record) = symbol_record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && matches!(dependency.anchor, EvidenceAnchor::Binding { .. }))
        .then_some(dependency)
    }) else {
        return false;
    };
    let EvidenceAnchor::Binding {
        span: binding_span,
        local_hash,
    } = binding_record.anchor
    else {
        return false;
    };
    if unit_defines_hash_visible_at(il, interner, local_hash, occurrence_span) {
        return false;
    }
    if !matches!(
        binding_identity_matches(il, local_hash, binding_span, expected),
        EvidenceResolution::Found(true)
    ) {
        return false;
    }
    if !binding_has_no_visible_conflicting_assignment(il, interner, local_hash, binding_span) {
        return false;
    }
    if !binding_has_no_visible_local_shadow(il, interner, local_hash, binding_span, occurrence_span)
    {
        return false;
    }
    binding_symbol_evidence_consistent_for_local(il, local_hash, expected)
}

fn binding_has_no_visible_conflicting_assignment(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
) -> bool {
    top_level_statements(il)
        .into_iter()
        .filter(|&stmt| assignment_alias_hash(il, interner, stmt) == Some(local_hash))
        .all(|stmt| il.node(stmt).span == binding_span)
}

fn binding_has_no_visible_local_shadow(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
    occurrence_span: Span,
) -> bool {
    let Some(function_span) = innermost_enclosing_function_span(il, occurrence_span) else {
        return true;
    };
    let occurrence_cid = var_cid_at_span(il, occurrence_span);
    !il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        if !span_contains(function_span, node.span)
            || node.span == binding_span
            || node.span.start_byte > occurrence_span.start_byte
            || innermost_enclosing_function_span(il, node.span) != Some(function_span)
        {
            return false;
        }
        match node.kind {
            NodeKind::Param => node_cid(il, node_id)
                .zip(occurrence_cid)
                .is_some_and(|(param_cid, occurrence_cid)| param_cid == occurrence_cid),
            NodeKind::Assign => {
                assignment_lhs_cid(il, node_id)
                    .zip(occurrence_cid)
                    .is_some_and(|(lhs_cid, occurrence_cid)| lhs_cid == occurrence_cid)
                    || assignment_lhs_raw_name_hash(il, interner, node_id) == Some(local_hash)
            }
            _ => false,
        }
    })
}

fn innermost_enclosing_function_span(il: &Il, span: Span) -> Option<Span> {
    il.nodes
        .iter()
        .filter_map(|node| {
            (node.kind == NodeKind::Func && span_contains(node.span, span)).then_some(node.span)
        })
        .min_by_key(|span| span.end_byte.saturating_sub(span.start_byte))
}

fn span_contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file
        && outer.start_byte <= inner.start_byte
        && inner.end_byte <= outer.end_byte
}

fn var_cid_at_span(il: &Il, span: Span) -> Option<u32> {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == NodeKind::Var && node.span == span).then_some(NodeId(idx as u32))
        })
        .and_then(|node| node_cid(il, node))
}

fn node_cid(il: &Il, node: NodeId) -> Option<u32> {
    match il.node(node).payload {
        Payload::Cid(cid) => Some(cid),
        _ => None,
    }
}

fn assignment_lhs_cid(il: &Il, stmt: NodeId) -> Option<u32> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    (il.kind(lhs) == NodeKind::Var)
        .then(|| node_cid(il, lhs))
        .flatten()
}

fn assignment_lhs_raw_name_hash(il: &Il, interner: &Interner, stmt: NodeId) -> Option<u64> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    match il.node(lhs).payload {
        Payload::Name(symbol) => Some(stable_symbol_hash(interner.resolve(symbol))),
        _ => None,
    }
}

fn binding_symbol_evidence_consistent_for_local(
    il: &Il,
    local_hash: u64,
    expected: SymbolEvidenceKind,
) -> bool {
    let mut saw_symbol = false;
    for record in &il.evidence {
        let EvidenceAnchor::Binding {
            local_hash: anchor_hash,
            ..
        } = record.anchor
        else {
            continue;
        };
        if anchor_hash != local_hash {
            continue;
        }
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || symbol != expected {
            return false;
        }
        saw_symbol = true;
    }
    saw_symbol
}

fn dependency_has_asserted_record(
    il: &Il,
    record: &EvidenceRecord,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == kind
        })
    })
}

pub fn library_free_name_collection_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Python => LibraryApiContractId::PythonBuiltinCollectionFactory,
                Lang::Rust => LibraryApiContractId::RustStdCollectionFactory,
                _ => return None,
            };
            Some(LibraryCollectionFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, row.shadow_guard),
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        })
}

pub fn library_free_name_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_collection_factory_contract(lang, name))
        })
}

pub fn library_free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryFreeFunctionBuiltinContract> {
    let result = free_function_builtin_contract(lang, name, arg_count)?;
    Some(LibraryFreeFunctionBuiltinContract {
        id: LibraryApiContractId::FreeFunctionBuiltin(result.builtin),
        callee: LibraryApiCalleeContract::FreeName {
            name: result.name,
            shadow: library_free_name_shadow_policy(lang, result.requires_unshadowed),
        },
        result,
    })
}

pub fn library_imported_collection_factory_contract(
    lang: Lang,
    module: &str,
    exported: &str,
) -> Option<LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .find(|row| {
            row.lang.is_none_or(|row_lang| row_lang == lang)
                && row.module == module
                && row.exported == exported
        })
        .map(|row| LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: row.module,
                exported: row.exported,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
}

pub fn library_imported_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .filter_map(move |row| {
            library_imported_collection_factory_contract(lang, row.module, row.exported)
        })
}

pub fn library_free_name_map_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Rust => LibraryApiContractId::RustStdMapFactory,
                _ => return None,
            };
            Some(LibraryMapFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, false),
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: row.entry_seq_tag,
                },
            })
        })
}

pub fn library_free_name_map_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_map_factory_contract(lang, name))
        })
}

pub fn library_java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_factory_contract(lang, receiver, method)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: contract.single_arg_spreads_array,
        },
    })
}

pub fn library_java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_constructor_contract(lang, type_name, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionConstructor(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type: contract.simple_type,
            qualified_type: contract.qualified_type,
            module: contract.module,
            requires_import_for_simple_type: contract.requires_import_for_simple_type,
            requires_no_local_type_shadow: contract.requires_no_local_type_shadow,
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = java_map_factory_contract(lang, receiver, method)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JavaMapFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryMapFactoryResult::JavaFactory {
            kind: contract.kind,
        },
    })
}

pub fn library_java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_map_entry_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapEntryFactoryContract> {
    java_map_entry_contract(lang, receiver, method).then_some(LibraryMapEntryFactoryContract {
        id: LibraryApiContractId::JavaMapEntryFactory,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: "Map",
            method: "entry",
        },
    })
}

pub fn library_java_map_entry_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapEntryFactoryContract> {
    (method_hash == stable_symbol_hash("entry"))
        .then(|| library_java_map_entry_contract(lang, receiver, "entry"))
        .flatten()
}

pub fn library_ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = ruby_set_factory_contract(lang, receiver, method, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RubySetFactory,
        callee: LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver: contract.receiver,
            method: contract.method,
            required_module: contract.required_module,
            shadow_root: contract.shadow_root,
        },
        result: LibraryCollectionFactoryResult::SequenceArgument,
    })
}

pub fn library_ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| library_ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

pub fn library_js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = js_like_set_constructor_contract(lang, receiver)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JsLikeSetConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument,
    })
}

pub fn library_js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = js_like_map_constructor_contract(lang, receiver)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JsLikeMapConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryMapFactoryResult::EntrySequence {
            entry_seq_tag: contract.entry_seq_tag?,
        },
    })
}

pub fn library_rust_vec_macro_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    (lang == Lang::Rust && name == "vec").then_some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecMacroFactory,
        callee: LibraryApiCalleeContract::RustMacro {
            name: "vec",
            shadow: LibraryApiShadowPolicy::SameName,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: false,
        },
    })
}

pub fn library_rust_vec_new_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = rust_vec_new_factory_contract(lang, name)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecNewFactory,
        callee: LibraryApiCalleeContract::FreeName {
            name: match name {
                "Vec::new" => "Vec::new",
                "std::vec::Vec::new" => "std::vec::Vec::new",
                "alloc::vec::Vec::new" => "alloc::vec::Vec::new",
                _ => return None,
            },
            shadow: LibraryApiShadowPolicy::ExplicitRoot(contract.shadow_root),
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    if arg_count != 0 {
        return None;
    }
    let result = match (lang, method) {
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
    };
    Some(LibraryMapKeyViewContract {
        id: LibraryApiContractId::MapKeyView(result.kind),
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: MethodReceiverContract::ExactMap,
        },
        result,
    })
}

pub fn library_map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

pub fn library_map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "from" || arg_count != 1 {
        return None;
    }
    let result = MapKeyViewWrapperContract {
        receiver: "Array",
        method: "from",
        qualified_path: "Array.from",
    };
    Some(LibraryMapKeyViewWrapperContract {
        id: LibraryApiContractId::MapKeyViewWrapper,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: true,
        },
        result,
    })
}

pub fn library_map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| library_map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

pub fn library_map_get_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    if !matches!(
        lang,
        Lang::Java
            | Lang::Rust
            | Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
    ) || method != "get"
        || arg_count != 1
    {
        return None;
    }
    let result = MapGetContract {
        method: "get",
        receiver: MethodReceiverContract::ExactMap,
    };
    Some(LibraryMapGetContract {
        id: LibraryApiContractId::MapGet,
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| library_map_get_contract(lang, "get", arg_count))
        .flatten()
}

pub fn library_js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalMethodContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "isArray" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalMethodContract {
        receiver: "Array",
        method: "isArray",
        qualified_path: "Array.isArray",
        requires_unshadowed_receiver: true,
    };
    Some(LibraryStaticGlobalMethodContract {
        id: LibraryApiContractId::JsArrayIsArray,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: result.requires_unshadowed_receiver,
        },
        result,
    })
}

pub fn library_js_boolean_coercion_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalFunctionContract> {
    if !js_like_lang(lang) || function != "Boolean" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalFunctionContract {
        function: "Boolean",
        requires_unshadowed_function: true,
    };
    Some(LibraryStaticGlobalFunctionContract {
        id: LibraryApiContractId::JsBooleanCoercion,
        callee: LibraryApiCalleeContract::StaticGlobalFunction {
            function: result.function,
            requires_unshadowed_function: result.requires_unshadowed_function,
        },
        result,
    })
}

pub fn library_regex_test_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryRegexTestContract> {
    if !js_like_lang(lang) || method != "test" || arg_count != 1 {
        return None;
    }
    let result = RegexTestContract {
        method: "test",
        required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
    };
    Some(LibraryRegexTestContract {
        id: LibraryApiContractId::RegexTest,
        callee: LibraryApiCalleeContract::RegexLiteralMethod {
            method: result.method,
            required_receiver_fact: result.required_receiver_fact,
        },
        result,
    })
}

pub fn library_static_index_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticIndexMembershipContract> {
    let result = static_index_membership_contract(lang, method, arg_count)?;
    Some(LibraryStaticIndexMembershipContract {
        id: LibraryApiContractId::JsLikeStaticIndexMembership(result.kind),
        callee: LibraryApiCalleeContract::StaticIndexMembershipMethod {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_imported_namespace_function_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryImportedNamespaceFunctionContract> {
    let result = match (lang, function, arg_count) {
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
    };
    Some(LibraryImportedNamespaceFunctionContract {
        id: LibraryApiContractId::ImportedNamespaceFunction(result.semantic),
        callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
            module: result.module,
            function: result.function,
        },
        result,
    })
}

pub fn library_promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryPromiseThenContract> {
    if !js_like_lang(lang) || method != "then" || arg_count != 1 {
        return None;
    }
    let result = PromiseThenContract {
        receiver: AsyncReceiverContract::ExactPromiseLike,
    };
    Some(LibraryPromiseThenContract {
        id: LibraryApiContractId::PromiseThen,
        callee: LibraryApiCalleeContract::AsyncMethod {
            method: "then",
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryIteratorIdentityAdapterContract> {
    let method = if lang == Lang::Rust && arg_count == 0 {
        match method {
            "iter" => "iter",
            "into_iter" => "into_iter",
            "iter_mut" => "iter_mut",
            "collect" => "collect",
            "to_vec" => "to_vec",
            "copied" => "copied",
            "cloned" => "cloned",
            _ => return None,
        }
    } else if lang == Lang::Java && method == "stream" && arg_count == 0 {
        "stream"
    } else {
        return None;
    };
    let result = IteratorIdentityAdapterContract {
        receiver: IteratorAdapterReceiverContract::ExactIterableValue,
    };
    Some(LibraryIteratorIdentityAdapterContract {
        id: LibraryApiContractId::IteratorIdentityAdapter,
        callee: LibraryApiCalleeContract::IteratorAdapterMethod {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_static_collection_adapter_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticCollectionAdapterContract> {
    if lang != Lang::Java || receiver != "Arrays" || method != "stream" || arg_count != 1 {
        return None;
    }
    let result = StaticCollectionAdapterContract {
        module: "java.util",
        exported: "Arrays",
    };
    Some(LibraryStaticCollectionAdapterContract {
        id: LibraryApiContractId::StaticCollectionAdapter,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: result.exported,
            method: "stream",
        },
        result,
    })
}

pub fn library_method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryMethodCallContract> {
    let result = method_call_contract_shape(lang, name, arg_count)?;
    let method = library_method_selector_name(name)?;
    Some(LibraryMethodCallContract {
        id: LibraryApiContractId::MethodCall(result.semantic),
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_receiver_method_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_get_contract(lang, method, arg_count)
        .map(|contract| LibraryReceiverMethodApiContract {
            id: contract.id,
            callee: contract.callee,
            rule: "library_api_map_get",
        })
        .or_else(|| {
            library_map_key_view_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_map_key_view",
                }
            })
        })
        .or_else(|| {
            library_iterator_identity_adapter_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_iterator_identity_adapter",
                }
            })
        })
        .or_else(|| {
            library_scalar_integer_method_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_scalar_integer_method",
                }
            })
        })
        .or_else(|| {
            library_rust_option_and_then_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_rust_option_and_then",
                }
            })
        })
        .or_else(|| {
            library_method_call_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_method_call",
                }
            })
        })
}

fn library_method_selector_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "__contains__" => "__contains__",
        "Abs" => "Abs",
        "Contains" => "Contains",
        "HasPrefix" => "HasPrefix",
        "HasSuffix" => "HasSuffix",
        "Max" => "Max",
        "Min" => "Min",
        "Print" => "Print",
        "Printf" => "Printf",
        "Println" => "Println",
        "abs" => "abs",
        "all" => "all",
        "all?" => "all?",
        "allMatch" => "allMatch",
        "any" => "any",
        "any?" => "any?",
        "anyMatch" => "anyMatch",
        "and_then" => "and_then",
        "append" => "append",
        "clamp" => "clamp",
        "collect" => "collect",
        "contains" => "contains",
        "containsKey" => "containsKey",
        "contains_key" => "contains_key",
        "count" => "count",
        "debug" => "debug",
        "empty?" => "empty?",
        "end_with?" => "end_with?",
        "endsWith" => "endsWith",
        "ends_with" => "ends_with",
        "endswith" => "endswith",
        "every" => "every",
        "fetch" => "fetch",
        "filter" => "filter",
        "filter_map" => "filter_map",
        "flatMap" => "flatMap",
        "flat_map" => "flat_map",
        "fold" => "fold",
        "get" => "get",
        "getOrDefault" => "getOrDefault",
        "has" => "has",
        "has_key?" => "has_key?",
        "include?" => "include?",
        "includes" => "includes",
        "info" => "info",
        "inject" => "inject",
        "isEmpty" => "isEmpty",
        "is_empty" => "is_empty",
        "is_none" => "is_none",
        "is_some" => "is_some",
        "join" => "join",
        "key?" => "key?",
        "len" => "len",
        "length" => "length",
        "log" => "log",
        "map" => "map",
        "map_or" => "map_or",
        "max" => "max",
        "member?" => "member?",
        "min" => "min",
        "nil?" => "nil?",
        "push" => "push",
        "reduce" => "reduce",
        "select" => "select",
        "size" => "size",
        "some" => "some",
        "start_with?" => "start_with?",
        "startsWith" => "startsWith",
        "starts_with" => "starts_with",
        "startswith" => "startswith",
        "sum" => "sum",
        "unwrap_or" => "unwrap_or",
        "unwrap_or_else" => "unwrap_or_else",
        "zip" => "zip",
        _ => return None,
    })
}

fn library_free_name_shadow_policy(lang: Lang, shadow_guard: bool) -> LibraryApiShadowPolicy {
    if shadow_guard {
        LibraryApiShadowPolicy::SameName
    } else if lang == Lang::Rust {
        LibraryApiShadowPolicy::RustStdRootForStdPath
    } else {
        LibraryApiShadowPolicy::None
    }
}

pub fn imported_literal_seq_tag_safe(lang: Lang, tag: &str) -> bool {
    seq_surface_contract(lang, Some(tag)).is_some_and(|contract| contract.imported_literal)
}

pub fn module_binding_mutating_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MethodEffectContract> {
    semantics(lang)
        .effects()
        .receiver_mutation_method_contract(method, arg_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance,
        EvidenceRecord, EvidenceStatus, FileId, FileMeta, GuardEvidenceKind, IlBuilder,
        ImportEvidenceKind, Interner, JsRecordGuardComparison, JsRecordGuardNullCheck,
        LibraryApiEvidenceKind, ParamSemantic, SequenceSurfaceKind, Span, Symbol,
        SymbolEvidenceKind, Unit, UnitKind,
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

    fn inferred_domains_for_added_literal(lit: Payload) -> Vec<ValueDomain> {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let varx = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let lit = b.add(NodeKind::Lit, lit, sp, &[]);
        let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[varx, lit]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[add]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        semantics(Lang::Python)
            .operators()
            .infer_param_value_domains(&il, func)
    }

    #[test]
    fn value_domain_inference_treats_retained_float_literals_as_numeric() {
        assert_eq!(
            inferred_domains_for_added_literal(Payload::LitInt(3)),
            vec![ValueDomain::Number]
        );
        assert_eq!(
            inferred_domains_for_added_literal(Payload::LitFloat(0xBEEF)),
            vec![ValueDomain::Number]
        );
    }

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, 1, 1)
    }

    fn span(start: u32, end: u32, line: u32) -> Span {
        Span::new(FileId(0), start, end, line, line)
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
        evidence_with_dependencies(id, anchor, kind, status, Vec::new())
    }

    fn evidence_with_dependencies(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
        dependencies: Vec<EvidenceId>,
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
            dependencies,
            status,
        }
    }

    fn push_node_effect(
        il: &mut Il,
        id: u32,
        node: NodeId,
        effect: EffectEvidenceKind,
    ) -> EvidenceId {
        push_node_effect_with_dependencies(il, id, node, effect, Vec::new())
    }

    fn push_node_effect_with_dependencies(
        il: &mut Il,
        id: u32,
        node: NodeId,
        effect: EffectEvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        let evidence_id = EvidenceId(id);
        il.evidence.push(evidence_with_dependencies(
            id,
            EvidenceAnchor::node(il.node(node).span, il.kind(node)),
            EvidenceKind::Effect(effect),
            EvidenceStatus::Asserted,
            dependencies,
        ));
        evidence_id
    }

    fn push_node_place(il: &mut Il, id: u32, node: NodeId, place: PlaceEvidenceKind) -> EvidenceId {
        push_node_place_with_dependencies(il, id, node, place, Vec::new())
    }

    fn push_node_place_with_dependencies(
        il: &mut Il,
        id: u32,
        node: NodeId,
        place: PlaceEvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        let evidence_id = EvidenceId(id);
        il.evidence.push(evidence_with_dependencies(
            id,
            EvidenceAnchor::node(il.node(node).span, il.kind(node)),
            EvidenceKind::Place(place),
            EvidenceStatus::Asserted,
            dependencies,
        ));
        evidence_id
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
        assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Array));
        assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Map));
        assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Set));
        assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Map));
        assert!(!DomainRequirement::SetOrMap.accepts(DomainEvidence::Collection));
    }

    #[test]
    fn method_receiver_contracts_expose_only_domain_backed_obligations() {
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ExactCollection),
            Some(DomainRequirement::ArrayCollectionOrSet)
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ExactProtocol),
            Some(DomainRequirement::ArrayCollectionOrSet)
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ExactCollectionOrMap),
            Some(DomainRequirement::CollectionOrMap)
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ExactSetOrMap),
            Some(DomainRequirement::SetOrMap)
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::RustMapGetOrExactOption),
            Some(DomainRequirement::Option)
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ExactMapLiteral),
            None
        );
        assert_eq!(
            method_receiver_domain_requirement(MethodReceiverContract::ImportedNamespace("math")),
            None
        );
    }

    #[test]
    fn domain_evidence_records_drive_param_domain_proof() {
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::None, sp(3), &[]);
        let root = b.add(NodeKind::Func, Payload::None, sp(3), &[param]);
        let mut il = finish_il(b, root, Lang::TypeScript);
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
    fn ambiguous_domain_evidence_stays_closed() {
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::None, sp(4), &[]);
        let root = b.add(NodeKind::Func, Payload::None, sp(4), &[param]);
        let mut il = finish_il(b, root, Lang::TypeScript);
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
    fn receiver_domain_evidence_at_node_is_preferred_over_param_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(20, 22, 2),
            &[receiver],
        );
        let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[stmt]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 30, 1),
            &[param, body],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Set),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_param(&il, param),
            Some(DomainEvidence::Set)
        );
        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            Some(DomainEvidence::Map)
        );
        assert!(receiver_satisfies_domain(
            &il,
            &interner,
            receiver,
            DomainRequirement::Map
        ));
        assert!(!receiver_satisfies_domain(
            &il,
            &interner,
            receiver,
            DomainRequirement::Set
        ));
    }

    #[test]
    fn ambiguous_receiver_domain_evidence_blocks_param_fallback() {
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
        let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 30, 1),
            &[param, body],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Set),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            2,
            EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));

        let interner = Interner::new();
        assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
    }

    fn binding_receiver_fixture(
        interner: &Interner,
        module_receiver: bool,
    ) -> (Il, NodeId, NodeId) {
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
        let rhs = b.add(NodeKind::Seq, Payload::None, span(15, 17, 1), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            span(10, 17, 1),
            &[lhs, rhs],
        );
        let receiver_payload = if module_receiver {
            Payload::Name(xs)
        } else {
            Payload::Cid(0)
        };
        let receiver = b.add(NodeKind::Var, receiver_payload, span(40, 42, 3), &[]);
        let root = if module_receiver {
            let stmt = b.add(
                NodeKind::ExprStmt,
                Payload::None,
                span(40, 42, 3),
                &[receiver],
            );
            let body = b.add(NodeKind::Block, Payload::None, span(38, 45, 3), &[stmt]);
            let func = b.add(NodeKind::Func, Payload::None, span(30, 50, 2), &[body]);
            b.add(
                NodeKind::Module,
                Payload::None,
                span(0, 60, 1),
                &[assign, func],
            )
        } else {
            let body = b.add(
                NodeKind::Block,
                Payload::None,
                span(10, 44, 1),
                &[assign, receiver],
            );
            b.add(NodeKind::Func, Payload::None, span(0, 50, 1), &[body])
        };
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.cid_names = vec![xs];
        (il, lhs, receiver)
    }

    #[test]
    fn binding_domain_evidence_drives_receiver_domain_proof() {
        let interner = Interner::new();
        let (mut il, lhs, receiver) = binding_receiver_fixture(&interner, false);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_binding_lhs(&il, &interner, lhs),
            Some(DomainEvidence::Collection)
        );
        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            Some(DomainEvidence::Collection)
        );

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            None,
            "conflicting binding-domain evidence must close receiver proof"
        );
    }

    #[test]
    fn binding_domain_evidence_validates_dependencies() {
        let interner = Interner::new();
        let (mut il, _, receiver) = binding_receiver_fixture(&interner, false);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(span(15, 17, 1)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            EvidenceStatus::Ambiguous,
        ));
        il.evidence.push(evidence_with_dependencies(
            1,
            EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
            vec![EvidenceId(0)],
        ));

        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            None,
            "dependency-broken binding-domain evidence must fail closed"
        );
    }

    #[test]
    fn module_binding_domain_evidence_reaches_free_name_receiver() {
        let interner = Interner::new();
        let (mut il, _, receiver) = binding_receiver_fixture(&interner, true);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            Some(DomainEvidence::Collection)
        );
    }

    #[test]
    fn binding_domain_evidence_requires_matching_local_hash() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let ys = interner.intern("ys");
        let mut b = IlBuilder::new(FileId(0));
        let xs_lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
        let xs_rhs = b.add(NodeKind::Seq, Payload::None, span(14, 15, 1), &[]);
        let xs_assign = b.add(
            NodeKind::Assign,
            Payload::None,
            span(10, 15, 1),
            &[xs_lhs, xs_rhs],
        );
        let ys_lhs = b.add(NodeKind::Var, Payload::Cid(1), span(10, 12, 1), &[]);
        let ys_rhs = b.add(NodeKind::Seq, Payload::None, span(18, 19, 1), &[]);
        let ys_assign = b.add(
            NodeKind::Assign,
            Payload::None,
            span(16, 19, 1),
            &[ys_lhs, ys_rhs],
        );
        let ys_receiver = b.add(NodeKind::Var, Payload::Cid(1), span(30, 32, 2), &[]);
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(8, 34, 1),
            &[xs_assign, ys_assign, ys_receiver],
        );
        let root = b.add(NodeKind::Func, Payload::None, span(0, 40, 1), &[body]);
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.cid_names = vec![xs, ys];
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_binding_lhs(&il, &interner, xs_lhs),
            Some(DomainEvidence::Collection)
        );
        assert_eq!(
            domain_evidence_for_binding_lhs(&il, &interner, ys_lhs),
            None,
            "same-span binding evidence must not cross local_hash boundaries"
        );
        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, ys_receiver),
            None
        );
    }

    #[test]
    fn binding_domain_evidence_requires_assignment_before_receiver() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
        let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
        let rhs = b.add(NodeKind::Seq, Payload::None, span(24, 25, 2), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            span(20, 25, 2),
            &[lhs, rhs],
        );
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(8, 28, 1),
            &[receiver, assign],
        );
        let root = b.add(NodeKind::Func, Payload::None, span(0, 30, 1), &[body]);
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.cid_names = vec![xs];
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(span(20, 22, 2), stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_binding_lhs(&il, &interner, lhs),
            Some(DomainEvidence::Collection)
        );
        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            None,
            "binding-domain evidence must not prove use-before-assignment receivers"
        );
    }

    #[test]
    fn cid_receiver_domain_uses_nearest_function_scope() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let first_param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
        let first_body = b.add(NodeKind::Block, Payload::None, span(14, 20, 1), &[]);
        let first_func = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 30, 1),
            &[first_param, first_body],
        );
        let second_param = b.add(NodeKind::Param, Payload::Cid(0), span(50, 52, 3), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 62, 4), &[]);
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(60, 62, 4),
            &[receiver],
        );
        let second_body = b.add(NodeKind::Block, Payload::None, span(58, 66, 4), &[stmt]);
        let second_func = b.add(
            NodeKind::Func,
            Payload::None,
            span(40, 80, 3),
            &[second_param, second_body],
        );
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            span(0, 90, 1),
            &[first_func, second_func],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::param(span(50, 52, 3)),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            Some(DomainEvidence::Map)
        );
    }

    #[test]
    fn dependency_broken_receiver_domain_evidence_blocks_param_fallback() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
        let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 30, 1),
            &[param, body],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Set),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            1,
            EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
            vec![EvidenceId(99)],
        ));

        assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
    }

    #[test]
    fn named_receiver_domain_requires_unassigned_param_scope() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
        let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(40, 42, 3),
            &[receiver],
        );
        let body = b.add(NodeKind::Block, Payload::None, span(20, 50, 2), &[stmt]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 60, 1),
            &[param, body],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            domain_evidence_for_receiver(&il, &interner, receiver),
            Some(DomainEvidence::Collection)
        );

        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
        let lhs = b.add(NodeKind::Var, Payload::Name(xs), span(24, 26, 2), &[]);
        let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), span(29, 30, 2), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            span(24, 30, 2),
            &[lhs, rhs],
        );
        let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(40, 42, 3),
            &[receiver],
        );
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(20, 50, 2),
            &[assign, stmt],
        );
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            span(0, 60, 1),
            &[param, body],
        );
        let mut il = finish_il(b, root, Lang::TypeScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(span(10, 12, 1)),
            EvidenceKind::Domain(DomainEvidence::Collection),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
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

        let go_entry = seq_surface_contract(Lang::Go, Some("keyed_element")).unwrap();
        assert_eq!(go_entry.value_tag, stable_symbol_hash("keyed_element"));
        assert!(!go_entry.exact_tree_safe);
        assert!(!go_entry.membership_collection);

        assert!(seq_surface_contract(Lang::Python, Some("composite_literal")).is_none());
        assert!(seq_surface_contract(Lang::Python, Some("keyed_element")).is_none());
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

        assert_eq!(
            seq_surface_contract_for_node(&il, &interner, seq),
            None,
            "raw sequence tags do not prove semantic surfaces without evidence"
        );

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

    fn js_record_guard_il(interner: &Interner, subject: &str) -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let tag = interner.intern("record_guard");
        let subject = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern(subject)),
            sp(12),
            &[],
        );
        let object = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("object")),
            sp(12),
            &[],
        );
        let non_null = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("non_null")),
            sp(12),
            &[],
        );
        let not_array = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("not_array")),
            sp(12),
            &[],
        );
        let guard = b.add(
            NodeKind::Seq,
            Payload::Name(tag),
            sp(12),
            &[subject, object, non_null, not_array],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(12), &[guard]);
        (finish_il(b, root, Lang::JavaScript), guard)
    }

    fn record_guard_evidence_with_null_check(
        subject: &str,
        null_check: JsRecordGuardNullCheck,
    ) -> EvidenceKind {
        EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
            subject_hash: stable_symbol_hash(subject),
            null_check,
            comparison: JsRecordGuardComparison::StrictOnly,
        })
    }

    fn array_is_array_dependency(id: u32, span: Span, status: EvidenceStatus) -> EvidenceRecord {
        evidence(
            id,
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.isArray"),
            }),
            status,
        )
    }

    fn boolean_dependency(id: u32, span: Span, status: EvidenceStatus) -> EvidenceRecord {
        evidence(
            id,
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Boolean"),
            }),
            status,
        )
    }

    fn record_guard_record(
        id: u32,
        span: Span,
        subject: &str,
        null_check: JsRecordGuardNullCheck,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        evidence_with_dependencies(
            id,
            EvidenceAnchor::sequence(span),
            record_guard_evidence_with_null_check(subject, null_check),
            EvidenceStatus::Asserted,
            dependencies.iter().copied().map(EvidenceId).collect(),
        )
    }

    fn js_own_property_guard_il(interner: &Interner) -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let tag = interner.intern("own_property_guard");
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("value")),
            sp(22),
            &[],
        );
        let key = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("ready")),
            sp(22),
            &[],
        );
        let own = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("own")),
            sp(22),
            &[],
        );
        let present = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("present")),
            sp(22),
            &[],
        );
        let guard = b.add(
            NodeKind::Seq,
            Payload::Name(tag),
            sp(22),
            &[receiver, key, own, present],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(22), &[guard]);
        (finish_il(b, root, Lang::JavaScript), guard)
    }

    fn qualified_global_dependency(
        id: u32,
        span: Span,
        path: &str,
        status: EvidenceStatus,
    ) -> EvidenceRecord {
        evidence(
            id,
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash(path),
            }),
            status,
        )
    }

    fn own_property_guard_record(
        id: u32,
        span: Span,
        path: &str,
        status: EvidenceStatus,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        evidence_with_dependencies(
            id,
            EvidenceAnchor::sequence(span),
            EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
                api_path_hash: stable_symbol_hash(path),
            }),
            status,
            dependencies.iter().copied().map(EvidenceId).collect(),
        )
    }

    #[test]
    fn own_property_guard_requires_dedicated_guard_evidence() {
        let interner = Interner::new();
        let (mut il, guard) = js_own_property_guard_il(&interner);

        assert!(!own_property_guard_for_node(&il, &interner, guard));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(22)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
            EvidenceStatus::Asserted,
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));

        il.evidence.push(qualified_global_dependency(
            1,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));

        il.evidence.push(own_property_guard_record(
            2,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
            &[1],
        ));
        assert!(own_property_guard_for_node(&il, &interner, guard));
        assert!(own_property_guard_evidence_at_span(&il, sp(22)));
    }

    #[test]
    fn own_property_guard_validates_api_dependencies() {
        let interner = Interner::new();
        let (mut il, guard) = js_own_property_guard_il(&interner);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(22)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(qualified_global_dependency(
            1,
            sp(22),
            "Array.from",
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(own_property_guard_record(
            2,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
            &[1],
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_own_property_guard_il(&interner);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(22)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(qualified_global_dependency(
            1,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Ambiguous,
        ));
        il.evidence.push(own_property_guard_record(
            2,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
            &[1],
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_own_property_guard_il(&interner);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(22)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(qualified_global_dependency(
            1,
            sp(22),
            "value.hasOwnProperty",
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(own_property_guard_record(
            2,
            sp(22),
            "value.hasOwnProperty",
            EvidenceStatus::Asserted,
            &[1],
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn own_property_guard_rejects_ambiguous_guard_evidence() {
        let interner = Interner::new();
        let (mut il, guard) = js_own_property_guard_il(&interner);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(22)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::OwnPropertyGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(qualified_global_dependency(
            1,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(qualified_global_dependency(
            2,
            sp(22),
            "Object.prototype.hasOwnProperty.call",
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(own_property_guard_record(
            3,
            sp(22),
            "Object.hasOwn",
            EvidenceStatus::Asserted,
            &[1],
        ));
        il.evidence.push(own_property_guard_record(
            4,
            sp(22),
            "Object.prototype.hasOwnProperty.call",
            EvidenceStatus::Asserted,
            &[2],
        ));
        assert!(!own_property_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn go_zero_map_surface_helpers_require_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let key = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("ready")),
            sp(32),
            &[],
        );
        let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(32), &[]);
        let entry = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("keyed_element")),
            sp(32),
            &[key, value],
        );
        let map = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("composite_literal")),
            sp(31),
            &[entry],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(31), &[map]);
        let mut il = finish_il(b, root, Lang::Go);

        assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_none());
        assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(31)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoCompositeMapLiteral),
            EvidenceStatus::Asserted,
        ));
        assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_some());
        assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(sp(32)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoMapEntry),
            EvidenceStatus::Asserted,
        ));
        assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_some());
    }

    #[test]
    fn record_shape_guard_requires_dedicated_guard_evidence() {
        let interner = Interner::new();
        let (mut il, guard) = js_record_guard_il(&interner, "value");

        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(record_shape_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn record_shape_guard_validates_required_dependencies() {
        let interner = Interner::new();

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            1,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::source_span(sp(12)),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.from"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Ambiguous,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(14),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::BooleanGlobalTruthy,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        il.evidence
            .push(boolean_dependency(3, sp(12), EvidenceStatus::Asserted));
        il.evidence.push(record_guard_record(
            4,
            sp(12),
            "value",
            JsRecordGuardNullCheck::BooleanGlobalTruthy,
            &[1, 3],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence
            .push(boolean_dependency(2, sp(12), EvidenceStatus::Asserted));
        il.evidence.push(record_guard_record(
            3,
            sp(12),
            "value",
            JsRecordGuardNullCheck::BooleanGlobalTruthy,
            &[1, 2],
        ));
        assert!(record_shape_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn record_shape_guard_rejects_mismatched_or_ambiguous_evidence() {
        let interner = Interner::new();
        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "other",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            2,
            EvidenceAnchor::sequence(sp(12)),
            record_guard_evidence_with_null_check("value", JsRecordGuardNullCheck::StrictNonNull),
            EvidenceStatus::Ambiguous,
            vec![EvidenceId(1)],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));

        let (mut il, guard) = js_record_guard_il(&interner, "value");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(12),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(12),
            "value",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        il.evidence.push(record_guard_record(
            3,
            sp(12),
            "candidate",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));
        assert!(!record_shape_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn record_shape_guard_keeps_source_subject_proof_after_alpha_rename() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let tag = interner.intern("record_guard");
        let subject = b.add(NodeKind::Var, Payload::Cid(0), sp(13), &[]);
        let object = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("object")),
            sp(13),
            &[],
        );
        let non_null = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("non_null")),
            sp(13),
            &[],
        );
        let not_array = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("not_array")),
            sp(13),
            &[],
        );
        let guard = b.add(
            NodeKind::Seq,
            Payload::Name(tag),
            sp(13),
            &[subject, object, non_null, not_array],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(13), &[guard]);
        let mut il = finish_il(b, root, Lang::JavaScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(13)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::RecordGuard),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(array_is_array_dependency(
            1,
            sp(13),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(record_guard_record(
            2,
            sp(13),
            "source_name",
            JsRecordGuardNullCheck::StrictNonNull,
            &[1],
        ));

        assert!(record_shape_guard_for_node(&il, &interner, guard));
    }

    #[test]
    fn import_fact_contracts_resolve_evidence_only_binding_and_namespace_proofs() {
        let mut b = IlBuilder::new(FileId(0));
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
        let binding = b.add(NodeKind::Seq, Payload::None, sp(1), &[collections, deque]);
        let math = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("math")),
            sp(2),
            &[],
        );
        let namespace = b.add(NodeKind::Seq, Payload::None, sp(2), &[math]);
        let raw_coordinates = b.add(NodeKind::Seq, Payload::None, sp(3), &[math]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(1),
            &[binding, namespace, raw_coordinates],
        );
        let mut il = finish_il(b, root, Lang::Python);

        assert_eq!(
            import_fact_contract(ImportFactKind::Binding).channel,
            ChannelEligibility::ExactProven
        );
        assert_eq!(import_fact_evidence_rhs(&il, binding), None);
        assert_eq!(import_fact_evidence_rhs(&il, namespace), None);

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(1)),
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash("collections"),
                exported_hash: stable_symbol_hash("deque"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(sp(2)),
            EvidenceKind::Import(ImportEvidenceKind::Namespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));

        assert_eq!(
            import_fact_evidence_rhs(&il, binding),
            Some(ImportFact {
                kind: ImportFactKind::Binding,
                module_hash: stable_symbol_hash("collections"),
                exported_hash: Some(stable_symbol_hash("deque")),
            })
        );
        assert_eq!(
            import_fact_evidence_rhs(&il, namespace),
            Some(ImportFact {
                kind: ImportFactKind::Namespace,
                module_hash: stable_symbol_hash("math"),
                exported_hash: None,
            })
        );
        assert_eq!(import_fact_evidence_rhs(&il, raw_coordinates), None);
    }

    #[test]
    fn ambiguous_import_evidence_stays_closed_without_raw_seq_fallback() {
        let mut b = IlBuilder::new(FileId(0));
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
        let binding = b.add(NodeKind::Seq, Payload::None, sp(10), &[module, exported]);
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

        assert_eq!(
            import_fact_evidence_rhs(&il, binding),
            Some(ImportFact {
                kind: ImportFactKind::Namespace,
                module_hash: stable_symbol_hash("math"),
                exported_hash: None,
            })
        );

        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(sp(10)),
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash("collections"),
                exported_hash: stable_symbol_hash("deque"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(import_fact_evidence_rhs(&il, binding), None);
    }

    #[test]
    fn imported_symbol_identity_does_not_fall_back_to_raw_import_seq() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("deque");
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
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[module, exported]);
        let assignment = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
        let use_site = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(30),
            &[assignment, use_site],
        );
        let mut il = finish_il(b, root, Lang::Python);

        assert_eq!(import_fact_evidence_rhs(&il, rhs), None);
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
    fn imported_occurrence_symbol_evidence_requires_binding_dependency() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local_hash = stable_symbol_hash("m");
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

        assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));

        il.evidence.clear();
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(19), local_hash),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            1,
            EvidenceAnchor::node(sp(20), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
            vec![EvidenceId(0)],
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
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(21), &[module]);
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
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(24), &[module]);
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

    fn js_array_is_array_call_il(interner: &Interner) -> (Il, NodeId, NodeId, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let array = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Array")),
            sp(29),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("isArray")),
            sp(30),
            &[array],
        );
        let value = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("value")),
            sp(31),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(32), &[callee, value]);
        let root = b.add(NodeKind::Module, Payload::None, sp(29), &[call]);
        (finish_il(b, root, Lang::JavaScript), call, callee, array)
    }

    fn library_api_record(
        id: u32,
        span: Span,
        contract_id: LibraryApiContractId,
        callee: LibraryApiCalleeContract,
        status: EvidenceStatus,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        evidence_with_dependencies(
            id,
            EvidenceAnchor::node(span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract_id),
                callee_hash: library_api_callee_contract_hash(callee),
                arity: 1,
            }),
            status,
            dependencies.iter().copied().map(EvidenceId).collect(),
        )
    }

    fn java_list_of_import_evidence_il(
        interner: &Interner,
        import_in_root: bool,
    ) -> (Il, NodeId, NodeId, Symbol, LibraryCollectionFactoryContract) {
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("List");
        let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[]);
        let import = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
        let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp(32),
            &[receiver],
        );
        let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, arg]);
        let root = if import_in_root {
            b.add(NodeKind::Module, Payload::None, sp(29), &[import, call])
        } else {
            b.add(NodeKind::Func, Payload::None, sp(35), &[call])
        };
        let mut il = finish_il(b, root, Lang::Java);
        let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
            .expect("List.of contract");
        let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("List"),
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(30), stable_symbol_hash("List")),
            binding_symbol,
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            1,
            EvidenceAnchor::node(sp(31), NodeKind::Var),
            binding_symbol,
            EvidenceStatus::Asserted,
            vec![EvidenceId(0)],
        ));
        il.evidence.push(library_api_record(
            2,
            sp(34),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[1],
        ));
        (il, call, root, local, contract)
    }

    #[test]
    fn library_api_evidence_resolution_is_dependency_backed_and_fail_closed() {
        let interner = Interner::new();
        let (mut il, call, callee, array) = js_array_is_array_call_il(&interner);
        let contract = library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1)
            .expect("test contract");

        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Missing
        );

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(il.node(array).span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Array"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.isArray"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(library_api_record(
            2,
            il.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(il.node(call).span),
                    callee_span: Some(il.node(callee).span),
                    receiver_span: Some(il.node(array).span),
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Admitted
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(il.node(call).span),
                    callee_span: Some(sp(99)),
                    receiver_span: Some(il.node(array).span),
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Rejected
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(il.node(call).span),
                    callee_span: Some(il.node(callee).span),
                    receiver_span: Some(sp(99)),
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Rejected
        );

        let (mut missing_dep, call, _callee, _array) = js_array_is_array_call_il(&interner);
        missing_dep.evidence.push(library_api_record(
            0,
            missing_dep.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &missing_dep,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );

        let (mut ambiguous_dep, call, callee, array) = js_array_is_array_call_il(&interner);
        ambiguous_dep.evidence.push(evidence(
            0,
            EvidenceAnchor::node(ambiguous_dep.node(array).span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Array"),
            }),
            EvidenceStatus::Ambiguous,
        ));
        ambiguous_dep.evidence.push(evidence(
            1,
            EvidenceAnchor::node(ambiguous_dep.node(callee).span, NodeKind::Field),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.isArray"),
            }),
            EvidenceStatus::Asserted,
        ));
        ambiguous_dep.evidence.push(library_api_record(
            2,
            ambiguous_dep.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &ambiguous_dep,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );

        let (mut conflicting_dep, call, callee, array) = js_array_is_array_call_il(&interner);
        conflicting_dep.evidence.push(evidence(
            0,
            EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Array"),
            }),
            EvidenceStatus::Asserted,
        ));
        conflicting_dep.evidence.push(evidence(
            1,
            EvidenceAnchor::node(conflicting_dep.node(callee).span, NodeKind::Field),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash("Array.isArray"),
            }),
            EvidenceStatus::Asserted,
        ));
        conflicting_dep.evidence.push(evidence(
            2,
            EvidenceAnchor::node(conflicting_dep.node(array).span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Map"),
            }),
            EvidenceStatus::Asserted,
        ));
        conflicting_dep.evidence.push(library_api_record(
            3,
            conflicting_dep.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &conflicting_dep,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );

        let boolean = library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1).unwrap();
        il.evidence.push(library_api_record(
            3,
            il.node(call).span,
            boolean.id,
            boolean.callee,
            EvidenceStatus::Asserted,
            &[0],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );

        let (mut wrong_anchor, call, _callee, _array) = js_array_is_array_call_il(&interner);
        wrong_anchor.evidence.push(library_api_record(
            0,
            sp(99),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &wrong_anchor,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Missing
        );
    }

    #[test]
    fn library_api_evidence_resolution_accepts_import_and_source_backed_callees() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let local = interner.intern("Values");
        let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(10), &[]);
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(10), &[]);
        let import = b.add(NodeKind::Assign, Payload::None, sp(10), &[lhs, rhs]);
        let callee = b.add(NodeKind::Var, Payload::Name(local), sp(11), &[]);
        let arg = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(12),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, arg]);
        let root = b.add(NodeKind::Module, Payload::None, sp(9), &[import, call]);
        let mut il = finish_il(b, root, Lang::Python);
        let contract =
            library_imported_collection_factory_contract(Lang::Python, "collections", "deque")
                .expect("deque contract");
        let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(10), stable_symbol_hash("Values")),
            binding_symbol,
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            1,
            EvidenceAnchor::node(sp(11), NodeKind::Var),
            binding_symbol,
            EvidenceStatus::Asserted,
            vec![EvidenceId(0)],
        ));
        il.evidence.push(library_api_record(
            2,
            sp(13),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[1],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );

        let mut b = IlBuilder::new(FileId(0));
        let regex = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("/x/")),
            sp(20),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("test")),
            sp(21),
            &[regex],
        );
        let arg = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("s")),
            sp(22),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, arg]);
        let root = b.add(NodeKind::Module, Payload::None, sp(19), &[call]);
        let mut il = finish_il(b, root, Lang::JavaScript);
        let contract =
            library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex contract");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::source_span(sp(20)),
            EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(library_api_record(
            1,
            sp(23),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );
    }

    #[test]
    fn library_api_evidence_resolution_accepts_free_name_and_require_backed_callees() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("list")),
            sp(40),
            &[],
        );
        let arg = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(41),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(42), &[callee, arg]);
        let root = b.add(NodeKind::Module, Payload::None, sp(39), &[call]);
        let mut il = finish_il(b, root, Lang::Python);
        let contract = library_free_name_collection_factory_contract(Lang::Python, "list")
            .expect("Python list contract");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(40), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("list"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(library_api_record(
            1,
            sp(42),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(sp(42)),
                    callee_span: Some(sp(40)),
                    receiver_span: None,
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Admitted
        );

        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("len")),
            sp(45),
            &[],
        );
        let arg = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(46),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(47), &[callee, arg]);
        let root = b.add(NodeKind::Module, Payload::None, sp(44), &[call]);
        let mut il = finish_il(b, root, Lang::Python);
        let contract = library_free_function_builtin_contract(Lang::Python, "len", 1)
            .expect("Python len contract");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(45), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("len"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(library_api_record(
            1,
            sp(47),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );

        let mut b = IlBuilder::new(FileId(0));
        let require_callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("require")),
            sp(48),
            &[],
        );
        let require_arg = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("set")),
            sp(48),
            &[],
        );
        let require_call = b.add(
            NodeKind::Call,
            Payload::None,
            sp(48),
            &[require_callee, require_arg],
        );
        let set = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Set")),
            sp(50),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("new")),
            sp(51),
            &[set],
        );
        let arg = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(52),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, arg]);
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            sp(49),
            &[require_call, call],
        );
        let mut il = finish_il(b, root, Lang::Ruby);
        let contract =
            library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).expect("Set.new");
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(50), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Set"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(48), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("require"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence_with_dependencies(
            2,
            EvidenceAnchor::source_span(sp(48)),
            EvidenceKind::Import(ImportEvidenceKind::Require {
                module_hash: stable_symbol_hash("set"),
            }),
            EvidenceStatus::Asserted,
            vec![EvidenceId(1)],
        ));
        il.evidence.push(library_api_record(
            3,
            sp(53),
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0, 2],
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );
    }

    #[test]
    fn library_api_evidence_resolution_accepts_import_binding_outside_unit_root() {
        let interner = Interner::new();
        let (mut il, call, _root, _local, contract) =
            java_list_of_import_evidence_il(&interner, false);
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(sp(34)),
                    callee_span: Some(sp(32)),
                    receiver_span: Some(sp(30)),
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Rejected
        );
        assert_eq!(
            library_api_contract_evidence_at_call_span(
                &il,
                &interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: Some(sp(34)),
                    callee_span: Some(sp(32)),
                    receiver_span: None,
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ),
            LibraryApiEvidenceStatus::Admitted
        );

        il.evidence.push(evidence(
            3,
            EvidenceAnchor::binding(sp(36), stable_symbol_hash("List")),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("other.module"),
                exported_hash: stable_symbol_hash("List"),
            }),
            EvidenceStatus::Asserted,
        ));
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );
    }

    #[test]
    fn library_api_evidence_resolution_rejects_shadowed_java_static_members() {
        let interner = Interner::new();
        let (mut il, call, root, local, contract) =
            java_list_of_import_evidence_il(&interner, true);
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Admitted
        );

        il.units.push(Unit {
            root,
            kind: UnitKind::Class,
            name: Some(local),
        });
        assert_eq!(
            library_api_contract_evidence_for_call(
                &il,
                &interner,
                call,
                contract.id,
                contract.callee,
                1,
            ),
            LibraryApiEvidenceStatus::Rejected
        );
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
    fn library_api_contracts_carry_identity_and_result_obligations() {
        assert_eq!(
            library_free_name_collection_factory_contract(Lang::Python, "list"),
            Some(LibraryCollectionFactoryContract {
                id: LibraryApiContractId::PythonBuiltinCollectionFactory,
                callee: LibraryApiCalleeContract::FreeName {
                    name: "list",
                    shadow: LibraryApiShadowPolicy::SameName,
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        );
        assert_eq!(
            library_free_function_builtin_contract(Lang::Python, "len", 1),
            Some(LibraryFreeFunctionBuiltinContract {
                id: LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
                callee: LibraryApiCalleeContract::FreeName {
                    name: "len",
                    shadow: LibraryApiShadowPolicy::SameName,
                },
                result: FreeFunctionBuiltinContract {
                    name: "len",
                    builtin: Builtin::Len,
                    args: BuiltinArgContract::First,
                    requires_unshadowed: true,
                },
            })
        );
        assert_eq!(
            library_imported_collection_factory_contract(Lang::Python, "collections", "deque"),
            Some(LibraryCollectionFactoryContract {
                id: LibraryApiContractId::PythonImportedCollectionFactory,
                callee: LibraryApiCalleeContract::ImportedBinding {
                    module: "collections",
                    exported: "deque",
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        );
        assert_eq!(
            library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from"),
            Some(LibraryMapFactoryContract {
                id: LibraryApiContractId::RustStdMapFactory,
                callee: LibraryApiCalleeContract::FreeName {
                    name: "std::collections::HashMap::from",
                    shadow: LibraryApiShadowPolicy::RustStdRootForStdPath,
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_TUPLE,
                },
            })
        );
        assert!(!library_api_free_name_shadow_safe(
            Lang::Rust,
            "std::collections::HashMap::from",
            LibraryApiShadowPolicy::RustStdRootForStdPath,
            |name| name == "std"
        ));
        assert!(library_api_free_name_shadow_safe(
            Lang::Rust,
            "std::collections::HashMap::from",
            LibraryApiShadowPolicy::RustStdRootForStdPath,
            |_| false
        ));
        assert_eq!(
            library_java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
            Some(LibraryCollectionFactoryContract {
                id: LibraryApiContractId::JavaCollectionFactory(
                    JavaCollectionFactoryKind::ArraysAsList,
                ),
                callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                    receiver: "Arrays",
                    method: "asList",
                },
                result: LibraryCollectionFactoryResult::VariadicElements {
                    single_arg_spreads_array: true,
                },
            })
        );
        assert_eq!(
            library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0),
            Some(LibraryCollectionFactoryContract {
                id: LibraryApiContractId::JavaCollectionConstructor(
                    JavaCollectionConstructorKind::EmptyList,
                ),
                callee: LibraryApiCalleeContract::JavaUtilConstructor {
                    simple_type: "ArrayList",
                    qualified_type: "java.util.ArrayList",
                    module: "java.util",
                    requires_import_for_simple_type: true,
                    requires_no_local_type_shadow: true,
                },
                result: LibraryCollectionFactoryResult::EmptySequence,
            })
        );
        assert_eq!(
            library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
            Some(LibraryCollectionFactoryContract {
                id: LibraryApiContractId::RubySetFactory,
                callee: LibraryApiCalleeContract::RubyRequireStaticMember {
                    receiver: "Set",
                    method: "new",
                    required_module: "set",
                    shadow_root: "Set",
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        );
        assert_eq!(
            library_js_like_map_constructor_contract(Lang::TypeScript, "Map"),
            Some(LibraryMapFactoryContract {
                id: LibraryApiContractId::JsLikeMapConstructor,
                callee: LibraryApiCalleeContract::JsGlobalConstructor {
                    receiver: "Map",
                    requires_unshadowed_global: true,
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_COLLECTION,
                },
            })
        );
        assert_eq!(
            library_free_name_collection_factory_contract(Lang::JavaScript, "list"),
            None
        );
        assert_eq!(
            library_java_map_factory_contract(Lang::Java, "List", "of"),
            None
        );
    }

    #[test]
    fn library_api_result_domain_mapping_is_contract_scoped() {
        assert_eq!(
            library_collection_factory_result_domain(
                library_free_name_collection_factory_contract(Lang::Python, "list").unwrap()
            ),
            DomainEvidence::Collection
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_free_name_collection_factory_contract(Lang::Python, "set").unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_free_name_collection_factory_contract(Lang::Python, "frozenset").unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_imported_collection_factory_contract(Lang::Python, "collections", "deque",)
                    .unwrap()
            ),
            DomainEvidence::Collection
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_free_name_collection_factory_contract(
                    Lang::Rust,
                    "std::collections::HashSet::from",
                )
                .unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_free_name_collection_factory_contract(
                    Lang::Rust,
                    "std::collections::VecDeque::from",
                )
                .unwrap()
            ),
            DomainEvidence::Collection
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_rust_vec_macro_factory_contract(Lang::Rust, "vec").unwrap()
            ),
            DomainEvidence::Collection
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_java_collection_factory_contract(Lang::Java, "List", "of").unwrap()
            ),
            DomainEvidence::Collection
        );
        let as_list =
            library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
        assert_eq!(
            library_collection_factory_result_domain_for_arity(as_list, 0),
            Some(DomainEvidence::Collection)
        );
        assert_eq!(
            library_collection_factory_result_domain_for_arity(as_list, 1),
            None,
            "single-argument Arrays.asList has ambiguous element provenance"
        );
        assert_eq!(
            library_collection_factory_result_domain_for_arity(as_list, 2),
            Some(DomainEvidence::Collection)
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_java_collection_factory_contract(Lang::Java, "Set", "of").unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_collection_factory_result_domain(
                library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap()
            ),
            DomainEvidence::Set
        );
        assert_eq!(
            library_map_factory_result_domain(
                library_free_name_map_factory_contract(
                    Lang::Rust,
                    "std::collections::HashMap::from",
                )
                .unwrap()
            ),
            DomainEvidence::Map
        );
        assert_eq!(
            library_map_factory_result_domain(
                library_java_map_factory_contract(Lang::Java, "Map", "of").unwrap()
            ),
            DomainEvidence::Map
        );
        assert_eq!(
            library_map_factory_result_domain(
                library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap()
            ),
            DomainEvidence::Map
        );
        assert_eq!(
            library_map_key_view_wrapper_result_domain(
                library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1)
                    .unwrap()
            ),
            DomainEvidence::Array
        );
    }

    #[test]
    fn library_non_factory_api_contracts_carry_identity_and_result_obligations() {
        assert_eq!(
            library_map_key_view_contract(Lang::TypeScript, "keys", 0),
            Some(LibraryMapKeyViewContract {
                id: LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator),
                callee: LibraryApiCalleeContract::Method {
                    method: "keys",
                    receiver: MethodReceiverContract::ExactMap,
                },
                result: MapKeyViewContract {
                    method: "keys",
                    kind: MapKeyViewKind::Iterator,
                },
            })
        );
        assert_eq!(
            library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
            Some(LibraryMapKeyViewWrapperContract {
                id: LibraryApiContractId::MapKeyViewWrapper,
                callee: LibraryApiCalleeContract::StaticGlobalMethod {
                    receiver: "Array",
                    method: "from",
                    qualified_path: "Array.from",
                    requires_unshadowed_receiver: true,
                },
                result: MapKeyViewWrapperContract {
                    receiver: "Array",
                    method: "from",
                    qualified_path: "Array.from",
                },
            })
        );
        assert_eq!(
            library_map_get_contract(Lang::Rust, "get", 1),
            Some(LibraryMapGetContract {
                id: LibraryApiContractId::MapGet,
                callee: LibraryApiCalleeContract::Method {
                    method: "get",
                    receiver: MethodReceiverContract::ExactMap,
                },
                result: MapGetContract {
                    method: "get",
                    receiver: MethodReceiverContract::ExactMap,
                },
            })
        );
        assert_eq!(
            library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
            Some(LibraryStaticGlobalMethodContract {
                id: LibraryApiContractId::JsArrayIsArray,
                callee: LibraryApiCalleeContract::StaticGlobalMethod {
                    receiver: "Array",
                    method: "isArray",
                    qualified_path: "Array.isArray",
                    requires_unshadowed_receiver: true,
                },
                result: StaticGlobalMethodContract {
                    receiver: "Array",
                    method: "isArray",
                    qualified_path: "Array.isArray",
                    requires_unshadowed_receiver: true,
                },
            })
        );
        assert_eq!(
            library_js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
            Some(LibraryStaticGlobalFunctionContract {
                id: LibraryApiContractId::JsBooleanCoercion,
                callee: LibraryApiCalleeContract::StaticGlobalFunction {
                    function: "Boolean",
                    requires_unshadowed_function: true,
                },
                result: StaticGlobalFunctionContract {
                    function: "Boolean",
                    requires_unshadowed_function: true,
                },
            })
        );
        assert_eq!(
            library_regex_test_contract(Lang::JavaScript, "test", 1),
            Some(LibraryRegexTestContract {
                id: LibraryApiContractId::RegexTest,
                callee: LibraryApiCalleeContract::RegexLiteralMethod {
                    method: "test",
                    required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
                },
                result: RegexTestContract {
                    method: "test",
                    required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
                },
            })
        );
        assert_eq!(
            library_imported_namespace_function_contract(Lang::Python, "prod", 2),
            Some(LibraryImportedNamespaceFunctionContract {
                id: LibraryApiContractId::ImportedNamespaceFunction(
                    ImportedNamespaceFunctionSemantic::ProductReduction {
                        op: Op::Mul,
                        identity: 1,
                    },
                ),
                callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
                    module: "math",
                    function: "prod",
                },
                result: ImportedNamespaceFunctionContract {
                    module: "math",
                    function: "prod",
                    receiver: MethodReceiverContract::ImportedNamespace("math"),
                    semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                        op: Op::Mul,
                        identity: 1,
                    },
                },
            })
        );
        assert_eq!(
            library_promise_then_contract(Lang::Vue, "then", 1),
            Some(LibraryPromiseThenContract {
                id: LibraryApiContractId::PromiseThen,
                callee: LibraryApiCalleeContract::AsyncMethod {
                    method: "then",
                    receiver: AsyncReceiverContract::ExactPromiseLike,
                },
                result: PromiseThenContract {
                    receiver: AsyncReceiverContract::ExactPromiseLike,
                },
            })
        );
        assert_eq!(
            library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
            Some(LibraryIteratorIdentityAdapterContract {
                id: LibraryApiContractId::IteratorIdentityAdapter,
                callee: LibraryApiCalleeContract::IteratorAdapterMethod {
                    method: "collect",
                    receiver: IteratorAdapterReceiverContract::ExactIterableValue,
                },
                result: IteratorIdentityAdapterContract {
                    receiver: IteratorAdapterReceiverContract::ExactIterableValue,
                },
            })
        );
        assert_eq!(
            library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 1),
            Some(LibraryStaticCollectionAdapterContract {
                id: LibraryApiContractId::StaticCollectionAdapter,
                callee: LibraryApiCalleeContract::JavaUtilStaticMember {
                    receiver: "Arrays",
                    method: "stream",
                },
                result: StaticCollectionAdapterContract {
                    module: "java.util",
                    exported: "Arrays",
                },
            })
        );
        assert_eq!(
            library_method_call_contract(Lang::Go, "Contains", 2),
            Some(LibraryMethodCallContract {
                id: LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::Contains,
                )),
                callee: LibraryApiCalleeContract::Method {
                    method: "Contains",
                    receiver: MethodReceiverContract::ImportedNamespace("slices"),
                },
                result: MethodCallContract {
                    semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                    receiver: MethodReceiverContract::ImportedNamespace("slices"),
                    args: MethodBuiltinArgs::GoSliceContains,
                },
            })
        );
    }

    #[test]
    fn library_non_factory_api_contracts_reject_raw_name_only_matches() {
        assert_eq!(
            library_map_key_view_contract(Lang::JavaScript, "keySet", 0),
            None
        );
        assert_eq!(library_map_key_view_contract(Lang::Python, "keys", 1), None);
        assert_eq!(
            library_map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
            None
        );
        assert_eq!(
            library_map_key_view_wrapper_contract(Lang::TypeScript, "Array", "from", 2),
            None
        );
        assert_eq!(library_map_get_contract(Lang::Python, "get", 1), None);
        assert_eq!(library_map_get_contract(Lang::Rust, "get", 2), None);
        assert_eq!(
            library_js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
            None
        );
        assert_eq!(
            library_js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
            None
        );
        assert_eq!(
            library_js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
            None
        );
        assert_eq!(
            library_js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
            None
        );
        assert_eq!(library_regex_test_contract(Lang::Ruby, "test", 1), None);
        assert_eq!(
            library_imported_namespace_function_contract(Lang::JavaScript, "prod", 1),
            None
        );
        assert_eq!(
            library_imported_namespace_function_contract(Lang::Python, "prod", 3),
            None
        );
        assert_eq!(library_promise_then_contract(Lang::Python, "then", 1), None);
        assert_eq!(
            library_promise_then_contract(Lang::TypeScript, "then", 2),
            None
        );
        assert_eq!(
            library_iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
            None
        );
        assert_eq!(
            library_iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
            None
        );
        assert_eq!(
            library_static_collection_adapter_contract(Lang::JavaScript, "Arrays", "stream", 1),
            None
        );
        assert_eq!(
            library_static_collection_adapter_contract(Lang::Java, "Arrays", "stream", 0),
            None
        );
        assert_eq!(library_method_call_contract(Lang::Python, "min", 2), None);
        assert_eq!(
            library_method_call_contract(Lang::JavaScript, "min", 1),
            None
        );
        assert_eq!(
            library_method_call_contract(Lang::JavaScript, "Contains", 2),
            None
        );
    }

    #[test]
    fn receiver_mutation_contracts_are_language_scoped_rows() {
        let js_push = module_binding_mutating_method_contract(Lang::JavaScript, "push", 1)
            .expect("js push receiver mutation contract");
        assert_eq!(js_push.pack_id, FIRST_PARTY_PACK_ID);
        assert_eq!(js_push.lang, Lang::JavaScript);
        assert_eq!(js_push.effect, EffectEvidenceKind::ReceiverMutation);
        assert_eq!(
            js_push.receiver,
            MethodEffectReceiverContract::PotentiallyMutableReceiver
        );
        assert!(module_binding_mutating_method_contract(Lang::TypeScript, "push", 1).is_some());
        assert!(module_binding_mutating_method_contract(Lang::JavaScript, "addAll", 1).is_none());
        assert!(module_binding_mutating_method_contract(Lang::Java, "addAll", 1).is_some());
        assert!(module_binding_mutating_method_contract(Lang::Python, "append", 1).is_some());
        assert!(module_binding_mutating_method_contract(Lang::Go, "append", 1).is_none());
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
                name: "len",
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
                name: "print",
                builtin: Builtin::Print,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Go, "append", 2),
            Some(FreeFunctionBuiltinContract {
                name: "append",
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
            free_function_builtin_contract(Lang::Python, "range", 0),
            None
        );
        assert!(free_function_builtin_contract(Lang::Python, "range", 3).is_some());
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "range", 4),
            None
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "max", 2),
            Some(FreeFunctionBuiltinContract {
                name: "max",
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
            library_java_collection_constructor_contract(Lang::Java, "ArrayList", 1),
            None
        );
        assert_eq!(
            library_java_collection_constructor_contract(Lang::JavaScript, "ArrayList", 0),
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
        let rust_push = builder_append_method_contract(Lang::Rust, "push", 1)
            .expect("rust push builder append contract");
        assert_eq!(rust_push.effect, EffectEvidenceKind::BuilderAppendCall);
        assert_eq!(
            rust_push.receiver,
            MethodEffectReceiverContract::ActiveCollectionBuilder
        );
        assert!(builder_append_method_contract(Lang::Rust, "push", 2).is_none());
        assert!(builder_append_method_contract(Lang::Java, "add", 1).is_some());
        assert!(builder_append_method_contract(Lang::JavaScript, "push", 1).is_some());
        assert!(builder_append_method_contract(Lang::TypeScript, "push", 1).is_some());
        assert!(builder_append_method_contract(Lang::Python, "append", 1).is_some());
        assert!(builder_append_method_contract(Lang::Ruby, "push", 1).is_none());
    }

    #[test]
    fn map_builder_index_write_contracts_are_language_scoped() {
        let contract =
            map_builder_index_write_contract(Lang::Python).expect("python map builder contract");
        assert_eq!(contract.pack_id, FIRST_PARTY_PACK_ID);
        assert_eq!(contract.id, IndexWriteContractId::MapBuilderEntryWrite);
        assert_eq!(
            contract.receiver,
            IndexWriteReceiverContract::ActiveMapBuilder
        );
        assert_eq!(contract.required_effect, EffectEvidenceKind::BindingWrite);
        assert_eq!(contract.channel, ChannelEligibility::ExactProven);
        assert!(map_builder_index_write_contract(Lang::Ruby).is_none());
        assert!(map_builder_index_write_contract(Lang::JavaScript).is_none());
    }

    #[test]
    fn exact_index_assignment_parts_require_effect_evidence() {
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
        let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(1), &[target, value]);
        let mut il = finish_il(b, assign, Lang::Go);

        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            None
        );
        assert!(!exact_non_overloadable_index_assignment(&il, assign));

        push_node_effect(
            &mut il,
            0,
            assign,
            EffectEvidenceKind::NonOverloadableIndexWrite,
        );
        push_node_effect(&mut il, 2, assign, EffectEvidenceKind::BindingWrite);

        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            Some((receiver, Some(key), value))
        );
        assert!(exact_non_overloadable_index_assignment(&il, assign));

        push_node_effect(&mut il, 1, assign, EffectEvidenceKind::BuilderAppendCall);
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            None
        );
        assert!(!exact_non_overloadable_index_assignment(&il, assign));
    }

    #[test]
    fn builder_append_call_args_require_effect_evidence() {
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

        assert_eq!(builder_append_call_args(&il, &interner, builtin), None);
        let mut il = il;
        push_node_effect(&mut il, 0, builtin, EffectEvidenceKind::BuilderAppendCall);
        push_node_effect(&mut il, 3, builtin, EffectEvidenceKind::ReceiverMutation);
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
    fn effect_evidence_can_prove_non_overloadable_index_write() {
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
        let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(9), &[target, value]);
        let mut il = finish_il(b, assign, Lang::Ruby);

        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            None
        );

        push_node_effect(
            &mut il,
            0,
            assign,
            EffectEvidenceKind::NonOverloadableIndexWrite,
        );
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            Some((receiver, Some(key), value))
        );

        push_node_effect(
            &mut il,
            1,
            assign,
            EffectEvidenceKind::SelfFieldWrite { field_hash: 1 },
        );
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            None
        );

        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
        let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(10), &[target, value]);
        let mut non_assign = finish_il(b, call, Lang::Ruby);
        push_node_effect(
            &mut non_assign,
            0,
            call,
            EffectEvidenceKind::NonOverloadableIndexWrite,
        );
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&non_assign, call),
            None
        );
    }

    #[test]
    fn append_effect_evidence_can_prove_raw_method_call() {
        let interner = Interner::default();
        let append = interner.intern("append");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
        let call = b.add(NodeKind::Call, Payload::None, sp(3), &[method, value]);
        let mut il = finish_il(b, call, Lang::Ruby);

        assert_eq!(builder_append_call_args(&il, &interner, call), None);

        push_node_effect(&mut il, 0, call, EffectEvidenceKind::BuilderAppendCall);
        assert_eq!(
            builder_append_call_args(&il, &interner, call),
            Some((receiver, value))
        );

        push_node_effect(
            &mut il,
            1,
            call,
            EffectEvidenceKind::NonOverloadableIndexWrite,
        );
        assert_eq!(builder_append_call_args(&il, &interner, call), None);
    }

    #[test]
    fn place_evidence_is_authoritative_for_self_field_proof() {
        let interner = Interner::default();
        let this = interner.intern("this");
        let field_name = interner.intern("value");
        let field_hash = stable_symbol_hash("value");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(this), sp(1), &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp(2),
            &[receiver],
        );
        let value = b.add(NodeKind::Var, Payload::Cid(1), sp(3), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(4), &[field, value]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[receiver]);
        let root = b.add(NodeKind::Block, Payload::None, sp(1), &[assign, ret]);
        let mut il = finish_il(b, root, Lang::Ruby);

        assert!(!exact_java_this_var(&il, &interner, receiver));
        assert!(!exact_java_this_field(&il, &interner, field));
        assert!(!exact_java_return_this(&il, &interner, ret));
        assert!(!exact_self_field_write_assignment(&il, &interner, assign));

        let receiver_evidence =
            push_node_place(&mut il, 0, receiver, PlaceEvidenceKind::SelfReceiver);
        let field_evidence = push_node_place_with_dependencies(
            &mut il,
            1,
            field,
            PlaceEvidenceKind::SelfField { field_hash },
            vec![receiver_evidence],
        );
        push_node_effect_with_dependencies(
            &mut il,
            2,
            assign,
            EffectEvidenceKind::SelfFieldWrite { field_hash },
            vec![field_evidence],
        );
        assert!(exact_java_this_var(&il, &interner, receiver));
        assert!(exact_java_this_field(&il, &interner, field));
        assert!(exact_java_return_this(&il, &interner, ret));
        assert!(exact_self_field_write_assignment(&il, &interner, assign));

        push_node_place(&mut il, 3, field, PlaceEvidenceKind::SelfReceiver);
        assert!(!exact_java_this_field(&il, &interner, field));
        assert!(!exact_self_field_write_assignment(&il, &interner, assign));

        push_node_place(
            &mut il,
            4,
            receiver,
            PlaceEvidenceKind::SelfField { field_hash },
        );
        assert!(!exact_java_this_var(&il, &interner, receiver));
        assert!(!exact_java_return_this(&il, &interner, ret));

        let other = interner.intern("other");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(other), sp(5), &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp(6),
            &[receiver],
        );
        let value = b.add(NodeKind::Var, Payload::Cid(1), sp(7), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(8), &[field, value]);
        let mut il = finish_il(b, assign, Lang::Ruby);
        push_node_place(
            &mut il,
            0,
            field,
            PlaceEvidenceKind::SelfField { field_hash },
        );
        push_node_effect(
            &mut il,
            1,
            assign,
            EffectEvidenceKind::SelfFieldWrite { field_hash },
        );
        assert!(!exact_java_this_field(&il, &interner, field));
        assert!(!exact_self_field_write_assignment(&il, &interner, assign));
    }

    #[test]
    fn source_fact_contracts_are_span_keyed_evidence() {
        let mut b = IlBuilder::new(FileId(0));
        let call = b.add(NodeKind::Call, Payload::None, sp(7), &[]);
        let regex = b.add(NodeKind::Lit, Payload::LitStr(42), sp(8), &[]);
        let await_boundary = b.add(NodeKind::Raw, Payload::None, sp(9), &[]);
        let root = b.add(
            NodeKind::Block,
            Payload::None,
            sp(7),
            &[call, regex, await_boundary],
        );
        let mut il = finish_il(b, root, Lang::JavaScript);
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::source_span(sp(7)),
            EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::source_span(sp(8)),
            EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            2,
            EvidenceAnchor::source_span(sp(9)),
            EvidenceKind::Source(SourceFactKind::Protocol(SourceProtocolKind::Await)),
            EvidenceStatus::Asserted,
        ));

        assert!(construct_syntax_proof(&il, call));
        assert!(regex_literal_proof(&il, regex));
        assert_eq!(
            source_protocol_at_node(&il, await_boundary),
            Some(SourceProtocolKind::Await)
        );
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
    fn source_fact_evidence_requires_live_dependencies() {
        let mut b = IlBuilder::new(FileId(0));
        let call = b.add(NodeKind::Call, Payload::None, sp(10), &[]);
        let root = b.add(NodeKind::Block, Payload::None, sp(10), &[call]);
        let mut il = finish_il(b, root, Lang::Rust);
        il.evidence.push(evidence_with_dependencies(
            0,
            EvidenceAnchor::source_span(sp(10)),
            EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
            EvidenceStatus::Asserted,
            vec![EvidenceId(99)],
        ));

        assert_eq!(source_call_at_node(&il, call), None);
        assert!(!source_fact_at_node(
            &il,
            call,
            SourceFactKind::Call(SourceCallKind::MacroInvocation),
        ));
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
