//! Type-4 clone detector over the normalized IL.
//!
//! Pipeline: normalize every file → extract units + features (subtree-shape
//! multiset, linearized tags, MinHash) → LSH candidate generation → structural
//! scoring (multiset Jaccard + LCS alignment) → union-find clustering. The
//! [`Detector`] trait makes the scorer pluggable so simhash / tf-idf / graph
//! variants can be compared later; v1 ships [`StructuralDetector`].

mod align;
mod cluster;
mod contiguous;
mod lsh;
mod minhash;
mod report;
mod units;

pub use report::{rank_families, RefactorFamily};
pub use units::UnitFeat;

use nose_il::{Corpus, Il, Interner};
use nose_normalize::NormalizeOptions;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct DetectOptions {
    pub min_lines: u32,
    pub min_tokens: usize,
    pub threshold: f64,
    pub minhash_k: usize,
    pub bands: usize,
    pub cfg_norm: bool,
    /// Enable dead-code / dead-assignment elimination (normalization).
    pub dce: bool,
    /// Weight of the Jaccard term vs the LCS-alignment term in the final score.
    pub jaccard_weight: f64,
    /// Extract sub-function block units (loops/ifs/try) in addition to
    /// functions/methods/classes. ON by default: measurement on the validated
    /// target showed gold clones are often sub-function fragments, and blocks lift
    /// recall (0.610→0.621), pool-precision (0.064→0.106) and AUC-PR (0.34→0.42)
    /// with HN-FP flat. Disable with `--no-blocks`.
    pub block_units: bool,
    /// Run the contiguous copy-paste channel alongside the structural one: a
    /// Rabin-Karp scan over each file's normalized-IL token stream that finds
    /// maximal duplicated runs *regardless of unit boundaries* (the Type-1/2 floor
    /// a token-based detector like jscpd catches). On by default for `scan`,
    /// off for the strict/gold `detect` path so Type-4 benchmark numbers are stable.
    pub contiguous: bool,
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions {
            min_lines: 5,
            min_tokens: 24,
            // 0.86: balanced operating point chosen from the unbiased precision
            // curve (§O). The 0.70–0.86 score bands are ~0% precision noise; 0.86
            // ~doubles precision (18%→33%) for a 0.07 recall cost and halves the
            // prediction count. Lower it for recall-completeness, raise for precision.
            threshold: 0.86,
            // 128/32 catches lower-similarity candidates (better recall ceiling)
            // at modest extra cost vs 64/16; bands=64 (rows=2) explodes candidates.
            minhash_k: 128,
            bands: 32,
            cfg_norm: true,
            dce: false,
            jaccard_weight: 0.5,
            block_units: true,
            contiguous: false,
        }
    }
}

/// Pluggable similarity scorer. Returns a score in `[0, 1]` for a candidate pair.
pub trait Detector: Sync {
    fn name(&self) -> &str;
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64;
}

/// The v1 default: weighted multiset Jaccard over subtree shapes, blended with an
/// LCS alignment over the linearized IL. A cheap Jaccard prefilter skips the
/// (more expensive) LCS for obviously-dissimilar pairs.
pub struct StructuralDetector {
    pub jaccard_weight: f64,
    /// Refactoring-candidate mode: disable the behavioral-precision gates
    /// (data-table, return-signature). Those gates demote "same shape, different
    /// data/operator" pairs — correct for behavioral-clone detection, but those
    /// pairs (locale-class families, comparison-operator families, sync/async
    /// wrappers) are exactly the refactoring candidates a human wants to review.
    /// Measured: under a refactoring-worthiness rubric, candidate mode (gates off,
    /// thr 0.70) surfaces ~4.5k pairs at ~99% review-worthy.
    pub candidate_mode: bool,
    /// Acceptance threshold, used only for a score-preserving early-exit (RANSAC and
    /// the gates can only lower the score below `wv·vj + ws·sj + wr`, so a pair whose
    /// upper bound is below threshold is rejected regardless — skip the alignment).
    /// 0.0 disables it.
    pub accept_threshold: f64,
}

