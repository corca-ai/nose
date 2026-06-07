# nose

**nose** finds refactoring candidates and semantic code clone families across Python,
JavaScript, TypeScript, Go, Rust, Java, C, Ruby, and embedded `<script>` logic in Vue,
Svelte, and HTML.

It ships as one self-contained Rust binary. By default, `nose scan` combines a
same-language copy-paste floor with exact modeled semantic matches, then ranks clone
families by how cleanly they can be extracted.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

To build from source:

```sh
cargo build --release
./target/release/nose scan examples --min-size 8
```

## First Scan

```sh
nose scan path/to/project
nose scan src --format markdown > REFACTOR.md
nose scan src --format json --top 50
nose scan src --mode syntax --fail-on any
nose review --base origin/main --fail
```

`nose scan` respects `.gitignore` files inside scanned trees. Use `--mode semantic` for
exact semantic-only findings, or `--mode syntax,semantic,near:0.70` to include fuzzy
near-duplicates.

## Documentation

- [Documentation home](docs/home.md) — entry point for using, integrating, and contributing.
- [Getting started](docs/getting-started.md) — first scan and how to read the report.
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
- [Scan JSON schema](docs/scan-json.md)

## License

MIT
