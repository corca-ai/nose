use crate::legacy_prelude::*;
use crate::verify_report::multiset_jaccard_u64;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
struct RecallLossReport {
    schema_version: u32,
    report_kind: &'static str,
    privacy: Privacy,
    command: CommandContext,
    summary: Summary,
    soundness_gate: SoundnessGate,
    completeness: Completeness,
    oracle_under_merges: Vec<UnderMerge>,
    oracle_exclusions: OracleExclusions,
    admission_rejections: Vec<AdmissionRejection>,
    by_reason: Vec<ReasonRollup>,
    top_opportunities: Vec<TopOpportunity>,
}

#[derive(Serialize)]
struct Privacy {
    local_artifact: bool,
    remote_collection: bool,
    raw_source_snippets_included: bool,
}

#[derive(Serialize)]
struct CommandContext {
    command: &'static str,
    paths: Vec<String>,
    no_cfg_norm: bool,
    max_violations: Option<usize>,
}

#[derive(Serialize)]
struct Summary {
    total_units: usize,
    interpretable_units: usize,
    excluded_units: usize,
    canon_checked: usize,
    canon_preservation_violations: usize,
    admission_rejections: usize,
}

#[derive(Serialize)]
struct SoundnessGate {
    fingerprint_groups: usize,
    false_merges: usize,
    lossy_fingerprint_collisions: usize,
    advisory_disagreements: usize,
    canon_preservation_violations: usize,
    max_violations: Option<usize>,
    gate_passed: Option<bool>,
}

#[derive(Serialize)]
struct Completeness {
    behavior_groups: usize,
    behavior_equal_pairs: usize,
    fingerprint_equal_pairs: usize,
    completeness_percent: Option<f64>,
    under_merged_behavior_groups: usize,
    structurally_near_under_merged_groups: usize,
}

#[derive(Clone, Serialize)]
struct Location {
    file: String,
    start_line: u32,
    end_line: u32,
    tokens: usize,
    language: String,
}

#[derive(Serialize)]
struct UnderMerge {
    a: Location,
    b: Location,
    value_jaccard: f64,
    structurally_near: bool,
    admission_reasons: Vec<String>,
}

#[derive(Serialize)]
struct OracleExclusions {
    counts: Vec<ReasonCount>,
    units: Vec<ExcludedUnit>,
}

#[derive(Serialize)]
struct ExcludedUnit {
    reason: &'static str,
    loc: Location,
}

#[derive(Clone, Serialize)]
struct AdmissionRejection {
    reason: &'static str,
    admission_gate: &'static str,
    capability_id: &'static str,
    pack_id: Option<&'static str>,
    missing_evidence: Vec<&'static str>,
    oracle_status: &'static str,
    loc: Location,
    value_fingerprint_len: usize,
}

#[derive(Serialize)]
struct ReasonRollup {
    reason: String,
    admission_gate: String,
    capability_id: String,
    count: usize,
    oracle_interpretable: usize,
}

#[derive(Serialize)]
struct TopOpportunity {
    opportunity_type: &'static str,
    reason: String,
    a: Location,
    b: Location,
    value_jaccard: f64,
    structurally_near: bool,
}

#[derive(Serialize)]
struct ReasonCount {
    reason: &'static str,
    count: usize,
}

pub(super) fn write_report(
    path: &Path,
    oracle: &VerifyOracle,
    paths: &[PathBuf],
    no_cfg_norm: bool,
    max_violations: Option<usize>,
) -> Result<()> {
    std::fs::write(
        path,
        serde_json::to_string_pretty(&build_report(oracle, paths, no_cfg_norm, max_violations))?,
    )
    .with_context(|| format!("writing recall-loss report {}", path.display()))
}

fn build_report(
    oracle: &VerifyOracle,
    paths: &[PathBuf],
    no_cfg_norm: bool,
    max_violations: Option<usize>,
) -> RecallLossReport {
    let soundness = soundness_gate(&oracle.recs, oracle.canon_violations.len(), max_violations);
    let (completeness, under_merges) = completeness_report(&oracle.recs);
    let admission_rejections = admission_rejections(&oracle.recs);
    let by_reason = reason_rollups(&admission_rejections);
    let top_opportunities = top_opportunities(&under_merges);

    RecallLossReport {
        schema_version: SCHEMA_VERSION,
        report_kind: "recall-loss-diagnostics",
        privacy: Privacy {
            local_artifact: true,
            remote_collection: false,
            raw_source_snippets_included: false,
        },
        command: CommandContext {
            command: "nose verify --recall-loss-report",
            paths: paths.iter().map(|p| p.display().to_string()).collect(),
            no_cfg_norm,
            max_violations,
        },
        summary: Summary {
            total_units: oracle.total,
            interpretable_units: oracle.recs.len(),
            excluded_units: oracle.total.saturating_sub(oracle.recs.len()),
            canon_checked: oracle.canon_checked,
            canon_preservation_violations: oracle.canon_violations.len(),
            admission_rejections: admission_rejections.len(),
        },
        soundness_gate: soundness,
        completeness,
        oracle_under_merges: under_merges,
        oracle_exclusions: oracle_exclusions(&oracle.exclusions),
        admission_rejections,
        by_reason,
        top_opportunities,
    }
}

