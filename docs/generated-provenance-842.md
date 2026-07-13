# #842 generated documentation provenance

Issue #842 implements only the generated-documentation gap selected by the #841 dev
taxonomy. It does not add a repository, directory, symbol, or language allowlist, and it
does not classify partially generated families. Held-out source remains closed.

## Product rule

A candidate HTML file has Jazzy provenance when its first 64 KiB, compared
ASCII-case-insensitively, contains both:

1. a Jazzy asset reference (`jazzy.css` or `jazzy.js`); and
2. a generated Apple/Dash symbol anchor (`class="dashanchor"` or `//apple_ref/`).

A family moves to `surface: "generated"` only when every member file has established
generated provenance or this new evidence. The human report omits it from the default and
counts it as `generated-code`. `all top=0 --format json` retains the family. A missing
marker, a non-HTML file, an unreadable file, evidence beyond the byte bound, or a mixture
of generated and unproven members fails open to the family's existing ranked surface.

The existing first-eight-lines generated header, compiled CSS, and CSS
source-plus-compiled pipeline rules are unchanged. The new evidence is stored separately
from `Loc::looks_generated`: that field also affects helper recommendations and other
semantic report fields. Overlap folding likewise uses the pre-transition opportunity set.
This separation makes the change a surface classification only and preserves the family
universe.

## Frozen evidence and hard negatives

The implementation replays the exact `generated-provenance.v1` predicate frozen by the
[#841 taxonomy](default-head-failure-taxonomy-841.md), including its hard boundaries.

| cohort | expected | result |
|---|---:|---:|
| bare-default head positives | 10 | 10 `generated` |
| independently audited rank 11–30 positives | 20 | 20 `generated` |
| worthy HTML hard negatives | 3 | 3 remain `default` |

The positives are all source-coherent Alamofire Jazzy documentation families. The hard
negatives are one Jekyll and two SQLite HTML families: an `.html` suffix alone is not
generator provenance. Focused tests also cover markers after the old eight-line bound,
case normalization, either marker variant, a non-HTML lookalike, either missing signal,
the 64 KiB boundary, partial-family generation, human omission, JSON reason coding, and
full-universe recovery.

## Exact output drift

The 66 pinned dev repositories were compared against immediate parent `bf6298ad`.

For the expanded default command (`nose query bench/repos/<repo> all top=0 --format
json`), 65 repositories are byte-identical. Alamofire retains the same 1,115 ordered
family IDs and every non-`surface` field. Exactly 507 surfaces change:

| transition | families |
|---|---:|
| `default -> generated` | 387 |
| `hidden -> generated` | 89 |
| `shallow -> generated` | 31 |

The first 30 former default rows are all source-coherent Jazzy output, so the measured
default top-30 replaces all 30. This is broader than the ten head positions used to select
the rule, but not an extrapolated predicate: ranks 11–30 were the independently audited
deep-positive cohort. The other 65 repositories do not change.

The established semantic command used by the runtime harness also preserves all 4,063
Alamofire family IDs, order, and non-surface fields. It moves 3,326 `default` and 426
`hidden` families to `generated`; the other 65 repositories are byte-identical. Across
that 66-repository surface the family total remains 9,850.

The [#842 closeout artifact](../bench/labels/generated_provenance_closeout_2026_07_13.dev.v1.json)
binds the compact machine-readable evidence.

## Official-release performance

Performance uses the published Darwin arm64 v0.19.0 binary (SHA-256
`0f73ea…e0f3`), not a source rebuild. Three alternating iterations after one warmup over
all 66 dev repositories measured:

| run | baseline | current | delta |
|---|---:|---:|---:|
| official v0.19.0 comparison | 15,841.49 ms | 15,769.21 ms | -72.28 ms / -0.46% |
| current/current control | 16,031.52 ms | 16,100.59 ms | +69.07 ms / +0.43% |
| approximate control-adjusted | — | — | -141.35 ms / -0.89% |

The material-regression gate requires a control-adjusted increase greater than both 5%
and 5 ms, so the all-dev result passes. The directly changed `query_surface` stage is
control-adjusted +2.5 ms, below the absolute threshold.

The first noisy control produced 13 repository-level apparent outliers. A nine-iteration
primary/control recheck of all 13 measured -35.85 ms / -1.15% adjusted, with +0.6 ms
adjusted in `query_surface`. Nginx alone remained above the formal adjusted repository
threshold even though its raw increase was below 5 ms; a final 21-iteration
primary/control recheck measured -0.78 ms / -0.38% adjusted. Its output hash was identical
throughout. No material aggregate, repository, or changed-stage regression remains.

The six raw harness artifacts linked from the compact closeout contain binary identities,
corpus commits, commands, alternating runs, stage medians, output hashes, family counts,
and surface counts.

## Validation

```sh
cargo test -p nose-cli jazzy -- --nocapture
python3 scripts/query-regression-harness.py --self-test
./scripts/check-ci-local.sh --fast
./scripts/check-docs.sh
```

The final epic quality and held-out gates remain owned by #845 and #846. This tranche
closes only the measured generated/build-artifact gap and leaves judgment-deep families
visible.
