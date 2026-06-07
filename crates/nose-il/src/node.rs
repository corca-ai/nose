//! The IL node model. The IL is a small, desugared core language: every
//! frontend lowers its surface syntax into these node kinds, and the
//! normalization passes rewrite within them. Keeping the set small is what lets
//! semantically-equivalent code from different languages converge to the same
//! shape.

use crate::intern::Symbol;
use crate::span::Span;
use serde::{Deserialize, Serialize};

/// Index into [`crate::Il::nodes`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// The desugared core node kinds. See [`Payload`] for the per-kind data.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum NodeKind {
    // ----- structural -----
    /// A whole file. Children: top-level statements / declarations.
    Module,
    /// A function or method. Children: params..., then one body `Block`.
    Func,
    /// A single parameter. Payload carries its canonical id once renamed.
    Param,
    /// A statement sequence. Children: the statements.
    Block,

    // ----- statements -----
    /// `lhs = rhs`. Children: `[lhs, rhs]`.
    Assign,
    /// An expression used as a statement. Children: `[expr]`.
    ExprStmt,
    /// `return e?`. Children: `[]` or `[expr]`.
    Return,
    /// `if cond { then } else { else }`. Children: `[cond, then]` or
    /// `[cond, then, else]`. `then`/`else` are `Block`s.
    If,
    /// A loop. Payload: [`LoopKind`], which fixes the child layout:
    /// - `While`:   `[cond, body]`
    /// - `CStyle`:  `[init, cond, update, body]`
    /// - `ForEach`: `[pattern, iterable, body]`
    ///
    /// Frontends emit the faithful kind; the desugar pass rewrites `CStyle` and
    /// `ForEach` into `While` so all loops converge to one shape.
    Loop,
    /// `break`. No children.
    Break,
    /// `continue`. No children.
    Continue,
    /// `throw e` / `raise e`. Children: `[expr]`.
    Throw,
    /// `try { body } catch { handler } finally { fin }`. Children:
    /// `[body, handler?, finally?]` (Blocks); presence flagged in payload.
    Try,

    // ----- expressions -----
    /// A variable reference. Payload: `Name` (raw) then `Cid` (after rename).
    Var,
    /// A literal. Payload: `Lit`.
    Lit,
    /// `callee(args...)`. Children: `[callee, args...]`. When the callee resolves
    /// to a known cross-language builtin, payload carries `Builtin` and the
    /// callee child is dropped.
    Call,
    /// A binary operation. Payload: `Op`. Children: `[lhs, rhs]` (or sorted
    /// operands for commutative ops after normalization).
    BinOp,
    /// A unary operation. Payload: `Op`. Children: `[operand]`.
    UnOp,
    /// `base[index]`. Children: `[base, index]`.
    Index,
    /// `base.field`. Payload: `Field` name. Children: `[base]`.
    Field,
    /// An anonymous function / lambda. Children: params..., then body.
    Lambda,
    /// An array/tuple/set/map literal. Children: the elements.
    Seq,
    /// A canonical higher-order operation (comprehension / map / flat-map /
    /// filter / reduce). Payload: `HoF`. Children: `[collection, fn]`.
    HoF,

    /// Escape hatch for surface constructs a frontend does not yet lower. Payload
    /// `Name` holds the original tree-sitter node kind for debugging. Children:
    /// whatever sub-IL was produced. Detection treats these opaquely.
    Raw,
}

