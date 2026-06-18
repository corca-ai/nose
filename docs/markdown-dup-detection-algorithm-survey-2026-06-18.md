# Markdown duplication detection — algorithm survey & first report (2026-06-18)

**Status:** first report (1차 보고서). Preliminary exploration; no code shipped to nose yet.
**Scope decision (from the goal):** **cross-lingual duplication is out of scope** — translation
clones are not a problem worth detecting. This report studies **same-language** Markdown
near-duplicate detection only.
**Constraints (nose philosophy):** deterministic, single self-contained binary, **no LLM at
runtime**, incremental/cacheable, must scale sub-quadratically to large corpora, and — because
nose is a refactoring tool — a match should be able to **point at the exact duplicated span**
(a *witness*).

This is a measurement-first study: every adoption/rejection claim below is a number, not a
vibe. The harness, datasets, and raw outputs are reproducible (see §9).

---

## Abstract

We empirically compare **19 similarity/distance algorithms** spanning every requested family —
bag-of-words, TF-IDF, BM25, set/shingle (Jaccard, containment, MinHash, SimHash, winnowing,
q-gram), edit distance (Levenshtein, Jaro–Winkler, Ratcliff/Obershelp), bioinformatics
sequence alignment (LCS, longest common substring, Needleman–Wunsch, Smith–Waterman, greedy
string tiling), and compression (NCD) — on a **balanced real + synthetic corpus** of 4,706
labeled Markdown block pairs, including a CJK (no-space-script) subset. We measure ROC-AUC,
PR-AUC, recall at fixed precision, recall-vs-edit-ratio degradation, hard-vs-easy negative
discrimination, and per-pair cost; we then run a **3-judge LLM rubric evaluation** with
distinct lenses (production-systems, IR/NLP-research, clone/plagiarism-detection). The
quantitative results, the qualitative panel, and an independent academic literature survey all
converge on the same conclusion:

> **No single algorithm satisfies both scale and witness.** The right design for nose is a
> three-stage pipeline over a **character-n-gram** substrate: **(1)** MinHash-LSH + winnowing
> fingerprints for sub-quadratic, incremental candidate generation (order-invariant → robust
> to block reorder); **(2)** **TF-IDF cosine** (or q-gram on CJK) to rank/verify, because IDF
> down-weighting is the single biggest lever against topical false positives; **(3)** local
> alignment (Smith–Waterman with affine gaps) or greedy string tiling to extract the exact
> duplicated span as a witness. Unanimously rejected: SimHash, Jaro–Winkler, Needleman–Wunsch,
> BM25, plain bag-of-words.

---

## 1. Introduction

### 1.1 Problem

nose finds refactoring candidates and Type-4 code clones. Extending it to Markdown means
finding **duplicated documentation** — copy-pasted sections that drift, boilerplate, restated
content — so that the duplication can be deduplicated or single-sourced. Unlike code, prose has
no machine-checkable denotation, so (under the no-LLM constraint) "same meaning, different
words" (the Type-4 analog / paraphrase / translation) is **out of reach and out of scope**.
What remains, and what matters in practice, is **same-language near-duplication**: exact copies,
formatting-only variants, lightly-to-heavily edited copies, reordered content, and
small-section-inside-large-document reuse.

### 1.2 Contributions

1. A reusable, pure-stdlib (no numpy/sklearn) **measurement harness** implementing 19
   algorithms and a controlled edit-injection dataset generator (§3, §9).
2. A **quantitative comparison** on real + synthetic data with a CJK subset (§4).
3. A **3-lens LLM rubric evaluation** and consensus scorecard (§5).
4. An **academic literature survey** of all families, with primary sources (§2).
5. A concrete **recommended architecture** and a set of **pre-registered next experiments**
   for nose (§6, §8).

---

## 2. Related work / algorithm families (literature survey)

Three independent literature surveys (IR/set-hash; edit-distance/bioinformatics/compression;
systems & evaluation methodology) were conducted with primary-source grounding. Condensed:

### 2.1 IR / lexical
- **Bag-of-words / TF-IDF cosine** (Salton; Stanford IIR). TF-IDF weights term *t* by
  `tf·idf`, `idf=log(N/df)`. Near-dup threshold ~0.9 is folklore. **Order-blind** (a bag), and
  `idf` is a **corpus-global** quantity → no stable per-document fingerprint, breaks
  incrementality. No span witness.
- **BM25 / Okapi** (Robertson & Zaragoza 2009). A probabilistic *ranking* function, k₁≈1.2,
  b≈0.75. **Asymmetric and unbounded** → not a symmetric, thresholdable similarity; corpus-global
  stats; no witness. Home is ranked retrieval, not dedup.

