//! Evidence resolution plus source/domain proof helpers for the semantic facade.
//!
//! This module owns fail-closed lookup rules over `EvidenceRecord`s and the
//! lightweight value/domain contracts consumed by operators, normalize, and detect.

use super::*;
use rustc_hash::FxHashMap;
use std::cell::RefCell;

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
pub(crate) enum EvidenceResolution<T> {
    Missing,
    Found(T),
    Ambiguous,
}

pub(crate) fn unique_evidence_at<T: Copy + Eq>(
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

pub(crate) fn unique_asserted_evidence_at<T: Copy + Eq>(
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
        SourceFactKind::Cast(cast) => source_cast_at_node(il, node) == Some(cast),
        SourceFactKind::Call(call) => source_call_at_node(il, node) == Some(call),
        SourceFactKind::Protocol(protocol) => source_protocol_at_node(il, node) == Some(protocol),
        SourceFactKind::Literal(literal) => source_literal_at_node(il, node) == Some(literal),
        SourceFactKind::Comprehension(comprehension) => {
            source_comprehension_at_node(il, node) == Some(comprehension)
        }
        SourceFactKind::Range(range) => source_range_at_node(il, node) == Some(range),
        SourceFactKind::Pattern(pattern) => source_pattern_at_node(il, node) == Some(pattern),
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

pub fn source_cast_at_node(il: &Il, node: NodeId) -> Option<SourceCastKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Cast(cast)) => Some(cast),
        _ => None,
    }) {
        EvidenceResolution::Found(cast) => Some(cast),
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

pub fn source_range_at_node(il: &Il, node: NodeId) -> Option<SourceRangeKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Range(range)) => Some(range),
        _ => None,
    }) {
        EvidenceResolution::Found(range) => Some(range),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_pattern_at_node(il: &Il, node: NodeId) -> Option<SourcePatternKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Pattern(pattern)) => Some(pattern),
        _ => None,
    }) {
        EvidenceResolution::Found(pattern) => Some(pattern),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn direct_function_call_target_at_call(il: &Il, call: NodeId, target_root: NodeId) -> bool {
    if il.kind(call) != NodeKind::Call || il.kind(target_root) != NodeKind::Func {
        return false;
    }
    let call_span = il.node(call).span;
    let target_span = il.node(target_root).span;
    match unique_asserted_evidence_at(
        il,
        |anchor| matches!(anchor, EvidenceAnchor::Node { span, kind } if span == call_span && kind == NodeKind::Call),
        |evidence| match evidence {
            EvidenceKind::CallTarget(target) => Some(target),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(CallTargetEvidenceKind::DirectFunction {
            target_span: proven_span,
            ..
        }) => proven_span == target_span,
        EvidenceResolution::Found(
            CallTargetEvidenceKind::DirectMethod { .. }
            | CallTargetEvidenceKind::ImportedFunction { .. }
            | CallTargetEvidenceKind::ImportedMember { .. }
            | CallTargetEvidenceKind::DynamicDispatch { .. },
        ) => false,
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallTargetEvidenceStatus {
    Missing,
    Admitted(CallTargetEvidenceKind),
    Rejected,
}

pub fn call_target_evidence_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<CallTargetEvidenceKind> {
    match call_target_evidence_status_at_call(il, interner, call) {
        CallTargetEvidenceStatus::Admitted(target) => Some(target),
        CallTargetEvidenceStatus::Missing | CallTargetEvidenceStatus::Rejected => None,
    }
}

pub fn call_target_evidence_status_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> CallTargetEvidenceStatus {
    if il.kind(call) != NodeKind::Call {
        return CallTargetEvidenceStatus::Missing;
    }
    let call_span = il.node(call).span;
    let target = match unique_asserted_evidence_at(
        il,
        |anchor| matches!(anchor, EvidenceAnchor::Node { span, kind } if span == call_span && kind == NodeKind::Call),
        |evidence| match evidence {
            EvidenceKind::CallTarget(target) => Some(target),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(target) => target,
        EvidenceResolution::Ambiguous => return CallTargetEvidenceStatus::Rejected,
        EvidenceResolution::Missing => return CallTargetEvidenceStatus::Missing,
    };
    if call_target_matches_call_shape(il, interner, call, target) {
        CallTargetEvidenceStatus::Admitted(target)
    } else {
        CallTargetEvidenceStatus::Rejected
    }
}

pub fn imported_function_call_target_at_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::ImportedFunction { .. })
    )
}

pub fn imported_member_call_target_at_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::ImportedMember { .. })
    )
}

