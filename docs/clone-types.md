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

This page states what nose does for each — including where it stops. Back to
[home](home.md); the engine is in [architecture](architecture.md).

## Type-1 — fully

Whitespace, layout, and comments never enter the IL, so Type-1 fragments produce identical
fingerprints. Caught by both the unit fingerprints and the contiguous (Rabin-Karp) channel.

## Type-2 — identifiers and types fully; literals on a two-axis split

- **Identifiers** are alpha-renamed to canonical ids, so renamed copies converge.
- **Types** are erased during normalization.
- **Literal values** are handled deliberately on two axes. The *behavioral* fingerprint
  RETAINS behavior-defining literals (`0` ≠ `1`, `true` ≠ `false`, distinct strings/floats) —
  different literals are different behavior. The *structural* fingerprint abstracts them to
  their class. So a Type-2 clone that differs only in literal values is matched in candidate
  mode (`nose scan`, structure-dominant), but deliberately kept distinct under `--strict`
  (behavioral mode).

## Type-3 — near-duplicate via similarity (the primary use)

Unit pairs are scored by value-graph + shape similarity and structural alignment, and
accepted above a threshold (0.70 in candidate mode). A copy with added/removed/changed
statements scores below 1.0 but above the threshold, so it surfaces as a near-duplicate
family — which is exactly what `nose scan` ranks. The contiguous channel adds the
copy-paste floor. How much divergence still matches is bounded by the threshold (raise it
for tighter matches, lower it for more recall).

## Type-4 — a modeled subset, not arbitrary equivalence

This is nose's distinguishing capability, but it is **bounded, not total** — arbitrary
semantic equivalence is undecidable. nose converges the equivalence classes its IL, value
graph, and canonicalizations actually model:

- loop ↔ `reduce`/`sum` ↔ comprehension ↔ `.append`/`.map` builder loop; an `any`/`all` loop
  ↔ the functional form;
- guard-clause ↔ nested-if, ternary ↔ early-return, min/max idioms, commutativity,
  `a − b ≡ a + (−b)`, De Morgan, short-circuit `and`/`or`;
- the same algorithm written in a **different language**.

For the classes it captures, the match carries a **soundness guarantee**: equal fingerprint
⟹ equal behavior, enforced by an interpreter oracle (`nose verify`) and machine-checked in
Lean (`formal/`). See [normalization](normalization.md) for the full pass list.

### What nose does *not* do (no overclaim)

- **Different algorithms with the same result** — e.g. bubble sort vs quicksort, or two
  different primality tests — are **not** recognized. Only the modeled transformations
  converge; nose does not search for arbitrary input/output equivalence.
- **Recursion ↔ iteration** is out of scope (deferred).
- The behavioral *proof* (`nose verify`) covers only interpretable (≈ pure) units; detection
  itself runs on every unit via the fingerprint, but pairs outside that interpretable slice
  carry no per-pair behavioral proof.
- Type-4 coverage is a **growing set of modeled equivalences**, not a guarantee about any
  given pair of semantically-equal fragments.

## Two modes, and cross-language

- **`nose scan`** (candidate mode): recall-oriented, structure-dominant — surfaces
  Type-1/2/3 and the modeled Type-4 as ranked refactoring candidates.
- **`nose scan --strict`** (behavioral mode): precision gates on — cleaner Type-4, where
  literal/return-value differences correctly split pairs.
- The taxonomy is usually stated within a single language; because every language lowers to
  one shared IL, nose applies Type-1–4 **across languages** as well.
