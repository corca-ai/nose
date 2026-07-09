# Type-4 frontier target packets

Implementation-ready selections from the corpus-balanced frontier evidence platform.
Each packet LINKS human-verified `real_frontier.v1.json` evidence (it never restates a
status) and adds team routing. See [frontier-platform](../../docs/frontier-platform.md).

- build ref: `None` · union signature `779a4975ba2b7b27…`
- corpus: 120 repos · commit digest `2bf0b8c147be66b7…`
- owner routes: proof-fact-prerequisite, team-a-detector, team-c-product
- packets: 7

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
- **current detector result (primary linked evidence)**: miss=True · `nose 0.5.0` @ `58c4c9b0c513` — Historic controlled query: abs control merged (1 family: absTern, absBuilt); clamp ternary/library bridge forms did not. Current focused equivalence/adversarial cases now cover the proof-backed controlled bridge forms.
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
- **current detector result (primary linked evidence)**: miss=False · `nose 0.18.0` @ `#739 detecto` — Semantic query reports one family containing the README all(...) function and early-return loop.
- **detector admission**: `real-pair-admitted` · README/focused Python all(generator) universal predicate versus counterexample early-return loop with boolean-only literal comparison De Morgan
- **remaining real-pair gap**: none
- **why now**: The front-page README uses this same-language Type-4 example to explain semantic duplication. The proof facts are now modeled-controlled, and the detector admits the README/focused positive while the adjacent hard-negative boundary remains executable.
- **blocked by**: nothing
- **notes**: This packet deliberately corrects the README-facing example from prose-only claim to auditable frontier evidence. The exact-admission request is now fulfilled for the README/focused pair, and the hard negatives document the proof perimeter.

## `membership-contains-2026-07-08` — axis `membership_contains`

