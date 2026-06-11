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

Point `nose scan` at a directory. It groups duplicated code into clone families
and ranks them by how cleanly each folds into one shared helper, so the report
reads top-down as a refactoring to-do list:

```
$ nose scan src
scanned 23 files · python 14 · typescript 9
4 clone families, ranked by extractability (cleanest to fold into one helper)  ·  ~118 duplicated lines  (showing 4)

#1  id b221962180c09063 · 2 copies · 8 of 10 lines shared, 2 spots differ · ~8 lines removable
    → local duplication — extract a method from the repeated block
    src/loaders/users.py:1-10  load_users
    src/loaders/orders.py:12-21  load_orders
…
```

The everyday commands:

```sh
nose scan src --show diff                  # also show exactly what differs inside each family
nose scan src --format markdown            # a report to paste into a PR or issue
nose review --base origin/main             # PR check: a change applied to one clone copy but not its siblings
nose scan src --mode syntax --fail-on any  # jscpd-style copy-paste gate for CI
nose scan src --format json                # machine-readable output for tooling and agents
```

By default `nose scan` runs all three channels — `syntax` (copy-paste runs),
`semantic` (exact same-logic Type-4 clones), and `near` (fuzzy near-duplicates) —
and respects `.gitignore`. Pass `--mode` to run exactly the channels you list.

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