fn soundness_gate(
    recs: &[VerifyRec],
    canon_preservation_violations: usize,
    max_violations: Option<usize>,
) -> SoundnessGate {
    let has_sym = |r: &VerifyRec| r.beh.iter().any(nose_normalize::behavior_has_sym);
    let mut by_fp: HashMap<&[u64], Vec<&VerifyRec>> = HashMap::new();
    for rec in recs {
        by_fp.entry(&rec.fp).or_default().push(rec);
    }

    let mut fingerprint_groups = 0usize;
    let mut false_merges = 0usize;
    let mut lossy_fingerprint_collisions = 0usize;
    let mut advisory_disagreements = 0usize;
    for members in by_fp.values() {
        if members.len() < 2 {
            continue;
        }
        fingerprint_groups += 1;
        let first = members[0];
        for rec in &members[1..] {
            if rec.beh == first.beh {
                continue;
            }
            if has_sym(first) || has_sym(rec) || first.domain_sig != rec.domain_sig {
                advisory_disagreements += 1;
            } else if first.claimable && rec.claimable {
                false_merges += 1;
            } else {
                lossy_fingerprint_collisions += 1;
            }
        }
    }

    SoundnessGate {
        fingerprint_groups,
        false_merges,
        lossy_fingerprint_collisions,
        advisory_disagreements,
        canon_preservation_violations,
        max_violations,
        gate_passed: max_violations
            .map(|budget| false_merges <= budget && canon_preservation_violations == 0),
    }
}

fn completeness_report(recs: &[VerifyRec]) -> (Completeness, Vec<UnderMerge>) {
    let mut by_beh: HashMap<&[nose_normalize::Behavior], Vec<&VerifyRec>> = HashMap::new();
    for rec in recs {
        if !is_trivial_behavior(&rec.beh) && !rec.beh.iter().any(nose_normalize::behavior_has_sym) {
            by_beh.entry(&rec.beh).or_default().push(rec);
        }
    }

    let mut behavior_equal_pairs = 0usize;
    let mut fingerprint_equal_pairs = 0usize;
    let mut under_merged_behavior_groups = 0usize;
    let mut structurally_near_under_merged_groups = 0usize;
    let mut under_merges = Vec::new();

    for members in by_beh.values() {
        if members.len() < 2 {
            continue;
        }
        let k = members.len();
        behavior_equal_pairs += k * (k - 1) / 2;
        let mut by_fp: HashMap<&[u64], Vec<&&VerifyRec>> = HashMap::new();
        for rec in members {
            by_fp.entry(&rec.fp).or_default().push(rec);
        }
        for sub in by_fp.values() {
            let s = sub.len();
            fingerprint_equal_pairs += s * (s - 1) / 2;
        }
        if by_fp.len() > 1 {
            under_merged_behavior_groups += 1;
            let miss = best_split_pair(by_fp.values().map(|v| *v[0]).collect());
            if miss.structurally_near {
                structurally_near_under_merged_groups += 1;
            }
            under_merges.push(miss);
        }
    }

    under_merges.sort_by(|a, b| {
        b.value_jaccard
            .partial_cmp(&a.value_jaccard)
            .unwrap()
            .then(a.a.file.cmp(&b.a.file))
            .then(a.a.start_line.cmp(&b.a.start_line))
            .then(a.b.file.cmp(&b.b.file))
            .then(a.b.start_line.cmp(&b.b.start_line))
    });

    (
        Completeness {
            behavior_groups: by_beh.values().filter(|members| members.len() >= 2).count(),
            behavior_equal_pairs,
            fingerprint_equal_pairs,
            completeness_percent: (behavior_equal_pairs > 0)
                .then(|| 100.0 * fingerprint_equal_pairs as f64 / behavior_equal_pairs as f64),
            under_merged_behavior_groups,
            structurally_near_under_merged_groups,
        },
        under_merges,
    )
}

