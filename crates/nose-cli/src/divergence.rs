//! Divergent-edit detection for `nose query <path> base=<ref>`.
//!
//! Given a git ref, this detects clone families **at that base** (where every
//! copy still matches), finds which lines the diff changed, and flags every family where
//! *some* copies were edited but *siblings were not* — a likely un-propagated edit ("you
//! changed X; its clone Y was not updated"). This is the divergent-edit (Kim *Inconsistent
//! Change*) predicate applied to one diff.
//!
//! Detection runs at the base, not the working tree, on purpose: an edit can push a copy out
//! of its clone family (a fix changes its shape), so it would be invisible in the current
//! tree. At the base the family is still intact, and the diff tells us which member moved.
//!
//! The structural signal is a candidate surfacer, not a proof: inspect the flagged siblings.

mod change_witness;
mod detect;
mod git;
mod output;
mod targets;
#[cfg(test)]
mod tests;
mod variant;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
pub(crate) use nose_detect::DIVERGENT_EDIT_V2_POLICY;
use nose_detect::{
    divergence_policy, DivergencePolicyInput, DivergenceScope, EnclosingUnit, FragmentKind, Loc,
    RefactorFamily, SharedLogicEvidence,
};
pub(crate) use nose_detect::{DivergenceLane, DivergencePolicyDecision, DivergenceTier};

pub(crate) use detect::{detect_divergences, divergences_fire};
pub(crate) use output::{divergence_items_json, lane_value};

#[cfg(test)]
pub(crate) const DIVERGENCE_LANE_VALUES: &[&str] = &["base-divergence", "new-copy"];
#[cfg(test)]
pub(crate) const DIVERGENCE_TIER_VALUES: &[&str] =
    &["strict", "review", "report-only", "suppressed"];
#[cfg(test)]
pub(crate) const DIVERGENCE_TIER_REASON_VALUES: &[&str] = &[
    "shared_logic_touched",
    "shared_logic_not_touched",
    "shared_logic_unproven",
    "non_test_scope",
    "test_scope",
    "variant_signal",
    "test_scaffolding",
    "grouping_artifact",
    "new_copy_no_base_member",
    "structured_ignore",
    "unclassified",
];
#[cfg(test)]
pub(crate) const DIVERGENCE_TAXONOMY_HINT_VALUES: &[&str] = &[
    "missed_propagation",
    "no_propagation_needed",
    "intentional_variant",
    "test_scaffolding",
    "grouping_artifact",
    "unclear",
];
#[cfg(test)]
pub(crate) const DIVERGENCE_SUPPRESSION_KIND_VALUES: &[&str] = &["structured-ignore"];

pub(crate) fn divergence_sarif(
    flagged: &[Divergence],
    top: Option<usize>,
    top_zero_spelling: &str,
) -> Result<String> {
    output::divergence_sarif(flagged, top, top_zero_spelling)
}

/// A flagged family: a clone whose copies were edited apart in this change set. Locations
/// are repo-relative (the report navigates the real working tree). `pub(crate)` so the
/// `nose query <paths> base=<ref>` view renders the same findings (the divergence/query
/// unification): query reuses this exact detection, preserving §BV fire precision.
pub(crate) struct Divergence {
    pub(crate) lane: DivergenceLane,
    pub(crate) family_id: String,
    pub(crate) similarity: f64,
    pub(crate) hazard: f64,
    pub(crate) divergence_priority: u8,
    pub(crate) complexity: usize,
    /// Family scope: `prod` / `test` / `mixed` (test scaffolding fires differently).
    pub(crate) scope: &'static str,
    /// The family's equivalence-witness kind (`exact-value-graph`,
    /// `copy-paste-run`, `shared-sub-dag`, `structural-similarity`).
    pub(crate) witness_kind: Option<&'static str>,
    /// The #245 conservative gate verdict: some changed member PROVABLY touches
    /// lines it shares with an un-updated sibling.
    pub(crate) fire_eligible: bool,
    /// The near family's graded equivalence witness (#315), when present — evidence
    /// for the consumer to judge a fire: a clean `equal_modulo_holes` family is a
    /// strong missed-propagation candidate, while `referent_mismatches` /
    /// `decorator-differs` mark a family whose copies are not really the same logic
    /// (a likely false fire). It does NOT gate `fire_eligible` (that would risk
    /// dropping a genuine shared-body propagation).
    pub(crate) graded: Option<nose_detect::GradedWitness>,
    /// Members whose base span was changed by the diff (the edit landed here).
    pub(crate) changed: Vec<Site>,
    /// Sibling members the change did *not* touch (where it may be missing).
    pub(crate) not_updated: Vec<Site>,
    /// Pair-local propagation targets backed by detector-accepted edges. Family
    /// members that are reachable only through transitive closure stay in the
    /// changed/not-updated review context but never appear here.
    pub(crate) targets: Vec<PropagationTarget>,
}

