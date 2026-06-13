# The verify oracle's value model — scope, gaps, and the extension plan

Design / go-no-go document for closing the three remaining sub-findings of the
P0 false-merge cluster ([#283](https://github.com/corca-ai/nose/issues/283) C,
D-div, D-int32). It scopes *what the oracle's value model is today*, *what each
finding actually needs* (which is **not** one shared "extend i64" prerequisite),
the **minimum** change per finding, the **recall-pricing protocol**, the
**regression targets**, and a **go/no-go** recommendation.

Companion to [design §1 (the sound core is the moat)](design.md),
[formal-soundness](formal-soundness.md), [normalization](normalization.md), the
[fragment behavior oracle](fragment-contracts.md), and the
[adversarial co-evolution runbook](adversarial-coevolution.md). The series-5
finding that opened #283 is [experiments §CE](experiments.md).

> **Bottom line up front.** The oracle's value model is richer than #283's prose
> implies — it already models strings as an order-sensitive free monoid. The
> three findings do **not** share a single "i64 → {int, float, string}"
> extension. They decompose into three independent pieces of very different cost:
> **C is an input-battery gap** (the value model already suffices), **D-int32 is
> a canonicalization-width problem** (no new value kind), and **D-div is the only
> one that needs a genuinely new value kind (Float)**. Each has a cheap, sound
> *fail-closed floor* that can ship first, with a measured recall cost, ahead of
> any full model.

---

## 1. What the value model is today

The offline interpreter oracle (`nose verify`, a differential-testing harness —
**not** a runtime accept gate; see [design §1](design.md)) interprets each unit
on a battery of inputs and compares observable behavior. Its value type is
`Value` (`crates/nose-normalize/src/interp.rs`):

| kind | models | notes |
|---|---|---|
| `Int(i64)` | every integer | **all numbers** — there is no float (see §3.3) |
| `Bool(bool)` | booleans | |
| `Str(Vec<u64>)` | strings as a **free monoid** over appended token hashes | **order-sensitive**: `"x"+"y"` = `[hx,hy]` ≠ `[hy,hx]` = `"y"+"x"`. No char content; length/index stay `Err`. |
| `List(Vec<Value>)` | sequences | |
| `Null` | null/none | |
| `Err` | a runtime error | itself observable — two programs must err on the same inputs |
| `Sym` | a symbolic value (opaque call / unproven read / any composition over one) | a differential convention; branching on a `Sym` bails the unit, and a `Sym`-containing behavior never feeds the hard SOUND gate |

Two facts matter for everything below:

- **Strings are already order-sensitive.** The free-monoid `Str` was built
  precisely so that an unsound commutative treatment of `+` on strings is
  *witnessable* — the model is not the gap for finding C.
- **There is no `Float`.** Every numeric literal and result is an `i64`. Float
  *literals* are kept distinct in the value graph (`LitFloat`, a hash of the
  source text, so `3.14 ≠ 2.71` — `crates/nose-il/src/node.rs`), but the
  interpreter has no float arithmetic, so any *computation* whose result depends
  on float semantics is invisible to the oracle.

### 1.1 The input battery (the other half of the oracle)

A bounded differential checker is only as strong as the inputs it feeds. The
production battery is `verify_battery` / `verify_probes`
(`crates/nose-cli/src/main.rs`):

- **Combinatorial core** — a fixed `pool` of **small integers and integer
  lists** (`3, 0, -1, 7, 2`, `[1,2,3,4]`, `[5,1,4,2,8]`, `[]`), enumerated
  mixed-radix over a width-4 row so a 2-arg function's first two slots see
  `a<b`, `a>b`, `a==b`. **The pool contains no strings and no floats.**
- **Literal probes** — the top-16 string-literal hashes and top-16 integers the
  corpus actually branches on, each injected at **one** position with the other
  slots filled by `pool[0]` (the list `[1,2,3,4]`).

The `wide` ("leap-3") battery widens the integer/list domain but follows the same
shape: still no strings in the combinatorial core, still one-position string
probes.

**Consequence:** the battery never binds **two distinct strings to two
parameters simultaneously**. So `f(a,b)=a+b` and `g(a,b)=b+a` are only ever
compared on integer/list rows (where `+` is commutative or both `Err`) and on
single-string probe rows (`"x" + [1,2,3,4]` → `Err`, same as the swap) — never on
`("x","y")`, the one input on which they differ. The order-sensitive `Str` model
is present but **starved of the input that would exercise it**.

---

## 2. The three findings, re-scoped

### C — untyped `+` commutativity / associativity *(input-battery gap)*

`a+b ≡ b+a` and `(a+b)+c ≡ a+(b+c)` are merged for untyped params. Wrong for
strings (`"x"+"y" ≠ "y"+"x"`) and for floats (`(1e100+1)-1e100`). The detector
commutes/reorders `+` whenever an operand is not *proven* concat
(`order_bin_operands`, `eval_sub_chain` in
`crates/nose-normalize/src/value_graph/`); for untyped params nothing is proven,
so it reorders optimistically.

- **Value model:** already sufficient for the **string** half — the `Str` monoid
  is order-sensitive. (The **float** half of C overlaps D-div; see there.)
- **Real gap:** the **battery** (§1.1) never feeds two distinct strings/lists to
  two params at once, so the oracle cannot witness the divergence — it reads
  SOUND while the detector merges. This is the §AS "green corpus, latent false
  merge" scenario, in the *oracle's own inputs* rather than the corpus.
- **Cheapest of the three.** No new value kind.

### D-int32 — JS bitwise width *(canonicalization-width problem)*

JS bitwise ops coerce operands to int32 (`ToInt32`); Python/Ruby are
arbitrary-precision. The detector merges JS `a&b` with Python `a&b` (same
`Bin(BitAnd,[a,b])` fingerprint) and the oracle reads SOUND (both modeled as i64
`x & y`). They differ for operands outside int32 range (`2^40 & 2^40` → JS `0`,
Python `2^40`). **Confirmed active false merge** (verified 2026-06-12; reproducer
below).

- **Value kind:** none new — int32 is representable inside `Int(i64)` via
  `x as i32 as i64`.
- **Real gap:** the *fingerprint* must differ (an oracle-only fix would make
  `verify` flag the merge but leave the detector merging, and risk reddening the
  corpus gate). The clean way — narrow JS bitwise operands — **collides with the
  existing bitwise canonicalizations**: De Morgan `~(a&b)→~a|~b`
  ([#284](https://github.com/corca-ai/nose/issues/284)) stops firing when `~`'s
  operand is a narrowing node instead of a `BitAnd`; the `a&a→a` idempotence
  (gated in [#283-B](https://github.com/corca-ai/nose/issues/283)) is itself
  **unsound for JS** (`x&x` = ToInt32(x) ≠ x for `x ≥ 2³¹`); the u16/u32
  byte-pack patterns no longer recognize wrapped operands. Width-awareness has to
  be threaded **through the bitwise canon**, not bolted on. This is canon
  surgery, not a value-model extension.

### D-div — true-float division *(the only genuine value-kind gap)*

Three-way split: `/` is **true-float** in Python 3 / JS (`7/2 == 3.5`),
**floored-int** in Ruby (`7/2 == 3`), **truncated-int** in C/Go/Java/Rust
(`-7/2 == -3`). The integer cases are partly handled (`Op::FloorDiv` exists), but
they are **operand-type-entangled** — Ruby `/` is floored only for integers and
true-float for floats, so even Ruby `/` ≡ Python `//` holds *only on integers*.
The true-float result `3.5` is **unrepresentable in `Int(i64)`**.

- **Value kind:** genuinely needs **`Float`** (plus the float semantics of
  `+ - * / %` and the per-language Int↔Float coercion rules).
- **Most expensive.** Float drags in NaN (`NaN != NaN`), `±0`, non-associativity
  (the float half of C), and equality quirks — a large new surface in both the
  interpreter and the canonicalizer.

---

## 3. Minimum scope per finding — a sound floor, then optionally a full model

Each finding has a **fail-closed floor** (sound, cheap, with a measured recall
cost) and a **full model** (recovers the recall, larger surface). Ship the floor
first; promote to the full model only if the priced recall loss justifies it.

### 3.1 C — battery enrichment (+ a detector gate)

- **Floor (measure-only):** add battery rows that bind **two distinct strings**
  (and two distinct lists) to multiple positions — e.g. probe pairs `("α","β")`
  drawn from the mined string set, plus a couple of distinct-list rows. No model
  change. *Effect:* `verify` now witnesses `a+b ≠ b+a` for strings and **flags
  the existing false merge** — turning a latent hole into a measured one. Expect
  `nose verify bench/repos` to surface new false merges if the corpus contains
  untyped `+` reorders (this is the safety net working; it tells us the size of
  the problem before we touch the detector).
- **Fix:** gate the untyped `+` commute/reorder (`order_bin_operands`,
  `eval_sub_chain`) on a *proven-not-concat* predicate (analogous to
  [#283-B](https://github.com/corca-ai/nose/issues/283)'s `proven_numeric`):
  reorder only when every operand is proven numeric/non-string. Untyped `+`
  stays ordered. **This is the recall-priced step** — many real clones are
  untyped numeric `+` that we must keep converging, so the predicate has to admit
  the genuine numeric case (via the same genuine-evidence channel as
  `proven_numeric`) without admitting untyped.
- **Cost:** Low (battery) + Medium (the gate + its recall pricing).

### 3.2 D-int32 — width-aware bitwise canon (or fail-closed)

- **Floor (fail-closed):** lower JS bitwise (`& | ^ ~ << >> >>>`) to a distinct
  **opaque/narrowing op** that (a) fingerprints differently from
  arbitrary-precision bitwise and (b) the interpreter models with int32
  narrowing. JS bitwise stops merging with Python/Ruby (and, conservatively, with
  C/Java-int — a recall loss to price). *Risk:* interacts with De Morgan /
  idempotence / byte-pack (§2) — those rules must be made width-aware **or**
  disabled for the narrowed op (recall loss).
- **Full model:** a first-class fixed-width bitwise semantics (int32 for JS,
  type-width for C/Java/Go/Rust) that the whole bitwise canon understands — so
  JS `a&b` ≡ Java-`int` `a&b` ≡ C-`int32_t` `a&b` reconverge correctly while
  staying split from bigint and 64-bit. Largest of the three canon surfaces.
- **Cost:** Medium (floor) → High (full).

### 3.3 D-div — Float value kind (or fail-closed)

- **Floor (fail-closed):** lower Python/JS `/` (true-float division) to an op the
  interpreter **bails on** (`Sym`/`Err`, so it never feeds the SOUND gate) and
  that fingerprints distinctly from integer division. True-float `/` units stop
  merging across the int/float boundary. *Recall cost:* every true-float `/` unit
  becomes oracle-opaque (no longer eligible for the exact behavioral claim) —
  potentially significant in float-heavy corpora; **must be priced**.
- **Full model:** add `Value::Float` with IEEE-754 semantics, the per-language
  Int↔Float coercion lattice, and the float half of C (`+`/`*` non-associativity,
  `(a+b)+c ≠ a+(b+c)`). Recovers the recall the floor gives up. **Largest single
  piece in the whole #283 cluster** — its own design doc if pursued.
- **Cost:** Low–Medium (floor) → High (full).

---

## 4. Recall-pricing protocol

Every change here is a soundness *gain* paid for in *recall*. None ships without
the price measured, on both the dev split and a heldout split, using the tooling
already in the loop ([design §4b coverage loop](design.md),
[experiments](experiments.md) pricing methodology,
[adversarial co-evolution runbook](adversarial-coevolution.md) accept-distribution
pre-gate).

For each candidate fix, record **before → after**:

1. **Soundness (must improve or hold).**
   `nose verify bench/repos` → `SOUND: no false merges ✓`, and the reproducer in
   §5 must flip from merged to split. Canon-preservation violations must not rise
   above the standing baseline (currently **20**, sympy/nushell).
2. **Recall delta (the price).**
   `nose scan bench/repos --mode semantic --top 0` family count, **dev split and
   heldout split separately**. A change that holds family count on dev but drops
   it on heldout is over-fit; a change that holds both at ~0 cost (as
   [#283-B](https://github.com/corca-ai/nose/issues/283) did: 4294 → 4294) is a
   free soundness win. Report the absolute family delta and the specific families
   lost, not just a percentage.
3. **Accept-distribution pre-gate.**
   Before trusting the family delta, confirm the accept distribution didn't shift
   pathologically (a fix that merges *nothing* trivially has "no false merges");
   the pre-gate guards against measuring a degenerate detector.
4. **No silent regressions elsewhere.**
   dup-gate (`scripts/check-duplication.sh`, budget 25) and the byte-identical
   `cmp` fingerprint-stability gate must stay green; build a `main` worktree
   baseline for the family count so the delta is apples-to-apples (see the perf
   tooling notes in the repo memory / [dogfooding](dogfooding.md)).

A fix is **GO** only when soundness improves, the reproducer flips, and the recall
price (dev **and** heldout) is either ~0 or an explicitly accepted trade.

---

## 5. Regression targets

The confirmed reproducers live in `bench/coevo/false_merges/` (kept until #283
fully closes; the fixed rows are already covered by permanent tests). Each fix
must move its target from "merged + oracle-blind/silent" to "split (or
oracle-caught)":

| target | file / repro | must become |
|---|---|---|
| C — string `+` non-commutativity | `untyped_add_commute.py` (`a+b` vs `b+a`) | oracle **witnesses** the difference; detector keeps them **split** |
| C — float `+` non-associativity | `float_assoc.py` (`(a+b)+c` vs `a+(b+c)`) | oracle witnesses (needs §3.3 float) or detector keeps split |
| D-int32 — JS bitwise vs bigint | cross-language `a & b` (JS vs Python), e.g. the §2 reproducer — **add to the battery** | detector keeps **split**; oracle models int32 |
| D-div — true-float `/` | Python/JS `a/b` vs Ruby `a/b` vs C `a/b` | the three **do not merge**; oracle models float (or fails closed) |

New permanent regression tests follow the pattern of
`crates/nose-cli/tests/equivalence.rs`
(`double_negation_cancels_only_for_proven_numeric` etc.): assert the
`value_fp` of the divergent forms differs, and that the genuine convergent case
still merges.

---

## 6. Go / no-go recommendation

The findings are **decoupled** — there is no forced ordering and no shared
foundational blocker (the original "extend the value model" framing was too
coarse). Recommended sequence, cheapest-and-highest-confidence first:

1. **C (battery + `+`-gate) — GO first.** The value model already supports it;
   the only new code is battery rows plus a `proven_numeric`-style gate we have a
   proven template for. Highest soundness-per-effort. The battery enrichment
   alone (the floor) is worth shipping immediately — it strengthens the oracle for
   *every* future finding and tells us how big the untyped-`+` problem actually
   is before we price the gate.
2. **D-int32 — GO as a fail-closed floor; defer the full width model.** Ship the
   distinct narrowing op + int32 interpreter semantics, price the JS↔C/Java-int
   recall loss. Promote to the full fixed-width canon only if that loss is
   material. The canon interactions (§3.2) are the real work and should be a
   separate PR from the floor.
3. **D-div (Float) — NO-GO for now; floor-only.** Ship the fail-closed floor
   (true-float `/` bails + fingerprints distinctly) and **measure** the recall
   cost. The full `Value::Float` model is the largest single surface in the
   cluster and should not be started until (a) the floor's recall price proves it
   necessary and (b) it gets its own design doc covering NaN/±0/coercion.

This converts "one big scary foundational extension" into **three independently
priced, independently shippable fixes**, each with a sound floor — exactly the
incremental, measured discipline [design §1](design.md) and the
[co-evolution runbook](adversarial-coevolution.md) call for.

## 7. Why the battery is not broadened by naive enumeration (#317)

#317 proposed replacing the hand-curated input battery (`verify_battery`) with an
*enumerative* distinguishing-input search. The naive form — feed more value kinds
(equal strings, bool, null) at more positions — is **unsound**, and the reason is
worth recording so it is not re-attempted:

The battery is global and positional: row `[a,b,c,d]` is bound to params 0–3 of
*every* unit, regardless of each param's type. A unit's param carries a coercion only
when it has **declared domain evidence** (`coerce_to_declared_domain`, interp.rs);
typed-language array/index params (Java `byte[] bytes`, `int startPos`,
`String[] a`) carry **none** today, so an incoherent value reaches them uncoerced.
Feeding, say, a string or a list to a slot a unit uses as an array index manufactures
an input that **can never occur**, and on it the *interpreter*, not the canonicalizer,
diverged — producing a spurious **canon-preservation** violation (the check is
concrete-only but does not filter `Err`).

This is not hypothetical, and not new to broadening: the *current* battery already
trips it. `nose verify` on `netty` reported 3 canon-preservation "violations", all on
type-incoherent rows, and `sympy` 20 — masked only because the nightly soundness gate's
pinned corpus includes neither.

### 7.1 The equality-over-`Err` mechanism — fixed (coevo series 9)

The series-9 oracle attacker sharpened the dominant class: the trigger was the
**`==`/`!=` canon meeting an `Err` operand**, and it was **language-independent**
(untyped Python `def f(b,s): return b[s]==0` reproduced the typed-Java `bytes[i] != 0`).
The root was an **interpreter asymmetry**, not a canon bug: `eval_bin_op` short-circuited
`Err` on the *left* operand only, and `bin`'s comparison fallthrough read `0 == Err` as a
concrete `Bool(false)` (and `!=` as `Bool(true)`). So the moment the sound `==`/`!=`
operand-ordering canon (algebra sorts commutative comparison operands by hash) moved the
erroring operand to the right, the full IL returned a `Bool` while the pre-canon core
returned `Err` — a spurious violation. **Fix: `bin` propagates `Err` from *either*
operand** (the symmetric twin of `un`'s existing `(Op::Not, Err) => Err`); the left
short-circuit stays for laziness/effect order. This is pure interpreter fidelity —
`interp.rs` is verify-only, so scan output is byte-identical — and it closes the class
wherever it arises: **sympy 20 → 2 canon violations, netty 3 → 2**, every other checked
repo unchanged at 0, zero false merges throughout.

### 7.2 The remaining violations were real dataflow bugs — fixed (coevo series 9)

The two narrower sub-classes that survived §7.1 turned out NOT to be spurious: the
canon-preservation check was correctly catching the copy-propagation inliner
(`dataflow.rs`) make **real-semantics-unsound** moves, surfaced via type-incoherent rows
only because the value model doesn't track in-place mutation (§7.3).

- **Inline past a clobbering indexed write** (netty/guava `swap`/`swapElements`/
  `ObjectArrays`): `t = a[i]; a[i] = a[j]; a[j] = t` was inlined to `a[i] = a[j]; a[j] =
  a[i]` — reading the *already-overwritten* `a[i]`, turning a swap into "set both to
  `a[j]`". The inliner's hazard check missed it because `collect_writes` only recorded
  `Var` assignments, not that an indexed store `a[i] = …` MUTATES the base `a`.
- **Inline into a lambda body** (sympy `_matches_get_other_nodes` /
  `unit_propagate_int_repr`): `ind = nodes[k]` (a possibly-raising read) was inlined into
  a comprehension filter — evaluated zero times on an empty iterable, eliding the `Err`
  the eager read raises. The inliner mapped the lambda-internal use to its top-level
  statement and saw it as "a later statement of the same block."

**Fix:** `collect_writes` records the root var of an `Index`/`Field` store target as a
write (so the hazard check blocks the swap inline), and the inliner skips any use in a
CONDITIONAL/REPEATED position — a lambda body, an `If` branch, or under a `Loop` — so it
never moves a read into a context evaluated under a different condition. Both are
real-semantics soundness fixes; the value-graph FINGERPRINT is essentially unchanged (the
value graph does its own env substitution, corpus family delta ≈ 0) and the soundness
oracle stays clean. Canon-preservation violations are now **zero** across netty/sympy/guava
and the wider sweep — together with `bin`'s symmetric `Err` (§7.1), every
canon-preservation violation the corpus surfaced is closed.

### 7.3 The deeper limit: in-place element mutation is not modeled (oracle-blind, OPEN)

The value model treats an indexed/collection store `a[i] = v` as an ordered, opaque effect
and does NOT update readable state — so a later `a[i]` read re-derives the *pre-write*
value (`field` stores ARE versioned via `field_env`; array elements are not). Under that
model `swap` (`t=a[i]; a[i]=a[j]; a[j]=t`) and `clobber` (`a[i]=a[j]; a[j]=a[i]`) compute
the same effect trace, so they share an `exact-value-graph` fingerprint — a latent false
merge. It is **oracle-blind**: the interpreter shares the same no-mutation model, so the
false-merge check sees them as behavior-equal too (no battery row distinguishes them). This
is the same category as float non-associativity (`bench/coevo/false_merges/float_assoc.py`):
a known value-model gap the oracle cannot witness, not a regression. Closing it needs
in-place element-mutation modeling (version `a[i]` reads by the array's write state) in
BOTH the value graph and the interpreter — a value-model extension on the scale of the
`Float` kind (§3.3), tracked separately. Until then it is recorded in
`bench/coevo/false_merges/` as an OPEN, oracle-blind class.

**Net:** the equality-over-`Err` class (§7.1) and the dataflow unsoundness (§7.2) are
closed, so the verify soundness gate can widen toward dynamic-language repos. The
array-mutation modeling gap (§7.3) and type-domain-aware input feeding remain the
floor-then-model follow-ups (§3); `verify_battery`'s Part 3 stays hand-curated **on
purpose** (a guard comment there points here).

---

*See also: [design & direction](design.md) · [formal soundness](formal-soundness.md) ·
[normalization](normalization.md) · [fragment contracts](fragment-contracts.md) ·
[adversarial co-evolution](adversarial-coevolution.md) · [experiments §CE](experiments.md)*
