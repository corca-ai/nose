# Semantic normalization roadmap (hard / high-value)

*Part of the [home](home.md) wiki. Normalization is step 2 of the pipeline in
[architecture](architecture.md); the experiments that validated these passes are in
[experiments](experiments.md).*


> **Status (all three tracks landed):** Track 1 — dataflow copy/expr propagation
> (`dataflow.rs`) + value-graph/GVN (`value_graph.rs`, the detection substrate;
> Stage 2 statement-order subsumed). Track 2 — algebraic canonicalization
> (`algebra.rs`: assoc/comm flatten, comparison-direction, De Morgan;
> value-independent). Track 3 — CFG normalization (`cfg_norm.rs` `structure()`:
> conjoined-guard merge, continue-guard unwrap). Pipeline: desugar → alpha →
> dataflow → [dce] → cfg_norm::structure → algebra → cfg_norm::run; value graph on top.
> (`dce.rs` dead-code/dead-assignment elimination is an optional pass, off by default.)
> Later additions on the value graph: a purpose-fit **type inference** (`types.rs`, now a
> fixpoint over subexpression result types) gating the type-dependent canons, free-monoid
> strings, map **and filter** fusion (a filter is the element-carrying `Hof(Map,[Elem,p])`,
> so nested filters fuse to `p∧q`), full **AC flatten+sort in the value graph itself** (not
> only the `algebra` IL pass), **distribution/factoring** `a*c+b*c→(a+b)*c` (Num-gated),
> min/max and any/all reductions (cross-language), simple **flag+break existence/universal
> loops** (`found=false; if p { found=true; break }` / the dual `all` form),
> **reduce-lambda selection** (`reduce(λ. a if a>b else b)≡max`), **count-of-filter**
> (`len([…if p])≡Σ(p?1:0)`), method-form iterator reductions (Rust
> `.sum()/.min()/.max()/.count()`), **dict-builder ≡ dict comprehension**
> (`d={}; for x: d[k]=v` ≡ `{k:v for x}` via a `DictEntry`-distinct rep that cannot collide
> with a list of tuples), ternary-return decomposition, negated-comparison canon,
> equality-chain literal membership (`x=="a" || x=="b"`), stricter record-shape guard
> facts, ordered string-builder joins (`out += elem` over a loop ≡ `"".join(xs)`),
> statically-false loop entry guards (a proven-true local boolean makes a left
> short-circuit `!local && ...` guard unreachable), primitive total-order comparator
> guard absorption (`x<y ∧ x≤y` keeps `x<y` for non-overloadable ordered comparisons),
> C byte-buffer `u16` big-endian packing (`(a[0]<<8)+a[1]` ≡ `(a[0]<<8)|a[1]` only
> under a byte-array parameter proof), Java `Arrays.asList(arrayParam)` membership over the
> proven array element domain, plus Java primitive-integer low-bit toggle selection
> (`x%2==0 ? x+1 : x-1` ≡ `x^1`).
> Also landed: **recursion → iteration** (`recursion.rs`) — tail recursion → `while`, and numeric
> structural (linear) recursion → an accumulator fold, so a recursive function converges with
> the loop a programmer would have written and with other same-shape recursions
> (cross-language included). Structural recursion is gated to a `+`/`·` numeric monoid
> (commutative + associative; identities `0`/`1`) with the base returning that identity
> literal; the interpreter now executes self-recursion so `nose verify` interprets the
> pre-canon recursive form and validates the rewrite (see *Recursion → iteration* below).
> Soundness enforced by the independent interpreter oracle + canon-preservation check
> (`nose verify`) and Lean proofs (`formal/`, incl. `distrib_sound`, `filter_fusion`,
> `Compare.lean`); see §AJ/§AW/§AX/§BA.
> Deferred: value-dependent folding (needs literal values), full distribution
> (equality saturation), general CFG flag-loop↔break, and non-local early-exit variants
> beyond the simple flag+break loop; recursion→iteration beyond the tail / numeric-monoid
> subset (tree & mutual recursion, list-tail catamorphisms over opaque slices, and the
> countdown↔`range` pairing — the rewrite is sound there but the value graph does not yet
> converge the two index forms). Rejected as cross-language-unsound: `x*2≡x+x`
> doubling and `s[-1]≡s[len(s)-1]` negative-index (§BA).


The IL today is a *normalized AST*: it canonicalizes surface syntax (loops,
idioms, identifiers, commutativity, local control flow). That captures Type-2 and
shallow Type-3 clones. The genuinely hard, high-value frontier is **semantic**
normalization — making structurally-different but behaviorally-equivalent code
converge. Three tracks, all pursued.