#[derive(Clone)]
pub(crate) struct PropagationTarget {
    /// Directed identity of (base changed site, base skipped site), independent
    /// of the temporary base worktree and enclosing-family clustering.
    pub(crate) target_id: String,
    pub(crate) changed: Site,
    pub(crate) skipped: Site,
    pub(crate) direct_witness: DirectPairWitness,
    /// Pair-local evidence that the two sites deliberately occupy different roles.
    /// #851 records this evidence. #852 found no non-degenerate v3 policy that
    /// qualified to consume it, so the active v2 tier remains unchanged.
    pub(crate) variant_evidence: variant::VariantEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub(crate) struct DirectPairWitness {
    pub(crate) kind: &'static str,
    pub(crate) similarity: f64,
}

impl Divergence {
    pub(crate) fn policy_decision(&self) -> DivergencePolicyDecision {
        let scope = if self.scope == "prod" {
            DivergenceScope::Production
        } else {
            DivergenceScope::TestOrMixed
        };
        let shared_logic = if self
            .changed
            .iter()
            .any(|site| site.touches_shared == Some(true))
        {
            SharedLogicEvidence::Touched
        } else if self
            .changed
            .iter()
            .any(|site| site.touches_shared == Some(false))
        {
            SharedLogicEvidence::NotTouched
        } else {
            SharedLogicEvidence::Unproven
        };
        divergence_policy(DivergencePolicyInput {
            lane: self.lane,
            scope,
            shared_logic,
        })
    }

    pub(crate) fn gate_fail_default(&self) -> bool {
        self.policy_decision().gate.fail_default
    }
}

#[derive(Clone)]
pub(crate) struct Site {
    pub(crate) file: String,
    pub(crate) name: Option<String>,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) lang: String,
    pub(crate) kind: nose_il::UnitKind,
    pub(crate) span_lines: u32,
    pub(crate) span_tokens: usize,
    pub(crate) is_fragment: bool,
    pub(crate) fragment_kind: Option<FragmentKind>,
    pub(crate) reason_code: Option<&'static str>,
    pub(crate) enclosing_unit: Option<EnclosingUnit>,
    /// For CHANGED sites: does the diff touch lines this member shares with an
    /// un-updated sibling? `Some(false)` = the edit stayed inside this member's
    /// varying spots; `None` = unprovable (unreadable source / capped diff) or a
    /// not-updated site.
    pub(crate) touches_shared: Option<bool>,
    /// Bounded base-to-current semantic change evidence (#849). This is deliberately
    /// presentation-only in v2: neither `policy_decision()` nor
    /// `gate_fail_default()` reads it.
    pub(crate) semantic_change: Option<SemanticChangeWitness>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticWitnessStatus {
    Complete,
    Advisory,
    Unavailable,
}

impl SemanticWitnessStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Advisory => "advisory",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticChangeKind {
    NoSemanticDelta,
    Replacement,
    Deletion,
    Insertion,
    Mixed,
    Unknown,
}

impl SemanticChangeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NoSemanticDelta => "no-semantic-delta",
            Self::Replacement => "replacement",
            Self::Deletion => "deletion",
            Self::Insertion => "insertion",
            Self::Mixed => "mixed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticChangeFacet {
    Value,
    Return,
    Control,
    Effect,
}

impl SemanticChangeFacet {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Return => "return",
            Self::Control => "control",
            Self::Effect => "effect",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticAlignment {
    ExactSpan,
    StableName,
    ChangedRange,
    NearestSpan,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticProjectionStatus {
    Ok,
    Missing,
    Unsupported,
    ReadFailed,
    LowerFailed,
    UnitMissing,
    AmbiguousUnit,
    CapExceeded,
    NotAttempted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticWitnessCaveat {
    PureInsertion,
    MixedChange,
    MissingCurrentUnit,
    UnsupportedLanguage,
    LossyBaseLowering,
    LossyCurrentLowering,
    FragmentUnsupported,
    AmbiguousAlignment,
    HeuristicAlignment,
    UnresolvedReferent,
    NoAffectedSemanticNode,
    NoSharedSemanticNode,
    ScopedDeltaUnmapped,
    Truncated,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct SemanticWitnessCoverage {
    pub(crate) base_affected_nodes: usize,
    pub(crate) current_affected_nodes: usize,
    pub(crate) mapped_shared_nodes: usize,
    pub(crate) sibling_units_checked: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct SemanticWitnessCaps {
    pub(crate) max_files: usize,
    pub(crate) max_file_bytes: usize,
    pub(crate) max_changed_sites_per_family: usize,
    pub(crate) max_siblings_per_family: usize,
    pub(crate) max_targets_per_family: usize,
    pub(crate) max_units_per_file: usize,
    pub(crate) max_nodes_per_unit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemanticSinkKind {
    Return,
    Cond,
    Effect,
    Break,
    Throw,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct SemanticSinkDelta {
    pub(crate) kind: SemanticSinkKind,
    pub(crate) removed: usize,
    pub(crate) inserted: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct SemanticChangeWitness {
    pub(crate) status: SemanticWitnessStatus,
    pub(crate) change_kind: SemanticChangeKind,
    pub(crate) facets: Vec<SemanticChangeFacet>,
    pub(crate) alignment: SemanticAlignment,
    pub(crate) base_projection: SemanticProjectionStatus,
    pub(crate) current_projection: SemanticProjectionStatus,
    pub(crate) coverage: SemanticWitnessCoverage,
    pub(crate) sink_deltas: Vec<SemanticSinkDelta>,
    pub(crate) caveats: Vec<SemanticWitnessCaveat>,
    pub(crate) caps: SemanticWitnessCaps,
}

impl SemanticChangeWitness {
    pub(crate) fn concise_label(&self) -> String {
        let facets = self
            .facets
            .iter()
            .map(|facet| facet.as_str())
            .collect::<Vec<_>>()
            .join("+");
        if facets.is_empty() {
            format!("{} {}", self.status.as_str(), self.change_kind.as_str())
        } else {
            format!(
                "{} {} ({facets})",
                self.status.as_str(),
                self.change_kind.as_str()
            )
        }
    }
}