impl StructuralDetector {
    /// Behavioral-clone detector: gates on (high precision, ~78% behavioral).
    pub fn strict(jaccard_weight: f64) -> Self {
        Self {
            jaccard_weight,
            candidate_mode: false,
            accept_threshold: 0.0,
        }
    }
    /// Refactoring-candidate detector: gates off (recall-oriented, ~99% review-worthy).
    pub fn candidates(jaccard_weight: f64) -> Self {
        Self {
            jaccard_weight,
            candidate_mode: true,
            accept_threshold: 0.0,
        }
    }
    /// Enable the threshold early-exit (set to the run's acceptance threshold).
    pub fn with_threshold(mut self, t: f64) -> Self {
        self.accept_threshold = t;
        self
    }
}

impl Detector for StructuralDetector {
    fn name(&self) -> &str {
        if self.candidate_mode {
            "structural-candidates"
        } else {
            "structural"
        }
    }

    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64 {
        // Oracle-certified fast path (§AJ): an identical value-graph fingerprint means
        // behaviorally-equal — `nose verify` proved fingerprint-equality ⟹ behavior
        // -equality across the corpus (0 false merges). So accept an exact match
        // outright, *regardless of syntactic divergence* — this is what lets a true
        // Type-4 clone (loop ≡ reduce ≡ comprehension) be detected even though its
        // shapes differ. Guarded by a minimum fingerprint size so trivial units don't
        // collapse. The size gate (min_tokens) already excludes tiny units upstream.
        if a.value.len() >= 6 && a.value == b.value {
            return 1.0;
        }
        // Score = wv·vj + ws·sj + wr·ransac (defaults reproduce the prior
        // 0.5·(0.6vj+0.4sj)+0.5·ransac = 0.3vj+0.2sj+0.5ransac). vj is the semantic
        // signal (value-graph, string/literal-aware), sj the syntactic, ransac the
        // order-sensitive alignment. Weights are env-tunable for the §P5 sweep.
        // §AH two-mode split: strict (behavioral) mode trusts the value graph;
        // candidate (refactoring) mode is structure-dominant, so two units with the
        // same skeleton but a different operator (a sum-loop vs a product-loop) — now
        // behaviorally distinct in the value graph (`Reduce(Add)` vs `Reduce(Mul)`) —
        // still group as a refactoring family worth a human's review.
        let (wv, ws, wr) = if self.candidate_mode {
            candidate_score_weights()
        } else {
            score_weights()
        };
        let vj = align::multiset_jaccard(&a.value, &b.value);
        let sj = align::multiset_jaccard(&a.shapes, &b.shapes);
        if 0.6 * vj + 0.4 * sj < 0.15 {
            return 0.6 * vj + 0.4 * sj; // prefilter: not worth the alignment DP
        }
        // Score-preserving early-exit: RANSAC (≤1) and the gates only lower the
        // score, so if the upper bound `wv·vj+ws·sj+wr` can't reach threshold the
        // pair is rejected anyway — skip the alignment DP.
        if wv * vj + ws * sj + wr < self.accept_threshold {
            return wv * vj + ws * sj + wr;
        }
        let l = align::ransac_ratio(&a.linear, &b.linear);
        let score = wv * vj + ws * sj + wr * l;
        // Refactoring-candidate mode keeps the raw similarity — the gates below
        // demote precisely the near-duplicate families that are good refactor targets.
        // (Tested: applying the data-table gate here to demote locale/version-table
        // families gave no precision lift and cost recall on the labelset — §X.)
        if self.candidate_mode {
            return score;
        }
        // Data-table gate: a unit dominated by literal constants (a locale/message
        // map, a config table) is a clone of another only if the constants agree.
        // Cap such pairs by their literal Jaccard — surgically demotes "same shape,
        // different data" false positives without touching algorithmic clones (which
        // have few constants, so the gate never triggers; recall is unaffected).
        let (dh_ratio, dh_abs) = data_heavy_params();
        let data_heavy = |u: &UnitFeat| {
            !u.value.is_empty()
                && (u.lits.len() as f64 / u.value.len() as f64 >= dh_ratio
                    || u.lits.len() >= dh_abs)
        };
        if data_heavy(a) && data_heavy(b) {
            return score.min(align::multiset_jaccard(&a.lits, &b.lits));
        }
        // Return-signature gate: two units that return DIFFERENT computed values are
        // not behavioral clones, however similar their bodies. When both return
        // something, cap the score by `ret_base + (1-ret_base)·return_jaccard`, so a
        // total return mismatch (e.g. `<` vs `<=`, an extra effect) caps below the
        // operating threshold while a return match leaves the score untouched.
        if !a.returns.is_empty() && !b.returns.is_empty() {
            let rj = align::multiset_jaccard(&a.returns, &b.returns);
            let base = ret_gate_base();
            return score.min(base + (1.0 - base) * rj);
        }
        score
    }
}

