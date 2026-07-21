# Order-aware performance controls

This document preregisters the prospective runtime decision contract for #927. It was
committed before replaying the frozen #892 r40 primary and same-binary control through
the new estimator. The existing 5% and 5 ms materiality thresholds do not change.

## Applicability and compatibility

The contract applies automatically to `nose.query_regression_harness.v3` reports.
Historical v1 and v2 reports retain their original decisions unless an explicit
historical-evaluation command selects this policy. Such a replay is a decision ledger,
not a rewrite of the #892 or #907 conclusions.

Product-output drift is evaluated before runtime. The code-identical-binary shortcut is
also unchanged: two matching code identities cannot establish a code regression, even
when full executable digests differ because of build metadata.

## Measurement design

Each repository is measured in alternating paired blocks. A block contains exactly one
baseline and one current observation. Odd blocks run baseline first and even blocks run
current first. Version 3 records the block number, position, and order explicitly rather
than requiring a consumer to infer them from array position.

The harness completes a repository's warmup and alternating blocks before moving to the
next repository. Keeping those blocks adjacent prevents the first process in every block
from repeatedly paying a cold-file penalty after the rest of a large corpus has displaced
that repository's pages. This scheduling detail does not change the paired observations,
the alternating orders, or any decision threshold.

An observation may use a declared odd number of at least five consecutive command samples
when the single-process timing variance is too large for the 5 ms floor. A single sample
remains valid for ordinary runs; the unsupported three-sample middle ground would leave
one process position with only one value. Within a block, odd samples follow the declared
pair order and even samples mirror it. For each label, the harness takes the median
elapsed and per-stage time at each actual process position, then averages the two position
medians. This exposes both labels to both positions and makes the observation
position-neutral while retaining the declared order as the odd-sample majority. The
report records the sample count and every raw elapsed/stage timing with its actual process
position; the checker reconstructs each aggregate from those values.
Every sample must produce identical product-output observations; any mismatch fails the
harness. Primary, same-binary control, and focused reports must use the same
samples-per-observation setting.

The final v0.20 qualification uses five samples per observation for every ordinary
query and base-view primary, control, and focused report. This choice is fixed before
measuring the replacement candidate: a preceding single-sample candidate exposed
process-position conflicts and failed its only focused rerun, despite recording no
triggered regression. Its failed artifacts remain part of the release audit and are
not reused in the replacement decision.

The v0.20 release qualification also applies this design to the frozen 17-repository
`base=<ref>` workload in `bench/base_view_release_workload.v1.json`. The manifest binds
each repository to an exact checked-out commit, ancestor base, source sample, and source
digest. The harness validates those Git objects and the source-row selection before
creating an isolated detached worktree. `--repo` selects an exact focused subset from
the same manifest; it cannot introduce a new workload after the primary measurement.

Base-view output has intentional additive evidence relative to v0.19.0. With
`--output-normalizer base-v0.19`, every raw output remains hash-accounted for the exact
drift declaration, while a second hash removes only `targets` and per-site
`semantic_change` fields. The checker reconstructs these normalized hash sets from all
raw observations and fails if baseline and candidate differ after that projection.
Timing never waives either the raw drift declaration or normalized equality.

For a single-sample observation, a material median in only one declared pair order is
still an inconclusive order conflict. A multi-sample position-neutral observation has
already exposed each label to both actual process positions and averaged their position
medians. Splitting those collapsed observations by the declared odd-sample majority would
apply the same order safeguard twice and can turn sub-threshold noise into a false
conflict. For these observations, the checker retains the declared-order strata as
diagnostics but decides from the position-neutral effect and block support. A material
effect without sufficient block support remains inconclusive.

If one physical focused run contains a superset of the repositories requested by the
final policy, the checker records that measured superset and derives the decision from an
exact projection onto the requested repositories. It rebuilds the projected raw-run
summary and corpus selection identity before validation. This permits reuse of the one
allowed focused run after a fail-closed policy correction without diluting an aggregate
signal with unrelated repositories or silently dropping a requested repository.

A metric is eligible only when all of these conditions hold:

- every included block has exactly one observation for each label;
- at least five complete blocks exist, with at least two blocks in each order stratum;
- the order-stratum counts differ by at most one;
- aggregate rows have a complete repository set in every included block;
- all elapsed and stage values are finite and non-negative.

Missing, duplicate, or malformed blocks are insufficient evidence. They are never
silently dropped. A primary run with insufficient evidence requests one focused rerun;
insufficient focused evidence fails closed without starting another rerun loop.

## Estimator

For block `i`, let `d_i = current_i - baseline_i`. Compute the median `d_i` separately
for baseline-first and current-first blocks, then average the two stratum medians. This
is the order-neutral primary effect. Aggregate effects are computed from the per-block
sum across repositories, not from independently aggregated side medians.

The same estimator is applied independently to the same-binary control. Its adjustment
is deliberately one-sided:

```text
control_correction = max(order_neutral_control_effect, 0)
adjusted_effect = order_neutral_primary_effect - control_correction
```

