# Pre-epic repository readiness (#948)

This record fixes the scope and evidence contract for the bounded cleanup before
the next product epic. It covers repository gate ownership, two high-churn Rust
module boundaries, and checked-in evidence artifact lifecycle. Product behavior,
soundness, determinism, and supported performance contracts stay unchanged.

## Baseline

The starting point is `main` at `bf5bd86e`:

- `scripts/check-ci-local.sh` has 29 named gates and 132 direct Python
  invocations;
- the Rust file-length gate checks 947 files with a 599-line default maximum
  and no per-file budgets;
- 68 Rust files are at least 560 lines;
- the checked-in tree is 180.5 MiB, with 502 JSON/JSONL files and 19 files at
  least 1 MiB;
- formatting, documentation, and file-length checks pass.

These are diagnostic measurements, not goals to game. The cleanup must leave
the same repository contracts behind a clearer, reproducible operating model.

## Gate inventory contract (#949)

The named `scripts/check-ci-local.sh --gate <name>` entry remains the executable
policy boundary. A checked machine-readable registry will describe every entry:
owner, implementation, tools, inputs, effects, cache behavior, lanes, lane
rationale, and focused command. The registry validator will compare that
inventory with the shell dispatcher and GitHub workflow invocations so metadata
cannot silently become a second, drifting command owner.

Timing evidence separates a clean-tree run with existing build caches from an
immediate no-change rerun. It records each named gate reached by `--fast` and
`--full`, plus total wall time and the local environment. Build/setup time
remains visible rather than being folded into policy claims.

The checked receipt at `ea4afaf3` covers all 30 registered gates with zero
failures and zero worktree drift. On the recorded arm64 macOS environment,
clean-tree fast took 943.682 seconds, clean-tree full took 940.274 seconds, and
the immediate no-change fast rerun took 861.685 seconds. Debug CLI tests
(523.703/509.551 seconds) and regression self-tests
(236.791/238.501 seconds) dominate both fast runs. The 82-second incremental
improvement is primarily the cached clippy gate (68.772 to 0.208 seconds), not
removed validation. The full-only leading costs are MSRV qualification
(303.229 seconds), release tests (109.171 seconds), and release build
(78.857 seconds). These measurements support the current tiering: the expensive
checks are product-contract or qualification work, while orchestration and
artifact checks remain sub-second and are worth keeping in fast feedback.

## Rust cohort contract (#950)

The cohort was selected before implementation from production files at least
560 lines, ranked by commits touching the path since 2026-06-01. It is limited
to two roots with both high change frequency and a concrete ownership split.

### `nose-detect/src/units.rs`

- Baseline: 596 lines and 145 touching commits.
- Current responsibilities: unit extraction orchestration, root collection,
  exact/value gates, post-extraction enrichment, and the public graded-witness
  value-DAG lookup.
- Planned boundary: move the independently callable, normalize-then-lookup
  value-DAG adapter into an owned `units::dags` module. Keep extraction state
  and gates in the root; share only the root-collection and value-context
  helpers required by both paths.
- Direct consumers: detect orchestration and the CLI graded-witness enrichment.
- Focused evidence: `nose-detect` tests, `nose-cli` query/equivalence tests,
  Type-4 executable expectations, query-schema checks, and semantic regression.

### `nose-semantics/src/packs.rs`

- Baseline: 599 lines and 72 touching commits.
- Current responsibilities: public pack façade, manifest-facing model types and
  deserialization, summary conversion, and `SemanticPackSet` construction.
- Planned boundary: keep the root as the public façade, move manifest-facing
  models and summary conversion to `packs::model`, and move pack-set assembly
  and accessors to `packs::set`.
- Direct consumers: semantic-pack loading/lock/conformance internals,
  frontend/normalize semantic evidence, and CLI pack commands/query metadata.
- Focused evidence: `nose-semantics` tests, semantic-pack examples and pricing,
  live query schema, Type-4 expectations, and semantic regression.

The extraction must preserve public paths through re-exports, add no public API
without a named consumer, and leave each selected root more than 20 lines below
the 599-line ceiling. The global ceiling is lowered only if the complete
near-cap cohort, not just these two files, makes that honest.

The implemented boundaries leave `units.rs` at 503 lines and `packs.rs` at 209
lines; the four new owned modules are all below the existing ceiling. The
public `unit_dags_at` and semantic-pack paths remain available through façade
re-exports. The Type-4 blind-attacker receipt was replayed because it binds the
complete `crates/` Git tree identity: only `product_crates_tree` changed from
`dc4e9280eed90ba7ec090473bc47086ab735a89f` to
`e45a98a6c5f20aa71068fa93b42104551f3e50e0`. The replay retained 54 exact
groups, 0 false merges, 0 canon-preservation violations, 86 advisory
disagreements, and all summary/exclusion counts exactly.

## Evidence lifecycle contract (#951)

The lifecycle catalog covers every tracked file at least 1 MiB and every
checked-in artifact set consumed by PR, nightly, release, soundness, benchmark,
or documentation validation. Large files receive explicit size and SHA-256
bindings. Active families may share inherited lifecycle metadata through
catalog sets, but receipts, seals, checksums, baselines, and their bound data
must have explicit relations.

