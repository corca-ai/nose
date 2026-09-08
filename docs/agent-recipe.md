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
nose query <path> witness=exact        # slice: only the exact-behavior families
nose query <path> scope=prod           # slice: production-scope only
nose query <path> group=dir            # facet: by directory, with a count + exemplar
nose query <path> id=<fam> full        # open one family: relation + bounded source comparisons
```

Each result is a pure function of (repo state, command); an unknown field or enum value is a
hard error (so a typo can't read as "no duplication"). Use `--format json` on any query for
the same rows structured. This surface is delivered as the agent's primary path; it is *not*
an MCP server (a Skill is the intended packaging).

Do not start an agent workflow with the CI gate surface. The exploratory default intentionally
combines `syntax`, `semantic`, and `near` so the agent sees copy-paste, exact semantic clones,
and near-duplicates before applying judgment. A CI gate should instead pin the channel and
budget, usually `--mode syntax` with explicit size filters; see [continuous integration](continuous-integration.md#jscpd-style-size-budgets).

## Continue exploration after an edit

Capture the admitted family population with `nose query <path> --save-analysis before.json`,
then repeat with `after.json` after editing. Run `nose query --before before.json --after after.json`
and follow its next commands: change-reason groups → selected changes → `change=<id> full`.
The landing view previews recheck observations first. Follow labeled `actions` or their
identical `next` commands verbatim; they retain the output format. Use
`evidence=recheck` to explore changed or uncertain evidence. For incomplete coverage,
read the per-input saved diagnostics; use a higher-budget action only for an incomplete
candidate search. Full details summarize observed member counts and locations. Retained evidence is a
fact for your review policy, not approval. Ordinary dashboard JSON is a truncated view
and cannot replace a capture. The [analysis comparison contract](region-identity.md#explore-changes-between-saved-analyses)
explains profiles, coverage, artifact addresses and the limits of absence-based conclusions.

## Inputs for the batch / gate path

For non-interactive consumption — a CI gate, a one-shot triage of the whole tree, or feeding
the versioned contract to other tooling — read the JSON directly:

```sh
nose query <path> --format json                    # the ranked triage surface (query-JSON contract)
nose query <path> base=origin/main --format json   # PR-time divergence (the base view)
```

Parse `families[]` in the dashboard or list view (dashboard also keeps
`top_candidates[]` as a compatibility alias). The per-family decision procedure below applies
to normal `nose query` rows and JSON families — they carry the same evidence fields.
The PR-time `base` view emits `items[]`; use the divergent-edit procedure below for those
records instead. Do not scrape human output.

## Per-family decision procedure

Read the fields in this order — each step either decides or narrows:

1. **Surface filter.** Act on `surface == "default"` only;
   `divergence`/`hidden`/`shallow`/`generated`/`declaration`/`debug` are diagnostic surfaces. The
   non-default `surface` value *is* the demotion reason — a decidable classification, not a
   worthiness verdict: `shallow` (the extracted helper would be mostly parameters), `declaration`
   (only import/include/use/re-export spans), `generated` (every location in generated-header
   source or CSS source-plus-compiled/minified build pipeline), and `divergence`/`hidden`
   (divergence-hazard or proof-only diagnostics, too small to extract).
   A default-surface family carries `surface == "default"`.
2. **Generated/vendored.** The `generated` surface already flags generated-header families and CSS
   source-plus-compiled/minified build pipelines; also drop families whose paths are vendored
   (`vendor/`, `.min.`, `*.pb.go`, lockfiles). A *partly* generated family keeps a ranked surface
   unless it is a CSS build pipeline — hand-written logic leaking into ordinary generated output is
   a real finding, so keep it.
3. **Why did it merge — `witness`.**
   - `exact`: a behavioral proof (identical value graphs, literal values included; `value_nodes`
     is *how much* was proven). Treat the members as computing the same thing; the only
     question left is whether merging them couples unrelated concerns.
   - `subdag` (the human report labels this `shared-core`; both are accepted as `witness=`
     filter values): a common heavy anchor (shared sub-DAG core) is proven inside each site — each
     member's `shared_subdag: [start, end]` shows where that computation lives. Same-language
     shared-core families can also carry graded evidence when `spotclass` is requested.
   - `copy-paste`: token-identical run — classic copy-paste; identifiers and literals may still
     vary per copy.
   - `similar`: the fuzzy near channel. Grade it with `spotclass` (next step) before trusting it.
4. **What differs — relation → differences → source.** Open `id=ID full` for the
   selected family's available graded pair and bounded source evidence. `graded_pair`
   identifies the two compared members; `graded` records holes, referent mismatches and
   caveats within its modeled scope. Independently, `source_evidence` reports literal
   alignment and pair diffs with member IDs and absolute source coordinates. Inspect
   missing members, sampling and truncation before extending a conclusion to the family.
   Zero literal overlap does not erase a semantic witness. `params` counts varying anchor
   regions, not proven parameters; `extraction_shape` is a routing hint. An `existing_helper`
   still requires checking visibility, imports and the actual call contract.
5. **Where it lives — `scope`.** `scope_evidence` explains the production/test classification.
   A path or test scope alone establishes neither intentional separation nor refactoring
   value. Use the same core question for all scopes.
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

- **Worthy** → propose a refactor after reviewing concrete source and contracts.
  Use `nose query <path> id=<fam> full`; follow member IDs and source coordinates in
  `source_evidence.diffs`. Derive the proposed signature and benefit yourself; neither
  `params`, `shared`, nor `removable` proves them.
- **Not worthy, recurring** → write a [structured ignore](structured-ignores.md)
  entry (`family_id`, `reason`, `owner`, optional `expires_at`) so the family stops
  resurfacing.
- **Unsure** → leave it; never auto-refactor on a `similar` witness alone.

## PR-time: divergent-edit findings

`nose query <path> base=<ref> --format json` (the `base` view) emits one `items[]`
finding per divergence, each carrying the v2 gate fields: `tier`, `tier_reasons[]`,
`taxonomy_hint`, and `gate.fail_default`, plus legacy `fire_eligible` compatibility
evidence, `witness_kind`, `scope`, per-changed-site `touches_shared`, and the advisory
`semantic_change` projection. `targets[]` names only detector-accepted changed→skipped
pairs; use its stable `target_id`, pair-local `direct_witness`, and target-local
`changed.touches_shared` / `changed.semantic_change` rather than treating every
transitively grouped family member as an action. Keep `changed[]` / `not_updated[]` as
broader review context. Read `variant_evidence` at the same target boundary: strong signals
name exact referent, decorator, async/effect, protocol, or disjoint-platform evidence; name,
path, and version differences are weak hints only. Under v2 this evidence does not override
`gate.fail_default`. For propagation triage over the top findings, judge each target as
should-propagate / intentional-divergence / not-a-clone using the changed member's
diff and the un-updated sibling's body. Most fires are not propagation hazards; the
current checked strict baseline has precision 0.562 ([experiments](experiments.md)
measured the base rates), so treat it as opt-in review-gate evidence, not a
default-on blocking claim. Do not reconstruct the default gate from `fire_eligible`;
only `gate.fail_default=true` blocks in the opt-in enforcing workflow. `review`,
`report-only`, `suppressed`, mixed/test findings, and `lane="new-copy"` are advisory:
inspect their evidence when useful, but do not ask CI to fail on them unless a later
measured policy explicitly promotes a lane. `taxonomy_hint` guides inspection; it does
not declare correctness or harm. Adding `near` to PR-time `base=` is an explicit audit
opt-in, not the default recommendation.

The #847/#854 closeout did not qualify a v3 blocker: no development target had the
required complete caveat-free witness, the blind set stayed sealed, and the cumulative
evidence path missed its runtime budget. Agents should use direct targets, semantic change,
and variant evidence to make review more specific, but must not promote them into a private
hard-block rule. See the [0.20 closeout](divergent-gate-closeout-854.md).

## Validation

The recipe was validated decide-from-JSON-only, then grade: an agent
following this page over a deterministic top-K sample of v5-labeled families
reproduced the human-audited worthy/not-worthy verdicts — see
[experiments §BX](experiments.md) for the run and its agreement numbers.

*See also: [usage › nose query](usage.md#nose-query) · [query JSON](query-json.md) ·
[continuous integration](continuous-integration.md) · [divergent edits](divergent-edits.md) ·
[structured-ignores](structured-ignores.md) · [design](design.md).*

For a large family, follow `member-group=dir` / `lang` / `scope` and the emitted member
filters instead of assuming the first copies represent every occurrence. Read `assessment`
and `scope_evidence` before proposing extraction. During saved-analysis review, use the
named verified-source action with the appropriate historical source directories. A caller
can preserve an explicit judgment with `--write-review` and later inspect `review=recheck`
using `--reviews`; the record binds its original analysis, so retain that capture. See the contract in
[source inspection and decision reuse](region-identity.md#inspect-source-and-carry-caller-decisions-forward) for the applicability rules.
