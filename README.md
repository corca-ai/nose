# nose

**nose** finds duplicated code — literal copy-paste, renamed copies, and the same
logic written in a different style or language — across Python, JavaScript,
TypeScript, Go, Rust, Java, C, Ruby, Swift, plus declarative **CSS** (computed-style
equivalence) and **HTML markup** (rendered-DOM equivalence), including the
`<script>`/`<style>`/markup regions inside Vue, Svelte, and HTML. It **proves** its
semantic matches are real — equal fingerprint ⟹ equal behavior, never a false
equivalence — and ranks candidates by refactoring value for review or CI. One
self-contained Rust binary; no runtime, services, or network.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

## Quick start

Point `nose query` at a directory to inspect its duplication. It prints a summary and
runnable next commands, so you can filter, group, or open one family without memorizing
flags:

```
$ nose query src
nose — duplicated code across languages, ranked for refactoring.
scanned 23 files · python 14 · typescript 9

4 duplicated-code families.
  proven 1 (exact 1 · shared-core 0) · copy-paste 1 · similar 2
  proven = same behavior, machine-verified · copy-paste = identical text · similar = similar shape

best candidates:
  src/loaders/users.py:1  load_users  2 copies · 8/10 shared, 2p · ~8 removable · similar   nose query src id=b221962180
  …
next commands — replace <path> with your path; terms combine with AND:
  filter  nose query <path> witness=exact   keep only the proven-identical families
  group   nose query <path> group=dir       totals by directory (or: witness, lang, scope)
  open    nose query <path> id=<id> full     one family: every copy + the extraction skeleton
```

`nose query` is the **one command** for every workflow over that dataset — explore, report,
gate a PR, or feed a machine:

```sh
nose query src                               # summary, best candidates first
nose query src id=<fam> full                 # open one family: every copy + extraction skeleton
nose query src base=origin/main              # PR check: a change applied to one copy but not its siblings
nose query src --mode syntax --fail-on any   # jscpd-style copy-paste gate for CI
nose query src --format markdown             # a ranked report to paste into a PR or issue
nose query src --format json                 # the versioned machine-readable contract (query-JSON v3)
nose query docs                              # also reports same-language near-duplicate Markdown prose
```

By default it runs all three channels — `syntax` (copy-paste runs), `semantic` (exact
same-logic Type-4 clones), and `near` (fuzzy near-duplicates) — and respects `.gitignore`;
pass `--mode` to run exactly the channels you list.

> `nose scan` (the one-shot ranked report + scan-JSON v1) and `nose review --base <ref>` (the
> PR check) still work but are **deprecated** in favour of `nose query` and `nose query
> base=<ref>`, which read the same dataset and now carry the CI gate and a versioned JSON
> contract. See [usage](docs/usage.md).

## Documentation

- [Getting started](docs/getting-started.md) — your first `nose query` and how to read the report.
- [Documentation home](docs/home.md) — entry point for using, integrating, and contributing.
- [Usage](docs/usage.md) — command and flag reference.
- [Configuration](docs/configuration.md) — `nose.toml`, excludes, modes, thresholds, ignores.
- [Contributing](CONTRIBUTING.md) — development workflow and quality gates.
- [Agent instructions](AGENTS.md) — repository-specific instructions for coding agents.

## Status

Pre-1.0. Current capability, language, JSON, benchmark, and soundness details live in the
wiki so claims have one source of truth:

- [Languages](docs/languages.md)
- [Markdown duplication](docs/markdown-duplication.md)
- [Clone types](docs/clone-types.md)
- [Benchmark](docs/benchmark.md)
- [Architecture](docs/architecture.md)
- [Semantic kernel](docs/semantic-kernel.md)
- [Query JSON contract](docs/query-json.md)

## License

MIT
