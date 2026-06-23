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
  `nose query <path> --format json` for tooling. Agent-specific guidance is in
  [agent-recipe](agent-recipe.md).
- **Adding a repo gate:** pin the detection surface and the size budget, for example
  `nose query <path> --mode syntax --min-size 80 'dup>80' --fail-on any`; see
  [continuous integration](continuous-integration.md), then commit shared defaults from
  [configuration](configuration.md).
- **Building an integration:** use [capabilities](capabilities.md) before invoking a binary
  and parse [query JSON](query-json.md), not human output.

## Using nose

You want to *run* nose on a codebase and act on what it finds.

- [usage](usage.md) — the complete command and flag reference: `query`, `stats`, `il`, `capabilities`, `semantic-pack`, the ranking keys, and the detection modes.
- [usage › nose query](usage.md#nose-query) — `nose query`: analyze a path, inspect the best duplicated-code families, filter/group/sort the list, open one family, run the `--fail-on` CI gate, or emit the versioned JSON contract.
- [divergent edits](divergent-edits.md) — the `base=<ref>` check: flag clones changed inconsistently in a diff (a copy fixed, its siblings missed).
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
- [query-json](query-json.md) — the versioned `nose query --format json` contract (schema v6): the structured, view-shaped machine form of the exploration surface.

## Contributing

You want to *change* nose or understand how it works inside. Start with the three
fundamentals; the rest is grouped by area.

### Fundamentals

- [design & direction](design.md) — the *why* behind the roadmap: the sound core as the moat, the two (non-human) consumers, and what that decides. **Check roadmap calls against this.**
- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [refactoring-ratchets](refactoring-ratchets.md) — repository quality ratchets for incremental design cleanup, including Rust file-length and CLI prelude budgets.

### Channels, witnesses & proofs

- [graded-witness](graded-witness.md) — the anti-unification grade for near families: "equal except *k* holes", each hole a candidate parameter, with the soundness-relevant referent check.
- [fragment-contracts](fragment-contracts.md) — how exact sub-function fragments are modeled: classification, contract, the wrapper-synthesis behavior oracle, the effect algebra, and fail-closed receiver identity.
- [reinvented-helpers](reinvented-helpers.md) — the containment channel: code that reimplements an existing pure helper inline, and the surface policy promoting non-test findings to the default report.
- [oracle-value-model](oracle-value-model.md) — the verify oracle's value model (Int/Bool/Str-monoid/List/Float/Sym), what it witnesses, and the outcomes that closed the #283 false-merge cluster (C string/`+`-non-assoc, D-int32 width, D-div float) plus the `--falsify` search.
- [value-float-kind-design](value-float-kind-design.md) — the IEEE-754 `Value::Float` kind (#342, SHIPPED): how fully-untyped float associativity was closed in both the oracle and the analyzer, with the full-corpus recall measurement (delta 0).
- [formal-soundness](formal-soundness.md) — Lean 4 proof-obligation registry for proof-sensitive IL, normalization, fragment, and oracle contracts.

### Semantic kernel & packs

- [semantic-kernel](semantic-kernel.md) — semantic-kernel and pack architecture: language/library semantics, extension boundaries, responsibility model, and exact-channel eligibility.
- [semantic-pack-architecture](semantic-pack-architecture.md) — the #473 migration rulebook for builtin/external pack terminology, kernel-vs-pack ownership, behavior gates, and performance gates.
- [semantic-pack-adoption](semantic-pack-adoption.md) — promotion, rollback, and adoption-gate reports for moving external or optional packs into official builtin support without forking semantic vocabulary.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) — manifest API, installed-version, kernel-vocabulary, and fail-closed external-influence compatibility policy for semantic packs.
- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md) — narrow-slice candidate matrix for future large-ecosystem builtin packs such as Guava, Lodash, NumPy, and RxJS.
- [semantic-pack-candidate-pricing](semantic-pack-candidate-pricing.md) — corpus-backed pricing loop for deciding which narrow semantic-pack rows are ready, blocked, or unpriced before implementation.
- [semantic-kernel-capability-minimization](semantic-kernel-capability-minimization.md) — issue #507 primitive census, blocker taxonomy, and accept/reject matrix for deriving minimal kernel capabilities from pack blockers.
- [semantic-kernel-builtin-expansion-509](semantic-kernel-builtin-expansion-509.md) — issue #509 blocker packet, admitted API result-domain primitive, and builtin expansion record.
- [semantic-kernel-expansion-511](semantic-kernel-expansion-511.md) — issue #511 R1-R3 cycle 1: generalized admitted API result-domain materialization, builtin expansion, and transition assessment.
- [semantic-pack-boundary-review-2026-06-22](semantic-pack-boundary-review-2026-06-22.md) — pre-release review of the semantic kernel vs builtin semantic-pack boundary after the #484 stabilization tracker.
- [semantic-kernel-snapshot](semantic-kernel-snapshot.md) — current implementation snapshot for semantic knowledge and the first internal kernel facade.
- [semantic-kernel-roadmap](semantic-kernel-roadmap.md) — decisions, history, phases, and open work for the semantic-kernel direction.
- [semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md) — post-PR #147 audit of remaining raw/local semantic pockets and follow-up owners.
- [semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md) — closeout for the #109 semantic-kernel foundation and follow-up tranche.
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) — versioned v0 schema and provider-facing extension API for language/library semantic packs.
- [semantic-pack-conformance](semantic-pack-conformance.md) — provider/user workflow for checking local pack manifests and the builtin inventory workflow for auditing shipped pack coverage.
- [semantic-pack-loading](semantic-pack-loading.md) — local pack manifest loading, explicit opt-in trust policy, and current metadata-only limits.
- [evidence-records](evidence-records.md) — the internal pack-facing evidence substrate for source, domain, import, symbol, type, guard, place/effect, library API, and sequence-surface facts.
- [demand-effect-semantics](demand-effect-semantics.md) — the internal demand/effect contract model for eager, lazy, short-circuit, async, generator, and channel boundaries.
- [source-facts](source-facts.md) — source-origin evidence for semantic contracts: construct syntax, async/generator/error boundaries, literal/operator provenance, pack boundaries, and fail-closed exact admission.

