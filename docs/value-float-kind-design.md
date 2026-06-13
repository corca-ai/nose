# Value::Float — the last C-float gap (#342, SHIPPED)

Design + outcome record for closing the **fully-untyped** float-associativity false merge —
the last piece of the [#283](https://github.com/corca-ai/nose/issues/283) C-float cluster
after the syntactic and typed cases shipped (#339/#340).

Companion to [oracle-value-model §3.3 / §6](oracle-value-model.md) (the cluster scope),
[design §1 (the sound core is the moat)](design.md), and
[formal-soundness](formal-soundness.md).

> **Bottom line — SHIPPED.** The recall objection that kept this NO-GO was **refuted by
> measurement**: holding untyped `+`/`*` chains costs **0 families on the full 105-repo
> pinned corpus** (4309 → 4309), and **0** on type4 + nose. So #342 shipped both halves
> together (the only gate-safe way — P1 alone exposes the merge as a visible violation
> without fixing it):
> - **P1 (oracle):** a real IEEE-754 `Value::Float` in `interp.rs`, plus a `verify_battery`
>   float row (`1e16 ± 1e16`) so the oracle WITNESSES `(a+b)+c ≠ a+(b+c)`.
> - **P2 (scan):** the value graph holds the grouping for a truly-untyped param in a
>   dynamically-typed language (`possibly_float`), mirrored in the `algebra` pass.
>
> The hold is **associativity-only**: commutativity is preserved (`a+b+1 ≡ b+a+1`, same
> grouping) by a grouping-preserving rebuild that still sorts non-concat operands, and
> `: int`/`Number`-typed params still fully reassociate (the oracle feeds them `Int`).
> Verify clean across type4, coevo, and 15 dynamic-language repos. The §1–§6 below are the
> original design; §7 records what shipped vs. deferred.

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

## 7. Outcome (what shipped, #342)

**SHIPPED — P1 + P2 together.** P1 alone is NOT gate-safe (it turns the latent merge into a
*witnessed* violation without fixing the scan), so both halves landed in one change:

- **P1 (oracle):** `Value::Float(F64)` in `interp.rs` with IEEE-754 `bin`/`un` arithmetic
  (`F64` is a newtype with a behavior-comparison `Eq` that canonicalizes NaN/±0, so `Value`'s
  derive is preserved). `verify_battery` Part 5 feeds adversarial floats (`1e16 ± 1e16`) so the
  oracle witnesses non-associativity. Float *literals* stay opaque (`LitFloat` carries only a
  hash) — only battery-fed params are concrete, which bounds the new interpretable surface.
- **P2 (scan):** `possibly_float` = `proven_float` OR a **truly-untyped** (`None`-domain) param
  in a dynamically-typed language, used by the grouping holds (`eval_assoc_comm_chain`,
  `eval_sub_chain`, `ac_chain_canon`) and mirrored in `algebra`. A grouping-preserving rebuild
  (`rebuild_grouped_float_chain`) keeps the source grouping while still sorting non-concat
  operands, so **commutativity is preserved** (`a+b+1 ≡ b+a+1`) and only **associativity** is
  held. `Number`-typed params are NOT held — the oracle feeds them `Int`, so it cannot witness
  float there, and holding would split int/`number` clones (incl. cross-language) for no
  soundness gain.

**Recall: 0** on the full 105-repo pinned corpus (4309 → 4309) — the design's gate, measured
on real code, not just type4. Verify clean on type4, coevo, and 15 dynamic-language repos.

**Deferred (breadth, not a gap):** a full Int↔Float coercion lattice (mixed-type `==`, float
literals once `LitFloat` retains a value, `Number`-param float witnessing). These widen what
the oracle witnesses; none is a known false merge today.

---

*See also: [oracle-value-model](oracle-value-model.md) · [design](design.md) ·
[formal-soundness](formal-soundness.md) · [normalization](normalization.md) ·
[adversarial co-evolution](adversarial-coevolution.md)*
