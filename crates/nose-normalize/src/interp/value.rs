use super::*;

/// An IEEE-754 double that participates in `Value`'s `Eq`/`Hash` (which `f64` cannot, #342).
/// Two floats are equal here iff their CANONICAL bit patterns match — all NaNs collapse to one
/// (so a unit returning NaN equals another returning NaN, the behavior-comparison invariant),
/// and `-0.0` is normalized to `+0.0` (`-0.0 == +0.0` in every source language). This keeps
/// the oracle's "self-consistent, deterministic" contract while modeling float non-associativity.
#[derive(Clone, Copy, Debug)]
pub struct F64(pub f64);

impl F64 {
    fn canonical_bits(self) -> u64 {
        if self.0.is_nan() {
            0x7ff8_0000_0000_0000 // one canonical quiet NaN
        } else if self.0 == 0.0 {
            0 // +0.0 and -0.0 both normalize to +0.0
        } else {
            self.0.to_bits()
        }
    }
}

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_bits() == other.canonical_bits()
    }
}
impl Eq for F64 {}
impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical_bits().hash(state);
    }
}

/// A runtime value. `List` is nested so `zip`/`enumerate` can yield pairs.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Value {
    Int(i64),
    /// An IEEE-754 double (#342). Distinct from `Int` so float `+`/`*` non-associativity is
    /// observable: `(a+b)+c` and `a+(b+c)` compute different `Float`s on adversarial inputs.
    Float(F64),
    Bool(bool),
    /// A string/builder value modeled as the FREE MONOID over its appended pieces: an
    /// ordered sequence of opaque token hashes. A literal is one token; `+`/concat
    /// appends (associative, identity = empty), and is ORDER-SENSITIVE — so `s + x` and
    /// `x + s` differ, exactly as string concatenation does (this is what makes the
    /// builder/join family interpretable and exposes any unsound commutative treatment of
    /// `+` on strings). No real character content is needed — the ordered pieces capture
    /// append behavior. (Char-level ops like length/index stay `Err`: unknown from pieces.)
    Str(Vec<u64>),
    List(Vec<Value>),
    Null,
    /// A runtime error (type mismatch, out-of-range, divide-by-zero). This is itself
    /// observable behavior — two equivalent programs err on the same inputs.
    Err,
    /// A SYMBOLIC value: the result of an operation the interpreter cannot execute —
    /// an opaque (unproven/unadmitted) call, an unproven field read, or any
    /// composition over such a value — identified by a stable structural hash of the
    /// operation and its operand values. This is a *differential convention*, not a
    /// semantics claim: two runs produce the same `Sym` iff they performed the same
    /// opaque operation on equal operands, in the same observable order (opaque calls
    /// are also recorded in the effect trace). Control flow is never guessed:
    /// branching on a `Sym` (truthiness, loop bounds, iteration) still bails the
    /// unit. Because symbolic identity is keyed on pre-canonicalization syntax, a
    /// behavior containing a `Sym` must never feed the hard SOUND gate — the verify
    /// report routes Sym-bearing disagreements to a separate advisory lane.
    Sym(u64),
}

/// Stable hash of a runtime value (deterministic: `FxHasher` carries no random
/// state), used to compose symbolic identities.
pub(super) fn vhash(v: &Value) -> u64 {
    hashed(v)
}

/// Stable hash of any hashable tag (operator, builtin, contract).
pub(super) fn hashed<T: Hash>(t: &T) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    t.hash(&mut h);
    h.finish()
}

/// Replace a battery value that violates the parameter's DECLARED type domain
/// with a deterministic conforming one. Only domains the interpreter can host
/// concretely are coerced; everything else binds unchanged.
pub(super) fn coerce_to_declared_domain(v: Value, d: nose_il::DomainEvidence) -> Value {
    use nose_il::DomainEvidence as D;
    if d == D::Number {
        return match v {
            Value::Float(_) => v,
            Value::Int(value) => Value::Float(F64(value as f64)),
            other => Value::Float(F64((vhash(&other) % 47) as f64 / 2.0 - 11.5)),
        };
    }
    let conforms = match d {
        D::Integer => matches!(v, Value::Int(_)),
        D::Float => matches!(v, Value::Float(_)),
        D::Boolean => matches!(v, Value::Bool(_)),
        D::String => matches!(v, Value::Str(_)),
        D::Array | D::Collection | D::Iterable => matches!(v, Value::List(_)),
        _ => true,
    };
    if conforms {
        return v;
    }
    let h = vhash(&v);
    match d {
        D::Integer => Value::Int((h % 23) as i64 - 11),
        D::Float => Value::Float(F64((h % 23) as f64 - 11.0)),
        D::Boolean => Value::Bool(h & 1 == 1),
        D::String => Value::Str(vec![h]),
        D::Array | D::Collection | D::Iterable => Value::List(vec![
            Value::Int((h % 7) as i64),
            Value::Int((h % 5) as i64 - 2),
        ]),
        D::Number
        | D::ByteArray
        | D::FutureLike
        | D::Iterator
        | D::Map
        | D::Nominal { .. }
        | D::Option
        | D::PromiseLike
        | D::Record
        | D::Result
        | D::Set => v,
    }
}

