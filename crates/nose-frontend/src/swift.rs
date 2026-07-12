//! Swift → raw IL lowering.
//!
//! Swift is lowered as a statically-typed C-family frontend: functions and methods
//! become units, declarations and assignments lower to `Assign`, `for`/`while` /
//! `repeat while` map to unified loops, expression `if`/`switch` become canonical
//! conditionals, and calls / member / index expressions use the shared call shape.
//! `try` and `await` stay source-backed protocol boundaries until semantic
//! contracts can prove those effects are erasable.

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    stable_symbol_hash, Builtin, EvidenceAnchor, EvidenceKind, FileId, Il, Interner, Lang,
    LitClass, LoopKind, NodeId, NodeKind, Op, Payload, RegionKind, SourceGranularity,
    SourceProtocolKind, Span, Symbol, TypeEvidenceKind, UnitBodyKind, UnitDomain, UnitDomains,
    UnitEvidenceFlag, UnitKind, UnitOrigin, UnitSubkind,
};
use nose_semantics::{
    SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER, SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER,
    SWIFT_NIL_LITERAL_CONFORMANCE_MARKER, SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER,
};
use tree_sitter::Node as TsNode;

mod calls;
mod expressions;
mod helpers;
mod items;
mod lambdas;
mod properties;
mod statements;

use self::{
    calls::*, expressions::*, helpers::*, items::*, lambdas::*, properties::*, statements::*,
};

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
) -> anyhow::Result<Il> {
    crate::lower::lower_file(
        file,
        path,
        src,
        interner,
        crate::lower::grammar::SWIFT,
        || tree_sitter_swift::LANGUAGE.into(),
        Lang::Swift,
        lower_items,
    )
}

#[cfg(test)]
mod tests;
