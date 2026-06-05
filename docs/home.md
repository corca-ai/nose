# nose documentation

**nose** finds syntax, semantic, and near-duplicate code clones across
eight languages — plus the `<script>` logic inside Vue, Svelte, and HTML — by
lowering every language into one normalized intermediate language (IL) and
ranking duplicated code by how much it's worth refactoring. The repository
[`README`](../README.md) is the one-screen overview; this wiki is the full guide.

Every page lives in `docs/` and links to its neighbours with relative links, so the
docs browse cleanly on GitHub and in any Markdown viewer. The pages are grouped by
what you're here to do.

## Start here

- [getting-started](getting-started.md) — install, run your first scan, and learn
  to read the report in a few minutes. **The friendly on-ramp; read this first.**

## Using nose

You want to *run* nose on a codebase and act on what it finds.

- [usage](usage.md) — the complete command and flag reference (`scan`, `stats`, `il`), the ranking keys, and the scan modes.
- [configuration](configuration.md) — the `nose.toml` file: excludes, thresholds, baselines, caching, inline `// nose-ignore`.
- [continuous-integration](continuous-integration.md) — the `--fail` gate, baseline-driven incremental adoption, SARIF, and fast re-runs.
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

- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [hazard-ranking](hazard-ranking.md) — the evidence base and design for ranking clones by divergent-edit risk (a severity axis, complementing extractability).
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and labeled dataset that hazard ranking is calibrated and validated against (repo selection, graded labels, quantitative sufficiency).
- [experiments](experiments.md) — the measured log of what was tried and what happened.
- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [type4-benchmark](type4-benchmark.md) — the evidence-carrying synthetic Type-4 benchmark factory.
- [field-evaluation](field-evaluation.md) — qualitative results from running nose on real third-party projects.
- [dogfooding](dogfooding.md) — nose run on its own source, and what its findings taught us.

The contributor workflow and quality gates live in
[`CONTRIBUTING`](../CONTRIBUTING.md); release history is in
[`CHANGELOG`](../CHANGELOG.md).
