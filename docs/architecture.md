# Architecture

nose lowers every language into one normalized intermediate language (IL),
designed so that semantically-equivalent code converges toward identical
structure, then finds and ranks duplication on top of it. The IL is **not** the
deliverable — it's the substrate.

The long-term boundary for language and library meaning is the
[semantic-kernel](semantic-kernel.md): a pack-based semantic contract layer that
makes evaluation strategy, effects, library APIs, laws, and proof status explicit
instead of scattering semantic assumptions across the engine. The first internal
facade is in `nose-semantics`; the pipeline below describes the current mixed
state while migration continues.

## North star

nose's exact Type-4 goal is **not** to guess arbitrary semantic similarity. It is
to become the strongest detector for the semantic equivalence classes it explicitly
models: broad cross-language coverage, exact fingerprint equality, and a defensible
soundness contract for every accepted semantic match.

That means recall work should raise more real equivalences to exact convergence
rather than lowering thresholds around partial similarity. A new semantic match is
only a durable win when it can be backed by the independent interpreter oracle,
counterexamples for rejected rewrites, and, for core canonicalizations, machine-checked
proofs. False merges are product bugs; fuzziness belongs in candidate generation and
review-oriented `near` scoring, not in the exact `semantic` channel.

## The pipeline

```
source ──tree-sitter──▶ raw IL ──normalize──▶ canonical IL ──▶ units + features
                                                                      │
                                       MinHash + LSH candidate gen ◀──┘
                                                  │
                          structural + value-graph scoring ──▶ clusters ──▶ ranked families
```

1. **Lower** ([languages](languages.md)): tree-sitter parses each file; a per-language pass
   walks the CST and emits raw IL using a small, desugared core node set. Every
   node copies its source span, so every match traces back to exact lines.
   Frontends also emit semantic evidence records when that core IL would
   otherwise erase exact source, domain, import, symbol, type, guard, place/effect,
   library API, or sequence-surface distinctions needed by semantic contracts,
   then tag syntactic unit boundaries (function/method/class/block), which gives
   detection accurate boundaries for free.
2. **Normalize** ([normalization](normalization.md)): a fixed sequence of passes canonicalizes
   the IL — desugaring (with idiom canonicalization), alpha-renaming, an oracle cutoff,
   recursion-to-iteration normalization, dataflow propagation, control-flow normalization,
   algebraic and operator canonicalization, and a hash-consed **value graph** (GVN) that
   captures *what the code computes*, invariant to temporaries, statement order, and common
   subexpressions. See [normalization](normalization.md) for the exact pass order.
3. **Extract units & features**: frontend units are augmented with bounded
   sub-function units: control-flow blocks (`loop` / statement `if` / `try`) and
   exact-safe statement fragments whose whole value subtree stays inside the reported
   source span. Exact fragments carry a first-class classification, contract, and
   behavior oracle — see [fragment-contracts](fragment-contracts.md). Each unit becomes a
   multiset of subtree-shape hashes, a value-graph fingerprint, a pre-order linearization
   for alignment, a MinHash signature, plus literal- and return-value multisets used by
   the strict precision gates.
4. **Candidate generation**: the selected scan channels decide which candidates exist.
   `semantic` uses value-fingerprint MinHash signatures plus exact-value buckets, `near`
   uses shape MinHash signatures, experimental `abstraction` reuses the near candidate
   stream, and `syntax` bypasses unit LSH with a Rabin-Karp token-stream pass.
5. **Accept / score**: `semantic` accepts only exact-safe value-fingerprint equality, `near`
   scores candidates with structural alignment (RANSAC) plus weighted shape/value
   Jaccard and accepts above the inline `near:T` threshold (default `near:0.70`), and
   `syntax` emits duplicated runs above the line/token floors. Experimental
   `abstraction` then checks same-language near-style families for one shared
   supported literal-leaf hole position and attaches a weak witness instead of an
   exact claim.
