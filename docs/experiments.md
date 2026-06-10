# nose — experiment log

A **curated** record of what we tried and what we learned — distilled to the durable
findings, the shipped techniques, and the dead ends worth not re-walking. The full
blow-by-blow (every intermediate baseline and rejected parameter sweep) lives in git
history; this page keeps the lessons. Sections are lettered chronologically (A…BF);
other docs and code comments cite them as `§<letter>`, so the letters are stable anchors.
The methodology and headline numbers are summarized in [benchmark](benchmark.md); the passes
these experiments shaped are in [normalization](normalization.md) and
[architecture](architecture.md).

The current user-facing `nose scan` command has three channels (`syntax`, `semantic`,
`near`), described in [usage](usage.md); all share one lower → normalize → feature
pipeline, with exact semantic matches coming from the value graph.

> **Historical record.** This log spans a pre-v5 era whose measurement code (many
> `bench/*.py` scripts and gold sets — `typed4`, labelsets v1–v4, the `judge/` pipeline)
> was later pruned to keep the repo lean; those names are the reproduction record of the
> time and live in git history. Older sections mention removed scan spellings (`--mode
> behavior`, `--no-contiguous`) — use [usage](usage.md) for the current CLI. The
> **current** benchmark is the v5 refactoring-family labelset (`bench/labels/eval_by_language.py`),
> see [benchmark](benchmark.md) (§AU onward).

## Measurement methodology

The early sections (A–T) measured against a 327-pair audited gold set
(`semantic_duplicates.v2`) with a **dev / held-out** repo split, line-span-IoU partial
credit, max-weight bipartite matching, repo-macro F1, and a hard-negative FP-rate, at a
**±0.019 macro-F1 noise floor** — deltas below it are not accepted. Reproduce with
`nose detect … --dump` then `nose eval` / `nose ceiling`. The target later moved twice
(§G to a strict Type-4 set, §W/§AU to the v5 refactoring-family labelset) as the goal
sharpened from "behavioral equivalence" to "refactoring-worthiness"; each move is noted
where it happens. Reference points at the start: a prior token-based baseline scored
~0.034/0.028 dev/held-out macro-F1; nose at that time scored 0.040/0.038.

---

## A. Adopted (in the main pipeline)

The shipped core, each validated by an equivalence fixture in `tests/equivalence.rs`:
tree-sitter frontends → one IL; coverage hardening (raw-node ratio 7.37% → <0.01%);
alpha-renaming; loop unification; idiom canonicalization; higher-order-function
unification; template ↔ concat; dataflow copy/expr propagation; the hash-consed value
graph (GVN) as the behavioral substrate; algebraic canonicalization (assoc/comm flatten,
De Morgan); control-flow normalization; LSH candidate generation at **k=128, b=32**.

### A2 — Determinism (symbol-content hashing)

Detection was nondeterministic (5099/5051/5066 predictions across identical runs) because
`ThreadedRodeo` assigns symbol ids in thread-race order. Fixed by hashing each symbol's
**string content** (`symbol_hash`, FNV-1a) rather than its interner id, so the
fingerprint is independent of arena order. Output is now byte-identical across runs,
thread counts, and machines.

## B. Measured on the gold set — what didn't move

LSH k=128/b=32 was the one adoption (candidate-reach 27% → 30%). Rejected or made
opt-in because they were within the noise floor or hurt precision: `b=64` (10.7M-pair
blowup), threshold lowering (F1 collapse), an atom-overlap scoring term (HN-FP up), DCE
(`--dce`, within noise), algebraic identity folding (byte-identical ⇒ zero effect), and a
coarse bag-of-operations channel (6× candidates, flat). Sub-function **blocks** were
opt-in here and judged FP-prone; that verdict was later overturned (§M) once the target
was fixed.

## C. Rejected idea *families* (the durable dead ends)

Three families that repeatedly fail, so don't rebuild them: (1) **threshold/parameter
tuning** — trades recall for precision on a shallow curve and collapses precision past a
point; (2) **coarse bag-of-operations features** — make divergent clones *surfaceable*
but not *separable*, so FP rises ≥1:1 with recall; (3) **cleanup normalization** (DCE,
algebraic identity folding) — real clones never differ this way, so **equality-saturation
/ an e-graph is not worth building** for cleanup (re-confirmed empirically in §BB).

## D. The recall funnel

Recall is lost in stages: gold → unit-extractable → candidate-reachable → scored. Early
on ~56% was lost at unit extraction and ~17% at candidate generation. The framing is
durable even as the numbers shift: the residual frontier is genuine algorithmic/structural
divergence (the undecidable Type-4 core), not a tuning problem.

## E. Cross-disciplinary candidate pipeline

A branch-per-idea protocol (clear the ±0.019 floor without a precision regression, else
drop) over ~20 ideas harvested from other fields (WL kernels, Smith-Waterman, NetSimile,
graph embeddings, ECFP/Morgan fingerprints, PDG slicing, Shazam hashing, BM25). A
nine-field sweep returned **1 merge / 8 drops**. The one win: **RANSAC consensus-offset
alignment** replacing the LCS scorer (held-out 0.0378 → 0.0489, HN-FP 0.103 → 0.077,
simpler and faster). Two lessons: candidate-widening is a dead family (recall is gated by
*scoring*, not generation), and **alignment selectivity is the lever** (selective RANSAC
helps, lenient DTW hurts).

## F. Non-exhaustive gold — LLM-as-judge pooling

A sparse gold set makes naive precision (~1%) meaningless. Fixed with pooling + a
calibrated LLM-judge oracle. The decisive finding: a **broad** "duplicate" definition
flags 95% of hard-negatives (ill-posed, non-separable), while a **strict** behavioral
Type-4 definition is crisp and separable but only ~20% of gold v2 meets it. Out-of-gold
real precision was ~6% (recall-corrected ~30%), not the naive 1%.

## G. Type-4-PURE benchmark — a benchmark-overturning finding

Strict-judging the full pool showed gold v2's `production_type4` label is **~95%
mislabeled** for behavioral Type-4: the pairs are same-skeleton/different-behavior siblings
a good detector *should reject* (`max >` vs `min <`, `parseFloat` vs `parseInt`). This
explains why nearly every prior experiment read "within-noise" — they optimized toward a
non-clone-laden target. **`typed4.v1` (65 strict positives), not gold v2, became the
target.**

## H. Judge reliability — a 3-persona panel, validated both ways

A prover/refuter/neutral panel (majority consensus) scored **0% false positives** across
all personas on 160 pairs, and the mislabel finding held (0% consensus on disputed
gold-type4). Judge *recall* was then validated on 18 hand-authored provably-equivalent
pairs → **100% consensus recall** (the two dissents were correct edge-case catches: NaN
identity, empty-array throw). Validated on both axes, the judge is a trustworthy oracle, so
`typed4.v1` and the §G overturn rest on solid ground. A detector floor-test (forced below
the size gate) showed a **tiny-function blind spot** — 13% recall, 60% cross-family false
merges — which is why `min_tokens=24` excludes that regime.

## J. Validated re-baseline + measurement stack

Rebuilt eval on the correct target with pool-aware precision and bootstrap CIs (`analyze.py`,
`synth.py`, one-command `bench.py`). Canonical baseline: type4 recall 0.589, pool-precision
0.059, AUC-PR 0.23, HN-FP 0.077. Three roadmap findings: the gap is **precision, not recall**
(AUC-PR 0.23 ≫ raw 0.06 ⇒ ranking is the top lever); nose was then a structural Type-2/3
matcher, not yet Type-4 (~6% transformation recall); normalization passes sat at noise on the
validated target.

## K. Semantic convergence + precision

The real Type-4 gap in production code is **async ↔ sync twins**, not loop ↔ reduce (which
barely occurs in real code). Adopted: async→sync name canonicalization (`__aexit__` →
`__exit__`) and small-int literal retention (`-2..=2` kept as value-keyed `LitInt`, since
`0` ≠ `1` is behavior). Rejected: a semantic floor `score = max(blend, vj)` — catastrophic,
predictions 3578 → 66,665. **Lesson: value-graph multiset-Jaccard is not precise enough as a
standalone acceptance criterion** (`vj ≥ 0.70` collides low-entropy small functions); it needs
precise semantic-key matching, not fuzzy similarity + a floor.

