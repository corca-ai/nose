# Formal soundness obligations

Back to [home](home.md). The runtime soundness check is described in
[benchmark](benchmark.md); the rewrite pipeline is in [normalization](normalization.md).

nose uses Lean 4 as a proof-obligation registry for semantic contracts whose soundness
should not depend only on corpus coverage. The registry lives under
[`formal/obligations`](../formal/obligations). Reusable Lean models live under
[`formal/lib`](../formal/lib), and each obligation directory contains:

- `meta.toml` — the id, status, related Rust files/symbols, theorem names, assumptions, and
  optional counterexample theorem names.
- `Proof.lean` — positive proof that the accepted rewrite preserves the modeled semantics.
- `Counterexamples.lean` — optional boundary proof for rewrites or missing preconditions that
  must stay closed.

The obligation id must match its path. For example,
`formal/obligations/normalize/value_graph/factor_distribute/meta.toml` declares
`normalize.value_graph.factor_distribute`.

## Semantic namespaces

The obligation path names the product contract, not the Rust source path. Use semantic
namespaces such as:

- `il.arena.*` — structural IL invariants such as arena bounds and deep-copy validity.
- `normalize.*` — behavior-preserving canonicalizations, including value-graph and
  recursion-to-iteration rewrites.
- `detect.fragment.*` — exact-fragment contracts, effect/place proof boundaries, free
  inputs, and wrapper synthesis.
- `oracle.*` — behavioral-oracle independence contracts such as the normalization cutoff.

The linter has a required-surface list for proof-sensitive areas whose omission would be
easy to miss. Those obligations must list the expected Rust files, symbols, and markers in
`meta.toml`; otherwise CI fails even if a Lean file exists somewhere else.

Every Rust-backed obligation must also be marked from the Rust side:

```rust
//! proof-obligation: normalize.recursion.structural_fold
```

The linter checks both directions. A marker without a matching `meta.toml` fails, and a
`meta.toml` whose `rust.markers` entry is not present in one of its `rust.files` fails. This
keeps proof-sensitive code from drifting away from the registry.

## Named rule modules

For new proof-sensitive rewrites, prefer a named Rust rule module instead of adding another
case inside a large canonicalizer function. The current standard is:

```text
crates/nose-normalize/src/value_graph/rules/<rule>.rs
formal/obligations/normalize/value_graph/<rule>/meta.toml
```

The linter checks that every file in `value_graph/rules/*.rs` has a matching obligation and
that the matching obligation sets `rust.rule_module = true`. This makes omission visible:
a new named semantic rule cannot be added without registering its proof state.

For proof-sensitive rewrites that are not value-graph rule modules, prefer the same shape:
put the rule-specific recognition/emission in a named module and mark that module with the
obligation id. Recursion now follows this pattern with `recursion/tail.rs` and
`recursion/structural_fold.rs`.

## Statuses

- `proven` — Lean proof file and theorem names are present and type-check.
- `covered` — the rule is covered by another registered obligation.
- `missing` — the obligation is acknowledged but not proved yet.
- `empirical-only` — currently guarded by the interpreter oracle or tests only.
- `rejected-counterexample` — the registry records why a tempting rewrite must stay closed.

## Local checks

```sh
python3 scripts/check-formal-obligations.py --self-test
python3 scripts/check-formal-obligations.py
./scripts/check-lean-proofs.sh
```

The proof script builds shared Lean modules into `target/lean` and then checks every
obligation proof with warnings as errors, so `sorry` and unused proof hints fail the gate.
