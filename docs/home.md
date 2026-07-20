# nose documentation

**nose** finds syntax, semantic, and near-duplicate code clones across
nine imperative languages — plus declarative **CSS** (computed-style equivalence)
and **HTML markup** (rendered-DOM equivalence), and the `<script>`/`<style>`/markup regions
inside Vue, Svelte, and HTML — by
lowering every language into one normalized intermediate language (IL) and
ranking the candidates by refactoring value — a deterministic triage signal, not a
worth-it verdict (that judgment is the consumer's). The repository
[README](../README.md) is the one-screen overview; this wiki is the full guide.

The pages are grouped by what you're here to do.

## Start here

- [getting-started](getting-started.md) — install, run your first `nose query`, and learn
  to read the report in a few minutes. **The friendly on-ramp; read this first.**

## Fast paths

- **Trying nose locally:** install from [getting-started](getting-started.md), then run
  `nose query <path>` to explore interactively (follow the suggested next-commands), or
  `nose query <path> --format markdown` for a one-shot ranked report.
- **Automating triage:** run `nose query <path>` for the human-readable loop, or
  `nose query <path> --format json` for tooling. Agent-specific guidance is in [agent-recipe](agent-recipe.md).
- **Adding a repo gate:** pin the detection surface and the size budget, for example
  `nose query <path> --mode syntax --min-size 80 'dup>80' --fail-on any`; see
  [continuous integration](continuous-integration.md), then commit shared defaults from [configuration](configuration.md).
- **Building an integration:** use [capabilities](capabilities.md) before invoking a binary
  and parse [query JSON](query-json.md), not human output.

## Using nose

You want to *run* nose on a codebase and act on what it finds.

- [usage](usage.md) — the complete command and flag reference: `query`, `stats`, `il`, `capabilities`, `semantic-pack`, the ranking keys, and the detection modes.
- [usage › nose query](usage.md#nose-query) — `nose query`: analyze a path, inspect the best duplicated-code families, filter/group/sort the list, open one family, run the `--fail-on` CI gate, or emit the versioned JSON contract.
- [divergent edits](divergent-edits.md) — the `base=<ref>` check: flag clones changed inconsistently in a diff (a copy fixed, its siblings missed).
- [divergent history mining](divergent-history-mining.md) — bounded offline replay of the divergent-edit check across commit ranges.
- [configuration](configuration.md) — the `nose.toml` file: excludes, modes, ranking, thresholds, and structured-ignore defaults.
- [continuous-integration](continuous-integration.md) — the `--fail-on any` gate, baseline-driven incremental adoption, SARIF, and fast re-runs.
- [structured-ignores](structured-ignores.md) — suppress reviewed findings with reason, owner, expiry, and machine-readable ignored-family output.
- [reinvented-helpers](reinvented-helpers.md) — the containment channel: code that reimplements an existing pure helper inline instead of calling it.
- [clone-types](clone-types.md) — what nose covers across the standard Type-1/2/3/4 taxonomy, with its honest limits.
- [languages](languages.md) — the supported languages, declarative CSS and HTML markup, and the `<script>`/`<style>`/markup region extraction for Vue/Svelte/HTML.
- [markdown-duplication](markdown-duplication.md) — same-language near-duplicate **prose** detection across Markdown documents, surfaced as a `nose query` domain (a separate char-n-gram engine; span witness + commonness evidence; no LLM, same-language only).

## Integrating nose

You're building tooling — an installer, CI wrapper, or editor integration — on top
of nose's machine-readable output.

- [capabilities](capabilities.md) — the `nose capabilities` JSON contract: what an installed binary supports, so a wrapper never has to scrape `--help`.
- [agent-recipe](agent-recipe.md) — the validated protocol for an LLM agent: use `nose query` for exploration, then read the `nose query --format json` contract for batch and gate workflows.
- [query-json](query-json.md) — the versioned `nose query --format json` contract (schemas v7 and v8): the structured, view-shaped machine form of the exploration surface.

## Contributing

You want to *change* nose or understand how it works inside. Start with the three
fundamentals; the rest is grouped by area.

### Fundamentals

- [design & direction](design.md) — the *why* behind the roadmap: the sound core as the moat, the two (non-human) consumers, and what that decides. **Check roadmap calls against this.**
- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [refactoring-ratchets](refactoring-ratchets.md) — repository quality ratchets for incremental design cleanup, including Rust file-length and the CLI prelude guard.
- [semantic-regression-smoke](semantic-regression-smoke.md) — base/head semantic output and runtime tripwire, exact intentional-drift declarations, focused reruns, and pinned evidence.

### Repository workflow

- [contributing](contributing.md) — contributor workflow, local quality gates,
  repository automation, conventions, changelog discipline, and release steps.
- [agent-instructions](agent-instructions.md) — repository-specific instructions for coding agents, including documentation workflow and docs checks.

### Channels, witnesses & proofs

- [graded-witness](graded-witness.md) — the anti-unification grade for near families: "equal except *k* holes", each hole a candidate parameter, with the soundness-relevant referent check.
- [fragment-contracts](fragment-contracts.md) — how exact sub-function fragments are modeled: classification, contract, the wrapper-synthesis behavior oracle, the effect algebra, and fail-closed receiver identity.
- [reinvented-helpers](reinvented-helpers.md) — the containment channel: code that reimplements an existing pure helper inline, and the surface policy promoting non-test findings to the default report.
- [oracle-value-model](oracle-value-model.md) — the verify oracle's value model (Int/Bool/Str-monoid/List/Float/Sym), what it witnesses, and the outcomes that closed the #283 false-merge cluster (C string/`+`-non-assoc, D-int32 width, D-div float) plus the `--falsify` search.
- [recall-loss-diagnostics](recall-loss-diagnostics.md) — the local `nose verify --recall-loss-report` artifact for keeping exact semantic admission strict while measuring under-merges, oracle exclusions, and structured admission rejections.
- [value-float-kind-design](value-float-kind-design.md) — the IEEE-754 `Value::Float` kind (#342, SHIPPED): how fully-untyped float associativity was closed in both the oracle and the analyzer, with the full-corpus recall measurement (delta 0).
- [formal-soundness](formal-soundness.md) — Lean 4 proof-obligation registry for proof-sensitive IL, normalization, fragment, and oracle contracts.
- [Soundness Lab](soundness-lab.md) — the frozen v0.19.0 exact-claim cohort,
  risk-weighted non-gameable scorecard, published-asset replay discrepancy, and
  reproduction gate for 0.20 soundness work.

### Semantic kernel & packs

#### Current architecture and operating process

- [semantic-kernel](semantic-kernel.md) — semantic-kernel and pack architecture: language/library semantics, extension boundaries, responsibility model, and exact-channel eligibility.
- [semantic-pack-architecture](semantic-pack-architecture.md) — the #473 migration rulebook for builtin/external pack terminology, kernel-vs-pack ownership, behavior gates, and performance gates.
- [semantic-kernel-snapshot](semantic-kernel-snapshot.md) — current implementation snapshot for semantic knowledge and the first internal kernel facade.
- [evidence-records](evidence-records.md) — the internal pack-facing evidence substrate for source, domain, import, symbol, type, guard, place/effect, library API, and sequence-surface facts.
- [source-facts](source-facts.md) — source-origin evidence for semantic contracts: construct syntax, async/generator/error boundaries, literal/operator provenance, pack boundaries, and fail-closed exact admission.
- [demand-effect-semantics](demand-effect-semantics.md) — the internal demand/effect contract model for eager, lazy, short-circuit, async, generator, and channel boundaries.
- [recall-loss-recovery-loop](recall-loss-recovery-loop.md) — checked-in baseline summaries, report diff workflow, five-cycle recovery/attribution loop, and the full-corpus priority census for semantic-kernel work.
- [semantic-pack-adoption](semantic-pack-adoption.md) — promotion, rollback, and adoption-gate reports for moving external or optional packs into official builtin support without forking semantic vocabulary.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) — manifest API, installed-version, kernel-vocabulary, and fail-closed external-influence compatibility policy for semantic packs.
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) — versioned v0 schema and provider-facing extension API for language/library semantic packs.
- [semantic-pack-extension-api-v1](semantic-pack-extension-api-v1.md) — closed typed Java/Maven package-API grammar, deterministic compiler, local dependency/occurrence evidence, bounded locked near influence, and receipt-backed collection-factory exact claims.
- [semantic-pack-reference-vavr](semantic-pack-reference-vavr.md) — shipped, disabled-by-default Vavr `List.of` reference pack, pinned lock/receipt, measured value, boundaries, and rollback.
- [semantic-pack-0.20-release-gate](semantic-pack-0.20-release-gate.md) — the #863 compatibility, provenance, soundness, performance, responsibility, and rollback closeout for influential local packs.
- [semantic-pack-project-lock](semantic-pack-project-lock.md) — local content-pinned v1 authorization, row/channel selection, dependency/receipt pins, path containment, deterministic conflict rejection, and evidence input boundary.
- [semantic-pack-conformance](semantic-pack-conformance.md) — provider/user workflow for structural and bounded kernel source checks, receipt output, and builtin inventory audits.
- [semantic-pack-loading](semantic-pack-loading.md) — local pack manifest loading, explicit opt-in trust policy, and fail-closed near/external-claim-exact boundaries.

#### Planning and pricing

- [semantic-kernel-roadmap](semantic-kernel-roadmap.md) — decisions, history, phases, and open work for the semantic-kernel direction.
- [semantic-kernel-capability-minimization](semantic-kernel-capability-minimization.md) — issue #507 primitive census, blocker taxonomy, and accept/reject matrix for deriving minimal kernel capabilities from pack blockers.
- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md) — narrow-slice candidate matrix for future large-ecosystem builtin packs such as Guava, Lodash, NumPy, and RxJS.
- [semantic-pack-candidate-pricing](semantic-pack-candidate-pricing.md) — corpus-backed pricing loop for deciding which narrow semantic-pack rows are ready, blocked, or unpriced before implementation.

