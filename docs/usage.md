# Usage

The complete command and flag reference for `nose`. New here? Start with
[getting-started](getting-started.md) — it walks through a first scan and how to
read the report. For settings you'd commit to a repo see
[configuration](configuration.md); for CI use see
[continuous integration](continuous-integration.md).

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
| Find refactoring candidates in a tree | `nose scan <paths...>` |
| Explore the duplication interactively (agent loop) | `nose query <path> [terms...]` |
| Catch a missed sibling edit in a diff or PR | `nose review --base <ref>` |
| Ask what an installed binary supports | `nose capabilities` |
| Check a local semantic-pack manifest | `nose semantic-pack check <file-or-dir>` |
| Inspect lowering coverage for a language | `nose stats <paths...>` |
| Debug why two snippets do or do not converge | `nose il <file> --normalized` |

## `nose scan`

`nose scan <paths…>` scans one or more files/directories (recursively, respecting
`.gitignore` files inside each scanned tree), groups duplicated code into **families**, and
ranks them by **extractability** — how cleanly each family folds into one shared helper — so the
duplication you can actually act on surfaces first. With no `--mode`, it runs
`syntax,semantic,near`: CPD-style syntax runs, exact semantic Type-4 clones, and
fuzzy Type-3 near-duplicates.

```sh
nose scan path/to/project
```

