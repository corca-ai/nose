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
  static imports, static projections, nullish defaults, own-property guards,
  record-shape guards, string prefix/suffix predicates, literal and typed dynamic collection membership,
  literal map-default lookup, map key-membership predicates,
  null/none/nil/option presence predicates including Rust
  option-pattern predicates, scalar absolute-value and min/max idioms, and
  C total-order three-way comparator guard/ternary forms and Java
  statically-false loop-entry guard forms, plus
  unsafe/unproven binding boundaries;
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

`eval_manifest.py` and `frontier.py` accept both the current versioned
`nose scan --format json` object and the older raw `families` array when `--scan-json`
is supplied, so saved scan output can be reused without post-processing.

Before spending implementation time on a new axis, run a focused preflight against the
baseline and candidate binaries:

```sh
python3 bench/type4/preflight_axis.py --axis null_presence_predicate --out-dir /tmp/nose-type4-preflight
```

The preflight fails when the candidate has false merges, when the baseline already covers
all strict positives with no false merges, or when the candidate does not reduce positive
misses or remove baseline false merges.

Current smoke result with the default ring cross-surface set:

```text
items: 3127
positive recall: 1098/1098
hard-negative false merges: 0/2029
```

With `--cross none`, same-surface coverage alone currently reports:

```text
items: 1940
positive recall: 682/682
hard-negative false merges: 0/1258
```

With `--cross all`, the dense corpus now has 6710 items. The routine dense smoke uses
coverage-preserving compaction before evaluation:

```text
selected items: 1841/6710
positive recall: 621/621
hard-negative false merges: 0/1220
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

Use the frontier summary to choose a micro-batch of about three adjacent under-merge
classes that share one proof invariant, add focused convergence tests, patch lowering or
value-graph normalization, and rerun the generated positives plus hard negatives. The
batch should be narrow enough that one exact proof rule explains every new positive; if
the positives need unrelated proofs, split them into separate loops. That loop is the
intended co-evolution process: the benchmark grows by adversarial siblings while the exact
detector gains new semantic convergence rules.

## Prioritize the next frontier

Mine the pinned real-repo corpus before choosing the next semantic axis:

```sh
python3 bench/type4/prioritize_frontier.py \
  --cache /tmp/nose-frontier-priorities.cache.json \
  --json-out /tmp/nose-frontier-priorities.json \
  --markdown-out bench/type4/FRONTIER_PRIORITIES.md
```

The ranking is a triage input, not gold evidence. It combines real-code frequency,
repo/language spread, estimated implementation cost, soundness risk, scope, and whether a
frontier is already covered. The next loop should prefer high-scoring all-language or
multi-language axes unless a language-family axis is fixing an urgent soundness bug.

Use the prioritizer as a repeated pattern loop, not as a one-off report:

- quantify broad pattern frequency and extraction gaps across the pinned repos;
- classify broad-probe overreach separately from true extraction gaps;
- add a narrow synthetic micro-batch, usually three adjacent positives, with hard-negative
  siblings that attack the shared proof invariant;
- patch the detector only where the miss is a strict proof gap;
- compare installed/release and modified detectors on real repos;
- update the candidate status so the next cost-effective axis rises.

Use `--cache` for routine reruns. The cache is invalidated when candidate regexes, probe
regexes, file metadata, `--max-bytes`, or `--sample-limit` change; unchanged corpus reruns
reuse the previous result. The report also lists top matching repos per candidate, which
are the default audit sample before doing a wider real-repo scan.

For a new semantic axis, run this loop at least once end to end before adding more
patterns. Continue for three to five passes while the top candidate still changes or real
delta audits expose missed strict families. Stop expanding that axis when synthetic
positives are closed, hard negatives stay clean, and the prioritizer has moved the axis to
`covered-current`.

Real-corpus audit findings are tracked in `real_frontier.v1.json`. Each item records the
repo-relative span, semantic claim, evidence, detector status, proof invariant, adjacent
hard negatives, and batch assignment. Use `already-covered`, `real-miss`, `hard-negative`,
`unsupported`, and `closed` as the audit states so prioritizer frequency, real evidence,
and detector progress stay separate.

## CI smoke

Run the Type-4 smoke gate locally:

```sh
scripts/type4-smoke.sh
```

Useful knobs:

```sh
GATE=focused AXIS=string_prefix_suffix NOSE=target/debug/nose scripts/type4-smoke.sh
GATE=core AXIS=string_prefix_suffix NOSE=target/debug/nose scripts/type4-smoke.sh
GATE=full AXIS=string_prefix_suffix NOSE=target/debug/nose scripts/type4-smoke.sh
OUT_DIR=/tmp/nose-type4-next CROSS=none NOSE=target/debug/nose scripts/type4-smoke.sh
BASELINE_JSON=/tmp/nose-type4-seed/frontier.json scripts/type4-smoke.sh
OUT_DIR=/tmp/nose-type4-all CROSS=all scripts/type4-smoke.sh
OUT_DIR=/tmp/nose-type4-all-full COMPACT_DIR=/tmp/nose-type4-all-core SUITE=core CROSS=all scripts/type4-smoke.sh
```

`GATE=focused` requires `AXIS` or `PROPOSAL_PREFIX` and defaults to `CROSS=none`, so the
inner detector loop exercises only the selected semantic class. `GATE=core` keeps the same
focused filters but runs the coverage-preserving compact selector, and `GATE=full` runs the
selected full manifest without compaction. Omit `GATE` for the historical full smoke.

`SUITE=core` first generates the full manifest, then writes a compact manifest whose cases
preserve proposal, split, representation, transform, hard-negative tag, cross-surface,
semantic-axis, and capability-state coverage. Use the compact suite for inner-loop detector
work; keep full ring and dense all-cross runs as periodic validation.
