use crate::oracle_gate::is_trivial_behavior;
use crate::verify_collect::{VerifyOracle, VerifyRec};
use crate::verify_soundness::count_verify_soundness;
use anyhow::{Context, Result};
use nose_detect::multiset_jaccard;
use nose_il::Corpus;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod location;
mod model;
mod obligations;
mod oracle_exclusions;
use location::verify_rec_location;
use model::*;
pub(crate) use obligations::rejection_obligation;
use oracle_exclusions::oracle_exclusions;

const SCHEMA_VERSION: u32 = 1;

pub(super) fn write_report(
    path: &Path,
    corpus: &Corpus,
    oracle: &VerifyOracle,
    paths: &[PathBuf],
    no_cfg_norm: bool,
    max_violations: Option<usize>,
) -> Result<()> {
    std::fs::write(
        path,
        serde_json::to_string_pretty(&build_report(
            corpus,
            oracle,
            paths,
            no_cfg_norm,
            max_violations,
        ))?,
    )
    .with_context(|| format!("writing recall-loss report {}", path.display()))
}

fn build_report(
    corpus: &Corpus,
    oracle: &VerifyOracle,
    paths: &[PathBuf],
    no_cfg_norm: bool,
    max_violations: Option<usize>,
) -> RecallLossReport {
    let soundness = soundness_gate(&oracle.recs, oracle.canon_violations.len(), max_violations);
    let (completeness, under_merges) = completeness_report(&oracle.recs);
    let admission_rejections = admission_rejections(&oracle.recs);
    let by_reason = reason_rollups(&admission_rejections);
    let by_obligation = obligation_rollups(&admission_rejections);
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
        import_snapshot_census: nose_frontend::imported_immutable_snapshot_census(corpus),
        admission_rejections,
        by_reason,
        by_obligation,
        top_opportunities,
    }
}

fn soundness_gate(
    recs: &[VerifyRec],
    canon_preservation_violations: usize,
    max_violations: Option<usize>,
) -> SoundnessGate {
    let soundness = count_verify_soundness(recs);
    let false_merges = soundness.false_merges;

    SoundnessGate {
        fingerprint_groups: soundness.fingerprint_groups,
        false_merges,
        lossy_fingerprint_collisions: soundness.lossy_fingerprint_collisions,
        advisory_disagreements: soundness.advisory_disagreements,
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
            let vj = multiset_jaccard(&reps[i].fp, &reps[j].fp);
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
        a: verify_rec_location(a),
        b: verify_rec_location(b),
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
    rec.admission_rejection.as_ref().map(|reason| {
        let (obligation_family, obligation_subreason) =
            rejection_obligation(reason.reason, &reason.missing_evidence);
        AdmissionRejection {
            reason: reason.reason,
            admission_gate: reason.admission_gate,
            capability_id: reason.capability_id,
            pack_id: reason.pack_id,
            missing_evidence: reason.missing_evidence.clone(),
            obligation_family,
            obligation_subreason,
            oracle_status: "interpretable",
            loc: verify_rec_location(rec),
            value_fingerprint_len: rec.fp.len(),
        }
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

fn obligation_rollups(rejections: &[AdmissionRejection]) -> Vec<ObligationRollup> {
    let mut by_key: HashMap<(&str, &str), usize> = HashMap::new();
    for rejection in rejections {
        *by_key
            .entry((rejection.obligation_family, rejection.obligation_subreason))
            .or_default() += 1;
    }
    let mut rollups: Vec<_> = by_key
        .into_iter()
        .map(
            |((obligation_family, obligation_subreason), count)| ObligationRollup {
                obligation_family: obligation_family.to_string(),
                obligation_subreason: obligation_subreason.to_string(),
                count,
                oracle_interpretable: count,
            },
        )
        .collect();
    rollups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.obligation_family.cmp(&b.obligation_family))
            .then(a.obligation_subreason.cmp(&b.obligation_subreason))
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
