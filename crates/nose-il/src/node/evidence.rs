use crate::span::Span;
use serde::{Deserialize, Serialize};

use super::{DomainEvidence, NodeKind, SourceFactKind};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceId(pub u32);

/// Stable subject addressed by a semantic evidence record. Node ids are not used
/// because normalization rebuilds arenas; consumers match by source span plus the
/// expected subject kind and fail closed when that is ambiguous.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceAnchor {
    SourceSpan(Span),
    Node { span: Span, kind: NodeKind },
    Param { span: Span },
    Binding { span: Span, local_hash: u64 },
    Sequence { span: Span },
}

impl EvidenceAnchor {
    pub fn source_span(span: Span) -> Self {
        EvidenceAnchor::SourceSpan(span)
    }

    pub fn node(span: Span, kind: NodeKind) -> Self {
        EvidenceAnchor::Node { span, kind }
    }

    pub fn param(span: Span) -> Self {
        EvidenceAnchor::Param { span }
    }

    pub fn binding(span: Span, local_hash: u64) -> Self {
        EvidenceAnchor::Binding { span, local_hash }
    }

    pub fn sequence(span: Span) -> Self {
        EvidenceAnchor::Sequence { span }
    }

    /// The anchor's subject span. Every anchor kind addresses exactly one span,
    /// and all matching is exact span equality — which is what makes anchors
    /// indexable by span (see `Il::evidence_anchored_at`).
    pub fn span(self) -> Span {
        match self {
            EvidenceAnchor::SourceSpan(span)
            | EvidenceAnchor::Node { span, .. }
            | EvidenceAnchor::Param { span }
            | EvidenceAnchor::Binding { span, .. }
            | EvidenceAnchor::Sequence { span } => span,
        }
    }

    pub fn matches_span(self, span: Span) -> bool {
        self.span() == span
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceEmitter {
    #[serde(rename = "FirstParty", alias = "Builtin")]
    Builtin,
    External,
}

impl EvidenceEmitter {
    #[allow(non_upper_case_globals)]
    pub const FirstParty: Self = Self::Builtin;
}

/// Provenance attached to semantic evidence. Hashes are stable symbol hashes so
/// serialized IL does not depend on an interner instance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceProvenance {
    pub emitter: EvidenceEmitter,
    pub pack_hash: Option<u64>,
    pub rule_hash: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceStatus {
    Asserted,
    Ambiguous,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ImportEvidenceKind {
    Binding {
        module_hash: u64,
        exported_hash: u64,
    },
    Namespace {
        module_hash: u64,
    },
    Wildcard {
        module_hash: u64,
    },
    Require {
        module_hash: u64,
    },
    ImmutableLiteralExport {
        module_hash: u64,
        exported_hash: u64,
        root_kind: NodeKind,
    },
    ImportedLiteralSnapshot {
        module_hash: u64,
        exported_hash: u64,
        root_kind: NodeKind,
    },
    CQuoteInclude {
        include_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CTypeTarget {
    UnsignedInteger { bits: u16 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum TypeEvidenceKind {
    CTypeAlias {
        alias_hash: u64,
        target: CTypeTarget,
    },
    NominalDomain {
        type_hash: u64,
        domain: DomainEvidence,
    },
}

/// Kernel-facing proof that a source-level symbol denotes a specific global or
/// imported API coordinate. The spelling is only a selector; exact consumers
/// must require one of these identities, or derive it through a compatibility
/// fallback that proves shadowing/import preconditions.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SymbolEvidenceKind {
    UnshadowedGlobal {
        name_hash: u64,
    },
    ImportedBinding {
        module_hash: u64,
        exported_hash: u64,
    },
    ImportedNamespace {
        module_hash: u64,
    },
    QualifiedGlobal {
        path_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum JsRecordGuardNullCheck {
    StrictNonNull,
    LooseNonNull,
    DoubleNegationTruthy,
    BooleanGlobalTruthy,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum JsRecordGuardComparison {
    StrictOnly,
    LooseEqualityAllowed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum GuardEvidenceKind {
    JsRecordShape {
        subject_hash: u64,
        null_check: JsRecordGuardNullCheck,
        comparison: JsRecordGuardComparison,
    },
    JsOwnProperty {
        api_path_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PlaceEvidenceKind {
    SelfReceiver,
    SelfField { field_hash: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EffectEvidenceKind {
    /// A write to a local/module binding or to a place rooted at such a binding.
    /// Consumers must still check the syntactic target and scope before applying
    /// this to a particular binding.
    BindingWrite,
    BuilderAppendCall,
    NonOverloadableIndexWrite,
    /// A call mutates its receiver. This is a mutation-risk fact, not proof that
    /// the call participates in any exact builder or collection law.
    ReceiverMutation,
    /// A call argument may escape to unknown code and be mutated outside the
    /// visible expression. Consumers must check the argument syntax before
    /// applying this to a particular binding.
    OpaqueArgumentEscape,
    SelfFieldWrite {
        field_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LibraryApiEvidenceKind {
    Contract {
        contract_hash: u64,
        callee_hash: u64,
        arity: u16,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CallTargetEvidenceKind {
    /// A call to a file-local function unit proven by lexical binding/scope
    /// evidence. This is opaque call identity, not a library semantic contract.
    DirectFunction { target_span: Span, name_hash: u64 },
    /// A call to a file-local method/function body whose receiver dispatch has
    /// been proven by a language producer. Exact consumers must still prove the
    /// receiver expression is exact-safe before treating the call as opaque
    /// same-target identity.
    DirectMethod {
        target_span: Span,
        receiver_type_hash: u64,
        method_hash: u64,
    },
    /// A call through a local binding proven to denote a specific imported
    /// function coordinate. This records target identity only; library semantics
    /// still require `LibraryApi` evidence.
    ImportedFunction {
        module_hash: u64,
        exported_hash: u64,
        local_hash: u64,
    },
    /// A static/member call where the receiver/member pair is proven to denote a
    /// specific imported coordinate, such as an imported namespace member.
    ImportedMember {
        module_hash: u64,
        exported_hash: u64,
        member_hash: u64,
    },
    /// A method dispatch proof that names a protocol/dispatch family but does
    /// not prove one concrete implementation target. By itself this is not exact
    /// opaque call identity.
    DynamicDispatch {
        protocol_hash: u64,
        method_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SequenceSurfaceKind {
    Untagged,
    Collection,
    Tuple,
    Map,
    Pair,
    RecordGuard,
    OwnPropertyGuard,
    GoCompositeMapLiteral,
    GoMapEntry,
    RustStructExpression,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceKind {
    Source(SourceFactKind),
    Domain(DomainEvidence),
    Import(ImportEvidenceKind),
    Symbol(SymbolEvidenceKind),
    Type(TypeEvidenceKind),
    Guard(GuardEvidenceKind),
    Place(PlaceEvidenceKind),
    Effect(EffectEvidenceKind),
    LibraryApi(LibraryApiEvidenceKind),
    CallTarget(CallTargetEvidenceKind),
    SequenceSurface(SequenceSurfaceKind),
}

/// Pack-facing semantic evidence record. It is evidence, not a verdict: exact
/// consumers must check contracts, provenance, dependencies, and ambiguity.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: EvidenceId,
    pub anchor: EvidenceAnchor,
    pub kind: EvidenceKind,
    pub provenance: EvidenceProvenance,
    pub dependencies: Vec<EvidenceId>,
    pub status: EvidenceStatus,
}