#### Historical closeouts and audits

- [stabilization-829](stabilization-829.md) — bounded pre-#821 documentation, test, code-quality, and official v0.18.0 performance-baseline closeout, including Mach-O code identity and the tightened duplication ratchet.
- [semantic-kernel-builtin-expansion-509](semantic-kernel-builtin-expansion-509.md) — issue #509 blocker packet, admitted API result-domain primitive, and builtin expansion record.
- [semantic-kernel-expansion-511](semantic-kernel-expansion-511.md) — issue #511 R1-R3 cycles: generalized admitted API result-domain materialization, external fixed-domain authoring, and transition assessment.
- [semantic-kernel-external-authorability-511](semantic-kernel-external-authorability-511.md) — issue #511 R4: external-pack authorability matrix, Guava fixed-domain dry run, and transition-to-R5 assessment.
- [semantic-kernel-hof-demand-511](semantic-kernel-hof-demand-511.md) — issue #511 R5: HOF, demand, and materialization boundary matrix.
- [semantic-kernel-expansion-closeout-511](semantic-kernel-expansion-closeout-511.md) — issue #511 R6 closeout: minimal capability set, builtin expansion, external authorability, remaining blockers, and validation gates.
- [semantic-pack-default-promotion-audit-678](semantic-pack-default-promotion-audit-678.md) — issue #678 audit of builtin semantic-pack lane state, exact-capable coverage, default-promotion candidates, and rollback gates.
- [semantic-kernel-closeout-533](semantic-kernel-closeout-533.md) — closeout for the #533 sequence-HOF and iterator materialization tranche, including product metrics and process-evidence gaps.
- [string-affix-protocol-closeout-548](string-affix-protocol-closeout-548.md) — closeout for the #548 string prefix/suffix predicate protocol extraction, including inventory, product-output, and review evidence.
- [go-string-affix-closeout-549](go-string-affix-closeout-549.md) — closeout for the #549 Go `strings.HasPrefix`/`HasSuffix` namespace-proof migration into the string-affix protocol.
- [js-ts-string-affix-hardening-closeout-550](js-ts-string-affix-hardening-closeout-550.md) — closeout for the #550 JavaScript/TypeScript string-affix receiver proof hardening, including false-open and runtime evidence.
- [ruby-string-affix-closeout-551](ruby-string-affix-closeout-551.md) — closeout for the #551 Ruby `String#start_with?`/`String#end_with?` proof slice, including product-output, inventory, and runtime evidence.
- [string-affix-coordinate-closeout-552](string-affix-coordinate-closeout-552.md) — closeout for the #552 string-affix coordinate boundary hardening, including parameter/binding coordinates and deferred multi/offset forms.
- [string-affix-conformance-closeout-558](string-affix-conformance-closeout-558.md) — closeout for the #558 string-affix conformance and builtin inventory hardening pass.
- [import-backed-immutable-provenance-closeout-567](import-backed-immutable-provenance-closeout-567.md) — closeout for the #567 imported immutable value provenance capability, including admitted coordinate families, hard-negative boundaries, recall-loss census, and runtime evidence.
- [semantic-pack-boundary-review-2026-06-22](semantic-pack-boundary-review-2026-06-22.md) — pre-release review of the semantic kernel vs builtin semantic-pack boundary after the #484 stabilization tracker.
- [semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md) — post-PR #147 audit of remaining raw/local semantic pockets and follow-up owners.
- [semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md) — closeout for the #109 semantic-kernel foundation and follow-up tranche.
- [scheduling-channel-callback-obligations-594](scheduling-channel-callback-obligations-594.md) — issue #594's cross-language obligation vocabulary, Promise/scheduling closeout, and #602 census-backed guardrails, exact Promise aggregate slices, reporting-only executor/cancellation/scheduling lifecycle slices, and closeout for scheduling, aggregate, cancellation, error/rejection channels, callback demand/effect, lifecycle, and mutation boundaries.

