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
//! CONVENTION: a meaning-preserving *canonicalization* (a value rewrite) needs Lean evidence.
//! Name it `canonicalize_*` and list it in an obligation's `[rust].symbols` — the formal
//! obligation gate (`scripts/check-formal-obligations.py`) ENFORCES that every `canonicalize_*`
//! fn is covered by some obligation (the name is the declaration; no separate marker needed). A
//! canon under another name must be registered in that script's REQUIRED_OBLIGATIONS, or it
//! skips the gate (the gap that let `.then`/`pure_inline` slip).
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

mod builders;
mod collections;
mod context;
mod control;
mod field_state;
mod inline;
mod ops;
mod output;
mod rules;
mod stdlib;

pub use context::ValueFingerprintContext;

use crate::combine;
use crate::module_facts::{
    assignment_name_in_scope, collect_all_node_symbols_in_scope,
    collect_module_mutations_in_scope_with_direct_definitions, local_scope_nodes,
    node_symbol_in_scope, shadowed_js_like_module_binding_nodes_for_symbol_in_scope,
    top_level_statements_for,
};
use field_state::FieldStateKey;
use nose_il::{
    stable_symbol_hash, Builtin, EffectEvidenceKind, HoFKind, Il, Interner, Lang, LoopKind, NodeId,
    NodeKind, Op, Payload, SourceCastKind, SourceComprehensionKind, SourceFactKind, Span, Symbol,
};
use nose_semantics::{
    admitted_builtin_semantics_at_call, asserted_unshadowed_global_symbol, binding_write_target,
    builder_append_call_args, builder_append_method_contract, builtin_tag, construct_syntax_proof,
    domain_evidence_for_param as semantic_domain_evidence_for_param,
    exact_non_overloadable_index_assignment_parts, exact_static_membership_predicate_operator,
    go_zero_map_default_kind, go_zero_map_entry_contract_for_node,
    go_zero_map_literal_contract_for_node, go_zero_map_lookup_contract, import_fact_evidence_rhs,
    imported_literal_producer_evidence_for_node, imported_namespace_symbol,
    library_api_contract_evidence_at_call_span, library_api_contract_evidence_for_call,
    library_api_contract_evidence_for_node, library_free_function_builtin_contract,
    library_free_name_collection_factory_contracts, library_free_name_map_factory_contracts,
    library_imported_collection_factory_contracts, library_imported_namespace_function_contract,
    library_iterator_identity_adapter_contract, library_java_collection_constructor_contract,
    library_java_collection_factory_contract_by_hash, library_java_map_entry_contract_by_hash,
    library_java_map_factory_contract_by_hash, library_js_like_map_constructor_contract,
    library_js_like_set_constructor_contract, library_map_get_contract_by_hash,
    library_map_key_view_contract_by_hash, library_map_key_view_wrapper_contract_by_hash,
    library_method_call_contract, library_property_builtin_contract,
    library_ruby_set_factory_contract_by_hash, library_rust_option_and_then_contract,
    library_rust_option_none_sentinel_contract, library_rust_option_some_constructor_contract,
    library_rust_vec_macro_factory_contract, library_rust_vec_new_factory_contract,
    library_scalar_integer_method_contract, library_static_index_membership_contract,
    map_builder_index_write_contract, nullish_global_contract, opaque_argument_escape_args,
    own_property_guard_for_node, receiver_mutation_call_receiver, record_shape_guard_for_node,
    reduction_builtin_contract, semantics, seq_surface_contract_for_node,
    source_comprehension_at_node, source_operator_at_node, BuiltinArgContract, CBytePackWidth,
    CardinalityPredicate, CardinalityThreshold, ComparisonLaw, DomainEvidence, DomainRequirement,
    GoZeroMapDefaultKind, ImportFactKind, ImportedNamespaceFunctionSemantic,
    IndexMembershipThreshold, IndexWriteReceiverContract, IteratorAdapterReceiverContract,
    JavaMapFactoryKind, LibraryApiCalleeContract, LibraryApiEvidenceStatus,
    LibraryApiSpanEvidenceQuery, LibraryCollectionFactoryResult, LibraryMapFactoryResult,
    MapKeyViewKind, MethodBuiltinArgs, MethodEffectReceiverContract, MethodReceiverContract,
    MethodSemanticContract, ReductionBuiltinContract, ScalarIntegerMethod, SeqSurfaceContract,
    StaticIndexMembershipKind, ValueDomain, ValueLaw, SEQ_VALUE_COLLECTION, SEQ_VALUE_MAP,
    SEQ_VALUE_OWN_PROPERTY_GUARD, SEQ_VALUE_PAIR, SEQ_VALUE_RECORD_GUARD, SEQ_VALUE_UNTAGGED,
};
use ops::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::sync::OnceLock;

