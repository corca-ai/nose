# Value::Float — design & go/no-go for the last C-float gap (#342)

Design document for closing the **fully-untyped** float-associativity false merge —
the one remaining piece of the [#283](https://github.com/corca-ai/nose/issues/283)
C-float cluster after the syntactic and typed cases shipped. It is the §6 "D-div full
model" prerequisite-(b) design doc, and it records the prerequisite-(a) **recall
measurement** that the go/no-go turns on.

Companion to [oracle-value-model §3.3 / §6](oracle-value-model.md) (the cluster scope),
[design §1 (the sound core is the moat)](design.md), and
[formal-soundness](formal-soundness.md).

> **Bottom line up front.** The recall objection that kept this NO-GO is **not borne
> out by measurement**: holding *every* untyped `+`/`*` chain unassociated costs **0
> families** on type4 (74 Python files + 6 other langs) and **0** on nose's own Rust.
> So the blocker is no longer recall — it is the implementation surface (an IEEE-754
> `Value::Float` in the interpreter + a per-language Int↔Float coercion lattice) and the
> fact that, until the oracle can *witness* float non-associativity, holding untyped
> chains splits units the i64 oracle calls equal (a **completeness** regression with no
> behavioral justification). Recommendation: **GO, phased** — build the float oracle
> first (so the split is justified), then enable the hold; re-measure recall on a pinned
> real-world corpus before flipping the scan default.

## 1. The gap

`bench/coevo/false_merges/float_assoc.py`:

```python
def assoc_l(a, b, c):
    return (a + b) + c

def assoc_r(a, b, c):
    return a + (b + c)
```

`a`, `b`, `c` bear **no** float evidence. IEEE-754 `+` is non-associative
(`(1e308 + 1e308) + (-1e308)` = `inf`, but `1e308 + (1e308 + -1e308)` = `1e308`), so if
the params are floats the two functions differ. The value graph flattens the `+` chain
to one AC node, so they share an `exact-value-graph` fingerprint — a latent false merge.

What makes this the *last* and *hardest* case:

- The closed cases had a **float witness**: a `LitFloat`/`TrueDiv` leaf (#339) or a
  float-**typed** param (#340). The canon could gate on that marker and hold only the
  proven-float chains — split-only, recall ≈ 0, no model change.
- Here there is **no marker**. Nothing distinguishes a float `a` from an integer `a`.
- It is **sound under the current i64 oracle**: integer `+` *is* associative, so the
  oracle considers `assoc_l ≡ assoc_r` behavior-equal. The merge is only wrong under
  IEEE-754 — a model the oracle does not have. So unlike `swap`/`clobber` (#337, a real
  behavioral difference for any list), this is unwitnessable today.

## 2. Why "just hold untyped `+`/`*`" is not enough on its own

Holding the chain (refusing to associate `(a+b)+c` with `a+(b+c)`) is **always sound** —
it only ever *splits*, never merges, so it cannot create a false merge. The cost is pure
recall: integer clones written with different groupings stop converging.

Two objections, one empirical and one principled:

- **Recall (empirical) — REMOVED by measurement.** See §3: 0 family loss on every corpus
  available locally.
- **Justification (principled) — REAL.** Under the i64 oracle `assoc_l ≡ assoc_r`, so
  splitting them is splitting **behavior-equal** units. The oracle's completeness check
  would report a new *under-merged* group (a missed clone). Splitting what your own
  ground-truth calls equal, with no model that says otherwise, is unprincipled — it is
  exactly the "split on a hunch" the recall-pricing protocol exists to prevent. The fix
  is to give the oracle the float model so it *witnesses* the inequality and **justifies**
  the split.

This is the inverse of the #337 lesson. There, the scoping doc was too pessimistic and a
sound bounded slice existed (read-forwarding). Here the bounded slice (the hold) exists
and is recall-cheap, but shipping it *alone* would be a completeness regression without
the oracle to back it. So the model is the actual work, not an avoidable epic.

## 3. The recall measurement (§6 prerequisite (a))

Method: force `chain_has_float` (algebra) and `proven_float` (value graph) to treat every
`+`/`*` chain as possibly-float — an **upper bound** on the hold's recall cost (it also
holds provably-integer chains the real model would leave associative). Scan `--mode
semantic`, compare exact-channel family counts.

| corpus | baseline families | hold-all families | delta |
|---|---|---|---|
| `bench/type4` (74 py, 35 rs, 34 js, 23 ts, 20 rb, 13 java, 4 go) | 20 | 20 | **0** |
| nose's own `crates/` (real Rust) | 5 | 5 | **0** |

Zero loss even at the upper bound, including the Python-heavy synthetic clone corpus.
**Caveat:** neither corpus is a large dynamically-typed real-world repo, where untyped
integer arithmetic written in varied groupings is most likely. Re-measure on the pinned
`bench/repos` set (`scripts/corpus-verify-nightly.sh` corpus) before flipping the scan
default in Phase 2.

## 4. The model

### 4.1 Interpreter — a real `Value::Float`

Add `Value::Float(f64)` alongside `Value::Int(i64)` in `interp.rs`:

- **Arithmetic** under IEEE-754: `+`/`-`/`*`/`/` on floats round per double precision;
  `(big + big) + -big ≠ big + (big + -big)` becomes observable, so the oracle can witness
  `assoc_l ≢ assoc_r` on a float battery row.
- **NaN / ±0 / inf**: `NaN != NaN` (so a function returning `NaN` is not equal to one
  returning `NaN` — matches every language's `==`), `+0.0 == -0.0` but `1/+0 = +inf ≠
  1/-0 = -inf`. These are the corners that need their own test matrix.
- **Mixed `Int`/`Float`**: an arithmetic op with one float operand promotes per the
  language's rule (§4.2); a comparison `1 == 1.0` follows the language (true in Python/JS,
  not directly expressible in Rust/Java without a cast).

### 4.2 The Int↔Float coercion lattice (per-language)

`+`/`*` non-associativity must fire only where the operands are *actually* float at
runtime, which is language-specific:

- **Python / JS / Ruby**: `/` is always float; `int + float → float`; an untyped param
  may be either, so a chain over untyped params is **possibly-float**.
- **Rust / Go / C / Java**: types are known; `f64`/`double`/`float64` chains are float
  (already held, #340), `i64`/`int` chains are integer (stay associative). Untyped does
  not arise — these have no fully-untyped numeric params.

So the *untyped-possibly-float* case is **only** reachable in the dynamically-typed
languages. That bounds Phase 2's scan-hold to Python/JS/Ruby `+`/`*` chains whose leaves
have no integer evidence — narrower (and cheaper) than the §3 upper-bound measurement.

### 4.3 Fingerprint — hold untyped `+`/`*`, justified

Once the oracle witnesses float non-associativity, extend the existing two-layer hold
(`algebra::chain_has_float`, value-graph `proven_float`) to also fire when a leaf is an
**untyped param in a dynamically-typed language** (no integer evidence). Reuses the exact
machinery #339/#340 shipped; the only new input is "untyped leaf in Python/JS/Ruby".

## 5. Phasing (each independently priced & shippable)

1. **P1 — float oracle (interp only).** Add `Value::Float`, IEEE-754 arithmetic, a
   `verify_battery` float row (a list/scalar pool entry that is a float), and the
   coercion rules for the dynamic languages. Deliverable: `nose verify` reports
   `assoc_l`/`assoc_r` in `float_assoc.py` as a **witnessed** false merge (behavior
   groups distinct). No scan change yet — this *exposes* the bug under the soundness gate.
   Risk: medium (NaN/±0/coercion correctness); fully covered by interp unit tests.
2. **P2 — scan hold (close the merge).** Extend `chain_has_float`/`proven_float` to the
   untyped-dynamic-language case so the fingerprint splits the two. Re-measure recall on
   the pinned corpus first; ship only if the loss stays negligible. Deliverable: scan
   splits them, verify clean again.
3. **P3 — cross-language coercion lattice.** Generalize §4.2 so float-ness propagates
   through calls and mixed Int/Float expressions, and fold the remaining float-coercion
   corners (string `+` number, `//` vs `/`). Largest surface; defer until P1+P2 prove the
   value.

## 6. Risks & open questions

- **NaN/±0/inf** are the classic float-oracle footguns; P1 must land with an explicit
  corner-case test matrix, not just the associativity case.
- **Recall on real dynamic-language repos** is unmeasured (only type4 + nose). P2 is
  gated on that measurement.
- **`==` across Int/Float** differs by language; the coercion lattice (§4.2) must encode
  it, and the battery must not manufacture impossible comparisons (the §7 lesson from
  oracle-value-model — type-incoherent rows cause spurious canon-preservation failures).
- **Completeness vs soundness framing** (§2): P2 deliberately trades a (measured-zero)
  completeness loss for a soundness gain; document it in the CHANGELOG so it is not read
  as a regression.

## 7. Recommendation

**GO, phased.** The recall objection that kept the full `Value::Float` model NO-GO is not
supported by measurement (§3). The remaining cost is the IEEE-754 interpreter model and
the coercion lattice — real but bounded, and sequenced so each phase is independently
priced and the scan default flips only after a real-corpus recall re-measurement. Start
with P1 (the float oracle), which converts an unwitnessable latent merge into a gate-
visible one without touching scan recall at all.

---

*See also: [oracle-value-model](oracle-value-model.md) · [design](design.md) ·
[formal-soundness](formal-soundness.md) · [normalization](normalization.md) ·
[adversarial co-evolution](adversarial-coevolution.md)*
