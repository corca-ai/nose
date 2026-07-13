use crate::{AbstractionWitness, Loc};
use nose_semantics::ValueLawProvenance;
use serde::Serialize;

use super::{
    paths::{is_generated_loc, is_test_loc, span_lines},
    score::{effective_copies, spread},
};

/// A compact direct-accepted-edge graph over one collapsed structural family.
/// Sites are stored once; edges are local indices, so dense evidence is not
/// expanded into duplicate location pairs.
#[derive(Clone)]
pub struct AcceptedCoverage {
    pub sites: Vec<Loc>,
    pub edges: Vec<(u32, u32)>,
}

#[derive(Serialize, Clone)]
pub struct RefactorFamily {
    /// Ranking score (higher = more worth refactoring). See `refactor_value`.
    pub value: f64,
    /// Number of duplicated sites.
    pub members: usize,
    /// Distinct files the family spans.
    pub files: usize,
    /// Distinct directories/modules the family spans (design-level spread).
    pub modules: usize,
    /// Distinct languages (cross-language family).
    pub languages: usize,
    /// Mean pairwise similarity within the family.
    pub mean_score: f64,
    /// Mean lines per member.
    pub mean_lines: u32,
    /// Lines that could be removed by extracting one shared copy
    /// (≈ `(members − 1) × mean_lines`).
    pub dup_lines: u32,
    /// Invariant (shared) source lines between the two representative copies — the
    /// body of the helper you'd extract. The honest counterpart to `mean_score`: two
    /// copies can be `sim 1.00` structurally yet share few literal lines (a dispatch
    /// skeleton wrapping divergent bodies). Computed at the presentation layer (needs
    /// source); `0` until then, or for cross-language families (no shared lines).
    pub shared_lines: u32,
    /// Number of varying spots between the two representative copies — the parameters
    /// the extracted helper would take. High param count ⇒ a costlier, uglier extract.
    /// Computed alongside `shared_lines`; `0` until then.
    pub params: u32,
    /// The same shared-line measure as `shared_lines`, but unrounded *and* weighted by
    /// how specific each line is (a pervasive idiom contributes ~0) — the value the
    /// ranking actually uses. Kept as a float so families don't collapse into integer
    /// ties (which would let the raw-volume tie-break re-dominate the order).
    /// `shared_lines` is just this rounded, for display. `0.0` until computed.
    pub shared_weight: f64,
    /// The duplicated sites, largest first.
    pub locations: Vec<Loc>,
    /// Structural accepted-family sites this row must keep covered if a later
    /// presentation layer suppresses it. Syntax-only windows start empty; when
    /// rank subsumption drops a structural family, its obligation moves to the
    /// covering survivor. Kept out of product JSON: this is bounded internal
    /// suppression provenance, not another reported clone family.
    #[doc(hidden)]
    #[serde(skip)]
    pub accepted_coverage: Vec<AcceptedCoverage>,
    /// All-copies parameter count computed alongside `shared_lines` while source
    /// slices are already cached. Query rendering reuses it instead of reading
    /// and diffing every displayed family a second time.
    #[doc(hidden)]
    #[serde(skip)]
    pub display_params: Option<u32>,
    /// Mean value-graph size across members (low → computation-poor type/data def).
    pub mean_sem: f64,
    /// Where the duplication lives: `"prod"`, `"test"` (all sites in test code), or
    /// `"mixed"` (logic duplicated *across* the test boundary — ranked normally
    /// because it's a real leak, unlike intentional test scaffolding).
    pub scope: &'static str,
    /// Refactor-worthiness discount in `(0, 1]` from `refactor_discount` — demotes
    /// all-type-definition and all-generated families. Applied to *both* `value` and
    /// `extractability` so the default ranking honors it too.
    pub discount: f64,
    /// Experimental weak-claim witness for `abstraction` mode. This records a typed
    /// template and caveats for near families that share structure but differ by one
    /// supported literal leaf position. It is not a semantic-equivalence proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction_witness: Option<AbstractionWitness>,
    /// Pack-facing semantic laws that influenced this family-level value fingerprint.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub semantic_laws: Vec<ValueLawProvenance>,
    /// WHY the members merged — the agent-facing equivalence witness (#222): an
    /// exact value-graph proof, a shared heavy sub-DAG, a token-identical
    /// copy-paste run, or structural similarity with its value/shape components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<crate::EquivalenceWitness>,
    /// WHAT differs between the two representative copies (#223): each varying
    /// spot the extracted helper would parameterize, with absolute line ranges
    /// and truncated text per side. Same provenance as `params` (the first
    /// readable representative pair); empty until the presentation layer reads
    /// source, and for cross-language families.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub varying_spots: Vec<VaryingSpot>,
}

