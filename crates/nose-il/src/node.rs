//! The IL node model. The IL is a small, desugared core language: every
//! frontend lowers its surface syntax into these node kinds, and the
//! normalization passes rewrite within them. Keeping the set small is what lets
//! semantically-equivalent code from different languages converge to the same
//! shape.

mod core;
mod domains;
mod evidence;
mod ops;
mod source;

pub use self::core::{Node, NodeId, NodeKind, Payload};
pub use self::domains::{DomainEvidence, ParamSemantic};
pub use self::evidence::{
    BoundOrderGuardActivation, CTypeTarget, CallTargetEvidenceKind, EffectEvidenceKind,
    EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
    EvidenceStatus, GuardEvidenceKind, ImportEvidenceKind, JsRecordGuardComparison,
    JsRecordGuardNullCheck, LibraryApiEvidenceKind, ParameterShapeEvidenceKind, PlaceEvidenceKind,
    PromiseSettledValueEvidenceKind, PromiseSettlementChannel, SequenceSurfaceKind,
    SymbolEvidenceKind, TypeEvidenceKind,
};
pub use self::ops::{Builtin, HoFKind, LitClass, LoopKind, Op};
pub use self::source::{
    SourceBindingKind, SourceCallKind, SourceCastKind, SourceComprehensionKind, SourceFactKind,
    SourceLiteralKind, SourceOperatorKind, SourcePatternKind, SourceProtocolKind, SourceRangeKind,
};
