use super::*;
use rustc_hash::FxHashMap;
use std::cell::RefCell;

pub fn domain_evidence_from_param_semantic(semantic: ParamSemantic) -> DomainEvidence {
    DomainEvidence::from_param_semantic(semantic)
}
pub fn domain_evidence_at_span(il: &Il, span: Span) -> Option<DomainEvidence> {
    match unique_asserted_evidence_at(
        il,
        span,
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
    Exact(DomainEvidence),
    AnyOf(&'static [DomainEvidence]),
}

impl DomainRequirement {
    pub const ARRAY: Self = Self::Exact(DomainEvidence::Array);
    pub const BOOLEAN: Self = Self::Exact(DomainEvidence::Boolean);
    pub const BYTE_ARRAY: Self = Self::Exact(DomainEvidence::ByteArray);
    pub const COLLECTION: Self = Self::Exact(DomainEvidence::Collection);
    pub const COLLECTION_OR_SET: Self =
        Self::AnyOf(&[DomainEvidence::Collection, DomainEvidence::Set]);
    pub const COLLECTION_OR_MAP: Self = Self::AnyOf(&[
        DomainEvidence::Array,
        DomainEvidence::Collection,
        DomainEvidence::Set,
        DomainEvidence::Map,
    ]);
    pub const FLOAT: Self = Self::Exact(DomainEvidence::Float);
    pub const FUTURE_LIKE: Self =
        Self::AnyOf(&[DomainEvidence::FutureLike, DomainEvidence::PromiseLike]);
    pub const ARRAY_OR_COLLECTION: Self =
        Self::AnyOf(&[DomainEvidence::Array, DomainEvidence::Collection]);
    pub const ARRAY_COLLECTION_OR_SET: Self = Self::AnyOf(&[
        DomainEvidence::Array,
        DomainEvidence::Collection,
        DomainEvidence::Set,
    ]);
    pub const ITERABLE: Self = Self::Exact(DomainEvidence::Iterable);
    pub const ITERABLE_OR_ITERATOR: Self =
        Self::AnyOf(&[DomainEvidence::Iterable, DomainEvidence::Iterator]);
    pub const ITERATOR: Self = Self::Exact(DomainEvidence::Iterator);
    pub const SET: Self = Self::Exact(DomainEvidence::Set);
    pub const SET_OR_MAP: Self = Self::AnyOf(&[DomainEvidence::Set, DomainEvidence::Map]);
    pub const MAP: Self = Self::Exact(DomainEvidence::Map);
    pub const NUMBER: Self = Self::AnyOf(&[DomainEvidence::Number, DomainEvidence::Float]);
    pub const OPTION: Self = Self::Exact(DomainEvidence::Option);
    pub const PROMISE_LIKE: Self = Self::Exact(DomainEvidence::PromiseLike);
    pub const RECORD: Self = Self::Exact(DomainEvidence::Record);
    pub const RESULT: Self = Self::Exact(DomainEvidence::Result);
    pub const STRING: Self = Self::Exact(DomainEvidence::String);
    pub const INTEGER: Self = Self::Exact(DomainEvidence::Integer);
    pub const INTEGER_OR_NUMBER: Self = Self::AnyOf(&[
        DomainEvidence::Integer,
        DomainEvidence::Float,
        DomainEvidence::Number,
    ]);

    pub const fn exact(domain: DomainEvidence) -> Self {
        Self::Exact(domain)
    }

    pub const fn any_of(domains: &'static [DomainEvidence]) -> Self {
        Self::AnyOf(domains)
    }

    pub fn accepts(self, domain: DomainEvidence) -> bool {
        match self {
            DomainRequirement::Exact(expected) => domain == expected,
            DomainRequirement::AnyOf(domains) => domains.contains(&domain),
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
        il.node(node).span,
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
        expected.span(),
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
        span,
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
    // Only assignments in the reference's own scope can match — plus, for a
    // free (pre-alpha) name, module-level assignments. The scope-bucketed
    // index replaces the whole-arena pass this did per reference.
    let module_level: &[NodeId] = if reference_is_free_name && scope.is_some() {
        il.assigns_in_scope(None)
    } else {
        &[]
    };
    for &assign in il.assigns_in_scope(scope).iter().chain(module_level) {
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
    il.assigns_in_scope(Some(scope)).iter().any(|&id| {
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
    il.assigns_in_scope(Some(scope)).iter().any(|&id| {
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var
            && matches!(il.node(lhs).payload, Payload::Cid(lhs_cid) if lhs_cid == cid)
    })
}

pub(crate) fn nearest_scope(il: &Il, node: NodeId) -> Option<NodeId> {
    il.nearest_scope(node)
}
