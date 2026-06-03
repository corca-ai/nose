//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

use nose_il::{Il, Interner, Lang, NodeId, NodeKind, Symbol, UnitKind};
use nose_normalize::node_tag;

/// A unit ready for comparison. Self-contained (owns its features and location)
/// so the detector can flatten units from many files into one vector. All feature
/// vectors are content-derived hashes (interner-independent), so a `UnitFeat` is
/// portable across runs — which is what lets the CLI cache it by source-content hash.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnitFeat {
    pub path: String,
    pub lang: Lang,
    pub kind: UnitKind,
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub token_count: usize,
    /// Sorted multiset of local shape hashes (syntactic structure).
    pub shapes: Vec<u64>,
    /// Sorted multiset of value-graph (GVN) hashes — the semantic substrate that
    /// is invariant to temporaries, statement order, and common-subexpression
    /// duplication.
    pub value: Vec<u64>,
    /// MinHash signature for candidate generation (over the value graph when
    /// available, else shapes).
    pub minhash: Vec<u64>,
    /// Pre-order node-tag sequence, for alignment scoring.
    pub linear: Vec<u64>,
    /// Sorted multiset of literal (`Const`) value hashes. A high `lits/value`
    /// ratio marks a "data-table" unit (constant-dominated, e.g. a locale map),
    /// where the constants must match for a clone.
    pub lits: Vec<u64>,
    /// Sorted multiset of RETURN-sink value hashes — what the unit returns. True
    /// clones return the same computed values; used to demote near-identical units
    /// that differ only in their result (`<` vs `<=`, an extra effect).
    pub returns: Vec<u64>,
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Upper bound (pre-order node count) for a *block* unit. ~10× the typical
/// fragment clone; bounds the cost of extracting features for every nested block in
/// a very large function/class (which the enclosing unit already covers).
const MAX_BLOCK_TOKENS: usize = 400;

#[inline]
fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(SEED)
}

/// Extract all units of `il` passing the size gates, with features computed.
pub(crate) fn extract(
    il: &Il,
    interner: &Interner,
    seeds: &[u64],
    min_lines: u32,
    min_tokens: usize,
    block_units: bool,
) -> Vec<UnitFeat> {
    // Frontend-tagged functions/methods/classes, and (when enabled) substantial
    // sub-function blocks (loops / ifs / try). The ceiling funnel showed ~56% of
    // gold pairs have a region that is a sub-function block, undetectable unless
    // extracted as its own unit — but block units only pay off once candidate
    // generation can surface them, so they are opt-in.
    let mut roots: Vec<(NodeId, UnitKind, Option<Symbol>)> =
        il.units.iter().map(|u| (u.root, u.kind, u.name)).collect();
    if block_units {
        collect_block_units(il, il.root, &mut roots);
    }

    let mut out = Vec::new();
    for (root, kind, uname) in roots {
        let span = il.node(root).span;
        let lines = span.line_count();

        let mut pre = Vec::new();
        collect_pre(il, root, &mut pre);
        // The value graph is the semantic fingerprint (already sorted), with the
        // literal-only multiset for data-table detection. Computed before the size
        // gate so the gate can consult semantic richness (below).
        let (value, lits, returns) = nose_normalize::value_fingerprint_lits(il, root, interner);

        // Size gate. A short unit normally isn't a meaningful clone — EXCEPT a
        // frontend-tagged function whose body is behaviorally *dense*: a functional
        // one-liner like `return sum(v for v in xs if v>0)` is a real Type-4 clone of a
        // multi-line loop (the value graph converges them to an *identical* fingerprint),
        // just compressed below the line/token gate. Admit such a function when its value
        // fingerprint is rich enough to be matched by the oracle-certified exact-match
        // path (`value.len() >= 6`, the same floor that path uses) — this recovers the
        // compressed functional Type-4 forms without lowering the gate for trivial units
        // (`return x` has 1–2 atoms) or for blocks (kept strict; they are the noisy ones).
        // Blocks share the function gate: measurement showed the real sub-function
        // clones are small (24–40 tokens), so a stricter block gate drops signal
        // (pool-precision 0.106→0.074, AUC 0.42→0.17) faster than noise.
        let dense_fn =
            matches!(kind, UnitKind::Function | UnitKind::Method) && value.len() >= 6;
        if (lines < min_lines || pre.len() < min_tokens) && !dense_fn {
            continue;
        }
        // …but a *huge* block (a whole big method/class body) is not a "fragment"
        // clone — it's covered by its enclosing function/class unit — and extracting
        // features for every nested block of a 700-line class is quadratic. Cap block
        // units well above real fragment clones (≈40 tokens) so only the pathological
        // giants are skipped; functions/methods/classes are never capped.
        if kind == UnitKind::Block && pre.len() > MAX_BLOCK_TOKENS {
            continue;
        }

        let mut shapes = Vec::with_capacity(pre.len());
        let mut linear = Vec::with_capacity(pre.len());
        for &nid in &pre {
            let n = il.node(nid);
            let tag = node_tag(n.kind, n.payload, interner);
            linear.push(tag);
            let mut shape = tag;
            for &c in il.children(nid) {
                let cn = il.node(c);
                shape = combine(shape, node_tag(cn.kind, cn.payload, interner));
            }
            shapes.push(shape);
        }
        shapes.sort_unstable();

        // Candidate generation keys on the value graph when present (so clones
        // that converge only semantically still become candidates).
        let mut distinct = if value.is_empty() {
            shapes.clone()
        } else {
            value.clone()
        };
        distinct.dedup();
        let minhash = crate::minhash::sign(&distinct, seeds);

        out.push(UnitFeat {
            path: il.meta.path.clone(),
            lang: il.meta.lang,
            kind,
            name: uname.map(|s| interner.resolve(s).to_string()),
            start_line: span.start_line,
            end_line: span.end_line,
            token_count: pre.len(),
            shapes,
            value,
            minhash,
            linear,
            lits,
            returns,
        });
    }
    out
}

/// Collect sub-function block roots (loops / ifs / try) as extra unit candidates.
fn collect_block_units(il: &Il, node: NodeId, out: &mut Vec<(NodeId, UnitKind, Option<Symbol>)>) {
    if matches!(il.kind(node), NodeKind::Loop | NodeKind::If | NodeKind::Try) {
        out.push((node, UnitKind::Block, None));
    }
    for &c in il.children(node) {
        collect_block_units(il, c, out);
    }
}

/// Pre-order DFS collecting all descendant node ids of `root` (inclusive).
fn collect_pre(il: &Il, root: NodeId, out: &mut Vec<NodeId>) {
    out.push(root);
    for &c in il.children(root) {
        collect_pre(il, c, out);
    }
}
