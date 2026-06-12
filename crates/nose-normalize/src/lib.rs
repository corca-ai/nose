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
//!
//! proof-obligation: oracle.cutoff

mod algebra;
mod alpha;
mod binding_evidence;
mod call_args;
mod call_target_evidence;
mod cfg_norm;
mod commutative;
mod dataflow;
mod dce;
mod desugar;
mod effect_evidence;
mod idioms;
mod interp;
mod library_api_evidence;
mod literals;
pub mod module_facts;
mod recursion;
mod value_graph;

pub use commutative::{node_tag, node_tag_valued, subtree_hashes, valued_tree_hash};
pub use interp::{
    behavior_has_sym, run_unit, run_unit_paths, Behavior, Value, MAX_SYM_BRANCH_SITES,
};
pub use value_graph::{
    anchor_min_weight, containment_anchor_min_weight, value_anchors, value_fingerprint,
    value_fingerprint_and_contracts, value_fingerprint_and_contracts_with_context,
    value_fingerprint_contracts, value_fingerprint_lits, value_fingerprint_lits_anchors,
    value_fingerprint_lits_anchors_laws, value_fingerprint_lits_anchors_laws_with_context,
    value_fingerprint_lits_anchors_with_context, value_fingerprint_lits_with_context, Anchor,
    Anchors, FingerprintBundle, FingerprintLawBundle, ValueFingerprintContext, ANCHOR_MIN_WEIGHT,
    CONTAINMENT_ANCHOR_MIN_WEIGHT,
};

use nose_il::{FileMeta, Il, IlBuilder, Interner, NodeId, NodeKind, Payload, Symbol, Unit};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;

/// Mixing constant for [`combine`] (the golden-ratio odd constant, as in fxhash).
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Fold a child hash `b` into an accumulator `a` — the one hash combiner shared by
/// every fingerprinting pass (`commutative`/`node_tag`, `algebra`, `value_graph`).
/// It is deliberately a single definition: the structural and value-graph fingerprints
/// only agree across passes if this stays byte-identical, so a lone copy removes the
/// drift risk three parallel copies carried.
#[inline]
pub(crate) fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(SEED)
}

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
            let child_count = self.old.children(old_id).len();
            let mut kids = Vec::with_capacity(child_count);
            for idx in 0..child_count {
                let child = self.old.children(old_id)[idx];
                kids.push(self.go(child));
            }
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

/// Side-effect-free for normalization rewrites that move or drop expressions.
pub(crate) fn is_pure(il: &Il, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Call | NodeKind::HoF | NodeKind::Assign | NodeKind::Throw => false,
        _ => il.children(node).iter().all(|&c| is_pure(il, c)),
    }
}

/// Assemble the rewritten `Il` shared by every rebuild pass: carry each old unit forward to
/// its remapped root (dropping units the pass deleted), copy the file metadata and parameter
/// type/source facts, and finish the builder with the given canonical-id names. Passes differ
/// only in the `cid_names` they preserve (most clone `old.cid_names`; desugar resets to empty
/// because it rewrites canonical ids), so that stays a caller-supplied argument.
pub(crate) fn finalize_rebuild(
    old: &Il,
    remap: &FxHashMap<u32, NodeId>,
    builder: IlBuilder,
    new_root: NodeId,
    cid_names: Vec<Symbol>,
) -> Il {
    let units: Vec<Unit> = old
        .units
        .iter()
        .filter_map(|u| {
            remap.get(&u.root.0).map(|&root| Unit {
                root,
                kind: u.kind,
                name: u.name,
            })
        })
        .collect();
    let meta = FileMeta {
        path: old.meta.path.clone(),
        lang: old.meta.lang,
    };
    let mut out = builder.finish(new_root, meta, units, cid_names);
    out.evidence = old.evidence.clone();
    out
}

/// A control-flow terminator: a statement that unconditionally diverts control out of the
/// straight-line flow, so any code after it in the same block is unreachable. Shared by DCE
/// (dead-code-after-terminator) and desugar (guard-clause then-block termination).
pub(crate) fn is_terminator(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
    )
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

struct NormalizeTimer<'a> {
    enabled: bool,
    path: &'a str,
    last: Instant,
}

impl<'a> NormalizeTimer<'a> {
    fn new(path: &'a str) -> Self {
        Self {
            enabled: std::env::var_os("NOSE_TIME_NORMALIZE").is_some(),
            path,
            last: Instant::now(),
        }
    }

    fn lap(&mut self, pass: &str) {
        if self.enabled {
            let now = Instant::now();
            eprintln!(
                "  [normalize] {:<12} {:>7.1}ms  {}",
                pass,
                now.duration_since(self.last).as_secs_f64() * 1e3,
                self.path,
            );
            self.last = now;
        }
    }
}

/// Normalize one lowered file, returning a fresh canonical [`Il`] (the input is
/// left untouched). Unit roots are remapped onto the new arena.
pub fn normalize(il: &Il, interner: &Interner, opts: &NormalizeOptions) -> Il {
    let mut timer = NormalizeTimer::new(&il.meta.path);
    let mut out = desugar::run(il, interner, opts);
    timer.lap("desugar");
    effect_evidence::run(&mut out, interner);
    timer.lap("effect-evidence");
    binding_evidence::run(&mut out, interner);
    timer.lap("binding");
    library_api_evidence::run(&mut out, interner);
    timer.lap("api-evidence");
    call_target_evidence::run(&mut out, interner);
    timer.lap("call-target");
    alpha::run(&mut out);
    timer.lap("alpha");
    if opts.oracle {
        // Behavior-preserving structural core only — the soundness oracle reads this so it
        // is independent of every semantic canonicalization below.
        debug_assert!(out.validate().is_ok());
        return out;
    }
    // Recursion → iteration: a SEMANTIC canon, so it runs after the oracle cutoff above
    // (the oracle interprets the original recursion to validate this rewrite). First in the
    // phase, so the loops it emits flow through dataflow / cfg-norm / the value graph and
    // converge with hand-written iteration.
    out = recursion::run(&out);
    timer.lap("recursion");
    if opts.dataflow {
        out = dataflow::run(&out);
        timer.lap("dataflow");
    }
    if opts.dce {
        out = dce::run(&out);
        timer.lap("dce");
    }
    if opts.cfg_norm {
        out = cfg_norm::structure(&out);
        timer.lap("cfg-structure");
    }
    out = algebra::run(&out, interner);
    timer.lap("algebra");
    if opts.cfg_norm {
        cfg_norm::run(&mut out, interner);
        timer.lap("cfg-orient");
    }
    effect_evidence::run(&mut out, interner);
    timer.lap("effect-evidence-final");
    library_api_evidence::run(&mut out, interner);
    timer.lap("api-evidence-final");
    // IR-verifier discipline: in debug/test builds, assert the rewrite pipeline
    // produced a structurally well-formed arena. Free in release.
    debug_assert!(
        out.validate().is_ok(),
        "normalized IL failed validation: {}",
        out.validate().unwrap_err()
    );
    out
}
