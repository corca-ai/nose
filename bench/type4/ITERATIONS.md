# Type-4 coverage iteration log

This document is a compact decision log for the Type-4 benchmark and detector
co-evolution work. It keeps the important facts from the longer iteration history:
what was selected, what invariant was proven, what changed, which gates mattered, and
what remains open. The full per-commit detail is recoverable from git history; this file
should stay short enough to read during the next batch.

## Operating Rules

- Completeness means evidence-backed Type-4 misses from the pinned real corpus, not only
  the synthetic pair-weighted ratio.
- Soundness wins over recall. A batch that removes false merges is successful even if the
  reported completeness number falls.
- Each batch should share one proof invariant or one semantic axis.
- Exact fragment expansion is allowed only when the fragment has a clear semantic boundary:
  self-contained span, no unproven mutation/alias/receiver effects, `exact_safe`, and a
  large enough value fingerprint.
- Every accepted positive needs adjacent hard negatives. Hard-negative false merges must
  remain zero.
- Performance is checked with `NOSE_TIME=1` on selected real repos and with compact
  all-cross smoke before a batch is considered done.

## Initial Synthetic Closure

The first seed benchmark covered aggregate/predicate Type-4 proposals such as filtered
sum/count/product and any/all predicates. The initial default ring smoke reported:

```text
items: 429
positive recall: 156/286
hard-negative false merges: 0/143
```

Detector co-evolution loops then closed that original ring frontier by adding shared
value-graph support for filtered reductions, count filters, Java streams, Ruby predicate
reductions, Rust early returns, Python `math.prod`, and C pointer-length loop contracts.
The closed baseline was:

```text
default ring: positive recall 286/286, false merges 0/143
same-surface: positive recall 143/143, false merges 0/143
all-cross hardening: positive recall 858/858, false merges 0/143
```

The key lesson from this phase was that synthetic pressure is useful when it is paired
with adversarial siblings and regression gates. Synthetic ratio alone is not a product
metric.

## Synthetic Axis Milestones

Later synthetic work widened the matrix in bounded slices. The important outcomes are
summarized here rather than repeated as per-loop transcripts.

| area | accepted invariant | important boundary |
|---|---|---|
| selection reductions | clamped `min`/`max` folds over proven numeric inputs | wrong extremum and boundary values stay distinct |
| map/contribution reductions | mapped `sum`/`product` contributions such as `x*x` | wrong contribution and predicate changes stay distinct |
| zip/dot-product | paired same-index contributions under bounded traversal | shifted indexes and unequal traversal contracts stay distinct |
| literal membership | literal/set/map membership with proven static collections | untyped receiver and custom collection semantics stay unsupported |
| map defaults | proven literal/typed map defaults | custom receiver defaults stay unsupported |
| record-shape guard | strict JS/TS not-array proof shapes | shadowed `Array` remains a hard negative |
| flag/break reductions | simple boolean flag loops equivalent to `any`/`all` | wrong early-exit predicate remains distinct |
| equality OR-chain | same candidate against the same static literal set | wrong literal set remains distinct |
| ordered string builder | Python literal-separator join from per-element appends | prepend or non-element builders stay unsupported |

Representative compact gate after these slices:

```text
GATE=core CROSS=all NOSE=target/release/nose scripts/type4-smoke.sh
positive recall: 613/613
hard-negative false merges: 0/1201
```

## Real-Corpus Frontier Batches

These are the batches that best match the practical completeness goal. They were selected
from pinned real repos, with a concrete semantic claim and adjacent hard negatives recorded
in `bench/type4/real_frontier.v1.json`.