const LARGE_AC_EXPR_OPERANDS: usize = 64;

/// A heavy sub-DAG anchor: a shared sub-computation's structural `hash`, its `weight` (sub-DAG
/// size), and the source line range (`line_start..=line_end`) of the IL subtree that produced it —
/// so a partial / sub-DAG clone can report WHERE the shared computation lives in each unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Anchor {
    pub hash: u64,
    pub weight: u32,
    pub line_start: u32,
    pub line_end: u32,
}

/// A unit's heavy sub-DAG anchors, sorted/deduped by hash.
pub type Anchors = Vec<Anchor>;

/// One value-graph build's fingerprints: `(value, literal, return)` hash multisets plus the
/// heavy sub-DAG [`Anchors`].
pub type FingerprintBundle = (Vec<u64>, Vec<u64>, Vec<u64>, Anchors);

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
pub fn value_anchors(il: &Il, root: NodeId, interner: &Interner) -> Anchors {
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
) -> FingerprintBundle {
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
) -> FingerprintBundle {
    let mut b = Builder::new(il, interner).with_context(context);
    b.build_unit_with_context(root, Some(context));
    let (v, l, r) = b.fingerprint_lits();
    let a = b.anchors(ANCHOR_MIN_WEIGHT);
    (v, l, r, a)
}

