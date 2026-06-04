//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

use nose_il::{Il, Interner, Lang, LitClass, NodeId, NodeKind, Payload, Symbol, UnitKind};
use nose_normalize::node_tag;
use rustc_hash::{FxHashMap, FxHashSet};

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
    /// MinHash signature over `shapes`, used by the Type-3 near-duplicate channel.
    pub shape_minhash: Vec<u64>,
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
    /// Whether the value fingerprint is safe to use as a strict semantic proof.
    ///
    /// `semantic` mode must not report units whose fingerprint passed through lossy
    /// lowering (`Raw`, abstract literals such as JS regex, or opaque calls). Those
    /// units can still participate in `near` via structural scoring.
    #[serde(default)]
    pub exact_safe: bool,
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Upper bound (pre-order node count) for a *block* unit. ~10× the typical
/// fragment clone; bounds the cost of extracting features for every nested block in
/// a very large function/class (which the enclosing unit already covers).
const MAX_BLOCK_TOKENS: usize = 400;
const EXACT_VALUE_MIN: usize = 4;

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

    let facts = StrictFacts::collect(il, interner);
    let mut out = Vec::new();
    for (root, kind, uname) in roots {
        let span = il.node(root).span;
        let lines = span.line_count();

        let mut pre = Vec::new();
        collect_pre(il, root, &mut pre);
        let exact_safe = strict_exact_safe_tree(il, interner, &facts, root);
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
        // path (`value.len() >= 4`, the same floor that path uses) — this recovers the
        // compressed functional Type-4 forms without lowering the gate for trivial units
        // (`return x` has 1–2 atoms) or for blocks (kept strict; they are the noisy ones).
        // Blocks share the function gate: measurement showed the real sub-function
        // clones are small (24–40 tokens), so a stricter block gate drops signal
        // (pool-precision 0.106→0.074, AUC 0.42→0.17) faster than noise.
        let dense_fn =
            matches!(kind, UnitKind::Function | UnitKind::Method) && value.len() >= EXACT_VALUE_MIN;
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
        let mut distinct_shapes = shapes.clone();
        distinct_shapes.dedup();
        let shape_minhash = crate::minhash::sign(&distinct_shapes, seeds);

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
            shape_minhash,
            value,
            minhash,
            linear,
            lits,
            returns,
            exact_safe,
        });
    }
    out
}

#[derive(Default)]
struct StrictFacts {
    immutable_names: FxHashSet<Symbol>,
    function_names: FxHashSet<Symbol>,
}

impl StrictFacts {
    fn collect(il: &Il, interner: &Interner) -> Self {
        let mut facts = StrictFacts::default();
        facts.collect_immutable_bindings(il);
        facts.collect_function_bindings(il, interner);
        facts
    }

    fn proven_name(&self, name: Symbol) -> bool {
        self.immutable_names.contains(&name) || self.function_names.contains(&name)
    }

    fn collect_immutable_bindings(&mut self, il: &Il) {
        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for stmt in top_level_statements(il) {
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }

        let mut env: FxHashSet<u32> = FxHashSet::default();
        for stmt in top_level_statements(il) {
            let kids = il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            if immutable_binding_safe(il, &env, &self.immutable_names, kids[1]) {
                self.immutable_names.insert(name);
                if let Payload::Cid(cid) = il.node(kids[0]).payload {
                    env.insert(cid);
                }
            }
        }
    }

    fn collect_function_bindings(&mut self, il: &Il, interner: &Interner) {
        for unit in &il.units {
            let Some(name) = unit.name else {
                continue;
            };
            if function_binding_safe(il, interner, self, unit.root, unit.root) {
                self.function_names.insert(name);
            }
        }
    }
}

fn top_level_statements(il: &Il) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &stmt in il.children(il.root) {
        if il.kind(stmt) == NodeKind::Block {
            out.extend(il.children(stmt).iter().copied());
        } else {
            out.push(stmt);
        }
    }
    out
}

fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let kids = il.children(stmt);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return None;
    }
    let Payload::Cid(cid) = il.node(kids[0]).payload else {
        return None;
    };
    il.cid_names.get(cid as usize).copied()
}

