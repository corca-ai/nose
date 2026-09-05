# Usage

The complete command and flag reference for `nose`. New here? Start with
[getting-started](getting-started.md) — it walks through a first run and how to
read the report. For settings you'd commit to a repo see
[configuration](configuration.md); for CI use see [continuous integration](continuous-integration.md).

Throughout, **IL** is nose's *intermediate language*: every source language is lowered into
one normalized representation, so clones match within and across languages, and unit sizes are
measured in IL tokens (its node count). See [architecture](architecture.md) for the pipeline.

## Install

The quickest install (Homebrew or the install script) is in
[getting-started](getting-started.md#install). Prebuilt binaries for macOS (Apple
Silicon + Intel) and Linux (x86_64 + arm64) are attached to every
[release](https://github.com/corca-ai/nose/releases). To build from source:

```sh
cargo build --release
# binary at ./target/release/nose
```

nose is a single self-contained binary — no runtime, services, or network; point
it at source files. The examples below write `nose`, which works the same as a
from-source `./target/release/nose`.

## Command map

| You want to... | Use |
|---|---|
| Inspect duplication and act on it | `nose query <path> [terms…]` |
| Inspect disjoint roots together | `nose query --root <path> --root <path> [terms…]` |
| Open one family with its bounded source comparison | `nose query <path> id=<fam> full` |
| Catch a missed sibling edit in a diff or PR | `nose query <path> base=<ref>` |
| Gate CI on duplication | `nose query <path> --fail-on any` |
| Inspect or reclaim a query cache | `nose cache status|prune|clear --dir <cache>` |
| Read the result as machine-readable JSON | `nose query <path> --format json` ([query-json](query-json.md)) |
| Refresh a local integration after each save | `nose query <path> --watch --format jsonl` ([query-watch](query-watch.md)) |
| Save a complete admitted code-family analysis | `nose query <path> --save-analysis FILE` |
| Explore changes without the source workspace | `nose query --before BEFORE --after AFTER [terms…]` — [analysis comparison](region-identity.md#explore-changes-between-saved-analyses) |
| Ask what an installed binary supports | `nose capabilities` |
| Check a local semantic-pack manifest | `nose semantic-pack check <file-or-dir>` |
| Create or inspect a content-pinned semantic-pack decision | `nose semantic-pack lock …` / `nose semantic-pack status <lock>` |
| Inspect semantic-pack compatibility policy | `nose semantic-pack compatibility` |
| Inspect lowering coverage for a language | `nose stats <paths...>` |
| Find near-duplicate Markdown **prose** | `nose query <path>` — see [markdown-duplication](markdown-duplication.md) |
| Debug why two snippets do or do not converge | `nose il <file> --normalized` |

`nose query` is the main command; the **Ranking** and **Detection modes** sections below
document the ranking keys and detection channels it uses.

## `nose query`

`nose query <path> [terms…]` analyzes a file or directory and prints the duplication
families nose found. With no terms it shows a summary: analysis scope, family counts by
evidence, the best candidates to inspect first, verified-evidence families, duplicated
directory hotspots, and next commands. Add terms to filter, group, sort, or open one
family; every result includes a runnable `nose query …` command.

`query --help` starts with the filter and navigation grammar. Numeric filter errors
fail before analysis and never become a successful empty result. Dashboard, list, group,
family, helper and watch navigation replay analysis options (`--mode`, exclusions,
thresholds, config, packs, cache and read-only baseline) with shell quoting. Query JSON's
`path` still describes only the roots. Group JSON includes per-row executable `next` links.

If candidate work exceeds its limit, the query fails without partial clone findings. The
error adds a source-file inventory and up to eight directory-root commands. Counts describe
files directly in each directory, not candidate work or the cause of the overload. The
commands retain settings but select a smaller root, which also changes the scope of
root-relative excludes; no mode or exclusion is changed automatically. `scope=` and
`path~` filter completed findings and cannot fix a candidate-budget failure.

Every live dashboard, list, group, family and helper view repeats its effective analysis
population: roots, scanned-file count, modes, exclusion patterns and size/member floors.
Query filters select findings inside that population; they do not change the analyzed files.
JSON exposes the same information in `analysis`, including skipped-source diagnostics.

`--max-candidate-pairs N` explicitly sets the positive candidate-work limit for a query,
including cached/watch queries. It overrides `NOSE_MAX_CANDIDATE_PAIRS`; otherwise the
default remains 16,000,000. Navigation preserves an explicit limit. The budget counts
unique pairs across all detection routes, excluding equal-line-span pairs in one file that
cannot yield ordinary or connected evidence. Larger limits cost time and memory and do
not guarantee completion. A budget failure offers both smaller-root commands and an
explicit higher-limit retry retaining the current roots/modes; neither runs automatically.

The landing page offers production, test, mixed-scope, and discovered evaluation/fixture
directory routes. Directory names are navigation hints, not proof of code purpose; no findings
are hidden by these routes until selected. Production routes exclude the listed evaluation
paths explicitly. Cross-directory relationships remain available through the full population.
Rust `#[test]` and unconditional `#[cfg(test)]` attributes propagate test context into nested
regions, as do existing test-name/module conventions. This classification does not evaluate
arbitrary conditional compilation or third-party test macros. Conventional `_tests` file and
directory names also classify out-of-line test helpers, without pretending to resolve every
custom module graph.

To analyze disjoint trees in one run, pass every root explicitly:
`nose query --root packages --root scripts [terms…]` (short: `-r`). When
`--root`/`-r` is present, bare arguments are query terms; use `-r` for each
analyzed path.

`nose query` carries the analysis flags, the `--fail-on` CI gate, and a structured contract: with
`--format json` every view emits the versioned [query-JSON contract](query-json.md), and
`--format markdown`/`sarif` produce a ranked report.

```text
nose query <path> [FILTER … | group=FIELD | id=FAM | at=FILE:LINE | reinvented | base=REF] [since=FILE] [sort=KEY] [top=N] [full] [all]
nose query --root <path> --root <path> [FILTER … | group=FIELD | id=FAM | at=FILE:LINE | reinvented | base=REF] …
```

| part | meaning |
|---|---|
| `field=value` | keep families where the field equals the value (terms AND-ed); `field>N`/`field<N` for finite numbers (`>=`, `<=`, NaN, infinity and malformed numeric values are errors); `path~substr` for a path substring; **set OR** with a comma — `witness=exact,shared-core` matches either; **negate** with `field!=value` / `path!~substr` (e.g. `path!~frontend` drops a directory; `witness!=exact,shared-core` drops both) |
| `group=FIELD` | facet the selection by a discrete field (`dir`, `file`, `scope`, `witness`, `lang`, `shape`, `same_symbol`, `spotclass`, `status`); each bucket carries its family count **and summed removable lines**, ranked by removable — so `group=dir`/`group=file` is the duplication **hotspot** map |
| `id=FAM` | open one family (any unambiguous id prefix): its copies, a bounded source comparison, overlapping-family links (`subsumes`/`subsumed_by`), and navigation |
| `member-group=dir` / `lang` / `scope` | with `id=` or `at=`, group copies inside the family; follow emitted commands to narrow members while retaining the full family identity and metrics |
| `member-dir=DIR`, `member-path~TEXT`, `member-lang=LANG`, `member-scope=prod` / `test` | with `id=` or `at=`, select member locations; `member-dir` is exact, `member-path` is a substring; `top=N` limits member rows/groups and `full` or `top=0` expands them. With a member filter, `full` also shows bounded selected source bodies with line numbers; grouping shows counts. Human/JSON inspection only. |
| `at=FILE:LINE` | open the family whose copy covers that source location — a stable handle across edits (the span-derived `id=` shifts when code moves) |
| `reinvented` | the **reinvented-helper** view: code that reimplements an existing helper inline (a computation match requires a separate reuse decision). Production findings are shown only when the helper is also production code; matches that point production code at a test-only helper are omitted with a count, because the safe action is to rehome/extract a production helper first. Complements `shape=call-existing-helper` (those are the cases the family clusterer caught; these are the ones it did not) |
| `base=REF` | the **divergent-edit** view: detect families at the git ref, flag the ones a diff changed in one copy but not its siblings — a likely un-propagated fix. It is its own view, so combine it only with `top=N`, detection flags, `--format`, or `--fail-on any`; ordinary family filters are for the non-`base=` query views. Each item carries legacy `fire_eligible` evidence plus the v2 `strict` / `review` / `report-only` / `suppressed` tier contract; `base=REF --fail-on any` fails only on unsuppressed `strict` findings. See [divergent edits](divergent-edits.md). |
| `since=FILE` | compare to a saved snapshot (written with `--baseline FILE --write-baseline`) and expose each family's **`status`** (`new`/`changed`/`unchanged`) as a queryable field — the temporal lens. Hides nothing (unlike `--baseline`); `since=B status!=unchanged --fail-on any` is the composable gate |
| `sort=KEY` | `extractability` (default), `value`, `members` (also `sites` and the experimental `hazard` — see [Ranking](#ranking)) |
| `top=N` | show the first N rows (default 30); `top=0` shows **all** |
| `full` | on `id=` or a list, render bounded source comparisons inline; holes are `⟨region N: class⟩` text observations, not proven parameters. With a member filter, show selected source bodies |
| `all` | widen past the curated default surface to the full raw universe (demoted families labeled) |

Family details explain extraction support separately from the detector witness: measured
shared source, no shared source, common syntax, cross-language comparison, or unavailable
source measurements. Review checks name dependency/ownership, call-contract, effects and
varying-region questions; they do not assert that a helper is safe to extract. Full locations
show their scope evidence (path/name convention, frontend or enclosing test context, or
absence of recognized test evidence). Recognized scope is not a full build-configuration model.

For saved comparisons, use explicit `--before-source DIR` / `--after-source DIR` on
`change=ID` to inspect hash-verified source. Record caller judgments using `--write-review
FILE --decision keep-separate|refactor|defer --reason TEXT`; reload with `--reviews FILE` and
narrow with `review=applicable|recheck|unreviewed`. See [source inspection and review
conditions](region-identity.md#inspect-source-and-carry-caller-decisions-forward).

Fields: `scope` (prod\|test\|mixed), `witness`
(exact\|shared-core\|connected\|bounded-window\|copy-paste\|similar — `shared-core` is spelled
`subdag` in `--format json`; `same-unit` is accepted as an alias for `bounded-window`),
`same_symbol` (true\|false — every copy is the same named symbol, the parallel-variant
signature), `spotclass` (leaf-only\|structural — for enriched near/shared-core
families, whether the witness is clean equal-modulo-value-leaves or demoted by structural,
referent, or async/sync transformation evidence; cross-language families can be enriched when
their value DAGs align but remain non-source-comparable; unenriched/non-near families group as
`unwitnessed` under `group=spotclass`, which is not itself a filterable value), `status`
(new\|changed\|unchanged — vs the `since=` snapshot), `lang`, `path`, `dir`,
`members`, `files`, `value`, `params`, `lines` (mean source-line span), `shared`
(invariant lines), `dup` (duplicated-line volume). Every row shows the payoff economics —
`M/REP shared, Pp · ~N removable` — so a candidate can be triaged without opening it. Each
result is a pure function of (repo state, command); an unknown field or enum value is a hard
error (so a typo can't read as "no duplication").
Quote comparison terms in shell commands (`'dup>80'`, `'lines>25'`) because bare `>` is a
redirection operator.

`spotclass` reads the [graded witness](graded-witness.md), which is presentation-layer
enrichment (the dominant extra analysis cost), so `query` computes it **on demand** — only when a term
filters or groups by `spotclass`. The common query path pays nothing; a `spotclass=` /
`group=spotclass` query re-derives the witness for alignable near/shared-core families first.

A `copy-paste` witness measures matching tokens, while `shared` measures invariant whole
source lines across the bounded member alignment. A matching token run can span parts
of differing lines and therefore have zero shared whole lines. The row explains this case.
Detailed members expose their source-boundary classification and any known enclosing unit.
An unclassified region may cross declarations; a detected region is not an extraction plan.

A typical loop: `nose query .` → `nose query . witness=exact` → `nose query . id=<id> full`.

`nose query` accepts the same **analysis flags** as the detection pipeline — `--mode`
(see [Detection modes](#detection-modes)), `--min-size`, `--min-value`, `--min-members`, `--exclude`,
`--cache-dir`, `--cache-max-bytes`, `--ignore-file`, `--semantic-pack`, `--config` — so the dataset it explores is
configured by flag while scope/sort/top are the DSL's `scope=`/`sort=`/`top=`. It also takes the
**CI gate** — `--fail-on any` / `--fail-on new` with `--baseline`/`--write-baseline` — and
drops structured-ignored families. The gate follows the untruncated family selection addressed
by the query terms (`top=N` only limits display), so `nose query <path> path~api --fail-on any`
fails only on reportable `path~api` families. See [continuous-integration](continuous-integration.md).
For jscpd-style copy-paste gates, pin `--mode syntax` and add size terms such as
`'lines>25'`, `'shared>20'`, or `'dup>80'`; see [continuous integration › jscpd-style size budgets](continuous-integration.md#jscpd-style-size-budgets).
The `base=` view is the exception: it reuses only detection flags (`--mode`, `--min-size`,
advanced `--min-lines`, `--exclude`, `--config`), `--ignore-file`, `--format`, `top=N`, and
`--fail-on any`; report-shaping and baseline flags are rejected instead of ignored.

For a foreground live session, use
`nose query <path> --watch --format jsonl`. It emits an initial complete dashboard
snapshot and then one revision per reconciled change. Watch v1 accepts analysis
flags but no query terms, baselines, or CI gates; see [query watch](query-watch.md)
for its schema, recovery behavior, and integration contract.

A named path that doesn't exist is an error (exit non-zero) — a typo'd path in a
CI gate must fail loudly. A path that exists but contains no supported source
files warns on stderr and reports an empty result. This holds for `nose query` and
`nose stats`.

How to read the resulting dashboard — the scope line, the confidence breakdown,
the per-family economics, scope tags, and the `→` hint — is covered in [getting-started → How to read the report](getting-started.md#how-to-read-the-report).

### The default surface

The dashboard, the other formats, and the `--fail-on` gate show a curated **default
surface** of action-oriented families, and hold the rest back behind a one-line omission
footer (`omitted N families …`). Held back are tiny proof-only fragments,
fragments, generated/distributed-output families (including generated-code headers,
self-declared generated HTML documents, and CSS source-plus-compiled/minified build
pipelines), **declaration runs**
(spans that are only import/include/use/re-export lines — duplication the language mandates
per file, with nothing to extract), and **shallow-extraction** families (a helper that would
be mostly parameters — `params` ≥ a third of the shared lines). Add `all` to widen the view
to the full raw universe, each demoted family labeled with why it was held back. This curated
surface is what `--fail-on` gates on — a family in the `all` view alone never fails a run.
Markdown prose findings are displayed as a separate dashboard domain and dashboard-JSON
`markdown[]` field; they are not code-clone `families[]` and do not trip `--fail-on`.

## Ranking

`sort=` chooses what "most worth your attention first" means. Changing sort order preserves the selected family IDs,
counts and overlap-fold relationships. Representatives are chosen using one canonical
extractability order before applying a display sort, including configured and watch sorts.
`--min-value` is a noise floor on raw value and applies under every sort. `nose query`
accepts `extractability`, `value`, `sites`, the alias `members`, and the experimental `hazard`.
(The query dashboard's
`sort` cheatsheet advertises only the common three — `extractability`, `value`, `members`.)

| key | ranks by | use when |
|---|---|---|
| `extractability` *(default)* | invariant (shared) lines × copies × spread, weighted by tightness (shared/total) and penalized by parameter count and by member-span heterogeneity (copies of unlike length aren't one shape). Cross-language families have no comparable source lines, so they fall back to semantic repeated volume and display as `cross-language · ~N repeated` instead of `~N removable`. | prioritize inspection by shared-line density; reuse feasibility remains a caller decision |
| `value` | raw duplicated volume: duplicated lines (mean span × copies) × similarity × spread — ranks by repeated *volume*, not the `removable` field (a structural Type-4 family can rank high here yet show `removable=0` when no literal lines survive all copies) | inspect the largest repeated volume; this does not predict deletions |
| `sites` / `members` | number of copies | hunting the most-repeated patterns |
| `hazard` *(experimental)* | divergent-edit *propensity*: line span × spread × invisibility × scope | you want a view of which clones tend to get edited inconsistently — **not yet a validated *harm* ranker** (see [hazard-ranking](hazard-ranking.md)) |

Extractability is a deterministic inspection-cost heuristic. It weights literal overlap,
spread, differing regions and member-span heterogeneity; it does not establish that a
helper is callable, desirable or safe. The displayed counts use a bounded sample;
`id=ID full` reports actual source coverage and pair-local differences. A family with no
literal shared lines can still have a strong semantic witness. Cross-language families
use repeated volume as a fallback (`source_comparable: false`), not a language-based
judgment about whether their projects can share code. Test scope does not impose a
worthiness penalty. Ranking changes require separate evaluation against labeled data.


`sort=hazard` is an **experimental** severity-style ranking calibrated on mined
divergent-edit history. It predicts *which clones get edited inconsistently* (divergence
propensity) but, per a gold-label audit, **does not yet rank actual *harm* better than
chance** — see [hazard-ranking](hazard-ranking.md) for the full, honest evaluation.

## Detection modes

nose's detection has three stable orthogonal channels, selected with `--mode` on
`nose query`. Omitting `--mode` runs all three:
`syntax,semantic,near`. When `--mode` is present, nose runs exactly the channels you list;
it does not add them to the default. See [clone-types](clone-types.md) for what each finds
against the standard Type-1–4 taxonomy.

| mode | clone type | detector channel | use when |
|---|---|---|---|
| `syntax` | Type-1 / exact token-run floor | same-language jscpd-style contiguous copy-paste runs | replacing jscpd / blocking copy-paste clones in CI |
| `semantic` | Type-4 | exact value-fingerprint matches | high-confidence semantic clones with no fuzzy threshold |
| `near` | Type-3 | shape candidates + fuzzy structural/value scoring | finding near-duplicates for triage |

Examples:

```sh
nose query src                                  # syntax + semantic + near (the default)
nose query src --mode syntax --min-size 80 'dup>80' --fail-on any  # jscpd-style gate
nose query src --mode semantic                  # exact Type-4 only
nose query src --mode near:0.70                 # Type-3 only
nose query src --mode syntax,semantic           # exact channels only, no fuzzy near
nose query src --mode syntax --mode semantic    # same as --mode syntax,semantic
```

The `near` channel takes its acceptance threshold inline — `--mode near:0.8` (default
`0.70`). `syntax` and `semantic` are exact channels and take no threshold.

There is also a hidden experimental `abstraction[:T]` mode. It reuses the `near`
candidate stream, defaults to threshold `0.50`, and then keeps only same-language
families whose normalized IL differs by exactly one supported literal leaf. Its
claim is weaker than `semantic`: it reports a refactoring-template witness with a
typed hole and caveats such as `numeric-domain-sensitive`; it does not say the two
copies are behavior-equivalent. Emit all families as JSON (`nose query … top=0 --format json`)
to consume the `abstraction_witness` field.
The witness is built from normalized IL, not line-level `--show proposal` text; its
template is machine-oriented today and carries typed hole metadata for tooling. A
reported abstraction family must share one literal-leaf hole position across all
members; mixed families with different changed positions are left unwitnessed.
When `near:T` and `abstraction:T` are combined in one run, they share one fuzzy
acceptance threshold; giving different inline values is rejected instead of silently
choosing the last one.

The `syntax` channel is the CPD floor: it finds same-language duplicated token runs even
when they start or end in the middle of a function. The normalized unit channels
(`semantic` and `near`) are where renamed identifiers, literal-varied Type-2 cases,
cross-language convergence, and Type-3 edits are handled.

`semantic` is still unit-bounded rather than an arbitrary-fragment equivalence search.
Besides whole functions/classes/blocks, it admits only exact-safe sub-function fragments
whose inputs, exits, and ordered effects are self-contained: direct return/throw values,
bounded conditional guards, selected for-each append/index effects, and fixed-`this` Java
field writes. General statement windows, dynamic receiver writes, mixed unproven effects,
and arbitrary field/property mutation stay closed. The exact fragment contract is
documented in [fragment-contracts](fragment-contracts.md).

### Flags

Grouped by what they do. Config-backed defaults are listed in
[configuration](configuration.md); workflow and output flags stay CLI-only. The ranking and
mode flags are documented under [Ranking](#ranking) and [Detection modes](#detection-modes) above.

**Filter & shape the report**

| flag | effect |
|---|---|
| `--min-members N` | only families with at least N duplicated sites (default 2) |
| `--min-value V` | hide families below this finite non-negative refactoring value (noise floor on large repos) |
| `--min-size N` | ignore units or syntax copy-paste runs smaller than this size, in IL tokens (default 24) |
| `--mode MODE` | one or more of `syntax`, `semantic`, `near[:T]`; comma-list or repeatable; when present, replaces the default. Experimental `abstraction[:T]` is accepted but not a stable capabilities mode. |
| `--root <path>` / `-r <path>` | analyze another root; repeat for multi-root query runs. With `--root`, bare positional arguments are query terms. |
| `--exclude <glob>` | skip paths matching a gitignore-syntax glob (repeatable) |
| `--generated-path <glob>` | assert that files matching this root-anchored glob are generated; repeatable and recoverable with `all top=0` |
| `--cache-dir <dir>` | reuse portable, checksummed analysis artifacts and transactional incremental state; exact no-op and independently provable one-leaf updates avoid restoring the whole IL corpus |
| `--cache-max-bytes <SIZE>` | bound the managed cache (default `5GiB`; accepts bytes or KiB/MiB/GiB/TiB) |
| `--ignore-file <file>` | suppress accepted families using a structured ignore file with reason/owner/expiry metadata |
| `--semantic-pack <file-or-dir>` | load local semantic-pack v0 metadata or compile a typed v1 manifest for provenance/digest reporting; unlocked external packs are metadata-only |
| `--semantic-pack-lock <file>` | validate one content-pinned typed v1 project lock before analysis; eligible rows may support near candidates or receipt-backed collection-factory exact claims, and unlocked pack paths cannot be mixed in |

**Output**

| flag | effect |
|---|---|
| `--format human\|json\|jsonl\|markdown\|sarif` | output format (default `human`); `jsonl` is paired with `--watch` |
| `--watch` | keep a foreground query session alive and emit `nose.query-watch/v1` JSONL snapshots |

Use terms for report shaping: `sort=KEY`, `top=N`, `scope=prod|test|mixed`, `group=dir`,
`id=<fam> full`, and `reinvented`. `--format json` emits the stable
[query-json](query-json.md) contract.

**Workflow** (`--baseline`, `--write-baseline`, `--fail-on any|new`, `--ignore-file`, `--cache-dir`, `--cache-max-bytes`, `--config`, `--generated-path`, `--semantic-pack`, `--semantic-pack-lock`) is covered in
[continuous integration](continuous-integration.md), [query cache](query-cache.md),
[configuration](configuration.md), and the [semantic-pack overview](semantic-packs.md).
Structured suppressions are covered in [structured-ignores](structured-ignores.md).

## Integrating with nose

Use `nose capabilities` when another tool needs to decide what this installed
binary supports before it invokes a query:

```sh
nose capabilities
```

The command emits JSON only. It reports the tool version, platform, stable
commands, supported detection modes and output formats such as SARIF, JSON schema
versions, config keys, and query capability flags such as baselines, caching,
and structured ignores. Do not scrape `nose --help`; help text is for humans
and may change to improve readability. The stable contract is documented in
[capabilities](capabilities.md); the structured result JSON is in [query-json](query-json.md).

## Other commands

- `nose cache status --dir <dir> [--max-bytes 5GiB] [--format human|json]` — report managed
  bytes/files, live and historical generations, and immediately reclaimable storage.
- `nose cache prune --dir <dir> [--max-bytes 5GiB] [--format human|json]` — remove old schemas,
  orphaned generations, and the oldest entries above the budget. Active queries finish before
  pruning starts.
- `nose cache clear --dir <dir> [--format human|json]` — remove only nose-managed cache data;
  unrelated files under the directory are preserved. All three JSON forms carry the schemas
  advertised by [`nose capabilities`](capabilities.md). Start with the
  [query cache guide](query-cache.md) for the everyday workflow.
- `nose query <paths> base=<ref>` — flags clones changed inconsistently in a diff
  (a copy edited, its siblings missed); use `base=HEAD` for uncommitted changes and
  `base=origin/main --mode syntax,semantic --fail-on any` for a PR gate; see
  [divergent edits](divergent-edits.md) for the full guide.
- `nose stats <paths…> [--top N] [--format human|json]` — per-language IL lowering coverage (the
  Raw-node ratio, split into by-design protocol boundaries vs genuine lowering gaps), with
  the top unhandled surface kinds (`--top`, default 30; `--format json` for machine output —
  the same `--format` contract as `query`/`il`).
  Use it to spot a language/construct that isn't lowering well; see [languages](languages.md).
- `nose semantic-pack check <file-or-dir> [--format human|json] [--receipt-out <file>]` — validate local
  semantic-pack v0 structure/fixtures or compile and source-check typed v1
  fixtures. `--receipt-out` writes a content-bound receipt only after a passing
  single-pack v1 run; it is a pack-author/user workflow, not external pack
  certification. See [semantic-pack-conformance](semantic-pack-conformance.md).
- `nose semantic-pack lock <file-or-dir> --dependency <file> [--channel near|external-exact] [--row ID] [--output <file>] [--format human|json]` — create a local content-pinned v1 project lock without fetching or installing anything.
- `nose semantic-pack status <lock> [--format human|json]` — revalidate pinned manifests, dependency inputs, selected rows/channels, optional receipts, path containment, and conflicts. See [semantic-pack-project-lock](semantic-pack-project-lock.md).
- `nose semantic-pack inventory [--format human|json]` — inspect compiled
  builtin pack declarations, conformance refs, exact-capable coverage status,
  and coverage gaps without running a query.
- `nose semantic-pack adoption-gates [--format human|json]` — inspect compiled
  builtin pack optional/default promotion gates, rollback actions, and blocker
  status without running a query.
- `nose semantic-pack compatibility [--format human|json]` — inspect manifest
  API, installed-version, kernel-vocabulary, and external-influence
  compatibility policy without loading external manifests or running a query.
- `nose il <file> [--normalized] [--no-cfg-norm] [--format sexpr|json]` — dump the IL
  for one file (`--normalized` shows the canonical form after the
  [normalization](normalization.md) passes). A debugging tool for understanding why two
  snippets do or don't converge; see [architecture](architecture.md).

Hidden `behavioral-gate`, `detect`, `verify`, `features`, `eval`, and `ceiling` commands
exist for strict/research workflows — an experimental Type-4 benchmark harness and oracle
tooling, not stable integration surfaces; do not build automation around them without
checking the current binary. `verify --recall-loss-report <file>` writes the local
[recall-loss diagnostics](recall-loss-diagnostics.md) artifact used by
semantic-kernel PRs to attribute fail-closed recall loss while preserving the
hard soundness gate; the checked-in baseline and diff workflow are in
[recall-loss-recovery-loop](recall-loss-recovery-loop.md). They are hidden from
`--help` because `query` is the command for everyday use; the
[benchmark](benchmark.md) page documents them.

## nose regions

`nose regions snapshot <path>` emits a JSON census of admitted source regions,
including singletons. `nose regions compare <before.json> <after.json>
[--max-candidates N]` compares explicit snapshots with a bounded candidate search.
The default candidate budget is 100,000. Results distinguish content matches,
modification/copy candidates, ambiguity, and unavailable correspondence. They do
not modify source or approve reviews. See [region identity](region-identity.md).
