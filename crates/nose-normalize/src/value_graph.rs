//! Track 1 Stage 3 — value-graph / global value numbering (GVN).
//!
//! Symbolically evaluates a function unit into a DAG of *values*, hash-consed by
//! `(op, operand-value-ids)`. Because a variable maps to the value it currently
//! holds (not its name), and identical computations intern to one node:
//!
//! - temporaries and intermediate names dissolve (`t=a+b; …t…` ≡ inline),
//! - common subexpressions share a node (CSE),
//! - the order of data-independent statements stops mattering,
//! - commutative operands are canonical.
//!
//! Branches merge variables with `Phi(cond, then, else)`. Loops are approximated:
//! variables written in the body become opaque loop values (no fixpoint — bounded
//! and deterministic). Calls/stores are treated as values too (fuzzy: identical
//! calls CSE). The per-unit **fingerprint** is the multiset of value-node hashes
//! reachable from the unit's sinks (returns, throws, branch conditions, effects).
//!
//! This is a *detection substrate*, not an IL rewrite: it returns a fingerprint
//! the detector can use instead of (or alongside) subtree shapes.
//!
//! proof-obligation: normalize.control_flow.guard_returns
//! proof-obligation: normalize.value_graph.algebra
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.clamp
//! proof-obligation: normalize.value_graph.compare
//! proof-obligation: normalize.value_graph.field_writes
//! proof-obligation: normalize.value_graph.free_monoid
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

mod rules;

use crate::combine;
use crate::module_facts::{
    assignment_name_in_scope, collect_all_node_symbols_in_scope, collect_module_mutations_in_scope,
    local_scope_nodes, mutating_method_name, node_symbol_in_scope,
    shadowed_js_like_module_binding_nodes_for_symbol_in_scope, top_level_statements_for,
};
use crate::types::Ty;
use nose_il::{
    stable_symbol_hash, Builtin, HoFKind, Il, Interner, Lang, LoopKind, NodeId, NodeKind, Op,
    ParamSemantic, Payload, Symbol, UnitKind,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::OnceLock;

const LARGE_AC_EXPR_OPERANDS: usize = 64;

/// The [`ValOp::Call`] tag for a known builtin: its discriminant + 1. The `+1` reserves tag
/// `0` for an opaque (non-builtin) callee — see [`ValOp::Call`]'s definition. Centralizes
/// that encoding so the reserved-zero invariant lives in one place.
fn builtin_tag(b: Builtin) -> u32 {
    b as u32 + 1
}

/// Public entry: the value-graph fingerprint of the unit rooted at `root`
/// (sorted multiset of `u64` value hashes). Equivalent computations → equal
/// multisets.
pub fn value_fingerprint(il: &Il, root: NodeId, interner: &Interner) -> Vec<u64> {
    value_fingerprint_lits(il, root, interner).0
}

/// Like [`value_fingerprint`], but also returns (1) the sorted multiset of literal
/// (`Const`) value hashes — for "data-table" detection — and (2) the sorted multiset
/// of RETURN-sink value hashes — what the unit actually computes/returns, for a
/// return-signature match (true clones return the same values).
pub fn value_fingerprint_lits(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    b.fingerprint_lits()
}

/// The default minimum sub-computation size (in value-nodes) for a node to be an extractable
/// anchor. Below this a shared sub-DAG is a common idiom (`x+1`, `len(xs)`), not a refactor.
pub const ANCHOR_MIN_WEIGHT: u32 = 20;

/// Heavy sub-DAG anchor hashes of a unit — see `Builder::anchors`. Two units sharing a (rare)
/// anchor share an extractable sub-computation: a partial / sub-DAG clone.
pub fn value_anchors(il: &Il, root: NodeId, interner: &Interner) -> Vec<u64> {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    b.anchors(ANCHOR_MIN_WEIGHT)
}

/// `value_fingerprint_lits` plus the unit's heavy sub-DAG anchors, all from ONE value-graph
/// build (anchors share the build, so adding them is free vs. fingerprinting alone).
pub fn value_fingerprint_lits_anchors(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    let (v, l, r) = b.fingerprint_lits();
    let a = b.anchors(ANCHOR_MIN_WEIGHT);
    (v, l, r, a)
}

/// Context-shared variant of [`value_fingerprint_lits_anchors`].
pub fn value_fingerprint_lits_anchors_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner).with_shared_subtree_hashes(&context.subtree_hashes);
    b.build_unit_with_context(root, Some(context));
    let (v, l, r) = b.fingerprint_lits();
    let a = b.anchors(ANCHOR_MIN_WEIGHT);
    (v, l, r, a)
}

/// File-level facts that are independent of the unit currently being fingerprinted.
///
/// `units::extract` may fingerprint hundreds of block units from the same large file.
/// Function binding proofs require scanning every function and building a
/// literal-sensitive subtree hash for the whole IL, and opaque raw/lambda values need a
/// structural subtree hash for the same file. Doing either once per unit turns a
/// file-level proof into the dominant cost. This context keeps the reusable proof result
/// and lazily shares structural subtree hashes. Each per-unit builder still interns
/// the corresponding lambda values into its own value arena, so value ids never cross
/// builder boundaries.
pub struct ValueFingerprintContext {
    module: ModuleSeedContext,
    function_bindings: Vec<(Symbol, u64)>,
    subtree_hashes: OnceLock<Vec<u64>>,
}

impl ValueFingerprintContext {
    pub fn new(il: &Il, interner: &Interner) -> Self {
        let module = ModuleSeedContext::new(il, interner);
        let subtree_hashes = OnceLock::new();
        let function_bindings = {
            let mut b = Builder::new(il, interner).with_shared_subtree_hashes(&subtree_hashes);
            b.seed_module_value_bindings_from_context(&module, None);
            b.collect_function_binding_hashes()
        };
        Self {
            module,
            function_bindings,
            subtree_hashes,
        }
    }
}

struct ModuleSeedContext {
    local_scope: Vec<bool>,
    top_level: Vec<NodeId>,
    assignment_counts: FxHashMap<Symbol, usize>,
    assignment_deps: FxHashMap<Symbol, FxHashSet<Symbol>>,
    mutated_bindings: FxHashSet<Symbol>,
    unit_symbols: FxHashSet<Symbol>,
}

impl ModuleSeedContext {
    fn new(il: &Il, interner: &Interner) -> Self {
        let local_scope = local_scope_nodes(il);
        let top_level = top_level_statements_for(il);
        let mut is_top_level = vec![false; il.nodes.len()];
        for &stmt in &top_level {
            if let Some(slot) = is_top_level.get_mut(stmt.0 as usize) {
                *slot = true;
            }
        }

        let mut assignment_counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for &stmt in &top_level {
            if let Some(name) = assignment_name_in_scope(il, stmt, &local_scope) {
                *assignment_counts.entry(name).or_insert(0) += 1;
            }
        }
        let mut assignment_deps: FxHashMap<Symbol, FxHashSet<Symbol>> = FxHashMap::default();
        for &stmt in &top_level {
            let Some(name) = assignment_name_in_scope(il, stmt, &local_scope) else {
                continue;
            };
            if let Some(&rhs) = il.children(stmt).get(1) {
                let mut deps = FxHashSet::default();
                collect_all_node_symbols_in_scope(il, rhs, &local_scope, &mut deps);
                assignment_deps.insert(name, deps);
            }
        }

        let unit_symbols: FxHashSet<Symbol> =
            il.units.iter().filter_map(|unit| unit.name).collect();
        let candidate_names: FxHashSet<Symbol> = assignment_counts
            .iter()
            .filter_map(|(&name, &count)| {
                (count == 1 && !unit_symbols.contains(&name)).then_some(name)
            })
            .collect();
        let mutated_bindings = collect_module_mutations_in_scope(
            il,
            interner,
            &candidate_names,
            &is_top_level,
            &local_scope,
        );

        Self {
            local_scope,
            top_level,
            assignment_counts,
            assignment_deps,
            mutated_bindings,
            unit_symbols,
        }
    }

    fn required_bindings_for(&self, il: &Il, root: NodeId) -> FxHashSet<Symbol> {
        let mut required = FxHashSet::default();
        collect_all_node_symbols_in_scope(il, root, &self.local_scope, &mut required);
        let mut stack: Vec<Symbol> = required.iter().copied().collect();
        while let Some(name) = stack.pop() {
            let Some(deps) = self.assignment_deps.get(&name) else {
                continue;
            };
            for &dep in deps {
                if self.assignment_counts.contains_key(&dep) && required.insert(dep) {
                    stack.push(dep);
                }
            }
        }
        required
    }
}

pub fn value_fingerprint_lits_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner).with_shared_subtree_hashes(&context.subtree_hashes);
    b.build_unit_with_context(root, Some(context));
    b.fingerprint_lits()
}

/// The pointer-length contracts the unit relied on to converge: deduped, sorted
/// `(array_param_pos, length_param_pos)` pairs. The behavioral oracle binds
/// `args[length_pos] = len(args[array_pos])` for each, so it interprets the unit under the
/// SAME `n = len(array)` convention the value graph used to merge it. Empty when none.
pub fn value_fingerprint_contracts(il: &Il, root: NodeId, interner: &Interner) -> Vec<(u32, u32)> {
    value_fingerprint_and_contracts(il, root, interner).1
}

/// Both the value fingerprint AND the pointer-length contracts from a SINGLE build — the
/// behavioral oracle needs both per unit, and building the value graph twice (once for each)
/// doubled the per-unit cost.
pub fn value_fingerprint_and_contracts(
    il: &Il,
    root: NodeId,
    interner: &Interner,
) -> (Vec<u64>, Vec<(u32, u32)>) {
    let mut b = Builder::new(il, interner);
    b.build_unit(root);
    let fp = b.fingerprint_lits().0;
    b.contracts.sort_unstable();
    b.contracts.dedup();
    (fp, b.contracts)
}

type ValueId = u32;

#[derive(Clone, PartialEq, Eq, Hash)]
enum ValOp {
    Input(u32),      // a parameter or free variable, keyed by canonical id
    Const(u32),      // literal class
    Bin(u32),        // binary operator
    Un(u32),         // unary operator
    Field(u64),      // field access, keyed by content hash of the name
    Index,           // base[index]
    Call(u32),       // 0 = opaque callee; otherwise builtin discriminant + 1
    Hof(u32),        // higher-order op kind
    Clamp,           // numeric clamp over proven integer bounds: args = [x, lo, hi]
    Seq(u64),        // aggregate literal, keyed by lowered sequence kind
    CollectionParam, // proven collection parameter, distinct from map-like key membership
    ArrayParam,      // proven array parameter, distinct from receiver-provided collections
    StringParam,     // proven string parameter, distinct from collection emptiness
    Phi,             // branch merge: args = [cond, then, else]
    Lambda(u64),     // opaque, keyed by a structural hash of the lambda body
    Loop(u32),       // loop-carried opaque value, keyed by canonical id
    Elem(u64),       // an element drawn from an iterable, keyed by the iterable's hash
    Idx(u64),        // the iteration index into a collection (range/while/enumerate)
    Reduce(u32),     // canonical fold over elements: args = [init, per-element contrib]
    Formula(u64),    // compact canonical hash for a very large generated expression
    Recurrence(u64), // compact non-reduction loop-carried update, keyed by full RHS hash
    Opaque(u64),     // anything not otherwise modeled, keyed by a tag (counter or subtree hash)
}

struct ValNode {
    op: ValOp,
    args: Vec<ValueId>,
}

#[derive(Clone, Copy)]
enum SinkKind {
    Return = 0,
    Cond = 1,
    Effect = 2,
    Break = 3,
    Throw = 4,
}

#[derive(Clone, Copy)]
struct Sink {
    kind: SinkKind,
    value: ValueId,
    effect_ord: Option<u32>,
}

impl Sink {
    fn new(kind: SinkKind, value: ValueId) -> Self {
        Self {
            kind,
            value,
            effect_ord: None,
        }
    }

    fn ordered_effect(value: ValueId, effect_ord: u32) -> Self {
        Self {
            kind: SinkKind::Effect,
            value,
            effect_ord: Some(effect_ord),
        }
    }
}

struct Builder<'a> {
    il: &'a Il,
    interner: &'a Interner,
    nodes: Vec<ValNode>,
    /// Structural hash per value node, kept in lockstep with `nodes`.
    vhash: Vec<u64>,
    intern: FxHashMap<(ValOp, Vec<ValueId>), ValueId>,
    sinks: Vec<Sink>,
    opaque_ctr: u32,
    /// Object field writes (`self.x = v`), keyed by field name → its CURRENT value
    /// (last-write-wins). Flushed to sinks at the end as one (field, final-value) sink
    /// each. This makes the fingerprint depend on the *final per-field state* — order-
    /// insensitive across DISTINCT fields (so two constructors that assign the same
    /// fields in swapped order converge, as they must: distinct field writes commute),
    /// yet correct for same-field overwrites (`x=1;x=2` ≠ `x=2;x=1` — last value wins).
    /// The old order-independent effect multiset got BOTH wrong (it split commuting
    /// swaps — false split vs the oracle — and merged same-field overwrites — unsound).
    field_env: FxHashMap<u64, ValueId>,
    /// Lazily-computed subtree hash per IL node (kind + payload + children), used to
    /// key unlowered `Raw` constructs and lambda bodies by content. Computed once per
    /// graph (the whole-IL pass is O(n)); `None` until first needed.
    subtree_hash: Option<Vec<u64>>,
    /// Shared file-level subtree hashes supplied by [`ValueFingerprintContext`]. This
    /// keeps contextual per-unit builders from recomputing the same whole-file pass.
    shared_subtree_hashes: Option<&'a OnceLock<Vec<u64>>>,
    /// Literal-sensitive subtree hash used only for proven function binding identity.
    /// The ordinary structural hash intentionally abstracts literals for shape work;
    /// callee identity must distinguish `helper(x)+1` from `helper(x)+2`.
    valued_subtree_hash: Option<Vec<u64>>,
    /// Inferred coarse type per value node (kept in lockstep with `nodes`). Powers
    /// type-aware canonicalization: `+` commutes only on numeric operands (string/list
    /// concat is non-commutative), and numeric/boolean simplifications fire only when
    /// proven. `Unknown` is the safe default — no type-gated rewrite fires on it.
    vty: Vec<Ty>,
    /// Inferred parameter types by position (from `types::infer_param_types`), seeding the
    /// type of each `Input` node.
    param_ty: Vec<Ty>,
    /// Explicit source-level parameter semantic facts keyed by the alpha-renamed cid
    /// currently in scope.
    param_semantic: FxHashMap<u32, ParamSemantic>,
    /// The branch conditions currently in effect (each a `cond` or `Not(cond)`). A
    /// `return`/`throw` reached under a non-empty path is tagged with that condition,
    /// so `if c {return A} else {return B}` and the branch-swapped `if c {return B}
    /// else {return A}` produce *different* fingerprints (path-sensitive returns).
    path: Vec<ValueId>,
    /// Active facts of the form `lo <= hi` established by dominating guard clauses.
    /// These are scoped like `path`: a fact learned from `if hi < lo { throw ... }`
    /// applies only to the fallthrough suffix of that block, and is truncated when the
    /// block returns to its caller. Literal integer bounds are proved on demand instead.
    bound_order_facts: Vec<(ValueId, ValueId)>,
    /// Ordered statement-effect slot for the current control-flow path. Alternative
    /// `if` branches start from the same slot, then join at the max consumed slot, so
    /// branch-source order does not matter while sequential effects still do.
    effect_slot: u32,
    /// Active list-builder variables during a loop body (`r = []; for x: r.append(f(x))`):
    /// cid → `Some((contrib, guard))` for a single clean per-element append, or `None` once
    /// spoiled (a second append, multi-arg append, or other use). On loop exit a clean
    /// builder's value becomes `Hof(Map, [contrib])` — the same node the comprehension
    /// `[f(x) for x in xs]` / `.map`/`.collect` builds, so the two converge.
    building: FxHashMap<u32, Option<(ValueId, Option<ValueId>)>>,
    /// Strictly captured module/global constants, keyed by their original symbol.
    /// Function units keep global references as `Name`, while module-level assignment
    /// targets are alpha-renamed to `Cid`; this map reconnects safe top-level literal
    /// data (`const table = {...}`) to free uses inside the function.
    global_env: FxHashMap<Symbol, ValueId>,
    /// Interprocedural inline registry: `name → (param cids, body)` for PURE, file-local,
    /// uniquely-named functions (`function_binding_safe`: no effects, loops, throws, lambdas, or
    /// user calls). A call to such a function is inlined to its body's value (β-reduction over a
    /// pure function — sound), so `f(args)` ≡ the extracted helper's body. Built once per unit
    /// build (after binding seeding, before body eval). Pure bodies have no user calls, so an
    /// inlined body never triggers further inlining — single-level, no cycles, no depth bound.
    inline_fns: FxHashMap<Symbol, (Vec<u32>, NodeId)>,
    /// Nodes under function/lambda scopes use local cid numbering. Their `Cid(0)` is not
    /// the module `cid_names[0]`, so module-symbol resolution fails closed there.
    local_scope_nodes: Vec<bool>,
    /// Current loop-carried placeholders while evaluating a loop body. Used only to
    /// compact coupled recurrences such as `s1 += f(s2); s2 += g(s1)`, which otherwise
    /// expand into a large raw expression DAG even though they are not clean reductions.
    loop_recurrence: Option<LoopRecurrenceScope>,
    next_loop_key_base: u32,
    /// Pointer-length contracts the unit RELIED ON to converge: `(array_param_pos,
    /// length_param_pos)` pairs recorded wherever `full_pointer_length_contract` fired (the
    /// loop bound `n` was treated as `len(array)`, not data, and dropped from the
    /// fingerprint). The behavioral oracle must interpret such a unit under the SAME contract
    /// — binding `n = len(array)` — else it tests the function on inputs the contract forbids
    /// (`n ≠ len`) and reports a spurious false merge. Gated this way so the binding only
    /// fires where the value graph actually used the contract (it cannot mask a non-contract
    /// false merge). Sorted+deduped on read for determinism.
    contracts: Vec<(u32, u32)>,
    /// Internal test counters for clamp canonicalization. They record clamp-shaped min/max
    /// nodes seen by `mk`, and the subset with a unique integer-domain `lo <= hi` proof.
    clamp_candidate_count: usize,
    clamp_proof_backed_candidate_count: usize,
}

#[derive(Clone)]
struct LoopRecurrenceScope {
    loop_values: FxHashMap<u32, ValueId>,
    loop_keys: FxHashMap<u32, u32>,
    loop_key_set: FxHashSet<u32>,
}

#[derive(Clone, Copy)]
struct SignedExprOperand {
    expr: NodeId,
    negated: bool,
}

#[derive(Default)]
struct ReductionCache {
    reductions: FxHashMap<(ValueId, ValueId), Option<(u32, ValueId)>>,
    references: FxHashMap<(ValueId, ValueId), bool>,
}

#[derive(Clone, Copy)]
enum FilterMapResult {
    Emit {
        value: ValueId,
        predicate: Option<ValueId>,
    },
    Drop,
}

impl<'a> Builder<'a> {
    fn new(il: &'a Il, interner: &'a Interner) -> Self {
        Builder {
            il,
            interner,
            nodes: Vec::new(),
            vhash: Vec::new(),
            intern: FxHashMap::default(),
            sinks: Vec::new(),
            opaque_ctr: 0,
            field_env: FxHashMap::default(),
            subtree_hash: None,
            shared_subtree_hashes: None,
            valued_subtree_hash: None,
            vty: Vec::new(),
            param_ty: Vec::new(),
            param_semantic: FxHashMap::default(),
            path: Vec::new(),
            bound_order_facts: Vec::new(),
            effect_slot: 0,
            building: FxHashMap::default(),
            global_env: FxHashMap::default(),
            inline_fns: FxHashMap::default(),
            local_scope_nodes: local_scope_nodes(il),
            loop_recurrence: None,
            next_loop_key_base: 0,
            contracts: Vec::new(),
            clamp_candidate_count: 0,
            clamp_proof_backed_candidate_count: 0,
        }
    }

    fn with_shared_subtree_hashes(mut self, hashes: &'a OnceLock<Vec<u64>>) -> Self {
        self.shared_subtree_hashes = Some(hashes);
        self
    }

    fn vty(&self, v: ValueId) -> Ty {
        self.vty.get(v as usize).copied().unwrap_or(Ty::Unknown)
    }

    /// Bottom-up coarse type of a fresh node from its op and already-typed operands.
    fn ty_of(&self, op: &ValOp, args: &[ValueId]) -> Ty {
        let at = |i: usize| args.get(i).map(|&a| self.vty(a)).unwrap_or(Ty::Unknown);
        match op {
            ValOp::Const(k) => const_ty(*k),
            ValOp::Input(k) => self
                .param_ty
                .get(*k as usize)
                .copied()
                .unwrap_or(Ty::Unknown),
            ValOp::Bin(o) => {
                let o = *o;
                if o == Op::Add as u32 {
                    let (a, b) = (at(0), at(1));
                    if a == Ty::Num && b == Ty::Num {
                        Ty::Num
                    } else if a == Ty::Str || b == Ty::Str {
                        Ty::Str
                    } else if a == Ty::List || b == Ty::List {
                        Ty::List
                    } else {
                        Ty::Unknown
                    }
                } else if matches!(
                    o,
                    x if x == Op::Sub as u32 || x == Op::Mul as u32 || x == Op::Div as u32
                        || x == Op::Mod as u32 || x == Op::Pow as u32 || x == Op::BitAnd as u32
                        || x == Op::BitOr as u32 || x == Op::BitXor as u32
                        || x == Op::Shl as u32 || x == Op::Shr as u32
                        || x == MIN_CODE || x == MAX_CODE
                ) {
                    Ty::Num
                } else if matches!(
                    o,
                    x if x == Op::Lt as u32 || x == Op::Le as u32 || x == Op::Gt as u32
                        || x == Op::Ge as u32 || x == Op::Eq as u32 || x == Op::Ne as u32
                        || x == Op::In as u32
                ) || ((o == Op::And as u32 || o == Op::Or as u32)
                    && at(0) == Ty::Bool
                    && at(1) == Ty::Bool)
                {
                    Ty::Bool
                } else {
                    Ty::Unknown
                }
            }
            ValOp::Un(o) => {
                let o = *o;
                if o == Op::Neg as u32
                    || o == Op::Pos as u32
                    || o == Op::BitNot as u32
                    || o == ABS_CODE
                {
                    Ty::Num
                } else if o == Op::Not as u32 {
                    Ty::Bool
                } else {
                    Ty::Unknown
                }
            }
            ValOp::Seq(_) | ValOp::CollectionParam | ValOp::ArrayParam => Ty::List,
            ValOp::Clamp => Ty::Num,
            ValOp::StringParam => Ty::Str,
            ValOp::Call(tag)
                if matches!(
                    *tag,
                    x if x == builtin_tag(Builtin::IsEmpty)
                        || x == builtin_tag(Builtin::StartsWith)
                        || x == builtin_tag(Builtin::EndsWith)
                        || x == builtin_tag(Builtin::Contains)
                        || x == JS_PROTOTYPE_IN_CODE
                ) =>
            {
                Ty::Bool
            }
            _ => Ty::Unknown,
        }
    }

    /// Flush accumulated object-field writes to sinks: one (field-name, final-value)
    /// sink per distinct field, in canonical name order. See `field_env`.
    fn flush_fields(&mut self) {
        let mut entries: Vec<(u64, ValueId)> = self.field_env.drain().collect();
        entries.sort_unstable_by_key(|(k, _)| *k);
        for (name, v) in entries {
            let f = self.mk(ValOp::Field(name), vec![v]);
            self.sinks.push(Sink::new(SinkKind::Effect, f));
        }
    }

    /// Content hash of an IL subtree (surface kind + payload + children), cached for the
    /// whole graph. Used to key unlowered constructs by *what they are* rather than by
    /// position — so two behaviorally-different `Raw` nodes stay DISTINCT.
    fn subtree_hash(&mut self, expr: NodeId) -> u64 {
        if let Some(shared) = self.shared_subtree_hashes {
            return shared
                .get_or_init(|| crate::subtree_hashes(self.il, self.interner))
                .get(expr.0 as usize)
                .copied()
                .unwrap_or(0);
        }
        if self.subtree_hash.is_none() {
            self.subtree_hash = Some(crate::subtree_hashes(self.il, self.interner));
        }
        self.subtree_hash
            .as_ref()
            .unwrap()
            .get(expr.0 as usize)
            .copied()
            .unwrap_or(0)
    }

    fn valued_subtree_hash(&mut self, expr: NodeId) -> u64 {
        if self.valued_subtree_hash.is_none() {
            let mut hashes = vec![0u64; self.il.nodes.len()];
            for i in 0..self.il.nodes.len() {
                let id = NodeId(i as u32);
                let node = self.il.node(id);
                let mut h = crate::node_tag_valued(node.kind, node.payload, self.interner);
                for &child in self.il.children(id) {
                    h = combine(h, hashes[child.0 as usize]);
                }
                hashes[i] = h;
            }
            self.valued_subtree_hash = Some(hashes);
        }
        self.valued_subtree_hash
            .as_ref()
            .unwrap()
            .get(expr.0 as usize)
            .copied()
            .unwrap_or(0)
    }

    fn source_salted_hash(&mut self, expr: NodeId, tag: u64) -> u64 {
        let span = self.il.node(expr).span;
        let mut h = combine(tag, self.valued_subtree_hash(expr));
        h = combine(h, span.file.0 as u64);
        h = combine(h, span.start_byte as u64);
        h = combine(h, span.end_byte as u64);
        h = combine(h, span.start_line as u64);
        combine(h, span.end_line as u64)
    }

    fn is_unproven_membership_like_call(&self, expr: NodeId, kids: &[NodeId]) -> bool {
        if matches!(self.il.node(expr).payload, Payload::Builtin(_)) {
            return false;
        }
        let Some(&callee) = kids.first() else {
            return false;
        };
        if self.il.kind(callee) != NodeKind::Field {
            return false;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return false;
        };
        matches!(
            self.interner.resolve(name),
            "Contains"
                | "contains"
                | "containsKey"
                | "containsValue"
                | "contains_key"
                | "contains_value"
                | "has"
                | "has_key?"
                | "include?"
                | "includes"
                | "key?"
                | "member?"
                | "__contains__"
        )
    }

    fn param_semantic_for_param(&self, param: NodeId) -> Option<ParamSemantic> {
        let span = self.il.node(param).span;
        self.il
            .param_type_facts
            .iter()
            .find(|fact| fact.span == span)
            .map(|fact| fact.semantic)
    }

    fn seed_param_semantics(&mut self, root: NodeId) {
        let scope = self.param_semantic_scope(root).unwrap_or(root);
        for &k in self.il.children(scope) {
            if self.il.kind(k) != NodeKind::Param {
                continue;
            }
            if let (Payload::Cid(cid), Some(semantic)) =
                (self.il.node(k).payload, self.param_semantic_for_param(k))
            {
                self.param_semantic.insert(cid, semantic);
            }
        }
    }

    fn param_semantic_scope(&self, root: NodeId) -> Option<NodeId> {
        if self.il.kind(root) == NodeKind::Func {
            return Some(root);
        }
        let root_span = self.il.node(root).span;
        let mut best: Option<(u32, NodeId)> = None;
        for (idx, node) in self.il.nodes.iter().enumerate() {
            if node.kind != NodeKind::Func {
                continue;
            }
            let span = node.span;
            if span.start_byte > root_span.start_byte || span.end_byte < root_span.end_byte {
                continue;
            }
            let width = span.end_byte.saturating_sub(span.start_byte);
            if best.is_none_or(|(best_width, _)| width < best_width) {
                best = Some((width, NodeId(idx as u32)));
            }
        }
        best.map(|(_, node)| node)
    }

    fn param_semantic_of_expr(&self, expr: NodeId) -> Option<ParamSemantic> {
        if self.il.kind(expr) != NodeKind::Var {
            return None;
        }
        let Payload::Cid(cid) = self.il.node(expr).payload else {
            return None;
        };
        self.param_semantic.get(&cid).copied()
    }

    fn is_collection_param_expr(&self, expr: NodeId) -> bool {
        matches!(
            self.param_semantic_of_expr(expr),
            Some(ParamSemantic::Collection)
        )
    }

    fn is_map_param_expr(&self, expr: NodeId) -> bool {
        matches!(self.param_semantic_of_expr(expr), Some(ParamSemantic::Map))
    }

    fn is_number_param_expr(&self, expr: NodeId) -> bool {
        matches!(
            self.param_semantic_of_expr(expr),
            Some(ParamSemantic::Integer | ParamSemantic::Number)
        )
    }

