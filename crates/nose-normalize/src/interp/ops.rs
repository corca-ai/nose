use super::{contains_sym, hashed, sym_id, vhash, Unsupported, Value, F64, R, STEP_BUDGET};
use nose_il::{Op, Payload};

pub(super) fn op_of(p: Payload) -> Op {
    match p {
        Payload::Op(o) => o,
        _ => Op::Add,
    }
}

/// Concrete truthiness; `None` for a symbolic value. Control flow is never guessed
/// from a `Sym` — every branching caller must bail (`Unsupported`) on `None`.
pub(super) fn truthy(v: &Value) -> Option<bool> {
    Some(match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => f.0 != 0.0, // 0.0/-0.0 falsy; nonzero incl. NaN truthy (self-consistent)
        Value::List(xs) => !xs.is_empty(),
        Value::Str(v) => !v.is_empty(),
        Value::Null | Value::Err => false,
        Value::Sym(_) => return None,
    })
}

pub(super) fn fold_ints(v: Option<&Value>, init: i64, f: impl Fn(i64, i64) -> i64) -> Value {
    match v {
        Some(Value::List(xs)) => {
            let mut acc = init;
            for x in xs {
                match x {
                    Value::Int(i) => acc = f(acc, *i),
                    _ => return Value::Err,
                }
            }
            Value::Int(acc)
        }
        _ => Value::Err,
    }
}

/// `min`/`max` over either canonical form: a single collection (`min(xs)`) or two
/// scalars (`min(a, b)` — the 2-way selection that `[a, b].min()` and the
/// `a if a < b else b` ternary also canonicalize to). Without the scalar form the
/// oracle was blind to exactly the convergences the value graph claims for it.
pub(super) fn min_max_value(args: &[Value], f: impl Fn(i64, i64) -> i64) -> Value {
    match args {
        [Value::Int(a), Value::Int(b)] => Value::Int(f(*a, *b)),
        // The 2-arg form is the SCALAR selection; on non-Int operands it is a
        // type error. Falling through to the collection fold here ignored the
        // second argument (`max([1,2,3,4], 7)` returned 4), which made the
        // builtin chain disagree with the if-chain it soundly merges with (#210).
        [_, _] => Value::Err,
        [coll] => fold_opt(Some(coll), f),
        _ => Value::Err,
    }
}

fn fold_opt(v: Option<&Value>, f: impl Fn(i64, i64) -> i64) -> Value {
    match v {
        Some(Value::List(xs)) => {
            let mut acc: Option<i64> = None;
            for x in xs {
                match x {
                    Value::Int(i) => acc = Some(acc.map_or(*i, |a| f(a, *i))),
                    _ => return Value::Err,
                }
            }
            acc.map(Value::Int).unwrap_or(Value::Err)
        }
        _ => Value::Err,
    }
}

pub(super) fn string_affix(value: Option<&Value>, affix: Option<&Value>, prefix: bool) -> Value {
    match (value, affix) {
        (Some(Value::Str(value)), Some(Value::Str(affix))) => {
            if affix.len() > value.len() {
                return Value::Bool(false);
            }
            let matches = if prefix {
                value.starts_with(affix)
            } else {
                value.ends_with(affix)
            };
            Value::Bool(matches)
        }
        _ => Value::Err,
    }
}

pub(super) fn string_contains(value: Option<&Value>, needle: Option<&Value>) -> R<Value> {
    let (Some(Value::Str(value)), Some(Value::Str(needle))) = (value, needle) else {
        return Ok(Value::Err);
    };
    if needle.is_empty() {
        return Ok(Value::Bool(true));
    }
    if value.windows(needle.len()).any(|window| window == needle) {
        return Ok(Value::Bool(true));
    }
    // String chunks are opaque literal/builder pieces, not characters. A chunk
    // mismatch only proves "not the same piece sequence"; it does not prove
    // substring absence inside a literal or across piece boundaries.
    Err(Unsupported)
}

