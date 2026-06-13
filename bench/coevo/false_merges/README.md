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
| untyped_add_commute.py | `a+b≡b+a` for untyped (string/list concat) | yes (battery floor #294) | FIXED #283-C — `proven_numeric` `+`-reorder gate |
| ruby_star_repetition.rb | Ruby `*` reorders, but `"ab"*3` ≠ `3*"ab"` (repetition is asymmetric) | yes | FIXED series 9 — `ac_chain_commutes` Ruby-`*` gate |
| (cross-lang shift) | JS `a<<b`/`a>>b` (int32) ≡ Python arbitrary-precision `<<`/`>>` | n/a (cross-language — no single-file repro) | FIXED series 9 — JS shift operand int32-narrowed |
| float_assoc.py | `(a+b)+c≡a+(b+c)` for floats | NO — oracle blind | OPEN (C/float) — needs the `Float` value kind (D-div) |
| array_element_mutation.py | `swap` ≡ `clobber` — `a[i]` read after an indexed write re-derives the pre-write value | NO — oracle blind | OPEN — needs in-place element-mutation modeling (value graph + interp), see oracle-value-model §7.3 |

FIXED rows are now covered by permanent regression tests (effect cases in the
value-graph suite; `-(-a)`/`a&a`/`a+b` in `crates/nose-cli/tests/equivalence.rs`,
`double_negation_cancels_only_for_proven_numeric` +
`bitwise_self_idempotence_gates_on_proven_numeric` +
`untyped_add_commute_gates_on_proven_numeric`; the series-9 shift and Ruby-`*` cases in
`js_shift_is_int32_and_distinct_from_arbitrary_precision` +
`ruby_star_repetition_is_ordered_but_other_multiply_commutes`; the series-9 dataflow
inline-soundness cases in `dataflow_does_not_unsoundly_inline_a_temp_past_a_write_or_into_a_lambda`).
The OPEN rows are oracle-blind value-model gaps: `float_assoc.py` needs the `Float` value
kind (D-div, #283-D); `array_element_mutation.py` needs in-place element-mutation modeling
(oracle-value-model §7.3). Both share the limitation that the interpreter cannot witness
them, so no battery row distinguishes the merged pair.