/// One varying spot between a family's two representative copies — the hole an
/// extracted helper would parameterize. Sides may be one-sided (a pure
/// insertion/deletion run has lines in only one copy).
#[derive(Clone, serde::Serialize)]
pub struct VaryingSpot {
    /// 1-based parameter index, matching the family's `params` count.
    pub param: u32,
    /// Absolute `[start, end]` source lines of this spot in the FIRST
    /// representative copy (the family's `locations[0]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_lines: Option<(u32, u32)>,
    /// Absolute `[start, end]` source lines in the second representative copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_lines: Option<(u32, u32)>,
    /// Trimmed, length-capped text of the spot in the first copy.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub a_text: String,
    /// Trimmed, length-capped text of the spot in the second copy.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub b_text: String,
}

impl RefactorFamily {
    /// How cleanly this family extracts into one shared helper — the default ranking.
    /// Where `value` ranks by raw duplicated *volume* (and so over-rewards a big block
    /// whose copies actually share little), extractability ranks by the lines you'd
    /// truly remove: the *invariant* lines, dampened by the number of parameters the
    /// helper would need. A 400-line dispatcher sharing 9 lines across 30 varying spots
    /// sinks below a 20-line pair sharing 18 lines with one parameter.
    ///
    /// The honest extractable size depends on whether the copies are comparable as text:
    /// - **same-language** (`languages == 1`): `shared_lines` is the truth. If it is 0,
    ///   the family shares no invariant lines — a structural-only match (a language
    ///   idiom like `if err != nil { return err }`, or two unrelated type literals with
    ///   the same shape). There is *nothing to extract*, so the score is 0. This is the
    ///   key correction over volume ranking, which floated these to the top with a
    ///   misleading `sim 1.00`.
    /// - **cross-language** (`languages > 1`): there are no shared *source* lines to
    ///   diff, yet these are real Type-4 clones — fall back to the structural estimate
    ///   `mean_lines × mean_score` so cross-language spread still ranks on its merits.
    ///
    /// Two cleanliness factors scale the result: a **parameter penalty** (each varying
    /// spot widens the helper signature) and, for same-language families, **tightness**
    /// — the fraction of each copy that is invariant (`shared_lines / rep_lines`).
    /// Tightness is what separates `15/15` (extract the whole thing) from `22/384`
    /// (extract a 22-line helper and leave 360 unique lines at every site — barely a
    /// dedup); absolute shared lines alone can't tell them apart.
    pub fn extractability(&self) -> f64 {
        let (extract_lines, tightness) = if self.languages > 1 {
            // cross-language: there are no shared *source* lines to diff, so we can
            // neither weight out idioms nor measure tightness. Require *substance*
            // instead — a tiny cross-language "clone" (a few lines) is almost always a
            // shared idiom (an error check, a one-liner), and we can't verify it line by
            // line — whereas a real cross-language abstraction is a substantial routine.
            // Confidence ramps from 0 at ≤3 lines to 1 at ≥9, standing in for tightness.
            let confidence = ((self.mean_lines as f64 - 3.0) / 6.0).clamp(0.0, 1.0);
            (self.mean_lines as f64 * self.mean_score, confidence)
        } else {
            let rep = self
                .locations
                .first()
                .map_or(self.mean_lines, span_lines)
                .max(1) as f64;
            (self.shared_weight, (self.shared_weight / rep).min(1.0))
        };
        // Each extra parameter makes the helper's signature wider and the call sites
        // noisier; 1–2 is clean, a dozen means the "shared" code is mostly scaffolding.
        let param_penalty = 1.0 / (1.0 + 0.5 * self.params as f64);
        extract_lines
            * effective_copies(self.members)
            * spread(self.files, self.modules, self.languages)
            * param_penalty
            * tightness
            * self.shape_homogeneity()
            * self.discount
            * self.default_surface_weight()
    }

