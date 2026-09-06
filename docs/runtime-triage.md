# Runtime triage

Runtime triage turns a query-regression report into a reproducible performance decision:
which repos are expected capability cost, which are noisy, and which need a focused fix.
Use it before optimizing a slow repo by hand.

## Rust import prescreen follow-up (2026-09-06)

The ten-cycle usability campaign left Alacritty `parse+lower` qualification
inconclusive. This follow-up targets an independently observed frontend cost;
it does not reclassify that earlier result as a confirmed regression.
A native single-worker `stats` sample of the unchanged 88-file Alacritty corpus
shows runtime-type lookup repeatedly traversing enclosing CST scopes and their
children. Ordinary unqualified types enter Tokio import/shadow checks even when
no matching runtime import exists.

Rust lowering now first checks for an asserted imported-binding record with the
same local name, module and export required by the existing resolver. Absence
cannot produce an accepted runtime domain, so it can return before CST traversal.
Possible matches still undergo all original visibility, namespace shadowing,
local type shadowing, ambiguity and dependency checks. The local type-shadow
check follows successful import resolution. There is no cached negative result;
each lookup sees the current evidence, including newly lowered imports. Qualified
Tokio paths retain their original path. No admission rule or cache schema changes.

The pre-change binary is retained from `c5dad268` in
`target/frontend-performance-20260906/baseline-nose`. The exploratory six-pair
Alacritty query comparison records median `parse+lower` 91.80 → 53.25 ms and
whole-query 166.15 → 133.65 ms. These are unadjusted medians; a positive control
movement is not counted as additional speedup. All twelve semantic output hashes
match. The exploratory checker requests a focused rerun for `normalize+extract`,
whose +3.95 ms order-adjusted movement has conflicting order strata; this is not
an all-stages-passing result. Final pinned-corpus qualification is recorded below
after the source commit and remaining verification.

Raw IL JSON is byte-identical for all 88 Alacritty Rust files and 25 runtime-type
fixtures, including import aliases and shadowing negatives. Existing frontend
runtime-domain tests and all 2,371 workspace tests pass. The native sample,
original commands, paired/control runs, raw IL hash ledger and candidate patch
are retained under `target/frontend-performance-20260906/`. The exploratory
measurement records the dirty candidate tree explicitly; final smoke binds
committed source SHAs and binary digests.

Final qualification compares `c5dad268` with product commit `fe069b3f` using
`scripts/semantic-regression-smoke.sh` and the seven pinned repositories, after
other verification workloads finish. It **passes on the primary run**, with
zero declared/unexpected output drift and no triggered or inconclusive runtime
signals. Alacritty median `parse+lower` is 87.90 → 54.40 ms (38.1% reduction);
whole-query median is 168.59 → 132.11 ms (21.6% reduction). These are raw median
comparisons on this corpus and machine, not a general Rust speed guarantee.
Ruby scaling passes at exponent 0.68. The exploratory order conflict and the
earlier campaign's failed qualification remain historical results; neither was
rewritten or retried until green. Final raw measurements, provenance and the
checker result are in `target/frontend-performance-20260906/smoke/`.

Strict clippy, docs, the 1,054-file length gate and the unchanged 19-family
duplication baseline (budget 20) pass. Type-4 records 54 exact groups with zero
false merges/canonicalization violations; `4bf44b83` binds the receipt to the
new crates tree. Fast local CI completed its product/test gates, then found the
corresponding stale Type-4 inventory digest. After reviewing that sole receipt
binding change, `1fa45bb8` updates only the inventory digest; the focused evidence
artifact gate passes. No quality threshold, evidence result or public contract
was relaxed.

## When Required

Run this process when a PR, release candidate, or post-release follow-up changes semantic
admission, lowering, normalization, query ranking/filtering, or corpus-scale analysis
behavior and the broad query-regression run reports a meaningful runtime increase.

Start with the PR-sized [semantic regression smoke](semantic-regression-smoke.md).
It owns the automatic 5%/5 ms base/head decision and focused rerun; this runbook is
the deeper classification step after that bounded gate identifies a material signal.

The broad gate answers whether product output or runtime moved. Run
`scripts/check-query-regression.py` on that artifact to make the no-behavior-change
decision mechanical: hashes, byte counts, and family counts must stay identical, while
runtime is compared against configured percentage and absolute thresholds using the documented
[same-binary control contract](order-aware-performance-controls.md) for attribution.
Positive control drift may reduce a primary movement, while a negative independent
control never inflates it. Runtime triage answers why a remaining measured slowdown
moved.
Do not skip the classification step just because a repo looks slow: capability-growth
cost, measurement noise, lower/front-end cost, and value-graph hot paths call for
different actions.

