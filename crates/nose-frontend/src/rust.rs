//! Rust → raw IL lowering.
//!
//! Convergence-friendly lowering: `&x`/`&mut x`/`*x` references peel to the
//! operand; `x op= y` desugars to an assignment; `for`/`while`/`loop` map to the
//! unified `Loop`; `match` becomes an `if`/`else if` chain (arm pattern as the
//! condition); `if let`/`while let` preserve their pattern tests. Rust `?`,
//! `.await`, and `async {}` stay as source-backed protocol boundaries until
//! contracts prove their error/async semantics.
//! `fn` items become function units; `impl`,
//! `trait`, `struct`, `enum` become class-like units so similar types cluster.

use crate::lower::Lowering;
use nose_il::{
    DomainEvidence, FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
    RegionKind, SourceCallKind, SourceFactKind, SourceGranularity, SourcePatternKind,
    SourceProtocolKind, SourceRangeKind, Span, Symbol, UnitBodyKind, UnitDomain, UnitDomains,
    UnitEvidenceFlag, UnitKind, UnitOrigin, UnitSubkind,
};
use tree_sitter::Node as TsNode;

mod expressions;
mod functions;
mod items;
mod macros;
mod matches;
mod statements;

use self::{expressions::*, functions::*, items::*, macros::*, matches::*, statements::*};

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
        crate::lower::grammar::RUST,
        || tree_sitter_rust::LANGUAGE.into(),
        Lang::Rust,
        lower_items,
    )
}

#[cfg(test)]
mod tests;
