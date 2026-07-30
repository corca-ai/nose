# Test ownership: divergent-edit policy

Issue [#963](https://github.com/corca-ai/nose/issues/963) uses one bounded
tranche to establish the test-ownership rule without turning test count into a
target. The selected cohort is divergent-edit tiering: given normalized lane,
scope, and shared-logic evidence, decide the tier, taxonomy, reason codes, and
default CI gate behavior.

## Inventory

The issue-opening Cargo inventory reported 2,208 Rust tests, 996 under
`nose-cli`. A whitespace-aware static `#[test]` scan at the start of this
tranche found 272 inline tests under `nose-cli/src` and 724 integration-test
declarations under `nose-cli/tests`. The static counts are an ownership aid
rather than an alternative test-count metric.

| Responsibility | Representative current locations | Owner and intended layer |
|---|---|---|
| pure policy | query option/settings decisions, cache invalidation decisions, divergent-edit tiering | the smallest deterministic module that owns the vocabulary; no filesystem, process, renderer, or broad mock setup |
| analysis integration | `tests/equivalence/`, `tests/detection.rs`, `tests/connected_witness.rs`, inline query/detection tests | the crate that composes the real frontend, normalization, detection, and query collaborators |
| filesystem/process boundary | `tests/cli/cache/`, `tests/cli/watch.rs`, `tests/robustness.rs`, command/config fixtures | `nose-cli` integration tests using temporary repositories and the real binary |
| public output contract | query JSON/SARIF/baseline tests, mode/config precedence, representative user workflows | a bounded `nose-cli` unit or process test that observes exit status and versioned output rather than private calls |

The divergent-edit cohort now has these explicit owners:

| Surface | Owner | Preserved evidence |
|---|---|---|
| lane/scope/shared-evidence → tier, taxonomy, reasons, and gate | `nose-detect::divergence_policy` | four deterministic owner tests cover new-copy, unproven product evidence, test/mixed scope, and all product gate outcomes |
| detection/CLI evidence projection | `nose-cli::divergence::Divergence::policy_decision` | a focused adapter test fixes scope normalization and `Touched > NotTouched > Unproven`; the real adapter has no I/O or second policy table |
| closed schema and JSON/SARIF agreement | `nose-cli::divergence::tests` | the closed v8 enums and shared JSON/SARIF policy fields remain checked |
| binary, Git, exit, and output behavior | `nose-cli/tests/cli/query_base/tier_policy.rs` | mixed/test report-only, exact strict, semantic witness, varying-spot review, shared-logic strict, SARIF, and `--fail` behavior remain end to end |

No production input, schema field, reason string, tier, taxonomy, or gate rule
changed. The former CLI policy examples moved to the owner tests; the broad
adapter-call equivalence test was removed because the JSON/SARIF and process
contracts observe the same behavior without asserting private delegation.

## Local timing and diagnostics

Measurements used the repository debug profile on the same Apple Silicon host
with dependencies already present. Immediately before each focused command,
the owning production source file was touched so Cargo had to rebuild and link
the affected test target. Test bodies themselves completed in `0.00s`.

| Layout | Focused command | Cargo compile/link | Wall | Test executable |
|---|---|---:|---:|---:|
| before: policy inside the CLI finding model | `cargo test -p nose-cli --lib divergence::tests::v2_ -- --nocapture` | 16.39s | 121.09s | 56 MiB |
| rejected candidate: isolated `nose-eval` policy target | `cargo test -p nose-eval --test divergence_policy -- --nocapture` | 60.00s | 130.05s | 1 MiB |
| selected: cohesive `nose-detect` owner target | `cargo test -p nose-detect divergence_policy -- --nocapture` | 47.12s | 144.88s | 33 MiB |

The local wall-time objective did not improve: this host spent most of each run
before the new test executable entered `main`, and the smaller target did not
offset that launch cost. This tranche therefore makes no local-speed claim.
What improved is failure ownership and target scope: a policy regression now
names one of four evidence-to-decision behaviors in the cohesive detector owner
instead of starting from the 272-test pre-move CLI unit binary. The smaller
`nose-eval` candidate was rejected because benchmark evaluation does not own
query-surface product policy. Hosted workspace compile/test timings are recorded
on the pull request and must remain green; a future tranche should be accepted
for speed only with a repeatable hosted or local win.

## Rules for later tranches

- Move a cohort only when its input and observable result can be named without
  CLI argument, filesystem, process, or renderer state.
- Keep at least one real-binary test for every touched precedence, exit-status,
  JSON, SARIF, baseline, or cache-correctness contract.
- Prefer an existing cohesive owner. Do not create a crate merely to obtain a
  smaller test binary.
- Record compile/link and execution evidence before and after. A test-count
  decrease is neither required nor sufficient.
- Treat cache, determinism, soundness, and schema changes as product changes;
  they do not belong in a relocation-only tranche.
