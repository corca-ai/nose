# Divergent History Mining Pilot 687

Issue #687 operationalizes the divergent-edit gate evidence from #681 without
promoting the gate to default-on. It records one bounded offline history-mining
run and one local observe-only pilot summary.

## Checked Artifacts

- History artifact: [issue-687-cli-tests-2026-07-06.v1.json](../bench/divergent_history/issue-687-cli-tests-2026-07-06.v1.json)
  is the checked `nose.divergent_history.v1` history-mining artifact.
- Pilot summary: [issue-687-maintainer-pilot-2026-07-06.v1.json](../bench/divergent_history/issue-687-maintainer-pilot-2026-07-06.v1.json)
  records aggregate counts, maintainer disposition, the raw-query hash, and the history artifact hash.

The raw observe-only query JSON is intentionally untracked under
`target/divergent-history/issue-687/`. The checked pilot summary records its
sha256 but does not check in full query rows.

## History Run

The bounded run used a rebuilt release binary and replayed the divergent-edit
query across the recent nose-on-nose CLI-test slice:

```sh
cargo build --release -p nose-cli
python3 scripts/divergent-history-mining.py --self-test

python3 scripts/divergent-history-mining.py \
  --repo . \
  --range "df2aab26~1..HEAD" \
  --path crates/nose-cli/tests/cli \
  --mode syntax,semantic \
  --min-size 8 \
  --max-commits 12 \
  --first-parent \
  --merge-policy skip \
  --nose target/release/nose \
  --output bench/divergent_history/issue-687-cli-tests-2026-07-06.v1.json
```

Result:

| count | value |
|---|---:|
| commits considered | 11 |
| commits analyzed | 11 |
| skipped commits | 0 |
| findings | 17 |
| groups | 17 |
| strict/default-failing findings | 0 |
| report-only findings | 17 |
| taxonomy | 17 `test_scaffolding` |

All groups were `base-divergence` lane, `report-only` tier. The artifact
records the exact command, script hash, nose binary hash, nose version, dirty
state, range, parameters, grouped findings, strict counts, skipped commits, and
suppression behavior.

## Maintainer Review

Maintainers review `groups[]` once, not every repeated per-commit occurrence.
Use each group's `representative` row for the review surface and
`occurrences[]` only to understand when the pattern appeared.

Record disposition with these buckets:

| bucket | #687 count |
|---|---:|
| should-propagate | 0 |
| intentional-variant | 0 |
| no-propagation-needed | 0 |
| test-scaffolding/report-only | 17 |
| grouping-artifact/not-a-clone | 0 |
| unclear | 0 |
| structured ignores added | 0 |

Accepted intentional findings should use structured ignores with reason, owner,
and expiry. Do not teach CI wrappers to reinterpret a finding that the runtime
already classifies.

## Observe-Only Pilot

The representative observe-only run used the normal PR-time query shape but no
failure step:

```sh
target/release/nose query crates/nose-cli/tests/cli base=HEAD~1 \
  --mode syntax,semantic --format json top=0 \
  > target/divergent-history/issue-687/observe-only-head.query-v8.json
```

Result:

| count | value |
|---|---:|
| changed files | 1 |
| findings | 0 |
| strict | 0 |
| review | 0 |
| report-only | 0 |
| default-failing (`items[].gate.fail_default`) | 0 |

This is opt-in tolerability evidence only. It is not a default-on readiness
claim, and it does not add history mining to PR-time CI.

## Validation

The checked artifacts are validated by the docs gate:

```sh
python3 scripts/divergent-history-mining.py --check-artifact \
  bench/divergent_history/issue-687-cli-tests-2026-07-06.v1.json
python3 scripts/check-divergent-history-artifacts.py
```

The validator recomputes summary counts and group keys, checks bounded/offline
metadata, rejects source-bearing keys such as snippets or patches, and ensures
the pilot summary still points at the checked history artifact hash.
