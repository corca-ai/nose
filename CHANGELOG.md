# Changelog

All notable changes to nose are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); pre-1.0, so minor versions may
break.

## [Unreleased]

### Performance
- **Interactive `nose scan` no longer pays for the graded witness it does not show.**
  The graded witness (#315) is serialized only by `--format json`; the human and SARIF
  surfaces never render it. Enrichment now runs only when JSON is emitted, so a default
  human scan skips it entirely — ~2.8s of a ~4.6s `--mode near` scan on netty (3249 near
  families), now ~1.9s. JSON output is unchanged. Referent resolution in the witness is
  also indexed (sorted call-target evidence + a name-by-span map) instead of an
  O(units × evidence) scan, and the anti-unification hot path no longer clones argument
  vectors. A `NOSE_TIME`-gated `enrich` stage timing was added.

### Fixed
- **False merge closed: float subtraction is no longer reassociated (#283 C-float, partial).**
  A `-` carrying a proven-float operand (a float literal, a `/` true-division result, or a
  float-typed param) is now kept as a literal `Sub` rather than routed through the
  associative `+` normalization (`a - b` ≡ `a + (-b)`) — so `(1e100 + x) - 1e100` (≈ 0.0, the
  large term swallows `x`) no longer shares an `exact-value-graph` fingerprint with the
  regrouped `(1e100 - 1e100) + x` (= x). Integer subtraction still normalizes and converges;
  corpus family delta is 0. The pure-`+`/`*` float case (`(a+b)+c ≡ a+(b+c)`) is NOT closed:
  the fingerprint flattens AC chains to a leaf sequence, so it is grouping-insensitive by
  design and needs the Float value kind, not a canon gate (the finding, recorded in
  [oracle-value-model §3.3](docs/oracle-value-model.md)).
- **False merge closed: dynamic module rebind via `globals()['f'] = …` / `setattr` (#307).**
  Reassigning a module function without a `global` declaration — `globals()['helper'] =
  other` or `setattr(<module>, 'helper', other)` — left `helper` looking like its `def`
  body, so callers of `helper()` across files that rebind it differently false-merged. Such
  rebinds are now recognized structurally and the string-literal key resolved (by content
  hash) to the module function it names, which then joins the `ModuleRebind` exclusion (no
  inlining / content-keying / exact channel) the lexical `global f; f = …` form already got.
  Conservative — it can only split fingerprints, never merge; corpus family delta = 0 (the
  pattern is absent from the pinned corpus).
- **Dataflow copy-propagation no longer makes two real-semantics-unsound moves (coevo
  series 9 oracle residue).** The single-use temp inliner (1) moved a temp's read past an
  indexed store that clobbers it — `t = a[i]; a[i] = a[j]; a[j] = t` became `a[i] = a[j];
  a[j] = a[i]`, turning a swap into "set both to `a[j]`" — because the hazard check's
  `collect_writes` ignored that `a[i] = …` mutates `a`; and (2) inlined a possibly-raising
  read into a comprehension filter lambda, eliding its `Err` when the iterable is empty.
  `collect_writes` now records the root var of an `Index`/`Field` store target, and the
  inliner skips uses in a conditional/repeated position (lambda body, `If` branch, `Loop`).
  This closes every remaining `nose verify` canon-preservation violation across the corpus
  (netty/sympy/guava → 0); the value-graph fingerprint is essentially unchanged (family
  delta ≈ 0). The deeper limit that array-element mutation is not modeled (so `swap` ≡
  `clobber` still share an exact fingerprint) is an oracle-blind value-model gap recorded as
  OPEN in `docs/oracle-value-model.md` §7.3 and `bench/coevo/false_merges/`.
- **`nose verify` interpreter propagates `Err` from either operand (coevo series 9 oracle
  follow-up).** A comparison whose erroring operand sat on the *right* (`0 == b[s]` after
  the sound `==`/`!=` operand-ordering canon) read as a concrete `Bool` instead of raising,
  because `eval_bin_op` short-circuited `Err` only on the left and `bin`'s fallthrough took
  `0 == Err` as `Bool(false)`. That made the canon look like a behavior change on
  type-incoherent battery rows — a spurious canon-preservation false positive. `bin` now
  propagates `Err` symmetrically (the twin of `un`'s `Not`-of-`Err`); the left short-circuit
  stays for laziness. Verify-only (`interp.rs`), so scan output is byte-identical. Closes the
  equality-over-`Err` false-positive class: **sympy 20 → 2 canon-preservation violations,
  netty 3 → 2**, no false merges. The narrower effect-trace and comprehension-context residue
  is scoped in [oracle-value-model §7](docs/oracle-value-model.md).
- **Two cross-/intra-language false merges closed (adversarial co-evolution series 9).**
  - **JS bitwise *shifts* are now int32.** #283-D narrowed JS `& | ^` (and `~`) to int32 so
    they fingerprint distinctly from arbitrary-precision Python/Ruby bitwise, but `<<`/`>>`
    were left un-narrowed — so JS `a << b` (which shifts `ToInt32(a)`) false-merged with
    Python's arbitrary-precision `a << b`, though e.g. `1 << 31` is `-2147483648` in JS and
    `2147483648` in Python. JS shifts now narrow their shifted operand at the build site.
  - **Ruby `*` is held ordered.** `*` is string/array *repetition* in Ruby and asymmetric —
    `"ab" * 3` → `"ababab"` but `3 * "ab"` raises — yet the algebra pass folded a constant
    to the chain's end and the value graph sorted operands by hash, false-merging the two.
    `*` is now commuted only when sound: Ruby gates on no operand being a possible
    string/sequence; Python repetition (commutative, `3 * "ab"` == `"ab" * 3`) and
    JS/TS/Java/Go/C numeric `*` are unaffected. Corpus output is byte-identical (the merges
    were latent); the soundness oracle stays clean. See [experiments §CK](docs/experiments.md).
- **Graded witness now sees definition-site decorators** (#315 follow-up). A decorator's
  arguments are dropped at lowering, so `@click.argument("x")` and
  `@click.argument("x", metavar="m")` produce the same value graph — the witness used to
  grade such a pair `equal_modulo_holes` while their configuration differed (the gap the
  PR #319 qualitative review found). The witness now compares the two copies'
  decorator/attribute **source lines**: a difference becomes a `decorator` hole, fires the
  `decorator-differs` pattern, and demotes the claim. Language-aware — a leading `@` is a
  decorator in Python/Java/JS/TS and an *instance variable* in Ruby (ignored); Rust uses
  `#[…]`. The witness's soundness is now recorded as the `empirical-only` Lean obligation
  `detect.graded_witness`. Anti-unification re-ranking was also measured on the gold set
  and left unadopted (within-noise: dev +2pp / held-out −1pp P@10); the default ranking is
  unchanged. See [graded-witness](docs/graded-witness.md).

### Added
- **Graded equivalence witness for near families** (#315). Same-language `near`
  (`structural-similarity`) families now carry a `witness.graded` object that
  anti-unifies their two representative copies' value graphs into "equal except *k*
  holes" — each hole classified (`literal`/`input`/`field`/`call`/… are clean
  parameters; `shape`/`arity`/`unmodeled`/`extra-sink` are structural divergence) with
  its differing source text per side, recognized divergence `patterns`
  (`effects-reordered`, `sink-superset-*`, `fragment-containment`, `low-substance`),
  and a soundness-relevant referent check: a name both copies consume that resolves to
  *different* definitions fires `referent_mismatches` and demotes the claim
  (fail-closed), while an unresolved name becomes a scoped `caveat_names` entry. The
  `equal_modulo_holes` grade is the near-channel counterpart to the exact channel's
  proof — evidence, not a Lean-backed theorem, scoped to the modeled unit *body*
  (definition-site decorators/annotations and signatures are out of scope; tracked
  follow-up). Validated full-corpus (104 repos: 86% of near pairs at *k* ≤ 3, exact
  control 100% *k* = 0) and by independent qualitative review. See
  [graded-witness](docs/graded-witness.md) and [scan JSON](docs/scan-json.md).

### Changed
- **The reinvented-helper channel is promoted to the default surface.** A hand-labeled
  [field audit](docs/reinvented-helper-audit-2026-06-13.md) of all 17 corpus findings
  measured 94% genuine value-duplications and 71% directly actionable (non-test); the
  bare `nose scan` report now LISTS the non-test findings instead of a one-line count.
  Test-container findings (`container_in_test` — where "calling the helper" would make a
  test circular) are a decidable judgment-deep class (§2b), excluded from the default but
  kept under `--show reinvented` and in the additive JSON.

### Fixed
- **Three value-model false merges from the packed `Const` key** (#313). The value-graph
  `Const(u32)` packed a literal's kind tag and its value/hash into one u32, with too few
  bits — so an int could wrap its kind nibble into the boolean range (`x + 536870914` ≡
  `x + True`), truncate to 32 bits (`0` ≡ `2^32`), or a string collide in #308's 28-bit
  mask (`"geU"` ≡ `"aaha"`). `ValOp::Const` now carries the kind explicitly
  (`ConstKind`) plus the FULL i64 value / 64-bit hash, so nothing wraps or truncates; the
  #308 string mask is removed. Found by adversarial co-evolution series 8
  ([experiments §CI](docs/experiments.md)); one of the merges was introduced by #308.

### Fixed
- **String-literal `+` no longer commutes** (#308). A string literal's value-graph `Const`
  key carried its content hash via `0x2000_0000.wrapping_add(h)`, which for a high-bit hash
  wrapped OUT of the `String` class range — so `const_value_domain` misread the kind and
  `proven_non_concat` wrongly admitted the operands to `+` commutativity, false-merging
  `"p" + "q"` with `"q" + "p"`. String keys now mask the hash into range
  (`string_const_key`), shared between the frontend's `LitStr` and the synthesized empty
  string so cross-language map-default lookups still converge. Surfaced as a residual by
  adversarial co-evolution series 7 ([experiments §CH](docs/experiments.md)); `nose verify`
  flagged it as a hard violation.

### Fixed
- **Three keyword/argument false merges, found by attacking the just-shipped #304/#305
  binding code** (adversarial co-evolution series 7, [experiments §CH](docs/experiments.md)):
  - **Spread arguments** (`f(*args)`, `f(**d)`) were stripped at lowering, so `stats(*xs)`
    false-merged with `stats(xs)`. A new `Splat` IL node keeps a spread distinct; the
    inline/oracle fail closed on its dynamic arity.
  - **`global`-rebind via tuple-unpack / aug-assign / walrus** (`global helper; helper,_=...`
    / `helper += 1` / `(helper := ...)`) escaped the #302 single-identifier check, so the
    rebound function's callers still false-merged. A post-lowering pass now records the
    rebind for every assignment form; recall stays precise (a local `helper = 5` is not
    gated).
  - **Reordered effectful keyword arguments** (`f(a=g(), b=h())` vs `f(b=h(), a=g())`) were
    merged by the #304 name-sort, though Python evaluates arguments in source order. The
    sort is now gated on `reorder_safe` — pure reorders still converge, effectful ones stay
    distinct.

### Fixed
- **A `global`-reassigned function no longer false-merges its callers** (#302). A
  module function rebound from inside another scope (`def setup(): global helper;
  helper = ...`) does not bind its `def` body at call time, but its callers were given
  `DirectFunction` evidence and inlined that body — so two files reassigning `helper`
  to different things false-merged their callers (`helper(a)*10` ≡ `helper(a)*10`
  though `helper` differs). The frontend now records a `ModuleRebind` source fact on a
  `global`-declared assignment (the `global`/`nonlocal` keyword is otherwise dropped at
  lowering), and call-target evidence + content-keyed seeding withhold the name. Precise
  where the [series-6](https://github.com/corca-ai/nose/pull/303) reassigned-anywhere
  predicate over-fired: a local `helper = 5` (no `global`) carries no fact and stays a
  valid target — measured recall-neutral across all 36 Python-bearing corpus repos.

### Fixed
- **Keyword arguments now bind by name, not position** (#301). A Python call
  `helper(b=p, a=q)` lowered to byte-identical IL as `helper(a=p, b=q)` — the keyword
  names were dropped — so two callers passing different `(name → value)` mappings
  false-merged as an "exact behavior match" (e.g. `(p-q)*3+p` vs `(q-p)*3+q`). Keyword
  arguments now lower to a `KwArg` node carrying the name; the value graph keys a call's
  keyword args by name (so `f(a=p, b=q)` ≡ `f(b=q, a=p)` but ≠ `f(a=q, b=p)`), and both
  interprocedural inlining and the behavioral oracle bind keywords by the same plan. A
  recall *gain* as well as a soundness fix: same-mapping reordered keyword calls now
  converge. Ruby was already sound (it keeps keyword keys as distinct literals).

### Added
- **Reinvented-helper containment findings** (experimental). A new exact-grade finding
  class: a function that reimplements an existing pure single-return helper inline
  instead of calling it, matched by the helper's whole-body value-graph hash appearing
  as an interior sub-DAG of the container. Callers of the helper (or of a
  behaviorally-equal twin) are excluded — calling is the fix, not the smell — and
  idiom-sized helpers are floored out (≥ 20 source tokens, ≥ 8 value nodes). Surfaces
  as a one-line count in the human report (`--show reinvented` lists findings) and an
  additive `reinvented_helpers` array in scan JSON. Measured on the 105-repo corpus:
  16 findings, 16/16 value-exact on hand-labeling, including a real upstream bug
  (h2database's `getGarbageCollectionCount()` still calls `getCollectionTime()`).

### Changed
- **Interprocedural pure inlining generalized** beyond straight-line helper bodies.
  Loop accumulators, builder loops, guard clauses, exhaustive if/else tails, and
  nested proven calls now inline into caller fingerprints behind an evaluation-time
  sink fence (any effect/throw/break/field-write or in-loop return fails closed to the
  opaque content-keyed call), with a recursion cycle guard and a body-size ceiling.
  Callers of a loop helper now converge with the hand-inlined form, and callers of
  body-identical helpers converge regardless of helper name — in the `near` channel;
  exact-channel admission of such calls is deliberately deferred until its precision
  is measured. Soundness re-verified: `nose verify` clean on the corpus stress repos,
  byte-identical output across thread counts, sympy scan cost +2.4%.

## [0.7.0] - 2026-06-12

### Fixed
- **Three-way `/` division no longer false-merges across languages** (#283-D). `/` is
  *true-float* in Python 3 / JS (`7/2 == 3.5`), *floored-int* in Ruby (`7/2 == 3`, like
  Python `//`), and *truncated-int* in C/Go/Java/Rust (`-7/2 == -3`). A single `Op::Div`
  for all merged Python `a/b` with C `a/b` (and Ruby `a/b`) though they differ. A distinct
  `Op::TrueDiv` (Python/JS) vs `Op::FloorDiv` (Ruby, like Python `//`) vs `Op::Div`
  (C-family) fingerprints them apart: Python `/` ↮ C `/` ↮ Ruby `/`, while Python `/` ≡
  JS `/` and Ruby `/` ≡ Python `//` still converge. The i64 interpreter models `TrueDiv`
  like truncated `Div` — blind to the float result but consistent within the op (which only
  compares with itself), so no false merge; an honest float result needs the `Float` value
  kind (deferred). Zero corpus recall change (4294 → 4294 families); `nose verify` SOUND.
- **JS int32 bitwise no longer false-merges with arbitrary-precision bitwise** (#283-D).
  JS-family languages coerce every bitwise operand to int32 (`a & b` is
  `ToInt32(a) & ToInt32(b)`, the result an int32); Python/Ruby bitwise is
  arbitrary-precision. They differ outside int32 range (`2^40 & 2^40` is `0` in JS,
  `2^40` in Python), so one `Bin(BitAnd)` for both was a confirmed active false merge
  the i64 oracle was blind to. The value graph now wraps the **leaf** operands of a JS
  `& | ^ ~` expression in a `ToInt32` narrowing node: the fingerprint differs from
  arbitrary-precision bitwise (merge closed), while wrapping only leaves keeps the op
  structure intact so the De Morgan (#284) and idempotence (#283-B) canons still fire
  over int32. Within JS, `a&b` still commutes with `b&a`. Zero corpus recall change
  (4294 → 4294 families); `nose verify` SOUND. Shifts (`<< >> >>>`) and the IL-interpreter
  int32 modeling (restoring the oracle's safety net) are deferred follow-ups; see
  `docs/oracle-value-model.md` §3.2.
- **Untyped `a + b` no longer false-merges with `b + a`** (#283-C). `+` *commutes* only
  for numbers — for strings/lists it is *ordered* concat (`"x"+"y" ≠ "y"+"x"`). The
  detector reordered `+` operands whenever neither was *proven* a string/list, which for
  an untyped param is optimistic. The fix, in three precise pieces:
  - The verify oracle already models strings as an order-sensitive free monoid, but its
    input battery never fed two distinct strings to two params at once, so it read SOUND
    while the merge was live — the battery now has order-sensitivity rows (#294) and the
    oracle witnesses it.
  - The `+`-COMMUTATIVITY gate (`add_values_not_concat`) now requires **at least one**
    operand to be `proven_non_concat` (genuine evidence it is never a string/list — never
    the optimistic inference, the #283-B channel). That is the exact sound condition: if
    one operand is numeric then any other either commutes or hits `num + str → Err` in
    every order. So `x + 4`, `x*x + y*y`, and any sum touching a number still commute;
    only `a + b` with two concat-possible operands stays ordered.
  - `+`-ASSOCIATIVITY (`(a+b)+c ≡ a+(b+c)`) is sound for ALL types (concat is associative),
    so the AC canon still *flattens* untyped `+` — only the operand *sort* (commutativity)
    is gated. And a `Reduce(Add)` (a `sum`) forces its elements numeric, so the per-element
    `+` of `sum(x+y for …)` is commuted in that context, keeping generator ≡ loop ≡ `reduce`
    cross-form convergence.

  Zero corpus recall change (4294 → 4294 families on `bench/repos`); `nose verify` SOUND.
  The audit surfaced ~15 latent false merges encoded in the test suite's own fixtures
  (untyped `+` commutation as a "clone" vehicle), now corrected. The `(a+b)+c ≡ a+(b+c)`
  *float* non-associativity half of C stays open — it needs the `Float` value kind (D-div);
  see `docs/oracle-value-model.md`.
- **`-(-a)` and `a & a` / `a | a` no longer false-merge with a bare `a` on untyped
  params** (#283-B). These identities hold only for numbers — on a list/string the
  inner op Errs, so `-(-a)` Errs while `a` does not. Two layers conspired to merge
  them anyway: the value-independent `algebra` pass cancelled `-(-x) → x`
  unconditionally (it has no operand type), and the value graph's `Num` gate trusted
  an OPTIMISTIC param domain that infers `a: Num` *from the very `-`/`&` being
  rewritten* — circular, since the canonical `a` then carries no numeric constraint.
  Fix: the algebra pass now preserves `-(-x)` (mirroring the `!!x → x` it already
  defers), and the cancellations gate on `proven_numeric` — genuine evidence only
  (numeric literals, annotated / pack-typed params), never the self-referential
  inference. An untyped `-(-a)` stays distinct from `a`; an annotated `a: int` still
  cancels. Zero corpus recall change (4294 → 4294 families on `bench/repos`); the
  pinned corpus held no untyped occurrences, so the merges it removed were all latent
  false merges (the §AS scenario). Remaining #283 sub-findings: C (untyped `+`
  commutativity, oracle-blind) and the `/`-division / int32 parts of D.
- **Floored vs truncated `%` no longer false-merges across languages** (#283-D).
  Python/Ruby `%` is floored (remainder takes the divisor's sign); JS/Go/Java/
  Rust/C `%` is truncated (dividend's sign) — they differ on sign-disagreeing
  operands (`-1 % 3 == 2` floored vs `== -1` truncated). A single `Op::Mod` for
  all languages merged them (a false merge the verify interpreter, also blind,
  could not catch). A distinct `Op::FloorMod` is now lowered for Python/Ruby and
  evaluated with floor semantics: Python `%` no longer merges with JS `%`, while
  Python ≡ Ruby and JS ≡ Go still converge (recall preserved). The remaining
  #283-D cases — three-way division semantics (`/` is true-float in Python/JS,
  floored-int in Ruby, truncated-int in C/Go/Java/Rust, with no float in the
  interpreter model) and JS bitwise int32 narrowing — are root-caused in the
  issue and remain open.

### Added
- **Three sound recall rules from coevo §CE / #284** — behaviorally-equal forms
  the exact channel now converges, each sound for ALL inputs (no type gate)
  because the error behavior is preserved on both sides (unlike the §BA identity
  rewrites): `abs(abs x) ≡ abs x` (idempotence), `~(a&b) ≡ ~a|~b` / `~(a|b) ≡
  ~a&~b` (bitwise De Morgan, the bitwise twin of the algebra pass's logical
  one), and `max(max(a,b),c) ≡ max(a,max(b,c))` (MIN/MAX flattening — they were
  per-level commutative, now associative-flatten-eligible too; associative on
  the ternary semantics, total even for NaN). Corpus verify unchanged (zero new
  false merges); hard negatives lock min≠max and `~(a&b)`≠`~(a|b)`.

### Fixed
- **Embedded `<script>` extraction (Vue/Svelte/HTML) is now context-aware** (#280,
  coevo §CE). The byte-scanner was naive `find_ci`, so five real shapes broke it:
  a `</script>` inside a JS string truncated the block (missed dup); a
  commented-out `<script>` was analyzed as live and the span swallowed the
  surrounding markup; a Vue 3.3 `generic="T extends Record<string, number>"`
  attribute `>` was taken as the tag end (span started mid-tag); an unclosed
  `<script>` (valid HTML) was dropped (missed dup); and trailing markup left as
  blank lines made the whole-block span bleed past `</script>`. The scanner now
  skips HTML comments, finds the tag-end `>` outside quoted attributes, finds
  `</script>` outside JS strings/comments, extracts an unclosed block to EOF, and
  truncates the analyzed buffer at the last script byte so spans stop at the
  content. Plain and multi-block extraction, `lang="ts"` detection, and
  same-as-plain-JS convergence are unchanged.
- **`--cache-dir` now reproduces cross-file imported-literal convergence** (#275).
  The cache keyed each file's units on its source bytes and lowered files
  independently, skipping the corpus-level `resolve_imported_immutable_bindings`
  pass — so a cached scan under-merged an `imported LOOKUP.get(k)` with an inline
  `{…}.get(k)` that the non-cached scan converges. The cached path now lowers and
  resolves the whole corpus every run (the smaller half of the work, §BQ) and
  caches only the dominant normalize+extract step, keyed on an interner-independent
  value-retaining hash of each file's **post-resolve** IL. Cached output is now
  byte-identical to non-cached (incl. imported-literal families); editing a
  provider's literal invalidates dependents' entries; warm-cache speedup is
  retained (~2.2× on sympy). Cache schema bumped v7 → v8.
- **Soundness: effectful operands of a commutative operator no longer false-merge**
  (#283 sub-finding A, experiments §CE). The value-graph canonicalizer sorted a
  commutative/AC operator's operands by structural hash, so `print(a) + print(b)`
  converged with `print(b) + print(a)` though their observable effect order
  differs (a false merge `nose verify` confirms). It now sorts only when every
  operand is effect-free (no call/HOF/lambda/loop/opaque node in the subtree);
  effect-free numeric operands still converge (`a+b+1 ≡ 1+b+a`). The remaining
  #283 sub-findings (optimistic-Number rewrites, untyped `+` commutativity, the
  language-blind verify interpreter) are root-caused in the issue and remain open.

### Security
- **Adversarial co-evolution series 5 found latent false merges in the soundness
  core (P0 #283, experiments §CE)** — the cardinal sin (design §1). The
  value-graph node-multiset fingerprint is blind to a commutative operator's
  operand order, so `print(a)+print(b)` and `print(b)+print(a)` get one exact
  fingerprint though their observable effect order differs (`nose verify`
  confirms); optimistic `Number` inference makes `-(-a)≡a` / `a&a≡a` merge
  untyped params; untyped `a+b≡b+a` is wrong for string/list concat; and the
  verify interpreter is language-blind (Python `%` ≡ JS `%`), so the oracle
  itself masks cross-language merges. All LATENT — `nose verify bench/repos`
  stays green (the corpus lacks these shapes, the §AS scenario). Reproducers
  checked in at `bench/coevo/false_merges/`. Fixes are moat work (value-graph
  effect sinks, genuine-Num proofs, interpreter language-awareness) tracked in
  #283; no behavior change shipped this entry.

### Fixed
- **Adversarial co-evolution, series 4** (experiments §CD, issue #279) — the AST
  declaration classifier's first attack plus the difference-evidence and
  encoding surfaces:
  - **Declaration classifier: call-shaped entries inspect their children**
    (S4-C1). `const { a = steal() } = require('lit')`, a computed key, and a
    Ruby `require('x') { … }` block smuggled live calls onto an import line
    because the node-kind match returned before recursing; the JS binding name
    and Ruby block must now execute nothing. Plain destructuring stays wiring.
  - **`N of M lines shared` is honest** (S4-C2): the displayed count is now the
    representative pair's physical invariant-line count, so the summary can no
    longer read `5 of 6 shared, 2 spots differ` (5+2>6) or undercount repeated
    identical lines; the majority-voted set still drives ranking weight.
  - **UTF-8 BOM no longer flips classification** (S4-C3): a BOM'd import-only
    file stays a declaration run (stripped in the classifier and prescreen),
    matching the IL-lowering path that already tolerated it.
  - Embedded `<script>` text-extraction defects deferred (#280, grammar-driven
    boundary detection is frontend core work); coverage rows adopted for Rust
    `extern crate`, Go `package`, Ruby `require_relative`.

### Changed
- **The AST declaration classifier is cost-neutral** (§CC addendum): the
  migration's serial per-file re-parse cost +29% wall on family-dense scans
  (measured A/B vs the prior binary); a sound-direction prescreen plus
  parallel candidate-file parsing brings sympy to 4.67 s and a 1,364-file TS
  app to 0.55 s — at or under the pre-migration baseline, with byte-identical
  classification.
- **Declaration classification now rests on AST facts** (experiments §CC):
  `nose_frontend::declaration_facts` derives per-line declaration/comment/code
  facts from the tree-sitter AST (ERROR subtrees never count as declarations;
  stray code on a declaration line poisons it), replacing the four-times-
  hardened text line grammar (−351 lines net). The 47-row adversarial battery
  transferred unchanged. Corpus effect: +14 families correctly classify
  (recovered fail-open leaks: multi-line imports with trailing comments,
  star-imports, mid-file `use` blocks); zero worthy-label cost.
- Deferred-queue dispositions: #269 closed (synthetic-only, revisit condition
  recorded), #270 closed (Phase-3 entry gate added to the semantic-kernel
  roadmap: pack expansion requires a priced consumer case), #275 escalated to
  a reproduced `--cache-dir` equivalence violation (imported-literal
  convergence disappears under the cache; reproducer on the issue).

### Fixed
- **Adversarial co-evolution, series 3** (experiments §CB, issue #274; method
  upgrades: executable self-verified packets, the bench/coevo packet ledger,
  claim-direction slot rules):
  - **Structured-ignore selectors are now ALL-members** (S3-C4): a
    `paths: ["vendor/**"]` entry no longer suppresses a family whose other
    copy lives outside the glob — first-party duplication could previously
    pass `--fail-on any` silently. An entry describes families wholly inside
    its selectors; partial coverage keeps the family reported. **Migration:**
    widen entry globs to cover every member (or use `family_id`).
  - **Declaration wiring lines validate their payloads** (S3-C1): from-clause
    sources must be lone string literals, Python simple-form name lists and
    Java/package dotted paths must be name-shaped —
    `import { x } from Math.max("a", "b");` no longer classifies.
  - Honest fences documented: baseline keys are span/path/mode-sensitive (pin
    `--mode`, re-baseline after refactors — every drift direction is loud);
    `is_test_loc` markers are ecosystem conventions, not a "never" guarantee.
  - Cache/cold scan code-path asymmetry deferred with reproduction notes
    (#275); coverage rows and caution-boundary tests adopted from the
    informed auditor.
- **Adversarial co-evolution, series 2** (experiments §CA, issue #272): first run
  with fresh-subagent attackers (blind/informed modes, persona rotation — now in
  the runbook). The blind grammar-lawyer found what two authored passes missed:
  open multi-line declarations consumed interior lines unvalidated and closers
  were suffix checks (`os.Exit(1))` could "close" a Go import block;
  `require 'fs' + 1` rode arithmetic on a require; `#include <stdio.h> int x = 1;`
  rode a definition on a directive) — interiors now validate as per-language
  specifiers, closers match strict shapes, and C/Ruby arguments must be lone
  string literals. The "call the existing helper" hint no longer bypasses the
  high-parameter caution. Seven untested-but-supported declaration shapes locked
  as fixtures (incl. ASI imports, `pub(crate) use`); the scan-JSON contract
  checker now requires the `generated`/`declaration` surface-count keys and the
  checked-in v1 example was refreshed. The review `--fail` gate and the scan-JSON
  contract survived their blind attackers with zero violations. Corpus price
  after all tightening: unchanged (2,265 declaration families, zero
  reclassification).
- **Adversarial co-evolution, series 1** (experiments §BZ, issue #268): five
  white-box campaigns against nose's own claims. Claim-violation fixes — the
  declaration filter now enforces a *single-statement discipline* (a line mixing
  a declaration with code, e.g. `import pdb;pdb.post_mortem()`, a multi-declarator
  `require`, or a Ruby modifier-conditional `require`, can no longer classify;
  shapes found verbatim in the corpus); the "call the existing helper" hint never
  points production copies at a test-code or generated-file helper; the
  declaration classifier reads each file once per scan instead of once per family
  member. Corpus price after all tightening: identical (2,265 declaration
  families, zero worthy-label loss). Two priced findings deferred with
  measurements: few-huge-files inputs serialize `normalize+extract` (#269), and
  semantic-laws provenance is structurally gated to ~zero field probability
  (#270 — the clamp-law escalation was refuted five-for-five by sound gates,
  explaining the LawPack audit's zero-provenance result). The runbook gained the
  performance/determinism attack surface, the claim-violation pricing asymmetry,
  defense-deferral verdicts, and measured campaign costs.

### Added
- **Triage ergonomics from the 0.6.0 field feedback** (issues #263/#264):
  - **Opportunity grouping**: families whose members are overlapping slices of
    the same source regions fold under their best-ranked family in the human
    report (`↳ N overlapping slice families fold into this entry`); scan JSON
    keeps every family and marks slices with `overlap_primary_id` (additive).
    Grouping is presentation policy — baselines, ignores, `--fail-on`, and the
    JSON family list are unchanged.
  - **`--scope prod|test|all`** on `nose scan`: keep one side of the test
    boundary (`prod` drops all-test families but keeps test↔prod leaks).
    An explicit consumer choice; the default stays `all`.
  - **"Call the existing helper" hint**: when exactly one family member is a
    whole named function and the rest are inline blocks/fragments, the hint
    becomes `N sites reimplement `name` — call the existing helper (file)` —
    the family's own equivalence already proves the replacement is safe.
  - **High-parameter caution**: extraction hints at ≥6 varying spots add
    "review readability; a smaller helper for the invariant core may fit
    better" instead of overclaiming a clean extract.
  - **Evidence tags in the human report**: each family line now names its
    equivalence witness (`exact behavior match`, `shared core computation`,
    `copy-paste`, `near-duplicate`) — the JSON has carried this since #222.

### Changed
- **Declaration runs leave the default surface** (experiments §BY, design §2b):
  a family whose every member span is only import/include/`use`/re-export
  declarations is real duplication the language mandates per file — there is
  nothing to extract, so it no longer occupies the default report,
  `--fail-on`, markdown, or SARIF surfaces. It stays in `--format json
  --top 0` as `recommended_surface: "declaration"` (a classification, not a
  deletion), is counted in the human report's omitted line
  (`N declaration-run`), and `ranking.surface_counts` gains a `declaration`
  field. Priced on the 105-repo corpus: 2,265 families across 43 repos leave
  the default surface with zero worthy-labeled families reclassified. The
  filter is fail-open: any span not provably all-declarations stays on its
  ranked surface.
- **design.md gains §2b (the decidability boundary) and §2c (the bare default
  is the product)**: mechanically-decidable non-actionability is the
  detector's job (the dual of "judgment-deep worthiness belongs to the
  calling agent"), and the no-flags `nose scan` report is the first-user
  surface that evidence-based filters defend.

## [0.6.0] - 2026-06-11

### Changed
- **CLI usability pass for first-time users.** `nose scan`, `nose review`, and
  `nose stats` now **error on a named path that doesn't exist** (a typo'd path
  in a CI gate must fail loudly) instead of warning and exiting 0; a path that
  exists but contains no supported files still warns and reports an empty scan.
  Top-level help and command descriptions were rewritten to match the current
  default channel mix (`syntax,semantic,near`, stale since the #241 flip) and
  mention `nose review`; the command list is ordered by user journey (`scan`,
  `review` first) with one-line summaries (details live in each command's
  `--help`). The human scan report prints a friendly line instead of
  `0 clone families … (showing 0)` when nothing is found, and ends with a
  one-line hint pointing at `--show diff`, `--show proposal`, and `--top 0`
  when extra views weren't already requested. README and getting-started were
  refreshed to match (quick-start sample output, `nose review` introduction).

- **`nose review --fail` now fires on the conservative gate tier by default**
  (#245, experiments §BV): only findings where the diff provably touches lines
  the changed copy shares with its un-updated sibling (by the family's own
  proof for exact-value-graph families; by varying-spot subtraction for
  token/fuzzy families), excluding all-test scaffolding. Measured on the §BR
  judge-labeled replay: every genuine missed propagation kept, 73% fewer fires,
  3.7× precision. **Migration:** `--fail-on any` restores the old
  fire-on-anything behavior. Review JSON findings gain `fire_eligible`,
  `witness_kind`, `scope`, and per-changed-site `touches_shared`; human output
  marks gate-firing findings with `[gate: touches shared lines]`.

- **`nose scan`'s default channel mix is now `syntax,semantic,near`** (#241,
  experiments §BM): omitting `--mode` also runs the fuzzy Type-3 `near` channel
  at its standard `0.70` acceptance floor. Measured on the 105-repo corpus,
  the flip lifts held-out worthy-recall 88.5% → 96.7% with held-out P@10
  *improving* 55.5% → 58.6%; the cost is a larger default report
  (+22% default-surface families corpus-wide, mostly production scope).
  **Migration:** an explicit `--mode` (or config `mode`) is unaffected — it
  replaces the default exactly as before. CI gates and baseline users should
  pin `--mode` (e.g. `--mode syntax,semantic` for the old mix) or re-baseline
  with `--write-baseline`, since a default-mode scan now reports more families
  and `--fail-on any` can newly fail. `nose review`'s default is **unchanged**
  (`syntax,semantic`): review feeds a gate, where false fires cost more than
  missed candidates, and the §BM pricing covered the scan surface only.
  `nose capabilities` advertises the new `scan.default_modes` truthfully.

### Added
- `NOSE_ANCHOR_MIN_WEIGHT` research knob: overrides the sub-DAG anchor weight
  floor (default 20). The #248 sweep (experiments §BW) measured floor 8 at
  +0.9pp held-out worthy-recall with flat P@10 and consolidated corpus
  families, but 3× more small families on near-only gate surfaces — so the
  default stays conservative and recall-first consumers can opt in.
- `nose verify`'s oracle now explores BOTH arms of a symbolic If/ternary
  condition under recorded assumptions instead of bailing the unit (#244,
  experiments §BU): conditioned, never guessed — each path's trace carries a
  symbolic assume marker, so explored-path disagreements can only reach the
  advisory lane, never the hard SOUND gate. Bounded fail-closed at 3 symbolic
  decision sites per execution (a new census-visible `path-bail` reason; 2,101
  units corpus-wide). Interpretable units 29.2% → 31.3% on the 105-repo corpus
  with verify SOUND everywhere; loop conditions and the strict `run_unit`
  contract (canon validation, fragment oracle) are unchanged.
- Ruby `for x in xs … out << e` builder loops now converge with comprehensions
  and builder loops across languages in the exact semantic channel (#247,
  experiments §BT). The shovel is admitted as an append only through the
  active-builder seed proof (`out = []`); an integer-seeded `<<` stays a shift,
  a parameter receiver never builds, and `each`-block builders remain pack-gated
  (no Enumerable inference from a method name).

### Performance
- Scans are 2–4× faster end-to-end on the benchmark corpus (sympy 20.0 → 4.7s,
  redis 3.9 → 1.0s, git 2.7 → 1.1s wall; experiments §BQ) with byte-identical
  output. The evidence passes and semantic-evidence queries that dominated
  `normalize+extract` were quadratic — per-node helpers re-scanning the whole
  `il.evidence`/`il.nodes` tables — and now go through index-backed lookups:
  the anchor-span evidence index everywhere (emit-path dedup included, plus a
  new binding-hash bucket and a staleness sentinel that survives
  `clear()`/`retain()`), new lazy span→nodes and scope→assignments arena
  indexes, a one-pass `ScopeMutationFacts` walk in binding evidence, and a
  per-file (was per-unit) pure-inline candidate registry shared through
  `ValueFingerprintContext`.

### Fixed
- Ruby `for x in xs` loops are no longer excluded from the exact channel:
  tree-sitter-ruby wraps the iterable in an `in` node, which lowered to an
  exact-unsafe `Raw("in")` and split every Ruby `for` loop from its
  cross-language equivalents (#247).
- `nose review --format json|sarif` now emits its machine contract (an empty
  findings envelope) instead of a human sentence when the diff has nothing
  reviewable — e.g. an adds-only change. Found by the #243 fire-precision
  replay: a JSON consumer parsing review stdout broke on exactly those PRs.
- Rust units inside an inline `#[cfg(test)] mod tests` now classify as test
  scope (the path+name heuristic tagged them `prod`, distorting triage — the
  #216 audit's alacritty family). Locations carry `in_test_module` in scan
  JSON; a copy-paste run crossing test functions counts as test scaffolding
  when every overlapping unit sits in the test module.
- **Soundness:** five fingerprint erasure classes no longer collapse working code
  onto stubs (#210, experiments §BP): Python `try/except/else` kept its `else`
  clause (black's try/import wrapper ≡ `return self`); Ruby `begin/rescue/else`
  moved `else` out of handler position; C/Go/Rust dereference STORES stay
  computed places (`(*nr)++` merged with bare `return 0` stubs — 38 pairs in git
  alone); Go type-switch arms survive lowering (a recursive traversal merged
  with a constant stub at exact-safe); try handlers are erased only for provably
  non-throwing bodies; element-free loop effects are keyed by the iteration
  source (for-in keys vs for-of values); and the oracle respects declared
  parameter type domains plus 2-arg scalar min/max arity. `nose verify` now
  reports zero hard violations on all 105 corpus repos.
- The verify oracle's hard SOUND gate is scoped to the product's exact claim
  (`exact_claim_eligible` in nose-detect: strict-exact-safe + the
  degenerate-fingerprint size floor); lossy-fingerprint collisions are a
  diagnostics lane and declaration-divergent or symbolic disagreements are
  advisory leads, so the gate measures exactly what the exact channel asserts.
- Combining `--mode syntax,semantic` no longer drops an exact semantic family when
  the syntax channel also creates a same-file window that collapses to one reported
  site. Such single-site windows are not clone families and no longer participate
  in rank-time overlap subsumption, so adding the syntax channel cannot erase an
  otherwise reported semantic region.
- Scan JSON `family_id` values are now unique for distinct reported families that
  share the same files and symbol names but point at different spans, especially
  hidden exact-fragment families. The ID now includes each member's displayed
  path, language, span, unit kind, symbol name, and fragment proof metadata; old
  baselines can still classify re-keyed overlapping families as `changed`, but
  structured ignores by `family_id` may need a one-time refresh.
- Fresh benchmark-corpus reconstruction works again: `bench/setup_repos.sh`
  now has its missing file-level prune helper checked in, guards that the helper
  exists before cloning, and writes a deterministic
  `bench/labels/prune_manifest.json` audit artifact with a post-prune corpus
  digest.
- Semantic scans no longer stack-overflow when a recursive helper is referenced
  from inside one of its own callback bodies while extracting a block or exact
  fragment. The value graph now excludes the enclosing function/method from the
  per-unit inline registry, preserving bounded inlining for sub-unit
  fingerprints. This fixes the `rxjs` corpus crash and keeps the 105-repo bench
  corpus green in semantic JSON mode.
- **Soundness:** a seeded selection loop (`best = 0; … if v > best: best = v`) no
  longer merges with the true builtin selection (`max(…)`) — the seed clamps the
  result (empty or all-negative input returns the seed), so it is
  behavior-defining. Selection reductions now carry their seed in the value
  fingerprint; equally-seeded loops still converge with each other. Found by
  `nose verify` the moment 2-argument `min`/`max` became interpretable; the
  mislabeled adversarial benchmark case was flipped to a hard negative.
- **Soundness:** Python `//` no longer shares a fingerprint with `/`. Floor division
  is its own IL operator (`FloorDiv`, quotient toward −∞) with matching interpreter
  semantics, so `5 / 2` (2.5) can never merge with `5 // 2` (2).
- **Soundness:** JS/TS `>>>` (zero-fill shift) no longer collapses onto `>>`
  (sign-extending shift); Python `@` (matmul) no longer collapses onto `*`. Both
  keep a raw shape keyed by their own operator spelling.
- **Soundness:** a strict null check (`x === null` / `x !== null`) no longer merges
  with the nullish family (`x ?? d`, `x == null ? d : x`) — strict and loose checks
  differ on every `undefined` input. `=== undefined` against a proven typed
  `Map.get` result still converges with `??` (there the strict check is the
  faithful absence test). `x ?? d` still converges with `x == null ? d : x`.
- **Soundness:** compound assignments with an operator the IL does not model no
  longer silently degrade: JS `x ??= y` now desugars to `x = x ?? y` (it lowered
  as `x += y`); Java `x >>>= y` lowered as `x = y`; unmapped operators across
  Python/JS/Go/Rust/Java/C/Ruby now keep a raw shape keyed by the operator
  spelling instead of defaulting to `Add` or plain assignment.
- Two different unmapped binary operators over the same operands no longer share a
  raw fingerprint — the raw fallback now keys by the operator spelling.
- The interpreter oracle evaluates 2-argument `min`/`max` (the 2-way selection
  `[a, b].min()` canonicalizes to) instead of erring — closing an oracle blind
  spot on exactly the convergences the value graph claims.

### Added
- Generated-code markers now reach scan JSON: the generated-header index the
  human report already used is computed for every output format, locations
  carry `looks_generated`, all-generated families report
  `recommended_surface: "generated"` (re2c output used to reach agents as
  `default` — the #216 audit's gap #3), and `surface_counts` gains a
  `generated` bucket. Partly-generated families stay on their ranked surface
  with generated members flagged.
- Block locations now carry `enclosing_unit` (the host function/method) in
  scan JSON — structural blocks via the fragment recovery path, copy-paste-run
  blocks via a new span lookup over the unit set, and same-span method/body
  pairs resolve to the method. Every sampled #216 audit block had `name: null`
  with nothing to anchor a discussion to.
- Scan JSON families now carry `varying_spots` difference evidence: per varying
  spot, both representative copies' absolute line ranges and trimmed text —
  consistent with `params` by construction. With the witness's shape-vs-value
  Jaccard split, a data-table family (the #216 audit's arrow case) is
  classifiable from JSON alone.
- Scan JSON families now carry an agent-facing equivalence `witness` naming WHY
  the members merged: `exact-value-graph` (with the shared multiset size),
  `shared-sub-dag`, `copy-paste-run`, or `structural-similarity` (with mean
  value vs shape Jaccard). The #216 audit's top gap — `shared_lines: 0` with
  `mean_score: 1.0` was uninterpretable on a real cross-language Type-4 family.
- A scan-JSON agent-usability audit artifact records whether an LLM agent can
  decide and act from the JSON alone: 14/18 sampled families were decidable, and
  the four failures fix the evidence roadmap — no equivalence witness on default
  families, no difference evidence, generated-content markers unsurfaced, and
  missing enclosing unit names (docs/scanjson-agent-audit-2026-06-10.md).
- Ruby `**` now lowers to the shared exponentiation operator and converges with
  Python/JS `**`.
- Compact CLI regression fixtures now pin the real-corpus strict-nullish hard
  negatives: `x ?? d` / `x == null ? d : x` stay separate from `x === null ? d :
  x`, and loose `!= null` object guards stay out of the strict non-null object
  guard family.
- Scan JSON `ranking` now includes `surface_counts`, a pre-`--top` breakdown of
  `default`, `review`, `hidden`, and `debug` families plus the same breakdown for
  exact-fragment families. This makes the human-action surface explicit for
  integrations that should filter `recommended_surface == "default"`.
- A three-reviewer fragment-quality audit artifact for Java/Python hidden/review
  exact-fragment families now records the criteria, votes, and policy decisions
  behind the diagnostic-surface follow-up.
- A LawPack provenance audit artifact now records the 105-repo and targeted
  real-corpus pass for `nose.value_graph.laws`: the pack is active in scan JSON,
  but the current two proof-backed laws produced no real clone families with
  `semantic_laws` provenance.
- The design §5 recall-ceiling probe (`bench/labels/recall_ceiling_probe.py` plus
  its dated artifact) now measures the residual sub-DAG / pure-inlining recall
  headroom on the v5 gold set: a 2.0% optimistic ceiling (0.6% at the shipped
  anchor weight), answering the recall-mechanism gate no-go and routing the
  residual to unit-extraction coverage and the fragment statement-window axis
  (experiments §BJ).
- An independent miss-mining arm (`bench/type4/miss_mining.py` plus its dated
  artifact) now measures in-the-wild unreported same-computation pairs beyond
  the nose ∪ jscpd label pool: 593 detector-suggested candidates corpus-wide,
  audited as overwhelmingly generated/scaffolding with a handful of
  worthy-shaped misses — and the audit exposed the #202 channel-merge family
  drop (experiments §BK).
- An oracle exclusion census (`nose verify --exclusion-census`, merged by
  `bench/labels/merge_exclusion_census.py`) now baselines real-corpus oracle
  coverage: 4.5% of function units are interpretable and 90.8% of
  fingerprint-equal pair mass carries no behavioral check, with opaque calls and
  field reads as the dominant, structure-keyed coverage targets. The companion
  `--leads` merge records 179 behavior-equal fingerprint-split groups (5 with
  vj ≥ 0.7) as convergence leads (experiments §BL).
- The interpreter oracle now models opaque calls and unproven field reads as
  identified SYMBOLIC values (recorded in the ordered effect trace) instead of
  bailing the whole unit: real-corpus oracle coverage rose from 4.5% to 29.4%
  of function units, and oracle-verified fingerprint-merge pairs from 9.2% to
  31.3%. Symbolic-trace disagreements go to a separate advisory lane — the hard
  SOUND gate and canon-preservation stay concrete-only, and the completeness /
  under-merge direction keeps its concrete meaning (experiments §BL.1). The
  corpus pass also exposed a pre-existing degenerate-fingerprint false-merge
  class, filed as #210.

### Changed
- Tiny test-only exact-fragment scaffolding now stays on the hidden diagnostic surface
  instead of the review surface: all-test fragments with enclosing context and mean span
  ≤3 lines, plus all-test effect/body fragments up to 4 lines. Larger test setup
  fragments remain available for review.

### Performance
- Minified-bundle-sized files no longer hit a quadratic cliff: `nearest_scope`
  and evidence-record lookups were per-query linear scans over all IL
  nodes/records and are now lazy per-file indexes. A 246 KB minified JS file
  went from 227 s to ≈ 2 s in normalize+extract (~118×), and ordinary repos get
  ~3× faster normalize+extract. Profiler-driven (`sample` + `NOSE_TIME=1`).
- Large Java test files with many imported API occurrences no longer spend
  minutes revalidating the same import-shadow proof. Imported occurrence
  validation now reuses a per-file function/local-shadow cache; the
  `commons-lang` semantic scan outlier went from ≈119 s to <1 s in
  normalize+extract, with identical family ids.

### Removed
- Pre-release compat shims: `seq_surface_contract_evidence_for_node`,
  `unshadowed_global_symbol` (spelling fallback), `builtin_demand` and the legacy
  `BuiltinDemand` enum (superseded by `builtin_demand_profile`), the superseded
  `lcs_ratio` scorer, the unused `minhash::estimate`, and the never-constructed
  `EvidenceEmitter::Legacy` variant.

## [0.5.0] - 2026-06-05

### Added
- Strict Type-4 proof facts for case-sensitive string prefix/suffix predicates across
  Go, Java, JavaScript/TypeScript, Python, Ruby, Rust, and embedded script surfaces.
  The benchmark now includes same-surface and cross-surface positives plus affix,
  direction, and wrong-receiver hard negatives.
- Strict Type-4 proof facts for static literal collection membership across Go,
  JavaScript/TypeScript, Python, Ruby, Rust, and embedded script surfaces. The detector
  now converges Python `in`, literal receiver `includes/include?/contains`, and Go
  `slices.Contains` while keeping substring contains and arbitrary receiver contains
  outside strict semantic reporting.
- Strict Type-4 proof facts for typed dynamic collection membership across Go, Java,
  Python, Rust, and TypeScript. Frontends now preserve coarse parameter semantic facts
  from explicit type annotations, and exact semantic mode uses them to converge
  collection membership APIs while keeping typed string receivers and wrong-element
  boundaries distinct.
- Strict Type-4 proof facts for proven Set membership. Exact semantic mode now
  converges typed TypeScript `Set<T>.has(value)`, inline `new Set([...]).has(value)`,
  and immutable local `Set` construction with the corresponding collection-membership
  predicates, while preserving wrong-element, wrong-collection, shadowed-constructor,
  untyped-receiver, and map-key-membership boundaries.
- Strict Type-4 proof facts for Java literal collection factories. Exact semantic mode
  now converges `List.of(...).contains(value)`, `Set.of(...).contains(value)`, and
  `Arrays.asList(...).contains(value)` with static literal collection membership, while
  preserving wrong-element, wrong-collection, local name shadowing, and same-file type
  shadowing boundaries.
- Strict Type-4 proof facts for literal Python/Ruby map lookup with fallback. The detector
  now converges dict `.get(key, default)` and hash `.fetch(key, default)` only when the
  receiver is a static literal map, preserving wrong-key, wrong-default, and wrong-map
  boundaries.
- Strict Type-4 proof facts for JavaScript/TypeScript `Map` construction default
  lookups. Exact semantic mode now converges inline `new Map([...]).get(key) ?? fallback`,
  immutable local `Map` construction, and proven `has/get` ternaries with literal
  Python/Ruby map defaults, while preserving wrong-key, wrong-default, wrong-map,
  untyped-receiver, and shadowed-`Map` boundaries.
- Strict Type-4 proof facts for JavaScript/TypeScript object-literal default
  lookups guarded by own-property checks. Exact semantic mode now converges
  `Object.hasOwn`, `Object.prototype.hasOwnProperty.call`, and negated own-property
  ternaries over static object literals with literal Python/Ruby map defaults,
  while preserving wrong-key, wrong-default, wrong-map, unguarded index default,
  prototype-aware `in`, direct `hasOwnProperty`, and shadowed-`Object` boundaries.
- Strict Type-4 proof facts for null/none/nil/option presence predicates. The detector
  now converges explicit null comparisons with Ruby `nil?` and Rust `is_none`/`is_some`
  method forms plus Rust `if let Some(_)`/`if let None` pattern predicates, while
  preserving non-null direction and wrong-value boundaries.
- Strict Type-4 proof facts for value-or-fallback defaulting across JavaScript/TypeScript
  nullish forms and Rust `Option` APIs. The detector now converges `??`, explicit
  nullish ternaries/guard returns, `unwrap_or`, capture-only `unwrap_or_else`, and
  identity `map_or`, while preserving truthy-or, wrong-fallback, and wrong-value
  boundaries.
- Strict Type-4 proof facts for scalar numeric idioms across C, Go, Java,
  JavaScript/TypeScript, Python, Ruby, and embedded script surfaces. The detector now converges
  explicit sign-normalizing conditionals with safe `abs`/`Math.abs`/`math.Abs` forms, and
  scalar two-way `min`/`max` conditionals with proven builtin and method forms including Ruby
  `value.abs`, Ruby two-element `[left, right].min/.max`, and Rust numeric
  `.abs()/.min()/.max()`, while preserving signed-identity, wrong-value, min/max
  direction, shadowed-`Math`, and Rust custom-method boundaries.
- Strict Type-4 proof facts for map key-membership predicates across Go, Java, Python,
  Ruby, Rust, and typed TypeScript `Map` receivers. The detector now converges
  `key in map`, map key APIs, Java `keySet().contains`, Rust `get(key).is_some`,
  TypeScript `Map.has`, and Go `_, ok := map[key]` while preserving wrong-key,
  wrong-map, and value-membership boundaries.
- Strict Type-4 proof facts for typed map lookup with fallback across Go, Java,
  Rust, and typed TypeScript `Map` receivers. The detector now converges Go
  lookup-ok fallback assignments, Java `containsKey/get` and `getOrDefault`,
  Rust `contains_key`/index and `get(key).unwrap_or(default)`, and TypeScript
  `Map.get(key) ?? fallback`, `has/get` ternaries, and temp-bound undefined guards
  while preserving wrong-key, wrong-default, wrong-map, and untyped-receiver
  boundaries.
- Strict semantic block extraction no longer treats expression ternaries as
  sub-function block units. This keeps exact semantic block candidates focused on
  real statement blocks and prevents proof-context-free expression fragments from
  bypassing function-level type facts.
- Type-4 focused generation filters (`--axis`, `--proposal-prefix`) and smoke gates
  (`GATE=focused|core|full`) so detector co-evolution loops can run on one frontier
  before periodic compact/full validation.
- Type-4 frontier preflight (`bench/type4/preflight_axis.py`) to reject benchmark-only
  loops when the baseline already covers all strict positives or when a candidate does
  not improve recall or remove baseline false merges without candidate false merges.

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
- New machine-checked Lean proofs: `NoseAlgebra.distrib_sound`,
  `NoseFunctor.{filter_fusion, filter_length_eq_count}`, and the
  `normalize.value_graph.compare` obligation (comparison-direction and
  negated-comparison canons). A CI `formal` job checks the proof-obligation registry
  and all `formal/**/*.lean` files on every push.

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
