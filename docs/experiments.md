# nose — experiment log

*Part of the [home](home.md) wiki. The methodology and headline numbers are summarized
in [benchmark](benchmark.md); the passes these experiments shaped are in [normalization](normalization.md)
and [architecture](architecture.md).*

A consistent record of what we tried and what happened. The current user-facing
`nose scan` command has three channels (`syntax`, `semantic`, `near`), described in
[usage](usage.md). Internally, all channels share the same lower → normalize → feature
pipeline where appropriate, with exact semantic matches coming from the value graph.

> **Historical record.** This log spans the whole project, including a pre-v5 era whose
> measurement code (many `bench/*.py` scripts — `bench.py`, `synth.py`, `value_add.py`,
> `clone_bench.py`, the `judge/` pipeline, …) and gold sets (`typed4`,
> `semantic_duplicates`, labelsets v1–v4, …) were later pruned to keep the repo lean. Those
> names appear below as the reproduction record of the time; the files live in git history.
> Older sections also mention removed scan spellings such as `--mode behavior` and
> `--no-contiguous`; use [usage](usage.md) for the current CLI.
> The **current** benchmark is the v5 refactoring-family labelset evaluated by
> `bench/labels/eval_by_language.py` — see [benchmark](benchmark.md) (§AU onward).

## Measurement methodology

- **Gold:** 327 audited duplicate pairs (Python/JS/TS), from
  `bench/goldens/semantic_duplicates.v2.json`, with a
  **dev (8 repos) / held-out (8 repos)** split as a generalization gate.
- **Metric:** line-span IoU partial credit, pair-order-invariant `pair_overlap`,
  **max-weight bipartite matching**, per-slice precision/recall/F1, **repo-macro
  F1** (dev vs held-out), and **hard-negative FP-rate** (39 confusable non-clones).
- **Noise floor:** ≈ **±0.019** macro F1 (per the benchmark bootstrap). Deltas
  below this are NOT accepted as improvements.
- **Headline number:** `macro[all]` F1 on dev and held-out.
- Reproduce: `nose detect <repos> --repos-root <repos> --dump /tmp/d`, then
  `nose eval --gold … --predictions /tmp/d/predictions.json --hard-negatives … --corpus …`
  and `nose ceiling --gold … --units /tmp/d/units.json --candidates /tmp/d/candidates.json`.

## Reference points

| | dev macro-F1 | held-out macro-F1 |
|---|---|---|
| prior token-based baseline (160 experiments) | ~0.034 | ~0.028 |
| **nose default at the time of this measurement** | **0.040** | **0.038** |

---

## A. Adopted (in the main pipeline)

Validated by **equivalence fixtures** (`crates/nose-cli/tests/equivalence.rs`,
26 tests): known-equivalent snippet pairs must produce identical normalized IL /
value-graph fingerprint, and a genuinely different pair must not.

| capability | what it converges | status |
|---|---|---|
| tree-sitter frontends (Go/TS/JS/Python) → IL | — | adopted |
| coverage hardening | Raw-node ratio **7.37% → 0.007%** on 3.6k files | adopted |
| alpha-renaming | identifier names (alpha-equivalence) | adopted |
| loop unification (C-for/while/foreach) | `for(;;)` ≡ `while` ≡ `range` | adopted |
| idiom canonicalization | `len`/`.length`, `print`/`console.log`/`fmt.Println`, … | adopted |
| HoF unification | Python comprehension ≡ JS `.map`/`.filter` | adopted |
| template↔concat | `` `a${x}` `` ≡ `"a"+x` | adopted |
| dataflow copy/expr propagation | `t=a+b; …t…` ≡ inlined | adopted |
| **value-graph / GVN** | temporaries, statement order, CSE | adopted (detection substrate) |
| algebra (assoc/comm flatten, comparison-direction, De Morgan) | `(a+b)+c`≡`a+(b+c)`, `a>b`≡`b<a`, `!(a&&b)`≡`!a‖!b` | adopted |
| CFG normalization (conjoined-guard, continue-guard, branch-orient) | `if a:if b` ≡ `if a&&b`; `if c:continue;S` ≡ `if !c:S` | adopted |
| **LSH k=128 / bands=32** | better candidate recall (cand-reachable 27→30%) | **adopted** (clean win) |
| **determinism fix** (`Interner::symbol_hash`) | stable hashes across runs | adopted (critical) |

### The determinism fix (important)
Detection was nondeterministic (5099/5051/5066 preds across identical runs)
because `ThreadedRodeo` assigns symbol ids in thread-race order under parallel
lowering, and those ids fed structural hashes. Fixed by hashing symbol **string
content** (`symbol_hash`, FNV-1a) instead of the id. Without this, every F1
delta was partly noise — so this silently underpinned all later measurement.

---

## B. Measured experiments on the gold set

| experiment | result | decision |
|---|---|---|
| baseline (value+shape candidate, k=64/b=16) | dev 0.0354 / held 0.0308 | reference |
| **LSH k=128 / b=32** | cand-reachable 27→30%, dev/held ↑ | **adopted (default)** |
| LSH b=64 (rows=2) | 10.7M candidate pairs (blowup), no gain | rejected |
| lower acceptance threshold (0.7→0.1) | recall 0.16→0.29 but **F1 collapses** (precision dies) | rejected |
| **multi-granularity block units** (loops/ifs/try) | reachability 43.7→**52.6%**, but cand-reachable flat (~30%), F1 within-noise/down, HN-FP doubled | **opt-in `--blocks`** (default off) |
| **coarse atom candidate channel** (operators/API bag) | cand-reachable 27→**35.5%**, but 6× candidates, F1 flat | rejected (alone) |
| atom term added to scoring | F1 **down**, HN-FP 0.077→0.103 | **rejected, reverted** |
| **dead-code elimination** (DCE) | within-noise, slightly negative (−0.0005 held) | **opt-in `--dce`** (default off) |
| **algebraic identity folding** (`x+0`→`x`) | **byte-identical to baseline (zero effect)** | **rejected, reverted** |

---

## C. Rejected idea *families* (and the lesson)

1. **Threshold/parameter tuning** → precision collapses or within-noise. (Same
   conclusion reached by the prior token-based baseline after 160 experiments.)
2. **Coarse bag-of-operations features** (candidate gen or scoring) → divergent
   clones become *surfaceable* but **not separable** from coincidental
   operator-overlap; FP rises ≥1:1 with recall.
3. **Cleanup-style normalization** (dead-code, algebraic identity folding) →
   real-world clones essentially never differ in these ways; **zero to
   within-noise effect.** (This is the measured verdict that full
   **egg / equality-saturation is not worth building** for this goal.)

**Net:** the easy/obvious levers are exhausted. Two whole families ruled out by
measurement.

---

## D. Current bottleneck (recall funnel, measured)

```
gold (327) → unit-reachable 43.7% → candidate-reachable 27.2% → detected
```
- **~56% lost at unit extraction**: a gold region isn't an extracted unit
  (it's a sub-function block, or a span that doesn't align to a unit boundary).
- **~17% more lost at candidate generation**: LSH doesn't pair the two units
  (their fingerprints don't overlap enough).
- **Remaining loss at scoring**: surfaced divergent pairs score below threshold
  and can't be accepted without FP.

**Frontier:** genuine algorithmic / structural divergence — same behavior via a
different algorithm or data structure. The value graphs and structures
*legitimately* differ; this is the (undecidable) Type-4 core.

---

## E. Cross-disciplinary candidate pipeline

Ideas borrowed from other fields (ML, math, bioinformatics, linguistics, network
science). Each promising candidate gets its own git branch, an implementation, a
measured evaluation vs the noise floor, and is merged to `main` only if it clears
the floor without a precision regression.

### E.1 Idea harvest (5 fields, parallel subagents)

Strong cross-field convergence. Recurring discipline from every field: fit corpus
stats on **train-only & freeze for held-out**; prefer **intermediate metrics**
(unit-reachability, candidate-reachability, retrieval recall@k) over noisy macro-F1
at n=327; **gate soft matches by type/arity** (the synonym/antonym hazard —
`push`/`pop`, `+`/`-` keep the same company); require each idea to clear ~2× the
±0.019 floor as a *marginal* (ablation) contribution.

### E.2 Curated candidate shortlist (ranked promise × feasibility)

**Tier 1 — cheap, high-confidence, near-linear, unsupervised, mostly drop-in**
1. **WL (Weisfeiler–Lehman) multi-round labels → MinHash** *(math, network, cheminformatics-adjacent)*. Replace the 1-hop node hash with the union of WL labels over rounds 0..K. Round 0 = today's exact features (precision floor preserved); rounds 1..K add contextual partial-overlap divergent clones share. **The standout cheap win.**
2. **Smith–Waterman local alignment + corpus-estimated substitution matrix** *(bioinformatics)*. Drop-in replacement for LCS scoring: substitution scores (near-equivalent ops score >0) + affine gaps + local. Biggest single scoring lever.
3. **Structural-topology fingerprint as a 2nd candidate channel** *(network science)*: NetSimile (cheapest) + typed graphlet/orbit (GDD) + RolX role histogram → cosine/ANN-LSH, **unioned** with MinHash candidates. Rewiring-robust recall.
4. **Distributional op/API embedding → synonym cluster-IDs into MinHash + soft-Jaccard scoring** *(linguistics, ML)*. PPMI+SVD over corpus co-occurrence; cluster synonymous APIs (`json.Marshal`≈`JSON.stringify`) → exact collide. Attacks synonymous-call gap at gen + score.
5. **Conservative graph coarsening before hashing** *(network)*: n-ary associative/commutative collapse, linear-chain collapse, CSE canonicalization → current pipeline starts pairing reassociated/CSE-divergent clones.
6. **Weighted/IDF MinHash (ICWS) + b-bit** *(math)*: down-weight boilerplate labels, collide on discriminative substructure.

**Tier 2 — medium effort, promising**
7. **Seed-and-extend (BLAST-style) candidate gen** *(bio)*: shared k-mers / shape-hashes as cheap seeds → SW extension; local conservation survives divergence (attacks 30% candidate ceiling).
8. **Linkage / phylogenetic-tree clustering instead of union-find** *(bio)*: decouple similarity from clustering; re-test threshold-lowering without transitive contamination.
9. **Gromov–Wasserstein / Graph-Matching-Network re-ranker** *(math, ML)*: per-candidate cross-graph alignment score; accept divergent pairs without FP flood.
10. **Pivot/landmark embedding of (approx) graph-edit-distance → LSH** *(math)*: make the "right" edit metric LSH-able.

**Tier 3 — high-difficulty, high-ceiling "big bets"**
11. **Self-supervised contrastive GNN embedding** *(ML)*: encoder = GIN/graph-transformer on the value graph; positives = our own normalization + IL-rewrite augmentations (incl. loop↔fold, recursion↔iteration); MoCo negatives. Then **neural/embedding-LSH** (unioned) + **2-scalar calibration** on gold (eval-only). Fixes both funnel losses; needs the synthetic-paraphrase generator.
12. **Behavioral / micro-execution fingerprint** *(ML)*: on pure functions, fingerprint by input→output behavior — the literal Type-4 definition; near-perfect-precision accept-booster on the executable subset.

### E.3 Branch protocol
Per candidate: `git checkout -b exp/<name>` off `main` → implement (flag-gated) →
`nose detect --dump` + `nose eval`/`nose ceiling` → record result row below →
merge to `main` only if it clears the floor without precision regression; else keep
the branch + record the negative result.

### E.4 Additional-fields sweep (one concrete technique per field, sequential branches)

| # | field | concrete technique | branch | role |
|---|---|---|---|---|
| 1 | cheminformatics | **ECFP/Morgan = multi-radius WL labels** on the value graph (incl. upward "used-by" context), unioned into the value fingerprint | `exp/f1-morgan` | fingerprint (gen+score) |
| 2 | compiler/PL & formal methods | **PDG**: add control-dependence edges to the value graph, then fingerprint | `exp/f2-pdg` | fingerprint |
| 3 | music IR | **Shazam landmark hashing**: anchor-pair + Δoffset combinatorial hashes | `exp/f3-landmark` | candidate channel |
| 4 | entity resolution | **multi-key blocking / canopy** (sorted-neighborhood, top-IDF keys) | `exp/f4-blocking` | candidate channel |
| 5 | crypto / fuzzy hashing | **TLSH-style** bucketed-histogram fuzzy digest + distance | `exp/f5-tlsh` | candidate + score |
| 6 | information retrieval | **BM25/TF-IDF inverted index** retrieve top-k → rerank | `exp/f6-bm25` | candidate channel |
| 7 | computer vision | **RANSAC geometric verification** of matched anchors (consensus alignment) | `exp/f7-ransac` | re-ranker |
| 8 | signal processing | **DTW** over linearized token sequence | `exp/f8-dtw` | score channel |
| 9 | cognitive science | **structure-mapping (SME)**: reward connected-subgraph alignment | `exp/f9-sme` | re-ranker |

Each: branch off `main`, implement, `detect --dump`→`eval`/`ceiling`, record the row
in E.5, merge iff clears ~2×floor without precision regression else drop.

### E.5 Results

Baseline (main): dev 0.0399 / held-out 0.0378, HN-FP 0.103, cand-reach 27.2%.

| branch | dev ΔF1 | held ΔF1 | HN-FP | decision |
|---|---|---|---|---|
| `exp/f1-morgan` (WL/ECFP multi-radius labels) | −0.0065 | +0.0040 | 0.051 | **DROP** — within floor & disagree; upward "used-by" context broke value-graph CSE/order invariants (2 fixtures). |
| `exp/f2-pdg` (control-dependence folded into sinks) | −0.0003 | +0.0030 | 0.103 | **DROP** — within-noise/neutral; invariants preserved (26 tests ok) but no gain. |
| `exp/f3-landmark` (Shazam anchor-pair candidate channel) | +0.0014 | 0.000 | 0.103 | **DROP** — within-noise; candidate-widening alone doesn't move F1. |
| `exp/f4-blocking` (sorted-neighborhood candidate channel) | −0.0009 | +0.0002 | 0.103 | **DROP** — within-noise. |
| `exp/f5-tlsh` (SimHash cosine-LSH candidate channel) | −0.0017 | −0.0016 | 0.103 | **DROP** — within-noise. |
| `exp/f6-bm25` (IDF inverted-index blocking) | −0.0020 | +0.0010 | 0.103 | **DROP** — within-noise. |
| `exp/f7-ransac` (RANSAC consensus-offset alignment, replaces LCS) | −0.0011 | **+0.0111** | **0.077** | ✅ **MERGE** — best signal: held-out +0.011 AND precision↑ (HN-FP 0.103→0.077), simpler/faster scorer, no regression (26 tests ok). |
| `exp/f8-dtw` (DTW alignment, vs RANSAC) | −0.0002 | −0.0108 | 0.077 | **DROP** — DTW less selective than RANSAC; held-out worse. |
| `exp/f9-sme` (SME size-weighted value multiset) | +0.0011 | −0.0003 | 0.077 | **DROP** — within-noise/neutral. |

### E.6 Sweep verdict (9 additional fields)

**1 merge / 8 drops.** The single win — **RANSAC consensus-offset alignment** (computer
vision, field 7) replacing LCS — lifted held-out macro-F1 **0.0378 → 0.0489** and
cut hard-negative FP **0.103 → 0.077** with a simpler scorer. Two clear lessons,
both now measured across many variants:
- **Candidate-generation widening is a dead family** (fields 3,4,5,6 + the earlier
  atom channel — 5 distinct LSH/blocking/sketch variants, all within-noise):
  divergent pairs become *surfaceable* but the precise scorer still rejects them,
  so F1 doesn't move. Recall is gated by scoring, not generation.
- **The alignment metric matters, and *selectivity* is what helps.** Of the three
  alignment swaps (LCS→RANSAC→DTW), the more *selective* one (RANSAC: requires a
  consistent translation consensus) generalized best; the more *lenient* one (DTW)
  hurt. Adding context/coarse features (f1 WL upward-context, f2 PDG, coarse atoms)
  is within-noise or breaks invariants.

Net main after the sweep: **dev 0.039 / held-out 0.049, HN-FP 0.077** (was
0.040/0.038, 0.103). Held-out + precision both improved.


## F. Fixing non-exhaustive gold — LLM-as-judge pooling (`bench/judge/`)

Gold is a non-exhaustive sample → naive precision (~1%) is an artifact (most
out-of-gold predictions may be real clones). Standard fix: **pooling + an oracle**.
Oracle = LLM-as-judge (batched subagents), gated by calibration. `prep.py` builds
blind judging batches (source snippets, labels withheld); `score.py` computes:

**1. Judge calibration (the gate).** On 40 gold positives + 39 hard-negatives + 71
random unrelated functions:
- gold positives confirmed: **8/40 = 20%** (judge recall — LOW)
- hard-neg confirmed: 1/39 = 2.6%; random: 0/71 = 0% → **judge precision 89%**
- **Finding:** the strict-behavioral judge agrees with only 20% of the human gold →
  **the gold v2 and a strict Type-4 judge use different "clone" definitions.** The
  gold conflates behavioral Type-4 with `production_structural`/`mirror`/`async_sync`
  (structural/boilerplate similarity). High judge precision means its *positives* are
  trustworthy; low recall means it can't (yet) grow gold completely. (Confound:
  45-line snippet truncation may suppress some equivalence.)

**2. Real precision of out-of-gold predictions.** Judged sample of 150:
**6.0% TRUE clones (95% CI 3.2–11.0%)**, vs the naive ~1%. Recall-correcting for the
judge's 20% recall → **true precision ≈ 30%**. Non-exhaustiveness was hiding that
out-of-gold predictions are real clones at many× the naive rate.

**3. Silver tier (pooling).** 9 judge-confirmed out-of-gold clones → `silver_llm.v1.json`
(high-confidence; e.g. radash `inRange` JS↔TS cross-language, boltons dup `__init__`).
Eval can report on gold∪silver (pooling assumption) for corpus-wide precision.

**Decision surfaced:** what is *our* clone target? (a) strict behavioral Type-4 →
gold v2 is partly mistargeted (build a Type-4-pure gold; strict judge already
well-calibrated, improve recall by dropping truncation + prompt tuning); (b) the
gold's broad "duplicate" → re-prompt the judge broader and re-calibrate until gold
recall is high with hard-neg FP ~0. Either path uses this same harness.

### F.1 Both clone definitions measured (strict vs broad judge, same batches)

