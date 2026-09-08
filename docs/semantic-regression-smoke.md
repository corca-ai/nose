# Semantic output and runtime regression smoke

The semantic regression smoke is the pull-request tripwire for changes that can
alter product query output or make repository-scale analysis materially slower.
It compares release binaries from the base and head commits through one local/CI
entry point:

```sh
git fetch origin main
scripts/semantic-regression-smoke.sh \
  --base-ref origin/main \
  --head-ref HEAD
```

The GitHub Actions `semantic output · runtime smoke` job invokes that same script.
Its route step finishes after checkout when a diff changes only documentation or
other unrelated files. Changes under `crates/`, Cargo/toolchain inputs, the pinned
corpus and prune machinery, this gate, or its workflow run the full smoke.

## Pinned representative slice

The smoke reconstructs seven repositories at the exact commits in
`bench/goldens/corpus.json` and applies the checked
`bench/labels/prune_manifest.json` removals without rewriting the manifest.
`bench/semantic_regression_corpus.v1.json` also pins the selected repository ids
and the post-prune content digest, so a wrong removal hash, changed protected file,
new undeclared prune candidate, or different final subset fails before measurement:

| Language | Repositories | Purpose |
| --- | --- | --- |
| Ruby | `fastlane`, `asciidoctor`, `sidekiq` | Preserve the #804/#807 same-file redefinition failure boundary. |
| Rust | `alacritty` | Non-Ruby systems-language control. |
| Python | `requests` | Dynamic-language control without Ruby redefinition analysis. |
| Java | `junit5` | JVM/front-end control. |
| JavaScript | `prettier` | Large parser/tooling control. |

Every full smoke report records the selected repository commits, corpus, prune,
and subset-state SHA-256 values, post-prune content digest, base/head source SHAs,
release-binary SHA-256 values, execution environment, exact harness command, and
raw measurements.

The harness also supports exploratory runs without corpus manifests; those reports
explicitly record `corpus: null`, and the checker accepts them in its generic mode.
The merge smoke invokes the checker with `--require-corpus-provenance`, so an
exploratory report cannot be substituted for pinned-corpus CI evidence.

## Output policy

The harness runs the pinned semantic product query:

```text
nose query <repo> all top=0 --mode semantic --format json
```

It compares the raw output SHA-256 and byte count, family count, query schema
version, and family counts by product surface for every repository. The harness
runs from the selected corpus root and always passes the stable repository id (for
example, `fastlane`) to nose. Since family/member ids include path identity, this
stable relative invocation is what makes exact output declarations portable between
a workstation and GitHub Actions. Unexpected drift fails.

Base-view campaigns use `query . base=<commit> top=0 --format json`; the general-list
term `all` is not valid in that view. Their isolated worktree path is stable across
primary, control and focused phases because navigation commands retain the real
working directory. `--worktrees-root` can select a campaign-specific root; use the
same root for all phases and exact output audits. The default is
`target/query-regression-worktrees`. Each checkout is exclusively reserved and
removed after use. Existing paths and reservations fail closed rather than being
replaced. Producer hashes and the resolved root are recorded in report provenance;
controls and focused reruns must preserve them.
This preserves complete navigation output without filtering commands or changing
sampling and timing limits.

Strict provenance checks recognize the pinned base-workload contract separately
from ordinary pruned-corpus state. They require the workload manifest path/digest,
source-selection provenance, exact ordered repository/head/base tuples, a rebuilt
selection digest, and the harness/worktree producer hashes and reserved root.
Focused base reports must preserve the primary head and ancestor for every selected
repository. Missing or substituted fields fail; the absence of unrelated ordinary
prune-state fields does not invalidate a complete base-workload report.

An intentional change passes only when
`.github/semantic-regression-expected-drift.json` contains an exact declaration
for the comparison base SHA and repository. A declaration includes every changed
before/after value, a reason, and an issue. A blanket repository waiver is not
valid, and an active declaration with no matching drift also fails. Historical
entries become inactive when the comparison base advances, leaving an auditable
record without weakening later comparisons.

The checker prints the exact drift object on failure; copy it only after reviewing
the product change and replace the explanation with the real semantic reason.
Runtime safety is evaluated independently, so declaring an output change never
declares its cost acceptable.

## Runtime policy

The first pass takes five independent paired blocks after one warmup. Every
observation contains five command samples: odd samples follow the block's declared
base/head order and even samples reverse it. Each binary therefore runs in both
process positions. The harness takes the median at each position and averages the
two medians; all raw times and exact output observations remain in the report.
The base-vs-base control uses the same design. The sign test still has five
independent blocks, not twenty-five independent samples.

The [order-aware control contract](order-aware-performance-controls.md) evaluates
these position-neutral observations. A positive same-binary movement may reduce the
result; a negative control is diagnostic only and can never inflate it. A signal
crosses the material threshold only when the adjusted point estimate exceeds both:

- 5%; and
- 5 ms.

Exact sign-test support is also required to confirm a regression. The declared
block-order strata remain diagnostics: each multi-sample observation already
balances actual process position. A material point estimate without sufficient
block support remains inconclusive. The checker evaluates the aggregate, each
repository, and each reported stage. The smoke explicitly selects
`--runtime-gate elapsed-v1`: aggregate and per-repository elapsed time block release;
internal stages retain their measured states as diagnostic warnings. A faster
aggregate cannot hide a slower repository. The independent scaling tripwire below
and correctness/resource gates remain mandatory.

A first-pass elapsed-time threshold crossing or inconclusive result requests the affected
repositories (or the whole slice for an aggregate signal). Exactly one focused
comparison uses six independent blocks after one warmup, with the same five
samples per observation and its own matching same-binary control. Confirmed and
remaining inconclusive elapsed signals fail. There is no second focused loop.
Stage-only warnings do not request focus, and primary warnings outside the focused
subset remain visible. Historical checker invocations default to `all-metrics-v1`
and preserve their original verdicts. The prospective scope and rationale are
recorded in the [control contract](order-aware-performance-controls.md).

This prospective sampling change follows the retained single-sample failure in
[the 0.21 release evidence](release-evidence-0.21.0.md). It does not reclassify that
run or alter the estimator, thresholds, output policy, or independent-block count.
The CI job allows 45 minutes for the larger fixed measurement workload and both
release builds; that job timeout is separate from product performance limits.

## Deterministic Ruby scaling tripwire

`scripts/ruby-redefinition-scaling.py` generates the same Ruby source for fixed
64- and 256-case sizes and measures normalized IL. It rejects a material growth
exponent above 1.35. The fixture is small enough for every relevant PR, but its
many `nil?` receivers expose a repeated whole-file scan as superlinear growth.

The #804 failure boundary validates both layers:

| Comparison | Result |
| --- | --- |
| `d28d82d7` → `f968dcbd`, seven-repo one-iteration validation probe | 1,547.41 ms → 42,850.91 ms; +2,673.32% after same-binary adjustment; focused rerun required |
| `d28d82d7` → `f968dcbd`, five-iteration Ruby rerun | 391.56 ms → 27,744.71 ms; +6,984.01% after same-binary adjustment; confirmed hard failure |
| Intentional output ledger | `fastlane` and `sidekiq` changes accepted exactly; zero unexpected drifts; runtime still blocked |
| Ruby scaling, fixed/current implementation | growth exponent 0.70; passes |
| Ruby scaling, `f968dcbd` | growth exponent 2.43; fails |

The checker self-test also fixes the state machine: an initial material signal asks
for a focused rerun, a safe five-iteration rerun passes, and a confirmed material
rerun fails.

## Artifacts and prebuilt reproduction

GitHub uploads `target/semantic-regression/artifacts`, including:

- `context.json`, which distinguishes the compared checkout SHA (the PR merge SHA
  on `pull_request`) from the event's PR head SHA and records both worktree build
  commands;
- `primary.json` and `primary-control.json`;
- `focused.json` and `focused-control.json` when the first pass triggers;
- `check-status.json`, `ruby-scaling.json`, and the compact `summary.md` shown in
  the job summary.

Both base and head releases are built from detached worktrees at their recorded
SHAs. The base-vs-base control records the base source SHA on both sides. This keeps
the artifact truthful when the runner is invoked with a non-`HEAD` head ref or from
a dirty orchestration checkout.

To investigate historical or already-built binaries without rebuilding or cloning,
reuse the same runner and checker path:

```sh
scripts/semantic-regression-smoke.sh \
  --force \
  --base-ref <base-sha> \
  --head-ref <head-sha> \
  --baseline-binary /path/to/base/nose \
  --current-binary /path/to/head/nose \
  --repos-root bench/repos \
  --skip-setup
```

`--skip-setup` may reuse a checkout only when its repos root already contains the
matching `.nose-corpus-state.json`. The strict checker rejects a root without that
checked post-prune state. For an exploratory comparison without checked corpus
state, invoke the generic harness/checker mode directly; it is not merge evidence.

Use the broader 120-repository query regression and the
[runtime triage runbook](runtime-triage.md) when the bounded smoke identifies a
change that needs product-wide classification.

The Markdown summary lists confirmed and inconclusive stage signals together,
including when both occur in one focused comparison. Passing stages stay omitted;
whole-query and aggregate rows remain visible. A completed focused comparison is
reported neutrally and does not itself claim confirmation or a passing result.
The raw status JSON and original measurements remain authoritative.
Checked-in derived summaries are regenerated with this renderer; their original
measurement reports, checked status JSON and numeric result rows stay unchanged.
