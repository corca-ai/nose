# Proof strength versus extraction actionability (#844)

Issue #844 closes as a checked **no-go**. Exact semantic proof answers “do these spans
compute alike?”; it does not answer “is extracting this repetition worthwhile?” The
existing `exact-value-graph` and `shared-sub-dag` protection from the coarse `trivial`
and `shallow-extraction` shape rules therefore remains in place. There is no product
output, ranking, detector, surface, or performance change.

## Audited decisions

The [#841 dev taxonomy](default-head-failure-taxonomy-841.md) provides two related
counterfactuals. Treating proof kind itself as non-actionable fails badly:

| proven cohort | reviewed | non-action | precision | worthy hard negatives |
|---|---:|---:|---:|---:|
| bare-default head | 57 | 22 | 38.60% | 35 |
| deterministic deep sample | 7 | 5 | 71.43% | 2 |
| combined | 64 | 27 | 42.19% | 37 |

The narrower counterfactual removes only the current exemption and then applies the
existing size and parameter-density rules. It still misses the required 90% non-action
precision:

| protected shape rule | reviewed | non-action | precision | worthy hard negatives |
|---|---:|---:|---:|---:|
| `trivial` (`mean_lines <= 4`) | 1 | 0 | 0% | 1 |
| `shallow-extraction` (`params >= 0.33 * shared_lines`) | 4 | 3 | 75% | 1 |
| combined | 5 | 3 | 60% | 2 |

The five direct boundary rows are source-hash bound and were independently reviewed by
three agents. All three reviewers returned the same label and the same no-go decision:

- Clap's four identical four-line `build_help` functions are a worthy shared-helper
  extraction, so “small” is not mechanically equivalent to “not actionable.”
- Delve's six architecture-specific unwind implementations are parallel by design;
  their common scaffold does not justify a large, architecture-obscuring parameter list.
- SwiftLint's five repeated configuration tests are a worthy parameterized-test/helper
  opportunity despite their parameter density.
- Lua's `discharge2reg` and `codenot` switches perform different compiler state
  transitions; their shared control shape is coincidental.
- Thor's two test fragments share too little cohesive behavior to justify another helper.

No post-hoc split by repository, language, scope, file relation, symbol, or extraction
shape is admitted. Such a split has no common mechanical cause, would be selected after
reading the labels, and would violate the scope-blind actionability contract. The two
deep non-action rows alone are not a confirmatory cohort; a perfect two-of-two result
cannot establish a 90% boundary.

## Frozen hard negatives

The 37 worthy proven rows remain a protection set: 23 helper extractions, nine
parameterizations, three data-table extractions, and two base extractions. In addition to
the direct small-helper boundary in Clap, the checked closeout names the two large
Prometheus documentation-table families and Zap's paired benchmark field tables. These
demonstrate that strong or exact proof can strengthen an actionable refactoring instead
of making it noise.

The checked [`proof_actionability_no_go_2026_07_14.dev.v1.json`
artifact](../bench/labels/proof_actionability_no_go_2026_07_14.dev.v1.json) binds both #841
artifact digests, all 64 proof-backed keys, all 37 worthy keys and reason
counts, the five current-exemption rows and source-bound hashes, the named helper/table
boundaries, and the unanimous three-review result. Its validator reconstructs every
count and row from the frozen taxonomy and rejects truth, witness, predicate, source
bound, cohort, input, decision, or preservation changes.

## Preservation and future admission bar

Because no runtime product path changes, family membership and fingerprints, witnesses
and provenance, accepted-pair coverage, fold forest, surfaces and reasons, default order,
full-universe recall, false-merge and canon-preservation gates all remain identical to
#843. The release binary is byte-identical to the immediate parent; consequently #844's
incremental runtime and output cost is exactly zero. The required official-v0.19.0
performance baseline remains the one already priced by #843, rather than a source-rebuilt
substitute.

A future rule must be pre-registered before its confirmatory source review, clear 90%
non-action precision independently, match none of the frozen worthy hard negatives, and
fail open on missing evidence. If admitted, it must be a presentation-only transition:
families remain recoverable through `all top=0` and `id=`, while semantic membership,
witnesses, fingerprints, and the pre-transition fold opportunity forest remain stable.

## Validation

```sh
python3 bench/labels/proof_actionability_no_go.py --self-test
python3 bench/labels/default_head_taxonomy.py --self-test
python3 bench/labels/default_head_taxonomy.py validate \
  bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json \
  --pragmatic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.pragmatic.v1.json \
  --dedupe bench/labels/default_head_taxonomy_votes_2026_07_13.dev.dedupe.v1.json \
  --skeptic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.skeptic.v1.json
cargo test -p nose-detect actionability -- --nocapture
./scripts/check-ci-local.sh --fast
./scripts/check-docs.sh
```

#845 may now tune only transparent, deterministic ranking evidence over the residual
default head. Held-out source remains closed until #846.
