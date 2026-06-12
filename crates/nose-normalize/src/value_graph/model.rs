//! Private value-graph model types and builder state.

use super::inline::InlineCandidate;
use super::*;

pub(super) type ValueId = u32;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum ValOp {
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

pub(super) struct ValNode {
    pub(super) op: ValOp,
    pub(super) args: Vec<ValueId>,
}

#[derive(Clone, Copy)]
pub(super) enum SinkKind {
    Return = 0,
    Cond = 1,
    Effect = 2,
    Break = 3,
    Throw = 4,
}

#[derive(Clone, Copy)]
pub(super) struct Sink {
    pub(super) kind: SinkKind,
    pub(super) value: ValueId,
    pub(super) effect_ord: Option<u32>,
}

impl Sink {
    pub(super) fn new(kind: SinkKind, value: ValueId) -> Self {
        Self {
            kind,
            value,
            effect_ord: None,
        }
    }

    pub(super) fn ordered_effect(value: ValueId, effect_ord: u32) -> Self {
        Self {
            kind: SinkKind::Effect,
            value,
            effect_ord: Some(effect_ord),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum BuilderKind {
    List,
    Map,
}

#[derive(Clone, Copy)]
pub(super) struct BuilderCandidate {
    pub(super) cid: u32,
    pub(super) kind: BuilderKind,
}

#[derive(Clone)]
pub(super) struct InlineFunction {
    pub(super) params: Vec<u32>,
    pub(super) body: NodeId,
}

/// One in-flight inline body evaluation. `emit_return` routes the callee's returns here
/// (with their inline-relative path guard) instead of pushing unit `Return` sinks; any
/// construct the value-only inline cannot represent faithfully marks the frame poisoned,
/// and the whole inline falls back to the opaque-call path (fail-closed).
pub(super) struct InlineCaptureFrame {
    /// `path.len()` at inline entry — captured guards are the conjunction of the path
    /// suffix above this base (conditions internal to the callee body).
    pub(super) path_base: usize,
    /// `loop_depth` at inline entry. A `return` reached while inside a callee loop has
    /// first-match-wins iteration semantics that a single `Phi` fold cannot express.
    pub(super) loop_depth_base: u32,
    pub(super) poisoned: bool,
    /// Captured `(guard, value)` returns in source order; `None` guard = unconditional.
    pub(super) returns: Vec<(Option<ValueId>, ValueId)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HofAdmission {
    SourceComprehension,
    LibraryApi,
}

pub(super) struct Builder<'a> {
    pub(super) il: &'a Il,
    pub(super) interner: &'a Interner,
    pub(super) nodes: Vec<ValNode>,
    /// Structural hash per value node, kept in lockstep with `nodes`.
    pub(super) vhash: Vec<u64>,
    /// Source span of the IL subtree that produced each value node (lockstep with `nodes`), so a
    /// sub-DAG anchor can report WHERE the shared computation lives. Stamped at creation from
    /// `cur_span` (the enclosing expression being evaluated).
    pub(super) node_span: Vec<Option<Span>>,
    /// The source span of the expression currently being evaluated (set by `eval`), used to stamp
    /// `node_span` for every node `mk` creates while evaluating it.
    pub(super) cur_span: Option<Span>,
    pub(super) intern: FxHashMap<(ValOp, Vec<ValueId>), ValueId>,
    pub(super) sinks: Vec<Sink>,
    pub(super) opaque_ctr: u32,
    /// Evidence-proven exact field writes, keyed by receiver identity plus field name -> its
    /// CURRENT value (last-write-wins). Today this is the Java `this.field` substrate backed
    /// by `Place(SelfField)` and `Effect(SelfFieldWrite)`. Raw dynamic attribute writes stay
    /// ordered effects because selector spelling alone is not place/effect proof.
    pub(super) field_env: FxHashMap<FieldStateKey, ValueId>,
    /// Lazily-computed subtree hash per IL node (kind + payload + children), used to
    /// key unlowered `Raw` constructs and lambda bodies by content. Computed once per
    /// graph (the whole-IL pass is O(n)); `None` until first needed.
    pub(super) subtree_hash: Option<Vec<u64>>,
    /// Shared file-level subtree hashes supplied by [`ValueFingerprintContext`]. This
    /// keeps contextual per-unit builders from recomputing the same whole-file pass.
    pub(super) shared_subtree_hashes: Option<&'a OnceLock<Vec<u64>>>,
    /// Literal-sensitive subtree hash used only for proven function binding identity.
    /// The ordinary structural hash intentionally abstracts literals for shape work;
    /// callee identity must distinguish `helper(x)+1` from `helper(x)+2`.
    pub(super) valued_subtree_hash: Option<Vec<u64>>,
    /// Kernel value domain per value node (kept in lockstep with `nodes`). Powers
    /// domain-aware canonicalization: `+` commutes only when concat is not proven,
    /// and numeric/boolean laws fire only when their domain preconditions are proven.
    /// `Unknown` is the safe default for positive domain requirements.
    pub(super) vty: Vec<ValueDomain>,
    /// Inferred parameter value domains by position, seeding each positional `Input`.
    pub(super) param_ty: Vec<ValueDomain>,
    /// Kernel domain evidence keyed by the alpha-renamed cid currently in scope.
    pub(super) param_domain: FxHashMap<u32, DomainEvidence>,
    /// The branch conditions currently in effect (each a `cond` or `Not(cond)`). A
    /// `return`/`throw` reached under a non-empty path is tagged with that condition,
    /// so `if c {return A} else {return B}` and the branch-swapped `if c {return B}
    /// else {return A}` produce *different* fingerprints (path-sensitive returns).
    pub(super) path: Vec<ValueId>,
    /// Active facts of the form `lo <= hi` established by dominating guard clauses.
    /// These are scoped like `path`: a fact learned from `if hi < lo { throw ... }`
    /// applies only to the fallthrough suffix of that block, and is truncated when the
    /// block returns to its caller. Literal integer bounds are proved on demand instead.
    pub(super) bound_order_facts: Vec<(ValueId, ValueId)>,
    /// Ordered statement-effect slot for the current control-flow path. Alternative
    /// `if` branches start from the same slot, then join at the max consumed slot, so
    /// branch-source order does not matter while sequential effects still do.
    pub(super) effect_slot: u32,
    /// Active aggregate-builder variables during a loop body:
    /// list builders (`r = []; for x: r.append(f(x))`) and map builders
    /// (`d = {}; for x: d[k] = v`). Candidate activation requires preserved
    /// collection/map surface evidence for the pre-loop seed.
    ///
    /// cid -> `Some((contrib, guard))` for a single clean per-element append, or `None` once
    /// spoiled (a second append/write, multi-arg append, or other use). On loop exit a clean
    /// builder's value becomes `Hof(Map, [contrib])` -- the same node the comprehension
    /// `[f(x) for x in xs]` / `.map`/`.collect` builds, so the two converge.
    pub(super) building: FxHashMap<u32, Option<(ValueId, Option<ValueId>)>>,
    pub(super) building_kind: FxHashMap<u32, BuilderKind>,
    /// Strictly captured module/global constants, keyed by their original symbol.
    /// Function units keep global references as `Name`, while module-level assignment
    /// targets are alpha-renamed to `Cid`; this map reconnects safe top-level literal
    /// data (`const table = {...}`) to free uses inside the function.
    pub(super) global_env: FxHashMap<Symbol, ValueId>,
    /// The unit's interprocedural inline registry: pure, file-local
    /// functions/methods (shared per file via [`ValueFingerprintContext`], or
    /// owned on the context-free path). Calls may consume an entry only through
    /// `CallTarget::DirectFunction` evidence at the exact call occurrence; the
    /// callee spelling is never a proof channel.
    pub(super) inline_candidates: Option<Cow<'a, [InlineCandidate]>>,
    /// The unit root currently being fingerprinted — inline targets whose
    /// subtree contains it are excluded (a function must not inline into
    /// itself through one of its sub-unit roots).
    pub(super) inline_exclude_root: Option<NodeId>,
    /// Snapshot of `global_env`'s keys taken when the candidates were adopted
    /// (post-seed, pre-process), so inline admission cannot drift with module
    /// statements processed later in the same unit.
    pub(super) inline_env_keys: FxHashSet<Symbol>,
    /// Roots of inline bodies currently being evaluated — the cycle/depth guard for
    /// nested pure inlining (`a` calling `b` calling `a` fails closed to opaque).
    pub(super) inline_stack: Vec<NodeId>,
    /// Active inline return-capture frames, innermost last (see [`InlineCaptureFrame`]).
    pub(super) inline_capture: Vec<InlineCaptureFrame>,
    /// Loop-statement processing depth; lets an inline capture detect returns that
    /// execute inside a callee loop (not representable as a value, so poison).
    pub(super) loop_depth: u32,
    /// Nodes under function/lambda scopes use local cid numbering. Their `Cid(0)` is not
    /// the module `cid_names[0]`, so module-symbol resolution fails closed there.
    pub(super) local_scope_nodes: Cow<'a, [bool]>,
    /// Current loop-carried placeholders while evaluating a loop body. Used only to
    /// compact coupled recurrences such as `s1 += f(s2); s2 += g(s1)`, which otherwise
    /// expand into a large raw expression DAG even though they are not clean reductions.
    pub(super) loop_recurrence: Option<LoopRecurrenceScope>,
    pub(super) next_loop_key_base: u32,
    /// Pointer-length contracts the unit RELIED ON to converge: `(array_param_pos,
    /// length_param_pos)` pairs recorded wherever `full_pointer_length_contract` fired (the
    /// loop bound `n` was treated as `len(array)`, not data, and dropped from the
    /// fingerprint). The behavioral oracle must interpret such a unit under the SAME contract
    /// -- binding `n = len(array)` -- else it tests the function on inputs the contract forbids
    /// (`n != len`) and reports a spurious false merge. Gated this way so the binding only
    /// fires where the value graph actually used the contract (it cannot mask a non-contract
    /// false merge). Sorted+deduped on read for determinism.
    pub(super) contracts: Vec<(u32, u32)>,
    /// Pack-facing value laws that actually rewrote or bridged this unit's value graph.
    pub(super) value_laws: Vec<ValueLaw>,
    /// Internal test counters for clamp canonicalization. They record clamp-shaped min/max
    /// nodes seen by `mk`, and the subset with a unique integer-domain `lo <= hi` proof.
    pub(super) clamp_candidate_count: usize,
    pub(super) clamp_proof_backed_candidate_count: usize,
}

#[derive(Clone)]
pub(super) struct LoopRecurrenceScope {
    pub(super) loop_values: FxHashMap<u32, ValueId>,
    pub(super) loop_keys: FxHashMap<u32, u32>,
    pub(super) loop_key_set: FxHashSet<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct SignedExprOperand {
    pub(super) expr: NodeId,
    pub(super) negated: bool,
}

#[derive(Default)]
pub(super) struct ReductionCache {
    pub(super) reductions: FxHashMap<(ValueId, ValueId), Option<(u32, ValueId)>>,
    pub(super) references: FxHashMap<(ValueId, ValueId), bool>,
}

#[derive(Clone, Copy)]
pub(super) enum FilterMapResult {
    Emit {
        value: ValueId,
        predicate: Option<ValueId>,
    },
    Drop,
}
