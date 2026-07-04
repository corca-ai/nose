use super::location::excluded_unit_location;
use super::model::{
    ExcludedUnit, OracleExclusionAttribution, OracleExclusionClassificationRollup,
    OracleExclusionObligationRollup, OracleExclusions, ReasonCount,
};
use super::obligations::rejection_obligation;
use crate::verify_admission::ExactAdmissionRejectionDiagnostic;
use crate::verify_collect::{VerifyExcludedUnit, VerifyExclusions};
use std::collections::HashMap;

pub(super) fn oracle_exclusions(exclusions: &VerifyExclusions) -> OracleExclusions {
    let by_obligation = oracle_exclusion_obligation_rollups(&exclusions.units);
    let by_classification = oracle_exclusion_classification_rollups(exclusions, &by_obligation);
    let mut units: Vec<_> = exclusions
        .units
        .iter()
        .map(|unit| ExcludedUnit {
            reason: unit.reason.label(),
            loc: excluded_unit_location(unit),
            attribution: unit.diagnostic.as_ref().map(oracle_exclusion_attribution),
        })
        .collect();
    units.sort_by(|a, b| {
        a.loc
            .file
            .cmp(&b.loc.file)
            .then(a.loc.start_line.cmp(&b.loc.start_line))
            .then(a.reason.cmp(b.reason))
    });
    OracleExclusions {
        counts: vec![
            ReasonCount {
                reason: "core-missing",
                count: exclusions.core_missing,
            },
            ReasonCount {
                reason: "battery-bail",
                count: exclusions.battery_bail,
            },
            ReasonCount {
                reason: "empty-fingerprint",
                count: exclusions.empty_fingerprint,
            },
            ReasonCount {
                reason: "uninterpretable",
                count: exclusions.uninterpretable,
            },
            ReasonCount {
                reason: "path-bail",
                count: exclusions.path_bail,
            },
        ],
        by_classification,
        by_obligation,
        units,
    }
}

fn oracle_exclusion_attribution(
    diagnostic: &ExactAdmissionRejectionDiagnostic,
) -> OracleExclusionAttribution {
    let (obligation_family, obligation_subreason) =
        rejection_obligation(diagnostic.reason, &diagnostic.missing_evidence);
    OracleExclusionAttribution {
        reason: diagnostic.reason,
        admission_gate: diagnostic.admission_gate,
        capability_id: diagnostic.capability_id,
        pack_id: diagnostic.pack_id,
        missing_evidence: diagnostic.missing_evidence.clone(),
        obligation_family,
        obligation_subreason,
        oracle_status: "excluded",
    }
}

fn oracle_exclusion_classification_rollups(
    exclusions: &VerifyExclusions,
    by_obligation: &[OracleExclusionObligationRollup],
) -> Vec<OracleExclusionClassificationRollup> {
    let semantic_boundary_attributed = by_obligation
        .iter()
        .filter(|row| row.exclusion_reason == "uninterpretable")
        .map(|row| row.oracle_excluded)
        .sum::<usize>();
    let missing_oracle_support = exclusions
        .uninterpretable
        .saturating_sub(semantic_boundary_attributed);

    let mut rollups = Vec::new();
    for (reason, classification, counter) in [
        (
            "core-missing",
            "core-span-missing",
            ClassificationCounter::unattributed(exclusions.core_missing),
        ),
        (
            "battery-bail",
            "oracle-cost-budget",
            ClassificationCounter::unattributed(exclusions.battery_bail),
        ),
        (
            "empty-fingerprint",
            "empty-value-fingerprint",
            ClassificationCounter::unattributed(exclusions.empty_fingerprint),
        ),
        (
            "path-bail",
            "path-exploration-budget",
            ClassificationCounter::unattributed(exclusions.path_bail),
        ),
        (
            "uninterpretable",
            "semantic-boundary-attributed",
            ClassificationCounter::attributed(semantic_boundary_attributed),
        ),
        (
            "uninterpretable",
            "missing-oracle-support",
            ClassificationCounter::unattributed(missing_oracle_support),
        ),
    ] {
        push_classification_rollup(&mut rollups, reason, classification, counter);
    }

    rollups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.exclusion_reason.cmp(b.exclusion_reason))
            .then(a.classification.cmp(b.classification))
    });
    rollups
}

struct ClassificationCounter {
    count: usize,
    attributed_units: usize,
    unattributed_units: usize,
}

impl ClassificationCounter {
    fn attributed(count: usize) -> Self {
        Self {
            count,
            attributed_units: count,
            unattributed_units: 0,
        }
    }

    fn unattributed(count: usize) -> Self {
        Self {
            count,
            attributed_units: 0,
            unattributed_units: count,
        }
    }
}

fn push_classification_rollup(
    rollups: &mut Vec<OracleExclusionClassificationRollup>,
    exclusion_reason: &'static str,
    classification: &'static str,
    counter: ClassificationCounter,
) {
    if counter.count == 0 {
        return;
    }
    rollups.push(OracleExclusionClassificationRollup {
        exclusion_reason,
        classification,
        count: counter.count,
        oracle_excluded: counter.count,
        attributed_units: counter.attributed_units,
        unattributed_units: counter.unattributed_units,
    });
}

fn oracle_exclusion_obligation_rollups(
    units: &[VerifyExcludedUnit],
) -> Vec<OracleExclusionObligationRollup> {
    let mut by_key: HashMap<(&'static str, &'static str, &'static str, &'static str), usize> =
        HashMap::new();
    for unit in units {
        let Some(diagnostic) = &unit.diagnostic else {
            continue;
        };
        let (obligation_family, obligation_subreason) =
            rejection_obligation(diagnostic.reason, &diagnostic.missing_evidence);
        *by_key
            .entry((
                unit.reason.label(),
                diagnostic.reason,
                obligation_family,
                obligation_subreason,
            ))
            .or_default() += 1;
    }
    let mut rollups: Vec<_> = by_key
        .into_iter()
        .map(
            |(
                (exclusion_reason, attribution_reason, obligation_family, obligation_subreason),
                count,
            )| OracleExclusionObligationRollup {
                exclusion_reason,
                attribution_reason,
                obligation_family: obligation_family.to_string(),
                obligation_subreason: obligation_subreason.to_string(),
                count,
                oracle_excluded: count,
            },
        )
        .collect();
    rollups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.exclusion_reason.cmp(b.exclusion_reason))
            .then(a.attribution_reason.cmp(b.attribution_reason))
            .then(a.obligation_family.cmp(&b.obligation_family))
            .then(a.obligation_subreason.cmp(&b.obligation_subreason))
    });
    rollups
}
