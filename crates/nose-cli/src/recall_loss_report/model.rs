use serde::Serialize;

#[derive(Serialize)]
pub(super) struct RecallLossReport {
    pub(super) schema_version: u32,
    pub(super) report_kind: &'static str,
    pub(super) privacy: Privacy,
    pub(super) command: CommandContext,
    pub(super) summary: Summary,
    pub(super) soundness_gate: SoundnessGate,
    pub(super) completeness: Completeness,
    pub(super) oracle_under_merges: Vec<UnderMerge>,
    pub(super) oracle_exclusions: OracleExclusions,
    pub(super) import_snapshot_census: nose_frontend::ImportSnapshotCensus,
    pub(super) admission_rejections: Vec<AdmissionRejection>,
    pub(super) by_reason: Vec<ReasonRollup>,
    pub(super) by_obligation: Vec<ObligationRollup>,
    pub(super) top_opportunities: Vec<TopOpportunity>,
}

#[derive(Serialize)]
pub(super) struct Privacy {
    pub(super) local_artifact: bool,
    pub(super) remote_collection: bool,
    pub(super) raw_source_snippets_included: bool,
}

#[derive(Serialize)]
pub(super) struct CommandContext {
    pub(super) command: &'static str,
    pub(super) paths: Vec<String>,
    pub(super) no_cfg_norm: bool,
    pub(super) max_violations: Option<usize>,
}

#[derive(Serialize)]
pub(super) struct Summary {
    pub(super) total_units: usize,
    pub(super) interpretable_units: usize,
    pub(super) excluded_units: usize,
    pub(super) canon_checked: usize,
    pub(super) canon_preservation_violations: usize,
    pub(super) admission_rejections: usize,
}

#[derive(Serialize)]
pub(super) struct SoundnessGate {
    pub(super) fingerprint_groups: usize,
    pub(super) false_merges: usize,
    pub(super) lossy_fingerprint_collisions: usize,
    pub(super) advisory_disagreements: usize,
    pub(super) canon_preservation_violations: usize,
    pub(super) max_violations: Option<usize>,
    pub(super) gate_passed: Option<bool>,
}

#[derive(Serialize)]
pub(super) struct Completeness {
    pub(super) behavior_groups: usize,
    pub(super) behavior_equal_pairs: usize,
    pub(super) fingerprint_equal_pairs: usize,
    pub(super) completeness_percent: Option<f64>,
    pub(super) under_merged_behavior_groups: usize,
    pub(super) structurally_near_under_merged_groups: usize,
}

#[derive(Clone, Serialize)]
pub(super) struct Location {
    pub(super) file: String,
    pub(super) start_line: u32,
    pub(super) end_line: u32,
    pub(super) tokens: usize,
    pub(super) language: String,
}

#[derive(Serialize)]
pub(super) struct UnderMerge {
    pub(super) a: Location,
    pub(super) b: Location,
    pub(super) value_jaccard: f64,
    pub(super) structurally_near: bool,
    pub(super) admission_reasons: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct OracleExclusions {
    pub(super) counts: Vec<ReasonCount>,
    pub(super) by_obligation: Vec<OracleExclusionObligationRollup>,
    pub(super) units: Vec<ExcludedUnit>,
}

#[derive(Serialize)]
pub(super) struct ExcludedUnit {
    pub(super) reason: &'static str,
    pub(super) loc: Location,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) attribution: Option<OracleExclusionAttribution>,
}

#[derive(Serialize)]
pub(super) struct OracleExclusionAttribution {
    pub(super) reason: &'static str,
    pub(super) admission_gate: &'static str,
    pub(super) capability_id: &'static str,
    pub(super) pack_id: Option<&'static str>,
    pub(super) missing_evidence: Vec<&'static str>,
    pub(super) obligation_family: &'static str,
    pub(super) obligation_subreason: &'static str,
    pub(super) oracle_status: &'static str,
}

#[derive(Serialize)]
pub(super) struct OracleExclusionObligationRollup {
    pub(super) exclusion_reason: &'static str,
    pub(super) attribution_reason: &'static str,
    pub(super) obligation_family: String,
    pub(super) obligation_subreason: String,
    pub(super) count: usize,
    pub(super) oracle_excluded: usize,
}

#[derive(Clone, Serialize)]
pub(super) struct AdmissionRejection {
    pub(super) reason: &'static str,
    pub(super) admission_gate: &'static str,
    pub(super) capability_id: &'static str,
    pub(super) pack_id: Option<&'static str>,
    pub(super) missing_evidence: Vec<&'static str>,
    pub(super) obligation_family: &'static str,
    pub(super) obligation_subreason: &'static str,
    pub(super) oracle_status: &'static str,
    pub(super) loc: Location,
    pub(super) value_fingerprint_len: usize,
}

#[derive(Serialize)]
pub(super) struct ReasonRollup {
    pub(super) reason: String,
    pub(super) admission_gate: String,
    pub(super) capability_id: String,
    pub(super) count: usize,
    pub(super) oracle_interpretable: usize,
}

#[derive(Serialize)]
pub(super) struct ObligationRollup {
    pub(super) obligation_family: String,
    pub(super) obligation_subreason: String,
    pub(super) count: usize,
    pub(super) oracle_interpretable: usize,
}

#[derive(Serialize)]
pub(super) struct TopOpportunity {
    pub(super) opportunity_type: &'static str,
    pub(super) reason: String,
    pub(super) a: Location,
    pub(super) b: Location,
    pub(super) value_jaccard: f64,
    pub(super) structurally_near: bool,
}

#[derive(Serialize)]
pub(super) struct ReasonCount {
    pub(super) reason: &'static str,
    pub(super) count: usize,
}
