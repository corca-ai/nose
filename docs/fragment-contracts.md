# Exact fragment contracts

How nose models **exact sub-function semantic fragments** — the return/throw/effect
shapes it lifts out of larger function bodies — as a shared substrate rather than an
accumulating set of ad-hoc predicates. Back to [architecture](architecture.md); the
benchmark that drives the frontier is [type4-benchmark](type4-benchmark.md).

## Why a substrate

A whole function is the natural unit, but many real Type-4 clones are a *region* inside a
larger body: a guarded `return`, an append loop, a `this.field` write. nose extracts these
as **exact fragments** (see the extract step in [architecture](architecture.md)) under a
hard rule — a fragment is admitted only when its behavior is self-contained and provable,
so opaque surrounding code can never smuggle in unproven semantics.

Historically each admissible shape was a standalone boolean predicate in
`nose-detect/src/units.rs`. That found sound cases but trended toward a case matrix: the
*reason* a fragment was accepted lived only in whichever predicate matched it, and nothing
verified a fragment's behavior the way [`nose verify`](architecture.md) verifies whole
functions. The substrate replaces the implicit predicate web with three explicit pieces.

## The three pieces

### 1. Classification — `FragmentKind`

Every accepted fragment carries a `FragmentKind` (direct return/throw, index-assign effect,
self-field assign, expression effect, conditional guard, loop effect, self-field body) and a
stable kebab-case **reason code** (`exact-direct-return`, `exact-loop-effect`, …). The reason
code names the exact proof shape that made the fragment safe; it is not the broader
family/actionability reason code tracked in issue #11. Reporting and ranking can separate
proof fragments from actionable refactors without re-reading the recognizer. The recognizer
returns a `FragmentKind` instead of a bare `bool`; `ProofFacts` records what was established
at acceptance time (context-safety, exit).

### 2. The contract — `FragmentContract`

A recognizer-independent description of one fragment: its free **inputs** (the canonical ids
it reads), its **exit** (normal / return / throw), its observable **effect**, and the write
**place** for heap-mutating shapes. Two fragments with the same contract are interchangeable
to the oracle regardless of which predicate matched them.

The contract carries only what the oracle needs to build a runnable wrapper. That is the
forcing function: **a contract that cannot be lowered into a wrapper is underspecified**, so
the model cannot drift into describing fragments the oracle can't vouch for.

### 3. The oracle — wrapper synthesis

A fragment is verified through the *same* independent behavior check as a whole function. We
do **not** add a new interpreter path. Instead the contract is lowered into a synthetic
single-function IL — free inputs become parameters, the fragment subtree is deep-copied into
the body — and handed to the existing [`run_unit`](architecture.md) interpreter. We reuse
its `Behavior` (returned value + ordered effect trace + final field state) and the same input
battery whole functions use.

Because `Behavior` already records effects and field state, **effects are preserved as
observable behavior for free**: appending to a parameter list shows up in the effect trace,
so two append fragments that append different values diverge without any special handling.

## The effect algebra

The substrate names *how* each effect is observed, because that determines its soundness
obligation — it does not treat every mutation as append-like:

| effect | observed as | receiver identity |
|---|---|---|
| `Append` | ordered effect trace (the appended values, in order) | not required |
| `IndexWrite` | ordered effect trace (key then value) | not required |
| `FieldWrite` | field-state map, keyed by **field name only** | **required (proven place)** |
| `Other` | established by running the fragment | — |

A field write is the one case where the interpreter does **not** observe the receiver (the
field-state map is keyed by name), so a field write is exact-safe only when its receiver is a
proven place. This is exactly why a fixed `this` is the only admitted field receiver — not a
special case, but a consequence of the algebra.

## Place — receiver identity, fail-closed

A write target resolves to a `Place`: `This`, `Param(cid)`, `LocalAlloc(id)`, a `Field` or
`Index` path over another place, or `Unknown`. The cardinal rule is that **`Unknown` is the
default**: any receiver that does not resolve to a proven place is `Unknown`, and a write that
*requires* a proven place (a field write) through an `Unknown` receiver is rejected, never
merged. An index write whose base is unproven (e.g. a Java instance field) still resolves —
to `Index(Unknown, …)` — and stays safe, because an index write is observable in the effect
trace and so carries no receiver-identity obligation.

This folds the former Java `this.field` special case into the general model: `this.x = v`
resolves to `Field(This, …)`, and the recognizer asserts the place is exact-safe.

## The differential gate

The recognizers are migrated onto the contract path one family at a time. An independent
contract recognizer (`fragment::recognize`) re-expresses each migrated shape, reusing only the
shared invalidation gates (span containment + context safety) rather than the per-shape
predicates. A test asserts that over a multi-language corpus the predicate path and the
contract path accept **exactly the same `(span, kind)` set** for migrated kinds, and a
`debug_assert` in the collector cross-checks the forward direction on every accepted fragment
across the whole fixture corpus. A migration step that changes which fragments are accepted
fails the gate — that is what keeps the re-expression behavior-invariant.

## What stays closed

In keeping with the exact-semantic goal, the substrate does not open new ground by itself:

- no arbitrary statement windows — only shapes with explicit input/exit/effect contracts;
- no dynamic-receiver semantics inferred from method names;
- no overloadable operations or library-purity assumptions without proof facts;
- map/set/object-update effect families stay closed until the substrate proves them sound.

The point is a clear model that makes the *next* sound family cheap to add, not more
proof-fragment noise.