    fn is_map_param_value(&self, value: ValueId) -> bool {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return false;
        };
        matches!(self.param_semantic.get(&cid), Some(ParamSemantic::Map))
    }

    fn is_array_param_value(&self, value: ValueId) -> bool {
        match self.nodes[value as usize].op {
            ValOp::ArrayParam => true,
            ValOp::Input(cid) => {
                matches!(self.param_semantic.get(&cid), Some(ParamSemantic::Array))
            }
            _ => false,
        }
    }

    fn param_domain_value(&mut self, value: ValueId) -> ValueId {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return value;
        };
        match self.param_semantic.get(&cid).copied() {
            Some(ParamSemantic::Array) => self.mk(ValOp::ArrayParam, vec![value]),
            Some(ParamSemantic::Collection) => self.mk(ValOp::CollectionParam, vec![value]),
            Some(ParamSemantic::String) => self.mk(ValOp::StringParam, vec![value]),
            _ => value,
        }
    }

    fn is_js_like_lang(&self) -> bool {
        matches!(
            self.il.meta.lang,
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
        )
    }

    fn free_name_input_key(&self, name: &str) -> u32 {
        let sym = self.interner.intern(name);
        0x8000_0000u32 | (self.interner.symbol_hash(sym) as u32)
    }

    fn is_free_name_value(&self, value: ValueId, name: &str) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Input(key) if key == self.free_name_input_key(name)
        )
    }

    fn proven_set_constructor_collection(&self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        if !self.is_free_name_value(node.args[0], "Set") {
            return None;
        }
        let collection = node.args[1];
        if !matches!(self.nodes[collection as usize].op, ValOp::Seq(1)) {
            return None;
        }
        Some(collection)
    }

    fn proven_java_collection_factory_value(&mut self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Java {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() < 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        let receiver = callee.args[0];
        let is_standard_factory = if method == stable_symbol_hash("of") {
            self.is_free_java_std_name(receiver, "List")
                || self.is_free_java_std_name(receiver, "Set")
        } else if method == stable_symbol_hash("asList") {
            self.is_free_java_std_name(receiver, "Arrays")
        } else {
            false
        };
        if !is_standard_factory {
            return None;
        }
        // A single argument to a varargs collection factory (`Arrays.asList(x)`,
        // `List.of(x)`, `Set.of(x)`) is ambiguous: when `x` is an array it is spread
        // into the element list, but when `x` is a single object it is the sole
        // element. The two readings have different membership semantics
        // (`value` in the array elements vs `value.equals(x)`), so a single argument
        // can only be canonicalized when the receiver is a proven array. Otherwise we
        // must refuse, or an array-typed field and a list-typed field of the same name
        // would false-merge. Multi-argument factories are always a literal element list.
        if args.len() == 2 {
            if method == stable_symbol_hash("asList") && self.is_array_param_value(args[1]) {
                return Some(self.mk(ValOp::ArrayParam, vec![args[1]]));
            }
            return None;
        }
        Some(self.mk(ValOp::Seq(1), args[1..].to_vec()))
    }

    fn proven_python_collection_factory_value(&self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Python {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        let builtin = ["list", "set", "frozenset", "tuple"]
            .into_iter()
            .any(|name| self.is_free_name_value(args[0], name) && !self.file_defines_name(name));
        let imported_stdlib_factory = self.is_import_binding_value(args[0], "collections", "deque");
        if !builtin && !imported_stdlib_factory {
            return None;
        }
        let collection = args[1];
        if !matches!(self.nodes[collection as usize].op, ValOp::Seq(1)) {
            return None;
        }
        Some(collection)
    }

    fn proven_ruby_set_factory_value(&self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Ruby
            || !self.ruby_file_requires_module("set")
            || self.file_defines_name("Set")
        {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        if !matches!(callee.op, ValOp::Field(method) if method == stable_symbol_hash("new"))
            || callee.args.len() != 1
            || !self.is_free_name_value(callee.args[0], "Set")
        {
            return None;
        }
        matches!(self.nodes[args[1] as usize].op, ValOp::Seq(1)).then_some(args[1])
    }

    fn proven_rust_vec_macro_collection_value(&mut self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Rust {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        if !self.is_free_name_value(args[0], "vec") || self.file_defines_name("vec") {
            return None;
        }
        Some(self.mk(ValOp::Seq(1), args[1..].to_vec()))
    }

    fn proven_rust_std_collection_factory_value(&self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Rust {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        if !self.is_free_name_value(args[0], "std::collections::HashSet::from")
            && !self.is_free_name_value(args[0], "std::collections::BTreeSet::from")
            && !self.is_free_name_value(args[0], "std::collections::VecDeque::from")
        {
            return None;
        }
        matches!(self.nodes[args[1] as usize].op, ValOp::Seq(1)).then_some(args[1])
    }

    fn is_free_java_std_name(&self, value: ValueId, name: &str) -> bool {
        self.is_free_name_value(value, name) && !self.java_file_defines_type_name(name)
    }

    fn is_import_namespace_expr(
        &mut self,
        expr: NodeId,
        module: &str,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let value = self.eval(expr, env);
        self.is_import_namespace_value(value, module)
    }

    fn is_import_namespace_value(&self, value: ValueId, module: &str) -> bool {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(6)) || node.args.len() != 1 {
            return false;
        }
        matches!(
            self.nodes[node.args[0] as usize].op,
            ValOp::Const(k) if k == stable_string_const_key(module)
        )
    }

    fn is_import_binding_value(&self, value: ValueId, module: &str, exported: &str) -> bool {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(5)) || node.args.len() != 2 {
            return false;
        }
        matches!(
            self.nodes[node.args[0] as usize].op,
            ValOp::Const(k) if k == stable_string_const_key(module)
        ) && matches!(
            self.nodes[node.args[1] as usize].op,
            ValOp::Const(k) if k == stable_string_const_key(exported)
        )
    }

    fn file_imports_namespace(&self, expr: NodeId, module: &str) -> bool {
        if self.il.meta.lang != Lang::Go {
            return false;
        }
        let Some(alias) = self.node_symbol(expr) else {
            return false;
        };
        self.top_level_statements().iter().any(|&stmt| {
            if self.assignment_name(stmt) != Some(alias) {
                return false;
            }
            let kids = self.il.children(stmt);
            if kids.len() != 2 || self.il.kind(kids[1]) != NodeKind::Seq {
                return false;
            }
            let Payload::Name(seq_name) = self.il.node(kids[1]).payload else {
                return false;
            };
            if self.interner.resolve(seq_name) != "import_namespace" {
                return false;
            }
            let Some(&module_node) = self.il.children(kids[1]).first() else {
                return false;
            };
            matches!(
                self.il.node(module_node).payload,
                Payload::LitStr(hash) if hash == stable_symbol_hash(module)
            )
        })
    }

    fn java_file_defines_type_name(&self, name: &str) -> bool {
        if self.il.meta.lang != Lang::Java {
            return false;
        }
        self.il.units.iter().any(|unit| {
            unit.kind == UnitKind::Class
                && unit
                    .name
                    .is_some_and(|symbol| self.interner.resolve(symbol) == name)
        })
    }

    fn file_defines_name(&self, name: &str) -> bool {
        self.top_level_statements().iter().any(|&stmt| {
            self.assignment_name(stmt)
                .is_some_and(|symbol| self.interner.resolve(symbol) == name)
        }) || self.il.units.iter().any(|unit| {
            unit.name
                .is_some_and(|symbol| self.interner.resolve(symbol) == name)
        })
    }

    fn ruby_file_requires_module(&self, module: &str) -> bool {
        if self.il.meta.lang != Lang::Ruby {
            return false;
        }
        let expected = stable_symbol_hash(module);
        self.top_level_statements().iter().any(|&stmt| {
            let expr = if self.il.kind(stmt) == NodeKind::ExprStmt {
                self.il.children(stmt).first().copied()
            } else {
                Some(stmt)
            };
            let Some(call) = expr else {
                return false;
            };
            if self.il.kind(call) != NodeKind::Call {
                return false;
            }
            let kids = self.il.children(call);
            if kids.len() != 2 || self.il.kind(kids[0]) != NodeKind::Var {
                return false;
            }
            let Payload::Name(callee) = self.il.node(kids[0]).payload else {
                return false;
            };
            if self.interner.resolve(callee) != "require" {
                return false;
            }
            matches!(self.il.node(kids[1]).payload, Payload::LitStr(hash) if hash == expected)
        })
    }

    fn proven_collection_value(&mut self, value: ValueId) -> Option<ValueId> {
        if matches!(self.nodes[value as usize].op, ValOp::Seq(1)) {
            return Some(value);
        }
        if matches!(self.nodes[value as usize].op, ValOp::Seq(2))
            || (self.il.meta.lang == Lang::Python
                && matches!(self.nodes[value as usize].op, ValOp::Seq(0)))
        {
            let items = self.nodes[value as usize].args.clone();
            return Some(self.mk(ValOp::Seq(1), items));
        }
        self.proven_set_constructor_collection(value)
            .or_else(|| self.proven_java_collection_factory_value(value))
            .or_else(|| self.proven_python_collection_factory_value(value))
            .or_else(|| self.proven_ruby_set_factory_value(value))
            .or_else(|| self.proven_rust_vec_macro_collection_value(value))
            .or_else(|| self.proven_rust_std_collection_factory_value(value))
    }

    fn proven_collection_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        self.proven_collection_value(value)
            .or_else(|| self.proven_local_collection_binding_value(expr, env))
    }

    fn proven_local_collection_binding_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(expr) != NodeKind::Var {
            return None;
        }
        let Payload::Cid(cid) = self.il.node(expr).payload else {
            return None;
        };
        if self.local_binding_mutated(cid) {
            return None;
        }
        let rhs = self.local_binding_initializer(cid)?;
        if self.node_contains_cid(rhs, cid) {
            return None;
        }
        let value = self.eval(rhs, env);
        self.proven_collection_value(value)
    }

    fn local_binding_initializer(&self, cid: u32) -> Option<NodeId> {
        let mut rhs = None;
        for (idx, node) in self.il.nodes.iter().enumerate() {
            if node.kind != NodeKind::Assign {
                continue;
            }
            let assign = NodeId(idx as u32);
            let kids = self.il.children(assign);
            if kids.len() != 2 {
                continue;
            }
            if self.node_refers_to_cid(kids[0], cid) {
                if rhs.is_some() {
                    return None;
                }
                rhs = Some(kids[1]);
            } else if self.node_contains_cid(kids[0], cid) {
                return None;
            }
        }
        rhs
    }

    fn local_binding_mutated(&self, cid: u32) -> bool {
        self.il
            .nodes
            .iter()
            .enumerate()
            .any(|(idx, node)| match node.kind {
                NodeKind::Call => self
                    .call_mutates_cid(NodeId(idx as u32), cid)
                    .unwrap_or(false),
                NodeKind::Field => self
                    .field_mutates_cid(NodeId(idx as u32), cid)
                    .unwrap_or(false),
                _ => false,
            })
    }

    fn call_mutates_cid(&self, call: NodeId, cid: u32) -> Option<bool> {
        if !matches!(
            self.il.node(call).payload,
            Payload::Builtin(Builtin::Append)
        ) {
            return Some(false);
        }
        let receiver = self.il.children(call).first().copied()?;
        Some(self.node_refers_to_cid(receiver, cid))
    }

    fn field_mutates_cid(&self, field: NodeId, cid: u32) -> Option<bool> {
        let Payload::Name(method) = self.il.node(field).payload else {
            return Some(false);
        };
        if !mutating_method_name(self.interner.resolve(method)) {
            return Some(false);
        }
        let receiver = self.il.children(field).first().copied()?;
        Some(self.node_refers_to_cid(receiver, cid))
    }

    fn proven_map_constructor_entries(&mut self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        if !self.is_free_name_value(args[0], "Map") {
            return None;
        }
        let entries_node = &self.nodes[args[1] as usize];
        if !matches!(entries_node.op, ValOp::Seq(1)) {
            return None;
        }
        let entries = entries_node.args.clone();
        let mut canonical_entries = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry_node = &self.nodes[entry as usize];
            if !matches!(entry_node.op, ValOp::Seq(1)) || entry_node.args.len() != 2 {
                return None;
            }
            let kv = entry_node.args.clone();
            canonical_entries.push(self.mk(ValOp::Seq(4), kv));
        }
        Some(self.mk(ValOp::Seq(3), canonical_entries))
    }

    fn proven_java_map_factory_entries(&mut self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Java {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 || !self.is_free_java_std_name(callee.args[0], "Map") {
            return None;
        }
        if method == stable_symbol_hash("of") {
            let entries = &args[1..];
            if entries.len() % 2 != 0 {
                return None;
            }
            let mut canonical_entries = Vec::with_capacity(entries.len() / 2);
            for kv in entries.chunks(2) {
                canonical_entries.push(self.mk(ValOp::Seq(4), kv.to_vec()));
            }
            return Some(self.mk(ValOp::Seq(3), canonical_entries));
        }
        if method == stable_symbol_hash("ofEntries") {
            let mut canonical_entries = Vec::with_capacity(args.len().saturating_sub(1));
            for entry in args.iter().skip(1).copied() {
                let kv = self.proven_java_map_entry_pair(entry)?;
                canonical_entries.push(self.mk(ValOp::Seq(4), kv));
            }
            return Some(self.mk(ValOp::Seq(3), canonical_entries));
        }
        None
    }

    fn proven_java_map_entry_pair(&self, value: ValueId) -> Option<Vec<ValueId>> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 3 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        if !matches!(callee.op, ValOp::Field(name) if name == stable_symbol_hash("entry"))
            || callee.args.len() != 1
            || !self.is_free_java_std_name(callee.args[0], "Map")
        {
            return None;
        }
        Some(args[1..].to_vec())
    }

    fn proven_rust_std_map_factory_entries(&mut self, value: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Rust {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        if !self.is_free_name_value(args[0], "std::collections::HashMap::from")
            && !self.is_free_name_value(args[0], "std::collections::BTreeMap::from")
        {
            return None;
        }
        let entries_node = &self.nodes[args[1] as usize];
        if !matches!(entries_node.op, ValOp::Seq(1)) {
            return None;
        }
        let entries = entries_node.args.clone();
        let mut canonical_entries = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry_node = &self.nodes[entry as usize];
            if !matches!(entry_node.op, ValOp::Seq(2)) || entry_node.args.len() != 2 {
                return None;
            }
            canonical_entries.push(self.mk(ValOp::Seq(4), entry_node.args.clone()));
        }
        Some(self.mk(ValOp::Seq(3), canonical_entries))
    }

    fn proven_go_literal_zero_map_value(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(tag) if tag == stable_symbol_hash("go_literal_zero_map"))
            || node.args.len() != 2
        {
            return None;
        }
        Some((node.args[1], node.args[0]))
    }

    fn proven_go_literal_zero_map_seq(
        &mut self,
        expr: NodeId,
        args: &[ValueId],
    ) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Go {
            return None;
        }
        let Payload::Name(name) = self.il.node(expr).payload else {
            return None;
        };
        if self.interner.resolve(name) != "composite_literal" || args.is_empty() {
            return None;
        }
        let entry_nodes = self.il.children(expr).to_vec();
        if entry_nodes.len() != args.len() {
            return None;
        }
        let mut canonical_entries = Vec::with_capacity(args.len());
        let mut value_kind = None;
        let mut default = None;
        for (&entry_node_id, &entry_value) in entry_nodes.iter().zip(args.iter()) {
            if self.il.kind(entry_node_id) != NodeKind::Seq {
                return None;
            }
            let Payload::Name(entry_name) = self.il.node(entry_node_id).payload else {
                return None;
            };
            if self.interner.resolve(entry_name) != "keyed_element" {
                return None;
            }
            let kv_nodes = self.il.children(entry_node_id);
            if kv_nodes.len() != 2
                || !matches!(self.il.node(kv_nodes[0]).payload, Payload::LitStr(_))
            {
                return None;
            }
            let (kind, value_default) =
                self.go_literal_zero_default_from_payload(self.il.node(kv_nodes[1]).payload)?;
            match value_kind {
                Some(current_kind) if current_kind != kind => return None,
                Some(_) => {}
                None => {
                    value_kind = Some(kind);
                    default = Some(value_default);
                }
            }
            let entry_value_node = &self.nodes[entry_value as usize];
            if !matches!(entry_value_node.op, ValOp::Seq(tag) if tag == stable_symbol_hash("keyed_element"))
                || entry_value_node.args.len() != 2
            {
                return None;
            }
            canonical_entries.push(self.mk(ValOp::Seq(4), entry_value_node.args.clone()));
        }
        let map = self.mk(ValOp::Seq(3), canonical_entries);
        Some(self.mk(
            ValOp::Seq(stable_symbol_hash("go_literal_zero_map")),
            vec![default?, map],
        ))
    }

    fn go_literal_zero_default_from_payload(&mut self, payload: Payload) -> Option<(u8, ValueId)> {
        match payload {
            Payload::LitInt(_) => Some((1, self.int_const(0))),
            Payload::LitStr(_) => Some((
                2,
                self.mk(ValOp::Const(stable_string_const_key("")), vec![]),
            )),
            Payload::LitBool(_) => Some((3, self.mk(ValOp::Const(0x3000_0001), vec![]))),
            Payload::LitFloat(_) => Some((
                4,
                self.mk(ValOp::Const(stable_float_const_key("0.0")), vec![]),
            )),
            Payload::Lit(nose_il::LitClass::Null) => Some((5, self.null_const())),
            _ => None,
        }
    }

    fn proven_map_value(&mut self, value: ValueId) -> Option<ValueId> {
        if matches!(self.nodes[value as usize].op, ValOp::Seq(3)) {
            return Some(value);
        }
        self.proven_map_constructor_entries(value)
            .or_else(|| self.proven_java_map_factory_entries(value))
            .or_else(|| self.proven_rust_std_map_factory_entries(value))
            .or_else(|| {
                self.proven_go_literal_zero_map_value(value)
                    .map(|(map, _)| map)
            })
    }

    fn proven_map_get_value(&mut self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[node.args[0] as usize];
        if !matches!(callee.op, ValOp::Field(name) if name == stable_symbol_hash("get"))
            || callee.args.len() != 1
        {
            return None;
        }
        let map = callee.args[0];
        let map = if self.is_map_param_value(map) {
            map
        } else {
            self.proven_map_value(map)?
        };
        Some((map, args[1]))
    }

    fn proven_map_key_view_value(&mut self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) {
            return None;
        }
        let args = node.args.clone();
        if args.len() == 1 {
            let callee = &self.nodes[args[0] as usize];
            if !matches!(callee.op, ValOp::Field(name) if name == stable_symbol_hash("keys"))
                || callee.args.len() != 1
            {
                return None;
            }
            let map = callee.args[0];
            return if self.is_map_param_value(map) {
                Some(map)
            } else {
                self.proven_map_value(map)
            };
        }
        if args.len() == 2 {
            let callee = &self.nodes[args[0] as usize];
            if !matches!(callee.op, ValOp::Field(name) if name == stable_symbol_hash("from"))
                || callee.args.len() != 1
                || !self.is_free_name_value(callee.args[0], "Array")
            {
                return None;
            }
            return self.proven_map_key_view_value(args[1]);
        }
        None
    }

    fn proven_map_key_view_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        self.proven_map_key_view_value(value)
    }

    fn eval_proven_collection_membership_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let &callee = kids.first()?;
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let callee_kids = self.il.children(callee);
        let receiver = callee_kids.first().copied();

        if matches!(
            method,
            "contains" | "includes" | "include?" | "member?" | "__contains__" | "has"
        ) && kids.len() == 2
        {
            let receiver = receiver?;
            let element = self.eval(kids[1], env);
            if let Some(map) = self.proven_map_key_view_expr(receiver, env) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, map]));
            }
            if self.is_collection_param_expr(receiver) {
                let collection = self.eval_membership_collection(receiver, env);
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            let receiver_value = self.eval(receiver, env);
            if let Some(collection) = self
                .proven_collection_value(receiver_value)
                .or_else(|| self.proven_local_collection_binding_value(receiver, env))
            {
                let collection = self.canonical_membership_collection_value(collection);
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            None
        } else if method == "Contains" && kids.len() == 3 {
            let receiver = receiver?;
            if !self.is_import_namespace_expr(receiver, "slices", env)
                && !self.file_imports_namespace(receiver, "slices")
            {
                return None;
            }
            let element = self.eval(kids[2], env);
            let collection = if self.is_collection_param_expr(kids[1]) {
                self.eval_membership_collection(kids[1], env)
            } else {
                self.proven_collection_expr(kids[1], env)?
            };
            let collection = self.canonical_membership_collection_value(collection);
            Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]))
        } else {
            None
        }
    }

    fn eval_proven_map_key_membership_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        let callee = kids[0];
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        let map_specific_method =
            matches!(method, "containsKey" | "contains_key" | "key?" | "has_key?");
        if !matches!(method, "has" | "__contains__") && !map_specific_method {
            return None;
        }
        let receiver = self.il.children(callee).first().copied()?;
        let key = self.eval(kids[1], env);
        let receiver_value = self.eval(receiver, env);
        let map = if self.is_map_param_expr(receiver) {
            receiver_value
        } else {
            self.proven_map_value(receiver_value)
                .or_else(|| map_specific_method.then_some(receiver_value))?
        };
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![key, map]))
    }

    fn eval_proven_map_get_default_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 3 {
            return None;
        }
        let callee = kids[0];
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        if self.interner.resolve(name) != "get" {
            return None;
        }
        let receiver = self.il.children(callee).first().copied()?;
        let receiver_value = self.eval(receiver, env);
        let map = if self.is_map_param_expr(receiver) {
            receiver_value
        } else {
            self.proven_map_value(receiver_value)?
        };
        let key = self.eval(kids[1], env);
        let default = self.eval(kids[2], env);
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    fn eval_proven_numeric_method_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        #[derive(Clone, Copy)]
        enum NumericMethod {
            Abs,
            Min(NodeId),
            Max(NodeId),
            Clamp(NodeId, NodeId),
        }

        let &callee = kids.first()?;
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        let numeric_op = match (method, kids.len()) {
            ("abs", 1) => Some(NumericMethod::Abs),
            ("min", 2) => kids.get(1).copied().map(NumericMethod::Min),
            ("max", 2) => kids.get(1).copied().map(NumericMethod::Max),
            ("clamp", 3) => Some(NumericMethod::Clamp(kids[1], kids[2])),
            _ => None,
        }?;
        let receiver = self.il.children(callee).first().copied()?;
        let receiver_value = self.eval_proven_numeric_expr(receiver, env)?;
        match numeric_op {
            NumericMethod::Abs => Some(self.mk(ValOp::Un(ABS_CODE), vec![receiver_value])),
            NumericMethod::Min(rhs) => {
                let rhs = self.eval(rhs, env);
                Some(self.mk(ValOp::Bin(MIN_CODE), vec![receiver_value, rhs]))
            }
            NumericMethod::Max(rhs) => {
                let rhs = self.eval(rhs, env);
                Some(self.mk(ValOp::Bin(MAX_CODE), vec![receiver_value, rhs]))
            }
            NumericMethod::Clamp(lo, hi) => {
                let lo = self.eval(lo, env);
                let hi = self.eval(hi, env);
                self.proof_backed_clamp_value(receiver_value, lo, hi)
            }
        }
    }

    fn eval_proven_numeric_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        if self.il.kind(expr) == NodeKind::Var {
            return self.is_number_param_expr(expr).then_some(value);
        }
        (self.vty(value) == Ty::Num).then_some(value)
    }

    /// Push an effect sink, tagged with the current path condition — so a *conditional*
    /// effect (`if c { append(x) }`) carries `c`, the way a guarded return does.
    fn push_effect(&mut self, v: ValueId) {
        let ord = self.next_effect_ordinal();
        let g = self.guarded(v);
        self.sinks.push(Sink::ordered_effect(g, ord));
    }

    fn next_effect_ordinal(&mut self) -> u32 {
        let ord = self.effect_slot;
        self.effect_slot = self.effect_slot.saturating_add(1);
        ord
    }

    fn emit_throw(&mut self, v: ValueId) {
        let g = self.guarded(v);
        self.sinks.push(Sink::new(SinkKind::Throw, g));
    }

    /// Tag a value with the current path condition: under branch conditions, the
    /// returned/thrown value is `Phi(path, v, ⊥)` (a sentinel for "not on this path"),
    /// so two branches that return swapped values no longer form the same multiset.
    /// Push a `Return` sink for value `v`, DECOMPOSING a ternary return into guarded
    /// returns. `return (a if c else b)` is behaviorally `if c {return a} else {return b}`,
    /// so we split a `Phi(c, a, b)` return into a `c`-guarded return of `a` and a
    /// `¬c`-guarded return of `b` — exactly the sink set the if-else / elif writing already
    /// produces via guard-clause path narrowing. Recursing on nested `Phi` makes a nested
    /// ternary converge with an `elif` cascade. Sound (behavior-preserving) and gated by the
    /// `verify` oracle; the abs/min/max idiom recognition runs first in `mk`, so a recognized
    /// `Abs`/`Min`/`Max` return is NOT a bare `Phi` here and stays atomic. Only genuine
    /// ternaries (both arms real values, not the `bot` placeholder) are decomposed.
    fn emit_return(&mut self, v: ValueId) {
        if let ValOp::Phi = self.nodes[v as usize].op {
            let args = self.nodes[v as usize].args.clone();
            if args.len() == 3 {
                let bot = self.mk(ValOp::Const(0x3000_0000), vec![]);
                let (cond, then_v, else_v) = (args[0], args[1], args[2]);
                if then_v != bot && else_v != bot {
                    self.path.push(cond);
                    self.emit_return(then_v);
                    self.path.pop();
                    let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
                    self.path.push(ncond);
                    self.emit_return(else_v);
                    self.path.pop();
                    return;
                }
            }
        }
        let g = self.guarded(v);
        self.sinks.push(Sink::new(SinkKind::Return, g));
    }

    fn guarded(&mut self, v: ValueId) -> ValueId {
        let mut pc: Option<ValueId> = None;
        for &c in &self.path.clone() {
            pc = Some(match pc {
                None => c,
                Some(p) => self.mk(ValOp::Bin(Op::And as u32), vec![p, c]),
            });
        }
        match pc {
            None => v,
            Some(pc) => {
                let bot = self.mk(ValOp::Const(0x3000_0000), vec![]);
                self.mk(ValOp::Phi, vec![pc, v, bot])
            }
        }
    }

    fn mk(&mut self, mut op: ValOp, mut args: Vec<ValueId>) -> ValueId {
        if let ValOp::Bin(opc) = op {
            // Canonicalize comparison DIRECTION: `a > b` ≡ `b < a`, `a >= b` ≡ `b <= a`.
            // Reduce the >/>= family to </<= with swapped operands so a guard converges
            // however it was written or negated (`0 < v`, `v > 0`, `!(v <= 0)` all become
            // one node). Language-agnostic and sound (total order). This is what lets a
            // `reduce(λa,v: a+v if v>0 else a, …)` fold converge with its loop, whose
            // guard may lower to the mirror comparison.
            if args.len() == 2 {
                if opc == Op::Gt as u32 {
                    op = ValOp::Bin(Op::Lt as u32);
                    args.swap(0, 1);
                } else if opc == Op::Ge as u32 {
                    op = ValOp::Bin(Op::Le as u32);
                    args.swap(0, 1);
                }
            }
            // Canonicalize commutative operands by structural hash. `+` commutes UNLESS an
            // operand is PROVEN string/list — concat is non-commutative (`s + x` ≠ `x + s`)
            // and the free-monoid oracle distinguishes the orders. Unknown operands keep
            // commuting (optimistic, and the oracle still checks it), so the common untyped
            // numeric case is unaffected; only known-concat is held ordered. Other
            // commutative ops Err on non-numeric regardless of order, so stay safe.
            if let ValOp::Bin(o) = op {
                let concat = o == Op::Add as u32
                    && args.len() == 2
                    && (is_concat_ty(self.vty(args[0])) || is_concat_ty(self.vty(args[1])));
                if is_commutative(o)
                    && args.len() == 2
                    && !concat
                    && self.vhash[args[0] as usize] > self.vhash[args[1] as usize]
                {
                    args.swap(0, 1);
                }
            }
        }
        if let ValOp::Bin(o) = op {
            if args.len() == 2 && (o == Op::Add as u32 || o == Op::BitOr as u32) {
                if let Some(v) = self.c_u16_be_byte_pack_pattern(args[0], args[1]) {
                    return v;
                }
            }
        }
        // Type-gated simplifications — now SOUND because the operand type is PROVEN (these
        // were the 17 false merges when applied untyped; they only hold on numbers/bools):
        //   -(-x) → x        when x : Num   (−(−x) = x; on a list it would Err ≠ x)
        //   x & x, x | x → x when x : Num   (idempotent integer bitwise)
        //   x && x, x || x → x when x : Bool (idempotent boolean)
        if let ValOp::Un(o) = op {
            // NEGATED COMPARISON: `!(a<=b) → a>b`, `!(a<b) → a>=b`, `!(a==b) → a!=b`, etc.
            // Sound for a total order, and on non-numeric operands both sides Err
            // (`!(Err)` propagates — see interp `un`), so the rewrite preserves behavior.
            // This canonicalizes the residual `Not` the algebra pass leaves on a pushed
            // double-negation (`!!(a<b)` → `!(b<=a)` → `a<b`), converging it with the bare
            // comparison without the unsound untyped `!!x → x`.
            if o == Op::Not as u32 && !args.is_empty() {
                if let ValOp::Bin(bo) = self.nodes[args[0] as usize].op {
                    if let Some(neg) = negate_cmp_code(bo) {
                        let cargs = self.nodes[args[0] as usize].args.clone();
                        return self.mk(ValOp::Bin(neg), cargs);
                    }
                }
            }
            if o == Op::Neg as u32 && !args.is_empty() {
                if let ValOp::Un(io) = self.nodes[args[0] as usize].op {
                    let inner = self.nodes[args[0] as usize].args[0];
                    if io == Op::Neg as u32 && self.vty(inner) == Ty::Num {
                        return inner;
                    }
                }
                // Distribute negation over addition: `-(x + y) → (-x) + (-y)`. Sound for
                // ALL types — `Neg` errors on non-numeric, and so does the distributed
                // form (`-list` is Err either way). Pushing Neg inward gives a canonical
                // form so `-(a+b)` converges with `-a - b` (= `-a + -b`).
                if let ValOp::Bin(bo) = self.nodes[args[0] as usize].op {
                    if bo == Op::Add as u32 {
                        let inner = self.nodes[args[0] as usize].args.clone();
                        let negs: Vec<ValueId> = inner
                            .iter()
                            .map(|&t| self.mk(ValOp::Un(Op::Neg as u32), vec![t]))
                            .collect();
                        let mut acc = negs[0];
                        for &n in &negs[1..] {
                            acc = self.mk(ValOp::Bin(Op::Add as u32), vec![acc, n]);
                        }
                        return acc;
                    }
                }
            }
        }
        if let ValOp::Bin(o) = op {
            if args.len() == 2 && args[0] == args[1] {
                let t = self.vty(args[0]);
                let bitwise = o == Op::BitAnd as u32 || o == Op::BitOr as u32;
                let logical = o == Op::And as u32 || o == Op::Or as u32;
                if (bitwise && t == Ty::Num) || (logical && t == Ty::Bool) {
                    return args[0];
                }
            }
            // NOTE: arithmetic identity elimination (`x+0→x`, `x*1→x`) is deliberately NOT
            // done — it is unsound for non-numeric `x` (`"a"+0` Errs; `self*1` on a
            // non-number need not equal `self`), and type inference is OPTIMISTIC (it infers
            // `x:Num` from `x*1` itself), so a Num gate would still merge `return self*1`
            // with an identity `return self`. The oracle's all-types battery sees the
            // difference. `x*1`/`x+0` keep their identity operand (algebra no longer drops
            // it) and stay distinct from a bare `x` — a tiny convergence cost for soundness.

            // DISTRIBUTION / FACTORING: `x*f + y*f → (x+y)*f`. Canonicalizes toward the
            // factored form so `a*c + b*c` converges with `(a+b)*c`. Sound ONLY on numbers —
            // string `*int` is repetition, where `"a"*2 + "b"*2` ("aabb") ≠ `("a"+"b")*2`
            // ("abab") — so every leaf must be PROVEN `Num`. Lean obligation:
            // `normalize.value_graph.factor_distribute`.
            if o == Op::Add as u32 && args.len() == 2 {
                if let Some(v) = rules::factor_distribute::apply(self, args[0], args[1]) {
                    return v;
                }
            }
            // DOUBLING (`x*k → x+…+x`, so `x*2` ≡ `x+x`) was TRIED and REJECTED: expansion is
            // sound only on numbers, so it must gate on a PROVEN `Num`; but then the canonical
            // form of `(a+b)*2` depends on whether the surrounding code happens to prove the
            // operands numeric, splitting two behaviorally-identical functions (`a+=b; a*=2`
            // diverged from `(a+b)*2`). It closed `x*2 vs x+x` but opened `compound assign` —
            // net-zero, plus fragility. The gap stays open; see experiments §BA. (`x+x` in
            // isolation cannot be proven `Num`, so the sound contraction direction never fires.)
        }
        // `and`/`or` are TYPE-GATED on commutativity, exactly like `+` is gated on concat:
        //   • both operands PROVEN Bool → boolean-and/or, which IS commutative
        //     (`X && Y` = `Y && X` for booleans) — sort operands so `p∧q` converges with
        //     `q∧p` (e.g. `(a>b)∧(b>0)` vs `(0<b)∧(b<a)` after comparison-direction canon).
        //   • otherwise → short-circuit VALUE-and/or, which is NOT commutative and yields
        //     the deciding operand's VALUE: `a or b ≡ a if a else b`, `a and b ≡ b if a
        //     else a`. Canonicalize to the positional `Phi` the ternary builds, so a guard
        //     written `a or b` converges with its `a if a else b` twin — without ever
        //     merging `a or b` with `b or a` (the value-or false merge the oracle now sees).
        // (Idempotent `x∧x`/`x∨x` on Bool, handled just above, returns before this.)
        if let ValOp::Bin(o) = op {
            let is_or = o == Op::Or as u32;
            let is_and = o == Op::And as u32;
            if (is_or || is_and) && args.len() == 2 {
                if self.vty(args[0]) == Ty::Bool && self.vty(args[1]) == Ty::Bool {
                    if is_or {
                        if let Some(v) = self.literal_equality_disjunction(args[0], args[1]) {
                            return v;
                        }
                    }
                    // LATTICE CANON on a total order — close the strict comparison from a
                    // non-strict one plus an (in)equality, so a guard written as the
                    // conjunction/disjunction converges with the strict comparison:
                    //   (x ≤ y) ∧ (x ≠ y) → x < y     (dual of below)
                    //   (x < y) ∨ (x = y) → x ≤ y
                    // Sound for any total order (`normalize.value_graph.compare`); on a type
                    // error every comparison Errs identically on both sides. It composes through
                    // the recursive `mk` fixpoint, so `not (a>b or a==b)` reaches `a<b`.
                    if is_and {
                        if let Some(v) = self.lattice_strict_absorbs_nonstrict(args[0], args[1]) {
                            return v;
                        }
                        if let Some(v) = self.lattice_le_ne_to_lt(args[0], args[1]) {
                            return v;
                        }
                    } else if let Some(v) = self.lattice_lt_eq_to_le(args[0], args[1]) {
                        return v;
                    }
                    if self.vhash[args[0] as usize] > self.vhash[args[1] as usize] {
                        args.swap(0, 1);
                    }
                } else if is_or {
                    return self.mk(ValOp::Phi, vec![args[0], args[0], args[1]]);
                } else {
                    return self.mk(ValOp::Phi, vec![args[0], args[1], args[0]]);
                }
            }
        }
        // Recognize select idioms on EVERY branch merge — `Phi(cond, then, els)` is built
        // both by a ternary (`a if c else b`) and by an if/else that assigns a variable, so
        // doing this here (not just at the ternary) keeps the two forms convergent:
        //   `x if x>=0 else -x` → Abs(x) ;  `x if x<y else y` → Min(x,y) / Max(x,y).
        if let ValOp::Phi = op {
            if args.len() == 3 {
                if self.bool_const(args[1]) == Some(true) && self.bool_const(args[2]) == Some(false)
                {
                    return args[0];
                }
                if self.bool_const(args[1]) == Some(false) && self.bool_const(args[2]) == Some(true)
                {
                    return self.mk(ValOp::Un(Op::Not as u32), vec![args[0]]);
                }
                if let Some(v) = self.boolean_guarded_identity_phi(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.abs_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.minmax_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.clamp_ternary_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.low_bit_toggle_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.map_default_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.value_default_pattern(args[0], args[1], args[2]) {
                    return v;
                }
                if let Some(v) = self.flatten_nested_guarded_identity_phi(args[0], args[1], args[2])
                {
                    return v;
                }
            }
        }
        // Boolean logical `and`/`or` is associative and commutative only when both sides are
        // proven Bool. Flattening that narrow shape lets `guard && (p && q)` converge with
        // `(guard && p) && q` without reviving value-short-circuit false merges for unknowns.
        if let ValOp::Bin(o) = op {
            if args.len() == 2 && (o == Op::And as u32 || o == Op::Or as u32) {
                let mut leaves = Vec::new();
                self.flatten_into(args[0], o, &mut leaves);
                self.flatten_into(args[1], o, &mut leaves);
                if leaves.len() > 2 && leaves.iter().all(|&v| self.vty(v) == Ty::Bool) {
                    leaves.sort_unstable_by_key(|&v| self.vhash[v as usize]);
                    return self.intern_ac_chain(o, &leaves);
                }
            }
        }

        // Full ASSOCIATIVE-COMMUTATIVE canonicalization: flatten a `+`/`*`/`&`/`|`/`^`
        // chain to its leaves, sort them by structural hash, and rebuild one canonical
        // left-leaning chain — so `(a+b)+c`, `a+(b+c)`, and a factored `(a+b)+d` (from
        // `factor_distribute`) all reach ONE node regardless of how they were grouped or
        // built. The value graph thus canonicalizes AC chains itself, not only via the
        // earlier `algebra` IL pass (which keyed by a different hash and did not see nodes
        // synthesized here). Sound: any operand permutation of an AC chain is denotation-
        // preserving (`normalize.value_graph.algebra`). String/list `+` is NOT reordered
        // (it is ordered concat); `* & | ^` Err on non-numeric regardless of order.
        if let ValOp::Bin(o) = op {
            if is_assoc_comm_code(o) && args.len() == 2 {
                let concat = o == Op::Add as u32
                    && (is_concat_ty(self.vty(args[0])) || is_concat_ty(self.vty(args[1])));
                if !concat {
                    let mut leaves = Vec::new();
                    for &a in &args {
                        self.flatten_into(a, o, &mut leaves);
                    }
                    if leaves.len() > 2 {
                        leaves.sort_unstable_by_key(|&v| self.vhash[v as usize]);
                        return self.intern_ac_chain(o, &leaves);
                    }
                }
            }
        }
        let id = self.intern_node(op, args);
        self.canonicalize_proof_backed_clamp(id).unwrap_or(id)
    }

    /// Intern a value node by `(op, args)` (hash-consing), computing its structural hash
    /// and coarse type. The raw constructor used by `mk` after canonicalization — it does
    /// NOT itself canonicalize, so callers must pass already-canonical operands (this is
    /// how `mk` folds an AC chain without re-triggering the flatten).
    fn intern_node(&mut self, op: ValOp, args: Vec<ValueId>) -> ValueId {
        let key = (op.clone(), args.clone());
        if let Some(&id) = self.intern.get(&key) {
            return id;
        }
        let id = self.nodes.len() as ValueId;
        let mut h = op_tag(&op);
        for &a in &args {
            h = combine(h, self.vhash[a as usize]);
        }
        let ty = self.ty_of(&op, &args);
        self.nodes.push(ValNode { op, args });
        self.vhash.push(h);
        self.vty.push(ty);
        self.intern.insert(key, id);
        id
    }

    fn boolean_guarded_identity_phi(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.vty(cond) != Ty::Bool || self.vty(then_v) != Ty::Bool {
            return None;
        }
        match self.bool_const(else_v) {
            Some(false) => Some(self.mk(ValOp::Bin(Op::And as u32), vec![cond, then_v])),
            Some(true) => {
                let not_then = self.mk(ValOp::Un(Op::Not as u32), vec![then_v]);
                let failure = self.mk(ValOp::Bin(Op::And as u32), vec![cond, not_then]);
                Some(self.mk(ValOp::Un(Op::Not as u32), vec![failure]))
            }
            None => None,
        }
    }

    fn flatten_nested_guarded_identity_phi(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        let inner_args = {
            let inner = &self.nodes[then_v as usize];
            if !matches!(inner.op, ValOp::Phi) || inner.args.len() != 3 || inner.args[2] != else_v {
                return None;
            }
            inner.args.clone()
        };
        let both = self.mk(ValOp::Bin(Op::And as u32), vec![cond, inner_args[0]]);
        Some(self.mk(ValOp::Phi, vec![both, inner_args[1], else_v]))
    }

    fn intern_ac_chain(&mut self, opc: u32, operands: &[ValueId]) -> ValueId {
        debug_assert!(!operands.is_empty());
        let mut acc = operands[0];
        for &operand in &operands[1..] {
            acc = self.intern_node(ValOp::Bin(opc), vec![acc, operand]);
        }
        acc
    }

    fn compact_formula(&mut self, opc: u32, operands: &[ValueId]) -> ValueId {
        let mut h = combine(0xF0A5_7A11, opc as u64);
        h = combine(h, operands.len() as u64);
        for &operand in operands {
            h = combine(h, self.vhash[operand as usize]);
        }
        self.mk(ValOp::Formula(h), vec![])
    }

    fn compact_add_sub_formula(
        &mut self,
        operands: Vec<SignedExprOperand>,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let mut values = Vec::new();
        for operand in operands {
            let mut value = self.eval(operand.expr, env);
            if operand.negated {
                value = self.mk(ValOp::Un(Op::Neg as u32), vec![value]);
            }
            self.flatten_into(value, Op::Add as u32, &mut values);
        }
        if values.iter().all(|&v| !is_concat_ty(self.vty(v))) {
            values.sort_by_key(|&v| self.vhash[v as usize]);
        }
        self.compact_formula(Op::Add as u32, &values)
    }

    /// Flatten an associative-commutative chain of value nodes into `out`.
    fn flatten_into(&mut self, vid: ValueId, opc: u32, out: &mut Vec<ValueId>) {
        let mut stack = vec![vid];
        while let Some(value) = stack.pop() {
            if let ValOp::Bin(o) = self.nodes[value as usize].op {
                if o == opc {
                    for &arg in self.nodes[value as usize].args.iter().rev() {
                        stack.push(arg);
                    }
                    continue;
                }
            }
            out.push(value);
        }
    }

    fn collect_add_sub_expr_operands(
        &self,
        expr: NodeId,
        negated: bool,
        out: &mut Vec<SignedExprOperand>,
    ) {
        if self.il.kind(expr) != NodeKind::BinOp {
            out.push(SignedExprOperand { expr, negated });
            return;
        }
        match op_code(self.il.node(expr).payload) {
            op if op == Op::Add as u32 => {
                for &child in self.il.children(expr) {
                    self.collect_add_sub_expr_operands(child, negated, out);
                }
            }
            op if op == Op::Sub as u32 && self.il.children(expr).len() == 2 => {
                let kids = self.il.children(expr);
                self.collect_add_sub_expr_operands(kids[0], negated, out);
                self.collect_add_sub_expr_operands(kids[1], !negated, out);
            }
            _ => out.push(SignedExprOperand { expr, negated }),
        }
    }

    fn collect_ac_expr_operands(&self, expr: NodeId, opc: u32, out: &mut Vec<NodeId>) {
        if self.il.kind(expr) == NodeKind::BinOp && op_code(self.il.node(expr).payload) == opc {
            for &child in self.il.children(expr) {
                self.collect_ac_expr_operands(child, opc, out);
            }
        } else {
            out.push(expr);
        }
    }

    fn fresh_opaque(&mut self) -> ValueId {
        let c = self.opaque_ctr;
        self.opaque_ctr += 1;
        self.mk(ValOp::Opaque(c as u64), vec![])
    }

    /// The operands of a comparison node `cmp`, if it has opcode `want`. `Le`/`Lt` are
    /// ORDERED (operands kept in source order); `Eq`/`Ne` are COMMUTATIVE (operands
    /// vhash-sorted), so callers compare them as an unordered pair.
    fn cmp_operands(&self, v: ValueId, want: u32) -> Option<(ValueId, ValueId)> {
        if let ValOp::Bin(o) = self.nodes[v as usize].op {
            if o == want && self.nodes[v as usize].args.len() == 2 {
                let a = &self.nodes[v as usize].args;
                return Some((a[0], a[1]));
            }
        }
        None
    }

    /// Lattice canon combining an ORDERED comparison (`ordered`, which fixes the operand
    /// pair `(x, y)` in source order) with a COMMUTATIVE (in)equality (`comm`, whose
    /// operands match `{x, y}` in either order) into a single `result` comparison. The two
    /// arguments may arrive in either slot (the conjunction/disjunction is itself sorted),
    /// so both assignments are tried. Sound on a total order; each instantiation cites its
    /// own Lean lemma at the call site.
    fn lattice_pair_canon(
        &mut self,
        a: ValueId,
        b: ValueId,
        ordered: u32,
        comm: u32,
        result: u32,
    ) -> Option<ValueId> {
        for (ord_v, comm_v) in [(a, b), (b, a)] {
            if let Some((x, y)) = self.cmp_operands(ord_v, ordered) {
                if let Some((c0, c1)) = self.cmp_operands(comm_v, comm) {
                    if (c0 == x && c1 == y) || (c0 == y && c1 == x) {
                        return Some(self.mk(ValOp::Bin(result), vec![x, y]));
                    }
                }
            }
        }
        None
    }

    /// `(x ≤ y) ∧ (x ≠ y) → x < y`. Sound on a total order
    /// (`normalize.value_graph.compare`); the post-normalize oracle re-checks it.
    fn lattice_le_ne_to_lt(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        self.lattice_pair_canon(a, b, Op::Le as u32, Op::Ne as u32, Op::Lt as u32)
    }

    fn has_primitive_order_comparisons(&self) -> bool {
        matches!(self.il.meta.lang, Lang::C | Lang::Go | Lang::Java)
    }

    /// `(x < y) ∧ (x ≤ y) → x < y`. Guard-clause lowering accumulates path conditions
    /// from earlier returns, so a comparator written as `if x<y return -1; if x>y return
    /// 1; return 0` otherwise leaves the second return guarded by `x≤y ∧ x<y` after
    /// comparison-direction canon. The non-strict half is implied by the strict half and
    /// can be absorbed only for source languages whose comparison operators are primitive
    /// rather than receiver-overloadable.
    fn lattice_strict_absorbs_nonstrict(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        if !self.has_primitive_order_comparisons() {
            return None;
        }
        for (lt_v, le_v) in [(a, b), (b, a)] {
            if let Some((x, y)) = self.cmp_operands(lt_v, Op::Lt as u32) {
                if self.cmp_operands(le_v, Op::Le as u32) == Some((x, y)) {
                    return Some(lt_v);
                }
            }
        }
        None
    }

    /// `(x < y) ∨ (x = y) → x ≤ y` — the dual of [`lattice_le_ne_to_lt`] over `∨`
    /// (`normalize.value_graph.compare`).
    fn lattice_lt_eq_to_le(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        self.lattice_pair_canon(a, b, Op::Lt as u32, Op::Eq as u32, Op::Le as u32)
    }

    /// Build the value graph for a `Func`/`Method`/class unit. The unit root may
    /// be a `Func` (params + body) or a `Block` (class body of methods); for a
    /// `Block` we process its statements directly.
    fn build_unit(&mut self, root: NodeId) {
        self.build_unit_with_context(root, None);
    }

    fn build_unit_with_context(&mut self, root: NodeId, context: Option<&ValueFingerprintContext>) {
        self.param_ty = crate::types::infer_param_types(self.il, root);
        self.param_semantic.clear();
        self.seed_param_semantics(root);
        self.seed_immutable_bindings(root, context);
        self.build_inline_registry(root);
        let mut env: FxHashMap<u32, ValueId> = FxHashMap::default();
        match self.il.kind(root) {
            NodeKind::Func => {
                // Seed parameters as inputs *by position*, so duplicate-named params
                // (which alpha-rename collapses to one cid) stay distinct values — the
                // accessible one wins, as at runtime. For well-formed code param cid ==
                // position, so this is identical to keying by cid.
                let kids = self.il.children(root).to_vec();
                let mut pos = 0u32;
                for &k in &kids {
                    if self.il.kind(k) == NodeKind::Param {
                        if let Payload::Cid(c) = self.il.node(k).payload {
                            if matches!(
                                self.param_semantic.get(&c),
                                Some(ParamSemantic::Integer | ParamSemantic::Number)
                            ) {
                                let pos_idx = pos as usize;
                                if self.param_ty.len() <= pos_idx {
                                    self.param_ty.resize(pos_idx + 1, Ty::Unknown);
                                }
                                self.param_ty[pos_idx] = Ty::Num;
                            }
                            let v = self.mk(ValOp::Input(pos), vec![]);
                            env.insert(c, v);
                            pos += 1;
                        }
                    }
                }
                if let Some(&body) = kids.last() {
                    self.process_stmt(body, &mut env);
                }
                self.recognize_value_default_returns();
                self.recognize_existence_reduction();
            }
            NodeKind::Module | NodeKind::Block => {
                // Class/other container unit. Two things make its data visible:
                //  (1) attribute assignments (`name = value`) land in `env` but reach no
                //      sink — a class's attributes ARE its data, so expose them (two
                //      locale-table classes that differ only in values must differ);
                //  (2) a container's *behavior* is the aggregate of its methods. Plain
                //      `process_stmt` has no `Func` case, so a method definition fell to
                //      the opaque-effect branch and the class collapsed to a near-empty
                //      structural shell — a one-operator change deep inside a method left
                //      the class fingerprint identical, so two classes were "behavioral
                //      clones" on structure alone. Descend into each contained method and
                //      fold its returns/effects into the container, so the class differs
                //      exactly when its methods do.
                self.process_container(root, &mut env);
                let mut vals: Vec<ValueId> = env.values().copied().collect();
                vals.sort_unstable();
                vals.dedup();
                for v in vals {
                    self.sinks.push(Sink::new(SinkKind::Effect, v));
                }
            }
            _ => {
                self.process_stmt(root, &mut env);
            }
        }
        self.flush_fields();
    }

    fn seed_immutable_bindings(&mut self, root: NodeId, context: Option<&ValueFingerprintContext>) {
        if let Some(context) = context {
            let required = context.module.required_bindings_for(self.il, root);
            self.seed_module_value_bindings_from_context(&context.module, Some(&required));
        } else {
            self.seed_module_value_bindings();
        }
        if let Some(context) = context {
            self.seed_function_binding_hashes(&context.function_bindings);
        } else {
            self.seed_function_bindings();
        }
    }

    fn seed_module_value_bindings(&mut self) {
        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for stmt in self.top_level_statements() {
            let Some(name) = self.assignment_name(stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }

        let top_level = self.top_level_statements();
        self.seed_module_value_bindings_from_parts(&top_level, &counts, None, None, None);
    }

    fn seed_module_value_bindings_from_context(
        &mut self,
        context: &ModuleSeedContext,
        required_bindings: Option<&FxHashSet<Symbol>>,
    ) {
        self.seed_module_value_bindings_from_parts(
            &context.top_level,
            &context.assignment_counts,
            Some(&context.mutated_bindings),
            Some(&context.unit_symbols),
            required_bindings,
        );
    }

    fn seed_module_value_bindings_from_parts(
        &mut self,
        top_level: &[NodeId],
        counts: &FxHashMap<Symbol, usize>,
        mutated_bindings: Option<&FxHashSet<Symbol>>,
        unit_symbols: Option<&FxHashSet<Symbol>>,
        required_bindings: Option<&FxHashSet<Symbol>>,
    ) {
        let mut env: FxHashMap<u32, ValueId> = FxHashMap::default();
        for &stmt in top_level {
            let kids = self.il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = self.assignment_name(stmt) else {
                continue;
            };
            if required_bindings.is_some_and(|required| !required.contains(&name)) {
                continue;
            }
            let unit_defines_symbol = unit_symbols
                .map(|symbols| symbols.contains(&name))
                .unwrap_or_else(|| self.unit_defines_symbol(name));
            if unit_defines_symbol {
                continue;
            }
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            let mutated = mutated_bindings
                .map(|bindings| bindings.contains(&name))
                .unwrap_or_else(|| self.module_binding_mutated(name));
            if mutated {
                continue;
            }
            let value = self.eval(kids[1], &env);
            let value = if self.immutable_binding_safe(kids[1], &env) {
                value
            } else {
                let Some(proven) = self
                    .proven_map_value(value)
                    .or_else(|| self.proven_collection_value(value))
                else {
                    continue;
                };
                proven
            };
            if let Payload::Cid(cid) = self.il.node(kids[0]).payload {
                env.insert(cid, value);
            }
            self.global_env.insert(name, value);
        }
    }

    fn top_level_statements(&self) -> Vec<NodeId> {
        top_level_statements_for(self.il)
    }

    fn seed_function_bindings(&mut self) {
        let bindings = self.collect_function_binding_hashes();
        self.seed_function_binding_hashes(&bindings);
    }

    fn collect_function_binding_hashes(&mut self) -> Vec<(Symbol, u64)> {
        let mut bindings = Vec::new();
        for unit in self.il.units.clone() {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method) {
                continue;
            }
            let Some(name) = unit.name else {
                continue;
            };
            if self.function_binding_safe(unit.root, unit.root) {
                let hash = self.valued_subtree_hash(unit.root);
                bindings.push((name, hash));
            }
        }
        bindings
    }

    fn seed_function_binding_hashes(&mut self, bindings: &[(Symbol, u64)]) {
        for &(name, hash) in bindings {
            let value = self.mk(ValOp::Lambda(hash), vec![]);
            self.global_env.insert(name, value);
        }
    }

    fn assignment_name(&self, stmt: NodeId) -> Option<Symbol> {
        assignment_name_in_scope(self.il, stmt, &self.local_scope_nodes)
    }

    fn unit_defines_symbol(&self, symbol: Symbol) -> bool {
        self.il
            .units
            .iter()
            .any(|unit| unit.name.is_some_and(|name| name == symbol))
    }

    fn module_binding_mutated(&self, name: Symbol) -> bool {
        let top_level = self.top_level_statements();
        let shadowed = shadowed_js_like_module_binding_nodes_for_symbol_in_scope(
            self.il,
            name,
            &self.local_scope_nodes,
        );
        self.il.nodes.iter().enumerate().any(|(idx, node)| {
            let node_id = NodeId(idx as u32);
            if shadowed.contains(&node_id) {
                return false;
            }
            match node.kind {
                NodeKind::Call => self.call_mutates_binding(node_id, name).unwrap_or(false),
                NodeKind::Field => self.field_mutates_binding(node_id, name).unwrap_or(false),
                NodeKind::Assign if !top_level.contains(&node_id) => self
                    .assignment_mutates_binding(node_id, name)
                    .unwrap_or(false),
                _ => false,
            }
        })
    }

    fn assignment_mutates_binding(&self, assign: NodeId, name: Symbol) -> Option<bool> {
        let lhs = self.il.children(assign).first().copied()?;
        Some(self.node_contains_symbol(lhs, name))
    }

    fn call_mutates_binding(&self, call: NodeId, name: Symbol) -> Option<bool> {
        if !matches!(
            self.il.node(call).payload,
            Payload::Builtin(Builtin::Append)
        ) {
            return Some(false);
        }
        let receiver = self.il.children(call).first().copied()?;
        Some(self.node_refers_to_symbol(receiver, name))
    }

    fn field_mutates_binding(&self, field: NodeId, name: Symbol) -> Option<bool> {
        let Payload::Name(method) = self.il.node(field).payload else {
            return Some(false);
        };
        if !mutating_method_name(self.interner.resolve(method)) {
            return Some(false);
        }
        let receiver = self.il.children(field).first().copied()?;
        Some(self.node_refers_to_symbol(receiver, name))
    }

    fn node_refers_to_symbol(&self, node: NodeId, name: Symbol) -> bool {
        self.node_symbol(node).is_some_and(|symbol| symbol == name)
    }

    fn node_symbol(&self, node: NodeId) -> Option<Symbol> {
        node_symbol_in_scope(self.il, node, &self.local_scope_nodes)
    }

    fn node_contains_symbol(&self, node: NodeId, name: Symbol) -> bool {
        self.node_refers_to_symbol(node, name)
            || self
                .il
                .children(node)
                .iter()
                .any(|&child| self.node_contains_symbol(child, name))
    }

    fn node_refers_to_cid(&self, node: NodeId, cid: u32) -> bool {
        matches!(self.il.node(node).payload, Payload::Cid(current) if current == cid)
    }

    fn node_contains_cid(&self, node: NodeId, cid: u32) -> bool {
        self.node_refers_to_cid(node, cid)
            || self
                .il
                .children(node)
                .iter()
                .any(|&child| self.node_contains_cid(child, cid))
    }

    fn immutable_binding_safe(&self, node: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        match self.il.kind(node) {
            NodeKind::Raw
            | NodeKind::Call
            | NodeKind::HoF
            | NodeKind::Func
            | NodeKind::Lambda
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Throw
            | NodeKind::Assign => false,
            NodeKind::Var => match self.il.node(node).payload {
                Payload::Cid(c) => env.contains_key(&c),
                Payload::Name(s) => self.global_env.contains_key(&s),
                _ => false,
            },
            NodeKind::Lit => matches!(
                self.il.node(node).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::LitFloat(_)
                    | Payload::Lit(nose_il::LitClass::Null)
            ),
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.immutable_binding_safe(c, env)),
        }
    }

    /// Build the interprocedural inline registry (see the `inline_fns` field): pure,
    /// uniquely-named, file-local functions/methods that can be inlined to their body's value.
    /// Excludes the unit currently being built (`root`) so a function is never inlined into
    /// itself, and drops any name shared by two definitions (ambiguous → not resolvable).
    fn build_inline_registry(&mut self, root: NodeId) {
        if !self.inline_fns.is_empty() {
            return;
        }
        let mut ambiguous: FxHashSet<Symbol> = FxHashSet::default();
        for unit in self.il.units.clone() {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method) || unit.root == root {
                continue;
            }
            let Some(name) = unit.name else { continue };
            if ambiguous.contains(&name) {
                continue;
            }
            if self.inline_fns.contains_key(&name) {
                self.inline_fns.remove(&name);
                ambiguous.insert(name);
                continue;
            }
            if !self.function_binding_safe(unit.root, unit.root) {
                continue;
            }
            // SOUNDNESS: only inline a function whose body is a single `return <expr>` — a pure
            // value with NO statement effects. `function_binding_safe` alone is too weak here: it
            // admits field/index writes (an effect the value-only inline would silently drop,
            // which the oracle — blind to cross-function calls — could not catch). A lone return
            // has no other statements, so there is nothing to drop.
            let Some(ret_expr) = self.single_return_expr(unit.root) else {
                continue;
            };
            let kids = self.il.children(unit.root);
            let params: Vec<u32> = kids
                .iter()
                .filter_map(|&p| match self.il.node(p).payload {
                    Payload::Cid(c) if self.il.kind(p) == NodeKind::Param => Some(c),
                    _ => None,
                })
                .collect();
            self.inline_fns.insert(name, (params, ret_expr));
        }
    }

    /// Inline a call to a PURE registered function: bind its parameters to the (caller-evaluated)
    /// argument values and evaluate its body to a single value — β-reduction over an effect-free
    /// function, so `f(args)` ≡ the function's body with `args` substituted (the extract-method /
    /// interprocedural-summary equivalence). Returns `None` for non-direct calls, unknown/
    /// ambiguous callees, or arity mismatch — leaving the opaque-call fallback to run.
    fn eval_inlined_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let &callee = kids.first()?;
        if self.il.kind(callee) != NodeKind::Var {
            return None;
        }
        let Payload::Name(fname) = self.il.node(callee).payload else {
            return None;
        };
        let (params, ret_expr) = self.inline_fns.get(&fname)?.clone();
        if params.len() != kids.len() - 1 {
            return None;
        }
        let mut fenv: FxHashMap<u32, ValueId> = FxHashMap::default();
        for (pi, &pc) in params.iter().enumerate() {
            let av = self.eval(kids[pi + 1], env);
            fenv.insert(pc, av);
        }
        Some(self.eval(ret_expr, &fenv))
    }

    /// The single `return <expr>` of a function body, or `None` if the body is anything else
    /// (locals, multiple statements, effects, control flow) — so only an effect-free pure value
    /// qualifies for value-only inlining. Accepts `{ return e; }` and a bare `return e`.
    fn single_return_expr(&self, root: NodeId) -> Option<NodeId> {
        let &body = self.il.children(root).last()?;
        let ret = match self.il.kind(body) {
            NodeKind::Return => body,
            NodeKind::Block => match self.il.children(body) {
                [only] if self.il.kind(*only) == NodeKind::Return => *only,
                _ => return None,
            },
            _ => return None,
        };
        self.il.children(ret).first().copied()
    }

    fn function_binding_safe(&self, root: NodeId, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Raw
            | NodeKind::HoF
            | NodeKind::Lambda
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Throw => false,
            NodeKind::Func if node != root => false,
            NodeKind::Call if !matches!(self.il.node(node).payload, Payload::Builtin(_)) => false,
            NodeKind::Var => match self.il.node(node).payload {
                Payload::Cid(_) => true,
                Payload::Name(s) => self.global_env.contains_key(&s),
                _ => false,
            },
            NodeKind::Lit => matches!(
                self.il.node(node).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::LitFloat(_)
                    | Payload::Lit(nose_il::LitClass::Null)
            ),
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.function_binding_safe(root, c)),
        }
    }

    /// Recognize an existence/universal loop written with an early return, and rewrite it
    /// to the same `Reduce(REDUCE_ANY/ALL, [predicate])` the functional `any`/`all` builds:
    ///   `for x in xs: if p(x): return True` ; `return False`        → `any(p(x) for x in xs)`
    ///   `for x in xs: if not p(x): return False` ; `return True`    → `all(p(x) for x in xs)`
    /// After lowering these are exactly two `Return` sinks (no effects): one guarded by a
    /// predicate over a loop ELEMENT returning a bool constant, plus the unguarded
    /// complementary bool constant. Behavior-preserving — `∃`/`∀` over a pure predicate
    /// is order- and short-circuit-insensitive — and gated by the oracle. Requiring an
    /// `Elem` in the guard ties it to genuine collection iteration (a first-element check
    /// `if xs[0]>0` keeps an `Index`, not `Elem`, so it is not mistaken for `any`).
    fn recognize_existence_reduction(&mut self) {
        if self.sinks.len() != 2
            || self
                .sinks
                .iter()
                .any(|sink| !matches!(sink.kind, SinkKind::Return))
        {
            return;
        }
        let r0 = self.sinks[0].value;
        let r1 = self.sinks[1].value;
        let bot = self.mk(ValOp::Const(0x3000_0000), vec![]);
        let tru = self.mk(ValOp::Const(0x3000_0002), vec![]);
        let fls = self.mk(ValOp::Const(0x3000_0001), vec![]);
        let int_one = self.int_const(1);
        let int_zero = self.int_const(0);
        for &(guarded, plain) in &[(r0, r1), (r1, r0)] {
            let ValOp::Phi = self.nodes[guarded as usize].op else {
                continue;
            };
            let a = self.nodes[guarded as usize].args.clone();
            if a.len() != 3 || a[2] != bot || !self.refs_elem(a[0]) {
                continue;
            }
            let (guard, ret) = (a[0], a[1]);
            let ret_true = ret == tru || ret == int_one;
            let ret_false = ret == fls || ret == int_zero;
            let plain_true = plain == tru || plain == int_one;
            let plain_false = plain == fls || plain == int_zero;
            let (code, pred) = if ret_true && plain_false {
                (REDUCE_ANY, guard)
            } else if ret_false && plain_true {
                (REDUCE_ALL, self.mk(ValOp::Un(Op::Not as u32), vec![guard]))
            } else {
                continue;
            };
            let red = self.mk(ValOp::Reduce(code), vec![pred]);
            self.sinks = vec![Sink::new(SinkKind::Return, red)];
            return;
        }
    }

    fn recognize_value_default_returns(&mut self) {
        if self.sinks.len() != 2
            || self
                .sinks
                .iter()
                .any(|sink| !matches!(sink.kind, SinkKind::Return))
        {
            return;
        }
        if let Some(defaulted) = self
            .map_default_from_partial_default_pair(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.map_default_from_partial_default_pair(self.sinks[1].value, self.sinks[0].value)
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) =
            self.map_default_from_guarded_pair(self.sinks[0].value, self.sinks[1].value)
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) = self
            .map_default_from_guarded_fallthrough(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.map_default_from_guarded_fallthrough(self.sinks[1].value, self.sinks[0].value)
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) =
            self.value_default_from_guarded_pair(self.sinks[0].value, self.sinks[1].value)
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
            return;
        }
        if let Some(defaulted) = self
            .value_default_from_guarded_fallthrough(self.sinks[0].value, self.sinks[1].value)
            .or_else(|| {
                self.value_default_from_guarded_fallthrough(
                    self.sinks[1].value,
                    self.sinks[0].value,
                )
            })
        {
            self.sinks = vec![Sink::new(SinkKind::Return, defaulted)];
        }
    }

    fn map_default_from_partial_default_pair(
        &mut self,
        partial: ValueId,
        fallback: ValueId,
    ) -> Option<ValueId> {
        let (map, key) = self.map_default_bottom_call(partial)?;
        let (cond, fallback_ret) = self.guarded_return_parts(fallback)?;
        let (guard_key, guard_map, present) = self.map_presence_condition(cond)?;
        if present || guard_key != key || guard_map != map {
            return None;
        }
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, fallback_ret],
        ))
    }

    fn map_default_from_guarded_pair(
        &mut self,
        first: ValueId,
        second: ValueId,
    ) -> Option<ValueId> {
        let (cond_a, ret_a) = self.guarded_return_parts(first)?;
        let (cond_b, ret_b) = self.guarded_return_parts(second)?;
        let (key_a, map_a, present_a) = self.map_presence_condition(cond_a)?;
        let (key_b, map_b, present_b) = self.map_presence_condition(cond_b)?;
        if key_a != key_b || map_a != map_b || present_a == present_b {
            return None;
        }
        let default = if present_a {
            if !self.map_lookup_value_matches(ret_a, map_a, key_a) {
                return None;
            }
            ret_b
        } else {
            if !self.map_lookup_value_matches(ret_b, map_a, key_a) {
                return None;
            }
            ret_a
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map_a, key_a, default],
        ))
    }

    fn map_default_from_guarded_fallthrough(
        &mut self,
        guarded: ValueId,
        fallthrough: ValueId,
    ) -> Option<ValueId> {
        let (cond, guarded_ret) = self.guarded_return_parts(guarded)?;
        let (key, map, present) = self.map_presence_condition(cond)?;
        let default = if present {
            if !self.map_lookup_value_matches(guarded_ret, map, key) {
                return None;
            }
            fallthrough
        } else {
            if !self.map_lookup_value_matches(fallthrough, map, key) {
                return None;
            }
            guarded_ret
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    fn value_default_from_guarded_pair(
        &mut self,
        first: ValueId,
        second: ValueId,
    ) -> Option<ValueId> {
        let (cond_a, ret_a) = self.guarded_return_parts(first)?;
        let (cond_b, ret_b) = self.guarded_return_parts(second)?;
        let (value_a, present_a) = self.null_condition(cond_a)?;
        let (value_b, present_b) = self.null_condition(cond_b)?;
        if value_a != value_b || present_a == present_b {
            return None;
        }
        let (value, default) = if present_a {
            if ret_a != value_a {
                return None;
            }
            (value_a, ret_b)
        } else {
            if ret_b != value_a {
                return None;
            }
            (value_a, ret_a)
        };
        if let Some((map, key)) = self.proven_map_get_value(value) {
            return Some(self.mk(
                ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                vec![map, key, default],
            ));
        }
        Some(self.mk_value_default(value, default))
    }

    fn value_default_from_guarded_fallthrough(
        &mut self,
        guarded: ValueId,
        fallthrough: ValueId,
    ) -> Option<ValueId> {
        let (cond, guarded_ret) = self.guarded_return_parts(guarded)?;
        let (value, present) = self.null_condition(cond)?;
        let default = if present {
            if guarded_ret != value {
                return None;
            }
            fallthrough
        } else {
            if fallthrough != value {
                return None;
            }
            guarded_ret
        };
        Some(self.mk_value_default(value, default))
    }

    fn guarded_return_parts(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Phi)
            || node.args.len() != 3
            || !self.is_bottom_value(node.args[2])
        {
            return None;
        }
        Some((node.args[0], node.args[1]))
    }

    fn map_default_bottom_call(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == builtin_tag(Builtin::GetOrDefault))
            && node.args.len() == 3
            && self.is_bottom_value(node.args[2])
        {
            return Some((node.args[0], node.args[1]));
        }
        None
    }

    fn is_bottom_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Const(k) if k == 0x3000_0000)
    }

    /// The conjunction of the current branch path (`c₁ ∧ c₂ ∧ …`), or `None` at top level.
    fn path_cond(&mut self) -> Option<ValueId> {
        let mut pc: Option<ValueId> = None;
        for &c in &self.path.clone() {
            pc = Some(match pc {
                None => c,
                Some(p) => self.mk(ValOp::Bin(Op::And as u32), vec![p, c]),
            });
        }
        pc
    }

    /// The canonical value of a dict key→value entry — `Call(DictEntry, [k, v])` — shared by
    /// a dict `pair`, a dict-comprehension body, and a `d[k]=v` building loop, so all three
    /// converge, while staying DISTINCT from a tuple `Seq([k, v])` (a list of pairs is a
    /// different value than a dict). Covered by the functor/map obligation; the build is a map.
    fn dict_entry(&mut self, kv: Vec<ValueId>) -> ValueId {
        self.mk(ValOp::Call(builtin_tag(Builtin::DictEntry)), kv)
    }

    /// If `target = Index(Var c, k)` for an ACTIVE dict-builder `c` (seeded `{}`/empty), record
    /// the per-element `DictEntry(k, rhs)` under the current path guard and return true — a
    /// `d[k] = v` write IS the build, so `d={}; for x: d[k]=v` converges with `{k: v for x}`.
    /// A second write spoils it (→ ordinary effect). Sound: an empty collection only supports
    /// keyed assignment as a dict (`[]​[k]=v` errors), so this fires only on genuine dict builds.
    fn try_record_index_assign(
        &mut self,
        target: NodeId,
        rhs: ValueId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.il.kind(target) != NodeKind::Index {
            return false;
        }
        let tk = self.il.children(target).to_vec();
        let Some(&base) = tk.first() else {
            return false;
        };
        let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(base), self.il.node(base).payload)
        else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        let Some(&keyn) = tk.get(1) else {
            self.building.insert(c, None);
            return true;
        };
        let k = self.eval(keyn, env);
        let entry = self.dict_entry(vec![k, rhs]);
        let guard = self.path_cond();
        self.building.insert(c, Some((entry, guard)));
        true
    }

    /// If `e` is a single-item `append(r, item)` to an ACTIVE builder var `r`, record the
    /// per-element contribution under the current path guard and return true (the append IS
    /// the build, not an effect). A multi-item form spoils the builder.
    /// Recognize the two effect-append shapes to a var `r`, returning `(r_cid, item_args)`:
    ///   • the canonical `Append(Var r, items…)` — Python/JS `.append`/`.push`, lowered by idioms;
    ///   • Java's `r.add(item…)` List method call — `Call(Field("add", Var r), items…)`.
    /// The caller spoils on `!= 1` item. The Java `.add` is recognized only structurally here;
    /// it becomes a *build* only via `builder_candidates`' empty-Seq-seed gate, which is satisfied
    /// only by `[]` / `new ArrayList<>()` — so overloaded `.add` (BigInteger, Set, `.add(i, x)`)
    /// never enters the Map build.
    fn list_append_parts(&self, e: NodeId) -> Option<(u32, Vec<NodeId>)> {
        // Ruby `r << item` appends to a list (`<<` lowers to `Shl`, a BinOp not a Call). Scoped
        // to Ruby and — via `builder_candidates`' empty-Seq-seed gate — to a `[]`-seeded builder,
        // so integer `a << b` shift (`a` is not a list-builder) never enters the Map build.
        if self.il.meta.lang == Lang::Ruby
            && self.il.kind(e) == NodeKind::BinOp
            && matches!(self.il.node(e).payload, Payload::Op(Op::Shl))
        {
            if let [recv, item] = self.il.children(e) {
                if let (NodeKind::Var, Payload::Cid(c)) =
                    (self.il.kind(*recv), self.il.node(*recv).payload)
                {
                    return Some((c, vec![*item]));
                }
            }
            return None;
        }
        if self.il.kind(e) != NodeKind::Call {
            return None;
        }
        let kids = self.il.children(e).to_vec();
        let &first = kids.first()?;
        if matches!(self.il.node(e).payload, Payload::Builtin(Builtin::Append)) {
            let (NodeKind::Var, Payload::Cid(c)) =
                (self.il.kind(first), self.il.node(first).payload)
            else {
                return None;
            };
            return Some((c, kids[1..].to_vec()));
        }
        if self.il.kind(first) == NodeKind::Field {
            if let Payload::Name(s) = self.il.node(first).payload {
                if self.interner.resolve(s) == "add" {
                    if let Some(&recv) = self.il.children(first).first() {
                        if let (NodeKind::Var, Payload::Cid(c)) =
                            (self.il.kind(recv), self.il.node(recv).payload)
                        {
                            return Some((c, kids[1..].to_vec()));
                        }
                    }
                }
            }
        }
        None
    }

    fn try_record_append(&mut self, e: NodeId, env: &mut FxHashMap<u32, ValueId>) -> bool {
        let Some((c, items)) = self.list_append_parts(e) else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        if items.len() != 1 {
            self.building.insert(c, None); // multi-item append — not a clean map
            return true;
        }
        let contrib = self.eval(items[0], env);
        let guard = self.path_cond();
        self.building.insert(c, Some((contrib, guard)));
        true
    }

    /// Go's functional append `r = append(r, item)` (an `Assign` whose RHS is `Append(r, …)`
    /// over the same var) to an ACTIVE builder var `r` is the same single-item build as the
    /// effect-form `r.append(item)`: record the per-element contribution under the path guard.
    /// A multi-item `append(r, a, b)` spoils the builder, like the effect form.
    fn try_record_reassign_append(
        &mut self,
        target: NodeId,
        rhs: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> bool {
        let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(target), self.il.node(target).payload)
        else {
            return false;
        };
        if !self.building.contains_key(&c) {
            return false;
        }
        if self.il.kind(rhs) != NodeKind::Call
            || !matches!(self.il.node(rhs).payload, Payload::Builtin(Builtin::Append))
        {
            return false;
        }
        let rkids = self.il.children(rhs).to_vec();
        // The append's receiver must be the same var being reassigned (`r = append(r, …)`).
        let same_receiver = rkids.first().is_some_and(|&f| {
            self.il.kind(f) == NodeKind::Var
                && matches!(self.il.node(f).payload, Payload::Cid(fc) if fc == c)
        });
        if !same_receiver {
            return false;
        }
        if rkids.len() != 2 {
            self.building.insert(c, None); // multi-item append — not a clean map
            return true;
        }
        let contrib = self.eval(rkids[1], env);
        let guard = self.path_cond();
        self.building.insert(c, Some((contrib, guard)));
        true
    }

    /// Local list-builder candidates of a loop body: a var `r` (1) bound to an empty list
    /// before the loop, (2) the target of exactly one single-item `append`, and (3) not
    /// otherwise mentioned in the body. Such a loop builds `Map(elem, contrib)` — the same
    /// node the comprehension `[contrib for x in xs]` / `.map`/`.collect` produces.
    fn builder_candidates(&self, body: NodeId, env: &FxHashMap<u32, ValueId>) -> Vec<u32> {
        let mut appends: FxHashMap<u32, u32> = FxHashMap::default();
        let mut mentions: FxHashMap<u32, u32> = FxHashMap::default();
        let mut spoiled: FxHashSet<u32> = FxHashSet::default();
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            // Go functional append `r = append(r, item)`: count it as r's single build append
            // and a single build mention (mirroring the effect form's one receiver mention),
            // then scan ONLY `item` — the two r occurrences (assign target + append receiver)
            // are the build, not other uses that would disqualify r.
            if self.il.kind(n) == NodeKind::Assign {
                if let [tgt, rhs] = self.il.children(n) {
                    if let (NodeKind::Var, Payload::Cid(c)) =
                        (self.il.kind(*tgt), self.il.node(*tgt).payload)
                    {
                        if self.il.kind(*rhs) == NodeKind::Call
                            && matches!(
                                self.il.node(*rhs).payload,
                                Payload::Builtin(Builtin::Append)
                            )
                        {
                            let rk = self.il.children(*rhs);
                            let same = rk.first().is_some_and(|&f| {
                                self.il.kind(f) == NodeKind::Var
                                    && matches!(self.il.node(f).payload, Payload::Cid(fc) if fc == c)
                            });
                            if same {
                                *mentions.entry(c).or_insert(0) += 1;
                                if rk.len() == 2 {
                                    *appends.entry(c).or_insert(0) += 1;
                                    stack.push(rk[1]);
                                } else {
                                    spoiled.insert(c);
                                }
                                continue;
                            }
                        }
                    }
                }
            }
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(n), self.il.node(n).payload) {
                *mentions.entry(c).or_insert(0) += 1;
            }
            // An effect-form append — `r.append/push(item)` (canonical) or Java `r.add(item)` —
            // counts as r's build append (the receiver mention is counted by the generic Var
            // scan above / below as the node's children are walked).
            if let Some((c, items)) = self.list_append_parts(n) {
                if items.len() == 1 {
                    *appends.entry(c).or_insert(0) += 1;
                } else {
                    spoiled.insert(c);
                }
            }
            // A `d[k] = v` assignment is a DICT build for `d` — counted like an append, so
            // `d={}; for x: d[k]=v` is recognized as a builder (finalized to a `Map` of
            // `DictEntry`s, converging with `{k: v for x}`).
            if self.il.kind(n) == NodeKind::Assign {
                let k = self.il.children(n);
                if k.len() == 2 && self.il.kind(k[0]) == NodeKind::Index {
                    if let Some(&base) = self.il.children(k[0]).first() {
                        if let (NodeKind::Var, Payload::Cid(c)) =
                            (self.il.kind(base), self.il.node(base).payload)
                        {
                            *appends.entry(c).or_insert(0) += 1;
                        }
                    }
                }
            }
            stack.extend(self.il.children(n).iter().copied());
        }
        appends
            .iter()
            .filter(|&(&c, &n)| {
                n == 1
                    && !spoiled.contains(&c)
                    && mentions.get(&c).copied() == Some(1)
                    && env.get(&c).is_some_and(|&v| {
                        matches!(self.nodes[v as usize].op, ValOp::Seq(_))
                            && self.nodes[v as usize].args.is_empty()
                    })
            })
            .map(|(&c, _)| c)
            .collect()
    }

    /// Does `v`'s value subgraph reference an `Elem` (a collection element)? Bounded DAG
    /// walk; used to confirm a guard is a per-element predicate of a loop.
    fn refs_elem(&self, v: ValueId) -> bool {
        let mut seen = FxHashSet::default();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if matches!(self.nodes[n as usize].op, ValOp::Elem(_)) {
                return true;
            }
            stack.extend(self.nodes[n as usize].args.iter().copied());
        }
        false
    }

    fn process_block(&mut self, block: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let path_base = self.path.len();
        let bound_order_base = self.bound_order_facts.len();
        for s in self.il.children(block).to_vec() {
            self.process_stmt(s, env);
            // GUARD-CLAUSE normalization: an `if c { …terminates… }` with no else means
            // the REST of the block is reached only when `!c`. Narrow the path by `!c`
            // for the remaining statements, so a guard-clause (`if c {return a}; return b`)
            // produces the same guarded sinks as the if-else form (`if c {return a} else
            // {return b}`) — converging the two writings of the same function (e.g. sympy
            // `symmetric_residue` vs `gf_int`). Cascades for stacked guards.
            if let Some(ncond) = self.guard_clause_negation(s, env) {
                self.record_bound_order_fact(ncond);
                self.path.push(ncond);
                continue;
            }
            // Statements after an UNCONDITIONAL terminator (a `return`/`throw` at this
            // block level — only when no guard narrowing is in effect) are unreachable
            // dead code; the interpreter takes the first return, so the value graph must
            // too (else C `#if return 1 #else return 0`, both preproc branches lowered
            // live, emits two order-independent return sinks → a branch-swapped twin
            // collapses to the same multiset while behaving differently — a false merge).
            if self.path.len() == path_base
                && matches!(self.il.kind(s), NodeKind::Return | NodeKind::Throw)
            {
                break;
            }
        }
        self.bound_order_facts.truncate(bound_order_base);
        self.path.truncate(path_base);
    }

    /// If `s` is a guard clause — `if c { …unconditionally exits… }` with no else — return
    /// `!c` (the condition under which control falls through to the rest of the block).
    /// Used to narrow the path so guard-clause and if-else writings of a function converge.
    fn guard_clause_negation(
        &mut self,
        s: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(s) != NodeKind::If {
            return None;
        }
        let kids = self.il.children(s).to_vec();
        if kids.len() != 2 || !self.branch_exits(kids[1]) {
            return None; // has an else, or the then-branch can fall through
        }
        let cond = self.eval(kids[0], env);
        Some(self.mk(ValOp::Un(Op::Not as u32), vec![cond]))
    }

    fn record_bound_order_fact(&mut self, cond: ValueId) {
        if let Some((lo, hi)) = self.bound_order_from_condition(cond) {
            self.bound_order_facts.push((lo, hi));
        }
    }

    fn bound_order_from_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId)> {
        match &self.nodes[cond as usize] {
            ValNode {
                op: ValOp::Bin(op),
                args,
            } if *op == Op::Le as u32 && args.len() == 2 => Some((args[0], args[1])),
            _ => None,
        }
    }

    fn has_bound_order_fact(&self, lo: ValueId, hi: ValueId) -> bool {
        if let (Some(lo_value), Some(hi_value)) =
            (self.int_const_value(lo), self.int_const_value(hi))
        {
            return lo_value <= hi_value;
        }
        self.bound_order_facts
            .iter()
            .any(|&(fact_lo, fact_hi)| fact_lo == lo && fact_hi == hi)
    }

    fn is_integer_param_value(&self, value: ValueId) -> bool {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return false;
        };
        matches!(self.param_semantic.get(&cid), Some(ParamSemantic::Integer))
    }

    fn is_safe_clamp_integer_value(&self, value: ValueId) -> bool {
        self.int_const_value(value).is_some() || self.is_integer_param_value(value)
    }

    fn proof_backed_clamp_value(
        &mut self,
        x: ValueId,
        lo: ValueId,
        hi: ValueId,
    ) -> Option<ValueId> {
        if self.is_safe_clamp_integer_value(x)
            && self.is_safe_clamp_integer_value(lo)
            && self.is_safe_clamp_integer_value(hi)
            && self.has_bound_order_fact(lo, hi)
        {
            Some(self.mk(ValOp::Clamp, vec![x, lo, hi]))
        } else {
            None
        }
    }

    fn canonicalize_proof_backed_clamp(&mut self, value: ValueId) -> Option<ValueId> {
        if !matches!(self.nodes[value as usize].op, ValOp::Bin(o) if o == MIN_CODE || o == MAX_CODE)
        {
            return None;
        }
        let candidates = self.clamp_minmax_candidates(value);
        if candidates.is_empty() {
            return None;
        }
        self.clamp_candidate_count += 1;
        let mut proven: Vec<_> = candidates
            .into_iter()
            .filter(|&(x, lo, hi)| {
                self.is_safe_clamp_integer_value(x)
                    && self.is_safe_clamp_integer_value(lo)
                    && self.is_safe_clamp_integer_value(hi)
                    && self.has_bound_order_fact(lo, hi)
            })
            .collect();
        proven.sort_unstable();
        proven.dedup();
        if proven.len() != 1 {
            return None;
        }
        self.clamp_proof_backed_candidate_count += 1;
        let (x, lo, hi) = proven[0];
        self.proof_backed_clamp_value(x, lo, hi)
    }

    fn clamp_minmax_candidates(&self, value: ValueId) -> Vec<(ValueId, ValueId, ValueId)> {
        let mut out = Vec::new();
        if let Some((outer_a, outer_b)) = self.bin_args(value, MIN_CODE) {
            for (inner, hi) in [(outer_a, outer_b), (outer_b, outer_a)] {
                if let Some((inner_a, inner_b)) = self.bin_args(inner, MAX_CODE) {
                    out.push((inner_a, inner_b, hi));
                    out.push((inner_b, inner_a, hi));
                }
            }
        }
        if let Some((outer_a, outer_b)) = self.bin_args(value, MAX_CODE) {
            for (inner, lo) in [(outer_a, outer_b), (outer_b, outer_a)] {
                if let Some((inner_a, inner_b)) = self.bin_args(inner, MIN_CODE) {
                    out.push((inner_a, lo, inner_b));
                    out.push((inner_b, lo, inner_a));
                }
            }
        }
        out
    }

    fn bin_args(&self, value: ValueId, want: u32) -> Option<(ValueId, ValueId)> {
        match &self.nodes[value as usize] {
            ValNode {
                op: ValOp::Bin(op),
                args,
            } if *op == want && args.len() == 2 => Some((args[0], args[1])),
            _ => None,
        }
    }

    fn bin_other_arg(&self, value: ValueId, want: u32, known: ValueId) -> Option<ValueId> {
        let (left, right) = self.bin_args(value, want)?;
        if left == known {
            Some(right)
        } else if right == known {
            Some(left)
        } else {
            None
        }
    }

    /// Does this branch unconditionally exit its enclosing block (return / throw / break /
    /// continue on every path)? Conservative: a block exits iff its last statement does;
    /// an `if` exits iff both arms do.
    fn branch_exits(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue => true,
            NodeKind::ExprStmt => self.il.children(node).first().is_some_and(|&expr| {
                matches!(
                    self.il.kind(expr),
                    NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
                )
            }),
            NodeKind::Block => self
                .il
                .children(node)
                .last()
                .is_some_and(|&c| self.branch_exits(c)),
            NodeKind::If => {
                let k = self.il.children(node);
                k.len() >= 3 && self.branch_exits(k[1]) && self.branch_exits(k[2])
            }
            _ => false,
        }
    }

    fn branch_returns(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Return => true,
            NodeKind::Block => self
                .il
                .children(node)
                .last()
                .is_some_and(|&c| self.branch_returns(c)),
            NodeKind::If => {
                let k = self.il.children(node);
                k.len() >= 3 && self.branch_returns(k[1]) && self.branch_returns(k[2])
            }
            _ => false,
        }
    }

    fn is_effect_free_throw_body(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::Throw => true,
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.il.kind(expr) == NodeKind::Throw),
            NodeKind::Block => {
                let Some((&last, prefix)) = self.il.children(node).split_last() else {
                    return false;
                };
                self.is_effect_free_throw_body(last)
                    && prefix
                        .iter()
                        .all(|&stmt| self.is_effect_free_throw_prefix(stmt))
            }
            _ => false,
        }
    }

    fn is_effect_free_throw_prefix(&self, node: NodeId) -> bool {
        match self.il.kind(node) {
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_none_or(|&expr| crate::is_pure(self.il, expr)),
            NodeKind::Block => self
                .il
                .children(node)
                .iter()
                .all(|&stmt| self.is_effect_free_throw_prefix(stmt)),
            NodeKind::Seq => self.il.children(node).is_empty(),
            _ => false,
        }
    }

    fn is_effect_free_static_err_body(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        match self.il.kind(node) {
            NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.expr_is_static_runtime_err(expr, env)),
            NodeKind::Return => self
                .il
                .children(node)
                .first()
                .is_some_and(|&expr| self.expr_is_static_runtime_err(expr, env)),
            NodeKind::Assign => self.assign_is_static_runtime_err(node, env),
            NodeKind::Block => {
                let Some((&last, prefix)) = self.il.children(node).split_last() else {
                    return false;
                };
                prefix
                    .iter()
                    .all(|&stmt| self.is_effect_free_throw_prefix(stmt))
                    && self.is_effect_free_static_err_body(last, env)
            }
            _ => false,
        }
    }

    fn assign_is_static_runtime_err(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 {
            return false;
        }
        let target = kids[0];
        let rhs = kids[1];
        if self.expr_is_static_runtime_err(rhs, env) {
            return crate::is_pure(self.il, rhs);
        }
        crate::is_pure(self.il, rhs) && self.assignment_target_is_static_runtime_err(target, env)
    }

    fn assignment_target_is_static_runtime_err(
        &mut self,
        target: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        match self.il.kind(target) {
            NodeKind::Field => self
                .il
                .children(target)
                .first()
                .is_some_and(|&receiver| self.expr_is_static_runtime_err(receiver, env)),
            NodeKind::Index => {
                self.il
                    .children(target)
                    .to_vec()
                    .split_first()
                    .is_some_and(|(&base, rest)| {
                        if self.expr_is_static_runtime_err(base, env) {
                            return true;
                        }
                        crate::is_pure(self.il, base)
                            && rest
                                .first()
                                .is_some_and(|&index| self.expr_is_static_runtime_err(index, env))
                    })
            }
            _ => false,
        }
    }

    fn expr_is_static_runtime_err(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        if self.il.kind(expr) == NodeKind::Seq {
            for child in self.il.children(expr).to_vec() {
                if self.expr_is_static_runtime_err(child, env) {
                    return crate::is_pure(self.il, child);
                }
                if !crate::is_pure(self.il, child) {
                    return false;
                }
            }
            return false;
        }
        if self.il.kind(expr) == NodeKind::HoF
            && matches!(
                self.il.node(expr).payload,
                Payload::HoF(HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter)
                    | Payload::HoF(HoFKind::FilterMap)
            )
        {
            let kids = self.il.children(expr).to_vec();
            return kids
                .first()
                .is_some_and(|&coll| self.expr_is_static_non_empty_seq(coll))
                && kids
                    .get(1)
                    .is_some_and(|&lambda| self.lambda_body_is_static_runtime_err(lambda, env));
        }
        if self.il.kind(expr) == NodeKind::If {
            let kids = self.il.children(expr).to_vec();
            let Some(&cond) = kids.first() else {
                return false;
            };
            if self.expr_is_static_runtime_err(cond, env) {
                return true;
            }
            let cond_value = self.eval(cond, env);
            return match self.bool_const(cond_value) {
                Some(true) => kids
                    .get(1)
                    .is_some_and(|&then_expr| self.expr_is_static_runtime_err(then_expr, env)),
                Some(false) => kids
                    .get(2)
                    .is_some_and(|&else_expr| self.expr_is_static_runtime_err(else_expr, env)),
                None => false,
            };
        }
        if self.il.kind(expr) == NodeKind::Call && self.call_has_static_runtime_arg_err(expr, env) {
            return true;
        }
        if self.il.kind(expr) == NodeKind::UnOp {
            return self
                .il
                .children(expr)
                .first()
                .is_some_and(|&operand| self.expr_is_static_runtime_err(operand, env));
        }
        if self.il.kind(expr) == NodeKind::Field {
            return self
                .il
                .children(expr)
                .first()
                .is_some_and(|&receiver| self.expr_is_static_runtime_err(receiver, env));
        }
        if self.il.kind(expr) == NodeKind::Index {
            let kids = self.il.children(expr).to_vec();
            if kids.len() != 2 {
                return false;
            }
            if self.expr_is_static_runtime_err(kids[0], env) {
                return true;
            }
            return crate::is_pure(self.il, kids[0])
                && self.expr_is_static_runtime_err(kids[1], env);
        }
        if self.il.kind(expr) != NodeKind::BinOp {
            return false;
        }
        let kids = self.il.children(expr).to_vec();
        if kids.len() != 2 {
            return false;
        }
        let Payload::Op(op) = self.il.node(expr).payload else {
            return false;
        };
        if self.expr_is_static_runtime_err(kids[0], env) {
            return true;
        }
        if !crate::is_pure(self.il, kids[0]) {
            return false;
        }
        if self.expr_is_static_runtime_err(kids[1], env) {
            return true;
        }
        if !crate::is_pure(self.il, kids[1]) {
            return false;
        }
        let rhs = self.eval(kids[1], env);
        match op {
            Op::Div | Op::Mod => self.int_const_eq(rhs, 0),
            Op::Pow => self
                .static_int_expr(kids[1])
                .is_some_and(|exp| !(0..=u32::MAX as i64).contains(&exp)),
            _ => false,
        }
    }

    fn call_has_static_runtime_arg_err(
        &mut self,
        call: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let kids = self.il.children(call).to_vec();
        match self.il.node(call).payload {
            Payload::Builtin(Builtin::ValueOrDefault) => kids
                .first()
                .is_some_and(|&value| self.expr_is_static_runtime_err(value, env)),
            Payload::Builtin(Builtin::Any | Builtin::All) => kids
                .first()
                .is_some_and(|&coll| self.expr_is_static_runtime_err(coll, env)),
            Payload::Builtin(Builtin::Reduce) => {
                kids.get(1)
                    .is_some_and(|&coll| self.expr_is_static_runtime_err(coll, env))
                    || kids
                        .get(2)
                        .is_some_and(|&init| self.expr_is_static_runtime_err(init, env))
                    || (kids
                        .get(1)
                        .is_some_and(|&coll| self.expr_is_static_non_empty_seq(coll))
                        && kids.first().is_some_and(|&lambda| {
                            self.lambda_body_is_static_runtime_err(lambda, env)
                        }))
            }
            Payload::Builtin(Builtin::Range) => {
                self.call_args_have_static_runtime_err(kids.iter().copied(), env)
                    || self.range_has_static_zero_step(&kids)
            }
            Payload::Builtin(_) => self.call_args_have_static_runtime_err(kids, env),
            _ => self.call_args_have_static_runtime_err(kids.into_iter().skip(1), env),
        }
    }

    fn range_has_static_zero_step(&self, kids: &[NodeId]) -> bool {
        kids.len() == 3
            && kids.iter().all(|&arg| crate::is_pure(self.il, arg))
            && self.static_int_expr(kids[2]) == Some(0)
    }

    fn call_args_have_static_runtime_err<I>(
        &mut self,
        args: I,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool
    where
        I: IntoIterator<Item = NodeId>,
    {
        for arg in args {
            if self.expr_is_static_runtime_err(arg, env) {
                return crate::is_pure(self.il, arg);
            }
            if !crate::is_pure(self.il, arg) {
                return false;
            }
        }
        false
    }

    fn expr_is_static_non_empty_seq(&self, expr: NodeId) -> bool {
        self.il.kind(expr) == NodeKind::Seq && !self.il.children(expr).is_empty()
    }

    fn lambda_body_is_static_runtime_err(
        &mut self,
        lambda: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return false;
        }
        self.il
            .children(lambda)
            .last()
            .is_some_and(|&body| self.is_effect_free_static_err_body(body, env))
    }

    fn static_int_expr(&self, expr: NodeId) -> Option<i64> {
        let node = self.il.node(expr);
        match (node.kind, node.payload) {
            (NodeKind::Lit, Payload::LitInt(value)) => Some(value),
            (NodeKind::UnOp, Payload::Op(Op::Pos)) => self
                .il
                .children(expr)
                .first()
                .and_then(|&child| self.static_int_expr(child)),
            (NodeKind::UnOp, Payload::Op(Op::Neg)) => self
                .il
                .children(expr)
                .first()
                .and_then(|&child| self.static_int_expr(child))
                .and_then(i64::checked_neg),
            _ => None,
        }
    }

    /// Walk a container (class/module body), folding each contained method's behavior
    /// into the current sinks. A `Func` is processed in its own parameter scope (its
    /// returns/effects become the container's), so the container's fingerprint is the
    /// aggregate of its methods; `Block` wrappers are descended; anything else (field
    /// initializers, attribute assigns) is processed as a normal statement.
    fn process_container(&mut self, node: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        for c in self.il.children(node).to_vec() {
            match self.il.kind(c) {
                NodeKind::Func => {
                    let kids = self.il.children(c).to_vec();
                    let mut menv = env.clone();
                    let saved_param_semantic = self.param_semantic.clone();
                    self.param_semantic.clear();
                    self.seed_param_semantics(c);
                    let mut pos = 0u32;
                    for &k in &kids {
                        if self.il.kind(k) == NodeKind::Param {
                            if let Payload::Cid(cid) = self.il.node(k).payload {
                                let v = self.mk(ValOp::Input(pos), vec![]);
                                menv.insert(cid, v);
                                pos += 1;
                            }
                        }
                    }
                    if let Some(&body) = kids.last() {
                        self.process_stmt(body, &mut menv);
                    }
                    self.param_semantic = saved_param_semantic;
                }
                NodeKind::Block => self.process_container(c, env),
                _ => self.process_stmt(c, env),
            }
        }
    }

    fn compact_coupled_recurrence(&mut self, cid: u32, value: ValueId) -> ValueId {
        let should_compact = self.loop_recurrence.as_ref().is_some_and(|scope| {
            scope.loop_values.contains_key(&cid)
                && self.references_nonself_loop_dependency(value, cid, scope)
        });
        if !should_compact {
            return value;
        }

        let key = self
            .loop_recurrence
            .as_ref()
            .and_then(|scope| scope.loop_keys.get(&cid))
            .copied()
            .unwrap_or(cid);
        let h = combine(combine(0xC0AD_D1EC, key as u64), self.vhash[value as usize]);
        self.mk(ValOp::Recurrence(h), vec![])
    }

    fn references_nonself_loop_dependency(
        &self,
        value: ValueId,
        self_cid: u32,
        scope: &LoopRecurrenceScope,
    ) -> bool {
        let self_key = scope.loop_keys.get(&self_cid).copied();
        let mut stack = vec![value];
        let mut seen = FxHashSet::default();
        while let Some(v) = stack.pop() {
            if !seen.insert(v) {
                continue;
            }
            match &self.nodes[v as usize].op {
                ValOp::Loop(key) if Some(*key) != self_key && scope.loop_key_set.contains(key) => {
                    return true;
                }
                ValOp::Recurrence(_) => return true,
                _ => {}
            }
            stack.extend(self.nodes[v as usize].args.iter().copied());
        }
        false
    }

    fn process_stmt(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        match self.il.kind(stmt) {
            NodeKind::Block => self.process_block(stmt, env),
            NodeKind::Assign => {
                let kids = self.il.children(stmt).to_vec();
                if kids.len() == 2 {
                    // Go-style functional append `r = append(r, item)` to an ACTIVE builder var
                    // IS the per-element build (the reassignment is the append), exactly like
                    // the effect-form `r.append(item)`. Record the contribution so the loop
                    // becomes `Map(elem, contrib)` instead of an opaque reassign.
                    if self.try_record_reassign_append(kids[0], kids[1], env) {
                        return;
                    }
                    let rhs = self.eval(kids[1], env);
                    if self.il.kind(kids[0]) == NodeKind::Var {
                        if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                            let rhs = self.compact_coupled_recurrence(c, rhs);
                            env.insert(c, rhs);
                            return;
                        }
                    }
                    // A field write (`self.x = v`) updates per-field state (last-write-
                    // wins), flushed as a (field, final-value) sink later — order-
                    // insensitive across distinct fields, correct for overwrites.
                    if self.il.kind(kids[0]) == NodeKind::Field {
                        if let Payload::Name(s) = self.il.node(kids[0]).payload {
                            let name = self.interner.symbol_hash(s);
                            let g = self.guarded(rhs);
                            self.field_env.insert(name, g);
                            return;
                        }
                    }
                    // `d[k] = v` to an ACTIVE dict-builder records a `DictEntry` contribution
                    // (so the loop becomes a `Map` of entries, converging with `{k:v for x}`).
                    if self.try_record_index_assign(kids[0], rhs, env) {
                        return;
                    }
                    // store into an index / computed target → an ordered effect
                    let target = self.eval(kids[0], env);
                    let st = self.mk(ValOp::Call(0), vec![target, rhs]);
                    self.push_effect(st);
                }
            }
            NodeKind::Return => {
                // A bare `return;` (no value) is still behaviorally significant: as an
                // *early* exit inside a loop/branch it changes which later code runs, and
                // its path guard distinguishes a conditional early-return from an
                // unconditional one. Model it as a guarded void-return sink (previously a
                // valueless return pushed nothing, so two functions differing only in a
                // conditional early `return;` collapsed — cf. the break sink, §AS).
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.mk(ValOp::Const(0xF00D_0000), vec![]),
                };
                self.emit_return(v);
            }
            NodeKind::Throw => {
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.mk(ValOp::Const(0xE22E_0000), vec![]),
                };
                self.emit_throw(v);
            }
            NodeKind::ExprStmt => {
                if let Some(&e) = self.il.children(stmt).first() {
                    if matches!(
                        self.il.kind(e),
                        NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
                    ) {
                        self.process_stmt(e, env);
                        return;
                    }
                    // A `coll.append(x)` to an ACTIVE local builder var records the
                    // per-element contribution (so the loop becomes a `Map`) instead of an
                    // opaque effect. Anything irregular (2nd append, multi-arg) spoils it.
                    if self.try_record_append(e, env) {
                        return;
                    }
                    let v = self.eval(e, env);
                    // an expression statement is kept only if it has effect value
                    self.push_effect(v);
                }
            }
            NodeKind::If => self.process_if(stmt, env),
            NodeKind::Loop => self.process_loop(stmt, env),
            NodeKind::Try => {
                let kids = self.il.children(stmt).to_vec();
                if kids.len() == 2 && self.is_effect_free_static_err_body(kids[0], env) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                if kids.len() == 2 && self.branch_returns(kids[0]) {
                    self.process_stmt(kids[0], env);
                    return;
                }
                if kids.len() == 2 && self.is_effect_free_throw_body(kids[0]) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                for c in kids {
                    self.process_stmt(c, env);
                }
            }
            NodeKind::Break => {
                // An early `break` truncates the loop to a prefix — the result is NOT
                // the full-iteration fold. Record the break's *path condition* as a sink
                // so an early-exit loop no longer fingerprints identically to one that
                // runs to completion, and two loops breaking on different conditions stay
                // distinct. (`continue` needs no handling: desugaring already hoists the
                // remainder of the body into the negated guard, so its filtering effect
                // is captured by the normal path-guard machinery.)
                let marker = self.mk(ValOp::Const(0xB2EA_C0DE), vec![]);
                let g = self.guarded(marker);
                self.sinks.push(Sink::new(SinkKind::Break, g));
            }
            NodeKind::Continue => {}
            _ => {
                // unknown statement: evaluate as effect best-effort
                let v = self.eval(stmt, env);
                self.push_effect(v);
            }
        }
    }

    fn process_if(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(stmt).to_vec();
        if kids.is_empty() {
            return;
        }
        // The condition is *not* a standalone sink: it is captured where it matters —
        // in the `Phi` merge of any variable the branches update, and in the path-guard
        // of any return/effect they perform. Emitting it separately would make a
        // statement-`if` that updates a variable (`if c { x = a }`) diverge from the
        // equivalent ternary (`x = a if c else x`), which has no such sink. (`cond` is
        // still evaluated — for the path stack and so its sub-values are interned.)
        let cond = self.eval(kids[0], env);
        let effect_slot_base = self.effect_slot;

        let mut env_then = env.clone();
        let then_effect_slot = if kids.len() >= 2 {
            self.effect_slot = effect_slot_base;
            self.path.push(cond);
            self.process_stmt(kids[1], &mut env_then);
            self.path.pop();
            self.effect_slot
        } else {
            effect_slot_base
        };
        let mut env_else = env.clone();
        let else_effect_slot = if kids.len() >= 3 {
            self.effect_slot = effect_slot_base;
            let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
            self.path.push(ncond);
            self.process_stmt(kids[2], &mut env_else);
            self.path.pop();
            self.effect_slot
        } else {
            effect_slot_base
        };
        self.effect_slot = then_effect_slot.max(else_effect_slot);

        // Merge: for each var that differs across branches, insert a Phi.
        let mut keys: Vec<u32> = env_then.keys().chain(env_else.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        for cid in keys {
            let base = env.get(&cid).copied();
            let t = env_then.get(&cid).copied().or(base);
            let e = env_else.get(&cid).copied().or(base);
            match (t, e) {
                (Some(tv), Some(ev)) if tv == ev => {
                    env.insert(cid, tv);
                }
                (Some(tv), Some(ev)) => {
                    let phi = self.mk(ValOp::Phi, vec![cond, tv, ev]);
                    env.insert(cid, phi);
                }
                (Some(v), None) | (None, Some(v)) => {
                    env.insert(cid, v);
                }
                (None, None) => {}
            }
        }
    }

    fn process_loop(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(stmt).to_vec();
        let kind = match self.il.node(stmt).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let body = match kids.last() {
            Some(&b) => b,
            None => return,
        };
        if kind == LoopKind::While
            && kids.len() == 2
            && self.loop_entry_condition_is_proven_false(kids[0], env)
        {
            return;
        }

        // Discover the loop's *element source* so per-element computations converge
        // across loop shapes (the §AH representation axis):
        //   • `for pat in iterable`        → element is the pattern variable;
        //   • `while i < len(xs) { … i+=1 }` → element is `xs[i]` for the induction
        //     variable `i` (index bookkeeping is iteration mechanics, not a result).
        // Both bind a single canonical `Elem(iterable)` value; an indexed `while`
        // additionally rewrites `xs[i]` → `Elem(xs)` and drops the induction variable.
        // Bindings applied to the loop's pattern/index variables (cid → value), and the
        // set of values that play the role of an *index*. Any `C[idx]` for such an
        // `idx` is the element of `C`, so indexed iteration converges with value
        // iteration. Iteration variables are not accumulators.
        let mut pattern_bindings: Vec<(u32, ValueId)> = Vec::new();
        let mut index_vals: FxHashSet<ValueId> = FxHashSet::default();
        let mut induction: FxHashSet<u32> = FxHashSet::default();

        match kind {
            LoopKind::ForEach if kids.len() >= 3 => {
                let pat = kids[0];
                let it = kids[1];
                if let Some(c) = self.range_len_collection(it) {
                    // `for i in range(len(C))`: `i` is a canonical index into `C`.
                    let cv = self.eval(c, env);
                    let ix = self.idx(cv);
                    index_vals.insert(ix);
                    for cid in self.pattern_cids(pat) {
                        pattern_bindings.push((cid, ix));
                    }
                } else if self.il.kind(it) == NodeKind::Call
                    && matches!(
                        self.il.node(it).payload,
                        Payload::Builtin(Builtin::Enumerate)
                    )
                {
                    // `for i, x in enumerate(C)`: `i` is the index, `x` the element.
                    let cids = self.pattern_cids(pat);
                    if let Some(&cnode) = self.il.children(it).first() {
                        let cv = self.eval(cnode, env);
                        let ix = self.idx(cv);
                        let el = self.elem(cv);
                        index_vals.insert(ix);
                        if cids.len() >= 2 {
                            pattern_bindings.push((cids[0], ix));
                            pattern_bindings.push((cids[1], el));
                        } else if let Some(&only) = cids.first() {
                            pattern_bindings.push((only, el));
                        }
                    }
                } else {
                    // Value iteration `for x in C` (or `for i in range(n)`): the
                    // pattern var is an element of the iterable. NOTE: we do *not* treat
                    // this element as a collection index — only a provably-*full* range
                    // (`range_len_collection`, above) licenses `C[i] → Elem(C)`. A bare
                    // `range(n)` or partial `range(1, len)` indexing `C[i]` must keep its
                    // `Index(C, …)` so it stays distinct from the full-range loop (else a
                    // subset sum merges with the full sum — a soundness bug).
                    let iv = self.eval(it, env);
                    let e = self.elem(iv);
                    for cid in self.pattern_cids(pat) {
                        pattern_bindings.push((cid, e));
                    }
                }
            }
            _ => {
                let cond = kids
                    .first()
                    .filter(|&&c| self.il.kind(c) != NodeKind::Block);
                // A genuine loop counter both steps by a constant *and* governs the
                // loop condition. An accumulator updated by `acc = acc + 1` (a counting
                // reduction) matches the `i = i ± c` shape too, so `induction_vars`
                // alone misclassifies it as iteration mechanics — which binds it to the
                // index and destroys the reduction (it would never reach a `Reduce`).
                // Intersect with the variables the condition actually mentions so only
                // the real counter(s) are treated as indices; the accumulator stays an
                // accumulator. (Sum loops were spared by luck — `sum += xs[i]` has a
                // non-literal operand, so `is_increment` already rejected them; counting
                // loops like `if p: count += 1` were the ones that collapsed.)
                let cond_cids = cond
                    .map(|&c| mentioned_cids(self.il, c))
                    .unwrap_or_default();
                induction = induction_vars(self.il, body)
                    .intersection(&cond_cids)
                    .copied()
                    .collect();
                let iter_node =
                    cond.and_then(|&c| self.loop_iterable(c, &induction).map(|it| (it, None)));
                let indexed_bound_loop = iter_node.or_else(|| {
                    cond.and_then(|&c| {
                        self.indexed_bound_loop_iterable(c, body, &induction)
                            .map(|(it, bound, cmp)| (it, Some((bound, cmp))))
                    })
                });
                match indexed_bound_loop {
                    // Indexed `while i < len(C)`: the raw `i < len(C)` guard is iteration
                    // mechanics. A counter that steps by +1 from 0 visits *every* index
                    // in order, so `C[i]` is the canonical `Elem(C)` (converges with `for
                    // x in C`). A non-unit stride (`i += 2`) or non-zero start (`i = 1`)
                    // visits a SUBSET — `C[i]` is NOT `Elem(C)` — so bind `i` to a strided
                    // index that encodes start+step (distinct strides stay distinct) and
                    // do NOT license the `C[i] → Elem(C)` rewrite (else a strided sum
                    // merges with the full sum — the while-loop analog of the range bug).
                    Some((it, bound_guard)) if !induction.is_empty() => {
                        let cv = self.eval(it, env);
                        if let Some((bound, cmp)) = bound_guard {
                            let bv = self.eval(bound, env);
                            if !self.full_pointer_length_contract(cmp, cv, bv) {
                                let gv = self.indexed_bound_guard(cmp, bv);
                                self.sinks.push(Sink::new(SinkKind::Cond, gv));
                            } else if let (Some(arr), Some(len)) =
                                (self.input_key(cv), self.input_key(bv))
                            {
                                // The bound `n` was dropped as "length of the array" — record
                                // (array_pos, length_pos) so the oracle interprets under n=len.
                                self.contracts.push((arr, len));
                            }
                        }
                        let zero = self.int_const(0);
                        for &i in &induction {
                            let step = induction_step(self.il, body, i);
                            let start_zero = env.get(&i).is_some_and(|&s| s == zero);
                            if step == Some(1) && start_zero {
                                let ix = self.idx(cv);
                                index_vals.insert(ix);
                                pattern_bindings.push((i, ix));
                            } else {
                                let start_val = env.get(&i).copied().unwrap_or(zero);
                                let step_val =
                                    self.int_const(step.unwrap_or(0).rem_euclid(1 << 24) as u32);
                                let base = self.idx(cv);
                                let h = combine(
                                    self.vhash[base as usize],
                                    combine(
                                        self.vhash[start_val as usize],
                                        self.vhash[step_val as usize],
                                    ),
                                );
                                let strided =
                                    self.mk(ValOp::Idx(h), vec![base, start_val, step_val]);
                                pattern_bindings.push((i, strided));
                            }
                        }
                    }
                    // Plain `while`/other: keep the raw condition; no element model.
                    _ => {
                        induction.clear();
                        if let Some(&c) = cond {
                            let cv = self.eval(c, env);
                            self.sinks.push(Sink::new(SinkKind::Cond, cv));
                        }
                    }
                }
            }
        }

        let mut assigned = FxHashSet::default();
        collect_assigned(self.il, body, &mut assigned);
        // List-builder vars (incl. Go's `r = append(r, …)`, which makes `r` assigned) are
        // activated as builders, NOT seeded as numeric loop-carried recurrences — otherwise a
        // Go builder var would be both a `Loop` placeholder and a `Map` build and collapse. The
        // seed (empty-collection) check reads the PRE-loop `env` (the real `[]` seed; the body's
        // reassignment would otherwise hide it). Builders are excluded from `carried`.
        let builder_cands = self.builder_candidates(body, env);
        let builder_set: FxHashSet<u32> = builder_cands.iter().copied().collect();
        for &c in &builder_cands {
            self.building.insert(c, None);
        }
        let mut carried: Vec<u32> = assigned
            .iter()
            .copied()
            .filter(|c| !builder_set.contains(c))
            .collect();
        carried.sort_unstable();

        // Seed each loop-carried variable with a symbolic "previous iteration" value
        // so the body expresses its update as a *recurrence* over `Loop(cid)`.
        let mut body_env = env.clone();
        let mut loop_vals: FxHashMap<u32, ValueId> = FxHashMap::default();
        let loop_key_base = self.next_loop_key_base;
        let loop_key_count = u32::try_from(carried.len()).unwrap_or(u32::MAX);
        self.next_loop_key_base = self
            .next_loop_key_base
            .wrapping_add(loop_key_count.saturating_add(1));
        let mut loop_keys: FxHashMap<u32, u32> = FxHashMap::default();
        let mut loop_key_set: FxHashSet<u32> = FxHashSet::default();
        for (slot, &cid) in carried.iter().enumerate() {
            let key = loop_key_base.wrapping_add(slot as u32);
            loop_keys.insert(cid, key);
            loop_key_set.insert(key);
            let lv = self.mk(ValOp::Loop(key), vec![]);
            loop_vals.insert(cid, lv);
            body_env.insert(cid, lv);
        }
        // Pattern/index bindings override the `Loop` placeholder for iteration vars.
        let iter_vars: FxHashSet<u32> = pattern_bindings.iter().map(|&(c, _)| c).collect();
        for &(cid, v) in &pattern_bindings {
            body_env.insert(cid, v);
        }

        // Every `C[idx]` for an index value `idx` is morally `Elem(C)`. Rewrite it
        // everywhere the body deposits values — the sinks it pushes (guard conditions,
        // effects) and the carried recurrences — so indexed iteration (`while i<len`,
        // `for i in range(len)`, multi-collection `a[i]*b[i]`) matches value iteration,
        // even when the accumulation is conditional (filter+reduce) not a clean fold.
        // (List-builder vars were activated above, before `carried`, so a Go functional-append
        // builder is excluded from numeric recurrence seeding.)
        let sink_start = self.sinks.len();
        let outer_recurrence = self.loop_recurrence.replace(LoopRecurrenceScope {
            loop_values: loop_vals.clone(),
            loop_keys,
            loop_key_set,
        });
        let pre_body_env = body_env.clone();
        self.process_stmt(body, &mut body_env);
        self.loop_recurrence = outer_recurrence;
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            for idx in sink_start..self.sinks.len() {
                let v = self.sinks[idx].value;
                self.sinks[idx].value = self.rewrite_indices(v, &index_vals, &mut memo);
            }
            for &cid in &carried {
                if let Some(&v) = body_env.get(&cid) {
                    let nv = self.rewrite_indices(v, &index_vals, &mut memo);
                    body_env.insert(cid, nv);
                }
            }
        }
        let mut flag_break_reduction = None;
        for &cid in &carried {
            if let Some(&init) = env.get(&cid) {
                if let Some(v) =
                    self.flag_break_reduction(body, cid, init, &pre_body_env, &index_vals)
                {
                    flag_break_reduction = Some((cid, v));
                    self.sinks.truncate(sink_start);
                    break;
                }
            }
        }
        // Finalize list builders: `r = []; for x: r.append(f(x))` → `r = Map(elem, f(x))`,
        // converging the loop with the comprehension `[f(x) for x in xs]` / `.map`. A
        // guarded append (`if cond: r.append(f(x))`) becomes the filtered map `Map(_, pred)`.
        // If the append happens inside a nested loop, the inner loop has already produced a
        // `Map`/`FlatMap`; wrap that per-outer-iteration collection in a `FlatMap` so
        // `for x: for y: r.append(f(x,y))` converges with `[f(x,y) for x in xs for y in ys]`.
        for &c in &builder_cands {
            if let Some(Some((mut contrib, guard))) = self.building.remove(&c) {
                let map = if !index_vals.is_empty() {
                    let mut memo = FxHashMap::default();
                    contrib = self.rewrite_indices(contrib, &index_vals, &mut memo);
                    match guard {
                        Some(g) => {
                            let g = self.rewrite_indices(g, &index_vals, &mut memo);
                            self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, g])
                        }
                        None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                    }
                } else {
                    match guard {
                        Some(g) => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, g]),
                        None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                    }
                };
                env.insert(c, map);
            } else if let Some(&nested) = body_env.get(&c) {
                if let Some(flat) = self.flat_map_builder_value(nested, &pattern_bindings) {
                    env.insert(c, flat);
                }
                self.building.remove(&c);
            } else {
                self.building.remove(&c);
            }
        }

        // For each loop-carried accumulator, recognize an associative-commutative
        // reduction `acc = acc ⊕ contrib` and canonicalize it to a `Reduce` value —
        // so sum/product/min/max/count loops converge regardless of loop shape,
        // accumulator name, or operand grouping (the §AH representation axis). The
        // per-element `contrib` keys the value, so a `+`-loop and a `*`-loop (or
        // `a[i]*b[i]` vs `a[i]+b[i]`) stay distinct (the behavior axis). When the
        // update is not a clean reduction, thread the raw recurrence (still better
        // than a bare opaque `Loop` value reaching the sinks).
        let mut reduction_cache = ReductionCache::default();
        for &cid in &carried {
            if let Some((flag_cid, v)) = flag_break_reduction {
                if flag_cid == cid {
                    env.insert(cid, v);
                    continue;
                }
            }
            if let Some(&init) = env.get(&cid) {
                if let Some(v) =
                    self.ordered_string_concat_loop(body, cid, init, &pre_body_env, &index_vals)
                {
                    env.insert(cid, v);
                    continue;
                }
            }
            if iter_vars.contains(&cid) || induction.contains(&cid) {
                continue; // iteration mechanics, not an accumulator
            }
            let Some(&newv) = body_env.get(&cid) else {
                continue;
            };
            let loopv = loop_vals[&cid];
            let loop_context: Vec<ValueId> = pattern_bindings.iter().map(|&(_, v)| v).collect();
            match (
                env.get(&cid).copied(),
                self.as_loop_reduction_step(newv, loopv, &loop_context, &mut reduction_cache),
            ) {
                (Some(init), Some((op, contrib))) => {
                    // Selection reductions (min/max) carry no init; folds carry one.
                    let args = if is_selection_code(op) {
                        vec![contrib]
                    } else {
                        vec![init, contrib]
                    };
                    let red = self.mk(ValOp::Reduce(op), args);
                    env.insert(cid, red);
                }
                (init, _) => {
                    // A non-reduction loop-carried value still depends on its pre-loop
                    // SEED. The compact `Recurrence` key is the per-iteration update
                    // expression ONLY, so `acc = a` (a parameter seed, the loop returning
                    // `a + Σ`) collapsed onto `acc = 0` (returning `Σ`) — a false merge,
                    // since the two differ exactly by that seed. Re-key the recurrence on
                    // the seed as well so the seed reaches the fingerprint. (Clean
                    // reductions above already carry their `init`.)
                    let v = match (init, self.nodes[newv as usize].op.clone()) {
                        (Some(init), ValOp::Recurrence(h)) => {
                            self.mk(ValOp::Recurrence(h), vec![init])
                        }
                        _ => newv,
                    };
                    env.insert(cid, v);
                }
            }
        }
    }

    fn flag_break_reduction(
        &mut self,
        body: NodeId,
        cid: u32,
        init: ValueId,
        env: &FxHashMap<u32, ValueId>,
        index_vals: &FxHashSet<ValueId>,
    ) -> Option<ValueId> {
        let init_bool = self.bool_const(init)?;
        let (cond_node, assigned_bool) = self.flag_break_if(body, cid)?;
        if init_bool == assigned_bool {
            return None;
        }
        let mut cond = self.eval(cond_node, env);
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            cond = self.rewrite_indices(cond, index_vals, &mut memo);
        }
        if !self.refs_elem(cond) {
            return None;
        }
        if !init_bool && assigned_bool {
            Some(self.mk(ValOp::Reduce(REDUCE_ANY), vec![cond]))
        } else {
            let pred = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
            Some(self.mk(ValOp::Reduce(REDUCE_ALL), vec![pred]))
        }
    }

    fn flat_map_builder_value(
        &mut self,
        value: ValueId,
        pattern_bindings: &[(u32, ValueId)],
    ) -> Option<ValueId> {
        let outer_elem = pattern_bindings.first()?.1;
        let op = self.nodes[value as usize].op.clone();
        match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 || k == HoFKind::FlatMap as u32 => {
                Some(self.mk(ValOp::Hof(HoFKind::FlatMap as u32), vec![outer_elem, value]))
            }
            _ => None,
        }
    }

    fn flag_break_if(&self, body: NodeId, cid: u32) -> Option<(NodeId, bool)> {
        let stmts = self.direct_block_statements(body);
        if stmts.len() != 1 || self.il.kind(stmts[0]) != NodeKind::If {
            return None;
        }
        let if_kids = self.il.children(stmts[0]);
        if if_kids.len() != 2 {
            return None;
        }
        let branch = self.direct_block_statements(if_kids[1]);
        if branch.len() != 2 || self.il.kind(branch[1]) != NodeKind::Break {
            return None;
        }
        Some((if_kids[0], self.flag_assignment(branch[0], cid)?))
    }

    fn ordered_string_concat_loop(
        &mut self,
        body: NodeId,
        cid: u32,
        init: ValueId,
        env: &FxHashMap<u32, ValueId>,
        index_vals: &FxHashSet<ValueId>,
    ) -> Option<ValueId> {
        if !self.is_empty_string_value(init) {
            return None;
        }
        let contrib_node = self.ordered_concat_contribution(body, cid)?;
        let mut contrib = self.eval(contrib_node, env);
        if !index_vals.is_empty() {
            let mut memo = FxHashMap::default();
            contrib = self.rewrite_indices(contrib, index_vals, &mut memo);
        }
        if !self.refs_elem(contrib) {
            return None;
        }
        let sep = self.empty_string_value();
        Some(self.mk(ValOp::Reduce(ORDERED_STRING_JOIN), vec![sep, contrib]))
    }

    fn ordered_concat_contribution(&self, body: NodeId, cid: u32) -> Option<NodeId> {
        let stmts = self.direct_block_statements(body);
        if stmts.len() != 1 || self.il.kind(stmts[0]) != NodeKind::Assign {
            return None;
        }
        let kids = self.il.children(stmts[0]);
        if kids.len() != 2 || !self.is_var_cid(kids[0], cid) {
            return None;
        }
        if self.il.kind(kids[1]) != NodeKind::BinOp
            || op_code(self.il.node(kids[1]).payload) != Op::Add as u32
        {
            return None;
        }
        let add = self.il.children(kids[1]);
        if add.len() != 2 || !self.is_var_cid(add[0], cid) {
            return None;
        }
        if mentioned_cids(self.il, add[1]).contains(&cid) {
            return None;
        }
        Some(add[1])
    }

    fn direct_block_statements(&self, node: NodeId) -> Vec<NodeId> {
        if self.il.kind(node) == NodeKind::Block {
            self.il.children(node).to_vec()
        } else {
            vec![node]
        }
    }

    fn flag_assignment(&self, stmt: NodeId, cid: u32) -> Option<bool> {
        if self.il.kind(stmt) != NodeKind::Assign {
            return None;
        }
        let kids = self.il.children(stmt);
        if kids.len() != 2 || !self.is_var_cid(kids[0], cid) {
            return None;
        }
        match self.il.node(kids[1]).payload {
            Payload::LitBool(value) => Some(value),
            _ => None,
        }
    }

    fn is_var_cid(&self, node: NodeId, cid: u32) -> bool {
        matches!(
            (self.il.kind(node), self.il.node(node).payload),
            (NodeKind::Var, Payload::Cid(c)) if c == cid
        )
    }

    fn loop_entry_condition_is_proven_false(
        &self,
        cond: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        if self.condition_atom_is_proven_false(cond, env) {
            return true;
        }
        if self.il.kind(cond) != NodeKind::BinOp
            || op_code(self.il.node(cond).payload) != Op::And as u32
        {
            return false;
        }
        let kids = self.il.children(cond);
        kids.len() == 2 && self.condition_atom_is_proven_false(kids[0], env)
    }

    fn condition_atom_is_proven_false(&self, atom: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        match self.il.node(atom).payload {
            Payload::LitBool(false) if self.il.kind(atom) == NodeKind::Lit => true,
            Payload::Cid(cid) if self.il.kind(atom) == NodeKind::Var => env
                .get(&cid)
                .and_then(|&v| self.bool_const(v))
                .is_some_and(|value| !value),
            Payload::Op(Op::Not) if self.il.kind(atom) == NodeKind::UnOp => {
                let kids = self.il.children(atom);
                if kids.len() != 1 {
                    return false;
                }
                match self.il.node(kids[0]).payload {
                    Payload::LitBool(true) if self.il.kind(kids[0]) == NodeKind::Lit => true,
                    Payload::Cid(cid) if self.il.kind(kids[0]) == NodeKind::Var => env
                        .get(&cid)
                        .and_then(|&v| self.bool_const(v))
                        .is_some_and(|value| value),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// The iterable of a `while i < len(xs)`-style loop: from a comparison whose
    /// bound side is `len(iterable)`, return the `iterable` node. Requires the other
    /// side to reference an induction variable (so we don't misread `a < len(b)`).
    fn loop_iterable(&self, cond: NodeId, induction: &FxHashSet<u32>) -> Option<NodeId> {
        if self.il.kind(cond) != NodeKind::BinOp {
            return None;
        }
        let kids = self.il.children(cond).to_vec();
        if kids.len() != 2 {
            return None;
        }
        let mentions_ind = |n: NodeId| {
            matches!((self.il.kind(n), self.il.node(n).payload),
                (NodeKind::Var, Payload::Cid(c)) if induction.contains(&c))
        };
        if !kids.iter().any(|&k| mentions_ind(k)) {
            return None;
        }
        // The other operand is `len(iterable)` → a Len builtin Call with one arg.
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Call {
                if let Payload::Builtin(Builtin::Len) = self.il.node(k).payload {
                    if let Some(&arg) = self.il.children(k).first() {
                        return Some(arg);
                    }
                }
            }
        }
        None
    }

    /// Conservative C-style pointer+length loop recognition:
    ///
    /// `while i < n { ... xs[i] ...; i += 1 }`
    ///
    /// Unlike `i < len(xs)`, the bound is not intrinsically tied to the collection.
    /// Therefore this only licenses the local `xs[i] -> Elem(xs)` rewrite and records a
    /// bound guard keyed by the normalized comparison and bound value. That lets
    /// C `for`/`while` spellings of the same `(ptr, len)` traversal converge without
    /// claiming the loop is automatically identical to a high-level full-collection
    /// traversal.
    fn indexed_bound_loop_iterable(
        &self,
        cond: NodeId,
        body: NodeId,
        induction: &FxHashSet<u32>,
    ) -> Option<(NodeId, NodeId, u32)> {
        if self.il.kind(cond) != NodeKind::BinOp {
            return None;
        }
        let cmp = op_code(self.il.node(cond).payload);
        let kids = self.il.children(cond);
        if kids.len() != 2 {
            return None;
        }

        let left_ind = self.direct_induction_cid(kids[0], induction);
        let right_ind = self.direct_induction_cid(kids[1], induction);
        let (cid, bound, normalized_cmp) = match (left_ind, right_ind) {
            (Some(cid), None) if !mentioned_cids(self.il, kids[1]).contains(&cid) => {
                (cid, kids[1], cmp)
            }
            (None, Some(cid)) if !mentioned_cids(self.il, kids[0]).contains(&cid) => {
                (cid, kids[0], reverse_cmp_code(cmp)?)
            }
            _ => return None,
        };
        if normalized_cmp != Op::Lt as u32 && normalized_cmp != Op::Le as u32 {
            return None;
        }

        let collection = self.indexed_collection_in_body(body, cid)?;
        Some((collection, bound, normalized_cmp))
    }

    fn direct_induction_cid(&self, node: NodeId, induction: &FxHashSet<u32>) -> Option<u32> {
        match (self.il.kind(node), self.il.node(node).payload) {
            (NodeKind::Var, Payload::Cid(c)) if induction.contains(&c) => Some(c),
            _ => None,
        }
    }

    fn indexed_collection_in_body(&self, node: NodeId, cid: u32) -> Option<NodeId> {
        if self.il.kind(node) == NodeKind::Index {
            let kids = self.il.children(node);
            if kids.len() == 2
                && matches!(
                    (self.il.kind(kids[1]), self.il.node(kids[1]).payload),
                    (NodeKind::Var, Payload::Cid(c)) if c == cid
                )
            {
                return Some(kids[0]);
            }
        }
        for &c in self.il.children(node) {
            if let Some(collection) = self.indexed_collection_in_body(c, cid) {
                return Some(collection);
            }
        }
        None
    }

    fn indexed_bound_guard(&mut self, cmp: u32, bound: ValueId) -> ValueId {
        let marker = self.int_const(0xC10C_0000);
        let cmp_value = self.int_const(0xC10C_1000u32.wrapping_add(cmp));
        self.mk(ValOp::Call(0), vec![marker, cmp_value, bound])
    }

    fn full_pointer_length_contract(&self, cmp: u32, collection: ValueId, bound: ValueId) -> bool {
        if cmp != Op::Lt as u32 {
            return false;
        }
        matches!(
            (self.input_key(collection), self.input_key(bound)),
            // Single pointer-length convention: `(xs, n)`.
            (Some(0), Some(1))
                // Two aligned pointer arrays with shared length: `(a, b, n)`.
                | (Some(0), Some(2))
                | (Some(1), Some(2))
        )
    }

    fn input_key(&self, value: ValueId) -> Option<u32> {
        match self.nodes[value as usize].op {
            ValOp::Input(key) => Some(key),
            _ => None,
        }
    }

    /// Rewrite every `Index(C, idx)` whose index is in `index_vals` to `Elem(C)`,
    /// throughout `val`'s subgraph (DAG-safe, memoized). This is what makes indexed
    /// iteration converge with value iteration: `xs[i]` (any collection, any index
    /// variable) becomes the canonical element of that collection.
    fn rewrite_indices(
        &mut self,
        val: ValueId,
        index_vals: &FxHashSet<ValueId>,
        memo: &mut FxHashMap<ValueId, ValueId>,
    ) -> ValueId {
        if let Some(&m) = memo.get(&val) {
            return m;
        }
        let (op, args) = {
            let n = &self.nodes[val as usize];
            (n.op.clone(), n.args.clone())
        };
        let new_args: Vec<ValueId> = args
            .iter()
            .map(|&a| self.rewrite_indices(a, index_vals, memo))
            .collect();
        // `C[idx]` with an index-role `idx` → `Elem(C)`.
        let r = if matches!(op, ValOp::Index)
            && new_args.len() == 2
            && index_vals.contains(&new_args[1])
        {
            self.elem(new_args[0])
        } else if new_args == args {
            val
        } else {
            self.mk(op, new_args)
        };
        memo.insert(val, r);
        r
    }

    /// Recognize an accumulator update `acc = acc ⊕ contrib` (⊕ associative and
    /// commutative) where `acc` is the previous-iteration value `loopv`. Returns the
    /// operator code and the canonical per-element `contrib` (with `acc` removed), or
    /// `None` if the update is not a single clean reduction step.
    fn as_reduction(&mut self, val: ValueId, loopv: ValueId) -> Option<(u32, ValueId)> {
        let mut cache = ReductionCache::default();
        self.as_reduction_cached(val, loopv, &mut cache)
    }

    fn as_loop_reduction_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        loop_context: &[ValueId],
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        if let ValOp::Reduce(op) = self.nodes[val as usize].op {
            let args = self.nodes[val as usize].args.clone();
            if is_selection_code(op)
                && args.len() == 1
                && !self.references_cached(args[0], loopv, cache)
                && self.references_any_cached(args[0], loop_context, cache)
            {
                return Some((op, args[0]));
            }
            if args.len() == 2
                && args[0] == loopv
                && !self.references_cached(args[1], loopv, cache)
                && self.references_any_cached(args[1], loop_context, cache)
            {
                return Some((op, args[1]));
            }
        }
        self.as_reduction_cached(val, loopv, cache)
    }

    fn as_reduction_cached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let key = (val, loopv);
        if let Some(cached) = cache.reductions.get(&key).copied() {
            return cached;
        }
        let result = self.as_reduction_uncached(val, loopv, cache);
        cache.reductions.insert(key, result);
        result
    }

    fn as_reduction_uncached(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        // Guarded (filtered) reduction: `if cond { acc = acc ⊕ contrib }` merges to
        // `Phi(cond, ⊕(acc, contrib), acc)`. Canonicalize to `Reduce(⊕, init, cond ?
        // contrib : identity)` so a filtered loop converges with `sum(c for x if cond)`
        // and the per-element contribution becomes 0/1 (the op identity) when filtered.
        if matches!(self.nodes[val as usize].op, ValOp::Phi) {
            let args = self.nodes[val as usize].args.clone();
            if args.len() == 3 && args[2] == loopv {
                // (a) guarded accumulation: `if cond { acc = acc ⊕ contrib }`.
                if let Some((op, contrib)) = self
                    .as_reduction_cached(args[1], loopv, cache)
                    .or_else(|| self.nested_reduce_step(args[1], loopv, cache))
                {
                    if let Some(id) = identity_of(op) {
                        let ident = self.int_const(id);
                        let guarded = self.mk(ValOp::Phi, vec![args[0], contrib, ident]);
                        return Some((op, guarded));
                    }
                }
                // (b) selection (min/max): `if cand {>,<} acc { acc = cand }` —
                // the new value does not reference the old accumulator and the guard
                // compares the two. `acc = max(acc, cand)` / `min`.
                let cand = args[1];
                if !self.references_cached(cand, loopv, cache) {
                    if let Some(code) = self.selection_code(args[0], cand, loopv) {
                        return Some((code, cand));
                    }
                }
            }
            // Swapped polarity: `if cond { acc } else { acc ⊕ contrib }`. cfg_norm can
            // flip a two-branch ternary's orientation, so the accumulator lands in the
            // THEN branch with a negated guard — a `functools.reduce(lambda acc,v: acc+v
            // if v>0 else acc, …)` lowers to `if v<=0 { acc } else { acc+v }`. Recognize
            // it with the negated guard so it converges with the loop form `if v>0:
            // acc+=v` (whose single-branch guard stays positive).
            if args.len() == 3 && args[1] == loopv {
                if let Some((op, contrib)) = self
                    .as_reduction_cached(args[2], loopv, cache)
                    .or_else(|| self.nested_reduce_step(args[2], loopv, cache))
                {
                    if let Some(id) = identity_of(op) {
                        let ident = self.int_const(id);
                        let ncond = self.negate_guard(args[0]);
                        let guarded = self.mk(ValOp::Phi, vec![ncond, contrib, ident]);
                        return Some((op, guarded));
                    }
                }
            }
            // Full conditional contribution: both branches update the accumulator once,
            // e.g. `if x < 0 { total += -x } else { total += x }`. This is one reduction
            // whose per-element contribution is itself a branch value:
            // `Reduce(⊕, init, cond ? then_contrib : else_contrib)`. The `Phi` builder
            // then canonicalizes idioms such as `x < 0 ? -x : x` to `Abs(x)`.
            if args.len() == 3 {
                if let (Some((then_op, then_contrib)), Some((else_op, else_contrib))) = (
                    self.as_reduction_cached(args[1], loopv, cache),
                    self.as_reduction_cached(args[2], loopv, cache),
                ) {
                    if then_op == else_op {
                        let contrib =
                            self.mk(ValOp::Phi, vec![args[0], then_contrib, else_contrib]);
                        return Some((then_op, contrib));
                    }
                }
            }
            return None;
        }
        // A min/max accumulator written as a Min/Max node (`minmax_pattern` turned the
        // conditional update `if x>acc { acc=x }` into `Max(acc, x)`): map the idiom code
        // to the selection-reduction code so the loop converges with the `max()`/`min()`
        // builtin (both → `Reduce(REDUCE_MAX/MIN, [contrib])`).
        if let ValOp::Bin(o) = self.nodes[val as usize].op {
            if o == MIN_CODE || o == MAX_CODE {
                let a = self.nodes[val as usize].args.clone();
                let red = if o == MAX_CODE {
                    REDUCE_MAX
                } else {
                    REDUCE_MIN
                };
                if a[0] == loopv && !self.references_cached(a[1], loopv, cache) {
                    return Some((red, a[1]));
                }
                if a[1] == loopv && !self.references_cached(a[0], loopv, cache) {
                    return Some((red, a[0]));
                }
            }
        }
        let op = match self.nodes[val as usize].op {
            ValOp::Bin(o) if is_assoc_comm_code(o) => o,
            _ => return None,
        };
        let mut operands = Vec::new();
        self.flatten_into(val, op, &mut operands);
        // Exactly one top-level operand must be the previous accumulator, and it must
        // not reappear nested in the remaining contribution (`acc = acc + acc*x`).
        if operands.iter().filter(|&&o| o == loopv).count() != 1 {
            return None;
        }
        let pos = operands.iter().position(|&o| o == loopv)?;
        operands.remove(pos);
        if operands.is_empty() {
            return None;
        }
        for &operand in &operands {
            if self.references_cached(operand, loopv, cache) {
                return None;
            }
        }
        operands.sort_by_key(|&v| self.vhash[v as usize]);
        let mut acc = operands[0];
        for &o in &operands[1..] {
            acc = self.mk(ValOp::Bin(op), vec![acc, o]);
        }
        Some((op, acc))
    }

    fn nested_reduce_step(
        &mut self,
        val: ValueId,
        loopv: ValueId,
        cache: &mut ReductionCache,
    ) -> Option<(u32, ValueId)> {
        let ValOp::Reduce(op) = self.nodes[val as usize].op else {
            return None;
        };
        let args = self.nodes[val as usize].args.clone();
        if args.len() == 2 && args[0] == loopv && !self.references_cached(args[1], loopv, cache) {
            return Some((op, args[1]));
        }
        None
    }

    /// The canonical negation of a guard value: a comparison flips to its complement
    /// (`a<=b` → `a>b`, `a==b` → `a!=b`, …) — same operands, so a negated guard
    /// converges with the positive guard a loop produces — and anything else is wrapped
    /// in logical `Not`.
    fn negate_guard(&mut self, v: ValueId) -> ValueId {
        if let ValOp::Bin(opc) = self.nodes[v as usize].op {
            if let Some(flip) = negate_cmp_code(opc) {
                let args = self.nodes[v as usize].args.clone();
                return self.mk(ValOp::Bin(flip), args);
            }
        }
        self.mk(ValOp::Un(Op::Not as u32), vec![v])
    }

    /// If `cond` compares `cand` against the accumulator `loopv` (`cand > loopv` etc.),
    /// classify the selection as max or min. Operand order is meaningful (comparisons
    /// are not commutative-canonicalized), so `cand > acc` and `acc < cand` both → max.
    fn selection_code(&self, cond: ValueId, cand: ValueId, loopv: ValueId) -> Option<u32> {
        let n = &self.nodes[cond as usize];
        let opc = match n.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if n.args.len() != 2 {
            return None;
        }
        let cand_first = n.args[0] == cand && n.args[1] == loopv;
        let acc_first = n.args[0] == loopv && n.args[1] == cand;
        if !cand_first && !acc_first {
            return None;
        }
        // `cand > acc` / `acc < cand` → take the larger ⇒ max; the reverse ⇒ min.
        let greater = opc == Op::Gt as u32 || opc == Op::Ge as u32;
        let lesser = opc == Op::Lt as u32 || opc == Op::Le as u32;
        if (greater && cand_first) || (lesser && acc_first) {
            Some(REDUCE_MAX)
        } else if (lesser && cand_first) || (greater && acc_first) {
            Some(REDUCE_MIN)
        } else {
            None
        }
    }

    /// Recognize the absolute-value idiom `x if x>=0 else -x` (and its mirror
    /// `-x if x<0 else x`) as `Un(ABS_CODE, [x])`, so it converges with `abs(x)`.
    fn abs_pattern(&mut self, cond: ValueId, then: ValueId, els: ValueId) -> Option<ValueId> {
        let is_neg_of = |s: &Self, neg: ValueId, base: ValueId| {
            matches!(s.nodes[neg as usize].op, ValOp::Un(o) if o == Op::Neg as u32)
                && s.nodes[neg as usize].args == [base]
        };
        // (v, the positive branch is `then`)
        let (v, pos_is_then) = if is_neg_of(self, els, then) {
            (then, true) // then = v (the x>=0 branch), else = -v
        } else if is_neg_of(self, then, els) {
            (els, false) // else = v, then = -v
        } else {
            return None;
        };
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if cn.args.len() != 2 {
            return None;
        }
        let is_zero = |s: &Self, id: ValueId| matches!(s.nodes[id as usize].op, ValOp::Const(c) if c == 0x1000_0000);
        let (nonneg, neg) = if cn.args[0] == v && is_zero(self, cn.args[1]) {
            (
                opc == Op::Ge as u32 || opc == Op::Gt as u32,
                opc == Op::Lt as u32 || opc == Op::Le as u32,
            )
        } else if cn.args[1] == v && is_zero(self, cn.args[0]) {
            (
                opc == Op::Le as u32 || opc == Op::Lt as u32,
                opc == Op::Gt as u32 || opc == Op::Ge as u32,
            )
        } else {
            return None;
        };
        // `then` is the value when the condition holds: positive branch must be `v`
        // exactly when the condition says v is non-negative.
        if (nonneg && pos_is_then) || (neg && !pos_is_then) {
            Some(self.mk(ValOp::Un(ABS_CODE), vec![v]))
        } else {
            None
        }
    }

    /// Recognize a 2-way min/max selection `x if x<y else y` (and its variants) as a
    /// canonical `Bin(MIN_CODE/MAX_CODE, [x, y])`, so the ternary idiom converges with a
    /// `min(x, y)` / `max(x, y)` call. The condition has already been canonicalized to the
    /// `</<= ` family by `mk` (`x>y` → `y<x`). Sound: it is the literal meaning of the
    /// ternary (and `MIN_CODE`/`MAX_CODE` are interpreted as exactly that).
    fn minmax_pattern(&mut self, cond: ValueId, then: ValueId, els: ValueId) -> Option<ValueId> {
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) || cn.args.len() != 2 {
            return None;
        }
        let (x, y) = (cn.args[0], cn.args[1]); // cond is `x < y`
                                               // `x if x<y else y` → min(x,y);  `y if x<y else x` → max(x,y).
        if then == x && els == y {
            Some(self.mk(ValOp::Bin(MIN_CODE), vec![x, y]))
        } else if then == y && els == x {
            Some(self.mk(ValOp::Bin(MAX_CODE), vec![x, y]))
        } else {
            None
        }
    }

    /// Recognize the two-comparison integer clamp surface after the inner ternary has already
    /// become a `Min`/`Max`: `lo if x < lo else min(x, hi)` or
    /// `hi if hi < x else max(x, lo)`. It still requires the same bound-order proof as nested
    /// min/max clamp composition, so unproven parameter bounds and float domains stay separate.
    fn clamp_ternary_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        let cn = &self.nodes[cond as usize];
        let opc = match cn.op {
            ValOp::Bin(o) => o,
            _ => return None,
        };
        if !(opc == Op::Lt as u32 || opc == Op::Le as u32) || cn.args.len() != 2 {
            return None;
        }
        let (left, right) = (cn.args[0], cn.args[1]);

        if then == right {
            let hi = self.bin_other_arg(els, MIN_CODE, left)?;
            return self.proof_backed_clamp_value(left, right, hi);
        }
        if then == left {
            let lo = self.bin_other_arg(els, MAX_CODE, right)?;
            return self.proof_backed_clamp_value(right, lo, left);
        }
        None
    }

    /// Java integer low-bit toggle: `x % 2 == 0 ? x + 1 : x - 1` (and the
    /// equivalent `!= 0` branch order) is exactly `x ^ 1`. The branch split avoids
    /// overflow at both signed extremes: max values take the `-1` branch and min
    /// values take the `+1` branch. Keep this Java-only for now because the real
    /// frontier evidence is Java and other surfaces may expose overload/coercion
    /// semantics for these operators.
    fn low_bit_toggle_pattern(
        &mut self,
        cond: ValueId,
        then: ValueId,
        els: ValueId,
    ) -> Option<ValueId> {
        if !self.has_java_primitive_integer_ops() {
            return None;
        }
        let (base, even_when_true) = self.parity_zero_condition(cond)?;
        let then_delta = self.additive_one_delta(then, base)?;
        let else_delta = self.additive_one_delta(els, base)?;
        let is_toggle = (even_when_true && then_delta == 1 && else_delta == -1)
            || (!even_when_true && then_delta == -1 && else_delta == 1);
        if !is_toggle {
            return None;
        }
        let one = self.int_const(1);
        Some(self.mk(ValOp::Bin(Op::BitXor as u32), vec![base, one]))
    }

    fn has_java_primitive_integer_ops(&self) -> bool {
        self.il.meta.lang == Lang::Java
    }

    fn parity_zero_condition(&self, cond: ValueId) -> Option<(ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        let even_when_true = match node.op {
            ValOp::Bin(o) if o == Op::Eq as u32 => true,
            ValOp::Bin(o) if o == Op::Ne as u32 => false,
            _ => return None,
        };
        if node.args.len() != 2 {
            return None;
        }
        for (candidate, zero) in [(node.args[0], node.args[1]), (node.args[1], node.args[0])] {
            if self.int_const_eq(zero, 0) {
                if let Some(base) = self.mod_by_two_base(candidate) {
                    return Some((base, even_when_true));
                }
            }
        }
        None
    }

    fn mod_by_two_base(&self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Mod as u32) || node.args.len() != 2 {
            return None;
        }
        if self.int_const_eq(node.args[1], 2) {
            Some(node.args[0])
        } else {
            None
        }
    }

    fn additive_one_delta(&mut self, value: ValueId, base: ValueId) -> Option<i8> {
        if !matches!(self.nodes[value as usize].op, ValOp::Bin(o) if o == Op::Add as u32) {
            return None;
        }
        let mut leaves = Vec::new();
        self.flatten_into(value, Op::Add as u32, &mut leaves);
        if leaves.len() != 2 {
            return None;
        }
        if leaves[0] == base {
            self.signed_one_const(leaves[1])
        } else if leaves[1] == base {
            self.signed_one_const(leaves[0])
        } else {
            None
        }
    }

    fn signed_one_const(&self, value: ValueId) -> Option<i8> {
        if self.int_const_eq(value, 1) {
            return Some(1);
        }
        if self.int_const_eq(value, -1) {
            return Some(-1);
        }
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Un(o) if o == Op::Neg as u32)
            && node.args.len() == 1
            && self.int_const_eq(node.args[0], 1)
        {
            return Some(-1);
        }
        None
    }

    fn c_u16_be_byte_pack_pattern(&mut self, left: ValueId, right: ValueId) -> Option<ValueId> {
        if self.il.meta.lang != Lang::C {
            return None;
        }
        for (shifted, low) in [(left, right), (right, left)] {
            // `else continue`, not `?`: the operands may sort either way by value-hash, so a
            // miss on the first ordering must fall through to the second, not abort the fn.
            let Some((base, high_index)) = self.shifted_byte_lane(shifted) else {
                continue;
            };
            let Some((low_base, low_index)) = self.byte_lane(low) else {
                continue;
            };
            if base == low_base
                && high_index == 0
                && low_index == 1
                && self.is_byte_array_param_value(base)
            {
                let zero = self.int_const(0);
                let one = self.int_const(1);
                return Some(self.mk(ValOp::Call(C_U16_BE_BYTE_PACK_CODE), vec![base, zero, one]));
            }
        }
        None
    }

    fn c_u32_be_byte_pack_pattern(&mut self, operands: &[ValueId]) -> Option<ValueId> {
        if self.il.meta.lang != Lang::C || operands.len() != 4 {
            return None;
        }
        let mut base = None;
        let mut seen = [false; 4];
        for &operand in operands {
            let (lane_base, index, shift, unsigned_cast) = self.c_u32_byte_pack_lane(operand)?;
            if Some(lane_base) != base {
                if base.is_some() {
                    return None;
                }
                base = Some(lane_base);
            }
            let expected_shift = (3u8.checked_sub(index)? as i64) * 8;
            if shift != expected_shift {
                return None;
            }
            if index == 0 && !unsigned_cast {
                return None;
            }
            if seen[index as usize] {
                return None;
            }
            seen[index as usize] = true;
        }
        if !seen.iter().all(|seen| *seen) {
            return None;
        }
        let base = base?;
        if !self.is_byte_array_param_value(base) {
            return None;
        }
        let zero = self.int_const(0);
        let one = self.int_const(1);
        let two = self.int_const(2);
        let three = self.int_const(3);
        Some(self.mk(
            ValOp::Call(C_U32_BE_BYTE_PACK_CODE),
            vec![base, zero, one, two, three],
        ))
    }

    fn c_u32_byte_pack_lane(&self, value: ValueId) -> Option<(ValueId, u8, i64, bool)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Bin(o) if o == Op::Shl as u32) && node.args.len() == 2 {
            let shift = self.int_const_value(node.args[1])?;
            let (base, index, unsigned_cast) = self.byte_lane_with_unsigned_cast(node.args[0])?;
            return Some((base, index, shift, unsigned_cast));
        }
        let (base, index, unsigned_cast) = self.byte_lane_with_unsigned_cast(value)?;
        Some((base, index, 0, unsigned_cast))
    }

    fn shifted_byte_lane(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Shl as u32) || node.args.len() != 2 {
            return None;
        }
        if !self.int_const_eq(node.args[1], 8) {
            return None;
        }
        self.byte_lane(node.args[0])
    }

    fn byte_lane(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let (base, index, _) = self.byte_lane_with_unsigned_cast(value)?;
        if index <= 1 {
            Some((base, index))
        } else {
            None
        }
    }

    fn byte_lane_with_unsigned_cast(&self, value: ValueId) -> Option<(ValueId, u8, bool)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == Builtin::UnsignedCast32 as u32 + 1)
            && node.args.len() == 1
        {
            let (base, index, _) = self.byte_lane_with_unsigned_cast(node.args[0])?;
            return Some((base, index, true));
        }
        self.byte_lane_any_index(value)
            .map(|(base, index)| (base, index, false))
    }

    fn byte_lane_any_index(&self, value: ValueId) -> Option<(ValueId, u8)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Index) || node.args.len() != 2 {
            return None;
        }
        if self.int_const_eq(node.args[1], 0) {
            Some((node.args[0], 0))
        } else if self.int_const_eq(node.args[1], 1) {
            Some((node.args[0], 1))
        } else if self.int_const_eq(node.args[1], 2) {
            Some((node.args[0], 2))
        } else if self.int_const_eq(node.args[1], 3) {
            Some((node.args[0], 3))
        } else {
            None
        }
    }

    fn is_byte_array_param_value(&self, value: ValueId) -> bool {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return false;
        };
        matches!(
            self.param_semantic.get(&cid),
            Some(ParamSemantic::ByteArray)
        )
    }

    fn int_const_eq(&self, value: ValueId, expected: i64) -> bool {
        self.int_const_value(value) == Some(expected)
    }

    fn int_const_value(&self, value: ValueId) -> Option<i64> {
        let ValOp::Const(key) = self.nodes[value as usize].op else {
            return None;
        };
        Some(key.wrapping_sub(0x1000_0000) as i32 as i64)
    }

    /// An integer-literal value, keyed identically to `eval`'s `LitInt` path so a
    /// builtin's implicit init (`sum` → 0) matches a loop's explicit `acc = 0`.
    fn int_const(&mut self, v: u32) -> ValueId {
        self.mk(ValOp::Const(0x1000_0000u32.wrapping_add(v)), vec![])
    }

    fn null_const(&mut self) -> ValueId {
        self.mk(ValOp::Const(nose_il::LitClass::Null as u32), vec![])
    }

    fn bool_const(&self, id: ValueId) -> Option<bool> {
        match self.nodes[id as usize].op {
            ValOp::Const(0x3000_0001) => Some(false),
            ValOp::Const(0x3000_0002) => Some(true),
            _ => None,
        }
    }

    fn literal_equality_disjunction(&mut self, left: ValueId, right: ValueId) -> Option<ValueId> {
        let mut element = None;
        let mut items = Vec::new();
        self.collect_literal_membership_terms(left, &mut element, &mut items)?;
        self.collect_literal_membership_terms(right, &mut element, &mut items)?;
        if items.len() < 2 {
            return None;
        }
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        let collection = self.mk(ValOp::Seq(1), items);
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![element?, collection]))
    }

    fn collect_literal_membership_terms(
        &self,
        value: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        let node = &self.nodes[value as usize];
        match node.op {
            ValOp::Bin(op) if op == Op::Or as u32 && node.args.len() == 2 => {
                self.collect_literal_membership_terms(node.args[0], element, items)?;
                self.collect_literal_membership_terms(node.args[1], element, items)
            }
            ValOp::Bin(op) if op == Op::Eq as u32 && node.args.len() == 2 => {
                let a = node.args[0];
                let b = node.args[1];
                let (candidate, literal) = if self.static_membership_literal_value(a) {
                    (b, a)
                } else if self.static_membership_literal_value(b) {
                    (a, b)
                } else {
                    return None;
                };
                self.record_literal_membership_term(candidate, literal, element, items)
            }
            ValOp::Bin(op) if op == Op::In as u32 && node.args.len() == 2 => {
                let candidate = node.args[0];
                let collection = &self.nodes[node.args[1] as usize];
                if !matches!(collection.op, ValOp::Seq(1))
                    || !collection
                        .args
                        .iter()
                        .all(|&item| self.static_membership_literal_value(item))
                {
                    return None;
                }
                match *element {
                    Some(current) if current != candidate => None,
                    Some(_) => {
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                    None => {
                        *element = Some(candidate);
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                }
            }
            _ => None,
        }
    }

    fn record_literal_membership_term(
        &self,
        candidate: ValueId,
        literal: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        match *element {
            Some(current) if current != candidate => None,
            Some(_) => {
                items.push(literal);
                Some(())
            }
            None => {
                *element = Some(candidate);
                items.push(literal);
                Some(())
            }
        }
    }

    fn static_membership_literal_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const(key) if key != 0x3000_0000
        )
    }

    fn empty_string_value(&mut self) -> ValueId {
        self.mk(ValOp::Const(stable_string_const_key("")), vec![])
    }

    fn is_empty_string_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const(key) if key == stable_string_const_key("")
        )
    }

    /// A canonical "element of `coll`" value. The collection is carried as an argument
    /// (not just folded into the key) so it is reachable from the fingerprint wherever
    /// the element is used — identically for a loop, a `reduce`, and a `sum(map)` over
    /// the same collection, so they converge without a separate iterable sink.
    fn elem(&mut self, coll: ValueId) -> ValueId {
        if let Some(value) = self.hof_emitted_elem(coll) {
            return value;
        }
        self.raw_elem(coll)
    }

    fn hof_emitted_elem(&mut self, coll: ValueId) -> Option<ValueId> {
        let (emitted, predicate) = self.hof_emitted_elem_with_pred(coll)?;
        predicate.is_none().then_some(emitted)
    }

    fn raw_elem(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Elem(self.vhash[coll as usize]), vec![coll])
    }

    fn collection_elem_with_pred(&mut self, coll: ValueId) -> (ValueId, Option<ValueId>) {
        if let Some(parts) = self.hof_emitted_elem_with_pred(coll) {
            return parts;
        }
        (self.raw_elem(coll), None)
    }

    fn hof_emitted_elem_with_pred(&mut self, coll: ValueId) -> Option<(ValueId, Option<ValueId>)> {
        // FUNCTOR LAW / map fusion: an element drawn from `map(f, c)` is `f` applied to an
        // element of `c`, and a pure Map node's `contrib` (args[0]) already *is* that
        // per-element value. So `Elem(Map(f, c)) -> contrib`, which fuses nested maps:
        // `map(g, map(f, xs))` and `map(g o f, xs)` converge to one fingerprint. Sound
        // (functor composition law: map g o map f = map (g o f)). A *filtered* map carries a
        // predicate (args.len() == 2) and is NOT peeled (the filter changes which elements).
        //
        // FlatMap emits the elements produced by its inner stream. When the modeled inner
        // stream is a pure Map, `Elem(FlatMap(xs, map(f, ys)))` is the same emitted `f(y)`
        // value. Keep this separate from the filtered-Map two-argument layout so aggregate
        // consumers do not confuse `FlatMap[outer, inner]` with `Map[contrib, pred]`.
        let (op, args) = {
            let n = &self.nodes[coll as usize];
            (n.op.clone(), n.args.clone())
        };
        match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 1 => Some((args[0], None)),
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 2 => {
                Some((args[0], Some(args[1])))
            }
            ValOp::Hof(k)
                if k == HoFKind::FlatMap as u32 && (args.len() == 2 || args.len() == 3) =>
            {
                let outer = args[0];
                let inner = args[1];
                let outer_predicate = args.get(2).copied();
                let (emitted, inner_predicate) = self.hof_emitted_elem_with_pred(inner)?;
                if !self.references(emitted, outer) {
                    return None;
                }
                let predicate = self.and_preds(outer_predicate, inner_predicate);
                Some((emitted, predicate))
            }
            _ => None,
        }
    }

    /// A canonical "iteration index into `coll`". `for i in range(len(xs))`, an indexed
    /// `while`, and `for i, _ in enumerate(xs)` all bind their index variable to this,
    /// so they converge — and `C[Idx(C)]` rewrites to `Elem(C)`.
    fn idx(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Idx(self.vhash[coll as usize]), vec![coll])
    }

    /// The canonical-id variables of a loop pattern: a single `Var`, or the elements
    /// of a tuple pattern `(i, x)` (lowered as a `Seq` of `Var`s).
    fn pattern_cids(&self, pat: NodeId) -> Vec<u32> {
        let mut out = Vec::new();
        let push = |n: NodeId, out: &mut Vec<u32>| {
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(n), self.il.node(n).payload) {
                out.push(c);
            }
        };
        if self.il.kind(pat) == NodeKind::Seq {
            for c in self.il.children(pat) {
                push(*c, &mut out);
            }
        } else {
            push(pat, &mut out);
        }
        out
    }

    /// If `node` is a *full* index range over `C` — `range(len(C))` or `range(0,
    /// len(C))` — return `C`: the loop visits every index of `C`, so `C[i]` is the
    /// canonical `Elem(C)`. A non-zero start (`range(1, len(C))`), an explicit step
    /// (`range(_, _, k)`), or any other form iterates a *subset*, so its element is NOT
    /// `Elem(C)` — abstracting `C[i]` to `Elem(C)` there drops the start/step bound and
    /// merges behaviorally-different loops (a soundness bug). Such forms return `None`.
    fn range_len_collection(&self, node: NodeId) -> Option<NodeId> {
        let len_arg = if self.il.kind(node) == NodeKind::Call
            && matches!(self.il.node(node).payload, Payload::Builtin(Builtin::Range))
        {
            let kids = self.il.children(node);
            match kids.len() {
                1 => kids[0],
                // `range(start, stop)` is a full iteration only when `start` is literally 0.
                2 if matches!(self.il.node(kids[0]).payload, Payload::LitInt(0)) => kids[1],
                _ => return None,
            }
        } else if self.il.kind(node) == NodeKind::Seq {
            let kids = self.il.children(node);
            match kids {
                // Rust `0..len(C)` lowers as `Seq(0, Len(C), inclusive=0)`.
                [start, stop, inclusive]
                    if matches!(self.il.node(*start).payload, Payload::LitInt(0))
                        && matches!(self.il.node(*inclusive).payload, Payload::LitInt(0)) =>
                {
                    *stop
                }
                _ => return None,
            }
        } else {
            return None;
        };
        if self.il.kind(len_arg) == NodeKind::Call
            && matches!(
                self.il.node(len_arg).payload,
                Payload::Builtin(Builtin::Len)
            )
        {
            return self.il.children(len_arg).first().copied();
        }
        None
    }

    /// Fold a reduction builtin (`sum`, `reduce`) to the canonical `Reduce(op, init,
    /// per-element contrib)` — the same value a loop accumulator produces. Returns
    /// `None` if it isn't a recognized clean reduction (caller falls back to a Call).
    fn eval_reduction_builtin(
        &mut self,
        b: Builtin,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match b {
            Builtin::Len => {
                if kids.len() == 1 {
                    self.eval_len_builtin(kids[0], env)
                } else {
                    None
                }
            }
            Builtin::Sum => {
                let av = self.eval(*kids.first()?, env);
                // `sum(map)` → the mapped stream's per-element contribution; a filtered
                // map/flat-map carries a predicate and becomes `pred ? contrib : 0`,
                // matching a guarded loop `if pred: acc += contrib`; `sum(xs)` → the raw
                // element.
                let zero = self.int_const(0);
                let (contrib, predicate) = self.collection_elem_with_pred(av);
                let contrib = if let Some(predicate) = predicate {
                    self.mk(ValOp::Phi, vec![predicate, contrib, zero])
                } else {
                    contrib
                };
                let init = self.int_const(0);
                Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
            }
            Builtin::Reduce => {
                if kids.len() < 2 {
                    return None;
                }
                let filtered = self.filter_parts(kids[1]);
                let (elems, guard) = if let Some((source, predicate)) = filtered {
                    let coll = self.eval(source, env);
                    let elem = self.elem(coll);
                    let guard = self.eval_lambda_body(predicate, &[elem], env)?;
                    (vec![elem], Some(guard))
                } else {
                    self.elem_bindings_with_pred(kids.get(1).copied(), env)
                };
                let acc = self.fresh_opaque();
                let mut params = Vec::with_capacity(elems.len() + 1);
                params.push(acc);
                params.extend(elems);
                let body = self.eval_lambda_body(kids[0], &params, env)?;
                let (op, contrib) = self.as_reduction(body, acc)?;
                let contrib = if let Some(guard) = guard {
                    let ident = self.int_const(identity_of(op)?);
                    self.mk(ValOp::Phi, vec![guard, contrib, ident])
                } else {
                    contrib
                };
                let init = kids
                    .get(2)
                    .map(|&i| self.eval(i, env))
                    .unwrap_or_else(|| self.int_const(0));
                let args = if is_selection_code(op) {
                    vec![contrib]
                } else {
                    vec![init, contrib]
                };
                Some(self.mk(ValOp::Reduce(op), args))
            }
            Builtin::Min | Builtin::Max => {
                let (reduce_code, choice_code) = if matches!(b, Builtin::Max) {
                    (REDUCE_MAX, MAX_CODE)
                } else {
                    (REDUCE_MIN, MIN_CODE)
                };
                if kids.len() == 2 {
                    let left = self.eval(kids[0], env);
                    let right = self.eval(kids[1], env);
                    return Some(self.mk(ValOp::Bin(choice_code), vec![left, right]));
                }
                let av = self.eval(*kids.first()?, env);
                // `max(f(x) for x in xs)` → the mapped per-element value; `max(xs)` →
                // the raw element. No init (selection reductions carry none), so it
                // matches a `best = max(best, f(x))` loop regardless of its seed.
                let (op, args) = {
                    let n = &self.nodes[av as usize];
                    (n.op.clone(), n.args.clone())
                };
                let contrib = match op {
                    ValOp::Hof(k) if k == HoFKind::Map as u32 && !args.is_empty() => args[0],
                    _ => self.elem(av),
                };
                Some(self.mk(ValOp::Reduce(reduce_code), vec![contrib]))
            }
            Builtin::Any | Builtin::All => {
                let code = if matches!(b, Builtin::All) {
                    REDUCE_ALL
                } else {
                    REDUCE_ANY
                };
                // `xs.some(p)` / `xs.any(p)` — method form `[coll, λ]`: the per-element
                // contribution is `p(Elem coll)`. `any(p(x) for x in xs)` — generator form
                // `[Map]`: the mapped predicate value; a *filtered* generator carries its
                // predicate, guarded by the OR/AND identity (false for any, true for all).
                let contrib = if kids.len() >= 2 && self.il.kind(kids[1]) == NodeKind::Lambda {
                    let coll = self.eval(kids[0], env);
                    let (elem, carried_guard) = self.collection_elem_with_pred(coll);
                    let pred = self.eval_lambda_body(kids[1], &[elem], env)?;
                    if carried_guard.is_none()
                        && code == REDUCE_ANY
                        && self.is_static_non_float_collection_expr(kids[0])
                    {
                        if let Some((element, collection)) =
                            self.static_literal_membership_predicate(pred)
                        {
                            return Some(
                                self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]),
                            );
                        }
                    }
                    if carried_guard.is_none()
                        && code == REDUCE_ALL
                        && self.is_static_non_float_collection_expr(kids[0])
                    {
                        if let Some((element, collection)) =
                            self.static_literal_absence_predicate(pred)
                        {
                            let membership =
                                self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                            return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
                        }
                    }
                    if let Some(carried_guard) = carried_guard {
                        let ident = self.mk(ValOp::Const(0x3000_0001 + code - REDUCE_ANY), vec![]);
                        self.mk(ValOp::Phi, vec![carried_guard, pred, ident])
                    } else {
                        pred
                    }
                } else {
                    let av = self.eval(*kids.first()?, env);
                    let (contrib, predicate) = self.collection_elem_with_pred(av);
                    if let Some(predicate) = predicate {
                        let ident = self.mk(ValOp::Const(0x3000_0001 + code - REDUCE_ANY), vec![]);
                        self.mk(ValOp::Phi, vec![predicate, contrib, ident])
                    } else {
                        contrib
                    }
                };
                Some(self.mk(ValOp::Reduce(code), vec![contrib]))
            }
            Builtin::Join => {
                if kids.len() != 2 {
                    return None;
                }
                let sep = self.eval(kids[0], env);
                let coll = self.eval(kids[1], env);
                let elem = self.elem(coll);
                Some(self.mk(ValOp::Reduce(ORDERED_STRING_JOIN), vec![sep, elem]))
            }
            _ => None,
        }
    }

    fn eval_len_builtin(&mut self, arg: NodeId, env: &FxHashMap<u32, ValueId>) -> Option<ValueId> {
        if let Some(count) = self.eval_filter_count(arg, env) {
            return Some(count);
        }

        let av = self.eval(arg, env);
        let (op, args) = {
            let n = &self.nodes[av as usize];
            (n.op.clone(), n.args.clone())
        };
        let ValOp::Hof(k) = op else { return None };
        if k != HoFKind::Map as u32 {
            return None;
        }

        let one = self.int_const(1);
        let contrib = if args.len() >= 2 {
            let zero = self.int_const(0);
            self.mk(ValOp::Phi, vec![args[1], one, zero])
        } else {
            one
        };
        let init = self.int_const(0);
        Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
    }

    fn eval_len_zero_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 || !(op == Op::Eq as u32 || op == Op::Ne as u32) {
            return None;
        }
        let coll = if self.is_zero_literal(kids[0]) {
            self.len_call_arg(kids[1])?
        } else if self.is_zero_literal(kids[1]) {
            self.len_call_arg(kids[0])?
        } else {
            return None;
        };
        let coll_value = self.eval(coll, env);
        let empty = self.is_empty_value(coll_value);
        if op == Op::Eq as u32 {
            Some(empty)
        } else {
            Some(self.mk(ValOp::Un(Op::Not as u32), vec![empty]))
        }
    }

    fn eval_static_filter_membership_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        if let Some((element, collection)) = self.static_filter_membership_parts(kids[0], env) {
            if self.is_count_nonempty_threshold(op, false, kids[1]) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            if self.is_count_zero_threshold(op, false, kids[1]) {
                let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
            }
        }
        if let Some((element, collection)) = self.static_filter_membership_parts(kids[1], env) {
            if self.is_count_nonempty_threshold(op, true, kids[0]) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            if self.is_count_zero_threshold(op, true, kids[0]) {
                let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                return Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]));
            }
        }
        None
    }

    fn static_filter_membership_parts(
        &mut self,
        len_expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, ValueId)> {
        let filter = self.len_call_arg(len_expr)?;
        let (source, predicate) = self.filter_parts(filter)?;
        if !self.is_static_non_float_collection_expr(source) {
            return None;
        }
        let collection = self.eval(source, env);
        let elem = self.elem(collection);
        let pred = self.eval_lambda_body(predicate, &[elem], env)?;
        self.static_literal_membership_predicate(pred)
    }

    fn is_count_nonempty_threshold(
        &self,
        op: u32,
        count_on_right: bool,
        threshold: NodeId,
    ) -> bool {
        if self.is_zero_literal(threshold) {
            return op == Op::Ne as u32
                || (!count_on_right && op == Op::Gt as u32)
                || (count_on_right && op == Op::Lt as u32);
        }
        if self.is_one_literal(threshold) {
            return (!count_on_right && op == Op::Ge as u32)
                || (count_on_right && op == Op::Le as u32);
        }
        false
    }

    fn is_count_zero_threshold(&self, op: u32, count_on_right: bool, threshold: NodeId) -> bool {
        if self.is_zero_literal(threshold) {
            return op == Op::Eq as u32
                || (!count_on_right && op == Op::Le as u32)
                || (count_on_right && op == Op::Ge as u32);
        }
        if self.is_one_literal(threshold) {
            return (!count_on_right && op == Op::Lt as u32)
                || (count_on_right && op == Op::Gt as u32);
        }
        false
    }

    fn eval_static_index_membership_comparison(
        &mut self,
        op: u32,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        if self.is_index_membership_threshold(op, false, kids[1]) {
            if let Some((element, collection)) = self.static_index_membership_parts(kids[0], env) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
        }
        if self.is_index_membership_threshold(op, true, kids[0]) {
            if let Some((element, collection)) = self.static_index_membership_parts(kids[1], env) {
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
        }
        None
    }

    fn static_index_membership_parts(
        &mut self,
        node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, ValueId)> {
        if self.il.kind(node) != NodeKind::Call {
            return None;
        }
        let kids = self.il.children(node);
        if kids.len() != 2 || self.il.kind(kids[0]) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = self.il.node(kids[0]).payload else {
            return None;
        };
        let method = self.interner.resolve(method);
        let receiver = *self.il.children(kids[0]).first()?;
        if !self.is_static_non_float_collection_expr(receiver) {
            return None;
        }
        if method == "indexOf" {
            let element = self.eval(kids[1], env);
            let collection = self.eval_membership_collection(receiver, env);
            return Some((element, collection));
        }
        if method == "findIndex" && self.il.kind(kids[1]) == NodeKind::Lambda {
            let collection = self.eval(receiver, env);
            let elem = self.elem(collection);
            let pred = self.eval_lambda_body(kids[1], &[elem], env)?;
            return self.static_literal_membership_predicate(pred);
        }
        None
    }

    fn is_index_membership_threshold(
        &self,
        op: u32,
        index_call_on_right: bool,
        threshold: NodeId,
    ) -> bool {
        if self.is_minus_one_literal(threshold) {
            return op == Op::Ne as u32
                || (!index_call_on_right && op == Op::Gt as u32)
                || (index_call_on_right && op == Op::Lt as u32);
        }
        if self.is_zero_literal(threshold) {
            return (!index_call_on_right && op == Op::Ge as u32)
                || (index_call_on_right && op == Op::Le as u32);
        }
        false
    }

    fn is_minus_one_literal(&self, node: NodeId) -> bool {
        if matches!(self.il.node(node).payload, Payload::LitInt(-1)) {
            return true;
        }
        if self.il.kind(node) != NodeKind::UnOp {
            return false;
        }
        if op_code(self.il.node(node).payload) != Op::Neg as u32 {
            return false;
        }
        let kids = self.il.children(node);
        kids.len() == 1 && matches!(self.il.node(kids[0]).payload, Payload::LitInt(1))
    }

    fn is_empty_value(&mut self, coll: ValueId) -> ValueId {
        let coll = self.param_domain_value(coll);
        let len = self.mk(ValOp::Call(builtin_tag(Builtin::Len)), vec![coll]);
        let zero = self.int_const(0);
        self.mk(ValOp::Bin(Op::Eq as u32), vec![len, zero])
    }

    fn map_default_pattern(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        let (key, map, negated) = self
            .own_property_condition(cond)
            .or_else(|| self.membership_condition(cond))?;
        let default = if negated {
            if !self.map_lookup_value_matches(else_v, map, key) {
                return None;
            }
            then_v
        } else {
            if !self.map_lookup_value_matches(then_v, map, key) {
                return None;
            }
            else_v
        };
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    fn value_default_pattern(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.is_bottom_value(then_v) || self.is_bottom_value(else_v) {
            return None;
        }
        let (value, present) = self.null_condition(cond)?;
        let default = if present {
            if !self.value_branch_returns_value(then_v, value) {
                return None;
            }
            else_v
        } else {
            if !self.value_branch_returns_value(else_v, value) {
                return None;
            }
            then_v
        };
        if let Some((map, key)) = self.proven_map_get_value(value) {
            return Some(self.mk(
                ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                vec![map, key, default],
            ));
        }
        Some(self.mk_value_default(value, default))
    }

    fn value_default_call(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Call(tag) if tag == builtin_tag(Builtin::ValueOrDefault))
            && node.args.len() == 2
        {
            Some((node.args[0], node.args[1]))
        } else {
            None
        }
    }

    fn value_branch_returns_value(&self, branch: ValueId, value: ValueId) -> bool {
        branch == value
            || self
                .value_default_call(branch)
                .is_some_and(|(inner_value, _)| inner_value == value)
    }

    fn mk_value_default(&mut self, value: ValueId, default: ValueId) -> ValueId {
        if self
            .value_default_call(value)
            .is_some_and(|(_, inner_default)| inner_default == default)
        {
            return value;
        }
        self.mk(
            ValOp::Call(builtin_tag(Builtin::ValueOrDefault)),
            vec![value, default],
        )
    }

    fn null_condition(&self, cond: ValueId) -> Option<(ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        if node.args.len() == 2 {
            if matches!(node.op, ValOp::Bin(o) if o == Op::Eq as u32) {
                if self.is_null_value(node.args[0]) {
                    return Some((node.args[1], false));
                }
                if self.is_null_value(node.args[1]) {
                    return Some((node.args[0], false));
                }
            }
            if matches!(node.op, ValOp::Bin(o) if o == Op::Ne as u32) {
                if self.is_null_value(node.args[0]) {
                    return Some((node.args[1], true));
                }
                if self.is_null_value(node.args[1]) {
                    return Some((node.args[0], true));
                }
            }
        }
        None
    }

    fn is_null_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Const(k) if k == nose_il::LitClass::Null as u32)
    }

    fn membership_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let node = &self.nodes[cond as usize];
        if matches!(node.op, ValOp::Bin(o) if o == Op::In as u32) && node.args.len() == 2 {
            return Some((node.args[0], node.args[1], false));
        }
        if matches!(node.op, ValOp::Un(o) if o == Op::Not as u32) && node.args.len() == 1 {
            let inner = &self.nodes[node.args[0] as usize];
            if matches!(inner.op, ValOp::Bin(o) if o == Op::In as u32) && inner.args.len() == 2 {
                return Some((inner.args[0], inner.args[1], true));
            }
        }
        None
    }

    fn map_presence_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let (key, map, negated) = self
            .own_property_condition(cond)
            .or_else(|| self.membership_condition(cond))?;
        Some((key, map, !negated))
    }

    fn static_literal_membership_predicate(&mut self, pred: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[pred as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Eq as u32) || node.args.len() != 2 {
            return None;
        }
        let left = node.args[0];
        let right = node.args[1];
        if let Some(collection) = self.static_literal_elem_collection(left) {
            return Some((right, collection));
        }
        if let Some(collection) = self.static_literal_elem_collection(right) {
            return Some((left, collection));
        }
        None
    }

    fn static_literal_absence_predicate(&mut self, pred: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[pred as usize];
        if !matches!(node.op, ValOp::Bin(o) if o == Op::Ne as u32) || node.args.len() != 2 {
            return None;
        }
        let left = node.args[0];
        let right = node.args[1];
        if let Some(collection) = self.static_literal_elem_collection(left) {
            return Some((right, collection));
        }
        if let Some(collection) = self.static_literal_elem_collection(right) {
            return Some((left, collection));
        }
        None
    }

    fn static_literal_elem_collection(&mut self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Elem(_)) || node.args.len() != 1 {
            return None;
        }
        let collection = node.args[0];
        self.is_static_membership_collection(collection)
            .then(|| self.canonical_membership_collection_value(collection))
    }

    fn is_static_membership_collection(&self, value: ValueId) -> bool {
        let node = &self.nodes[value as usize];
        matches!(node.op, ValOp::Seq(1)) && !node.args.is_empty()
    }

    fn own_property_condition(&self, cond: ValueId) -> Option<(ValueId, ValueId, bool)> {
        let parse = |node: &ValNode| {
            if matches!(node.op, ValOp::Seq(OWN_PROPERTY_GUARD_SEQ_TAG)) && node.args.len() == 4 {
                let map = node.args[0];
                if !matches!(self.nodes[map as usize].op, ValOp::Seq(3)) {
                    return None;
                }
                return Some((node.args[1], map, false));
            }
            None
        };
        let node = &self.nodes[cond as usize];
        if let Some(parts) = parse(node) {
            return Some(parts);
        }
        if matches!(node.op, ValOp::Un(o) if o == Op::Not as u32) && node.args.len() == 1 {
            let inner = &self.nodes[node.args[0] as usize];
            if let Some((key, map, _)) = parse(inner) {
                return Some((key, map, true));
            }
        }
        None
    }

    fn map_lookup_value_matches(&mut self, value: ValueId, map: ValueId, key: ValueId) -> bool {
        let node = &self.nodes[value as usize];
        if matches!(node.op, ValOp::Index) && node.args.as_slice() == [map, key] {
            return true;
        }
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return false;
        }
        let args = node.args.clone();
        if args[1] != key {
            return false;
        }
        let callee = &self.nodes[args[0] as usize];
        if !matches!(callee.op, ValOp::Field(name) if name == stable_symbol_hash("get"))
            || callee.args.len() != 1
        {
            return false;
        }
        let receiver = callee.args[0];
        receiver == map
            || self
                .proven_map_value(receiver)
                .is_some_and(|candidate| candidate == map)
    }

    fn eval_membership_collection(
        &mut self,
        collection: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        if self.is_collection_param_expr(collection) {
            let value = self.eval(collection, env);
            return self.mk(ValOp::CollectionParam, vec![value]);
        }
        if self.il.kind(collection) != NodeKind::Seq {
            let value = self.eval(collection, env);
            let collection = self
                .proven_collection_value(value)
                .or_else(|| self.proven_local_collection_binding_value(collection, env))
                .unwrap_or(value);
            return self.canonical_membership_collection_value(collection);
        }
        let kids = self.il.children(collection).to_vec();
        let mut items: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        self.mk(ValOp::Seq(1), items)
    }

    fn canonical_membership_collection_value(&mut self, value: ValueId) -> ValueId {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(1)) {
            return value;
        }
        let mut items = node.args.clone();
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        self.mk(ValOp::Seq(1), items)
    }

    fn is_static_non_float_collection_expr(&self, collection: NodeId) -> bool {
        if self.il.kind(collection) != NodeKind::Seq {
            return false;
        }
        let kids = self.il.children(collection);
        !kids.is_empty()
            && kids.iter().all(|&kid| {
                self.il.kind(kid) == NodeKind::Lit
                    && matches!(
                        self.il.node(kid).payload,
                        Payload::LitInt(_)
                            | Payload::LitBool(_)
                            | Payload::LitStr(_)
                            | Payload::Lit(nose_il::LitClass::Null)
                    )
            })
    }

    fn eval_map_lookup_collection(
        &mut self,
        collection: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> ValueId {
        let value = if self.il.kind(collection) == NodeKind::Seq {
            let kids = self.il.children(collection).to_vec();
            let entries: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
            self.mk(ValOp::Seq(3), entries)
        } else {
            self.eval(collection, env)
        };
        self.proven_map_value(value).unwrap_or(value)
    }

    fn len_call_arg(&self, node: NodeId) -> Option<NodeId> {
        if self.il.kind(node) != NodeKind::Call {
            return None;
        }
        if !matches!(self.il.node(node).payload, Payload::Builtin(Builtin::Len)) {
            return None;
        }
        self.il.children(node).first().copied()
    }

    fn is_zero_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(0))
    }

    fn is_one_literal(&self, node: NodeId) -> bool {
        matches!(self.il.node(node).payload, Payload::LitInt(1))
    }

    fn filter_parts(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        if self.il.kind(node) != NodeKind::HoF {
            return None;
        }
        if !matches!(self.il.node(node).payload, Payload::HoF(HoFKind::Filter)) {
            return None;
        }
        let kids = self.il.children(node);
        Some((*kids.first()?, *kids.get(1)?))
    }

    fn eval_filter_count(
        &mut self,
        filter_node: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let (source, predicate) = self.filter_parts(filter_node)?;
        self.eval_predicate_count(source, predicate, env)
    }

    fn eval_predicate_count(
        &mut self,
        source: NodeId,
        predicate: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let coll = self.eval(source, env);
        let elem = self.elem(coll);
        let pred = self.eval_lambda_body(predicate, &[elem], env)?;
        let one = self.int_const(1);
        let zero = self.int_const(0);
        let contrib = self.mk(ValOp::Phi, vec![pred, one, zero]);
        let init = self.int_const(0);
        Some(self.mk(ValOp::Reduce(Op::Add as u32), vec![init, contrib]))
    }

    fn eval_count_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let callee = *kids.first()?;
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        if self.interner.resolve(name) != "count" {
            return None;
        }
        let base = *self.il.children(callee).first()?;
        match kids {
            // Rust-style `iter.filter(p).count()`.
            [_] => self.eval_filter_count(base, env),
            // Ruby-style `xs.count { |x| p(x) }`.
            [_, predicate] if self.il.kind(*predicate) == NodeKind::Lambda => {
                self.eval_predicate_count(base, *predicate, env)
            }
            _ => None,
        }
    }

    fn eval_product_call(
        &mut self,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let callee = *kids.first()?;
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        if self.interner.resolve(name) != "prod" {
            return None;
        }
        let base = *self.il.children(callee).first()?;
        if !matches!(
            (self.il.kind(base), self.il.node(base).payload),
            (NodeKind::Var, Payload::Name(s)) if self.interner.resolve(s) == "math"
        ) {
            return None;
        }
        let coll = self.eval(*kids.get(1)?, env);
        let (op, args) = {
            let n = &self.nodes[coll as usize];
            (n.op.clone(), n.args.clone())
        };
        let contrib = match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() >= 2 => {
                let one = self.int_const(1);
                self.mk(ValOp::Phi, vec![args[1], args[0], one])
            }
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 1 => args[0],
            _ => self.elem(coll),
        };
        let init = kids
            .get(2)
            .map(|&i| self.eval(i, env))
            .unwrap_or_else(|| self.int_const(1));
        Some(self.mk(ValOp::Reduce(Op::Mul as u32), vec![init, contrib]))
    }

    /// The element value(s) a map lambda binds to, plus any predicate CARRIED by the
    /// collection (map/filter fusion). If the collection evaluates to a *filtered* Map
    /// `Hof(Map,[c,p])`, the element is `c` and the carried predicate is `p` — so an outer
    /// map composes into one `filtered-map`, converging `map(h, map(f, filter p))` with the
    /// direct `filtered-map (h∘f)@p`. A pure Map collection is peeled by `elem`; `zip` binds
    /// multiple elements and carries no predicate.
    fn map_source(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> (Vec<ValueId>, Option<ValueId>) {
        let Some(c) = coll_node else {
            return (Vec::new(), None);
        };
        if self.il.kind(c) == NodeKind::Call
            && matches!(self.il.node(c).payload, Payload::Builtin(Builtin::Zip))
        {
            return (self.elem_bindings(Some(c), env), None);
        }
        let cv = self.eval(c, env);
        if let ValOp::Hof(k) = self.nodes[cv as usize].op {
            if k == HoFKind::Map as u32 && self.nodes[cv as usize].args.len() == 2 {
                let args = self.nodes[cv as usize].args.clone();
                return (vec![args[0]], Some(args[1]));
            }
        }
        (vec![self.elem(cv)], None)
    }

    /// Conjoin two optional predicates (a filter's own predicate and one carried up through
    /// a fused collection): both → `p ∧ q`, one → it, none → none.
    fn and_preds(&mut self, a: Option<ValueId>, b: Option<ValueId>) -> Option<ValueId> {
        match (a, b) {
            (Some(x), Some(y)) => Some(self.mk(ValOp::Bin(Op::And as u32), vec![x, y])),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }

    /// The per-element value(s) a map/filter lambda's parameters bind to: a single
    /// `Elem(coll)`, or — for `zip(a, b)` with a tuple pattern — `[Elem(a), Elem(b)]`.
    fn elem_bindings(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> Vec<ValueId> {
        self.elem_bindings_with_pred(coll_node, env).0
    }

    fn elem_bindings_with_pred(
        &mut self,
        coll_node: Option<NodeId>,
        env: &FxHashMap<u32, ValueId>,
    ) -> (Vec<ValueId>, Option<ValueId>) {
        let Some(c) = coll_node else {
            return (Vec::new(), None);
        };
        if self.il.kind(c) == NodeKind::Call
            && matches!(self.il.node(c).payload, Payload::Builtin(Builtin::Zip))
        {
            let mut out = Vec::new();
            for k in self.il.children(c).to_vec() {
                let v = self.eval(k, env);
                out.push(self.elem(v));
            }
            return (out, None);
        }
        let cv = self.eval(c, env);
        let (elem, predicate) = self.collection_elem_with_pred(cv);
        (vec![elem], predicate)
    }

    /// Evaluate a lambda's body with its positional parameters bound to `params`,
    /// returning the value of its first `return` (intermediate assignments update the
    /// local env). Used to unfold a `map`/`reduce` lambda over a canonical `Elem`.
    /// `p.then(λr. body)` ≡ `const r = await p; body`. With `await` already stripped, beta-reduce
    /// the single-argument continuation callback with the receiver promise's value, so a
    /// `.then`-chain converges with the equivalent `await`/sequential form. Only the one-argument
    /// `.then(λr. …)` shape is reduced: `.then(fnRef)` (no lambda body to inline), the
    /// two-argument `.then(onOk, onErr)`, and `.catch`/`.finally` (error/cleanup continuations
    /// whose parameter is the error, not the resolved value) are left opaque. Promises wrap
    /// external async values, so this is a NEAR-channel recall extension, never an exact merge.
    fn eval_promise_then(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let kids = self.il.children(expr).to_vec();
        if kids.len() != 2 {
            return None;
        }
        let callee = kids[0];
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(s) = self.il.node(callee).payload else {
            return None;
        };
        if self.interner.resolve(s) != "then" {
            return None;
        }
        let cb = kids[1];
        if self.il.kind(cb) != NodeKind::Lambda {
            return None;
        }
        let &recv = self.il.children(callee).first()?;
        let recv_v = self.eval(recv, env);
        self.eval_lambda_body(cb, &[recv_v], env)
    }

    fn eval_lambda_body(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda).to_vec();
        let mut env = parent_env.clone();
        let mut pi = 0;
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Param {
                if let Payload::Cid(c) = self.il.node(k).payload {
                    if let Some(&v) = params.get(pi) {
                        env.insert(c, v);
                    }
                    pi += 1;
                }
            }
        }
        let body = *kids.last()?;
        self.eval_block_return(body, &mut env)
    }

    fn eval_filter_map_lambda_body(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<(ValueId, Option<ValueId>)> {
        match self.eval_filter_map_lambda_result(lambda, params, parent_env)? {
            FilterMapResult::Emit { value, predicate } => Some((value, predicate)),
            FilterMapResult::Drop => None,
        }
    }

    fn eval_filter_map_lambda_result(
        &mut self,
        lambda: NodeId,
        params: &[ValueId],
        parent_env: &FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda).to_vec();
        let mut env = parent_env.clone();
        let mut pi = 0;
        for &k in &kids {
            if self.il.kind(k) == NodeKind::Param {
                if let Payload::Cid(c) = self.il.node(k).payload {
                    if let Some(&v) = params.get(pi) {
                        env.insert(c, v);
                    }
                    pi += 1;
                }
            }
        }
        self.eval_filter_map_output(*kids.last()?, &mut env)
    }

    fn eval_filter_map_output(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        match self.il.kind(node) {
            NodeKind::Block => {
                let kids = self.il.children(node).to_vec();
                let n = kids.len();
                for (i, &stmt) in kids.iter().enumerate() {
                    if self.il.kind(stmt) == NodeKind::Assign {
                        self.eval_filter_map_assignment(stmt, env);
                        continue;
                    }
                    if self.il.kind(stmt) == NodeKind::Return || i + 1 == n {
                        return self.eval_filter_map_output(stmt, env);
                    }
                    if !matches!(self.il.kind(stmt), NodeKind::ExprStmt) {
                        continue;
                    }
                    return None;
                }
                None
            }
            NodeKind::Return | NodeKind::ExprStmt => self
                .il
                .children(node)
                .first()
                .and_then(|&expr| self.eval_filter_map_output(expr, env)),
            NodeKind::Assign => {
                self.eval_filter_map_assignment(node, env);
                None
            }
            NodeKind::If => self.eval_filter_map_if(node, env),
            NodeKind::Lit if self.is_null_literal(node) => Some(FilterMapResult::Drop),
            NodeKind::Call => {
                if let Some((receiver, lambda)) = self.rust_option_and_then_call_parts(node) {
                    return self.eval_filter_map_and_then(receiver, lambda, env);
                }
                let value = self
                    .rust_some_call_arg(node)
                    .map(|value| self.eval(value, env))?;
                Some(FilterMapResult::Emit {
                    value,
                    predicate: None,
                })
            }
            _ => None,
        }
    }

    fn eval_filter_map_and_then(
        &mut self,
        receiver: NodeId,
        lambda: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        let receiver_result = self.eval_filter_map_output(receiver, env)?;
        let FilterMapResult::Emit { value, predicate } = receiver_result else {
            return Some(FilterMapResult::Drop);
        };
        let inner_result = self.eval_filter_map_lambda_result(lambda, &[value], env)?;
        match inner_result {
            FilterMapResult::Emit {
                value,
                predicate: inner_predicate,
            } => Some(FilterMapResult::Emit {
                value,
                predicate: self.and_preds(predicate, inner_predicate),
            }),
            FilterMapResult::Drop => Some(FilterMapResult::Drop),
        }
    }

    fn eval_filter_map_assignment(&mut self, node: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(node).to_vec();
        if kids.len() == 2 && self.il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                let rhs = self.eval(kids[1], env);
                env.insert(c, rhs);
            }
        }
    }

    fn eval_filter_map_if(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<FilterMapResult> {
        let kids = self.il.children(node).to_vec();
        let cond_node = *kids.first()?;
        let then_node = *kids.get(1)?;
        let else_node = *kids.get(2)?;
        let cond = self.eval(cond_node, env);
        let mut then_env = env.clone();
        let then_result = self.eval_filter_map_output(then_node, &mut then_env)?;
        let mut else_env = env.clone();
        let else_result = self.eval_filter_map_output(else_node, &mut else_env)?;
        match (then_result, else_result) {
            (FilterMapResult::Emit { value, predicate }, FilterMapResult::Drop) => {
                Some(FilterMapResult::Emit {
                    value,
                    predicate: self.and_preds(Some(cond), predicate),
                })
            }
            (FilterMapResult::Drop, FilterMapResult::Emit { value, predicate }) => {
                let not_cond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
                Some(FilterMapResult::Emit {
                    value,
                    predicate: self.and_preds(Some(not_cond), predicate),
                })
            }
            (
                FilterMapResult::Emit {
                    value: then_value,
                    predicate: None,
                },
                FilterMapResult::Emit {
                    value: else_value,
                    predicate: None,
                },
            ) => Some(FilterMapResult::Emit {
                value: self.mk(ValOp::Phi, vec![cond, then_value, else_value]),
                predicate: None,
            }),
            _ => None,
        }
    }

    fn rust_some_call_arg(&self, node: NodeId) -> Option<NodeId> {
        if self.il.kind(node) != NodeKind::Call {
            return None;
        }
        let kids = self.il.children(node);
        if kids.len() != 2 {
            return None;
        }
        let callee = kids[0];
        let (NodeKind::Var, Payload::Name(name)) =
            (self.il.kind(callee), self.il.node(callee).payload)
        else {
            return None;
        };
        let text = self.interner.resolve(name);
        (text == "Some" || text.ends_with("::Some")).then_some(kids[1])
    }

    fn rust_option_and_then_call_parts(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        if self.il.meta.lang != Lang::Rust || self.il.kind(node) != NodeKind::Call {
            return None;
        }
        let kids = self.il.children(node);
        if kids.len() != 2 || self.il.kind(kids[0]) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = self.il.node(kids[0]).payload else {
            return None;
        };
        if self.interner.resolve(method) != "and_then" {
            return None;
        }
        let receiver = *self.il.children(kids[0]).first()?;
        Some((receiver, kids[1]))
    }

    fn is_rust_vec_new_call(&self, callee: NodeId) -> bool {
        let (NodeKind::Var, Payload::Name(name)) =
            (self.il.kind(callee), self.il.node(callee).payload)
        else {
            return false;
        };
        let text = self.interner.resolve(name);
        text == "Vec::new" || text.ends_with("::Vec::new")
    }

    fn is_null_literal(&self, node: NodeId) -> bool {
        matches!(
            (self.il.kind(node), self.il.node(node).payload),
            (NodeKind::Lit, Payload::Lit(nose_il::LitClass::Null))
        )
    }

    /// Walk a (possibly nested) block, applying assignments to `env`, and return the
    /// value of the first `return` expression reached.
    fn eval_block_return(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match self.il.kind(node) {
            NodeKind::Block => {
                let kids = self.il.children(node).to_vec();
                let n = kids.len();
                for (i, &s) in kids.iter().enumerate() {
                    // An explicit `return` anywhere wins; a let-binding binds; the LAST
                    // statement is the *implicit* return value (Rust closures and Ruby
                    // blocks have no `return` — their trailing expression is the result).
                    if let Some(v) = self.eval_block_return(s, env) {
                        return Some(v);
                    }
                    if i + 1 == n {
                        if let NodeKind::ExprStmt = self.il.kind(s) {
                            return self.il.children(s).first().map(|&e| self.eval(e, env));
                        }
                    }
                }
                None
            }
            NodeKind::Return => self.il.children(node).first().map(|&e| self.eval(e, env)),
            NodeKind::Assign => {
                let kids = self.il.children(node).to_vec();
                if kids.len() == 2 && self.il.kind(kids[0]) == NodeKind::Var {
                    if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                        let rhs = self.eval(kids[1], env);
                        env.insert(c, rhs);
                    }
                }
                None
            }
            // A bare-expression lambda body (`|a, v| a + v`) — its value is the result.
            NodeKind::ExprStmt => self.il.children(node).first().map(|&e| self.eval(e, env)),
            _ => Some(self.eval(node, env)),
        }
    }

    /// Whether `target` appears anywhere in `v`'s value subgraph (DAG-safe).
    fn references(&self, v: ValueId, target: ValueId) -> bool {
        let mut stack = vec![v];
        let mut seen = FxHashSet::default();
        while let Some(x) = stack.pop() {
            if x == target {
                return true;
            }
            if seen.insert(x) {
                stack.extend(self.nodes[x as usize].args.iter().copied());
            }
        }
        false
    }

    fn references_cached(&self, v: ValueId, target: ValueId, cache: &mut ReductionCache) -> bool {
        let key = (v, target);
        if let Some(&cached) = cache.references.get(&key) {
            return cached;
        }
        let result = self.references(v, target);
        cache.references.insert(key, result);
        result
    }

    fn references_any_cached(
        &self,
        v: ValueId,
        targets: &[ValueId],
        cache: &mut ReductionCache,
    ) -> bool {
        targets
            .iter()
            .copied()
            .any(|target| self.references_cached(v, target, cache))
    }

    fn eval(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
        let node = *self.il.node(expr);
        match node.kind {
            NodeKind::Var => match node.payload {
                Payload::Cid(c) => env
                    .get(&c)
                    .copied()
                    .unwrap_or_else(|| self.mk(ValOp::Input(c), vec![])),
                // A free variable (global / un-canonicalized callee) kept its name in
                // alpha — give it a STABLE identity keyed by that name (high-bit range,
                // clear of positional cids), so `foo(x)` ≠ `bar(x)` while two uses of
                // `foo` agree. Without this, distinct globals collapsed to one cid.
                Payload::Name(s) => {
                    if let Some(&v) = self.global_env.get(&s) {
                        return v;
                    }
                    let key = 0x8000_0000u32 | (self.interner.symbol_hash(s) as u32);
                    self.mk(ValOp::Input(key), vec![])
                }
                _ => self.fresh_opaque(),
            },
            NodeKind::Lit => {
                // Behavior-defining constants must be distinct values: `0` ≠ `1`
                // (else `x % 2 == 0` and `x % 2 == 1` collapse). Retained small ints
                // key by value (offset clear of the class range); others by class.
                let key = match node.payload {
                    Payload::LitInt(v) => 0x1000_0000u32.wrapping_add(v as u32),
                    // strings in a separate key range from ints to avoid collision
                    Payload::LitStr(h) => 0x2000_0000u32.wrapping_add(h as u32),
                    // floats keyed by source-text hash, in their own range (so `3.14`≠`2.71`)
                    Payload::LitFloat(h) => 0x4000_0000u32.wrapping_add(h as u32),
                    // true/false are behavior-defining and must be distinct (own range)
                    Payload::LitBool(b) => 0x3000_0001u32 + b as u32,
                    Payload::Lit(c) => c as u32,
                    _ => 0,
                };
                self.mk(ValOp::Const(key), vec![])
            }
            NodeKind::BinOp => {
                let op = op_code(node.payload);
                let kids = self.il.children(expr).to_vec();
                if op == Op::In as u32 && kids.len() == 2 {
                    let element = self.eval(kids[0], env);
                    if self.is_js_like_lang() {
                        let collection = self.eval(kids[1], env);
                        return self
                            .mk(ValOp::Call(JS_PROTOTYPE_IN_CODE), vec![element, collection]);
                    }
                    if let Some(map) = self.proven_map_key_view_expr(kids[1], env) {
                        return self.mk(ValOp::Bin(op), vec![element, map]);
                    }
                    let collection = self.eval_membership_collection(kids[1], env);
                    return self.mk(ValOp::Bin(op), vec![element, collection]);
                }
                if let Some(v) = self.eval_static_filter_membership_comparison(op, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_len_zero_comparison(op, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_static_index_membership_comparison(op, &kids, env) {
                    return v;
                }
                if op == Op::Add as u32 || op == Op::Sub as u32 {
                    let mut operands = Vec::new();
                    self.collect_add_sub_expr_operands(expr, false, &mut operands);
                    if operands.len() >= LARGE_AC_EXPR_OPERANDS {
                        return self.compact_add_sub_formula(operands, env);
                    }
                }
                // Canonicalize subtraction to addition-of-negation: `a - b ≡ a + (-b)`
                // (sound for the two's-complement Int model: a.wrapping_sub(b) ==
                // a.wrapping_add(-b)). Routing it through the AC `+` normalization unifies
                // `a - b`, `a + (-b)`, and `-b + a` to one fingerprint — converging the
                // many subtraction/negation algebraic variants (e.g. sympy `__sub__`
                // `self + (-a)` with a sibling `self - a`). `verify` is the soundness gate.
                if op == Op::Sub as u32 && kids.len() == 2 {
                    let a = self.eval(kids[0], env);
                    let b = self.eval(kids[1], env);
                    let neg_b = self.mk(ValOp::Un(Op::Neg as u32), vec![b]);
                    let mut operands = Vec::new();
                    self.flatten_into(a, Op::Add as u32, &mut operands);
                    self.flatten_into(neg_b, Op::Add as u32, &mut operands);
                    // Sort unless an operand is proven concat (string/list); otherwise the
                    // operands Err in the oracle regardless of order, so sorting is safe.
                    if operands.iter().all(|&v| !is_concat_ty(self.vty(v))) {
                        operands.sort_by_key(|&v| self.vhash[v as usize]);
                    }
                    let mut acc = operands[0];
                    for &o in &operands[1..] {
                        acc = self.mk(ValOp::Bin(Op::Add as u32), vec![acc, o]);
                    }
                    return acc;
                }
                if is_assoc_comm_code(op) {
                    // Flatten the chain (resolving temps), sort by structural hash, and
                    // rebuild canonically — so groupings/temps converge. EXCEPT `+` is only
                    // commutative on numeric operands; on strings/lists it is concat, which
                    // is ordered, so we keep source order there (sorting would be unsound).
                    //
                    // Very large generated formulas can arrive as deeply nested binary ASTs.
                    // For those, collect same-op source operands first so one giant expression
                    // pays for flatten/sort/rebuild once instead of once per nested pair.
                    let mut expr_operands = Vec::new();
                    for &k in &kids {
                        self.collect_ac_expr_operands(k, op, &mut expr_operands);
                    }
                    if expr_operands.len() >= LARGE_AC_EXPR_OPERANDS {
                        let mut operands = Vec::new();
                        for k in expr_operands {
                            let v = self.eval(k, env);
                            self.flatten_into(v, op, &mut operands);
                        }
                        if let Some(v) = self.c_u32_be_byte_pack_pattern(&operands) {
                            return v;
                        }
                        let do_sort = op != Op::Add as u32
                            || operands.iter().all(|&v| !is_concat_ty(self.vty(v)));
                        if do_sort {
                            operands.sort_by_key(|&v| self.vhash[v as usize]);
                        }
                        return self.compact_formula(op, &operands);
                    }

                    let mut operands = Vec::new();
                    for k in kids {
                        let v = self.eval(k, env);
                        self.flatten_into(v, op, &mut operands);
                    }
                    if let Some(v) = self.c_u32_be_byte_pack_pattern(&operands) {
                        return v;
                    }
                    let do_sort = op != Op::Add as u32
                        || operands.iter().all(|&v| !is_concat_ty(self.vty(v)));
                    if do_sort {
                        operands.sort_by_key(|&v| self.vhash[v as usize]);
                    }
                    let mut acc = operands[0];
                    for &o in &operands[1..] {
                        acc = self.mk(ValOp::Bin(op), vec![acc, o]);
                    }
                    acc
                } else {
                    let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                    self.mk(ValOp::Bin(op), a)
                }
            }
            NodeKind::UnOp => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let op = op_code(node.payload);
                self.mk(ValOp::Un(op), a)
            }
            NodeKind::Field => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let name = match node.payload {
                    Payload::Name(s) => self.interner.symbol_hash(s),
                    _ => 0,
                };
                if a.len() == 1 {
                    if let Payload::Name(s) = node.payload {
                        let receiver = &self.nodes[a[0] as usize];
                        if matches!(receiver.op, ValOp::Seq(6)) && receiver.args.len() == 1 {
                            let module = receiver.args[0];
                            let exported = self.mk(
                                ValOp::Const(stable_string_const_key(self.interner.resolve(s))),
                                vec![],
                            );
                            return self.mk(ValOp::Seq(5), vec![module, exported]);
                        }
                    }
                }
                if let Some(&written) = self.field_env.get(&name) {
                    return written;
                }
                self.mk(ValOp::Field(name), a)
            }
            NodeKind::Index => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                if a.len() == 2 {
                    if let Some((map, default)) = self.proven_go_literal_zero_map_value(a[0]) {
                        return self.mk(
                            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                            vec![map, a[1], default],
                        );
                    }
                }
                self.mk(ValOp::Index, a)
            }
            NodeKind::Call => {
                let kids = self.il.children(expr).to_vec();
                // `p.then(λr. body)` is a Promise continuation ≡ `const r = await p; body`.
                // Since `await` is already stripped to its operand, beta-reduce the callback
                // with the receiver's value so a `.then`-chain converges with the equivalent
                // await / sequential code (the #1 real-world async Type-4 gap). Chains reduce
                // recursively (evaluating the receiver triggers the inner `.then`).
                if let Some(v) = self.eval_promise_then(expr, env) {
                    return v;
                }
                // Reduction builtins fold a collection to one value — canonicalize to
                // the same `Reduce(op, init, per-element contrib)` a loop produces, so
                // `sum(x*x for x in xs)` / `reduce(λa,x. a+x*x, xs, 0)` converge with
                // the explicit accumulator loop (§AI representation axis).
                if let Payload::Builtin(Builtin::Abs) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        return self.mk(ValOp::Un(ABS_CODE), vec![v]);
                    }
                }
                if let Payload::Builtin(Builtin::IsNull | Builtin::IsNotNull) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        let op = if matches!(node.payload, Payload::Builtin(Builtin::IsNull)) {
                            Op::Eq
                        } else {
                            Op::Ne
                        };
                        let nil = self.null_const();
                        return self.mk(ValOp::Bin(op as u32), vec![v, nil]);
                    }
                }
                if let Payload::Builtin(Builtin::IsEmpty) = node.payload {
                    if let Some(&arg) = kids.first() {
                        let v = self.eval(arg, env);
                        return self.is_empty_value(v);
                    }
                }
                if let Payload::Builtin(Builtin::Contains) = node.payload {
                    if let [element, collection] = kids.as_slice() {
                        let element = self.eval(*element, env);
                        if let Some(map) = self.proven_map_key_view_expr(*collection, env) {
                            return self.mk(ValOp::Bin(Op::In as u32), vec![element, map]);
                        }
                        let collection = self.eval_membership_collection(*collection, env);
                        return self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]);
                    }
                }
                if let Payload::Builtin(Builtin::GetOrDefault) = node.payload {
                    if let [map, key, default] = kids.as_slice() {
                        let map = self.eval_map_lookup_collection(*map, env);
                        let key = self.eval(*key, env);
                        let default = self.eval(*default, env);
                        return self.mk(
                            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
                            vec![map, key, default],
                        );
                    }
                }
                if let Payload::Builtin(Builtin::ValueOrDefault) = node.payload {
                    if let [value, default] = kids.as_slice() {
                        let value = self.eval(*value, env);
                        let default = self.eval(*default, env);
                        return self.mk_value_default(value, default);
                    }
                }
                if let Payload::Builtin(b) = node.payload {
                    if let Some(r) = self.eval_reduction_builtin(b, &kids, env) {
                        return r;
                    }
                }
                if let Some(r) = self.eval_count_call(&kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_product_call(&kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_numeric_method_call(&kids, env) {
                    return r;
                }
                if kids.len() == 1 && self.is_rust_vec_new_call(kids[0]) {
                    return self.mk(ValOp::Seq(1), vec![]);
                }
                // Iteration-identity adapters: `xs.iter()` / `.into_iter()` / `.collect()`
                // / `.to_vec()` / `.copied()` / `.cloned()` don't change *what* is iterated
                // or produced, so peel them — `xs.iter().map(f).collect()` (Rust) converges
                // with `[f(x) for x in xs]` (Python). The callee is a `Field` whose base is
                // the receiver; the call has no value arguments.
                if kids.len() == 1 && self.il.kind(kids[0]) == NodeKind::Field {
                    if let Payload::Name(s) = self.il.node(kids[0]).payload {
                        if matches!(
                            self.interner.resolve(s),
                            "iter"
                                | "into_iter"
                                | "iter_mut"
                                | "collect"
                                | "to_vec"
                                | "copied"
                                | "cloned"
                        ) {
                            if let Some(&base) = self.il.children(kids[0]).first() {
                                return self.eval(base, env);
                            }
                        }
                    }
                }
                // 2-way `min(x, y)` / `max(x, y)` → canonical Min/Max, converging with the
                // ternary idiom `x if x<y else y`. (1-arg `min(iterable)` is a reduction,
                // handled above.) The callee is a free `Var` keeping its name after alpha.
                if kids.len() == 3 {
                    if let (NodeKind::Var, Payload::Name(s)) =
                        (self.il.kind(kids[0]), self.il.node(kids[0]).payload)
                    {
                        let code = match self.interner.resolve(s) {
                            "min" => Some(MIN_CODE),
                            "max" => Some(MAX_CODE),
                            _ => None,
                        };
                        if let Some(c) = code {
                            let x = self.eval(kids[1], env);
                            let y = self.eval(kids[2], env);
                            return self.mk(ValOp::Bin(c), vec![x, y]);
                        }
                    }
                }
                if let Some(r) = self.eval_proven_collection_membership_call(&kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_key_membership_call(&kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_get_default_call(&kids, env) {
                    return r;
                }
                if self.is_unproven_membership_like_call(expr, &kids) {
                    let salt = self.source_salted_hash(expr, 0x4D45_4D42_4552);
                    return self.mk(ValOp::Opaque(salt), vec![]);
                }
                // Interprocedural pure inline: `f(args)` to a pure file-local function ≡ its body
                // with `args` substituted — converges with the same logic written inline / with
                // a different extracted helper. Sound (β-reduction of an effect-free function).
                if let Some(v) = self.eval_inlined_call(&kids, env) {
                    return v;
                }
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                let tag = match node.payload {
                    Payload::Builtin(b) => builtin_tag(b),
                    _ => 0,
                };
                self.mk(ValOp::Call(tag), a)
            }
            NodeKind::HoF => {
                let kind = match node.payload {
                    Payload::HoF(h) => h,
                    _ => HoFKind::Map,
                };
                let kids = self.il.children(expr).to_vec();
                match kind {
                    // `xs.map(λx. body)` / a comprehension → the per-element value over a
                    // canonical `Elem(xs)`, so `[x*x for x in xs]` and `xs.map(x=>x*x)`
                    // converge regardless of the (opaque) lambda's syntax. `map_source`
                    // resolves the collection to its element stream and ANY predicate it
                    // carries (a filtered collection is a `Hof(Map, [elem, pred])`, see the
                    // `Filter` arm below) — that carried predicate is map/filter FUSION:
                    // `map(h, filter(p, xs))` ≡ `filtered-map h@p`, so `[h(y) for y in
                    // [x for x in xs if p]]` and `[h(x) for x in xs if p]` converge.
                    HoFKind::Map => {
                        let (elems, carried_pred) = self.map_source(kids.first().copied(), env);
                        let fallback = elems
                            .first()
                            .copied()
                            .unwrap_or_else(|| self.fresh_opaque());
                        let contrib = match kids.get(1) {
                            Some(&l) => self.eval_lambda_body(l, &elems, env).unwrap_or(fallback),
                            None => fallback,
                        };
                        match carried_pred {
                            Some(p) => self.mk(ValOp::Hof(kind as u32), vec![contrib, p]),
                            None => self.mk(ValOp::Hof(kind as u32), vec![contrib]),
                        }
                    }
                    HoFKind::FlatMap => {
                        let (elems, carried_pred) = self.map_source(kids.first().copied(), env);
                        let outer_elem = elems
                            .first()
                            .copied()
                            .unwrap_or_else(|| self.fresh_opaque());
                        let inner = match kids.get(1) {
                            Some(&l) => self
                                .eval_lambda_body(l, &elems, env)
                                .unwrap_or_else(|| self.fresh_opaque()),
                            None => self.fresh_opaque(),
                        };
                        // proof-obligation: normalize.value_graph.flatmap_identity
                        // `flatMap(λx. x)` (identity inner: the lambda returns the outer
                        // element unchanged) ≡ `flatMap(λx. map(λy. y, x))` ≡ flatten — the
                        // monad law `flatMap id = join` / `concatMap id = concat`. Canonicalize
                        // the identity inner to the modeled element-stream inner `Map[Elem(x)]`
                        // so it converges with the nested builder loop and the explicit
                        // inner-identity-map form. Sound: `map id = id` on the sublist, so every
                        // emitted element is unchanged. A non-identity inner (`x.map(y=>y+1)`,
                        // changed element) does not equal `outer_elem`, so it is left intact.
                        let inner = if inner == outer_elem {
                            let elem = self.elem(outer_elem);
                            self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem])
                        } else {
                            inner
                        };
                        let mut args = vec![outer_elem, inner];
                        if let Some(p) = carried_pred {
                            args.push(p);
                        }
                        self.mk(ValOp::Hof(kind as u32), args)
                    }
                    HoFKind::FilterMap => {
                        let (elems, carried_pred) = self.map_source(kids.first().copied(), env);
                        let Some((contrib, own_pred)) = kids
                            .get(1)
                            .and_then(|&l| self.eval_filter_map_lambda_body(l, &elems, env))
                        else {
                            let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                            return self.mk(ValOp::Hof(kind as u32), a);
                        };
                        match self.and_preds(own_pred, carried_pred) {
                            Some(p) => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib, p]),
                            None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![contrib]),
                        }
                    }
                    HoFKind::Filter => {
                        // `filter(p, coll)` ≡ the *identity map with a predicate*:
                        // `Hof(Map, [Elem(coll), pred])`. Representing a filter this way
                        // (rather than the old `Hof(Filter, [pred])`, which stored ONLY the
                        // predicate and lost the element stream) makes `Filter` carry its
                        // element — so nested filters FUSE: `filter(q, filter(p, xs))` and
                        // `filter(p∧q, xs)` both reduce to `Hof(Map, [Elem(xs), p∧q])`. It
                        // also unifies a standalone filter with the filtered-loop builder
                        // (`r=[]; for x: if p: r.append(x)` → the same node) and the filtered
                        // comprehension `[x for x in xs if p]`
                        // (`normalize.value_graph.functor`).
                        let (elems, carried_pred) = self.map_source(kids.first().copied(), env);
                        let elem = elems
                            .first()
                            .copied()
                            .unwrap_or_else(|| self.fresh_opaque());
                        let own_pred = kids
                            .get(1)
                            .and_then(|&l| self.eval_lambda_body(l, &elems, env));
                        match self.and_preds(own_pred, carried_pred) {
                            Some(p) => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem, p]),
                            None => self.mk(ValOp::Hof(HoFKind::Map as u32), vec![elem]),
                        }
                    }
                    _ => {
                        let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                        self.mk(ValOp::Hof(kind as u32), a)
                    }
                }
            }
            NodeKind::Seq => {
                let kids = self.il.children(expr).to_vec();
                let a: Vec<ValueId> = kids.iter().map(|&k| self.eval(k, env)).collect();
                if matches!(node.payload, Payload::Builtin(Builtin::DictEntry)) {
                    return self.dict_entry(a);
                }
                if let Some(map) = self.proven_go_literal_zero_map_seq(expr, &a) {
                    return map;
                }
                self.mk(ValOp::Seq(self.seq_tag(expr)), a)
            }
            NodeKind::If => {
                // Ternary / expression-if. Rust closures lower `if c { x } else { y }`
                // branches as Blocks whose trailing expression is the branch value, so
                // evaluate branches with the same implicit-return rule used for lambdas.
                let kids = self.il.children(expr).to_vec();
                let mut a = Vec::new();
                if let Some(&cond) = kids.first() {
                    a.push(self.eval(cond, env));
                }
                for &branch in kids.iter().skip(1).take(2) {
                    let mut branch_env = env.clone();
                    let value = self
                        .eval_block_return(branch, &mut branch_env)
                        .unwrap_or_else(|| self.eval(branch, env));
                    a.push(value);
                }
                // abs/min/max idiom recognition happens in `mk(Phi, …)` so it applies to
                // both the ternary and the equivalent if/else-assign form uniformly.
                self.mk(ValOp::Phi, a)
            }
            NodeKind::Lambda => {
                let hash = self.subtree_hash(expr);
                self.mk(ValOp::Lambda(hash), vec![])
            }
            // Any unlowered / unhandled construct — notably `Raw`, which wraps a
            // macro, C compound literal, `#ifdef`, parse-ERROR, etc. Key it by its full
            // subtree hash (surface kind + lowered children), exactly like `Lambda`, so
            // behaviorally-different unlowered constructs produce DIFFERENT fingerprints.
            // A positional opaque counter collapsed them (e.g. two distinct C compound
            // literals → one fingerprint = an unsound false merge the interpreter oracle
            // can't catch, since `Raw` is uninterpretable). Identical constructs converge.
            _ => {
                let hash = self.subtree_hash(expr);
                self.mk(ValOp::Opaque(hash), vec![])
            }
        }
    }

    /// Multiset of value-node hashes reachable from the unit's sinks, plus the
    /// sink-tagged hashes themselves, and (separately) just the literal `Const`
    /// hashes. Sorted for a canonical, order-independent fingerprint.
    /// Heavy sub-DAG ANCHORS: the structural hashes of reachable value-nodes whose
    /// sub-computation is at least `min_weight` nodes. Two units that share an anchor hash
    /// compute the *exact same* sub-value (hash-consed) — so a shared, large, rare anchor is a
    /// partial clone: an extractable common sub-computation, even when the units differ
    /// elsewhere (the case whole-unit Jaccard misses). `Const`/`Input`/`Elem` leaves are never
    /// anchors (no computation to extract). Weight is the memoized subtree size (args precede
    /// their parent in id order); capped so a deeply-shared DAG can't blow it up.
    fn anchors(&self, min_weight: u32) -> Vec<u64> {
        const WEIGHT_CAP: u32 = 1 << 20;
        let n = self.nodes.len();
        let mut reachable = vec![false; n];
        let mut stack: Vec<ValueId> = self.sinks.iter().map(|s| s.value).collect();
        while let Some(v) = stack.pop() {
            let vi = v as usize;
            if reachable[vi] {
                continue;
            }
            reachable[vi] = true;
            for &a in &self.nodes[vi].args {
                if !reachable[a as usize] {
                    stack.push(a);
                }
            }
        }
        let mut weight = vec![0u32; n];
        for i in 0..n {
            let mut w: u32 = 1;
            for &a in &self.nodes[i].args {
                w = w.saturating_add(weight[a as usize]);
            }
            weight[i] = w.min(WEIGHT_CAP);
        }
        let mut out = Vec::new();
        for i in 0..n {
            if reachable[i]
                && weight[i] >= min_weight
                && !matches!(
                    self.nodes[i].op,
                    ValOp::Const(_) | ValOp::Input(_) | ValOp::Elem(_)
                )
            {
                out.push(self.vhash[i]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn fingerprint_lits(&self) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
        let h = &self.vhash; // structural hashes, maintained during construction
                             // reachable from sinks
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack: Vec<ValueId> = self.sinks.iter().map(|sink| sink.value).collect();
        while let Some(v) = stack.pop() {
            let vi = v as usize;
            if reachable[vi] {
                continue;
            }
            reachable[vi] = true;
            for &a in &self.nodes[vi].args {
                if !reachable[a as usize] {
                    stack.push(a);
                }
            }
        }
        let mut out: Vec<u64> = Vec::new();
        let mut lits: Vec<u64> = Vec::new();
        for i in 0..self.nodes.len() {
            if reachable[i] {
                out.push(h[i]);
                if matches!(self.nodes[i].op, ValOp::Const(_)) {
                    lits.push(h[i]);
                }
            }
        }
        let mut returns: Vec<u64> = Vec::new();
        for sink in &self.sinks {
            let mut tag = 0x5117 + sink.kind as u64;
            if matches!(sink.kind, SinkKind::Effect) {
                if let Some(ord) = sink.effect_ord {
                    tag = combine(tag, EFFECT_ORDINAL_SINK_TAG ^ u64::from(ord));
                }
            }
            out.push(combine(tag, h[sink.value as usize]));
            if matches!(sink.kind, SinkKind::Return) {
                returns.push(h[sink.value as usize]);
            }
        }
        out.sort_unstable();
        lits.sort_unstable();
        returns.sort_unstable();
        (out, lits, returns)
    }

    fn seq_tag(&self, node: NodeId) -> u64 {
        match self.il.node(node).payload {
            Payload::Name(s) => match self.interner.resolve(s) {
                "array" | "list" | "array_expression" | "composite_literal" => 1,
                "tuple" | "tuple_expression" => 2,
                "object" | "dictionary" | "hash" => 3,
                "pair" => 4,
                "import_binding" => 5,
                "import_namespace" => 6,
                "record_guard" => 7,
                "own_property_guard" => OWN_PROPERTY_GUARD_SEQ_TAG,
                _ => self.interner.symbol_hash(s),
            },
            _ => 0,
        }
    }
}

fn collect_assigned(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Assign {
        if let Some(&lhs) = il.children(node).first() {
            if il.kind(lhs) == NodeKind::Var {
                if let Payload::Cid(c) = il.node(lhs).payload {
                    out.insert(c);
                }
            }
        }
    }
    for &c in il.children(node) {
        collect_assigned(il, c, out);
    }
}

fn op_code(p: Payload) -> u32 {
    match p {
        Payload::Op(op) => op as u32,
        _ => 0,
    }
}

/// All canonical variable ids referenced anywhere in `node`'s subtree.
fn mentioned_cids(il: &Il, node: NodeId) -> FxHashSet<u32> {
    let mut out = FxHashSet::default();
    mentioned_scan(il, node, &mut out);
    out
}

fn mentioned_scan(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            out.insert(c);
        }
    }
    for &c in il.children(node) {
        mentioned_scan(il, c, out);
    }
}

