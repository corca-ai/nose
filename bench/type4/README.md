# Type-4 benchmark factory

This directory contains the first executable pieces of the Type-4 benchmark factory
described in [`docs/type4-benchmark.md`](../../docs/type4-benchmark.md).

The factory is evidence-carrying by design:

- proposal cards describe the semantic class to explore;
- the capability matrix records which proof facts each supported surface currently emits;
- `generate.py` emits positive and hard-negative source pairs for every supported
  language surface;
- positives carry same-spec/spec-interpreter evidence;
- negatives carry concrete counterexamples;
- evidence is level-tagged (`E1` same-spec/property evidence, `E2` concrete
  counterexample evidence in the current synthetic slice);
- generated source paths and metadata are written into a manifest.

The generated manifest is a candidate benchmark artifact. A case becomes gold only after it
passes the promotion rules in the docs.

## Generate the seed corpus

```sh
python3 bench/type4/generate.py --out-dir /tmp/nose-type4-seed
```

The manifest is written to `/tmp/nose-type4-seed/manifest.json`; source files live under
`/tmp/nose-type4-seed/sources/`.

By default the generator emits:

- same-surface positive and negative pairs for all supported base languages;
- same-surface pairs for Vue, Svelte, and HTML script extraction;
- held-out indexed-loop positives and same-template negatives for single-list specs;
- C pointer-length contract hard negatives for skipped-first and stride-two loops;
- sign-normalizing `sum(abs(x))` map/reduce cases across every supported surface;
- semantic-axis cases for immutable bindings, proven callee identity, literal table access,
  and unsafe/unproven binding boundaries;
- a ring of cross-language positive pairs and cross-template hard negatives so every
  supported surface participates in cross-language coverage without exploding the seed size.

Use `--cross all` to generate every cross-surface positive pair and cross-template
negative sibling.

## Evaluate a semantic scan

```sh
python3 bench/type4/eval_manifest.py /tmp/nose-type4-seed/manifest.json
```

The evaluator runs `nose scan --mode semantic` over the generated sources and reports
positive recall plus hard-negative false merges. Use `--fail-on-false-merge` when this
becomes a CI gate.

Current smoke result with the default ring cross-surface set:

```text
items: 1479
positive recall: 616/616
hard-negative false merges: 0/863
```

With `--cross none`, same-surface coverage alone currently reports:

```text
items: 1061
positive recall: 407/407
hard-negative false merges: 0/654
```

With `--cross all`, the dense corpus now has 3220 items. The routine dense smoke uses
coverage-preserving compaction before evaluation:

```text
selected items: 428/3220
positive recall: 180/180
hard-negative false merges: 0/248
```

These are not product-quality scores. They are frontier measurements for the exact semantic
channel: missed positives are under-merge work items, while hard-negative false merges are
soundness bugs.

The coverage-expansion iterations and detector co-evolution loops are recorded in
`ITERATIONS.md`.

## Type-4 loop Definition of Done

A detector co-evolution loop is complete only when it leaves all of these artifacts:

- a frontier summary showing the missed exact-positive class selected for the loop;
- a focused convergence or hard-negative regression test;
- a detector change in frontend lowering, idiom canonicalization, or the shared value graph;
- a generated benchmark comparison showing positive misses went down or a false-merge bug
  was removed;
- a hard-negative check with `eval_manifest.py --fail-on-false-merge`;
- an iteration note in `ITERATIONS.md`.

Adding proposal cards or generated cases without a detector change is a coverage-expansion
iteration, not a detector co-evolution loop.

## Summarize the improvement frontier

After evaluation, group misses into detector work items:

```sh
python3 bench/type4/frontier.py /tmp/nose-type4-seed/manifest.json
```

Write machine-readable frontier output and compare it with a previous loop:

```sh
python3 bench/type4/frontier.py /tmp/nose-type4-seed/manifest.json \
  --json-out /tmp/nose-type4-seed/frontier.json

python3 bench/type4/frontier.py /tmp/nose-type4-next/manifest.json \
  --compare-to /tmp/nose-type4-seed/frontier.json \
  --compare-out /tmp/nose-type4-next/frontier-compare.json \
  --fail-on-regression
```

Use the frontier summary to choose one narrow under-merge class, add a failing convergence
test, patch lowering or value-graph normalization, and rerun the generated positives plus
hard negatives. That loop is the intended co-evolution process: the benchmark grows by
adversarial siblings while the exact detector gains new semantic convergence rules.

## CI smoke

Run the Type-4 smoke gate locally:

```sh
scripts/type4-smoke.sh
```

Useful knobs:

```sh
OUT_DIR=/tmp/nose-type4-next CROSS=none NOSE=target/debug/nose scripts/type4-smoke.sh
BASELINE_JSON=/tmp/nose-type4-seed/frontier.json scripts/type4-smoke.sh
OUT_DIR=/tmp/nose-type4-all CROSS=all scripts/type4-smoke.sh
OUT_DIR=/tmp/nose-type4-all-full COMPACT_DIR=/tmp/nose-type4-all-core SUITE=core CROSS=all scripts/type4-smoke.sh
```

`SUITE=core` first generates the full manifest, then writes a compact manifest whose cases
preserve proposal, split, representation, transform, hard-negative tag, cross-surface,
semantic-axis, and capability-state coverage. Use the compact suite for inner-loop detector
work; keep full ring and dense all-cross runs as periodic validation.