### 2.2 Set / hashing fingerprints
- **k-shingling + Jaccard** (Broder 1997). Document = set of k-grams; resemblance = Jaccard.
  Word-shingle k≈3–9 (web near-dup k≈4); **char-gram k≈5** is the de-facto multilingual default
  (RefinedWeb, Japanese web corpora). The shingle intersection **is** a witness.
- **MinHash + LSH banding** (Broder 1997; Leskovec–Rajaraman–Ullman MMD ch.3).
  `Pr[minhash agree]=Jaccard`; signature of K perms (K=128–256 typical), variance `J(1-J)/K`;
  LSH S-curve `1-(1-s^r)^b`. **The** incremental, cacheable, sub-quadratic candidate generator.
  No span witness by itself.
- **SimHash** (Charikar 2002; Manku et al. 2007, Google web crawl). 64-bit fingerprint,
  Hamming ≤3; reported ~0.75/0.75 P/R at web scale; **no span witness**, weak on short docs.
- **Containment** (Broder 2000; Mash Screen 2019). Asymmetric `|A∩B|/|A|` — the **small-in-large**
  primitive that symmetric Jaccard structurally misreports.
- **Winnowing / MOSS** (Schleimer, Wilkerson & Aiken 2003). Local fingerprinting: window the
  k-gram hashes, keep the per-window minimum. **Guarantee:** every shared substring ≥ `w+k-1`
  is detected; fingerprints carry positions → **exact span witness** *and* inverted-index scale.

### 2.3 Edit distance, sequence alignment, compression
- **Levenshtein** (1966; Wagner–Fischer 1974). O(n·m), **no strongly sub-quadratic algorithm
  unless SETH fails** (Backurs–Indyk 2015). Edit-script witness. Reorder-rigid. Second-pass only.
- **Jaro–Winkler** (Jaro 1989; Winkler 1990). Built for short **names/records** (5–30 chars);
  degenerates on documents (matching window becomes vacuous); not a metric; no witness. **Wrong tool.**
- **LCS / longest common substring.** O(n·m) DP, but longest common substring escapes DP via
  **suffix arrays/automata/generalized suffix trees** (near-linear) — the one part of this
  family with corpus-scale *and* self-witnessing potential (offsets fall out directly).
- **Needleman–Wunsch** (1970, global) / **Smith–Waterman** (1981, local) / **Gotoh affine
  gaps** (1982). Local + affine gaps is the correct model for "a whole section was inserted"
  (one long gap ≪ many short gaps; BLASTP defaults open=11, extend=1). Gapped-alignment witness.
  O(n·m) per pair; needs BLAST-style k-mer seeding for scale (at which point the seeder is the
  candidate generator). Global NW is the wrong model (penalizes partial overlap).
- **Greedy String Tiling / RKR-GST** (Wise 1993; JPlag). Non-overlapping maximal tiles
  ≥ MML (JPlag default **9**); Dice over tiles. **Reorder-robust** and the **tile set = exact
  duplicated spans** (best-in-class witness). O(n³) worst / ~O(n) average; per-pair second-pass.
- **q-gram distance** (Ukkonen 1992). L1 of q-gram profiles; the **counting filter** gives a
  *sound* edit-distance prefilter (indexable, recall-guaranteed). No witness; bridges to alignment.
- **NCD** (Cilibrasi–Vitányi 2005). `(C(xy)-min)/max`. No witness, no prefilter, O(N²) full
  compressions, non-incremental, compressor-block-size limits. **Wrong tool here.**
- **Myers O(ND) diff** (1986). Near-linear for near-dups; "snakes" = verbatim shared spans.
  The de-facto **witness renderer** (git/editors). Reorder-rigid; second-pass.

### 2.4 Systems & evaluation methodology
The standard near-dup pipeline is **candidate generation → verification → span extraction**:
- **Broder 1997/2000 (AltaVista, ~30M pages, ~29% near-dup):** k-shingle → MinHash sketch →
  **super-shingles** (shingle the sorted sketch — LSH banding at the sketch level) → candidate
  pairs → exact Jaccard verify → union-find cluster → canonical member.
- **Manku et al. 2007 (Google, ~8B pages):** 64-bit **SimHash**, Hamming ≤ 3, multi-permutation
  sorted tables for O(log N) neighbor lookup. One integer/doc; bag-of-features semantics.
- **Henzinger 2006 (1.6B pages):** shingling and random-projection are **complementary** — their
  *intersection* lifts precision 0.38/0.50 → **0.79** at 79% recall. Same-site pairs are
  essentially undetectable by shingling alone (boilerplate dominates) — motivating §5.
- **Plagiarism systems:** COPS (sentence fingerprints), SCAM (TF vectors), **MOSS/winnowing**
  (gap-guaranteed local fingerprints), **JPlag** (token canonicalization + greedy string tiling).
