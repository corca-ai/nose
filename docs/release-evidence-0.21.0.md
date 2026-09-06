# 0.21.0 candidate qualification

Updated on 2026-09-07. Release decision: **NO-GO (performance qualification remains incomplete)**. The feature scope is frozen;
remaining changes address qualification, packaging or a reproduced release blocker.
No release tag or Homebrew publication is part of this preparation.

## Upgrade from 0.20.0

- Ordinary query JSON uses schema 10; base queries retain schema 8. Integrations
  should inspect `nose capabilities`, support nullable region/review keys and
  read the [query contract](query-json.md). Do not parse human output.
- Existing family/member navigation handles retain their meaning. A content key
  is not occurrence identity, ancestry or review approval. See [region identity](region-identity.md).
- Cache artifacts whose versions changed rebuild automatically. The first cached
  run after upgrade can cost a full analysis. Preserve unrelated files and use
  the existing cache commands; manual deletion is not required.
- Saved analyses are separate from rebuildable caches. Retain original captures
  referenced by caller reviews. Older captures without optional handle/diagnostic
  metadata remain readable with explicit unavailable/not-recorded status; older
  strict readers can reject newly extended artifacts. Do not rewrite captures to
  make a review appear applicable.
- Semantic packs with a compatibility upper bound below 0.21 remain incompatible.
  Providers should revalidate before extending their range. Changed manifests
  require regenerated project locks and applicable conformance receipts; changing
  the bound alone does not authorize external-exact influence. Shipped examples
  are revalidated on this candidate.

## Latest candidate: exact score reuse

The [scoring follow-up record](../bench/release/0.21.0/scoring-followup.v1.json)
binds optimization `25230ae5`, test cleanup `deeb1eaa` and proof `5d1610f7`.
It avoids redundant unhinted-header parsing, reuses exact scoring inputs and
accelerates repeated additions while retaining sequential IEEE-754 results.
The fixture cleanup produces a byte-identical executable. All 360 ordinary and
17 base outputs match the preceding candidate; saved-analysis/review journeys
and 0.20 cache upgrade also pass.

Full local CI passes 2,392 tests, 89.56% line coverage, strict Clippy, MSRV,
supply-chain checks and Lean. The original full run's duplicated test setup was
removed through a shared fixture; eighteen accepted families remain within
budget twenty. The original failure is retained. Cache passes 2,100 mutation
rows and 180 paired SymPy observations. Watch passes 30 revisions at both sizes,
fresh-query equality and forced restart, with ready p95 81.08/386.29 ms.

