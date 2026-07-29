# Contributing to nose

This page owns the contributor workflow, local quality gates, repository
automation, and release process for nose. The root
[`CONTRIBUTING.md`](../CONTRIBUTING.md) stays as a short entry point.

New to the codebase? Start with the [docs wiki](home.md) —
[Architecture](architecture.md) for how it fits together,
[Normalization](normalization.md) for the hard part, and
[Experiments](experiments.md)/[Benchmark](benchmark.md) for how quality is
measured.

## Quality gates

Run commands on this page from the repository root unless a section says
otherwise.

Run the fast PR/push preflight locally before opening or updating a PR:

```sh
./scripts/check-ci-local.sh --fast
```

That runs the common source-quality checks, cheap checked-evidence and runner
self-tests, the `nose-cli` test suite, and live debug-binary product-contract
checks. It is the gate meant to catch common CI failures before push.

Run everything CI runs, locally, with one command:

```sh
./scripts/check-ci-local.sh --full
```

`./scripts/check.sh` is kept as a backwards-compatible alias for `--full`. A
green full run here is a green CI. The gate implementations live behind named
`scripts/check-ci-local.sh --gate <name>` entries; GitHub Actions supplies
runner setup and invokes the same entries.

Use `./scripts/check-ci-local.sh --list-gates` for the complete current list,
owners, lanes, worktree effects, caches, and focused rerun commands. The
[repository gate inventory](repository-gates.md) documents the checked registry,
timing receipt, lane policy, and drift validation. This generated view replaces
a hand-maintained table here so the contributor guide cannot silently diverge
from executable policy.

The dev/CI toolchain is pinned in `rust-toolchain.toml` (rustup installs it
automatically); the **MSRV** (`rust-version`, currently 1.85) is deliberately older
and checked by its own CI job. Bumping the MSRV is a conscious change — update
`Cargo.toml` and note why.

The lint policy is defined once in the root `Cargo.toml` under `[workspace.lints]`
and inherited by every crate via `[lints] workspace = true`. The tunable
thresholds (`cognitive-complexity-threshold`, `too-many-lines-threshold`,
`too-many-arguments`, `type-complexity`) live in `clippy.toml`. Both the clippy
thresholds and the coverage floor start lenient and are **ratchets** — tighten
them over time as the code is simplified and tests are added; never loosen them
to make a red build pass.
The coverage floor itself lives in `scripts/coverage-threshold.env` so local and
GitHub CI cannot drift.

The file-length gate is a design ratchet, not a formatter preference. New Rust
files under `crates/` must stay below 600 lines (the enforced default max is 599).
Existing files above that target are listed in `scripts/file-length-budgets.json`
at their current line count; they may not grow, and any refactor that shrinks one
must lower its budget in the same change. CI compares the budget file with the
base ref, so `default_max_lines`, existing file budgets, and new over-target
budget entries cannot be loosened in the same change. Use it to force incremental
module extraction and clearer ownership, not to split files mechanically.

The local preflight uses `origin/main` as the no-loosening baseline for that
budget file and fails if the ref is missing; run `git fetch origin main` if the
gate asks for it.

The broader refactoring policy lives in [refactoring-ratchets](refactoring-ratchets.md).

### Runtime regression triage

For semantic-kernel, lowering, normalization, query, or corpus-scale behavior
changes, run the bounded [semantic regression smoke](semantic-regression-smoke.md)
against the PR base. The CI job uses the same script, exact output-drift ledger,
same-binary control, and focused-rerun policy as local reproduction. If the smoke
shows a meaningful increase, run the broad query-regression gate and use
the [runtime triage runbook](runtime-triage.md) to classify the affected repos
as noise, capability-growth cost, lower/front-end cost, value-graph cost, or a mixed
hot path.

Do not optimize a repo-level slowdown until the classification is recorded. For
capability-growth cases, report runtime cost per newly surfaced family first; for
no-family-growth cases, name the measured stage before changing code. Link the
focused artifact from the issue, PR, or release evidence page so future maintainers
can distinguish expected semantic expansion cost from accidental degradation.

