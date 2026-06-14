# Default-surface noise audit — 2026-06-14

A fresh-repo head-of-ranking audit (the [design](design.md) §2c standing instrument:
*"field evaluation on unseen neighbours"*) run to re-judge three pieces of user
feedback against the **current** binary, not the binary the reporters ran:
[#263](https://github.com/corca-ai/nose/issues/263) (TS/React triage noise),
[#264](https://github.com/corca-ai/nose/issues/264) (Go-CLI test-scaffolding
dominance), [#11](https://github.com/corca-ai/nose/issues/11) (refactorability reason
codes), [#353](https://github.com/corca-ai/nose/issues/353) (JSX declarative shapes).

Most of those issues' *literal* asks had already shipped between filing and this audit
(opportunity-grouping, the `declaration` surface, `scope`, the reinvented-helper channel,
the value-vs-shape witness). The open question this audit answers: **does the residual
"too noisy to triage" complaint still reproduce, and if so, what is the measured,
principle-respecting lever?**

Two public, unseen (non-corpus) repos, one per reporter shape:
[goreleaser](https://github.com/goreleaser/goreleaser) (Go CLI, 388 `.go` / 171 `_test.go`)
and [excalidraw](https://github.com/excalidraw/excalidraw) (React/TSX, 602 TS/TSX, 290 TSX).

## 1. The complaints reproduce on current `main`

`nose scan . --format json --top 0`, HEAD binary:

| | goreleaser | excalidraw |
|---|---:|---:|
| default-surface families | 721 | 749 |
| test scope | 76% | 60% |
| `copy-paste-run` (weakest witness) | 74% | 83% |
| **test × copy-paste-run (one bucket)** | **59%** | **58%** |
| proven moat (`exact-value-graph` + `shared-sub-dag`) | **4%** | **5%** |

The bare-default head is ~60–76% test scope; in goreleaser the first 18 ranked families
are 14 `*_test.go` pairs, first prod at rank 3. The soundness moat the whole proof program
defends is 4–5% of what the default surface actually shows. `scope`-weighting (shipped
2026-06-03) down-weights but does **not** move test off the ranking head. #263 and #264 are
**live**, not obsolete.

## 2. Method — labeling which *extraction shapes* are non-actionable

A 206-family stratified sample of the two default surfaces (94 `copy-paste-run`, 71
`structural-similarity`, 34 `shared-sub-dag`, 7 `exact-value-graph`; 107 test / 96 prod / 3
mixed), labeled by five parallel judge agents that **read the actual source** of each
family. The rubric encodes nose's standing principle that *test duplication is a real smell
— `scope` is a context tag, never a worthiness penalty*
([report.rs](../crates/nose-detect/src/report.rs), [field-evaluation](field-evaluation.md)):

- **KEEP** (belongs on the default head): a cleanly extractable shared helper; a duplicated
  production block; **test code that reimplements production logic**; a fixture that is one
  domain concept duplicated and centralizable.
- **DEMOTE** (intentional / shallow — buries the signal): AAA scaffolding documenting
  *separate scenarios*; table-test rows; declarative JSX with no named concept; import/type
  boilerplate; a *shallow* extraction where the shared part is mostly varying spots
  (params ≈ shared_lines); idiomatic same-file repetition (switch arms, struct fields).
- **JUDGMENT** only if the source genuinely under-determines it.

Result: **KEEP 71 / DEMOTE 135 / JUDGMENT 0.** No family in the sample reimplemented
production logic in test code; worthy test *fixtures/helpers* did occur and were KEEP.

## 3. The decisive finding — the noise is two populations

**(a) Decidable-by-shape noise (~43% of DEMOTE).** Separable scope-blind at high precision,
near-zero worthy loss:

| demotion rule (extraction-shape, scope-agnostic) | precision | KEEP lost | noise recall |
|---|---:|---:|---:|
| `unproven ∧ ratio≥0.33` (shallow, the clean cut) | **0.89** | 5 | 0.30 |
| `structural-similarity ∧ ratio≥0.33` | 0.91 | 2 | 0.15 |
| `copy-paste-run ∧ same_file ∧ ratio≥0.33` | 0.91 | 1 | 0.07 |

(`ratio` = params / shared_lines; high ratio = "the helper would be almost all
parameters".) DEMOTE reason tags in this population: `shallow-high-param` (16),
`idiomatic-same-file` (17), `trivial` (25), `import-type-noise` (7), `jsx-declarative` (4,
the #353 class), `table-rows` (3).

**(b) AAA test-scaffold bulk (~43% of DEMOTE; 58 of 135).** Long verbatim test-block
copies documenting separate scenarios. **Not separable from worthy test fixtures/helpers by
any feature.** Within the `test × copy-paste-run` cell (n=70, 17 KEEP / 53 DEMOTE), the
KEEP and DEMOTE medians are ~identical (files 2 vs 1, modules 1 vs 1, members 3 vs 3,
mean_lines 14 vs 13); every structural cut caps at 0.74–0.77 precision — the same as
demoting the whole cell, which costs 17 worthy KEEP. Only the labeler's *semantic read*
("one domain concept worth centralizing" vs "AAA documenting separate scenarios")
separated them. This is **judgment-deep** — §2's consumer-LLM call, not the detector's.

**The `scope` lever is measured-bad.** `scope = test` captures 107, precision 0.74, and
demotes **28 worthy KEEP** families (worthy fixtures, test-helpers, prod-blocks living in
test files) — it contradicts the principle *and* loses value. (Corollary: proven channels
are only keep-rate 0.41, so "proven ⇒ always default" is also false; shape filters apply
across witnesses but never demote a proven family on shape alone.)

## 4. The lever — one capability, split at the decidability boundary (§2b)

**Shipped** (both arms, this PR — see [CHANGELOG](../CHANGELOG.md) Unreleased):

- **(a) Decidable actionability vocabulary** ([#11](https://github.com/corca-ai/nose/issues/11)):
  shipped as two JSON fields — `actionability_reason` (why a family is *not* a clean candidate)
  and `extraction_shape` (the structural shape if a clean candidate is acted upon) — classification,
  not a `refactorability_score`/`confidence` verdict (§2). The decidable reason codes:
  - **`shallow-extraction`** (unproven, helper-mostly-parameters) — `shallow` surface; 0.89
    precision, **−36%** (goreleaser) / **−34%** (excalidraw), 0 proven demoted.
  - **`trivial`** (`mean_lines` ≤ 4, unproven) — `hidden` surface; re-measured against the labels
    here at **0.95 precision** (loses 1 of 24: a 3-line structural helper).
  - `declaration-run`, `generated-source` — the pre-existing source-derived classes, now also
    surfaced under the unified `actionability_reason` field.
  - **`idiomatic-repetition` = NO-GO.** No decidable rule separates "same-file switch-arm/struct
    repetition" from real same-file production duplication with the available features (no AST
    switch-arm marker): the broad `same_file ∧ Block` cut is 0.68 precision and loses 21 worthy
    (10 dup-prod-block, 6 worthy-fixture); every tighter cut either catches ~nothing or is
    subsumed by `trivial`. Left to the consumer as evidence.

  All reason codes are scope-blind and never fire on a proven channel; the `markup`/JSX code
  ([#353](https://github.com/corca-ai/nose/issues/353)) was also a NO-GO — see §5.
- **(b) AAA bulk → scope-aware *rendering*, not penalty.** Collapse/summarize test-scope
  families beneath prod findings on the human surface (the way overlapping slices already
  fold into one opportunity); **nothing dropped** — every test family stays in the ranking,
  in `--format json`, and under `--scope test`. This honors *test-dup-is-a-smell ·
  nothing-dropped · scope-is-context-not-penalty* while answering #264's "let me see prod
  first." The worthy-vs-noise call inside the collapsed set is the consumer's, with nose
  carrying the evidence (§2).

This is **capabilities over features**: a single capability — decidable reason codes plus
scope-aware rendering — answers #11/#263/#264/#353 together, instead of the issues' literal
pile of per-category heuristics (role taxonomy, bug-prone weighting, shallow-abstraction
penalty), most of which are the judgment §2 already delegates to the consumer.

Projection on the two surfaces: the shape cut removes ~35%; with test rendered beneath prod,
the prod-and-not-shallow head is **105** (goreleaser) / **208** (excalidraw) families —
converging on #263's *"~20 worth reading"* once the head is what the first screen shows.

## 5. [#353](https://github.com/corca-ai/nose/issues/353) (JSX markup) — measured NO-GO as a detector filter

The follow-on idea was a decidable `markup` class: a family whose every member span is
**provably behavior-free JSX** (no `subtree_executes` node — no call, arrow, `await`, `new`,
function, or `yield` anywhere in the JSX subtree), demoted off the default like `declaration`.
Measured on two React repos before building it:

| repo | default-surface families with all-`.tsx`/`.jsx` locations | JSX-ish span (starts `<`/`{`) | **decidable behavior-free markup** |
|---|---:|---:|---:|
| excalidraw | 314 | 18 | **1** (static SVG `<path>` data) |
| react-bootstrap | 23 | 2 | **0** |

**NO-GO** — the decidable, safe `markup` filter catches ~0–1 families, for structural reasons,
not a tuning miss:

1. Clone families are whole-component **functions**, so their spans carry `function`/`return`/`=>`
   code lines — never pure markup (the `declaration` line-grammar would poison them).
2. Real JSX embeds `clsx(...)`, event handlers, and `{items.map(...)}` — all `subtree_executes`
   nodes, so a behavior-free JSX span is rare.
3. Catching the *actual* JSX noise the field reports name (the Homepage `.map` list-render, the
   `clsx`-wrapped button `<input>`) requires whitelisting list-render / class-helper idioms —
   which crosses from **decidable** into **judgment** ("is this list-render worth extracting?").
   §2 puts that on the consumer's model, not the detector.

So JSX/markup-presentational-ness is **not** a detector surface; it becomes one **evidence**
input for the consumer's own call (the #11 vocabulary), exactly like the AAA-scaffold bulk in §3.
Shipping a `markup` surface for ~1 family would be dead complexity against *capabilities over
features*. #353 is closed measured-negligible; the cheap re-run path is the same per-repo
measurement script if a future component-library corpus suggests otherwise.

## 6. Tier-2 sibling-family folding — measured NO-GO

§4(b) collapses *overlapping slices of one region* and renders test beneath prod, but the bare
default is still a long list (fresh `--top 0` default families: rxjs 636, prometheus 1455,
redis 484, zod 397). The tempting next lever: fold the **per-variant sibling-family wall** —
many *distinct* families that are copies of one shape (rxjs per-operator marble tests,
prometheus per-service AWS-discovery inits, serde owned/borrowed impls) — into one
"opportunity", lifting the genuine standalone wins into view. Fully implemented and measured on
a 7-repo slice (rich, serde_json, zod, rxjs, prometheus, redis, cobra), reading cluster
members' real source. **NO-GO, for two structural reasons:**

1. **nose already folds the real repetition** into multi-member families: `finalize-spec.ts` is
   one 31-member family, serde `write_i8` one 10-member family. The *separate* families that
   remain are below the clone-merge threshold *because they are genuinely structurally
   distinct* — there is little cross-family redundancy left to fold.
2. **Cross-family folding can't cluster coherently.** A metadata key
   `(scope, extraction_shape, dir, size-band)` "reduces" the surface 72–89% but **incoherently**
   (rich's best finding `replace_link_ids` grouped with unrelated tests; a 4-line
   `__rich_measure__` with an 86-line `__init__`). A leaf-abstracted value-DAG shape key
   (per-node Merkle hash over `(VgOp, arity, child-hashes)`, multiset Jaccard / overlap) does
   not rescue it: exact-match folds ~nothing; and even **complete-link @ Jaccard 0.6 groups
   `map.rs new()`/`iter()` with `deserialize_tuple_struct`** — a small unit's leaf-abstracted
   whole-unit shape is generic ("calls + return" ≈ "construct + return"), so unrelated small
   units are mutually similar. No metric × threshold × linkage × min-node floor separated true
   siblings from generic-shape collisions; cost was **+67% scan time** (re-lowering the whole
   surface for shapes).

Shipping it would hide distinct genuine findings under a misleading "same shape" label *and*
slow scans — strictly worse. Recorded in [experiments §CO](experiments.md). The independent,
source-verified lever stands: a decidable **evidence** flag for language-forced parallel
duplication (`owned-vs-borrowed` / `covariant-type-only` / `high-param-ratio` from the graded
per-spot `class`, evidence-not-verdict) and retiring "proven ⇒ trust/lead" (serde's top
`shared-sub-dag` is `Value` vs `&Value`, value 179, **params 15**).

## Honest limits

Two repos, two languages (Go, TS/React); single-judge labels (no adversarial refuter pass,
unlike [experiments §BR](experiments.md)); the sample is stratified, so per-cell precision is
sound but surface-level fractions are sample-reweighted, not population estimates (the §1
fractions are the population numbers). The exact `ratio` threshold (0.33) and the rendering
collapse shape should be re-confirmed on a wider repo set before the default flips, per the
§2c *measured-before-flipped* discipline. Artifacts: the audit was run from `/tmp` scratch
(`feats.json`, `labels_*.json`, `analyze.py`); re-runnable from any `--depth 1` clone.
