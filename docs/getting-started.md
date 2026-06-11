# Getting started

nose finds duplicated code — literal copy-paste, renamed copies, and the same
logic rewritten in a different style (and, occasionally, a different language) — and ranks each
group by how cleanly you could fold it into one shared helper. Point it at a
directory, read the list, refactor from the top.

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
services, or network needed at scan time. Prebuilt binaries for macOS (Apple
Silicon + Intel) and Linux (x86_64 + arm64) are attached to every
[release](https://github.com/corca-ai/nose/releases). To build from source
instead, see [usage → Install](usage.md#install).

## Your first scan

Run `nose scan` on any file or directory. It recurses, respects `.gitignore` files inside
the scanned tree, and prints the duplication it found, most worth-refactoring first:

```sh
nose scan path/to/project
```

```
$ nose scan examples --min-size 8
scanned 3 files · go 1 · python 1 · typescript 1
1 clone family, ranked by extractability (cleanest to fold into one helper)  ·  ~30 duplicated lines  (showing 1)

#1  id b658f483dcc2b097 · 6 copies · same logic in 3 languages (go, python, typescript) · ~30 lines removable
    → local duplication — extract a helper (cross-language)
    examples/sum.go:3-9  SumFor
    examples/sum.go:11-17  SumRange
    examples/sum.py:1-7  sum_while
    examples/sum.ts:1-7  sumFor
    examples/sum.ts:9-15  sumOf
    examples/sum.py:10-14  sum_for
```

That's the whole loop: scan, look at `#1`, decide whether to extract it, move on.

## How to read the report

**The first line — for example, `scanned 3 files · go 1 · python 1 · typescript 1`** — is what
nose actually analyzed. If `.gitignore` or `--exclude` pruned vendored deps or
build output, this count will be far smaller than the files on disk. Glance at it
to confirm nose looked where you expected. (The *ignored* count is deliberately
not shown — counting it would mean walking into the very trees `.gitignore` exists
to skip.)

**The summary line** tells you how many groups were found, how they're ranked, and
the total duplicated volume. By default nose shows the top 30 (`--top N` to change,
`--top 0` for all).

**Each numbered entry is one _family_** — one refactoring decision (extract a
shared helper, base class, or data table). Read it left to right:

- `3 copies` — how many places this code appears.
- `same logic in 2 languages …` — what's shared. For copies in one language this
  instead reads `N of M lines identical` (or `… shared, K spots differ`) — the
  *honest* overlap, so a pair that looks identical but really shares few lines is
  obvious. Cross-language copies have no shared *source* lines, so they report the
  language list instead.
- `~134 lines removable` — roughly how much code you'd delete by consolidating.
- The **`→` line is a hint**, grounded in facts (a shared symbol name, how many
  modules it spans), never a guess about what the code means.
- Then **every site is listed** with its exact `file:line-range` — you can't act on
  a clone you can't see.

**Scope tags.** A family may end with `· in test code` (all copies in test code) or
`· same code in tests and prod` (the same logic in a test *and* in production). These are context
for *where* to refactor, not a penalty — duplication in tests is still a smell.

### See more per family

The `--show` option adds views so you can see the code, not just where it lives
(repeatable / comma-list):

```sh
nose scan src --show diff       # show each family as a unified diff of its two copies
nose scan src --show proposal   # show the extracted helper skeleton, varying spots as parameters
```

## Common recipes

| You want… | Command |
|---|---|
| A report to paste into a PR or issue | `nose scan src --format markdown > REFACTOR.md` |
| Only the biggest, cleanest wins | `nose scan src --min-value 300 --min-members 3` |
| A copy-paste gate for CI (jscpd-style) | `nose scan src --mode syntax --fail-on any` |
| High-confidence "same logic" clones only | `nose scan src --mode semantic` |
| Fuzzy near-duplicates for review | `nose scan src --mode near:0.70` |
| Machine-readable output | `nose scan src --format json` |
| Faster repeated runs | `nose scan src --cache-dir .nose-cache` |

`nose scan` runs `syntax` + `semantic` + `near` by default (literal copy-paste,
exact same-logic clones, and fuzzy near-duplicates). Passing `--mode` replaces
that default with exactly the channels you list — see
[clone-types](clone-types.md) for what each one finds.

## Where to go next

- **[usage](usage.md)** — every command and flag, the ranking keys, and the scan
  modes in full.
- **[configuration](configuration.md)** — commit a `nose.toml` so CI and teammates
  don't retype long flag lists.
- **[continuous-integration](continuous-integration.md)** — turn a scan into a
  pass/fail gate that flags only *new* duplication, with baselines and SARIF.
- **[clone-types](clone-types.md)** — what `syntax` / `semantic` / `near` cover
  across the Type-1–4 taxonomy, and the honest limits.
- **[languages](languages.md)** — the supported languages and the embedded
  `<script>` extraction for Vue/Svelte/HTML.
