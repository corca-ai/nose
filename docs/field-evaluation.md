# Field evaluation

## Context-blind agent usability cycles — 2026-10-02

These qualitative studies simulate first-time agent users; they are not human
validation or statistical adoption evidence. Each fresh agent receives a maintainer
goal, an isolated export of a pinned corpus repository and an immutable binary.
Prior conversations, implementation details and previous feedback are withheld.
CLI help, emitted navigation and ordinary repository source remain available.
Record commands, outputs, errors, workarounds and unmet goals; independently replay
reported defects before treating feedback as evidence.

Following [Distorted user-centered design](https://wiki.g15e.com/pages/Distorted%20user-centered%20design),
the maintainer's requested solution is not the implementation specification.
Observe how they pursue the goal, identify the underlying obstacle, compare
solutions against evidence and product contracts, then test the selected change
with fresh agents. Preserve rejected hypotheses and experiment limitations.

The initial three users on ripgrep, click and radash all completed candidate
inspection, saved decisions and changed-checkout rechecks. Their comment edits
also relocated checkout roots, so those effects were confounded. Independent
replay rejected an alleged successful exit on a failed command: the user's logger
had captured a later command's status.

The first product cycle retained compact triage lists and made row commands open
family comparisons, rather than dumping every pair diff into the list. Human and
Markdown previews now bound long source lines and sections while retaining
collected JSON text. The same radash family had a longest output line of 12,199
characters before and 233 afterwards. All non-navigation JSON fields remained
equal on the three repositories before the separate admission fix below.
Fresh click and radash users found the comparison and saved their judgments;
both independently created two identical captures for their initial review.

The next cycle exposed the existing one-capture first-review workflow instead of
adding implicit live-state writes. Another fresh click user saved and reopened a
judgment with exactly one capture and no CLI errors. The user still found the
live-ID to captured-change-ID transition cognitively expensive; guidance improved
the workaround without establishing that the entire review workflow is optimal.

A fresh ripgrep user correctly distinguished exact-observation decision
applicability from incomplete population evidence. Byte inspection revealed that
legitimate ANSI expected-output literals caused an entire Rust source file to be
skipped. The solution corrects Rust source classification and invalidates stale
analysis caches, rather than suppressing that source or relaxing review reuse.
Raw ANSI sequences must all belong to valid literal/comment syntax nodes; real
highlighted output remains excluded. See [source classification](languages.md#source-classification-and-resource-errors)
and [cache migration](portable-cache-artifacts.md).
After that correction, another fresh ripgrep user independently captured the whole
checkout twice: each run scanned 101 files with zero skipped sources and captured
333 families. The comparison retained all 333 observations; the saved decision
was applicable and its eight before/after source lookups verified. The user also
identified remaining lowering gaps separately from discovery completeness rather
than claiming complete semantic understanding.

Commands, per-user feedback, immutable binary snapshots and design decisions for
these local runs are retained under ignored `target/corpus-user-study-2026-10-01/`
and `target/corpus-user-study-2026-10-02/`. These exploratory artifacts are not
checked release qualification receipts. Candidate refactor suggestions were not
implemented or independently adjudicated; no policy for automatic review transfer
was inferred from task completion.

## Ten additional sequential usability rounds — 2026-10-02

The user requested ten further iterations from `4ff31fa3`; earlier studies above
are excluded from this count. Ten fresh agents tested seven pinned repositories
with immutable per-round binaries, without conversation history or previous
implementation/feedback. Each round was followed by planner judgment, a concrete
improvement and verification before the next round. The reports are synthetic
agent-user observations, not human research or a measured adoption rate.
All ten maintainer goals were eventually completed; command failures, manual
workarounds and limitations remain in their individual transcripts.

The fixed philosophy is: nose supplies deterministic evidence; the caller owns
maintenance judgment and edit authorization. Keep triage compact, make detail
navigable, retain explicit capture-bound decisions and expose uncertainty.
Requests for automatic approval, inferred substitutes, automatic source copying,
caller graphs, competing command aliases or relaxed gates were not adopted.

| Round | Corpus | Observed obstacle | Planner-led improvement |
|---|---|---|---|
| 1 | radash | JSON review writes required reconstructing flags from help | Describe eligible recording prefix and required caller values |
| 2 | click | A path-filtered baseline write silently accepted all 165 families | Reject query terms on baseline writes; explain selective structured ignores |
| 3 | radash | The human review-start command retained an incompatible baseline option | Preserve detection settings but omit reporting suppressions for capture |
| 4 | boltons | Context required manual source searches | Discover existing bounded live context from family detail; keep caller search separate |
| 5 | ripgrep | An old change ID failed after a comment edit | Explain comparison address scope and explicit review recovery |
| 6 | cobra | Earlier manual narrowing exposed a route-dependent saved-review action | Bind reopening to the written target and replace stale status filters |
| 7 | axios | Configured multi-root JSON still lacked a first-capture entry | Expose explicit capture requirements and the original family lookup term |
| 8 | curl | Guessed capture/review commands failed; archive layout required preparation | Signpost the actual query workflow and give a concrete archive example |
| 9 | click | Tiny budget yielded 330 recheck observations on identical inputs | Explain incomplete search before counts; distinguish comparison from capture coverage |
| 10 | radash | Complete analysis coexisted with unavailable archived source bodies | Summarize explicit source verification separately, with unavailable files and sides |

The round 6 user succeeded through a retained live-ID filter. The preceding
round 4 user had manually narrowed the write result; an independent regression
without that live-ID filter reopened a different unreviewed family. Fixing this
concrete wrong-target route took priority over round 6's capture-entry suggestion,
which was validated again and implemented in round 7.

Observed boundary cases remain explicit. In round 5 a one-line comment produced
six recheck observations while 327 retained their evidence; the stored decision
remained recheck despite the caller's continuing keep-separate judgment. Four
historical/current body lookups verified. In round 9, increasing the comparison
budget changed 330 recheck observations into 165 retained observations and zero
rechecks; successful exit did not establish completed search. Round 10 restored
two missing CDN files: member-read counts changed from two verified/four unavailable
to six verified/zero unavailable, independently of analysis completeness. Candidate
refactor suggestions were recorded as caller intent and not applied to corpus code.

Harness limitations are retained: round 3 briefly resolved `nose` to another PATH
version; the failing capture action was independently reproduced with the pinned
binary. Later sessions prepended the pinned binary directory. The round 7 initial
root and exclusion pattern used `test` instead of the actual `tests`; those parent
fixture errors were separated from product defects, and the actual roots/policy
were recorded. These observations support particular navigation and reporting
changes, not a first-use success-rate or universal usability claim.

Commands, feedback, immutable binary manifests, before/after regression logs and
the decision ledger live under ignored `target/corpus-ut-ten-rounds/`. The stable
regressions are in `crates/nose-cli/tests/analysis_changes/ut_cycles.rs`. Source
verification is a reporting projection over explicit verified reads: it neither
reads files implicitly nor changes matching, family identities or review reuse.

## Second set of ten sequential usability rounds — 2026-10-02

This continuation starts at `f860e57d`; the earlier ten rounds are excluded.
Fresh agents receive pinned exports and frozen per-round binaries, with the same
philosophy and withheld context. Feedback is evaluated against independent
replays and existing contracts. These are simulated agent users, not human
validation. Corpus edits are confined to disposable exports and archives.

| Round | Corpus | Observation | Planner decision |
|---|---|---|---|
| 1 | black | Ignore schema required repeated guesses | Show minimal reason-bearing schema; preserve overlapping families |
| 2 | execa | Conventional tsd type tests appeared as production | Recognize bounded test-d conventions; invalidate derived-unit cache |
| 3 | fd | Cache reuse inferred from files | Expose existing opt-in stderr diagnostics, keeping JSON deterministic |
| 4 | ky | Mode change removed a candidate on unchanged source | Document policy compatibility separately from completed comparison |
| 5 | chi | JSON baseline acceptance emitted no stdout | Return a receipt for the persisted whole population; retain CI semantics |
| 6 | date-fns | Multi-root caller rebuilt an existing family action | Explain actions and config-relative resources; replay preserved policy |
| 7 | httpx | all still limited display to thirty rows | Explain top=0 and separate immutable review records |
| 8 | rich | Higher-work recovery omitted the original CI gate | Preserve original selection and gate; distinguish subset inspection |
| 9 | bat | Applicable review coexisted with stale archive | Explain independent fields and full-buffer digest; retain existing review action |
| 10 | httpie | Incomplete analysis and completed gate both exit 1 | Document fail-closed JSON validation; verify acceptance and positive/restored controls |

The test-path regression failed before its fix and passed afterwards. The
baseline receipt regression first failed on empty stdout. The recovery regression
first failed because the emitted retry omitted `--fail-on`; the corrected retry
retains both the gate and query selection and executes against a small fixture.
No work budget rises automatically and incomplete analysis emits no findings.
The final fresh caller parsed acceptance for 45 families over 122 Python files,
verified unchanged pass, introduced exact-copy failure and restored pass, without
changing the accepted baseline. The planner independently reran its restored CI
wrapper. All ten assigned goals completed; failures and manual recovery remain
in the logs. A new incomplete-analysis JSON envelope is deferred until its
versioning and broader error contract can be evaluated.

Round 4's default-to-syntax change produced 167 versus 156 families with the
same source; the old decision required rechecking. Round 7 combined two immutable
records into a 241-family queue with two applicable and 239 unreviewed decisions,
without suppressing any findings. Round 9 retained 189 captured families with
sixteen skipped highlighted fixtures: complete candidate search did not establish
complete coverage. Appending a comment to an archive changed six verified member
reads into six unavailable reads, while its current-observation decision remained
applicable. Restoring identical bytes recovered all six reads. Existing
`inspect-review` navigation reopened the correct applicable record independently.

Requests for new cache counters, automatic profile normalization, redundant
navigation, mutable aggregate decisions, automatic archive copying and partial
results were rejected. CLI/help discovery remains a learning cost; successful
agent completion does not establish universal usability or an adoption rate.
The round 6 parent initially assumed a conventional src layout before correcting
the monorepo fixture; this setup error is separate from product feedback.

Commands, feedback, binary/source manifests, regression logs and planner decisions
live under ignored `target/corpus-ut-next-ten-rounds/`. Stable regressions are in
`crates/nose-cli/tests/analysis_changes/ut_continuation.rs` and the detector's
bounded test-path tests. Final qualification records identify the actual tested
source snapshot separately from the original main-workspace HEAD.
The main workspace Git directory is read-only in the managed environment, so
source-bound gates run on an identical committed temporary checkout. An initial
target-directory symlink confused a sealed path check; local binary copies
replaced it. A subsequent file-length failure was fixed by extracting the
existing cache-size parser and tests, preserving the 599-line limit. Per-round
metadata terminators were normalized to valid JSON without changing binary or
source digests. Failed qualification attempts remain recorded as failures.
The final fast plan also encountered eight watch integration timeouts. A
single-thread control reproduced the timeout; both the frozen initial release
and final release emitted an initial snapshot but no edited-file revision in
this environment. These observations do not establish a new regression or its
root cause. Watch failures remained unresolved at that qualification, and its full fast
plan was not reported as passing. Non-watch contracts and remaining named gates are checked
separately; no watch tests or gate thresholds are relaxed.

A subsequent CI repair isolated the notification transport: a direct notify
probe received no native FSEvents event, while its content-polling watcher
received the same edit. Watch sessions now support an explicit positive
`NOSE_WATCH_POLL_INTERVAL_MS`; CI selects 100 ms while preserving the complete
Cargo test commands, assertions and timeouts. All ten watch tests passed in the
focused repair run, including a new same-size, same-modification-time edit.
The previous failed qualification remains historical evidence. Repair logs and
the new final qualification are retained under ignored `target/ci-watch-fix/`.

This page records a qualitative, read-only pass over several unrelated real
codebases. The project names are intentionally anonymized: the point is whether
nose's findings are useful in realistic repositories, not to publish details of
local workspaces used during development. No project other than nose was
modified. The quantitative counterpart is [benchmark](benchmark.md); the self-review on
nose's own source is [dogfooding](dogfooding.md).

## Projects exercised

| project shape | languages | files analyzed | Raw% | verdict |
|---|---|---:|---:|---|
| web app A | Svelte + TS | 172 | 0.000% | strong -- real cross-component duplication |
| collaboration app | TS + TSX | 1113 | 0.001% | strong -- exact 4-5x helper copies |
| research tooling | Python | 90 | ~0% | strong -- shared utility candidates |
| small CLI/game project | Python | 23 | 0.013% | good -- real shared test helper |
| clean frontend repo | TS + TSX | 169 | 0.002% | clean repo -> only 2 families |
| Go CLI | Go | 37 | 0.403% | works, but Go coverage was weak at the time |
| node-heavy Python project | Python | 629 | ~0% | strong -- near-duplicate API classes |

Some projects had far fewer analyzed files than raw files on disk because
`.gitignore` correctly pruned vendored dependencies, virtualenvs, generated
models, and build output. That is a real adoption win, but a one-line "analyzed N
files, ignored M" notice would build trust.

## What it gets right

- **Genuinely actionable findings**, not just noise. Examples seen:
  - exact DOM helper copies across sibling UI verifier modules;
  - near-identical API node classes in one large Python module;
  - repeated normalization/progress helpers in scripts;
  - cross-container duplication between Svelte components and TypeScript helpers.
- **Coverage is excellent for TS/JS/Python/Svelte** in these repos: Raw-node
  ratios were essentially zero.
- **Fast enough for interactive use**: hundreds of files analyzed in well under
  100 ms, with no crashes in this pass.
- **Clean repos mostly stay quiet**, which matters as much as finding large
  duplication in noisy repos.

## What's missing for practitioner use

### P0 -- blocks real adoption

1. **Relative paths in output.** Reports should print paths relative to the
   analyzed root or current directory so CI logs and review comments are portable.
2. **Baseline / incremental adoption.** Existing codebases often show many
   families; a fail-on-any gate is unusable until accepted duplication can be
   recorded and only new or changed duplication is reported.
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
compiled builtin `nose.value_graph.laws` pack across the 105-repo
`bench/repos` corpus, plus a targeted 10-repo subset selected for clamp/min-max
and arithmetic-law surfaces. The pack was active in all 104 successful full-corpus
query runs, covering 59,865 source files and 10,967 reported families, but no family
contained `semantic_laws` provenance. The checked-in audit recorded `rxjs`
separately because it hit a scanner stack overflow at the time; follow-up #198
fixed that crash, and the current semantic JSON corpus loop completes 105/105
repos.

The qualitative read is a negative field result, not a pack-loading failure:
real code contains many clamp-shaped idioms (`fzf` generic `Constrain`,
`pixijs` `Math.min(Math.max(...))`, Rust `.clamp`), but current provenance only
appears when a proof-backed law actually participates in a reported clone
family. The next useful layer is miss-mining for singleton law-shaped candidates
and proof blockers, not merely another broad clone query.

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
weighted by tightness and penalized by parameter count and member-span heterogeneity —
#365/§CM), with the report's
similarity cell replaced by an honest `N/M shared · Pp`. Same-language families
that share *no* invariant lines (a language idiom, or two unrelated type literals
of the same shape) now sink instead of topping the list. `sort=value` retains
the raw-volume ranking. This is not the abstractness re-rank §AU/§AV rejected:
the historical §AZ run recorded a held-out lift for extractability. The current
reproducible v5 evaluator and its confidence intervals are summarized in [benchmark](benchmark.md).

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
   unit-name heuristic (`test/`, `tests/`, `__tests__/`, `spec/`, `*_test.go`,
   `test.rs`, `tests.rs`, `*.spec.*`, `*.test.*`, `conftest.py`, ...; see
   `is_test_loc` in `nose-detect/src/report/paths.rs`).
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
claim for normal repositories — every project analyzed in ≤ 0.13 s end-to-end,
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
   the same file is analyzed in **≈ 2 s** — with small-repo query runs getting ~3× faster
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

The practical advice stands: analyze source roots, not build output — but a
committed bundle must degrade gracefully, and now it does.

## Fourth pass follow-up — imported call-target proof cache

The 105-repo bench corpus pass exposed a second real-world performance cliff:
`commons-lang` analyzed 620 files, but one large Java test file
(`ArrayUtilsTest.java`) made `normalize+extract` take **≈119 s** while
parse/lower and detector clustering stayed below 0.1 s and 0.02 s respectively.
Sampler output showed almost all CPU under imported call-target occurrence
validation, repeatedly proving that the same imported binding was still visible
and not locally shadowed.

Imported occurrence validation now reuses a per-file function/local-shadow cache.
The same `commons-lang` semantic query runs `normalize+extract` in **<1 s** with
identical family ids; the single pathological `ArrayUtilsTest.java` query dropped
from **≈119 s** to **≈0.8 s**. This keeps the semantic-kernel fail-closed proof
policy intact while removing the quadratic proof-validation shape.

## Fourth pass follow-up — strict-nullish regression pins

The same corpus comparison also exposed two precision wins from the semantic
kernel hardening: a nullish default family in a formatter-style JavaScript helper
no longer merged with `x === null ? d : x`, and an object-guard family no longer
accepted a loose `candidate != null` guard as equivalent to a strict
`candidate !== null` guard.

Those are now covered by compact CLI fixtures instead of depending on checked-out
third-party repositories. The fixtures run `nose query --mode semantic` over small
JavaScript projects and assert that the exact semantic channel keeps
loose-nullish and strict-null families separate while preserving the positive
families on each side.

## Fourth pass follow-up — diagnostic surface volume

The 18-repo semantic sample intentionally used `--format json top=0`, so it
captured diagnostic families as well as the default human action surface. The
experiment rollup was:

- 940 semantic families total;
- 277 `surface = "default"` families;
- 48 `review` families;
- 615 `hidden` families;
- 657 families with at least one exact fragment location.

That hidden/review volume is expected: exact fragments are proof and review
substrate, not automatically top-level refactoring recommendations. The
integration contract now makes that explicit through each family's `surface`
field; full `top=0` query JSON can be counted before display truncation.

## Fourth pass follow-up — fragment quality audit

The Java/Python hidden/review volume was sampled rather than tuned by guesswork. A
three-reviewer audit of 20 exact-fragment candidates is recorded in
[fragment-quality-audit-2026-06-10](fragment-quality-audit-2026-06-10.md), with the
machine-readable votes in [bench/labels/fragment_quality_audit_2026_06_10.json](../bench/labels/fragment_quality_audit_2026_06_10.json).

The result was mostly validating for the semantic kernel: 17/20 candidates were
consensus correct diagnostic substrate, but 15/20 were too small or scaffold-like to be
action output. The `review` surface had no consensus noise and kept the two useful
signals in the sample: a long RxJava2/RxJava3 adapter constructor parity fragment and a
larger Poetry authenticator test setup fragment. The policy change from the audit is
narrow: tiny test-only exact fragments with enclosing-unit context now stay hidden
instead of review-visible. Broader pruning is deferred; #199 closed the stable
`family_id` collision follow-up by including location spans and fragment metadata in
query JSON IDs. The remaining follow-up is overly generic one-line direct-return
fragments.
