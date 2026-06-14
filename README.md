# nose

**nose** finds duplicated code worth refactoring — literal copy-paste, renamed
copies, and the same logic written in a different style or language — across
Python, JavaScript, TypeScript, Go, Rust, Java, C, Ruby, and the embedded
`<script>` logic in Vue, Svelte, and HTML. One self-contained Rust binary; no
runtime, services, or network.

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
  src/loaders/users.py:1  2 copies · 8/10 shared, 2p · ~8 removable · similar   nose query src id=b221962180
  …
grammar:  nose query <path> [field=value | field>N | field~substr …] [group=FIELD | id=FAM] [sort=KEY] [top=N] [full] [all]
```

Then drill: `nose query src witness=exact` (only behavior-proven families), `nose query src
group=dir` (by directory), `nose query src id=b221962180 full` (open one family with its
all-copies extraction skeleton).

For a **one-shot ranked report** — to read top-down, paste into a PR, or gate CI — use
`nose scan`:

```sh
nose scan src                              # the ranked report, cleanest-to-fold first
nose scan src --show diff                  # also show exactly what differs inside each family
nose scan src --format markdown            # a report to paste into a PR or issue
nose review --base origin/main             # PR check: a change applied to one clone copy but not its siblings
nose scan src --mode syntax --fail-on any  # jscpd-style copy-paste gate for CI
nose scan src --format json                # the versioned machine-readable contract for batch tooling and CI
```

`query` and `scan` read the **same** dataset: `query` is the interactive/agent surface,
`scan` is the one-shot report plus the frozen JSON contract and the `--fail-on` CI gate. By
default both run all three channels — `syntax` (copy-paste runs), `semantic` (exact
same-logic Type-4 clones), and `near` (fuzzy near-duplicates) — and respect `.gitignore`.
Pass `--mode` to run exactly the channels you list.

## Documentation

- [Getting started](docs/getting-started.md) — first scan and how to read the report.
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
- [Scan JSON schema](docs/scan-json.md)

## License

MIT
