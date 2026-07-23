# Semantic packs

Most users do not need semantic packs. nose ships with builtin language and
library knowledge, and ordinary `nose query` runs use it automatically.

A semantic pack is a local JSON manifest that describes additional,
project-specific library semantics. Loading one is an explicit opt-in. nose
does not download packs, contact a registry, or execute code from a pack.

## Choose the workflow you need

| Goal | Workflow |
|---|---|
| Use builtin nose knowledge | Do nothing |
| Validate or inspect a local pack | `nose semantic-pack check <path>` |
| Include a local pack as metadata in one query | `nose query . --semantic-pack <path>` |
| Authorize reviewed v1 rows to influence results | Create and use a project lock |

An unlocked local pack is metadata-only: nose validates and reports it, but it
does not change analysis results. Influential v1 packs require a
content-pinned project lock that records the exact manifests, rows, channels,
and local dependency evidence the project reviewed.

## Load metadata

For one run:

```sh
nose query . --semantic-pack semantic-packs/example.json
```

For a stable project opt-in:

```toml
[query]
semantic-packs = ["semantic-packs/example.json"]
```

Paths in `nose.toml` are relative to that configuration file. A directory
loads its direct `*.json` children in sorted order and does not recurse.

## Authorize an influential v1 pack

First validate the manifest, then create and inspect a lock from local files:

```sh
nose semantic-pack check semantic-packs/example.json
nose semantic-pack lock semantic-packs/example.json \
  --dependency pom.xml \
  --channel near \
  --output nose.semantic-pack-lock.json
nose semantic-pack status nose.semantic-pack-lock.json
```

Commit the reviewed lock and configure it:

```toml
[query]
semantic-pack-lock = "nose.semantic-pack-lock.json"
```

The lock is invalidated when pinned content changes. A missing, stale,
conflicting, or path-escaping lock fails before source analysis instead of
silently widening trust. A project lock and unlocked `semantic-packs` cannot be
used together.

Pack authors and teams using receipt-backed exact claims should continue with
the detailed [loading and trust policy](semantic-pack-loading.md), the
[project-lock guide](semantic-pack-project-lock.md), and the
[conformance guide](semantic-pack-conformance.md) for provider workflows.