## Harness

For a post-release stabilization or performance pass, use the official binary
asset from the most recent non-prerelease GitHub release as the product
baseline. Verify the published archive checksum, record the extracted binary's
SHA-256, and name the release tag and commit in the harness provenance. A local
rebuild of the release tag is useful for diagnosis but is not the release
baseline. A pre-change `main` binary may be retained as a secondary attribution
comparison; it does not replace the official release binary.

Record native build provenance as well as source and binary digests. On Mach-O, capture
`LC_BUILD_VERSION`'s minimum OS and SDK plus the `__text` size. Source-identical C/C++
parser code can differ when an official release and a local candidate use different
SDKs or linker/code-generation environments; do not attribute that difference to a
frontend source change without call-path or normalized-disassembly evidence.

Use `scripts/runtime-triage-harness.py` when comparing two binaries on one or more corpus
repos. It runs `nose query <repo> all top=0 --mode semantic --format json` with
`NOSE_TIME=1` and `NOSE_TIME_UNIT_SUMMARY=1`, stores raw runs, aggregates median stage
timings, records representative top unit summaries, and classifies each repo.

Example:

```sh
python3 scripts/runtime-triage-harness.py \
  --baseline-binary /tmp/nose-v0.16.0-target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref v0.16.0 \
  --current-source-ref HEAD \
  --all-repos \
  --iterations 5 \
  --warmups 1 \
  --output target/runtime-triage-all-repos.json
```

Run the built-in parser/classifier self-test after changing the script:

```sh
python3 scripts/runtime-triage-harness.py --self-test
```

For no-behavior-change work, run the query-regression checker on the broad artifact:

```sh
python3 scripts/check-query-regression.py \
  target/query-regression.json \
  --same-binary-control target/same-binary-control.json \
  --max-runtime-delta-pct 5
```

## Classifications

The harness writes `schema: nose.runtime_triage_harness.v1` and classifies each repo:

- `not-reproduced`: current median is not slower than baseline.
- `small-or-noisy`: runtime delta is below the configured percentage or absolute threshold.
- `capability-growth`: family count increased; first report runtime cost per additional
  surfaced family before optimizing.
- `no-family-growth-value-hot-path`: family count did not grow and representative unit
  value time dominates.
- `no-family-growth-lower-or-frontend`: family count did not grow and lower/front-end
  stage delta dominates.
- `no-family-growth-mixed-hot-path`: family count did not grow, but timing is split across
  stages and needs narrower instrumentation before a fix.

Both runtime harnesses record two binary identities. The full-file SHA-256 owns
artifact provenance. The code SHA-256 owns regression identity: on thin
little-endian Mach-O binaries it zeros `LC_UUID` and the ad-hoc code-signature
blob before hashing, because Darwin relinks otherwise identical executable code
with fresh values in those fields. Other binaries use their full-file digest as
their code digest. `check-query-regression.py` treats equal code digests as a
same-binary comparison, while still retaining distinct artifact digests in the
report. `scripts/binary_identity.py --self-test` pins this normalization.

The default thresholds are intentionally conservative:

- `--regression-pct 20`
- `--small-absolute-ms 25`
- `--hot-unit-ms 20`

## Timing Knobs

Use the timing knobs in increasing order of verbosity:

- `NOSE_TIME=1`: top-level discover/lower/normalize/query stage timings.
- `NOSE_TIME_UNIT_SUMMARY=1`: per-file/per-unit-kind aggregate extraction timings.
- `NOSE_TIME_UNITS=1`: individual unit timings for units taking at least 10ms.
- `NOSE_TIME_NORMALIZE=1`: per-normalization-pass timings by file.
- `NOSE_TIME_VALUE_GRAPH=1`: per-value-graph build timings split into seed,
  immutable-binding seeding, inline-candidate setup, process, and finish phases.

`NOSE_TIME_VALUE_GRAPH=1` is intentionally verbose. Use it only after
`NOSE_TIME_UNIT_SUMMARY=1` has identified a repo/file/unit-kind worth inspecting.

## Process

1. Run `scripts/query-regression-harness.py` for broad product-level elapsed timing.
2. Run `scripts/check-query-regression.py` to fail any unexpected product-output drift.
3. Run `scripts/runtime-triage-harness.py` on the largest remaining runtime regressions.
4. Optimize only `no-family-growth-*` cases with clear stage attribution.
5. For `capability-growth`, report cost per newly surfaced family before changing code.
6. Record the artifact under `bench/recall_loss/` or `target/` and link the summary doc.