| judge | gold recall | hard-neg FP | rand FP | judge precision | out-of-gold real precision |
|---|---|---|---|---|---|
| **strict** (behavioral Type-4) | 20% | **3%** | 0% | **89%** | 6% (CI 3–11%); recall-corr. ~30% |
| **broad** (gold's "duplicate") | 92% | **95%** | 1% | 49% | 100% (vacuous) |

**Decisive finding:** the broad judge matches gold v2 (92% recall) **but flags 95% of
the hard-negatives** (curated confusable NON-duplicates) as duplicates → **the gold's
broad "duplicate" definition is not separable from its own hard-negatives; it is an
ill-posed target** (its "100% precision" is vacuous — yes to everything structurally
similar). The **strict behavioral-Type-4** definition is crisp and separable (hard-neg
FP 3%, rand 0%, judge precision 89%) — but only ~20% of gold v2 meets it.

**Conclusions:**
1. **Non-exhaustiveness quantified:** under the crisp strict definition, real
   out-of-gold precision ≈ **6% (lower bound) / ~30% (recall-corrected)** vs the naive
   ~1% artifact. Problem solved/measured.
2. **gold v2 is the wrong benchmark for strict Type-4** (mostly structural/mirror
   dupes, boundary not separable). The **strict judge (89% precision) can build a
   Type-4-pure gold / grow `silver_strict`** — that is the path to a well-posed,
   exhaustive-enough benchmark for nose's actual goal.
3. Per-definition silver tiers written: `silver_strict.v1.json` (9, high-trust),
   `silver_broad.v1.json` (150, low-trust — broad judge unreliable per its 95% hard-neg FP).

## G. Type-4-PURE benchmark + a benchmark-overturning finding

Built `typed4.v1.json` by strict-judging the whole pool (gold v2 327 + 252 out-of-gold
predictions). Composition:
- **gold v2: only 51/323 (16%) confirmed as behavioral Type-4.** Of the gold's 166
  `production_type4`-labeled pairs, the strict judge confirmed just **11**.
- new positives from predictions: 14/248 (real out-of-gold precision 5.6%).
- **Type-4-pure positives total: 65** (pool 248 → defines a real precision denominator).

**Disambiguation (truncation ruled out):** re-judged 30 of the disputed
gold-`type4`-but-rejected pairs with FULL untruncated snippets + careful reasoning →
still only **1/30 = 3%** confirmed. The rejection rationales are concrete and
self-evidently correct: "max uses `>`, min uses `<` — opposite results", "OneOf raises
if not-in; NoneOf raises if in — opposite logic", "parseFloat vs parseInt", "startOfDay
vs startOfHour — different granularity", "remove pops a key; set assigns a value".

**FINDING (overturns the benchmark): gold v2's `production_type4` label is ~95%
mislabeled for *behavioral* Type-4.** Those pairs are sibling/analogous functions with
the same skeleton but DIFFERENT behavior — several are precisely what a good Type-4
detector should *reject*, yet the gold lists them as positives. This explains a lot:
why nearly every experiment was "within-noise" (we were partly optimizing toward a
target full of non-clones), and why the broad judge matched the gold (gold ≈
"structurally similar", which includes these siblings).

**Consequences:**
1. **`typed4.v1.json` (65 strict positives + judged pool), not gold v2, is the right
   target** for nose's actual goal. nose should be *rewarded* for rejecting the
   gold's mislabeled sibling-pairs.
2. The strict LLM judge is validated as a reliable oracle here (verifiable rationales;
   89% calibration precision). Human spot-check of ~15 disputed pairs is the cheap
   final confirmation (the rationales — max/min, parseFloat/parseInt — are trivially
   checkable), but confidence is already high.
3. All prior F1 numbers vs gold v2 should be read with this caveat: they partly
   measure agreement with a mislabeled set.

Artifacts: `bench/judge/{prep,score,prep_typed4,build_typed4,prep_dispute}.py`,
`bench/goldens/{typed4.v1.json, silver_strict.v1.json, silver_broad.v1.json}`.

## H. Judge reliability hardening — 3-persona panel (no human spot-check)

160 pairs (40 gold-pos, 39 hard-neg, 35 random, 46 disputed gold-`type4`) judged by a
3-persona panel with FULL snippets: **prover** (argue equivalence), **refuter** (find a
distinguishing input), **neutral** (senior engineer). Consensus = majority of 3.

| set | prover | refuter | neutral | **consensus** |
|---|---|---|---|---|
| gold positives | 16% | 3% | 27% | **14%** |
| hard negatives | 0% | 0% | 0% | **0%** |
| random unrelated | 0% | 0% | 0% | **0%** |
| disputed gold-`type4` | 2% | 0% | 4% | **0%** |

- **Inter-judge agreement: 91% unanimous** (Fleiss κ=0.32 — "fair", but deflated by the
  prevalence paradox: when ~all pairs are non-clones, κ understates the strong raw
  agreement). The personas agree.
- **Negatives: 0% FP across all personas + consensus** — the judge reliably rejects
  hard-negatives and random pairs. High precision confirmed, robustly.
- **'gold type4 mislabeled' is ROBUST: 0% panel-consensus** confirmation of disputed
  pairs (even the lenient prover persona: 2%). Not a single-judge artifact.

**Honest residual:** the panel proves the judge is *precise* (0% FP) and the
mislabel finding robust, but **judge RECALL on true behavioral Type-4 is still
unmeasured** — we lack a trusted positive set (gold positives are mostly not Type-4;
human spot-check excluded by request). The prover↔refuter spread on gold-positives
(16% vs 3%) is exactly this residual recall uncertainty. **Next (judge-independent):
synthetic provably-equivalent positives** (loop↔reduce, recursion↔iteration via IL
rewrites) to measure judge recall without humans or the distrusted gold.

Artifacts: `bench/judge/{prep_panel,score_panel}.py`, `verdict_panel_{P,R,N}_*.jsonl`.

## I. Judge RECALL via synthetic ground truth (no human) + detector floor test

18 hand-authored **provably-equivalent** pairs (loop↔reduce↔recursion↔while, comprehension↔
loop, recursive↔iterative, builtin↔manual; intra- and cross-language py/js/ts/go) — known
Type-4 by construction. Judged by the same 3-persona panel. `bench/judge/prep_syn.py`.

**Judge recall (the previously-unmeasured axis):**

| | prover | refuter | neutral | **consensus (maj≥2)** |
|---|---|---|---|---|
| confirmed equivalent | 100% | 94% | 94% | **100%** (16/18 unanimous) |

The only 2 dissents are *correct* edge-case catches, not judge failures:
`contains: loop vs in` (Python `in` does identity-before-equality → differs on NaN);
`max: reduce-no-init` (throws on empty array vs loop returns undefined). The judges
distinguish behavior on degenerate inputs — exactly the precision we want.

**→ Judge is now validated on BOTH axes, judge-independently:**
- precision: **0% FP** on hard-neg + random (panel, §H)
- recall: **100% consensus** on ground-truth Type-4 (here)
- ⇒ the judge is a trustworthy oracle; `typed4.v1.json` (judge-built) rests on a
  validated oracle, and the "gold type4 mislabeled" overturn stands on solid ground.

**Detector floor test (same pairs, `synthetic_gold.json`, full equivalence-class closure):**
forced below the size gate (`--min-tokens 8`, real default 24), nose scores
**13% cluster recall** AND **60% cross-family false-merges** — tiny textbook functions
collapse to a generic "loop-accumulate-over-sequence" shape, so distinct tasks (sum/count/
factorial/square) both under-converge within-task and over-merge across-task. This is the
sub-gate regime nose's `min_tokens=24` default deliberately excludes; the honest takeaway
is a **tiny-function blind spot** in both directions. A fair detector-recall test needs
larger equivalent functions (follow-up); the judge-recall result above is the primary win.

## J. Foundation: validated re-baseline + measurement stack

After validating the judge (§H–I), rebuilt the evaluation foundation on the *correct*
target and froze a canonical baseline. New infrastructure:

- **pool-aware precision** (`nose-eval`): consumes `typed4.pool` (judged predictions) →
  honest precision denominator. Naive tp/preds (≈0.011) is meaningless on a non-exhaustive
  gold; pool precision is the number of record.
- **`bench/analyze.py`**: threshold-free AUC-PR over the judged pool, recall@k, and
  paired-bootstrap 95% CIs — incl. Δ between two runs (significance, not eyeballing).
- **`bench/synth.py`**: diagnostic suite v2 — larger functions tagged by transformation
  (loop↔reduce, recursion↔iteration, while↔for, idiom, cfg-reshape, reorder) + near-miss
  NEGATIVES. Reports per-transformation recall + discrimination (negative-FP).
- **`bench/bench.py`**: one command = build→detect→eval→analyze→synth→ablation→determinism
  →perf, writes `bench/baseline.json`, diffs reruns. See `bench/BASELINE.md`.

**Canonical baseline (default config, validated typed4.v1):**
type4 recall **0.589** [0.465,0.707] · pool precision **0.059** [0.033,0.089] ·
AUC-PR **0.23** · HN-FP 0.077 · ~5,600 files/sec · deterministic.

**Findings that set the roadmap:**
1. Gap is **precision, not recall**. AUC-PR 0.23 ≫ raw 0.06 ⇒ score ranks true clones
   well; **ranking/threshold/calibration** is the top precision lever, not candidate-gen
   (already proven a dead family in §funnel).
2. nose is a **structural (Type-2/3) matcher, not Type-4**: ~6% transformation recall,
   4/8 near-miss negatives merged. Two failure modes — extraction (compact functional
   forms below the size gate: 16/33 synth fns survive) and scoring (structural, not
   semantic). This is the frontier the IL must cross.
3. Normalization passes are **at-noise on the validated target**: `--no-cfg-norm` Δrecall
   0.0, `--dce` Δrecall 0.0. Real gains need semantic convergence (value-graph/GVN for
   loop↔reduce, recursion↔iteration; idiom↔explicit) — now measurable via synth.py.

## K. Semantic convergence + precision (targeting #1 ranking & #2 Type-4)

Error analysis on the *validated* target (26/65 missed) showed the real Type-4 gap is
**async↔sync twins** (httpx `__exit__`/`__aexit__`, `read`/`aread`; execa/marked/zod),
not loop↔reduce (which barely occurs in real code). Retargeted accordingly.

**MERGED (each measured, principled, net-positive-or-neutral, deterministic):**
- **async→sync name canonicalization** (`idioms::async_to_sync`, applied in desugar):
  `__aexit__`→`__exit__`, `aread`→`read`, `AsyncIterable`→`Iterable`, … (curated, high
  confidence). `await` was already stripped, so these *names* were the last divergence.
  → recall **0.589→0.604**, Δrecall 95% CI [+0.000,+0.045] (never negative), 0 precision cost.
- **small-int literal retention** (`-2..=2` kept as `LitInt`; value-graph keys by value,
  structural tag stays abstract): for a *behavioral* detector, `0`≠`1` — abstracting
  behavior-defining constants is a Type-2 heuristic that costs Type-4 precision. Un-merges
  near-misses (`x%2==0` vs `==1`). → pool-precision **0.0595→0.0643**, AUC-PR **0.23→0.263**,
  predictions **3575→3344** (−231 false merges), recall + HN-FP unchanged.

**TRIED & REJECTED (kept off main; evidence recorded):**
- **Reduction-aware value-graph** (recognize accumulator loops + `reduce(λ)` as a canonical
  `Reduce[iterable, combiner]` so loop↔reduce converge): makes vj converge in isolation
  (loop-sum vs reduce-sum 0.857), but **inert on the real target** — A/B with the recognizer
  on/off gave identical metrics (3343 vs 3344 preds). Loop↔reduce barely occurs in real
  Type-4, and the blended score gates the converged vj out anyway.
- **Semantic floor** `score = max(blend, vj)` (trust a strong value-graph match regardless
  of structure): **catastrophic** — predictions 3578→**66,665** (18×), macro-F1 0.063→0.015,
  AUC-PR 0.23→0.174. `vj≥0.70` alone is far too permissive: low-entropy value graphs of
  small functions collide. **Key lesson:** the value-graph multiset-Jaccard is *not* precise
  enough to be a standalone acceptance criterion. True cross-structure Type-4 (loop↔reduce
  scores only ~0.43 in the blend) needs **precise semantic-key matching** (index units by a
  canonical reduction/effect signature and match exactly), not a fuzzy similarity + floor —
  that is the path for the next phase.

## L. Recall is extraction-bound: arrow-function units (JS/TS)

Funnel decomposition of the 25 missed validated positives (via `--dump`): **19 blocked at
EXTRACTION** (an endpoint isn't even a unit), 4 at candidate-gen, only 2 at scoring — so the
binding recall constraint is *extraction*, not the scorer (semantic-key matching would have
addressed ~2). Cause: **13 endpoints sit in files with ZERO units** — all JS/TS — because the
frontend only tagged `function_declaration`/`method_definition` as units, while modern JS/TS
defines functions as `export const f = (…) => {…}` (arrow/function-expression bound to a name).
Those lowered to inline `Lambda` *expressions*, never units.

**Fix (correctness):** `const f = <arrow|function-expr>` now lowers to a named `Func` unit
(`lower_func_value`), while inline callback arrows stay `Lambda`. → previously-empty files now
yield units (e.g. execa run-sync.js 0→6); unit-reachable 39→42; **AUC-PR 0.263→0.337**;
recall 0.604→0.610; pool-precision 0.064 and HN-FP 0.077 unchanged; 26 fixtures pass.
Predictions 3344→4841 and macro-F1 0.063→0.047 — both raw-count artifacts of correctly
analyzing far more JS/TS code (the honest precision metrics, pool-precision + HN-FP, held).

**Remaining recall is still extraction-bound (16/25):** units that don't align to the gold
fragment span, plus other unparsed patterns — pointing the next phase at unit-boundary /
sub-unit (block) extraction rather than scoring.

## M. Sub-unit (block) extraction — default ON

The funnel (§L) left recall extraction-bound: gold/pool clones are often sub-function
**fragments**, undetectable when only whole functions are units. The `--blocks` path
(extract substantial `Loop`/`If`/`Try` nodes as units) was previously off ("FP-prone") —
but that verdict predated the validated target + honest pool-precision. Re-measured:

| | recall | pool-precision | AUC-PR | HN-FP | preds |
|---|---|---|---|---|---|
| functions only | 0.610 | 0.064 [0.036,0.096] | 0.337 | 0.077 | 4841 |
| **+ blocks (default)** | **0.621** | **0.106 [0.072,0.141]** | **0.419** | 0.077 | 10373 |

Every *honest* metric improves — pool-precision nearly doubles (block predictions align to
the fragment spans the gold/pool actually mark, hitting more true clones: 37/348 vs 16/249),
AUC-PR +0.082, recall +0.011, HN-FP flat, deterministic, 26 fixtures pass. Made **default ON**
(`--no-blocks` to revert). A *stricter* block gate (40 tok/6 ln) was tried and **rejected**:
it dropped AUC 0.42→0.17 and pool-precision 0.106→0.074 — the real sub-function clones are
small (24–40 tok), so over-gating removes signal faster than noise; blocks share the function
gate. Cost: raw prediction count and macro-F1 (raw precision) — both raw-count artifacts of
analyzing more regions; the honest metrics (pool-precision, HN-FP, AUC) all held or rose.

Session arc (main→now): recall **0.589→0.621**, pool-precision **0.0595→0.106**,
AUC-PR **0.23→0.419**, HN-FP 0.077 throughout. Next lever (funnel): the ranking/threshold
exploit of the now-strong AUC (0.42) to convert the 10k raw predictions into a high-precision
top set.

## N. Ranking/threshold for precision — IDF re-ranking REJECTED

With recall extraction-gains banked (§L–M) and AUC-PR 0.42 indicating the score ranks
true clones above false, the plan was to convert ranking into precision.

**Threshold (a knob, weak lever):** sweeping the acceptance threshold trades recall for
pool-precision along a shallow curve — 0.70→0.621 rec / 0.106 P; 0.86→0.547 / 0.154;
0.94→0.522 / 0.165. Precision tops out ~0.16 with heavy recall loss. Left default 0.70
(recall-oriented); the curve is documented for callers who want a higher-precision point.

**IDF feature weighting (the real attempt) — REJECTED.** Hypothesis: down-weight features
shared by many units (boilerplate loops/ifs/`return x`) and up-weight rare/specific ones,
so a match on a distinctive computation outranks a boilerplate match (BM25/IR idea).
Implemented as `idf(f)=ln(N/df(f))`-weighted multiset Jaccard over corpus-global DF.
Result: **AUC-PR 0.419→0.417 (flat)** — no ranking improvement. At *matched* recall (0.621),
IDF gave identical HN-FP (0.077), negligible pool-precision change, and *more* predictions.
The apparent HN-FP drop at default threshold (0.077→0.026) was purely a stricter operating
point (recall fell to 0.603), not the reweighting. The value-graph + shape multisets already
encode enough specificity that global IDF only rescales without separating. Reverted.

**Conclusion:** precision-via-scoring is the hard frontier — simple re-ranking (IDF) and
thresholding don't move it. The session's gains came from *extraction* (blocks, arrow units)
and *normalization* (async, literals), not scoring. The next precision lever needs heavier
machinery: a **verification re-ranker** (the validated LLM judge, or a precise structural
equivalence check, applied to top candidates) and/or a **better-sampled precision benchmark**
(the current pool is prediction-derived with a ~10% base rate, which caps measurable precision).

## O. Unbiased precision benchmark — overturns §N

§N concluded "threshold is a weak lever (precision tops ~0.16)" — but that used the
**prediction-derived pool**, which is biased (built from one old config) and, worse,
**pool-precision is overlap-weighted** (it counts predictions hitting each pool pair),
so dense low-score regions dominate it. Fixed the measurement: **stratified-random sample
of the current detector's predictions by score band, labeled by the validated judge oracle**
(`prep_precision.py` → judge → `save_precision_bench.py` → `bench/goldens/precision_sample.v1.json`;
score with `bench/precision_eval.py`). Population-reweighting gives an unbiased estimate.

| score band | population | sampled precision |
|---|---|---|
| 0.70–0.78 | 2180 | **0%** |
| 0.78–0.86 | 2575 | **0%** |
| 0.86–0.94 | 1069 | 3.3% |
| ≥0.94 | 4549 | **40%** |

- **Unbiased overall precision = 17.9%** (pop-reweighted), vs the pool's misleading 10.6%.
- **The score is strongly discriminative** — §N was a measurement artifact. Precision-vs-threshold:
  ≥0.70 → 17.9% (10373 preds), ≥0.86 → **33%** (5618), ≥0.94 → **40%** (4549).
- The **bottom two bands (~4755 predictions) are ~0% precision — pure noise** the 0.70 default admits.

**Lessons:** (1) confirms the earlier hypothesis that measurement is co-evolved — the precision
ceiling was a pooling artifact, invisible until precision was the optimization target;
(2) pool-precision is a flawed estimator (overlap-weighted, prediction-biased) — superseded by
the stratified population-reweighted estimate; do NOT fold stratified labels into the pool
(it corrupts pool-precision: tested, crashed it 0.106→0.060). (3) The snapshot is detector-
specific — re-judge after a material prediction shift.

**Actionable (next):** threshold IS a strong precision lever. Bottom-band noise + the
≥0.94→40% / ≥0.86→33% curve make raising the default operating point (recall 0.621→~0.547 at
0.86) a real precision/recall trade — a product decision, now measurable.

## P. Iteration loop toward world-class (goal-driven). Objective: recall@thr0.86 ↑, HN-FP=0 held, no prediction explosion; precision re-validated by the §O judge snapshot for substantive changes. Funnel at start: unit-reachable 47/65, candidate-reachable 35/65, recall@0.86 0.547.

- **P1 — LSH candidate-gen param sweep (optuna-style, one session): FLAT/REJECTED.**
  Grid bands∈{24,32,48,64} × minhash_k∈{128,256} at thr 0.86. recall 0.547 across ALL
  configs (HN-FP 0, ~5.6k preds). Candidate generation is exhausted — even max sensitivity
  (bands=64) doesn't surface the 12 unit-reachable-but-not-candidate pairs (their value-graph
  fingerprints genuinely diverge: cross-structure clones). recall@0.86 is extraction- and
  scoring-bound, not candidate-bound. `bench/sweep.py` added.

- **P2 — extract block-bodied callback arrows as units (test blocks `it('…',()=>{…})`):
  REJECTED.** Extraction worked (zustand test file 0→13 units) but recall UNCHANGED at
  every threshold (0.86: 0.542→0.542; 0.70: 0.621→0.621) while predictions exploded
  (5617→10100 at 0.86; 10373→18991 at 0.70). The gold test-clones (zustand create+hook vs
  createStore; swr) don't match — different APIs/structure — so extracting them only adds
  mutually-similar boilerplate-callback noise. Confirms (with P1): the bottleneck is MATCHING
  the hard cross-structure/cross-API pairs, not extraction or candidate-gen. Reverted.

- **P3 — string-literal value retention (extend §K int-retention to strings): SUCCESS.**
  FP analysis of the §O labeled sample showed the dominant high-score FP mode is
  *same structure, different string constant*: locale/i18n tables (Macedonian vs
  Norwegian messages), HTTP method `"OPTIONS"` vs `"HEAD"`, schema format `"nanoid"` vs
  `"cidrv4"`. Strings were abstracted to `LitClass::Str`, so these matched. Fix: retain a
  string content hash (`Payload::LitStr`, frontend `str_lit`); value-graph keys by it (own
  range), structural tag stays abstract `Str`. Result (recall held EXACTLY): pool-precision
  **0.154→0.316** (2×), AUC-PR **0.328→0.759** (2.3×), predictions 5617→4091, HN-FP 0,
  recall 0.5418 unchanged, 26 fixtures pass. No true Type-4 clone hinged on string equality.

- **P4 — literal values in the STRUCTURAL tag (node_tag), to demote the residual
  0.86–0.94 locale/string FPs: REJECTED (breaks correctness).** Folding values into the
  shape tag raised precision hugely (pool 0.154→0.70, labeled 31.7%→45.7%, preds 5617→2238)
  BUT (a) broke known-equivalence fixtures — full literals broke `loop_unification_cfor_
  equals_while`, strings-only broke `template_literal_equals_concat` — and (b) cost 3 true
  clones (recall 0.542→0.509; labeled true-recall 100%→84%). Literal values belong ONLY in
  the value-graph (soft, §P3), not the structural IL: the IL's job is to *converge*
  equivalent forms (template↔concat, loop unification), and value-laden shape tags break
  that. Reverted. The residual locale FPs are "same shape, different data" — the structural
  + RANSAC terms (string-abstract) keep them high; demoting them needs score reweighting
  (next), not IL changes.

- **P5 — score-weight search (optuna-style, one session): SUCCESS (big precision gain).**
  The residual 0.86–0.94 locale FPs survive because RANSAC (0.5 of the final score) is
  computed over the string-ABSTRACT linear tag sequence — locale tables have identical token
  sequences, so RANSAC scores them high. Made the (vj,sj,ransac) weights env-tunable and swept
  a 10-point simplex at thr 0.86 (objective: labeled-precision ↑ s.t. recall, HN=0). Best:
  **(0.5, 0.3, 0.2)** — RANSAC down-weighted 0.5→0.2, weight to value-graph+shape. INDEPENDENT
  re-judge (fresh stratified sample, not the tuning set): **unbiased precision 38.1%→57.0%**
  (≥0.94 band 60%→77%), recall 0.542→0.529 (~1 pair), HN-FP 0, deterministic, 26 fixtures pass.
  RANSAC was over-weighted; it rewards token-order agreement but is blind to literal values.

- **P6 — literal-weighted value-graph Jaccard (Const nodes ×3) to demote mid-band locale
  tables: REJECTED.** recall 0.529→0.484 (3 gold pairs lost — true clones with incidental
  literal diffs over-penalized) while trusted precision (pool-P 0.232→0.224, AUC 0.743→0.730)
  did NOT improve. The locale-FP win showed only in the labeled proxy (which over-represents
  them and is circular w.r.t. tuning); the typed4 pool barely contains locale tables, so it
  can't reward the fix, while the recall cost is real on the 65 gold. Literal-weighting is
  too blunt — it hits every constant, not just data-table constants. Reverted.

- **P7 — data-table literal gate (surgical fix for the locale-FP mode): KEPT (real FP
  removal; aggregate metric flat).** A unit whose value-graph is ≥20% literal `Const` nodes
  is a "data table"; such a pair is capped by its literal Jaccard (constants must match).
  `value_fingerprint_lits` exposes the literal multiset; `UnitFeat.lits`; gate in the scorer
  (threshold `NOSE_DH`, swept: 0.20 is the knee — 0.15 starts costing recall). Removes **218
  predictions, ALL verified locale-table non-clones** (ca↔fr-CA, tr↔az, locales.py, …) at
  **zero recall cost** (recall 0.5292 unchanged, lblR 100%). Since TP is unchanged and 218
  confirmed FPs are gone, true precision necessarily rises — but the aggregate unbiased
  precision is flat (56% vs 57%, within the 60-pair sample noise) because the 0.86–0.94 band
  has *other* FP modes dominating the residual, and the typed4 pool contains no locale tables
  (so pool-P/AUC dip slightly — the known pool blind spot, not a real regression). A
  better-sampled precision benchmark would resolve the gain; kept because it strictly reduces
  false matches with no recall cost.

- **P8 — capture class-level attribute values in the value-graph: SUCCESS (biggest gain).**
  Diagnosis: `locales.py` had 783 predicted pairs — locale *classes* store data as class-level
  attributes (`past = '…'`), which `process_stmt` puts in `env` but never pushes to a SINK, so
  the strings are unreachable and absent from the fingerprint (the value-graph saw class data as
  empty). Fix: for non-Func (class) units, expose the final `env` values as effect sinks — a
  class's attributes ARE its data. Locale classes now differ (different string data) and the §P7
  gate demotes them. Result: predictions 2914→1877 (locales.py pairs 783→112), recall held 0.529,
  HN 0, 26 fixtures pass. INDEPENDENT re-judge: **unbiased precision 57%→75.3%** (≥0.94 band 83%,
  0.86-band 3.3%→16.7%); trusted typed4 pool-P **0.217→0.469**, AUC **0.709→0.939** — broad, not
  just locales (many class-data-mismatch FPs across the pool were demoted).

**Iteration §P summary: precision ~6%→75% (unbiased) at recall 0.53, HN-FP 0, via P3 (string
retention) + P5 (RANSAC down-weight) + P7 (data-table gate) + P8 (class-attribute capture).
Rejected: P1 (LSH sweep, flat), P2 (callback units, noise), P4 (structural literals, breaks
fixtures), P6 (literal-weighted vj, recall cost).**

- **P9 — dual candidate channel (value-graph MinHash ∪ shape MinHash): REJECTED.** Added a
  shapes channel to surface structurally-identical (renamed/types-erased) pairs the value
  channel misses. candidate-reachable 33→40 (+7) BUT recall@0.86 unchanged (0.529), predictions
  unchanged — the surfaced pairs score <0.86 because the gold *fragment* doesn't align to the
  *unit* cover() finds (unit ≠ fragment). Confirms (with P1) candidate-gen is dead for recall:
  the bottleneck is unit-level scoring + fragment/unit boundary mismatch + hard cross-API cases,
  not candidacy. Reverted (added cost, no gain).

- **P10 — operating-point re-characterization (the §O curve predated the P3–P8 scorer
  gains): CONFIRMS 0.86.** Re-judged a fresh full-range stratified sample of the P8 detector.
  Post-P8 cumulative curve: ≥0.70 → 44%/rec0.580, ≥0.78 → 49%/0.563, **≥0.86 → 72%/0.535**,
  ≥0.94 → 80%/0.417. The whole curve lifted (≥0.86 was 33% in §O → 72% now) but 0.86 remains
  the balanced optimum — lowering it trades ~28 precision points for ~3 recall pairs; 0.94
  trades 0.12 recall for +8 precision. Default unchanged. (Note: the data-table gate caps
  locale-table FPs into the 0.70–0.86 bands, where the 0.86 operating point correctly excludes
  them — which is why band 0.78 is noisy but ≥0.86 is clean.)

- **P11 — return-signature gate: SUCCESS.** The ≥0.94 residual FPs are one-element diffs
  (e.g. version.py `__lt__`/`__gt__`/`__le__` — identical body, different comparison operator)
  that score high because the diff is diluted in the multiset. Fix: expose RETURN-sink value
  hashes (`value_fingerprint_lits` 3rd return; `UnitFeat.returns`); cap a pair's score by
  `ret_base + (1-ret_base)·return_jaccard` when both return values (a total return mismatch
  caps below threshold). Swept `NOSE_RET`: 0.80 is the knee. Removes 32 verified FPs (all
  version-comparison dunders with different operators → different returns), recall held EXACTLY
  (0.529), HN 0, 26 fixtures pass. INDEPENDENT re-judge: **unbiased precision 75.3%→78.1%**
  (≥0.94 band 83%→87%); trusted pool-P **0.469→0.489**, AUC **0.939→0.952** (these FPs ARE in
  the typed4 pool, so trusted metrics moved too).

**§P final: unbiased precision ~6%→78% at recall 0.53, HN-FP 0, AUC 0.95, on a judge-validated
benchmark. Wins: P3 strings, P5 RANSAC↓, P7 data-table gate, P8 class-attrs, P11 return-gate.
Rejected w/ evidence: P1 LSH, P2 callback units, P4 structural literals, P6 literal-weight,
P9 dual-channel, P10 (confirms 0.86). Recall (0.53) is architecturally capped — candidate-gen
is dead (P1,P9) and the misses are hard cross-API / fragment-boundary cases.**

## Q. Goal reframe — refactoring-candidate discovery (not strict behavioral equivalence)

The user clarified the actual goal: find code **similar enough to be worth a human's review as
a refactoring candidate** — small FP rate is *fine* (the human filters), recall + ranking +
clustering matter. The strict behavioral judge (§H–P) was the wrong oracle.

**Re-judged the detector's predictions under a refactoring-worthiness rubric** ("would a dev
review these to extract shared structure?"):
- candidate config (gates OFF, thr 0.72): **99.0% review-worthy** (118/120 sampled), ~4.5k pairs.
- the §P precision gates (P7 data-table, P11 return-gate) were *deleting good candidates* — locale
  classes, comparison-operator families, sync/async wrappers are exactly the refactor targets.
- threshold floor: ≥0.72 → 99%, [0.55,0.70) → 70% (superficial cross-domain boilerplate creeps
  in). So **0.70 is the candidate-mode operating point** (~4.5–5.6k pairs / ~720 families).
- recall vs the BROAD structural gold v2: strict 0.103 → candidate **0.158** (+53%).

**Shipped `--candidates` mode** (`StructuralDetector::candidates`): gates off, threshold default
0.70, output labeled `structural-candidates`. Strict behavioral mode (gates on, 0.86) remains the
default for clone detection. Both verified; strict baseline unchanged (1845 preds); 26 fixtures pass.

Reframe of the §P arc: the precision work built a trustworthy *behavioral* detector (~78%); for the
*refactoring* goal the same machinery, gates off + lower threshold, is ~99% review-worthy. Recall
(finding more families) and ranked-cluster output are now the levers.

- **Q1 — dual candidate channel re-tested under candidate mode (thr 0.70): REJECTED again.**
  At the low threshold the shapes channel adds only +16 families (720→736), +0.003 recall vs
  gold v2, while candidate pairs explode 4× (302k→1.27M). The structurally-identical pairs were
  already candidates via the value channel at 0.70. Candidate-gen is dead even for the refactoring
  goal; family-recall is near the architectural ceiling. Reverted.

## R. Performance — frontend parser pool (win) + SmallVec children (null)

Profiled `scan` on date-fns (1621 TS files). The detection pipeline is already
cheap (normalize 8ms, extract 2ms, candidates 1.4ms, score 1ms, cluster 0.1ms ≈
13ms); the **frontend (discover+parse+lower) dominates at ~88ms warm**. Added a
`NOSE_TIME` `lower` stage line so this is visible.

- **R1 — thread-local parser pool: ACCEPTED (~1.8×).** Every file allocated a fresh
  `tree_sitter::Parser` and reloaded its grammar. `lower::parse(key, lang, src)` now
  caches one parser per grammar per rayon worker (no lock — each worker owns its pool).
  date-fns lower stage **88ms → ~48ms warm**, output byte-identical. Shipped.
- **R2 — `SmallVec<[TsNode; 8]>` for `named_children`: REJECTED (null).** Hypothesis:
  the per-node `Vec` allocation in `named_children` (110 call sites, runs per CST node)
  is a hot allocation. Measured lower stage ~47ms vs ~48ms — within run-to-run noise.
  Parsing dominates the remaining frontend time, not child-list allocation. Adding a
  dependency for no measurable gain isn't justified; reverted. (Noise-floor discipline:
  a ~1ms move on a 48ms stage is not a result.)

