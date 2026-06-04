# Changelog

All notable changes to nose are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); pre-1.0, so minor versions may
break.

## [Unreleased]

### Added
- Strict Type-4 proof facts for case-sensitive string prefix/suffix predicates across
  Go, Java, JavaScript/TypeScript, Python, Ruby, Rust, and embedded script surfaces.
  The benchmark now includes same-surface and cross-surface positives plus affix,
  direction, and wrong-receiver hard negatives.
- Type-4 focused generation filters (`--axis`, `--proposal-prefix`) and smoke gates
  (`GATE=focused|core|full`) so detector co-evolution loops can run on one frontier
  before periodic compact/full validation.

### Changed
- Type-4 frontier prioritization now separates true uncovered broad-probe gaps from
  filtered probe overreach, so pattern loops do not promote non-strict examples just
  to improve apparent coverage.
- Type-4 frontier prioritization can now reuse cached corpus analysis and reports top
  matching repos per candidate for targeted real-repo audits.
- Type-4 manifest evaluation and frontier summaries now index scan locations by file,
  making full-manifest validation practical after corpus growth.

## [0.3.0] - 2026-06-04

### Added
- **Exact Type-4 (semantic-clone) convergence pass** — more behaviorally-equivalent
  code now converges to one value-fingerprint, with each new algebraic law
  machine-checked in Lean and the soundness contract held (full-corpus `nose verify`
  stays **0 false merges**, canon-preserved):
  - **Distribution / factoring** `a*c + b*c ≡ (a+b)*c` (numeric, Lean `distrib_sound`)
    and full associative-commutative flatten+sort in the value graph itself.
  - **Filter fusion** `filter q (filter p) ≡ filter (p∧q)` via an element-carrying
    filter representation (Lean `filter_fusion`) — unifies nested filters, a
    `.filter().filter()` chain, and the filtered builder loop.
  - **Reduce-lambda selection** (`reduce(λ. a if a>b else b) ≡ max`), **count-of-filter**
    (`len([… if p]) ≡ Σ(p?1:0)`, Lean `filter_length_eq_count`), and Rust method-form
    iterator reductions (`.sum()/.min()/.max()/.count()`).
  - **Dict-builder loop ≡ dict comprehension** (`d={}; for x: d[k]=v` ≡ `{k: v for x}`)
    via a `DictEntry`-distinct representation that cannot collide with a list of tuples.
  - **Stronger IL type inference** — a fixpoint over subexpression result types — gating
    the numeric rewrites soundly.

### Formal
- New machine-checked Lean proofs: `Algebra.lean::distrib_sound`,
  `Functor.lean::{filter_fusion, filter_length_eq_count}`, and a new `Compare.lean`
  (comparison-direction and negated-comparison canons). A CI `formal` job checks
  `formal/*.lean` on every push.

## [0.2.0] - 2026-06-04

### Changed
- **`nose scan --mode` is now channel-based**: `syntax` (Type-1/2 copy-paste),
  `semantic` (exact value-fingerprint Type-4), and `near` (Type-3 fuzzy
  near-duplicates). Omitting `--mode` runs `syntax,semantic`; specifying `--mode`
  runs exactly the comma-separated/repeated channels provided.
- Removed the old `cpd`, `refactor`, `behavior`, and `behavior-strict` scan modes
  and removed `--no-contiguous`. `--threshold` is now valid only when `near` is
  enabled.
- The `near` channel now uses shape-based candidate generation, so Type-3 edits
  that change value fingerprints still reach fuzzy scoring instead of being
  filtered out before scoring.
- Documentation and CLI help now spell out that omitting `--mode` means
  `syntax,semantic`, while specifying `--mode` replaces that default exactly.

## [0.1.1] - 2026-06-04

### Fixed
- **`nose scan --top 0` now shows all families**, as `docs/usage.md` and
  `docs/benchmark.md` document. The code used `.take(top)` with no special case,
  so `--top 0` silently returned an empty report; `0` is now treated as unlimited,
  the flag help says so, and a regression test covers it.

