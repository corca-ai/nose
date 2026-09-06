# 0.21.0 candidate qualification

Prepared on 2026-09-06. Release decision: **NO-GO (performance blocker reproduced)**. The feature scope is frozen;
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

## Performance follow-up

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
