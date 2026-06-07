# Formal core — machine-checked soundness obligations

nose's soundness contract is *fingerprint-equal => behavior-equal*. The interpreter oracle
(`nose verify`) checks that contract empirically against the pinned corpus; this directory
adds a machine-checked Lean 4 layer for proof-sensitive semantic contracts.

The registry is directory-shaped:

```text
formal/lib/
  *.lean                    # shared models used by obligations

formal/obligations/<area>/<subsystem>/<rule>/
  meta.toml
  Proof.lean
  Counterexamples.lean    # optional, but required when meta.toml lists counterexamples
```

The obligation id is the dot-joined directory path. For example,
`formal/obligations/normalize/value_graph/factor_distribute/meta.toml` must declare:

```toml
id = "normalize.value_graph.factor_distribute"
```

Proof-sensitive Rust rule modules follow the same name. A file such as
`crates/nose-normalize/src/value_graph/rules/factor_distribute.rs` must have the matching
obligation directory above, and `meta.toml` must set `rust.rule_module = true`. The linter
checks this pairing mechanically, so a new named semantic rule cannot land without a
registered proof obligation.

Some semantic surfaces are required even when they are not one-file rule modules. The linter
currently requires obligations for IL arena validity/deep-copy, recursion rewrites, exact
fragment effect/place and wrapper contracts, and the oracle cutoff.

Every Rust-backed obligation must carry a comment marker — `// proof-obligation: <obligation id>`
— in one of its listed Rust files. The marker IS the obligation id, so there is no separate
`rust.markers` field (the linter derives the expected marker from the id):

```rust
//! proof-obligation: detect.fragment.effect_place
```

The linter also scans Rust files in the other direction: a marker with no matching
obligation, or a marker in a file not named by that obligation's `rust.files`, fails CI.

## Registered proof families

- `normalize.value_graph.algebra` — associative-commutative numeric algebra,
  subtraction-as-add-neg, negation distribution, and distributivity over `Int`.
- `normalize.value_graph.factor_distribute` — the named Rust rule module for
  `x*f + y*f -> (x+y)*f`, gated to proven numeric leaves.
- `normalize.value_graph.free_monoid` — ordered string/list builder concatenation:
  associative with identity, not commutative, and not a ring for distribution.
- `normalize.value_graph.compare` — comparison direction, negated comparisons, and
  total-order lattice canons.
- `normalize.control_flow.guard_returns` — guard-return, dead-code-after-return, and
  ternary-return control-flow canons.
- `normalize.value_graph.functor` — map/filter fusion and count-of-filter.
- `normalize.value_graph.bool_reduce` — any/all Bool reductions.
- `normalize.value_graph.min_max` — min/max select idioms and reductions.
- `normalize.value_graph.clamp` — clamp equivalences plus boundary counterexamples.
- `normalize.value_graph.field_writes` — final field-state semantics, last-write-wins,
  distinct-field commutativity, and same-field order counterexample.
- `normalize.recursion.tail` — tail-recursive self-call elimination into a while loop.
- `normalize.recursion.structural_fold` — numeric structural recursion as an accumulator
  fold for `+` and `*`, with counterexamples for subtraction and wrong identities.
- `detect.fragment.effect_place` — append/index effects need no receiver proof; field
  writes require an exact-safe place and reject `Unknown`.
- `detect.fragment.free_inputs` — wrapper parameters are reads minus fragment-local
  bindings.
- `detect.fragment.wrapper_synthesis` — wrapper arity and synthesized body arena bounds.
- `il.arena.validity` / `il.arena.deep_copy` — structural IL bounds invariants used by
  validation and wrapper deep-copying.
- `oracle.cutoff` — oracle-mode normalization stops before semantic canonicalizations.

## Check

```sh
python3 scripts/check-formal-obligations.py
./scripts/check-lean-proofs.sh
```

`--error=warning` makes `sorry`, unused proof hints, and similar Lean warnings fail the
gate. Shared `formal/lib` modules are compiled into `target/lean`; the root
`lean-toolchain` pins the Lean version used by CI and local checks.
