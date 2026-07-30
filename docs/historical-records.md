# Historical records index

These pages preserve measurements, audits, rejected alternatives, release
qualification, and issue closeouts. They are evidence for why nose reached its
current design; they are not the current user or contributor contract.

For current behavior, start at the [documentation home](home.md). For current
architecture, proof contracts, and active plans, use the
[development and evidence index](development-and-evidence.md). The checked
[documentation lifecycle](documentation-lifecycle.md) owns the complete
classification and retention rules.

## Append-only ledgers

- [Experiment log](experiments.md) — stable lettered anchors for adopted,
  rejected, and measured ideas. Current commands and benchmark claims live in
  [usage](usage.md) and [benchmark](benchmark.md).
- [Dogfooding history](dogfooding-history.md) — detailed baseline decisions and
  candidate-by-candidate judgments. The current ratchet workflow lives in
  [dogfooding](dogfooding.md), which owns active instructions.
- [Semantic-kernel roadmap](semantic-kernel-roadmap.md) — closed #473 decisions,
  tranches, phases, and then-open questions.
- [Semantic-kernel snapshot](semantic-kernel-snapshot.md) — the dated 2026-07-02
  implementation baseline and append-only notes through 2026-07-19.

The ledgers keep their established filenames and anchors because code comments,
design pages, and reproduction notes cite them. Their current-vs-history
boundaries are explicit instead of moving thousands of lines and invalidating
those citations.

## Release and performance records

- [0.20.0 release evidence](release-evidence-0.20.0.md) — final 0.20 qualification.
- [0.19.0 release evidence](release-evidence-0.19.0.md) — final 0.19 qualification.
- [0.18.0 release evidence](release-evidence-0.18.0.md) — final 0.18 qualification.
- [0.17.0 release evidence](release-evidence-0.17.0.md) — final 0.17 qualification.
- [0.17.0 runtime triage](runtime-triage-0.17.0.md) — post-release follow-up.
- [20-optimization runtime pass](runtime-performance-20-optimizations-2026-07-02.md) — measured optimization sequence.
- [default-query performance closeout](runtime-performance-issue-892-2026-07-18.md) — frozen default-query result.
- [etcd Go frontend attribution](runtime-performance-issue-907-2026-07-19.md) — build-provenance diagnosis.
- [normalize-and-extract closeout](runtime-performance-issue-908-2026-07-19.md) — MinHash optimization result.
- [Ruby redefinition runtime triage](runtime-triage-ruby-redefinitions-2026-07-10.md) — focused frontend diagnosis.
- [incremental-cache benchmark](incremental-cache-benchmark.md) — frozen cache workload contract.

Current performance triage instructions remain in
[runtime triage](runtime-triage.md); current cache behavior remains in
[query cache](query-cache.md) and [portable cache artifacts](portable-cache-artifacts.md).

## Semantic-kernel and pack closeouts

- [Semantic-kernel foundation audit](semantic-kernel-audit-2026-06-09.md) — initial pocket inventory.
- [Semantic-kernel foundation tranche closeout](semantic-kernel-tranche-closeout-2026-06-09.md) — foundation completion record.
- [Builtin expansion](semantic-kernel-builtin-expansion-509.md) — admitted primitive result.
- [Capability minimization census](semantic-kernel-capability-minimization.md) — blocker-derived primitive matrix.
- [Expansion cycles R1–R3](semantic-kernel-expansion-511.md) — early capability cycles.
- [External authorability R4](semantic-kernel-external-authorability-511.md) — external-pack dry run.
- [HOF and demand boundary R5](semantic-kernel-hof-demand-511.md) — demand boundary matrix.
- [Expansion closeout R6](semantic-kernel-expansion-closeout-511.md) — completed capability set.
- [Sequence-HOF closeout](semantic-kernel-closeout-533.md) — sequence tranche result.
- [Pack default-promotion audit](semantic-pack-default-promotion-audit-678.md) — promotion decision evidence.
- [Pack boundary review](semantic-pack-boundary-review-2026-06-22.md) — pre-release boundary audit.
- [0.20 semantic-pack release gate](semantic-pack-0.20-release-gate.md) — release closeout.
- [Scheduling/channel/callback closeout](scheduling-channel-callback-obligations-594.md) — closed obligation census.