pub(super) fn join_strings(separator: Option<&Value>, collection: Option<&Value>) -> Value {
    let (Some(Value::Str(separator)), Some(Value::List(items))) = (separator, collection) else {
        return Value::Err;
    };
    let mut out = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let Value::Str(piece) = item else {
            return Value::Err;
        };
        if idx > 0 {
            out.extend(separator.iter().copied());
        }
        out.extend(piece.iter().copied());
    }
    Value::Str(out)
}

pub(super) fn range_values(args: &[Value]) -> R<Value> {
    let (start, stop, step) = match args {
        [Value::Int(stop)] => (0, *stop, 1),
        [Value::Int(start), Value::Int(stop)] => (*start, *stop, 1),
        [Value::Int(start), Value::Int(stop), Value::Int(step)] => (*start, *stop, *step),
        _ => return Ok(Value::Err),
    };
    if step == 0 {
        return Ok(Value::Err);
    }

    let mut out = Vec::new();
    let mut cur = start;
    while if step > 0 { cur < stop } else { cur > stop } {
        if out.len() as u64 > STEP_BUDGET {
            return Err(Unsupported);
        }
        out.push(Value::Int(cur));
        let Some(next) = cur.checked_add(step) else {
            return Err(Unsupported);
        };
        cur = next;
    }
    Ok(Value::List(out))
}

/// Coerce an integer `Value` to int32 (`x & 0xFFFF_FFFF`, sign-extended), the operand coercion
/// every JS-family bitwise operator applies (#344). Non-`Int` values pass through (a bitwise op
/// on them already `Err`s / stays symbolic in `bin`).
pub(super) fn to_int32(v: Value) -> Value {
    match v {
        Value::Int(i) => Value::Int(i as i32 as i64),
        other => other,
    }
}

pub(super) fn bin(op: Op, a: &Value, b: &Value) -> Value {
    use Value::{Bool, Int};
    // Symbolic composition: an operation over an unknown value is itself unknown,
    // keyed by (operator, operand identities). Never collapses to Bool/Err —
    // comparisons over `Sym` stay symbolic (`f(x) == f(x)` is NOT provably true for
    // an impure callee), and branching on the result bails at the truthiness gate.
    if contains_sym(a) || contains_sym(b) {
        return Value::Sym(sym_id(0x00B1_0911, &[hashed(&op), vhash(a), vhash(b)]));
    }
    // Err propagates from EITHER operand — an erroring sub-expression raises the whole
    // expression regardless of its position. Without this the fallthrough arm below read
    // `0 == Err` as `Bool(false)` (and `!=` as `Bool(true)`), so the SOUND `==`/`!=`
    // operand-ordering canon (algebra sorts commutative comparison operands by hash) looked
    // like a behavior change the moment it moved an erroring operand to the right — a
    // spurious canon-preservation violation on type-incoherent battery rows (coevo series 9
    // oracle finding). The `eval_bin_op` left short-circuit still fires first so laziness is
    // preserved; this is the symmetric twin of `un`'s `(Op::Not, Err) => Err`.
    if matches!(a, Value::Err) || matches!(b, Value::Err) {
        return Value::Err;
    }
    match (a, b) {
        (Int(x), Int(y)) => int_bin(op, *x, *y),
        // Float arithmetic (#342): any float operand promotes the other (an `Int` coerces to
        // f64 — the dynamic-language int/float rule, and self-consistent for the oracle), then
        // computes under IEEE-754. This is what makes `(a+b)+c` and `a+(b+c)` over floats
        // compute DIFFERENT values, so the oracle witnesses float non-associativity.
        (Value::Float(_), Value::Float(_))
        | (Value::Float(_), Int(_))
        | (Int(_), Value::Float(_)) => {
            let to_f = |v: &Value| match v {
                Value::Float(f) => f.0,
                Int(i) => *i as f64,
                _ => unreachable!("guarded by the match arm"),
            };
            float_bin(op, to_f(a), to_f(b))
        }
        (Bool(x), Bool(y)) => match op {
            Op::And => Bool(*x && *y),
            Op::Or => Bool(*x || *y),
            Op::Eq => Bool(x == y),
            Op::Ne => Bool(x != y),
            _ => Value::Err,
        },
        // String/builder concatenation — the free-monoid op: ordered append of pieces.
        // Order-sensitive (`s + x` ≠ `x + s`), the defining non-commutative behavior.
        (Value::Str(x), Value::Str(y)) if op == Op::Add => {
            let mut v = x.clone();
            v.extend_from_slice(y);
            Value::Str(v)
        }
        // Membership `a in b`: a is an element of list b (directional). Modeled for
        // lists so the value graph's `Op::In` is oracle-verifiable; other collections
        // (strings/dicts) aren't modeled → Err.
        (_, Value::List(items)) if op == Op::In => Bool(items.iter().any(|e| e == a)),
        // Equality across the same shape (lists, strings, null).
        _ => match op {
            Op::Eq => Bool(a == b),
            Op::Ne => Bool(a != b),
            _ => Value::Err,
        },
    }
}

