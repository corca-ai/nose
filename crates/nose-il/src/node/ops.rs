use serde::{Deserialize, Serialize};

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
    /// Floor division (quotient rounded toward −∞): Python `//`. DISTINCT from
    /// [`Op::Div`], whose integer model truncates toward zero — the two differ on
    /// any negative operand (`-5 // 2 == -3` vs `-5 / 2 == -2`), so conflating
    /// them is a false merge.
    FloorDiv,
    /// Floored modulo (remainder takes the sign of the DIVISOR): Python/Ruby `%`.
    /// DISTINCT from [`Op::Mod`], the C-family truncated remainder (sign of the
    /// DIVIDEND) used by JS/Go/Java/Rust/C — they differ on any sign-disagreeing
    /// operands (`-1 % 3 == 2` floored vs `== -1` truncated), so conflating them
    /// (as a single `Op::Mod` for all languages did) is a false merge the verify
    /// interpreter was blind to (#283-D).
    FloorMod,
    /// True (real) division — the quotient is a float, NOT truncated: Python 3 / JS
    /// `/` (`7 / 2 == 3.5`). DISTINCT from [`Op::Div`] (C/Go/Java/Rust truncated-int,
    /// `7 / 2 == 3`) and [`Op::FloorDiv`] (Ruby `/` and Python `//`, floored-int) —
    /// the three disagree (`7/2` is `3.5`, `3`, `3`; `-7/2` is `-3.5`, `-3`, `-4`), so
    /// one `Op::Div` for all of them is a false merge (#283-D). `Value::Float` now models float
    /// arithmetic (#342), but an Int÷Int `TrueDiv` is not promoted to it in the interpreter — it
    /// stays i64-truncated (consistent within the op, no cross-language merge); the Int→Float
    /// division promotion is the remaining Int↔Float breadth.
    TrueDiv,
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
    /// Case-sensitive string substring predicate over `(value, substring)`.
    StringContains,
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