### One-time tool install

`cargo-machete`, `cargo-deny`, `cargo-llvm-cov`, `shellcheck`,
[`awiki`](https://github.com/corca-ai/awiki), `elan`, and the MSRV Rust
toolchain are required for `--full`. `--fast` requires the Rust toolchain plus
`awiki` and `shellcheck`. Install the local CI tools with:

```sh
cargo install cargo-machete cargo-deny cargo-llvm-cov
rustup component add llvm-tools-preview   # cargo-llvm-cov needs this
brew install shellcheck
brew install corca-ai/tap/awiki   # or: go install github.com/corca-ai/awiki/cmd/awiki@latest
curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh
rustup toolchain install 1.85
```

### Git hooks

Versioned hooks live in `.githooks`. Enable them once per clone:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook stays cheap: rustfmt plus docs wiki connectivity. The
pre-push hook first runs [`scripts/prune-cargo-target.sh`](../scripts/prune-cargo-target.sh)
to remove stale `target/debug/deps/*.rcgu.o` files when the Cargo object
directory has grown large, then runs `./scripts/check-ci-local.sh --fast`.
That catches clippy/test/doc issues before a branch reaches GitHub and avoids
macOS code-signing stalls from very large local Cargo target directories.
Deliberately bypass it with:

```sh
NOSE_SKIP_PRE_PUSH=1 git push
```

Set `NOSE_PRUNE_CARGO_TARGET=0` to keep the pre-push quality gate but skip only
the target prune step.

### The duplication gate (dogfooding)

nose *is* a clone detector, so it polices its own duplication. The gate fails when
the substantial Type-3 near-duplicate family IDs (refactoring value ≥ 40, default
surface) on the crates differ from the reviewed baseline in
`scripts/duplication-baseline.json`. The scan includes tests as well as production
code, so fixture/scaffolding copy-paste is visible instead of hidden behind
file-length-only pressure. The current gate workflow is documented in
[dogfooding](dogfooding.md), and the accepted-family decision trail is
recorded in [dogfooding history](dogfooding-history.md) (e.g. the
borrow-checker-blocked `generic` node-copy and reviewed test scaffolding). If your
change introduces or removes a substantial family, either factor it out or update the
dogfooding review and baseline in the same PR. It is a ratchet, not a fixed wall.

## Repository CI and automation

These are gates on *this* repository, distinct from running nose as a gate on your own
project (that user-facing guide is [continuous integration](continuous-integration.md)).

### Nightly pinned-corpus verify — the soundness moat

The scheduled `.github/workflows/corpus-verify.yml` gate guards soundness. Every night, and in
manual `nightly` or `release` mode, it partitions the pinned benchmark corpus into four jobs,
reconstructs each exact subset with `bench/setup_repos.sh`, builds `target/release/nose`, and
runs every corpus repository through:

```sh
target/release/nose verify bench/repos/<repo> --max-violations 0
```

The runner is `scripts/corpus-verify-nightly.sh`. It keeps per-repository diagnostics separately
from the byte-deterministic TSV, Markdown, repository selection, and JSON evidence artifact. The
merge job rejects an incomplete, missing, or overlapping shard, contradictory status/exit row,
missing artifact, wrong pin, per-repository timeout, hard false merge, or canon-preservation
change. Every shard evidence artifact, diagnostic artifact, and merged artifact is uploaded on
success as well as failure. Symbolic-trace
disagreements stay advisory, but all per-repository deltas from the official v0.19.0 baseline are
retained instead of being reduced to a total. The weekly deep job separately runs source-runtime,
metamorphic equivalence, and multi-seed falsification campaigns; manual `release` mode requires
that deep evidence and the full nightly evidence to belong to the same commit. See
[Soundness Lab](soundness-lab.md#ci-nightly-deep-and-release-gates-862) for the complete contract.
For a local spot check:

```sh
./scripts/corpus-verify-nightly.sh --repo arrow --repo click --jobs 2 --timeout-seconds 900
```

### External review bots

CodeRabbit repository automation is disabled with the root `.coderabbit.yaml`. The file opts
out of inherited CodeRabbit settings, turns off automatic and incremental review, leaves no
keyword/label trigger for review opt-in, excludes all paths from review scope, and disables
review statuses, summaries, chat auto-replies, finishing touches, pre-merge checks, issue
enrichment, knowledge-base retention, external knowledge sources, and built-in review tools.

That YAML is the repository-owned control. CodeRabbit documents that manual
`@coderabbitai review` commands can still trigger a review regardless of auto-review settings
while the app has repository access. The CodeRabbit GitHub App is installed at the `corca-ai`
organization level, so a hard block still requires an organization owner to change the app
installation from "all repositories" to a selected-repositories installation that excludes
`corca-ai/nose`, or to uninstall CodeRabbit from the organization.

## Conventions

- **No `unsafe`** — the workspace forbids it (`unsafe_code = "forbid"`).
- **Convergence over coverage** — when adding or changing lowering, add an
  equivalence test (`crates/nose-cli/tests/equivalence.rs`) proving the new form
  converges with an existing one. A construct can lower cleanly yet to the *wrong*
  shape; the convergence tests are what catch that (see [experiments](experiments.md) §S).
- **Determinism** — output must be byte-identical across runs and thread counts
  (there are tests for both). Don't introduce iteration over a `HashMap` in a way
  that reaches the output.

## Changelog discipline

Update `CHANGELOG.md` in the same PR as any notable user-facing or operator-facing
change. Use the top `## [Unreleased]` section and keep entries short enough that
release cutting is mostly a mechanical rename.

Add an entry when a PR changes any of these:

- CLI behavior, flags, output wording that users rely on, or help text.
- Machine-readable contracts such as query JSON, capabilities, SARIF, baseline files, ignore files,
  or config keys.
- Detection behavior, ranking, default surface policy, recall/precision gates, or supported
  languages/domains.
- CI/release/benchmark workflows that contributors or operators run directly.
- Performance in a way users would notice or we measured and want release readers to see.
- Breaking behavior or migration steps, even pre-1.0.

Skip the changelog for purely internal refactors, tests-only changes, typo-only docs edits, and
metadata churn that does not affect a user, operator, or release reader. If a PR intentionally has
no changelog entry, say so in the PR body so reviewers do not have to infer it.

## Releasing

Releases are cut by [cargo-dist](https://opensource.axo.dev/cargo-dist/): push a
`vX.Y.Z` tag matching the workspace version and CI does the rest. Artifact builds may run
while the repository quality gates (`.github/workflows/ci.yml`, reused through
`workflow_call`) are still running, but publishing is blocked until those gates pass. Only
then does the workflow publish a GitHub Release with the macOS (Apple Silicon + Intel) and
Linux (x86_64 + arm64) archives + checksums and push the `nose` formula to
[`corca-ai/homebrew-tap`](https://github.com/corca-ai/homebrew-tap) so
`brew install corca-ai/tap/nose` picks up the new version.

```sh
# 1. Review `## [Unreleased]`, then cut the CHANGELOG: rename it to
#    `## [X.Y.Z] - <date>` and open a fresh empty `## [Unreleased]` above it.
# 2. Bump `version` in the root Cargo.toml ([workspace.package]) — the internal
#    path deps share it — and land both in the release commit.
# 3. Tag the release commit and push the tag:
git tag vX.Y.Z
git push origin vX.Y.Z
```

The tag is what triggers CI, so the CHANGELOG and version bump must land **before**
it — a tag pushed against a stale `[Unreleased]` ships a release the changelog never
records.

The cargo-dist pipeline lives in `dist-workspace.toml`; the artifact-building jobs in
`.github/workflows/release.yml` are generated from it. The repository-owned quality-gate
job at the top of that workflow is a local publishing guard; preserve it if regenerating
the cargo-dist workflow. Publishing the formula needs the `HOMEBREW_TAP_TOKEN` secret
(a token with push access to the tap), set on the repo/org.