/// Two's-complement / Python-`//`-`%` integer binary op. Extracted from `bin` so each numeric
/// kind's arithmetic is one focused function. (`x`/`y` are rebound to references so the body —
/// which predates the float kind — is unchanged.)
fn int_bin(op: Op, x: i64, y: i64) -> Value {
    use Value::{Bool, Int};
    let (x, y) = (&x, &y);
    match op {
        Op::Add => Int(x.wrapping_add(*y)),
        Op::Sub => Int(x.wrapping_sub(*y)),
        Op::Mul => Int(x.wrapping_mul(*y)),
        Op::Div => {
            if *y == 0 {
                Value::Err
            } else {
                Int(x.wrapping_div(*y))
            }
        }
        // True (float) division. `Value::Float` models float arithmetic now (#342), but an
        // Int÷Int `TrueDiv` is NOT promoted to it here — it stays truncated `Div` (BLIND to the
        // float result but CONSISTENT within `TrueDiv`, a distinct op that only compares with
        // itself, so no false merge). Promoting Int÷Int to a float result is the remaining
        // Int↔Float breadth (oracle-value-model §3.2). Div-by-zero still Errs.
        Op::TrueDiv => {
            if *y == 0 {
                Value::Err
            } else {
                Int(x.wrapping_div(*y))
            }
        }
        Op::Mod => {
            if *y == 0 {
                Value::Err
            } else {
                Int(x.wrapping_rem(*y))
            }
        }
        // Floored modulo (Python/Ruby `%`): the remainder takes the sign of the
        // DIVISOR. Truncated `wrapping_rem` takes the dividend's sign, so adjust
        // by adding the divisor when they disagree (#283-D). `-1 %% 3 == 2`.
        Op::FloorMod => {
            if *y == 0 {
                Value::Err
            } else {
                let r = x.wrapping_rem(*y);
                Int(if r != 0 && (r < 0) != (*y < 0) {
                    r.wrapping_add(*y)
                } else {
                    r
                })
            }
        }
        // Floor division rounds the quotient toward −∞ (Python `//`): the
        // truncating quotient is decremented when the remainder is nonzero
        // and the operands disagree in sign (`-5 // 2 == -3`, `5 // -2 == -3`).
        Op::FloorDiv => {
            if *y == 0 {
                Value::Err
            } else {
                let q = x.wrapping_div(*y);
                let r = x.wrapping_rem(*y);
                Int(if r != 0 && (r < 0) != (*y < 0) {
                    q - 1
                } else {
                    q
                })
            }
        }
        // An exponent that isn't a non-negative `u32` has no usable value here: a
        // negative one is fractional, and one past `u32::MAX` truncated under `as u32`
        // (so `b ** 2^32` collapsed onto `b ** 0 == 1`). Both err, like Div/Mod by zero,
        // rather than silently colliding distinct exponents.
        Op::Pow if !(0..=u32::MAX as i64).contains(y) => Value::Err,
        Op::Pow => Int(x.wrapping_pow(*y as u32)),
        Op::Eq => Bool(x == y),
        Op::Ne => Bool(x != y),
        Op::Lt => Bool(x < y),
        Op::Le => Bool(x <= y),
        Op::Gt => Bool(x > y),
        Op::Ge => Bool(x >= y),
        Op::BitAnd => Int(x & y),
        Op::BitOr => Int(x | y),
        Op::BitXor => Int(x ^ y),
        Op::Shl => Int(x.wrapping_shl(*y as u32)),
        Op::Shr => Int(x.wrapping_shr(*y as u32)),
        Op::And => Int(if *x != 0 { *y } else { *x }),
        Op::Or => Int(if *x != 0 { *x } else { *y }),
        _ => Value::Err,
    }
}