## L. Recall is extraction-bound — arrow-function units

Of 25 missed pairs, **19 were blocked at extraction**: the frontend only tagged
`function_declaration`/`method_definition`, so modern JS/TS `export const f = (…) => {…}`
lowered to an inline Lambda and never became a unit. Fix: a `const f = arrow/function-expr`
becomes a named `Func` unit (`lower_func_value`); inline callbacks stay Lambda. AUC-PR
0.263 → 0.337, precision held.

## M. Sub-unit (block) extraction — default ON

Flipped `--blocks` from opt-in (the §B "FP-prone" verdict predated the validated target) to
**default on**: every honest metric improved (pool-precision 0.064 → 0.106, AUC-PR 0.337 →
0.419, recall up, HN-FP flat). Real sub-function clones are small (24–40 tokens), so blocks
share the function size gate rather than a stricter one.

## N. IDF re-ranking rejected (threshold conclusion superseded by §O)

IDF-weighted multiset Jaccard was rejected (AUC-PR flat; the apparent HN-FP drop was just a
stricter operating point). This section also concluded "threshold is a weak lever, precision
tops ~0.16" — **that conclusion is a pool-precision artifact, overturned by §O.** The
IDF rejection stands.

## O. Unbiased precision benchmark — overturns §N

Replaced the biased overlap-weighted pool-precision with a **stratified-random sample by
score band, judge-labeled, population-reweighted** (`precision_sample.v1.json`). Score is in
fact strongly discriminative — §N was an artifact. **Unbiased overall precision = 17.9%**
(pop-reweighted, vs the pool's misleading 10.6%), and the precision-vs-threshold curve is the
load-bearing result:

| operating point | precision | predictions |
|---|---|---|
| ≥ 0.70 | 17.9% | 10,373 |
| ≥ 0.86 | 33% | 5,618 |
| ≥ 0.94 | 40% | 4,549 |

The bottom two bands (0.70–0.86, ~4,755 preds) are ~0% precision — pure noise the 0.70
default admits. **Do not fold the stratified labels back into the pool** (tested: it corrupts
pool-precision 0.106 → 0.060). Lessons: pool-precision is a flawed estimator; the threshold
*is* a strong precision lever (a product decision), not the weak one §N reported.

## P. Iteration loop toward world-class

A goal-driven loop (objective: recall@0.86 up, HN-FP=0 held, no prediction explosion). Net
result: **unbiased precision ~6% → 78% at recall 0.53, HN-FP 0, AUC 0.95.** The wins were
value-content gates, not threshold tuning. Confirmed dead-ends (cut here, recorded once): LSH
param sweeps (P1), callback-arrow extraction (P2), literal-weighted Jaccard (P6), a dual
candidate channel (P9) — all left recall flat because the bottleneck is *matching hard
cross-structure pairs*, not candidate generation. The operating point ≥0.86 was re-confirmed
as the balanced optimum (P10). The shipped sub-points:

- **P3 — string-literal value retention.** The dominant high-score FP was "same structure,
  different string constant" (locale tables, HTTP methods). Retain the string-content hash
  (`Payload::LitStr`) in value-graph keys while the structural tag stays abstract `Str`.
  pool-precision 0.154 → 0.316, AUC-PR 0.328 → 0.759.
- **P4 — literal values in the structural tag (rejected, durable lesson).** Folding values
  into the *shape* tag raised precision but broke known-equivalence fixtures and cost true
  clones. **Literal values belong only in the value graph (soft), never the structural IL —
  the IL's job is to converge equivalent forms.**
- **P5 — score-weight search.** Swept the `(vj, sj, ransac)` simplex at threshold 0.86.
  **Best weights = (0.5, 0.3, 0.2)** — RANSAC down-weighted from 0.5 to 0.2, because it
  rewards token-order agreement but is blind to literal values (locale tables share token
  sequences). Unbiased precision 38.1% → 57.0%.
- **P7 — data-table literal gate.** **A unit whose value-graph is ≥20% literal `Const` nodes
  is a "data table"; such a pair is capped by its literal Jaccard.** Plumbing:
  `value_fingerprint_lits` exposes the literal multiset, `UnitFeat.lits`, threshold env
  `NOSE_DH` (swept: **0.20 is the knee** — 0.15 starts costing recall). Removes 218 verified
  locale-table FPs at zero recall cost.
- **P8 — class-level attribute values in the value graph.** Class units stored data as
  class-level attributes that `process_stmt` put in `env` but never pushed to a sink, so the
  value graph saw class data as empty. Fix: for non-`Func` (class) units, expose final `env`
  values as effect sinks. Unbiased precision 57% → 75.3% (the biggest single gain).
- **P11 — return-signature gate.** The ≥0.94 residual FPs were one-element diffs (`__lt__` vs
  `__gt__` — identical body, different operator) diluted in the multiset. **Cap a pair's score
  by `ret_base + (1 - ret_base)·return_jaccard` when both units return values.** Plumbing:
  `value_fingerprint_lits` (3rd return), `UnitFeat.returns`, env `NOSE_RET` (**0.80 is the
  knee**). Removes 32 verified FPs, precision 75.3% → 78.1%.

## Q. Goal reframe — refactoring-candidate discovery

The strict behavioral judge (§H–P) was the wrong oracle for the *actual* goal: surfacing code
worth a human's refactoring review, where a small FP rate is fine. Under a
refactoring-worthiness rubric, the §P precision gates were *deleting good candidates*. This
split the tool into two operating points — a strict behavioral path (gates on, 0.86) and a
candidate/refactoring path (gates off, **0.70 operating point**) — the seed of today's
`semantic` vs `near` channels. A dual candidate channel was re-tested here and rejected again
(pairs explode 4×, recall flat): candidate generation is architecturally dead for recall.

## R. Performance — frontend parser pool

The frontend (discover + parse + lower) dominates a scan (~88ms warm vs ~13ms pipeline).
**Adopted: a thread-local parser pool** — cache one `tree_sitter::Parser` per grammar per
rayon worker (`lower::parse`); ~1.8× (date-fns 88 → 48ms), byte-identical. (A `SmallVec`
child-list was a noise-level null result — parsing dominates, not allocation.)

## S. Cross-language convergence audit (bug hunt via equivalence testing)

Writing the same algorithm in each language and asserting the units converge to one IL hash
surfaced lowering bugs no single-language test catches. **The durable principle: per-language
coverage (Raw% ≈ 0) does not imply correct convergence — a construct can lower cleanly yet to
the *wrong shape*; one-algorithm-×-N-languages → one-hash convergence tests are the
discriminating check.** Bugs fixed: Rust `*x` deref wrongly became `UnOp(Neg)` (any non-`!`
unary treated as negation); Python f-strings / Ruby interpolation dropped the interpolated
expr (now folded into a `Str`+`Add` chain like `lower_template`); and branch-orientation
produced non-canonical comparisons — `invert_comparison` now returns the canonical operator
plus an operand-swap flag (`Lt`→`Le`+swap). Corpus coverage after fixes: 99.99%.

## T. Performance — parallelize every stage (~14k → ~19.5k files/sec)

parse+lower already scaled 11.6× across cores (CPU-bound on tree-sitter); the wins were in the
remaining stages. **T1 — parallel file discovery** via `ignore`'s parallel walker (33 → 20ms);
crucially, **paths are sorted by name, so a file's `FileId` is deterministic across machines**.
**T2 — sort-based parallel LSH** (3.6×): emit `(band-hash, unit)` entries → `par_sort_unstable`
→ pairs per equal-hash run; byte-identical. **T3 — fuse normalize+extract** into one
`flat_map_iter`, halving peak IL working set. (Pre-sizing the IL arena was slightly slower;
reverted.)

## U. Refactor-worthiness ranking — test-awareness + type-def discount

For the refactoring goal the metric is top-family precision, not Type-4 recall. The dominant
real noise is test duplication and value-poor type definitions. A ranking-time discount (scan
path only; `rank_families`, gold path untouched): each family is tagged `scope = prod | test |
mixed`, and **all-`Class` families with mean `sem < 12` are ×0.25**. Disable with
`NOSE_NO_REFACTOR_DISCOUNT=1`. **Mixed test↔prod is *not* discounted** — logic that lives in
both a test and production is a real smell.

The all-`test` ×0.2 discount this section originally added was **reverted in §U.1**:
duplication in tests is a genuine smell, and suppressing it works against being a copy-paste
recall superset (79% of jscpd-weak findings are in test code). The `scope` tag survives as
reported *context* with no ranking effect; the value-poor type-def discount stays.

## V. jscpd-weak superset — the contiguous channel

Expanding the corpus to 31 repos across all 8 languages exposed that nose was far from a
jscpd-weak superset (all-pairs coverage 18.2%): **jscpd matches arbitrary contiguous token
runs, nose matched unit-bounded families.** Closing the gap needed a second channel.
**V.2 — the contiguous copy-paste channel** (`contiguous.rs`): a Rabin-Karp scan over each
file's **raw-IL** token stream finding maximal duplicated runs regardless of unit boundaries —
the Type-1/2 floor. Built from raw IL because alpha-renaming is function-scoped; honours
`// nose-ignore`. Coverage 18.2% → 78.1%. This is today's `syntax` channel.

## W. Refactoring-family labelset + the product metric

Built the ground-truth eval the goal needs (`bench/labels/`): an unbiased candidate pool
(nose-structural ∪ jscpd-weak) labeled `worthy`/not by a 3-persona LLM panel, dev/held-out
split. Result: worthy-**recall 97%**, **precision@10 57%** — recall is excellent, **ranking
precision is the lever**. ~43% of the top-10 are not-worthy (parallel-by-design, locale/i18n
maps, generated/vendored). This is the measurement foundation the §U/§V false starts lacked.

## X. Ranking precision — labelset-driven

Using the §W labelset as ground truth, **every candidate ranking signal was validated before
shipping — and the labelset rejected most of them, exactly as intended.** Only the
generated/vendored-path discount (×0.1, scan-only) shipped: precision@10 61% → 63%, recall
held at 97%. Rejected: a literal-dominance (`data_ratio`) down-weight (the opposite of the
hypothesis — high `data_ratio` is *more* worthy) and a candidate-mode data-table gate. The
dominant remaining polluters are zod-style locale/version parallel-data variants, structurally
identical to worthy duplication under every cheap signal.

## Y. Anti-unification re-rank — the reframe (metric gain didn't replicate)

The durable reframe: rank by how clean the shared *abstraction* is, not raw duplication
volume — anti-unify two members into a template with `struct_holes`/`value_holes`, where
`value_holes` catch the zod-locale polluter (the holes *are* the content, not a parameter).
The reframe ships later as `--show proposal` (§AF) and informs `extractability` (§AZ). The
simulated **+8pp** precision gain, however, was validated only on the small v1 set and **did
not replicate** on larger labelsets (next sections).

## Z–AD. The re-rank metric was noise; precision power is repo-bound

A multi-section arc (per-language eval, bootstrap CIs, labelsets v2–v4 up to 4,615 families)
that **dissolved its own narrative before anything shipped**. A per-language A/B first showed
the §Y gain was +22pp on TypeScript and −5pp on Rust; bootstrap CIs (§AB) then showed *both*
were within noise and the re-rank gain never replicated heldout (62% → 62%). Two durable
results: **do not ship the uniform re-rank** (recall-side levers are the real ones), and
**per-language precision power is bounded by #repos × 10, not #labels** (P@10 samples only the
top-10/repo), so adding labels per repo doesn't tighten per-language CIs — adding repos does.
"The eval infra paid for itself by dissolving a multi-section false narrative."

