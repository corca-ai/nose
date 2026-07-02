# Recall-loss baselines

This directory keeps compact, reproducible summaries for
`nose verify --recall-loss-report`. The full JSON reports are local artifacts;
the checked-in files record the command, selected surface, hard gate, reason
rollups, and representative fixtures needed to reproduce or review a semantic
kernel PR.

Scheduling lifecycle audit artifacts use a separate source-prevalence status
vocabulary:

- `closed-boundary` marks residual source surfaces that remain exact-closed.
- `reporting-supported-closed-boundary` marks exact-closed surfaces whose
  diagnostics can already name the missing obligation.
- `exact-supported-boundary` marks source surfaces already covered by existing
  proof-backed exact capability; they are accounted for but not implementation
  candidates.
- `superseded-overlap-boundary` marks broad historical buckets retained for
  continuity after concrete operation rows replace them as actionable work.

## Regenerate

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.crates.json

cargo run -q -p nose-cli -- verify \
  bench/repos/chi/middleware/content_type.go \
  bench/repos/boltons/boltons/iterutils.py \
  bench/repos/thor/lib/thor/actions.rb \
  bench/repos/radash/src/array.ts \
  bench/repos/hyperfine/src/util/number.rs \
  bench/repos/swift-metrics/Sources/CoreMetrics/Metrics.swift \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.corpus-slice.json
```

Compare two full reports with:

```sh
python3 scripts/recall-loss-diff.py before.json after.json
```

Build a callee-identity census from a full report with:

```sh
python3 scripts/recall-loss-callee-census.py \
  target/recall-loss.crates.json \
  --format json \
  > target/callee-census.json
```

Build the full pinned-corpus priority census with:

```sh
python3 scripts/corpus-priority-census.py \
  --jobs 4 \
  --logs-dir target/corpus-priority-census-full \
  --output target/corpus-priority-census-full.json
```

Use `--scan-only` for a source-prevalence-only dry run, and
`--summarize-only` to rebuild the aggregate from existing per-repo
`--recall-loss-report` files. The source scan is a lexical pricing signal, not
semantic proof.

Full reports also include `import_snapshot_census`, which records successful
imported immutable snapshot counts and unresolved binding-import miss reasons.
Use that section directly when deciding the next import-backed provider-value
slice.

Build the Java `Arrays`/`Collections` partial-coverage audit with:

```sh
python3 scripts/java-arrays-collections-audit.py \
  --output target/java-arrays-collections-audit.v1.json
```

Build the Go `sort`/`slices`/`maps` partial-coverage audit with:

```sh
python3 scripts/go-stdlib-collections-audit.py \
  --output target/go-stdlib-collections-audit.v1.json
```

Build the JS/TS builtin partial-coverage audit with:

```sh
python3 scripts/js-ts-stdlib-partial-audit.py \
  --output target/js-ts-stdlib-partial-audit.v1.json
```

Build the Python HOF/runtime attribution audit with:

```sh
python3 scripts/python-hof-runtime-audit.py \
  --output target/python-hof-runtime-audit.v3.json
```

Build the Rust stdlib partial-coverage audit with:

```sh
python3 scripts/rust-stdlib-partial-audit.py \
  --output target/rust-stdlib-partial-audit.v2.json
```

Build the Swift stdlib partial-coverage audit with:

```sh
python3 scripts/swift-stdlib-partial-audit.py \
  --output target/swift-stdlib-partial-audit.v2.json
```

Build the #594 cross-language scheduling/error/callback boundary census with:

```sh
python3 scripts/cross-language-boundary-census.py \
  --output target/cross-language-boundary-census-594.v1.json
```

Build the #602 scheduling/lifecycle boundary audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.issue-602.crates.json \
  --output target/scheduling-lifecycle-boundary-audit-602.v1.json
```

Build the cross-language async function/block obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.async-function-obligation.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.async-function-obligation.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.async-function-obligation.json \
  --generated-on 2026-06-30
```

Build the non-JS async runtime API obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.non-js-async-runtime.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.non-js-async-runtime.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.non-js-async-runtime.json \
  --generated-on 2026-06-30
```

Build the non-JS async runtime breadth audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.non-js-async-runtime-breadth.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.non-js-async-runtime-breadth.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.non-js-async-runtime-breadth.json \
  --generated-on 2026-07-01
```

Build the Python async protocol lifecycle audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.python-async-lifecycle.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.python-async-lifecycle.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.python-async-lifecycle.json \
  --generated-on 2026-07-01
```

Build the Swift async iteration protocol audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-async-iteration.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-async-iteration.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-async-iteration.json \
  --generated-on 2026-07-01
```

Build the Swift async task source-protocol audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-async-task.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-async-task.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-async-task.json \
  --generated-on 2026-07-01
```

Build the Swift throwing callable source-protocol audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-throwing-callable.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-throwing-callable.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-throwing-callable.json \
  --generated-on 2026-07-01 \
  --include-zero-surface swift.error.throwing_function \
  --include-zero-surface swift.error.throwing_closure
```

Build the Swift try-expression reporting audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-try-expression-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-try-expression-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-try-expression-reporting.json \
  --generated-on 2026-07-02
```

Build the Swift exception residual-accounting audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-exception-residual-accounting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-exception-residual-accounting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-exception-residual-accounting.json \
  --generated-on 2026-07-02
```

Build the Rust async closure source-protocol audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.rust-async-closure.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.rust-async-closure.crates.json \
  --output target/rust-async-closure-source-protocol-audit.json \
  --generated-on 2026-07-01 \
  --include-zero-surface rust.async.closure
```

Build the Ruby Thread/Fiber runtime obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.ruby-thread-fiber-runtime.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.ruby-thread-fiber-runtime.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.ruby-thread-fiber-runtime.json \
  --generated-on 2026-07-01
```

Build the Ruby yield source-protocol audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.ruby-yield-source-protocol.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.ruby-yield-source-protocol.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.ruby-yield-source-protocol.json \
  --generated-on 2026-07-01

python3 scripts/query-regression-harness.py \
  --baseline-binary /tmp/nose-ruby-yield-main-target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref origin/main \
  --current-source-ref ruby-yield-source-protocol \
  --iterations 15 \
  --repo rubocop --repo rspec-core --repo sidekiq \
  --repo rack --repo fastlane --repo sinatra \
  --output target/ruby-yield-source-protocol-query-regression.json
```

Build the Ruby exception-channel reporting audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.ruby-exception-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.ruby-exception-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.ruby-exception-reporting.json \
  --generated-on 2026-07-02

python3 scripts/query-regression-harness.py \
  --baseline-binary /tmp/nose-ruby-exception-main/target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref origin/main \
  --current-source-ref ruby-exception-reporting-alignment \
  --current-source-sha 8bb0ed37d13d98bbb94f29fc23cc19163d8d52e3 \
  --iterations 15 \
  --repo rubocop --repo rspec-core --repo sidekiq \
  --repo rack --repo fastlane --repo sinatra \
  --output target/ruby-exception-reporting-query-regression.json
```

Build the Java `CompletableFuture`/FutureLike obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-completablefuture.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --output target/scheduling-lifecycle-boundary-audit.java-completablefuture.json \
  --generated-on 2026-06-30 \
  --recall-loss-report target/recall-loss.java-completablefuture.crates.json
```

Build the Java `CompletableFuture` constructor reporting audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-completablefuture-constructor.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --generated-on 2026-07-02 \
  --recall-loss-report target/recall-loss.java-completablefuture-constructor.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.java-completablefuture-constructor.json

python3 scripts/query-regression-harness.py \
  --baseline-binary /tmp/nose-java-cf-main-worktree/target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref origin/main \
  --current-source-ref java-completablefuture-constructor-reporting \
  --iterations 9 \
  --repo netty --repo rxjava --repo retrofit \
  --repo junit5 --repo jedis --repo h2database \
  --output target/java-completablefuture-constructor-query-regression.json
```

Build the Java `Executor`/`Future` receiver-method obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-executor-future-runtime.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.java-executor-future-runtime.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.java-executor-future.json \
  --generated-on 2026-07-01
```

Build the Java `Executor`/`Future` local and explicit `this.<field>` receiver
audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-local-this-field-executor-future.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.java-local-this-field-executor-future.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.java-local-this-field-executor-future.json \
  --generated-on 2026-07-01
```

Build the Java `Executor`/`Future` wildcard-import receiver audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-wildcard-executor-future.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.java-wildcard-executor-future.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.java-wildcard-executor-future.json \
  --generated-on 2026-07-01
```

Build the Java Future/Executor residual-accounting audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.java-future-residual-accounting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.java-future-residual-accounting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.java-future-residual-accounting.json \
  --generated-on 2026-07-02
```

Build the Go channel/goroutine/defer obligation audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.go-channel-protocol.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.go-channel-protocol.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.go-channel-protocol.json \
  --generated-on 2026-06-30
```

Build the Go protocol reporting-support audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.go-protocol-reporting-support.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.go-protocol-reporting-support.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.go-protocol-reporting-support.json \
  --generated-on 2026-07-01

python3 scripts/query-regression-harness.py \
  --baseline-binary /tmp/nose-go-protocol-main-target/release/nose \
  --current-binary target/release/nose \
  --baseline-source-ref origin/main \
  --current-source-ref go-protocol-reporting-support \
  --repo nats-server --repo etcd --repo minio \
  --repo prometheus --repo badger --repo delve \
  --output target/go-protocol-reporting-support-query-regression.json
```

Build the non-JS task-spawn reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.non-js-task-spawn-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.non-js-task-spawn-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.non-js-task-spawn-reporting.json \
  --generated-on 2026-07-01
```