/// Loop induction variables: those updated by `i = i ± constant` in the body.
fn induction_vars(il: &Il, body: NodeId) -> FxHashSet<u32> {
    let mut out = FxHashSet::default();
    induction_scan(il, body, &mut out);
    out
}

fn induction_scan(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 && il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(kids[0]).payload {
                if is_increment(il, kids[1], c) {
                    out.insert(c);
                }
            }
        }
    }
    for &c in il.children(node) {
        induction_scan(il, c, out);
    }
}

/// The constant step of induction variable `cid` if the body updates it *exactly once*
/// as `i = i + k` / `i = k + i` / `i = i - k` (k an int literal); else `None`. `k - i`
/// is a reflection (not a step) and is rejected, as is a variable updated 0 or ≥2 times.
fn induction_step(il: &Il, body: NodeId, cid: u32) -> Option<i64> {
    let mut step = None;
    let mut count = 0u32;
    induction_step_scan(il, body, cid, &mut step, &mut count);
    if count == 1 {
        step
    } else {
        None
    }
}

fn induction_step_scan(il: &Il, node: NodeId, cid: u32, step: &mut Option<i64>, count: &mut u32) {
    if il.kind(node) == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 && il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(kids[0]).payload {
                if c == cid {
                    *count += 1;
                    *step = increment_amount(il, kids[1], cid);
                }
            }
        }
    }
    for &c in il.children(node) {
        induction_step_scan(il, c, cid, step, count);
    }
}

