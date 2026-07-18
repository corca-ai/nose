# Default-query performance closeout for #892

Generated on 2026-07-18. The
[durable #892 performance artifact](../bench/recall_loss/issue-892-official-v0.19.0-performance-2026-07-18.v1.json) binds
the measurements and binary identities below.
The reusable measurement contract is documented in [runtime triage](runtime-triage.md).

## Outcome

#892's persistent default-query opportunity and gating regressions are removed without
changing product output. The all-120 aggregate is `0.88%` faster before control
adjustment and `0.69%` faster after it. The required semantic regression smoke passes
with zero output drift and a `2.28%` adjusted aggregate improvement.

The final preregistered r40 check retained three independent pre-query stage owners:
etcd Go frontend work and Guava/MinIO normalization. They are split to #907 and #908,
as required by #892's result-dependent rule. They do not block #847 because its frozen
runtime criterion is the control-adjusted aggregate, which is safely improved here.

## Diagnosis and implementation

Sampling Alamofire before editing showed `family_key` dominating the new query path.
The #842/#843 surface split repeatedly sorted and hashed every family location while:

- deciding whether a family was default-actionable;
- rebuilding the set of default family identities after opportunity grouping;
- checking effective surfaces during gating and rendering.

The fix uses a process-local `(locations pointer, length)` family handle for surface
override membership. These handles remain stable while the location vectors live, even
when the outer family vector is sorted. `OpportunityGroups` now also decides default
slices when it creates direct fold edges, eliminating the second full-family identity
pass. Stable serialized family IDs remain the authority at process boundaries.

Post-change sampling reduced `family_key` top-stack samples from 78 to 29; the remaining
calls are the direct fold-edge IDs that product output requires.

## Product measurements

The baseline is the published v0.19.0 arm64 macOS binary at source
`0985e6963c58d5a97e523bc532b88aa5e34f2ef9`. Its file SHA-256 is
`0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3`.
The candidate source is `8c37b8f9c75b9543dc746e3eb6b9b18209810059`; its binary SHA-256 is
`111c6507dd028a194c000fbc279e05e6b9b2899552a9dca7f56a15d23c5ad38c`.

| Scope | Baseline | Candidate | Raw delta | Control-adjusted delta |
| --- | ---: | ---: | ---: | ---: |
| all 120, r3 | 35,752.50 ms | 35,438.88 ms | -313.62 ms / -0.88% | -247.12 ms / -0.69% |
| final 9, r40 | 6,806.02 ms | 6,760.25 ms | -45.77 ms / -0.67% | -16.15 ms / -0.23% |

The original seven-repository r9 replay also showed the intended path improvement:

| Repository | `query_opp` v0.19.0 → candidate | `query_gate` v0.19.0 → candidate |
| --- | ---: | ---: |
| Alamofire | 47.4 → 34.5 ms | 24.1 → 14.8 ms |
| Guava | 5.8 → 2.8 ms | 7.0 → 4.0 ms |
| Netty | 6.8 → 3.6 ms | 7.4 → 5.1 ms |
| RxJava | 10.6 → 6.7 ms | 8.6 → 5.0 ms |
| SQLAlchemy | 3.4 → 1.5 ms | 5.2 → 3.4 ms |
| SymPy | 3.5 → 1.6 ms | 5.1 → 3.2 ms |

The established escalation was applied once at each depth: all-120 r3, 33-repository
r9, 18-repository r21, and 9-repository r40. No threshold was changed.

## Output and semantic safety

A separately built pre-fix binary from `963af51f38e1a9004f8aed3a0bd0d3ab1648e353`
was compared directly with the fixed binary across all 120 repositories. Every family
count, order, surface count, metadata byte, output byte count, and SHA-256 matched.
This isolates #892's zero output drift from intentional v0.19.0-to-main changes.

`scripts/semantic-regression-smoke.sh --force` compared the same two binaries on the
seven-language pinned smoke corpus. It passed after the standard focused rerun:

- aggregate: 580.38 → 575.69 ms;
- control-adjusted: -13.23 ms / -2.28%;
- output drift: zero declared, zero unexpected;
- Ruby scaling: within threshold, exponent 0.69 against the 1.35 limit.

The implementation also passed:

- `cargo test --workspace --all-targets`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `scripts/check-docs.sh`;
- the focused query-family, generated-surface, and declaration-surface tests.

## Independent residuals

The unchanged 5% / 5 ms checker retained four r40 stage rows:

| Owner | Stage | Raw delta | Control-adjusted delta | Follow-up |
| --- | --- | ---: | ---: | --- |
| etcd | lower | +2.50 ms | +5.55 ms / +6.52% | #907 |
| etcd | parse+lower | +2.55 ms | +5.90 ms / +10.55% | #907 |
| Guava | normalize+extract | +19.30 ms | +26.50 ms / +7.39% | #908 |
| MinIO | normalize+extract | +3.20 ms | +7.05 ms / +6.39% | #908 |

These execute before the query classification changed by #892. The Go frontend was
untouched by this fix, while normalization grew materially after v0.19.0 in #900 and
#859. The split therefore follows the issue's explicit independent-cause rule rather
than broadening one query optimization into unrelated frontend and semantic work.