### Type-4, hazard & measurement

- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [0.20 default-head baseline](default-head-baseline-839.md) — the published
  v0.19.0 bare-default precision/full-universe recall baseline, parity proof,
  and compatibility metric.
- [0.20 default-head label runway](default-head-label-runway-840.md) — the
  split-safe v7 dev overlay, sealed held-out selection, independent panel and
  arbitration, and complete dev-head coverage.
- [0.20 default-head failure taxonomy](default-head-failure-taxonomy-841.md) — the
  complete dev-head cross-tabs, independently audited generated/declaration cohorts,
  source-bound hard negatives, and proof/actionability no-go.
- [0.20 generated provenance](generated-provenance-842.md) — the bounded Jazzy
  all-member classifier, reason-coded recovery contract, exact dev output drift, hard
  negatives, and official-v0.19.0 runtime price.
- [0.20 checked-in generated artifact provenance](generated-artifact-provenance-891.md) — the
  producer-independent HTML generator declaration, mixed-family fail-open boundary,
  fresh-repository confirmation, exact transition ledger, and residual API no-go.
- [0.20 caller-provided generated paths](caller-generated-path-provenance.md) — the
  root-anchored CLI/config assertion contract, canonical containment and fail-open rules,
  all-member recovery semantics, and machine-readable trust boundary.
