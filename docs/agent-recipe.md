# Agent recipe — exploring & triaging nose findings

[design](design.md) §2: nose's primary consumer is an LLM coding agent that **calls
nose and applies its own judgment on top**. nose surfaces candidates with
deterministic, machine-readable evidence; the judgment-deep question — *worth
refactoring, or parallel-by-design?* — belongs in the caller ([experiments](experiments.md)
measured that ceiling; an internal LLM would be redundant for agents and harmful for
gates). This page is the protocol for that caller: how to **explore** the findings, which
fields to read, in what order, and what to do with each verdict. It was validated by
replaying it against the human-audited v5 labels (see *Validation* below).

## Explore: the `nose query` loop (start here)

`nose query <path>` is the interactive entry point — a stateless, self-describing surface
over the same family dataset, built for an agent loop. Start with no terms for a landing
dashboard, then **follow the runnable `next:` command on each result** rather than
pre-scripting field reads:

```sh
nose query <path>                      # landing dashboard: counts by confidence + cleanest candidates
nose query <path> witness=exact        # slice: only the behavior-proven families
nose query <path> scope=prod           # slice: production-scope only
nose query <path> group=dir            # facet: by directory, with a count + exemplar
nose query <path> id=<fam> full        # open one family: copies + all-copies extraction skeleton
```

Each result is a pure function of (repo state, command); an unknown field or enum value is a
hard error (so a typo can't read as "no duplication"). Use `--format json` on any query for
the same rows structured. This surface is delivered as the agent's primary path; it is *not*
an MCP server (a Skill is the intended packaging).

## Inputs for the batch / gate path

For non-interactive consumption — a CI gate, a one-shot triage of the whole tree, or feeding
the versioned contract to other tooling — read the JSON directly:

```sh
nose query <path> --format json                    # the ranked triage surface (query-JSON v2 contract)
nose query <path> base=origin/main --format json   # PR-time divergence (the base view)
```

The deprecated equivalents are `nose scan <path> --format json` and `nose review --base
origin/main --format json`. Parse the family objects — `top_candidates[]` in the dashboard
view, `families[]` in a list view (a filter/`sort=`/`top=` term), `items[]` in the `base` view
([query JSON](query-json.md)). Do not scrape human output. The per-family decision procedure
below applies to both a `nose query` row and a JSON family — they carry the same evidence fields.

## Per-family decision procedure

Read the fields in this order — each step either decides or narrows:

1. **Surface filter.** Act on `surface == "default"` only;
   `review`/`hidden`/`shallow`/`generated`/`declaration` are diagnostic surfaces. The
   non-default `surface` value *is* the demotion reason — a decidable classification, not a
   worthiness verdict: `shallow` (the extracted helper would be mostly parameters), `declaration`
   (only import/include/use/re-export spans), `generated` (every location in generated-header
   source), and `review`/`hidden` (review-hazard or proof-only diagnostics, too small to extract).
   A default-surface family carries `surface == "default"`.
2. **Generated/vendored.** The `generated` surface already flags families whose every location
   sits in generated-header source; also drop families whose paths are vendored (`vendor/`,
   `.min.`, `*.pb.go`, lockfiles). A *partly* generated family keeps a ranked surface — it is a
   real leak, so keep it.
3. **Why did it merge — `witness`.**
   - `exact`: a behavioral proof (identical value graphs, literal values included; `value_nodes`
     is *how much* was proven). Treat the members as computing the same thing; the only
     question left is whether merging them couples unrelated concerns.
   - `subdag` (the human report labels this `shared-core`; both are accepted as `witness=`
     filter values): a common heavy anchor (shared sub-DAG core) is behavior-proven — each
     member's `shared_subdag: [start, end]` shows where that computation lives.
   - `copy-paste`: token-identical run — classic copy-paste; identifiers and literals may still
     vary per copy.
   - `similar`: the fuzzy near channel. Grade it with `spotclass` (next step) before trusting it.
4. **What differs — `params` + `shared` + `spotclass`.** `params` counts the varying spots the
   extracted helper would parameterize; with `full`, `skeleton` renders each as a
   `⟨param N: class⟩` placeholder (`class` = literal/name/call/expr/block). An all-literal
   placeholder list over near-identical lines is a data table (a consolidate-into-a-table or
   not-worthy locale/i18n parallel-data case — check whether the literals are *content* or
   *parameters*). Many
   `params` relative to `shared` (the lines invariant across **all** copies) means a costly, ugly
   extraction. For a near (`similar`) family, `spotclass` says whether those spots are `leaf-only`
   (clean value-leaves to parameterize — interesting) or `structural` (a shape/arity/referent
   divergence — genuine logic difference, be skeptical). `extraction_shape` names the decidable
   shape of the fix for a clean candidate (`call-existing-helper` is the strongest — an existing
   helper is reinvented inline, so the action is to *call* it, not extract anew).
5. **Where it lives — `scope`.** `scope` is `prod` / `test` / `mixed` (a Rust inline `mod tests`
   counts as test scope even in a production file). Test-scaffolding duplication is still worthy
   (a test helper is the refactor) — but weigh it below production logic when budgeting attention.
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

- **Worthy** → propose the refactor. The `params` are the helper's parameters;
  `nose query <path> id=<fam> full` (or `full` on a list) renders the all-copies extraction
  `skeleton`; `shared` is the helper body size. Reference locations by `file:start` (each
  `locations[]` entry is `{file, start, end, name, lang}`).
- **Not worthy, recurring** → write a [structured ignore](structured-ignores.md)
  entry (`family_id`, `reason`, `owner`, optional `expires_at`) so the family stops
  resurfacing.
- **Unsure** → leave it; never auto-refactor on a `similar` witness alone.

## PR-time: divergent-edit findings

`nose query <path> base=<ref> --format json` (the `base` view — `nose review --format json` is
the deprecated alias) emits one `items[]` finding per divergence, each carrying the gate
fields: `fire_eligible` (the conservative shared-logic policy verdict the gate fires on),
`witness_kind`, `scope`, and per-changed-site `touches_shared`. For a harm pass over the top
findings, judge each as
should-propagate / intentional-divergence / not-a-clone using the changed member's
diff and the un-updated sibling's body. Most fires are not propagation hazards; the
gate's `fire_eligible` tier is the high-precision slice ([experiments](experiments.md)
measured the base rates).

## Validation

The recipe was validated decide-from-JSON-only, then grade: an agent
following this page over a deterministic top-K sample of v5-labeled families
reproduced the human-audited worthy/not-worthy verdicts — see
[experiments §BX](experiments.md) for the run and its agreement numbers.

*See also: [usage › nose query](usage.md#nose-query) · [query JSON](query-json.md) ·
[scan JSON](scan-json.md) · [review](review.md) · [structured-ignores](structured-ignores.md) ·
[design](design.md).*