## AE. Robustness — never crash on real input

The 62-repo corpus surfaced a stack overflow (deeply-nested minified bundles) in the recursive
lowering walk on rayon's ~2MB worker stacks. Fix: 1 GiB stacks for the workers and the
command thread; regression test `deeply_nested_file_does_not_overflow` (depth 40,000). A clone
detector must never crash on real input.

## AF. Extraction-proposal output (`--show proposal`)

Shipped the §Y reframe as the user-facing proposal view: line-granularity anti-unification
of two representatives (reusing the diff-view LCS) — shared lines become the helper body,
differing runs become `⟨param N⟩`. The current CLI exposes it as `--show proposal`.
Line-level is the pragmatic granularity (sharp on function-level near-dups, coarse on
whole-file clones).

## AG. Lowering closure — every language to non-ERROR Raw ≤ 0.5%

Closed the lowering campaign (begun in §AA, the per-language Raw-gap work) to target: all 9
languages at 0.01–0.25% non-ERROR Raw, no construct > 0.3%. Two disciplines: route stray
statement kinds back through the statement path, and erase type-level nodes to `empty_block`,
not `Raw`. The remaining Raw is essentially all ERROR (tree-sitter parse failures — the
irreducible floor); further Raw fixes would be metric-gaming. `bench/lowering_gaps.py` is the
work-queue dashboard.

---

## AH. The two-axis principle — why "find similar" and "be rigorous" don't conflict

The apparent conflict between finding behaviorally-same code that *looks* different (Type-4
recall) and rejecting off-by-one / wrong-operator near-misses (rigor) is an **architecture
smell, not a goal clash.** Two conflations cause it:

1. **Two purposes under one threshold.** Refactoring/DRY wants recall and tolerates
   near-misses; behavioral-equivalence assertion wants precision and must reject an off-by-one.
   One global threshold cannot serve both.
2. **Two kinds of difference under one scalar.** A single similarity score blurs
   *representation* differences (names, order, sugar, loop form, commutative reorder — which
   Type-4 should ignore) with *behavioral* differences (`+` vs `*`, `>=` vs `>`, constants,
   control flow — which Type-4 must never ignore).

**The resolution — and it is what nose exists to do — is to separate the axes:**
representation differences are absorbed by *exact* canonicalization (alpha-rename, GVN,
commutative sort — push more variation into byte-identical post-normalization); behavioral
differences are measured *strictly* on the residual (an operator swap is a different program,
not a near-miss); and output is graded, not binary, so the consumer picks the cut. The deeper
thesis: token-set similarity was exhausted after 160 experiments, and **rigor is not in tension
with the thesis — rigor *is* the thesis. Hardening the substrate is what enables tolerance: the
more exactly representation variants collapse to identical, the farther-apart forms can safely
be called the same.** This drives work items #1 (the two-axis evaluator) and #3 (the
value-graph loop-recurrence normal form).

## AI. The two-axis evaluator + value-graph reduction normal form

Operationalizes §AH. **#1 — the instrument** (`nose features` + a convergence probe): read
fingerprints *directly*, bypassing the LSH → threshold → union-find pipeline (which confounds
"did the signal converge?" with "did the pipeline surface it?"). It measures value-Jaccard for
*equivalent* pairs (representation axis, want → 1.0) vs *near-miss negatives* (behavior axis,
want → 0.0), the margin between the two clouds, and a threshold-free **rank-separation** =
P(an equivalent pair outscores its family's negatives). The baseline was damning and
clarifying: **representation 0.25, behavior 0.57, margin −0.32, rank-sep 18% — the signal was
inverted** (near-miss bugs looked *more* similar than true Type-4 equivalents).

**#3 — the loop-recurrence normal form** fixed it: thread the recurrence (carry symbolic
prev-iteration values so reductions reach the fingerprint), canonical reductions
`Reduce(⊕, init, contrib)` whose per-element `contrib` keys the value (so sum vs product stay
distinct — behavior preserved), and indexed-`while` induction-variable detection
(`xs[i]` → `Elem(xs)`). Nine increments closed the long tail (HoF → Reduce, guarded/filtered
reductions, min/max selection, zip/`enumerate`), flipping the margin positive. Final:
**representation 0.25 → 0.73, behavior 0.57 → 0.39, margin −0.32 → +0.34, rank-sep 18% → 76%** —
inverted to strongly correct, each transformation family locked by an equivalence test.

