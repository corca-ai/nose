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
> **C's detector-side string/mixed-coercion floor has shipped** (remaining work:
> oracle input/model coverage and float non-associativity), **D-int32's
> detector/value-graph floor has shipped** (remaining work: oracle int32 execution
> plus any full fixed-width recall recovery), and **D-div's fingerprint floor has
> shipped** while the full `Float` value kind remains deferred. The remaining work
> is now oracle/model enrichment and priced recall recovery, not one shared
> foundational rewrite.

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

### C — untyped `+` commutativity / associativity *(detector gate shipped; oracle coverage gap remains)*

Historically, `a+b ≡ b+a` and `(a+b)+c ≡ a+(b+c)` merged for untyped params. That
is wrong for strings (`"x"+"y" ≠ "y"+"x"`), for JS/Java-style mixed string
coercion (`"a"+2+3` differs from `"a"+(2+3)`), and for floats
(`(1e100+1)-1e100`). The detector now fails closed for this family: in
JS/TS/Vue/Svelte/HTML/Java it only flattens/reorders `+`, rewrites `x-y` to
`x+(-y)`, or distributes `-(x+y)` when every `+` leaf is proven non-concat.
Untyped mixed forms such as `x+4`, `x+(2+3)`, `x-3`, and `-(x+2)` stay split from
numeric-only rewrites.

- **Value model:** sufficient for the pure **string + string** half — the `Str`
  monoid is order-sensitive. The oracle still does not model JS/Java
  numeric-to-string coercion for `string + number`, so mixed-coercion regressions
  rely on external-language witnesses plus detector fingerprint splits until that
  model exists. The **float** half of C overlaps D-div; see there.
- **Remaining gap:** the **battery** (§1.1) never feeds two distinct strings/lists
  to two params at once, and it has no mixed string/number coercion rows. The
  detector no longer merges the known C string/mixed cases, but `nose verify`
  remains too weak to witness all of them by itself.
- **Cheapest of the three.** The detector floor is in place; the remaining work
  is oracle coverage/modeling plus recall pricing on larger corpora.

### D-int32 — JS bitwise width *(detector floor shipped; oracle width gap remains)*

JS bitwise ops coerce operands to int32 (`ToInt32`); Python/Ruby are
arbitrary-precision. Historically the detector merged JS `a&b` with Python
`a&b` (same `Bin(BitAnd,[a,b])` fingerprint) and the oracle read SOUND (both
modeled as i64 `x & y`). They differ for operands outside int32 range (`2^40 &
2^40` → JS `0`, Python `2^40`).

- **Value kind:** none new — int32 is representable inside `Int(i64)` via
  `x as i32 as i64`.
- **Detector floor (shipped):** JS-family bitwise leaves now carry a `ToInt32`
  wrapper in the value graph. That makes JS `& | ^ ~ << >>` fingerprints distinct
  from arbitrary-precision Python/Ruby while preserving within-JS commutativity
  and De Morgan recall (`~(a&b)` still converges with `~a|~b`). `>>>` remains a
  distinct raw/operator surface.
- **Remaining gap:** the independent interpreter still executes bitwise over
  i64, so `nose verify` cannot yet witness the JS-vs-bigint difference by itself.
  A full fixed-width model must thread width through bitwise canon and interpreter
  execution, then price any JS↔C/Java-int recall recovery.

### D-div — true-float division *(fingerprint floor shipped; Float value gap remains)*

Three-way split: `/` is **true-float** in Python 3 / JS (`7/2 == 3.5`),
**floored-int** in Ruby (`7/2 == 3`), **truncated-int** in C/Go/Java/Rust
(`-7/2 == -3`). The integer cases are partly handled (`Op::FloorDiv` exists), but
they are **operand-type-entangled** — Ruby `/` is floored only for integers and
true-float for floats, so even Ruby `/` ≡ Python `//` holds *only on integers*.
The true-float result `3.5` is **unrepresentable in `Int(i64)`**.

- **Value kind:** genuinely needs **`Float`** (plus the float semantics of
  `+ - * / %` and the per-language Int↔Float coercion rules).
- **Detector floor (shipped):** Python and JS `/` now lower to `Op::TrueDiv`,
  Ruby `/` and Python `//` lower to `Op::FloorDiv`, and C-family integer `/`
  remains `Op::Div`. The value graph no longer merges true-float, floored, and
  truncated division.