The validator fails on missing large-file entries, stale paths or digests,
invalid lifecycle metadata, broken relations, unowned active derived output,
and globs that no longer match. Domain validators remain authoritative for
semantic content; the lifecycle check verifies provenance and maintenance
coverage rather than replacing them.

Retention is conservative:

- canonical, gold, sealed, receipt, active-baseline, historical, and
  published-claim evidence stays in Git unless an explicitly reviewed migration
  preserves equivalent auditability;
- reproducible derived output needs a producer and validator;
- superseded output is removable only after all consumers and relations are
  cleared;
- lack of a current CI reference is not by itself evidence for deletion.

The implemented
[`scripts/evidence/artifacts.json`](../scripts/evidence/artifacts.json) catalog
owns 17 exact-inventory sets covering 511 checked JSON, JSONL, and checksum
artifacts. It explicitly binds all 19 files at least 1 MiB and records 28
receipt, seal, checksum, baseline, supersession, closeout, provenance, and
soundness-manifest relations. The
[lifecycle policy and retention audit](evidence-artifact-lifecycle.md) records
why all 19 large artifacts remain: every candidate still carries gold, sealed,
baseline, closeout, release-reconstruction, or published-claim value, so no
deletion met the conservative removal contract.

## Post-readiness follow-up

The next-epic handoff cleanup preserved every unmerged line of work while
removing 28 merged local branches and six merged remote branches. `main` was
fast-forwarded and pruned before the implementation branch was created.

The monolithic `regression-selftests` gate is replaced by four ownership-based
gates: `default-head-evidence`, `divergence-evidence`,
`surface-recall-evidence`, and `runtime-soundness-evidence`. Local fast/full
plans retain the complete checks in deterministic order, while GitHub Actions
runs the four gates as independent jobs. The registry now contains 33 gates;
fast covers 23 and full covers 31. The checked post-readiness measurement at
`1bfce491` records 23/23 clean fast gates in 458.469 seconds, 31/31 clean full
gates in 621.739 seconds, and 23/23 no-change fast gates in 460.272 seconds,
with zero failures and zero worktree drift. The four evidence gates total
237.326 seconds in the clean local sequence, while their longest independent
job is the 213.988-second default-head gate.

A later cache-isolation follow-up at source commit `9481f3ec` removed the
duplicate frontier-platform corpus check from the docs gate while retaining it
under `type4-frontier`, and moved MSRV artifacts to `target/msrv/`. Its checked
receipt records clean fast at 412.252 seconds, clean full at 382.548 seconds,
and no-change fast at 364.204 seconds, again with complete gates, no failures,
and no worktree drift. The docs gate fell from 45.948 to 0.583 seconds; the
warm isolated MSRV gate fell from 224.451 to 0.170 seconds, while a focused
empty-cache MSRV run took 17.343 seconds. The next measured bottleneck is
`default-head-evidence` at 214.754 seconds on clean fast.

A parallel-planning follow-up at source commit `b8db7fda` moved the five
independent default-head mutation self-tests behind a bounded three-worker pool.
A focused run passed in 143.550 seconds, 71.204 seconds (33.2%) below the
preceding 214.754-second serial result. Local fast/full plans remain sequential
by default; `--jobs N` opts into the registry planner, whose declared
dependencies, parallel-safety barriers, and resource groups prevent build
consumers and shared-output owners from colliding. Aggregate local timing was
not recorded because macOS policy inspection independently delayed locally
built Rust test binaries during the measurement; hosted CI owns the complete
gate qualification.

The 599-line `crates/nose-cli/tests/cli/support.rs` test helper is now a
13-line façade over focused `fixtures.rs` (210 lines), `process.rs` (149
lines), and `query.rs` (245 lines). Existing `support::*` call sites remain
valid through crate-visible re-exports. Process-diagnostic tests stay beside
their owner and continue to execute real CLI processes rather than mock command
results.

Because the support split changes the complete `crates/` Git tree identity, the
Type-4 blind-attacker receipt was replayed. Only `product_crates_tree` changed,
from `e45a98a6c5f20aa71068fa93b42104551f3e50e0` to
`ab781571dd0a1513e2a7e4239b606e9bd0d307af`; the result remains 54 exact
groups, zero false merges, zero canon-preservation violations, and 86 advisory
disagreements. The self-query corpus also moved the already reviewed
`int_bin`/`float_bin` family below the value-40 threshold, so its stale
duplication ID was removed and the accepted count tightened from 29 to 28
without adding a replacement family.

## Completion evidence

The tranche closes only when:

1. the gate registry, timing receipt, lifecycle catalog, and both validators
   pass their self-tests and live checks;
2. the selected Rust boundaries pass focused crate and product-contract tests;
3. documentation and dogfooding deltas are reviewed explicitly;
4. `./scripts/check-ci-local.sh --fast` and `--full` pass from a clean tree;
5. #949, #950, and #951 are closed before the parent #948 is closed.