- [0.20 declaration-only type contracts](declaration-only-type-contracts-843.md) — the
  all-member `UnitOrigin` classifier, cross-language fail-open boundaries, exact dev
  surface/origin drift, worthy-recall preservation, and official-v0.19.0 runtime price.
- [0.20 proof/actionability no-go](proof-actionability-no-go-844.md) — the independently
  reviewed small/shallow proven-channel boundary, worthy helper/table hard negatives,
  checked 90%-gate failure, and zero-product-change preservation contract.
- [0.20 residual ranking calibration](residual-ranking-calibration-845.md) — frozen
  complete dev universe, blind three-persona panel and arbitration, exact-key overlay,
  and the fully judged 68.24%/language-floor no-go.
- [0.20 default-head blind closeout](default-head-blind-closeout-846.md) — the
  one-shot held-out unseal, opaque-ID panel, fresh-repository audit, and final #838
  threshold decision against the published v0.19.0 binary.
- [current missed-worthy frontier](missed-worthy-frontier-816.md) — the #816
  dev-first recall audit, accepted-pair coverage-loss result, route-tree protocol
  deviation, rejected alternatives, and #817 follow-up gates.
- [accepted-pair endpoint coverage](accepted-pair-coverage-817.md) — the #817
  bounded grouping fix, full dev accepted-edge census, exact recovered worthy
  families, product/runtime price, hard negatives, and frozen held-out gate.
