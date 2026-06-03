# Usage

How to install nose and run it on a codebase. For settings you'd commit to a
repo see [configuration](configuration.md); for CI use see [continuous-integration](continuous-integration.md). Back to
[home](home.md).

## Install

nose is a single self-contained Rust binary:

```sh
cargo build --release
# binary at ./target/release/nose
```

It needs no runtime, services, or network — point it at source files.

## The one command you need: `scan`

`nose scan <paths…>` scans one or more files/directories (recursively,
respecting `.gitignore`), groups duplicated code into **families**, and ranks
them by **extractability** — how cleanly each family folds into one shared helper —
so the duplication you can actually act on surfaces first. (See [Ranking](#ranking)
below to sort by raw volume, copy count, or spread instead.)

```sh
./target/release/nose scan path/to/project
```

```
$ nose scan bench/repos/radash
150 refactoring candidates, ranked by extractability (cleanest to fold into one helper)  ·  ~5840 duplicated lines  (showing 30)

#1  3 copies · same logic in 2 languages (javascript, typescript) · ~134 lines removable
    → consolidate `series` — 3 copies (cross-language)
    bench/repos/radash/src/series.ts:7-97        series
    bench/repos/radash/cdn/radash.esm.js:823-877 series
    bench/repos/radash/cdn/radash.js:826-880     series
```

Each **family** is one refactoring decision (extract a shared helper / base class
/ data table), described in plain language: how many copies, how much is actually
shared vs varies, and how many lines you could remove. For same-language families the
description reads `N of M lines identical, K spots differ` — the *honest* overlap, so a
structurally-identical pair that actually shares few lines is obvious (cross-language
families have no shared *source* lines, so they read `same logic in N languages`).
**Every site is listed** — you can't act on a clone you can't see. The `→` line is a
**hint** grounded in the facts (a shared symbol name, cross-language spread, how many
modules it touches), never a guess about semantics. Add `--proposal` for the extracted
helper, `--diff` for exactly what varies.

**Scope tags.** Families are tagged by where the duplication lives — `· test`
(all sites in test code) or `· test↔prod` (the same logic in a test *and* in
production). Duplication in tests is a real smell too, so these are ranked by
value like any other family; the tag is **context** (where to refactor), not a
penalty. The classification is a conservative path/name heuristic; everything
else is `prod`.

### Flags

Grouped by what they do. Anything here can also be set in [configuration](configuration.md).

**Filter & shape the report**

| flag | effect |
|---|---|
| `--sort KEY` | ranking: `extractability` (default), `value`, or `sites` (see [Ranking](#ranking)) |
| `--top N` | show only the top N families (default 30; `--top 0` = all) |
| `--min-members N` | only families with at least N duplicated sites (default 2) |
| `--min-value V` | hide families below this refactoring value (noise floor on large repos) |
| `--min-tokens N` | ignore units smaller than this (structural size in IL nodes; default 24) — the single minimum-size knob |
| `--threshold T` | acceptance similarity in `[0,1]` (default 0.70) |
| `--strict` | behavioral-clone mode: precision gates on, threshold 0.86 (see below) |
| `--exclude <glob>` | skip paths matching a gitignore-syntax glob (repeatable) |

### Ranking

`--sort` chooses what "most worth your attention first" means. `--min-value` is a
noise floor on raw value and applies under every sort.

| key | ranks by | use when |
|---|---|---|
| `extractability` *(default)* | invariant (shared) lines × copies × spread, weighted by tightness (shared/total) and penalized by parameter count | you want the duplication that folds *cleanly* into one helper — not the biggest block that merely looks similar |
| `value` | raw duplicated volume: removable lines × similarity × spread | you want the most *code* deleted, accepting that divergent copies cost more to merge |
| `sites` | number of copies | hunting the most-repeated patterns |

Extractability is the default because raw volume over-rewards a large block whose
copies share little: a 384-line family that shares only 22 lines across 14 varying
spots is mostly scaffolding (6% invariant), not an extraction — it ranks far below a
tight `15/15`-shared, zero-parameter pair. The honest `N/M shared · Pp` cell in the
report is the same signal the ranking uses. Same-language families with **no** shared
invariant lines (a language idiom, or two unrelated type literals with the same shape)
have nothing to extract and sink to the bottom, even at `sim 1.00`.

**Review what was found**

| flag | effect |
|---|---|
| `--diff` | show each family inline as a unified diff between its two representative copies — both versions and exactly what differs |
| `--proposal` | show an extraction skeleton per family — the shared structure with the differing parts marked as parameters |
| `--hotspots` | after the report, rank directories/modules by total duplicated lines (architecture view) |
| `--format human\|json\|markdown\|sarif` | output format (default `human`) |

**Workflow** (`--baseline`, `--write-baseline`, `--fail`, `--cache-dir`, `--config`)
are covered in [continuous-integration](continuous-integration.md) and [configuration](configuration.md).

### Candidate vs. strict mode

nose runs in **candidate mode** by default: it trades a little precision for
recall and ranks families so you review the highest-value duplication first.
This is the right mode for finding refactoring opportunities.

`--strict` switches to **behavioral-clone (Type-4) mode**: it turns on
precision gates (literal/string-value awareness, data-table and return-signature
gates) and raises the threshold to 0.86, for clean dedup/clone-audit results.
Both share one engine and one IL.

## Inspecting & measuring

- `nose il <file> [--normalized] [--format sexpr|json]` — dump the IL for one
  file. `--normalized` shows the canonical form after the [normalization](normalization.md)
  passes; invaluable when debugging why two snippets do (or don't) converge.
- `nose stats <paths…> [--top N] [--json]` — per-language IL lowering coverage (the
  Raw-node ratio), with the top unhandled surface kinds (`--top`, default 30; `--json`
  for machine output). Use it to spot a language/construct that isn't lowering well; see
  [languages](languages.md).

A `detect` command (raw clone pairs/groups) and `eval` / `ceiling` (scoring
predictions against a gold set) also exist as the strict/research surface. They
are hidden from `--help` because `scan` is the command for everyday use; the
[benchmark](benchmark.md) page documents them.
