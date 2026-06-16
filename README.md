# nose

**nose** finds duplicated code — literal copy-paste, renamed copies, and the same
logic written in a different style or language — across Python, JavaScript,
TypeScript, Go, Rust, Java, C, Ruby, plus declarative **CSS** (computed-style
equivalence) and **HTML markup** (rendered-DOM equivalence), including the
`<script>`/`<style>`/markup regions inside Vue, Svelte, and HTML. It **proves** its
semantic matches are real — equal fingerprint ⟹ equal behavior, never a false
equivalence — and ranks candidates by refactoring value for the coding agent or CI
gate that consumes them. One self-contained Rust binary; no runtime, services, or
network.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

## Quick start

Point `nose query` at a directory to **explore** its duplication. It prints a landing
dashboard, and every result suggests the next command to run — so you (or an agent) navigate
by following links instead of memorizing flags:

```
$ nose query src
nose — finds duplicated & refactorable code across languages.
scanned 23 files · python 14 · typescript 9

4 duplicated-code families on the default surface.
  by confidence: exact 1 · subdag 0 · copy-paste 1 · similar 2

cleanest to extract (production first):
  src/loaders/users.py:1  load_users  2 copies · 8/10 shared, 2p · ~8 removable · similar   nose query src id=b221962180
  …
grammar:  nose query <path> [field=value | field>N | field~substr …] [group=FIELD | id=FAM | at=FILE:LINE] [sort=KEY] [top=N] [full] [all]
```

Then drill: `nose query src witness=exact` (only behavior-proven families), `nose query src
group=dir` (by directory), `nose query src id=b221962180 full` (open one family with its
all-copies extraction skeleton).

`nose query` is the **one command** for every workflow over that dataset — explore, report,
gate a PR, or feed a machine:

```sh
nose query src                               # the landing dashboard, cleanest-to-fold first
nose query src id=<fam> full                 # open one family: every copy + extraction skeleton
nose query src base=origin/main              # PR check: a change applied to one copy but not its siblings
nose query src --mode syntax --fail-on any   # jscpd-style copy-paste gate for CI
nose query src --format markdown             # a ranked report to paste into a PR or issue
nose query src --format json                 # the versioned machine-readable contract (query-JSON v2)
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
- [Clone types](docs/clone-types.md)
- [Benchmark](docs/benchmark.md)
- [Architecture](docs/architecture.md)
- [Semantic kernel](docs/semantic-kernel.md)
- [Query JSON contract](docs/query-json.md)

## License

MIT
