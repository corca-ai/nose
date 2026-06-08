# Exact fragment contracts

How nose models **exact sub-function semantic fragments** — the return/throw/effect
shapes it lifts out of larger function bodies — as a shared substrate rather than an
accumulating set of ad-hoc predicates. This extends the extraction step in
[architecture](architecture.md); the benchmark that drives the frontier is
[type4-benchmark](type4-benchmark.md).

## Why a substrate

A whole function is the natural unit, but many real Type-4 clones are a *region* inside a
larger body: a guarded `return`, an append loop, a `this.field` write. nose extracts these
as **exact fragments** (see the extract step in [architecture](architecture.md)) under a
hard rule — a fragment is admitted only when its behavior is self-contained and provable,
so opaque surrounding code can never smuggle in unproven semantics.

Historically each admissible shape was a standalone boolean predicate in
`nose-detect/src/units.rs`. That found sound cases but trended toward a case matrix: the
*reason* a fragment was accepted lived only in whichever predicate matched it, and nothing
verified a fragment's behavior the way [nose verify](benchmark.md#soundness--the-behavioral-oracle)
verifies whole functions. The substrate replaces the implicit predicate web with three
explicit pieces.

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
it reads), its **exit** (normal / return / throw), and its **effects** — an *ordered sequence*
of observable effects, each paired with the write **place** for the receiver-bearing ones. The
sequence is empty for a pure value/control sink (a direct return/throw), one entry for a
single-statement write, and several — in execution order — for a multi-statement body (a
conditional branch, a loop body, an ordered effect sequence). Two fragments with the same
contract are interchangeable to the oracle regardless of which predicate matched them.

The contract carries only what the oracle needs to build a runnable wrapper. That is the
forcing function: **a contract that cannot be lowered into a wrapper is underspecified**, so
the model cannot drift into describing fragments the oracle can't vouch for. The
`writes_proven` check is the fail-closed gate over the effect sequence: every field write must
resolve to a proven [`Place`]. Append and index writes carry no field-place obligation after
they have already been classified by their own proof gates.

### 3. The oracle — wrapper synthesis

The contract path can verify a fragment through the *same* independent behavior machinery as
a whole function. It does **not** add a new interpreter path. Instead the contract is lowered
into a synthetic single-function IL — free inputs become parameters, the fragment subtree is
deep-copied into the body — and handed to the existing
[interpreter](../crates/nose-normalize/src/interp.rs). The production scan still uses the
predicate path described below; the contract path is kept in lockstep by differential tests
and proof obligations.

Wrapper synthesis preserves the copied nodes' original spans and carries the source IL's
evidence graph into the wrapper. This is required for semantic-kernel admission: canonical
builtins, append effects, source facts, and library API occurrences remain executable only
when their original evidence records and dependencies are still asserted. The wrapper does
not mint new semantic facts from selector names or payload tags.

Because `Behavior` already records effects and field state, **effects are preserved as
observable behavior for free**: appending to a parameter list shows up in the effect trace,
so two append fragments that append different values diverge without any special handling. A
multi-statement body is lowered by splicing its statements into the wrapper body in order, so
an ordered effect sequence (e.g. two appends) is observed in order — swapping them diverges.

