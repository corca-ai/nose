//! Effect, place, and exact-fragment helper contracts.

use crate::evidence::{unique_asserted_evidence_at, EvidenceResolution};
use crate::{
    admitted_library_method_call_at_call, js_like_lang, ChannelEligibility,
    LibraryApiCalleeContract, MethodBuiltinArgs, MethodSemanticContract, FIRST_PARTY_PACK_ID,
};
use nose_il::{
    stable_symbol_hash, Builtin, EffectEvidenceKind, EvidenceAnchor, EvidenceKind, EvidenceStatus,
    Il, Interner, Lang, NodeId, NodeKind, Payload, PlaceEvidenceKind,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectSemantics {
    pub(crate) lang: Lang,
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
    pub(crate) lang: Lang,
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

pub(crate) fn asserted_effect_at_node(il: &Il, node: NodeId, wanted: EffectEvidenceKind) -> bool {
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

pub fn builder_append_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MethodEffectContract> {
    EffectSemantics { lang }.builder_append_method_contract(method, arg_count)
}

pub fn map_builder_index_write_contract(lang: Lang) -> Option<IndexWriteContract> {
    EffectSemantics { lang }.map_builder_index_write_contract()
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

/// `(receiver, value)` of a source method call whose append meaning is proven by
/// same-span `LibraryApi(MethodCall(Builtin(Append)))` occurrence evidence.
pub fn admitted_builder_append_method_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return None;
    }
    let kids = il.children(node);
    let [_callee, item] = kids else {
        return None;
    };
    let admitted = admitted_library_method_call_at_call(il, interner, node)?;
    let receiver = admitted.receiver?;
    let LibraryApiCalleeContract::Method { method, .. } = admitted.contract.callee else {
        return None;
    };
    let effect = builder_append_method_contract(il.meta.lang, method, admitted.arg_count)?;
    if effect.effect != EffectEvidenceKind::BuilderAppendCall
        || effect.receiver != MethodEffectReceiverContract::ActiveCollectionBuilder
    {
        return None;
    }
    if admitted.contract.result.semantic != MethodSemanticContract::Builtin(Builtin::Append)
        || admitted.contract.result.args != MethodBuiltinArgs::ReceiverThenAll
    {
        return None;
    }
    Some((receiver, *item))
}

/// `(receiver, value)` of a source method call licensed by the first-party
/// active-builder append method-effect contract.
pub fn contracted_builder_append_method_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    let (receiver, method_text, item, arg_count) =
        single_item_method_call_parts(il, interner, node)?;
    let effect = builder_append_method_contract(il.meta.lang, method_text, arg_count)?;
    if effect.effect != EffectEvidenceKind::BuilderAppendCall
        || effect.receiver != MethodEffectReceiverContract::ActiveCollectionBuilder
    {
        return None;
    }
    Some((receiver, item))
}

fn single_item_method_call_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<(NodeId, &'a str, NodeId, usize)> {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return None;
    }
    let kids = il.children(node);
    let [callee, item] = kids else {
        return None;
    };
    if il.kind(*callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(*callee).payload else {
        return None;
    };
    let receiver = *il.children(*callee).first()?;
    Some((receiver, interner.resolve(method), *item, kids.len() - 1))
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

pub fn module_binding_mutating_method_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MethodEffectContract> {
    EffectSemantics { lang }.receiver_mutation_method_contract(method, arg_count)
}
