# Faster repeated queries with a cache

By default, `nose query` analyzes from scratch and writes nothing. Add
`--cache-dir` when you expect to run the same query repeatedly:

```sh
nose query . --cache-dir .nose-cache
```

The first run fills the cache. Later runs reuse unchanged analysis and
recalculate what changed. Caching changes how work is reused, not which
duplication nose reports. Test-context extraction changes invalidate raw-IL and derived-unit
cache versions, so old entries cannot silently retain outdated production/test scope.

## When to use it

Use a persistent cache for:

- repeated local scans while refactoring;
- CI jobs that restore the same cache directory between runs; or
- faster startup for a long-running [watch session](query-watch.md).

For an occasional scan, plain `nose query .` is simpler. The cache is opt-in;
nose never creates a persistent cache directory unless you provide one.

Choose a project-local directory such as `.nose-cache` and add it to the
repository's ignore file. You can also keep the directory outside the working
tree:

```sh
nose query . --cache-dir ../nose-cache/my-project
```

## Storage and recovery

The cache uses checksummed, versioned artifacts and commits updates as complete
generations. If a query is interrupted, two queries overlap, an entry is
damaged, or an old format is encountered, nose ignores incomplete or invalid
data and recalculates it. Source, configuration, and relevant semantic-pack
changes invalidate the affected work.

Unreadable included sources fail analysis even when an older cached result exists.
Damaged entries are replaced with a complete checksummed record on every platform,
without deleting the visible entry before its replacement is ready. Incremental
score keys include the effective scoring configuration; enabling timing or cache
diagnostics does not invalidate pair scores.

The default storage limit is 5 GiB. Override it for one run:

```sh
nose query . --cache-dir .nose-cache --cache-max-bytes 2GiB
```

Or set `query.cache-max-bytes` in [`nose.toml`](configuration.md). The
configuration sets the limit only; `--cache-dir` still chooses and enables the
cache.

## Inspect and clean up

```sh
nose cache status --dir .nose-cache
nose cache prune --dir .nose-cache
nose cache clear --dir .nose-cache
```

`status` reports managed storage. `prune` removes old and excess entries.
`clear` removes nose-managed cache data while preserving unrelated files in the
same directory.

## Combine it with watch mode

```sh
nose query . --cache-dir .nose-cache --watch --format jsonl
```

The persistent cache speeds startup and can be reused after a restart. If
`--cache-dir` is omitted, watch mode uses a private temporary cache and removes
it when the session ends.

See the [usage reference](usage.md#other-commands) for cache administration and
the [continuous integration guide](continuous-integration.md#fast-re-runs---cache-dir)
for CI examples.
