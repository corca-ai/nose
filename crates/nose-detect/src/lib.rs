//! Clone detection over the normalized IL.
//!
//! Pipeline: normalize every file → extract units + features (value fingerprints,
//! subtree-shape multisets, linearized tags, MinHash signatures) → channel-specific
//! candidate generation (value for semantic, shape for near, token streams for syntax)
//! → scoring/acceptance → union-find clustering. The [`Detector`] trait makes the unit
//! scorer pluggable so simhash / tf-idf / graph variants can be compared later.

mod abstraction;
mod align;
mod cluster;
mod contiguous;
mod fragment;
mod il_utils;
mod lsh;
mod minhash;
mod report;
mod strict_exact;
mod units;
mod witness;

pub use contiguous::Stream;
pub use fragment::{
    fragment_behavior, free_input_cids, synthesize_wrapper, Effect, EffectSite, Exit,
    FragmentContract, FragmentKind, Place, ProofFacts,
};
pub use report::{is_test_loc, rank_families, RefactorFamily, VaryingSpot};
pub use units::{unit_dags_at, UnitFeat};
pub use witness::{graded_witness, GradedWitness, WitnessHole};

/// Build one file's syntax-channel token stream from its (raw) IL. Exposed so the
/// CLI's `--cache-dir` can cache it per file and pass it to [`detect_from_units`] — the
/// counterpart to [`units_of_file`] for the syntax channel.
pub fn file_stream(il: &Il, interner: &Interner) -> Stream {
    contiguous::stream(il, interner)
}

use nose_il::{Corpus, Il, Interner, NodeId, NodeKind, UnitKind};
use nose_normalize::NormalizeOptions;
use nose_semantics::ValueLaw;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

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
    /// Extract sub-function block units (loops/ifs/try plus exact statement
    /// fragments) in addition to functions/methods/classes. ON by default:
    /// measurement on the validated target showed gold clones are often
    /// sub-function fragments, and blocks lift recall (0.610→0.621),
    /// pool-precision (0.064→0.106) and AUC-PR (0.34→0.42) with HN-FP flat.
    /// Disable with `--no-blocks`.
    pub block_units: bool,
    /// Minimum duplicated run size for the contiguous copy-paste channel, in IL
    /// tokens. This is separate from structural unit size internally, but the CLI's
    /// `scan --min-size` intentionally drives both so syntax gates have one size knob.
    pub contiguous_min_tokens: usize,
    /// Minimum duplicated run size for the contiguous copy-paste channel, in source
    /// lines. The CLI's `scan --min-lines` drives both unit extraction and this floor.
    pub contiguous_min_lines: u32,
    /// Run the syntax copy-paste channel: a Rabin-Karp scan over each file's IL token
    /// stream that finds
    /// maximal duplicated runs *regardless of unit boundaries* (the Type-1/2 floor
    /// a token-based detector like jscpd catches). Enabled by `scan --mode syntax`,
    /// and off for the strict/gold `detect` path so Type-4 benchmark numbers are stable.
    pub contiguous: bool,
    /// Run the unit detector used by the semantic and near channels. Turning this off
    /// leaves only any enabled syntax copy-paste channel.
    pub structural: bool,
    /// Generate structural candidates from value fingerprints. This is the semantic
    /// Type-4 path: loop/reduce/comprehension rewrites converge here even when their
    /// surface shape differs.
    pub value_candidates: bool,
    /// Generate structural candidates from syntactic shape fingerprints. This is the
    /// near Type-3 path: code can reach scoring even when behavior-defining literals or
    /// operators differ and therefore the value fingerprint no longer matches.
    pub shape_candidates: bool,
    /// Build syntactic unit features (`shapes`, `shape_minhash`, `linear`) for fuzzy
    /// structural scoring. Exact semantic scans do not need them: candidate generation
    /// and scoring both use the value graph only.
    pub shape_features: bool,
    /// Attach experimental abstraction witnesses to near-derived families whose normalized
    /// structure differs by exactly one supported literal leaf. This is a weak refactoring
    /// claim and never participates in exact semantic acceptance.
    pub abstraction_witnesses: bool,
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
            contiguous_min_tokens: 24,
            contiguous_min_lines: 5,
            contiguous: false,
            structural: true,
            value_candidates: true,
            shape_candidates: false,
            shape_features: true,
            abstraction_witnesses: false,
        }
    }
}

/// Pluggable similarity scorer. Returns a score in `[0, 1]` for a candidate pair.
pub trait Detector: Sync {
    fn name(&self) -> &str;
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64;
}

/// A no-op scorer used when a scan mode intentionally runs only the contiguous
/// copy-paste channel.
pub struct CopyPasteDetector;

impl Detector for CopyPasteDetector {
    fn name(&self) -> &str {
        "copy-paste"
    }

    fn score(&self, _a: &UnitFeat, _b: &UnitFeat) -> f64 {
        0.0
    }
}

/// Exact behavioral scorer: accept only the oracle-backed value-graph fast path.
/// This gives the `semantic` scan channel a high-confidence Type-4 surface without fuzzy
/// structural similarity.
pub struct ExactBehaviorDetector;

const EXACT_VALUE_MIN: usize = 4;

/// Can this unit ever participate in the exact `semantic` channel's merge claim?
/// The product asserts behavioral equality only for strict-exact-safe units whose
/// value fingerprint clears the degenerate-size floor — the same two gates
/// `ExactBehaviorDetector` and the candidate value-accept apply. The verify
/// oracle's HARD soundness gate is scoped to exactly this surface; collisions
/// between lossy fingerprints are diagnostics, not product false merges (#210).
pub fn exact_claim_eligible(u: &UnitFeat) -> bool {
    exact_claim_eligible_parts(u.exact_safe, u.value.len())
}

/// The exact-claim gate when the caller already has the two relevant facts.
pub fn exact_claim_eligible_parts(exact_safe: bool, value_len: usize) -> bool {
    exact_safe && value_len >= EXACT_VALUE_MIN
}

