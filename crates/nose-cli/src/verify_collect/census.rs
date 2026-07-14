use super::VerifyOracle;
use crate::verify_admission::ExactAdmissionRejectionDiagnostic;
use crate::verify_census;

pub(super) struct CensusLocation {
    pub(super) unique: String,
    pub(super) verify: String,
}

/// Record one unit's oracle outcome in the exclusion census (no-op unless the
/// `--exclusion-census` instrument is on). `tag_il`/`tag_root` name the subtree
/// the oracle would have interpreted (the core IL when span-matched, else the
/// fully-normalized unit).
pub(super) fn push_verify_census(
    oracle: &mut VerifyOracle,
    location: &CensusLocation,
    tag_il: &nose_il::Il,
    tag_root: nose_il::NodeId,
    fp: &[u64],
    outcome: verify_census::CensusOutcome,
) {
    if !oracle.census_enabled {
        return;
    }
    oracle.census.push(verify_census::CensusUnit {
        loc: location.unique.clone(),
        verify_loc: location.verify.clone(),
        language: tag_il.meta.lang.name(),
        reason: outcome.reason,
        fp: fp.to_vec(),
        tags: verify_census::census_tags(tag_il, tag_root),
        exact_safe: outcome.exact_safe,
        claimable: outcome.claimable,
        classification: outcome.classification,
        obligation_family: outcome.obligation_family,
        obligation_subreason: outcome.obligation_subreason,
        first_blocker: outcome.first_blocker,
    });
}

pub(super) fn synthetic_blocker(
    category: &'static str,
    capability_id: &'static str,
    construct: &'static str,
) -> nose_normalize::InterpreterBlocker {
    nose_normalize::InterpreterBlocker {
        category,
        capability_id,
        blocker_stack: vec![nose_normalize::InterpreterBlockerFrame {
            role: "collect",
            construct: construct.to_string(),
        }],
    }
}

pub(super) fn census_outcome(
    reason: &'static str,
    exact_safe: bool,
    claimable: bool,
    diagnostic: Option<&ExactAdmissionRejectionDiagnostic>,
    first_blocker: Option<nose_normalize::InterpreterBlocker>,
) -> verify_census::CensusOutcome {
    let (classification, obligation_family, obligation_subreason) = match reason {
        "interpretable" => (
            "interpretable",
            "interpretable".to_string(),
            "interpretable".to_string(),
        ),
        "no-core-span" => (
            "core-span-missing",
            "oracle-capability".to_string(),
            "il.core-span".to_string(),
        ),
        "battery-bail"
            if first_blocker
                .as_ref()
                .is_some_and(|blocker| blocker.capability_id == "budget.oracle-cost") =>
        {
            (
                "oracle-cost-budget",
                "oracle-capability".to_string(),
                "budget.oracle-cost".to_string(),
            )
        }
        "empty-fp" => (
            "empty-value-fingerprint",
            "oracle-capability".to_string(),
            "value.empty-fingerprint".to_string(),
        ),
        "path-bail" => (
            "path-exploration-budget",
            "oracle-capability".to_string(),
            "budget.symbolic-branch-sites".to_string(),
        ),
        _ => match diagnostic {
            Some(diagnostic) => {
                let (family, subreason) = crate::recall_loss_report::rejection_obligation(
                    diagnostic.reason,
                    &diagnostic.missing_evidence,
                );
                (
                    "semantic-boundary-attributed",
                    family.to_string(),
                    subreason.to_string(),
                )
            }
            None => {
                let capability_id = first_blocker
                    .as_ref()
                    .map_or("oracle.capability-unknown", |blocker| blocker.capability_id);
                (
                    "missing-oracle-support",
                    "oracle-capability".to_string(),
                    capability_id.to_string(),
                )
            }
        },
    };
    verify_census::CensusOutcome {
        reason,
        exact_safe,
        claimable,
        classification,
        obligation_family,
        obligation_subreason,
        first_blocker,
    }
}
