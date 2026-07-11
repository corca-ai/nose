# Post-#817 missed-worthy frontier (#820)

Issue [#820](https://github.com/corca-ai/nose/issues/820) refreshes the
source-independent worthy-recall frontier after accepted-pair endpoint coverage shipped in
#817. The result is a bounded candidate-acceptance follow-up, not permission to lower a
threshold or admit the optimistic multiset ceiling.

## Protocol and chronology

The audit kept the frozen v5 multi-source pool as the only worthy-recall denominator. The
v6 overlay remained precision-only. Evidence was frozen in this order:

1. Make frontier validation snapshot-aware, while retaining strong validation of the #816
   artifact and rejecting unregistered evaluation/count substitutions.
2. From a clean commit, freeze the combined post-#817 product evaluation and frontier.
3. Commit the deterministic 35-row dev selection, then regenerate all 158 dev raw stages.
4. Bind 35 dev source decisions to the selected candidate hashes and frozen source bytes.
5. Commit the dev proposal before running one mechanical held-out stage confirmation.
6. Open the result-dependent implementation issue only after those gates passed.

Sixteen selected candidate hashes exactly overlap the frozen #816 audit and reuse those
decisions byte-for-byte. Three sources — `curl:2a436119a08187ba`,
`mockito:d82f6e75097748da`, and `ripgrep:519afdcaed73af0d` — had been read during #820
planning before the new selection was committed. They are explicitly marked exploratory,
not presented as freshly blinded judgments. The other sixteen new rows were reviewed only
after the selection and stage artifacts were committed. No held-out source was rendered or
judged.

## Checked post-#817 product baseline

The combined [product-quality report](../bench/labels/product_quality_evaluation_post_817_2026_07_12.v1.json)
uses the post-#817 binary (`sha256 5999e791c276a7d098099911b8252d35609d1ec7dbcf38888dad2815b126dd0a`)
and the pre-#817 binary as `--comparison-nose`. Its evaluation digest is registered with an
exact recall and comparison contract; CI rejects a substituted digest, count, or regression
summary.

| split | worthy recall | remaining miss | labeled P@10 | delta from pre-#817 |
| --- | ---: | ---: | ---: | ---: |
| dev | 2,691 / 2,849 (94.45%) | 158 | 264 / 447 (59.06%) | +65 |
| held-out | 1,996 / 2,091 (95.46%) | 95 | 214 / 384 (55.73%) | +47 |

The exact comparison contains 112 recovered worthy families and zero regressions.

## Refreshed frontier and raw stages

The checked [frontier artifact](../bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json)
binds the evaluation and binary hashes, all inputs, all 120 repository pins, the post-prune
corpus digest, query/feature output hashes, and the deterministic dev selection.

| split | miss | sub-DAG | sub-DAG >=20 | inline | same-unit | unrecovered | extraction/other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dev | 158 | 60 | 14 | 16 | 16 | 44 | 22 |
| held-out | 95 | 29 | 8 | 5 | 10 | 10 | 41 |

These are optimistic ceilings. In particular, `intersection_mass` is not a connected
witness and never authorizes acceptance.

The regenerated [dev stage artifact](../bench/labels/missed_worthy_stage_audit_post_817_2026_07_12.dev.v1.json)
accounts for every remaining dev miss:

| dev raw stage | families |
| --- | ---: |
| direct accepted pair | 0 |
| candidate only | 40 |
| extracted, no direct candidate | 85 |
| missing unit | 33 |

The accepted-pair grouping cohort fixed by #817 is therefore gone rather than counted a
second time.

## Frozen dev source audit

The selection SHA-256 is
`419ab53ca32c9d33443c64dca03687220d2d92940e47bb8dd2df4b61e40d3712`:
five rows per language across C, Go, Java, Python, Ruby, Rust, and TypeScript. Every
judgment is recorded in the [v2 decision artifact](../bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json),
while adjacent [source bounds](../bench/labels/missed_worthy_audit_source_bounds_post_817_2026_07_12.dev.v1.json)
bind each evidence interval and cited file to the frozen candidate and source hashes.

| judgment origin | rows |
| --- | ---: |
| unchanged frozen #816 decision | 16 |
| exploratory read before #820 freeze | 3 |
| reviewed after #820 selection freeze | 16 |

| blocker classification | selected rows |
| --- | ---: |
| connected anchor / sub-DAG construction | 7 |
| candidate generation | 6 |
| unit extraction / missing unit kind | 10 |
| same-unit actionable fragment | 3 |
| no coherent general mechanism | 9 |

## Cohort comparison and decision

The three leading mechanisms do not have equal evidence:

| cohort | exact dev ceiling | selected reviewed | source-coherent | no-go | coverage |
| --- | ---: | ---: | ---: | ---: | --- |
| candidate-only + sub-DAG >=20 | 9 | 9 | 6 | 3 | complete |
| same-unit window | 16 | 4 | 3 | 1 | sampled |
| missing-unit extraction | 33 | 6 | 6 | 0 | sampled, heterogeneous |

Same-unit evidence is real, but it needs a new bounded-fragment output contract; reporting
the enclosing function is forbidden. Missing-unit evidence is also real, but the reviewed
rows cover unrelated Go helpers, Java/Python test methods, Rust local items, and JavaScript
test callbacks. No one extraction invariant owns that mass.

The selected tranche is therefore the complete nine-row candidate-only connected-witness
cohort. Six rows are required recoveries:

- `curl:2a436119a08187ba`
- `delve:de84b1952aa09a8e`
- `graphhopper:2acc71582f85cc79`
- `mockito:d82f6e75097748da`
- `ripgrep:519afdcaed73af0d`
- `thor:07e233ddffad07f0`

Three rows are mandatory hard negatives:

- `gson:4014d594ab6a8e54`
- `scrapy:be72d6b46ad8eaf1`
- `serde_json:946bfa61cb71d562`

The smallest sound invariant is:

> Every newly accepted pair carries one connected mapped witness on both sides whose
> entry, exit, ordering, and relevant effects are preserved. Multiset mass and transitive
> A-B-C overlap never substitute for that witness.

The implementation may not change candidate generation, thresholds, ranking,
default-surface policy, or #817 grouping. Its dev gate requires all six positives, none of
the three no-go rows, zero worthy regressions, and zero false merges. Every added output
must be connected-witness-backed; unrelated added rows are zero. The checked #809 slice
budgets at most 2% default-output growth and 5% / 5 ms control-adjusted runtime cost.

The result-dependent follow-up is
[#821](https://github.com/corca-ai/nose/issues/821). If a sound connected witness cannot
meet those gates without admitting disconnected mass or lowering thresholds, the mechanism
is a no-go and the next issue becomes a language-bounded same-unit fragment tranche. If a
sound implementation exceeds product/runtime budget, bound or optimize it rather than
relaxing precision. A surviving language-specific rule must be split into the narrowest
language/construct issue.

## Held-out mechanical confirmation

Only after the dev proposal commit did the checked
[v2 held-out stage artifact](../bench/labels/missed_worthy_stage_confirmation_post_817_2026_07_12.heldout.v2.json) rerun
the raw mechanical method once:

| held-out raw stage | families |
| --- | ---: |
| direct accepted pair | 0 |
| candidate only | 14 |
| extracted, no direct candidate | 35 |
| missing unit | 46 |

The pre-registered gate expected exactly two candidate-only + sub-DAG>=20 rows in C and
Java and observed `vim:a3155684430a48be` and `jsoup:9b909cd145cab0a1`. They are mechanical
cross-split confirmation only. Their source was not read, they are not implementation
fixtures, and no positive source claim is made for them.

## Reproduction and validation

```sh
CARGO_TARGET_DIR=target/issue-820 cargo build --release -p nose-cli

python3 bench/labels/eval_by_language.py \
  --labelset bench/labels/refactoring_families.v6.json \
  --nose target/issue-820/release/nose \
  --comparison-nose target/release/nose \
  --rank extractability --bootstrap 500 \
  --cache-dir target/issue-820/eval-cache \
  --json-out bench/labels/product_quality_evaluation_post_817_2026_07_12.v1.json

python3 bench/labels/recall_ceiling_probe.py \
  --nose target/issue-820/release/nose --repos-root bench/repos \
  --recall-labelset bench/labels/refactoring_families.v5.json \
  --precision-labelset bench/labels/refactoring_families.v6.json \
  --evaluation-report bench/labels/product_quality_evaluation_post_817_2026_07_12.v1.json \
  --corpus-manifest bench/goldens/corpus.json \
  --prune-manifest bench/labels/prune_manifest.json \
  --json-out bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json

python3 bench/labels/recall_ceiling_probe.py \
  --validate bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
python3 bench/labels/missed_worthy_stage_audit.py \
  --validate bench/labels/missed_worthy_stage_audit_post_817_2026_07_12.dev.v1.json \
  --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
python3 bench/labels/recall_ceiling_probe.py \
  --validate-decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json \
  --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
python3 bench/labels/missed_worthy_source_bounds.py \
  --validate bench/labels/missed_worthy_audit_source_bounds_post_817_2026_07_12.dev.v1.json \
  --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
  --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
python3 bench/labels/missed_worthy_heldout_confirmation.py \
  --validate bench/labels/missed_worthy_stage_confirmation_post_817_2026_07_12.heldout.v2.json \
  --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
  --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
```
