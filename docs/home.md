# nose documentation

**nose** finds duplicated code, including copies that were renamed or rewritten,
and ranks the results so you can decide what is worth refactoring. It runs as one
local binary with no service or network dependency.

The repository [README](../README.md) is the one-screen overview. This wiki starts
with the everyday workflow, then links to reference and integration details.

## New to nose?

Read these in order:

1. [Getting started](getting-started.md) — install nose, run your first query,
   read the report, and open one result.
2. [Clone types](clone-types.md) — understand the difference between copy-paste,
   verified same behavior, and similar-looking code.
3. [Faster repeated queries](query-cache.md) — reuse prior analysis safely while
   you work.
4. [Configuration](configuration.md) — commit shared defaults in `nose.toml`.
5. [Continuous integration](continuous-integration.md) — add a stable gate only
   after the local query is useful.

You can stop after step 1 and keep using `nose query <path>` interactively.

## Common workflows

| You want to… | Start here |
|---|---|
| Explore and rank duplication | [Getting started](getting-started.md) |
| Look up a command or flag | [Usage reference](usage.md) |
| Make repeated runs faster | [Query cache](query-cache.md) |
| Refresh an editor or local tool after each save | [Watch mode](query-watch.md) |
| Find a fix applied to one copy but missed in its siblings | [Divergent edits](divergent-edits.md) |
| Adopt nose in CI | [Continuous integration](continuous-integration.md) |
| Suppress a reviewed result with a reason and expiry | [Structured ignores](structured-ignores.md) |
| Detect duplicated prose in Markdown | [Markdown duplication](markdown-duplication.md) |

## Advanced workflows

- [Semantic packs](semantic-packs.md) — add reviewed project-specific semantic
  knowledge. Most users do not need this.
- [Divergent history mining](divergent-history-mining.md) — audit missed sibling
  edits across a bounded commit range. Keep this out of PR-time CI.

## Reference

- [Usage](usage.md) — all commands, query terms, modes, and flags.
- [Configuration](configuration.md) — `nose.toml` settings and precedence.
- [Languages](languages.md) — supported languages and embedded regions.
- [Clone types](clone-types.md) — coverage and limits across Type-1 through Type-4.
- [Reinvented helpers](reinvented-helpers.md) — inline code that repeats an
  existing pure helper.

## Building an integration

Human-readable output is for interactive use. Integrations should inspect
[capabilities](capabilities.md) and consume the versioned
[query JSON](query-json.md) contract.

- [Watch mode](query-watch.md) — complete JSONL snapshots after source changes.
- [Agent recipe](agent-recipe.md) — a validated exploration and batch protocol
  for coding agents.

## Contributing and implementation details

These pages are not required reading for users:

- [Contributing](contributing.md) — development workflow and quality gates.
- [Auxiliary development tools](tooling.md) — checked local/CI tool versions,
  diagnostics, and explicit bootstrap.
- [Architecture](architecture.md) — crates and the analysis pipeline.
- [Design and direction](design.md) — product principles and roadmap decisions.
- [Development and evidence index](development-and-evidence.md) — internal
  proof contracts, semantic-kernel design, benchmarks, audits, and release
  evidence.
- [Documentation lifecycle](documentation-lifecycle.md) — how current guides,
  maintained references, decisions, active roadmaps, and historical records are
  classified and reviewed.
- [Agent instructions](agent-instructions.md) — repository-specific instructions
  for coding agents.

Release history is in the [changelog](../CHANGELOG.md). Dated audits, closed
issue records, and append-only research ledgers are retained separately in the
[historical records index](historical-records.md) with their evidence context.