pub fn direct_method_call_target_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    target_root: NodeId,
) -> bool {
    if il.kind(call) != NodeKind::Call || il.kind(target_root) != NodeKind::Func {
        return false;
    }
    matches!(
        call_target_evidence_at_call(il, interner, call),
        Some(CallTargetEvidenceKind::DirectMethod { target_span, .. })
            if target_span == il.node(target_root).span
    )
}

fn call_target_matches_call_shape(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    target: CallTargetEvidenceKind,
) -> bool {
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    match target {
        CallTargetEvidenceKind::DirectFunction { name_hash, .. }
        | CallTargetEvidenceKind::ImportedFunction {
            local_hash: name_hash,
            ..
        } => var_selector_matches_if_available(il, interner, callee, name_hash),
        CallTargetEvidenceKind::DirectMethod { method_hash, .. }
        | CallTargetEvidenceKind::DynamicDispatch { method_hash, .. } => {
            field_selector_matches(il, interner, callee, method_hash)
        }
        CallTargetEvidenceKind::ImportedMember { member_hash, .. } => {
            field_selector_matches(il, interner, callee, member_hash)
        }
    }
}

fn var_selector_matches_if_available(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_hash: u64,
) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    match il.node(callee).payload {
        Payload::Name(name) => interner.symbol_hash(name) == expected_hash,
        Payload::Cid(_) => true,
        _ => false,
    }
}

fn field_selector_matches(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_hash: u64,
) -> bool {
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    match il.node(callee).payload {
        Payload::Name(name) => interner.symbol_hash(name) == expected_hash,
        _ => false,
    }
}

pub fn admitted_hof_api_at_node(il: &Il, node: NodeId, kind: HoFKind) -> bool {
    if il.kind(node) != NodeKind::HoF || il.node(node).payload != Payload::HoF(kind) {
        return false;
    }
    library_api_dependency_id_for_normalized_hof(il, node).is_some()
}

pub fn admitted_hof_demand_effect_profile_at_node(
    il: &Il,
    node: NodeId,
    kind: HoFKind,
) -> Option<DemandEffectProfile> {
    if il.kind(node) != NodeKind::HoF || il.node(node).payload != Payload::HoF(kind) {
        return None;
    }
    if let Some(source) = source_comprehension_at_node(il, node) {
        return source_comprehension_hof_demand_effect_profile(kind, source);
    }
    admitted_hof_api_at_node(il, node, kind)
        .then(|| library_hof_demand_effect_profile(il.meta.lang, kind))
        .flatten()
}

pub fn admitted_terminal_count_reduction_at_call(il: &Il, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Call || il.node(node).payload != Payload::Builtin(Builtin::Len) {
        return false;
    }
    let Some(contract) = library_method_call_contract(il.meta.lang, "count", 0) else {
        return false;
    };
    library_api_dependency_id_for_canonical_builtin_method_call(
        il,
        node,
        Builtin::Len,
        contract.callee,
        0,
    )
    .is_some()
}

