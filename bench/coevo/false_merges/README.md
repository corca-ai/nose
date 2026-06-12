# Confirmed false-merge reproducers (coevo series 5)

Each `.py`/`.js`/… file holds two functions that `nose scan --mode semantic`
reports as one `exact-value-graph` family but that compute different things.
`nose verify <file> --max-violations 0` exits non-zero on the verify-confirmed
ones — the offline soundness oracle catches them; they are LATENT (the pinned
corpus does not contain the shapes, so `nose verify bench/repos` stays green —
the §AS scenario design.md §1 cites as the reason adversarial batteries exist).

These are the cardinal sin (design §1: equal fingerprint ⟹ equal behavior).
Tracked as a P0 in issue #283. Do not delete until #283 closes with these
moved into the permanent regression battery.

| file | claim violated | verify-caught? | status |
|---|---|---|---|
| effect_commute.py | commutative `+` reorders observable effects | yes | FIXED #286 (A) — `reorder_safe` |
| effect_acchain.py | AC-chain sorts effectful leaves | yes | FIXED #286 (A) — `reorder_safe` |
| neg_involution.py | `-(-x)→x` on optimistically-Num param | yes (canon-preservation) | FIXED #283-B — `proven_numeric` |
| untyped_add_commute.py | `a+b≡b+a` for untyped (string/list concat) | **yes** (battery floor) | OPEN (C) — oracle now witnesses it; detector gate pending |
| float_assoc.py | `(a+b)+c≡a+(b+c)` for floats | NO — oracle blind | OPEN (C/float) — needs the `Float` value kind (D-div) |

FIXED rows are now covered by permanent regression tests (effect cases in the
value-graph suite; `-(-a)`/`a&a` in `crates/nose-cli/tests/equivalence.rs`,
`double_negation_cancels_only_for_proven_numeric` +
`bitwise_self_idempotence_gates_on_proven_numeric`). The reproducer files stay
until #283 fully closes (the OPEN rows are oracle-blind — see #283-C/-D).
