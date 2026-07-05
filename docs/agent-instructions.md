# Agent instructions

This page owns the repository-specific instructions for coding agents working on
nose. [`AGENTS.md`](../AGENTS.md) stays as the minimal entry point so agent hosts
can find this wiki without duplicating mutable documentation policy.

## Documentation workflow

Documentation is part of the codebase. Before changing code, read the relevant
wiki pages linked from [home](home.md), then update the affected docs in the same
change.

Follow these principles:

- treat docs like code: reduce duplication, reveal intent, and keep them clear
  and simple;
- prefer small focused pages with one responsibility over long mixed pages;
- link related pages, like the rest of the wiki;
- keep [`README.md`](../README.md) minimal and point to [`AGENTS.md`](../AGENTS.md);
- keep [`AGENTS.md`](../AGENTS.md) minimal and route details into `docs/`;
- use [home](home.md) as the entry point for the project documentation.

Do not edit [`AGENTS.md`](../AGENTS.md) for ordinary documentation workflow
changes. Update this page instead, and touch `AGENTS.md` only when the agent
entry-point links themselves need to change.

## Docs checks

Use [`scripts/check-docs.sh`](../scripts/check-docs.sh) for the repository docs
gate. It wraps the installed `awiki` version and the semantic-pack examples check.

When running `awiki` directly, use `docs/` as the wiki root with syntax supported
by the installed `awiki` version.

## Claude compatibility

[`CLAUDE.md`](../CLAUDE.md) is a symlink to [`AGENTS.md`](../AGENTS.md) because
Claude Code does not follow the `AGENTS.md` convention.