/// Per-node data. Most nodes carry [`Payload::None`]; the variant in use is
/// determined by the node's [`NodeKind`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Payload {
    None,
    /// Operator for `BinOp` / `UnOp`.
    Op(Op),
    /// Literal class for `Lit` (value abstracted away).
    Lit(LitClass),
    /// Retained small-integer literal value (optional, flag-controlled).
    LitInt(i64),
    /// Retained boolean-literal value. `true` and `false` are behavior-defining
    /// constants (a predicate `return x>0` ≡ `if x>0: return True else False`, and its
    /// negation returns the swapped booleans), so — like `0`≠`1` — they must stay
    /// distinct rather than collapse to the abstract `Bool` class.
    LitBool(bool),
    /// Retained string-literal content hash. Distinguishes behavior-defining string
    /// constants (`"OPTIONS"` vs `"HEAD"`, locale messages) in the value-graph while
    /// the structural tag stays the abstract `Str` class.
    LitStr(u64),
    /// Retained float (or other non-`i64`-decimal numeric) literal — a hash of its source
    /// text, so `3.14` ≠ `2.71` in the value graph. Previously all floats collapsed to one
    /// `Lit(Float)` token, a latent false merge (float-only-differing functions shared a
    /// fingerprint, and the interpreter has no float so the oracle couldn't catch it). The
    /// structural tag stays the abstract `Float` class (shape similarity unaffected).
    LitFloat(u64),
    /// Raw identifier name, before alpha-renaming. Also used by `Field` and by
    /// `Raw` (to stash the original surface kind).
    Name(Symbol),
    /// Canonical identifier id assigned by the alpha-renaming pass.
    Cid(u32),
    /// Canonicalized cross-language builtin op for `Call`.
    Builtin(Builtin),
    /// Higher-order op kind for `HoF`.
    HoF(HoFKind),
    /// Loop flavor for `Loop`.
    Loop(LoopKind),
}

/// Coarse semantic facts about a function parameter recovered from explicit source
/// annotations. These are proof gates only: `Unknown` is represented by absence.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ParamSemantic {
    Array,
    ByteArray,
    Collection,
    Integer,
    Map,
    Number,
    Option,
    Set,
    String,
}

/// Kernel-facing receiver/domain evidence recovered from source annotations,
/// inference, or semantic packs. Unknown is represented by absence, and
/// consumers must fail closed when evidence is missing or conflicting.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum DomainEvidence {
    Array,
    ByteArray,
    Collection,
    Integer,
    Map,
    Number,
    Option,
    Set,
    String,
}

impl DomainEvidence {
    pub fn from_param_semantic(semantic: ParamSemantic) -> Self {
        match semantic {
            ParamSemantic::Array => DomainEvidence::Array,
            ParamSemantic::ByteArray => DomainEvidence::ByteArray,
            ParamSemantic::Collection => DomainEvidence::Collection,
            ParamSemantic::Integer => DomainEvidence::Integer,
            ParamSemantic::Map => DomainEvidence::Map,
            ParamSemantic::Number => DomainEvidence::Number,
            ParamSemantic::Option => DomainEvidence::Option,
            ParamSemantic::Set => DomainEvidence::Set,
            ParamSemantic::String => DomainEvidence::String,
        }
    }

    pub fn is_array(self) -> bool {
        self == DomainEvidence::Array
    }

    pub fn is_byte_array(self) -> bool {
        self == DomainEvidence::ByteArray
    }

    pub fn is_collection_or_set(self) -> bool {
        matches!(self, DomainEvidence::Collection | DomainEvidence::Set)
    }

    pub fn is_array_or_collection(self) -> bool {
        matches!(self, DomainEvidence::Array | DomainEvidence::Collection)
    }

    pub fn is_array_collection_or_set(self) -> bool {
        matches!(
            self,
            DomainEvidence::Array | DomainEvidence::Collection | DomainEvidence::Set
        )
    }

    pub fn is_set(self) -> bool {
        self == DomainEvidence::Set
    }

    pub fn is_map(self) -> bool {
        self == DomainEvidence::Map
    }

    pub fn is_option(self) -> bool {
        self == DomainEvidence::Option
    }

    pub fn is_string(self) -> bool {
        self == DomainEvidence::String
    }

    pub fn is_integer(self) -> bool {
        self == DomainEvidence::Integer
    }