6. **Cluster & rank**: union-find over accepted pairs/runs forms clone groups, which
   are grouped into **families** and sorted by refactoring value (removable lines
   × similarity × cross-module/-file/-language spread). See [usage](usage.md) for how the
   ranked report reads.

## Crates

A Cargo workspace; data flows left-to-right through them.

| crate | role |
|---|---|
| `nose-il` | arena IL model (`Vec<Node>`, `NodeId(u32)`, out-of-line edges), provenance spans, semantic evidence records, interner, serialization, IR verifier |
| `nose-semantics` | first-party semantic facade: language profiles, evidence/source-fact helpers, type-domain contracts, effect/operator/module/stdlib predicates, API contracts, and exact-channel proof obligations |
| `nose-frontend` | tree-sitter parse + per-language CST→IL lowering and first-party evidence emission (one module per language; embedded `<script>` extraction) |
| `nose-normalize` | the normalization passes, inferred immutable binding-domain evidence, and the value graph (GVN) |
| `nose-detect` | unit/feature extraction, MinHash/LSH, scoring, clustering, refactor ranking |
| `nose-eval` | benchmark scoring (precision/recall, pooled, stratified) — see [benchmark](benchmark.md) |
| `nose-cli` | the `nose` binary, config loading, baselines, caching |

The current semantic assumptions these crates share are summarized in
[semantic-kernel-snapshot](semantic-kernel-snapshot.md).

## Design choices worth knowing

- **Arena, not boxed trees.** The IL is a flat `Vec<Node>` with `NodeId(u32)`
  indices and out-of-line child edges — cache-friendly and cheap to serialize,
  which is what makes per-file feature caching ([continuous-integration](continuous-integration.md))
  possible.
- **Index-backed lookups on the arena.** Nodes are immutable once an `Il` is
  built (passes rebuild the arena), so `Il` carries lazy indexes — nearest
  enclosing scope, span → nodes, scope → assignments, and the evidence anchor
  index (span buckets, binding-hash buckets, id resolution). Per-node helpers
  must query these instead of scanning `il.nodes`/`il.evidence`; the raw scans
  were the dominant scan cost until they were indexed
  ([experiments §BQ](experiments.md)).
- **Interner-independent features.** A unit's features are content-derived
  hashes, not interner ids, so they're portable across runs — the basis for the
  content-hash cache.
- **Determinism is a hard invariant.** Output is byte-identical across runs *and*
  thread counts. File ids come from a sorted path list; nothing iterates a
  `HashMap` into the output. There are tests for both.
- **Parallel by default.** Discovery, lowering, and the detection stages run
  under rayon; the LSH stage is sort-based so it parallelizes cleanly. See
  [experiments](experiments.md) §T for the throughput work.
- **The behavioral fingerprint is sound by intent.** The value graph's contract
  (§AJ) is *fingerprint-equal ⟹ behavior-equal* — two units sharing a value-graph
  fingerprint must compute the same thing; a *false merge* is a bug, not an accepted
  approximation. Two mechanisms enforce it. (1) A tree-walking **interpreter oracle**
  (`nose verify`) runs every interpretable unit on a battery of inputs and flags any
  fingerprint-equal pair whose behavior differs. Crucially it interprets the
  *pre-canonicalization* core IL, not the fully-normalized IL it fingerprints, so a
  behavior-changing canonicalization cannot mask itself (§AX). (2) A **canon-preservation**
  check requires each unit's core-IL behavior to equal its full-IL behavior — catching a
  bad canon even with no colliding twin. The core algebraic/control canonicalizations,
  recursion templates, IL arena invariants, fragment contracts, and oracle cutoff are
  additionally **machine-checked in Lean** ([formal-soundness](formal-soundness.md)).
  Both checks currently report zero violations; the fuzziness a clone detector needs lives
  in the *candidate* axis and its scoring, never in the behavioral base (the two-axis
  principle, §AH).

For *why* the normalization passes look the way they do, read [normalization](normalization.md).