/// The signed step if `expr` is `cid + k`, `k + cid`, or `cid - k` (k an int literal);
/// `k - cid` and anything else → `None`.
fn increment_amount(il: &Il, expr: NodeId, cid: u32) -> Option<i64> {
    if il.kind(expr) != NodeKind::BinOp {
        return None;
    }
    let kids = il.children(expr);
    if kids.len() != 2 {
        return None;
    }
    let is_self = |n: NodeId| matches!((il.kind(n), il.node(n).payload), (NodeKind::Var, Payload::Cid(c)) if c == cid);
    let lit = |n: NodeId| match il.node(n).payload {
        Payload::LitInt(v) => Some(v),
        _ => None,
    };
    match il.node(expr).payload {
        Payload::Op(Op::Add) => {
            if is_self(kids[0]) {
                lit(kids[1])
            } else if is_self(kids[1]) {
                lit(kids[0])
            } else {
                None
            }
        }
        // Only `i - k` is a step; `k - i` reflects.
        Payload::Op(Op::Sub) if is_self(kids[0]) => lit(kids[1]).map(|v| -v),
        _ => None,
    }
}

/// Whether `expr` is `cid ± literal` — a step of the induction variable `cid`.
fn is_increment(il: &Il, expr: NodeId, cid: u32) -> bool {
    if il.kind(expr) != NodeKind::BinOp
        || !matches!(
            il.node(expr).payload,
            Payload::Op(Op::Add) | Payload::Op(Op::Sub)
        )
    {
        return false;
    }
    let mut refs_self = false;
    let mut others_literal = true;
    for &k in il.children(expr) {
        match (il.kind(k), il.node(k).payload) {
            (NodeKind::Var, Payload::Cid(c)) if c == cid => refs_self = true,
            (NodeKind::Lit, _) => {}
            _ => others_literal = false,
        }
    }
    refs_self && others_literal
}