- **PAN shared tasks (2009–):** formalized the **two-stage** split (source retrieval → text
  alignment with character offsets), evaluated blind on TIRA to prevent threshold tuning;
  metric **plagdet** = F1 penalized by *granularity* (over-fragmentation of one copied span).

**Preprocessing that matters:** NFKC + casefold (folds fullwidth/ligature/width variants — but
*not* math notation); collapse whitespace; **char n-grams for no-space CJK/Thai** (Elasticsearch/
OpenSearch CJK-bigram is the industry standard; trigram for higher precision); **word 5-grams for
Latin** (BigCode production finding: unigram/4-gram over-fire on common words); **do not remove
stopwords** inside n-grams (they add specificity). Markdown: strip heading markers/emphasis,
reduce links→text and images→alt, decode entities, and **hash code blocks separately from prose**
(different n-gram distributions).

**Boilerplate suppression (essential to precision, per Henzinger):** **I-Match** (Chowdhury 2002)
keeps only the *middle* IDF band — drop the lowest-25% IDF (boilerplate/function words) *and*
highest-25% IDF (hapax) terms; **SpotSigs** anchors phrases at stop-words; **stop-shingles**
(Google US7698317) dynamically drop any shingle appearing in > ~1% of corpus docs. These are the
concrete mechanisms for the Contributor-Covenant/license/badge false-positive class.

**Evaluation standard:** **PR-AUC over ROC-AUC** under the extreme class imbalance of near-dup
(O(N²) pairs, O(N·d) duplicates) — ROC is "overly optimistic under skew" (Davis & Goadrich,
ICML 2006); recall-vs-edit-ratio curves; gold via **controlled edit injection** (Svajlenko
mutation operators: substitute/insert/delete/reorder; PAN obfuscation ladder
verbatim→auto→manual paraphrase); fixed-threshold-on-held-out protocol; and a caution that gold
sets themselves can be mislabeled (BigCloneBench: 93% of weak Type-3/4 labels wrong → a tool's
F1 0.94 became 0.06 after correction — validate a sample of any gold set).

**Synthesis of the literature:** the family bifurcates fundamentally — O(n) sketch/hash methods
that scale but carry **no witness**, vs O(n·m) alignment methods that **witness** but don't
scale. The only near-dual-use primitive is a **generalized suffix structure**. This predicts a
pipelined design, which our measurements confirm.

---

## 3. Methodology

### 3.1 Datasets (balanced real + synthetic)

**Synthetic — controlled edit injection (4,176 pairs).** From 6,000 harvested prose
Markdown sections (heading-rooted, 40–280 words) across `bench/repos`, we take base blocks and
generate labeled pairs with *known* divergence:

- **Positives** (per base): exact copy; **reword** at edit ratio r ∈ {0.1, 0.2, 0.35, 0.5}
  (random tokens replaced); **indel** at r ∈ {0.2, 0.4} (delete+insert); **reorder** (sentence/
  line permutation — content identical, order changed); **format** (Markdown formatting-only
  changes: bullet markers, emphasis style, blank lines — meaning preserved); **mixed** (0.3).
- **Negatives:** **random** unrelated block (easy) + **sibling** = a *different section of the
  same source document* (hard, topically related non-duplicate — the realistic false-positive
  class).
- **CJK subset (898 pairs):** the same pipeline on Japanese/Chinese/Korean prose blocks
  (no-space scripts), to test script-agnosticism within a single language.

**Real — provenance-labeled boilerplate (530 pairs).** From `bench/repos`:
- **Positives:** the **Contributor Covenant** `CODE_OF_CONDUCT.md` family (264 pairs), labeled
  by provenance (signature phrase + version) — these are the same source document independently
  customized per project (a genuine real-world near-dup phenomenon); plus normalized-identical
  CONTRIBUTING/SECURITY/PR-template pairs.
- **Negatives:** custom-CoC vs Covenant-CoC, and cross-basename pairs (CONTRIBUTING vs SECURITY).

Label provenance is deliberately *not* derived from any similarity measure (which would bias
toward set-based algorithms). Synthetic labels are by construction; real labels are by
document provenance.

### 3.2 Algorithms (19)

| Family | Algorithms |
|---|---|
| IR/lexical | bow_cosine, tfidf_cosine, bm25 |
| Set/hashing | jaccard_word3, jaccard_char5, containment_word3, minhash128, simhash64, winnowing, qgram3 |
| Edit distance | levenshtein, jaro_winkler, ratcliff (Ratcliff/Obershelp) |
| Alignment (bioinformatics) | lcs, lcsubstr, needleman_wunsch, smith_waterman, greedy_tiling |
| Compression | ncd_zlib |