The wrapper's parameters are the fragment's **free inputs**, computed binding-aware: a cid
read from the enclosing scope is an input, but one *bound within* the fragment — a local
assigned before use, a `for-each` loop variable, a nested lambda parameter — is not, because
the interpreter binds it as the wrapper runs. (The binding model mirrors alpha-renaming's.)
This is what lets loop- and temp-bearing bodies be modeled at all: treating a loop variable as
a parameter would inflate the arity and feed it a battery value the loop immediately
overwrites. Omitting a genuine input only ever under-reports — an unbound read makes the
wrapper uninterpretable (fail-closed), never a false merge.

## The effect algebra

The substrate names *how* each effect is observed, because that determines its soundness
obligation — it does not treat every mutation as append-like:

| effect | observed as | receiver identity |
|---|---|---|
| `Append` | ordered effect trace (the appended values, in order) | not required |
| `IndexWrite` | ordered effect trace (key then value) | not required |
| `FieldWrite` | final field-state map, keyed by **receiver+field place** | **required (proven place)** |
| `Other` | established by running the fragment | — |

The `Append` row is deliberately about receiver *place* identity after an append
has been proven. It is not permission to infer append semantics from a method
name. Exact append fragments consume canonical `Builtin::Append` evidence:
frontends and normalizers must first prove the language/library receiver or
active-builder contract for the specific surface (`Array.push`, Python
`list.append`, Java builder `add`, Rust builder `push`, etc.) and lower it to
that canonical form, or a pack/frontend must attach `Effect(BuilderAppendCall)`
to the call. A raw selector-only call can still be compared under the separate
opaque-call policy as `Other`, but it does not prove append semantics.

A field write is the one case whose final-state slot is receiver-bearing, so a field write is
exact-safe only when its receiver resolves to a proven place. This is exactly why a fixed
`this` is the only admitted field receiver — not a special case, but a consequence of the
algebra. The boundary is registered as
[detect.fragment.effect_place](../formal/obligations/detect/fragment/effect_place/Proof.lean);
free-input extraction and wrapper synthesis are tracked by
[detect.fragment.free_inputs](../formal/obligations/detect/fragment/free_inputs/Proof.lean)
and
[detect.fragment.wrapper_synthesis](../formal/obligations/detect/fragment/wrapper_synthesis/Proof.lean).

## Place — receiver identity, fail-closed

A receiver-bearing write resolves its target to a `Place`: `This`, `Param(cid)`,
`LocalAlloc(id)`, a `Field` or `Index` path over another place, or `Unknown`. The cardinal
rule is that **`Unknown` is the default**: any receiver that does not resolve to a proven place
is `Unknown`, and a field write through an `Unknown` receiver is rejected, never merged. A
`Place` is recorded on the contract *only* for effects that need this proof — i.e. field
writes. An index write carries no place at all: it is observable in the effect trace (key and
value are recorded), so its receiver identity is irrelevant to soundness, and conflating its
target into the contract would mix proof with diagnostics.

This folds the former Java `this.field` special case into the general model: `this.x = v`
resolves to `Field(This, …)`, and the recognizer asserts the place is exact-safe.

## The differential gate

The recognizers are migrated onto the contract path one family at a time. An independent
contract recognizer (`fragment::recognize`) re-expresses each migrated shape, reusing only the
shared invalidation gates (span containment + context safety) rather than the per-shape
predicates. Tests assert that over representative multi-language snippets the predicate path
and the contract path accept **exactly the same `(span, kind)` set** for migrated kinds, and a
`debug_assert` in the collector cross-checks the forward direction on every accepted fragment
while collecting units. A migration step that changes which fragments are accepted fails the
gate — that is what keeps the re-expression behavior-invariant.

## Output surfaces

Exactness is not the same as refactorability. A fragment can be a proven semantic match and
still be poor default output: one-line guards, common assertions, fixture setup, or tiny
effect snippets are often better as review context or diagnostic evidence than as top-level
refactoring candidates.

`scan` and `review` therefore keep two facts separate:

- fragment proof metadata (`is_fragment`, `fragment_kind`, `reason_code`, span size, and
  `enclosing_unit` when recoverable) explains why the sub-function region is exact-safe;
- family placement (`recommended_surface`) says whether the finding belongs on the default
  action-oriented surface, the review-hazard surface, or hidden/debug output.

The default human, Markdown, SARIF, and `--fail-on` scan surfaces show action-oriented
families. Full scan JSON keeps diagnostic fragment families available for tooling and audits;
see [scan-json](scan-json.md#fragment-metadata). `nose review` uses review-surface fragments
when changed-line context makes a small exact region useful as an un-propagated-change hint.

## Migrated kinds

The predicate path in `units.rs` is still the **production authority**: it decides which
fragments are accepted. The contract path is an independent shadow recognizer, kept in lockstep
by the differential gate above. Migrating a kind means re-expressing it on the contract path
and adding it to that gate — it does **not** change production output.

| `FragmentKind` | contract-migrated? |
|---|---|
| `DirectReturn`, `DirectThrow` | yes — value/control sinks |
| `IndexAssignEffect`, `SelfFieldAssign`, `ExprEffect` | yes — single-effect writes |
| `LoopEffect` | yes — for-each iteration-dependent effect body |
| `SelfFieldBody` | yes — fixed self-field-write body |
| `ConditionalGuard` | yes — recursive branch admissibility matrix |

`LoopEffect` is the first multi-statement-body migration: its independent recognizer lives in
`fragment/loop_effect.rs` and re-expresses the for-each acceptance (an iteration-dependent
append/index effect, possibly through one or two local temps or nested `if` branches) on the
binding-aware free-input + multi-statement-lowering substrate, with no reuse of the predicate's
acceptance helpers.

`SelfFieldBody` is the only migrated shape whose acceptance boundary intentionally bypasses
the shared top-level context gate: it proves self-containment through fixed self-field
writes, allows conditional self-field statements, and permits a terminal `return this` in
the current first-party Java compatibility path. The evidence path requires matching
`Effect(SelfFieldWrite)` plus `Place(SelfField)`/`Place(SelfReceiver)` proof. That bypass is
scoped to this kind only.

`ConditionalGuard` re-expresses the full recursive branch admissibility matrix on the
contract path: empty branches, direct return/throw/effect branches, branch-local temp
consumption, bounded ordered effect sequences, loop-effect branches, conditional direct-effect
branches, and nested conditional guards. After that migration base, `ConditionalGuard` also
admits a narrow ordered self-field branch body: exactly two or three fixed self-field
assignments. The invariant is the same as `SelfFieldAssign` and `SelfFieldBody`: every field
write resolves to a proven `Place::This` field path, and the oracle observes the final
field-state map. A sibling branch containing `other.field = ...` or an implicit field/local
assignment stays rejected because receiver identity is not proven.
The interim "branch-local temp", "ordered multi-effect", and ordered self-field branch
shapes remain proof mechanisms *inside* existing kinds, not new `FragmentKind` variants or
reason codes.

## What stays closed

In keeping with the exact-semantic goal, the substrate does not open new ground by itself:

- no arbitrary statement windows — only shapes with explicit input/exit/effect contracts;
- no dynamic-receiver semantics inferred from method names;
- no overloadable operations or library-purity assumptions without proof facts;
- map/set/object-update effect families stay closed until the substrate proves them sound.

The point is a clear model that makes the *next* sound family cheap to add, not more
proof-fragment noise.