    /// Member-span **heterogeneity** demotion: copies that vary widely in length are
    /// not one shape — folding them needs a catch-all, not a clean helper, no matter how
    /// many lines happen to align in the representative pair. This is the decidable proxy
    /// for the issue's "signature/arity heterogeneity" (#365): the per-member source span
    /// is the one size signal available without re-parsing. Validated on the v5 labelset —
    /// not-worthy families carry ~2.7× the member-span coefficient-of-variation of worthy
    /// ones (mean 0.137 vs 0.051), and families with CV ≥ 0.3 are worthy only 23% of the
    /// time vs 52% overall. The penalty is **deliberately gentle** (`1/(1+CV)`): the gold
    /// set rewards demoting the worst offenders (serde's 25 heterogeneous `Serializer`
    /// methods drop from #1 to #2, the clean 10-method numeric writer takes #1) but a
    /// harder penalty regresses held-out past α≈1 — the §AV/§CL judgment-deep ceiling,
    /// where high-CV worthy families start paying too. Same-language only, like
    /// `tightness`: cross-language families have no shared line basis to fold against, and
    /// the gold set showed gating it there is metric-neutral. Returns 1.0 (no demotion)
    /// for uniform copies. See experiments §CM.
    fn shape_homogeneity(&self) -> f64 {
        if self.languages > 1 {
            return 1.0;
        }
        let lens: Vec<f64> = self
            .locations
            .iter()
            .map(|l| span_lines(l) as f64)
            .collect();
        if lens.len() < 2 {
            return 1.0;
        }
        let mean = lens.iter().sum::<f64>() / lens.len() as f64;
        if mean <= 0.0 {
            return 1.0;
        }
        let var = lens.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / lens.len() as f64;
        let cv = var.sqrt() / mean;
        1.0 / (1.0 + cv)
    }