fn best_split_pair(mut reps: Vec<&VerifyRec>) -> UnderMerge {
    reps.sort_by(|a, b| a.loc.cmp(&b.loc));
    let mut best = (0.0f64, reps[0], reps[0]);
    for i in 0..reps.len() {
        for j in (i + 1)..reps.len() {
            let vj = multiset_jaccard_u64(&reps[i].fp, &reps[j].fp);
            if vj >= best.0 {
                best = (vj, reps[i], reps[j]);
            }
        }
    }
    let (a, b) = if best.1.loc <= best.2.loc {
        (best.1, best.2)
    } else {
        (best.2, best.1)
    };
    let value_jaccard = best.0;
    UnderMerge {
        a: loc(a),
        b: loc(b),
        value_jaccard,
        structurally_near: value_jaccard >= 0.7,
        admission_reasons: pair_admission_reasons(a, b),
    }
}

fn admission_rejections(recs: &[VerifyRec]) -> Vec<AdmissionRejection> {
    let mut items: Vec<_> = recs.iter().filter_map(unit_admission_rejection).collect();
    items.sort_by(|a, b| {
        a.loc
            .file
            .cmp(&b.loc.file)
            .then(a.loc.start_line.cmp(&b.loc.start_line))
            .then(a.reason.cmp(b.reason))
    });
    items
}

fn unit_admission_rejection(rec: &VerifyRec) -> Option<AdmissionRejection> {
    rec.admission_rejection
        .as_ref()
        .map(|reason| AdmissionRejection {
            reason: reason.reason,
            admission_gate: reason.admission_gate,
            capability_id: reason.capability_id,
            pack_id: reason.pack_id,
            missing_evidence: reason.missing_evidence.clone(),
            oracle_status: "interpretable",
            loc: loc(rec),
            value_fingerprint_len: rec.fp.len(),
        })
}

fn pair_admission_reasons(a: &VerifyRec, b: &VerifyRec) -> Vec<String> {
    let mut reasons = Vec::new();
    if let Some(reason) = &a.admission_rejection {
        reasons.push(format!("a:{}", reason.reason));
    }
    if let Some(reason) = &b.admission_rejection {
        reasons.push(format!("b:{}", reason.reason));
    }
    if reasons.is_empty() {
        reasons.push("fingerprint-split".to_string());
    }
    reasons
}

fn reason_rollups(rejections: &[AdmissionRejection]) -> Vec<ReasonRollup> {
    let mut by_key: HashMap<(&str, &str, &str), usize> = HashMap::new();
    for rejection in rejections {
        *by_key
            .entry((
                rejection.reason,
                rejection.admission_gate,
                rejection.capability_id,
            ))
            .or_default() += 1;
    }
    let mut rollups: Vec<_> = by_key
        .into_iter()
        .map(
            |((reason, admission_gate, capability_id), count)| ReasonRollup {
                reason: reason.to_string(),
                admission_gate: admission_gate.to_string(),
                capability_id: capability_id.to_string(),
                count,
                oracle_interpretable: count,
            },
        )
        .collect();
    rollups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.reason.cmp(&b.reason))
            .then(a.admission_gate.cmp(&b.admission_gate))
    });
    rollups
}

fn top_opportunities(under_merges: &[UnderMerge]) -> Vec<TopOpportunity> {
    under_merges
        .iter()
        .take(50)
        .map(|miss| {
            let reason = miss
                .admission_reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "fingerprint-split".to_string());
            TopOpportunity {
                opportunity_type: if reason == "fingerprint-split" {
                    "oracle-under-merge"
                } else {
                    "oracle-under-merge-with-admission-rejection"
                },
                reason,
                a: miss.a.clone(),
                b: miss.b.clone(),
                value_jaccard: miss.value_jaccard,
                structurally_near: miss.structurally_near,
            }
        })
        .collect()
}

fn oracle_exclusions(exclusions: &VerifyExclusions) -> OracleExclusions {
    let mut units: Vec<_> = exclusions
        .units
        .iter()
        .map(|unit| ExcludedUnit {
            reason: unit.reason.label(),
            loc: Location {
                file: unit.file.clone(),
                start_line: unit.start,
                end_line: unit.end,
                tokens: unit.tokens,
                language: language_from_path(&unit.file),
            },
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
        units,
    }
}

fn loc(rec: &VerifyRec) -> Location {
    Location {
        file: rec.file.clone(),
        start_line: rec.start,
        end_line: rec.end,
        tokens: rec.tokens,
        language: language_from_path(&rec.file),
    }
}

fn language_from_path(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match ext {
        "c" | "h" => "c",
        "css" => "css",
        "go" => "go",
        "html" | "htm" => "html",
        "java" => "java",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "md" | "markdown" => "markdown",
        "py" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "swift" => "swift",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        _ => "unknown",
    }
    .to_string()
}