Crucially, **§AH is now in the code, forced by a test**: sharpening behavioral precision broke a
candidate-mode test that merged a sum-loop with a product-loop, so **strict mode trusts the
value graph (behavioral); candidate/refactoring mode is structure-dominant (shape-weighted)**,
and two units sharing a skeleton but differing in a behavior-defining operator still surface for
human review. `recursion_iteration` (loop ↔ recursion) is left **explicitly out of v1 scope** as
a meaning-risking rewrite.

## AJ. Behavioral oracle — verifying the value graph is *sound*

A deterministic partial interpreter over the normalized IL (`crates/nose-normalize/src/interp.rs`)
plus a checker (`nose verify`) that groups units by value fingerprint and asserts
**fingerprint-equal ⟹ behavior-equal on every input** (a battery of input vectors per
interpretable function). It is intentionally *partial*: any unmodeled construct (opaque call,
field access, exception) makes the whole unit uninterpretable and it is excluded — never
guessed. (A genuine runtime *type error*, though, is behavior, not an unmodeled construct: e.g.
iterating a non-iterable — a scalar where the battery feeds one to a `for`-each — yields `Err`,
so a foreach-accumulator stays interpretable across the battery instead of being dropped.) It **need not match any language exactly, only be self-consistent**: a genuinely
equivalent pair agrees under any consistent semantics, so a merge the interpreter contradicts is
a real bug. This sets the asymmetry that defines the instrument: **soundness violations are
proofs (every one a real bug); completeness misses are leads (some real, some battery
artifacts).** Run on the 62-repo corpus it caught two violations a synthetic dashboard could not,
then reached SOUND (0 violations) after fixing them: **(1) path-insensitive returns** —
branch-swapped `if c {return A} else {return B}` fingerprinted identically; fix tags each
return/throw with its **path condition**. **(2) duplicate-parameter collapse** — `f(a,a)`
matched `f(data)`; fix seeds parameters by **position**, not cid.

## AK. Wiring the verified value graph into detection (the soundness payoff)

The value-graph work was *stranded*: the detector blended syntactic terms (shape + RANSAC) that
drag a true Type-4 clone below threshold no matter how well `vj` converges. **Because `nose
verify` proved identical value fingerprints ⟹ behaviorally equal (0 false merges across 15k
units), the detector trusts an exact value-fingerprint match outright and accepts it regardless
of syntax** — a one-line fast path, guarded by a minimum fingerprint size. Calibration
(`P(behavior-equal | vj)`) shows a sharp cliff that justifies exact-only:

| value-Jaccard | P(behavior-equal) |
|---|---|
| 1.0 (exact) | **100%** (347,513 pairs) |
| [0.8, 0.9) | ~75% |
| [0.5, 0.8) | ~82% |

**A verified-sound semantic signal can be trusted aggressively, and that trust is what converts
representation convergence into real detection.** The rule that follows: **the remaining
partial-`vj` Type-4 clones must be caught by *raising* their `vj` to exact (more
canonicalization), not by *lowering* the threshold to admit them.** (Synth T4 recall 0/17 →
3/17, 0 FPs; labelset P@10 59% → 62%.)

## AL. Closing the jscpd-superset recall gap (72% → 92%)

Four frontend fixes, no gaming (labelset P@10 59% → 69%, worthy-recall 97% → 99%): recurse into
C `preproc_if`; lower TS type/interface/enum decls to a structural skeleton; emit
import/`#include`/`use` block tokens (54% of misses); and lower the contiguous floor from 20/4
to 10/3 tokens/lines.

## AM. Quantifying value-add over jscpd — the oracle as judge

`bench/value_add.py` uses the §AJ oracle as an independent judge (GOLD = interpretable pairs with
identical non-trivial behavior). Baseline: jscpd recall 90.0%, nose 95.7%, **value-add 57.1%**
(12 of 21 jscpd-missed pairs recovered) at **100% behavioral precision**. The size gate is
critical — 7,391 raw pairs reduce to 211 meaningful ones (97% were trivial fixtures) at
≥5 lines / ≥24 IL tokens.

## AN. Scaling the controlled benchmark — 8 languages, the two-axis guard

Rebuilt the controlled set (742 fixtures / 671 clone pairs, 8 languages × 10 algorithms ×
base/t2/t3/t4/neg + cross-language). Negatives are single-operator behavioral near-misses
(`>` → `!=`) — the no-gaming spine. The benchmark **must report both §AH axes** (candidate vs
behavioral) and read precision off the behavioral axis. (A two-axis precision leak measured here
at 61% was a threshold-measurement bug, corrected to 25% in §AP.)

## AO. Behavioral-axis fix — the counting-loop induction misclassification

A counting accumulator `count += 1` matched the induction-variable shape, was bound to
`idx(xs)`, and never reached a `Reduce` — the whole accumulation evaporated (identical
fingerprints across `>`/`>=`). Fix: a genuine loop counter both steps by a constant **and**
governs the loop condition (intersect `induction_vars` with the condition variables). A textbook
Pareto move — lifts both precision and recall.

## AP. The threshold measurement bug — the real baseline is 25%, not 61%

The benchmark read the behavioral axis through the refactor path's `0.70` candidate default
instead of the detector's calibrated `0.86`, inflating the whole baseline. Corrected: behavioral
neg-FP 61% → 25%, T4-strict 25% → 18%. Separately, a Java `class { method }` wrapper collapsed to
a 2-atom shell (`process_stmt` had no `Func` case); fix: **a container's behavior is the
aggregate of its methods** (`build_unit` descends into each `Func`). Java FPs 20 → 7. This is the
canonical correction later sections defer to.

## AQ. The size gate was the T4 recall blocker, not the value graph

The missed T4 forms were dense one-liners (`sum(v for v in xs if v>0)`, `max(xs)`); the value
graph *does* converge them (jaccard 1.00), but they fell below the unit size gate and were never
extracted. Fix, in the spirit of §AH (**gate on *semantic* content, not surface size**): admit a
frontend-tagged function below the line/token gate when its value fingerprint is rich enough
(`value.len() >= 6`, the floor the exact-match path already requires). The largest single
increment, clean Pareto: value-add 57% → 66%, precision held.

## AR. Two idiom fixes (partial)

`functools.reduce(f, xs, init)` was misrouted through the method-HoF arm (treating the
`functools` module as the collection); special-cased to `Builtin::Reduce`. And swapped-polarity
guarded folds (`acc + v if v > 0 else acc`) gained the swapped `Phi` case. Both sound but
partial — the Python idiom long tail has diminishing returns.

## AS. Soundness bug hunt — seven false merges, each with a reproducer

An adversarial hunt for false merges (the corpus oracle reported 0, so bugs were crafted as
one-dimension near-miss pairs). **fingerprint-equal must imply behavior-equal.** Seven bugs, each
locked by a `tests/equivalence.rs` reproducer (fails before, passes after), in two families:

**Family A — loop iteration-extent dropped** (the value graph abstracted `C[i]` → `Elem(C)` as
if every loop visited all of `C`): **(1) range-start** — `range(len(a))` ≡ `range(1, len(a))`,
now only a provably-full range licenses the `Elem` rewrite; **(2) while-stride** — `i += 1` ≡
`i += 2`, only unit-stride zero-start counters are full indices; **(3) early-break** —
prefix-sum-with-`break` ≡ full sum, `break` now records its path condition as a distinct sink.

**Family B — identity/value dropped in lowering or alpha-renaming:** **(4) slice bounds
(Python)** — `a[1:]` ≡ `a[:1]`, collecting only *named* slice children dropped which slot the
bound occupied; **(5) slice/range bounds (Go, Rust)** — same collapse, plus Rust merged `1..2`
with `1..=2`; **(6) free-variable collapse** — alpha-rename gave *every* name a positional cid so
`foo(x)` ≡ `bar(x)` and `max(a,b)` ≡ `min(a,b)`; now only *bound* names are renamed, free names
keep identity (zero recall cost); **(7) boolean literal values** — `True` ≡ `False` (abstracted to
a valueless `Lit(Bool)`); added `Payload::LitBool(bool)` end-to-end.

## AT. Reconsidering the "lossy approximations" — `in` was a bug too