/// Strict exact-safety by source-line span for known roots.
///
/// `verify` already computes value fingerprints for the normalized functions it can
/// afford to interpret. This helper lets it reuse those fingerprints and ask only for
/// the exact-safety half of the product claim, without running full unit extraction for
/// soon-to-be-excluded oversized functions.
pub fn exact_safe_roots_by_span(
    il: &Il,
    interner: &Interner,
    roots: &[NodeId],
) -> HashMap<(u32, u32), bool> {
    let facts = strict_exact::StrictFacts::collect(il, interner);
    roots
        .iter()
        .map(|&root| {
            let span = il.node(root).span;
            (
                (span.start_line, span.end_line),
                strict_exact::strict_exact_safe_tree(il, interner, &facts, root),
            )
        })
        .collect()
}

impl Detector for ExactBehaviorDetector {
    fn name(&self) -> &str {
        "exact-behavior"
    }

    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64 {
        if a.exact_safe && b.exact_safe && a.value.len() >= EXACT_VALUE_MIN && a.value == b.value {
            1.0
        } else {
            0.0
        }
    }
}

const EXACT_VALUE_BUCKET_ALL_PAIRS_CAP: usize = 48;

/// The v1 default: weighted multiset Jaccard over subtree shapes, blended with an
/// LCS alignment over the linearized IL. A cheap Jaccard prefilter skips the
/// (more expensive) LCS for obviously-dissimilar pairs.
pub struct StructuralDetector {
    pub jaccard_weight: f64,
    /// Accept exact value-fingerprint matches before fuzzy structural scoring. Scan's
    /// `near` channel disables this so Type-3 near-duplicates stay separate from the
    /// exact semantic Type-4 channel.
    pub exact_behavior: bool,
    /// Near-candidate mode: disable the behavioral-precision gates
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
            exact_behavior: true,
            candidate_mode: false,
            accept_threshold: 0.0,
        }
    }
    /// Near-candidate detector: gates off (recall-oriented, ~99% review-worthy).
    pub fn candidates(jaccard_weight: f64) -> Self {
        Self {
            jaccard_weight,
            exact_behavior: true,
            candidate_mode: true,
            accept_threshold: 0.0,
        }
    }
    /// Disable the exact Type-4 fast path, leaving this detector to score only fuzzy
    /// near-duplicate structure.
    pub fn without_exact_behavior(mut self) -> Self {
        self.exact_behavior = false;
        self
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
        if self.exact_behavior
            && a.exact_safe
            && b.exact_safe
            && a.value.len() >= EXACT_VALUE_MIN
            && a.value == b.value
        {
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
        let (wv, ws, wr) = score_weights(self.candidate_mode);
        let vj = align::multiset_jaccard(&a.value, &b.value);
        let sj = align::multiset_jaccard(&a.shapes, &b.shapes);
        // Candidate mode trusts the value graph: a near-identical value fingerprint — produced
        // AFTER semantic canonicalization (a `.then`-chain ≡ await code, a loop ≡ a
        // comprehension) — is the strongest refactoring signal there is, even when the
        // syntactic shapes diverge and the unit is NOT exact-safe (impure: async, I/O, opaque
        // calls). The shape-dominant blend below would miss these, so accept a very-high `vj`
        // directly. Impure units never reach the exact channel, so this is the only place such
        // behaviorally-convergent pairs can surface. Tight threshold + size floor keep it precise.
        if self.candidate_mode
            && a.value.len() >= EXACT_VALUE_MIN
            && b.value.len() >= EXACT_VALUE_MIN
            && vj >= candidate_value_accept()
        {
            return vj;
        }
        // Partial / sub-DAG clone: the units share a rare heavy anchor (an extractable common
        // sub-computation) even though the whole-unit blend is low. Surface it for review at a
        // score above the near floor but below a full clone — it's a real refactor lead (pull
        // the shared computation into a helper), just a partial one. Keep the higher of the two.
        if self.candidate_mode {
            let shared = shared_anchor_weight(&a.anchors, &b.anchors);
            if shared > 0 {
                return (wv * vj + ws * sj).max(anchor_partial_score(shared));
            }
        }
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
        // Near-candidate mode keeps the raw similarity — the gates below
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

/// Surfacing score for a partial / sub-DAG clone, GRADED by the shared sub-DAG's weight: a
/// minimal shared computation sits at the floor (just above the near threshold so it appears);
/// a larger shared computation saturates toward the cap (still below a full clone). So a pair
/// sharing a big extractable chunk ranks above one sharing a marginal one. Env-overridable.
fn anchor_partial_score(weight: u32) -> f64 {
    let floor: f64 = env_or("NOSE_ANCHOR_SCORE", 0.72);
    let cap: f64 = env_or("NOSE_ANCHOR_SCORE_CAP", 0.90);
    let half: f64 = env_or("NOSE_ANCHOR_SCORE_REF", 60.0_f64).max(1.0); // extra weight at half-saturation
    let extra = (f64::from(weight) - f64::from(nose_normalize::anchor_min_weight())).max(0.0);
    floor + (cap - floor) * (extra / (extra + half))
}

/// Value-Jaccard threshold above which candidate mode accepts a pair on the value graph alone
/// (behaviorally convergent despite shape divergence — e.g. async `.then` ≡ await). Deliberately
/// high so it only fires on near-identical post-canonicalization fingerprints. Env-overridable.
fn candidate_value_accept() -> f64 {
    use std::sync::OnceLock;
    static V: OnceLock<f64> = OnceLock::new();
    *V.get_or_init(|| env_or("NOSE_CAND_VJ", 0.90))
}

/// Final-score weights (vj, sj, ransac). Env-overridable for parameter search.
fn score_weights(candidate_mode: bool) -> (f64, f64, f64) {
    use std::sync::OnceLock;
    static STRICT: OnceLock<(f64, f64, f64)> = OnceLock::new();
    static CANDIDATE: OnceLock<(f64, f64, f64)> = OnceLock::new();

    if candidate_mode {
        return cached_score_weights(
            &CANDIDATE,
            ("NOSE_CWV", "NOSE_CWS", "NOSE_CWR"),
            (0.3, 0.5, 0.2),
        );
    }

    cached_score_weights(
        &STRICT,
        // §P5: RANSAC down-weighted 0.5→0.2 (it ignores string values, so it kept
        // "same shape, different data" locale-table FPs high); weight shifted to the
        // value-graph + shape Jaccard. Labeled precision 31.7%→45.2%, recall held.
        ("NOSE_WV", "NOSE_WS", "NOSE_WR"),
        (0.5, 0.3, 0.2),
    )
}

fn cached_score_weights(
    cache: &'static std::sync::OnceLock<(f64, f64, f64)>,
    keys: (&'static str, &'static str, &'static str),
    defaults: (f64, f64, f64),
) -> (f64, f64, f64) {
    *cache.get_or_init(|| {
        (
            env_or(keys.0, defaults.0),
            env_or(keys.1, defaults.1),
            env_or(keys.2, defaults.2),
        )
    })
}

/// Data-table criteria: a unit is a "data table" (subject to the literal-match
/// gate) if its literal/total value-node ratio ≥ `dh_ratio` OR it has ≥ `dh_abs`
/// literal nodes in absolute terms — the latter catches locale *classes* whose
/// formatting methods dilute the ratio below threshold. Env-overridable for §P7.
fn data_heavy_params() -> (f64, usize) {
    use std::sync::OnceLock;
    static P: OnceLock<(f64, usize)> = OnceLock::new();
    *P.get_or_init(|| (env_or("NOSE_DH", 0.20), env_or("NOSE_DHN", 25)))
}

/// Return-signature gate base: a unit pair with totally mismatched return values
/// is capped at this score. 1.0 disables the gate. Env-overridable for §P11.
fn ret_gate_base() -> f64 {
    use std::sync::OnceLock;
    static B: OnceLock<f64> = OnceLock::new();
    *B.get_or_init(|| env_or("NOSE_RET", 0.80))
}

fn env_or<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Copy,
{
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[derive(Serialize, Clone)]
pub struct EnclosingUnit {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub kind: UnitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub unit_key: String,
}

impl EnclosingUnit {
    pub fn refresh_unit_key(&mut self) {
        self.unit_key = unit_key(
            &self.file,
            self.kind,
            self.name.as_deref(),
            self.start_line,
            self.end_line,
        );
    }
}

#[derive(Serialize, Clone)]
pub struct Loc {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lang: String,
    /// What kind of syntactic unit this site is (function/method/class/block) —
    /// lets the report suggest the right refactor (helper vs base class).
    pub kind: UnitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Size of this unit's value graph (number of distinct computed values). A
    /// unit that computes things has a rich value graph; a pure type definition
    /// or data/match table has a near-empty one and can only match on *shape* —
    /// the signal the refactor ranking uses to discount structural-only families.
    pub sem: usize,
    /// Explicit source-line span so consumers do not have to recalculate inclusive
    /// ranges. Kept for every location, not only fragments.
    pub span_lines: u32,
    /// Stable normalized-token span used by the detector's size gates.
    pub span_tokens: usize,
    /// Whether this location is an exact sub-function fragment rather than a whole
    /// function/method/class location.
    pub is_fragment: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_kind: Option<FragmentKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_unit: Option<EnclosingUnit>,
    /// The location sits inside an inline test module (`mod tests`) — counted
    /// as test scope even when the file path looks like production (#226).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_test_module: bool,
    /// The file's head carries a generated-code marker (`Generated by …`,
    /// `@generated`, `Code generated … DO NOT EDIT`). Set at the presentation
    /// layer (needs source); a family whose every location looks generated is
    /// dropped from reports, mirroring the path-based generated policy (#224 —
    /// re2c output used to top a default surface with no signal in the JSON).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looks_generated: bool,
    /// For a sub-DAG (partial) clone, the inclusive source line range AT THIS SITE of the heavy
    /// shared computation the family is grouped on (the anchor present in every member). `None`
    /// when the family shares no heavy sub-DAG, or the anchor carries no source span. Lets a
    /// partial clone point at *where* the shared computation lives in each copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_subdag: Option<(u32, u32)>,
}

/// Inclusive source-line range used to construct a [`Loc`].
#[derive(Clone, Copy)]
pub struct LineSpan {
    /// First 1-based source line included in the location.
    pub start_line: u32,
    /// Last 1-based source line included in the location.
    pub end_line: u32,
}

impl LineSpan {
    /// Build an inclusive source-line range.
    pub fn new(start_line: u32, end_line: u32) -> Self {
        Self {
            start_line,
            end_line,
        }
    }

    /// Inclusive line count, saturating for malformed ranges.
    pub fn line_count(self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Constructor input for [`Loc`].
///
/// Keeping this as a named struct makes location metadata additions explicit at call sites
/// without widening a positional constructor.
#[derive(Clone)]
pub struct LocInit {
    /// Source file path as reported to users.
    pub file: String,
    /// Inclusive source-line range.
    pub source_span: LineSpan,
    /// Normalized language name.
    pub lang: String,
    /// Syntactic unit kind at this location.
    pub kind: UnitKind,
    /// Optional function/method/class name.
    pub name: Option<String>,
    /// Value-graph size for this location.
    pub sem: usize,
    /// Normalized-token span used by detector size gates.
    pub span_tokens: usize,
}

impl Loc {
    pub fn new(init: LocInit) -> Self {
        let LocInit {
            file,
            source_span,
            lang,
            kind,
            name,
            sem,
            span_tokens,
        } = init;
        Loc {
            file,
            start_line: source_span.start_line,
            end_line: source_span.end_line,
            lang,
            kind,
            name,
            sem,
            span_lines: source_span.line_count(),
            span_tokens,
            is_fragment: false,
            fragment_kind: None,
            reason_code: None,
            enclosing_unit: None,
            in_test_module: false,
            looks_generated: false,
            shared_subdag: None,
        }
    }
}

#[derive(Serialize)]
pub struct DupPair {
    pub left: Loc,
    pub right: Loc,
    pub score: f64,
    pub cross_language: bool,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct AbstractionWitness {
    pub claim: &'static str,
    pub basis: &'static str,
    pub members_checked: u32,
    pub reason_code: &'static str,
    pub template_format: &'static str,
    pub template: Vec<String>,
    pub holes: Vec<AbstractionHole>,
    pub caveats: Vec<&'static str>,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct AbstractionHole {
    pub index: u32,
    pub template_index: u32,
    pub kind: &'static str,
    pub role: &'static str,
    pub left: &'static str,
    pub right: &'static str,
    pub observed: Vec<&'static str>,
    pub left_line: u32,
    pub right_line: u32,
}

#[derive(Serialize)]
pub struct Group {
    pub score: f64,
    pub members: Vec<Loc>,
    #[serde(skip)]
    pub semantic_laws: Vec<ValueLaw>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction_witness: Option<AbstractionWitness>,
    /// WHY the members merged — the agent-facing equivalence witness (#222).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<EquivalenceWitness>,
}

/// The evidence behind a group: which kind of convergence merged its members,
/// derived from the same predicates the channels gate on. An agent reading scan
/// JSON could not previously tell an exact value-graph proof from shape likeness
/// (`shared_lines: 0` with `mean_score: 1.0` was uninterpretable — the #216
/// audit's top gap); this names it without re-plumbing the scorer.
#[derive(Clone, Serialize)]
pub struct EquivalenceWitness {
    /// `exact-value-graph` (every member strict-exact-safe with one identical
    /// value multiset), `shared-sub-dag` (a common heavy anchor — see each
    /// location's `shared_subdag` span), `copy-paste-run` (token-identical
    /// contiguous run), or `structural-similarity` (the fuzzy near channel).
    pub kind: &'static str,
    /// For `exact-value-graph`: the size of the shared value multiset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_nodes: Option<usize>,
    /// For `structural-similarity`: mean value-graph Jaccard vs the first member
    /// — high here with low shape similarity means behaviorally-driven
    /// convergence, not surface likeness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_value_jaccard: Option<f64>,
    /// For `structural-similarity`: mean shape Jaccard vs the first member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_shape_jaccard: Option<f64>,
    /// For `structural-similarity` (near) families: the anti-unification grade of the
    /// two representative copies — "equal except these k holes", with each hole's value
    /// class and a referent check (#315). Computed by the presentation layer, which has
    /// source access; `None` for non-near witnesses and until that layer runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graded: Option<GradedWitness>,
}

/// Derive a group's witness from its members' own features (star against the
/// first member — deterministic and O(n); a witness, not a score).
fn group_witness(members: &[usize], units: &[UnitFeat]) -> EquivalenceWitness {
    let first = &units[members[0]];
    if members.iter().all(|&m| {
        let u = &units[m];
        exact_claim_eligible(u) && u.value == first.value
    }) {
        return EquivalenceWitness {
            kind: "exact-value-graph",
            value_nodes: Some(first.value.len()),
            mean_value_jaccard: None,
            mean_shape_jaccard: None,
            graded: None,
        };
    }
    if members.len() >= 2 && shared_subdag_hash(members, units).is_some() {
        return EquivalenceWitness {
            kind: "shared-sub-dag",
            value_nodes: None,
            mean_value_jaccard: None,
            mean_shape_jaccard: None,
            graded: None,
        };
    }
    let (mut vj, mut sj) = (0.0, 0.0);
    for &m in &members[1..] {
        vj += align::multiset_jaccard(&first.value, &units[m].value);
        sj += align::multiset_jaccard(&first.shapes, &units[m].shapes);
    }
    let n = (members.len().saturating_sub(1)).max(1) as f64;
    EquivalenceWitness {
        kind: "structural-similarity",
        value_nodes: None,
        mean_value_jaccard: Some(round3(vj / n)),
        mean_shape_jaccard: Some(round3(sj / n)),
        graded: None,
    }
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
    /// Reinvented-helper containment findings — see [`ReinventedHelper`].
    pub reinvented: Vec<ReinventedHelper>,
    pub metrics: Metrics,
}

/// One reinvented-helper containment finding: `container` computes, as an interior
/// sub-DAG, exactly the whole body of the pure single-return `helper` — WITHOUT calling
/// it. The actionable fix is the inverse of extract-method: replace the matched lines
/// with a call to the existing helper.
///
/// The claim is exact-grade and one-sided: both units pass the strict exact gate, and
/// an equal value-graph node hash is the same hash-consed canonical-structure guarantee
/// the exact channel rides — the container's sub-computation and the helper's body are
/// the same computation, not merely similar. Units that *call* a helper with this
/// behavior (directly, or via a behaviorally-equal twin) are excluded — calling is the
/// fix, not the smell.
#[derive(Serialize, Clone)]
pub struct ReinventedHelper {
    pub helper_file: String,
    pub helper_name: Option<String>,
    pub helper_start_line: u32,
    pub helper_end_line: u32,
    pub container_file: String,
    pub container_name: Option<String>,
    pub container_start_line: u32,
    pub container_end_line: u32,
    /// Source lines INSIDE the container where the helper's computation lives.
    pub site_start_line: u32,
    pub site_end_line: u32,
    /// The site is APPROXIMATE — the matched computation is a synthesized loop fold with
    /// no precise source span, so the site is the whole container range and includes
    /// computation beyond the helper's. Consumers must not mechanically replace these
    /// exact lines (coevo series 6, S3-1).
    pub site_approximate: bool,
    /// The container is a TEST file. Such findings are judgment-deep (§2b): the
    /// "reinvention" is often intentional — a test asserts the helper's expected value as
    /// a literal (calling it would be circular), or duplicates fixture setup. Kept in
    /// `--show reinvented` / JSON, excluded from the bare-default surface; a field audit
    /// (2026-06-13) measured them as the dominant non-actionable class.
    pub container_in_test: bool,
    /// Value-graph size of the reinvented computation — the ranking key.
    pub weight: u32,
}

/// Minimum value-graph size for a helper to participate in containment matching —
/// below this, "reinventing" it is idiom-sized noise (`x + 1` helpers), not a finding.
const REINVENTED_HELPER_MIN_NODES: usize = 8;

/// Minimum SOURCE size (pre-normalization tokens) for a containment helper. Value-graph
/// weight alone cannot tell a compressed loop (a whole accumulator loop canonicalizes to
/// a ~4-node `Reduce` — semantically rich) from a one-line delegation idiom
/// (`self._print(expr.args[0])` — w7 but trivial to re-type); the helper's source size
/// is the honest "is calling it actually better than writing it" proxy. Measured on
/// sympy: the delegation-idiom noise band sits at ≤12 tokens, real helpers at ≥25.
const REINVENTED_HELPER_MIN_TOKENS: usize = 20;

/// Compute the reinvented-helper containment findings over the extracted units: join
/// each eligible helper's single return-sink hash against every other unit's sub-DAG
/// anchors (which carry interior source spans). Deterministic: helpers bucket by hash
/// with a path/line-ordered representative, and the output is sorted by weight then
/// location.
pub fn reinvented_helpers(units: &[UnitFeat]) -> Vec<ReinventedHelper> {
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, u) in units.iter().enumerate() {
        if matches!(u.kind, UnitKind::Function | UnitKind::Method)
            && u.fragment_kind.is_none()
            && u.exact_safe
            && u.pure_single_return
            && !u.used_length_contract
            && u.returns.len() == 1
            && u.value.len() >= REINVENTED_HELPER_MIN_NODES
            && u.token_count >= REINVENTED_HELPER_MIN_TOKENS
        {
            by_hash.entry(u.returns[0]).or_default().push(i);
        }
    }
    if by_hash.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<ReinventedHelper> = Vec::new();
    for (ci, c) in units.iter().enumerate() {
        if !matches!(c.kind, UnitKind::Function | UnitKind::Method)
            || c.fragment_kind.is_some()
            || !c.exact_safe
        {
            continue;
        }
        for anchor in &c.anchors {
            let Some(helpers) = by_hash.get(&anchor.hash) else {
                continue;
            };
            // The container already obtains this computation BY CALLING a helper.
            if c.called_helper_returns.binary_search(&anchor.hash).is_ok() {
                continue;
            }
            // The container must also carry the helper's loop guards (same iteration
            // scheme) — matching a fold while iterating differently is not containment.
            let guards_present = |h: &UnitFeat| {
                h.cond_sinks
                    .iter()
                    .all(|g| c.value.binary_search(g).is_ok())
            };
            // Deterministic representative among behaviorally-equal helpers; the
            // container must be strictly bigger than the helper (an equal-size match
            // is the exact channel's whole-unit clone, not a containment).
            let rep = helpers
                .iter()
                .copied()
                .filter(|&h| {
                    h != ci && c.value.len() > units[h].value.len() && guards_present(&units[h])
                })
                .min_by_key(|&h| (&units[h].path, units[h].start_line));
            let Some(h) = rep else { continue };
            let h = &units[h];
            // A matched anchor with a REAL (non-zero) source span must lie inside the
            // container's own line range. A span outside it means the shared computation
            // textually lives in a DIFFERENT function that this unit merely inlined
            // (generalized inlining splices a callee's value graph in) — the unit is a
            // transitive CALLER, not a reinventor, and `called_helper_returns` (one call
            // level deep) does not catch a two-hop chain. Reject (coevo series 6, S3-2).
            // A synthesized loop-fold anchor carries no span (line 0); it falls back to
            // the container range and is the faithful flagship case, so it is allowed.
            let real_span = anchor.line_start != 0;
            let inside = anchor.line_start >= c.start_line && anchor.line_end <= c.end_line;
            if real_span && !inside {
                continue;
            }
            // Loop-exit nodes (a synthesized `Reduce`) may carry no source span; fall
            // back to the container's own range rather than reporting line 0. When this
            // fallback fires the site is the WHOLE container (approximate — the matched
            // computation is a sub-part), flagged for honest consumer reporting.
            let (site_start, site_end, site_approximate) = if anchor.line_start == 0 {
                (c.start_line, c.end_line, true)
            } else {
                (anchor.line_start, anchor.line_end, false)
            };
            out.push(ReinventedHelper {
                helper_file: h.path.clone(),
                helper_name: h.name.clone(),
                helper_start_line: h.start_line,
                helper_end_line: h.end_line,
                container_file: c.path.clone(),
                container_name: c.name.clone(),
                container_start_line: c.start_line,
                container_end_line: c.end_line,
                site_start_line: site_start,
                site_end_line: site_end,
                site_approximate,
                container_in_test: report::is_test_path(&c.path),
                weight: anchor.weight,
            });
        }
    }
    out.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| {
                (&a.container_file, a.container_start_line)
                    .cmp(&(&b.container_file, b.container_start_line))
            })
            .then_with(|| {
                (&a.helper_file, a.helper_start_line).cmp(&(&b.helper_file, b.helper_start_line))
            })
    });
    out
}

fn loc_of(u: &UnitFeat, enclosing_unit: Option<EnclosingUnit>) -> Loc {
    let fragment_kind = u.fragment_kind;
    let mut loc = Loc::new(LocInit {
        file: u.path.clone(),
        source_span: LineSpan::new(u.start_line, u.end_line),
        lang: u.lang.name().to_string(),
        kind: u.kind,
        name: u.name.clone(),
        sem: u.value.len(),
        span_tokens: u.token_count,
    });
    loc.is_fragment = fragment_kind.is_some();
    loc.fragment_kind = fragment_kind;
    loc.reason_code = fragment_kind.map(FragmentKind::reason_code);
    loc.enclosing_unit = enclosing_unit;
    loc.in_test_module = u.in_test_module;
    loc
}

fn unit_kind_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Function => "Function",
        UnitKind::Method => "Method",
        UnitKind::Class => "Class",
        UnitKind::Block => "Block",
    }
}

fn unit_key(
    file: &str,
    kind: UnitKind,
    name: Option<&str>,
    start_line: u32,
    end_line: u32,
) -> String {
    format!(
        "{}:{}:{}-{}:{}",
        file,
        unit_kind_name(kind),
        start_line,
        end_line,
        name.unwrap_or("")
    )
}

fn can_enclose_fragment(u: &UnitFeat) -> bool {
    u.fragment_kind.is_none()
        && matches!(
            u.kind,
            UnitKind::Function | UnitKind::Method | UnitKind::Class
        )
}

fn contains_span(parent: &UnitFeat, child: &UnitFeat) -> bool {
    parent.path == child.path
        && parent.start_line <= child.start_line
        && parent.end_line >= child.end_line
        // Strict containment, except that a DIFFERENT-kind parent may share the
        // exact span: a method and its whole-body block are one region in two
        // unit kinds, and the method IS the block's enclosing context (#225).
        // Same-kind twins still never enclose each other.
        && (parent.start_line < child.start_line
            || parent.end_line > child.end_line
            || parent.kind != child.kind)
}

fn enclosing_unit_of(parent: &UnitFeat) -> EnclosingUnit {
    let mut unit = EnclosingUnit {
        file: parent.path.clone(),
        start_line: parent.start_line,
        end_line: parent.end_line,
        kind: parent.kind,
        name: parent.name.clone(),
        unit_key: String::new(),
    };
    unit.refresh_unit_key();
    unit
}

/// Attach enclosing function/method names to copy-paste-run members. Contiguous
/// groups are built from token streams, not from the unit set, so they never
/// passed through `enclosing_units` — and the #216 audit's sampled block
/// locations (all contiguous) carried `name: null` with nothing to anchor a
/// discussion to (#225). A run that crosses unit boundaries keeps `None`.
fn attach_enclosing_units(groups: &mut [Group], units: &[UnitFeat]) {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        if can_enclose_fragment(unit) {
            by_file.entry(unit.path.as_str()).or_default().push(idx);
        }
    }
    for parents in by_file.values_mut() {
        parents.sort_by_key(|&idx| {
            (
                LineSpan::new(units[idx].start_line, units[idx].end_line).line_count(),
                units[idx].start_line,
            )
        });
    }
    for group in groups {
        for loc in &mut group.members {
            if loc.enclosing_unit.is_some() {
                continue;
            }
            let Some(parents) = by_file.get(loc.file.as_str()) else {
                continue;
            };
            let parent = parents.iter().copied().find(|&idx| {
                let u = &units[idx];
                u.start_line <= loc.start_line && u.end_line >= loc.end_line
            });
            if let Some(idx) = parent {
                loc.enclosing_unit = Some(enclosing_unit_of(&units[idx]));
                loc.in_test_module = units[idx].in_test_module;
            } else {
                // A run crossing unit boundaries is test scaffolding iff EVERY
                // overlapping unit sits in the inline test module (#226 — the
                // alacritty audit run spanned four #[test] fns yet read `prod`).
                let overlapping: Vec<usize> = parents
                    .iter()
                    .copied()
                    .filter(|&idx| {
                        let u = &units[idx];
                        u.start_line <= loc.end_line && loc.start_line <= u.end_line
                    })
                    .collect();
                loc.in_test_module = !overlapping.is_empty()
                    && overlapping.iter().all(|&idx| units[idx].in_test_module);
            }
        }
    }
}

