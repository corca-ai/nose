# 0.21.0 candidate qualification

Prepared on 2026-09-06. Release decision: **pending**. The feature scope is frozen;
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

## Qualification requirements

| Requirement | Status | Evidence |
| --- | --- | --- |
| Final versioned candidate and local `--full` | Pending | `target/release-0.21.0/` |
| Exact candidate remote CI | Pending | Candidate draft PR |
| Published v0.20.0 comparison | Baseline archive checksum verified; comparison pending | `target/release-0.21.0/official-provenance.json` |
| Cache mutation and watch recovery | Pending | 30-replay existing harness contracts |
| Saved-analysis/source/review workflows | Pending | Exact candidate CLI journeys |
| Four native package smoke checks | Pending | Candidate PR artifact uploads |
| Installer and upgrade compatibility | Pending | Isolated installation and cache reuse |

The local plan in `target/release-0.21.0/plan.md` fixes query comparisons before
measurement: all 120 pinned repositories in default, semantic and near/no-pack
modes, plus the frozen 17-repository base workload. Primary/control observations
use five samples and five paired blocks, with at most one six-block focused
rerun under the unchanged 5%/5 ms policy. Intentional schema or behavior changes
need a reviewed, exact drift declaration. Historical failures are retained.

The [0.20 release evidence](release-evidence-0.20.0.md) is historical context;
its successful checks do not qualify this candidate. Current source, binary,
corpus, package and result identities will be recorded here before a GO decision.

The cache and watch harnesses accept `--official-baseline` for a selected,
checksum-pinned release manifest. Their default remains the historical 0.19
baseline; old reports remain readable. New watch reports label the baseline
`official` with its version, rather than naming every release as 0.19.
