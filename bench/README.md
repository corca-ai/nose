# nose benchmark

Measurement data for nose, frozen and audited so detector changes stay directly
comparable over time. There are two layers, both over the same corpus split.

## Corpus

- `goldens/corpus.json` — the repo list with `split: dev|heldout` (the generalization
  gate). The checkouts themselves are NOT committed; point `--repos` at a directory whose
  immediate subdirectories are the repo ids (e.g. `bench/repos`, populated by
  `setup_repos.sh`).
- `setup_repos.sh` clones at pinned commits, then **prunes generated/vendored files**
  (npm/vendored deps, build output, committed Javadoc/minified bundles, `@generated`
  source) — not code a developer refactors, and they skew clone measurements toward
  boilerplate. Test fixtures (e.g. `prettier/tests/format`) are intentionally kept (test
  data is a separate category from generated/vendor).

## Primary metric — the refactoring-family labelset (current)

`labels/refactoring_families.v5.json` (105 repos, ~9.5k families) is the active gold set,
built by a 3-persona panel with tie-break/arbiter escalation (see `labels/RUBRIC.md`,
`labels/schema.json`, `labels/README.md`). Evaluate with:

```sh
python3 bench/labels/eval_by_language.py        # P@10 + recall, per language, bootstrap 95% CIs
```

This is the metric the experiment log (`docs/experiments.md` §AU onward) reports.
`v5` is the frozen current version; earlier versions (`v1`–`v4`) are kept for the
historical measurements those sections cite.

## Type-4 benchmark factory

`type4/` contains the executable implementation for the evidence-carrying synthetic Type-4
benchmark factory described in `docs/type4-benchmark.md`. It generates pair-level
positive and hard-negative cases from semantic spec cards across every supported language
surface, records the semantic proof capabilities currently available per surface, then
evaluates `nose scan --mode semantic` against the manifest.

```sh
python3 bench/type4/generate.py --out-dir /tmp/nose-type4-seed
python3 bench/type4/eval_manifest.py /tmp/nose-type4-seed/manifest.json
```

## Original Type-4 gold set (historical)

The project's first benchmark, superseded by the labelset above but kept for the results
in `docs/experiments.md` §A–§AT:

- `goldens/semantic_duplicates.v2.json` — 327 audited duplicate pairs, 16 repos.
- `goldens/hard_negatives.v1.json` — 39 confusable non-duplicate pairs (precision guard).
- the other `goldens/*.json` (silver/synthetic/typed4/precision-sample) — intermediate
  gold sets and pools from that era; the `judge/` scripts are the LLM-judge pooling
  pipeline that produced them.

## Reproduction scripts

The Python scripts here are the reproduction record for the experiment log — each
documented result names the script that produced it. They are not part of the build or CI
(which only need the Rust workspace + `setup_repos.sh` for `nose verify`/`detect`).