### Docs
- Documented Homebrew / prebuilt-binary install and the cargo-dist release process
  (`README.md`, `CONTRIBUTING.md`).
- Added `AGENTS.md` (with `CLAUDE.md` as a symlink) per the Corca convention; the
  release process now includes the CHANGELOG step that this release recovers.

### Tooling & quality gates
- **`awiki` docs-wiki connectivity gate** — `awiki lint --root docs` keeps `docs/`
  a single connected graph (no orphan pages). Wired into `scripts/check-docs.sh`,
  `scripts/check.sh`, the `.githooks/pre-commit` hook, and the CI `docs` job, using
  the same skipped-with-notice pattern as the other optional-tool gates.

## [0.1.0] - 2026-06-04

### Added
- **`nose scan --mode`** — four explicit scan modes: `cpd` (copy-paste channel only,
  jscpd-style CI gate), `refactor` (the default broad refactoring-candidate workflow),
  `behavior` (strict behavioral scorer with the calibrated 0.86 threshold), and
  `behavior-strict` (exact value-fingerprint Type-4 matches plus the copy-paste floor,
  with no fuzzy similarity threshold).
- **Independent soundness oracle** (`nose verify`) — the value-graph contract is
  *fingerprint-equal ⟹ behavior-equal*; a tree-walking interpreter runs every unit on an
  input battery and flags any fingerprint-equal pair whose behavior differs. It interprets
  the **pre-canonicalization core IL** (not the IL it fingerprints), so a behavior-changing
  canon cannot mask itself, plus a **canon-preservation** check (core-IL behavior must
  equal full-IL behavior — catches a bad canon with no colliding twin). Both report zero
  violations. See Experiments §AJ/§AX.
- **Machine-checked canons** (`formal/`, Lean 4) — the core algebraic/control/functor/
  min-max/boolean-reduction canonicalizations are proven behavior-preserving (no `sorry`):
  AC-operand canon, `sub`→`add+neg`, neg-distribution, guard-clause, dead-code-after-return,
  ternary-return decomposition, map fusion/identity, min/max monoid, and the `any`/`all`
  OR/AND monoids.
- **Purpose-fit type inference** (`types.rs`) — infers `Num | Bool | Str | List | Unknown`
  per parameter from strictly-typed uses, gating the type-dependent canons (commutativity,
  identity elimination, double-negation, idempotence).
- **Cross-language `any`/`all`** — Python `any(p(x) for x in xs)`, JS `xs.some(p)`, and
  Rust `xs.iter().any(p)` (and `all`/`every`/`.all`) converge to one canonical boolean
  short-circuit reduction. Free-monoid string model, map/filter fusion, and a
  ternary-return decomposition (`return a if c else b` ↔ `if c {return a} else {return b}`)
  also landed on the value graph.
- **`nose scan`** — ranked architecture/design-level refactoring candidates.
  Human / JSON / Markdown / SARIF output; `--diff` shows source diffs between
  representatives, `--proposal` shows extraction skeletons with the differing parts
  marked as parameters.
- **`nose scan --sort`** — `extractability` (default), `value`, or `sites`.
  The default ranks by how cleanly a family folds into one helper — *invariant*
  (shared) lines × copies × spread, weighted by **tightness** (shared/total: 22 shared
  of 384 lines is 6% invariant — barely a dedup) and penalized by parameter count —
  instead of raw duplicated volume, which over-rewards a big block whose copies share
  little. The all-type-definition / all-generated **discount and `.d.ts` exclusion**
  now apply to extractability too (they previously only touched `value`). Same-language
  families with **no** shared invariant lines (a language idiom like Go
  `if err != nil { return err }`, or two unrelated type literals of the same shape) have
  nothing to extract and sink to the bottom — they no longer top the list at a
  misleading `sim 1.00`. Field evaluation across six real projects drove these fixes.
