# Type-4 frontier target packets

Implementation-ready selections from the corpus-balanced frontier evidence platform.
Each packet LINKS human-verified `real_frontier.v1.json` evidence (it never restates a
status) and adds team routing. See [frontier-platform](../../docs/frontier-platform.md).

- build ref: `None` · union signature `fec264f35c3f1ded…`
- corpus: 120 repos · commit digest `2bf0b8c147be66b7…`
- owner routes: proof-fact-prerequisite, team-a-detector, team-c-product
- packets: 3

## `numeric-clamp-2026-06-06` — axis `numeric_clamp`

- **owner route**: `proof-fact-prerequisite` (no team yet) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 22% · primary-language 88% (7/8) · dev 14 · held-out 12 · both-splits
- **semantic claim**: min(max(x,lo),hi), max(min(x,hi),lo), and (x<lo ? lo : (x>hi ? hi : x)) all denote the same clamp for a totally-ordered numeric domain with lo <= hi. The boltons `clamp` and the fzf `Constrain` are the two canonical min/max compositions, in different languages, and should converge.
- **proof invariant**: Recognize clamp as min(max(x,lo),hi) = max(min(x,hi),lo) = two-comparison form ONLY with proven scalar min/max facts and a lo <= hi precondition; reject swapped bound order min(max(x,hi),lo), wrong nesting max(min(x,lo),hi), the lo>hi precondition violation, and float NaN (where min/max builtins vs comparison chains can diverge by language). Machine-checked in formal/obligations/normalize/value_graph/clamp/Proof.lean.
- **hard negatives**:
  - swapped bound order min(max(x, hi), lo) -- clamp Counterexamples.lean swapped_bounds_not_clamp
  - wrong nesting max(min(x, lo), hi) -- clamp Counterexamples.lean wrong_nesting_not_clamp
  - lo > hi precondition violation: the two compositions diverge -- clamp Counterexamples.lean precondition_required
  - float NaN inputs where min/max builtins and comparison chains can return different values depending on language NaN semantics
- **evidence**: `numeric-clamp-minmax-ternary-real-miss` (`real_frontier.v1.json`)
- **real frontier replay**: `numeric-clamp-boltons-fzf-real-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `boltons` (heldout, Python) `boltons/mathutils.py:40-69`
  - `fzf` (heldout, Go) `src/util/util.go:63-65`
- **current detector result**: miss=True · `nose 0.5.0` @ `58c4c9b0c513` — Historic controlled query: abs control merged (1 family: absTern, absBuilt); clamp ternary/library bridge forms did not. Current focused equivalence/adversarial cases now cover the proof-backed controlled bridge forms.
- **detector admission**: `controlled-slice-admitted` · proof-backed controlled integer clamp surfaces
- **remaining real-pair gap**: the boltons/fzf real-corpus pair still lacks fzf-side bound-order evidence and shared integer-only domain evidence, so it remains a real miss
- **why now**: A genuine machine-checked semantic under-merge (formal/obligations/normalize/value_graph/clamp/Proof.lean) that is broad and generalizing — present in 7 of the 8 corpus primary-language buckets, with hits in both the dev and held-out splits. The proof-backed min/max plus controlled two-comparison/library bridge slices are implemented; the remaining value is identifying the next real-corpus bound-order proof without weakening the hard-negative boundary.
- **blocked by**: the current fzf member has no modeled bound-order evidence; parameter naming such as `Constrain(val, minimum, maximum)` is not a proof, the current boltons/fzf pair has no shared integer-only domain proof; Python dynamic parameters and Go `cmp.Ordered` remain float/NaN-sensitive boundaries
- **notes**: The proof-backed integer Clamp canon now covers min/max composition plus controlled two-comparison and library method bridge surfaces when literal or asserted Guard(BoundOrder) evidence proves lo<=hi and integer-domain evidence excludes float/NaN behavior. The remaining packet is still routed proof-fact-prerequisite because the real fzf member lacks modeled order evidence and the cross-language pair lacks a shared integer-only domain proof.

## `python-loop-demorgan-all-2026-07-07` — axis `python_loop_demorgan_all`

- **owner route**: `team-a-detector` (#739) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `fragment-contract`
- **breadth**: repo 7% · primary-language 25% (2/8) · dev 4 · held-out 4 · both-splits
- **semantic claim**: Under the packet's proof conditions, a Python `all(x != 0 and x != 1 for x in xs)` universal predicate is equivalent to an early-return loop that returns False when `x == 0 or x == 1` and True after exhausting `xs`. The loop searches for a counterexample; for pure scalar comparisons where `==` and `!=` are complementary, De Morgan rewrites `not (x == 0 or x == 1)` to `x != 0 and x != 1`, so both forms accept exactly the same elements and both are true for an empty iterable.
- **proof invariant**: Open the equivalence only when the loop is a pure universal counterexample scan over the same iterable: the only loop exit returns literal False on `not P(x)`, fallthrough returns literal True, empty iterables preserve vacuous truth, and the all(...) generator evaluates the same pure boolean predicate in the same order. The De Morgan step is allowed only for proven boolean operands such as comparisons; Python value-returning `and`/`or`, predicate side effects, helper calls without a separate purity proof, overloaded comparisons, changed predicates, and changed empty-iterable results must remain non-equivalent.
- **hard negatives**:
  - vacuous-truth boundary: a loop that returns False after exhausting an empty iterable is not all(...)
  - predicate side effects or iterator mutation before returning, where short-circuit timing is observable
  - predicate helper calls without a separate purity proof, where effects can be hidden behind a name
  - Python value-returning boolean expressions whose operand payload is returned or observed rather than used only as a truth test
  - changed predicates such as `x != 0 or x != 1`, which is almost always true and not equivalent to excluding 0 and 1
  - different iterable identity: all(...) over xs is not equivalent to an early-return loop over ys
- **evidence**: `python-loop-demorgan-all-readme-real-miss` (`real_frontier.v1.json`)
- **real frontier replay**: `python-loop-demorgan-readme-focused-real-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (docs, Python) `README.md:15-33`
  - `nose` (focused, Python) `bench/type4/adversarial/cases/python_loop_demorgan/positive.py:1-9`