Guiding constraints for every pass:
- **Determinism**: same input → same output, independent of arena order.
- **Soundness is axis-dependent** (the two-axis principle, [experiments](experiments.md) §AH).
  nose has two fingerprints over this IL: a **behavioral** one (the value graph /
  GVN — *what the code computes*) and a **representation/candidate** one (structural
  shape, used by `scan`'s default candidate mode). The behavioral fingerprint is
  **sound by intent** — a *false merge* (two behaviorally-different snippets sharing a
  value-graph fingerprint) is a **bug**, not an accepted approximation. Soundness is
  enforced by an **independent interpreter oracle** (`nose verify`) that interprets the
  *pre-canonicalization* core IL — so a behavior-changing canon cannot mask itself — plus
  a **canon-preservation** check (core-IL behavior must equal full-IL behavior). The §AS
  hunt fixed seven false merges; §AX, using the now-independent oracle, fixed a whole
  further class of *treating-a-non-commutative-op-as-commutative* bugs (value `and`/`or`,
  `!!x`, `not(Err)`, `x+0`/`x*1`, string-`+` operand sort). The core canons are also
  **Lean-proven** (`formal/`). Operations whose commutativity/identity depends on type are
  **type-gated**, never assumed: `+` commutes only on non-concat operands (string/list `+`
  is ordered concatenation), and `and`/`or` commute only on Bool (else they are the
  value-returning short-circuit `Phi`). Identity elimination `x+0`/`x*1`→`x` is dropped
  entirely — it is unsound for non-Num (`"a"+0` Errs), and type inference is optimistic
  (it would infer `Num` from the `*1` itself), so even a Num gate can't make it safe.
  Typed emptiness checks carry the proven receiver domain in the value graph: collection
  receivers, arrays, and strings do not share the same strict fingerprint merely because
  each surface exposes an “empty” idiom. A real Netty audit caught this boundary when Java
  `Object[]` length, `Queue.isEmpty()`, and `String.isEmpty()` had collapsed.
  Java `Arrays.asList(values).contains(x)` also carries the array domain when `values` is
  a proven array parameter, keeping element membership distinct from singleton-list
  membership such as `List.of(values).contains(x)` or `Arrays.asList(listParam).contains(x)`.
  The remaining documented *exceptions* are large-constant/float abstraction (genuinely
  missing type information). The **fuzziness** a clone detector needs — abstracting magic
  numbers, tolerating structural difference — lives in the **candidate axis** and its
  scoring, never in the behavioral base. Never nondeterministic, either way.
- **Termination**: bounded rewriting (no infinite saturation).
- **Composition order**: desugar → alpha → **dataflow** → [dce] → **cfg_norm::structure**
  → **algebra** → **cfg_norm::run** → (later) value-graph (matching the status block above;
  CFG normalization straddles algebra — `structure()` runs before it, `run()` after). Each
  documented below.

---

## Track 1 — Dataflow normalization (value identity)

Make temporaries, intermediate steps, statement order, and common subexpressions
irrelevant. This is the highest-leverage track and the substrate for the
downstream value-graph.

- **Stage 1 — copy / expression propagation** *(implementing first)*
  Inline a variable that is assigned once, used once, with a side-effect-free RHS,
  when no statement between def and use writes a variable the RHS reads.
  `t = a + b; return t * 2` ≡ `return (a + b) * 2`. Chains fold transitively.
  Hard parts: scoping cids per function, purity approximation, the
  no-intervening-write hazard check, determinism.
- **Stage 2 — statement-order canonicalization**
  Within a block, reorder data-independent statements into a canonical order
  (topological sort keyed by a deterministic structural hash). Converges code that
  differs only by the order of independent steps.
- **Stage 3 — value-graph / GVN normal form**
  Convert a unit into a graph of *values* (each interned by `(op, operand-value-ids)`,
  commutative-aware) with global value numbering → automatic CSE, temporaries and
  renamings dissolve into value identity. Becomes the detection substrate
  (fingerprint subgraphs) and the natural home for the downstream graph/vectorize
  experiments. Hard parts: φ-handling across control flow, effect ordering,
  canonical graph hashing.

  A narrow Java-only selection idiom lives here: `x % 2 == 0 ? x + 1 : x - 1`
  and the equivalent `x % 2 != 0 ? x - 1 : x + 1` canonicalize to `x ^ 1`.
  The proof relies on Java primitive integer operators: even values take `+1`, odd
  values take `-1`, and the branch split avoids overflow at both signed extremes.
  It deliberately does not apply to overloadable or coercive operator surfaces.

## Track 2 — Algebraic expression canonicalization (E-graph)

Generalize the current commutative-operand sort into a principled canonicalizer
via bounded equality saturation over a fixed rule set:
- associativity flattening (`a+(b+c)` → canonical n-ary sum); all-literal constant
  folding (`2+3`→`5`). Identity elimination (`x+0`/`x*1`→`x`) is intentionally NOT done —
  unsound for non-Num and untypeable here (the optimistic inference would self-justify it).
  negation normalization (De Morgan,
  double-negation `!!x` cancelled only on Bool, negated-comparison `!(a<=b)`→`a>b`);
  comparison-direction canonicalization; short-circuit `and`/`or` to the value-`Phi`.
  Distribution (e.g. `a*c+b*c`) is intentionally NOT applied — it is unsound for strings
  (`("x"+"y")*2` ≠ `"xx"+"yy"`) and the operands can't be proven numeric.
Extract a canonical term by a cost function. Self-contained; strong on
expression-level Type-4. Hard parts: rule confluence, termination, choosing the
canonical extraction, integer/float/overflow caveats (kept approximate).

## Track 3 — Control-flow graph normalization

Beyond today's local rewrites (else-after-return, branch orientation): build a
structured CFG and canonicalize equivalent shapes — flag-variable loop ↔ `break`,
nested guards ↔ flattened guards, `continue`-skip ↔ wrapped body, redundant-jump
elimination. Hard parts: structuring
arbitrary control flow, proving shape-equivalence, determinism.

A narrow value-graph CFG fact is accepted for statically-false loop entry guards: if a local
boolean is already proven `true`, `while (!local && rhs) { ... }` has an unreachable body and
update by short-circuit semantics. This deliberately does not fold the right-hand guard and
does not apply after reassignment or to unproven receiver/dynamic facts. Loop-carried
placeholders are keyed by traversal/carry slot, not source variable id, so unused parameters
do not keep equivalent loop recurrences apart.

## Track 4 — Recursion → iteration

`recursion.rs` rewrites the two recursion schemes that have a behavior-preserving iterative
form, in the SEMANTIC phase (after the oracle's structural cutoff), so the loops it emits
flow through dataflow / cfg-norm / the value graph and converge with hand-written iteration.

- **Tail recursion → `while`.** `f(p…): if c₀: return v₀; …; return f(a…)` becomes
  `while not(c₀ or …) { p… := a… }; if c₀: return v₀; …; return vₖ₋₁`. The next call's
  argument bindings run each turn in a hazard-safe order (a cyclic binding such as a swap
  bails); on exit exactly one guard holds, so the post-loop guard chain returns the same base
  value. This is plain tail-call elimination — sound for *any* guards/arguments, no algebra
  needed.
- **Structural (linear) recursion → accumulator fold.** `f(p…): if base: return e;
  return HEAD ⊕ f(a…)` becomes `acc = e; while not(base) { acc = acc ⊕ HEAD; p… := a… };
  return acc`. The recursion is a right fold `HEAD₀ ⊕ (HEAD₁ ⊕ (… ⊕ e))`; the loop is a left
  fold. They are equal **iff ⊕ is an associative monoid with identity `e`**, so the rewrite
  fires only for `⊕ ∈ {+, ·}` proven `Num` (commutative + associative; identities `0`/`1`)
  with the base case returning exactly that identity literal. Short-circuit `and`/`or` are
  excluded: their early-exit skips later `HEAD`s the accumulator loop still evaluates.

Both schemes require exactly one self-call (a same-named call inside a standalone function);
anything else is left untouched. **Soundness** is checked, not assumed: the interpreter
([`interp`](../crates/nose-normalize/src/interp.rs)) now executes self-recursion, so
`nose verify` interprets the original recursion *and* the rewritten loop and flags any
behavioral difference (when the recursion terminates on the input battery — a guard like
`n == 0` that loops forever on negatives is excluded on both sides, identically). On real
code `nose verify` stays sound (0 false merges). Its concrete model covers
`range(stop)` and `range(start, stop[, step])`, including zero-step error behavior, so
range-index loops are checked under the same bounded input semantics. Bare `throw`/`raise`
statements execute as observable `Err` behavior, while exception handlers remain outside
the interpreter rather than guessed.

Out of scope (sound but not yet convergent, or genuinely hard): tree & mutual recursion
(multiple / non-tail self-calls); list-tail catamorphisms `head ⊕ f(xs[1:])`, whose slice is
opaque to the interpreter and value graph; and the countdown-loop ↔ `range`-loop pairing,
where the rewrite's `while n != 0` countdown is correct but does not converge with a
`for i in range(n)` form.

(A pre-existing value-graph false merge surfaced while building this — a non-reduction
loop accumulator's compact `Recurrence` value was keyed on its per-iteration update only,
dropping the pre-loop **seed**, so `a + Σ` (parameter seed) collapsed onto `Σ` (literal-`0`
seed). Fixed in the same change: the recurrence now carries its seed as an operand, so the
seed reaches the fingerprint. It reproduced with hand-written loops alone — the recursion
rewrite merely made it reachable from recursive functions too.)

---

### Why semantic normalization is still worth trying

Naive PDG-style slicing was below the noise floor in an earlier token-based
prototype. nose runs these analyses on a genuinely parsed, type-erased,
alpha-renamed IL, so dataflow and value identities are recoverable. Same idea,
different substrate.
