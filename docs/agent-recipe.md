# Agent recipe — triaging nose scan JSON

[design](design.md) §2: nose's primary consumer is an LLM coding agent that **calls
nose and applies its own judgment on top**. nose surfaces candidates with
deterministic, machine-readable evidence; the judgment-deep question — *worth
refactoring, or parallel-by-design?* — belongs in the caller (experiments §AV/§AY
measured that ceiling; an internal LLM would be redundant for agents and harmful for
gates). This page is the protocol for that caller: which fields to read, in what
order, and what to do with each verdict. It was validated by replaying it against
the human-audited v5 labels (see *Validation* below).

## Inputs

```sh
nose scan <path> --format json --top 30        # the ranked triage surface
nose review --base origin/main --format json   # PR-time divergence findings
```

Parse `families[]` ([scan JSON](scan-json.md)). Do not scrape human output.

## Per-family decision procedure

Read the fields in this order — each step either decides or narrows:

1. **Surface filter.** Act on `recommended_surface == "default"` only;
   `review`/`hidden` are diagnostic surfaces, `generated` means every member sits
   in generated output, and `declaration` means every member span is only
   import/include/use/re-export declarations (real duplication, but the language
   mandates it per file — there is nothing to extract).
2. **Generated/vendored.** Drop the family if every location has
   `looks_generated: true` or the paths are vendored (`vendor/`, `.min.`,
   `*.pb.go`, lockfiles). A *partly* generated family is a real leak — keep it.
3. **Why did it merge — `witness.kind`.**
   - `exact-value-graph`: a behavioral proof (identical value graphs, literal
     values included). Treat the members as computing the same thing; the only
     question left is whether merging them couples unrelated concerns.
   - `copy-paste-run`: token-identical run — classic copy-paste; identifiers and
     literals may still vary per copy.
   - `structural-similarity`: fuzzy. Read `mean_value_jaccard` vs
     `mean_shape_jaccard`: high value + low shape = behaviorally convergent
     (interesting); high shape + low value = surface likeness (skeptical).
4. **What differs — `varying_spots` + `params` + `shared_lines`.** An all-literal
   spot list over near-identical lines is a data table (`extract-data-table` or
   not-worthy locale/i18n parallel data — check whether the literals are *content*
   or *parameters*). Many spots (`params` high) relative to `shared_lines` means a
   costly, ugly extraction.
5. **Where it lives — `scope` + `in_test_module`.** Test-scaffolding duplication is
   still worthy (a test helper is the refactor) — but weigh it below production
   logic when budgeting attention.
6. **The core question** (the same rubric the v5 labels use,
   [bench/labels/RUBRIC.md](../bench/labels/RUBRIC.md)): *would extracting one
   shared abstraction reduce duplication without coupling unrelated concerns or
   leaking per-variant quirks?* The not-worthy classes to name explicitly:
   `parallel-by-design` (per-backend/per-grammar variants), `coincidental-shape`,
   `type-def`, `generated`, `trivial`.

   Two calibrations the first validation round measured agents getting wrong
   (both under-calls — see [experiments §BX](experiments.md)):

   - **Location never excuses duplication.** Code under `examples/`, `tests/`,
     fixtures, or demo directories is judged by the same core question; "they're
     meant to be standalone" does not auto-make it `parallel-by-design`. Forty
     copies of the same 5-line handler in `example/` is a worthy extract.
   - **`parallel-by-design` requires the variants' LOGIC to differ by design.**
     Many per-variant siblings whose bodies are near-identical — the only spots
     being a covariant return type, a class name, or a constant — are the
     textbook `extract-base`/`parameterize` case, *not* parallel-by-design.
     Parallel-by-design is for variants that genuinely encode different rules
     behind a shared skeleton.

## Acting on a verdict

- **Worthy** → propose the refactor. `varying_spots` are the helper's parameters;
  `--show proposal` renders an extraction skeleton; `shared_lines` is the helper
  body size. Reference locations by `file:start_line`.
- **Not worthy, recurring** → write a [structured ignore](structured-ignores.md)
  entry (`family_id`, `reason`, `owner`, optional `expires`) so the family stops
  resurfacing.
- **Unsure** → leave it; never auto-refactor on `structural-similarity` alone.

## PR-time: review findings

`nose review --format json` findings carry the #245 gate fields: `fire_eligible`
(the conservative policy verdict), `witness_kind`, `scope`, and per-changed-site
`touches_shared`. For a harm pass over the top findings, judge each as
should-propagate / intentional-divergence / not-a-clone using the changed member's
diff and the un-updated sibling's body — the §BG-gold method; experiments §BR/§BV
measured the base rates (most fires are not propagation hazards; the gate tier is
the high-precision slice).

## Validation

The recipe was validated the #227 way (decide from JSON only, then grade): an agent
following this page over a deterministic top-K sample of v5-labeled families
reproduced the human-audited worthy/not-worthy verdicts — see
[experiments §BX](experiments.md) for the run and its agreement numbers.

*See also: [scan JSON](scan-json.md) · [review](review.md) ·
[structured-ignores](structured-ignores.md) · [design](design.md).*