/// The complementary comparison op code, if `opc` is a comparison; else `None`.
fn negate_cmp_code(opc: u32) -> Option<u32> {
    let flip = if opc == Op::Lt as u32 {
        Op::Ge
    } else if opc == Op::Le as u32 {
        Op::Gt
    } else if opc == Op::Gt as u32 {
        Op::Le
    } else if opc == Op::Ge as u32 {
        Op::Lt
    } else if opc == Op::Eq as u32 {
        Op::Ne
    } else if opc == Op::Ne as u32 {
        Op::Eq
    } else {
        return None;
    };
    Some(flip as u32)
}

/// The same comparison with operands swapped: `a < b` becomes `b > a`.
fn reverse_cmp_code(opc: u32) -> Option<u32> {
    let rev = if opc == Op::Lt as u32 {
        Op::Gt
    } else if opc == Op::Le as u32 {
        Op::Ge
    } else if opc == Op::Gt as u32 {
        Op::Lt
    } else if opc == Op::Ge as u32 {
        Op::Le
    } else if opc == Op::Eq as u32 {
        Op::Eq
    } else if opc == Op::Ne as u32 {
        Op::Ne
    } else {
        return None;
    };
    Some(rev as u32)
}

fn is_commutative(opc: u32) -> bool {
    is_assoc_comm_code(opc)
        || opc == Op::Eq as u32
        || opc == Op::Ne as u32
        || opc == MIN_CODE
        || opc == MAX_CODE
}

