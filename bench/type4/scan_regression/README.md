# Semantic-scan regression harness

A repeatable harness for the **product** semantic scan path, so detector changes
(#33 and later) can be checked for **runtime regression** and **output-volume
drift** without any chat history. Issue: #37. Part of the
[Type-4 benchmark factory](../README.md); see also the
[scan JSON contract](../../../docs/scan-json.md).

Everything a fresh worker needs is in this directory:

| file | what it is |
|---|---|
| `scan_regression.py` | the harness (`baseline`, `compare`, `cache`, `selftest` subcommands) |
| `subset.json` | the small, language-diverse repo subset to measure |
| `baseline.v1.json` | the recorded reference snapshot (binary identity + per-repo canonical output + runtime medians) |
| `compare-summary.md` | the latest `compare` markdown report (regenerated each run) |

## The one fixed command

Output drift is always measured on the product path, and only that path:

```
nose scan <repo> --mode semantic --format json --top 0
```

The hidden `nose detect` path uses a different detector/scoring route, so it is
**never** used as a substitute for product family drift — this harness does not
call it at all. Candidate counts (`candidate_pairs`) are not exposed on the product
JSON today, so the harness does not report them; it records only what
`--format json` and `NOSE_TIME` emit on the product path.

## Quick start

A fresh worktree has **no corpus** (`bench/repos` is gitignored). Either populate it
with `bench/setup_repos.sh`, or point `--repos-root` at an existing checkout (e.g. the
main worktree's `bench/repos`). Build the binary you want to measure:

```sh
cargo build --release --bin nose
```

Record a baseline from a known-good binary (usually `main`):

```sh
python3 bench/type4/scan_regression/scan_regression.py baseline \
    --nose ./target/release/nose \
    --repos-root /path/to/main/bench/repos \
    --build-ref "main@$(git rev-parse --short HEAD)"
```

Then build your change and compare:

```sh
python3 bench/type4/scan_regression/scan_regression.py compare \
    --nose ./target/release/nose \
    --repos-root /path/to/main/bench/repos
```

`compare` writes `compare-summary.md` and prints it. It **exits 0 even when triggers
fire** — triggers are investigation prompts, not merge blockers (see below). Pass
`--strict` to make any trigger non-zero once the thresholds are calibrated.

Run the corpus-free unit checks for the canonicalization/gate logic:

```sh
python3 bench/type4/scan_regression/scan_regression.py selftest
```

## What gets compared (output drift)

The `--top 0` full JSON is canonicalized so **family order and ranking tie-breaks are
ignored** and locations are made **repo-relative**. Each scan runs with `cwd` set to the
repo and a `.` target, so the CLI emits repo-relative paths and `family_id`,
`product_json_bytes`, and the location keys are **independent of where the corpus is
checked out** — the committed baseline compares cleanly whether the corpus lives under the
main worktree or any other path you pass to `--repos-root`. Per repo we record and diff:

- `total_families` / `shown_families`
- `product_json_bytes` — payload byte size with the volatile `tool_version` removed
- `kind_counts` — unit kinds across all locations (`Block`, `Function`, `Method`, …)
- `span_buckets` — families bucketed by `mean_lines` (`1`, `2-3`, `4-10`, `11-30`, `31+`)
- `recommended_surface_counts` — family product placement counts for `default`,
  `review`, `hidden`, and reserved `debug`
- `family_shape_counts` — whole-only, all-fragment, and mixed family counts
- per family — **keyed by the normalized location set, not `family_id`** (the product
  `family_id` is not unique; distinct families can share one id, so keying on it would
  silently drop a family): `family_id` (kept as an attribute and a drift signal),
  `members`, `location_count`, `mean_lines`, `recommended_surface`, family shape,
  fragment count, per-kind counts, fragment-only span buckets, enclosing-unit recovery,
  and the sorted locations. `distinct_location_sets` is recorded so a true location-set
  collision would surface rather than collapse silently.
- `fragment_kind_counts` / `reason_code_counts` — exact-fragment metadata buckets from
  product scan JSON. #45 makes these live for current output, so fragment/reason drift is
  visible separately from generic `Block` counts.
- `fragment_kind_surface_counts` / `reason_code_surface_counts` — exact-fragment proof
  buckets crossed with `recommended_surface`, so calibration drift is visible before it
  changes detector output.
- `fragment_line_span_buckets` / `fragment_token_span_buckets` — fragment-location span
  distributions. Line spans use the same buckets as family `mean_lines`; token spans use
  `0`, `1-8`, `9-23`, `24-49`, `50-99`, and `100+`.
- `enclosing_unit_recovery_counts` — recovered vs missing exact enclosing unit metadata
  across fragment locations.

These #51 C1 metrics are computed by the harness from stable scan JSON. They do not add
new scan JSON fields and do not change detector acceptance, ranking, or `--top 0`
visibility.

`baseline` runs each repo `runtime_repeats` times and asserts the canonical output is
**identical across runs** on one binary — a determinism guard. A mismatch aborts before
any drift comparison can be trusted.

## What gets measured (runtime)

Runtime is measured **without `--cache-dir`** (cache state would mix #33's
normalize/extract cost with cache effects). Each repo is run `runtime_repeats` times
(default 5, minimum should be ≥ 3) and the **median** wall-clock and median per-phase
timings (`NOSE_TIME` stages: `lower`, `normalize+extract`, `candidates`, `score`,
`cluster`, `groups`, `contiguous`) are recorded.

Wall-clock is **not portable across machines or load**. For a meaningful runtime
comparison, record the baseline and run `compare` **on the same machine**: build `main`,
`baseline`, then build your change and `compare`. When the binary `sha256` matches the
baseline, the summary says so explicitly and any delta is environment noise. The
committed `baseline.v1.json` runtime numbers are a snapshot from one machine; the
**output drift** in it is portable, the **runtime** is not.

Cache performance is a **separate** mode that never feeds the baseline:

```sh
python3 bench/type4/scan_regression/scan_regression.py cache --nose ./target/release/nose
```

It reports no-cache vs cold (fresh temp cache) vs warm (reused cache) wall-clock per
repo, keeping the cache effect isolated from the normalize/extract cost.

## Thresholds = investigation triggers, not merge blockers

Until calibrated, a single noisy wall-clock run must not fail a build. `compare` flags,
for a human to look at, not to gate:

| signal | trigger |
|---|---|
| family set | any added/removed location set, or a family changing shape (members/locations/mean_lines/kinds/surface/fragment metadata) |
| `total_families` | any change |
| `product_json_bytes` | > 5% change |
| kind / span / fragment / reason-code / surface / enclosing-unit buckets | any count change |
| runtime (per-phase + wall median) | > 25% growth **and** > 5 ms absolute (loose + floored because it's noisy) |

The thresholds live in `THRESHOLDS` at the top of `scan_regression.py`. Tune them there
as the harness is calibrated, then turn on `--strict` for the signals you trust.

## Subset (and the #36 link)

`subset.json` lists one repo per supported language plus a second small Go repo, all
sub-second single-pass scans so the no-cache repeats stay cheap. `liquid` (ruby) and
`junit5` (java) also appear in the Type-4 frontier (`../real_frontier.v1.json`, #36), so
the subset already overlaps live frontier work. To measure #36's next batch, edit
`repos` (and `repos_root` if needed) — the harness is fully data-driven.

## Refreshing the committed baseline

Re-record `baseline.v1.json` when `main`'s product output legitimately changes (a
reviewed detector change merges). Regenerate from `main` with the `baseline` command
above and commit the result, so the snapshot keeps tracking accepted product behavior.