Build the non-JS async aggregate reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.non-js-async-aggregate-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.non-js-async-aggregate-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.non-js-async-aggregate-reporting.json \
  --generated-on 2026-07-01
```

Build the non-JS source-protocol reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.non-js-source-protocol-alignment.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.non-js-source-protocol-alignment.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.non-js-source-protocol-alignment.json \
  --generated-on 2026-07-02
```

Build the Python generator-yield reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.python-generator-yield-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.python-generator-yield-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.python-generator-yield-reporting.json \
  --generated-on 2026-07-02
```

Build the Python `asyncio.sleep` reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.python-asyncio-sleep-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.python-asyncio-sleep-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.python-asyncio-sleep-reporting.json \
  --generated-on 2026-07-02
```

Build the Swift await and Java settled-factory reporting alignment audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.swift-await-java-factory-reporting.crates.json

python3 scripts/scheduling-lifecycle-boundary-audit.py \
  --recall-loss-report target/recall-loss.swift-await-java-factory-reporting.crates.json \
  --output target/scheduling-lifecycle-boundary-audit.swift-await-java-factory-reporting.json \
  --generated-on 2026-07-02
```

Build the first #602 `Promise.all` exact aggregate slice audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-promise-all.crates.json

python3 scripts/promise-all-aggregate-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-promise-all.crates.json \
  --output target/promise-all-literal-aggregate-recovery.v1.json
```

Build the #602 `Promise.allSettled` exact aggregate slice audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-promise-allsettled.crates.json

python3 scripts/promise-allsettled-aggregate-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-promise-allsettled.crates.json \
  --output target/promise-allsettled-literal-aggregate-recovery.v1.json
```

Build the #602 Promise aggregate raw-input assimilation slice audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-promise-aggregate-raw-input.crates.json

python3 scripts/promise-aggregate-raw-input-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-promise-aggregate-raw-input.crates.json \
  --output target/promise-aggregate-raw-input-recovery.v1.json
```

Build the #602 `Promise.race`/`Promise.any` first-observed aggregate slice audit
with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-promise-race-any.crates.json

python3 scripts/promise-race-any-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-promise-race-any.crates.json \
  --output target/promise-race-any-literal-aggregate-recovery.v1.json
```

Build the #602 `new Promise(...)` executor boundary audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-promise-executor.crates.json

python3 scripts/promise-executor-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-promise-executor.crates.json \
  --output target/promise-executor-boundary-audit.v1.json
```

Build the #602 AbortSignal cancellation boundary audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-abort-signal.crates.json

python3 scripts/abort-signal-cancellation-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-abort-signal.crates.json \
  --output target/abort-signal-cancellation-boundary-audit.v1.json
```

Build the #602 interval/scheduler lifecycle boundary audit with:

```sh
cargo run -q -p nose-cli -- verify crates \
  --max-violations 0 \
  --recall-loss-report target/recall-loss.issue-602-interval-scheduler.crates.json

python3 scripts/interval-scheduler-lifecycle-slice-audit.py \
  --recall-loss-report target/recall-loss.issue-602-interval-scheduler.crates.json \
  --output target/interval-scheduler-lifecycle-boundary-audit.v1.json
