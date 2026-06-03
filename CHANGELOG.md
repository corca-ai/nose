# Changelog

All notable changes to nose are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); pre-1.0, so minor versions may
break.

## [Unreleased]

### Added
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
- **`nose scan`** — ranked architecture/design-level refactoring candidates:
  clone families sorted by refactoring value (removable lines × similarity ×
  cross-module/-file/-language spread). Human / JSON / Markdown / SARIF output;
  `--diff` shows source diffs between representatives, `--proposal` shows extraction
  skeletons with the differing parts marked as parameters.
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
