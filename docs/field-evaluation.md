# Field evaluation

This page records a qualitative, read-only pass over several unrelated real
codebases. The project names are intentionally anonymized: the point is whether
nose's findings are useful in realistic repositories, not to publish details of
local workspaces used during development. No project other than nose was
modified. The quantitative counterpart is [benchmark](benchmark.md); the self-review on
nose's own source is [dogfooding](dogfooding.md).

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

## LawPack provenance audit -- 2026-06-10

The [LawPack provenance audit](lawpack-provenance-audit-2026-06-10.md) ran the
compiled first-party `nose.value_graph.laws` pack across the 105-repo
`bench/repos` corpus, plus a targeted 10-repo subset selected for clamp/min-max
and arithmetic-law surfaces. The pack was active in all 104 successful full-corpus
scans, covering 59,865 source files and 10,967 reported families, but no family
contained `semantic_laws` provenance. The checked-in audit recorded `rxjs`
separately because it hit a scanner stack overflow at the time; follow-up #198
fixed that crash, and the current semantic JSON corpus loop completes 105/105
repos.

The qualitative read is a negative field result, not a pack-loading failure:
real code contains many clamp-shaped idioms (`fzf` generic `Constrain`,
`pixijs` `Math.min(Math.max(...))`, Rust `.clamp`), but current provenance only
appears when a proof-backed law actually participates in a reported clone
family. The next useful layer is miss-mining for singleton law-shaped candidates
and proof blockers, not merely another broad clone scan.

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
the historical §AZ run recorded a held-out lift for extractability. The current
reproducible v5 evaluator and its confidence intervals are summarized in
[benchmark](benchmark.md).

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

## Third pass — performance pathology and an oracle-exposed false merge

A third read-only pass (ten unrelated local projects: Go CLIs, TypeScript
games/tools, Python services, mixed monorepos) confirmed the interactive-speed
claim for normal repositories — every project scanned in ≤ 0.13 s end-to-end,
lowering coverage stayed under 0.1 % Raw, and the semantic channel's findings
were true positives on inspection (e.g. two identically-shaped `` `${x},${z}` ``
key-builder methods in different modules of a TypeScript game; a duplicated
HSL→RGB conversion helper surfaced by the near channel).

Two substantive findings came out of this pass:

1. **Minified bundle artifacts were a performance cliff.** One monorepo carried
   committed build output (a multi-megabyte minified `*.js` bundle). A single
   246 KB minified file took **227 s** in normalize+extract: `nearest_scope`
   and the evidence-record lookups were linear scans *per query*, which goes
   quadratic when one file has hundreds of thousands of IL nodes and evidence
   records. Both are now lazy per-file indexes (a whole-arena
   nearest-enclosing-scope table and an exact-anchor-span evidence index), and
   the same file scans in **≈ 2 s** — with small-repo scans getting ~3× faster
   normalize+extract as a side effect. The fix was profiler-driven
   (`sample` on the live process; `NOSE_TIME=1` stage timing).
2. **The oracle blind spot it closed found a real false merge.** Making
   2-argument `min`/`max` interpretable let `nose verify` check the
   selection-reduction convergences for the first time — and it immediately
   flagged `max(x + y for x in xs for y in ys)` ≡ a `best = 0`-seeded nested
   loop as a false merge (the seed clamps: empty or all-negative input returns
   0, true `max(...)` errs or goes negative). Selection reductions now carry
   their seed in the fingerprint; seedless builtin forms stay 1-arg, so
   equally-seeded loops still converge with each other and the mislabeled
   benchmark case was flipped to a hard negative.

The practical advice stands: scan source roots, not build output — but a
committed bundle must degrade gracefully, and now it does.

## Fourth pass follow-up — imported call-target proof cache

The 105-repo bench corpus pass exposed a second real-world performance cliff:
`commons-lang` scanned 620 files, but one large Java test file
(`ArrayUtilsTest.java`) made `normalize+extract` take **≈119 s** while
parse/lower and detector clustering stayed below 0.1 s and 0.02 s respectively.
Sampler output showed almost all CPU under imported call-target occurrence
validation, repeatedly proving that the same imported binding was still visible
and not locally shadowed.

Imported occurrence validation now reuses a per-file function/local-shadow cache.
The same `commons-lang` semantic scan runs `normalize+extract` in **<1 s** with
identical family ids; the single pathological `ArrayUtilsTest.java` scan dropped
from **≈119 s** to **≈0.8 s**. This keeps the semantic-kernel fail-closed proof
policy intact while removing the quadratic proof-validation shape.

## Fourth pass follow-up — strict-nullish regression pins

The same corpus comparison also exposed two precision wins from the semantic
kernel hardening: a nullish default family in a formatter-style JavaScript helper
no longer merged with `x === null ? d : x`, and an object-guard family no longer
accepted a loose `candidate != null` guard as equivalent to a strict
`candidate !== null` guard.

Those are now covered by compact CLI fixtures instead of depending on checked-out
third-party repositories. The fixtures run `nose scan --mode semantic` over small
JavaScript projects and assert that the exact semantic channel keeps
loose-nullish and strict-null families separate while preserving the positive
families on each side.

## Fourth pass follow-up — diagnostic surface volume

The 18-repo semantic sample intentionally used `--format json --top 0`, so it
captured diagnostic families as well as the default human action surface. The
current output shape was:

- 940 semantic families total;
- 277 `recommended_surface = "default"` families;
- 48 `review` families;
- 615 `hidden` families;
- 657 families with at least one exact fragment location.

That hidden/review volume is expected: exact fragments are proof and review
substrate, not automatically top-level refactoring recommendations. The
integration contract now makes that explicit by documenting
`recommended_surface == "default"` as the human-action filter and by emitting
`ranking.surface_counts`, including a nested exact-fragment breakdown, before
`--top` truncation.

## Fourth pass follow-up — fragment quality audit

The Java/Python hidden/review volume was sampled rather than tuned by guesswork. A
three-reviewer audit of 20 exact-fragment candidates is recorded in
[fragment-quality-audit-2026-06-10](fragment-quality-audit-2026-06-10.md), with the
machine-readable votes in
[bench/labels/fragment_quality_audit_2026_06_10.json](../bench/labels/fragment_quality_audit_2026_06_10.json).

The result was mostly validating for the semantic kernel: 17/20 candidates were
consensus correct diagnostic substrate, but 15/20 were too small or scaffold-like to be
action output. The `review` surface had no consensus noise and kept the two useful
signals in the sample: a long RxJava2/RxJava3 adapter constructor parity fragment and a
larger Poetry authenticator test setup fragment. The policy change from the audit is
narrow: tiny test-only exact fragments with enclosing-unit context now stay hidden
instead of review-visible. Broader pruning is deferred; #199 closed the stable
`family_id` collision follow-up by including location spans and fragment metadata in
scan JSON IDs. The remaining follow-up is overly generic one-line direct-return
fragments.
