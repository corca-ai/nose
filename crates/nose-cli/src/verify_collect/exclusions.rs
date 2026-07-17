use super::ExactAdmissionRejectionDiagnostic;

#[derive(Clone, Copy)]
pub(crate) enum VerifyExclusionReason {
    CoreMissing,
    BatteryBail,
    EmptyFingerprint,
    Uninterpretable,
    /// #244 fail-closed: the unit forked on more symbolic If/ternary sites than
    /// the per-execution exploration cap allows.
    PathBail,
}

impl VerifyExclusionReason {
    pub(crate) fn label(self) -> &'static str {
        match self {
            VerifyExclusionReason::CoreMissing => "core-missing",
            VerifyExclusionReason::BatteryBail => "battery-bail",
            VerifyExclusionReason::EmptyFingerprint => "empty-fingerprint",
            VerifyExclusionReason::Uninterpretable => "uninterpretable",
            VerifyExclusionReason::PathBail => "path-bail",
        }
    }
}

pub(crate) struct VerifyExcludedUnit {
    pub(crate) reason: VerifyExclusionReason,
    pub(crate) file: String,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) tokens: usize,
    pub(crate) diagnostic: Option<ExactAdmissionRejectionDiagnostic>,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeDiagnosticSource<'a> {
    pub(crate) il: &'a nose_il::Il,
    pub(crate) root: Option<nose_il::NodeId>,
}

#[derive(Default)]
pub(crate) struct VerifyExclusions {
    pub(crate) core_missing: usize,
    pub(crate) battery_bail: usize,
    pub(crate) empty_fingerprint: usize,
    pub(crate) uninterpretable: usize,
    pub(crate) path_bail: usize,
    pub(crate) units: Vec<VerifyExcludedUnit>,
}

impl VerifyExclusions {
    pub(crate) fn record_core_missing(&mut self, file: &str, span: nose_il::Span, tokens: usize) {
        self.record(VerifyExclusionReason::CoreMissing, file, span, tokens, None);
    }

    pub(crate) fn record_battery_bail(&mut self, file: &str, span: nose_il::Span, tokens: usize) {
        self.record(VerifyExclusionReason::BatteryBail, file, span, tokens, None);
    }

    pub(crate) fn record_empty_fingerprint(
        &mut self,
        file: &str,
        span: nose_il::Span,
        tokens: usize,
    ) {
        self.record(
            VerifyExclusionReason::EmptyFingerprint,
            file,
            span,
            tokens,
            None,
        );
    }

    pub(crate) fn record(
        &mut self,
        reason: VerifyExclusionReason,
        file: &str,
        span: nose_il::Span,
        tokens: usize,
        diagnostic: Option<ExactAdmissionRejectionDiagnostic>,
    ) {
        match reason {
            VerifyExclusionReason::CoreMissing => self.core_missing += 1,
            VerifyExclusionReason::BatteryBail => self.battery_bail += 1,
            VerifyExclusionReason::EmptyFingerprint => self.empty_fingerprint += 1,
            VerifyExclusionReason::Uninterpretable => self.uninterpretable += 1,
            VerifyExclusionReason::PathBail => self.path_bail += 1,
        }
        self.units.push(VerifyExcludedUnit {
            reason,
            file: file.to_string(),
            start: span.start_line,
            end: span.end_line,
            tokens,
            diagnostic,
        });
    }

    pub(crate) fn append(&mut self, other: VerifyExclusions) {
        self.core_missing += other.core_missing;
        self.battery_bail += other.battery_bail;
        self.empty_fingerprint += other.empty_fingerprint;
        self.uninterpretable += other.uninterpretable;
        self.path_bail += other.path_bail;
        self.units.extend(other.units);
    }

    pub(crate) fn total(&self) -> usize {
        self.core_missing
            + self.battery_bail
            + self.empty_fingerprint
            + self.uninterpretable
            + self.path_bail
    }
}