Current semantic-kernel and pack contracts are indexed under [development and
evidence](development-and-evidence.md#semantic-kernel--packs) for active use.

## Product, Type-4, and field closeouts

- [Pre-epic readiness record](pre-epic-readiness-948.md) — bounded maintenance audit.
- [Test ownership: divergent-edit policy](test-ownership-963.md) — boundary and timing closeout.
- [Default-head baseline](default-head-baseline-839.md) — published-release baseline.
- [Default-head label runway](default-head-label-runway-840.md) — sealed review runway.
- [Default-head failure taxonomy](default-head-failure-taxonomy-841.md) — complete dev taxonomy.
- [Generated provenance](generated-provenance-842.md) — classifier qualification.
- [Checked-in generated artifact provenance](generated-artifact-provenance-891.md) — producer-independent follow-up.
- [Declaration-only type contracts](declaration-only-type-contracts-843.md) — closed type-contract slice.
- [Proof/actionability no-go](proof-actionability-no-go-844.md) — rejected threshold proposal.
- [Residual ranking calibration](residual-ranking-calibration-845.md) — frozen ranking no-go.
- [Default-head blind closeout](default-head-blind-closeout-846.md) — held-out decision.
- [Missed-worthy frontier](missed-worthy-frontier-816.md) — initial frontier audit.
- [Accepted-pair endpoint coverage](accepted-pair-coverage-817.md) — grouping fix evidence.
- [Post-coverage frontier](missed-worthy-frontier-820.md) — refreshed frontier.
- [Connected mapped witnesses](connected-witness-821.md) — bounded witness closeout.
- [Bounded same-unit windows](bounded-same-unit-windows-832.md) — window-route closeout.
- [Divergent-history mining pilot](divergent-history-mining-pilot-687.md) — observe-only pilot.
- [Divergent-gate product/runtime evidence](divergent-gate-product-runtime-688.md) — output and cost record.
- [Divergent-gate closeout](divergent-gate-closeout-854.md) — final 0.20 gate decision.
- [Default-surface noise audit](default-surface-noise-audit-2026-06-14.md) — field-feedback recheck.
- [Fragment quality audit](fragment-quality-audit-2026-06-10.md) — labeled fragment sample.
- [Lawpack provenance audit](lawpack-provenance-audit-2026-06-10.md) — provenance census.
- [Query JSON agent audit, first pass](query-json-agent-audit-2026-06-10.md) — initial machine-contract audit.
- [Query JSON agent audit, revalidation](query-json-agent-audit-2026-06-13.md) — post-fix recheck.
- [Reinvented-helper audit](reinvented-helper-audit-2026-06-13.md) — promotion evidence.
- [Markdown detector survey](markdown-dup-detection-algorithm-survey-2026-06-18.md) — algorithm selection record.
- [Field evaluation](field-evaluation.md) — third-party qualitative snapshot.

## Protocol and value-model closeouts

- [String-affix protocol](string-affix-protocol-closeout-548.md) — protocol extraction closeout.
- [Go string-affix migration](go-string-affix-closeout-549.md) — namespace-proof migration.
- [JavaScript/TypeScript string-affix hardening](js-ts-string-affix-hardening-closeout-550.md) — receiver-proof hardening.
- [Ruby string-affix slice](ruby-string-affix-closeout-551.md) — Ruby protocol slice.
- [String-affix coordinate boundary](string-affix-coordinate-closeout-552.md) — coordinate hardening.
- [String-affix conformance](string-affix-conformance-closeout-558.md) — inventory closeout.
- [Import-backed immutable provenance](import-backed-immutable-provenance-closeout-567.md) — provenance capability result.
- [Float-kind design closeout](value-float-kind-design.md) — IEEE-754 correction evidence.
- [Pre-#821 stabilization](stabilization-829.md) — bounded stabilization record.

The lifecycle catalog is the exhaustive inventory. This page intentionally
groups records by the current question they help answer rather than mirroring
every wiki filename on the documentation home.