Implemented in pure-Python stdlib with **deterministic hashing** (BLAKE2b, fixed seeds) so runs
are byte-reproducible. Alignment inputs capped at 160 tokens for tractability.

### 3.3 Normalization & tokenization

Unicode **NFC**, **casefold**, whitespace collapse, and lightweight Markdown-syntax stripping
(strip code fences, reduce links/images to text, drop heading hashes/list markers/emphasis/
blockquote markers). **Script-aware tokenization:** space-delimited scripts → word tokens;
**no-space scripts (CJK/Thai) → per-character tokens**. (Consequently "word 3-shingles" on CJK
are effectively character trigrams — see §4.4.)

### 3.4 Metrics & protocol

- **ROC-AUC** (rank/Mann–Whitney) — threshold-free separation; fairest cross-algorithm compare.
- **PR-AUC** (average precision) — robust under class imbalance.
- **Recall@Precision=0.95 / 0.99** — the operating points that matter for a low-false-positive tool.
- **Recall-vs-edit-profile curves** — each algorithm at *its own* P≥0.95 operating point.
- **Hard vs easy negatives** — ROC vs random negatives vs sibling negatives, and the gap.
- **Cost** — single-core µs/pair microbenchmark.

---

## 4. Quantitative results

### 4.1 Discrimination (Table 1)

LATIN synthetic (English prose), top of ranking by ROC-AUC:

| algo | ROC-AUC | PR-AUC | R@P95 | R@P99 |
|---|---|---|---|---|
| **tfidf_cosine** | **0.995** | 0.998 | 0.995 | 0.976 |
| **winnowing** | 0.994 | 0.997 | 0.995 | 0.966 |
| smith_waterman | 0.992 | 0.995 | 0.995 | 0.942 |
| ratcliff | 0.992 | 0.996 | 0.985 | 0.967 |
| jaccard_char5 | 0.991 | 0.996 | 0.983 | 0.961 |
| lcs | 0.990 | 0.996 | 0.983 | 0.966 |
| levenshtein | 0.987 | 0.994 | 0.979 | 0.951 |
| ncd_zlib | 0.984 | 0.993 | 0.975 | 0.925 |
| jaccard_word3 | 0.983 | 0.990 | 0.974 | 0.949 |
| minhash128 | 0.981 | 0.989 | 0.971 | 0.925 |
| bow_cosine | 0.980 | 0.990 | 0.961 | 0.920 |
| bm25 | 0.978 | 0.990 | 0.948 | 0.871 |
| simhash64 | 0.963 | 0.983 | 0.921 | 0.810 |
| jaro_winkler | 0.946 | 0.975 | 0.917 | **0.579** |
| needleman_wunsch | **0.889** | 0.954 | 0.807 | 0.784 |

Most methods separate well on English (ROC > 0.97). **Threshold stability** separates the
contenders: `lcsubstr` (R@P95 0.981 → R@P99 0.617) and `jaro_winkler` (0.917 → 0.579) collapse
at high precision — unsafe for auto-dedup. `tfidf_cosine`/`winnowing`/`ratcliff`/`lcs` stay
stable (R@P99 ≥ 0.96). Global alignment (`needleman_wunsch`) is the worst real method.

### 4.2 Degradation by perturbation — the reorder fault line (Table 2)

Recall by profile, each algorithm at its own P≥0.95 threshold (LATIN). The **reorder** column
is the discriminator:

| algo | exact | format | **reorder** | reword.35 | reword.5 | indel.4 |
|---|---|---|---|---|---|---|
| jaccard_word3 / containment / minhash | 1.00 | 1.00 | **1.00** | 1.00 | 1.00 | 1.00 |
| tfidf_cosine | 1.00 | 1.00 | **0.97** | 0.99 | 0.99 | 1.00 |
| winnowing | 1.00 | 1.00 | **0.98** | 0.99 | 0.98 | 1.00 |
| ratcliff | 1.00 | 1.00 | **0.96** | 0.97 | 0.95 | 0.99 |
| lcs | 1.00 | 1.00 | **0.95** | 0.97 | 0.94 | 0.99 |
| levenshtein | 1.00 | 1.00 | **0.91** | 0.97 | 0.94 | 0.99 |
| simhash64 | 1.00 | 1.00 | **0.93** | 0.84 | 0.76 | 0.95 |
| jaro_winkler | 1.00 | 1.00 | **0.83** | 0.84 | 0.71 | 0.92 |

- **Set/hashing methods are order-invariant** (1.00 on reorder) by construction.
- **Sequence/alignment/edit methods degrade on reorder** (0.83–0.98) — they are order-sensitive.
- **All methods are robust to formatting-only changes** (1.00) — normalization works.
- Set-based methods hold even at reword@0.5 because random negatives sit at ≈0 overlap, giving a
  huge separation margin. (`needleman_wunsch`/`greedy_tiling` flat 1.00 are tie/low-resolution
  artifacts, consistent with their low ROC.)