| batch | corpus evidence | invariant | result |
|---|---|---|---|
| Python docstring no-op | SymPy behavior-equal helpers split by leading static docstrings | a leading static function/class docstring is metadata, not callable behavior | selected verify 66/70 to 70/70; 7 evidence-backed families added |
| JS/TS namespace import shadowing | Jest `replaceRootDirInPath` duplicate helpers | parameter shadowing does not mutate an imported namespace binding; static template fragments are behavior-defining | selected verify 0/1 to 1/1 |
| C total-order comparator | Vim `lnum_compare` split from `int_cmp`/`syn_compare_stub` | primitive non-overloadable order guards can absorb redundant same-direction non-strict guards | selected verify 1/3 to 3/3 |
| Java empty-domain soundness | Netty false merge across array, collection, and string emptiness | strict empty checks must retain receiver domain | false merges removed; completeness intentionally fell because bad merges split |
| Java statically-false loop | libgdx `BufferUtils.findFloats` overloads | proven-true local boolean makes left-hand `!local && ...` loop guard unreachable without evaluating the RHS | selected verify 0/1 to 1/1 |
| Java low-bit toggle | Graphhopper reverse-edge key helpers | Java primitive even/odd branch `x + 1`/`x - 1` equals `x ^ 1` without overflow at signed extremes | selected verify 0/1 to 1/1 |
| C u16 byte-pack | SQLite big-endian 16-bit decoders | proven byte buffer lanes 0 and 1 may combine by `+` or `|` after `<< 8` | selected verify 5/16 to 7/16; unsupported include typedef left open |
| Java `Arrays.asList(array)` | frontier boundary repeatedly identified by audit | single-argument `Arrays.asList` is element membership only for a proven Java array domain | generic `Arrays.asList(listParam)` remains a hard negative |
| Ruby literal `Hash#fetch` | real custom receivers were audited but unsupported | only literal/proven Hash receivers with pure fallback blocks get map-default behavior | custom `cookies`, `headers`, cache receivers remain unsupported |
| C include typedef u16 | SQLite local/direct include typedef byte aliases | `u8` aliases may prove byte-buffer lanes only when typedef provenance is explicit | include-blind guesses remain rejected |
| C u32 unsigned-cast byte-pack | SQLite cast-proven big-endian 32-bit decoders | four byte lanes are accepted only when the high lane has a proven unsigned 32-bit cast | uncasted high lane remains a hard negative |
| Four unsupported-lead audit (#32/#34) | Java EnumSet, Java class-literal API chain, Ruby custom fetch receivers, Java single-arg collection factories | abstain unless receiver/domain/API-chain proof is explicit; a single factory argument is not a one-element literal without proof | three leads remain unsupported; one latent `Arrays.asList`/`of` false merge was fixed; real-corpus family-set diff stayed 0 |
| Issue #36 frontier curation | Prioritizer rerun plus focused audits of SymPy, SQLAlchemy, Guava, RuboCop, ANTLR Python, and Alacritty Rust | priority hits, verifier leads, and implementation-ready Type-4 misses are separate evidence tiers | no same-invariant implementation batch is ready; one semantic-only SymPy lead is already covered by default output, and one Guava verifier lead is recorded as a hard negative |

Current frontier state in `real_frontier.v1.json`: 20 items: 14 closed, 4 unsupported,
1 already-covered, and 1 hard-negative. The unsupported items are intentionally parked
because they need stronger proof facts: Java EnumSet/method-level mismatch, Java
`Arrays.asList` provenance in one audited case, JUnit reflection/API-chain purity, and
Ruby custom receiver `fetch` semantics.

The Issue #36 audit used current `main` (`target/release/nose` 0.5.0 at
`0b57dd25a0e6cdb6f2742abad82585ca8c517e38`) and the pinned corpus from
`bench/repos`. `bench/type4/prioritize_frontier.py --repos-root /Users/ak/prjs/cc/nose/bench/repos`
was byte-identical to `FRONTIER_PRIORITIES.md`: `membership_contains` and
`map_default_lookup` remain the top queue signals, but both still show 100% broad-probe
coverage and no uncovered gap samples. Selected `nose verify` passes then separated
actual evidence from queue noise:

- SymPy and SQLAlchemy top Python repos still have verifier under-merges, but the strong
  SymPy `rot90` lead is already reported by default `syntax,semantic` output, and the
  SQLAlchemy leads are low-nearness oracle groups rather than a shared map-default or
  membership proof.
- Guava was audited instead of avoiding Java. Its `Sets.minSize`/`maxSize` verifier lead
  is a concrete hard negative because `SetView.minSize()` and `SetView.maxSize()` may
  differ, and whole-Guava verify was otherwise SOUND.
- RuboCop was SOUND but produced only low-nearness leads unrelated to the map-default
  receiver-provenance frontier already parked in `real_frontier.v1.json`.
- ANTLR Python `dict.get(..., None)` and Alacritty Rust `TermMode::VI` `contains` samples
  did not produce under-merged leads in selected scans. The Alacritty hits are dynamic
  bitflag receiver calls, so they remain queue noise unless a future proof fact can prove
  the receiver/domain and a concrete clone pair.