    pub fn is_integer_or_number(self) -> bool {
        matches!(self, DomainEvidence::Integer | DomainEvidence::Number)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ParamTypeFact {
    pub span: Span,
    pub semantic: ParamSemantic,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceId(pub u32);

/// Stable subject addressed by a semantic evidence record. Node ids are not used
/// because normalization rebuilds arenas; consumers match by source span plus the
/// expected subject kind and fail closed when that is ambiguous.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceAnchor {
    SourceSpan(Span),
    Node { span: Span, kind: NodeKind },
    Param { span: Span },
    Binding { span: Span, local_hash: u64 },
    Sequence { span: Span },
}

impl EvidenceAnchor {
    pub fn source_span(span: Span) -> Self {
        EvidenceAnchor::SourceSpan(span)
    }

    pub fn node(span: Span, kind: NodeKind) -> Self {
        EvidenceAnchor::Node { span, kind }
    }

    pub fn param(span: Span) -> Self {
        EvidenceAnchor::Param { span }
    }

    pub fn binding(span: Span, local_hash: u64) -> Self {
        EvidenceAnchor::Binding { span, local_hash }
    }

    pub fn sequence(span: Span) -> Self {
        EvidenceAnchor::Sequence { span }
    }

    pub fn matches_span(self, span: Span) -> bool {
        match self {
            EvidenceAnchor::SourceSpan(subject)
            | EvidenceAnchor::Node { span: subject, .. }
            | EvidenceAnchor::Param { span: subject }
            | EvidenceAnchor::Binding { span: subject, .. }
            | EvidenceAnchor::Sequence { span: subject } => subject == span,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceEmitter {
    FirstParty,
    External,
    Legacy,
}

/// Provenance attached to semantic evidence. Hashes are stable symbol hashes so
/// serialized IL does not depend on an interner instance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceProvenance {
    pub emitter: EvidenceEmitter,
    pub pack_hash: Option<u64>,
    pub rule_hash: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceStatus {
    Asserted,
    Ambiguous,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ImportEvidenceKind {
    Binding {
        module_hash: u64,
        exported_hash: u64,
    },
    Namespace {
        module_hash: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SequenceSurfaceKind {
    Untagged,
    Collection,
    Tuple,
    Map,
    Pair,
    ImportBinding,
    ImportNamespace,
    RecordGuard,
    OwnPropertyGuard,
    GoCompositeMapLiteral,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum EvidenceKind {
    Source(SourceFactKind),
    Domain(DomainEvidence),
    Import(ImportEvidenceKind),
    SequenceSurface(SequenceSurfaceKind),
}

/// Pack-facing semantic evidence record. It is evidence, not a verdict: exact
/// consumers must check contracts, provenance, dependencies, and ambiguity.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: EvidenceId,
    pub anchor: EvidenceAnchor,
    pub kind: EvidenceKind,
    pub provenance: EvidenceProvenance,
    pub dependencies: Vec<EvidenceId>,
    pub status: EvidenceStatus,
}

/// Source-origin facts that must survive lowering before a semantic contract can
/// consume them. These are evidence records, not semantic approval: contracts in
/// `nose-semantics` decide whether a fact is sufficient for an exact rule.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SourceFact {
    pub span: Span,
    pub kind: SourceFactKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceFactKind {
    Operator(SourceOperatorKind),
    Call(SourceCallKind),
    Literal(SourceLiteralKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceOperatorKind {
    StrictEquality,
    StrictInequality,
    LooseEquality,
    LooseInequality,
    ValueEquality,
    ValueInequality,
    IdentityEquality,
    IdentityInequality,
    TypeMembership,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceCallKind {
    Construct,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceLiteralKind {
    Regex,
}

/// Loop flavor; see [`NodeKind::Loop`] for the child layout each implies.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LoopKind {
    /// `while cond { body }` — also the canonical form after desugaring.
    While,
    /// C-style three-clause `for (init; cond; update) { body }`.
    CStyle,
    /// Iterator loop: `for pattern in iterable { body }`.
    ForEach,
}

/// Operators, normalized across languages (e.g. Python `and` and JS `&&` both
/// become [`Op::And`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Op {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    // comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Membership `a in b` — a is an element/key of collection b. NON-commutative
    /// and distinct from `Eq`: `a in b` ≠ `b in a` ≠ `a == b`.
    In,
    // logical
    And,
    Or,
    Not,
    // bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    BitNot,
    // unary arithmetic
    Neg,
    Pos,
}

impl Op {
    /// Commutative binary operators whose operands may be reordered to a canonical
    /// form during normalization.
    pub fn is_commutative(self) -> bool {
        matches!(
            self,
            Op::Add
                | Op::Mul
                | Op::Eq
                | Op::Ne
                | Op::And
                | Op::Or
                | Op::BitAnd
                | Op::BitOr
                | Op::BitXor
        )
    }
}

/// Literal value classes. The concrete value is abstracted to its class so that
/// two structurally identical computations over different constants match.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LitClass {
    Int,
    Float,
    Str,
    Bool,
    Null,
    Other,
}

/// Cross-language builtins collapsed to one canonical op (see the idiom pass).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Builtin {
    /// `len(x)` / `x.length` / `len(x)`.
    Len,
    /// `print` / `console.log` / `fmt.Println`.
    Print,
    /// `xs.append(x)` / `xs.push(x)` / `append(xs, x)`.
    Append,
    /// `range` / numeric range generators.
    Range,
    /// `sum(xs)` / `sum(gen)` — additive reduction over a collection.
    Sum,
    /// `reduce(f, xs, init)` / `functools.reduce` / `xs.reduce(f)` — explicit fold.
    Reduce,
    /// `min(xs)` / `min(gen)` — minimum reduction over a collection.
    Min,
    /// `max(xs)` / `max(gen)` — maximum reduction over a collection.
    Max,
    /// `abs(x)` — absolute value (converges with the `x if x>=0 else -x` idiom).
    Abs,
    /// `zip(a, b)` — pairwise iteration over two collections.
    Zip,
    /// `enumerate(xs)` — index+value iteration over a collection.
    Enumerate,
    /// Keys/indices of a collection — JS `for (x in obj)` iterates these (NOT the
    /// values, which is `for (x of obj)`). Wrapping the for-in iterable in `Keys`
    /// keeps the two loops behaviorally distinct.
    Keys,
    /// `any(p(x) for x in xs)` / `xs.some(p)` / `xs.iter().any(p)` — existential
    /// (short-circuit OR) reduction over a predicate-mapped collection.
    Any,
    /// `all(p(x) for x in xs)` / `xs.every(p)` / `xs.iter().all(p)` — universal
    /// (short-circuit AND) reduction over a predicate-mapped collection.
    All,
    /// A single dict key→value entry: a `pair` `k: v` in a dict literal / dict
    /// comprehension, and the per-element contribution of a `d[k] = v` dict-building
    /// loop. A DISTINCT op (not a `Seq`/tuple) so `{k: v for x in xs}` and the building
    /// loop converge with each other WITHOUT colliding with `[(k, v) for x in xs]`
    /// (a list of tuples — a `Seq`), which is behaviorally different.
    DictEntry,
    /// `x.is_empty()` / `x.isEmpty()` / `x.empty?` — a static collection
    /// emptiness predicate, converging with `Len(x) == 0`.
    IsEmpty,
    /// Case-sensitive string prefix predicate over `(value, prefix)`.
    StartsWith,
    /// Case-sensitive string suffix predicate over `(value, suffix)`.
    EndsWith,
    /// Static literal-collection membership predicate over `(element, collection)`.
    Contains,
    /// Static literal-map lookup with fallback over `(map, key, default)`.
    GetOrDefault,
    /// Nullish/option value defaulting over `(value, default)`.
    ValueOrDefault,
    /// Null/none/nil absence predicate over `(value)`.
    IsNull,
    /// Non-null/some presence predicate over `(value)`.
    IsNotNull,
    /// Ordered string join over `(separator, collection)`.
    Join,
    /// C unsigned 32-bit cast proof over one numeric value. This is emitted only when
    /// the C frontend sees an explicit unsigned 32-bit cast; it is not a general
    /// language-agnostic conversion.
    UnsignedCast32,
}

/// Kinds of canonical higher-order operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum HoFKind {
    Map,
    Filter,
    Reduce,
    FlatMap,
    /// Option/null-producing map: `Null` drops the item, `Err` propagates, every
    /// other value is emitted.
    FilterMap,
}

/// One IL node. Children are stored out-of-line in [`crate::Il::edges`] as a
/// contiguous slice `[child_start, child_start + child_len)`, keeping `Node`
/// small and the arena cache-friendly.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub payload: Payload,
    pub span: Span,
    pub child_start: u32,
    pub child_len: u32,
}