/// Final-score weights (vj, sj, ransac). Env-overridable for parameter search;
/// defaults reproduce the historical blend.
fn score_weights() -> (f64, f64, f64) {
    use std::sync::OnceLock;
    static W: OnceLock<(f64, f64, f64)> = OnceLock::new();
    *W.get_or_init(|| {
        let g = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        // §P5: RANSAC down-weighted 0.5→0.2 (it ignores string values, so it kept
        // "same shape, different data" locale-table FPs high); weight shifted to the
        // value-graph + shape Jaccard. Labeled precision 31.7%→45.2%, recall held.
        (g("NOSE_WV", 0.5), g("NOSE_WS", 0.3), g("NOSE_WR", 0.2))
    })
}

/// Candidate-mode weights (§AH): structure-dominant. The behavioral value graph is
/// down-weighted relative to syntactic shape so refactoring families that share a
/// skeleton but differ in a behavior-defining operator/constant (sum↔product,
/// `<`↔`<=`) still surface for review — the recall-oriented purpose. Strict mode uses
/// the behavioral weights above. Env-overridable for tuning.
fn candidate_score_weights() -> (f64, f64, f64) {
    use std::sync::OnceLock;
    static W: OnceLock<(f64, f64, f64)> = OnceLock::new();
    *W.get_or_init(|| {
        let g = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        (g("NOSE_CWV", 0.3), g("NOSE_CWS", 0.5), g("NOSE_CWR", 0.2))
    })
}