Conclusion for #36: do not force a three-item recommendation. The audited evidence does
not support a same-proof `real-miss` batch. The next useful handoff to #37 is to track
these audited repos/axes as performance/output-regression samples while detector work
continues only when a new candidate has a current semantic miss, a proof invariant, and an
adjacent hard negative.

## Soundness Hardening

Several batches were driven by false-merge or unsound-proof risk rather than recall.
These are important because they keep later completeness work credible.

| hardening | problem | fix |
|---|---|---|
| typed empty domains | Java array, collection, and string emptiness collapsed | value fingerprints are salted by proven receiver domain |
| ordered effects | `append(a); append(b)` merged with swapped order | statement-level effect sinks carry branch-aware ordinal tags |
| receiver/base evaluation order | field/index operations could ignore receiver/base errors | interpreter and value graph propagate receiver/base/subscript errors before reads or writes |
| try/throw behavior | bare throw and simple handlers were under-modeled | bare throws become observable `Err`; simple handlers run only under the supported boundary |
| static error propagation | builtin args, opaque call args, HOF lambdas, range steps, ternary branches, field/index writes had missing error paths | value graph mirrors the interpreter's strict order for these visible errors |
| field state | same-unit field writes and reads were too lossy | proven same-unit field writes can resolve later reads; unproven field reads stay unsupported |
| single-arg collection factories | unproven Java `Arrays.asList(x)`/`List.of(x)`/`Set.of(x)` could collapse array and non-array receivers | only proven Java array arguments retain array-membership semantics; ambiguous single arguments abstain |

Representative all-cross gate after ordered-effect hardening:

```text
positive recall: 629/629
hard-negative false merges: 0/1240
```

## Exact Fragment Units

Fragment expansion is the main recent non-real-corpus direction. The detector now extracts
some exact sub-function units that are not whole functions/methods/classes or broad control
blocks. These are deliberately narrow and proof-shaped.

| batch group | fragment shape | invariant |
|---|---|---|
| 1 | top-level exact statement fragments | a direct statement can be a unit only when its value graph is exact-safe and self-contained |
| 2-7 | return, throw, conditional exits, bare return, and expression-effect branches | guarded return/throw/effect sinks carry path guards and reject wrong guards, values, and preceding mutations |
| 8 | nested conditional branches | nesting is accepted only when each non-empty branch is itself an exact conditional/exit shape |
| 9 | non-overloadable index assignment effects | index target, index value, and assigned value are all fingerprint coordinates |
| 10-11 | ForEach append effects and conditional ForEach branches | each body effect is a single-item append whose value depends on the iteration binding |
| 12-14 | Java `this.field` assignment bodies and fluent `return this` builders | Java `this` is a fixed receiver; arbitrary receivers and implicit field/local ambiguity stay unsupported |
| 15-19 | branch-local temps, loop-local temps, temp chains, and temp-fed index effects | temps are accepted only when local, consumed by the exact effect, and not used as receivers or live-outs |
| 20-21 | ordered append/index branch fragments | exactly two ordered effects preserve execution order and reject swapped-order siblings |
| 22-23 | three append/index fragments | the same effect invariants extend to exactly three items, still capped and ordered |
| 24-29 | ordered foreach, mixed-effect, conditional, and loop-conditional branch fragments | only small bounded branch bodies with accepted direct effects are opened; arbitrary statement windows remain closed |

The useful product effect is that `semantic` can now find behaviorally exact sub-function
clones hidden inside larger functions, while avoiding arbitrary slice semantics. The risk is
that many new reports are 1-3 line proof fragments. Those are sound, but not always worth a
human refactoring pass unless ranking or views separate "semantic proof" from "actionable
refactor".

Representative recent fragment gates:

```text
fragment focused gates: expected positives found, adjacent hard negatives excluded
compact all-cross core: positive recall 634/634, hard-negative false merges 0/1246
full-corpus semantic scans: no unexplained candidate-path regression observed in the measured batches
```

## Performance Notes

Performance checks were mostly stable. Selected real-repo scans usually changed by only a
few milliseconds in normalize/extract or candidate paths. Two notes should remain visible:

- The C u32 unsigned-cast byte-pack batch saw SQLite candidate path move from 15.6ms to
  115.7ms in one measured run; it was called out to watch next batch rather than ignored.
- Exact fragment expansion can increase reported families without much scan-time cost, but
  output noise and ranking pressure need monitoring separately from raw runtime.

## Frontier Evidence Platform (issue #44)

Added `frontier_platform.py`, a presence-based companion to `prioritize_frontier.py` (left
byte-stable). It ranks the eight prevalence axes by repo/language breadth and dev→held-out
generalization across the pinned 105-repo corpus (dev 58 / held-out 47), keeps the regex
queue signal separate from human-verified `real_frontier.v1.json` evidence (read-only
cross-reference; no auto-finalized status), and carries curated controlled-vocabulary
fields (`implementation_cost` / `soundness_risk` / `substrate_required` / `evidence_tier`).
Output (`frontier_platform.v1.json` + markdown) is deterministic and records corpus commit
digest, candidate signature, build ref, and tool version.

Audit conclusion for the current axis set: **no implementation-ready batch.**

- Presence ranking refuses raw-count bias: `null_option_presence` has the largest raw
  occurrence (~126k) yet ranks below `membership_contains`, which spreads to more repos.
- The two highest-breadth axes (`membership_contains`, `collection_empty_check`) are already
  `frontier-recorded` (human evidence: unsupported / closed); `map_default_lookup` likewise.
  High prevalence is not next work — the #36 lesson, now visible via `evidence_tier`.
- All eight axes report 100% broad-probe coverage and zero uncovered forms (consistent with
  `prioritize_frontier`), so there is no uncovered-gap signal to promote.
- A future real-miss needs a NEW wide-breadth axis whose broad probe surfaces uncovered
  forms and whose equivalence a human can pin to a narrow proof invariant with a concrete
  hard-negative sibling, recorded in `real_frontier.v1.json` — not inferred from prevalence.

No new `real_frontier.v1.json` records or statuses were added: this pass produced no
human-proven miss, and forcing one is disallowed (#36 decision 4).

## Frontier Target Packets (issue #50, Team B)

Extended the frontier platform from queue-signal triage to **implementation-ready target
packets**. New corpus-driven axes now live in `frontier_axes.py` (`EXTRA_CANDIDATES`),
unioned in by `frontier_platform.py` while `prioritize_frontier.py` stays byte-stable; a
`union_signature` + `validate_union` guard the combined set separately from the #44
eight-axis conclusion.

The tool-assisted manual audit produced **one implementation-ready packet**: `numeric_clamp`.

- `min(max(x,lo),hi)` / `max(min(x,hi),lo)` / two-comparison clamp are equivalent for
  `lo ≤ hi`, but the value graph converges none of them — unlike the structurally-similar
  abs idiom, which it DOES canonicalize. Confirmed on the current binary: a controlled scan
  merges `absTern`/`absBuilt` but not `clampTern`/`clampBuilt`, and boltons `clamp` (min(max))
  and fzf `Constrain` (max(min)) do not merge across files.
- Broad and generalizing: present in 26 repos across all 7 primary languages on both splits
  (dev 14 / held-out 12). The identity and its hard negatives (swapped bound order, wrong
  nesting, the `lo ≤ hi` precondition) are machine-checked in `formal/Clamp.lean`.
- Recorded as a `real-miss` in `real_frontier.v1.json` (existing schema/status) and linked by
  a target packet routed to `team-a-detector` (#49) — a value-graph clamp canonicalization
  over already-proven scalar min/max facts (not blocked, not proof-fact-prerequisite). Team A
  owns the recognizer and soundness gates; the packet's contract ends at the proof invariant
  and target evidence.

## Current Next Work

- Continue the real-corpus frontier loop in small batches: pick one proof invariant,
  audit the top real repos, close only evidence-backed misses, and keep unsupported items
  explicit.
- Prefer non-Java or multi-language axes unless a Java lead clearly dominates on evidence
  and cost. The recent work already has a Java-heavy slice.
- Treat fragment expansion as a substrate for better product views, not as an end in
  itself. The next product question is how to surface exact fragments that are actionable
  while keeping tiny proof fragments from dominating the report.
- Keep unsupported receiver/API semantics unsupported until a frontend proof fact or shared
  value-graph invariant makes them sound.