Auditing the §AS "deliberate lossy approximations" found an eighth rationalized bug: `in`/`is` →
`Op::Eq` was unsound — membership is non-commutative (`a in b` ≢ `b in a`) and lowering dropped
negation. Fix: a non-commutative `Op::In` (interp gained a membership arm); `not in`/`is not` keep
negation. This established the **standing three-way classification** that
[normalization](normalization.md)'s soundness constraint now states:

- **Rationalized bugs** — none known; the §AS seven and `in` are fixed.
- **Genuine limitation, not "acceptable"** — string/list concatenation via a commutative `+`
  (`s + x` ≡ `x + s`) is unsound, but a sound fix needs type/sequence inference a type-free
  cross-language tool lacks (first supplied as `types.rs`, later moved to `ValueDomain` /
  `ValueLaw` contracts in `nose-semantics`; §AW/§BA).
- **Legitimate fuzzy tradeoff, but mis-placed** — large-constant / float abstraction
  (`x % 7` ≡ `x % 11`) belongs on the candidate axis, not the shared value graph (it violates the
  behavioral axis's "constants must be distinct" rule). The principled fix is an axis split.

## AU. Cross-field divergence → the precision frontier → v5 settles the re-rank

Six subagents brainstorming from different fields **all converged on the same architecture —
structure-invariant *candidate generation* → behavioral *confirmation* (the oracle as generator,
not just checker).** Two concrete bets were refuted by measurement (behavioral-near-match gating;
symmetry-orbit/naming-parallelism — zero separation). The product reframe: **worthy-recall is
solved (~100%); the headroom is precision, and 62% of the precision loss is one category —
`parallel-by-design`.** Growing the gold set to v5 (105 repos, 9,461 families) settled the §Y
abstractness re-rank: **it does not generalize** (the v4 +5pp dev gain collapses to ~0 heldout, a
Rust-only effect) — **do not ship it.** The precision frontier is real and **judgment-deep**; the
remaining lever is an LLM-judge re-ranker, not another cheap feature.

## AV. The precision loss is judgment-deep all the way down

There is no cheap *sound* structural gate for the "detectable" not-worthy categories — type-def
vs extract-base, trivial vs worthy-parameterize, generated — each is entangled with worthy
lookalikes of *identical shape* (e.g. httpx `get/post/put → request(VERB)` is structurally
identical to a non-worthy thin delegation). The §Y abstractness re-rank nets positive only for
Rust, and only because Rust's *base* value-rank is poor (it buries clean small helpers under
module-level matches); elsewhere it demotes worthy larger families. The genuine,
language-agnostic sub-signal is "base value-rank under-ranks small clean helpers" — which §AZ
exploits.

## AW. Core-hardening — sound foundation + machine-checked canons + type inference

