# Synthetic recall base corpus

Distinct prose paragraphs used by the synthetic recall-vs-edit-ratio benchmark
(`nose-markdown::synth`). Each section is a base block; the benchmark injects controlled edits at
known ratios and measures how reliably the detector still recovers base↔edited as a pair. Blocks
are deliberately distinct (different topics/vocabulary) so they do not cross-match.

## Configuration loader
The configuration loader reads the settings file once at startup and validates every declared
field against its schema before the rest of the system initializes. Missing required keys abort
the boot sequence with a precise diagnostic, while unknown keys produce a warning so typos in a
deployment file surface immediately rather than failing silently much later under real traffic.

## Authentication middleware
The authentication middleware inspects each incoming request for a signed bearer token, verifies
its signature against the rotating public keys, and rejects expired or malformed credentials with
a uniform error. Successful verification attaches the resolved principal to the request context so
downstream handlers can make authorization decisions without re-parsing the original token again.

## Cache eviction
The cache keeps frequently requested records in memory and evicts the least recently used entries
once the configured capacity is exceeded. Each lookup refreshes the recency timestamp, and a
background sweep periodically removes stale items whose time-to-live has elapsed so that callers
never observe data that the upstream source has already changed or deleted some time ago.

## Structured logging
The logging subsystem emits one structured record per event with a stable set of fields, so logs
can be queried by request identifier, severity, and component without brittle text matching. Each
record carries a monotonic sequence number and a wall-clock timestamp, and sensitive values are
redacted at the boundary before any line is written to the durable sink on disk.

## Deployment pipeline
The deployment pipeline builds an immutable artifact, runs the full test matrix against it, and
promotes the same bytes through staging and production without rebuilding. A failed gate halts the
promotion and notifies the owning team, and every release records the exact commit, the toolchain
version, and the configuration snapshot that produced the shipped binary for later auditing.

## Database migrations
Schema migrations are applied in a single transaction where the engine supports it, and each
migration is paired with a tested rollback so an unhealthy release can be reverted quickly. The
runner records which migrations have executed in a dedicated table and refuses to start the
application when the recorded version is ahead of the binary, preventing accidental data loss.

## Network retries
Outbound calls retry transient failures with exponential backoff and jitter, capping the total
attempt budget so a struggling dependency cannot stall the caller indefinitely. Idempotent
requests carry a stable key so a retried write is deduplicated on the server, and non-idempotent
operations are never retried automatically to avoid duplicating an effect the user did not intend.

## Rendering layer
The rendering layer turns the view model into markup on the server and hydrates it on the client
so the first paint does not wait for a round trip. Components declare their data dependencies
explicitly, the framework batches updates within a frame, and list reconciliation uses stable keys
so reordering an item moves the existing node instead of discarding and recreating its subtree.

## Test harness
The test harness discovers cases by convention, isolates each one in a fresh fixture, and runs the
independent suites in parallel across the available cores. Flaky cases are quarantined rather than
retried blindly, golden outputs are compared byte for byte, and the summary reports the slowest
cases so contributors can keep the overall feedback loop short as the suite grows over time.

## Build cache
The build system fingerprints every input — source, flags, and toolchain — and reuses a cached
output whenever the fingerprint is unchanged, so an incremental build only recompiles what truly
moved. Cache entries are content-addressed and shared across machines through a remote store,
turning a cold checkout on continuous integration into a mostly warm build within seconds.

## Markup parser
The parser tokenizes the source into a flat stream, then folds the tokens into a tree whose shape
mirrors the document outline rather than the raw byte layout. Unrecognized constructs become opaque
raw nodes instead of aborting, so a single malformed region never discards the rest of the file,
and the resulting tree preserves precise source spans for accurate diagnostics and edits.

## Job scheduler
The scheduler dispatches queued jobs to workers according to priority and fairness, leasing each
job for a bounded interval so a crashed worker's task returns to the queue automatically. Long
jobs check in periodically to extend their lease, completed jobs are acknowledged exactly once, and
a dead-letter queue captures tasks that exhaust their retry budget for later human inspection.

## Metrics collection
The metrics layer records counters, gauges, and histograms in memory and exposes them on a scrape
endpoint without blocking the hot path. Labels are bounded to a fixed cardinality to keep storage
predictable, and the collector pre-aggregates high-frequency events so a downstream system ingests
a compact summary instead of millions of individual points every single scraping interval.

## Message queue
The queue accepts messages durably, fans them out to subscribed consumers, and tracks per-consumer
offsets so each one resumes exactly where it stopped after a restart. Back-pressure slows a fast
producer when a consumer lags, ordering is preserved within a partition, and a poison message that
repeatedly fails processing is routed aside so it cannot block the rest of the stream forever.

## Encryption at rest
Stored records are encrypted with a per-tenant key wrapped by a master key held in the key service,
so rotating the master never requires re-encrypting the underlying data. Decryption happens only
inside the trusted boundary, keys are never written to logs or temporary files, and an audit trail
records every wrap and unwrap operation with the requesting identity and the precise time.