### 4.3 Hard vs easy negatives — the IDF lever (Table 3)

ROC vs random negatives vs **sibling** negatives (different section, same doc):

| algo | vs_random | **vs_SIBLING** | gap |
|---|---|---|---|
| **tfidf_cosine** | 0.998 | **0.991** | 0.007 |
| winnowing | 0.998 | 0.988 | 0.010 |
| ratcliff | 0.995 | 0.986 | 0.009 |
| smith_waterman | 0.997 | 0.984 | 0.013 |
| jaccard_char5 | 0.995 | 0.984 | 0.012 |
| jaccard_word3 | 0.986 | 0.977 | 0.010 |
| minhash128 | 0.985 | 0.975 | 0.010 |
| bow_cosine | 0.988 | 0.968 | 0.020 |
| bm25 | 0.988 | 0.962 | 0.026 |
| **lcsubstr** | 0.996 | **0.961** | **0.035** |
| **jaro_winkler** | 0.961 | **0.922** | **0.039** |

**TF-IDF resists topical false positives best** — IDF down-weights the common words sibling
sections share. `lcsubstr` and `jaro_winkler` are most fooled by shared surface phrases. This
empirically confirms the literature's "IDF/boilerplate down-weighting is essential" claim.

### 4.4 Script-agnosticism — CJK (Table 4)

CJK same-language synthetic, ROC-AUC, vs the LATIN value:

| algo | CJK ROC | LATIN ROC | Δ |
|---|---|---|---|
| qgram3 | 0.978 | 0.981 | −0.003 |
| ratcliff | 0.976 | 0.992 | −0.016 |
| smith_waterman | 0.975 | 0.992 | −0.017 |
| lcs | 0.973 | 0.990 | −0.017 |
| jaccard_word3 (=char-3gram on CJK) | 0.971 | 0.983 | −0.012 |
| jaccard_char5 | 0.959 | 0.991 | −0.032 |
| tfidf_cosine | 0.959 | 0.995 | −0.036 |
| winnowing | 0.936 | 0.994 | −0.058 |
| bm25 | 0.904 | 0.978 | −0.074 |
| **bow_cosine** | **0.855** | 0.980 | **−0.125** |
| **simhash64** | **0.825** | 0.963 | **−0.138** |

**Word-token IR (bow, bm25, simhash-over-words) collapses on CJK**; character/q-gram and
alignment methods stay robust (0.96–0.98). **Tokenizer choice gates script-agnosticism:**
character n-grams (or per-character tokens) are the universal substrate. Note `char5` and
`winnowing` (char-5-gram) dip on CJK because 5 characters is too long a gram for ideographic
density — **q=2–3 is the right CJK gram size**, exactly as the CJK-dedup literature recommends.

### 4.5 Cost (Table 5) and MinHash fidelity

µs/pair, single core: `simhash64` 0.13 · `minhash128` 3.2 · `containment` 5.3 ·
`jaccard_word3` 5.4 · `winnowing` 7.3 · `tfidf_cosine` 11.1 · `jaccard_char5` 12.2 ·
`bow_cosine` 12.3 · `ncd_zlib` 13.7 · `bm25` 33.6 · `ratcliff` 64 · `qgram3` 70 ·
`lcsubstr` 302 · `jaro_winkler` 480 · `lcs` 863 · `levenshtein` 946 · `smith_waterman` 1642 ·
`needleman_wunsch` 1719 · `greedy_tiling` 2405.

**Set/hash methods are 3–13 µs/pair; alignment/edit methods are 300–2400 µs/pair (100–700×
slower), are O(n·m) per pair with no fingerprint, and cannot do corpus-scale candidate
generation.** (These alignment costs are on *truncated 160-token* inputs; full documents are far
worse.) **MinHash (128 perms) tracks exact Jaccard** (ROC 0.981 vs 0.983) at ~2–3 R@P99 points
of variance cost — the expected accuracy/scale trade for candidate generation.

---

## 5. Qualitative evaluation (LLM rubric panel)

Three LLM judges scored all 19 algorithms 1–5 on eight rubric dimensions, each from a distinct
lens: **production-systems**, **IR/NLP-research**, **clone/plagiarism-detection**. Consensus
(mean of 3 judges), sorted by overall mean:

| algo | discr | edit | reord | fmt | interp | cost | script | nose-fit | **mean** | role (majority) |
|---|---|---|---|---|---|---|---|---|---|---|
| **winnowing** | 4.0 | 4.7 | 4.0 | 5.0 | 4.7 | 5.0 | 3.0 | 5.0 | **4.42** | primary |
| jaccard_char5 | 4.3 | 4.0 | 4.0 | 4.7 | 2.7 | 4.0 | 4.3 | 4.3 | 4.04 | verify |
| **tfidf_cosine** | 5.0 | 5.0 | 4.7 | 5.0 | 1.7 | 3.7 | 3.7 | 3.3 | 4.00 | verify |
| qgram3 | 4.0 | 4.0 | 4.3 | 5.0 | 2.7 | 3.0 | 5.0 | 4.0 | 4.00 | primary |
| jaccard_word3 | 4.0 | 2.7 | 3.3 | 5.0 | 3.0 | 5.0 | 4.3 | 4.3 | 3.96 | verify |
| containment_word3 | 4.0 | 3.3 | 4.0 | 5.0 | 3.0 | 4.3 | 4.3 | 3.7 | 3.96 | verify/primary |
| smith_waterman | 4.3 | 5.0 | 1.7 | 5.0 | 5.0 | 1.0 | 5.0 | 3.7 | 3.83 | verify/witness |
| **minhash128** | 4.0 | 2.7 | 3.3 | 5.0 | 1.7 | 5.0 | 4.0 | 4.3 | 3.75 | primary |
| ratcliff | 4.3 | 4.0 | 2.0 | 4.7 | 4.0 | 2.3 | 5.0 | 3.3 | 3.71 | verify |
| greedy_tiling | 3.7 | 3.0 | 3.0 | 4.7 | 5.0 | 1.0 | 4.3 | 3.0 | 3.46 | witness |
| lcs | 4.3 | 4.0 | 1.7 | 4.7 | 4.3 | 1.0 | 4.7 | 2.7 | 3.42 | witness |
| ncd_zlib | 4.0 | 4.0 | 4.0 | 4.3 | 1.0 | 2.0 | 4.3 | 2.0 | 3.21 | reject |
| levenshtein | 3.7 | 4.0 | 1.3 | 4.7 | 4.0 | 1.0 | 4.0 | 2.7 | 3.17 | verify |
| bow_cosine | 2.7 | 3.3 | 4.7 | 5.0 | 1.3 | 4.0 | 1.7 | 2.7 | 3.17 | **reject** |
| lcsubstr | 2.3 | 2.3 | 1.3 | 4.0 | 5.0 | 1.3 | 4.3 | 2.3 | 2.88 | witness |
| bm25 | 3.0 | 3.3 | 4.3 | 4.3 | 1.3 | 2.7 | 2.0 | 2.0 | 2.88 | **reject** |
| simhash64 | 2.0 | 2.3 | 4.0 | 4.7 | 1.0 | 5.0 | 1.3 | 2.3 | 2.83 | **reject** |
| jaro_winkler | 1.7 | 2.3 | 2.0 | 4.3 | 1.3 | 1.0 | 3.7 | 1.3 | 2.21 | **reject** |
| needleman_wunsch | 1.3 | 2.0 | 1.3 | 3.7 | 4.0 | 1.0 | 1.3 | 1.3 | 2.00 | **reject** |

**Unanimous panel conclusions:**
- **Consensus top-3:** winnowing, MinHash, TF-IDF (clone-lens swapped TF-IDF for smith_waterman
  as a witness — same pipeline, different stage emphasis).
- **Unanimous rejects:** bow_cosine, bm25, simhash64, jaro_winkler, needleman_wunsch.
- **Consensus pipeline:** char-gram MinHash-LSH + winnowing (candidate gen) → TF-IDF / qgram
  (verify, IDF kills siblings) → smith_waterman / greedy_tiling (span witness).
- **Consensus failure modes:** (1) alignment collapses on block/list reorder; (2) word-token IR
  collapses on CJK; (3) symmetric Jaccard misses small-in-large (containment needed); (4) without
  IDF, boilerplate/sibling sections inflate similarity; (5) SimHash/Jaro–Winkler can't separate
  even unrelated docs (scores 0.55–0.75 on negatives).

The qualitative panel, the quantitative results, and the literature survey are mutually
consistent — a strong triangulation.

---

## 6. Synthesis — recommended architecture for nose

No single algorithm is both scalable and witness-producing (literature, measurements, and panel
all agree). The recommended design is a **three-stage pipeline over a character-n-gram
substrate**, which preserves nose's determinism, single-binary, no-LLM, incremental, and
witness properties:

```
            ┌─ Stage 1: CANDIDATE GENERATION (cheap, incremental, sub-quadratic) ─┐
 Markdown   │  char n-gram shingles (q≈3 CJK / 5 Latin)                           │
 blocks ──▶ │  → MinHash(128) signatures + LSH banding  (order-invariant)         │
            │  → winnowing fingerprints in an inverted index (carries positions)  │
            │  → containment for "small section inside large doc"                  │
            └────────────────────────────┬───────────────────────────────────────┘
                                          ▼  (shortlist of candidate pairs)
            ┌─ Stage 2: VERIFY / RANK ────────────────────────────────────────────┐
            │  TF-IDF cosine over char-gram features (IDF down-weights boilerplate │
            │  → best topical-FP resistance);  q-gram on CJK                       │
            └────────────────────────────┬───────────────────────────────────────┘
                                          ▼  (confirmed pairs)
            ┌─ Stage 3: WITNESS / SPAN EXTRACTION ───────────────────────────────┐
            │  Smith–Waterman (affine gaps) for the exact contiguous copied span; │
            │  greedy string tiling when reorder is suspected (multi-tile);       │
            │  Myers diff to render the human-facing highlight                     │
            └─────────────────────────────────────────────────────────────────────┘
```

Why each choice:
- **Stage 1 must be order-invariant** because block/list reorder is common in Markdown and
  kills all single-thread alignment — so reorder handling lives entirely in the set/fingerprint
  layer. MinHash gives sub-quadratic candidate gen with a tunable S-curve; winnowing adds a
  positional fingerprint (and a recall guarantee for spans ≥ w+k−1); containment covers the
  asymmetric small-in-large case Jaccard misses.
- **Stage 2 must down-weight boilerplate** — TF-IDF's IDF is the single biggest precision lever
  against topically-related (sibling) non-duplicates (Table 3). The corpus-global-IDF
  incrementality cost is acceptable because Stage 2 runs only on the Stage-1 shortlist.
- **Stage 3 is where the O(n·m) aligners earn their cost** — only on confirmed pairs, to produce
  the exact span witness a refactoring tool must show. Affine gaps handle whole-section
  insertions; greedy string tiling handles reordered blocks; Myers diff renders the result.

**Substrate decision:** character n-grams (per-character tokens for no-space scripts), **not**
whitespace words — this is what makes the whole pipeline script-agnostic with no per-language
segmenter (a hard requirement for a single binary). CJK gram size q≈2–3; Latin q≈5.

This mirrors nose's existing declarative-track discipline: a domain-appropriate representation
(here, normalized char-gram + block structure) feeding cheap fingerprints, with expensive,
exact, witness-producing work gated behind a cheap candidate filter.

---

## 7. Threats to validity

- **Synthetic realism.** Edit injection (random token reword/indel, sentence reorder) is a
  proxy for human editing; real doc drift mixes rewording with semantic change. Mitigated by
  the **real** boilerplate set and by reporting per-profile curves rather than one aggregate.
- **Reword realism.** Random-token substitution destroys n-grams more uniformly than
  human rewording (which keeps phrases). This likely *understates* set-method recall at high
  edit ratios and *overstates* the gap to alignment — i.e., conservative for our recommendation.
- **Class balance.** The test sets are positive-heavy (latin 67% pos); a real dedup corpus is
  overwhelmingly negative (O(N²) pairs, O(N·d) duplicates). ROC is therefore optimistic under
  skew (Davis & Goadrich 2006); the honest metrics are **PR-AUC**, **R@P99** (both reported), and
  the hard-negative (sibling) ROC, which is the realistic precision stress. A production eval must
  re-weight to the true negative-heavy prior.
- **Gold-label trust.** Real labels come from provenance (Contributor Covenant signature/version)
  and synthetic from construction, so they are reliable here — but the literature warns gold sets
  can be badly mislabeled (BigCloneBench: 93% of weak Type-3/4 labels wrong). Any future
  human/LLM-labeled gold set must be sample-validated before trust.
- **Block granularity.** We operate on heading-rooted sections; whole-document and
  sliding-window units are not yet evaluated (next experiment).
- **Alignment truncation.** Alignment inputs capped at 160 tokens for tractability; this *helps*
  alignment methods (shorter = less reorder exposure), so their reorder weakness is, if anything,
  understated.
- **LLM judge bias.** Judges saw the quantitative tables, so their quant dimensions partly echo
  the numbers; their value-add is the qualitative dimensions (witness, nose-fit, failure modes)
  and the cross-lens consensus, which is where they agreed independently.

---

## 8. Recommendations & pre-registered next experiments

**Immediate recommendation.** Adopt the §6 three-stage pipeline; build Stage 1 first (MinHash +
winnowing over char n-grams) as the minimal end-to-end slice, since it is the scalable core and
the rest is gated behind it.

**Pre-registered experiments (measurement-first; record numbers, GO/NO-GO):**

1. **Stage-1 candidate-gen recall/precision at corpus scale.** Build MinHash-LSH + winnowing
   over all `bench/repos` Markdown; measure candidate-set recall against the labeled pairs and
   the candidate-reduction factor (pairs examined / N²). *GO if recall ≥ 0.98 at ≥ 100× reduction.*