- [post-#817 missed-worthy frontier](missed-worthy-frontier-820.md) — the #820
  snapshot-aware refresh, source-bound dev comparison of connected witnesses,
  same-unit fragments, and extraction, plus the bounded #821 decision.
- [connected mapped witnesses](connected-witness-821.md) — the #821 pair-local connected
  witness, hard-negative boundary, output/work budgets, official-release runtime price,
  and zero-regression v6 closeout.
- [bounded same-unit windows](bounded-same-unit-windows-832.md) — the #832 bounded intra-unit
  near route, disjoint-location contract, frozen dev review, and release-based price.
- [type4-benchmark](type4-benchmark.md) — the evidence-carrying synthetic Type-4 benchmark factory.
- [type4-adversarial-coverage](type4-adversarial-coverage.md) — focused Type-4 cases, target-packet task cards, and verifier-lead draft workflow.
- [frontier-platform](frontier-platform.md) — corpus-balanced evidence platform that ranks the next Type-4 axis by presence breadth (not raw count) and separates the queue signal from human-verified evidence.
- [proof-carrying frontier](proof-carrying-frontier.md) — target-packet admission report that requires linked evidence, proof invariants, hard negatives, blockers, and co-evolution guardrail context before exact Type-4 admission opens.
- [Type-4 semantic pattern loop](type4-semantic-pattern-loop.md) — repeatable process for turning one admitted language surface into a language-neutral semantic pattern with proof facts, capability matrix, and reusable hard-negative templates.
- [adversarial-coevolution](adversarial-coevolution.md) — the cross-axis campaign runbook: a white-box attacker derives structurally-missed patterns, an assessor prices them, a defender ships the largest sound generalization.
- [hazard-ranking](hazard-ranking.md) — the evidence base for the experimental `sort=hazard` (a divergence-*propensity* signal; **not** a validated harm ranker — it ranks actual harm near chance) and the honest evaluation trail.
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and labeled dataset hazard is measured against (repo selection, graded labels, quantitative sufficiency).
- [hazard-release-checklist](hazard-release-checklist.md) — what to do for the hazard ranking on every new nose release (one-page runbook: refresh the dataset, re-tune, re-validate).
- [runtime triage](runtime-triage.md) — reproducible query-runtime regression triage: harness, classification policy, timing knobs, and when not to optimize.
- [order-aware performance controls](order-aware-performance-controls.md) — the preregistered paired-block estimator, one-sided same-binary correction, exact sign-test decision rule, and frozen #892 decision ledger.
- [Ruby redefinition runtime triage](runtime-triage-ruby-redefinitions-2026-07-10.md) — #807 diagnosis, indexed same-file facts, focused noise control, and all-corpus output evidence.
- [0.19.0 release evidence](release-evidence-0.19.0.md) — the official v0.18.0 baseline comparison, accepted-coverage reporting hot-path fix, v6 product-quality reproduction, and recall-loss gate for the 0.19.0 release candidate.
- [0.18.0 release evidence](release-evidence-0.18.0.md) — the pre-release performance pass, all-corpus query-regression, and recall-loss gate for the 0.18.0 release candidate.
- [0.17.0 release evidence](release-evidence-0.17.0.md) — the hazard refresh, all-corpus query-regression, recall-loss gate, and profiling notes for the 0.17.0 release candidate.
- [0.17.0 post-release runtime triage](runtime-triage-0.17.0.md) — focused follow-up on the largest release-candidate query-runtime regressions, including the `arrow` hot-path fix and remaining no-family-growth follow-ups.
- [20-optimization runtime pass](runtime-performance-20-optimizations-2026-07-02.md) — the post-0.17.0 profile-guided optimization sequence, all-120-repo before/after run, focused recheck, and noise-aware interpretation.
- [default-query performance closeout](runtime-performance-issue-892-2026-07-18.md) — #892's output-preserving surface-membership optimization, official-v0.19.0 all-120/r40 measurements, semantic smoke, and bounded independent residual split.
- [etcd Go frontend attribution](runtime-performance-issue-907-2026-07-19.md) — #907's frozen-r40 source, executable, machine-code, and profile evidence attributing the residual to build provenance plus control subtraction.
- [normalize-and-extract closeout](runtime-performance-issue-908-2026-07-19.md) — #908's output-identical MinHash hot-loop optimization and frozen Guava/MinIO r40 closeout against official v0.19.0.
- [incremental cache benchmark](incremental-cache-benchmark.md) — the #872 official-v0.19.0 binary pin, clean/cold/warm equivalence rule, mutation closure inventory, and real/synthetic workload contract for the Instant Monorepo engine.
- [portable cache artifacts](portable-cache-artifacts.md) — the #873 SHA-256 layered CAS, #874
  dependency-aware source/raw/resolved reuse, and #877 bounded direct warm-leaf restoration with
  exact invalidation closure and fail-safe reporting.
- [experiments](experiments.md) — the measured log of what was tried and what happened.

### Field evidence & audits

- [field-evaluation](field-evaluation.md) — qualitative results from running nose on real third-party projects.
- [dogfooding](dogfooding.md) — the current nose-on-nose duplication gate and baseline workflow.
- [dogfooding history](dogfooding-history.md) — detailed nose-on-nose review log and accepted-family decisions.
- [divergent fire-precision results](../eval/divergence_fire/RESULTS.md) — final #681 replay/policy closeout for the opt-in divergent-edit gate, including the checked strict precision baseline and retention evidence.
- [divergent-history-mining-pilot-687](divergent-history-mining-pilot-687.md) — checked #687 evidence for bounded divergent history mining and a non-required observe-only pilot.
- [divergent-gate-product-runtime-688](divergent-gate-product-runtime-688.md) — checked #688 product-output and runtime evidence for the opt-in divergent gate.
- [divergent-edit 0.20 closeout](divergent-gate-closeout-854.md) — #847/#854's
  fail-closed policy decision, still-sealed blind population, official-v0.19.0 runtime
  result, compatibility checks, and supported opt-in claim.
- [reinvented-helper-audit-2026-06-13](reinvented-helper-audit-2026-06-13.md) — the hand-labeled field audit that promoted the reinvented-helper channel to the default surface.
- [query-json-agent-audit-2026-06-10](query-json-agent-audit-2026-06-10.md) — machine-contract audit for consumer 1's evidence surface.
- [query-json-agent-audit-2026-06-13](query-json-agent-audit-2026-06-13.md) — re-validation after the gap fixes (incl. the graded witness): all five gaps closed, 8/8 decidable from JSON alone.
- [fragment-quality-audit-2026-06-10](fragment-quality-audit-2026-06-10.md) — labeled Java/Python exact-fragment sample and the resulting surface policy.
- [lawpack-provenance-audit-2026-06-10](lawpack-provenance-audit-2026-06-10.md) — full-corpus and targeted real-repo audit of `nose.value_graph.laws` provenance.
- [default-surface-noise-audit-2026-06-14](default-surface-noise-audit-2026-06-14.md) — re-judging the #263/#264/#11/#353 triage-noise feedback on fresh repos: the default-surface noise is two populations (decidable-shape vs judgment-deep AAA scaffolding), and the principle-respecting lever.

The root [CONTRIBUTING](../CONTRIBUTING.md) is a short entry point to the
[contributing](contributing.md) workflow page; release history is in [CHANGELOG](../CHANGELOG.md).