Criterion microbenchmarks are diagnostic only. Use their absolute intervals and
a freshly named baseline; do not treat an unnamed cached local `change` result
as a product performance verdict. Product-level alternating runs, output hashes,
same-binary controls, and focused triage remain authoritative.

The triage is complete when every selected repo has a recorded classification, any
code change names the stage it is intended to improve, and the follow-up document
links both the broad query-regression artifact and the focused runtime-triage artifact.

The [0.17.0 post-release runtime triage](runtime-triage-0.17.0.md) is the first
documented use of this process.

The [20-optimization runtime pass](runtime-performance-20-optimizations-2026-07-02.md)
records the first longer optimization sequence using this process, including the
same-binary noise control, all-120-repo before/after artifact, and focused recheck of
the largest apparent regressions.

## Cortex first-analysis follow-up (2026-09-06)

The local comparison is `71b44a89` to `d55665b2`, on Cortex
`0baac1230c442aeb7109aadbe035bec729321ff1`. It measures a new process without
nose analysis-cache reuse; the operating-system page cache was not flushed.
The query is `nose query cortex --format json` from the repositories' parent.
Both binaries use the same root, modes, thresholds, and automatic candidate policy.

Six alternating pairs reduced median elapsed time from 8,963.22 ms to 5,449.67 ms
(39.20%). The paired order-aware movement was -3,487.53 ms (-38.91%); the
same-binary control had a -20.53 ms movement, which cannot inflate an improvement.
All twelve result byte hashes match. The contiguous stage fell from 3,499.45 ms
to 65.00 ms. Sampling and stage timings identified repeated token extension of
long runs whose remaining source spans could never satisfy the existing line floor.
Conservative suffix bounds avoid that extension while retaining first-occurrence
seeds, including seeds in one-line code that can match later multiline code.
The bounds include the whole current block to tolerate nonmonotonic source spans.

A separate comparison of `all top=0` in the default mode set preserved every
output byte on Cortex and all seven pinned smoke repositories. Regression tests
also preserve first-seed behavior and exercise valid runs with nonmonotonic spans,
empty streams, and streams without operations. All 2,352 workspace tests, strict
Clippy, formatting, docs, file-length, and the unchanged 18-family duplication
ratchet passed. No source/feature cache schema changed. An additional equal-slice
Jaccard shortcut was measured, showed no product speed improvement, and was removed.

The required seven-repository semantic smoke preserved output exactly and passed
Ruby scaling (exponent 0.67). Its runtime gate did **not** pass: after its single
permitted focused rerun, asciidoctor remained inconclusive at +4.53 ms (+3.35%)
with order strata of +7.78 ms (+5.66%) and +1.28 ms (+0.98%). No confirmed
material regression was found, and no threshold or retry policy was changed.

The Cortex focused check also remains **inconclusive**, despite the repeated total
improvement (8,993.10 ms to 5,576.83 ms). Its adjusted stage movements were
+16.33 ms (+4.91%) for rendering, +9.70 ms (+5.22%) for family ranking, and
+9.63 ms (+5.54%) for rank mapping; their order strata disagree. The first six
focused blocks were insufficient because the primary already used six. Exactly
two more blocks were appended to each focused comparison to satisfy the checker's
strictly larger-sample requirement. Original reports were preserved, and extension
provenance records the reason, script hash, environment, and appended iterations.
No observations were replaced, and no further performance rerun was attempted.
The blind attacker retained 54 exact groups with zero false merges or canonicalization
violations. These correctness results do not convert the runtime gates into passes.

Raw measurements, code/binary identities, controls, complete output comparisons,
and smoke reports are retained in `target/first-analysis-performance-2026-09-06/`.
The process-local timing and sampling originals are in `/tmp/nose-first-run-perf/`.
Candidate scoring still takes about 2.3 seconds and parsing/normalization about
2 seconds on Cortex; this improvement does not establish interactive latency or
lower peak memory use.

## Scoring and feature extraction follow-up (2026-09-06)

The next local product comparison is `8d939004` to `0696c0b5`, using the same
clean Cortex commit and query as above. Every observation starts a new process
without nose analysis-cache reuse; the operating-system page cache is not flushed.
This measures the next optimization against the preceding local product, not a
release-baseline qualification. Five alternating pairs and a separate five-pair
same-binary control were followed by exactly one six-pair focused comparison and
control, each with one warmup. No thresholds, observations, or retry limits changed.