A positive control movement estimates shared slowdown and may reduce an apparent
product regression. A negative control remains visible as a diagnostic but contributes
zero correction: an independently noisy speedup cannot manufacture evidence that the
product became slower. Primary and control runs are not treated as paired with each
other because they were not interleaved in one randomized block sequence.

## Decision rule

For each complete primary block, subtract the non-negative control correction from
`d_i`. A block supports material regression only when the adjusted difference is
strictly greater than both 5 ms and 5% of that block's baseline value. Let `k` be the
number of supporting blocks among `n`. Evidence support is the exact one-sided sign
test under `P(support) = 0.5`:

A metric with a zero baseline has no finite relative increase and therefore cannot
satisfy both thresholds by itself. This matters for timing labels introduced after the
baseline release: their time remains included in repository and aggregate elapsed
signals, while the new label's zero-denominator diagnostic is not misclassified as an
independent stage regression.

```text
p = sum(combination(n, j) for j in k..n) / 2^n
```

Support requires `p <= 0.05`. Both order strata must also have adjusted median movement
strictly above both thresholds. A runtime signal triggers only when:

1. the order-neutral adjusted point estimate exceeds 5 ms and 5%;
2. the exact sign test supports it; and
3. both execution orders independently agree on the direction and materiality.

For a single-sample report, a material median in either order with a non-material
median in the other is also `inconclusive`, even when the combined point estimate is
below threshold; this is the order-conflict rule stated above. Otherwise, if the point
estimate is material but support, order consistency, or required blocks are missing,
the signal is `inconclusive`. A primary inconclusive signal requests the single focused
rerun. A focused inconclusive signal fails as insufficient evidence; it does not pass
and does not request another measurement.

## Preregistered synthetic expectations

- A stable true regression above both thresholds triggers in both orders.
- Positive same-binary drift reduces the measured product effect but cannot increase it.
- Negative same-binary drift is reported and contributes zero correction.
- A pure first/second-position bias disagrees between order strata and is inconclusive.
- High-variance mixed-sign measurements that produce a material point estimate without
  sign-test support are inconclusive.
- Missing or duplicate rounds are insufficient evidence.
- A code-identical comparison passes through the existing shortcut.
- Any product-output drift still follows the existing exact declaration gate regardless
  of runtime evidence.

## Frozen historical evaluation input

The later evaluation is bound to the already-published #892 source artifact and these
original r40 files:

- source record:
  `bench/recall_loss/issue-892-official-v0.19.0-performance-2026-07-18.v1.json`;
- primary SHA-256:
  `a539b07ede84b3531b28623a4b84278fe5225ad1c4327c5331a9d5e191fc948a`;
- same-binary control SHA-256:
  `a3e615b896b9520604ddcd19ed60e2a0e6390746179513c1ab5541e9f987fb63`.

The implementation may be corrected if it fails this written contract, but the
contract will not be tuned after observing which historical rows change decision.

## Frozen replay outcome

The policy was implemented after the preregistration commit and then replayed once on
the two SHA-bound #892 r40 reports. The compact checked decision
[ledger](../bench/recall_loss/issue-927-order-aware-control-decision-ledger-2026-07-21.v1.json) records
all 226 runtime signals: 215 retain their legacy state and exactly 11 change.
The four legacy triggers become three `within-threshold` rows and one `inconclusive`
row. Seven formerly clear rows expose a material split between execution orders and
therefore also become `inconclusive`; this is intentional fail-closed behavior, not a
new regression claim.

| Signal | Legacy | Order-aware | Why |
| --- | --- | --- | --- |
| `etcd:lower` | triggered | within threshold | negative control no longer inflates; both order medians are below threshold |
| `etcd:parse+lower` | triggered | within threshold | negative control no longer inflates; both order medians are below threshold |
| `guava:normalize+extract` | triggered | within threshold | order-neutral adjusted effect is 3.88 ms / 1.08% |
| `minio:normalize+extract` | triggered | inconclusive | baseline-first is material; current-first moves in the opposite direction |
| `alamofire:lower` | within threshold | inconclusive | only current-first is material |
| `bat:parse+lower` | within threshold | inconclusive | only current-first is material |
| `minio:lower` | within threshold | inconclusive | the two order medians have opposite signs |
| `netty:normalize+extract` | within threshold | inconclusive | only current-first is material |
| `regex:lower` | within threshold | inconclusive | only baseline-first is material |
| `rxswift` | within threshold | inconclusive | repository medians split by execution order |
| `rxswift:lower` | within threshold | inconclusive | the two order medians have opposite signs |

The replay does not rewrite #892 or #907. Version 2 reports still use the legacy
estimator by default; `--runtime-policy order-aware-v1` is an explicit historical
evaluation switch. New version 3 reports record pair order and position directly and
select the new policy automatically. The merge smoke now takes the minimum five
eligible primary blocks; a material or inconclusive primary signal gets exactly one
six-block focused rerun, after which a trigger or remaining inconclusive result fails.