/// Coarse type of a `Const` value node from its key range (mirrors the `eval` Lit keys):
/// int range → Num, string range → Str, bool range → Bool, small `LitClass` codes → their
/// type; sentinels (⊥, void-return, break) → Unknown.
/// Is this type a concatenation monoid (string/list) — where `+` is non-commutative?
fn is_concat_ty(t: Ty) -> bool {
    matches!(t, Ty::Str | Ty::List)
}

fn const_ty(k: u32) -> Ty {
    match k {
        0x1000_0000..=0x1FFF_FFFF => Ty::Num,
        0x2000_0000..=0x2FFF_FFFF => Ty::Str,
        0x3000_0001 | 0x3000_0002 => Ty::Bool,
        0x4000_0000..=0x4FFF_FFFF => Ty::Num, // retained float
        0 | 1 => Ty::Num,                     // LitClass::Int / Float
        2 => Ty::Str,                         // LitClass::Str
        3 => Ty::Bool,                        // LitClass::Bool
        _ => Ty::Unknown,
    }
}

/// Associative *and* commutative operators (flatten-eligible).
fn is_assoc_comm_code(opc: u32) -> bool {
    // NOTE: logical `And`/`Or` are deliberately ABSENT — short-circuit value-and/or is
    // associative but NOT commutative (`1 or 2` ≠ `2 or 1`; it returns the deciding
    // operand's value). Treating them as commutative here swapped their operands by hash
    // and silently merged `a or b` with `b or a` (a false merge the post-normalize oracle
    // can't see). They are instead rewritten to the positional `Phi` form in `mk`.
    opc == Op::Add as u32
        || opc == Op::Mul as u32
        || opc == Op::BitAnd as u32
        || opc == Op::BitOr as u32
        || opc == Op::BitXor as u32
}