pub fn value_fingerprint_lits_with_context(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: &ValueFingerprintContext,
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut b = Builder::new(il, interner).with_context(context);
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
    Input(u32), // a parameter or free variable, keyed by canonical id
    Const(u32), // literal class
    Bin(u32),   // binary operator
    Un(u32),    // unary operator
    Field(u64), // field access, keyed by content hash of the name
    Index,      // base[index]
    Call(u32),  // 0 = opaque callee; otherwise builtin discriminant + 1
    Hof(u32),   // higher-order op kind
    Clamp,      // numeric clamp over proven integer bounds: args = [x, lo, hi]
    Seq(u64),   // aggregate literal, keyed by lowered sequence kind
    ImportNamespace {
        module_hash: u64,
    },
    ImportBinding {
        module_hash: u64,
        exported_hash: u64,
    },
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum BuilderKind {
    List,
    Map,
}

#[derive(Clone, Copy)]
struct BuilderCandidate {
    cid: u32,
    kind: BuilderKind,
}

#[derive(Clone)]
struct InlineFunction {
    params: Vec<u32>,
    body: NodeId,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HofAdmission {
    SourceComprehension,
    LibraryApi,
}

struct Builder<'a> {
    il: &'a Il,
    interner: &'a Interner,
    nodes: Vec<ValNode>,
    /// Structural hash per value node, kept in lockstep with `nodes`.
    vhash: Vec<u64>,
    /// Source span of the IL subtree that produced each value node (lockstep with `nodes`), so a
    /// sub-DAG anchor can report WHERE the shared computation lives. Stamped at creation from
    /// `cur_span` (the enclosing expression being evaluated).
    node_span: Vec<Option<Span>>,
    /// The source span of the expression currently being evaluated (set by `eval`), used to stamp
    /// `node_span` for every node `mk` creates while evaluating it.
    cur_span: Option<Span>,
    intern: FxHashMap<(ValOp, Vec<ValueId>), ValueId>,
    sinks: Vec<Sink>,
    opaque_ctr: u32,
    /// Evidence-proven exact field writes, keyed by receiver identity plus field name → its
    /// CURRENT value (last-write-wins). Today this is the Java `this.field` substrate backed
    /// by `Place(SelfField)` and `Effect(SelfFieldWrite)`. Raw dynamic attribute writes stay
    /// ordered effects because selector spelling alone is not place/effect proof.
    field_env: FxHashMap<FieldStateKey, ValueId>,
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
    /// Kernel value domain per value node (kept in lockstep with `nodes`). Powers
    /// domain-aware canonicalization: `+` commutes only when concat is not proven,
    /// and numeric/boolean laws fire only when their domain preconditions are proven.
    /// `Unknown` is the safe default for positive domain requirements.
    vty: Vec<ValueDomain>,
    /// Inferred parameter value domains by position, seeding each positional `Input`.
    param_ty: Vec<ValueDomain>,
    /// Kernel domain evidence keyed by the alpha-renamed cid currently in scope.
    param_domain: FxHashMap<u32, DomainEvidence>,
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
    /// Active aggregate-builder variables during a loop body:
    /// list builders (`r = []; for x: r.append(f(x))`) and map builders
    /// (`d = {}; for x: d[k] = v`). Candidate activation requires preserved
    /// collection/map surface evidence for the pre-loop seed.
    ///
    /// cid → `Some((contrib, guard))` for a single clean per-element append, or `None` once
    /// spoiled (a second append/write, multi-arg append, or other use). On loop exit a clean
    /// builder's value becomes `Hof(Map, [contrib])` — the same node the comprehension
    /// `[f(x) for x in xs]` / `.map`/`.collect` builds, so the two converge.
    building: FxHashMap<u32, Option<(ValueId, Option<ValueId>)>>,
    building_kind: FxHashMap<u32, BuilderKind>,
    /// Strictly captured module/global constants, keyed by their original symbol.
    /// Function units keep global references as `Name`, while module-level assignment
    /// targets are alpha-renamed to `Cid`; this map reconnects safe top-level literal
    /// data (`const table = {...}`) to free uses inside the function.
    global_env: FxHashMap<Symbol, ValueId>,
    /// Interprocedural inline registry, keyed by target unit root. Calls may consume an entry only
    /// through `CallTarget::DirectFunction` evidence at the exact call occurrence; the callee
    /// spelling is never a proof channel. Pure bodies have no user calls, so an inlined body never
    /// triggers further inlining — single-level, no cycles, no depth bound.
    inline_fns: FxHashMap<NodeId, InlineFunction>,
    /// Nodes under function/lambda scopes use local cid numbering. Their `Cid(0)` is not
    /// the module `cid_names[0]`, so module-symbol resolution fails closed there.
    local_scope_nodes: Cow<'a, [bool]>,
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
            node_span: Vec::new(),
            cur_span: None,
            intern: FxHashMap::default(),
            sinks: Vec::new(),
            opaque_ctr: 0,
            field_env: FxHashMap::default(),
            subtree_hash: None,
            shared_subtree_hashes: None,
            valued_subtree_hash: None,
            vty: Vec::new(),
            param_ty: Vec::new(),
            param_domain: FxHashMap::default(),
            path: Vec::new(),
            bound_order_facts: Vec::new(),
            effect_slot: 0,
            building: FxHashMap::default(),
            building_kind: FxHashMap::default(),
            global_env: FxHashMap::default(),
            inline_fns: FxHashMap::default(),
            local_scope_nodes: Cow::Owned(local_scope_nodes(il)),
            loop_recurrence: None,
            next_loop_key_base: 0,
            contracts: Vec::new(),
            clamp_candidate_count: 0,
            clamp_proof_backed_candidate_count: 0,
        }
    }

    fn vty(&self, v: ValueId) -> ValueDomain {
        self.vty
            .get(v as usize)
            .copied()
            .unwrap_or(ValueDomain::Unknown)
    }

    fn value_law_satisfied(&self, law: ValueLaw, values: &[ValueId]) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .value_law(law)
            .is_some_and(|contract| {
                contract
                    .requirement
                    .accepts(values.iter().map(|&v| self.vty(v)))
            })
    }

    fn add_values_not_concat(&self, law: ValueLaw, values: &[ValueId]) -> bool {
        self.value_law_satisfied(law, values)
    }

    /// Bottom-up kernel value domain of a fresh node from its op and operands.
    fn value_domain_of(&self, op: &ValOp, args: &[ValueId]) -> ValueDomain {
        let at = |i: usize| {
            args.get(i)
                .map(|&a| self.vty(a))
                .unwrap_or(ValueDomain::Unknown)
        };
        let operators = semantics(self.il.meta.lang).operators();
        match op {
            ValOp::Const(k) => const_value_domain(*k),
            ValOp::Input(k) => self
                .param_ty
                .get(*k as usize)
                .copied()
                .unwrap_or(ValueDomain::Unknown),
            ValOp::Bin(o) => {
                if *o == MIN_CODE || *o == MAX_CODE {
                    ValueDomain::Number
                } else if let Some(op) = op_from_code(*o) {
                    operators.binary_result_domain(op, at(0), at(1))
                } else {
                    ValueDomain::Unknown
                }
            }
            ValOp::Un(o) => {
                if *o == ABS_CODE {
                    ValueDomain::Number
                } else if let Some(op) = op_from_code(*o) {
                    operators.unary_result_domain(op)
                } else {
                    ValueDomain::Unknown
                }
            }
            ValOp::Seq(_) | ValOp::CollectionParam | ValOp::ArrayParam => ValueDomain::Sequence,
            ValOp::Clamp => ValueDomain::Number,
            ValOp::StringParam => ValueDomain::String,
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
                operators.builtin_result_domain(Builtin::Contains)
            }
            _ => ValueDomain::Unknown,
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

    fn admitted_builtin_call(&self, node: NodeId, builtin: Builtin) -> bool {
        admitted_builtin_semantics_at_call(self.il, node, builtin)
    }

    fn domain_evidence_for_param(&self, param: NodeId) -> Option<DomainEvidence> {
        semantic_domain_evidence_for_param(self.il, param)
    }

    fn seed_param_domains(&mut self, root: NodeId) {
        let scope = self.param_domain_scope(root).unwrap_or(root);
        for &k in self.il.children(scope) {
            if self.il.kind(k) != NodeKind::Param {
                continue;
            }
            if let (Payload::Cid(cid), Some(domain)) =
                (self.il.node(k).payload, self.domain_evidence_for_param(k))
            {
                self.param_domain.insert(cid, domain);
            }
        }
    }

    fn seed_param_value_domains(&mut self, root: NodeId) {
        self.param_ty = semantics(self.il.meta.lang)
            .operators()
            .infer_param_value_domains(self.il, root);
        self.overlay_param_value_domains(root);
    }

    fn overlay_param_value_domains(&mut self, root: NodeId) {
        let scope = self.param_domain_scope(root).unwrap_or(root);
        let mut pos = 0usize;
        for &k in self.il.children(scope) {
            if self.il.kind(k) != NodeKind::Param {
                continue;
            }
            if let Payload::Cid(cid) = self.il.node(k).payload {
                if let Some(value_domain) = self
                    .param_domain
                    .get(&cid)
                    .copied()
                    .and_then(ValueDomain::from_domain_evidence)
                {
                    if self.param_ty.len() <= pos {
                        self.param_ty.resize(pos + 1, ValueDomain::Unknown);
                    }
                    self.param_ty[pos] = value_domain;
                }
            }
            pos += 1;
        }
    }

    fn param_domain_scope(&self, root: NodeId) -> Option<NodeId> {
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

    fn domain_evidence_of_expr(&self, expr: NodeId) -> Option<DomainEvidence> {
        nose_semantics::domain_evidence_for_receiver(self.il, self.interner, expr)
    }

    fn is_collection_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::ArrayCollectionOrSet,
        )
    }

    fn is_set_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Set,
        )
    }

    fn is_map_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Map,
        )
    }

    fn is_integer_param_expr(&self, expr: NodeId) -> bool {
        nose_semantics::receiver_satisfies_domain(
            self.il,
            self.interner,
            expr,
            DomainRequirement::Integer,
        )
    }

    /// Whether `value` is a parameter (an `Input`) carrying the given proof-gate domain.
    /// `is_array` adds the `ArrayParam` op on top.
    fn is_param_value(&self, value: ValueId, domain: DomainEvidence) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::Input(cid)
            if self.param_domain.get(&cid) == Some(&domain))
    }

    fn is_array_param_value(&self, value: ValueId) -> bool {
        matches!(self.nodes[value as usize].op, ValOp::ArrayParam)
            || self.is_param_value(value, DomainEvidence::Array)
    }

    fn param_domain_value(&mut self, value: ValueId) -> ValueId {
        let ValOp::Input(cid) = self.nodes[value as usize].op else {
            return value;
        };
        match self.param_domain.get(&cid).copied() {
            Some(domain) if domain.is_array() => self.mk(ValOp::ArrayParam, vec![value]),
            Some(domain) if domain.is_collection_or_set() => {
                self.mk(ValOp::CollectionParam, vec![value])
            }
            Some(domain) if domain.is_string() => self.mk(ValOp::StringParam, vec![value]),
            _ => value,
        }
    }

    fn is_js_like_lang(&self) -> bool {
        semantics(self.il.meta.lang)
            .modules()
            .js_like_shadowed_module_bindings()
    }

    fn free_name_input_key(&self, name: &str) -> u32 {
        let sym = self.interner.intern(name);
        self.free_name_key(sym)
    }

    fn free_name_key(&self, sym: Symbol) -> u32 {
        0x8000_0000u32 | (self.interner.symbol_hash(sym) as u32)
    }

    fn is_free_name_value(&self, value: ValueId, name: &str) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Input(key) if key == self.free_name_input_key(name)
        )
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
            if args.len() == 2 && self.comparison_law_enabled(ComparisonLaw::DirectionCanon) {
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
                    && !self.add_values_not_concat(ValueLaw::AddCommutativity, &args);
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
            if o == Op::Not as u32
                && !args.is_empty()
                && self.comparison_law_enabled(ComparisonLaw::Negation)
            {
                if let ValOp::Bin(bo) = self.nodes[args[0] as usize].op {
                    if let Some(neg) = negate_cmp_code(self.il.meta.lang, bo) {
                        let cargs = self.nodes[args[0] as usize].args.clone();
                        return self.mk(ValOp::Bin(neg), cargs);
                    }
                }
            }
            if o == Op::Neg as u32 && !args.is_empty() {
                if let ValOp::Un(io) = self.nodes[args[0] as usize].op {
                    let inner = self.nodes[args[0] as usize].args[0];
                    if io == Op::Neg as u32
                        && self.value_law_satisfied(ValueLaw::NumericNegationInvolution, &[inner])
                    {
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
                let bitwise = o == Op::BitAnd as u32 || o == Op::BitOr as u32;
                let logical = o == Op::And as u32 || o == Op::Or as u32;
                if (bitwise
                    && self.value_law_satisfied(ValueLaw::NumericBitwiseIdempotence, &[args[0]]))
                    || (logical
                        && self.value_law_satisfied(ValueLaw::BooleanIdempotence, &[args[0]]))
                {
                    return args[0];
                }
            }
            // NOTE: arithmetic identity elimination (`x+0→x`, `x*1→x`) is deliberately NOT
            // done — it is unsound for non-numeric `x` (`"a"+0` Errs; `self*1` on a
            // non-number need not equal `self`), and value-domain inference is optimistic (it infers
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
                if self.value_law_satisfied(ValueLaw::BooleanCommutativity, &args) {
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
                if leaves.len() > 2
                    && self.value_law_satisfied(ValueLaw::BooleanAssociativity, &leaves)
                {
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
                    && !self.add_values_not_concat(ValueLaw::AddAssociativity, &args);
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
        rules::clamp::apply(self, id).unwrap_or(id)
    }

    /// Intern a value node by `(op, args)` (hash-consing), computing its structural hash
    /// and kernel value domain. The raw constructor used by `mk` after canonicalization
    /// does not itself canonicalize, so callers must pass already-canonical operands.
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
        let ty = self.value_domain_of(&op, &args);
        self.nodes.push(ValNode { op, args });
        self.vhash.push(h);
        self.vty.push(ty);
        self.node_span.push(self.cur_span);
        self.intern.insert(key, id);
        id
    }

    fn boolean_guarded_identity_phi(
        &mut self,
        cond: ValueId,
        then_v: ValueId,
        else_v: ValueId,
    ) -> Option<ValueId> {
        if self.vty(cond) != ValueDomain::Boolean || self.vty(then_v) != ValueDomain::Boolean {
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
        if self.add_values_not_concat(ValueLaw::AddAssociativity, &values) {
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
        if !self.comparison_law_enabled(ComparisonLaw::LatticeLeNeToLt) {
            return None;
        }
        self.lattice_pair_canon(a, b, Op::Le as u32, Op::Ne as u32, Op::Lt as u32)
    }

    fn comparison_law_enabled(&self, law: ComparisonLaw) -> bool {
        semantics(self.il.meta.lang)
            .operators()
            .comparison_law(law)
            .is_some()
    }

    /// `(x < y) ∧ (x ≤ y) → x < y`. Guard-clause lowering accumulates path conditions
    /// from earlier returns, so a comparator written as `if x<y return -1; if x>y return
    /// 1; return 0` otherwise leaves the second return guarded by `x≤y ∧ x<y` after
    /// comparison-direction canon. The non-strict half is implied by the strict half and
    /// can be absorbed only for source languages whose comparison operators are primitive
    /// rather than receiver-overloadable.
    fn lattice_strict_absorbs_nonstrict(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::LatticeStrictAbsorbsNonstrict) {
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
        if !self.comparison_law_enabled(ComparisonLaw::LatticeLtEqToLe) {
            return None;
        }
        self.lattice_pair_canon(a, b, Op::Lt as u32, Op::Eq as u32, Op::Le as u32)
    }

    /// Recognize the absolute-value idiom `x if x>=0 else -x` (and its mirror
    /// `-x if x<0 else x`) as `Un(ABS_CODE, [x])`, so it converges with `abs(x)`.
    fn abs_pattern(&mut self, cond: ValueId, then: ValueId, els: ValueId) -> Option<ValueId> {
        if !self.comparison_law_enabled(ComparisonLaw::AbsSignTernary) {
            return None;
        }
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
        if !self.comparison_law_enabled(ComparisonLaw::MinMaxTernary) {
            return None;
        }
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
        semantics(self.il.meta.lang)
            .stdlib()
            .java_primitive_integer_ops()
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
        let _contract = semantics(self.il.meta.lang)
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U16)?;
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
                && self.is_param_value(base, DomainEvidence::ByteArray)
            {
                let zero = self.int_const(0);
                let one = self.int_const(1);
                return Some(self.mk(ValOp::Call(C_U16_BE_BYTE_PACK_CODE), vec![base, zero, one]));
            }
        }
        None
    }

    fn c_u32_be_byte_pack_pattern(&mut self, operands: &[ValueId]) -> Option<ValueId> {
        let contract = semantics(self.il.meta.lang)
            .operators()
            .c_integer_byte_pack_contract(CBytePackWidth::U32)?;
        if operands.len() != 4 {
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
            if index == 0 {
                match contract.required_high_lane_cast {
                    Some(SourceFactKind::Cast(SourceCastKind::CUnsigned32)) if unsigned_cast => {}
                    Some(_) => return None,
                    None => {}
                }
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
        if !self.is_param_value(base, DomainEvidence::ByteArray) {
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

    fn int_const_eq(&self, value: ValueId, expected: i64) -> bool {
        self.int_const_value(value) == Some(expected)
    }

    fn int_const_value(&self, value: ValueId) -> Option<i64> {
        let ValOp::Const(key) = self.nodes[value as usize].op else {
            return None;
        };
        // `LitInt(v)` is keyed as `0x1000_0000 + v as u32`, so retained
        // negative integers sit below the positive range. Exclude only the
        // small `LitClass` discriminants; strings/floats/bools live in their
        // own higher ranges and must never count as integer-bound proofs.
        if !(0x0000_0006..=0x1FFF_FFFF).contains(&key) {
            return None;
        }
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
        let collection = self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items);
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
                if !matches!(collection.op, ValOp::Seq(SEQ_VALUE_COLLECTION))
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
        // Track the enclosing source expression so EVERY node created while evaluating it (the top
        // node AND the intermediate nodes a reduce/map unfolds via `mk`) is stamped with its span
        // at creation — those intermediates are exactly what a heavy sub-DAG anchor points at.
        let prev = self.cur_span;
        self.cur_span = Some(self.il.node(expr).span);
        let v = self.eval_inner(expr, env);
        self.cur_span = prev;
        v
    }

    fn eval_inner(&mut self, expr: NodeId, env: &FxHashMap<u32, ValueId>) -> ValueId {
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
                    let name = self.interner.resolve(s);
                    if self.is_rust_option_none_node(expr) {
                        return self.null_const();
                    }
                    if let Some(contract) = nullish_global_contract(self.il.meta.lang, name) {
                        if !contract.requires_unshadowed
                            || asserted_unshadowed_global_symbol(self.il, expr, contract.name)
                        {
                            return self.null_const();
                        }
                    }
                    self.mk(ValOp::Input(self.free_name_key(s)), vec![])
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
                    if semantics(self.il.meta.lang)
                        .operators()
                        .membership_operator(Op::In)
                        .is_none()
                    {
                        let collection = self.eval(kids[1], env);
                        let salt = self.source_salted_hash(expr, 0x494E_4F50);
                        return self.mk(ValOp::Opaque(salt), vec![element, collection]);
                    }
                    if let Some(map) = self.proven_map_key_view_expr(kids[1], env) {
                        return self.mk(ValOp::Bin(op), vec![element, map]);
                    }
                    let collection = self.eval_membership_collection(kids[1], env);
                    return self.mk(ValOp::Bin(op), vec![element, collection]);
                }
                if let Some(v) = self.eval_rust_option_some_pattern_comparison(op, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_static_filter_membership_comparison(op, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_len_zero_comparison(op, &kids, env) {
                    return v;
                }
                if let Payload::Op(op_kind) = node.payload {
                    if let Some(v) =
                        self.eval_static_index_membership_comparison(op_kind, &kids, env)
                    {
                        return v;
                    }
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
                    if self.add_values_not_concat(ValueLaw::AddAssociativity, &operands) {
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
                            || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands);
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
                        || self.add_values_not_concat(ValueLaw::AddAssociativity, &operands);
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
                        let property_contract = library_property_builtin_contract(
                            self.il.meta.lang,
                            self.interner.resolve(s),
                        );
                        if let Some(contract) = property_contract.filter(|contract| {
                            contract.result == Builtin::Len
                                && matches!(
                                    library_api_contract_evidence_for_node(
                                        self.il,
                                        self.interner,
                                        expr,
                                        contract.id,
                                        contract.callee,
                                        0,
                                    ),
                                    LibraryApiEvidenceStatus::Admitted
                                )
                        }) {
                            if let Some(len) = self.eval_len_value(a[0]) {
                                return len;
                            }
                            if self
                                .domain_evidence_of_expr(kids[0])
                                .is_some_and(DomainEvidence::is_array_or_collection)
                            {
                                return self.mk(ValOp::Call(builtin_tag(contract.result)), a);
                            }
                        }
                        let receiver = &self.nodes[a[0] as usize];
                        if let ValOp::ImportNamespace { module_hash } = receiver.op {
                            return self.mk(
                                ValOp::ImportBinding {
                                    module_hash,
                                    exported_hash: stable_symbol_hash(self.interner.resolve(s)),
                                },
                                vec![],
                            );
                        }
                    }
                }
                if a.len() == 1 {
                    let Some(key) = self.exact_field_state_key(expr) else {
                        return self.mk(ValOp::Field(name), a);
                    };
                    if let Some(&written) = self.field_env.get(&key) {
                        return written;
                    }
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
                // Promise continuation beta-reduction is exact only when a semantic pack can
                // prove the receiver is Promise-like; otherwise arbitrary `.then` methods stay
                // opaque.
                if let Some(v) = rules::promise_then::apply(self, expr, env) {
                    return v;
                }
                if let Payload::Builtin(builtin) = node.payload {
                    if !self.admitted_builtin_call(expr, builtin) {
                        return self.source_salted_opaque(expr, 0x4255_494C);
                    }
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
                        return self.mk_value_or_map_default(value, default);
                    }
                }
                if let Payload::Builtin(b) = node.payload {
                    if let Some(r) = self.eval_reduction_builtin(b, &kids, env) {
                        return r;
                    }
                }
                if matches!(node.payload, Payload::Builtin(Builtin::UnsignedCast32))
                    && !nose_semantics::source_fact_at_node(
                        self.il,
                        expr,
                        SourceFactKind::Cast(SourceCastKind::CUnsigned32),
                    )
                {
                    return self.source_salted_opaque(expr, 0x5543_3332);
                }
                if let Some(r) = self.eval_count_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_product_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_integer_method_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_rust_map_get_unwrap_or_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_rust_map_get_is_some_call(expr, &kids, env) {
                    return r;
                }
                if kids.len() == 1 && self.is_rust_vec_new_call(expr, kids[0]) {
                    return self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]);
                }
                if let Some(v) = self.eval_java_collection_constructor_expr(expr, &kids) {
                    return v;
                }
                if let Some(v) = self.eval_js_like_constructed_collection_or_map(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_java_map_factory_expr(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_iterator_identity_adapter(expr, &kids, env) {
                    return v;
                }
                if let Some(v) = self.eval_proven_free_minmax_call(expr, &kids, env) {
                    return v;
                }
                if let Some(r) = self.eval_proven_collection_membership_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_key_membership_call(expr, &kids, env) {
                    return r;
                }
                if let Some(r) = self.eval_proven_map_get_default_call(expr, &kids, env) {
                    return r;
                }
                if self.is_unproven_membership_like_call(expr, &kids) {
                    let salt = self.source_salted_hash(expr, 0x4D45_4D42_4552);
                    return self.mk(ValOp::Opaque(salt), vec![]);
                }
                // Interprocedural pure inline: `f(args)` to a pure file-local function ≡ its body
                // with `args` substituted — converges with the same logic written inline / with
                // a different extracted helper. Sound (β-reduction of an effect-free function).
                if let Some(v) = self.eval_inlined_call(expr, &kids, env) {
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
                    _ => return self.source_salted_opaque(expr, 0x484F_465F),
                };
                let Some(admission) = self.hof_value_admission(expr, kind) else {
                    return self.source_salted_opaque(expr, 0x484F_465F);
                };
                self.eval_hof_value(
                    expr,
                    kind,
                    env,
                    admission == HofAdmission::SourceComprehension,
                )
            }
            NodeKind::Seq => {
                if let Some(value) = self.import_fact_value(expr) {
                    return value;
                }
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
                let hash = self.valued_subtree_hash(expr);
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
}

#[cfg(test)]
mod tests;