How to read the resulting report — the `scanned …` scope line, the per-family
breakdown, scope tags, and the `→` hint — is covered in
[getting-started → How to read the report](getting-started.md#how-to-read-the-report).
The default human, Markdown, SARIF, and `--fail-on` surfaces show only
action-oriented findings. Tiny proof-only fragments, review-only fragments,
families wholly inside files with generated-code headers, declaration runs
(spans that are only import/include/use/re-export lines — duplication the
language mandates per file, with nothing to extract), and `shallow-extraction`
families (unproven matches whose helper would be mostly parameters — `params` ≥ a
third of the shared lines) are omitted with a short count line; `--format json
--top 0` keeps the post-ranking diagnostic families that survive rank-time pruning.

Within the human report, **production findings lead and test-scope duplication is
ranked beneath**, behind a `── N test-scope families …` separator (or a `+N more`
footer when `--top` cuts them). Test duplication is still a real smell — nothing is
dropped: the families stay ranked, in `--format json`, and one `--scope test` away.
`--scope prod` hides them entirely; `--scope test` focuses on them.

A named path that doesn't exist is an error (exit non-zero) — a typo'd path in a
CI gate must fail loudly. A path that exists but contains no supported source
files warns on stderr and reports an empty scan. The same rule applies to
`nose review` and `nose stats`.

Families whose members are overlapping slices of the same source regions are
one refactoring *opportunity*: the best-ranked family keeps its numbered entry
and the others fold into a `↳ N overlapping slice families…` note under it
(`--format json` keeps every family, with `overlap_primary_id` marking the
slices). Each entry also names its equivalence evidence — `exact behavior
match` (value-graph proof), `shared core computation`, `copy-paste`, or
`near-duplicate` — so a *shared decision* reads differently from a *shared
shape*.

### Flags

Grouped by what they do. Config-backed scan defaults are listed in
[configuration](configuration.md); workflow and output flags stay CLI-only.

**Filter & shape the report**

| flag | effect |
|---|---|
| `--sort KEY` | ranking: `extractability` (default), `value`, `sites`, or `hazard` (experimental; see [Ranking](#ranking)) |
| `--top N` | show only the top N families (default 30; `--top 0` = all) |
| `--min-members N` | only families with at least N duplicated sites (default 2) |
| `--min-value V` | hide families below this finite non-negative refactoring value (noise floor on large repos) |
| `--min-size N` | ignore units or syntax copy-paste runs smaller than this size, in IL tokens (default 24) |
| `--scope prod\|test\|all` | keep one side of the test boundary: `prod` drops all-test families (test↔prod leaks stay), `test` keeps only them (default `all`). Applies to every output format and `--fail-on` |
| `--mode MODE` | one or more of `syntax`, `semantic`, `near[:T]`; comma-list or repeatable; when present, replaces the default. Experimental `abstraction[:T]` is accepted but not a stable capabilities mode. |
| `--exclude <glob>` | skip paths matching a gitignore-syntax glob (repeatable) |
| `--ignore-file <file>` | suppress reviewed families using a structured ignore file with reason/owner/expiry metadata |
| `--semantic-pack <file-or-dir>` | explicitly load local semantic-pack v0 manifest metadata for provenance reporting; external packs are metadata-only today |

### Ranking

`--sort` chooses what "most worth your attention first" means. `--min-value` is a
noise floor on raw value and applies under every sort.

| key | ranks by | use when |
|---|---|---|
| `extractability` *(default)* | invariant (shared) lines × copies × spread, weighted by tightness (shared/total) and penalized by parameter count and by member-span heterogeneity (copies of unlike length aren't one shape) | you want the duplication that folds *cleanly* into one helper — not the biggest block that merely looks similar |
| `value` | raw duplicated volume: removable lines × similarity × spread | you want the most *code* deleted, accepting that divergent copies cost more to merge |
| `sites` | number of copies | hunting the most-repeated patterns |
| `hazard` *(experimental)* | divergent-edit *propensity*: line span × spread × invisibility × scope | you want a view of which clones tend to get edited inconsistently — **not yet a validated *harm* ranker** (see [hazard-ranking](hazard-ranking.md)) |

Extractability is the default because raw volume over-rewards a large block whose
copies share little: a 384-line family that shares only 22 lines across 14 varying
spots is mostly scaffolding (6% invariant), not an extraction — it ranks far below a
tight `15/15`-shared, zero-parameter pair. The honest `N/M shared · Pp` cell in the
report is the same signal the ranking uses. Same-language families with **no** shared
invariant lines (a language idiom, or two unrelated type literals with the same shape)
have nothing to extract and sink to the bottom, even at `sim 1.00`. Extractability also
demotes families whose copies **vary widely in length**: 25 same-shaped-but-different
`Serializer` methods are not one helper waiting to happen, however many copies there
are — a measured proxy for signature heterogeneity (experiments §CM).

`--sort hazard` is an **experimental** severity-style ranking calibrated on mined
divergent-edit history. It predicts *which clones get edited inconsistently* (divergence
propensity) but, per a gold-label audit, **does not yet rank actual *harm* better than
chance** — see [hazard-ranking](hazard-ranking.md) for the full, honest evaluation.

**Review what was found**

| flag | effect |
|---|---|
| `--show diff` | show each family inline as a unified diff between its two representative copies — both versions and exactly what differs |
| `--show proposal` | show an extraction skeleton per family — the structure shared across **all** the family's copies, with the differing parts marked as parameters (so the helper is safe to apply to every copy, not just a representative pair) |
| `--show hotspots` | after the report, rank directories by total duplicated lines (architecture view) |
| `--show reinvented` | list **every** [reinvented-helper](reinvented-helpers.md) containment finding (code that reimplements an existing pure helper inline), including the test-container ones the default report excludes — the bare default already lists the non-test findings |
| `--format human\|json\|markdown\|sarif` | output format (default `human`) |

`--format json` emits a versioned object with `schema_version`, `tool_version`,
scan scope, ranking metadata, and a `families` array. The stable contract and
compatibility rule are documented in [scan-json](scan-json.md).

**Workflow** (`--baseline`, `--write-baseline`, `--fail-on any|new`, `--ignore-file`, `--cache-dir`, `--config`, `--semantic-pack`) is covered in
[continuous-integration](continuous-integration.md), [configuration](configuration.md), and
[semantic-pack-loading](semantic-pack-loading.md).
Structured suppressions are covered in [structured-ignores](structured-ignores.md).

### Scan modes

`nose scan` has three stable orthogonal channels. Omitting `--mode` runs all
three: `syntax,semantic,near`. When `--mode` is present, nose runs exactly the
channels you list; it does not add them to the default. See
[clone-types](clone-types.md) for what each finds against the standard Type-1–4
taxonomy.

| mode | clone type | detector channel | use when |
|---|---|---|---|
| `syntax` | Type-1 / exact token-run floor | same-language jscpd-style contiguous copy-paste runs | replacing jscpd / blocking copy-paste clones in CI |
| `semantic` | Type-4 | exact value-fingerprint matches | high-confidence semantic clones with no fuzzy threshold |
| `near` | Type-3 | shape candidates + fuzzy structural/value scoring | finding near-duplicates for review |

Examples:

```sh
nose scan src                                  # syntax + semantic + near (the default)
nose scan src --mode syntax --fail-on any             # jscpd-style gate
nose scan src --mode semantic                  # exact Type-4 only
nose scan src --mode near:0.70                 # Type-3 only
nose scan src --mode syntax,semantic           # exact channels only, no fuzzy near
nose scan src --mode syntax --mode semantic    # same as --mode syntax,semantic
```

The `near` channel takes its acceptance threshold inline — `--mode near:0.8` (default
`0.70`). `syntax` and `semantic` are exact channels and take no threshold.

There is also a hidden experimental `abstraction[:T]` mode. It reuses the `near`
candidate stream, defaults to threshold `0.50`, and then keeps only same-language
families whose normalized IL differs by exactly one supported literal leaf. Its
claim is weaker than `semantic`: it reports a refactoring-template witness with a
typed hole and caveats such as `numeric-domain-sensitive`; it does not say the two
copies are behavior-equivalent. Use `--format json --top 0` to consume the
`abstraction_witness` field documented in [scan-json](scan-json.md#abstraction-witnesses).
The witness is built from normalized IL, not line-level `--show proposal` text; its
template is machine-oriented today and carries typed hole metadata for tooling. A
reported abstraction family must share one literal-leaf hole position across all
members; mixed families with different changed positions are left unwitnessed.
When `near:T` and `abstraction:T` are combined in one scan, they share one fuzzy
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
and arbitrary field/property mutation stay closed. The exact fragment contract and JSON
metadata are documented in [fragment-contracts](fragment-contracts.md) and
[scan-json](scan-json.md#fragment-metadata).

## `nose query`

`nose query <path> [terms…]` is the **exploration surface** (#359): a stateless,
self-describing query over the same family dataset `scan` computes, designed for an LLM
agent loop. With no terms it prints a **landing dashboard** — what nose is, the family
count by confidence, the cleanest production candidates (each with its own runnable drill
link), the highest-confidence families, the most-duplicated directories, and a one-line
omission footer. Add terms to slice, facet, or open one family; every result ends in
runnable `nose query …` next-commands, so an agent navigates by following links rather
than re-reading a schema or hand-writing `jq`.

It is **opt-in and additive** — `nose scan`, `--fail-on`, and the scan-JSON contract are
unchanged.

```text
nose query <path> [FILTER … | group=FIELD | id=FAM] [sort=KEY] [top=N] [full] [all]
```

| part | meaning |
|---|---|
| `field=value` | keep families where the field equals the value (AND-ed); `field>N`/`field<N` for numbers; `path~substr` for a path substring |
| `group=FIELD` | facet the selection by a discrete field (`dir`, `scope`, `witness`, `lang`, `shape`), with a count and an exemplar per bucket |
| `id=FAM` | open one family (any unambiguous id prefix): its copies, the all-copies extraction skeleton, and navigation links |
| `sort=KEY` | `extractability` (default), `value`, or `members` |
| `top=N` | show the first N rows (default 30) |
| `full` | on `id=` or a list, render the all-copies extraction skeletons inline (batched) |
| `all` | widen past the curated default surface to the full raw universe (demoted families labeled) |

Fields: `scope` (prod\|test\|mixed), `witness` (exact\|subdag\|copy-paste\|similar),
`lang`, `path`, `dir`, `members`, `files`, `value`, `params`, `shared`. Every row shows
the payoff economics — `M/REP shared, Pp · ~N removable` — so a candidate can be triaged
without opening it. Each result is a pure function of (repo state, command); an unknown
field or enum value is a hard error (so a typo can't read as "no duplication").

A typical loop: `nose query .` → `nose query . witness=exact` → `nose query . id=<id> full`.

## Integrating with nose

Use `nose capabilities` when another tool needs to decide what this installed
binary supports before it invokes a scan:

```sh
nose capabilities
```

The command emits JSON only. It reports the tool version, platform, stable
commands, supported scan modes and output formats such as SARIF, JSON schema
versions, config keys, and scan capability flags such as baselines, caching,
and structured ignores. Do not scrape `nose --help`; help text is for humans
and may change to improve readability. The stable contract is documented in
[capabilities](capabilities.md).

## Other commands

- `nose review [paths…] [--base <ref>]` — flag clones changed inconsistently in a diff (a
  copy edited, its siblings missed). Defaults to `HEAD` for uncommitted local changes;
  use `--base origin/main` for a PR branch. The git-aware companion to `scan`; full guide in
  [review](review.md).
- `nose stats <paths…> [--top N] [--json]` — per-language IL lowering coverage (the
  Raw-node ratio), with the top unhandled surface kinds (`--top`, default 30; `--json`
  for machine output). Use it to spot a language/construct that isn't lowering well; see
  [languages](languages.md).
- `nose semantic-pack check <file-or-dir> [--format human|json]` — validate local
  semantic-pack v0 manifest structure and declared fixture assets. It is a
  pack-author/user workflow, not external pack certification; see
  [semantic-pack-conformance](semantic-pack-conformance.md).
- `nose il <file> [--normalized] [--no-cfg-norm] [--format sexpr|json]` — dump the IL
  for one file (`--normalized` shows the canonical form after the
  [normalization](normalization.md) passes). A debugging tool for understanding why two
  snippets do or don't converge; see [architecture](architecture.md).

`nose behavioral-gate` is a visible experimental Type-4 benchmark command, not a stable
integration surface; do not build automation around it without checking the current binary.

Hidden `detect`, `verify`, `features`, `eval`, and `ceiling` commands exist for
strict/research workflows. They are hidden from `--help` because `scan` is the command for
everyday use; the [benchmark](benchmark.md) page documents them.
