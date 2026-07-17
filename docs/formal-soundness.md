# Formal soundness obligations

The runtime soundness check is described in [benchmark](benchmark.md); the rewrite pipeline
is in [normalization](normalization.md).

nose uses Lean 4 as a proof-obligation registry for semantic contracts whose soundness
should not depend only on corpus coverage. The registry lives under
[formal/obligations](../formal/obligations). Reusable Lean models live under
[formal/lib](../formal/lib), and each obligation directory contains:

- `meta.toml` — separate claim identity, modeled theorem, runtime preconditions and their
  status, product surface, executable evidence, related Rust files/symbols, and theorem names.
- `Proof.lean` — positive proof that the accepted rewrite preserves the modeled semantics.
- `Counterexamples.lean` — optional boundary proof for rewrites or missing preconditions that
  must stay closed.

The planned [semantic-kernel](semantic-kernel.md) keeps this registry as the
first-party proof boundary for exact laws. External semantic packs may declare
their own evidence status, but nose does not certify external packs; providers
and users own those claims unless the pack is adopted as first-party.

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
easy to miss. Those obligations must list the expected Rust files and symbols in
`meta.toml`; otherwise CI fails even if a Lean file exists somewhere else.

Every Rust-backed obligation must also be marked from the Rust side:

```rust
//! proof-obligation: normalize.recursion.structural_fold
```

The linter checks both directions. A marker without a matching `meta.toml` fails, and an
obligation whose `// proof-obligation: <id>` marker is absent from its `rust.files` fails. The
marker IS the obligation id — there is no `rust.markers` field to repeat it (a `canonicalize_*`
fn is likewise required to appear in some obligation's `rust.symbols`). This keeps
proof-sensitive code from drifting away from the registry.

Exact-normalization and canonicalization surfaces also carry an auto-discoverable claim marker:

```rust
// proof-claim: nose.claim.normalize.value_graph.factor_distribute
```

The claim id is derived from the obligation path. An unregistered marker, a claim marker in an
unlisted source file, or an exact/canonicalization obligation without its marker fails CI. The
linter self-test injects an unregistered exact claim to keep this reverse-index gate live.

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

## Theorem, precondition, and surface coverage

There is deliberately no top-level status. A claim records these dimensions independently:

- `[theorem]` says exactly what the Lean model proves (or marks a scoped theorem empirical);
- every `[preconditions.<id>]` labels a modeled or runtime precondition as `proven`,
  `empirical`, or `rejected`, with concrete evidence;
- `[product]` identifies whether the result affects exact normalization, declarative
  canonicalization, a near witness, a structural invariant, or a verification boundary;
- `[evidence]` links executable tests and counterexamples.

This matters for claims such as `detect.graded_witness`: its anti-unification theorem is proven,
while referent identity, decorators, sink alignment, and async lifecycle are four separately
visible empirical grade-demotion checks. The report prints theorem, precondition, and product-
surface counts instead of collapsing them into a misleading “proven obligations” total.

The former monolithic CSS empirical obligation is also gone. Lean now checks parsed color,
number/unit, box-shorthand, and Boolean query-order cores under `normalize.css.*`; parser/table
correspondence, custom-property token spelling, and the browser/cascade/DOM remainder stay
explicitly empirical and are linked to executable counterexamples. Safe Promise `.finally` and
literal-aggregate recovery are likewise separate empirical exact claims rather than being hidden
under the proven `.then`/`.catch` model.

## Local checks

```sh
python3 scripts/check-formal-obligations.py --self-test
python3 scripts/check-formal-obligations.py
./scripts/check-lean-proofs.sh
```

The proof script builds shared Lean modules into `target/lean` and then checks every
obligation proof with warnings as errors, so `sorry` and unused proof hints fail the gate.
