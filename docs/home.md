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
- [clone-types](clone-types.md) — what nose covers across the standard Type-1/2/3/4 taxonomy, with its honest limits.
- [languages](languages.md) — the supported languages and the embedded `<script>` extraction for Vue/Svelte/HTML.

## Integrating nose

You're building tooling — an installer, CI wrapper, or editor integration — on top
of nose's machine-readable output.

- [capabilities](capabilities.md) — the `nose capabilities` JSON contract: what an installed binary supports, so a wrapper never has to scrape `--help`.
- [scan-json](scan-json.md) — the versioned `nose scan --format json` contract for downstream tooling.

## Contributing

You want to *change* nose or understand how it works inside.

- [design & direction](design.md) — the *why* behind the roadmap: the sound core as the moat, the two (non-human) consumers, and what that decides. **Check roadmap calls against this.**
- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [semantic-kernel](semantic-kernel.md) — semantic-kernel and pack architecture: language/library semantics, extension boundaries, responsibility model, and exact-channel eligibility.
- [source-facts](source-facts.md) — source-origin evidence for semantic contracts: construct syntax, literal/operator provenance, pack boundaries, and fail-closed exact admission.
- [semantic-kernel-snapshot](semantic-kernel-snapshot.md) — current implementation snapshot for semantic knowledge and the first internal kernel facade.
- [semantic-kernel-roadmap](semantic-kernel-roadmap.md) — decisions, history, phases, and open work for the semantic-kernel direction.
- [fragment-contracts](fragment-contracts.md) — how exact sub-function fragments are modeled: classification, contract, the wrapper-synthesis behavior oracle, the effect algebra, and fail-closed receiver identity.
- [formal-soundness](formal-soundness.md) — Lean 4 proof-obligation registry for proof-sensitive IL, normalization, fragment, and oracle contracts.
- [hazard-ranking](hazard-ranking.md) — the evidence base for the experimental `--sort hazard` (a divergence-*propensity* signal; **not** a validated harm ranker — it ranks actual harm near chance) and the honest evaluation trail.
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and labeled dataset hazard is measured against (repo selection, graded labels, quantitative sufficiency).
- [hazard-release-checklist](hazard-release-checklist.md) — what to do for the hazard ranking on every new nose release (one-page runbook: refresh the dataset, re-tune, re-validate).
- [experiments](experiments.md) — the measured log of what was tried and what happened.
- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [type4-benchmark](type4-benchmark.md) — the evidence-carrying synthetic Type-4 benchmark factory.
- [type4-adversarial-coverage](type4-adversarial-coverage.md) — focused Type-4 cases, target-packet task cards, and verifier-lead draft workflow.
- [frontier-platform](frontier-platform.md) — corpus-balanced evidence platform that ranks the next Type-4 axis by presence breadth (not raw count) and separates the queue signal from human-verified evidence.
- [field-evaluation](field-evaluation.md) — qualitative results from running nose on real third-party projects.
- [dogfooding](dogfooding.md) — nose run on its own source, and what its findings taught us.

The contributor workflow and quality gates live in
[CONTRIBUTING](../CONTRIBUTING.md); release history is in
[CHANGELOG](../CHANGELOG.md).
