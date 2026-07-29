# Repository gate inventory

The repository's executable quality-policy boundary is:

```sh
./scripts/check-ci-local.sh --gate <name> [gate arguments...]
```

GitHub Actions supplies runner setup and calls that same boundary. Local plans,
gate ownership, required tools, inputs, worktree effects, cache behavior,
dependencies, parallel-safety, resource groups, lane rationale, and focused
commands are declared in the checked
[`scripts/ci/gates.json`](../scripts/ci/gates.json) registry.

## Discover and validate gates

Render the current inventory:

```sh
./scripts/check-ci-local.sh --list-gates
./scripts/check-ci-local.sh --list-gates --format json
```

Validate the registry, shell dispatcher, local plans, and workflow membership:

```sh
./scripts/check-ci-local.sh --validate-gates
```

Every named `--gate` invocation performs the live registry validation before it
runs. The validator fails if:

- the registry and `run_named_gate` dispatcher name different gates;
- a `fast` or `full` plan disagrees with its declared lane;
- `.github/workflows/ci.yml` calls a gate outside the pull-request lane or omits
  a pull-request gate;
- the release workflow stops reusing `ci.yml`;
- the Soundness Lab starts calling named gates without declaring the nightly
  lane;
- a checked-output gate does not name the output it verifies.
- a plan dependency is absent, ordered after its consumer, or names the
  consumer itself;
- a parallel-safety or resource-group declaration is malformed.

The registry owns selection, ordering, and descriptive metadata. The shell
dispatcher owns executable commands and diagnostics. The cross-check prevents
either side from silently becoming an independent policy.

Repository evidence validators require Python 3.10 or newer. The dispatcher
checks the selected `python3` before any named gate runs and reports the
observed version, so a system Python that is too old cannot fail later inside a
large evidence batch with a misleading error.

## Lanes

`local-fast`
: Pre-push feedback using debug product-contract checks. It includes cheap
  artifact and orchestration self-tests because those checks catch stale
  checked evidence before expensive compilation completes.

`local-full`
: Complete local mirror of repository quality policy. It adds release builds
  and tests, coverage, duplication, MSRV, supply-chain, Rust documentation, and
  formal proof gates.

`pull-request`
: Named gates invoked by `.github/workflows/ci.yml`, including gates split into
  dedicated coverage, MSRV, supply-chain, documentation, and formal jobs.

`release`
: The same named quality policy, reused through the release workflow's
  `quality-gate` call to `ci.yml`. Packaging remains owned by `cargo-dist`.

`nightly`
: Named gates invoked directly by the Soundness Lab workflow. The current
  Soundness Lab owns its campaign commands directly, so no named repository gate
  is assigned to this lane. Its cheap runner mutation test is the separate
  `corpus-verify-selftest` pull-request gate.

## Local execution

Local plans remain sequential by default. Opt into bounded parallel execution
when the machine has enough CPU and memory:

```sh
./scripts/check-ci-local.sh --fast --jobs 2
./scripts/check-ci-local.sh --full --jobs 2
```

`NOSE_CI_JOBS` supplies the same default without changing the checked command.
The planner starts only gates declared `parallel_safe`, waits for their
mode-specific `depends_on` gates, and never overlaps members of one
`resource_group`. A non-parallel-safe gate is an ordering barrier: earlier work
must finish before it starts, and later work waits until it completes. Stable
Cargo work shares `cargo-stable`; MSRV, coverage, checked label evidence, and
checked Type-4 evidence use separate groups that match their isolated outputs.

The `default-head-evidence` gate also runs its five independent mutation
self-tests through a bounded worker pool after all checked artifacts validate.
It fills each free slot immediately and defaults to three workers even when the
outer plan is sequential. Set
`NOSE_DEFAULT_HEAD_JOBS=1` to reproduce the former serial diagnostic order.
Each worker writes only to its own temporary log or the self-test's own
temporary directory; output is replayed in declaration order.

The serial phase validates the residual-ranking artifact chain once through the
aggregate default-head closeout. That closeout rebuilds the calibration, top-up
selection, blind panel, arbitration, decisions, label component, and residual
closeout, so the local plan does not repeat their focused validation commands
before running the same aggregate check.

## Worktree effects

Most gates are `read-only`: build/test output is confined to ignored caches such
as `target/`. The MSRV gate uses `target/msrv/` so switching compilers does not
invalidate the stable toolchain's incremental artifacts; set
`NOSE_MSRV_TARGET_DIR` only when a different isolated cache location is needed.
A `verify-checked-output` gate may deterministically regenerate a tracked receipt
or evidence file, but it must compare that output and leave the worktree
unchanged when the checked artifact is current.

The timing harness fingerprints the complete tracked/untracked status before
and after every gate. A successful gate that changes the worktree makes the
measurement fail, so artifact production cannot hide behind a green command.

## Timing protocol

Gate time depends on machine, compiler cache, and corpus state. The checked
[`gate-timings.v1.json`](../scripts/ci/gate-timings.v1.json) receipt therefore
records its commit, environment, profile, mode, total time, per-gate time, exit
status, and worktree-drift result instead of presenting one duration as a
universal SLA.

The checked per-gate receipt uses the default sequential outer plan so each
gate retains an attributable duration. Measure the opt-in parallel plan
separately:

```sh
/usr/bin/time -p ./scripts/check-ci-local.sh --fast --jobs 2
/usr/bin/time -p ./scripts/check-ci-local.sh --full --jobs 2
```

