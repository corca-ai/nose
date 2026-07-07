//! `nose-il` — the normalized Intermediate Language (IL) at the heart of nose.
//!
//! The IL is a compact, arena-backed tree (see [`Node`]). One [`Il`] holds one
//! lowered source file; a whole codebase is a [`Corpus`] of them sharing a single
//! string [`Interner`]. Every node carries a [`Span`] for sourcemap-style
//! traceback. The crate defines the data model and (de)serialization only — the
//! frontends build raw IL, and `nose-normalize` rewrites it into canonical form.
//!
//! proof-obligation: il.arena.validity

mod builder;
mod corpus;
pub mod ident;
mod il;
pub mod intern;
pub mod node;
mod sexpr;
pub mod span;
#[cfg(test)]
mod tests;
mod unit;
mod unit_domains;
mod unit_evidence;
mod unit_facets;

pub use builder::IlBuilder;
pub use corpus::Corpus;
pub use ident::{
    contains_c_identifier, contains_js_identifier, is_c_identifier_continue,
    is_js_identifier_continue,
};
pub use il::Il;
pub use intern::{stable_symbol_hash, symbol_index, Interner, Symbol, FNV_OFFSET_BASIS, FNV_PRIME};
pub use node::{
    BoundOrderGuardActivation, Builtin, CTypeTarget, CallTargetEvidenceKind, DomainEvidence,
    EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, GuardEvidenceKind, HoFKind,
    ImportEvidenceKind, JsRecordGuardComparison, JsRecordGuardNullCheck, LibraryApiEvidenceKind,
    LitClass, LoopKind, Node, NodeId, NodeKind, Op, ParamSemantic, Payload, PlaceEvidenceKind,
    PromiseSettledValueEvidenceKind, PromiseSettlementChannel, SequenceSurfaceKind,
    SourceBindingKind, SourceCallKind, SourceCastKind, SourceComprehensionKind, SourceFactKind,
    SourceLiteralKind, SourceOperatorKind, SourcePatternKind, SourceProtocolKind, SourceRangeKind,
    SymbolEvidenceKind, TypeEvidenceKind,
};
pub use span::{FileId, FileMeta, Lang, Span};
pub use unit::{Unit, UnitKind, UnitOrigin};
pub use unit_domains::{UnitDomain, UnitDomains};
pub use unit_evidence::{UnitEvidenceFlag, UnitEvidenceFlags};
pub use unit_facets::{
    RegionKind, SourceGranularity, UnitBodyKind, UnitContainerKind, UnitSubkind,
};
