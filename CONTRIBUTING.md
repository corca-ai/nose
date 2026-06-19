# Contributing to nose

New to the codebase? Start with the [`docs/`](docs/home.md) wiki —
[Architecture](docs/architecture.md) for how it fits together,
[Normalization](docs/normalization.md) for the hard part, and
[Experiments](docs/experiments.md)/[Benchmark](docs/benchmark.md) for how
quality is measured.

## Quality gates

Run the fast PR/push preflight locally before opening or updating a PR:

```sh
./scripts/check-ci-local.sh --fast
```

That runs rustfmt, the Rust file-length and CLI legacy-prelude ratchets, clippy
with warnings as errors, the `nose-cli` test suite, and the docs wiki lint. It
also self-tests the nightly corpus-verify runner without checking out the full
corpus. It is the gate meant to catch the common CI failures quickly.

Run everything CI runs, locally, with one command:

```sh
./scripts/check-ci-local.sh --full
```

`./scripts/check.sh` is kept as a backwards-compatible alias for `--full`. A green
full run here is a green CI. The full gates are:

| gate | command | what it enforces |
|---|---|---|
| **format** | `cargo fmt --all --check` | canonical rustfmt formatting |
| **file length** | `python3 scripts/check-file-lengths.py` | Rust files under `crates/` stay under the 600-line target unless they are existing ratcheted debt |
| **CLI prelude** | `python3 scripts/check-legacy-prelude.py` | top-level `nose-cli` modules and the temporary legacy-prelude export surface do not grow |
| **lints** | `cargo clippy --all-targets --all-features -- -D warnings` | clippy clean; warnings are errors |
| **docs** | `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --workspace` | no broken/private intra-doc links |
| **build** | `cargo build --release` | the workspace compiles in release |
| **tests** | `cargo test --release` | the full suite, incl. cross-language convergence |
| **coverage** | `cargo llvm-cov --workspace --summary-only --fail-under-lines 86` | line coverage stays above the ratchet floor (currently ~86%); runs before PR merge and release publishing |
| **copy-paste** | `./scripts/check-duplication.sh` | nose run on its own source, including tests — substantial duplicate family IDs match the reviewed baseline |
| **MSRV** | `cargo +$MSRV check --workspace --all-targets` | the crates still build on the declared minimum Rust (`rust-version` in `Cargo.toml`) |
| **unused deps** | `cargo machete` | no dependency declared but unused (à la *knip*) |
| **supply chain** | `cargo deny check` | no advisories/yanked crates, only allowed licenses, no dup/wildcard deps, crates.io-only |
| **docs wiki** | `awiki lint --root docs` | the `docs/` wiki is one connected graph — no orphan pages or islands |
| **formal obligations** | `python3 scripts/check-formal-obligations.py --self-test && python3 scripts/check-formal-obligations.py` | proof-sensitive Rust markers are registered, theorem names exist, and counterexample files are tracked |
| **formal proofs** | `./scripts/check-lean-proofs.sh` | Lean shared models and obligation proofs type-check with warnings, including `sorry`, treated as errors |

The dev/CI toolchain is pinned in `rust-toolchain.toml` (rustup installs it
automatically); the **MSRV** (`rust-version`, currently 1.85) is deliberately older
and checked by its own CI job. Bumping the MSRV is a conscious change — update
`Cargo.toml` and note why.

The lint policy is defined once in the root `Cargo.toml` under `[workspace.lints]`
and inherited by every crate via `[lints] workspace = true`. The tunable
thresholds (`cognitive-complexity-threshold`, `too-many-lines-threshold`,
`too-many-arguments`, `type-complexity`) live in `clippy.toml`. Both the clippy
thresholds and the coverage floor start lenient and are **ratchets** — tighten
them over time (lower the clippy thresholds, raise `--fail-under-lines`) as the
code is simplified and tests are added; never loosen them to make a red build
pass.

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

The broader refactoring policy lives in
[`docs/refactoring-ratchets.md`](docs/refactoring-ratchets.md).

### One-time tool install

`cargo-machete`, `cargo-deny`, `cargo-llvm-cov`,
[`awiki`](https://github.com/corca-ai/awiki), `elan`, and the MSRV Rust
toolchain are required for `--full`. `--fast` requires the Rust toolchain plus
`awiki`. Install the local CI tools with:

```sh
cargo install cargo-machete cargo-deny cargo-llvm-cov
rustup component add llvm-tools-preview   # cargo-llvm-cov needs this
brew install corca-ai/tap/awiki   # or: go install github.com/corca-ai/awiki/cmd/awiki@latest
curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh
rustup toolchain install 1.85
```

### Git hooks

Versioned hooks live in `.githooks`. Enable them once per clone:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook stays cheap: rustfmt plus docs wiki connectivity. The pre-push
hook runs `./scripts/check-ci-local.sh --fast`, which catches clippy/test/doc issues
before a branch reaches GitHub. Deliberately bypass it with:

```sh
NOSE_SKIP_PRE_PUSH=1 git push
```

### The duplication gate (dogfooding)

nose *is* a clone detector, so it polices its own duplication. The gate fails when
the substantial Type-3 near-duplicate family IDs (refactoring value ≥ 40, default
surface) on the crates differ from the reviewed baseline in
`scripts/duplication-baseline.json`. The scan includes tests as well as production
code, so fixture/scaffolding copy-paste is visible instead of hidden behind
file-length-only pressure. The currently accepted families are reviewed and recorded in
[`docs/dogfooding.md`](docs/dogfooding.md) (e.g. the borrow-checker-blocked `generic`
node-copy and reviewed test scaffolding). If your change introduces or removes a
substantial family, either factor it out or update the dogfooding review and baseline
in the same PR. It is a ratchet, not a fixed wall.

## Repository CI and automation

These are gates on *this* repository, distinct from running nose as a gate on your own
project (that user-facing guide is [continuous-integration](docs/continuous-integration.md)).

### Nightly pinned-corpus verify — the soundness moat

The scheduled `.github/workflows/corpus-verify.yml` gate guards soundness. Every night, and
on manual `workflow_dispatch`, it reconstructs the pinned benchmark corpus with
`bench/setup_repos.sh`, verifies the prune manifest, builds `target/release/nose`, and runs
every corpus repository through:

```sh
target/release/nose verify bench/repos/<repo> --max-violations 0
```

The runner is `scripts/corpus-verify-nightly.sh`. It shards by repository, keeps a per-repo
log under `target/corpus-verify-logs/`, writes a Markdown summary to the GitHub step summary,
and exits non-zero if any repo reports a hard false merge or a canon-preservation change.
Symbolic-trace disagreements stay advisory: the summary counts them, but they do not fail the
job. On failure the workflow uploads `target/corpus-verify-logs` as the `corpus-verify-logs`
artifact so triage starts from the failing repo output. It caches `bench/repos` with a key
derived from the pinned corpus manifest and prune scripts; a cold run still works because
`bench/setup_repos.sh` removes unpinned local entries, then reconstructs any missing or
drifted checkout. For a local spot check:

```sh
./scripts/corpus-verify-nightly.sh --repo arrow --repo click --jobs 2
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
  shape; the convergence tests are what catch that (see `docs/experiments.md` §S).
- **Determinism** — output must be byte-identical across runs and thread counts
  (there are tests for both). Don't introduce iteration over a `HashMap` in a way
  that reaches the output.

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
# 1. Cut the CHANGELOG: rename `## [Unreleased]` to `## [X.Y.Z] - <date>` and
#    open a fresh empty `## [Unreleased]` above it.
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
