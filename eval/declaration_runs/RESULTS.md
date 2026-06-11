# Declaration-run filter — corpus price (2026-06-11)

`python3 eval/declaration_runs/sweep.py` from the repo root (needs
`bench/repos` checked out and `./target/release/nose` built) re-derives every
number in [experiments §BY](../../docs/experiments.md).

## Result

- **2,265** families across **43**/105 repos classify `declaration`
  (every member span provably import/include/`use`/re-export lines only) and
  leave the default surface.
- By extension: java 1,850 · py 254 · ts 90 · js 30 · rs 30 · tsx 11.
- Span-overlap join against `bench/labels/refactoring_families.v5.json`:
  419 overlaps, **1** with a worthy label — nushell `6094823c2d64a432`
  (extract-base, medium confidence). The declaration family there is the
  imports-only sub-span (`contains.rs:8-14`); the label's actionable content
  (the near-identical polars command modules) still reports via two
  default-surface families (`contains.rs:3-15` pair and the whole-file
  `1-186` family). **Zero worthy-labeled families were themselves
  reclassified.**

## Protocol notes

- The join is by file + line-span overlap, not family id, so it is robust to
  detector-version span drift but intentionally over-counts: any labeled
  family merely *touching* a declaration span counts as an overlap.
- The classifier under test is the shipped one (`declaration_run_ids` in
  `crates/nose-cli/src/main.rs`), exercised through `nose scan --format json
  --top 0`.
