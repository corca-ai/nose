//! Normalization passes that rewrite raw IL into canonical IL, so that
//! semantically-equivalent code from any frontend converges to (near-)identical
//! structure. Pipeline:
//!
//! 1. `desugar` — structural rebuild: C-style loops → `while`, idiom
//!    canonicalization, `x.length` → `Len`, block flattening, else-after-return.
//! 2. `alpha` — canonical identifier ids (alpha-equivalence).
//! 3. `dataflow` — copy/expression propagation (single-use temp inlining).
//! 4. `cfg_norm::structure` — conjoined-guard merge + continue-guard unwrap.
//! 5. `algebra` — algebraic canonicalization (assoc/comm flatten, comparison
//!    direction, De Morgan; subsumes commutative operand sorting).
//! 6. `cfg_norm::run` — orient `if` branches (when `cfg_norm` is enabled).

mod algebra;
mod alpha;
mod cfg_norm;
mod commutative;
mod dataflow;
mod dce;
mod desugar;
mod idioms;
mod interp;
mod literals;
mod types;
mod value_graph;

pub use commutative::{node_tag, node_tag_valued, subtree_hashes};
pub use interp::{run_unit, Behavior, Value};
pub use value_graph::{value_fingerprint, value_fingerprint_lits};

use nose_il::{Il, IlBuilder, Interner, NodeId, NodeKind, Payload};
use rustc_hash::FxHashSet;

/// Copy a node from `old` into `b` with the given (already-rebuilt) children,
/// preserving its kind/payload/span. The rebuild passes' identical "generic" node
/// copy delegates here (the pass-specific part is only the child recursion).
pub(crate) fn rebuild_like(b: &mut IlBuilder, old: &Il, old_id: NodeId, kids: &[NodeId]) -> NodeId {
    let n = *old.node(old_id);
    b.add(n.kind, n.payload, n.span, kids)
}

/// The `generic` node-copy method every rewrite pass needs verbatim: recurse into the
/// children via the pass's own `go`, then [`rebuild_like`]. A macro rather than a trait
/// because the body borrows the pass's `self.old` and `self.b` fields *disjointly* — which
/// trait accessor methods can't express (they'd each borrow all of `self`) — while `go` is
/// the only pass-specific part. Each pass writes `crate::rebuild_generic!();` in its impl.
macro_rules! rebuild_generic {
    () => {
        fn generic(&mut self, old_id: NodeId) -> NodeId {
            let kids: Vec<NodeId> = self
                .old
                .children(old_id)
                .to_vec()
                .iter()
                .map(|&c| self.go(c))
                .collect();
            crate::rebuild_like(&mut self.b, self.old, old_id, &kids)
        }
    };
}
pub(crate) use rebuild_generic;

/// Summarize the scope rooted at `node`: the canonical-ids of its parameters, the
/// variables it assigns (`defs`, keyed by the LHS `Var`'s node id), and the nested
/// function nodes it contains. Recursion stops at each nested `Func` (a new scope),
/// pushing it to `nested` instead of descending. Shared by the dataflow and DCE
/// passes, which both need this exact per-scope summary before rewriting.
pub(crate) fn collect_scope(
    il: &Il,
    node: NodeId,
    is_root: bool,
    defs: &mut FxHashSet<u32>,
    params: &mut FxHashSet<u32>,
    nested: &mut Vec<NodeId>,
) {
    let kind = il.kind(node);
    if kind == NodeKind::Func && !is_root {
        nested.push(node);
        return; // scope boundary
    }
    if kind == NodeKind::Param {
        if let Payload::Cid(c) = il.node(node).payload {
            params.insert(c);
        }
    }
    if kind == NodeKind::Assign {
        if let Some(&lhs) = il.children(node).first() {
            if il.kind(lhs) == NodeKind::Var {
                defs.insert(lhs.0);
            }
        }
    }
    for &c in il.children(node) {
        collect_scope(il, c, false, defs, params, nested);
    }
}

/// Knobs for the normalization pipeline.
#[derive(Clone, Copy, Debug)]
pub struct NormalizeOptions {
    /// Enable control-flow normalization (else-after-return flattening + branch
    /// orientation). On by default; `--no-cfg-norm` turns it off for ablation.
    pub cfg_norm: bool,
    /// Enable dataflow copy/expression propagation (single-use temp inlining).
    pub dataflow: bool,
    /// Enable dead-code / dead-assignment elimination.
    pub dce: bool,
    /// Oracle mode: stop after the structural, behavior-preserving core (desugar + alpha)
    /// and skip every SEMANTIC canonicalization (dataflow, dce, cfg_norm, algebra). The
    /// soundness oracle interprets THIS form so its behavioral ground truth is independent
    /// of the canon layer — a canonicalization that changes behavior (e.g. wrongly sorting
    /// non-commutative `a or b`) is then caught, instead of being masked by interpreting
    /// the already-canonicalized IL that both operands collapsed into.
    pub oracle: bool,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        NormalizeOptions {
            cfg_norm: true,
            dataflow: true,
            dce: false,
            oracle: false,
        }
    }
}

/// Normalize one lowered file, returning a fresh canonical [`Il`] (the input is
/// left untouched). Unit roots are remapped onto the new arena.
pub fn normalize(il: &Il, interner: &Interner, opts: &NormalizeOptions) -> Il {
    let mut out = desugar::run(il, interner, opts);
    alpha::run(&mut out);
    if opts.oracle {
        // Behavior-preserving structural core only — the soundness oracle reads this so it
        // is independent of every semantic canonicalization below.
        debug_assert!(out.validate().is_ok());
        return out;
    }
    if opts.dataflow {
        out = dataflow::run(&out);
    }
    if opts.dce {
        out = dce::run(&out);
    }
    if opts.cfg_norm {
        out = cfg_norm::structure(&out);
    }
    out = algebra::run(&out, interner);
    if opts.cfg_norm {
        cfg_norm::run(&mut out, interner);
    }
    // IR-verifier discipline: in debug/test builds, assert the rewrite pipeline
    // produced a structurally well-formed arena. Free in release.
    debug_assert!(
        out.validate().is_ok(),
        "normalized IL failed validation: {}",
        out.validate().unwrap_err()
    );
    out
}
