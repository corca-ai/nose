# Formal core — machine-checked soundness obligations

nose's soundness contract is *fingerprint-equal => behavior-equal*. The interpreter oracle
(`nose verify`) checks that contract empirically against the pinned corpus; this directory
adds a machine-checked Lean 4 layer for the proof-sensitive rewrite rules.

The registry is directory-shaped:

```text
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

## Check

```sh
python3 scripts/check-formal-obligations.py
while IFS= read -r f; do
  lean --error=warning "$f"
done < <(find formal -name '*.lean' -print | sort)
```

`--error=warning` makes `sorry`, unused proof hints, and similar Lean warnings fail the
gate. The root `lean-toolchain` pins the Lean version used by CI and local checks.
