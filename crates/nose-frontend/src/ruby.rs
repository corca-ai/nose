//! Ruby → raw IL lowering.
//!
//! Convergence-friendly lowering: `def` → function unit, `class`/`module` →
//! class-like unit; `if`/`unless`/`while`/`until`/`case` map to `If`/`Loop`;
//! `for x in xs` maps to a `ForEach` loop; `x op= y` desugars to an assignment;
//! method calls → `Field`-call form. Ruby's implicit
//! last-expression return is wrapped in `Return` to converge with explicit returns.

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    FileId, Il, Interner, Lang, LitClass, LoopKind, NodeId, NodeKind, Op, Payload,
    SourceProtocolKind, Span, Symbol, UnitKind,
};
use tree_sitter::Node as TsNode;

mod blocks_calls;
mod control;
mod expressions;
mod statements;

use self::{blocks_calls::*, control::*, expressions::*, statements::*};

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
        crate::lower::grammar::RUBY,
        || tree_sitter_ruby::LANGUAGE.into(),
        Lang::Ruby,
        |lo, root| crate::lower::collect_into(lo, root, NodeKind::Module, lower_stmt),
    )
}

#[cfg(test)]
mod tests;