- **owner route**: `team-a-detector` (#754) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 88% · primary-language 100% (8/8) · dev 56 · held-out 49 · both-splits
- **semantic claim**: A proven literal collection membership predicate, proven standard Set membership predicate, and focused Swift Array.contains predicate are equivalent when they share the same searched element and collection/source coordinates and the receiver is not mutated.
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
  - `nose` (focused, Swift) `bench/type4/adversarial/cases/collection_membership/positive.swift:1-4`
- **current detector result (primary linked evidence)**: miss=False · `nose` @ `#754 semanti` — Semantic query reports a family containing py_literal_member, jsSetMember, and swiftArrayMember.
- **detector admission**: `real-pair-admitted` · controlled literal, factory-backed, imported immutable, typed dynamic, and focused Swift collection membership surfaces
- **remaining real-pair gap**: none
- **why now**: membership_contains is the top breadth frontier axis and already has multi-language controlled coverage. The remaining value is to preserve the receiver/element/collection/mutation proof perimeter as reusable neutral facts before future contains/has/include expansions add more language surfaces.
- **blocked by**: nothing
- **notes**: This packet records the current controlled membership perimeter as reusable proof facts. The real-corpus EnumSet and single-argument Arrays.asList leads remain guarded by their unsupported evidence records and must not be used to widen exact admission without missing enum/array source facts.

## `collection-empty-check-2026-07-08` — axis `collection_empty_check`

- **owner route**: `team-a-detector` (#755/#780) · evidence tier: `frontier-recorded` · cost `low` · risk `low` · substrate `none`
- **breadth**: repo 86% · primary-language 100% (8/8) · dev 56 · held-out 47 · both-splits
- **semantic claim**: A proven collection length-zero predicate and a named empty predicate are equivalent across Rust and focused Swift Array surfaces when they read the same receiver coordinate, collection domain/kind, empty direction, and unmutated receiver state; the same holds for the explicitly negated non-empty direction.
- **proof invariant**: Open collection-empty convergence only when source evidence proves the receiver coordinate, compatible collection domain and kind, the empty versus non-empty boolean direction, and no intervening receiver mutation. Length-one/cardinality thresholds, raw length payloads, string/custom empty APIs, incompatible array/map domains, wrong receivers, and stale mutated receivers must remain non-equivalent.
- **hard negatives**:
  - length-one or greater-than-one thresholds are not strict emptiness
  - the same empty predicate over a different receiver parameter is not equivalent
  - Java array length, Java Collection.isEmpty, and String.isEmpty stay split without domain/kind proof
  - truthiness over unproven or value-returning domains cannot be folded into collection emptiness
  - mutation before the empty check changes the receiver state
- **evidence**: `collection-empty-focused-controlled`, `java-empty-domain-netty-array-queue-string` (`real_frontier.v1.json`)
- **real frontier replay**: `collection-empty-focused-controlled-pair`, `collection-nonempty-focused-controlled-pair`, `collection-empty-swift-focused-controlled-pair`, `collection-nonempty-swift-focused-controlled-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (focused, Rust) `bench/type4/adversarial/cases/collection_empty_check/positive.rs:1-15`
  - `nose` (focused, Swift) `bench/type4/adversarial/cases/collection_empty_check/positive.swift:1-15`
  - `netty` (dev, Java) `common/src/main/java/io/netty/util/concurrent/AbstractScheduledEventExecutor.java:147-149`
- **current detector result (primary linked evidence)**: miss=False · `nose` @ `#780 Swift e` — Semantic query reports the Rust length-zero/named-empty pair, the Swift count-zero/named-empty pair, and the Rust/Swift non-empty/negated-named pairs, while threshold, wrong-receiver, wrong-domain, and mutation fixtures stay outside those families.
- **detector admission**: `real-pair-admitted` · controlled length-zero, named-empty, and non-empty collection checks with receiver, domain/kind, direction, and mutation proof
- **remaining real-pair gap**: none
- **why now**: collection_empty_check has broad controlled coverage, focused Swift Array evidence, and a real Java domain-boundary soundness record. The remaining value is to preserve the receiver/domain/direction/mutation proof perimeter as reusable neutral facts before future empty?/isEmpty/len/size/truthiness expansions add more surfaces.
- **blocked by**: nothing
- **notes**: This packet records the current collection-empty perimeter as reusable proof facts. The Java Netty array/Queue/String record remains a hard-negative domain sibling; it must not be used to merge incompatible empty domains without explicit domain/kind proof.

## `string-prefix-suffix-2026-07-08` — axis `string_prefix_suffix`

- **owner route**: `team-a-detector` (#756) · evidence tier: `frontier-recorded` · cost `low` · risk `low` · substrate `none`
- **breadth**: repo 82% · primary-language 100% (8/8) · dev 52 · held-out 46 · both-splits
- **semantic claim**: Case-sensitive string prefix/suffix predicates are equivalent only when source evidence proves the same string receiver coordinate, the same literal/parameter/imported affix coordinate, standard API or imported namespace identity, the same prefix-versus-suffix direction, and a whole-string single-affix shape. Parameter affixes and immutable local/module affix bindings may join literal affixes under those facts; offset overloads, tuple/multi-affix disjunctions, custom same-name methods, monkey patches, untyped receivers, wrong receivers, and mutated affix bindings remain outside the exact family.
- **proof invariant**: Open string-affix convergence only when source evidence proves exact string receiver identity, standard case-sensitive affix API or imported namespace identity, the same affix value coordinate, the same prefix/suffix direction, and a whole-string single-affix shape. Untyped receivers, boxed or nullable receivers, wrong receivers, custom same-name APIs, missing or shadowed imports, monkey-patched builtins, wrong affix coordinates, dynamic or mutated affix bindings, case-insensitive or locale-sensitive variants, offset overloads, tuple/multi-affix forms, and substring/contains-style APIs must remain non-equivalent.
- **hard negatives**:
  - wrong receiver coordinate or unproven string receiver type
  - wrong literal, parameter, imported, dynamic, or mutated affix coordinate
  - prefix and suffix directions are not interchangeable
  - custom same-name methods, borrowed prototypes, missing imports, and monkey-patched builtins are not standard API proof
  - JS/Java offset overloads are not whole-string prefix proof
  - Python tuple affixes and Ruby multi-affix forms are disjunctions, not single-affix predicates
  - case-insensitive or locale-sensitive variants stay outside case-sensitive affix proof
- **evidence**: `string-affix-focused-controlled` (`real_frontier.v1.json`)
- **real frontier replay**: `string-affix-prefix-focused-controlled-pair`, `string-affix-suffix-focused-controlled-pair`, `string-affix-parameter-coordinate-controlled-pair`, `string-affix-ruby-prefix-controlled-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (focused, multi-language) `crates/nose-cli/tests/fixtures/string_affix_550/prefix.py:1-2`
  - `nose` (focused, TypeScript) `crates/nose-cli/tests/fixtures/string_affix_550/prefix.ts:1-3`
  - `nose` (focused, Ruby) `crates/nose-cli/tests/fixtures/string_affix_551/prefix.rb:1-3`
- **current detector result (primary linked evidence)**: miss=False · `nose` @ `#756 semanti` — Semantic queries report the expected prefix, suffix, Ruby prefix/suffix, same-role parameter, and immutable binding families while adjacent hard negatives stay outside those families.
- **detector admission**: `controlled-slice-admitted` · controlled case-sensitive whole-string prefix/suffix predicates with receiver, API/import, affix coordinate, direction, and arity proof
- **remaining real-pair gap**: a non-focused real-corpus string-affix pair still needs separate audit before this packet can claim real-pair admission
- **why now**: string_prefix_suffix has broad controlled coverage across core languages, a Swift probe row, and closeout evidence for Go ownership, Ruby receiver proof, and affix-coordinate boundaries. The remaining value is to preserve the receiver/API/affix/direction/arity perimeter as reusable neutral facts before future case-insensitive, locale, offset, or multi-affix expansions add more surfaces.
- **blocked by**: nothing
- **notes**: This packet records the current string-affix perimeter as reusable proof facts. It intentionally leaves case-insensitive, locale-sensitive, offset, and multi-affix semantics outside exact admission until their extra proof facts exist.

## `null-option-presence-2026-07-08` — axis `null_option_presence`

- **owner route**: `team-a-detector` (#757) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `none`
- **breadth**: repo 82% · primary-language 100% (8/8) · dev 53 · held-out 46 · both-splits
- **semantic claim**: Null/Option absence predicates, present predicates, and pure or already-evaluated fallback defaulting surfaces can converge only when they prove the same checked value coordinate, absence-channel boundary, presence direction or fallback coordinate, and standard nullish/Option API identity. JS/TS nullish defaulting and Rust Option::unwrap_or converge for the focused slice; truthy defaults, strict-null-only defaults, effectful fallback timing, wrong checked values, wrong fallbacks, Result channels, shadowed constructors, and unproven option-like APIs remain outside the exact family.
- **proof invariant**: Open null/Option presence and defaulting convergence only when source evidence proves the checked value coordinate, the absence-channel boundary, the presence-vs-absence direction or fallback/default coordinate, standard nullish/Option API identity, and a pure or already-evaluated fallback default trigger. Falsey present payloads, strict-null-only defaulting behavior, shadowed undefined, wrong value coordinates, wrong fallback coordinates, custom same-name option helpers, bare Java Optional without type-domain proof, Rust Result channels, shadowed constructors, and effectful fallback expressions must remain non-equivalent.
- **hard negatives**:
  - absence and present predicates are opposite directions and must stay split
  - checking `other` instead of `value` changes the nullable/Option value coordinate
  - truthy defaulting such as `value || fallback` drops falsey present payloads and is not nullish defaulting
  - strict-null-only defaulting is not loose nullish defaulting because undefined handling differs
  - wrong fallback/default coordinates change defaulting behavior
  - Java Optional needs fully-qualified java.util.Optional type-domain proof; bare Optional and custom option-like helpers are not enough
  - Rust Result Ok/Err channels and shadowed Some/None constructors are not Option Some/None channel proof
- **evidence**: `null-option-presence-focused-controlled` (`real_frontier.v1.json`)
- **real frontier replay**: `null-option-presence-absence-focused-controlled-pair`, `null-option-presence-present-focused-controlled-pair`, `nullish-default-focused-controlled-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (focused, Python) `bench/type4/adversarial/cases/null_option_presence/presence.py:1-9`
  - `nose` (focused, Rust) `bench/type4/adversarial/cases/null_option_presence/presence.rs:1-15`
  - `nose` (focused, JavaScript) `bench/type4/adversarial/cases/null_option_presence/default.js:1-26`
- **current detector result (primary linked evidence)**: miss=False · `nose` @ `#757 semanti` — Semantic query reports distinct absence, present, and defaulting families while direction, wrong-value, truthy-default, strict-null, and wrong-coordinate hard negatives stay outside those families.
- **detector admission**: `controlled-slice-admitted` · controlled null/Option absence, present, and pure/already-evaluated fallback defaulting predicates with value-coordinate, specified channel boundary, direction, fallback, default-trigger, and API identity evidence
- **remaining real-pair gap**: a non-focused real-corpus null/Option/defaulting pair still needs separate audit before this packet can claim real-pair admission
- **why now**: null_option_presence has very broad coverage and the largest raw occurrence signal in the frontier platform, but this packet records only the controlled proof perimeter: value coordinate, specified absence-channel boundary, presence direction, fallback coordinate, pure/default trigger, and API/channel identity. That makes future nullable, Optional, and Option surfaces attach to neutral facts instead of per-language null selector shortcuts.
- **blocked by**: nothing
- **notes**: This packet records the current null/Option presence/defaulting perimeter as reusable proof facts. It intentionally leaves Ruby nil? focused admission, Swift full Optional admission, Rust match-default focused convergence, and effectful fallback timing outside the exact claim until separate evidence covers those boundaries.

## `reduction-minmax-anyall-2026-07-08` — axis `reduce_minmax_anyall`

- **owner route**: `team-a-detector` (#758) · evidence tier: `frontier-recorded` · cost `medium` · risk `medium` · substrate `fragment-contract`
- **breadth**: repo 88% · primary-language 100% (8/8) · dev 58 · held-out 47 · both-splits
- **semantic claim**: Reduction surfaces can converge only when source evidence proves the same traversal source, identity and empty-input behavior, numeric.aggregate-value-model-domain for controlled aggregate arithmetic, numeric.selection-value-order-domain for controlled min/max or relational selection, reduction step or terminal predicate coordinate, any/all short-circuit direction, selection seed/domain behavior for min/max, receiver/API identity for protocol methods, and effect-safe value-only predicates or callbacks. The focused slice admits sum loops and typed reduce/sum APIs across C, Go, Java, JS loop, Python, Rust, and TypeScript; Rust any/all, TypeScript any/some, dense-literal one-argument TypeScript every/for-of terminal bridges; Swift eager Array/Collection allSatisfy terminal bridges; and Python/Rust seeded min/max selection loops/folds. Wrong seeds, changed additive/product/count steps, changed terminal predicates, any-vs-all direction changes, TypeScript array-param every sparse-hole boundaries, TypeScript every callbacks observing index/source-array arguments, Swift changed predicate/source, wrong empty truth, callback or loop effects, two-argument custom allSatisfy overloads, lazy allSatisfy demand semantics, max-vs-min direction changes, unseeded selection APIs with different all-negative behavior, broad runtime no-overflow or total-order claims, and unproven overflow/float/NaN/signed-zero numeric domains remain outside the exact family through numeric.float-special-value-boundary.
- **proof invariant**: Open reduction convergence only when the compared forms traverse the same source in the same order, preserve the identity seed and empty-input behavior, satisfy numeric.aggregate-value-model-domain when controlled aggregate arithmetic matters, satisfy numeric.selection-value-order-domain when controlled min/max or relational selection matters, apply the same reducer contribution or terminal predicate to the same element coordinate, preserve any/all short-circuit direction and fallthrough result, prove receiver/API identity for protocol methods, require value-only predicate callback shape, and for min/max preserve the same explicit seed and selection domain. Predicate/callback effects, TypeScript Array.every callbacks observing index or source-array arguments, Swift two-argument custom allSatisfy overloads, Swift lazy allSatisfy receiver demand semantics, wrong seeds, changed contributions, changed terminal predicates, all-vs-any direction changes, unseeded terminal selection APIs, broad runtime no-overflow or total-order claims, and unproven receiver/protocol, overflow, float, NaN, signed-zero, or numeric-domain evidence must remain non-equivalent.
- **hard negatives**:
  - seed 0 versus seed 1 changes empty-input behavior and every non-empty sum
  - additive sum, multiplicative product, and positive-count contributions are distinct step coordinates
  - any/all terminal predicates such as x > 0, x >= 0, and x < 0 are distinct coordinates
  - any and all have opposite short-circuit direction and fallthrough truth values
  - TypeScript number[] parameters do not prove dense Array.every sources because sparse holes are skipped by every but observable to for-of iteration
  - TypeScript Array.every callbacks that observe index or source-array arguments are outside the value-only predicate proof
  - numeric aggregate arithmetic without numeric.aggregate-value-model-domain evidence stays outside exact reduction admission
  - min/max or relational selection without numeric.selection-value-order-domain evidence stays outside exact selection admission
  - NaN, signed-zero, and float non-associativity surfaces are tracked by numeric.float-special-value-boundary and stay split
  - Swift allSatisfy two-argument custom overloads are outside the value-only stdlib Sequence quantifier proof
  - Swift allSatisfy over .lazy receivers is outside the eager Array/Collection demand proof
  - Swift allSatisfy changed predicate/source coordinates, wrong empty truth, or callback/loop effects stay split
  - seeded max over max(0, xs...) differs from max(xs).unwrap_or(0) on all-negative non-empty inputs
  - min and max selection directions are distinct even when both use the same seed
  - effectful reducers, predicates, callbacks, or receiver/protocol mutations are behavior-defining
- **evidence**: `reduction-minmax-anyall-focused-controlled`, `reduction-typescript-every-append-only-flags-drizzle-real-miss` (`real_frontier.v1.json`)
- **real frontier replay**: `reduction-sum-focused-controlled-pair`, `reduction-any-focused-controlled-pair`, `reduction-typescript-every-dense-literal-controlled-pair`, `reduction-typescript-every-array-param-boundary-controlled-pair`, `reduction-typescript-every-append-only-flags-drizzle-real-pair`, `reduction-selection-focused-controlled-pair` (`real_frontier_replay.v1.json`)
- **representative locations**:
  - `nose` (focused, multi-language) `bench/type4/adversarial/cases/reduction_minmax_anyall/sum.py:1-23`
  - `nose` (focused, Rust) `bench/type4/adversarial/cases/reduction_minmax_anyall/any_all.rs:1-42`
  - `nose` (focused, TypeScript) `bench/type4/adversarial/cases/reduction_minmax_anyall/any_all.ts:1-124`
  - `nose` (focused, Rust) `bench/type4/adversarial/cases/reduction_minmax_anyall/selection.rs:1-35`
  - `nose` (focused, Ruby) `bench/type4/adversarial/cases/ruby_enumerable_quantifier/any_all.rb:1-82`
  - `nose` (focused, Ruby) `bench/type4/adversarial/cases/ruby_enumerable_quantifier/monkey_patch.rb:1-8`
  - `nose` (focused, Ruby) `bench/type4/adversarial/cases/ruby_enumerable_quantifier/module_eval_patch.rb:1-9`
  - `nose` (focused, Swift) `bench/type4/adversarial/cases/swift_all_satisfy/all_satisfy.swift:1-80`
  - `drizzle-orm` (dev, TypeScript) `drizzle-kit/src/cli/commands/mysqlIntrospect.ts:35-38`
  - `drizzle-orm` (dev, TypeScript) `drizzle-kit/src/cli/commands/sqliteIntrospect.ts:41-44`
- **current detector result (primary linked evidence)**: miss=False · `nose` @ `#769 Swift a` — Semantic queries report a broad sum/reduce family, TypeScript any/some, dense-literal one-argument TypeScript every/for-of, Swift eager allSatisfy/loop, plus Rust any/all terminal families, and seeded selection families while wrong-seed, changed product/count step, changed-predicate, TypeScript array-param every, TypeScript every callback-extra-argument, Swift changed predicate/source, wrong empty truth, effect, two-argument custom overload, lazy receiver, Rust any-vs-all direction, and unseeded selection boundaries stay outside those families.
- **detector admission**: `controlled-slice-admitted` · controlled integer/value-model sum/product, any/all terminal, Swift eager allSatisfy, and seeded min/max selection reductions with source, identity/empty, aggregate value-model numeric-domain, selection value-order numeric-domain, float-special-value boundary, step/predicate, short-circuit direction, selection seed/domain, receiver/API identity, and predicate/reducer effect evidence
- **remaining real-pair gap**: the linked Drizzle real-corpus TypeScript every(Boolean) pair is replayed as split until append-only dense local-array provenance and value-only Boolean predicate facts are modeled; broader reduce/min/max/any/all real-pair admission still needs separate audit
- **why now**: reduce_minmax_anyall has all-language probe coverage and already appears in loops_and_reductions, iteration_contracts, and semantic idiom tests. The useful work is to record the shared reduction proof perimeter — identity/empty behavior, aggregate value-model closure for arithmetic reductions, selection value-order closure for min/max, step or terminal predicate coordinate, short-circuit direction, selection seed/domain, source identity, and predicate or reducer effect closure — so future reduce, any/all, and min/max surfaces extend neutral facts instead of per-language spellings.
- **blocked by**: the Drizzle flags.every(Boolean) real pair uses a local array populated by pushes; the current TypeScript every proof facts only admit dense literal sources, the current detector has no reusable append-only dense local-array provenance fact, so arbitrary array-parameter every/for-of sparse-hole boundaries must stay closed, Boolean-as-callback is value-only only when the binding is the standard Boolean function and all pushed values are proven boolean
- **notes**: This packet records the current focused reduction perimeter as reusable proof facts. The linked Drizzle real-corpus replay is an executable split expectation for the next TypeScript every source-provenance fact, not a real-pair admission. It intentionally does not claim a new non-focused real-corpus admission, untyped JS relational reduction admission, Ruby parameter/custom Enumerable receiver admission, Swift reduce, Swift contains(where:), or Swift lazy allSatisfy admission until those proof perimeters are separately covered.