## S. Cross-language convergence audit (bug hunt via equivalence testing)

Wrote the *same* algorithm (guarded accumulator; find-max; map/comprehension;
string interpolation) in each language and asserted the units converge to one IL
hash. This surfaced three real lowering bugs that silently broke matching — none
caught by single-language tests:

- **S1 — Rust `*x` deref → `UnOp(Neg)`.** `lower_unary` treated any non-`!` unary as
  negation, so a dereference became a *negation* node. Cross-language: `*x > 0` could
  never match `x > 0`. Fix: peel `*x` to its operand like `&x`. The 6-language
  accumulator family (py/ts/go/rust/java/ruby) only forms after this.
- **S2/S3 — Python f-string & Ruby interpolation dropped the interpolated expr.**
  `f"hi {name}"` / `"hi #{name}"` lowered to one opaque string literal — `name` was
  discarded. Fix: detect interpolations and fold them into a base `Str` + `Add`-per-expr
  chain, matching `lower_template` (JS). f-string ≡ interpolation ≡ template now.

Lesson: per-language coverage (Raw% ≈ 0) does **not** imply correct *convergence* —
a construct can lower cleanly yet to the wrong shape. The convergence equivalence
tests (one algorithm × N languages → one hash) are the discriminating check; they
are cheap and each new shape (a second algorithm) is a fresh chance to catch a
shape-specific bug. Corpus coverage after fixes: 99.993% (94 Raw of 1.34M nodes).

- **S4 — branch orientation produced non-canonical comparisons.** The documented
  equivalence `if a<b {X} else {Y}` ≡ `if a>=b {Y} else {X}` never converged:
  orientation inverted `Lt`→`Ge`, but `algebra` (an earlier pass) maps `Ge`→`Le`
  with swapped operands, so the inverted form sat outside the canonical
  `Lt/Le/Eq/Ne` set the rest of the IL uses. Fix: `invert_comparison` returns the
  canonical operator plus an operand-swap flag (`Lt`→`Le`+swap, `Le`→`Lt`+swap).
  Both `</>=` and `<=/>` branch pairs converge now. Lesson reinforced: a documented
  normalization is not a *verified* one until a convergence test exercises it — this
  pass shipped with an ablation number but no equivalence test, and was silently
  inert for the common case.

## T. Performance — parallelize every stage; ~14k → ~19.5k files/sec

Profiled the full `scan` pipeline (3620-file corpus, 18 cores). parse+lower
already scaled **11.6× across cores** (1011ms→87ms single→18 threads) — CPU-bound on
tree-sitter, not serial-limited — so the wins were in the remaining stages:

- **T1 — parallel file discovery: ACCEPTED.** The `ignore` directory walk ran
  single-threaded (33ms serial latency before any lowering). Switched to `ignore`'s
  parallel walker → ~20ms. Paths are now sorted by name, so a file's `FileId` is
  deterministic across machines (the old readdir order was only same-machine stable).
- **T2 — sort-based parallel LSH: ACCEPTED (3.6×, 22ms→6ms).** Replaced the
  HashMap-of-buckets + HashSet-dedup with: emit all `(band-hash, unit)` entries in
  parallel → `par_sort_unstable` (equal-hash runs are the buckets) → emit pairs per
  bucket in parallel → sort+dedup once. Data-oriented (contiguous 16-byte entries,
  no per-bucket allocation) and parallel. Output byte-identical.
- **T3 — fuse normalize+extract: ACCEPTED (memory).** Two corpus-wide passes became
  one `flat_map_iter`; the full normalized `Vec<Il>` is no longer materialized
  alongside the raw corpus, ~halving peak IL working set. Wall-time neutral.
- **T4 — pre-size the IL arena from source length: REJECTED.** Hypothesis: `len/10`
  capacity avoids grow-reallocations. A same-thermal-window A/B (min-of-8) showed it
  was *slightly slower* (96 vs 85ms) — the eager per-file allocations cost more than
  the doubling-growth they save for a corpus of mostly-small files. Reverted (cf. the
  §R2 SmallVec null result — measure before claiming).

## U. Refactor-worthiness ranking — test-awareness + type-def discount (adopted)

Goal reframe (§Q) is *refactoring candidates*, so the metric that matters is
top-family precision on real code, not Type-4 gold recall. Instrumented per-family
signals — mean value-graph size (`sem`), literal-dominance, pairwise literal
agreement — and read the top families on `bench/repos` and nose's own `crates`.

**Hypothesis rejected — literal agreement / "CPG-slice".** The idea was that
shape-only false positives (two unrelated lookup tables that look identical once
literals are abstracted, e.g. `py_bin_op` vs `from_extension`) would show low
literal agreement. The data killed it: literals are near-empty after abstraction
(`lit_ratio` ≈ 0 everywhere), and low literal agreement also appears on **genuine**
families (sync/async `Client` extract-base; cross-language `lower_string`). Not
separable. Dropped — measure before believing (cf. §N→§O).

**What the data actually showed.** The dominant real noise is two classes:
1. **Test duplication** — `Test*Locale` classes (×5/×14/×7), `TestComponent` ×18.
   Intentional scaffolding (fixtures, arrange/act/assert) that buries the prod signal.
