# Clone types — what nose covers

The standard clone taxonomy is from Roy, Cordy & Koschke, *"Comparison and evaluation of
code clone detection techniques and tools: A qualitative approach"*, Science of Computer
Programming (2009) — <https://www.sciencedirect.com/science/article/pii/S0167642309000367>.
The four types:

- **Type-1** — identical fragments except whitespace, layout, and comments.
- **Type-2** — syntactically identical except identifiers, literals, and types (plus Type-1
  variations).
- **Type-3** — copied fragments with statements changed, added, or removed (plus Type-2
  variations).
- **Type-4** — fragments that perform the same computation but are implemented by different
  syntactic variants (semantic clones).

This page states what nose does for each — including where it stops. The scan modes are
detector channels, not perfect taxonomy buckets: the default combines `syntax` and
`semantic`, and `near` is the opt-in fuzzy Type-3 surface. Back to [home](home.md);
the engine is in [architecture](architecture.md).

## Type-1 — fully

Whitespace, layout, and comments never enter the IL, so Type-1 fragments produce identical
fingerprints. They are caught by the `syntax` CPD floor and by the unit fingerprints.

## Type-2 — identifiers and types fully; literals on a two-axis split

- **Identifiers** are alpha-renamed to canonical ids in the normalized unit channels, so
  renamed copies can converge under `semantic` or `near`.
- **Types** are erased during normalization.
- **Literal values** are handled deliberately on two axes. The *behavioral* fingerprint
  RETAINS behavior-defining literals (`0` ≠ `1`, `true` ≠ `false`, distinct strings/floats) —
  different literals are different behavior. The *structural* fingerprint abstracts them to
  their class. So a Type-2 clone that differs only in literal values is matched by
  `--mode near`, but deliberately kept distinct by exact `--mode semantic`.

`--mode syntax` is intentionally the CPD floor: it is excellent for copied runs that a
token detector should catch, including runs that cross function boundaries, but it is not
the whole renamed-Type-2 story by itself. Use the default or add `near` when renamed or
literal-varied copies matter.

## Type-3 — near-duplicate via similarity (the primary use)

Unit pairs are scored by value-graph + shape similarity and structural alignment, and
accepted above a threshold (0.70 by default in `--mode near`). A copy with added/removed/changed
statements scores below 1.0 but above the threshold, so it surfaces as a near-duplicate
family. How much divergence still matches is bounded by the threshold (raise it for tighter
matches, lower it for more recall).

## Type-4 — a modeled subset, not arbitrary equivalence

This is nose's distinguishing capability, but it is **bounded, not total** — arbitrary
semantic equivalence is undecidable. nose converges the equivalence classes its IL, value
graph, and canonicalizations actually model:

- loop ↔ `reduce`/`sum` ↔ comprehension ↔ `.append`/`.map` builder loop; an `any`/`all` loop
  ↔ the functional form; a `reduce`-lambda min/max ↔ `max`/`min`; `len([… if p]) ↔ Σ(p?1:0)`;
  a dict-building loop `d={}; for x: d[k]=v` ↔ the dict comprehension `{k: v for x in xs}`;
- literal map-default lookup through proven map/key/fallback coordinates, including
  Python `dict.get(key, fallback)` and Ruby literal `Hash#fetch(key, fallback)` or
  zero-arg pure fallback blocks such as `Hash#fetch(key) { fallback }`;
- `filter q (filter p) ↔ filter (p∧q)` (and the filtered comprehension / `.filter().filter()`
  chain / filtered builder loop);
- guard-clause ↔ nested-if, ternary ↔ early-return, min/max idioms, commutativity +
  associativity (AC-canonical operands), distribution `a·c + b·c ≡ (a+b)·c` (numeric),
  `a − b ≡ a + (−b)`, De Morgan, short-circuit `and`/`or`;
- the same algorithm written in a **different language** (incl. Rust `it.iter().filter(p).sum()`
  ↔ Python `sum(x for x in xs if p)`).

