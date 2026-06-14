# nose documentation

**nose** finds syntax, semantic, and near-duplicate code clones across
eight languages — plus the `<script>` logic inside Vue, Svelte, and HTML — by
lowering every language into one normalized intermediate language (IL) and
ranking duplicated code by how much it's worth refactoring. The repository
[README](../README.md) is the one-screen overview; this wiki is the full guide.

Every wiki page lives in `docs/` and links to its neighbours with relative links, so the
docs browse cleanly on GitHub and in any Markdown viewer. Root project docs are linked
where relevant. The pages are grouped by what you're here to do.

## Start here

- [getting-started](getting-started.md) — install, run your first scan, and learn
  to read the report in a few minutes. **The friendly on-ramp; read this first.**

## Fast paths

- **Trying nose locally:** install from [getting-started](getting-started.md), then run
  `nose scan <path>` and read the first few ranked families.
- **Adding a repo gate:** start with [continuous integration](continuous-integration.md),
  then commit shared defaults from [configuration](configuration.md).
- **Building an integration:** use [capabilities](capabilities.md) before invoking a binary
  and parse [scan JSON](scan-json.md), not human output.

## Using nose

You want to *run* nose on a codebase and act on what it finds.

- [usage](usage.md) — the complete command and flag reference (`scan`, `review`, `stats`, `il`, `capabilities`), the ranking keys, and the scan modes.
- [review](review.md) — `nose review`: flag clones changed inconsistently in a diff (a copy fixed, its siblings missed) — a PR/CI check on top of git.
- [configuration](configuration.md) — the `nose.toml` file: excludes, modes, ranking, thresholds, and structured-ignore defaults.
- [continuous-integration](continuous-integration.md) — the `--fail-on any` gate, baseline-driven incremental adoption, SARIF, and fast re-runs.
- [structured-ignores](structured-ignores.md) — suppress reviewed findings with reason, owner, expiry, and machine-readable ignored-family output.
- [reinvented-helpers](reinvented-helpers.md) — the containment channel: code that reimplements an existing pure helper inline instead of calling it.
- [reinvented-helper-audit-2026-06-13](reinvented-helper-audit-2026-06-13.md) — the hand-labeled field audit that promoted the reinvented-helper channel to the default surface.
- [clone-types](clone-types.md) — what nose covers across the standard Type-1/2/3/4 taxonomy, with its honest limits.
- [languages](languages.md) — the supported languages and the embedded `<script>` extraction for Vue/Svelte/HTML.

## Integrating nose

You're building tooling — an installer, CI wrapper, or editor integration — on top
of nose's machine-readable output.

- [capabilities](capabilities.md) — the `nose capabilities` JSON contract: what an installed binary supports, so a wrapper never has to scrape `--help`.
- [scan-json](scan-json.md) — the versioned `nose scan --format json` contract for downstream tooling.
- [agent-recipe](agent-recipe.md) — the validated protocol for an LLM agent triaging scan JSON: which fields to read, in what order, and what to do with each verdict.

## Contributing

You want to *change* nose or understand how it works inside. Start with the three
fundamentals; the rest is grouped by area.

### Fundamentals

- [design & direction](design.md) — the *why* behind the roadmap: the sound core as the moat, the two (non-human) consumers, and what that decides. **Check roadmap calls against this.**
- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).

### Channels, witnesses & proofs

- [graded-witness](graded-witness.md) — the anti-unification grade for near families: "equal except *k* holes", each hole a candidate parameter, with the soundness-relevant referent check.
- [fragment-contracts](fragment-contracts.md) — how exact sub-function fragments are modeled: classification, contract, the wrapper-synthesis behavior oracle, the effect algebra, and fail-closed receiver identity.
- [reinvented-helpers](reinvented-helpers.md) — the containment channel: code that reimplements an existing pure helper inline, and the surface policy promoting non-test findings to the default report.
- [oracle-value-model](oracle-value-model.md) — the verify oracle's value model (Int/Bool/Str-monoid/List/Float/Sym), what it witnesses, and the outcomes that closed the #283 false-merge cluster (C string/`+`-non-assoc, D-int32 width, D-div float) plus the `--falsify` search.
- [value-float-kind-design](value-float-kind-design.md) — the IEEE-754 `Value::Float` kind (#342, SHIPPED): how fully-untyped float associativity was closed in both the oracle and the scan, with the full-corpus recall measurement (delta 0).
- [formal-soundness](formal-soundness.md) — Lean 4 proof-obligation registry for proof-sensitive IL, normalization, fragment, and oracle contracts.

### Semantic kernel & packs

- [semantic-kernel](semantic-kernel.md) — semantic-kernel and pack architecture: language/library semantics, extension boundaries, responsibility model, and exact-channel eligibility.
- [semantic-kernel-snapshot](semantic-kernel-snapshot.md) — current implementation snapshot for semantic knowledge and the first internal kernel facade.
- [semantic-kernel-roadmap](semantic-kernel-roadmap.md) — decisions, history, phases, and open work for the semantic-kernel direction.
- [semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md) — post-PR #147 audit of remaining raw/local semantic pockets and follow-up owners.
- [semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md) — closeout for the #109 semantic-kernel foundation and follow-up tranche.
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) — versioned v0 schema and provider-facing extension API for language/library semantic packs.
- [semantic-pack-conformance](semantic-pack-conformance.md) — provider/user workflow for checking local pack manifests and declared fixture assets without implying nose approval.
- [semantic-pack-loading](semantic-pack-loading.md) — local pack manifest loading, explicit opt-in trust policy, and scan JSON provenance reporting.
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
- [scanjson-agent-audit-2026-06-10](scanjson-agent-audit-2026-06-10.md) — can an LLM agent decide and act from scan JSON alone? The measured gap list for consumer 1's evidence surface.
- [scanjson-agent-audit-2026-06-13](scanjson-agent-audit-2026-06-13.md) — the re-validation after the gap fixes (incl. the graded witness): all five gaps closed, 8/8 decidable from JSON alone.
- [fragment-quality-audit-2026-06-10](fragment-quality-audit-2026-06-10.md) — labeled Java/Python hidden/review exact-fragment sample and the resulting surface policy.
- [lawpack-provenance-audit-2026-06-10](lawpack-provenance-audit-2026-06-10.md) — full-corpus and targeted real-repo audit of `nose.value_graph.laws` provenance.
- [default-surface-noise-audit-2026-06-14](default-surface-noise-audit-2026-06-14.md) — re-judging the #263/#264/#11/#353 triage-noise feedback on fresh repos: the default-surface noise is two populations (decidable-shape vs judgment-deep AAA scaffolding), and the principle-respecting lever.

The contributor workflow and quality gates live in
[CONTRIBUTING](../CONTRIBUTING.md); release history is in
[CHANGELOG](../CHANGELOG.md).
