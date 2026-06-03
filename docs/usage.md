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
them by **refactoring value** — `removable lines × similarity × cross-module
/-file /-language spread` — so a pattern repeated across many modules outranks a
local copy-paste.

```sh
./target/release/nose scan path/to/project
```

```
$ nose scan bench/repos/radash
62 refactoring candidate families  ·  ~1723 duplicated lines  (showing 30)

#1   value     348  ·  3 sites · 3 files · 2 modules · 2 langs (javascript, typescript) · sim 1.00 · ~134 dup lines
     → consolidate `series` — 3 copies (cross-language)
     bench/repos/radash/src/series.ts:7-97        series
     bench/repos/radash/cdn/radash.esm.js:823-877 series
     bench/repos/radash/cdn/radash.js:826-880     series
```

Each **family** is one refactoring decision (extract a shared helper / base class
/ data table). The `→` line is a **hint** grounded in the facts — a shared symbol
name, cross-language spread, how many modules it touches — so you know what kind
of refactor applies before opening a file.

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
| `--top N` | show only the top N families (default 30; `--top 0` = all) |
| `--min-members N` | only families with at least N duplicated sites (default 2) |
| `--min-value V` | hide families below this refactoring value (noise floor on large repos) |
| `--min-tokens N` | ignore units smaller than this (structural size in IL nodes; default 24) — the single minimum-size knob |
| `--threshold T` | acceptance similarity in `[0,1]` (default 0.70) |
| `--strict` | behavioral-clone mode: precision gates on, threshold 0.86 (see below) |
| `--exclude <glob>` | skip paths matching a gitignore-syntax glob (repeatable) |

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