/// Does this behavior carry any symbolic value? Sym-bearing behaviors are
/// comparable under the differential convention, but a disagreement involving one
/// must never feed the hard SOUND gate: symbolic identity is keyed on pre-canon
/// syntax, so a proof-backed canonicalization (e.g. AC ordering) can legitimately
/// make two equivalent units' symbolic traces differ.
pub fn behavior_has_sym(b: &Behavior) -> bool {
    contains_sym(&b.ret)
        || b.effects.iter().any(contains_sym)
        || b.fields.iter().any(|(_, v)| contains_sym(v))
}

/// Oracle behavioral equivalence: like `==`, except two ABORTING runs (both `ret ==
/// Value::Err`) are equal regardless of their `effects`/`fields`. An erroring execution
/// has no observable result — the input is outside the unit's domain — and the side
/// effects recorded before a trap are not committed behavior, so a canonicalization that
/// reorders operations ahead of a guaranteed trap (leaving one IL with a partial effect
/// trace the other lacks, but BOTH still trapping) has not changed what the unit computes.
/// A real behavior change — `Ok→Err`, `Err→Ok`, or a differing successful result — is
/// still unequal: the `ret`s differ, or both are non-`Err` and the full behaviors compare.
/// Used by the canon-preservation gate, where impossible inputs (e.g. an int bound to an
/// array parameter) would otherwise manufacture spurious violations.
pub fn behavior_equiv(a: &Behavior, b: &Behavior) -> bool {
    if a.ret == Value::Err && b.ret == Value::Err {
        return true;
    }
    a == b
}

/// Does the value contain a `Sym` anywhere (including inside lists)? Concrete
/// operations must never run over a hidden symbolic operand — `sum([f(x)])`
/// collapsing to a concrete `Err` would launder unknownness into the hard
/// soundness lane. Every composition guard uses this DEEP check.
pub(super) fn contains_sym(v: &Value) -> bool {
    match v {
        Value::Sym(_) => true,
        Value::List(xs) => xs.iter().any(contains_sym),
        _ => false,
    }
}

/// Normalize positive zero at a JavaScript/TypeScript function boundary. The compact literal
/// form already represents `+0` as `Int(0)`; doing the same for external inputs keeps identity
/// and double-negation observations aligned. Negative zero remains a Float because source
/// runtimes can distinguish it and the falsifier deliberately exercises that boundary.
pub(super) fn compact_javascript_positive_zero(value: Value) -> Value {
    match value {
        Value::Float(F64(value)) if value == 0.0 && !value.is_sign_negative() => Value::Int(0),
        Value::List(values) => Value::List(
            values
                .into_iter()
                .map(compact_javascript_positive_zero)
                .collect(),
        ),
        other => other,
    }
}

/// A receiver identity proven by the IL shape during interpretation.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum FieldPlace {
    SelfReceiver,
}

/// A concrete final field-state slot: receiver identity plus field name.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct FieldKey {
    pub receiver: FieldPlace,
    pub field: u64,
}

/// The observable behavior of one run: the returned value, an ordered I/O effect trace
/// (appended/printed values, in order — order IS observable), and the final per-place
/// object state (`this.x = ...`) as a receiver+name→value map in canonical place order.
/// Field state is order-INSENSITIVE across distinct places but reflects
/// last-write-wins per receiver+field. Two units are behaviorally equal on an input iff
/// all three components match.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Behavior {
    pub ret: Value,
    pub effects: Vec<Value>,
    pub fields: Vec<(FieldKey, Value)>,
}
