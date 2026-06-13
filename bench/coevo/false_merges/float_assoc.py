# CLOSED (#342). `a`,`b`,`c` are untyped, so they could be floats — and float `+` is
# non-associative: `(1e16 + -1e16) + 1 = 1` but `1e16 + (-1e16 + 1) = 0`. These once shared an
# `exact-value-graph` fingerprint (the i64 oracle is associative). Now the interpreter models a
# real IEEE-754 `Value::Float` and a float battery row feeds untyped params adversarial floats, so
# `nose verify` WITNESSES the difference; the value graph holds the grouping for truly-untyped
# params in dynamically-typed languages (`possibly_float`). Commutativity is preserved (`a+b+1`
# ≡ `b+a+1`); only associativity is held. See docs/value-float-kind-design.md (#342).
def assoc_l(a, b, c):
    return (a + b) + c

def assoc_r(a, b, c):
    return a + (b + c)
