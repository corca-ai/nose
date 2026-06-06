# Field evaluation

*Part of the [home](home.md) wiki. The quantitative counterpart is [benchmark](benchmark.md); the
self-review on nose's own source is [dogfooding](dogfooding.md).*

This page records a qualitative, read-only pass over several unrelated real
codebases. The project names are intentionally anonymized: the point is whether
nose's findings are useful in realistic repositories, not to publish details of
local workspaces used during development. No project other than nose was
modified.

## Projects exercised

| project shape | languages | files scanned | Raw% | verdict |
|---|---|---:|---:|---|
| web app A | Svelte + TS | 172 | 0.000% | strong -- real cross-component duplication |
| collaboration app | TS + TSX | 1113 | 0.001% | strong -- exact 4-5x helper copies |
| research tooling | Python | 90 | ~0% | strong -- shared utility candidates |
| small CLI/game project | Python | 23 | 0.013% | good -- real shared test helper |
| clean frontend repo | TS + TSX | 169 | 0.002% | clean repo -> only 2 families |
| Go CLI | Go | 37 | 0.403% | works, but Go coverage was weak at the time |
| node-heavy Python project | Python | 629 | ~0% | strong -- near-duplicate API classes |

Some projects had far fewer scanned files than raw files on disk because
`.gitignore` correctly pruned vendored dependencies, virtualenvs, generated
models, and build output. That is a real adoption win, but a one-line "scanned N
files, ignored M" notice would build trust.

## What it gets right

- **Genuinely actionable findings**, not just noise. Examples seen:
  - exact DOM helper copies across sibling UI verifier modules;
  - near-identical API node classes in one large Python module;
  - repeated normalization/progress helpers in scripts;
  - cross-container duplication between Svelte components and TypeScript helpers.
- **Coverage is excellent for TS/JS/Python/Svelte** in these repos: Raw-node
  ratios were essentially zero.
- **Fast enough for interactive use**: hundreds of files scanned in well under
  100 ms, with no crashes in this pass.
- **Clean repos mostly stay quiet**, which matters as much as finding large
  duplication in noisy repos.

## What's missing for practitioner use

### P0 -- blocks real adoption

1. **Relative paths in output.** Reports should print paths relative to the
   scanned root or current directory so CI logs and review comments are portable.
2. **Baseline / incremental adoption.** Existing codebases often show many
   families; a fail-on-any gate is unusable until accepted duplication can be
   recorded and only *new* duplication is reported.
3. **Config file** (`nose.toml`). Real use needs committed settings for
   excludes, thresholds, `min-value`, and `min-members`.
4. **Test-awareness.** Test files can dominate reports. Duplication among tests
   is sometimes valuable, but production and mixed test<->production families
   should be easy to review separately.

### P1 -- quality and coverage

5. **Go coverage** was initially weak around composite literals, type syntax,
   slice expressions, type assertions, variadic arguments, and qualified types.
6. **Per-finding diff.** Reviewers need to know what differs between copies.
7. **Large-file extraction cost.** Very large functions/classes need profiling
   so extraction/value-graph work stays predictable.
8. **Cross-family dedup.** The same region can surface in adjacent families; the
   report should avoid making a reviewer inspect it twice.

### P2 -- integration and polish

9. **SARIF output** for GitHub code scanning and inline PR annotations.
10. **Inline suppression** (`// nose-ignore`) for consciously accepted clones.
11. **A short machine-readable summary** for CI logs.

## Bottom line

The core engine is useful on real repositories: it surfaced refactors a
maintainer would plausibly act on, across multiple languages and frontend
containers. The gap to adoption is mostly workflow and ranking ergonomics rather
than basic parsing or detection.

## Update -- backlog addressed

Most workflow items above have since landed:

- relative paths;
- `--baseline` and `--write-baseline`;
- `nose.toml` config;
- improved Go lowering;
- `--show diff`;
- large-file extraction improvements;
- cross-family dedup;
- `--format sarif`;
- inline `// nose-ignore`;
- `--fail-on any|new` CI gate;
- `--cache-dir` incremental cache.

### Ranking reworked -- extractability is the default

A second field pass (six unrelated real projects) found the old `value` ranking
over-rewarded a big block whose copies share little -- a 384-line family sharing
22 lines across 14 varying spots topped the list at a misleading `sim 1.00`,
above a tight `15/15`-shared pair. The default sort is now **extractability** --
how cleanly a family folds into one helper (invariant lines × copies × spread,
weighted by tightness and penalized by parameter count), with the report's
similarity cell replaced by an honest `N/M shared · Pp`. Same-language families
that share *no* invariant lines (a language idiom, or two unrelated type literals
of the same shape) now sink instead of topping the list. `--sort value` retains
the raw-volume ranking. This is not the abstractness re-rank §AU/§AV rejected:
measured on the v5 labelset it holds dev P@10 (61%) and lifts the **held-out**
split +6pp (54%→60%) with no recall cost -- it generalizes where the prototype
did not. See [experiments](experiments.md) §AZ.

The same pass drove four detector fixes (all landed): the contiguous copy-paste
channel is same-language by construction (no cross-language false merges), a
copy-paste run must contain at least one *operation* (flat name/field/literal
lists are skipped), window-shifted overlapping families are subsumed, and
`.gitignore` is honored even outside a git checkout.

### Test-awareness -- landed

Duplication between test and production code is a real smell and should be
reported. The nuance is duplication among tests: fixtures and arrange/act/assert
scaffolding are often duplicated on purpose, and test families can bury the
source-code signal.

This shipped as a ranking-time policy layer (experiments §U):

1. Each family is tagged `scope = prod | test | mixed` by a conservative path +
   unit-name heuristic (`test/`, `tests/`, `__tests__/`, `*_test.go`, `*.spec.*`,
   `*.test.*`, `conftest.py`, ...; see `is_test_loc` in `nose-detect/src/report.rs`).
2. Test-only families are **not** value-discounted anymore. That early discount hid real
   repeated test helpers, so it was reverted in [experiments](experiments.md) §U.1.
3. Nothing is dropped: the scope is shown in the report (`· in test code`,
   `· same code in tests and prod`) and serialized, so a reviewer can still separate
   test-only duplication from production/test leaks.

The remaining refactor-worthiness discount targets generated-looking and computation-poor
type-definition families, not test scope. See [usage](usage.md) for the scope tags.