fn enclosing_units(units: &[UnitFeat]) -> Vec<Option<EnclosingUnit>> {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        by_file.entry(unit.path.as_str()).or_default().push(idx);
    }

    let mut out = vec![None; units.len()];
    for indices in by_file.values() {
        let mut parents: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&idx| can_enclose_fragment(&units[idx]))
            .collect();
        parents.sort_by_key(|&idx| {
            (
                LineSpan::new(units[idx].start_line, units[idx].end_line).line_count(),
                units[idx].start_line,
                units[idx].end_line,
            )
        });

        for &idx in indices {
            // Fragments AND plain Block units get their enclosing
            // function/method recovered — an agent cannot even NAME the region
            // of a block family without it (#225: every sampled block location
            // had `name: null`). Whole function/method/class units need none.
            if units[idx].fragment_kind.is_none() && units[idx].kind != UnitKind::Block {
                continue;
            }
            if let Some(parent) = parents
                .iter()
                .copied()
                .find(|&parent_idx| contains_span(&units[parent_idx], &units[idx]))
            {
                out[idx] = Some(enclosing_unit_of(&units[parent]));
            }
        }
    }
    out
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
    if raw_il_is_empty_module(il) {
        return Vec::new();
    }
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
        units::ExtractFeatures {
            shape_features: opts.shape_features,
            abstraction_witnesses: opts.abstraction_witnesses,
        },
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
    let per_file: Vec<(Vec<UnitFeat>, Option<Stream>)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let units = if opts.structural {
                if raw_il_is_empty_module(il) {
                    Vec::new()
                } else {
                    let n = nose_normalize::normalize(il, &corpus.interner, &norm_opts);
                    units::extract(
                        &n,
                        &corpus.interner,
                        &seeds,
                        opts.min_lines,
                        opts.min_tokens,
                        opts.block_units,
                        units::ExtractFeatures {
                            shape_features: opts.shape_features,
                            abstraction_witnesses: opts.abstraction_witnesses,
                        },
                    )
                }
            } else {
                Vec::new()
            };
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
    let mut streams: Vec<Stream> = Vec::new();
    for (u, s) in per_file {
        units.extend(u);
        if let Some(s) = s {
            streams.push(s);
        }
    }
    clk.lap("normalize+extract");

    // `detect_from_units` runs its own `StageTimer` for the detection sub-phases
    // (candidates/score/groups/contiguous), so no lap here — a single outer lap would
    // mislabel the whole call (group scoring dwarfs contiguous) as "contiguous".
    detect_from_units(units, corpus.files.len(), &streams, opts, detector)
}

