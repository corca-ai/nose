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
> facts, and ordered string-builder joins (`out += elem` over a loop ≡ `"".join(xs)`).
> Soundness enforced by the independent interpreter oracle + canon-preservation check
> (`nose verify`) and Lean proofs (`formal/`, incl. `distrib_sound`, `filter_fusion`,
> `Compare.lean`); see §AJ/§AW/§AX/§BA.
> Deferred: value-dependent folding (needs literal values), full distribution
> (equality saturation), general CFG flag-loop↔break, loop↔recursion, and non-local
> early-exit variants beyond the simple flag+break loop. Rejected as cross-language-unsound: `x*2≡x+x`
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
elimination. Excludes loop↔recursion (out of scope). Hard parts: structuring
arbitrary control flow, proving shape-equivalence, determinism.

---

### Why semantic normalization is still worth trying

Naive PDG-style slicing was below the noise floor in an earlier token-based
prototype. nose runs these analyses on a genuinely parsed, type-erased,
alpha-renamed IL, so dataflow and value identities are recoverable. Same idea,
different substrate.
