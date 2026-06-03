# nose documentation

**nose** finds syntax, semantic, and near-duplicate code clones across
eight languages — plus the `<script>` logic inside Vue, Svelte, and HTML — by
lowering every language into one normalized intermediate language (IL) and
ranking duplicated code by how much it's worth refactoring. The repository
[`README`](../README.md) is the one-screen overview; this wiki is the full guide.

Every page lives in `docs/` and links to its neighbours with relative links, so the
docs browse cleanly on GitHub and in any Markdown viewer.

## For users

Start here if you want to *run* nose on a codebase.

- [usage](usage.md) — install, the commands (`scan`, `il`, `stats`), every flag, and how to read the report.
- [clone-types](clone-types.md) — what nose covers across the standard Type-1/2/3/4 taxonomy, with its honest limits.
- [configuration](configuration.md) — the `nose.toml` file: excludes, thresholds, baselines, caching, inline `// nose-ignore`.
- [continuous-integration](continuous-integration.md) — the `--fail` gate, baseline-driven incremental adoption, SARIF, and fast re-runs.
- [languages](languages.md) — the supported languages and the embedded `<script>` extraction for Vue/Svelte/HTML.

## For contributors

Start here if you want to *change* nose or understand how it works.

- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [experiments](experiments.md) — the measured log of what was tried and what happened.
- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [field-evaluation](field-evaluation.md) — qualitative results from running nose on real third-party projects.
- [dogfooding](dogfooding.md) — nose run on its own source, and what its findings taught us.

The contributor workflow and quality gates live in
[`CONTRIBUTING`](../CONTRIBUTING.md); release history is in
[`CHANGELOG`](../CHANGELOG.md).
