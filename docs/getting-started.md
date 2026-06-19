# Getting started

nose finds duplicated code — literal copy-paste, renamed copies, and the same
logic rewritten in a different style (and, occasionally, a different language) — and ranks each
group by how cleanly you could fold it into one shared helper. Point it at a
directory, read what it found, refactor from the top.

This page takes you from install to a report you can act on in a few minutes. When
you want the exact flag for something, the [usage](usage.md) reference has all of
them.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

Both put a single self-contained `nose` binary on your `PATH` — no runtime,
services, or network needed at analysis time. Prebuilt binaries for macOS (Apple
Silicon + Intel) and Linux (x86_64 + arm64) are attached to every
[release](https://github.com/corca-ai/nose/releases). To build from source
instead, see [usage → Install](usage.md#install).

## Your first run: `nose query`

`nose query <path>` is the one command you need. Point it at any file or directory
and it prints a summary of what it found, plus runnable next commands for filtering,
grouping, and opening one family. It recurses, respects `.gitignore` files inside the
analyzed tree, and nothing is written to disk.

```
$ nose query examples
nose finds duplication in code and docs.
nose finds; you judge. Filter, group, sort, or open families to explore.
analyzed 3 files · go 1 · python 1 · typescript 1

1 duplicated-code family.
  verified 1 (exact 1 · shared-core 0) · copy-paste 0 · similar 0
  verified = machine-checked evidence · exact = same unit behavior · shared-core = shared computation

best candidates:
  examples/sum.go:3  SumFor  6 copies · cross-language · ~30 repeated · exact   nose query examples id=a47c37baa1
  nose query examples sort=extractability       # all 1, best first

verified families (exact behavior or shared computation):
  examples/sum.go:3  SumFor  6 copies · cross-language · ~30 repeated · exact   nose query examples id=a47c37baa1
  nose query examples witness=exact             # the 1 exact whole-unit family

~30 duplicated lines on the default surface.

next commands — replace <path> with your path; terms combine with AND:
  filter  nose query <path> witness=exact        keep only the exact-behavior families
          nose query <path> members>3 path~api   compare with > < , ~ (contains), != (negate)
  group   nose query <path> group=dir            totals by directory (or: witness, lang, scope, same_symbol)
  open    nose query <path> id=<id> full         one family: every copy + the extraction skeleton
  sort    nose query <path> sort=value           by duplicated volume (or: extractability [default], members)
  more    nose query <path> all                  include families held back below the default surface
```

That's the whole loop in one screen: a summary, the best candidates, and a runnable
command on every line. From here you **open** a family, **slice** the list, or **facet** it.

## How to read the report

**The first line — `analyzed 3 files · go 1 · python 1 · typescript 1`** — is what nose
actually analyzed. If `.gitignore` or `--exclude` pruned vendored deps or build output, this
count is far smaller than the files on disk; glance at it to confirm nose looked where you
expected. (The *ignored* count is deliberately not shown — counting it would mean walking into
the very trees `.gitignore` exists to skip.)

**The confidence line** breaks the families down by *why* their copies merged — the evidence,
strongest first. The two **verified** channels come first because they carry machine-checked
evidence, not guesses:

- **verified** — `exact` / `shared-core`: `exact` proves the reported units compute the same
  thing; `shared-core` proves a shared sub-computation inside each site. (In `--format json`
  the `shared-core` witness is spelled `subdag`.)
- `copy-paste` — identical text; classic copy-paste (identifiers/literals may vary).
- `similar` — similar *shape*, not a verified shared decision.

**Each candidate row is one _family_** — one refactoring decision (extract a helper, base
class, or data table). Read it left to right:

```
examples/sum.go:3  SumFor  6 copies · cross-language · ~30 repeated · exact   nose query examples id=a47c37baa1
└─ first copy ──┘  └sym─┘  └ sites ┘  └─ payoff economics ──┘  witness   └─ the runnable drill command ─┘
```

- `6 copies` — how many places this code appears.
- `cross-language · ~30 repeated` — the family spans Go, Python, and TypeScript, so source
  lines are not directly comparable. nose ranks it by repeated semantic volume instead of
  pretending there are shared literal lines to extract.
- `exact` — the evidence tag from the confidence line above.
- The trailing `nose query … id=…` is the command to **open** this family.

**Scope tags.** A family may be tagged `prod`, `test`, or `mixed` (the same logic in a test
*and* in production). These are context for *where* to refactor, not a penalty — duplication in
tests is still a smell. Slice with `scope=prod` or `scope=test`.

## Open one family

Add `id=<id>` (any unambiguous prefix) to open a family — its copies, the all-copies extraction
skeleton, and a diff. Add `full` to render the skeleton inline:

```
$ nose query examples id=a47c37baa1 full
nose finds duplication in code and docs.
nose finds; you judge. Filter, group, sort, or open families to explore.
a47c37baa1 — exact · prod · 6 copies · cross-language · ~30 repeated
  → local duplication — extract a helper (cross-language)
  why this hint:
    - an implementation body was found
  copies:
    examples/sum.go:3-9  SumFor
    examples/sum.go:11-17  SumRange
    examples/sum.py:1-7  sum_while
    examples/sum.ts:1-7  sumFor
    examples/sum.ts:9-15  sumOf
    examples/sum.py:10-14  sum_for
     proposal  extract a shared helper · 0 shared lines · 1 parameter(s) vary (across all 6 copies)
       │ ⟨param 1: block⟩
     diff  examples/sum.go:3-9  vs  examples/sum.go:11-17
       …
```

The `→` line is a **hint** grounded in facts (a shared symbol name, how many directories it
spans), never a guess about what the code means. Every site is listed with its exact
`file:line-range` — you can't act on a clone you can't see. Each varying spot in the skeleton is
a `⟨param N: class⟩` placeholder (the coarse value-class — `literal`/`name`/`call`/`expr`/`block`
— a hint for the helper's signature).

## Slice, facet, and follow links

Every run also prints a cheatsheet of the query grammar. The moves:

| You want… | Command |
|---|---|
| Only the verified-evidence families (exact + shared-core) | `nose query src witness=exact,shared-core` |
| Narrow to one scope (test and prod rank equally otherwise) | `nose query src scope=prod` (or `scope=test`) |
| Families in one area | `nose query src path~loaders` |
| The duplication **hotspot** map (by directory) | `nose query src group=dir` |
| Open one family with its skeleton | `nose query src id=<id> full` |
| A ranked report to paste into a PR/issue | `nose query src --format markdown` |
| The versioned machine contract | `nose query src --format json` |
| Faster repeated runs | `nose query src --cache-dir .nose-cache` |

Each result is a **pure function of (repo state, command)**, and an unknown field or value is a
hard error — so a typo can never read as "no duplication." A typical loop is just
`nose query .` → `nose query . witness=exact` → `nose query . id=<id> full`.

By default nose runs all three channels — `syntax` (copy-paste runs), `semantic` (exact
same-logic clones — the Type-4 case: the same computation written differently), and `near`
(fuzzy near-duplicates). Pass `--mode` to run exactly the channels you list — see
[clone-types](clone-types.md) for what each finds.

## Catch a missed sibling edit: `nose query base=<ref>`

Once a codebase has clones, the risky moment is editing one of them: a fix applied to one copy
and missed in its siblings ships a half-fixed bug. `nose query <path> base=<git-ref>` compares
the working tree to a git ref and flags exactly that — a clone family changed in one copy but
not the others:

```sh
nose query .                            # explore the duplication first
nose query . base=HEAD                  # inspect uncommitted local changes
nose query . base=origin/main           # inspect a PR branch (e.g. in CI)
nose query . base=origin/main --fail-on any   # the CI gate (fires only on the proven case)
```

See [divergent edits](divergent-edits.md) for how these findings are ranked and the gate policy.

## Gate CI

`--fail-on any` makes nose exit non-zero when families survive the filters; `--baseline` plus
`--fail-on new` ignores accepted debt and fails only on *new* duplication. Pin `--mode` in a
gate so its surface stays stable across upgrades:

```sh
nose query src --mode syntax --fail-on any          # jscpd-style copy-paste gate
nose query src --baseline .nose-baseline.json --write-baseline   # accept today's state
nose query src --baseline .nose-baseline.json --fail-on new      # then fail only on new/changed
```

The full gate, baselines, SARIF, and fast re-runs are in
[continuous-integration](continuous-integration.md).

## Where to go next

- **[usage › nose query](usage.md#nose-query)** — the full query grammar: filters,
  groups, opening one family, and the CI gate.
- **[usage](usage.md)** — every command and flag, the ranking keys, and the detection
  modes in full.
- **[divergent edits](divergent-edits.md)** — the divergent-edit check (`nose query base=<ref>`): catch a
  clone changed in one copy but not its siblings.
- **[configuration](configuration.md)** — commit a `nose.toml` so CI and teammates
  don't retype long flag lists.
- **[continuous-integration](continuous-integration.md)** — turn a query into a
  pass/fail gate that flags only *new* duplication, with baselines and SARIF.
- **[clone-types](clone-types.md)** — what `syntax` / `semantic` / `near` cover
  across the Type-1–4 taxonomy, and the honest limits.
- **[languages](languages.md)** — the supported languages, declarative CSS and HTML
  markup, and the `<script>`/`<style>`/markup region extraction for Vue/Svelte/HTML.