```

## Files

- [crates.baseline.v1.json](crates.baseline.v1.json) records the current
  `crates` baseline and the #570 attribution improvement from the first coarse
  #569 baseline.
- [corpus-slice.baseline.v1.json](corpus-slice.baseline.v1.json) records a
  small mixed-language corpus slice across Go, Python, Ruby, TypeScript, Rust,
  and Swift.
- [stdlib-support-slices-2026-06-28.v1.json](stdlib-support-slices-2026-06-28.v1.json)
  records the staged corpus-priority stdlib support slices, including focused
  fixtures, unsupported boundaries, and per-slice performance checks.
- [java-arrays-collections-audit-2026-06-28.v1.json](java-arrays-collections-audit-2026-06-28.v1.json)
  records method-level corpus prevalence and support/boundary classification
  for Java `Arrays` and `Collections`.
- [go-stdlib-collections-audit-2026-06-28.v1.json](go-stdlib-collections-audit-2026-06-28.v1.json)
  records alias-aware corpus prevalence and support/boundary classification for
  Go `sort`, `slices`, and `maps`.
- [js-ts-stdlib-partial-audit-2026-06-28.v1.json](js-ts-stdlib-partial-audit-2026-06-28.v1.json)
  records comment/string-masked JS/TS builtin prevalence across Array HOFs,
  Map/Set receiver selectors, Object key/reflection APIs, Promise/async
  protocol surfaces, mutation/effect methods, and simple receiver hints. The
  only 5,000+ group is the closed Promise/async protocol boundary.
- [python-hof-runtime-audit-2026-06-28.v1.json](python-hof-runtime-audit-2026-06-28.v1.json)
  records AST-based Python builtin/HOF, `itertools`, and `functools`
  attribution and boundary classification.
- [python-hof-runtime-audit-2026-06-28.v2.json](python-hof-runtime-audit-2026-06-28.v2.json)
  refines the Python HOF/runtime audit with call-shape counts, top repos,
  broader lexical binding collection, and ranked next-work groups for
  materializer domain proof, ordering semantics, HOF callback proof,
  `itertools` lifecycle, callable runtime identity, combinatoric iterators, and
  runtime attribution.
- [python-hof-runtime-audit-2026-06-28.v3.json](python-hof-runtime-audit-2026-06-28.v3.json)
  records the first high-volume candidate processing decision: the
  `8,432`-occurrence Python materializer-domain group is split into strict
  materializer subgroups without widening semantic admission.
- [rust-stdlib-partial-audit-2026-06-28.v1.json](rust-stdlib-partial-audit-2026-06-28.v1.json)
  records comment/string-masked lexical prevalence and support/boundary
  classification for Rust `Option`, `Result`, iterator adapters/HOFs, `Vec`,
  membership/map lookup, `std::collections` factories, mutation, ordering, and
  allocation/lifetime surfaces.
- [rust-stdlib-partial-audit-2026-06-28.v2.json](rust-stdlib-partial-audit-2026-06-28.v2.json)
  aligns the Rust audit with existing generic method/effect contracts and
  records high-volume processing decisions for iterator-domain, Option/Result
  channel, HOF callback, iterator-view lifecycle, and mutation/effect groups.
  Unsupported attribution moves `12,232 -> 3,949` while semantic admission stays
  closed and `sort_by_key` gains receiver-mutation evidence.
- [swift-stdlib-partial-audit-2026-06-28.v1.json](swift-stdlib-partial-audit-2026-06-28.v1.json)
  records comment/string-masked lexical prevalence and support/boundary
  classification for Swift cardinality properties, collection/map factories,
  sequence HOFs, membership, sequence views, mutation/effect, ordering, and
  reduction surfaces.
- [swift-stdlib-partial-audit-2026-06-28.v2.json](swift-stdlib-partial-audit-2026-06-28.v2.json)
  records the high-volume Swift cardinality processing decision: `count` and
  `isEmpty` remain on existing `ExactCollection` receiver-gated property and
  method contracts.
- [cross-language-boundary-census-594-2026-06-28.v1.json](cross-language-boundary-census-594-2026-06-28.v1.json)
  records the #594 starting census for scheduling, success/error channel, and
  callback demand/effect obligations across JS/TS, Python, Rust, Go, Java,
  Swift, Ruby, and C. It combines existing language audits with conservative
  Ruby/C lexical pricing and keeps `semantic_admission_delta = 0`.
- [issue-597-obligation-taxonomy-2026-06-28.v1.json](issue-597-obligation-taxonomy-2026-06-28.v1.json)
  records the first `--recall-loss-report` obligation-family rollup for #597:
  broad admission reasons now also expose diagnostics-only
  `obligation_family` and `obligation_subreason` fields while preserving the
  hard gate.
- [issue-598-hard-negative-inventory-2026-06-28.v1.json](issue-598-hard-negative-inventory-2026-06-28.v1.json)
  maps existing cross-language hard negatives to the #594 obligation vocabulary
  before producer/admission work starts. It considers JS/TS, Python, Rust, Go,
  Java, Swift, Ruby, and C, records no omitted languages, and keeps new exact
  admissions at zero.
- [issue-599-callback-obligation-slice-2026-06-28.v1.json](issue-599-callback-obligation-slice-2026-06-28.v1.json)
  records the first callback demand/effect reporting slice: `18,637`
  callback-shaped source-prevalence occurrences and `30` `crates`
  callback-demand/effect recall-loss rows, with no exact admission changes.
- [callback-demand-effect-diagnostics-2026-06-28.v1.json](callback-demand-effect-diagnostics-2026-06-28.v1.json)
  records the first post-#594 callback diagnostics refinement. The broad
  `hof-demand-effect-profile-missing` rollup stays at `30`, but its
  `by_obligation` subreasons split into callback-effect proof (`27`),
  callback identity/shape proof (`2`), and predicate callback profile (`1`)
  without opening exact admission.
- [callback-demand-effect-diagnostics-2026-06-28.v2.json](callback-demand-effect-diagnostics-2026-06-28.v2.json)
  refines the callback-effect proof bucket into concrete producer obligations:
  callback call effects (`22`), callback assignment effects (`5`),
  callback identity/shape (`2`), and predicate callback profile (`1`).
  Runtime-boundary callback effects are priced at `0` on the current `crates`
  surface, and exact admission remains closed.
- [callback-demand-effect-diagnostics-2026-06-28.v3.json](callback-demand-effect-diagnostics-2026-06-28.v3.json)
  refines callback call-effect proof into producer-facing call shapes: member
  call proof (`10`), Rust macro call proof (`8`), direct-function effect
  contracts (`3`), and imported-function effect contracts (`1`). Assignment
  effects (`5`), callback identity/shape (`2`), and predicate callback profile
  (`1`) remain separate; exact admission still records
  `semantic_admission_delta = 0`.
- [issue-600-channel-scheduling-obligation-slice-2026-06-28.v1.json](issue-600-channel-scheduling-obligation-slice-2026-06-28.v1.json)
  records the first channel/scheduling reporting slice: `95,805`
  channel/scheduling-shaped source-prevalence occurrences and `14` `crates`
  scheduling-boundary recall-loss rows, with Promise/Future/async/channel
  admission still closed.
- [promise-protocol-diagnostics-2026-06-28.v1.json](promise-protocol-diagnostics-2026-06-28.v1.json)
  records the Promise/async diagnostics split. The JS/TS source-prevalence group
  stays at `29,094` occurrences, but runtime-boundary report labels now separate
  await scheduling, async function scheduling, Promise executor callbacks,
  Promise factories, aggregate result channels, rejection channels, and
  non-construct Promise calls without opening exact admission.
  Current reports use the language-neutral `async-await-scheduling-contract`
  label for the shared `Source::Protocol(Await)` boundary; this historical
  artifact may still contain the older Promise-specific await label.
- [cross-language-await-obligation-reporting-2026-06-30.v1.json](cross-language-await-obligation-reporting-2026-06-30.v1.json)
  records the post-#602 reporting-only label migration: JS/TS, Python, Rust,
  and Swift `await` protocol boundaries now share
  `async-await-scheduling-contract`; exact admission remains closed with
  `semantic_admission_delta = 0`.
- [oracle-exclusion-obligation-reporting-2026-06-30.v1.json](oracle-exclusion-obligation-reporting-2026-06-30.v1.json)
  records the follow-up recall-loss report-shape refinement: runtime/protocol
  oracle exclusions can now carry diagnostics-only obligation attribution under
  `oracle_exclusions.by_obligation`. A focused fixture verifies JS/TS, Python,
  Rust, and Swift await exclusions roll up to
  `scheduling-boundary/async-await-scheduling-contract-missing`, while
  top-level `by_obligation` stays interpretable-only.
- [cross-language-async-function-obligation-reporting-2026-06-30.v1.json](cross-language-async-function-obligation-reporting-2026-06-30.v1.json)
  records the next reporting-only migration: JS/TS, Python, Rust, and Swift
  runtime-body async functions now share
  `async-function-scheduling-contract`, and Rust async blocks use
  `async-block-scheduling-contract`. Exact admission remains closed, and
  Promise-specific async-function return producer proof remains JS/TS-only.
- [scheduling-lifecycle-boundary-audit-async-function-obligation-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-async-function-obligation-2026-06-30.v1.json)
  records the 120-repo lexical pricing after the async function/block vocabulary
  migration. It splits Rust `async fn` from Rust async blocks and aligns JS/TS,
  Python, Rust, and Swift async function rows to the shared scheduling
  subreason. Its `current_recall_loss` section carries the matching crates gate
  summary plus separate interpretable and oracle-exclusion obligation rollups.
- [non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json](non-js-async-runtime-api-obligation-reporting-2026-06-30.v1.json)
  records the next reporting-only runtime API migration. Python `asyncio`
  task/timer/aggregate calls, Rust async task spawn plus qualified
  `tokio`/`futures`/`futures_util` `join!`/`select!` macros, and Swift `Task`
  creation now report shared `task-*` and `async-aggregate-*` obligations
  without opening exact admission.
- [scheduling-lifecycle-boundary-audit-non-js-async-runtime-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-non-js-async-runtime-2026-06-30.v1.json)
  records the 120-repo lexical pricing for that runtime API slice: Rust
  `tokio`/`async-std` spawn (`349` occurrences / `3` repos), Swift `Task`
  (`210` / `12`), Python `asyncio.sleep` (`104` / `6`), qualified Rust
  `tokio`/`futures`/`futures_util` `join!`/`try_join!` (`68` / `2`), Python
  `asyncio.gather` (`17` / `4`), Python `asyncio.create_task`/`ensure_future`
  (`14` / `3`), qualified Rust `tokio`/`futures`/`futures_util` `select!`
  (`5` / `1`), and Python `asyncio.wait` (`4` / `3`).
- [non-js-async-runtime-attribution-hardening-2026-06-30.v1.json](non-js-async-runtime-attribution-hardening-2026-06-30.v1.json)
  records the follow-up reporting-only safety hardening. Python `asyncio.*`
  reporting now requires import-backed namespace evidence and no path-visible
  local `asyncio` module, Rust spawn and aggregate reporting require qualified
  `tokio`/`async_std`/`futures`/`futures_util` paths whose root is not locally
  defined in the same file, and Swift `Task` reporting requires an unshadowed
  `Task` root with no corpus-visible Swift `Task` definition. Exact admission
  stays closed and the crates gate remains at `0` false merges.
- [non-js-async-runtime-import-proof-2026-06-30.v1.json](non-js-async-runtime-import-proof-2026-06-30.v1.json)
  records the next reporting-only import-proof expansion. Python `asyncio`
  namespace aliases such as `import asyncio as aio; aio.create_task(...)` and
  Rust imported runtime bindings such as `use tokio::spawn; spawn(...)` now use
  existing import/symbol evidence before receiving the same shared obligations.
  Exact admission stays closed.
- [scheduling-lifecycle-boundary-audit-non-js-async-runtime-import-proof-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-non-js-async-runtime-import-proof-2026-06-30.v1.json)
  records the matching 120-repo source-prevalence pricing after alias/imported
  binding support. The new surfaces add `11` priced occurrences over the prior
  qualified-only audit: Rust imported spawn (`3` / `1` repo) and Rust imported
  `join!`/`try_join!` (`8` / `1` repo). Python `asyncio` alias and Rust
  imported `select!` support are exercised by fixtures but have `0` occurrences
  in the pinned corpus.
- [non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json](non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
  records the next reporting-only imported-binding expansion. Python
  `from asyncio import create_task`/`sleep`/`gather`/`wait` bindings and Rust
  brace imports such as `use tokio::{spawn}` or
  `use futures::{select as fut_select}` now reuse existing `ImportedBinding`
  proof before receiving shared task/timer/aggregate obligations. Imported
  occurrence evidence is scoped at the producer, so Rust block-scoped or
  parent-module `use` bindings do not prove out-of-scope unqualified calls.
  Exact admission stays closed.
- [scheduling-lifecycle-boundary-audit-non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-non-js-async-runtime-imported-binding-proof-2026-06-30.v1.json)
  records the matching 120-repo source-prevalence pricing. It raises total
  source prevalence from `142,845` to `142,847` by adding `2` Python
  `from asyncio import sleep` occurrences in `1` repo. Rust imported rows remain
  `11` priced occurrences because the prior audit already counted direct and
  brace `use` spellings; this slice makes brace evidence-only imports
  actionable in admission.
- [swift-structured-concurrency-obligation-reporting-2026-06-30.v1.json](swift-structured-concurrency-obligation-reporting-2026-06-30.v1.json)
  records the Swift structured-concurrency reporting-only expansion.
  `Task.sleep`, `Task.yield`, and task-group calls now receive shared timer,
  task-yield, aggregate, cancellation/liveness, result-channel, and
  exception-channel obligations without opening exact admission.
- [rust-block-on-future-drive-obligation-reporting-2026-07-01.v1.json](rust-block-on-future-drive-obligation-reporting-2026-07-01.v1.json)
  records the Rust Future bridge reporting-only expansion.
  Qualified/import-backed `tokio_test::block_on` calls and proof-backed
  `Handle::current().block_on` plus inline `Runtime`/`Builder` receiver chains
  now receive
  `future-drive-scheduling-contract` and
  `future-settled-value-channel-contract` without opening exact admission.
  In that slice, selector-only `.block_on` and unproven variable/field/parameter
  receivers remained closed.
- [rust-block-on-local-runtime-provenance-2026-07-01.v1.json](rust-block-on-local-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust local receiver-provenance expansion. Local
  variables whose last visible assignment is proof-backed `Handle::current()`,
  `Runtime::new().unwrap()/expect`, direct `Runtime::new()?`, or
  `Builder::new_*().build().unwrap()/expect/?` now receive the same
  future-drive and future-settled obligations. Exact admission stays closed. In
  that slice, function parameters, struct fields, wrapper calls, and
  `map_err(...)?` construction remained closed.
- [rust-block-on-parameter-runtime-provenance-2026-07-01.v1.json](rust-block-on-parameter-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust parameter receiver-provenance expansion. Nominal
  `tokio::runtime::Runtime` and `tokio::runtime::Handle` parameters now receive
  the same future-drive and future-settled obligations when the type is fully
  qualified or backed by scope-visible exact imported-binding evidence. Exact
  admission stays closed. In that slice, struct fields, nested brace-import
  parameter types, child-module parameters with only parent-module imports, type
  aliases, wrapper calls, and `map_err(...)?` construction remained closed.
- [rust-nested-brace-runtime-provenance-2026-07-01.v1.json](rust-nested-brace-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust nested static brace-import expansion. Nested items
  such as `use tokio::{runtime::{Runtime}}` now emit per-item import evidence,
  allowing those `Runtime` parameters to receive the same future-drive and
  future-settled obligations. Exact admission stays closed; struct fields,
  wildcard/relative imports, type aliases, wrapper calls, and `map_err(...)?`
  construction remain closed.
- [rust-self-field-runtime-provenance-2026-07-01.v1.json](rust-self-field-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust self-field receiver-provenance expansion. Exact
  `self.<field>.block_on(...)` receivers now receive future-drive and
  future-settled obligations when the enclosing impl method has a self
  parameter and a same-scope struct field declaration proves
  `tokio::runtime::Runtime` or `Handle` through fully qualified or exact
  imported-binding type evidence. Exact admission stays closed; non-self
  fields, local struct fields, project-local `tokio` roots or aliases,
  wildcard/relative imports, type aliases, wrapper calls, constructor-assigned
  fields, and `map_err(...)?` construction remained closed in that slice.
- [rust-local-self-field-runtime-provenance-2026-07-01.v1.json](rust-local-self-field-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust local self-field receiver-provenance expansion.
  Function/block-local `struct Runner { rt: Runtime }` plus local
  `impl Runner { fn run(&self) { self.rt.block_on(...) } }` now receive the
  same future-drive and future-settled obligations when the local declaration
  scope or enclosing module scope has exact tokio runtime import/type proof. The Tokio
  `task_local_set.rs` spot check moves the `self.rt.block_on(...)` row from
  `receiver-mutation/effect-preserving-contract-missing` to
  `scheduling-boundary/future-drive-scheduling-contract-missing` with `0` false
  merges. Exact admission stays closed; duplicate local structs, wrong local
  `Runtime` imports, same-scope `Runtime` types, namespace aliases named
  `tokio`, non-self fields, type aliases, wrapper calls, and
  constructor-assigned fields remain closed.
- [rust-block-on-builder-config-runtime-provenance-2026-07-01.v1.json](rust-block-on-builder-config-runtime-provenance-2026-07-01.v1.json)
  records the follow-up Rust Builder configuration receiver-provenance
  expansion. Receiver-preserving Tokio Builder methods `start_paused`,
  `unhandled_panic`, `thread_keep_alive`, `global_queue_interval`,
  `event_interval`, and `disable_lifo_slot` now preserve proof-backed Builder
  identity before `build().unwrap()/expect/?` exposes the runtime receiver. The
  pinned corpus has `34` such config-method occurrences across `15` files in
  `tokio`; representative Tokio spot checks move future-drive evidence units
  from `6` to `8` with `0` false merges. Exact admission stays closed; Builder
  callback hooks, `thread_name_fn`, constructor-assigned fields, and
  block_on/await convergence remain closed.
- [scheduling-lifecycle-boundary-audit-swift-structured-concurrency-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-swift-structured-concurrency-2026-06-30.v1.json)
  records the matching 120-repo source-prevalence pricing. It raises total
  source prevalence from `142,847` to `143,178`: `Task.sleep` contributes
  `161` occurrences in `10` repos, task groups contribute `153` in `9` repos,
  `Task.yield` contributes `12` in `3` repos, and the audit now also counts `5`
  already-supported `Task.detached(...)` spawn occurrences.
- [java-completablefuture-obligation-reporting-2026-06-30.v1.json](java-completablefuture-obligation-reporting-2026-06-30.v1.json)
  records the Java Future reporting-only expansion. Proof-backed
  `CompletableFuture` static calls and exact-import-backed CompletionStage-style
  receiver continuations now receive shared future, task, aggregate, callback,
  and exception obligations without opening exact admission.
- [scheduling-lifecycle-boundary-audit-java-completablefuture-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-java-completablefuture-2026-06-30.v1.json)
  records the matching 120-repo source-prevalence pricing. It raises total
  source prevalence from `143,178` to `143,188` while splitting `40` lexical
  Java future reporting candidates out of the broad
  `CompletableFuture` bucket and leaving `276` broad mentions closed.
- [scheduling-lifecycle-boundary-audit-java-completablefuture-constructor-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-java-completablefuture-constructor-reporting-2026-07-02.v1.json)
  records the Java `CompletableFuture` constructor reporting split. It marks
  fully qualified or exact-/wildcard-import-backed `new CompletableFuture`
  calls reporting-supported, newly aligning `46` occurrences across `5` repos
  and reducing the residual broad `CompletableFuture` bucket from `276` to
  `230`.
- [java-completablefuture-constructor-reporting-2026-07-02.v1.json](java-completablefuture-constructor-reporting-2026-07-02.v1.json)
  records the compact closeout for the same slice, including the checked
  `crates` recall-loss gate (`0` false merges, `0` canon preservation
  violations) and Java reporting-supported totals after the split.
- [java-completablefuture-constructor-query-regression-2026-07-02.v1.json](java-completablefuture-constructor-query-regression-2026-07-02.v1.json)
  records the Java-heavy product query regression for the constructor slice.
  The 6-repo alternating r9 run kept product output hashes identical on every
  measured repo and measured aggregate median runtime at
  `7023.49ms -> 6991.92ms` (`-0.45%`).
- [scheduling-lifecycle-boundary-audit-non-js-source-protocol-alignment-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-non-js-source-protocol-alignment-2026-07-02.v1.json)
  records the non-JS async source-protocol reporting alignment. It marks the
  already runtime-boundary-backed Python `await`/`async def`, Rust
  `.await`/`async fn`/`async block`, and Swift `async` function rows
  reporting-supported while keeping exact admission closed.
- [non-js-source-protocol-reporting-alignment-2026-07-02.v1.json](non-js-source-protocol-reporting-alignment-2026-07-02.v1.json)
  records the compact closeout for the same slice. It newly aligns `19,144`
  source-prevalence occurrences, brings all reporting-supported
  closed-boundary rows to `70,491` occurrences across `57` rows, and keeps the
  checked `crates` gate at `0` false merges and `0` canon preservation
  violations.
- [scheduling-lifecycle-boundary-audit-python-generator-yield-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-python-generator-yield-reporting-2026-07-02.v1.json)
  records the Python generator-yield reporting alignment. It marks
  source-backed `yield` and `yield from` lifecycle/protocol boundaries
  reporting-supported while keeping exact admission closed.
- [python-generator-yield-reporting-2026-07-02.v1.json](python-generator-yield-reporting-2026-07-02.v1.json)
  records the compact closeout for the same slice. It newly aligns `2,404`
  Python generator-yield occurrences across `21` repos, bringing all
  reporting-supported closed-boundary rows to `90,875` occurrences across
  `60` rows while the checked `crates` gate remains at `0` false merges and
  `0` canon preservation violations.
- [scheduling-lifecycle-boundary-audit-python-asyncio-sleep-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-python-asyncio-sleep-reporting-2026-07-02.v1.json)
  records the direct Python `asyncio.sleep` reporting alignment. It marks the
  already runtime-boundary-backed timer row reporting-supported while keeping
  exact admission closed.
- [python-asyncio-sleep-reporting-2026-07-02.v1.json](python-asyncio-sleep-reporting-2026-07-02.v1.json)
  records the compact closeout for the same slice. It newly aligns `104`
  `asyncio.sleep` occurrences across `6` repos, bringing all
  reporting-supported closed-boundary rows to `90,979` occurrences across
  `61` rows and leaving no Python closed-boundary rows in the scheduling
  lifecycle audit.
- [scheduling-lifecycle-boundary-audit-java-wildcard-executor-future-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-java-wildcard-executor-future-2026-07-01.v1.json)
  records the Java `java.util.concurrent.*` receiver-domain follow-up. It keeps
  exact admission closed while allowing wildcard-derived import evidence to
  prove `Future`/`CompletableFuture` and executor receiver declarations when no
  local type or explicit same-name import conflicts. On the current
  `origin/main` baseline, Java reporting-supported receiver-method candidates
  rise from `858` to `1,093` (`+235`) across the pinned 120-repo corpus.
- [scheduling-lifecycle-boundary-audit-java-future-residual-accounting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-java-future-residual-accounting-2026-07-02.v1.json)
  records the Java Future/Executor residual-accounting alignment. It marks
  existing product-backed `FutureLike.handle/whenComplete` settlement
  continuations reporting-supported at `10` occurrences across `2` repos, and
  keeps the historical `Executor/Future` type-name bucket visible at `3,297`
  occurrences while marking it as a superseded overlap row.
- [java-future-residual-accounting-2026-07-02.v1.json](java-future-residual-accounting-2026-07-02.v1.json)
  records the compact closeout for the same correction. Reporting-supported
  totals move to `88,471` occurrences across `59` rows, and the largest
  actionable Java closed boundary becomes `stream/parallelStream` at `1,996`
  occurrences across `15` repos.
- [scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-go-channel-protocol-2026-06-30.v1.json)
  records the Go channel/goroutine/defer reporting-only refinement. It keeps
  exact admission closed while splitting Go protocol-node pricing into `4,294`
  channel receives, `1,525` sends, `155` comma-ok receives, `1,920` select
  parents, `3,590` select cases, `546` select defaults, `1,949` goroutines,
  and `17,521` defers. Select parents and arms are counted separately because
  they are distinct source-backed protocol boundaries in lowering.
- [scheduling-lifecycle-boundary-audit-go-protocol-reporting-support-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-go-protocol-reporting-support-2026-07-01.v1.json)
  records the Go protocol reporting-support follow-up. It keeps exact
  admission closed while marking the already source-backed Go channel,
  select, goroutine, and defer protocol rows reporting-supported. Go
  `go`/`defer` now also have scheduled/deferred callback demand/effect
  profiles in the shared kernel.
- [go-protocol-reporting-support-2026-07-01.v1.json](go-protocol-reporting-support-2026-07-01.v1.json)
  records the compact Go protocol closeout. It prices `31,500` Go protocol
  occurrences, keeps the checked `crates` gate at `0` false merges, and records
  a Go-heavy r9 query regression of `3560.13ms -> 3563.06ms` (`+0.08%`) with
  identical product hashes on all six measured repos.
- [scheduling-lifecycle-boundary-audit-non-js-task-spawn-reporting-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-non-js-task-spawn-reporting-2026-07-01.v1.json)
  records the non-JS task-spawn reporting alignment. It marks the already
  runtime-boundary-backed Rust `tokio`/`async-std` spawn, Swift
  `Task`/`Task.detached`, Python `asyncio.create_task`/`ensure_future`, and
  Java `CompletableFuture.supplyAsync`/`runAsync` rows reporting-supported
  while keeping exact admission closed.
- [non-js-task-spawn-reporting-alignment-2026-07-01.v1.json](non-js-task-spawn-reporting-alignment-2026-07-01.v1.json)
  records the compact closeout for the same slice. It newly aligns `590`
  source-prevalence occurrences, brings currently backed task-spawn
  reporting-supported rows to `1,123` occurrences, and keeps the checked
  `crates` gate at `0` false merges and `0` canon preservation violations.
- [scheduling-lifecycle-boundary-audit-non-js-async-aggregate-reporting-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-non-js-async-aggregate-reporting-2026-07-01.v1.json)
  records the non-JS async aggregate reporting alignment. It marks the already
  runtime-boundary-backed Rust `tokio`/`futures`/`futures_util`
  `join!`/`try_join!`/`select!`, Python `asyncio.gather`/`wait`, and Java
  `CompletableFuture.allOf`/`anyOf` rows reporting-supported while keeping
  exact admission closed.
- [non-js-async-aggregate-reporting-alignment-2026-07-01.v1.json](non-js-async-aggregate-reporting-alignment-2026-07-01.v1.json)
  records the compact closeout for the same slice. It newly aligns `98`
  source-prevalence occurrences, brings currently backed async-aggregate
  reporting-supported rows to `286` occurrences, and keeps the checked `crates`
  gate at `0` false merges and `0` canon preservation violations.
- [scheduling-lifecycle-boundary-audit-swift-await-java-factory-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-swift-await-java-factory-reporting-2026-07-02.v1.json)
  records the Swift await and Java settled-factory reporting alignment. It marks
  already source-protocol-backed Swift `await` and static-runtime-backed Java
  `CompletableFuture.completedFuture`/`failedFuture` rows reporting-supported
  while keeping exact admission closed. The broad Java `CompletableFuture`
  bucket and looser FutureLike settlement receiver bucket remain deferred.
- [swift-await-java-factory-reporting-alignment-2026-07-02.v1.json](swift-await-java-factory-reporting-alignment-2026-07-02.v1.json)
  records the compact closeout for the same slice. It newly aligns `8,703`
  source-prevalence occurrences, brings all reporting-supported
  closed-boundary rows to `51,301` occurrences across `50` rows, and keeps the
  checked `crates` gate at `0` false merges and `0` canon preservation
  violations.
- [scheduling-lifecycle-boundary-audit-python-async-lifecycle-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-python-async-lifecycle-2026-07-01.v1.json)
  records the Python async protocol lifecycle reporting slice. It keeps exact
  admission closed while splitting `async for` statements/comprehensions and
  `async with` into source-backed async iteration/context protocol boundaries.
  The 120-repo audit prices `114` `async for` and `361` `async with`
  occurrences across `5` repos, with `0` false merges and `0` canon
  preservation violations on the checked `crates` gate.
- [swift-async-iteration-protocol-reporting-2026-07-01.v1.json](swift-async-iteration-protocol-reporting-2026-07-01.v1.json)
  records the Swift async iteration reporting slice. It keeps exact admission
  closed while preserving `for await` and `for try await` as source-backed
  async iteration protocol boundaries. The 120-repo audit prices `193`
  occurrences across `11` repos, and representative Swift NIO, Composable
  Architecture, and Alamofire spot checks move async-iteration lifecycle
  evidence units from `0` to `31` with `0` false merges.
- [swift-async-task-source-protocol-2026-07-01.v1.json](swift-async-task-source-protocol-2026-07-01.v1.json)
  records the Swift async task source-protocol slice. It adds the internal
  `TaskSpawn` source protocol capability for `async let` and reuses
  `AsyncFunction` for Swift async closures. The 120-repo audit prices `100`
  async closures across `4` repos and `51` async-let bindings across `7` repos;
  Alamofire/Swift NIO/Vapor spot checks move `task_spawn` raw protocol tags
  from `0` to `36` and async-function tags from `110` to `139` with `0` false
  merges.
- [swift-throwing-callable-protocol-reporting-2026-07-01.v1.json](swift-throwing-callable-protocol-reporting-2026-07-01.v1.json)
  records the Swift throwing callable source-protocol slice. It reuses the
  existing `TryPropagation` source protocol for body-bearing plain and typed
  `throws`/`rethrows` functions and throwing closures, including async throwing
  callables, without opening exact admission. The 120-repo audit prices `7,008`
  throwing functions across `17` repos and `169` throwing closures across `6`
  repos; the broad `throws`/`try` bucket remains closed at `26,608`
  occurrences. The checked `crates` gate remains at `0` false merges and `0`
  canon preservation violations.
- [swift-throwing-callable-query-regression-2026-07-01.v1.json](swift-throwing-callable-query-regression-2026-07-01.v1.json)
  records the product query-regression evidence for the same Swift slice. It
  compares `origin/main@01ed07fc` with `swift-throwing-callable-protocol@0b105f87`
  on six Swift repos using `nose query <repo> all top=0 --mode semantic
  --format json`. The official sequential r15 compare had small-repo runtime
  investigation triggers but no output drift; the paired alternating r15 run
  measured `898.66ms -> 890.29ms` aggregate wall-clock medians (`-0.93%`) with
  identical product JSON hashes for every repo.
- [scheduling-lifecycle-boundary-audit-swift-try-expression-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-swift-try-expression-reporting-2026-07-02.v1.json)
  records the Swift try-expression reporting alignment. It marks source-backed
  `try`, `try?`, `try!`, and `for try await` propagation boundaries
  reporting-supported while keeping exact admission closed.
- [swift-try-expression-reporting-2026-07-02.v1.json](swift-try-expression-reporting-2026-07-02.v1.json)
  records the compact closeout for the same slice. It newly aligns `17,970`
  Swift try-expression occurrences across `18` repos, bringing all
  reporting-supported closed-boundary rows to `88,461` occurrences across
  `58` rows while the checked `crates` gate remains at `0` false merges and
  `0` canon preservation violations.
- [scheduling-lifecycle-boundary-audit-swift-exception-residual-accounting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-swift-exception-residual-accounting-2026-07-02.v1.json)
  records the Swift exception residual-accounting correction. The historical
  `throws/try` lexical bucket remains visible at `26,608` occurrences across
  `18` repos but is marked as a superseded overlap bucket rather than an
  actionable closed-boundary.
- [swift-exception-residual-accounting-2026-07-02.v1.json](swift-exception-residual-accounting-2026-07-02.v1.json)
  records the compact closeout for the same correction. Reporting-supported
  totals remain `88,461` occurrences across `58` rows; the measured change is
  that Ruby `raise/rescue` at `4,010` occurrences becomes the largest non-JS
  actionable closed exception-channel bucket.
- [rust-async-closure-source-protocol-2026-07-01.v1.json](rust-async-closure-source-protocol-2026-07-01.v1.json)
  records the Rust async closure source-protocol parity slice. It reuses
  `AsyncFunction` for `async |...|` and `async move |...|` closures without
  adding a Rust-only kernel feature or opening exact admission. The pinned
  120-repo corpus has `0` Rust async closure occurrences, while the same audit
  now distinguishes those closures from `1,342` Rust async blocks across
  `4` repos. The checked `crates` gate remains at `0` false merges and `0`
  canon preservation violations.
- [ruby-thread-fiber-runtime-reporting-2026-07-01.v1.json](ruby-thread-fiber-runtime-reporting-2026-07-01.v1.json)
  records the Ruby Thread/Fiber reporting-only expansion. `Thread.new`,
  `Thread.start`, `Thread.fork`, `Fiber.new`, and `Fiber.schedule` now reuse
  shared task-spawn, task-handle, cancellation/liveness, and concurrency
  scheduling obligations when the runtime root is not defined in the same file.
- [scheduling-lifecycle-boundary-audit-ruby-thread-fiber-runtime-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-ruby-thread-fiber-runtime-2026-07-01.v1.json)
  records the matching 120-repo source-prevalence pricing. It raises total
  source prevalence from `146,987` to `146,988`, marks the Ruby Thread/Fiber row
  reporting-supported, and prices `74` occurrences across `11` repos.
- [ruby-yield-source-protocol-reporting-2026-07-01.v1.json](ruby-yield-source-protocol-reporting-2026-07-01.v1.json)
  records the Ruby block-yield reporting-only expansion. Ruby `yield` now
  preserves a source-backed `BlockYield` protocol boundary and reports callback
  demand/effect obligations without opening exact admission or widening
  generator-yield semantics.
- [scheduling-lifecycle-boundary-audit-ruby-yield-source-protocol-2026-07-01.v1.json](scheduling-lifecycle-boundary-audit-ruby-yield-source-protocol-2026-07-01.v1.json)
  records the matching 120-repo pricing: `801` Ruby yield occurrences across
  `17` repos are reporting-supported closed boundaries.
- [ruby-yield-source-protocol-query-regression-2026-07-01.v1.json](ruby-yield-source-protocol-query-regression-2026-07-01.v1.json)
  records the Ruby-heavy product query regression. The 6-repo alternating r15
  aggregate median was `2504.55ms -> 2574.79ms` (`+2.80%`). `rspec-core`
  changed one same-location 3-member HTML family's representative label from
  `pre` to `code` while keeping the family count stable.
- [scheduling-lifecycle-boundary-audit-ruby-exception-reporting-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-ruby-exception-reporting-2026-07-02.v1.json)
  records the Ruby exception-channel reporting audit. It prices `2,065`
  source-backed unqualified `raise` occurrences and `1,933` source-backed
  `rescue` occurrences as reporting-supported closed-boundaries, while the
  broad `4,010`-occurrence `raise/rescue` overlap bucket remains visible as
  superseded. The 12 broad-only occurrences are receiver-qualified `.raise`
  overlaps that stay outside the concrete reporting-supported rows.
- [ruby-exception-reporting-2026-07-02.v1.json](ruby-exception-reporting-2026-07-02.v1.json)
  records the compact closeout for the same slice. Reporting-supported
  closed-boundary rows rise to `94,977` occurrences across `63` rows, exact
  admission remains closed, and Java Stream lifecycle becomes the largest
  non-JS actionable closed boundary at `1,996` occurrences.
- [ruby-exception-reporting-query-regression-2026-07-02.v1.json](ruby-exception-reporting-query-regression-2026-07-02.v1.json)
  records the Ruby-heavy product query regression. The 6-repo alternating r15
  aggregate median was `3295.78ms -> 3330.24ms` (`+1.05%`). Family counts stay
  stable across all 6 repos. Output hashes stay identical on `fastlane`,
  `rack`, `sidekiq`, and `sinatra`; `rubocop` changes only
  `value_nodes`/`mean_sem` metadata on one Ruby helper family, and `rspec-core`
  changes one stable-count HTML hidden family's representative from `code` to
  `pre` with `static-attrs-only` origin evidence.
- [scheduling-lifecycle-boundary-audit-java-stream-lifecycle-split-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-java-stream-lifecycle-split-2026-07-02.v1.json)
  records the Java stream lifecycle audit split. It separates existing
  proof-backed `receiver.stream()` adapter occurrences (`372` across `10`
  repos) and `Arrays.stream(xs)` adapter occurrences (`128` across `12` repos)
  from the broad stream lifecycle bucket. The residual
  `stream/parallelStream` closed-boundary row falls from `1,996` to `1,496`
  occurrences across `14` repos.
- [java-stream-lifecycle-split-2026-07-02.v1.json](java-stream-lifecycle-split-2026-07-02.v1.json)
  records the compact closeout for the same accounting split. No product
  admission code changed; the artifact documents existing exact-supported
  iterator identity/static collection adapter capability and keeps untyped
  stream plus `parallelStream` lifecycle semantics closed for later proof.
- [scheduling-lifecycle-boundary-audit-java-completablefuture-receiver-split-2026-07-02.v1.json](scheduling-lifecycle-boundary-audit-java-completablefuture-receiver-split-2026-07-02.v1.json)
  records the Java `CompletableFuture` receiver split. It moves
  scope-proven `complete`/`completeExceptionally` receiver settlement methods
  (`45` across `2` repos) and `join`/`getNow`/`isCompletedExceptionally`
  observation methods (`45` across `1` repo) to reporting-supported
  closed-boundary rows, while the remaining `230` broad type/reference mentions
  become a superseded overlap row instead of an actionable residual. Same-name
  receivers in other scopes, custom imports, lambda parameters, and unsupported
  arities remain closed.
- [java-completablefuture-receiver-split-2026-07-02.v1.json](java-completablefuture-receiver-split-2026-07-02.v1.json)
  records the compact closeout for the same slice, and
  [java-completablefuture-receiver-split-query-regression-2026-07-02.v1.json](java-completablefuture-receiver-split-query-regression-2026-07-02.v1.json)
  records the Java-heavy r9 product query regression. All six measured repos
  kept identical output hashes and family counts; aggregate median runtime was
  `8118.22ms -> 8151.13ms` (`+0.41%`).
- [issue-655-hard-negative-matrix-2026-07-02.v1.json](issue-655-hard-negative-matrix-2026-07-02.v1.json)
  records the #653/#655 async/scheduling hard-negative matrix before fixture
  expansion. It audits `82` scoped surface rows across JS/TS, Go, Swift, Rust,
  Java, Python, and Ruby, including `65` reporting-supported closed-boundary
  rows, plus `11` supplemental JS/TS Promise continuation/rejection reporting
  and timer/scheduler priced surfaces; prices `177,789` scoped
  source-prevalence occurrences including those supplemental rows; maps `18`
  positive fixture groups, `27` hard-negative fixture groups, and `25`
  reporting artifact evidence groups; and enumerates `48` missing
  hard-negative classes for #657.
  This hand-curated inventory records its source-audit regenerate and validation
  commands, and keeps `semantic_admission_delta = 0`.
- [issue-657-hard-negative-fixtures-2026-07-02.v1.json](issue-657-hard-negative-fixtures-2026-07-02.v1.json)
  records the #653/#657 executable hard-negative fixture expansion. It adds one
  cross-language fixture group with `8` test symbols and `54` fail-closed
  assertions across JS/TS, Go, Python, Rust, Java, Swift, and Ruby. The suite
  pins Promise continuation/channel, executor, timer/microtask/cancellation,
  Go channel/select/goroutine/defer, Python asyncio/protocol, Rust Future-drive,
  Java Future/Executor/Stream, Swift task/async/try/continuation, and Ruby
  thread/fiber/yield/exception boundaries without opening exact admission.
  The checked artifact maps all `48` #655 matrix classes to explicit evidence
  status: `40` now have direct evidence in this new executable suite, `8` rely
  only on existing executable/reporting evidence, and, across those mapped
  statuses, `14` remain flagged for more granular future executable follow-up
  before broader exact admission. It keeps `semantic_admission_delta = 0`.
- [issue-654-semantic-kernel-capability-audit-2026-07-02.v1.json](issue-654-semantic-kernel-capability-audit-2026-07-02.v1.json)
  records the #653/#654 semantic-kernel capability vocabulary audit. It checks
  `15` capability groups, `12` evidence kinds, `9` source fact kinds, `16`
  source protocol kinds, and `98` runtime-boundary obligation rules. The audit
  accepts the current capability groups, preserves `4` legacy aliases for old
  artifact readability, identifies `6` duplicate/merge candidates and `8`
  feature-shaped Promise diagnostics for #656 follow-up, and keeps
  `public_api_expansion = 0` and `semantic_admission_delta = 0`.
- [issue-656-obligation-label-docs-cleanup-2026-07-02.v1.json](issue-656-obligation-label-docs-cleanup-2026-07-02.v1.json)
  records the #656 non-behavioral label/docs cleanup from the #654 audit. It
  documents `3` shared scheduling labels as the canonical wording for new docs
  and reports, preserves `4` compatibility aliases for old artifacts, groups
  `8` feature-shaped Promise diagnostics under their reusable capability
  blockers, documents `6` duplicate/grouping decisions, and keeps
  historical artifacts untouched with `semantic_admission_delta = 0`.
- [scheduling-lifecycle-boundary-audit-non-js-async-runtime-scope-shadowing-2026-06-30.v1.json](scheduling-lifecycle-boundary-audit-non-js-async-runtime-scope-shadowing-2026-06-30.v1.json)
  records the Python/Rust async runtime scope-shadowing hardening. It keeps
  exact admission closed while making unrelated local shadows in other
  functions stop suppressing import-backed `asyncio` and Rust runtime
  reporting. Same-scope, enclosing-scope, and module-level shadows remain
  closed. The 120-repo pricing total stays unchanged at `146,880`, so this is a
  safety/precision improvement for report attribution rather than a new
  corpus-prevalence slice. Python/Rust async-runtime diagnostics use
  source-preserving unit roots before normalized fallback so alpha-renamed
  oracle units keep the same alias-shadow boundary.
- [promise-protocol-hard-negatives-2026-06-28.v1.json](promise-protocol-hard-negatives-2026-06-28.v1.json)
  records the follow-up Promise hard-negative slice. It keeps exact admission
  closed while pinning async-function/sync, Promise executor/sync,
  Promise.resolve/sync, Promise.then/custom receiver, thenable assimilation, and
  Promise.all/Promise.race boundaries with `4` new tests and `8` fail-closed
  assertions.
- [promise-resolve-recovery-2026-06-28.v1.json](promise-resolve-recovery-2026-06-28.v1.json)
  records the first narrow Promise recovery slice. Dependency-closed
  `Promise.resolve(value)` can now enter exact semantic families only when the
  receiver is proven as the global `Promise.resolve` and the argument is a
  non-thenable-safe literal/nullish/scalar value; sync payloads, possible
  thenables, explicit PromiseLike values, custom receivers, executors,
  aggregate channels, and rejection channels remain closed.
- [promise-rejection-continuation-diagnostics-2026-06-28.v1.json](promise-rejection-continuation-diagnostics-2026-06-28.v1.json)
  records a reporting-only Promise rejection split. `Promise.reject`, `.catch`,
  and `.finally` now emit distinct missing-evidence labels for rejected-value
  channels, rejection continuations, settlement continuations, and callback
  demand/effect obligations while exact Promise continuation admission remains
  closed.
- [promise-then-obligation-diagnostics-2026-06-28.v1.json](promise-then-obligation-diagnostics-2026-06-28.v1.json)
  records the reporting-only `.then` split. Selector-only and custom receivers
  now report PromiseLike receiver proof separately from fulfillment
  continuation, rejection continuation, and callback demand/effect labels; exact
  Promise continuation admission remains closed.
- [promise-continuation-report-rows-2026-06-28.v1.json](promise-continuation-report-rows-2026-06-28.v1.json)
  records the focused row-visibility follow-up. `.then`, `.catch`, and
  `.finally` now appear as oracle-interpretable recall-loss
  `admission_rejections`, and the next recovery queue is quantified as
  receiver-first across `68` unhinted `.then`/`.catch` occurrences.
- [promise-local-continuation-recovery-2026-06-29.v1.json](promise-local-continuation-recovery-2026-06-29.v1.json)
  records the first broader local Promise continuation recovery slice.
  First-party `Promise.reject`, `.catch`, and two-argument `.then` now have
  contract evidence, local fulfilled/rejected Promise states are represented in
  the value graph, handler-returned `Promise.resolve` is flattened, and
  `catch` converges with `then(undefined, onRejected)` only for recoverable
  first-party rejected producers. Broad async scheduling, arbitrary thenables,
  `.finally`, aggregate combinators, custom receivers, and sync payload
  equivalence remain closed.
- [promise-receiver-producer-diagnostics-2026-06-29.v1.json](promise-receiver-producer-diagnostics-2026-06-29.v1.json)
  records the follow-up reporting-only receiver-producer split for Promise
  continuations. `new Promise(...).then`, async-function-return `.then`/`.catch`,
  and generic call-return receivers now have separate missing-evidence labels;
  a 120-repo JS/TS source scan found `835` generic call-return receivers,
  `49` same-file async-function call receivers, and only `2` constructor
  receivers, so exact admission remains closed and the next recovery target is
  call-return producer attribution rather than constructor semantics.
- [promise-call-return-callee-diagnostics-2026-06-29.v1.json](promise-call-return-callee-diagnostics-2026-06-29.v1.json)
  records the next reporting-only split inside generic Promise call-return
  receivers. Missing evidence now distinguishes member, local/parameter,
  imported binding/member, known-target return-domain, and unknown callee
  shapes. The revised 120-repo JS/TS scan found `932` member call-return
  candidates, `184` local/parameter candidates, `105` imported-member
  candidates, and `73` imported-binding candidates, so exact admission remains
  closed until callee identity and returned `PromiseLike` domain proof are both
  explicit.
- [promise-async-function-return-recovery-2026-06-29.v1.json](promise-async-function-return-recovery-2026-06-29.v1.json)
  records the first JS/TS same-file async-function producer recovery slice.
  Direct calls to JS/TS source-proven async functions now emit `PromiseLike`
  result-domain evidence, and pure non-thenable-safe returned payloads can feed
  local `.then` fulfillment recovery while preserving the Promise boundary.
  Await,
  throw/rejection, possible thenables, opaque call results, constructor
  receivers, imported/member call returns, `.finally`, and aggregate
  combinators remain closed.
- [promise-direct-function-return-recovery-2026-06-29.v1.json](promise-direct-function-return-recovery-2026-06-29.v1.json)
  records the direct-function subset of the local/parameter Promise call-return
  queue. Same-file direct calls now receive `Domain(PromiseLike)` result
  evidence when the target is a non-async single-return function whose returned
  expression already has PromiseLike domain proof. Literal and typed
  non-thenable `Promise.resolve` returns, plus `Promise.reject` returns, can
  feed local `.then`/`.catch` recovery while preserving the Promise boundary.
  Parameter callees, member/imported call returns, unsafe thenables,
  constructors, `.finally`, and aggregate/scheduling paths remain closed.
- [promise-direct-method-return-recovery-2026-06-29.v1.json](promise-direct-method-return-recovery-2026-06-29.v1.json)
  records the proof-backed DirectMethod subset of the member Promise
  call-return queue. Existing DirectMethod target evidence plus returned
  expression `Domain(PromiseLike)` evidence now derives PromiseLike call-result
  proof for non-async single-return methods. Value-graph recovery evaluates only
  the returned expression, closes when it reads receiver context, and preserves
  the Promise boundary; selector-only member calls, dynamic dispatch, imported
  members, unsafe thenables, `.finally`, constructors, and aggregate/scheduling
  paths remain closed.
- [promise-imported-call-return-boundary-2026-06-29.v1.json](promise-imported-call-return-boundary-2026-06-29.v1.json)
  records the reporting-only imported function/member Promise call-return
  boundary. Target-present imported receivers now report missing settled-value
  contracts instead of return-domain proof, because imported call-target
  identity has no local body that can recover fulfilled or rejected payloads.
  The `105` imported-member and `73` imported-binding source candidates remain
  closed behind source-level hard negatives.
- [promise-branch-return-producer-recovery-2026-06-29.v1.json](promise-branch-return-producer-recovery-2026-06-29.v1.json)
  records the branch-return extension for proof-backed local Promise producers.
  DirectFunction and DirectMethod call-result `Domain(PromiseLike)` evidence can
  now depend on every returned expression on the supported paths, and the value
  graph recovers same-channel Promise Phi states while preserving Promise
  boundaries. Mixed fulfilled/rejected branches, selector-only members, imported
  receivers, parameter callees, `.finally`, constructors, and aggregate or broad
  scheduling paths remain closed.
- [promise-finally-settlement-recovery-2026-06-29.v1.json](promise-finally-settlement-recovery-2026-06-29.v1.json)
  records the safe `.finally` settlement recovery slice. `Promise.finally` now
  has a builtin Promise contract, and the value graph preserves the original
  fulfilled/rejected settlement only for admitted PromiseLike receivers plus
  absent or zero-argument non-thenable-safe handlers. A finally handler returning
  `Promise.reject(reason)` switches the result to that rejected channel, while
  parameterized handlers, possible thenables, selector-only receivers, imported
  producers without settled-value contracts, constructors, aggregates, and broad
  scheduling remain closed.
- [promise-imported-settled-value-contract-2026-06-29.v1.json](promise-imported-settled-value-contract-2026-06-29.v1.json)
  records the reusable settled-value contract capability for imported Promise
  producers. `PromiseSettledValue` evidence can now compose with imported
  call-target identity, `Domain(PromiseLike)` receiver proof, and admitted
  Promise continuation API evidence to recover focused imported `.then`/`.catch`
  fixtures behind Promise boundaries. Ordinary imported producers without the
  contract, fulfilled possible-thenable payloads, selector-only members,
  constructors, aggregates, and broad scheduling remain closed.
- [promise-node-timers-domain-recovery-2026-06-29.v1.json](promise-node-timers-domain-recovery-2026-06-29.v1.json)
  records the Node `timers/promises` domain-only slice. A 120-repo JS/TS scan
  found `82` ESM named-import call sites across `execa`, `ky`, and `pixijs`;
  those calls can now materialize dependency-backed `Domain(PromiseLike)` for
  `setTimeout`/`setImmediate` through admitted `LibraryApi` occurrence evidence.
  At that point no settled payload recovery was opened, and `15` CommonJS
  destructuring require call sites remained closed until lowering emitted static
  imported-binding proof for that shape.
- [promise-node-timers-commonjs-domain-recovery-2026-06-29.v1.json](promise-node-timers-commonjs-domain-recovery-2026-06-29.v1.json)
  records the CommonJS follow-up for the same domain-only Node timers
  capability. Conservative `const` destructuring requires with unshadowed
  `require` now emit dependency-backed `ImportedBinding` proof for
  `setTimeout`/`setImmediate`, opening the previously priced `15` `jest` call
  sites and raising the Node timers PromiseLike domain slice from `82` to `97`
  priced call sites. At that point settlement/payload recovery, mutable
  destructuring, dynamic patterns, namespace/default imports, and broad
  scheduling remained closed.
- [promise-node-timers-safe-payload-recovery-2026-06-29.v1.json](promise-node-timers-safe-payload-recovery-2026-06-29.v1.json)
  records the bounded Node timers settled-payload follow-up. Exactly
  `setTimeout(delay, value)` and `setImmediate(value)` now emit fulfilled
  `PromiseSettledValue` evidence through the existing imported Promise producer
  contract, while option-bearing calls, possible-thenable payloads, scheduler
  APIs, interval streams, and broad scheduling remain closed. The 120-repo
  direct named-binding source scan found `0` safe-payload call sites, so the
  pinned-corpus recall delta is `0`; the covered capability is exercised by
  focused positive and hard-negative fixtures.
- [promise-scheduling-closeout-2026-06-29.v1.json](promise-scheduling-closeout-2026-06-29.v1.json)
  records the closeout decision for the current Promise/scheduling recovery
  cycle. Local producer recovery, imported `PromiseSettledValue` evidence, and
  Node timers slices are treated as complete for this cycle; aggregate
  combinators, executor timing, cancellation/liveness, scheduler APIs, interval
  streams, and cross-language async/channel/lifecycle models move to issue
  [#602](https://github.com/corca-ai/nose/issues/602) instead of more
  API-by-API expansion.
- [scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json](scheduling-lifecycle-boundary-audit-602-2026-06-29.v1.json)
  records the first #602 reporting-only pricing slice. It scans the 120-repo
  corpus for scheduling, aggregate, cancellation, channel, executor, lifecycle,
  and exception surfaces, attaches the current local `crates` recall-loss gate,
  and ranks the next safe reporting targets without opening exact admission.
- [promise-all-literal-aggregate-recovery-2026-06-29.v1.json](promise-all-literal-aggregate-recovery-2026-06-29.v1.json)
  records the first #602 exact aggregate capability slice. It opens only
  fulfilled `Promise.all` over literal array arguments whose elements already
  recover as fulfilled Promise evidence, while dynamic iterables, rejected
  elements, first-settled/first-fulfilled aggregates, thenables, executor timing,
  and sync arrays remain closed.
- [promise-allsettled-literal-aggregate-recovery-2026-06-29.v1.json](promise-allsettled-literal-aggregate-recovery-2026-06-29.v1.json)
  records the next #602 exact aggregate capability slice. It opens fulfilled
  `Promise.allSettled` results over literal array arguments whose elements
  already recover as fulfilled or rejected Promise evidence, preserving ordered
  settled-record payloads behind the Promise boundary. Dynamic iterables,
  first-settled/first-fulfilled aggregates, thenables, executor timing, and
  sync settled-record arrays remain closed.
- [promise-aggregate-raw-input-recovery-2026-06-29.v1.json](promise-aggregate-raw-input-recovery-2026-06-29.v1.json)
  records the shared #602 raw-input assimilation slice for literal
  `Promise.all` and `Promise.allSettled` aggregates. A raw element may become a
  fulfilled aggregate input only under the same non-thenable-safe proof used by
  `Promise.resolve`; at that slice, dynamic iterables, object/function raw
  inputs, untyped possible thenables, `Promise.race`, `Promise.any`, executor
  timing, and sync aggregate equivalence remained closed.
- [promise-race-any-literal-aggregate-recovery-2026-06-30.v1.json](promise-race-any-literal-aggregate-recovery-2026-06-30.v1.json)
  records the #602 first-observed aggregate slice for literal `Promise.race`
  and `Promise.any`. `Promise.race` can recover the first-settled element only
  for non-empty fully closed literal arrays; `Promise.any` can recover the
  first fulfilled element only for fully closed literal arrays with a fulfilled
  candidate. Dynamic iterables, possible thenables, all-rejected `Promise.any`
  AggregateError payloads, executor timing, and sync value equivalence remain
  closed.
- [promise-executor-boundary-audit-2026-06-30.v1.json](promise-executor-boundary-audit-2026-06-30.v1.json)
  records the #602 reporting-only `new Promise(...)` executor readiness audit.
  It prices `795` constructor occurrences across the pinned corpus, splits
  inline/identifier executors, settlement calls, timer/scheduler use,
  multi-settlement, throw-to-rejection, and possible thenable payload risks,
  and keeps `semantic_admission_delta = 0`. The lexical direct single-settlement
  upper bound is `27` scalar resolves plus `4` scalar rejects, but exact
  constructor recovery remains closed until executor timing, callback identity,
  settlement precedence, throw-to-rejection, and non-thenable proof are all
  represented.
- [abort-signal-cancellation-boundary-audit-2026-06-30.v1.json](abort-signal-cancellation-boundary-audit-2026-06-30.v1.json)
  records the #602 reporting-only AbortSignal cancellation/liveness readiness
  audit. The pinned corpus has `260` Abort mentions, `156`
  `AbortController` constructors, `175` `.abort()` selector calls, `323`
  `.signal` property reads, `193` `signal` option properties, `6` `fetch`
  calls with signal options, `2` timer calls with signal options, and `2`
  `addEventListener` calls with signal options. Exact cancellation admission
  remains closed with `semantic_admission_delta = 0`.
- [interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json](interval-scheduler-lifecycle-boundary-audit-2026-06-30.v1.json)
  records the #602 reporting-only interval/scheduler lifecycle readiness audit.
  The pinned corpus has `780` `setTimeout` calls, `57` `setImmediate` calls,
  `73` bare `setInterval` calls, `23` member `.setInterval` calls, `55`
  `clearInterval` calls, `133` `clearTimeout` calls, `14` `queueMicrotask`
  calls, `43` `requestAnimationFrame` calls, and `11` `scheduler.yield` calls.
  Exact scheduling/lifecycle admission remains closed with
  `semantic_admission_delta = 0`.
- [issue-602-closeout-2026-06-30.v1.json](issue-602-closeout-2026-06-30.v1.json)
  records the #602 closeout decision. Every opened exact aggregate slice has a
  checked artifact, reporting-only executor/cancellation/scheduling/lifecycle
  slices keep `semantic_admission_delta = 0`, local `crates` gate remains at
  `false_merges = 0` and `canon_preservation_violations = 0`, and remaining
  Promise/scheduling/aggregate/cancellation/lifecycle surfaces are named closed
  obligations rather than opaque runtime rows.
- [issue-601-first-slice-closeout-2026-06-28.v1.json](issue-601-first-slice-closeout-2026-06-28.v1.json)
  records the #601 decision to close the first exact-admission slice as a
  quantified closed boundary instead of forcing unsafe async/callback/channel
  admission.
- [issue-594-closeout-2026-06-28.v1.json](issue-594-closeout-2026-06-28.v1.json)
  records the #594 epic closeout: every child issue's artifact, quantitative
  summary, validation commands, remaining closed boundaries, and hard-gate
  status.
- [issue-570-cycles.v1.json](issue-570-cycles.v1.json) records the five focused
  top-bucket cycles and the explicit unsupported/fail-closed boundary decision.
- [issue-572-cycle.v1.json](issue-572-cycle.v1.json) records the first
  post-#570 refinement cycle: expression-statement effect boundaries and Rust
  macro surfaces are split out of the callee-identity bucket while preserving
  the hard gate.
- [issue-574-callee-census.v1.json](issue-574-callee-census.v1.json) records
  the #567/#574 census of the remaining callee-identity bucket by language and
  call-target surface.
- [issue-576-cycle.v1.json](issue-576-cycle.v1.json) records the first recovery
  slice after the census: Rust brace `use` imports now feed dependency-backed
  imported call-target evidence while wildcard/nested/relative brace imports
  stayed closed in that slice.
- [issue-578-cycle.v1.json](issue-578-cycle.v1.json) records the next Rust
  scoped-path recovery slice: imported roots such as `Span::new` now feed
  dependency-backed imported member call-target evidence while raw `crate`,
  `std`, unimported, and ambiguous roots stay closed.
- [issue-580-cycle.v1.json](issue-580-cycle.v1.json) records the Rust
  struct-expression surface slice: Rust struct literals now carry exact-safe
  `SequenceSurface` evidence, recovering the imported-member target-present
  follow-ups from #578 while keeping untagged sequences and collection/map
  contracts separate.
- [issue-567-phase1-js-ts-constructors.v1.json](issue-567-phase1-js-ts-constructors.v1.json)
  records imported immutable provider snapshots for JS/TS `new Map(...)` and
  `new Set(...)`.
- [issue-567-phase2-collection-factories.v1.json](issue-567-phase2-collection-factories.v1.json)
  records imported immutable provider snapshots for existing Python and Java
  collection factory contracts.
- [issue-567-phase3-import-snapshot-census.v1.json](issue-567-phase3-import-snapshot-census.v1.json)
  records the report-infrastructure closeout: recall-loss artifacts now include
  import snapshot success counts and binding-import miss reasons.
- [issue-567-phase4-aggregate-boundary-triage.v1.json](issue-567-phase4-aggregate-boundary-triage.v1.json)
  records the first census-driven triage pass: the broad provider-aggregate
  miss bucket is split into non-import-literal sequence surfaces and child
  reference boundaries without admitting new snapshots.
- [issue-567-closeout.v1.json](issue-567-closeout.v1.json) records the epic
  closeout audit: requirement coverage, admitted imported-coordinate families,
  hard-negative inventory, false-merge hard gates, and product/runtime evidence.
- [issue-587-module-export-census.v1.json](issue-587-module-export-census.v1.json)
  records the #587 starting census for the next import-snapshot milestone:
  module/export miss counts by reason, crate, import surface, top files, and
  recommended implementation order.
- [issue-587-module-resolution-1-3.v1.json](issue-587-module-resolution-1-3.v1.json)
  records the first #587 implementation slice: Rust crate/module identity,
  `self::`/`super::` provider lookup, and boundary splitting for callable,
  type, module namespace, stdlib, and workspace-crate imports.
- [issue-587-reexport-pricing.v1.json](issue-587-reexport-pricing.v1.json)
  records the direct Rust public re-export slice: one-hop `pub use` lookup for
  already literal-safe provider exports, bare child module aliases, and
  re-export-specific boundary reasons.
- [issue-587-residual-census.v1.json](issue-587-residual-census.v1.json)
  records the post-re-export #587 residual census: the remaining generic
  module/export misses are mostly external crate imports, with a smaller
  relative-`super` and local export tail for follow-up triage.
- [issue-587-residual-boundary-split.v1.json](issue-587-residual-boundary-split.v1.json)
  records the #587 residual boundary split: external crate, residual stdlib, and
  residual workspace-crate imports are priced as explicit closed boundaries
  instead of generic provider-module misses.
- [issue-587-relative-super-closeout.v1.json](issue-587-relative-super-closeout.v1.json)
  records the #587 relative-super closeout: imports that name the parent module
  itself now resolve to callable/type boundaries, moving
  `provider-module-missing` `11 -> 0` without admitting new snapshots.
- [post-587-census.v1.json](post-587-census.v1.json) records the current
  post-#587 `crates` recall-loss census: module/provider-missing import
  snapshot work is complete on the checked surface, and the next largest
  capability buckets are receiver-domain proof, callee identity, and
  mutation/effect contracts.
- [corpus-priority-census-2026-06-28.v1.json](corpus-priority-census-2026-06-28.v1.json)
  records the first full 120-repo corpus priority census after #587, combining
  per-repo recall-loss reports with lexical stdlib/API source prevalence to
  choose the next semantic-kernel capability slices from corpus evidence.