2. **Value-poor type definitions** — field-only `Class` units (sem ≈ 6) matching on
   shape alone, no behavior to extract (the dogfood-documented #1 false positive).

**Adopted — a ranking-time discount** (scan path only; `detect`/`eval` gold
path is untouched, `rank_families` has a single caller, so Type-4 numbers can't
move). Each family is tagged `scope = prod | test | mixed` by a conservative path +
unit-name heuristic; all-`test` families ×0.2; **`mixed` test↔prod is NOT
discounted** (logic across the test boundary is a real smell); all-`Class` families
with mean sem < 12 ×0.25 (rich classes like `OrderedMultiDict`, sem 90, untouched).

| corpus | top-15 before | top-15 after |
|---|---|---|
| bench/repos | 2 `Test*Locale` families | replaced by genuine prod (series, error, declension) |
| crates | value-poor `Class:2` type-def at #4 | demoted out |

**Cost:** none — ranking-side path/name/sem checks; cluster stage 0.4ms either way
(~18k files/s). Disable for A/B with `NOSE_NO_REFACTOR_DISCOUNT=1`. `scope` is
shown in the human report (`· test`, `· test↔prod`) and serialized.

## V. jscpd-weak superset — corpus expansion + recall-floor measurement

Built the eval foundation for the refactoring-candidate goal: expanded the
benchmark corpus from 16 repos (Python/JS/TS only) to **31 across all 8
languages** — added 3 each for Go (cobra/zap/gin), Rust (ripgrep/serde_json/
tokei), Java (gson/commons-lang/jackson-core), C (curl/jq/libuv), Ruby
(sinatra/faraday/puma), pinned by commit, dev/heldout split. Added
`bench/jscpd_superset.py`: runs jscpd `weak` mode per repo and measures what
fraction of its copy-paste pairs nose also surfaces (both sides contained ≥0.5
in a nose family span, output-wide).

**Two findings from the expansion:**
1. **Coverage (Raw-node ratio) was overfit to Py/JS/TS.** On real-world code the
   ratio is 3–11% for Rust/C/Java/Ruby (vs <0.01% on the old corpus) — many
   idiomatic constructs in those languages still fall through to `Raw`. Logged as
   lowering work; recall on those languages is capped until closed.
2. **nose is *far* from a jscpd-weak superset, structurally.** Measured coverage:

   | scope | coverage |
   |---|---|
   | all pairs | **18.2%** |
   | production only (excl. test/vendor) | 27% |
   | production + substantial (≥100 tokens) | 40% |
   | production + substantial (≥200 tokens) | 47% |

   Miss breakdown: **79% of misses are in test/spec code** (jscpd-weak floods
   tests with sub-unit copy-paste — test tables, assertion blocks — exactly the
   scaffolding §U discounts), 9% prod sub-unit fragments, 8% prod files with no
   nose family, 3% vendored.

**Root cause (fundamental, not tuning):** jscpd matches *arbitrary contiguous
token runs*; nose matches *unit-bounded* (function/class/loop/if/try) families
via the value graph. Even at min-tokens 200 and threshold 0.45, sub-unit runs and
unpaired regions don't map to any nose unit. Closing to ~90% needs a **contiguous
token/statement-sequence detection channel** (jscpd's core) added alongside the
structural one — a real capability with precision/perf tradeoffs and a shift in the
tool's character. The guardrail script exists and measures this; it is **not** wired
as a blocking gate until the target/scope is decided.

### U.1 — test-discount reverted (it was the wrong heuristic)

The §U all-test ×0.2 discount was **removed**. Duplication in tests is a genuine
smell — it should be surfaced and ranked like any other, not folded down. Treating
test copy-paste as "intentional scaffolding" was an inappropriate heuristic: it
suppressed real findings and worked directly against being a recall superset of
copy-paste detectors (see §V — 79% of jscpd-weak's findings are in test code). The
value-poor type-definition discount is kept (those genuinely have no behavior to
extract). The `scope = prod | test | mixed` tag stays as *context*, with no ranking
effect.

### V.2 — contiguous copy-paste channel (jscpd-weak 18%→78%)

Added a second detection channel (`contiguous.rs`): a Rabin-Karp scan over each
file's raw-IL token stream (`node_tag`, content-hashed) finding maximal duplicated
runs regardless of unit boundaries — the Type-1/2 floor the unit-level structural
channel can't reach. Built from *raw* IL (alpha-renaming is function-scoped, so
normalized tokens of a copy-pasted block diverge by enclosing-function context);
honours `// nose-ignore` via `Il::suppressed` byte-ranges. On for `scan`
(`--no-contiguous` off-switch), off for the strict/gold path.

jscpd-weak pair coverage, 31-repo corpus: **18.2% → 78.1%**. Per-language the floor
tracks lowering coverage: jq/boltons/marked 100%, cobra/sinatra 96%, zap 92%,
date-fns 87%, curl 79% — but Java (gson 46%, commons-lang 54%, jackson-core 68%)
and Rust (serde_json 69%, tokei 71%) lag, the same languages with 2–8% Raw-node
ratio (§V finding 1). Perf: +~24 ms on commons-lang (620 files); O(tokens).

**Remaining path to 90%** (not yet done): (a) close the Rust/C/Java/Ruby lowering
gaps — unlowered constructs become unstable tokens and break runs; this is the
single biggest lever and lifts both channels; (b) approximate/gapped matching to
mimic jscpd-*weak*'s fuzzy merging across ignored tokens (≈half the residual misses
are `<0.5` line-containment where our *exact* run is shorter than jscpd's fuzzy
one). The guardrail (`bench/jscpd_superset.py`, target 90%) stays a measurement
tool, not a blocking CI gate, until those land.

## W. Refactoring-family labelset + the product metric (precision is the lever)

Built the ground-truth eval set the goal actually needs (see `bench/labels/`):
RUBRIC.md defines `worthy` (extract-helper/base/data-table/parameterize) vs not
(parallel-by-design/coincidental-shape/type-def/generated/trivial), judged
independent of scope (test duplication counts the same). An unbiased candidate pool
(`pool_candidates.py`: nose-structural ∪ jscpd-weak over the 18 dev repos, 235
families balanced across 8 languages) was labeled by a 3-persona LLM panel
(pragmatic / dedupe / skeptic), reconciled by majority vote, and the 90 2-1 splits
settled by a tiebreak judge (1 escalated to human). Result: **235 families, 143
worthy / 92 not, 186 high-confidence**, dev/heldout split honoured (only dev
labeled; heldout reserved).

`eval_refactor.py` scores nose's ranked `scan` output against it:

| metric | dev |
|---|---|
| **worthy-recall** | **97%** (139/143) |
| precision@10 | 57% (of matched top-10) |
| precision@20 | 66% |

**Finding: recall is excellent, ranking precision is the lever.** nose surfaces
almost every worthy family *somewhere*, but ~43% of the top-10 ranked are
not-worthy — parallel-by-design variants, locale/i18n maps (zod), generated/vendored
code — ranked high by raw `refactor_value`. The next improvement is a worthy-vs-not
ranking signal, now measurable against this set (precision@10 57% → ? without
hurting the 97% recall). This is the measurement foundation the §U/§V false starts
lacked.

## X. Ranking precision — labelset-driven (recall 97%, precision@10 61%→63%)

With the labelset (§W) as ground truth, attacked the real lever: of nose's top-K
ranked families, how many are *worthy*. Baseline (dev): worthy-recall 97%,
precision@10 57% (61% after the §W example/demo relabels). Characterized the
top-10 not-worthy "polluters": parallel-by-design (zod locale/i18n maps), generated
(rich Unicode version tables), coincidental-shape.

Every candidate signal was **validated on the labelset before shipping** — and the
labelset rejected most of them, exactly as intended (cf. the §U/§V/test-discount
false starts that had no such gate):

| signal | result | decision |
|---|---|---|
| vendored/generated path (all sites) | 0/12 worthy — clean separator | **adopted** (×0.1) |
| literal-dominance (`data_ratio`) down-weight | high data_ratio is *more* worthy (100% ≥0.15) — opposite of hypothesis | rejected |
| data-table gate in candidate mode | no precision gain, −1 recall (locale maps aren't "data-heavy" by the gate's measure) | rejected |

Net: **precision@10 61%→63%, recall held at 97%** from the one validated signal
(generated-path discount), `scan`-only (gold path untouched). The dominant
remaining polluters — zod-style locale/version *parallel data variants* — are
structurally identical to worthy duplication under every cheap signal tried (their
literal values differ but the value-graph dilutes the literal ratio below the gate,
and abstraction hides the string content). Distinguishing them looks to need either
literal-value-aware scoring that actually fires for them, or path-structural
heuristics (`locales/`, per-version dirs) that would overfit the dev repos —
deferred rather than risk a §U-style regression. The labelset makes any future
attempt measurable.

## Y. Anti-unification re-rank — refactorability beats duplication-volume (precision@10 +8pp)

The fundamental reframe (similarity ≠ refactorability): rank by *how clean the
shared abstraction is*, not raw duplication. For each family, anti-unify (most-
specific generalization of) two representative members' normalized-IL subtrees by a
parallel tree walk (`antiunify_probe.py`):
  - **template** — positions agreeing in kind (the shared skeleton),
  - **struct_holes** — kind/arity divergence (whole differing subtrees),
  - **value_holes** — leaves, same kind, different payload (clean parameters; e.g. a
    locale's strings, where `LitStr` is a content hash so they're cross-file comparable).

Separability on the labelset (235 dev families):

| signal | worthy rate |
|---|---|
| abstractness ≥0.95 vs <0.5 | 74% vs 41% |
| value_hole_ratio ≥0.20 (locale-style) | 39% (vs 65% baseline) |
| combined clean-skeleton heuristic | 72% (matches) vs 49% (fails) |

This is the first signal that catches the dominant polluter — zod-style locale /
version *parallel data variants* (high abstractness BUT many value_holes → the holes
ARE the content, not a parameter → not extractable). Simulated re-rank
(`antiunify_rerank.py`, value × refactorability, top-40 reordered):

| | precision@10 | recall |
|---|---|---|
| baseline (value) | 58% | 97% |
| **anti-unif re-rank** | **66%** | **97%** |

**+8pp precision, recall held** — 4× the generated-path signal, and unlike the §X
heuristics it attacks the root cause. Validated in Python; productionizing means
threading a compact pre-order (kind, payload-hash, arity) per unit into `UnitFeat`
and computing the anti-unifier at group-build time. The reframe — nose reasoning
about *the refactoring* (skeleton + parameters), not just similarity — is also what
turns the output from a clone list into a concrete extraction proposal.

## Z. Per-language eval, decoupled from lowering — the aggregate +8pp hid a Rust regression

Per-language A/B of the §Y anti-unification re-rank, reported alongside the lowering
confound (`eval_by_language.py`):

| lang | n | worthy | P@10 base | P@10 re-rank | meanRaw |
|---|---|---|---|---|---|
| TypeScript | 48 | 56% | 39% | **61% (+22)** | 1.0% |
| C | 28 | 36% | 50% | 75% (n=4) | 2.1% |
| Python | 50 | 62% | 61% | 65% (+4) | 0.0% |
| Java | 28 | 61% | 88% | 91% (+3) | 6.1% |
| Go | 28 | 79% | 60% | 60% | 0.0% |
| Ruby | 25 | 92% | 100% | 100% | 2.8% |
| **Rust** | 28 | 64% | 47% | **42% (−5)** | 5.1% |

The aggregate +8pp is really **+22pp on TypeScript** (where zod-locale parallel-data
families dominate and anti-unification's value-hole signal demotes them) and a
**−5pp regression on Rust**. The decoupling (P@10 on clean-IL vs Raw-heavy families)
shows why: Rust's worthy families sit in the Raw-heavy bucket (clean 2/7=29% vs
raw-heavy 5/8=62%, meanRaw 5.1%). Anti-unification on trees full of opaque `Raw`
nodes produces meaningless holes, so the re-rank misranks exactly the languages
whose frontend is weakest.

**Design implication (for productionization):** gate the refactorability re-rank on
lowering quality — apply it only to families whose members are cleanly lowered (low
Raw-node ratio), fall back to value-rank otherwise. And/or fix the Rust/C/Java
lowering gaps first (§V finding 1), which would lift both the signal and the floor.
Per-language numbers are not comparable until Raw ratio is controlled — this table
is the instrument that makes the attribution (ranker vs frontend) visible.

Caveats: small per-language n (C/Ruby top-10 span 1–2 repos); dev-only (heldout
unlabeled). Both are the next eval-infrastructure steps.

## AA. Lowering campaign — closing the per-language Raw gaps (measured, Pareto-first)

§Z showed lowering gaps (3–11% Raw) confound per-language eval and break the
anti-unification signal. Targeting the highest-frequency unhandled constructs per
language (`nose stats --json` Pareto), verified by Raw-drop + convergence tests.

**Ruby: 9.66% → 1.79% Raw.** Top offenders were core constructs:
- `simple_symbol` / `hash_key_symbol` (6.8k) — the code matched `"symbol"` but
  tree-sitter-ruby emits `simple_symbol`; `:foo` and `{foo: …}` keys fell to Raw.
  Routed to string-literal lowering (the symbol's value participates in matching).
- `pair` (3.4k) — hash entries `k => v` / `k: v`, now `Seq[k, v]`.
- `if_modifier` / `unless_modifier` (1.1k) — guard clauses, now `If`, via a shared
  `lower_modifier` used from both statement and expression (tail) position;
  convergence test proves `x if c` ≡ block `if c then x end`.
- `scope_resolution` (`Foo::Bar`), ternary `conditional`, `regex` literals.

Remaining Ruby Raw (begin/rescue→Try, blocks) is lower-frequency, deferred. C
(18%, dominated by statement-kinds leaking through the expr fallback under
unhandled GCC statement-expressions/macros) and Rust (8%, `token_tree` macro args)
are the next targets.

**C: 18.11% → 4.15% Raw.** The dominant cause was the `lower_expr` fallback
recursing children as expressions: GCC statement-expressions `({ … })` reach
`lower_expr` via `parenthesized_expression`, so their `compound_statement` body and
every inner statement (`expression_statement` 10.3k, `if_statement` 3.4k,
`return_statement` 1.5k) fell to Raw. Fixes: handle `compound_statement` in
`lower_expr` (→ Block, routing inner statements through `lower_stmt`);
`labeled_statement` (goto targets) → lower the inner statement, drop the label;
`sizeof_expression` → int literal (operand is often a type). Remaining C Raw is
`ERROR` (tree-sitter parse failures on macros), type-level declarator nodes (should
be erased), `preproc_ifdef`, and `goto` — lower-value, deferred.

Corpus-wide Raw after Ruby+C: **2.00%** (Rust 6.7% now worst — `token_tree` macro
args — then C 4.2%, Java 3.0%).

**Rust: 6.66% → 1.08% Raw.** `token_tree` (macro args, 8.5k) dominated: `lower_macro`
passed nested token_trees to `lower_expr`, which Raw'd them. Macro args are an
unparsed token stream (nested delimiters are sub-token_trees), so now we recurse
through them collecting only real atoms (identifiers/literals) as call args and drop
delimiters — a macro never leaves Raw token_tree nodes. Remaining Rust Raw is
type-level (type_arguments, generics, lifetimes — should be erased) + parse ERRORs.

**Campaign result:** Ruby 9.66%→1.79%, C 18.11%→4.15%, Rust 6.66%→1.08%; corpus-wide
Raw **1.75%**. This directly targets the §Z confound — the next step is re-checking
the anti-unification re-rank per language now that Rust's IL is clean.

### Z.1 — lowering did NOT fix the Rust re-rank regression (premise falsified)

After the §AA lowering campaign cleaned Rust's IL (meanRaw 5.1%→0.0%), re-ran the
per-language re-rank A/B expecting the §Z Rust regression to vanish. It did not:
Rust still 47%→38%. Decomposing the refactorability formula
(abstractness × value-hole-penalty × struct-hole-penalty) by term:

| | base | full | value-hole-only |
|---|---|---|---|
| TS | 39% | **61%** | 39% |
| Rust | 47% | **38%** | 47% |

The TS gain comes from the abstractness/struct-hole terms (value-hole-only does
nothing); those same terms cause the Rust loss. The helpful and harmful effects are
the same terms — no clean separation at current label resolution (Rust n≈13–15 in
top-10). **Conclusion:** the anti-unification re-rank is a TS/Python/Java win and a
Rust loss, *not* a universal one, and lowering quality was not the cause. Do not
productionize it globally — gate to validated languages or defer until more
per-language labels allow calibration. The lowering wins (recall + signal
cleanliness) stand on their own regardless.

This is the fifth hypothesis the eval has rejected or qualified (test-discount,
literal-agreement §V, data_ratio §X, candidate data-gate §X, and now the
"lowering-then-rerank-is-universal" premise). The measurement discipline is the
durable deliverable.


## AB. Eval infrastructure boost — bootstrap CIs reveal the precision story was mostly noise

Strengthened the eval per the per-language-evaluation need: grew the labelset to
**v2 = 576 families** (was 235) across **dev + heldout** (added a held-out
generalization split), via a larger balanced candidate pool (nose∪jscpd, dev+
heldout, 12 nose + 8 jscpd per repo), the same 3-persona panel + tiebreak (0
escalations). Per-language counts now 53–145 (Rust 28→53). Added **95% bootstrap
confidence intervals** and the dev/heldout split to `eval_by_language.py`.

The CIs overturn the recent precision narrative:

| (dev) | P@10 base | P@10 re-rank |
|---|---|---|
| Rust | 50% [25–75] n=16 | 47% [20–73] n=15 |
| TypeScript | 45% [25–65] | 55% [32–73] |
| **OVERALL** | **62% [52–72]** | **62% [54–72]** |

- **The §Z/§Z.1 "Rust re-rank regression" is not significant** — CIs [25–75] vs
  [20–73] almost fully overlap. We were chasing noise (n≈15, CI width ±25pp).
- **The anti-unification re-rank shows no significant aggregate benefit on v2**
  (62%→62% dev, 53%→53% heldout). The v1 +8pp (§Y) did **not** replicate on the
  larger labelset — it was largely v1-sample noise/overfit.
- Per-language precision differences are mostly **within noise** at current n; the
  point estimates from §Z were not trustworthy, exactly as that section cautioned.

What IS robust across both splits and languages: **worthy-recall** (dev ~98–100%,
heldout high). nose reliably *finds* the worthy families; ranking-precision
*improvements* are not statistically established at this label scale.

**Implication:** do not ship the anti-unification re-rank — it is not a validated
win. The durable, measurable levers remain recall-side (lowering coverage, the
contiguous channel). Tightening precision conclusions needs a substantially larger
labelset still (the CIs quantify how much). The eval infra paid for itself by
dissolving a multi-section false narrative before any of it shipped.

## AC. Labelset v3 (5×) + QA — CIs tightened, recall robust, re-rank still not generalizing

Built the high-quality 5× eval set (the /goal): **v3 = 3,092 families** (1,923 worthy
/ 1,169 not) across 41 repos / 8 languages, dev (1,897) + held-out (1,195), per
language 350–549. 3-persona panel + tiebreak, worked-example-calibrated rubric,
58% unanimous, votes embedded per family. See `bench/labels/README.md`.

QA via `eval_by_language.py` (bootstrap 95% CIs, dev/heldout):

| | dev base | dev re-rank | heldout base | heldout re-rank |
|---|---|---|---|---|
| OVERALL | 61% [53-68] | 66% [59-73] | 52% [43-60] | 53% [44-61] |
| TypeScript | 41% | 57% | 30% | 34% |
| Rust | 52% [32-72] | 54% [35-73] | 56% | 41% |

Findings (now on a properly-powered set):
- **worthy-recall is rock-solid**: dev ~98–99% every language (C 124/125, Java
  277/287, Ruby 183/184…). nose reliably *finds* worthy families — the durable result.
- **CIs tightened** overall (dev re-rank [59-73] vs base [53-68]) but **per-language
  precision@10 is still ±15-25pp** — because precision@10 samples only the top-10 per
  repo, so it's bounded by *#repos × 10* per language (16-40), not by #labels. More
  labels tightened recall + overall; tightening per-language *precision* needs more
  repos/language (or a larger-K metric).
- **The anti-unification re-rank shows a dev gain (+5pp) that does NOT replicate on
  held-out (52→53)** — the generalization gate says it's dev-overfit/noise, confirming
  §AB: do not ship it. The earlier "Rust regression" remains within noise (dev 52→54).

Net: the eval *data* is now strong (5×, dev+heldout, CIs, balanced, audited). It
re-confirms recall as the robust lever and the re-rank as unvalidated, and it
pinpoints that per-language *precision* power is repo-bound, not label-bound — the
next eval-infra lever if per-language precision becomes the focus.

## AD. Labelset v4 (62 repos) — more repos/language tightens the CIs (as §AC predicted)

Acting on §AC ("per-language precision@10 is bounded by #repos×10, not #labels"),
added 3 repos/language (+21 → 62 repos; repos/language now 7–12) and labeled the
1,546 new candidates (panel + tiebreak). **v4 = 4,615 families** (2,854 worthy /
1,761 not), dev 3,105 / heldout 1,510, 3,650 high-confidence.

CI eval (`eval_by_language.py`):

| | dev base | dev re-rank | heldout base | heldout re-rank |
|---|---|---|---|---|
| OVERALL | 61% [54-67] | 66% [60-72] | 53% [45-61] | 56% [48-63] |

- **CIs tightened** as predicted: overall dev width 15→13pp (n 157→238); per-language
  n roughly doubled (C 16→31, Java 24→40, Rust 25→34). Per-language CIs are still
  ±15-20pp — top-10 sampling means tightening further needs *even more* repos
  (~15/language for ±10pp), with diminishing returns.
- **worthy-recall stays robust** (~98–99% dev) across all languages.
- **The re-rank gain is now CI-separated on dev** (base [54-67] vs re-rank [60-72],
  +5pp) but **heldout CIs still overlap** (base [45-61] vs re-rank [48-63], +3pp not
  significant). Same qualitative verdict as §AB/§AC on the larger set: a dev gain
  that the generalization gate does not confirm — still not a clear ship, but less
  clearly noise than before. A v5-scale held-out set would settle it.

Net: the eval data is now an 8-language, 62-repo, 4.6k-family, dev/held-out,
CI-equipped set — strong enough that overall conclusions are tight and the per-
language confound (Raw ratio) and repo-bound precision power are both explicit.

## AE. Robustness — stack overflow on deep real-world files (fixed)

Expanding the corpus to 62 repos surfaced a real crash: `nose stats/refactor` on
the full set aborted with a stack overflow. Cause: lowering/normalization walk the
syntax tree recursively, and a pathologically deep file (prettier's test fixtures —
minified/generated code, deep nested expressions) overflowed the stack. `nose il`
on the same files didn't crash because it runs on the main thread (~8 MB) while
`stats`/`scan` lower via rayon, whose workers have a ~2 MB stack — and rayon also
runs tasks inline on the *calling* thread, so even an enlarged worker stack wasn't
enough.

Fix: size both the rayon worker stacks and the command's own thread to 1 GiB
(virtual; pages commit lazily) — `main` now runs the command body on a big-stack
`std::thread`. A clone detector must never crash on real input. Regression test
`deeply_nested_file_does_not_overflow` (a depth-40 000 nested literal) guards it.

Result: the full 62-repo / 26,697-file corpus lowers cleanly (overall Raw 2.78%;
Ruby 9.5% — rubocop metaprogramming — now the worst), refactor end-to-end ~2.3 s
(~11k files/s, speed preserved). Remaining true-robustness note: a 1 GiB stack still
has a ceiling; a depth-bounded lowering (degrade to `Raw` past a limit) would make it
unconditionally crash-proof — deferred as the recursion is now far beyond any real
file.

## AF. Extraction-proposal output — clone list → refactoring proposal

The §Y reframe (nose should reason about *the refactoring*, not just similarity)
shipped as a user-facing `--proposal` view. For each family it anti-unifies two
representative copies at line granularity (reusing the `--diff` LCS): the shared
lines become the body of the proposed shared helper, and each maximal run of
differing lines collapses to a `⟨param N⟩` placeholder. Output: "extract a shared
helper · K shared lines · N parameter(s) vary" + the skeleton. Turns "these N sites
are similar" into "extract this, parameterize these N spots" — the concrete next
action. Sharpest on function-level near-duplicates (few params); coarse on whole-
file clones. Line-granularity is the pragmatic version; token-level anti-unification
(the §Y prototype) would sharpen the placeholders to the exact differing values.

## AG. Lowering closure — every language to non-ERROR Raw ≤0.5% (goal met)

§AA drove the worst offenders down but left a clear, measurable target: per
language, **non-ERROR Raw ≤0.5%** with **no single construct >0.3%** of that
language's nodes. ERROR (tree-sitter parse failures, corpus-driven by adversarial
fixtures) is the irreducible floor and excluded — `bench/lowering_gaps.py` is the
dashboard that computes `(raw − ERROR)/nodes` per language and lists every
construct over the 0.3% gate (the work queue, biggest-first).

The recurring root cause across languages is the same as §AA's C finding: the
`lower_expr` fallback recurses children as expressions, so any *statement-* or
*type-level* node reaching expression position falls to Raw. Two disciplines close
it: (1) route stray statement kinds (`block`, `expression_statement`, switch-rule
bodies) back through the block/stmt path; (2) **erase type-level nodes to
`empty_block`** rather than Raw — generics, type arguments, primitive/array/pointer
types, parameter lists, `where`/throws clauses, annotations carry no behavior.

- **JS 1.29% → 0.18%**: `labeled_statement` → lower the inner statement (drop the
  label); `html_comment` → erase (comment arm).
- **Rust 0.64% → 0.01%**: `generic_function`/`unsafe_block` → unwrap child; the full
  type grammar (`type_arguments`, `reference_type`, `generic_type`, `where_clause`,
  lifetimes, …) → erase; `function_signature_item`/`associated_type` → drop as items.
- **Ruby 9.6% → 0.25%**: heredocs → string lowering; `begin`/`do` → `Try` (body +
  rescue/ensure/else handler blocks); splat/block/hash-splat → unwrap; `yield`/
  argument-list/range → `Seq`; `super`/`forward_argument` → `Var`; `alias`/`undef`
  dropped.
- **C 2.75% → 0.24%**: `goto` → `Break`; `#if`/`#ifdef`/… → lower guarded body as a
  Block (drop the condition); `offsetof(T,m)` → int literal (compile-time constant,
  like sizeof); `a, b` comma → `Seq`; enum constant → its value; designated-init
  pairs/designators handled; declarator + storage/qualifier + macro-body nodes erased.
- **Java 2.78% → 0.20%**: `Foo.class` → `Field("class")`; `super` → `Var`;
  `x instanceof T` → the runtime value (type erased); `this(…)`/`super(…)` → `Call`;
  `new int[n]` keeps the size expr (`dimensions_expr` → inner) while empty
  `dimensions`/generics/annotations/modifiers/throws erase; method references,
  switch labels (→ matched value), enum constants, stray block/expr-stmt handled.

**Result: all 9 languages OK** — non-ERROR Raw 0.01%–0.25%, no construct >0.3%:
Ruby 0.25 · C 0.24 · Java 0.20 · JS 0.18 · Go 0.16 · Python 0.15 · TS 0.07 · HTML
0.01 · Rust 0.01. jscpd-superset coverage is unchanged (coverage-neutral: the work
removes Raw noise, it doesn't move detection), and all cargo gates stay green. The
remaining Raw is now essentially all ERROR — a floor we can't lower without fixing
tree-sitter's parse of adversarial fixtures, which is out of scope.

## AH. The two-axis principle — why "find similar" and "be rigorous" don't conflict

A design reckoning, not an experiment. The product's core requirement — *find code
that is behaviorally the same even when it doesn't look the same* — appeared to
conflict with the push for rigor (the synth `DISCRIMINATION` gate merging off-by-one
and wrong-operator near-misses as clones). It does not. The requirement is the
product; dropping it makes nose a Type-1/2 detector, i.e. `dry` minus the
structural insight. The conflict was an **architecture smell**, not a goal clash.

Two distinct things were conflated:

1. **Two purposes under one threshold.** "Find similar" quietly serves two consumers
   with opposite tolerances. *Refactoring/DRY* ("these N sites do the same thing,
   extract a helper") wants high recall and is glad to see near-misses — a human
   glances and dismisses. *Behavioral-equivalence assertion* ("these are provably the
   same computation") wants high precision and must reject off-by-one. The synth gate
   tests the second; `--proposal`/§AF serves the first. One global threshold cannot
   satisfy both.

2. **Two kinds of difference under one scalar.** A single similarity score (Jaccard
   over shape features) blurs *representation* differences (names, statement order,
   sugar, loop form, commutative reorder — Type-4 should ignore these) and *behavioral*
   differences (`+` vs `*`, `>=` vs `>`, the constants and control flow that change
   results — Type-4 must never ignore these) into the same "fewer shared features."
   On a scalar, the recall/precision trade-off is then **unavoidable**: loosen the
   threshold to catch fuzzy Type-4 and you also catch the off-by-one bug, because both
   differ "a little" on the same axis.

The resolution — and it is what nose exists to do — is to **separate the axes**:

- **Representation differences → absorbed by *exact* canonicalization.** alpha-rename,
  GVN/value-graph, commutative sort already do this; the goal is to push *more* variation
  into making equivalent code *byte-identical* after normalization (Type-1 post-norm),
  so the fuzzy layer has less to blur.
- **Behavioral differences → measured *strictly* on the residual.** An operator swap
  is not a near-miss; it is a different program. Literal abstraction erasing the
  constant that distinguishes a guard, or alignment not penalizing an operator swap,
  is the bug — not "too much tolerance."
- **Output graded, not binary.** Emit "canonicalizes to the same" vs "structurally
  similar, differs *here*" (the §AF diff already gestures at this) and let the
  *consumer* pick the cut, instead of a global threshold forcing one tolerance.

This reframes the standing failures. The §AB "precision was mostly noise" and the
re-rank's dev→heldout generalization failure are symptoms of measuring on a conflated
scalar. The synth `DISCRIMINATION` FPs are over-blur on the behavioral axis; the
family-collapse 0/8 (measured this session at the shipped threshold 0.86 — Type-4
transformation recall is **0/17** there) is under-canonicalization on the
representation axis. The same root, opposite ends.

The deeper point: `dry` concluded after 160 experiments that token-set similarity is
exhausted and the lever is a *discriminative structural signal* — one exact on
behavior while invariant to form. A single fuzzy threshold is exactly the wall `dry`
hit. So **rigor is not in tension with the thesis — rigor is the thesis.** Hardening
the substrate is what *enables* tolerance: the more exactly representation variants
collapse to identical, the farther-apart forms can safely be called the same.

This principle drives the next two work items: (#1) an honest **two-axis evaluator**
that scores representation-convergence and behavioral-separation *separately*, on
author-certified ground truth, sidestepping the gate/threshold/cluster confounds; and
(#3) a **value-graph with a loop-recurrence normal form** so loop-carried computations
(sum/product/dot-product across loop↔reduce↔recursion) actually collapse — the
representation axis — while the behavioral residual stays strict.

## AI. The two-axis evaluator + value-graph reduction normal form (#1, #3 — first increment)

**#1 — the instrument (`nose features` + `bench/value_convergence.py`).** §AH is only
actionable with a metric that separates the two axes. The detector's normal path
(LSH gate → threshold → union-find) confounds "did the signal converge?" with "did
the pipeline surface it?", so the dashboard reads the **fingerprints directly**:
`nose features` dumps each unit's value-graph / shape / return multisets, and the
script computes, over the author-certified synth families, value-Jaccard for
*equivalent* pairs (REPRESENTATION axis, want →1.0) vs *near-miss negatives* (BEHAVIOR
axis, want →0.0), the MARGIN between the two clouds, and a threshold-free
RANK-SEP = P(an equivalent pair outscores its family's negatives).

The baseline was damning and clarifying: **representation 0.25, behavior 0.57, margin
−0.32, rank-sep 18%.** The signal was *inverted* — near-miss bugs looked **more**
similar than true Type-4 equivalents, because the value graph was invariant to the
operators/constants that define behavior (an off-by-one shares almost the whole graph)
but sensitive to the structure that should be ignored (a loop and a `reduce` share
almost nothing). Exactly §AH's two failure modes, one signal.

**#3 — loop-recurrence normal form (value graph).** The root cause on the
representation axis: `process_loop` havoc'd every loop-carried variable to an opaque
`Loop(cid)` and processed the body on a throwaway env, so a var-assignment (which
creates no sink) meant the reduction `total = total + f(x)` **never reached the
fingerprint** — only the bare opaque accumulator did. Two changes:

1. **Thread the recurrence.** Seed each carried variable with a symbolic
   previous-iteration value, process the body, and write the resulting update back to
   the post-loop env so it reaches the sinks.
2. **Canonical reductions.** For a `for pat in iterable` loop, bind the pattern to a
   canonical `Elem(iterable)` value (so `x*x` converges across accumulator names), and
   recognize accumulator updates `acc = acc ⊕ contrib` (⊕ associative-commutative) as
   a canonical `Reduce(⊕, init, contrib)` node. The per-element `contrib` keys the
   value, so a sum-loop and a product-loop — or `a[i]*b[i]` vs `a[i]+b[i]` — stay
   distinct (behavior axis preserved). Non-reductions fall back to the threaded
   recurrence.

**Indexed-`while` induction variables.** A `while i < len(xs) { … xs[i] …; i += 1 }`
hides its element behind index bookkeeping, so it didn't match a `for x in xs`. Now
the value graph detects induction variables (`i = i ± constant`), reads the iterable
from the `i < len(xs)` guard, binds a canonical `Elem(xs)`, and rewrites every
`xs[i]` → `Elem(xs)` throughout the loop body's sinks and recurrences (so it works
even when the accumulation is conditional — a filter+reduce — not a clean fold), while
dropping the induction variable as iteration mechanics. An indexed `while` and the
equivalent `for`-each now produce *identical* fingerprints (`while↔for` 0.16 → **1.00**,
even for the filtered sum-of-squares). This is high-value beyond the synth pair:
C/Go/Java/JS indexed `for` loops desugar to exactly this shape.

Measured effect (cumulative): behavior axis **0.57 → 0.42** (near-misses separate
better, the recurrence adds discriminative structure), representation **0.25 → 0.29**,
margin **−0.32 → −0.13**, rank-sep 18% → 24%, loop↔reduce 0.02 → 0.16,
recursion↔iteration 0.00 → 0.12, while↔for 0.22 → 1.00. The signal is still inverted
overall — the remaining drag is the HoF forms (`sum(gen)`, `reduce(f,xs,0)`,
comprehensions) lowering to *opaque* calls (`sum`/`reduce` aren't recognized builtins),
so none reaches the canonical `Reduce`. That is the clearly-scoped next increment.

**The §AH two-mode split, forced by a test.** Making the value graph behaviorally
precise immediately broke a candidate-mode test asserting a sum-loop and a product-loop
are one refactoring family — precisely §AH's prediction that you cannot sharpen
behavioral precision without giving the recall-oriented mode a structural fallback. The
fix made the modes' scoring weights diverge: **strict mode trusts the value graph
(behavioral); candidate/refactoring mode is structure-dominant** (shape-weighted), so
two units sharing a skeleton but differing in a behavior-defining operator still
surface for human review. §AH is now in the code, not just the doc. Re-validated:
labelset dev precision@10 59% / recall 97% and jscpd-superset 71.4% both unchanged,
all 108 cargo tests green.

**HoF→Reduce unification (third increment).** The bulk of the remaining gap was the
HoF forms reaching the canonical `Reduce`. Four coordinated changes: (1) a frontend
fix — `sum(x*x for x in xs)` was mangled to `call(sum, x*x, xs)` (the generator binding
dropped); a bare generator argument now lowers as a comprehension (`HoF(Map)`); (2)
`sum`/`reduce` recognized as reduction builtins in the idiom table; (3) the value graph
evaluates `HoF(Map)[xs, λ]` by binding the lambda's parameter to a canonical `Elem(xs)`
and unfolding its body to the per-element contribution, recognizes `sum`/`reduce`
builtins (unfolding a `reduce` lambda over `Elem` + an accumulator marker via the same
`as_reduction` used for loops), and emits the *same* `Reduce(op, init, contrib)` a loop
produces; (4) `Elem` carries the collection as an argument (not just folded into its
key), so the collection is reachable from the fingerprint identically for a loop, a
`reduce`, and a `sum(map)` — letting the loop drop its now-redundant iterable `Cond`
sink. Result: a product loop ≡ `reduce(λa,b. a*b, xs, 1)` and a sum-of-squares loop ≡
`sum(x*x for x in xs)` produce **identical** fingerprints (locked by tests).

Cumulative dashboard: representation **0.25 → 0.35**, behavior **0.57 → 0.41**, margin
**−0.32 → −0.06**, rank-sep 18% → 29%, loop↔reduce 0.02 → 0.43, while↔for 1.00.

A second §AH datapoint: sharpening the value graph again dropped a sum-loop/product-loop
pair below the candidate-mode threshold — these are now genuinely distinct reductions,
so they make a poor "near-duplicate family" fixture. The `--diff` test was repointed at
a pair differing in a *literal* (a true near-duplicate); the principle held, the test
got more honest. Re-validated neutral: labelset dev 59% / recall 97%, jscpd 71.4%, 112
tests green.

**Guarded (filtered) reductions — the margin flips positive (fourth increment).** Most
synth families are deliberately *filtered* (`if x%2==0: acc += …`), which merges to a
`Phi(cond, acc⊕contrib, acc)` — not a clean fold. `as_reduction` now recognizes this
guarded shape and canonicalizes it to `Reduce(⊕, init, cond ? contrib : identity)` (the
filtered-out element contributes the operator's identity — 0 for `+`, 1 for `*`), so a
filtered for-each and the equivalent filtered indexed-while converge (locked by a test).

This was the lever: **MARGIN −0.06 → +0.01 — the signal is no longer inverted.** Across
the four increments the dashboard moved representation **0.25 → 0.41**, behavior **0.57
→ 0.39**, margin **−0.32 → +0.01**, rank-sep 18% → 35%, loop↔reduce 0.02 → 0.69,
while↔for 1.00. Equivalent pairs now outscore near-miss negatives on average — the §AH
goal direction. Re-validated neutral throughout: labelset dev 59% / recall 97%, jscpd
71.4%, 113 cargo tests green.

**Filtered comprehensions (fifth increment).** The comprehension side now lowers
`if`-clauses: `[body for x in xs if cond]` → `HoF(Map)[HoF(Filter)[xs, λx.cond], λx.body]`.
The value graph unwraps a `Map`-over-`Filter` so the collection and the predicate share
one `Elem`, emitting `Hof(Map, [contrib, pred])`; `sum` of a filtered map then guards the
contribution — `Reduce(Add, 0, pred ? contrib : 0)` — the *same* canonical value the
guarded loop produces. So `sum(x for x in xs if x>0)` and `if x>0: t += x` converge (the
comprehension's fingerprint is a strict subset of the loop's — the loop additionally
records the guard as a branch-condition sink; locked by a containment test).

Dashboard: idiom_comprehension 0.23 → **0.63**, idiom_gen 0.15 → **0.91**, representation
**0.41 → 0.51**, margin **+0.01 → +0.12**, rank-sep 35% → **53%**. Cumulative from the
inverted baseline: representation 0.25 → 0.51, behavior 0.57 → 0.39, margin −0.32 →
+0.12, rank-sep 18% → 53%. Neutral throughout: labelset 59% / 97%, jscpd 71.4%, 114 tests.

**Selection reductions + abs idiom (sixth increment).** `min`/`max` are selection
reductions: `as_reduction` recognizes the loop pattern `if cand {>,<} acc: acc = cand`
(= `acc = max/min(acc, cand)`) and `min(gen)`/`max(gen)` builtins, emitting a canonical
selection `Reduce` that — unlike a `+`/`*` fold — carries *no* init, so it ignores the
loop's incidental seed. This was inert until `abs` was also canonicalized: `abs(x)` (a
builtin) and the `x if x>=0 else -x` ternary both lower to `Un(Abs, [x])`. Together they
made the `max(abs(x) …)` family converge — including the `continue`-guarded variant
(`cfg_reshape_continue` 0.18 → **1.00**). `max` and `min` stay distinct (behavior axis).

Dashboard: idiom (max-abs) 0.23 → 0.48, cfg_reshape 0.18 → 1.00, representation 0.51 →
**0.59**, margin +0.12 → **+0.17**, rank-sep 53% → **59%**. Cumulative from the inverted
baseline: representation 0.25 → 0.59, behavior 0.57 → 0.41, margin −0.32 → +0.17,
rank-sep 18% → 59%. Neutral throughout: labelset 59% / 97%, jscpd 71.4%, 116 tests.

**Iteration-idiom long tail (increments 7–9).** Three more closed the common
iteration idioms by unifying how indices and elements are modeled:

- **Generalized index iteration.** `C[idx]` for *any* index-role `idx` (a `while`
  induction var, or the iterate of `for i in range(len(…))`) rewrites to `Elem(C)` for
  *any* collection `C` — a structural `rewrite_indices` pass replacing the old
  exact-pair `xs[i]` rewrite. This unifies value iteration, while-indexed, range-indexed,
  and multi-collection `a[i]*b[i]` loops. A Python indexed-while and a JS C-style for now
  converge exactly (`crosslang_js` 0.59 → 1.00 — cross-language convergence, the project
  thesis, for indexed loops).
- **`zip`.** A tuple pattern `(x, y)` over `zip(a, b)` binds `x→Elem(a)`, `y→Elem(b)`, so
  `sum(x*y for x,y in zip(a,b))` ≡ the indexed dot product (`idiom_zip` 0.20 → 1.00).
- **Canonical iteration index `Idx(C)` + `enumerate`.** `range(len(C))`, indexed `while`,
  and `for i,x in enumerate(C)` all bind their index variable to one canonical `Idx(C)`,
  so the *returned index* and the element both converge: `for i,x in enumerate(xs): if
  x>t: return i` ≡ `for i in range(len(xs)): if xs[i]>t: return i` (`idiom_enumerate` 0.24
  → 1.00).

The returns axis was also added to the dashboard, which proved the remaining gap is
*representation*, not behavior: near-miss negatives are well separated (return-J equiv
0.73 vs neg 0.29), so no return gate closes it — only convergence does.

**Final state of the §AH/§AI campaign.** Across nine value-graph increments the honest
dashboard moved **representation 0.25 → 0.73, behavior 0.57 → 0.39, margin −0.32 →
+0.34, rank-sep 18% → 76%** — the signal went from *inverted* (near-miss bugs scored
above true clones) to strongly correct. Ten of twelve transformation families now
converge ≥0.90 (most exactly 1.00), each locked by an equivalence test; all neutral on
the labelset (dev P@10 59% / recall 97%) and jscpd (71.4%), 119 cargo tests green.

The remaining rank-sep gap (4/17) is the genuine frontier and is left deliberately:
`idiom_next` (`next((i for … if p), default)` — a Python-specific first-match-with-default
with early-exit control flow) and `recursion_iteration` (loop↔recursion, **explicitly out
of v1 scope** as a meaning-risking rewrite). Both are contrived synth stressors, not
common real-code duplication. Beyond them the standing broad lever is unchanged: an
alignment weighting the discriminative return-sinks, toward rank-sep → 100%.

## AJ. Behavioral oracle — verifying the value graph is *sound* (no false merges)

§AH/§AI improved the value-graph fingerprint on a synthetic dashboard, but nothing
*verified* its core claim: that two units with the same fingerprint actually compute
the same thing. After nine increments of aggressive canonicalization (reductions,
`Elem`/`Idx`, `abs`, `zip`, `enumerate`, guarded folds, selection), the risk of a
*false merge* — calling two behaviorally-different functions clones, the cardinal sin
of a clone detector — was real and unchecked. This builds the verifier.

**The oracle (`crates/nose-normalize/src/interp.rs`).** A small, deterministic
interpreter over the normalized IL: `Int`/`Bool`/`Str`/nested-`List` values, all loop
shapes, if/ternary, the modeled builtins (`len`/`sum`/`min`/`max`/`abs`/`range`/`zip`/
`enumerate`/`reduce`), `HoF` map/filter/reduce, and lambdas (incl. tuple-destructured
params over `zip`/`enumerate` pairs). It is intentionally *partial*: any unmodeled
construct (opaque call, field access, exception) makes the whole unit uninterpretable
and it is excluded — never guessed. A step budget guarantees termination. It need not
match any language exactly, only be self-consistent: a genuinely-equivalent pair agrees
under *any* consistent semantics, so a fingerprint merge the interpreter contradicts is
a real bug.

**The checker (`nose verify`).** For every interpretable function it runs a fixed
battery of 16 input vectors → a behavior signature (return value + effect trace), groups
units by value fingerprint, and asserts **fingerprint-equal ⟹ behavior-equal on every
input**. Run on the 62-repo corpus: **15,317 interpretable units, 510 fingerprint
groups — and it caught 2 soundness violations** the synthetic dashboard never could:

1. **Path-insensitive returns.** `if c {return A} else {return B}` and the
   branch-swapped `if c {return B} else {return A}` produced the *same* fingerprint —
   the two `Return` sinks formed an order-insensitive multiset. Fix: returns/throws are
   now tagged with the **path condition** (the conjunction of branch conditions in
   effect), so a value returned under `c` differs from the same value returned under
   `¬c`. A genuine bug — and common (any two-way conditional return). Bonus: it also
   sharpened the behavior axis (near-miss return-J 0.29 → 0.17, margin 0.43 → 0.54).
2. **Duplicate-parameter collapse.** `f(a,a){return a}` (alpha-rename gives both params
   one cid) fingerprinted identically to `transformRequest(data){return data}` though it
   returns the *second* argument. Fix: seed parameters by **position**, not cid (for
   well-formed code position == cid, so this is a no-op except for the degenerate case).

After both fixes: **SOUND — 0 violations** across all 15,317 interpretable units.
Re-validated neutral/up: labelset dev P@10 59% / recall 97%, jscpd 71.5%, 120 tests
(incl. a branch-swap divergence regression test).

This is the rigor the §AH thesis demands made *checkable*: the value graph is now a
verified-sound behavioral fingerprint on everything the oracle can interpret, not a
trust-me one. The oracle also generalizes evaluation beyond the 8 hand-built synth
families to thousands of real functions — and is a permanent regression gate (`nose
verify bench/repos` must stay SOUND). Open frontier unchanged: completeness (the
under-merge direction) and the remaining contrived idioms.

**Completeness (the under-merge direction).** The same oracle, run the other way:
group interpretable units by *behavior* (over a combinatorial input battery — full
cross-coverage of the value pool for arity ≤ 2, so a 2-arg comparison sees `a<b`/`a>b`/
`a==b`), restrict to *non-trivial* behaviors (the return varies and isn't uniformly
`Err`/`Null`), and ask whether behavior-equal units also share a fingerprint. On the
corpus: **~65% of non-trivial behavior-equal pairs converge** (≈22 under-merged groups), after hardening the battery (below).

Unlike soundness, this is a *noisy lower bound*: behavior-equal on a finite battery is
necessary-not-sufficient for equivalence, so the misses mix real and coincidental.
Triage of examples bears this out — `getFdSpecificValue` vs `getStripFinalNewline`
agree only because their string-keyed ternaries (`fdNumber === 'ipc'`/`'all'`) are never
triggered by the (string-less) battery, a blind spot; whereas `lastChar(str){return
str[len(str)-1]}` and a stack's `top(self){return self[-1]}` are a *genuine* missed
clone (both return the last element — the value graph doesn't canonicalize `s[len(s)-1]`
to `s[-1]`). So the asymmetry is fundamental: **soundness violations are proofs (every
one a real bug); completeness misses are leads (some real, some battery artifacts).**
The instrument is a permanent regression gate for soundness and a lead-generator for
recall (missed Type-4 idioms like len-relative indexing); chasing the leads is the same
diminishing-returns idiom long tail as §AI, deferred.

**Hardening the completeness battery.** The first completeness pass was polluted by
*coincidental* agreement — two different functions agreeing on a weak battery. Two
fixes made the signal trustworthy without touching the (already sound) value graph:
(1) **literal probes** — mine the string/int constants the corpus actually branches on
and inject each at every input position, so a value-keyed branch (`fdNumber === 'ipc'`)
is exercised rather than always falling through; this dissolved the
`getFdSpecificValue`/`getStripFinalNewline` coincidence. (2) **field-aware effects** —
a store records *what* it writes to (field symbol / index), not just the value, so
near-twin setters (`self.a = x` vs `self.b = x`) — which the value graph correctly
distinguishes — stop registering as behavior-equal. (Subtlety relearned twice: the
battery must have a *fixed row count* across arities, else fingerprint-equal units of
different arity get unequal-length behavior vectors and false soundness violations; the
inputs are a fixed width and a function binds only its first `arity` of them.) Net:
still SOUND, completeness a cleaner 65%. The residual misses are genuine leads (e.g.
`s[len(s)-1]` ≡ `s[-1]`) plus branches on uncommon literals the probes don't cover —
diminishing, and the recall fix is the deferred idiom long tail.

## AK. Wiring the verified value graph into detection (the soundness payoff)

Nine §AI increments improved the value-graph fingerprint, yet every real-code metric
(labelset, jscpd) stayed flat — and the synth `detect` gate caught **0/8** Type-4
families at the shipped threshold. The value-graph work was *stranded*: the detector's
score is `wv·vj + ws·sj + wr·ransac`, half of it (shape + RANSAC) *syntactic*, and a
true Type-4 clone (loop ≡ reduce ≡ comprehension) is syntactically dissimilar — so the
syntactic terms drag the blend below threshold no matter how well `vj` converges.

The §AJ oracle unlocked the fix. Because `nose verify` proved **identical value
fingerprints ⟹ behaviorally equal** (0 false merges across 15k units), the detector can
*trust an exact value-fingerprint match outright* and accept it regardless of syntax (a
one-line fast path, guarded by a minimum fingerprint size). This is the safest possible
way to lean on the value graph — only exact, oracle-certified matches — yet it is the
first change all session to move the real numbers:

- synth Type-4 recall **0/17 → 3/17** (the exactly-converged `while↔for`,
  `enumerate↔range`, `continue`-filtered families now detected) with discrimination
  still **0/8** false positives;
- **jscpd-superset 71.5% → 72.4%** (~118 more real duplicates surfaced);
- **labelset dev precision@10 59% → 62%**, worthy-recall 97% (more worthy families,
  no precision loss at @10).

The lesson is the thesis closing on itself: a *verified-sound* semantic signal can be
trusted *aggressively*, and that trust is what converts representation convergence into
real detection. The partial-`vj` Type-4 clones (`loop↔reduce` at 0.69, etc.) still need
the diluted blend and remain uncaught — accepting near-(not-exact) matches would catch
them but is not oracle-certified, so it is left as measured future work (it needs a
strict-mode real-code precision benchmark first).

**Calibration: how far can the value graph be trusted?** The fast path accepts only
*exact* fingerprint matches. Is near-match safe too (to catch partial-`vj` Type-4 like
`loop↔reduce` 0.69)? The oracle answers it directly — bin interpretable pairs by
value-Jaccard and measure `P(behavior-equal | vj)`:

| vj band | behavior-equal |
|---|---|
| **1.0 (exact)** | **100%** (347,513 pairs) |
| [0.9, 1.0) | (sparse) |
| [0.8, 0.9) | 75% |
| [0.5, 0.8) | ~82% |

A sharp cliff: exact equality is 100% sound (definitive — a third of a million sampled
pairs), but even high-but-inexact `vj` is only ~75–82% behavior-equal (≈1 in 5 a false
merge). So **exact match is the trustworthy ceiling**; the committed fast path is the
oracle-calibrated operating point, and near-match acceptance would trade ~20% precision
for recall — wrong for behavioral detection (the §AH behavior axis). The remaining
partial-`vj` Type-4 clones must be caught by *raising* their `vj` to exact (more
canonicalization), not by *lowering* the threshold to admit them. Calibration is a
permanent `nose verify` diagnostic that both justifies the threshold and would flag
any future regression (if exact-match precision ever drops below 100%).

**Path-captured conditions → if-assign ≡ ternary (and the standalone `Cond` sink
retires).** With returns/effects now path-guarded and variable merges captured by
`Phi`, an `if`'s condition is already present wherever it matters — so the standalone
`Cond` sink it used to emit became redundant *and* harmful: it made a statement-`if`
(`if c { x = a }`) diverge from the equivalent ternary (`x = a if c else x`), which has
no such sink. Dropping it (and path-guarding effects so a conditional `append`/store
still carries its condition) converges the two — a common real refactoring. The oracle
verified the change introduced **no false merge** (still SOUND across 15k units — the
exact use the soundness gate was built for), real metrics held (labelset P@10 62%,
jscpd 72.3%), and dashboard convergence rose (representation 0.73 → **0.78**, margin
+0.34 → **+0.41**; `reorder_ternary` and `idiom_gen` reached exact 1.00, `idiom`/
`idiom_comprehension`/`loop_reduce` all up). `continue`/`break` filters keep their
condition via cfg-norm → guarded reduction, so nothing is lost.

## AL. Closing the jscpd-superset recall gap (72% → 92%)

nose aims to be a superset of jscpd-weak's contiguous copy-paste detection. The
guardrail had sat at ~72% coverage for the whole project. A miss-classification across
the 62-repo corpus showed the gap was three concrete causes, fixed without gaming (the
labelset's top-N precision is the no-gaming guard — it *rose* throughout, because the
new low-value matches rank below the user-facing cut):

1. **Top-level `#if`-wrapped C functions** lowered to an empty module (the collector
   didn't descend into `preproc_if`) → functions invisible. Recurse into preprocessor
   conditionals. (nginx 71→94%.)
2. **TypeScript type/interface/enum declarations** were erased to nothing, so
   duplicated generated `.gen.ts` type files were invisible. Lower type *declarations*
   (not annotations) to a structural skeleton. (trpc 72→85%.)
3. **Import/`#include`/`use`/`package` blocks** (54% of all misses!) carried no tokens,
   so the contiguous channel couldn't see duplicated import blocks. A shared
   `import_tokens` helper emits their identifier/string leaves across all frontends.
   They form no unit and rank near-zero. (mockito 66→87%, retrofit 75→92%.)
4. **Contiguous floor** (20 tokens / 4 lines) sat above jscpd's granularity; lowered to
   10/3 (k-gram size is the hard floor) to match it.

Result: jscpd-superset **72% → 92.3%** (gate passes ≥90%). Validated throughout: oracle
SOUND (the import/type units are non-behavioral, excluded), labelset dev precision@10
**59% → 69%** and worthy-recall **97% → 99%** (the recall work surfaced *more* worthy
families without diluting the top-N), all 9 languages still ≤0.5% non-ERROR Raw, 121
cargo tests green. The remaining ~8% is jscpd's own noise floor (comment blocks,
sub-`k`-token fragments) that nose legitimately doesn't rank as duplication.

## AM. Quantifying value-add over jscpd — the behavioral oracle as judge

`jscpd_superset` proved nose ⊇ jscpd (92% coverage). The converse — what nose finds
that a token detector *cannot* (renamed Type-2, behavioral Type-4, cross-language) — is
the product's reason to exist, and was unmeasured. The hard part is judging "is this
*really* a clone?" without circularity (nose judging nose). The §AJ behavioral oracle
is exactly that independent judge: two functions are clones iff they compute the same
thing on a battery of inputs.

`bench/value_add.py` (+ `nose verify --json`): over interpretable functions, GOLD
clone pairs = pairs with identical, non-trivial behavior. Each tool (jscpd, nose) gets
connectivity over those units; a gold pair is "detected" iff the tool links spans
covering both. Both scored on the *oracle's* gold, equal footing.

**The size gate matters enormously.** A first pass found GOLD = 7391 pairs, nose
value-add +24% — but 97% of it was 1-line test fixtures (`function foo(x):any{return
x}`) that *aren't* meaningful clones and that both tools rightly ignore. Restricting the
gold to the detector's meaningful-size floor (≥5 lines, ≥24 IL tokens) collapsed it to
**211 real pairs** — the honest population.

**Baseline (62-repo corpus, meaningful-size interpretable functions):**
- jscpd recall **90.0%** (it's a Type-1 detector; the 90% is identical/near-identical copies, dominated by trpc's 190 generated-code duplicates),
- nose recall **95.7%**,
- **VALUE-ADD RECALL: of the 21 genuine clones jscpd misses, nose recovers 12 = 57.1%** — the renamed / restructured / cross-form clones token matching can't see,
- **behavioral precision 100%** (the no-gaming guard: every interpretable pair nose clusters is truly behavior-equal — recall wasn't bought with noise).

Per-repo outside trpc the split is stark: where clones are Type-1 (trpc) jscpd already
gets them (no add to be had); where they're Type-2/4 (marshmallow, radash, clap, regex,
poetry → jscpd 0%, nose 100%; netty/commons-lang ~50%) the value-add is the whole
signal. The honest limitation: the oracle only judges *interpretable* (numeric/list)
functions, so this is a small, trpc-heavy slice (21 jscpd-missed pairs) — value-add on
classes/string/IO logic is real (the renamed-function demo: jscpd 0, nose 1) but not
objectively measurable without a clone oracle for non-numeric code.

The metric is the goal: **drive value-add recall up (recover more of what jscpd misses)
while behavioral precision stays ~100%** — a target with the no-gaming guard built in.

**Controlled per-Type benchmark (`bench/clone_bench.py`).** To enlarge the gold beyond
the oracle's interpretable slice and cover the value-add *types* with author-certain
ground truth, a controlled set: meaningful-size base functions across 8 algorithms,
each with clone variants tagged T1 (exact, control), T2 (renamed), T4 (behavioral
restructure), XL (cross-language JS). jscpd's floor is lowered to the fixtures'
granularity so it isn't gated out on the control (this only *helps* jscpd; remaining
misses are pure capability). Both tools scored by base↔variant linkage.

Result (31 pairs):

> **Superseded by §AN.** This 8-algorithm, JS-only-XL, no-T3 / no-negatives table was
> too thin to trust per-type. §AN rebuilt it as 742 controlled fixtures across 8
> languages with T3 and negatives — read the current per-type numbers there (held-out:
> T2 ~90%, T3 ~84%, XL ~35%, T4 ~33%). Kept below as the first datapoint.

| type | jscpd | nose | |
|---|---|---|---|
| T1 exact (control) | 88% | 100% | both catch copies ✓ |
| **T2 renamed** | **0%** | **75%** | token matching can't; alpha-rename can |
| **XL cross-language** | **0%** | **100%** | jscpd is single-language by construction |
| T4 behavioral | 14% | 14% | **nose's weak spot** |

This sharpens the picture the oracle gave: nose's value-add is **strong and clear on
renamed (T2) and cross-language (XL)** — the cases token detectors fundamentally cannot
do — and **weak on compressed behavioral variants (T4)**: a `sum(v*v for v in xs if
v>0)` clone of a 6-line loop is both below the detector's size gate and only partially
convergent (the filtered-comprehension ↔ filtered-loop fingerprint is a subset, not
exact — §AI). T4 is the measured improvement target. Combined with the oracle's
value-add recall (57% on the interpretable slice, 100% precision), this is the robust,
multi-faceted value-add baseline.

## AN. Scaling the controlled benchmark — 8 languages, T3, and the two-axis precision guard

The §AM controlled set was 8 algorithms, JS-only XL, no T3, no negatives — too thin to
trust per-type. Rebuilt it properly with a `clone-benchmark-fixtures` **workflow** (8
parallel per-language authoring agents): **8 languages** (python, js, ts, java, ruby,
rust, go, c) × **10 algorithms**, each with `base`, `t2` (renamed), `t3[]` (gapped:
insert/remove/modify), `t4[]` (behavioral restructure), and `neg[]` (near-miss
NON-clones — the precision guard). XL pairs are formed across all language pairs for the
6 shared canonical algorithms. **742 fixtures, 671 clone pairs, 0 dropped** (every
fixture parses; an authoring typo penalizes neither tool). dev/held-out split by
canonical name + own-algo index parity; improvements made on dev, goal read off held.

**Why T3 and negatives were missing, and why they matter.** T3 (gapped) is jscpd's
actual failure mode in the wild (a copy with one inserted line breaks its exact run), so
omitting it understated the value-add. Negatives are the *no-gaming* spine: without a
"must NOT match" population, recall can always be inflated by lowering threshold. The
workflow authored negatives as **single-operator behavioral near-misses** — `>`→`!=`,
`>`→`>=`, `+=`→`*=`, `+=`→`-=`: structurally near-identical, behaviorally different.
This is exactly the §AH behavior axis, and it exposed a real split the §AM set couldn't.

**The benchmark must report both §AH axes** — nose is not one detector:
- **candidate axis** (`scan`, structure-weighted, copy-paste floor on) — surfaces
  near-identical families worth consolidating. High representation recall; a behavioral
  near-miss is a *valid* candidate, so it is expected to link here.
- **behavioral axis** (historically `scan --mode behavior --no-contiguous`; current
  exact scan surface is `scan --mode semantic`, and fuzzy behavioral benchmark runs live
  on the hidden `detect` path) — "do these compute the same?". The precision guard is
  read off this axis.

**Baseline (held-out):**

| type | jscpd | nose candidate | nose behavioral |
|---|---|---|---|
| T1 exact (control) | 30% | **100%** | 90% |
| **T2 renamed** | 0% | **90%** | 90% |
| **T3 gapped** | 11% | **86%** | 58% |
| T4 behavioral | 22% | 33% | 25% |
| **XL cross-language** | 0% | **35%** | 20% |

Headline value-add (candidate axis): **of 591 clones jscpd misses, nose recovers 338 =
57%** — and the per-type story is now crisp. Strong, clear value-add on **T2 renamed
(~86%)** and **T3 gapped (~81%)** — jscpd's two blind spots — and on **XL** (jscpd 0% by
construction). The **T4 weak spot is specifically functional idioms**: structural T4
(Go while↔for **80%**, C **53%**) converges, but comprehension/reduce/map/stream forms
do not (Python **10%**, Ruby **4%**, Rust **8%**, JS **14%**). That is the §AI long tail
made measurable across languages.

**The precision guard found a real two-axis leak.** On the behavioral near-miss
negatives (lower = better):

> **Correction (see §AP):** the `61%` below was measured at the wrong threshold —
> the old refactor-path behavioral run used refactor's `0.70` *candidate* default,
> not the strict detector's calibrated `0.86`. At `0.86` the real behavioral-axis
> baseline is **25%**, not 61%. The table is kept as recorded; read §AP for the
> corrected number.

| | FP rate |
|---|---|
| jscpd | 0% (token runs below its floor) |
| nose candidate | 97% (expected — near-misses *are* consolidation candidates) |
| nose behavior, contiguous **on** | 97% (the leak) |
| nose behavior, no contiguous floor | **61%** [→ 25% at the calibrated 0.86; §AP] |

Two findings: (1) with the contiguous floor left **on**, the behavioral scorer
reports at sim 1.0 *un-gated* — a partial shared fragment around the one differing
operator links the files, so the strict behavioral claim was contaminated by a pure
representation signal (97%). Excluding it (the principled behavioral axis) drops FP to
61%. (2) Even then, **61% is too high for a behavioral precision guard**: structure
(0.3) + return (0.2) = 0.5 before any value signal, so a lenient value-graph similarity
on `Reduce(Add)` vs `Reduce(Mul)` (they differ only in the reduce op) crosses the 0.86
threshold. The *crisp* behavioral signal — exact value-graph equality, the §AJ
oracle — is precise (`value_add.py`: 100%); the **fuzzy 0.86 threshold blends the two
axes** and lets single-operator near-misses through. This is the next target: sharpen
the behavioral axis so it *recognizes* behavior-preserving restructures (T4 recall up)
while *discriminating* single-operator behavioral differences (negative FP down) —
moving the Pareto frontier, not trading one for the other (no gaming: T4↑ at neg-FP↓,
both validated on held-out).

## AO. Behavioral-axis increment 1 — the counting-loop induction misclassification

Chasing the §AN target (behavioral-axis neg-FP 61%→<20%, T4 held 25%→≥50%), the first
lead came from *where* the negatives passed: **91 of 97 were non-Python** (Java 20/20,
TS 14, C 13…; Python only 6). Dumping fingerprints (`nose features`) on a Java
`count_above` near-miss (`v > limit` vs `v >= limit`) showed the method's value
fingerprint was **2 atoms and byte-identical** across the operator change — the value
graph had collapsed the whole loop. With identical structure and a vacuous value signal,
no weight/threshold setting could separate them (a threshold sweep confirmed FP was flat
from 0.80 to 0.90 — the pairs were hitting the exact-match fast path, not the margin).

Bisecting by loop shape isolated it cleanly: Python `for i in range(len(xs))` (lowers to
`ForEach(Range(Len))`) distinguished the operator (9 atoms, jaccard 0.38), but Python
`while i < len(xs)` (lowers to `While`) **collapsed identically (2 atoms)** — same as
Java's C-style `for`, which also lowers to `While`. So it was *not* Java-specific; it was
the **`While` index-loop path**.

Root cause: that path calls `induction_vars` to find loop counters (`i = i ± const`) and
treats them as iteration mechanics — bound to the collection index, excluded from
reduction recognition. But a **counting accumulator** `count += 1` *is* `count = count +
1` — it matches the induction shape exactly. So `count` was misclassified as an index,
bound to `idx(xs)`, and never reached a `Reduce`; the entire accumulation evaporated.
(Sum loops were spared by luck: `sum += xs[i]` has a non-literal operand, so the
increment test already rejected them. Only constant-step counting loops collapsed.)

The fix is one principled rule: **a genuine loop counter both steps by a constant *and*
governs the loop condition.** Intersect `induction_vars(body)` with the variables the
loop condition mentions. The counter `i` (in `i < len`) survives; the accumulator
`count` (absent from the condition) is correctly left as an accumulator and folds to
`Reduce(Add)`.

This is a textbook §AH Pareto move — **one change lifts both axes**:
- *precision*: Java/Python-while `>` vs `>=` now 9 atoms, jaccard 0.38 (was identical);
- *recall*: a Java index-`for` counting loop and a Python `for v in xs` counting loop
  now produce **identical** fingerprints (convergence 1.00) — a true cross-language,
  cross-shape Type-4 clone the exact-match path now detects.

Verified: `cargo test` green (121 tests), clippy clean, and the §AJ soundness oracle
still reports **0 false merges** across arrow/radash/poetry/clap — the richer value graph
did not over-merge. On the held-out benchmark the behavioral-axis neg-FP fell **61% →
55%** (both at the mismeasured 0.70 threshold — §AP shows the calibrated-0.86 baseline
is 25%, and this fix is part of getting there). A real gain, but counting loops were
only one subset; the remaining 88 FPs are
broader value-graph completeness gaps (equality flips `==`/`!=`, `range` start offsets,
return-value swaps, and whole non-loop algorithms — Java still 20/20, its value graph
near-empty for if-chains and early-return search). Those are the next increments toward
the <20% / ≥50% target.

## AP. The threshold measurement bug + class-dilution fix — the real baseline is 25%, not 61%

Chasing the remaining Java FPs (20/20) surfaced a **measurement bug that had inflated the
whole behavioral-axis baseline.** Bisecting a Java `linearSearch` near-miss (`==` vs
`!=`) by threshold: at 0.86 the method pair is correctly rejected (value jaccard 0.57 →
score 0.78 < 0.86). Yet the benchmark reported it as a FP. The cause: the benchmark read
the behavioral axis through the old refactor-path behavioral run, whose `--threshold`
default was **0.70** (candidate mode) — so it ran the strict behavioral *gates* at the
*candidate threshold*. The strict detector's calibrated operating point is **0.86**.
Passing `--threshold 0.86` explicitly gave the honest behavioral axis. That fuzzy
behavior scan spelling was later removed; use `scan --mode semantic` for the exact
Type-4 surface, or the hidden `detect` path to reproduce this historical thresholded
benchmark.

Corrected, the §AN/§AO numbers move sharply: **behavioral-axis neg-FP 61% → 25%**, and
T4-strict held 25% → 18% (the higher threshold rejects partially-converged T4 clones too,
so recall tightens). The honest behavioral-axis baseline at the calibrated point is
**neg-FP 25%, T4-strict 18%** — much closer to the precision target than the mismeasured
61% suggested, and the recall gap is the real work. (The goal's metrics are restated on
this axis: T4-strict 18% → ≥50%, neg-FP 25% → <20%, both held-out.)

At 0.86, **Java was 20 of 40 FPs** — every Java negative, because a `class { method }`
wrapper's value fingerprint was **2 atoms and identical** across a deep one-operator
change: `process_stmt` has no `Func` case, so a method definition fell to the
opaque-effect branch and the class collapsed to a structural shell, accepted on structure
alone (`vj` 1.0 on two near-empty vectors). Fix: **a container's behavior is the
aggregate of its methods** — `build_unit` now descends into each contained `Func`,
processing its body in its own parameter scope so the method's returns/effects fold into
the container. The Java `count_above` class now fingerprints to 9 atoms and distinguishes
`>`/`>=` (jaccard 0.38); the pair no longer links. Java FPs fell **20 → 7**.

Verified: `cargo test` green, clippy clean, §AJ soundness oracle 0 false merges
(arrow/radash/poetry/clap) — aggregating methods did not over-merge. Aggregate held-out
neg-FP held at 25% (the Java −13 was offset by cross-fixture clustering shifts in the
global `scan` pass — many container fingerprints changed at once, re-bucketing LSH;
isolated Java/Go pairs verify as correctly rejected). The Java dilution was a genuine
correctness bug regardless of the aggregate. Remaining FPs are construct-level value-graph
gaps — comparison operators in non-loop `if`-chains, `range` start offsets, return-value
discrimination — the next increments.

## AQ. The size gate was the T4 recall blocker, not the value graph

The T4 strict recall was the far gap (18% held vs the ≥50% target). Diagnosing the
*missed* T4 forms showed they are the **functional idioms** — `sum(v for v in xs if
v>0)`, `max(xs)`, `sum(1 for v in xs if v>limit)`, `len([v for v in xs if v>limit])`,
`functools.reduce(…)` — and Python was **0/21**, Ruby 0/26, Rust 1/26. The instinct was
"the value graph doesn't recognize comprehensions". **It does** — extracted and
fingerprinted, a loop and its `sum`-generator / `sum`-comprehension / `sum(1 for…)` form
converge to a *byte-identical* fingerprint (jaccard **1.00**, the exact-match path). The
real blocker was upstream: these dense one-liners are 2 lines / ~10 tokens, **below the
unit size gate**, so they were never extracted as units at all. The gate, measured in raw
lines/tokens, cannot tell a trivial `return x` from a behaviorally-dense `return sum(v
for v in xs if v>0)`.

Fix (§AH-aligned: gate on *semantic* content, not surface size): admit a frontend-tagged
**function** below the line/token gate when its value fingerprint is rich enough to be
matched by the oracle-certified exact-match path — `value.len() >= 6`, the *same* floor
that path already requires. A trivial `return x` (1–2 atoms) stays out; blocks stay
strict (they are the noisy units); only behaviorally-dense short functions are recovered.

This was the largest single increment, and a clean Pareto gain — recall up, precision
flat:
- controlled benchmark **value-add recall 57% → 66%**;
- strict-axis (held) T1 90→95%, T2 90→95%, T3 47→51%, T4 18→20%, and on dev the
  functional-heavy algorithms jumped hard (XL strict 35→63%, T4 strict 12→24%) — the
  held/dev asymmetry is real, not overfit: the held canonical algorithms (max/dot/reverse)
  have fewer natural `sum`-comprehension forms than the dev ones (sum/count/factorial);
- **neg-FP held at 25%** (negatives are multi-line, already admitted — nothing new to
  misfire on);
- broader-corpus **behavioral precision 100%** (202/202) and recall 95.7% unchanged — the
  no-gaming guard; admitting dense functions added no noise to real clustering.

Verified: `cargo test` green, clippy clean, §AJ soundness 0 false merges. The T4 *strict*
recall (20% held) is still well below ≥50%: the remaining misses are the forms that do
*not* converge — `functools.reduce` written as `Field(functools, reduce)` (recognized
only as bare `reduce`), `max(xs)` vs an explicit max-loop in some languages, and
`len([comprehension])` as a count (jaccard 0.33, `len` not folded to a count-reduce).
Those are genuine value-graph idiom gaps — the next increments.

## AR. Two idiom fixes (partial) — `functools.reduce` routing and swapped-guard folds

Attacking the §AQ remaining-misses list produced two correct-but-partial fixes:

1. **`functools.reduce` was misrouted.** `functools.reduce(f, xs, init)` is a `Call`
   whose callee is `Field(functools, reduce)`; the idiom table read it via the generic
   `.map/.filter/.reduce` *method*-HoF arm, treating the **module `functools` as the
   collection**. Special-cased it (base == `functools`) to the explicit `Builtin::Reduce`
   fold over `xs`, the same canonical form as a bare `reduce(f, xs, init)`.

2. **Swapped-polarity guarded folds.** `cfg_norm` canonicalizes a two-branch ternary's
   orientation, so `acc + v if v>0 else acc` lowers to `if v<=0 { acc } else { acc+v }`
   — accumulator in the THEN branch, guard negated — while a single-branch loop guard
   `if v>0: acc+=v` stays positive. `as_reduction` only recognized the canonical
   `Phi(cond, ⊕, acc)`; added the swapped `Phi(¬cond, acc, ⊕)` case with a value-graph
   guard negation (`a<=b`→`a>b`, …) so the two polarities converge.

Both are genuine correctness improvements (a module is not a collection; a ternary fold
*is* a reduction) and verified sound (`cargo test` green, 0 false merges), and they lift
Python T4 *candidate* recall (10% → 29%). But they are **partial**: `functools.reduce`
with a ternary lambda still only reaches jaccard 0.27, and `functools.reduce(lambda a,b:
a if a>b else b, …)` as a max reaches 0.14 — selection-via-reduce-lambda isn't mapped to
`Reduce(Max)`, and residual structural divergences (the fold's `Input`-seeded accumulator
vs the loop's `Loop(cid)` recurrence, the explicit `init` arg) keep the fingerprints from
becoming identical. Aggregate held-out T4-strict held at 20%, neg-FP at 25% — these were
correctness/foundation fixes, not a metric move. Fully converging reduce-lambda folds
(selection recognition + accumulator-seed unification) is the next increment; it is a
Python-slice long tail with diminishing leverage relative to the cross-language gains
already banked (value-add 57% → 66%, behavioral precision 100%).

## AS. Soundness bug hunt — seven false merges, each with a unit-test reproducer

A directed hunt for *false merges* (behaviorally-distinct code → identical fingerprint),
driven by the §AJ insight that fingerprint-equal must imply behavior-equal. The corpus
oracle reported 0 false merges across 63 repos, so the bugs had to be found
*adversarially*: craft near-miss pairs that differ in one behavioral dimension and assert
their fingerprints differ. Each bug is locked in by a `tests/equivalence.rs` reproducer
(fails before, passes after). All fixes verified: full suite green, clippy clean, oracle
0 false merges, controlled value-add 66% (and per-type recall) unchanged.

Two families emerged:

**Loop iteration-extent dropped** — the value graph abstracted `C[i] → Elem(C)` (and
ran the reduction normal form) as if every loop visited *all* of `C` in order:
1. **range-start** — `range(len(a))` (sum all) ≡ `range(1, len(a))` (skip first). Only a
   provably-full range (`range(len)` / `range(0, len)`) now licenses the `Elem` rewrite.
2. **while-stride** — `while i<len: …a[i]…; i+=1` ≡ `i+=2`. Only a unit-stride, zero-start
   counter is a full index; other strides bind a strided index encoding start+step. (Also
   fixed `++`/`--` lowering to a concrete `LitInt(1)` in js_ts/go — it was an abstracted
   `Lit(Int)`, so the step was illegible *and* `x++` didn't converge with `x = x + 1`.)
3. **early-break** — `for x in xs: acc+=x; if acc>K: break` (prefix) ≡ the full sum.
   `break` was a no-op; it now records its path condition as a distinct sink.

**Identity/value dropped in lowering or alpha-renaming:**
4. **slice bounds (python)** — `a[1:]` ≡ `a[:1]`: collecting only the slice's *named*
   children dropped which slot the bound occupied. Now position-preserving (None-filled).
5. **slice/range bounds (go, rust)** — same collapse for `a[1:]`/`a[:1]` and `&a[1..]`/
   `&a[..1]`; Rust also merged `1..2` with `1..=2` (inclusivity). Fixed in both frontends.
6. **free-variable collapse** — alpha-renaming gave *every* name a positional cid, so
   distinct globals/callees merged: `foo(x)` ≡ `bar(x)`, `max(a,b)` ≡ `min(a,b)`. Alpha
   now renames only *bound* names (params/locals/loop vars); a free name keeps its
   identity (keyed by symbol in `node_tag` and the value graph). Zero recall cost.
7. **boolean literal values** — `True` ≡ `False` (abstracted to a valueless `Lit(Bool)`;
   the interpreter even read *every* bool literal as `true`), so a predicate `if x>0:
   return True else False` merged with its boolean-swapped negation. Added
   `Payload::LitBool(bool)` (parallel to `LitInt`) end-to-end.

The hunt converged: after these seven, broad adversarial probing (operator/operand order,
stores, fields, indices, chained comparisons, path-sensitivity, recursion, nested loops,
comprehensions, cross-language) turns up only correct merges and the exceptions below.

## AT. Reconsidering the "lossy approximations" — `in` was a bug too

A follow-up audit of what §AS had filed as "deliberate, documented lossy approximations"
found one more rationalized bug. `in`/`is` → `Op::Eq` was **not** a principled choice:
membership is directional and non-commutative, so `a in b` collapsed with `b in a` AND
with `a == b` (a membership filter ≡ an equality check), and — worse — the comparison
lowering **dropped negation**, so `a is not b` ≡ `a is b` and `x is not None` ≡
`x is None` (opposite, extremely common checks). Fixed (8th bug): a non-commutative
`Op::In` (lists are in the interpreter's scope, so membership is now *soundly verifiable*
— interp gained a list-membership arm); `not in` / `is not` keep their negation; `is` /
`instanceof` stay equality-shaped (identity ≈ equality in a value model — defensible).

This left a cleaner three-way classification of what remains, which is the project's
standing position (and what [normalization](normalization.md)'s soundness constraint now states):
- **Rationalized bugs** — none known; the §AS seven and `in` are fixed.
- **Genuine limitation, not "acceptable"** — string/list concatenation via a commutative
  `+` (`s+x` ≡ `x+s`) is unsound, but a sound fix needs type/sequence inference, which a
  type-free cross-language tool doesn't have. Honest status: a limitation, not a choice.
- **Legitimate fuzzy tradeoff, but mis-placed** — large-constant / float abstraction
  (`x%7` ≡ `x%11`) and default-param-value drop. Abstracting magic numbers is reasonable
  *for the candidate/representation axis*, but baking it into the **shared** value graph
  violates the behavioral axis's own "constants must be distinct" rule (§AH). The
  principled fix is an **axis split** — the behavioral fingerprint retains constants
  (sound), the candidate fingerprint abstracts them (fuzzy). Achievable, but a redesign
  with recall implications that need measuring; not yet done.

## AU. Cross-field divergence → the precision frontier → v5 (105 repos) settles the re-rank

A push for "world-class beyond applying known clone-detection research": six subagents
each brainstormed from a *different field* (compilers/formal-methods, ML/representation,
bioinformatics, physics/dynamical-systems, information-theory, category-theory/cognition).
All six independently converged on the **same** architecture — structure-invariant
*candidate generation* → behavioral *confirmation* (the oracle as generator, not just
checker). Two of its concrete bets were attacked and **refuted by measurement**:

- **Behavioral near-match gating.** `nose verify` already computes a per-function
  behavior vector over a ~190-row battery; its "under-merged" groups (behavior-equal,
  fingerprint-split) are the candidate set. Instrumenting them: of 29 such groups, only
  **1** is structurally near (vj≥0.7) and it's a test fixture; the real clone
  (`AppendableJoiner` joinArray≡joinIterable) sits at **vj=0.39**, and the
  behavior-equal pairs are dominated by *coincidental battery agreement* that differs
  off-battery (`Charsets.toCharset(Charset)` vs `(String)`, `jsoup.isWhitespace` vs
  `isActuallyWhitespace`). The behavioral signal and structural-nearness do not
  intersect on real clones → not a metric-mover. (vj diagnostic added to `verify`.)
- **Symmetry-orbit / naming-parallelism** (physics/biology/cognition). Measured on the
  labelset: parallel-by-design mean name-parallelism 0.256 vs worthy 0.250 — **zero
  separation**. The same surface signal (`test_*_option` ≈ `digest_*_handler`) appears
  in both classes.

**Reframing the metric.** Interpretable-slice completeness (the `verify` oracle metric)
is near-exhausted and unrepresentative (~10% of functions, mostly small leaves). The
*product* metric is `refactoring_families` precision@K / worthy-recall. On it,
**worthy-recall is solved (~100%)**; the headroom is **precision**, and a breakdown
showed **62% of the precision loss is one category — `parallel-by-design`** (per-locale/
platform/primitive variants whose differing payload is the point). The only signal that
separates worthy from not-worthy is anti-unification **abstractness** (44%→78% worthy
rate) — already the §Y re-rank, which gave +5pp dev / +3pp heldout on v4 (not a clear
ship). So precision is **judgment-deep**: cheap structural signals don't crack it (now
four refutations — behavioral, orbit, naming, abstractness-rerank).

**v5 — grow the gold set to settle it (the §AD "v5-scale heldout would settle it").**
- **Pruning re-audited and strengthened** (`bench/prune_corpus.py`, a verified Python
  pass replacing the bash grep): it omitted `.rs`/`.py` (Rust/Python generated code
  leaked), and couldn't see data-table dirs (`_unicode_data/`, `unicode_tables/`),
  vendored stdlib forks (`internal/go_templates/`), ragel output, or `$OpenBSD:` compat
  shims. Every removal is cross-checked against the protected gold set (frozen
  duplicates ∪ worthy labels) — **0 protected files removed**; a dry-run caught a
  catastrophic false positive (libuv's own "Joyent" license header) before any delete.
- **+43 diverse repos → 105 total** (`bench/add_repos.py`), deliberately filling domain
  gaps the original 62 missed (databases, games/graphics, crypto, symbolic math,
  messaging, monitoring, test frameworks, image processing, search); split leans
  held-out (heldout 24→~40 repos) since that gate was the underpowered one.
- **4,879 new candidates labeled** by the full pipeline: 3-persona panel (60 sonnet
  agents, index-based labeling to avoid family-id corruption) → reconcile → rubric-strict
  **opus tiebreak** of the 2,488 splits → **Opus final-arbiter re-adjudication** of the
  652 still-ambiguous (526 resolved, 126 genuinely undecidable). → **v5: 9,461 families**
  (4,940 worthy / 4,521 not), dev 5,445 / heldout 4,016.

**Verdict (CI eval, `eval_by_language.py` on v5):**

| | dev base | dev re-rank | heldout base | heldout re-rank |
|---|---|---|---|---|
| OVERALL | 60% [55-64] | 61% [56-66] | 53% [48-59] | 52% [47-58] |

The abstractness re-rank **does not generalize**: the v4 dev +5pp collapses to **+1pp**
on the powered set (it was small-sample overfit), and heldout is **−1pp**. It is a
**Rust-only** effect (dev 44%→69%, CI-separated) that *hurts* C/Go/Java/Ruby/TS — a
uniform re-rank is a wash-to-negative. **Do not ship it** (now settled, not "unvalidated").
worthy-recall stays ~100% across all 7 languages / both splits.

Honest byproduct: precision on the diverse 105-repo set is **53–60%**, below the 62-repo
**66%** — the original number was optimistic; harder/varied domains (DB engines, game
engines, systems) surface more not-worthy families. (A label-strictness confound exists
for the *absolute* level — new panel labels may be slightly stricter than v4's
human-adjudicated ones — but the within-v5 base-vs-re-rank verdict is immune to it.)
Net: the precision frontier is real and **judgment-deep**; the remaining lever is an
LLM-judge re-ranker, not another cheap feature.

## AV. The precision loss is judgment-deep *all the way down* — and the Rust re-rank is confounded

Two follow-ups on §AU's v5 precision (the loss breaks down, P@20: parallel-by-design 62%
/ coincidental-shape 21% / trivial 8% / generated 5% / type-def 3%):

**Sound structural gates for the "detectable 21%" (type-def/trivial/generated) — mostly
NOT achievable.** The categories that *look* structural turn out entangled with worthy
lookalikes at the *same* structure:
- **type-def vs extract-base.** A value-graph-emptiness gate seemed clean (annotations
  have ~no behavior). But exception classes (`gson` `JsonParseException`) carry
  constructor-`super()` calls → value≈10, indistinguishable from a small real class; and
  worthy `extract-base` (`commons-lang` `MutableFloat`) is just a class *with* methods.
  No value floor separates them.
- **trivial vs worthy parameterize.** They are *structurally identical*: a thin
  delegation differing by one constant is worthy `parameterize` (httpx
  `get`/`post`/`put` → `request(VERB,…)`, value≈2) while a thin delegation not worth
  extracting is `trivial` — same shape, opposite label. Any value/size gate that drops
  one drops the other → recall regression on the canonical worthy case.
- **generated is partly mislabeled.** Of 514 `generated`-labeled families, many are real
  source (`radash/src/*.ts` tagged generated only because nose self-matched it against
  the kept `cdn/` bundle). Pruning by label would delete real source. The *genuine*
  prune-missers are repo-specific (ANTLR `// Generated … by ANTLR` parsers, tokei
  `*.tera.rs` templates, puma's ragel Java) — added the two unambiguous rules to
  `prune_corpus.py` (12 more files, 0 protected removed); the rest is whack-a-mole.

So precision is judgment-deep **all the way down**: even the "structural" not-worthy
categories can't be gated soundly without hitting worthy lookalikes of identical shape.
There is no cheap sound gate. (Cleanly-winnable ≈ the unambiguous-marker generated slice
only, <1pp.) This is the fourth-and-a-half refutation; it makes the LLM-judge re-ranker
the *only* remaining precision lever, not a preference.

**Why the abstractness re-rank helps Rust but hurts the rest (§AU's open thread).** A
base-vs-re-rank top-10 flip diagnostic (`rust_diag.py`): the re-rank *adds* clean-skeleton
worthy helpers (`abstractness=1.0, struct_hole=0`, mostly `extract-helper`/`parameterize`
test helpers in alacritty/clap/bat) that base value-rank under-ranks (small → low
"value"), and *drops* high-`struct_hole` families. In Rust this nets positive **because
Rust's base value-rank is poor** — it buries clean small helpers under module-level/
line-1 matches whose anti-unification is meaningless (`struct_hole≈1.0`, the §Z lowering
confound). In C/Go/Java/Ruby the base rank already surfaces worthy families, so the same
reorder only demotes legitimately-worthy larger families → net negative. The real
sub-signal ("base value-rank under-ranks small clean helpers") is genuine but the
abstractness re-rank is a confounded, language-dependent way to exploit it — not
shippable. A cleaner attack would fix the *base* value ranking's small-helper bias
directly, language-agnostically.

## AW. Core-hardening program — sound foundation + machine-checked canons + type inference

A pivot from the (precision-frontier, judgment-deep) product metric to making the **core
sound and capable** — with formal methods, type inference, and category theory used
aggressively, and the *verifier as the safety net for bold attempts* (run `verify`; an
unsound canon shows up instantly as a false merge, and gets rolled back). The 43-repo
expansion (§AU) was the trigger: it exposed that the soundness contract (§AJ) was not
actually met.

**Phase 0 — soundness floor: 15 false merges → 0.** Five language-general fixes, each
measured (`verify` on 105 repos, all tests, v5):
- `Raw` (unlowered) nodes were keyed by a *positional* opaque counter (and dropped their
  lowered children), collapsing distinct unlowered constructs — invisible to the oracle
  (`Raw` is uninterpretable). Now keyed by **subtree hash** (like `Lambda`). Reproducer: two
  C compound-literal functions had identical fingerprints.
- C `#if return 1 #else return 0` lowers both branches live → two order-independent return
  sinks; a branch-swapped twin collapsed. Fix: **dead code after an unconditional return**
  is dropped (matches the interpreter's first-return-wins).
- Field writes: the oracle recorded them as an *ordered* effect trace (so commuting
  constructors `{a;b}` vs `{b;a}` falsely differed), while the value graph's order-
  independent multiset merged same-field overwrites (unsound the other way). Fix: model
  `self.x = v` as **last-write-wins per field** in *both* engines.
- **`Err` propagates through conditions** (`Flow::Err`): a type error in an if/loop/ternary
  test yields `Err`, so a lenient `x>0?x:-x` / accumulator loop Errs on non-numbers exactly
  as the `abs`/`sum` builtin it canonicalizes to → the canon is sound *and* recall kept.
  (Lifted completeness 58→62%.)
- `verify` excludes **empty** value fingerprints (an empty value graph has nothing to merge
  on — the detector keys candidates on structure there); distinct empty bodies "colliding"
  was a measurement artifact, not a product false merge.

**Phase 1 — machine-checked core + verifier-gated bold canons.** The contract moved from
empirical ("0 merges on N repos") to **proven**. Lean 4.30, all checking clean
(`formal/`):
- `Algebra.lean` — the AC-operand flatten+sort is denotation-preserving (`canon_sound`),
  and `a - b → a + (-b)` (`sub_eq_add_neg`).
- `Control.lean` — guard-clause ≡ if-else (`guard_clause`, cascade) and dead-code-after-
  return.
- `Functor.lean` — map fusion `map g (map f xs) = map (g∘f) xs` (the functor law) + identity.

Bold canons attempted (each verifier-gated; sound ones kept + proven):
- **`a - b → a + (-b)`** routed through AC-`+` → unifies subtraction/negation variants.
- **Guard-clause normalization** — narrow the block path by `¬c` after `if c {return}` so
  guard-clause and if-else writings converge (sympy `symmetric_residue` ≡ `gf_int`).
- **Free monoid for strings/builders** (the monoid abstraction) — `+`/concat = ordered
  append. Correct and sound, but ~0 corpus impact (builders are uninterpretable for *other*
  reasons — object methods / field mutation); it also exposes that `+` is overloaded.
- **Untyped simplifications `-(-x)→x`, `x&x→x` — REFUTED.** The verifier caught **17 false
  merges**: untyped, they drop the operator's observable type-error behavior (`-(-list)`,
  `list&list` are `Err`). Rolled back — the safety net working exactly as intended.
- **Purpose-fit type inference** (`types.rs`, coarse Num/Bool/Str/List/Unknown, params typed
  from strictly-typed uses; `Unknown` is the safe top). Uses: (1) `+` commutes UNLESS an
  operand is *proven* string/list — concat held ordered **by construction** (the latent
  unsoundness the free monoid exposed, now fixed, not merely untriggered); Unknown still
  commutes (oracle-checked) so the common numeric case is unaffected; (2) the 17-false-merge
  simplifications are re-enabled **soundly**, gated on proven type.
- **Map fusion** via the functor law (above) — converges functional pipelines.

Net Phase 1: `verify` stays at **0 false merges**; completeness **62%**; v5 unchanged
throughout (dev 60% / heldout 54%); all tests pass; per-repo `scan` perf on par with
the pre-program binary (no regression — the value-graph/interp changes are per-unit). The
honest observation: the aggregate completeness metric is **insensitive** to individual
correct canons (it is dominated by coincidental large behavior-groups + a diverse long
tail), so each canon is justified by *correctness + soundness + a proof*, not by moving a
noisy number. And untyped sound-canon space is fundamentally bounded — further algebraic
simplification needs the type inference (now in place).

**Phase 2 — per-language lowering: already strong; honest marginal headroom.** Corpus Raw
is **0.435%** (worst: JS 1.16%, C 0.88%). The residual is dominated by parse-`ERROR`
(unfixable), non-behavioral C declarations/preproc, and niche constructs; the common
behavioral forms (multi-assign/return, goroutines, every loop shape, comprehensions) lower
cleanly. Probed candidates (`python line_continuation`, `go expression_list`) turned out to
lower fine in the common case — the Raw is elusive edge contexts of low convergence value.
The low *per-language precision* (e.g. C 24% heldout) is judgment-deep (§AU/AV), not a
lowering gap. Added the unambiguous generated-prune rules (ANTLR/tera). Manufacturing
marginal Raw fixes would be metric-gaming, so the honest Phase-2 result is: **the lowering
is already at a strong, low-Raw state; the remaining levers are marginal or judgment-deep.**

## AX. The independent oracle — unmasking the commutativity-of-non-commutative-ops bug class

§AW built the verifier as a safety net, but the net had a hole: `nose verify` interpreted
the *same fully-normalized IL it fingerprinted*. So any **behavior-changing canonicalization
masked itself** — `a or b` and `b or a` both sorted to one IL, the interpreter saw them as
identical, and the false merge was invisible. The fix (this round) makes the oracle's ground
truth **independent of the canon layer**: it now interprets the *pre-canonicalization* core
IL (`desugar`+`alpha` only, via `NormalizeOptions.oracle`), matched to each fully-normalized
unit by source span, while still fingerprinting the full normalize. A canon that changes
behavior is now caught instead of hiding behind its own rewrite.

That single architectural change exposed a whole **bug class**: *treating a non-commutative
operator as commutative*. Each was a real, latent false merge, fixed at the root:

- **`a or b` / `a and b` sorted as commutative** — short-circuit value-and/or returns the
  deciding operand's value and is NOT commutative (`1 or 2` = 1 ≠ `2 or 1` = 2). Also the
  interpreter evaluated both operands eagerly, so `5 or (1/0)` wrongly Err'd. Fixed: the
  interpreter short-circuits (and yields the operand value); algebra keeps `and`/`or`
  ordered; the value graph canonicalizes them TYPE-GATED — both-Bool ⇒ commutative (sorted),
  else ⇒ the positional `Phi` the ternary builds. This also *converges* `a or b` with
  `a if a else b` (probe2 12/12).
- **`!!x → x`** — `!!x` is `bool(x)` (truthiness), not `x` (`!!5` = true ≠ 5); it merged
  `return !!x` with identity `return x`. Algebra preserves the double negation; the value
  graph cancels it only via the sound negated-comparison canon `!(a<=b) → a>b`.
- **`not(Err)` → `Bool(true)`** — negating an error must propagate it (Python `not (1/0)`
  raises). Without this the SOUND `!(a<=b) ≡ a>b` looked like a false merge. Now `not Err =
  Err`.
- **`x*1 → x`, `x+0 → x`** — unsound for non-numeric `x` (`"a"+0` Errs; `self*1` need not be
  `self`), and type inference is optimistic (infers `x:Num` from `x*1` itself), so a Num gate
  doesn't help. No longer eliminated.
- **`"a" + b + "c"` operand sort** — string/list `+` is concatenation (non-commutative); the
  typeless algebra pass reordered the pieces of every string-building expression. Found by a
  second, *pair-free* check added alongside: **canon preservation** — interpret each unit on
  the core IL AND the full IL and require equal behavior (no colliding twin needed). It
  flagged 20 concat sites. Fixed: algebra no longer sorts `+` (nor `and`/`or`); the value
  graph sorts `+` itself, gated on concat, so numeric `a+b ≡ b+a` still converges.

Also added this round, both sound + machine-checked: **ternary-return decomposition**
(`return (a if c else b)` splits into guarded returns, converging a nested ternary with an
`elif` cascade — `formal/Control.lean:ternary_return`, the dual of `guard_clause`).

**Result.** `verify` = 0 false merges *under the independent oracle*; CANON PRESERVATION =
0 behavior-changing units. Probes: probe1 9/10, probe2 12/12, probe3 8/12, xlang 9/10. v5
unchanged (dev 60% / heldout 54%, recall ~99%) — the metric held while the *foundation*
got materially more trustworthy. The lesson generalizes: **a differential oracle must not
share its subject's canonicalization**, or it certifies the very rewrites it should police.
The lower completeness ratio (≈59% vs 62%) is honest oracle fidelity — short-circuit + Err
propagation made the interpreter recognize tens of thousands more true equivalences, growing
the denominator faster than convergence; the absolute converged count rose.

## AY. Re-sweeping the experiment log with the better system (types + v5 + oracle)

Goal: walk the whole log, re-try anything whose blocker the current system (purpose-fit
type inference, the v5 labelset + bootstrap CIs, the independent oracle + IL convergence
work) might have lifted, adopt what measures well, record the rest.

**IL / normalization tier — 3 adopted, 1 rejected-with-evidence.**
- ✅ **Existence/universal LOOP form** — `for x: if p(x): return True; return False` ≡
  `any(p(x) for x in xs)` (and `all`). Was deferred as "fragile whole-function pattern";
  the independent oracle now backstops it. probe3 8→10/12, verify=0, v5 held.
- ✅ **Collection-building loops** ≡ comprehensions/`.map`/`.push`/`.collect`, cross-language
  and filtered — the canonical Type-4 pattern. Needed a coupled interp (faithful local-list
  build) + value-graph (`builder → Hof(Map)`) change; the canon-preservation check forced
  the statement-vs-expression `append` split (Go's value-returning `append`). completeness
  +8, verify=0.
- ✅ **Float-constant distinction** (§AT's "mis-placed abstraction") — all floats had
  collapsed to one `Lit(Float)` token (a latent false merge invisible to the float-less
  oracle); a retained source-text hash makes `3.14` ≠ `2.71`. Sound by construction.
- ❌ **Doubling `x*2 ≡ x+x`** — sound only if the interpreter models `str/list * int`
  repetition; doing so made `verify` ~10× slower (battery-wide repeated allocation/cloning)
  for a marginal idiom. Rolled back (evaluate → bad).

**Detector / candidate tier — re-confirmed settled (no adoption).** §E already swept this
(9 cross-field experiments, 1 merge — RANSAC; candidate-widening a dead family) and §AB/§AU/§AV
re-validated with v5 (precision is judgment-deep; re-ranking does not generalize). Re-checked
against the new system and added a fresh measurement: a value-graph-selective candidate-weight
variant (CWV 0.3→0.55) left v5 base at dev 60% / heldout 54% (flat, within CI). Decisively,
the three IL adoptions above all STRENGTHEN the value-graph (behavioral) fingerprint — the
scoring substrate §E.6 said gates recall — yet v5 P@10 did not move. That empirically
confirms the precision ceiling is **judgment-deep, not semantic-signal-limited**, so the one
genuinely new-capability detector idea (§E.2 #12, behavioral/micro-execution fingerprint —
now buildable since the interpreter exists) would not clear the §E.3 bar: it adds more of the
behavioral signal that demonstrably doesn't move precision, at the cost of wiring the
interpreter into `detect` for the ~11-pair recoverable set. Not pursued (would be churning a
settled negative).

**Net:** the new system's leverage is in the IL/fingerprint tier (semantic convergence), where
it landed 3 sound adoptions; the detector/precision tier remains judgment-deep and exhausted.

## AZ. Extractability as the default ranking — the re-rank that *does* generalize

§AU/§AV settled that a uniform anti-unification **abstractness** re-rank does not generalize:
on v5 it was a Rust-only win (dev 44%→69%) that washed-to-negative elsewhere (overall dev
+1pp, heldout −1pp), so it was not shipped. That left the question open whether *any*
re-rank could beat raw `value` (removable lines × similarity × spread) on the worthy
labelset, or whether precision is ranking-invariant.

The `extractability` ranking — now the **default** sort for `nose scan` — answers it. It is
**not** the rejected re-rank: instead of a bare abstractness multiplier it scores the
*invariant* (shared) source lines × copies × spread, with three correctives the prototype
lacked — **tightness** (shared/total, so a 22/384 dispatch skeleton can't outrank a 15/15
pair), a **parameter penalty** (a 30-hole "helper" is scaffolding), and an **IDF idiom-gate**
(shared lines pervasive across the corpus — `if err != nil {` — contribute ~0), plus the
rule that a same-language family sharing **zero** invariant lines scores 0 (the structural-only
`sim 1.00` pathology), and the type-def/generated discount. Cross-language families, with no
shared source lines to diff, fall back to the structural estimate.

**Result (`bench/labels/extractability_vs_value.py`, v5, P@10, nose's native order vs
value-sorted):**

| | dev value | dev extractability | heldout value | heldout extractability |
|---|---|---|---|---|
| OVERALL | 61% [56-66] | 61% [56-66] | 53–54% [48-59] | **60% [54-65]** |

Dev is **flat**; the **held-out split — the generalization gate the prototype failed — rises
+6pp** (54%→60%), with no recall cost (reordering only; worthy-recall stays ~99–100% per §AU).
Per-language the held-out gains are broad, not one-language: Java **42%→71%**, C 24%→35%,
TS 45%→53%, Go 72%→77%, Rust 50%→55%, Ruby 80%→83% (Python −3pp, within CI). On dev the
Rust/TS/Python gains (Rust 51%→74%) offset Java/Ruby dips, netting flat. The contrast with
§AU's prototype is the point: a re-rank built from *what actually extracts* (tight, few-param,
non-idiom shared lines) generalizes, where one built from raw structural abstractness did not.

This is the first ranking change to move the held-out number, and it moves it the right way —
so extractability ships as the default, with `--sort value` retained for raw-volume triage.
(Detection is unchanged: the same families are found and the §AU recall holds; only the order,
and the honest `N/M shared · Pp` cell, differ.)
