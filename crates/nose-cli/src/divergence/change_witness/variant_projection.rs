use super::*;
use crate::divergence::variant::{
    enrich_projected, enrich_source, VariantCaveatCode, VariantEvidence,
};

impl WitnessBuilder<'_> {
    pub(super) fn variant_evidence(
        &mut self,
        changed: &Site,
        skipped: &Site,
        evidence: &mut VariantEvidence,
    ) {
        if changed.is_fragment || skipped.is_fragment {
            evidence.caveat(
                VariantCaveatCode::ProjectionUnavailable,
                ["fragment-unsupported".to_string()],
            );
            return;
        }
        let changed_base = self.project_base(changed);
        let skipped_projection = self.project_base(skipped);
        let current_path = self.current_path(&changed.file);
        let current_projection = match (changed_base.unit.as_ref(), current_path.as_deref()) {
            (Some(base_unit), Some(current_path)) => {
                let ranges = self
                    .current_changed
                    .get(current_path)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                self.project_current(base_unit, current_path, ranges)
            }
            _ => ProjectionAttempt::failed(SemanticProjectionStatus::NotAttempted),
        };
        if let Some(current_path) = current_path.as_deref() {
            let current_span = current_projection
                .unit
                .as_ref()
                .map(|unit| (unit.start_line, unit.end_line));
            enrich_source(
                evidence,
                changed,
                current_path,
                current_span,
                skipped,
                self.current_root,
                self.base_root,
                &mut self.source_lines,
            );
        }
        let mut details = Vec::new();
        if current_projection.status != SemanticProjectionStatus::Ok {
            details.push(format!(
                "current:{}",
                projection_status_label(current_projection.status)
            ));
        }
        if skipped_projection.status != SemanticProjectionStatus::Ok {
            details.push(format!(
                "skipped:{}",
                projection_status_label(skipped_projection.status)
            ));
        }
        match (current_projection.unit, skipped_projection.unit) {
            (Some(changed_unit), Some(skipped_unit)) => {
                let truncated = changed_unit.truncated || skipped_unit.truncated;
                let witness = nose_detect::graded_witness(
                    &changed_unit.dag,
                    &skipped_unit.dag,
                    !changed_unit.exact_safe,
                    !skipped_unit.exact_safe,
                );
                enrich_projected(
                    evidence,
                    witness.as_ref(),
                    changed_unit.origin,
                    skipped_unit.origin,
                    &details,
                    truncated,
                );
            }
            (changed_unit, skipped_unit) => enrich_projected(
                evidence,
                None,
                changed_unit.map_or(UnitOrigin::unknown(), |unit| unit.origin),
                skipped_unit.map_or(UnitOrigin::unknown(), |unit| unit.origin),
                &details,
                false,
            ),
        }
    }
}

fn projection_status_label(status: SemanticProjectionStatus) -> &'static str {
    match status {
        SemanticProjectionStatus::Ok => "ok",
        SemanticProjectionStatus::Missing => "missing",
        SemanticProjectionStatus::Unsupported => "unsupported",
        SemanticProjectionStatus::ReadFailed => "read-failed",
        SemanticProjectionStatus::LowerFailed => "lower-failed",
        SemanticProjectionStatus::UnitMissing => "unit-missing",
        SemanticProjectionStatus::AmbiguousUnit => "ambiguous-unit",
        SemanticProjectionStatus::CapExceeded => "cap-exceeded",
        SemanticProjectionStatus::NotAttempted => "not-attempted",
    }
}