fn raw_il_is_empty_module(il: &Il) -> bool {
    il.units.is_empty() && il.kind(il.root) == NodeKind::Module && il.children(il.root).is_empty()
}

/// Run candidate-generation → scoring → clustering over already-built `units` (the
/// value-graph channel) and, when `opts.contiguous`, the copy-paste channel over
/// `streams` — producing the report and diagnostic dump. Split from unit/stream
/// extraction so a caller (the CLI's cache path) can supply both, built — and cached —
/// per file. `files` is the source file count, for the report's metrics only.
pub fn detect_from_units(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    let (candidates, accepted) = if opts.structural {
        // 3. LSH candidate generation. Semantic scans use the value-graph signature;
        //    near-duplicate scans also use shape signatures so Type-3 edits that
        //    change behavior-defining values still reach the scorer. When both
        //    channels run, score the union once.
        let candidates = structural_candidates(&units, opts);
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
        (candidates, accepted)
    } else {
        clk.lap("candidates");
        (Vec::new(), Vec::new())
    };

    clk.lap("score");

    // 5. Cluster.
    let mut uf = cluster::UnionFind::new(units.len());
    for &(i, j, _) in &accepted {
        uf.union(i, j);
    }
    let raw_groups = uf.groups(units.len());
    clk.lap("cluster");

    let enclosing = enclosing_units(&units);

    // Build pair output (sorted by score desc).
    let mut duplicates: Vec<DupPair> = accepted
        .iter()
        .map(|&(i, j, s)| DupPair {
            left: loc_of(&units[i], enclosing[i].clone()),
            right: loc_of(&units[j], enclosing[j].clone()),
            score: round3(s),
            cross_language: units[i].lang != units[j].lang,
        })
        .collect();
    duplicates.sort_by(|a, b| b.score.total_cmp(&a.score));

    let groups = build_groups(&units, &accepted, &mut uf, &raw_groups, &enclosing, opts);
    clk.lap("groups");

    let reinvented = if opts.structural {
        reinvented_helpers(&units)
    } else {
        Vec::new()
    };
    let mut report = Report {
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
        reinvented,
    };

    // Copy-paste channel over the (raw-IL) token streams. Runs here, after the
    // value-graph channel, so both `detect` and the CLI's `--cache-dir` path produce
    // the same families — the cache supplies cached streams, otherwise this would
    // silently omit every contiguous clone.
    if opts.contiguous {
        let mut extra = contiguous::detect(
            streams,
            opts.contiguous_min_tokens,
            opts.contiguous_min_lines,
        );
        attach_enclosing_units(&mut extra, &units);
        report.metrics.groups += extra.len();
        report.groups.extend(extra);
    }
    clk.lap("contiguous");

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

/// LSH candidate generation for the structural pass: the value-graph and/or shape
/// channels per `opts`, unioned, sorted, and deduped.
fn structural_candidates(units: &[UnitFeat], opts: &DetectOptions) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();
    if opts.value_candidates {
        candidates.extend(lsh::candidates(
            units.len(),
            |i| units[i].minhash.as_slice(),
            opts.bands,
        ));
        candidates.extend(exact_value_candidates(units));
    }
    if opts.shape_candidates {
        candidates.extend(lsh::candidates(
            units.len(),
            |i| units[i].shape_minhash.as_slice(),
            opts.bands,
        ));
        // Partial / sub-DAG clones: pair units that share a rare heavy anchor (an
        // extractable common sub-computation). They share no shape band, so shape-LSH
        // alone never proposes them — this is the candidate channel's sub-DAG path.
        candidates.extend(anchor_candidates(units));
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// Build the report's `groups` from the clustered components.
///
/// Group score = mean of the accepted-pair scores within the group. Accumulate it in
/// ONE pass over `accepted` instead of rescanning every accepted pair for every group
/// (which was O(groups × accepted) — ~1e9 iterations / ~0.9s on guava's 17.6k groups ×
/// 59k pairs, the detector's real hot spot). Each accepted pair was unioned, so its two
/// endpoints share a component; index its contribution by that component's root. The
/// per-group sum still walks `accepted` in order, so the float total — and the rounded
/// score — is byte-identical to the per-group rescan.
fn build_groups(
    units: &[UnitFeat],
    accepted: &[(usize, usize, f64)],
    uf: &mut cluster::UnionFind,
    raw_groups: &[Vec<usize>],
    enclosing: &[Option<EnclosingUnit>],
    opts: &DetectOptions,
) -> Vec<Group> {
    let mut by_root: rustc_hash::FxHashMap<usize, (f64, u32)> = rustc_hash::FxHashMap::default();
    for &(i, _j, s) in accepted {
        let e = by_root.entry(uf.find(i)).or_insert((0.0, 0));
        e.0 += s;
        e.1 += 1;
    }
    raw_groups
        .iter()
        .map(|members| {
            let root = uf.find(members[0]);
            let (sum, n) = by_root.get(&root).copied().unwrap_or((0.0, 0));
            let score = if n == 0 { 0.0 } else { sum / n as f64 };
            let mut locs: Vec<Loc> = members
                .iter()
                .map(|&m| loc_of(&units[m], enclosing[m].clone()))
                .collect();
            // If every member shares a heavy sub-DAG (a partial / sub-DAG clone), annotate each
            // site with its OWN source range for that shared computation — so the report can point
            // at where the shared logic lives in each copy, not just that one exists.
            if let Some(hash) = shared_subdag_hash(members, units) {
                for (&m, loc) in members.iter().zip(locs.iter_mut()) {
                    if let Some(a) = units[m].anchors.iter().find(|a| a.hash == hash) {
                        if a.line_start > 0 || a.line_end > 0 {
                            loc.shared_subdag = Some((a.line_start, a.line_end));
                        }
                    }
                }
            }
            Group {
                score: round3(score),
                members: locs,
                semantic_laws: semantic_laws_for_members(members, units),
                abstraction_witness: if opts.abstraction_witnesses {
                    units::abstraction_family_witness(members.iter().map(|&m| &units[m]))
                } else {
                    None
                },
                witness: Some(group_witness(members, units)),
            }
        })
        .collect()
}

fn semantic_laws_for_members(members: &[usize], units: &[UnitFeat]) -> Vec<ValueLaw> {
    let mut laws = members
        .iter()
        .flat_map(|&member| units[member].semantic_laws.iter().copied())
        .collect::<Vec<_>>();
    laws.sort_unstable();
    laws.dedup();
    laws
}

fn exact_value_candidates(units: &[UnitFeat]) -> Vec<(usize, usize)> {
    let mut buckets: HashMap<Vec<u64>, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        if unit.exact_safe && unit.value.len() >= EXACT_VALUE_MIN {
            buckets.entry(unit.value.clone()).or_default().push(idx);
        }
    }
    let mut out = Vec::new();
    for members in buckets.values() {
        if members.len() < 2 {
            continue;
        }
        if members.len() <= EXACT_VALUE_BUCKET_ALL_PAIRS_CAP {
            for a in 0..members.len() {
                for b in (a + 1)..members.len() {
                    out.push(ordered_pair(members[a], members[b]));
                }
            }
        } else {
            for w in members.windows(2) {
                out.push(ordered_pair(w[0], w[1]));
            }
            for &m in &members[1..] {
                out.push(ordered_pair(members[0], m));
            }
        }
    }
    out
}

/// An anchor present in more than this many units is boilerplate (a common idiom), not a
/// specific extractable sub-computation — skip it. Env-overridable.
fn anchor_max_df() -> usize {
    use std::sync::OnceLock;
    static D: OnceLock<usize> = OnceLock::new();
    *D.get_or_init(|| env_or("NOSE_ANCHOR_MAX_DF", 6.0) as usize)
}

const ANCHOR_PAIR_CAP: usize = 64;

/// Partial / sub-DAG clone candidates: units that share a RARE heavy anchor — an extractable
/// common sub-computation that whole-unit Jaccard misses. Index anchor → units; for anchors
/// present in `2..=anchor_max_df` units, emit their pairs (capped per bucket). Common anchors
/// (boilerplate above the df ceiling) are skipped so this stays specific.
fn anchor_candidates(units: &[UnitFeat]) -> Vec<(usize, usize)> {
    // Anchors are COLLECTED at the finer containment floor; the near channel keeps its
    // own coarser floor at every consumption point, so its behavior is unchanged.
    let floor = nose_normalize::anchor_min_weight();
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, unit) in units.iter().enumerate() {
        for a in &unit.anchors {
            if a.weight < floor {
                continue;
            }
            buckets.entry(a.hash).or_default().push(idx);
        }
    }
    let max_df = anchor_max_df();
    let mut out = Vec::new();
    for members in buckets.values() {
        if members.len() < 2 || members.len() > max_df {
            continue;
        }
        let mut count = 0;
        'pairs: for a in 0..members.len() {
            for b in (a + 1)..members.len() {
                out.push(ordered_pair(members[a], members[b]));
                count += 1;
                if count >= ANCHOR_PAIR_CAP {
                    break 'pairs;
                }
            }
        }
    }
    out
}

