# Type-4 Coevolution Handoff

Date: 2026-06-05

This records where the adversarial Type-4 coevolution work stopped and how to resume it.
The work was intentionally paused after a real-repository evaluation pass. Do not start
another autonomous frontier loop unless the user explicitly resumes it.

## Current State

- Branch: `main`.
- Last committed detector/frontier change: `0b63ad9 feat(type4): prove python module membership`.
- Installed baseline used for real-repo comparison: `/opt/homebrew/bin/nose`, version `0.2.0`.
- Current candidate used for comparison: `target/release/nose`, version `0.4.0`.
- Worktree at pause time: clean except the pre-existing untracked `.claude/` directory.

## Last Completed Coevolution Loop

The last completed strict-frontier loop was `Batch-3 Python module collection membership`
recorded in `bench/type4/ITERATIONS.md` as loops 402-406.

The batch opened:

- `axis_membership_module_python_tuple_identity`;
- `axis_membership_module_python_set_identity`;
- `axis_membership_module_python_mutated_boundary`.

The detector change:

- canonicalized Python module-level tuple/set literal bindings as strict collection
  membership values only after stable-binding proof;
- rejected normalized `@Append(receiver, value)` mutations for module/local bindings,
  closing a false merge where `VALUES.append(...)` bypassed the old field-method scanner.

Final validation:

```text
focused:                  4/4 positives, 0/6 false merges
literal-membership core:  175/175 positives, 0/424 false merges
compact all-cross core:   613/613 positives, 0/1201 false merges
```

Afterwards, a broader generated smoke run also showed:

```text
same-surface/full smoke: 1089/1089 positives, 0/2010 false merges
```

## Where Work Stopped

After the last completed loop, the next candidate was explored but not implemented.

The likely next synthetic frontier candidate was `map_default_lookup` for Ruby
`Hash#fetch(key, default)` on a proven dynamic receiver. A probe showed Ruby fetch lowers
to this normalized IL shape:

```lisp
(call
  (field "fetch"
    (var v0))
  (var v2)
  (lit int=0))
```

No files were changed for that candidate. It is only a candidate note.

## Real-Repo Evaluation Pass

Before continuing synthetic work, a real-repo sample comparison was run as requested:
two repos per supported language by actual file extension where possible, comparing
installed `nose 0.2.0` against current `nose 0.4.0` with:

```text
nose scan <repo> --mode semantic --format json --top 0
```

Results were written to:

```text
/tmp/nose-real-compare2/summary.json
```

Final sampled repos:

| language | repos |
|---|---|
| C | `tmux`, `zstd` |
| Go | `chi`, `gin` |
| Java | `gson`, `jsoup` |
| JavaScript | `axios`, `marked` |
| Python | `scrapy`, `sqlalchemy` |
| Ruby | `rspec-core`, `pry` |
| Rust | `regex`, `alacritty` |
| TypeScript | `zod`, `trpc` |

The initially selected Rust repo `clap` timed out on the current binary after 90s and was
replaced by `alacritty`. Treat `clap` as a separate performance investigation target.

Aggregate real-repo sample result:

```text
semantic families: 2021 -> 635  (delta -1386)
prod families:      944 -> 260  (delta -684)
test families:     1030 -> 346  (delta -684)
dup lines:        29265 -> 5047 (delta -24218)
value sum:        43916.2 -> 7661.5
added families:     256
removed families:  1642
```

Interpretation:

- The current detector is much stricter than the installed version. It removes many broad
  semantic families that are not proven strict Type-4.
- The current detector still adds some new useful strict families, especially in Python,
  C legacy-version code, Rust shared utility code, and TypeScript helper predicates.
- This is a good direction for strict Type-4, but the result also shows that synthetic
  recall alone is no longer the right success metric.

Examples that looked useful:

- `zstd`: repeated legacy-version blocks across `zstd_v02`-`zstd_v07`.
- `regex`: duplicated `mkwordset` logic across `regex-automata` and `regex-lite`.
- `scrapy`: repeated `from_crawler` / setup-family methods.
- `sqlalchemy`: repeated test setup/mapping patterns.
- `trpc`: small repeated predicate/helper functions.

Examples with low refactoring value:

- one-line `axios` test callbacks;
- short constructor boilerplate in `gson`/`jsoup`;
- short `expecting` helpers in `alacritty`.

## What To Do Differently Next

1. Do not keep running only synthetic batch loops.

   The generated strict suite is currently clean. More synthetic batches can still widen
   the frontier, but the marginal value is lower unless the new proof invariant is backed
   by real-repo evidence.

2. Make real-repo useful yield part of the loop gate.

   A future loop should pass all of these:

   - focused generated batch improves recall or closes a false merge;
   - axis-core and compact all-cross remain at zero false merges;
   - installed-vs-current real-repo sample produces at least a few human-useful added
     families, or demonstrably removes unsafe installed-version families;
   - runtime does not regress badly on representative repos.

3. Preserve batch-3, but choose batches from one invariant.

   Good batch shape:

   - two or three positives sharing the same proof rule;
   - one hard-negative boundary sharing the same coordinates;
   - one focused baseline before implementation;
   - one axis-core and one compact all-cross after implementation.

   Bad batch shape:

   - mixing unrelated language features;
   - adding examples just because generator coverage is easy;
   - broadening ambiguous receiver methods without type/import/mutation proof.

4. Before Ruby `fetch`, investigate real examples.

   If resuming the Ruby `Hash#fetch(key, default)` idea, first mine `rubocop`,
   `fastlane`, `rspec-core`, and `pry` for concrete repeated fetch-default patterns.
   Only implement it if receiver/key/default coordinates can be proven without trusting
   arbitrary `fetch` methods.

5. Investigate `clap` performance separately.

   `clap` timed out for current semantic scan under the 90s sample script while smaller
   Rust repos finished quickly. This should be profiled before using `clap` as a routine
   real-repo gate.

## Suggested Resume Sequence

1. Rebuild current binary:

   ```text
   cargo build --release -p nose-cli
   ```

2. Re-run a small strict generated smoke to make sure the baseline is still clean:

   ```text
   GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
   ```

3. Re-run the real-repo sample if `/tmp/nose-real-compare2/summary.json` is gone.

4. Pick the next frontier only after reviewing real examples. The best current candidate
   is Ruby `Hash#fetch(key, default)` under `map_default_lookup`, but it is not yet
   committed to the plan.

5. If implementing a new batch, record it in `bench/type4/ITERATIONS.md` and keep this
   handoff file updated at the end of the session.