/// `Reduce` op codes for the selection reductions (min/max). Kept clear of the small
/// `Op` discriminants (used for `+`/`*` folds) and of the `Const` int range.
const REDUCE_MAX: u32 = 0xFF00;
const REDUCE_MIN: u32 = 0xFF01;
/// `Reduce` op codes for the boolean short-circuit reductions: `any`/`some` (existential
/// OR) and `all`/`every` (universal AND). `REDUCE_ALL == REDUCE_ANY + 1` (the fold reuses
/// the offset to pick the OR/AND identity false/true).
const REDUCE_ANY: u32 = 0xFF02;
const REDUCE_ALL: u32 = 0xFF03;
const ORDERED_STRING_JOIN: u32 = 0xFF04;

/// `Un` op code for absolute value — `abs(x)` and the `x if x>=0 else -x` idiom both
/// canonicalize to `Un(ABS_CODE, [x])`. Clear of the small `Op` discriminants.
const ABS_CODE: u32 = 0xAB5;
/// Pseudo-ops for the 2-way min/max idiom (`x if x<y else y` ≡ `min(x,y)`), clear of the
/// `Op` discriminants and `ABS_CODE`. Commutative (min/max are symmetric).
const MIN_CODE: u32 = 0x319;
const MAX_CODE: u32 = 0x32A;
const JS_PROTOTYPE_IN_CODE: u32 = 0x4A53_494E;
const C_U16_BE_BYTE_PACK_CODE: u32 = 0x4331_3642;
const C_U32_BE_BYTE_PACK_CODE: u32 = 0x4333_3242;
const OWN_PROPERTY_GUARD_SEQ_TAG: u64 = 8;
const EFFECT_ORDINAL_SINK_TAG: u64 = 0xEFFE_C701;

