# Scan-regression compare summary

> Investigation triggers, not merge blockers (issue #37, rule 7).
> Artifact identity: current `source_git_describe` / `build_ref` name the checkout and binary that generated this report. If the report is committed after generation, those refs intentionally point at the generator commit, not at the later artifact commit.

## Binaries

| | baseline | current |
|---|---|---|
| version | `nose 0.5.0` | `nose 0.5.0` |
| sha256 | `bdd103dfc3e3e3d26545c2aac04049dbeb5aa8250fb84567829ffcfe6040f079` | `903db79891bdec5ccb4d0f80cce121f9ccb1e3add89c234e56835b4ab343aecc` |
| source_git_describe | `v0.5.0-188-gcb0c549` | `v0.5.0-221-g1cb8a79-dirty` |
| build_ref | `issue-51-generator@cb0c549` | `hof-budget@1cb8a79` |

## Results

- repos compared: 0
- repos with triggers: 0
- skipped (missing in one side): boltons, chi, cmark, gin, junit5, ky, liquid, serde_json

No investigation triggers fired. ✅

## HoF Value-Graph Budget Smoke

- features wall: 11.42ms (budget 3000ms)
- semantic scan wall: 12.10ms (budget 3000ms)
- scan total families: 0

| case | tokens | value fp nodes | return fp nodes | budgets |
|---|---:|---:|---:|---|
| `deep_hof_chain_budget` | 527 | 377 | 1 | tokens<=900, value<=450, returns<=12 |
| `wide_hof_chain_budget` | 1299 | 326 | 1 | tokens<=1600, value<=1200, returns<=12 |
