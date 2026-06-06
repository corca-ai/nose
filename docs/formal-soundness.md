# Formal soundness obligations

Back to [home](home.md). The runtime soundness check is described in
[benchmark](benchmark.md); the rewrite pipeline is in [normalization](normalization.md).

nose uses Lean 4 as a proof-obligation registry for semantic rewrites whose soundness should
not depend only on corpus coverage. The registry lives under
[`formal/obligations`](../formal/obligations), and each obligation directory contains:

- `meta.toml` — the id, status, related Rust files/symbols, theorem names, assumptions, and
  optional counterexample theorem names.
- `Proof.lean` — positive proof that the accepted rewrite preserves the modeled semantics.
- `Counterexamples.lean` — optional boundary proof for rewrites or missing preconditions that
  must stay closed.

The obligation id must match its path. For example,
`formal/obligations/normalize/value_graph/factor_distribute/meta.toml` declares
`normalize.value_graph.factor_distribute`.

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

## Statuses

- `proven` — Lean proof file and theorem names are present and type-check.
- `covered` — the rule is covered by another registered obligation.
- `missing` — the obligation is acknowledged but not proved yet.
- `empirical-only` — currently guarded by the interpreter oracle or tests only.
- `rejected-counterexample` — the registry records why a tempting rewrite must stay closed.

## Local checks

```sh
python3 scripts/check-formal-obligations.py
while IFS= read -r f; do
  lean --error=warning "$f"
done < <(find formal -name '*.lean' -print | sort)
```

The full local CI mirror runs both checks. Lean warnings are errors, so `sorry` and unused
proof hints fail the gate.