| Stage | Primary baseline/current median | Focused baseline/current median |
| --- | --- | --- |
| Whole query | 5,589.89 / 4,840.73 ms | 5,904.43 / 5,082.16 ms |
| Candidate scoring | 2,318.80 / 1,620.00 ms | 2,465.85 / 1,658.65 ms |
| Parse and lower | 866.80 / 825.60 ms | 919.25 / 903.55 ms |
| Normalize and extract | 1,148.30 / 1,087.20 ms | 1,192.50 / 1,177.30 ms |

The whole-query median reduction is 13.40% in the primary and 13.93% in the
focused comparison. The focused paired order-aware movement is -812.29 ms
(-13.76%); its same-binary movement is -16.58 ms and does not inflate improvement.
Scoring improves consistently. The normalization benefit is small and does not
hold in every aggregation: its focused paired movement is +9.70 ms, despite the
lower marginal median. These results do not establish uniformly faster stages.

Profiling identified repeated scoring of exactly equal feature inputs, sequential
joins between small parallel batches, repeated MinHash work, and whole-arena
parent searches in the Object.keys guard. The implementation uses complete typed
score-input equality (including metadata), private bounded ordered-pair memo maps,
parallel chunks without sequential joins, corpus-wide shared signature computation,
and a lazy unique-parent index. Full equality checks resolve hash collisions;
custom scorers opt out by default. Candidate order, nesting checks, rejected scores,
thresholds, semantic guards, and connected-seed selection remain intact. No feature
or persistent-cache schema changes. The [architecture](architecture.md) owns the
implementation and lifetime contracts.

All 22 primary/focused output hashes match. Full `all top=0` output also matches
byte-for-byte on Cortex and all seven pinned smoke repositories. Four Cortex cache
paths agree: uncached, previous-binary cold cache, current-binary reuse of that
cache, and current-binary cold cache. Only the expected cache argument in suggested
`next` commands differs across cache locations; reuse of the same cache is byte-identical.
All 2,356 workspace tests, strict Clippy, formatting, docs, file-length checks, and
the reviewed 18-family duplication ratchet pass. The blind attacker retains 54 exact
groups, zero false merges, and zero canonicalization violations.

Both runtime qualification gates remain **failed due to inconclusive evidence**.
Cortex's focused contiguous stage moves +4.10 ms (+6.11%) with disagreeing order
strata; groups moves +14.25 ms (+9.93%) with insufficient sign-test support.
The seven-repository smoke preserves output exactly and passes Ruby scaling
(exponent 0.68), but focused asciidoctor remains inconclusive at +5.31 ms (+4.09%),
with order strata +11.95 and -1.32 ms. Neither inconclusive result is proof of a
regression or a passing performance result. No further performance rerun was made.

Commands, source/binary identities, raw reports, output comparisons, sampling,
and validation logs are retained in `target/scoring-first-analysis-performance-2026-09-06/`.
Original experiments remain in `/tmp/nose-score-perf/`. The improvement reduces
first-analysis work; it does not establish interactive latency or lower peak memory.

## v0.21.0 candidate performance follow-up (2026-09-06)

The release candidate's NO-GO remains the starting point for this optimization.
The immutable pre-change product binary is `target/release-0.21.0/candidate-nose`,
bound to `283f9e1d` (the same crates tree as preparation commit `78f9feac`).
The checksum-verified published v0.20.0 remains the release baseline; the older
incomplete dense-bucket candidate policy is not restored to recover its timing.

Sampling attributed the dense Alamofire near-query cost to repeated structural
scores and expanding/sorting overlapping candidate neighborhoods. Exact input
classes alone still repeated work at batch boundaries. The new compressed rows
also require identical candidate-bucket membership and connected-seed eligibility.
They count rejected, ineligible location pairs without materializing them; original
location checks, all accepted edges, score direction, source order, connected ties,
and explicit candidate budgets remain intact. The [architecture](architecture.md)
owns the full execution and memory contracts.

Query-list rendering now indexes primary membership lazily once per selection,
constructs independent JSON rows in indexed parallel order, and moves completed
location/family arrays into the report. The final JSON is serialized into one
output buffer. Every field, navigation command and row order remains part of the
byte-equivalence comparison; no payload fields are omitted to recover throughput.