### Type-4, hazard & measurement

- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [type4-benchmark](type4-benchmark.md) — the evidence-carrying synthetic Type-4 benchmark factory.
- [type4-adversarial-coverage](type4-adversarial-coverage.md) — focused Type-4 cases, target-packet task cards, and verifier-lead draft workflow.
- [frontier-platform](frontier-platform.md) — corpus-balanced evidence platform that ranks the next Type-4 axis by presence breadth (not raw count) and separates the queue signal from human-verified evidence.
- [adversarial-coevolution](adversarial-coevolution.md) — the cross-axis campaign runbook: a white-box attacker derives structurally-missed patterns, an assessor prices them, a defender ships the largest sound generalization.
- [hazard-ranking](hazard-ranking.md) — the evidence base for the experimental `--sort hazard` (a divergence-*propensity* signal; **not** a validated harm ranker — it ranks actual harm near chance) and the honest evaluation trail.
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and labeled dataset hazard is measured against (repo selection, graded labels, quantitative sufficiency).
- [hazard-release-checklist](hazard-release-checklist.md) — what to do for the hazard ranking on every new nose release (one-page runbook: refresh the dataset, re-tune, re-validate).
- [experiments](experiments.md) — the measured log of what was tried and what happened.

### Field evidence & audits

- [field-evaluation](field-evaluation.md) — qualitative results from running nose on real third-party projects.
- [dogfooding](dogfooding.md) — nose run on its own source, and what its findings taught us.
- [reinvented-helper-audit-2026-06-13](reinvented-helper-audit-2026-06-13.md) — the hand-labeled field audit that promoted the reinvented-helper channel to the default surface.
- [scanjson-agent-audit-2026-06-10](scanjson-agent-audit-2026-06-10.md) — historical machine-contract audit for consumer 1's evidence surface.
- [scanjson-agent-audit-2026-06-13](scanjson-agent-audit-2026-06-13.md) — historical re-validation after the gap fixes (incl. the graded witness): all five gaps closed, 8/8 decidable from JSON alone.
- [fragment-quality-audit-2026-06-10](fragment-quality-audit-2026-06-10.md) — labeled Java/Python exact-fragment sample and the resulting surface policy.
- [lawpack-provenance-audit-2026-06-10](lawpack-provenance-audit-2026-06-10.md) — full-corpus and targeted real-repo audit of `nose.value_graph.laws` provenance.
- [default-surface-noise-audit-2026-06-14](default-surface-noise-audit-2026-06-14.md) — re-judging the #263/#264/#11/#353 triage-noise feedback on fresh repos: the default-surface noise is two populations (decidable-shape vs judgment-deep AAA scaffolding), and the principle-respecting lever.

The contributor workflow and quality gates live in
[CONTRIBUTING](../CONTRIBUTING.md); release history is in
[CHANGELOG](../CHANGELOG.md).