    /// Decidable, scope-blind actionability reason — a stable code naming WHY a family
    /// is not a clean default-surface refactor candidate, computed purely from family
    /// shape (no source, no judgment, no `scope` input). The family-side codes
    /// (declaration/generated are decided CLI-side from source — see
    /// `family_actionability_reason`):
    ///
    /// - `trivial` — `mean_lines` ≤ 4: too small to be an extraction target (0.95
    ///   precision in the audit).
    /// - `shallow-extraction` — the extracted helper would be mostly parameters
    ///   (`params` ≥ a third of `shared_lines`); the dual of extractability's
    ///   `param_penalty` (0.89 precision).
    ///
    /// Measured 2026-06-14
    /// ([default-surface-noise-audit](../../../docs/default-surface-noise-audit-2026-06-14.md)).
    /// A **proven** channel (exact value-graph / shared-sub-dag) is never demoted on these
    /// shape/size heuristics. The #844 dev re-audit found actionable small-helper and
    /// high-parameter hard negatives, so proof strength remains separate from extraction
    /// actionability and the exemption fails open. Returns `None` for a clean candidate.
    pub fn actionability_reason(&self) -> Option<&'static str> {
        // Size floor below which a clone is too small to be an extraction target. Measured
        // 2026-06-14 (default-surface-noise-audit): `trivial` labels at 0.95 precision.
        const TRIVIAL_MAX_LINES: u32 = 4;
        const SHALLOW_PARAM_RATIO: f64 = 0.33;
        // A proven channel is never demoted on a shape/size heuristic.
        let proven = matches!(
            self.witness.as_ref().map(|w| w.kind),
            Some("exact-value-graph") | Some("shared-sub-dag")
        );
        if proven {
            return None;
        }
        // `trivial` before `shallow-extraction`: size is the more fundamental reason.
        if self.mean_lines <= TRIVIAL_MAX_LINES {
            return Some("trivial");
        }
        if self.shared_lines > 0
            && self.params as f64 >= SHALLOW_PARAM_RATIO * self.shared_lines as f64
        {
            return Some("shallow-extraction");
        }
        None
    }

    /// The structural shape of the fix IF this family is acted upon — a **decidable**
    /// classification from unit kinds and channel, NOT a worth-it verdict (that is the
    /// consumer's, §2). A stable machine field (#11) parallel to the prose hint. The prose
    /// hint (`family_hint`) additionally refines its wording from per-unit origin facets
    /// (#453) when present, so the two can read slightly differently (e.g. a type/API
    /// contract); this enum stays origin-independent so the machine contract is stable.
    /// The consumer reads it only for a clean candidate (`actionability_reason` absent).
    pub fn extraction_shape(&self) -> &'static str {
        use nose_il::UnitKind;
        // call-existing-helper: exactly one named whole function/method, every other
        // member an inline block/fragment — the inline copies recompute the existing
        // helper, so the fix is "call it", not a fresh extraction (#263's local `clamp`).
        let named: Vec<&Loc> = self
            .locations
            .iter()
            .filter(|l| {
                matches!(l.kind, UnitKind::Function | UnitKind::Method)
                    && l.name.is_some()
                    && !l.is_fragment
            })
            .collect();
        let inline = self
            .locations
            .iter()
            .filter(|l| l.kind == UnitKind::Block || l.is_fragment)
            .count();
        if let [helper] = named[..] {
            let callable =
                !helper.looks_generated && (self.scope == "test" || !is_test_loc(helper));
            if callable && inline >= 1 && inline == self.locations.len() - 1 {
                return "call-existing-helper";
            }
        }
        if self.languages > 1 {
            return "consolidate-cross-language";
        }
        let all_classes = self.locations.iter().all(|l| l.kind == UnitKind::Class);
        let all_blocks = self.locations.iter().all(|l| l.kind == UnitKind::Block);
        if all_classes && self.mean_sem < 12.0 {
            "consolidate-type"
        } else if all_classes {
            "extract-base-class"
        } else if all_blocks {
            "extract-method-from-block"
        } else {
            "extract-helper"
        }
    }

    /// Product placement for the default/divergence/debug surfaces. This is a
    /// presentation/ranking decision, not detector semantics: exact fragments remain
    /// present in `--top 0` JSON even when their default ranking is dampened.
    ///
    /// A family that would otherwise reach the **default** head but is a decidable
    /// non-action shape ([`actionability_reason`](Self::actionability_reason)) is demoted
    /// to the `shallow` surface, reason-coded and kept in `--top 0` JSON (the §2b
    /// decidability boundary, measured 2026-06-14). It never overrides a *more specific*
    /// diagnostic placement (a tiny test-scaffold fragment stays `hidden`), and never
    /// fires on a proven channel.
    pub fn recommended_surface(&self) -> &'static str {
        let base = self.recommended_surface_base();
        if base == "default" {
            // Map the decidable reason to its placement: `shallow-extraction` to the
            // `shallow` surface; `trivial` to `hidden` (it is a size floor). The reason
            // itself rides the separate `actionability_reason` JSON field.
            match self.actionability_reason() {
                Some("shallow-extraction") => return "shallow",
                Some("trivial") => return "hidden",
                _ => {}
            }
        }
        base
    }

    fn recommended_surface_base(&self) -> &'static str {
        let fragment_sites = self.locations.iter().filter(|l| l.is_fragment).count();
        let all_generated = self.locations.iter().all(is_generated_loc);
        let high_fanout = self.members >= 8;
        if all_generated
            || self.mean_lines <= 1
            || (fragment_sites > 0 && high_fanout && self.mean_lines <= 3)
        {
            return "hidden";
        }
        if fragment_sites == 0 {
            return "default";
        }
        // Mixed whole-unit + fragment families stay default: the enclosing unit is already a
        // product-sized candidate, and the fragment locations serve as supporting evidence.
        // The tiny/high-fanout proof-only cases above are deliberately not promoted by
        // this escape hatch.
        if fragment_sites < self.locations.len() {
            return "default";
        }

        let all_test = self.locations.iter().all(is_test_loc);
        let all_have_enclosing = self.locations.iter().all(|l| l.enclosing_unit.is_some());
        let has_effect_or_body = self.locations.iter().any(|l| {
            matches!(
                l.fragment_kind,
                Some(
                    crate::FragmentKind::LoopEffect
                        | crate::FragmentKind::IndexAssignEffect
                        | crate::FragmentKind::SelfFieldAssign
                        | crate::FragmentKind::SelfFieldBody
                )
            )
        });
        let has_guard_or_exit = self.locations.iter().any(|l| {
            matches!(
                l.fragment_kind,
                Some(
                    crate::FragmentKind::ConditionalGuard
                        | crate::FragmentKind::DirectReturn
                        | crate::FragmentKind::DirectThrow
                )
            )
        });
        let tiny_test_scaffold = all_test
            && all_have_enclosing
            && (self.mean_lines <= 3 || (has_effect_or_body && self.mean_lines <= 4));

        if tiny_test_scaffold {
            // The fragment-quality audit found these are usually correct but too often
            // test arrange/assert or fixture-constructor substrate to be divergence items.
            "hidden"
        } else if high_fanout && self.mean_lines <= 3 {
            "hidden"
        } else if has_effect_or_body {
            // Receiver/effect-bearing fragments are usually synchronization hazards first.
            // Promote only substantial, cross-file production fragments to default; keep
            // test/generated-looking and tiny forms in divergence/hidden.
            if !all_test && self.mean_lines >= 12 && self.files >= 2 && self.modules >= 2 {
                "default"
            } else if self.mean_lines <= 3 && all_have_enclosing {
                "hidden"
            } else {
                "divergence"
            }
        } else if has_guard_or_exit {
            if self.mean_lines >= 12 && self.files >= 2 && !all_test {
                "default"
            } else if self.mean_lines <= 3 {
                "hidden"
            } else {
                "divergence"
            }
        } else if self.mean_lines <= 8 {
            "divergence"
        } else {
            "default"
        }
    }

    fn default_surface_weight(&self) -> f64 {
        match self.recommended_surface() {
            "default" => 1.0,
            "divergence" => 0.35,
            "shallow" => 0.05,
            "hidden" => 0.05,
            "debug" => 0.02,
            _ => 1.0,
        }
    }

    /// Divergent-edit **hazard**: how likely this family is to be edited inconsistently
    /// (one copy changed, the siblings missed) and cause a bug. A *severity* axis,
    /// orthogonal to `extractability` (which is about *fixability*). This is the formula
    /// calibrated against mined ground truth — a leave-one-repo-out evaluation over a
    /// 12-repo / 8-language corpus of real divergent edits (G1) and bug-linked ones (G2);
    /// see `eval/hazard/RESULTS.md` and `docs/hazard-ranking.md`.
    ///
    /// The data overturned the intuitive design: semantic-fingerprint *size* (`mean_sem`)
    /// is anti-predictive (typical divergences are in smaller families); source-**line**
    /// span is the real magnitude signal. The terms, with their learned weight signs:
    ///   - `mean_lines` (+) — edit surface; the more lines, the more chances to diverge.
    ///   - `spread` (+) — cross-directory dispersion; far-apart copies are missed more.
    ///   - `invisibility` (+) — `1 − tightness`: copies that share little *text* despite
    ///     being a matched (semantically equivalent) family are the ones a developer can't
    ///     see, so won't update. This is the **inverse** of extractability's `tightness`
    ///     term. Identical copies still carry some hazard, so it floors at 0.3 rather
    ///     than 0.
    ///   - `scope_weight` — a divergence in prod is costlier than in tests.
    ///
    /// (`mean_sem`, `members`, and `params` were tested and dropped — anti-predictive,
    /// redundant, or sign-unstable noise. No `discount` term: the calibration corpus had
    /// negligible vendored/generated code, so it is omitted to keep the score faithful to
    /// the measured formula.)
    pub fn hazard(&self) -> f64 {
        // tightness = invariant fraction; 0 for cross-language (no shared source lines),
        // which makes invisibility maximal — exactly where the data says hazard is highest.
        let tightness = (self.shared_weight / (self.mean_lines.max(1) as f64)).clamp(0.0, 1.0);
        let invisibility = 0.3 + 0.7 * (1.0 - tightness);
        let scope_weight = match self.scope {
            "test" => 0.25,
            "mixed" => 0.5,
            _ => 1.0, // prod
        };
        self.mean_lines as f64
            * spread(self.files, self.modules, self.languages)
            * invisibility
            * scope_weight
    }
}