- **Honest shared-line reporting** — the report's similarity cell now shows `N/M
  shared · Pp` (invariant lines of total, with P parameter spots) for same-language
  families, computed by the same anti-unification as `--proposal`. Replaces a bare
  `sim 1.00` that read as "identical" even when two copies shared only a handful of
  literal lines (a dispatch skeleton over divergent bodies). Cross-language families,
  which share no *source* lines, still show structural `sim`.
- **`scanned N files` scope line** — `scan`'s human/Markdown output now opens with the
  file count and per-language breakdown (e.g. `scanned 1113 files · typescript 900 · tsx
  213`). A repo whose `.gitignore`/`--exclude` pruned vendored or generated code scans
  far fewer files than sit on disk; the count makes that scope explicit instead of a
  silent gap. JSON/SARIF output is unaffected; the language breakdown is omitted under
  `--cache-dir` (which tracks only the count).
- **Refactoring-candidate mode** (`--candidates` on `detect`, default for `scan`):
  gates off + lower threshold, ~99% review-worthy on a refactoring-worthiness rubric.
- **Rust, Java, C, and Ruby frontends** — 8 base languages (Python, JS, TS, Go,
  Rust, Java, C, Ruby). Cross-language convergence (a Rust/Java/C/Ruby accumulator
  loop normalizes to the same IL as the Python one).
- **JSX/TSX, and embedded `<script>` in Vue / Svelte / HTML.** The embedded frontend
  extracts each `<script>` block and blanks the surrounding markup to whitespace
  (newlines kept), so the script parses as JS/TS in place with exact line numbers;
  `lang="ts"` selects TypeScript. The same logic in a `.ts`, `.vue`, `.svelte`, and
  `.html` file forms one cross-container clone family.
- `nose scan --min-value V` to hide low-value families (noise floor on large repos).
- `nose scan --hotspots` — architecture view ranking directories by the lines that
  sit in a clone family (e.g. surfaces `zod/.../locales`, translation/locale dirs).
- Per-family **refactoring hint** (e.g. "consolidate `name` — N copies", "extract a shared
  base class / mixin", cross-language flag) and the languages a cross-language family spans.
- `--version`; richer CLI help; LICENSE.

### Changed
- **Contiguous copy-paste channel is same-language by construction** — its k-gram
  table is keyed by `(hash, language)`, so a literal-copy-paste family can no longer
  span languages (you don't copy-paste TS into a `.mjs`; cross-language equivalence is
  Type-4, recovered by the value-graph channel). Removes a class of false cross-language
  merges (unrelated functions grouped by a shared normalized-IL token run) that the new
  ranking couldn't catch — cross-language families bypass the shared-line check. Also
  stops a collision in one language from masking a real same-language match in another.
- **Overlapping families merge** — a family whose sites are a window-shifted overlap
  (≥60%) of a larger family's sites is now subsumed, not reported twice. Previously
  only strict containment collapsed; the contiguous channel finding the same run at a
  few different start lines surfaced as several near-identical families.
- **`.gitignore` is respected even outside a git checkout** (`require_git(false)` on
  the walker), so an extracted tarball / vendored sub-tree honors its own `.gitignore`
  instead of leaking generated and vendored files into the report.
- **Detector modes split**: strict behavioral-clone mode (precision gates on, the
  default for `detect`) vs candidate mode. Behavioral precision raised ~6%→~78%
  (unbiased, judge-validated) via string-literal value retention, RANSAC re-weighting,
  data-table & return-signature gates, class-attribute capture in the value graph.
- Default behavioral threshold 0.86 (balanced operating point from the precision curve).
- **Refactoring value is fanout-aware**: the copy count is dampened beyond a small
  knee (square-root tail), so a fragment repeated across hundreds of sites
  (generated boilerplate, test scaffolding) no longer dominates the ranking over
  genuine few-site refactors. Fixed garbage-at-top on large corpora (a 421-site
  Javadoc family and a 541-site spec-scaffolding family ranked #1–2). The reported
  `dup_lines` estimate is unchanged (honest `mean_lines × (members−1)`); only the
  ranking score is dampened.
- **Contiguous copy-paste channel is value-sensitive**: it now keys on literal
  *values* (string hash / int / bool), not the abstract literal class, so two
  *different* data tables (distinct HTML-entity / locale maps) no longer collapse
  into one giant cross-file/cross-language family. Aligns the channel with how a
  raw-token detector behaves; token-detector-superset coverage held at 90.9%.

### Fixed
- **`--cache-dir` no longer drops copy-paste clones.** The on-disk cache stored only
  each file's value-graph units, so a cached run executed *only* the value-graph
  channel and silently omitted every contiguous (Type-1/2 copy-paste) family —
  e.g. radash reported 148 families uncached but 92 with `--cache-dir`, despite the
  cache being documented as a transparent speed-up. The cache now also stores each
  file's contiguous token stream (content-derived, so cacheable by source hash like
  the units), and the copy-paste channel runs from it. Cached output is byte-identical
  to a normal scan again (verified across the corpus). Cache schema bumped, so existing
  caches repopulate.
- **Byte-identical output restored across thread counts.** Three latent
  nondeterminism sources let `scan`/`verify` output vary with `RAYON_NUM_THREADS`
  (and the per-process hash seed) on some repos, violating the determinism
  guarantee: (1) the honest shared-line ranking summed `idf` weights over lines in
  `HashMap` order, so float-add non-associativity perturbed `shared_weight` (and,
  via sort ties, family order); (2) the RANSAC aligner picked its consensus offset
  with `max_by_key(votes)`, and a vote-count tie resolved by the reused thread-local
  map's capacity-dependent iteration order — fixed by breaking ties on the offset
  value; (3) `nose verify`'s under-merged-clones diagnostic iterated `HashMap`s into
  its output. A determinism sweep over the 105-repo corpus now reports **0**
  nondeterministic repos for `scan` and `detect` (was 4 for `scan`). A stronger
  cross-thread-count regression test (8 families × 5 near-duplicate copies) guards
  the class.
- **`--proposal` no longer overstates family-wide overlap.** The skeleton is a
  pairwise anti-unification of the two largest copies; for families with more copies
  it now says so (`… of the 2 largest of N copies; the rest may share fewer`), so it
  no longer silently contradicts the family summary's majority-shared count.
- Refactoring families collapse overlapping/nested sites (a function and its inner
  block, or near-identical off-by-one spans) into one site — accurate site counts
  and dup-line estimates.
- **Value-graph soundness — the "treat a non-commutative op as commutative" bug class**
  (Experiments §AX). The independent oracle (above) exposed a class of latent false merges
  the old same-IL oracle had masked — 11 fingerprint collisions, plus 20 behavior-changing
  units the new canon-preservation check caught — all fixed at the root: short-circuit
  value-`and`/`or` are
  associative but NOT commutative (`1 or 2`≠`2 or 1`) — no longer sorted, and now correctly
  lazy in the interpreter; `!!x` is `bool(x)` not `x` (cancelled only on Bool);
  `not(Err)` propagates the error instead of yielding `true`; `x+0`/`x*1` identity
  elimination is dropped (unsound for non-Num, and untypeable — the optimistic inference
  would self-justify it); and string `+` (concatenation) operands are never reordered. A new
  negated-comparison canon (`!(a<=b)`→`a>b`) converges what double-negation pushes.
- **Value-graph soundness — eight false merges fixed** (behaviorally-different code
  that shared a fingerprint; the behavioral fingerprint is sound by intent, see
  Experiments §AS/§AT and the Normalization soundness note): loop iteration-extent was
  dropped (`range(len)` ≡ `range(1,len)`, `i+=1` ≡ `i+=2`, early `break` ≡ full loop);
  slice/range bounds collapsed (`a[1:]` ≡ `a[:1]` in Python/Go/Rust, `1..2` ≡ `1..=2`);
  alpha-renaming collapsed distinct globals/callees (`foo(x)` ≡ `bar(x)`, `max` ≡ `min`);
  boolean literal *values* were discarded (`True` ≡ `False`); and `in`/`is` → `Op::Eq`
  merged membership with equality and dropped negation (`x is not None` ≡ `x is None`).
  Fixes added `Op::In` (non-commutative, list-membership interpretable) and
  `Payload::LitBool`, and made the slice/`range`/`++` lowerings position- and
  value-preserving. Each has a `tests/equivalence.rs` reproducer.
- **Convergence bugs surfaced by cross-language tests** (each broke matching):
  - Rust `*x` deref was mislowered as `UnOp(Neg)`; now peels to its operand (`*x > 0`
    matches a plain `x > 0`).
  - Python f-strings (`f"hi {name}"`) and Ruby interpolation (`"hi #{name}"`) dropped
    the interpolated expression, lowering to an opaque literal; both now lower to a
    string-concat chain that converges with a JS template literal.
  - `cfg_norm` branch orientation inverted comparisons to non-canonical operators
    (`Lt`→`Ge`), so `if a<b {X} else {Y}` never converged with `if a>=b {Y} else {X}`;
    it now stays in the canonical `Lt`/`Le`/`Eq`/`Ne` set (operands swapped as needed).
  - Python `lambda x: e` lowered a bare-expression body while JS arrows wrap theirs
    in `Block(Return(e))`; the lambda now uses the same canonical shape, so
    `lambda x: e` ≡ `x => e`.
- A convergence test matrix (one algorithm × N languages → one IL hash) now guards
  these and the documented equivalences (loop forms, ternary/switch, comprehension/
  `.map`, conjoined/continue guards, De Morgan, optional chaining, try/except).

### Performance
- RANSAC alignment reuses per-thread scratch (scoring −37%).
- Threshold early-exit skips alignment for un-acceptable pairs (scoring 4–6× faster).
- Thread-local parser pool: one `tree_sitter::Parser` per grammar per worker instead
  of one per file (lowering ~1.8× faster — the dominant stage on large corpora).
- Every pipeline stage is parallel: parallel file discovery (`ignore`'s walker),
  sort-based parallel LSH candidate-gen (22→6 ms), fused normalize+extract (~halves
  peak IL memory). parse+lower scales 11.6× on 18 cores. **~14k → ~19.5k files/sec**
  on the 3620-file corpus; deterministic across runs, threads, *and* machines.
- `nose scan --cache-dir <dir>` — opt-in on-disk cache of per-file units keyed
  by content hash; ~1.6× faster re-runs on unchanged files (output byte-identical).

### Tooling & quality gates
- Centralized `[workspace.lints]` (rust + clippy) inherited by every crate;
  `unsafe_code = "forbid"`. `unreachable_pub` narrowed 73 over-exposed `pub` items
  to `pub(crate)`.
- `cargo-machete` (unused-dependency gate) — removed 3 unused deps.
- `cargo-deny` (`deny.toml`): security advisories, license allow-list, no
  duplicate/wildcard deps, crates.io-only sources.
- Broken-intra-doc-link gate (`RUSTDOCFLAGS=-D warnings cargo doc`); fixed the links
  it caught.
- **Copy-paste gate** (`scripts/check-duplication.sh`) — nose run on its own source,
  ratcheted to a committed budget; the clone detector polices its own duplication.
- `rust-toolchain.toml` pins the dev/CI toolchain (1.96.0); **MSRV 1.85** declared
  (`rust-version`) and checked by a dedicated CI job (floor set by the dependency
  tree's `edition2024` requirement).
- One-command local runner `scripts/check.sh`; all gates wired into CI; documented
  in `CONTRIBUTING.md`.
- Automated dependency updates via Dependabot (`.github/dependabot.yml`).
- **IR verifier** (`Il::validate`) run under `debug_assert!` after normalization —
  the LLVM-`verify`/MIR-validator analogue. Normalization proven idempotent
  (fixpoint) by test.

### Internal
- Self-hosted benchmark corpus under `bench/repos` (pinned commits; see
  `bench/setup_repos.sh`) — no dependency on sibling projects.
- Dogfooded on its own code (`docs/dogfooding.md`); acted on real findings — extracted
  shared `lower::{binary, while_loop, collect_into, function_unit, switch_to_if_chain,
  lower_file}` and `normalize::collect_scope` across the frontends/passes.