Exploratory variants and raw measurements are retained under
`target/release-performance-20260906/`. They are diagnostic observations, not a
replacement for release qualification. The intermediate class-row and quotient
Alamofire outputs match the complete pre-existing diagnostic output byte for byte;
that comparison alone does not establish a passing release performance gate.
The final product is `d8744855`, crates tree
`333f18c66b8b43bd9c38f974139ac7edd5a3e847`, binary SHA-256
`bce2cd46a3f917979c18039e7352b0e4013e9cd2ed488224b9488b8d0bcd3d9a`.
The checked [performance follow-up](../bench/release/0.21.0/performance-followup.v1.json)
retains source/binary bindings, every semantic replay hash and raw evidence seals.

One isolated Alamofire `all top=0 --mode near:0.8 --format json` observation took
111.89 seconds on the frozen pre-change candidate and 8.91 seconds on the new
candidate. The complete output bytes match. Scoring moved 109,783.1 → 7,541.5 ms;
rendering moved 961.9 → 270.9 ms. These are diagnostic observations, not paired
release qualification or a general speed guarantee. The published v0.20.0 binary
still took 1.61 seconds under its earlier incomplete candidate policy. A bitmap
neighbor experiment and a branchless Jaccard experiment were excluded; the latter
made scoring slower in the exploratory comparison.

Local `--full` passes: 2,373 optimized tests, 89.39% line coverage, strict Clippy,
MSRV, Lean and the unchanged 19-family duplication result (budget 20). All 120
semantic outputs match the frozen candidate byte for byte. Default and near
outputs also match on the seven smoke repositories and Cortex `0baac123`, plus
the dense Alamofire near case. The source-class tests compare against exhaustive
pairs, including asymmetric scores, nesting, equal spans, mixed bucket membership,
connected eligibility, overflow ties and multiple thread/batch sizes.

Cache correctness passes all 2,100 mutation rows and 180 paired SymPy rows.
Thirty alternating replays per binary measure the following elapsed times:

| SymPy phase | Published/candidate p50 | p50 change | Published/candidate p95 |
| --- | --- | --- | --- |
| Clean | 2,393.53 / 2,507.41 ms | +4.76% | 2,479.98 / 2,663.68 ms |
| Empty cache | 2,593.66 / 2,698.68 ms | +4.05% | 2,706.55 / 2,842.23 ms |
| History reuse | 315.21 / 310.10 ms | −1.62% | 334.47 / 323.84 ms |

The history-reuse regression clears, but clean and empty-cache p95 still exceed
the unchanged 5%/5 ms limits. Watch passes 30 revisions each at 10k/100k files,
full fresh-query equivalence and forced restart, with ready p95 76.39/383.22 ms.

Release qualification remains **NO-GO**. The local seven-repository runtime gate
has no confirmed material signal but remains inconclusive after its single
focused run, including Asciidoctor/JUnit5 frontend stages. The
[remote CI](https://github.com/corca-ai/nose/actions/runs/34027976008) focused run
confirms Asciidoctor `normalize+extract` +12.90 ms/+10.82% and Sidekiq `lower`
+6.20 ms/+7.00%, with additional inconclusive signals. Neither gate reports
unexpected output drift. The reviewed schema changes were also bound to the
actual published tag `47adbab7`; the previous ledger covered PR base `de43f4b4`.
The existing primary/control measurements were rechecked without replacing them,
then exactly one focused run was performed. Thresholds and retry policy remain
unchanged.

The same product tree passes fresh [120-repository soundness](https://github.com/corca-ai/nose/actions/runs/34028031591)
with zero false merges/canonicalization violations and
[independent deep checks](https://github.com/corca-ai/nose/actions/runs/34028033026).
All [four native packages](https://github.com/corca-ai/nose/actions/runs/34027976126)
and the actual CI installer pass checksum/extraction/execution checks.

The registered full 120-repository timing and 17-repository base campaign remain
unqualified. Dense class-to-class scoring and frontend/normalization costs still
need work before a replacement candidate can close those release conditions.
No release tag or Homebrew update was published.


## v0.21.0 release completion work (2026-09-06)

The follow-up candidate `d8744855` remains the frozen comparison for correctness;
its failed release qualification above is retained. New diagnostic work lives in
`target/release-completion-20260906/`. Release baselines, timing limits and the
registered full-corpus campaign remain unchanged.

The current work removes redundant syntax-budget traversal using stored subtree
counts, shares exact multiset intersections across dense score rows, and counts
source-admitted candidate pairs from row sizes and sparse span multiplicities.
It also reduces connected-seed selection costs without changing caps or tie rules,
and batches arena invalidation during alpha-renaming and branch orientation.
These mechanisms are documented in [architecture](architecture.md) and
[normalization](normalization.md). Intermediate binaries and single observations
are diagnostic only; replacement candidate qualification is still pending.