/// A selection reduction (min/max) keeps no additive/multiplicative identity, so its
/// `Reduce` carries only the per-element contribution (no init) — a `max`-loop and
/// `max(gen)` then converge regardless of the loop's incidental seed value.
fn is_selection_code(op: u32) -> bool {
    // any/all carry only the per-element predicate (no accumulator seed), like min/max.
    op == REDUCE_MAX || op == REDUCE_MIN || op == REDUCE_ANY || op == REDUCE_ALL
}

/// The identity element of a reduction operator (`acc ⊕ identity = acc`), used to
/// neutralize a filtered-out element in a guarded reduction. Only the operators with
/// an integer-literal identity are handled (`+`→0, `*`→1).
fn identity_of(opc: u32) -> Option<u32> {
    if opc == Op::Add as u32 {
        Some(0)
    } else if opc == Op::Mul as u32 {
        Some(1)
    } else {
        None
    }
}

fn op_tag(op: &ValOp) -> u64 {
    let (k, p): (u64, u64) = match op {
        ValOp::Input(c) => (1, *c as u64),
        ValOp::Const(c) => (2, *c as u64),
        ValOp::Bin(o) => (3, *o as u64),
        ValOp::Un(o) => (4, *o as u64),
        ValOp::Field(n) => (5, *n),
        ValOp::Index => (6, 0),
        ValOp::Call(t) => (7, *t as u64),
        ValOp::Hof(h) => (8, *h as u64),
        ValOp::Clamp => (20, 0),
        ValOp::Seq(t) => (9, *t),
        ValOp::CollectionParam => (17, 0),
        ValOp::ArrayParam => (18, 0),
        ValOp::StringParam => (19, 0),
        ValOp::Phi => (10, 0),
        ValOp::Lambda(h) => (11, *h),
        ValOp::Loop(c) => (12, *c as u64),
        ValOp::Elem(h) => (14, *h),
        ValOp::Reduce(o) => (15, *o as u64),
        ValOp::Idx(h) => (16, *h),
        ValOp::Formula(h) => (19, *h),
        ValOp::Recurrence(h) => (18, *h),
        ValOp::Opaque(c) => (13, *c),
    };
    combine(k.wrapping_mul(0xF00D), p)
}

fn stable_string_const_key(value: &str) -> u32 {
    0x2000_0000u32.wrapping_add(stable_symbol_hash(value) as u32)
}

fn stable_float_const_key(value: &str) -> u32 {
    let normalized = value.trim().trim_end_matches(['f', 'F', 'd', 'D']);
    0x4000_0000u32.wrapping_add(stable_symbol_hash(normalized) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, ParamTypeFact, Span, Unit, UnitKind};

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, line, line)
    }

    #[derive(Clone, Copy)]
    enum ClampShape {
        MinMax,
        SwappedBounds,
        WrongNesting,
    }

    #[derive(Clone, Copy)]
    enum GuardShape {
        None,
        Exiting,
        NonExiting,
    }

    fn param(b: &mut IlBuilder, cid: u32, line: u32) -> NodeId {
        b.add(NodeKind::Param, Payload::Cid(cid), sp(line), &[])
    }

    fn var(b: &mut IlBuilder, cid: u32) -> NodeId {
        b.add(NodeKind::Var, Payload::Cid(cid), sp(10 + cid), &[])
    }

    fn int_lit(b: &mut IlBuilder, value: i64) -> NodeId {
        b.add(NodeKind::Lit, Payload::LitInt(value), sp(20), &[])
    }

    fn builtin(b: &mut IlBuilder, op: Builtin, args: &[NodeId]) -> NodeId {
        b.add(NodeKind::Call, Payload::Builtin(op), sp(30), args)
    }

    fn clamp_expr(
        b: &mut IlBuilder,
        shape: ClampShape,
        x: NodeId,
        lo: NodeId,
        hi: NodeId,
    ) -> NodeId {
        match shape {
            ClampShape::MinMax => {
                let inner = builtin(b, Builtin::Max, &[x, lo]);
                builtin(b, Builtin::Min, &[inner, hi])
            }
            ClampShape::SwappedBounds => {
                let inner = builtin(b, Builtin::Max, &[x, hi]);
                builtin(b, Builtin::Min, &[inner, lo])
            }
            ClampShape::WrongNesting => {
                let inner = builtin(b, Builtin::Min, &[x, lo]);
                builtin(b, Builtin::Max, &[inner, hi])
            }
        }
    }

    fn guarded_function(
        guard: GuardShape,
        shape: ClampShape,
        semantics: [Option<ParamSemantic>; 3],
    ) -> (usize, usize) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let px = param(&mut b, 0, 1);
        let plo = param(&mut b, 1, 2);
        let phi = param(&mut b, 2, 3);
        let mut stmts = Vec::new();
        if !matches!(guard, GuardShape::None) {
            let hi_guard = var(&mut b, 2);
            let lo_guard = var(&mut b, 1);
            let cond = b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Lt),
                sp(4),
                &[hi_guard, lo_guard],
            );
            let then_stmt = match guard {
                GuardShape::Exiting => {
                    let err = int_lit(&mut b, 0);
                    b.add(NodeKind::Throw, Payload::None, sp(5), &[err])
                }
                GuardShape::NonExiting => {
                    let err = int_lit(&mut b, 0);
                    b.add(NodeKind::ExprStmt, Payload::None, sp(5), &[err])
                }
                GuardShape::None => unreachable!(),
            };
            let then_block = b.add(NodeKind::Block, Payload::None, sp(5), &[then_stmt]);
            stmts.push(b.add(NodeKind::If, Payload::None, sp(4), &[cond, then_block]));
        }
        let x = var(&mut b, 0);
        let lo = var(&mut b, 1);
        let hi = var(&mut b, 2);
        let expr = clamp_expr(&mut b, shape, x, lo, hi);
        let ret = b.add(NodeKind::Return, Payload::None, sp(6), &[expr]);
        stmts.push(ret);
        let body = b.add(NodeKind::Block, Payload::None, sp(4), &stmts);
        let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, plo, phi, body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t.java".to_string(),
                lang: Lang::Java,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        );
        for (idx, semantic) in semantics.into_iter().enumerate() {
            if let Some(semantic) = semantic {
                il.param_type_facts.push(ParamTypeFact {
                    span: sp(idx as u32 + 1),
                    semantic,
                });
            }
        }
        let mut builder = Builder::new(&il, &interner);
        builder.build_unit(func);
        (
            builder.clamp_candidate_count,
            builder.clamp_proof_backed_candidate_count,
        )
    }

    fn literal_bound_function(shape: ClampShape, lo_value: i64, hi_value: i64) -> (usize, usize) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let px = param(&mut b, 0, 1);
        let x = var(&mut b, 0);
        let lo = int_lit(&mut b, lo_value);
        let hi = int_lit(&mut b, hi_value);
        let expr = clamp_expr(&mut b, shape, x, lo, hi);
        let ret = b.add(NodeKind::Return, Payload::None, sp(1), &[expr]);
        let body = b.add(NodeKind::Block, Payload::None, sp(1), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t.java".to_string(),
                lang: Lang::Java,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        );
        il.param_type_facts.push(ParamTypeFact {
            span: sp(1),
            semantic: ParamSemantic::Integer,
        });
        let mut builder = Builder::new(&il, &interner);
        builder.build_unit(func);
        (
            builder.clamp_candidate_count,
            builder.clamp_proof_backed_candidate_count,
        )
    }

    #[test]
    fn clamp_literal_bound_order_is_proof_backed_only_when_ordered() {
        assert_eq!(literal_bound_function(ClampShape::MinMax, 1, 10), (1, 1));
        assert_eq!(literal_bound_function(ClampShape::MinMax, 10, 1), (1, 0));
    }

    #[test]
    fn clamp_guarded_bound_order_requires_exiting_inverse_guard() {
        let integer = Some(ParamSemantic::Integer);
        assert_eq!(
            guarded_function(GuardShape::Exiting, ClampShape::MinMax, [integer; 3]),
            (1, 1)
        );
        assert_eq!(
            guarded_function(GuardShape::NonExiting, ClampShape::MinMax, [integer; 3]),
            (1, 0)
        );
        assert_eq!(
            guarded_function(GuardShape::None, ClampShape::MinMax, [integer; 3]),
            (1, 0)
        );
    }

    #[test]
    fn clamp_proof_rejects_floatish_number_and_wrong_shapes() {
        let integer = Some(ParamSemantic::Integer);
        let number = Some(ParamSemantic::Number);
        assert_eq!(
            guarded_function(GuardShape::Exiting, ClampShape::MinMax, [number; 3]),
            (1, 0),
            "float-sensitive Number params need a separate NaN/domain proof"
        );
        assert_eq!(
            guarded_function(GuardShape::Exiting, ClampShape::SwappedBounds, [integer; 3]),
            (1, 0)
        );
        assert_eq!(
            guarded_function(GuardShape::Exiting, ClampShape::WrongNesting, [integer; 3]),
            (1, 0)
        );
    }
}
