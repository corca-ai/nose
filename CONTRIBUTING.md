# Contributing to nose

New to the codebase? Start with the [`docs/`](docs/home.md) wiki —
[Architecture](docs/architecture.md) for how it fits together,
[Normalization](docs/normalization.md) for the hard part, and
[Experiments](docs/experiments.md)/[Benchmark](docs/benchmark.md) for how
quality is measured.

## Quality gates

Run everything CI runs, locally, with one command:

```sh
./scripts/check.sh
```

A green run here is a green CI. The gates, in order:

| gate | command | what it enforces |
|---|---|---|
| **format** | `cargo fmt --all --check` | canonical rustfmt formatting |
| **lints** | `cargo clippy --all-targets --all-features -- -D warnings` | clippy clean; warnings are errors |
| **docs** | `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --workspace` | no broken/private intra-doc links |
| **build** | `cargo build --release` | the workspace compiles in release |
| **tests** | `cargo test --release` | the full suite, incl. cross-language convergence |
| **unused deps** | `cargo machete` | no dependency declared but unused (à la *knip*) |
| **supply chain** | `cargo deny check` | no advisories/yanked crates, only allowed licenses, no dup/wildcard deps, crates.io-only |
| **copy-paste** | `./scripts/check-duplication.sh` | nose run on its own source — substantial duplicate families stay within budget |
| **MSRV** | `cargo +$MSRV check --workspace` | the crates still build on the declared minimum Rust (`rust-version` in `Cargo.toml`) |

The dev/CI toolchain is pinned in `rust-toolchain.toml` (rustup installs it
automatically); the **MSRV** (`rust-version`, currently 1.85) is deliberately older
and checked by its own CI job. Bumping the MSRV is a conscious change — update
`Cargo.toml` and note why.

The lint policy is defined once in the root `Cargo.toml` under `[workspace.lints]`
and inherited by every crate via `[lints] workspace = true`.

### One-time tool install

`cargo-machete` and `cargo-deny` are optional (the runner skips them with a notice
if absent). To run them locally:

```sh
cargo install cargo-machete cargo-deny
```

### The copy-paste gate (dogfooding)

nose *is* a clone detector, so it polices its own duplication. The gate fails when
the number of substantial duplicate families (refactoring value ≥ 40) on the crates
exceeds the budget committed in `scripts/check-duplication.sh`. The currently
accepted families are reviewed and recorded in [`docs/dogfooding.md`](docs/dogfooding.md)
(e.g. the borrow-checker-blocked `generic` node-copy). If your change introduces a
new substantial family, either factor it out or — with a one-line justification in
the PR — raise the budget. It is a ratchet, not a fixed wall.

## Conventions

- **No `unsafe`** — the workspace forbids it (`unsafe_code = "forbid"`).
- **Convergence over coverage** — when adding or changing lowering, add an
  equivalence test (`crates/nose-cli/tests/equivalence.rs`) proving the new form
  converges with an existing one. A construct can lower cleanly yet to the *wrong*
  shape; the convergence tests are what catch that (see `docs/experiments.md` §S).
- **Determinism** — output must be byte-identical across runs and thread counts
  (there are tests for both). Don't introduce iteration over a `HashMap` in a way
  that reaches the output.
