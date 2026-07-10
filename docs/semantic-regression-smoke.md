# Semantic output and runtime regression smoke

The semantic regression smoke is the pull-request tripwire for changes that can
alter product query output or make repository-scale analysis materially slower.
It compares release binaries from the base and head commits through one local/CI
entry point:

```sh
git fetch origin main
scripts/semantic-regression-smoke.sh \
  --base-ref origin/main \
  --head-ref HEAD
```

The GitHub Actions `semantic output · runtime smoke` job invokes that same script.
Its route step finishes after checkout when a diff changes only documentation or
other unrelated files. Changes under `crates/`, Cargo/toolchain inputs, the pinned
corpus and prune machinery, this gate, or its workflow run the full smoke.

## Pinned representative slice

The smoke reconstructs seven repositories at the exact commits in
`bench/goldens/corpus.json` and applies the checked
`bench/labels/prune_manifest.json` removals without rewriting the manifest:

| Language | Repositories | Purpose |
| --- | --- | --- |
| Ruby | `fastlane`, `asciidoctor`, `sidekiq` | Preserve the #804/#807 same-file redefinition failure boundary. |
| Rust | `alacritty` | Non-Ruby systems-language control. |
| Python | `requests` | Dynamic-language control without Ruby redefinition analysis. |
| Java | `junit5` | JVM/front-end control. |
| JavaScript | `prettier` | Large parser/tooling control. |

Every report records the selected repository commits, corpus and prune manifest
SHA-256 values, base/head source SHAs, release-binary SHA-256 values, execution
environment, exact harness command, and raw measurements.

## Output policy

The harness runs the pinned semantic product query:

```text
nose query <repo> all top=0 --mode semantic --format json
```

It compares path-canonicalized output SHA-256 and byte count, family count, query
schema version, and family counts by product surface for every repository. The
raw, non-canonicalized output hash and byte count remain in the artifact; replacing
the checkout-specific absolute repository root with `<repo>` makes an exact drift
declaration portable between a workstation and GitHub Actions. Unexpected drift
fails.

An intentional change passes only when
`.github/semantic-regression-expected-drift.json` contains an exact declaration
for the comparison base SHA and repository. A declaration includes every changed
before/after value, a reason, and an issue. A blanket repository waiver is not
valid, and an active declaration with no matching drift also fails. Historical
entries become inactive when the comparison base advances, leaving an auditable
record without weakening later comparisons.

The checker prints the exact drift object on failure; copy it only after reviewing
the product change and replace the explanation with the real semantic reason.
Runtime safety is evaluated independently, so declaring an output change never
declares its cost acceptable.

## Runtime policy

The first pass takes two measurements with opposite base/head execution order and
runs a base-vs-base same-binary control on the same corpus. It records wall time
and every stage emitted by `NOSE_TIME=1`. A signal crosses the material threshold
only when its
control-adjusted increase is both:

- greater than 5%; and
- greater than 5 ms.

The checker evaluates the aggregate, each repository, and each reported stage.
A repository or stage can therefore fail even when faster controls dilute the
aggregate.

A first-pass threshold crossing is not yet a hard regression failure. It exits
with the dedicated focused-rerun status, selects the affected repositories (or the
whole slice for an aggregate signal), and repeats five alternating measurements
after one warmup with another same-binary control. Only a material signal confirmed
by that focused run fails the runtime comparison.

## Deterministic Ruby scaling tripwire

`scripts/ruby-redefinition-scaling.py` generates the same Ruby source for fixed
64- and 256-case sizes and measures normalized IL. It rejects a material growth
exponent above 1.35. The fixture is small enough for every relevant PR, but its
many `nil?` receivers expose a repeated whole-file scan as superlinear growth.

The #804 failure boundary validates both layers:

| Comparison | Result |
| --- | --- |
| `d28d82d7` → `f968dcbd`, seven-repo one-iteration validation probe | 1,547.41 ms → 42,850.91 ms; +2,673.32% after same-binary adjustment; focused rerun required |
| `d28d82d7` → `f968dcbd`, five-iteration Ruby rerun | 391.56 ms → 27,744.71 ms; +6,984.01% after same-binary adjustment; confirmed hard failure |
| Intentional output ledger | `fastlane` and `sidekiq` changes accepted exactly; zero unexpected drifts; runtime still blocked |
| Ruby scaling, fixed/current implementation | growth exponent 0.70; passes |
| Ruby scaling, `f968dcbd` | growth exponent 2.43; fails |

The checker self-test also fixes the state machine: an initial material signal asks
for a focused rerun, a safe five-iteration rerun passes, and a confirmed material
rerun fails.

## Artifacts and prebuilt reproduction

GitHub uploads `target/semantic-regression/artifacts`, including:

- `primary.json` and `primary-control.json`;
- `focused.json` and `focused-control.json` when the first pass triggers;
- `check-status.json`, `ruby-scaling.json`, and the compact `summary.md` shown in
  the job summary.

To investigate historical or already-built binaries without rebuilding or cloning,
reuse the same runner and checker path:

```sh
scripts/semantic-regression-smoke.sh \
  --force \
  --base-ref <base-sha> \
  --head-ref <head-sha> \
  --baseline-binary /path/to/base/nose \
  --current-binary /path/to/head/nose \
  --repos-root bench/repos \
  --skip-setup
```

Use the broader 120-repository query regression and the
[runtime triage runbook](runtime-triage.md) when the bounded smoke identifies a
change that needs product-wide classification.