- **Remaining gap:** the interpreter still maps `TrueDiv` integer inputs through
  i64 truncation internally. That is safe for the detector floor because
  `TrueDiv` has a distinct fingerprint, but it is not a real float model and
  cannot witness `7 / 2 == 3.5` or NaN/±0 behavior.
- **Most expensive.** Float drags in NaN (`NaN != NaN`), `±0`, non-associativity
  (the float half of C), and equality quirks — a large new surface in both the
  interpreter and the canonicalizer.

---

## 3. Minimum scope per finding — a sound floor, then optionally a full model

Each finding has a **fail-closed floor** (sound, cheap, with a measured recall
cost) and a **full model** (recovers the recall, larger surface). Ship the floor
first; promote to the full model only if the priced recall loss justifies it.

### 3.1 C — battery/model enrichment after the detector gate

- **Detector floor (shipped):** gate untyped `+` commute/reorder and related
  add/sub/negation rewrites on a *proven-not-concat* predicate: reorder only when
  every operand is proven numeric/non-string. Untyped `+` stays ordered in
  string-coercive languages, while typed numeric `+` still converges.
- **Oracle coverage:** add battery rows that bind **two distinct strings** (and
  two distinct lists) to multiple positions — e.g. probe pairs `("α","β")` drawn
  from the mined string set, plus a couple of distinct-list rows. Add mixed
  string/number rows only after the interpreter has explicit JS/Java coercion
  semantics. *Effect:* `verify` can witness `a+b ≠ b+a` for strings and later
  `"a"+2+3 ≠ "a"+(2+3)` for coercive languages, instead of relying only on
  detector fingerprint splits and external-language witnesses.
- **Cost:** Medium detector gate already paid; remaining Low-Medium oracle
  enrichment/modeling plus recall pricing.

### 3.2 D-int32 — width-aware bitwise canon

- **Floor (shipped):** JS bitwise (`& | ^ ~ << >>`) fingerprints through a
  distinct `ToInt32` narrowing wrapper, so it stops merging with
  arbitrary-precision bitwise. De Morgan and same-language bitwise recall remain
  green because the wrap lands on leaves rather than replacing the whole operator.
- **Full model:** a first-class fixed-width bitwise semantics (int32 for JS,
  type-width for C/Java/Go/Rust) that the whole bitwise canon understands — so
  JS `a&b` ≡ Java-`int` `a&b` ≡ C-`int32_t` `a&b` reconverge correctly while
  staying split from bigint and 64-bit. Largest of the three canon surfaces.
- **Cost:** floor paid; remaining Medium-High for oracle int32 execution and full
  fixed-width recall recovery.

### 3.3 D-div — Float value kind

- **Floor (shipped):** Python/JS `/` (true-float division) fingerprints as
  `TrueDiv`, distinct from C-family truncated `Div` and Ruby/Python `FloorDiv`.
  Same-semantics surfaces still converge (`Python /` with JS `/`, Ruby `/` with
  Python `//`).
- **Non-associativity — syntactic-float AND float-typed-param `+`/`*`/`-` (shipped).** Float
  `+`/`*` is non-associative (`(1.0+a)+b ≠ 1.0+(a+b)`) and a float `-` must not fold into a
  sum, so a chain that is **provably float** keeps its source grouping. Two sources of proof
  are now honored: a **syntactically-float** leaf (a float literal or a `/` true-division
  result), and a **float-TYPED param** (`: float`, `f64`, `double`, `float64` — via the param
  domain evidence, `proven_float(Input)`). Held in BOTH layers: the `algebra` IL pass
  (`chain_has_float` over the syntactic markers + the float-typed-param cids → leave the tree
  intact, like the mixed-coercion `+` bail) and the value graph (`proven_float` → rebuild the
  source grouping instead of flatten/sort, in the general AC path AND the string-coercion `+`
  path used by JS/TS/Java; `eval_sub_chain` keeps a float `-` as `Sub`). Closes the documented
  `(1e100+1)-1e100`, the float-literal/division `(a+b)+c ≡ a+(b+c)`, and now the
  `def f(a: float, b: float, c: float): (a+b)+c` typed-param case across Python/Rust/Java/Go/TS.
  **Corpus family delta = 0** (type4: 20→20; the typed-param cases are split-only — int-typed
  and untyped chains still associate); verify clean. The earlier worry that "the fingerprint
  flattens AC chains to a leaf sequence so no canon gate can help" was a **misdiagnosis**: the
  fingerprint is the reachable-**node** multiset (structure-sensitive, `intern_node` hashes
  args in order); the grouping was lost at the `algebra` reassociation and the value-graph
  flatten, both of which ARE gateable.
