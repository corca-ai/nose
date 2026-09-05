use crate::intern::Symbol;
use crate::span::Span;
use serde::{Deserialize, Serialize};

use super::{Builtin, HoFKind, LitClass, LoopKind, Op};

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

    /// A keyword/named call argument `name=value`. Payload: `Name` (the keyword,
    /// content-hashed, never alpha-renamed — it labels a parameter, not a binding).
    /// Children: `[value]`. Appears only as a child of `Call`. Keyword arguments are
    /// order-independent BY NAME, so the value graph keys a call's keyword args by their
    /// names (not positions): `f(a=p, b=q)` ≡ `f(b=q, a=p)` but ≠ `f(a=q, b=p)`. An
    /// unhandled consumer treats it opaquely (fail closed: keyword calls then simply do
    /// not converge with positional ones). Declared LAST so adding it does not shift the
    /// discriminants (and thus the shape hashes) of the existing kinds.
    KwArg,

    /// An unpacked argument: `*iterable` (positional spread) or `**mapping` (keyword
    /// spread). Payload: `Name` (`"*"` or `"**"`) to distinguish the two. Children:
    /// `[inner]`. Without this, the frontend stripped `*expr`/`**expr` to the bare
    /// `expr`, so `f(*args)` lowered identically to `f(args)` — a false merge, since a
    /// spread changes the calling convention (`stats(*[[1,2,3]])` ≠ `stats([[1,2,3]])`).
    /// Carrying it keeps the call distinct; the inline binding plan and the oracle fail
    /// closed on a spread (the arity is dynamic). Declared LAST, like `KwArg`, so the
    /// discriminants stay stable.
    Splat,

    // ----- declarative (CSS / HTML) -----
    // These model declarative markup/style, which has no imperative behavior: a CSS
    // rule's meaning is its *computed style*, an HTML element's is its *rendered DOM*.
    // They are NOT lowered through the imperative value graph (GVN) — the exact
    // `semantic` fingerprint for a declarative unit is computed by a domain-specific
    // canonicalizer (see `nose-normalize::css_canon`) and dispatched in
    // `value_graph::api` by the unit-root kind. The GVN treats any declarative node it
    // somehow reaches as `Opaque` (it never should — fingerprinting is dispatched away).
    // Declared LAST so adding them does not shift the discriminants (and thus the shape
    // hashes / feature cache) of every kind above.
    /// A CSS rule-set: a selector list plus a declaration block. The unit root for
    /// CSS clone families. Children: `[CssSelector..., CssDecl...]` (or nested rules
    /// for at-rules like `@media`).
    CssRule,
    /// A CSS selector (one entry of a rule's selector list). Payload: `Name` (the
    /// canonicalized selector text). No children.
    CssSelector,
    /// A CSS declaration `property: value`. Payload: `Name` (the lowercased property).
    /// Children: the value tokens (`Lit`/`Var`), already shorthand/color/unit
    /// canonicalized.
    CssDecl,

    /// An HTML element. Payload: `Name` (the lowercased tag). Children:
    /// `[HtmlAttr..., (child element / HtmlText)...]`. The unit root for markup clone
    /// families; matched by rendered-DOM equivalence (see `nose-normalize::html`).
    HtmlElement,
    /// An HTML attribute `name="value"`. Payload: `Name` (lowercased attribute name).
    /// Children: `[Lit(Name=value)]`, or none for a boolean attribute.
    HtmlAttr,
    /// HTML text content (collapsed whitespace). Payload: `Name` (the text).
    HtmlText,
    /// A markup CONTROL construct wrapping a templated subtree — a repeat (`v-for`,
    /// Svelte `{#each}`, JSX `.map`) or a conditional (`v-if`, `{#if}`, JSX `&&`/ternary).
    /// Payload: `Name` ("repeat"/"if") naming the kind. Children: the template element(s).
    /// Distinct from a plain element so the exact fingerprint never equates a loop with a
    /// single element (sound); the `near` channel abstracts it (node_tag) so the three
    /// dialects' control idioms converge structurally. Appended LAST to keep NodeKind
    /// discriminants — and therefore shape hashes and the on-disk cache — stable.
    HtmlControl,
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

/// One IL node. Children are stored out-of-line in [`crate::IlContents::edges`] as a
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