/// Data-table criteria: a unit is a "data table" (subject to the literal-match
/// gate) if its literal/total value-node ratio ≥ `dh_ratio` OR it has ≥ `dh_abs`
/// literal nodes in absolute terms — the latter catches locale *classes* whose
/// formatting methods dilute the ratio below threshold. Env-overridable for §P7.
fn data_heavy_params() -> (f64, usize) {
    use std::sync::OnceLock;
    static P: OnceLock<(f64, usize)> = OnceLock::new();
    *P.get_or_init(|| {
        let r = std::env::var("NOSE_DH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.20);
        let n = std::env::var("NOSE_DHN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(25);
        (r, n)
    })
}

/// Return-signature gate base: a unit pair with totally mismatched return values
/// is capped at this score. 1.0 disables the gate. Env-overridable for §P11.
fn ret_gate_base() -> f64 {
    use std::sync::OnceLock;
    static B: OnceLock<f64> = OnceLock::new();
    *B.get_or_init(|| {
        std::env::var("NOSE_RET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.80)
    })
}

#[derive(Serialize, Clone)]
pub struct Loc {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lang: String,
    /// What kind of syntactic unit this site is (function/method/class/block) —
    /// lets the report suggest the right refactor (helper vs base class).
    pub kind: nose_il::UnitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Size of this unit's value graph (number of distinct computed values). A
    /// unit that computes things has a rich value graph; a pure type definition
    /// or data/match table has a near-empty one and can only match on *shape* —
    /// the signal the refactor ranking uses to discount structural-only families.
    pub sem: usize,
}

#[derive(Serialize)]
pub struct DupPair {
    pub left: Loc,
    pub right: Loc,
    pub score: f64,
    pub cross_language: bool,
}

#[derive(Serialize)]
pub struct Group {
    pub score: f64,
    pub members: Vec<Loc>,
}

#[derive(Serialize)]
pub struct Metrics {
    pub files: usize,
    pub units: usize,
    pub candidate_pairs: usize,
    pub accepted_pairs: usize,
    pub groups: usize,
}

#[derive(Serialize)]
pub struct Report {
    pub tool: &'static str,
    pub version: &'static str,
    pub detector: String,
    pub duplicates: Vec<DupPair>,
    pub groups: Vec<Group>,
    pub metrics: Metrics,
}

fn loc_of(u: &UnitFeat) -> Loc {
    Loc {
        file: u.path.clone(),
        start_line: u.start_line,
        end_line: u.end_line,
        lang: u.lang.name().to_string(),
        kind: u.kind,
        name: u.name.clone(),
        sem: u.value.len(),
    }
}

/// Two units from the same file where one span contains the other (e.g. a method
/// and its enclosing class) — exclude these trivial nesting matches.
fn is_nested(a: &UnitFeat, b: &UnitFeat) -> bool {
    a.path == b.path
        && ((a.start_line <= b.start_line && a.end_line >= b.end_line)
            || (b.start_line <= a.start_line && b.end_line >= a.end_line))
}

/// One extracted unit's location, for ceiling/diagnostic dumps.
#[derive(Serialize)]
pub struct UnitLoc {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Diagnostic dump: all extracted units and all LSH candidate index pairs (into
/// `units`). Lets the evaluator split recall loss across extraction / candidate
/// generation / scoring.
pub struct Dump {
    pub units: Vec<UnitLoc>,
    pub candidates: Vec<(u32, u32)>,
}

/// Run detection over a (raw) corpus and produce a report.
pub fn detect(corpus: &Corpus, opts: &DetectOptions, detector: &dyn Detector) -> Report {
    detect_with_dump(corpus, opts, detector).0
}

/// Per-stage wall-clock timing, printed to stderr when `NOSE_TIME` is set. A
/// zero-cost no-op otherwise (the `Instant`s are cheap; only the env check gates
/// printing).
struct StageTimer {
    on: bool,
    start: std::time::Instant,
    last: std::time::Instant,
}
impl StageTimer {
    fn new() -> Self {
        let now = std::time::Instant::now();
        StageTimer {
            on: std::env::var_os("NOSE_TIME").is_some(),
            start: now,
            last: now,
        }
    }
    fn lap(&mut self, stage: &str) {
        let now = std::time::Instant::now();
        if self.on {
            eprintln!(
                "  [time] {stage:<12} {:>7.1}ms   (total {:>7.1}ms)",
                now.duration_since(self.last).as_secs_f64() * 1e3,
                now.duration_since(self.start).as_secs_f64() * 1e3,
            );
        }
        self.last = now;
    }
}

/// Like [`detect`] but also returns the unit/candidate [`Dump`] for diagnostics.
/// Normalize one file and extract its detection units. The resulting [`UnitFeat`]s
/// are interner-independent (every feature is a content-derived hash), so a caller
/// may pass a throwaway per-file interner — which is exactly what makes caching a
/// file's units by its source-content hash sound.
pub fn units_of_file(il: &Il, interner: &Interner, opts: &DetectOptions) -> Vec<UnitFeat> {
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    let n = nose_normalize::normalize(il, interner, &norm_opts);
    units::extract(
        &n,
        interner,
        &seeds,
        opts.min_lines,
        opts.min_tokens,
        opts.block_units,
    )
}

pub fn detect_with_dump(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    // Normalize each file and extract its units in one fused parallel pass — a file's
    // normalized IL stays hot in cache through extraction and is freed immediately,
    // rather than materializing the whole normalized corpus first.
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    // Normalize each file once; extract its units and (when enabled) its contiguous
    // token stream from the same hot normalized IL.
    let per_file: Vec<(Vec<UnitFeat>, Option<contiguous::Stream>)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let n = nose_normalize::normalize(il, &corpus.interner, &norm_opts);
            let units = units::extract(
                &n,
                &corpus.interner,
                &seeds,
                opts.min_lines,
                opts.min_tokens,
                opts.block_units,
            );
            // Build the contiguous stream from the *raw* IL, not the normalized one:
            // alpha-renaming is function-scoped, so a copy-pasted block's variable
            // cids depend on its enclosing function and identical blocks diverge.
            // Raw tokens (names content-hashed by `node_tag`) are stable across files
            // — matching jscpd's name-based copy-paste. Renamed Type-2/3/4 is the
            // structural channel's job.
            let stream = opts
                .contiguous
                .then(|| contiguous::stream(il, &corpus.interner));
            (units, stream)
        })
        .collect();
    let mut units: Vec<UnitFeat> = Vec::new();
    let mut streams: Vec<contiguous::Stream> = Vec::new();
    for (u, s) in per_file {
        units.extend(u);
        if let Some(s) = s {
            streams.push(s);
        }
    }
    clk.lap("normalize+extract");

    let (mut report, dump) = detect_from_units(units, corpus.files.len(), opts, detector);
    if opts.contiguous {
        let extra = contiguous::detect(&streams);
        clk.lap("contiguous");
        report.metrics.groups += extra.len();
        report.groups.extend(extra);
    }
    (report, dump)
}

/// Run candidate-generation → scoring → clustering over already-built `units`,
/// producing the report and diagnostic dump. Split from unit extraction so a caller
/// (the CLI's cache path) can supply units it built — and cached — per file.
/// `files` is the source file count, for the report's metrics only.
pub fn detect_from_units(
    units: Vec<UnitFeat>,
    files: usize,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    // 3. LSH candidate generation over the value-graph fingerprint. (A second
    //    coarse atom-bag channel was tried: it lifted candidate-reachable
    //    27%→35% but the surfaced divergent pairs were not separable by the
    //    scorer — net F1 flat, 6× candidates. Removed; see notes.)
    let candidates = lsh::candidates(units.len(), |i| units[i].minhash.as_slice(), opts.bands);
    clk.lap("candidates");

    // 4. Score candidates in parallel; keep accepted pairs.
    let accepted: Vec<(usize, usize, f64)> = candidates
        .par_iter()
        .filter_map(|&(i, j)| {
            if is_nested(&units[i], &units[j]) {
                return None;
            }
            let s = detector.score(&units[i], &units[j]);
            (s >= opts.threshold).then_some((i, j, s))
        })
        .collect();

    clk.lap("score");

    // 5. Cluster.
    let mut uf = cluster::UnionFind::new(units.len());
    for &(i, j, _) in &accepted {
        uf.union(i, j);
    }
    let raw_groups = uf.groups(units.len());
    clk.lap("cluster");

    // Build pair output (sorted by score desc).
    let mut duplicates: Vec<DupPair> = accepted
        .iter()
        .map(|&(i, j, s)| DupPair {
            left: loc_of(&units[i]),
            right: loc_of(&units[j]),
            score: round3(s),
            cross_language: units[i].lang != units[j].lang,
        })
        .collect();
    duplicates.sort_by(|a, b| b.score.total_cmp(&a.score));

    // Group score = mean pair score among members (approximated by member count).
    let groups: Vec<Group> = raw_groups
        .iter()
        .map(|members| {
            let score = group_score(members, &accepted);
            Group {
                score: round3(score),
                members: members.iter().map(|&m| loc_of(&units[m])).collect(),
            }
        })
        .collect();

    let report = Report {
        tool: "nose",
        version: env!("CARGO_PKG_VERSION"),
        detector: detector.name().to_string(),
        metrics: Metrics {
            files,
            units: units.len(),
            candidate_pairs: candidates.len(),
            accepted_pairs: accepted.len(),
            groups: groups.len(),
        },
        duplicates,
        groups,
    };

    let dump = Dump {
        units: units
            .iter()
            .map(|u| UnitLoc {
                path: u.path.clone(),
                start_line: u.start_line,
                end_line: u.end_line,
                lang: u.lang.name().to_string(),
                name: u.name.clone(),
            })
            .collect(),
        candidates: candidates
            .iter()
            .map(|&(i, j)| (i as u32, j as u32))
            .collect(),
    };

    (report, dump)
}

fn group_score(members: &[usize], accepted: &[(usize, usize, f64)]) -> f64 {
    let set: rustc_hash::FxHashSet<usize> = members.iter().copied().collect();
    let mut sum = 0.0;
    let mut n = 0;
    for &(i, j, s) in accepted {
        if set.contains(&i) && set.contains(&j) {
            sum += s;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}