A deliberate pivot from the judgment-deep product metric toward a *sound and capable core*, with
**the verifier as the safety net for bold attempts: an unsound canon shows up instantly as a
false merge and gets rolled back.** **Phase 0** drove false merges 15 → 0 via five
language-general fixes (subtree-hash keying for `Raw` nodes, dead-code-after-unconditional-return
drop, last-write-wins per field, `Err` propagation through conditions, excluding empty
fingerprints from `verify`). **Phase 1** moved the soundness contract from empirical ("0 merges on
N repos") to **proven in Lean 4** (`normalize.value_graph.algebra`,
`normalize.control_flow.guard_returns`, `normalize.value_graph.functor`:
AC-flatten+sort denotation-preserving, `a − b → a + (−b)`, guard-clause ≡ if-else, map-fusion
functor law). Bold canons were verifier-gated: untyped `-(-x) → x` / `x & x → x` were **refuted
(caught 17 false merges** — they drop the operator's type-error behavior), then re-enabled
*soundly* via purpose-fit type inference (`types.rs` at the time, now `ValueDomain` /
`ValueLaw` contracts in `nose-semantics`; coarse Num/Bool/Str/List/Unknown):
**`+` commutes unless an operand is proven string/list; Unknown still commutes, so the common
numeric case is unaffected.** The standing principle: **each canon is justified by correctness +
soundness + a proof, not by moving a noisy completeness number** (which is insensitive to any one
correct canon).

## AX. The independent oracle — unmasking the commutativity-of-non-commutative-ops bug class

§AW's verifier had a hole: it interpreted the *same fully-normalized IL it fingerprinted*, so any
behavior-changing canon **masked itself** (`a or b` and `b or a` both sorted to one IL, looked
identical). **A differential oracle must not share its subject's canonicalization, or it certifies
the very rewrites it should police.** The fix: it now interprets the **pre-canonicalization core
IL (`desugar` + `alpha` only, via `NormalizeOptions.oracle`), matched to each fully-normalized
unit by source span, while still fingerprinting the full normalize.** This exposed a whole bug
class — treating non-commutative operators as commutative — each a real latent false merge fixed at
root: value-`and`/`or` short-circuit (commutativity type-gated on Bool, else a positional `Phi`),
`!!x → x` (`!!5` = true ≠ 5), `not(Err) → Bool(true)` must propagate, `x*1 → x` / `x+0 → x`
unsound for non-numeric, and string/list `+` operand sort (concat is non-commutative). A second,
**pair-free canon-preservation check** (interpret each unit on core IL *and* full IL, require equal
behavior) flagged 20 concat sites with no colliding twin needed. Result: `verify` = 0 false merges
under the independent oracle, canon-preservation = 0 behavior-changing units. (The completeness
ratio dip 62% → ~59% is honest oracle fidelity — the denominator grew — not a regression.)

## AY. Re-sweeping the log with the better system (types + v5 + oracle)

Re-tried old blockers the hardened system might lift. Three IL adoptions: existence/universal loop
forms (`for … return True/False` ≡ `any`/`all`), collection-building loops ≡
comprehensions/`.map`/`.collect` (cross-language, +8 completeness), and float-constant distinction
(retained source-text hash — floats had collapsed to one token, a latent false merge the float-less
oracle couldn't see). One rejection: doubling `x*2 ≡ x+x` (made `verify` ~10× slower for a marginal
idiom). Critically, all three adoptions *strengthened* the behavioral fingerprint yet v5 P@10 did
not move — **empirically confirming the precision ceiling is judgment-deep, not
semantic-signal-limited.**

## AZ. Extractability as the default ranking — the re-rank that *does* generalize

§AU/§AV settled that a uniform abstractness re-rank does not generalize. The **`extractability`
ranking — now the default sort for `nose scan`** — is not that re-rank: instead of a bare
abstractness multiplier it scores *invariant (shared) source lines × copies × spread* with three
correctives the prototype lacked — **tightness** (shared/total, so a 22/384 dispatch skeleton can't
outrank a 15/15 pair), a **parameter penalty** (a 30-hole "helper" is scaffolding), and an **IDF
idiom-gate** (pervasive lines like `if err != nil {` contribute ~0) — plus zero-invariant-line
families score 0 (the structural-only `sim 1.00` pathology) and a type-def/generated discount;
cross-language families fall back to the structural estimate. In the historical §AZ slice it was
the first ranking change to move the held-out number in the right direction (held-out +6pp,
dev flat, no recall cost, reordering only). The durable lesson is that a re-rank built from
what actually extracts (tight, few-param, non-idiom shared lines) generalized where one built
from raw structural abstractness did not. For current reproducible P@10/recall numbers, use
[benchmark](benchmark.md); `--sort value` is retained for raw-volume triage, and detection is
unchanged (same families, only order and the `N/M shared · Pp` cell differ).

## BA. Exact-Type-4 convergence push — stronger types, Lean-backed algebra, filter fusion

A focused pass to raise *exact* Type-4 convergence while holding full-corpus `verify` = 0 and
backing each algebraic law with a Lean proof. **Adopted** (93 equivalence tests green, SOUND):
fixpoint param-type inference over subexpression result types (`types.rs` at the time, now
`ValueDomain` / `ValueLaw` contracts in `nose-semantics`, licensing the gated numeric rewrites);
distribution/factoring `a*c + b*c → (a+b)*c` gated on proven Num
(`NoseAlgebra.distrib_sound`); full **AC canonicalization in the value graph itself** (`mk`
flattens+sorts `+ * & | ^`, so *synthesized* nodes re-canonicalize, not only the IL algebra pass);
**filter fusion** representing `filter(p, c)` as a filtered identity-map `Hof(Map, [Elem c, p])` so
nested filters fuse to `p ∧ q` (`NoseFunctor.filter_fusion` — the deferred "make Filter carry its
element"; an earlier peel-to-bare-`Filter` caused 2 false merges, this does not); reduce-lambda
min/max selection; count-of-filter; method-form iterator reductions
(`xs.iter().filter(p).sum()` ≡ Python `sum(… if p)`); and **dict-builder ≡ dict-comprehension**,
sound by *representation* — `DictEntry` is a distinct node from a tuple `Seq` (guarded by
`assert_ne!`), since dicts are not oracle-modeled. **Rejected as cross-language unsound:** doubling
`x*2 ≡ x+x` (canonical form depends on whether operands prove Num) and negative-index
`s[-1] ≡ s[len(s)-1]` (last-element in Python/Ruby, undefined in JS) — both *genuine
language-semantic divergences, not representation gaps.* Verdict: **full-corpus `verify` stays 0
false merges across 28,113 interpretable units, and the v5 refactoring-precision number is
unchanged — reconfirming §AY that behavioral-convergence gains don't move the judgment-deep
number while costing nothing there. The win is squarely on the exact-Type-4 axis.** The Lean core
gained the `normalize.value_graph.compare` obligation; a `formal` CI job regression-checks all
theorems.

## BB. Confluence audit + lattice comparison canon (rules, not a new engine)

Probed the "replace ordered passes with an e-graph / equality saturation" thesis by first
*measuring* whether the recursive `mk` already behaves as a fixpoint: seven phase-ordering-stressing
equivalences → **6/7 already converge** (including multi-step `a*c + b*c + d*c → (a+b+d)*c`). This
reproduces §C/§AW by construction: **the lever is new sound rules, not a better rule-application
engine** — an e-graph would still need each rule declared, and the fixpoint it buys is largely
already present. The one gap was the lattice identity `(x ≤ y) ∧ (x ≠ y) ≡ x < y`; adding just that
one rule (`value_graph.rs lattice_le_ne_to_lt` + dual) *composes through the `mk` fixpoint* to close
the full cross-language `not(a > b or a == b) ≡ a < b`. Sound on any total order
(`normalize.value_graph.compare`).

## BC–BF. Behavioral-equivalence gate and widening the oracle

A four-part thread (a research subcommand only — not a detection channel) that probed using the
interpreter oracle as an in-loop *acceptance gate*, then chased the lead it surfaced.

- **BC — the gate has no headroom.** On a 10k synthetic corpus, exact fingerprint already merges
  100% of interpretable positives, so behavioral acceptance recovers nothing and only adds false
  merges. A wider input battery cut false merges 7.9% → 5.5% but never to zero — reaffirming the §AK
  cliff that *only exact equality is 100% sound.* The actionable finding inverts the hypothesis:
  **the interpreter oracle, not the fingerprint, is now the weaker model** (behavioral recall 64.9%
  vs the fingerprint's 100%, because map/option/string predicates fall outside the interpreter's
  faithful Int/Bool/Str/List domain).
- **BD — the lead was mis-aimed.** Classifying 1,056 synthetic `verify` "violations": ≈98% are
  numeric reductions in C aligned-array form `f(int *xs, int n)` merged by the "`n` is exact length"
  contract while the oracle feeds a *free* `n` — i.e. **the C pointer-length contract, not maps**
  (maps are <2%). Modeling `GetOrDefault` was inert because `verify` interprets the pre-canon core
  IL (§AX) where a map-default is still raw indexing; reverted.
- **BE — the pointer-length contract, executed.** The oracle now binds `n = len(array)` per battery
  row where `full_pointer_length_contract` fires (the same contract the value graph used to merge).
  Synthetic violations **1,056 → 508 (−52%, strictly monotone)**; real-code `verify` stays SOUND.
- **BF — rebase verdict (what survived a refactored `main`).** A later `main` removed a family of
  interpreter builtins (`IsEmpty`/`Contains`/`GetOrDefault`/…) and changed some lowerings (Java
  `Math.min(a,b)` now an opaque call, not `Builtin::Min`). **Obsoleted and dropped:** map-read and
  nullish/option modeling (depended on the deleted builtins) and two-arg scalar `min`/`max` (now
  inert). **Survived / re-validated:** the §BB lattice canon (`convergence_probe5` 10/10) and the
  §BE pointer-length contract (re-measured 800 → 252). The durable lesson: **a soundness-oracle
  improvement is durable only insofar as the IL shape it keys on is durable** — canons keyed on
  stable value-graph structure survived; builtin-keyed modeling did not.

## BG. Hazard ranking — divergent-edit calibration from mined history

A *severity* ranking ([hazard-ranking](hazard-ranking.md)) distinct from extractability:
rank families by how likely they are to be edited inconsistently and cause a bug. The
literature ([hazard-benchmark](hazard-benchmark.md)) gave the signals and directions but
not the weights, so we mined ground truth before implementing.

- **BG-data.** Used nose as a cross-revision linker (`eval/hazard/`): monthly snapshots
  of **12 repos across 8 languages** (django, pandas, kafka[Java], terraform, hugo, tokio,
  ripgrep, redis, vue-core, express; thrift[X], grpc[X]), labeling each family-interval by
  Kim's Inconsistent-Change from `git diff` over member spans; **G2** = a G1 whose changed
  sibling's *function* was modified by a bug-fix commit that did not propagate (git
  `-L:funcname`). **462,569 events; 4,639 divergent edits (G1), 181 "G2" over 15,199
  families.** Function-level attribution landed the G2 *rate* in the literature's 1–3%
  range — **but an LLM-judge audit of all 181 found the G2 label only ~11% precise**
  (48 message false-matches, 47 intentional divergences, 41 not-clones). So **G2 is
  retracted as a gold label**; validation rests on the clean, directly-observed **G1**.
- **BG-finding — the pre-data formula was mis-specified.** Leave-one-repo-out logistic
  weights (stable): `mean_lines` **+0.43** (top), `modules` **+0.28**, `mean_sem`
  **−0.27 (anti)**, `invisibility` **+0.14**, `members` +0.13, `params` +0.04 (noise — sign
  flipped from −0.06 at 7 repos). The first-draft design led with `mean_sem` as the
  *primary* multiplier — but semantic-fingerprint size is **anti-predictive** for
  divergent-edit ranking (typical divergences are in smaller families; the mean is a
  large-tail artifact). Source-**line** span is the real magnitude signal.
- **BG-formula.** `hazard = mean_lines × spread(files,modules,languages) × invisibility ×
  scope_weight` — leave-one-repo-out AUC **G1 0.644** vs **0.609** size-led draft, 0.611
  value-baseline, ~0.49 random. **Implemented as opt-in `--sort hazard`**;
  `extractability` stays the default fixability axis. The param-dampening term tested
  earlier was dropped (sign-unstable weight).
  `invisibility` (1−tightness) is a modest, stable general signal (+0.14). **Correction:**
  a first draft claimed it was "the top signal in the cross-language stratum (0.67)" —
  but that was a repo-level mislabel (thrift+grpc tagged X). True cross-language families
  are structurally rare (37 of 15,199; arrow 0 of 928), so the cross-language-specific
  claim is retracted; invisibility holds as a general predictor.
- **BG-audit — the gold label was mostly noise.** An LLM judge reviewed all 181 G2 events
  blind (`audit_sample.py` rebuilds the two members' code + the bug-fix commit): **strict
  precision 11% (20/180)**. False sources: 48 message false-matches (the bug-fix keyword
  caught version drops, features, typo/docs/config changes), 47 intentional divergences
  (async/sync, virtual/stored, test variants that legitimately differ), 41 not-clones
  (near@0.70 grouped trivial stubs). The lesson: `rate-match ≠ precision`, and a real gold
  label needs the LLM judge *as the labeler*, not the keyword heuristic. The 20 confirmed
  positives seed a real (small) gold set.
- **BG-gold — the formula predicts propensity, not harm.** Built that real gold: an LLM
  labeled 1,390 G1 candidates blind *with the diff* into harm/should-propagate/benign,
  adversarial pass refuting weak positives (`build_candidates.py` → `gold-label-divergence`
  → `gold_eval.py`). Only 22 (strict) / 53 (lenient) are genuine should-propagate harms
  (~1.6–3.8%, reproducing the literature's 1–3%). On this gold, AUC for harmful-vs-benign
  divergence: `mean_sem` 0.61–0.64 (the *dropped* feature, best), `extractability`
  0.59–0.64, **`hazard` 0.51 (chance)**, value 0.42. **The G1 0.64 does not transfer to
  harm** — propensity ≠ harm, and static features cap ~0.6 (harm depends on whether a
  change *applies to the sibling*, a semantic question). Also: 50% of candidates are not
  real clones (near@0.70 precision). → `hazard` reverted to opt-in (default stays
  `extractability`); subsequent rounds test whether git-history, larger gold, and better
  clone precision can move the ceiling.
- **BG-gold2 — the structural+history ceiling is ~0.60 (definitive).** Did all three:
  a clone-quality gate (`shared_weight≥4`), a larger gold (2,296 labeled, 51 confirmed
  harm positives, usable CIs), and a git-history feature (blame: were the changed vs
  lagging member last touched *together*?). Harm-AUC: `-skew_days` 0.600, `mean_sem` 0.572,
  `same_commit` 0.568, `hazard` 0.531, `extractability` 0.475; a leave-one-repo-out logistic
  **combination 0.524 — no lift.** git-history is real and theory-aligned (harm happens in
  families previously maintained *together*, Barbour/Kim) but weak and only ~52%
  computable; the gate still left 46% non-clones. **Conclusion: clone-structural +
  git-history features cannot rank harm above ~0.60.** Harm is semantic — the LLM judge
  captures it (the gold's basis), metrics do not. The evidence-indicated harm ranker is a
  **bounded LLM pass over top-K structurally-surfaced candidates**, not more features.
- **BG-gold3 — cognitive complexity (#23) moved the ceiling, post-divergence.** Tested
  the parked #23 edit-surface idea on the same gold from captured member code/diff
  (`cogcomplexity.py`, `harm_model.py`). `diff_per_cog` (a small subtle change in a
  *complex* function — Krinke "critical change") harm-AUC **0.65**, the best signal yet —
  but it needs the diff, so it is a **post-divergence** signal. The best **pre-divergence**
  signal is `cog` (member cognitive complexity) at ~0.61 (≈ prior ceiling). The #23
  axis-B "edit-surface symmetry" hypothesis was wrong (asymmetry AUC 0.44); absolute
  complexity × change locality is the signal. Combos still do not lift (logistic 0.595 on
  51 positives). Revised view: harm is best assessed *after* a divergence (it is a
  property of the realized edit), where #23 reaches ~0.65 — a usable **post-divergence**
  ranker. Pre-divergence ranking still caps ~0.61.
- **BG-gold4 — does the IL obscure cognitive complexity? No (tested).** Worry: cog is a
  surface property, the IL normalizes for equivalence. `il_cog.py` computed cog from
  `nose il --normalized` (If/Loop + nesting + And/Or) vs the source-text proxy on the gold
  (95% IL parse rate): **harm-AUC 0.599 (IL) vs 0.597 (source) — identical.** Control
  structure survives `il --normalized`; only the deeper value-fingerprint collapse
  (loop≡comprehension, = `mean_sem`) erases it, and cog is not computed from that. Flip
  side: a fancier IL-cog will NOT beat the proxy — cog is ~0.60 regardless of
  representation. **Firmly established: the pre-divergence structural harm ceiling is
  ~0.60 across every representation and feature; only `diff_per_cog` (post-divergence,
  0.65) is above it. A strong harm ranker needs the semantic (LLM) layer.**
- **BG-durability.** Labels are git-derived (version-independent); features/families are
  nose-derived (stamped `nose_ver`). Only *detection* changes force a re-mine+re-tune;
  ranking changes (this work) do not. Refresh = `run_corpus.sh` + `tune.py` (minutes,
  cached clones); per-release steps in [hazard-release-checklist](hazard-release-checklist.md).
  Full numbers in [eval/hazard/RESULTS.md](../eval/hazard/RESULTS.md).

## BH. Scan performance — normalize proof lookup, not path exclusions

Profiling real corpora across Rust (`nose-normalize`, `nose-detect`), TypeScript
(`moonlight-server`, `moonlight-web`, `tex`), Python (`episteme2-app`), and Go
(`sah-cli`) showed that semantic/near scans were bottlenecked in the shared
`normalize+extract` path, not in JS-specific parsing or candidate scoring. Large generated
JS bundles can dominate an unscoped scan, but the product fix is not a built-in generated-path
exclusion; benchmark scoping used only the existing `--exclude`/config mechanism.

The hot path was `desugar` repeatedly re-scanning the whole IL to prove receiver-domain
facts for method/property idiom recognition. Replacing that with a shared
receiver-domain cache kept the exact same proof policy while removing repeated O(nodes)
lookups; the cache now lives behind the semantic-kernel facade rather than a normalize-local
side table. Additional behavior-preserving cleanup reserved rebuild arena capacity, avoided
per-node child `Vec` copies in common rebuild loops, reused file-local scope facts in value
fingerprinting, and skipped no-op recursion/dataflow/algebra/cfg-orientation rebuilds.

Representative output JSON was byte-equivalent to `origin/main` after canonical sorting
(`nose-normalize`, `nose-detect`, `moonlight-server`, `tex`, `sah-cli`, and
`craken-cli`; earlier matrix runs also covered `episteme2-app` and scoped
`moonlight-web`). After rebasing onto `origin/main@42545f2`, representative
`NOSE_TIME` deltas for `normalize+extract` were: `nose-normalize` semantic
1452ms→228ms (6.4x), near 1457ms→236ms (6.2x); `tex` semantic 445ms→64ms
(7.0x); `moonlight-server` semantic 48ms→27ms (1.8x); `sah-cli` semantic
12ms→9ms (1.3x). Whole-pipeline speedups are lower where parse/lower now
dominates; this moves the next performance frontier toward lower/parse and remaining
multi-pass normalization overhead rather than file selection policy.

Follow-up profiling split frontend timing into `parse+lower` and `import-resolve`
inside `NOSE_TIME`. The import pass is corpus-level, not JS-specific: sibling literal
exports are modeled through language semantics for Python, JavaScript/TypeScript, Java,
and Rust, while unsupported languages such as Go/C/Ruby do not build the added indexes.
Caching file top-level statements, path-derived module hashes, and binding-use facts
reduced representative `import-resolve` costs without changing output JSON: `tex`
~31–34ms→~4ms, `moonlight-server` ~21–25ms→~6ms, `nose-normalize` ~7–8ms→<1ms,
and `episteme2` ~6–7ms→~2ms; Go corpora stayed at 0ms. Two tempting follow-ups were
rejected after output checks: skipping import resolution for `syntax`-only scans changed
`moonlight-server` syntax families, and caching pure-inline registries changed one
Python near-family. Both are behavior changes, not safe speedups.

## BI. Language profile pass — file roots, not language-specific exclusions

A follow-up language-by-language semantic scan profiled Python, JavaScript, TypeScript,
Go, Rust, Java, C, Ruby, and embedded-script containers on local corpus repos. The goal
was to avoid repeating the earlier JS-specific trap: large bundles can be expensive, but
the product should not learn new built-in file/path exclusions. The safe optimization was
in discovery mechanics instead:

- direct file roots now bypass `ignore`'s directory walker when no `--exclude`/config
  excludes are active;
- directory discovery now checks `Path::extension()` before allocating a path string, so
  unsupported files do not pay string allocation just to be rejected;
- embedded `<script>` tag TypeScript detection uses case-insensitive byte search instead
  of allocating a lowercase copy of the tag;
- semantic extraction skips normalization only for a raw IL that is exactly an empty
  module, preserving top-level block extraction for files that have executable statements.

The representative before/after medians below used `NOSE_TIME=1 nose scan --mode semantic
--top 0 --format json`, five repetitions after the change, and the same corpus inputs as
the baseline run:

| language | files | baseline wall | after wall | result |
|---|---:|---:|---:|---|
| python | 128 | 79.7ms | 79.1ms | stable |
| javascript | 5 | 110.7ms | 114.3ms | stable/noisy; generated-bundle cost remains a scoping issue |
| typescript | 263 | 133.0ms | 126.6ms | small common-path win |
| go | 54 | 53.3ms | 52.4ms | stable |
| rust | 37 | 500.7ms | 464.9ms | small common-path win; output diff was only shifted line numbers in edited Rust files |
| java | 13 | 89.0ms | 4.5ms | file-root discovery fixed the benchmark-shape overhead |
| c | 1241 | 546.7ms | 532.9ms | small common-path win |
| ruby | 1722 | 249.3ms | 220.6ms | small common-path win |
| embedded | 61 | 381.2ms | 26.8ms | file-root discovery fixed the benchmark-shape overhead |

Canonical JSON output was unchanged for Python, JavaScript, TypeScript, Go, Java, C, Ruby,
and embedded. Rust matched after removing line-number fields; the only diff came from this
branch adding lines to a Rust source file included in the profiling input.

## BJ. The design.md §5 recall-ceiling probe — sub-DAG / inlining headroom, measured

[design](design.md) §5 named one decisive measurement that had never been run: *on the
gold set, how many missed worthy pairs would largest-common-pure-sub-DAG matching (and
helper inlining) recover?* §3 gates any further recall-mechanism bet on it. Context that
makes the question sharper: PR #82 already shipped a bounded v1 of **both** mechanisms
(shared heavy anchors at weight ≥ 20 / df ≤ 6 in `near` candidate mode; single-`return`
file-local pure inlining in the value graph), so the probe measures the **residual**
beyond everything reachable today.

**Method** (`bench/labels/recall_ceiling_probe.py`, artifact
`bench/labels/recall_ceiling_probe_2026_06_10.json`): for every worthy v5 label, two
scans — arm0 = the default surface (`syntax,semantic`), arm1 = the maximal current
surface (`syntax,semantic,near --min-value 0`). Labels arm1 misses are classified from
`nose features` dumps of the member files: **subdag-ceiling** if the two members'
tightest covering units share value-fingerprint multiset-intersection mass ≥ 8
(reported also at 12/20; 20 = the shipped `ANCHOR_MIN_WEIGHT`), **inline-ceiling** if
one same-file sibling unit's multiset added to either side lifts the mass over 20,
**same-unit-window** if both members map into one enclosing unit (the statement-window
shape), **no-overlapping-unit** if a member has no unit at all, else **unrecovered**.
Multiset intersection ignores connectivity and single-file `features` lacks whole-repo
import resolution, so the sub-DAG/inline classes **over-approximate** — a ceiling, not a
forecast. Caveats: the original run excluded `rxjs` for a scanner stack overflow later
fixed by #198; corpus was dir-pruned but not file-pruned because `prune_corpus.py` was
missing at the time. Follow-up #200 restored the script and checked-in prune manifest.

**Result** (4,921 worthy labels; dev / heldout):

| | dev | heldout |
|---|---:|---:|
| worthy-recall, arm0 (default) | 86.2% | 88.5% |
| worthy-recall, arm1 (maximal current) | 94.3% | 96.4% |
| arm1-missed | 161 | 74 |
| — subdag-ceiling (mass ≥ 8) | 64 | 35 |
| — inline-ceiling | 11 | 4 |
| — same-unit-window | 19 | 9 |
| — no-overlapping-unit | 29* | 13* |
| — unrecovered (shared mass ≈ 0) | 38 | 13 |

*combined with the residual `other` classes in the per-language table the script prints.

Of the 99 subdag-ceiling labels, only **31** reach the shipped anchor weight (mass ≥ 20;
median mass 14) — i.e. at the weight the product already considers extractable, the
unit-pair sub-DAG residual is **0.6%** absolute worthy-recall; even the optimistic
mass ≥ 8 ceiling is **2.0%**, and the one-step inlining ceiling is **0.3%**.

**Verdict — the §3 gate answers "no-go" for a headline mechanism bet.** The shipped #82
mechanisms plus the `near` channel already recover the bulk (630 default-surface misses
→ 235 maximal-surface misses); what remains is not a unit-pair matching gap:

- the **no-overlapping-unit** cluster is a *unit-extraction* gap with two concrete,
  nameable shapes — Ruby test-DSL blocks (`asciidoctor`, 21 labels) and Rust
  `macro_rules!` bodies (`clap` `macros.rs`, 14 labels) — frontier-evidence material
  (#36 discipline), not matcher work;
- **same-unit-window** (28) is the statement-window fragment axis the coverage taxonomy
  already tracks;
- **unrecovered** (51) shares ~zero value mass — parameterize/extract-helper judgments
  whose similarity is not in the computation at all (the §AV judgment-deep shape).

The one cheap knob left on the table: the 8–20 mass band (68 labels) would respond to a
lower anchor weight floor, but those small shared chunks are weak refactor targets and
the band is an over-approximation — worth at most a knob experiment
(`NOSE_ANCHOR_*`), not a mechanism. The honest headline for further worthy-recall is
**unit extraction coverage and the fragment axis, not more matching**.

## BK. The independent miss-mining arm — measuring in-the-wild misses (modality B)

The §BJ probe answered the *mechanism* question; this answers the *measurement* one
(#194): the v5 pool is nose ∪ jscpd, so semantic clones missed by **both** can't appear
in any recall denominator. `bench/type4/miss_mining.py` is the independent arm: per
pinned repo, LSH-band the detection minhash over all ≥ 5-line/≥ 40-token units, confirm
candidate pairs by exact value-multiset Jaccard (≥ 0.80), and keep pairs **no family on
the maximal current surface co-reports**. Output is a queue signal only (#36 two-layer
discipline): every record carries `evidence_tier: detector-suggested`, annotated with
`fp_equal`, `exact_safe`, and a source `text_similarity` ratio so the
textually-dissimilar tail (the jscpd-shaped blind spot) is sliceable. The complementary
modality A — behavior-equal fingerprint-split pairs — is the existing
`nose verify --leads`. Artifact: `bench/type4/miss_mining_2026_06_10.json`
(104 repos; original run excluded `rxjs` for the stack overflow later fixed by #198).

**Result: 593 candidates corpus-wide, and the audit says the residual is structured,
small, and mostly *not* detector-recall.**

| class | n | read |
|---|---:|---|
| fp-equal, same-file, not exact-safe | 375 | parameterized-test / scaffolding bodies, annotation-varying twins |
| fp-equal, cross-file | 78 | dominated by generated code (`etcd` protobuf `*.pb.go`) — correctly excluded from ranking by the generated-file policy |
| fp-equal, same-file, exact-safe | 44 | small proven twins, mostly test scaffolds |
| vj ∈ [0.8, 1.0), mixed | 96 | the only class with worthy-shaped finds |

Spot-audit of the non-fp-equal cross-file slice found one clearly worthy-shaped miss —
`libgdx` `Widget.pack()` ≡ `WidgetGroup.pack()` (identical method duplicated across
sibling classes; impure, vj 0.80 < the 0.90 value-accept, shapes split, so no channel
fires) — among stub/parallel-by-design neighbours (curl no-op callbacks, sympy protocol
type-defs). The deeper catch was mechanical: chasing *why* fp-equal `exact_safe` pairs
(`junit5` annotation-varying `fail(...)` methods) never surface exposed that **adding
the syntax channel can drop an exact semantic family the semantic channel reports
alone** — a channel-merge reporting bug, filed with a single-file reproducer as #202.
That is the arm working as designed: candidates are cheap, and each audited one either
dies as scaffolding/generated (raising confidence in the policy layers) or names a
precise defect.

Honest bottom line for the in-the-wild recall question: at vj ≥ 0.8 the unreported mass
is ~600 pairs across 59k files, overwhelmingly generated/scaffolding; the worthy-shaped
tail is a handful per corpus — consistent with §BJ's "no headline recall mechanism
left" verdict. Below vj 0.8 (true different-algorithm Type-4) this arm is blind by
construction; that frontier remains unmeasured and would need a behavior- or
embedding-based candidate source — recorded as the arm's known limit, not claimed
covered.
