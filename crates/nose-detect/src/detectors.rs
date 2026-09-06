use crate::{
    align,
    candidates::shared_anchor_weight,
    exact_policy::{exact_claim_eligible_parts, exact_value_match_eligible, exact_value_rich},
    strict_exact,
    units::UnitFeat,
};
use nose_il::{Il, Interner, NodeId};
use std::collections::HashMap;

mod inputs;
use inputs::ScoreInputs;

/// Pluggable similarity scorer. Returns a score in `[0, 1]` for a candidate pair.
pub trait Detector: Sync {
    fn name(&self) -> &str;
    fn score(&self, a: &UnitFeat, b: &UnitFeat) -> f64;
    /// Optional exact equivalence classes: one id per unit in this immutable slice. Equal ids
    /// must be interchangeable in either score argument, including rejected scores.
    /// Return `None` for scorers that depend on location, history, or other state.
    fn score_classes(&self, _units: &[UnitFeat]) -> Option<Vec<usize>> {
        None
    }
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

/// Strict exact-safety by source-byte span for known roots.
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
                (span.start_byte, span.end_byte),
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

    fn score_classes(&self, units: &[UnitFeat]) -> Option<Vec<usize>> {
        Some(inputs::classes(
            units.iter().map(|unit| (&unit.value, unit.exact_safe)),
        ))
    }
}

/// The v1 default: weighted multiset Jaccard over subtree shapes, blended with an
/// LCS alignment over the linearized IL. A cheap Jaccard prefilter skips the
/// (more expensive) LCS for obviously-dissimilar pairs.
pub struct StructuralDetector {
    scoring: crate::ScoreConfig,
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
            scoring: crate::ScoreConfig::default(),
            jaccard_weight,
            exact_behavior: true,
            candidate_mode: false,
            accept_threshold: 0.0,
        }
    }
    /// Near-candidate detector: gates off (recall-oriented, ~99% triage-worthy).
    pub fn candidates(jaccard_weight: f64) -> Self {
        Self {
            scoring: crate::ScoreConfig::default(),
            jaccard_weight,
            exact_behavior: true,
            candidate_mode: true,
            accept_threshold: 0.0,
        }
    }
    pub fn with_scoring(mut self, scoring: crate::ScoreConfig) -> Self {
        self.scoring = scoring;
        self
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
        let (a, b) = (ScoreInputs::from(a), ScoreInputs::from(b));
        let protocol_match = self.candidate_mode && external_near_protocol_match(&a, &b);
        let score = self.base_score(&a, &b, protocol_match);
        if protocol_match && score >= 0.60 {
            // A reviewed protocol row is supporting near evidence, never an exact proof.
            // Move an already-substantial existing candidate only one quarter of the
            // remaining distance toward 1.0 and keep it visibly below exact confidence.
            (score + (1.0 - score) * 0.25).min(0.95)
        } else {
            score
        }
    }

    fn score_classes(&self, units: &[UnitFeat]) -> Option<Vec<usize>> {
        Some(inputs::classes(units.iter().map(ScoreInputs::from)))
    }
}

fn external_near_protocol_match(a: &ScoreInputs<'_>, b: &ScoreInputs<'_>) -> bool {
    a.semantic_pack_near_protocols.iter().any(|left| {
        left.provenance.is_some()
            && b.semantic_pack_near_protocols
                .iter()
                .any(|right| right.operation == left.operation)
    }) || b.semantic_pack_near_protocols.iter().any(|right| {
        right.provenance.is_some()
            && a.semantic_pack_near_protocols
                .iter()
                .any(|left| left.operation == right.operation)
    })
}