[Remote CI](https://github.com/corca-ai/nose/actions/runs/34053175158) remains
inconclusive after its single Asciidoctor/Fastlane focus. Asciidoctor
`normalize+extract` has an adjusted +9.40 ms/+7.53% movement, four supporting
blocks of six and sign-test p=0.34375; no focused signal is a confirmed regression.
This still fails the unchanged gate. PR, nightly and deep soundness pass. All
four native packages and the actual installer are independently verified. The
Intel Mac package's first upload failed with GitHub DNS ENOTFOUND after its
build/smoke passed; only that failed package job was retried.

Balanced semantic diagnostics reduce Raylib from 2.42 to 2.02 seconds and libGDX
from 1.66 to 1.47 seconds against the previous candidate. Delve remains 0.50
seconds versus published 0.20's 0.26 seconds. Group/ranking stage costs also
remain. The new broad release runtime campaign is unqualified; these diagnostics
never replace its registered primary/control and focused policy.

An isolated parser experiment reduces the Delve diagnostic from 0.50 to 0.24
seconds with identical output. Its admission decisions match complete parsing
for 6,294 C/header files and 10,000 token mutations; all 360 product outputs
also match. This dependency experiment is not in the candidate above and still
requires integration, provenance and release verification before adoption.

## Previous candidate: deferred evidence

The [lazy follow-up record](../bench/release/0.21.0/lazy-followup.v1.json) binds
product `bbc91b01`, proof commit `1be93eda` and harness commit `ce94f372` to their
actual binaries and completed checks. The default Alamofire diagnostic now takes
5.23 seconds with about 2.03 GB peak physical footprint; near:0.8 takes 2.36 seconds.
All 360 outputs match the preceding candidate. These diagnostics do not qualify
the release's runtime comparison.

| Requirement | Current candidate result |
| --- | --- |
| Full local CI | Passed: 2,385 optimized tests, 89.51% line coverage, strict Clippy, MSRV, supply-chain checks and Lean |
| Remote CI | [Passed on the proof commit](https://github.com/corca-ai/nose/actions/runs/34040129650), including its single Asciidoctor focus; [harness commit also passed](https://github.com/corca-ai/nose/actions/runs/34042267663) |
| Output review | All 120 repositories in three modes and all 17 base workloads audited; exact intentional differences declared |
| Published 0.20 semantic runtime | Failed after the registered primary/control and single focused pair |
| Other query runtime workloads | Unqualified; stopped after semantic rejected this candidate |
| Cache correctness and latency | Passed: 2,100 mutation rows and 180 paired SymPy rows; all three cache modes within unchanged timing limits |
| Watch and recovery | Passed: 30 revisions each at 10k/100k files, fresh-query equality and forced restart; ready p95 75.94/398.49 ms |
| Saved analysis, sources and reviews | Passed: moves, copies, verified/stale sources, legacy metadata, no overwrite and 0.20 cache upgrade |
| Type-4 and soundness | Passed: 54 exact groups, no false merges/canon violations, PR/deep/120-repository nightly checks |
| Packages and installer | Four native packages and the actual generated installer independently verified; later harness-only package builds also passed |

The 120-repository semantic primary's total median sum decreases by 0.36%, but
individual regressions remain. The six-block focus confirms these wall-time
increases after the same-binary adjustment:

| Repository | Baseline median | Candidate median | Adjusted increase |
| --- | ---: | ---: | ---: |
| Delve | 328.78 ms | 626.50 ms | 294.16 ms / 89.5% |
| Raylib | 2,154.10 ms | 2,522.50 ms | 370.10 ms / 17.2% |
| libGDX | 1,461.47 ms | 1,644.90 ms | 183.27 ms / 12.5% |

The focus also confirms group construction, score, ranking or rendering signals
in Alamofire, Guava, Hugo, RxJava and RxSwift. Rack, Vim and zstd retain
inconclusive signals. Both confirmed and inconclusive outcomes reject the
candidate under the unchanged policy. Every semantic output hash matches the
reviewed declaration; output correctness does not waive runtime requirements.

All four semantic phase reports completed before the failed candidate's
coordinator was terminated. Its measured child ran uninterrupted. The remaining
base/default/near timings are explicitly unqualified, and no failed observation
was replaced. A changed candidate must complete the entire registered campaign.

The base-view harness now uses stable, exclusively reserved worktrees so emitted
navigation commands retain the same absolute working directory across phases.
Controls and focused runs must match the producer hashes and workspace root.
Invalid-command and random-path probes are retained separately from qualifying
observations. See the [runtime harness contract](semantic-regression-smoke.md).

## Earlier performance follow-up

Product `d8744855` reduces the isolated dense Alamofire observation from 111.89
to 8.91 seconds with identical output. All 120 semantic outputs, 16 additional
mode comparisons, 2,373 tests, cache/watch correctness, fresh soundness and four
native packages pass. History-cache timing improves, but clean-cache tail latency,
dense analysis and frontend runtime signals still prevent release.

The [follow-up record](../bench/release/0.21.0/performance-followup.v1.json) retains the new candidate's measurements.
See [runtime triage](runtime-triage.md#v0210-candidate-performance-follow-up-2026-09-06) for the remaining conditions. The table below
is the retained qualification of initial candidate `283f9e1d`; its passing
checks do not independently qualify a later product tree.

## Initial candidate qualification

| Requirement | Status | Evidence |
| --- | --- | --- |
| Versioned candidate and local `--full` | Passed: 2,371 optimized tests, 89.39% lines | Candidate `283f9e1d`; compact record below |
| Exact candidate remote CI | Failed: runtime, including the permitted focused rerun | [Audited CI run](https://github.com/corca-ai/nose/actions/runs/34024046297) |
| Published v0.20.0 comparison | Failed: dense-candidate and cache timing blockers | 120-repo semantic identities match; broad timing remains unqualified |
| Cache mutation correctness | Passed: 14 cases × 30 replays × 5 phases = 2,100 rows | Complete payload equality with verified navigation context |
| Paired real-repository cache | Correctness passed; timing failed | SymPy, 30 alternating replays per binary |
| Watch recovery | Passed: 30 revisions each at 10k/100k files | Fresh-query equality and forced crash/restart |
| Saved-analysis/source/review workflows | Passed | Verified/stale source, moves, copies, legacy metadata, no overwrite |
| Soundness Lab | Passed: 120 repositories, zero false merges/canon violations | [Nightly](https://github.com/corca-ai/nose/actions/runs/34023008275), [deep](https://github.com/corca-ai/nose/actions/runs/34023009587) |
| Four native packages | Passed; downloaded archives independently rechecked | [Package workflow](https://github.com/corca-ai/nose/actions/runs/34023009854) |
| Installer and upgrade compatibility | Passed | Actual CI installer, embedded checksums, isolated install, 0.20 cache reuse |

The checked [qualification record](../bench/release/0.21.0/qualification.v1.json)
contains source/binary identities, platform checksums, measured outcomes and raw
artifact seals. The tested product `crates` tree is
`8d0e11903a2b685f0bf7904385963ef731f07760`; initial qualification-document and
harness commits through `78f9feac` retain that product tree. The measured local distribution binary
SHA-256 is `b499631fb90867b37c9e8adbaa8eb31e53cea727d62859cf9778b449d1b98fcb`.

The local plan in `target/release-0.21.0/plan.md` fixes query comparisons before
measurement: all 120 pinned repositories in default, semantic and near/no-pack
modes, plus the frozen 17-repository base workload. Primary/control observations
use five samples and five paired blocks, with at most one six-block focused
rerun under the unchanged 5%/5 ms policy. Intentional schema or behavior changes
need a reviewed, exact drift declaration. Historical failures are retained.

The [0.20 release evidence](release-evidence-0.20.0.md) is historical context;
its successful checks do not qualify this candidate. Current source, binary,
corpus, package and result identities are recorded in the qualification record.

The cache and watch harnesses accept `--official-baseline` for a selected,
checksum-pinned release manifest. Their default remains the historical 0.19
baseline; old reports remain readable. New watch reports label the baseline
`official` with its version, rather than naming every release as 0.19.

Cache and watch equivalence verify each emitted navigation cache argument against its query
context, then remove only that argument before comparing the complete JSON payload.
This accounts for the new context-preserving navigation contract; detector fields,
source evidence and all other command arguments remain compared. Raw output hashes
are retained alongside the explicit `nose.query-cache-navigation/v1` comparison hash.

## Candidate blocker found during qualification

Candidate `283f9e1d` passes local full CI (2,371 optimized tests, 89.39% line
coverage, MSRV, dependency checks and Lean) and native package smoke on all four
targets. Its complete 120-repository semantic output audit preserves all 15,819 family
and 120,641 member identities, including multiplicity. The audited JSON v9-to-v10 changes are explicitly recorded
in the CI drift ledger; runtime checks remain enforced independently.

The pinned Alamofire near query takes 1.59 seconds on the checksum-verified
published 0.20 binary; the candidate exceeds a 30-second isolated timeout.
Profiling locates the cost in complete dense-candidate scoring. The old release
used a chain/star shortcut that omitted non-hub bucket pairs, so reinstating that
shortcut would discard required candidates. A bounded cross-batch score-cache
prototype passes focused equivalence tests but still takes 114.40 seconds on this
query. Its patch and measurements are retained locally and are excluded from the
release candidate. The default/near output audits were stopped to isolate this
blocker; the preregistered performance campaign cannot qualify this candidate.

Remote CI also observes material rendering signals on the larger v10 payload.
Its initial output-ledger failure is retained; the exact seven audited schema
changes are declared without weakening any timing threshold. A passing historical
or semantic-only check must not be represented as a passing release comparison.


## Measured cache and watch outcomes

SymPy's 30 paired replays preserve complete clean/empty/history payload equality
on each binary. That correctness result does not imply a performance pass:

| SymPy phase | Official 0.20 p50 | Candidate p50 | Delta | p95 delta |
| --- | ---: | ---: | ---: | ---: |
| Clean analysis | 2,425.20 ms | 2,560.55 ms | +5.58% | +6.40% |
| Empty cache | 2,611.95 ms | 2,731.90 ms | +4.59% | +2.25% |
| History cache | 317.10 ms | 345.15 ms | +8.85% | +5.64% |

Clean and history phases exceed both unchanged materiality thresholds, 5% and
5 ms. No extra replay was used to replace those results.

Watch passes its existing ready-latency limits, 250 ms at 10k files and 1,000 ms
at 100k. Ready p95 is 75.72/394.45 ms respectively; end-to-end p95 is
88.43/407.01 ms. All 60 snapshots match fresh queries and both forced restarts
recover correctly. The watch report binds its one-shot evidence to the exact
same candidate source and binary.

## Required before a new release decision

Reduce dense candidate work while preserving complete accepted-edge and connected
witness behavior; reduce v10 rendering and clean/history overhead. Then freeze a
replacement candidate and complete the preregistered 120-repository three-mode and
17-repository base timing campaign, cache comparisons and exact candidate CI.
The current default/near audits were stopped at the blocker, and base timing was
not run. No result here authorizes a GO decision or substitutes for those checks.


## First completion candidate

The [completion follow-up record](../bench/release/0.21.0/completion-followup.v1.json)
retains candidate `843bd52e` / proof commit `0de3169b` separately from the earlier
failed qualifications. It passes full local CI (2,379 tests, 89.44% line coverage),
Type-4 checks, 120-repository soundness, deep checks, four native packages and the
actual generated installer. Cache correctness passes 2,100 mutation and 180 SymPy
rows; all three SymPy p95 comparisons fall below the unchanged materiality limits.
The remote runtime gate remains inconclusive for Asciidoctor normalization after
its single focused run. The full timing campaign remains unqualified.

The output audit matches 359 of 360 comparisons. Both old and new candidates were
interrupted during the Alamofire default query because accepted-edge allocation
caused sustained paging; empty outputs are not equality evidence. This motivates
the subsequent compressed-relation implementation described in
[runtime triage](runtime-triage.md). Results from that implementation must bind a
new candidate; they cannot retroactively qualify `843bd52e`.


## Compressed relation candidate

The [dense follow-up record](../bench/release/0.21.0/dense-followup.v1.json) binds
product `73b45837` / proof commit `a155a379` to full local CI (2,382 tests, 89.49%
line coverage), all 360 output checks, Type-4 validation, saved-analysis journeys,
120-repository soundness, independent deep checks, four native packages and the
actual installer. The remote runtime gate remains inconclusive for Asciidoctor
normalization after its single focus. Full timing and current-candidate cache/watch
qualification were still pending for that product. Later lazy-projection and
interner improvements belong to the separately evaluated `bbc91b01` candidate
described above; its completed checks and rejection do not rewrite this history.
