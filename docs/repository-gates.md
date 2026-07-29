# Repository gate inventory

The repository's executable quality-policy boundary is:

```sh
./scripts/check-ci-local.sh --gate <name> [gate arguments...]
```

GitHub Actions supplies runner setup and calls that same boundary. Local plans,
gate ownership, required tools, inputs, worktree effects, cache behavior, lane
rationale, and focused commands are declared in the checked
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

## Worktree effects

Most gates are `read-only`: build/test output is confined to ignored caches such
as `target/`. A `verify-checked-output` gate may deterministically regenerate a
tracked receipt or evidence file, but it must compare that output and leave the
worktree unchanged when the checked artifact is current.

The timing harness fingerprints the complete tracked/untracked status before
and after every gate. A successful gate that changes the worktree makes the
measurement fail, so artifact production cannot hide behind a green command.

## Timing protocol

Gate time depends on machine, compiler cache, and corpus state. The checked
[`gate-timings.v1.json`](../scripts/ci/gate-timings.v1.json) receipt therefore
records its commit, environment, profile, mode, total time, per-gate time, exit
status, and worktree-drift result instead of presenting one duration as a
universal SLA.

Refresh it from a clean worktree with existing build caches:

```sh
python3 scripts/ci/measure_gates.py \
  --profile clean-tree \
  --mode fast --mode full \
  --output scripts/ci/gate-timings.v1.json

python3 scripts/ci/measure_gates.py \
  --profile no-change \
  --mode fast \
  --output scripts/ci/gate-timings.v1.json \
  --append

python3 scripts/ci/measure_gates.py \
  --validate scripts/ci/gate-timings.v1.json
```

The `clean-tree` profile means source and checked evidence have no pending
changes; it deliberately reports existing build-cache state rather than calling
`cargo clean`. The immediate `no-change` fast rerun is the representative
incremental feedback measurement. The validator requires complete clean-tree
fast/full runs, a complete no-change fast run, coverage of every registered
gate, zero failed gates, and zero worktree drift.

Use measurements to find duplicated setup or a gate assigned to the wrong lane.
Do not remove validation or move release/soundness qualification to a faster
lane merely to improve the aggregate time.