fn immutable_binding_safe(
    il: &Il,
    env: &FxHashSet<u32>,
    immutable_names: &FxHashSet<Symbol>,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::Call
        | NodeKind::HoF
        | NodeKind::Func
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw
        | NodeKind::Assign => false,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => env.contains(&c),
            Payload::Name(s) => immutable_names.contains(&s),
            _ => false,
        },
        NodeKind::Lit => exact_literal_safe(il, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| immutable_binding_safe(il, env, immutable_names, c)),
    }
}

fn function_binding_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    root: NodeId,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::HoF
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw => false,
        NodeKind::Func if node != root => false,
        NodeKind::Call => matches!(il.node(node).payload, Payload::Builtin(_)),
        NodeKind::Seq => strict_exact_safe_seq(il, interner, node),
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| function_binding_safe(il, interner, facts, root, c)),
    }
}

fn strict_exact_safe_tree(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Raw => false,
        NodeKind::Seq => {
            strict_exact_safe_seq(il, interner, node)
                && il
                    .children(node)
                    .iter()
                    .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
        }
        NodeKind::Call => strict_exact_safe_call(il, interner, facts, node),
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c)),
    }
}

fn exact_literal_safe(il: &Il, node: NodeId) -> bool {
    matches!(
        il.node(node).payload,
        Payload::LitInt(_)
            | Payload::LitBool(_)
            | Payload::LitStr(_)
            | Payload::LitFloat(_)
            | Payload::Lit(LitClass::Null)
    )
}

fn strict_exact_safe_var(il: &Il, facts: &StrictFacts, node: NodeId) -> bool {
    match il.node(node).payload {
        Payload::Cid(_) => true,
        Payload::Name(name) => facts.proven_name(name),
        _ => false,
    }
}

fn strict_exact_safe_seq(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.node(node).payload {
        Payload::None => true,
        Payload::Name(name) => matches!(
            interner.resolve(name),
            "array"
                | "list"
                | "tuple"
                | "dictionary"
                | "hash"
                | "array_expression"
                | "tuple_expression"
                | "object"
                | "pair"
                | "import_binding"
                | "import_namespace"
        ),
        _ => false,
    }
}

fn strict_exact_safe_call(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    if matches!(il.node(node).payload, Payload::Builtin(_)) {
        return il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c));
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if strict_exact_callee_name(il, interner, callee, "typeof") {
        return strict_exact_call_args_safe(il, interner, facts, node);
    }
    if il.kind(callee) != NodeKind::Field {
        return strict_exact_callee_identity(il, facts, callee)
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(name);
    if method == "test" {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        return matches!(il.node(receiver).payload, Payload::LitStr(_))
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    if method == "isArray" {
        return strict_exact_field_receiver_name(il, interner, callee, "Array")
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    if matches!(
        method,
        "iter" | "into_iter" | "iter_mut" | "collect" | "to_vec" | "copied" | "cloned"
    ) {
        return strict_exact_call_args_safe(il, interner, facts, node);
    }
    strict_exact_callee_identity(il, facts, callee)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_call_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node)
        .iter()
        .skip(1)
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn strict_exact_field_receiver_name(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    expected: &str,
) -> bool {
    let Some(&receiver) = il.children(field).first() else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(receiver).payload else {
        return false;
    };
    interner.resolve(name) == expected
}

fn strict_exact_callee_name(il: &Il, interner: &Interner, callee: NodeId, expected: &str) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    interner.resolve(name) == expected
}

fn strict_exact_callee_identity(il: &Il, facts: &StrictFacts, callee: NodeId) -> bool {
    match il.kind(callee) {
        NodeKind::Var => strict_exact_safe_var(il, facts, callee),
        NodeKind::Field => {
            matches!(il.node(callee).payload, Payload::Name(_))
                && il
                    .children(callee)
                    .first()
                    .is_some_and(|&receiver| strict_exact_callee_identity(il, facts, receiver))
        }
        _ => false,
    }
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