pub fn admitted_builtin_semantics_at_call(il: &Il, node: NodeId, builtin: Builtin) -> bool {
    if il.kind(node) != NodeKind::Call || il.node(node).payload != Payload::Builtin(builtin) {
        return false;
    }
    language_core_builtin_at_call(il, node, builtin)
        || library_api_dependency_id_for_canonical_builtin_call(il, node, builtin).is_some()
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

    pub(crate) fn is_known(self) -> bool {
        self != ValueDomain::Unknown
    }

    pub fn from_domain_evidence(domain: DomainEvidence) -> Option<ValueDomain> {
        if domain.is_integer_or_number() {
            Some(ValueDomain::Number)
        } else if domain.is_boolean() {
            Some(ValueDomain::Boolean)
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

pub(crate) fn strict_numeric_operand_operator(op: Op) -> bool {
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
    Boolean,
    ByteArray,
    Collection,
    CollectionOrSet,
    CollectionOrMap,
    Float,
    FutureLike,
    ArrayOrCollection,
    ArrayCollectionOrSet,
    Iterable,
    IterableOrIterator,
    Iterator,
    Set,
    SetOrMap,
    Map,
    Nominal { type_hash: u64 },
    Number,
    Option,
    PromiseLike,
    Record,
    Result,
    String,
    Integer,
    IntegerOrNumber,
}

impl DomainRequirement {
    pub fn accepts(self, domain: DomainEvidence) -> bool {
        match self {
            DomainRequirement::Array => domain.is_array(),
            DomainRequirement::Boolean => domain.is_boolean(),
            DomainRequirement::ByteArray => domain.is_byte_array(),
            DomainRequirement::Collection => domain == DomainEvidence::Collection,
            DomainRequirement::CollectionOrSet => domain.is_collection_or_set(),
            DomainRequirement::CollectionOrMap => {
                domain.is_array_collection_or_set() || domain.is_map()
            }
            DomainRequirement::Float => domain.is_float(),
            DomainRequirement::FutureLike => domain.is_future_like(),
            DomainRequirement::ArrayOrCollection => domain.is_array_or_collection(),
            DomainRequirement::ArrayCollectionOrSet => domain.is_array_collection_or_set(),
            DomainRequirement::Iterable => domain.is_iterable(),
            DomainRequirement::IterableOrIterator => domain.is_iterable_or_iterator(),
            DomainRequirement::Iterator => domain.is_iterator(),
            DomainRequirement::Set => domain.is_set(),
            DomainRequirement::SetOrMap => domain.is_set() || domain.is_map(),
            DomainRequirement::Map => domain.is_map(),
            DomainRequirement::Nominal { type_hash } => domain.is_nominal(type_hash),
            DomainRequirement::Number => {
                matches!(domain, DomainEvidence::Number | DomainEvidence::Float)
            }
            DomainRequirement::Option => domain.is_option(),
            DomainRequirement::PromiseLike => domain.is_promise_like(),
            DomainRequirement::Record => domain.is_record(),
            DomainRequirement::Result => domain.is_result(),
            DomainRequirement::String => domain.is_string(),
            DomainRequirement::Integer => domain.is_integer(),
            DomainRequirement::IntegerOrNumber => domain.is_integer_or_number(),
        }
    }
}

pub fn nominal_type_domain_at_node(
    il: &Il,
    node: NodeId,
    type_hash: u64,
) -> Option<DomainEvidence> {
    match unique_asserted_evidence_at(
        il,
        |anchor| anchor == EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        |evidence| match evidence {
            EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
                type_hash: actual_hash,
                domain,
            }) if actual_hash == type_hash => Some(domain),
            _ => None,
        },
    ) {
        EvidenceResolution::Found(domain) => Some(domain),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
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

/// Cached receiver-domain resolver for consumers that inspect many call/property receivers.
///
/// The proof policy is identical to [`domain_evidence_for_receiver`]: exact node evidence first,
/// then immutable binding evidence, then scoped parameter evidence, with ambiguous/conflicting or
/// dependency-broken records closing the proof. This type exists so normalize/detect consumers do
/// not need local domain side tables that can drift from the kernel facade.
pub struct ReceiverDomainEvidenceIndex<'a> {
    il: &'a Il,
    interner: &'a Interner,
    cache: RefCell<FxHashMap<NodeId, Option<DomainEvidence>>>,
}

impl<'a> ReceiverDomainEvidenceIndex<'a> {
    pub fn new(il: &'a Il, interner: &'a Interner) -> Self {
        Self {
            il,
            interner,
            cache: RefCell::new(FxHashMap::default()),
        }
    }

    pub fn domain_evidence_for_receiver(&self, receiver: NodeId) -> Option<DomainEvidence> {
        if let Some(domain) = self.cache.borrow().get(&receiver).copied() {
            return domain;
        }
        let domain = domain_evidence_for_receiver(self.il, self.interner, receiver);
        self.cache.borrow_mut().insert(receiver, domain);
        domain
    }

    pub fn receiver_satisfies_domain(
        &self,
        receiver: NodeId,
        requirement: DomainRequirement,
    ) -> bool {
        self.domain_evidence_for_receiver(receiver)
            .is_some_and(|domain| requirement.accepts(domain))
    }
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

pub(crate) fn unique_binding_lhs_for_var_reference(
    il: &Il,
    node: NodeId,
) -> EvidenceResolution<NodeId> {
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

pub(crate) fn assignment_is_visible_at_reference(
    il: &Il,
    assign: NodeId,
    reference: NodeId,
) -> bool {
    il.node(assign).span.end_byte <= il.node(reference).span.start_byte
}

pub(crate) fn var_references_same_binding(il: &Il, lhs: NodeId, reference: NodeId) -> bool {
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

pub(crate) fn nearest_named_param_scope(
    il: &Il,
    node: NodeId,
    name: Symbol,
) -> Option<(NodeId, NodeId)> {
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

pub(crate) fn span_contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file
        && outer.start_byte <= inner.start_byte
        && inner.end_byte <= outer.end_byte
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

pub(crate) fn nearest_scope(il: &Il, node: NodeId) -> Option<NodeId> {
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