2. **Unit granularity.** Compare block vs section vs sliding-window units on recall-vs-edit-ratio
   and on small-in-large detection (containment). *Record which unit maximizes R@P99.*
3. **Char-gram size sweep** {2,3,4,5} × {Latin, CJK}; confirm q≈3 CJK / q≈5 Latin and pick a
   single cross-script default. *Record the per-script ROC surface.*
4. **IDF necessity ablation.** TF-IDF vs plain cosine on the **hard-negative** (sibling) set;
   quantify the precision lift from IDF (Table 3 predicts ~0.02 ROC, larger at P99). *GO/NO-GO on
   whether corpus-global IDF complexity is worth the lift; if marginal, prefer a stateless
   alternative.*
5. **Witness quality.** On confirmed pairs, measure Smith–Waterman / GST span-localization
   accuracy against injected copy spans (IoU of reported vs true span). *Record IoU and cost.*
6. **Boilerplate suppression.** Compare **stop-shingle** filtering (drop shingles in > ~1% of
   corpus docs, Google US7698317) and **I-Match** two-sided IDF band (drop bottom-25% + top-25%
   IDF terms, Chowdhury 2002) on the Contributor-Covenant / license / badge false-positive
   class. *Record FP rate before/after, and confirm true template reuse is not dropped.*

**Out of scope (confirmed):** cross-lingual / translation detection; paraphrase / Type-4
semantic equivalence (needs an LLM). Rejected algorithms (do not implement): BM25, plain
bag-of-words, SimHash, Jaro–Winkler, Needleman–Wunsch, NCD.

---

## 9. Reproducibility

All artifacts under `tmp/md-dup/` (gitignored scratch):
- `src/lib.py` — normalization, tokenization, 19 algorithms (cached per-text reprs), metrics.
- `src/build_data.py` — real + synthetic dataset generation (seed 20260618).
- `src/run_eval.py` — parallel scoring → `out/metrics.json`, `curves.json`, `pair_scores.jsonl`.
- `src/post.py` — corrected operating-point curves, timing microbenchmark, casebook.
- `src/make_judge_input.py` — curated 22-case judge input.
- `out/` — `metrics.json`, `curves_fixed.json`, `timing.json`, `judges.json`, `digest.md`,
  `casebook.json`.

Determinism: fixed RNG seed; BLAKE2b hashing (no Python salted `hash()`); sorted file walks;
pure stdlib (Python 3.14, no numpy/sklearn). Re-running reproduces identical metrics.

Run: `python3 tmp/md-dup/src/build_data.py && python3 tmp/md-dup/src/run_eval.py && python3 tmp/md-dup/src/post.py`

---

## 10. References

Broder 1997 (resemblance/containment); Broder 2000 (near-dup filtering); Leskovec–Rajaraman–
Ullman, *Mining of Massive Datasets* ch.3 (MinHash/LSH); Charikar 2002 (SimHash); Manku, Jain &
Das Sarma 2007 (web near-dup at scale); Schleimer, Wilkerson & Aiken 2003 (winnowing/MOSS);
Robertson & Zaragoza 2009 (BM25); Stanford IIR (shingling, cosine). Levenshtein 1966;
Wagner–Fischer 1974; Backurs–Indyk 2015 (SETH lower bound); Damerau 1964; Jaro 1989 / Winkler
1990; Needleman–Wunsch 1970; Smith–Waterman 1981; Gotoh 1982 (affine gaps); Altschul et al.
1990 (BLAST); Ukkonen 1992 (q-grams); Gravano et al. 2001 (counting filter); Ondov et al. 2016
(Mash); Wise 1993 / Prechelt et al. (JPlag, GST); Cilibrasi & Vitányi 2005 (NCD); Myers 1986
(O(ND) diff); Huang & Wang 2008 (sentence-level LCS near-dup). Systems & methodology: Henzinger
2006 (shingling vs SimHash complementarity); Chowdhury et al. 2002 (I-Match, two-sided IDF band);
Theobald et al. 2008 (SpotSigs); Brin et al. 1995 (COPS); Shivakumar & Garcia-Molina 1995 (SCAM);
Prechelt et al. 2002 (JPlag); Potthast, Stein et al. 2010 + PAN-PC-11 (plagdet, obfuscation
taxonomy, TIRA); Davis & Goadrich 2006 (PR vs ROC under imbalance); Svajlenko et al. (mutation
framework); Krinke 2022 (BigCloneBench mislabeling); Lee et al. 2022 (suffix-array dedup); Google
US7698317 (stop-shingles); Manning–Raghavan–Schütze (IR textbook); Unicode UAX #15 (NFKC). Full
URLs in the three survey appendices retained with the harness.