/// The heaviest sub-DAG anchor present in EVERY member of a group (`None` if none is shared by
/// all) — the shared computation the report annotates each site with. For a 2-member partial clone
/// this is the shared anchor; for a larger family it is the common heavy computation.
fn shared_subdag_hash(members: &[usize], units: &[UnitFeat]) -> Option<u64> {
    if members.len() < 2 {
        return None;
    }
    let floor = nose_normalize::anchor_min_weight();
    units[members[0]]
        .anchors
        .iter()
        .filter(|a| {
            a.weight >= floor
                && members[1..]
                    .iter()
                    .all(|&m| units[m].anchors.iter().any(|b| b.hash == a.hash))
        })
        .max_by_key(|a| a.weight)
        .map(|a| a.hash)
}

/// The weight of the LARGEST sub-DAG the two units share (0 if none) — a shared anchor is a
/// shared extractable sub-computation, and a bigger one is a stronger partial-clone signal.
/// Both anchor lists are sorted by hash, so this is a linear merge.
fn shared_anchor_weight(a: &[nose_normalize::Anchor], b: &[nose_normalize::Anchor]) -> u32 {
    // Near-channel floor (collection runs at the finer containment floor).
    let floor = nose_normalize::anchor_min_weight();
    let (mut i, mut j, mut best) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].hash.cmp(&b[j].hash) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                if a[i].weight >= floor && b[j].weight >= floor {
                    best = best.max(a[i].weight.min(b[j].weight));
                }
                i += 1;
                j += 1;
            }
        }
    }
    best
}

#[inline]
fn ordered_pair(i: usize, j: usize) -> (usize, usize) {
    if i < j {
        (i, j)
    } else {
        (j, i)
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}