/// IEEE-754 binary op on two f64 operands (#342). Self-consistent and deterministic — it need
/// not match any one language. `==`/`!=` use RAW f64 equality (so `NaN == NaN` is `false`, the
/// IEEE semantics of the SOURCE operator), which is distinct from `F64`'s canonical behavior-
/// comparison `Eq`. Division by zero `Err`s (consistent with the integer arm) rather than
/// producing inf/NaN; bitwise ops `Err` (floats have no bit ops).
fn float_bin(op: Op, x: f64, y: f64) -> Value {
    use Value::{Bool, Float};
    match op {
        Op::Add => Float(F64(x + y)),
        Op::Sub => Float(F64(x - y)),
        Op::Mul => Float(F64(x * y)),
        Op::Div | Op::TrueDiv | Op::FloorDiv if y == 0.0 => Value::Err,
        Op::Div | Op::TrueDiv => Float(F64(x / y)),
        Op::FloorDiv => Float(F64((x / y).floor())),
        Op::Mod | Op::FloorMod if y == 0.0 => Value::Err,
        Op::Mod => Float(F64(x % y)),
        Op::FloorMod => Float(F64(x.rem_euclid(y))),
        Op::Pow => Float(F64(x.powf(y))),
        Op::Eq => Bool(x == y),
        Op::Ne => Bool(x != y),
        Op::Lt => Bool(x < y),
        Op::Le => Bool(x <= y),
        Op::Gt => Bool(x > y),
        Op::Ge => Bool(x >= y),
        Op::And => Float(F64(if x != 0.0 { y } else { x })),
        Op::Or => Float(F64(if x != 0.0 { x } else { y })),
        _ => Value::Err,
    }
}

pub(super) fn un(op: Op, a: &Value) -> Value {
    if contains_sym(a) {
        return Value::Sym(sym_id(0x0004_0911, &[hashed(&op), vhash(a)]));
    }
    match (op, a) {
        // `wrapping_neg` (not `-i`) so negating `i64::MIN` wraps to `i64::MIN` instead of
        // panicking on overflow — consistent with the wrapping binary arithmetic above.
        (Op::Neg, Value::Int(i)) => Value::Int(i.wrapping_neg()),
        (Op::Pos, Value::Int(i)) => Value::Int(*i),
        (Op::BitNot, Value::Int(i)) => Value::Int(!i),
        // Float sign ops (#342): `-x`/`+x` on a float; `~` Errs (no bit ops on floats).
        (Op::Neg, Value::Float(f)) => Value::Float(F64(-f.0)),
        (Op::Pos, Value::Float(f)) => Value::Float(*f),
        // Negating an ERROR propagates the error — `not (1/0)` raises in Python, it does
        // NOT yield `True`. Without this, `not (a<=b)` on non-numeric operands wrongly gave
        // `Bool(true)` while the direct `a>b` gave `Err`, making the SOUND comparison-
        // negation canon (`!(a<=b) ≡ a>b`, a total order) look like a false merge.
        (Op::Not, Value::Err) => Value::Err,
        // Operand is concrete here — the symbolic arm above intercepts `Sym`.
        (Op::Not, _) => truthy(a).map_or(Value::Err, |b| Value::Bool(!b)),
        _ => Value::Err,
    }
}
