# Query watch sessions

`nose query <root> --watch --format jsonl` keeps one foreground analysis
session alive and emits a complete dashboard snapshot whenever the analyzed
source set or dashboard changes, including Markdown and structured ignores. It is intended for editors, local refactoring loops, and
other integrations that need fresh results without starting a new process for
every save.

```sh
nose query src --watch --format jsonl
```

Watch mode is opt-in. Ordinary `nose query` behavior and every existing output
format remain unchanged. The command is self-contained: it does not start a
daemon, open a network port, or require a service. Stop it with the usual
process interrupt.

## Should I use watch mode?

Use watch mode when an editor, dashboard, or local automation needs a fresh
machine-readable result after each save. Successful revisions carry a complete JSON snapshot,
so the consumer can replace its state instead of merging partial updates.

For a person running occasional terminal checks, repeated cached queries are
simpler:

```sh
nose query src --cache-dir .nose-cache
```

To make a watch session reuse earlier work after a restart, combine both
features:

```sh
nose query src --cache-dir .nose-cache --watch --format jsonl
```

See [faster repeated queries](query-cache.md) for cache storage and cleanup.

## Stream contract

Each stdout line is one JSON object with schema `nose.query-watch/v1`. Successful analysis emits `kind: "snapshot"`:

```json
{
  "schema": "nose.query-watch/v1",
  "kind": "snapshot",
  "sequence": 1,
  "source_set_digest": "<sha256>",
  "changed_paths": ["src/example.py"],
  "reconciliation": "incremental-leaf",
  "invalidation": { "schema": "nose.invalidation/v1" },
  "latency_ms": 12.4,
  "snapshot": { "tool": "nose", "view": "dashboard", "schema_version": 10 }
}
```

`sequence` starts at `0` for the initial snapshot and increases once per emitted
event, including errors. `source_set_digest` binds the analyzed code path/content identity
set; Markdown-only or presentation-policy revisions can retain that digest.
Use `sequence` to identify dashboard revisions. `changed_paths` reports the filesystem hints reconciled for the revision;
it is not a substitute for the digest. `reconciliation` is `initial`,
`incremental-leaf`, or `full-reconciliation`. `latency_ms` measures from the
first event in the debounced batch to the ready snapshot.

The nested `snapshot` is the same dashboard JSON produced by a clean
`nose query <root> --format json` with the same analysis options. Consumers can
therefore replace their complete local state on every line instead of applying
fragile partial diffs. Unknown fields must be ignored.

## Correctness and recovery

Filesystem notifications are hints, not truth. nose hashes source contents and
falls back to the ordinary query pipeline when an event is ambiguous, spans
multiple sources, changes membership or configuration, or requests an overflow
rescan. Atomic-save rename sequences and delete/recreate bursts converge on the
final filesystem state after a short debounce.

Configuration and ignore files are watched even outside the analysis roots.
Their containing directories are observed so atomic replacement keeps working.
A continuously arriving event stream is processed in batches of at most 250 ms
before analysis begins. Both the initial and replacement code snapshots and their
source digests come from the same session generation.

An explicit `--cache-dir` makes startup reuse the normal transactional cache. If
it is omitted, watch mode creates and removes a private temporary cache. A killed
session never makes the cache authoritative: restart validation uses source
contents, checksums, and the last committed generation, and safely recomputes
anything not committed.

A malformed configuration or transient read/analysis error after startup emits
`kind: "error"`, `sequence`, `changed_paths`, `snapshot_valid: false`,
`last_good_sequence`, and `error.message`. The process keeps watching. Consumers
must mark their last result stale until the next successful snapshot. nose drops
partially refreshed state and performs full reconciliation on the next event;
even an unchanged recovered snapshot is emitted. Startup errors still fail the
command, and output-stream errors end the stream.

## Supported surface

Watch v1 emits the unfiltered dashboard. It accepts the ordinary analysis inputs
such as `--mode`, thresholds, roots, excludes, generated-path assertions, config,
and cache policy. Query terms, `base=`, external semantic-pack influence,
baseline writes, and `--fail-on` gates are rejected. Those workflows retain
their one-shot contracts; watch mode is an observation stream, not a CI gate.

Integrations should preflight all three values from `nose capabilities`:

- `query.capabilities.query_watch`
- `query.capabilities.query_watch_jsonl_v1`
- `schemas.query_watch_jsonl` containing `nose.query-watch/v1`

See [usage](usage.md) and [query JSON](query-json.md) for adjacent contracts. The checked
[benchmark evidence](incremental-cache-benchmark.md) records measured session behavior.