- **current detector result**: miss=False · `nose 0.18.0` @ `#739 detecto` — Semantic query reports one family containing the README all(...) function and early-return loop.
- **detector admission**: `real-pair-admitted` · README/focused Python all(generator) universal predicate versus counterexample early-return loop with boolean-only literal comparison De Morgan
- **remaining real-pair gap**: none
- **why now**: The front-page README uses this same-language Type-4 example to explain semantic duplication. The proof facts are now modeled-controlled, and the detector admits the README/focused positive while the adjacent hard-negative boundary remains executable.
- **blocked by**: nothing
- **notes**: This packet deliberately corrects the README-facing example from prose-only claim to auditable frontier evidence. The exact-admission request is now fulfilled for the README/focused pair, and the hard negatives document the proof perimeter.

## `membership-contains-2026-07-08` — axis `membership_contains`

- **owner route**: `team-a-detector` (#754) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 88% · primary-language 100% (8/8) · dev 56 · held-out 49 · both-splits
- **semantic claim**: A proven literal collection membership predicate and a proven standard Set membership predicate are equivalent when they share the same searched element and collection/source coordinates and the receiver is not mutated.
- **proof invariant**: Open collection-membership convergence only when source evidence proves the receiver is a collection-membership domain, the searched element coordinate is identical, the collection/source coordinate is identical across literals/factories/imports/typed receivers, and no mutation changes the receiver before membership. Substring/regex contains, map-key or value membership, raw index/count payloads, loose equality, NaN-sensitive equality, missing imports, shadowed constructors, custom contains receivers, and mutated providers/importers must remain non-equivalent.
- **hard negatives**:
  - wrong searched element: contains(value) is not contains(other)
  - wrong collection/source coordinate: ["red", "blue"] is not ["green", "blue"]
  - substring or regex contains is not element membership
  - map-key membership and value search remain distinct semantic families
  - raw indexOf/findIndex/filter length payloads are not boolean membership predicates unless compared to the correct sentinel
  - missing imports, shadowed constructors, untyped dynamic has receivers, and custom contains methods are not API/domain proof
  - provider-side or importer-side mutation after construction/import changes the receiver state
- **evidence**: `collection-membership-focused-controlled` (`real_frontier.v1.json`)
- **real frontier replay**: `collection-membership-focused-controlled-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (focused, Python) `bench/type4/adversarial/cases/collection_membership/positive.py:1-2`
  - `nose` (focused, JavaScript) `bench/type4/adversarial/cases/collection_membership/positive.js:1-3`
- **current detector result**: miss=False · `nose` @ `#754 semanti` — Semantic query reports a family containing py_literal_member and jsSetMember.
- **detector admission**: `real-pair-admitted` · controlled literal, factory-backed, imported immutable, typed dynamic, and probe collection membership surfaces
- **remaining real-pair gap**: none
- **why now**: membership_contains is the top breadth frontier axis and already has multi-language controlled coverage. The remaining value is to preserve the receiver/element/collection/mutation proof perimeter as reusable neutral facts before future contains/has/include expansions add more language surfaces.
- **blocked by**: nothing
- **notes**: This packet records the current controlled membership perimeter as reusable proof facts. The real-corpus EnumSet and single-argument Arrays.asList leads remain guarded by their unsupported evidence records and must not be used to widen exact admission without missing enum/array source facts.