Refresh it from a clean worktree with existing build caches:

```sh
python3 scripts/ci/measure_gates.py \
  --profile clean-tree \
  --mode fast --mode full \
  --output target/gate-timings.v1.json

python3 scripts/ci/measure_gates.py \
  --profile no-change \
  --mode fast \
  --output target/gate-timings.v1.json \
  --append

python3 scripts/ci/measure_gates.py \
  --validate target/gate-timings.v1.json

cp target/gate-timings.v1.json scripts/ci/gate-timings.v1.json
```

The `clean-tree` profile means source and checked evidence have no pending
changes; it deliberately reports existing build-cache state rather than calling
`cargo clean`. The immediate `no-change` fast rerun is the representative
incremental feedback measurement. The validator requires complete clean-tree
fast/full runs, a complete no-change fast run, coverage of every registered
gate, zero failed gates, and zero worktree drift.

Build the complete receipt under ignored `target/` and install it only after
validation. The timing receipt belongs to the lifecycle catalog's
`repository-policy` inventory, so writing the first partial run directly to the
tracked path would make the later `evidence-artifacts` gate observe inventory
drift. After installation, refresh that inventory digest and run the artifact
lifecycle validator.

### Recorded cache-isolation follow-up

The current receipt records source commit `9481f3ec` on arm64 macOS with Python
3.14.6 and Rust/Cargo 1.96.0. It covers all 33 registered gates:

| Profile | Plan | Gates | Wall time | Failures | Worktree drift |
|---|---|---:|---:|---:|---:|
| clean-tree | fast | 23/23 | 412.252 s | 0 | 0 |
| clean-tree | full | 31/31 | 382.548 s | 0 | 0 |
| no-change | fast | 23/23 | 364.204 s | 0 | 0 |

Compared with the preceding `1bfce491` receipt, clean fast is 46.217 seconds
(10.1%) faster and clean full is 239.191 seconds (38.5%) faster. The docs gate
no longer repeats the corpus-backed frontier-platform check owned by
`type4-frontier`; its clean fast time fell from 45.948 to 0.583 seconds while
`type4-frontier` still passed in 45.670 seconds. The MSRV gate now writes to the
isolated `target/msrv/` cache. With that cache already warm, its clean full time
fell from 224.451 to 0.170 seconds without invalidating stable-toolchain
artifacts. A focused empty-cache MSRV run took 17.343 seconds; the warm result
is not a cold-bootstrap claim.

The dominant remaining clean-fast costs are `default-head-evidence` (214.754
seconds), `test-debug-cli` (96.670 seconds), `type4-frontier` (45.670 seconds),
and `runtime-soundness-evidence` (18.058 seconds). Registry validation,
artifact validation, formatting, shell lint, and file-length policy remain
sub-second or low-single-digit work, so removing them from fast feedback would
save little while delaying actionable failures.

### Recorded parallel-planning follow-up

At source commit `b8db7fda`, a focused three-worker
`default-head-evidence` run on the same arm64 macOS environment took 143.550
seconds and passed without worktree drift. That is 71.204 seconds (33.2%) below
the preceding receipt's 214.754-second serial result. The five independent
mutation self-tests account for the change; all artifact validation remains
serial and runs before the worker pool.

At source commit `9d5be0b7`, the serial phase stopped invoking six focused
residual-chain validators that the aggregate default-head closeout already
rebuilds and validates. The same focused gate passed in 98.060 seconds: 45.490
seconds (31.7%) below the three-worker result and 116.694 seconds (54.3%) below
the original 214.754-second result. The aggregate artifact validation and all
five mutation self-tests remain in the gate.

### Calculation-reuse follow-up

The residual-ranking evaluator now computes each repository/proposal order once
and aggregates it for the full dataset and all repository-CV folds. Run-scoped
validation contexts also keep the top-up/panel/closeout mutation tests from
rebuilding an already validated source chain. Focused arm64 macOS measurements
with Python 3.14.6 reduced calibration validation from 6.61 to 1.22 seconds,
the panel mutation self-test from its previously observed 56–58 seconds to
1.77 seconds, and residual closeout self-test from its previously observed
16–21 seconds to 3.04 seconds. All checked evaluation and closeout artifacts
reproduced exactly. The complete focused `default-head-evidence` gate passed in
41.53 seconds, down from the preceding 98.060-second focused run.

The frontier platform now scans pinned repositories through a deterministic
four-worker map/reduce. On the same corpus, `--check --jobs 1` took 50.90
seconds and `--check --jobs 4` took 12.91 seconds. Both paths accepted the
same checked JSON, Markdown, and packet artifacts; the corpus-free self-test
also compares serial and parallel projections with an order-sensitive sample
limit. The complete `type4-frontier` gate passed in 13.74 seconds.

No aggregate `--jobs 2` wall-time claim is recorded for this follow-up. During
measurement, macOS policy inspection delayed each locally built Rust test
binary independently of the planner, making the result non-representative.
Planner behavior is covered by its scheduling and real-dispatch self-tests;
the complete gate results are qualified by the hosted CI lanes.

Use measurements to find duplicated setup or a gate assigned to the wrong lane.
Do not remove validation or move release/soundness qualification to a faster
lane merely to improve the aggregate time.

The preceding 33-gate `1bfce491` result and the original 30-gate #949
measurement remain in the
[pre-epic readiness record](pre-epic-readiness-948.md) as historical evidence.