impl StructuralDetector {
    fn base_score(&self, a: &ScoreInputs<'_>, b: &ScoreInputs<'_>, protocol_match: bool) -> f64 {
        // Oracle-certified fast path (§AJ): an identical value-graph fingerprint means
        // behaviorally-equal — `nose verify` proved fingerprint-equality ⟹ behavior
        // -equality across the corpus (0 false merges). So accept an exact match
        // outright, *regardless of syntactic divergence* — this is what lets a true
        // Type-4 clone (loop ≡ reduce ≡ comprehension) be detected even though its
        // shapes differ. Guarded by a minimum fingerprint size so trivial units don't
        // collapse. The size gate (min_tokens) already excludes tiny units upstream.
        if self.exact_behavior
            && exact_claim_eligible_parts(a.exact_safe, a.value.len())
            && exact_claim_eligible_parts(b.exact_safe, b.value.len())
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
        // still group as a refactoring family worth a human's attention.
        let [wv, ws, wr] = if self.candidate_mode {
            self.scoring.candidate_weights
        } else {
            self.scoring.strict_weights
        };
        let vj = align::multiset_jaccard(a.value, b.value);
        // Candidate mode trusts the value graph: a near-identical value fingerprint — produced
        // AFTER semantic canonicalization (a `.then`-chain ≡ await code, a loop ≡ a
        // comprehension) — is the strongest refactoring signal there is, even when the
        // syntactic shapes diverge and the unit is NOT exact-safe (impure: async, I/O, opaque
        // calls). The shape-dominant blend below would miss these, so accept a very-high `vj`
        // directly. Impure units never reach the exact channel, so this is the only place such
        // behaviorally-convergent pairs can surface. Tight threshold + size floor keep it precise.
        if self.candidate_mode
            && exact_value_rich(a.value.len())
            && exact_value_rich(b.value.len())
            && vj >= self.scoring.candidate_value_accept
        {
            return vj;
        }
        // Shape overlap is only needed after the value-graph fast path above. Corpus profiling
        // showed many candidate-mode pairs exit there; computing shapes first spent measurable
        // time without changing any accepted score.
        let sj = align::multiset_jaccard(a.shapes, b.shapes);
        // Partial / sub-DAG clone: the units share a rare heavy anchor (an extractable common
        // sub-computation) even though the whole-unit blend is low. Surface it for inspection at a
        // score above the near floor but below a full clone — it's a real refactor lead (pull
        // the shared computation into a helper), just a partial one. Keep the higher of the two.
        if self.candidate_mode {
            let shared = shared_anchor_weight(a.anchors, b.anchors);
            if shared > 0 {
                return (wv * vj + ws * sj).max(self.scoring.anchor_score(shared));
            }
        }
        if 0.6 * vj + 0.4 * sj < 0.15 {
            return 0.6 * vj + 0.4 * sj; // prefilter: not worth the alignment DP
        }
        // Score-preserving early-exit: RANSAC (≤1) and the gates only lower the
        // score, so if the upper bound `wv·vj+ws·sj+wr` can't reach threshold the
        // pair is rejected anyway — skip the alignment DP.
        if !protocol_match && wv * vj + ws * sj + wr < self.accept_threshold {
            return wv * vj + ws * sj + wr;
        }
        let l = align::ransac_ratio(a.linear, b.linear);
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
        let (dh_ratio, dh_abs) = (self.scoring.data_heavy_ratio, self.scoring.data_heavy_count);
        let data_heavy = |u: &ScoreInputs<'_>| {
            !u.value.is_empty()
                && (u.lits.len() as f64 / u.value.len() as f64 >= dh_ratio
                    || u.lits.len() >= dh_abs)
        };
        if data_heavy(a) && data_heavy(b) {
            return score.min(align::multiset_jaccard(a.lits, b.lits));
        }
        // Return-signature gate: two units that return DIFFERENT computed values are
        // not behavioral clones, however similar their bodies. When both return
        // something, cap the score by `ret_base + (1-ret_base)·return_jaccard`, so a
        // total return mismatch (e.g. `<` vs `<=`, an extra effect) caps below the
        // operating threshold while a return match leaves the score untouched.
        if !a.returns.is_empty() && !b.returns.is_empty() {
            let rj = align::multiset_jaccard(a.returns, b.returns);
            let base = self.scoring.return_base;
            return score.min(base + (1.0 - base) * rj);
        }
        score
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{units_of_file, DetectOptions};
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, NodeKind, Payload, Span};
    use nose_semantics::{
        SemanticPackNearDependency, SemanticPackNearProtocol, SemanticPackNearProvenance,
        SemanticPackV1Channel, SemanticPackV1ProtocolOperation,
    };

    #[test]
    fn exact_safety_keeps_same_line_functions_separate_by_byte_span() {
        let file = FileId(0);
        let mut builder = IlBuilder::new(file);
        let safe_span = Span::new(file, 0, 8, 1, 1);
        let unsafe_span = Span::new(file, 9, 18, 1, 1);
        let value = builder.add(NodeKind::Lit, Payload::LitInt(1), safe_span, &[]);
        let safe = builder.add(NodeKind::Func, Payload::None, safe_span, &[value]);
        let raw = builder.add(NodeKind::Raw, Payload::None, unsafe_span, &[]);
        let unsafe_root = builder.add(NodeKind::Func, Payload::None, unsafe_span, &[raw]);
        let root = builder.add(
            NodeKind::Seq,
            Payload::None,
            safe_span.merge(unsafe_span),
            &[],
        );
        let il = builder.finish(
            root,
            FileMeta {
                path: "same-line.js".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        let safety = exact_safe_roots_by_span(&il, &Interner::new(), &[safe, unsafe_root]);
        assert_eq!(safety.len(), 2);
        assert!(safety[&(0, 8)]);
        assert!(!safety[&(9, 18)]);
    }

    #[test]
    fn locked_protocol_evidence_only_lifts_substantial_near_scores() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "T.java",
            b"class T { Object a(Object x) { return x; } Object b(Object x) { return x; } }",
            Lang::Java,
            &interner,
        )
        .expect("Java fixture lowers");
        let options = DetectOptions {
            min_lines: 1,
            min_tokens: 1,
            shape_features: true,
            ..DetectOptions::default()
        };
        let mut units = units_of_file(&il, &interner, &options);
        let mut right = units.pop().expect("second method");
        let mut left = units.pop().expect("first method");
        left.value = vec![1, 2];
        right.value = vec![1, 3];
        left.shapes = vec![1, 2, 3, 4, 5];
        right.shapes = vec![1, 2, 3, 4, 6];
        left.linear = vec![1, 2, 3, 4];
        right.linear = left.linear.clone();
        let detector = StructuralDetector::candidates(0.5)
            .without_exact_behavior()
            .with_threshold(0.70);
        let base = detector.score(&left, &right);
        assert!((0.60..0.70).contains(&base), "calibrated base={base}");

        left.semantic_pack_near_protocols = vec![external_collection_protocol()];
        right.semantic_pack_near_protocols = vec![SemanticPackNearProtocol {
            operation: SemanticPackV1ProtocolOperation::CollectionFactory,
            provenance: None,
        }];
        let supported = detector.score(&left, &right);
        assert!((0.70..1.0).contains(&supported), "score={supported}");

        right.semantic_pack_near_protocols[0].operation =
            SemanticPackV1ProtocolOperation::MapFactory;
        assert_eq!(detector.score(&left, &right), base);
    }

    fn external_collection_protocol() -> SemanticPackNearProtocol {
        SemanticPackNearProtocol {
            operation: SemanticPackV1ProtocolOperation::CollectionFactory,
            provenance: Some(SemanticPackNearProvenance {
                pack_id: "example.pack".into(),
                row_id: "example.row".into(),
                semantic_digest: "sha256:pack".into(),
                row_digest: "sha256:row".into(),
                lane: SemanticPackV1Channel::Near,
                trust: "external-opt-in".into(),
                operation: SemanticPackV1ProtocolOperation::CollectionFactory,
                dependency: SemanticPackNearDependency {
                    coordinate: "example:pack".into(),
                    declared_version: "1.0.0".into(),
                    matched_version: "1.0.0".into(),
                    sources: Vec::new(),
                },
                occurrence_file: "T.java".into(),
                call_start_line: 1,
                call_end_line: 1,
                caveats: Vec::new(),
            }),
        }
    }
}
