use crate::{
    align,
    candidates::shared_anchor_weight,
    exact_policy::{candidate_value_floor_met, exact_value_match_eligible},
    strict_exact,
    units::UnitFeat,
};
use nose_il::{Il, Interner, NodeId};
use std::collections::HashMap;

/// Pluggable similarity scorer. Returns a score in `[0, 1]` for a candidate pair.
pub trait Detector: Sync {
    fn name(&self) -> &str;
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64;
}

/// A no-op scorer used when a mode intentionally runs only the contiguous
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
/// This gives the `semantic` channel a high-confidence Type-4 surface without fuzzy
/// structural similarity.
pub struct ExactBehaviorDetector;

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
        if exact_value_match_eligible(a, b) {
            1.0
        } else {
            0.0
        }
    }
}

/// The v1 default: weighted multiset Jaccard over subtree shapes, blended with an
/// LCS alignment over the linearized IL. A cheap Jaccard prefilter skips the
/// (more expensive) LCS for obviously-dissimilar pairs.
pub struct StructuralDetector {
    pub jaccard_weight: f64,
    /// Accept exact value-fingerprint matches before fuzzy structural scoring. The
    /// `near` channel disables this so Type-3 near-duplicates stay separate from the
    /// exact semantic Type-4 channel.
    pub exact_behavior: bool,
    /// Near-candidate mode: disable the behavioral-precision gates
    /// (data-table, return-signature). Those gates demote "same shape, different
    /// data/operator" pairs — correct for behavioral-clone detection, but those
    /// pairs (locale-class families, comparison-operator families, sync/async
    /// wrappers) are exactly the refactoring candidates a human wants to inspect.
    /// Measured: under a refactoring-worthiness rubric, candidate mode (gates off,
    /// thr 0.70) surfaces ~4.5k pairs at ~99% triage-worthy.
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
    /// Near-candidate detector: gates off (recall-oriented, ~99% triage-worthy).
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
        if self.exact_behavior && exact_value_match_eligible(a, b) {
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
        // still group as a refactoring family worth a human's attention.
        let (wv, ws, wr) = score_weights(self.candidate_mode);
        let vj = align::multiset_jaccard(&a.value, &b.value);
        // Candidate mode trusts the value graph: a near-identical value fingerprint — produced
        // AFTER semantic canonicalization (a `.then`-chain ≡ await code, a loop ≡ a
        // comprehension) — is the strongest refactoring signal there is, even when the
        // syntactic shapes diverge and the unit is NOT exact-safe (impure: async, I/O, opaque
        // calls). The shape-dominant blend below would miss these, so accept a very-high `vj`
        // directly. Impure units never reach the exact channel, so this is the only place such
        // behaviorally-convergent pairs can surface. Tight threshold + size floor keep it precise.
        if self.candidate_mode && candidate_value_floor_met(a, b) && vj >= candidate_value_accept()
        {
            return vj;
        }
        // Shape overlap is only needed after the value-graph fast path above. Corpus profiling
        // showed many candidate-mode pairs exit there; computing shapes first spent measurable
        // time without changing any accepted score.
        let sj = align::multiset_jaccard(&a.shapes, &b.shapes);
        // Partial / sub-DAG clone: the units share a rare heavy anchor (an extractable common
        // sub-computation) even though the whole-unit blend is low. Surface it for inspection at a
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

pub(crate) fn env_or<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Copy,
{
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