For the classes it captures, the match carries a **soundness guarantee**: equal fingerprint
⟹ equal behavior, enforced by an interpreter oracle (`nose verify`) and machine-checked in
Lean (`formal/`). See [normalization](normalization.md) for the full pass list.

### What nose does *not* do (no overclaim)

- **Different algorithms with the same result** — e.g. bubble sort vs quicksort, or two
  different primality tests — are **not** recognized. Only the modeled transformations
  converge; nose does not search for arbitrary input/output equivalence.
- **Recursion ↔ iteration** is partially modeled — a bounded subset converges (see
  [normalization](normalization.md)); general recursion↔iteration remains out of scope.
- The behavioral *proof* (`nose verify`) covers only interpretable (≈ pure) units; detection
  itself runs on every unit via the fingerprint, but pairs outside that interpretable slice
  carry no per-pair behavioral proof.
- Type-4 coverage is a **growing set of modeled equivalences**, not a guarantee about any
  given pair of semantically-equal fragments.
- Sub-function semantic coverage is intentionally bounded: nose extracts control-flow
  blocks and exact-safe single-statement fragments (return/throw expressions and simple
  conditional return/throw/effect guards, including bare returns, explicit empty no-op
  branches, and nested branches whose only non-empty statement is another exact
  conditional, a single exact ForEach effect loop, or exactly two ordered exact effect
  items drawn from ForEach effect loops, append effects, conditional direct effects, and
  non-overloadable C/Go/Java index assignments, plus branches that assign one local
  temporary and immediately consume it in a direct return/throw/effect statement or
  assign two local temporaries as a linear chain and immediately consume the final
  temporary in such a statement or a non-overloadable C/Go/Java index assignment whose
  receiver does not depend on the temporary, plus ForEach loops whose only loop-body
  effects are appends or non-overloadable C/Go/Java index assignments that depend on the
  iteration binding,
  optionally preceded by one loop-local temporary assignment or a two-temporary linear
  chain whose first RHS depends on the iteration binding and whose final value is
  immediately consumed by that effect, plus exact conditional branches containing two or
  three ordered single-item append effects where each item is direct or immediately
  consumes a branch-local temp/temp-chain, plus exact conditional branches containing two or
  three ordered non-overloadable C/Go/Java index-assignment effects where each assignment is direct or
  immediately consumes a branch-local temp/temp-chain, plus Java
  `this.field = value` self-field assignments and all-self-field Java function-body blocks
  with the receiver fixed to `this`, optionally ending in `return this`), not arbitrary
  statement windows with unmodeled free-variable, live-out, receiver-overload, or effect
  boundaries. Multiple statement-level effects are ordered through control-flow-aware
  sink tags; swapping appends/emits on one execution path is a behavior change, not a
  Type-4 clone.
- **Multi-clause (flat-map) comprehensions** — a Python `[e for a in A for b in B]`
  flattens (`A.flatMap(a => B.map(b => e))`), and nose has no `FlatMap` IL primitive yet,
  so these lower fail-closed (an opaque node) rather than converging with the equivalent
  nested loop. Single-clause comprehensions converge as above. Tracked in issue #57.

Exact fragment proof is not the same thing as user-facing refactorability. The
[fragment output audit](fragment-output-audit.md) classifies current real-corpus fragment
output into refactor-worthy, review-hazard, proof-only/noise, and metadata-ambiguous
buckets, and records the ranking/grouping metadata needed before small fragments should be
promoted in default reports.

## Scan modes, and cross-language

Each type maps to a detection channel: **Type-1/2 → `syntax`**, **Type-3 → `near`**
(fuzzy; the threshold rides on the mode, `near:0.8`), **Type-4 → `semantic`** (exact).
The default is `syntax,semantic`; see [usage → Scan modes](usage.md#scan-modes) for the
full table and how to compose channels.

The taxonomy is usually stated within a single language; because every language lowers to
one shared IL, nose applies Type-1–4 **across languages** as well — though in practice
rarely, since cross-language fragments seldom converge to the same fingerprint.