- **Fully-untyped — CLOSED (#342, the `Value::Float` kind).** The last case, `(a+b)+c` vs
  `a+(b+c)` over params with NO float marker (`float_assoc.py`), is now closed in both halves:
  the interpreter gained a real IEEE-754 `Value::Float` (`interp.rs`), and a `verify_battery`
  float row (`1e16 ± 1e16`) feeds untyped params adversarial floats so the oracle WITNESSES the
  non-associativity; the scan holds the grouping (`possibly_float` = a truly-untyped param in a
  dynamically-typed language, mirrored in `algebra`). Crucially the hold is associativity-only —
  COMMUTATIVITY is preserved (`a+b+1 ≡ b+a+1`, same grouping) via a grouping-preserving rebuild
  that still sorts operands when the chain is non-concat — and `: int`-typed and `Number`-typed
  params still fully reassociate (the oracle feeds them `Int`). **Recall delta 0 on the full
  105-repo pinned corpus** (4309 → 4309; the design doc's gate, measured), verify clean across
  type4/coevo and 15 dynamic-language repos. See [value-float-kind-design](value-float-kind-design.md).
- **Cost:** floor + syntactic + float-typed-param + fully-untyped non-associativity all paid
  (0 recall on the full pinned corpus). The remaining float work is breadth, not a gap: a
  full Int↔Float coercion lattice (mixed-type comparison, float literals — `LitFloat` stores
  only a hash today) so the oracle witnesses MORE float behavior, not just non-associativity.

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
   dup-gate (`scripts/check-duplication.sh`, budget 28) and the byte-identical
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
| C — string/mixed-coercive `+` ordering | `untyped_add_commute.py` (`a+b` vs `b+a`) and JS/TS/Java-style `x+4` / `x+(2+3)` / `x-3` / `-(x+2)` | detector keeps them **split**; oracle witnesses pure string now only after battery enrichment, and mixed string/number after coercion modeling |
| C — float `+` non-associativity | `float_assoc.py` (`(a+b)+c` vs `a+(b+c)`) | detector keeps them **split** AND oracle witnesses — CLOSED (#342, the `Value::Float` kind, §3.3); guarded by `float_addition_is_non_associative_in_the_oracle` + `algebra_associativity` + `float_typed_param_addition_is_held_unassociated` |
| D-int32 — JS bitwise vs bigint | cross-language `a & b` (JS vs Python), e.g. the §2 reproducer | detector keeps **split**; oracle int32 execution remains follow-up |
| D-div — true-float `/` | Python/JS `a/b` vs Ruby `a/b` vs C `a/b` | the three **do not merge**; real Float oracle remains follow-up |
| in-place element mutation | `array_element_mutation.py` (`swap` vs `clobber`) | detector keeps them **split** AND oracle witnesses — CLOSED (#337, §7.3); guarded by `array_element_swap_does_not_merge_with_clobber` + `index_store_is_observed_by_later_read` |

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
2. **D-int32 — floor shipped; defer the full width model.** The distinct
   narrowing fingerprint is in place. Next work is int32 oracle execution plus
   measured JS↔C/Java-int recall recovery. Promote to the full fixed-width canon
   only if that loss is material.
3. **D-div (Float) — CLOSED.** The true/floored/truncated fingerprints are split, and float
   `+`/`*` non-associativity is held for syntactic-float, float-typed-param (#339/#340) AND
   fully-untyped (#342) chains. The full `Value::Float` model shipped: the interpreter models
   IEEE-754 (so the oracle witnesses non-associativity), the scan holds untyped chains, and the
   recall price was **measured 0 on the full 105-repo pinned corpus** (4309 → 4309). Remaining
   float work is breadth (a full Int↔Float coercion lattice, float literals), not a soundness
   gap — see **[value-float-kind-design](value-float-kind-design.md)** (#342).

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

### 7.3 In-place element mutation — CLOSED (#337)

The value model treated an indexed store `a[i] = v` as an ordered, opaque effect that did
NOT update readable state — so a later `a[i]` read re-derived the *pre-write* value (`field`
stores were versioned via `field_env`; array elements were not). Under that model `swap`
(`t=a[i]; a[i]=a[j]; a[j]=t`) and `clobber` (`a[i]=a[j]; a[j]=a[i]`) produced the same
element-write trace and shared an `exact-value-graph` fingerprint — a false merge (`swap`
on `[1,2]` gives `[2,1]`, `clobber` gives `[2,2]`).

**The fix is READ-FORWARDING, not a versioned read node — which dissolves the blockers.**
The earlier scoping framed this as needing a third "generation" arg on every `a[i]` read
node (an epic with four blockers). That framing was wrong. The element WRITE is kept an
ordered effect (`push_effect`, order-sensitive — already sound even when indices alias);
the only new behavior is that a `base[index]` READ evaluated after a write to that exact
place FORWARDS to the written value (`index_env`, `value_graph/index_state.rs`). So
`clobber`'s second `a[i]` read becomes the value just stored in `a[j]`, while `swap`'s `t`
holds the pre-write value — distinct write traces, distinct fingerprints. The four "blockers":
1. **`ValOp::Index` 2-arity** — preserved. Forwarding returns the stored *value node*; it
   never adds an arg, so the dict-builder/getter positional matches are untouched.
2. **`eval` hash-consing** — irrelevant. `eval` is not traversal-memoised, and pre/post-write
   reads are distinct IL nodes anyway; forwarding consults `index_env` at eval time.
3. **Aliasing** — handled conservatively. A forward is installed only for an UNCONDITIONAL,
   top-level, non-loop write (`path` empty, `loop_depth == 0`), and ANY ordered effect
   (`push_effect` — a method call, `f(a)`, an opaque store), branch, or loop CLEARS the
   forwarding map. So a forward survives only across a straight-line, effect-free run with no
   possibly-aliasing write — sound by construction; errs toward fewer forwards (recall, never
   soundness). Conditional base reassignment becomes a `Phi` (a different value node), so its
   key never matches.
4. **Two models move together** — done. The interpreter (`interp.rs` `bind` for `Index`) now
   mutates the array in place (clean Var-holding-`List` + in-bounds-`Int` case; everything
   else falls back to the opaque no-mutation effect), so the soundness oracle WITNESSES the
   `swap`/`clobber` difference instead of being blind to it. This is essential: value-graph
   forwarding *without* interpreter mutation would false-merge `a[0]=5; return a[0]` with
   `a[0]=5; return 5` against the old non-mutating oracle.

`verify_battery` Part 4 adds `(list, int, int, int)` rows so a `swap(a,i,j)`/`clobber(a,i,j)`
sees a list base with two distinct int indices — the combinatorial pool binds slot ≥2 of a
≥3-arg function to a list, so without these rows the distinguishing input never occurs. (A
list base with int indices is the normal array shape, NOT a type-incoherent string-as-index
row, so it does not manufacture the spurious canon-preservation divergence §7 warns about —
verified clean on type4 and coevo.) Recorded outcome: **corpus family delta 0** (type4
20→20), verify clean, swap/clobber split AND oracle-witnessed. A Lean obligation analogous
to `normalize.value_graph.field_writes` is a tracked follow-up (the argument today is
conservative-by-construction plus the oracle).

**Net:** the equality-over-`Err` class (§7.1), the dataflow unsoundness (§7.2), and the
in-place element-mutation gap (§7.3) are all closed, so the verify soundness gate can widen
toward dynamic-language repos. Type-domain-aware input feeding remains the floor-then-model
follow-up (§3); the rest of `verify_battery` stays hand-curated **on purpose** (the guard
comment there points here).

---

*See also: [design & direction](design.md) · [formal soundness](formal-soundness.md) ·
[normalization](normalization.md) · [fragment contracts](fragment-contracts.md) ·
[adversarial co-evolution](adversarial-coevolution.md) · [experiments §CE](experiments.md)*
