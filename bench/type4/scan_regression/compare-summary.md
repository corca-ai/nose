# Scan-regression compare summary

> Investigation triggers, not merge blockers (issue #37, rule 7).
> Artifact identity: current `source_git_describe` / `build_ref` name the checkout and binary that generated this report. If the report is committed after generation, those refs intentionally point at the generator commit, not at the later artifact commit.

## Binaries

| | baseline | current |
|---|---|---|
| version | `nose 0.5.0` | `nose 0.5.0` |
| sha256 | `bdd103dfc3e3e3d26545c2aac04049dbeb5aa8250fb84567829ffcfe6040f079` | `bdd103dfc3e3e3d26545c2aac04049dbeb5aa8250fb84567829ffcfe6040f079` |
| source_git_describe | `v0.5.0-188-gcb0c549` | `v0.5.0-189-g12ec95e` |
| build_ref | `issue-51-generator@cb0c549` | `issue-51-generator@12ec95e` |

**Note: identical binary sha256 — any output/runtime delta below is environment noise, not a code change.**

## Results

- repos compared: 8
- repos with triggers: 0

No investigation triggers fired. ✅
