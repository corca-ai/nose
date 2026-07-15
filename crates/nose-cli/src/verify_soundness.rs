use crate::verify_collect::VerifyRec;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SoundnessDisagreement {
    pub(super) a: String,
    pub(super) b: String,
    pub(super) differing_inputs: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct VerifySoundness {
    pub(super) fingerprint_groups: usize,
    pub(super) false_merges: Vec<SoundnessDisagreement>,
    pub(super) lossy_fingerprint_collisions: Vec<SoundnessDisagreement>,
    pub(super) advisory_disagreements: Vec<SoundnessDisagreement>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct VerifySoundnessCounts {
    pub(super) fingerprint_groups: usize,
    pub(super) false_merges: usize,
    pub(super) lossy_fingerprint_collisions: usize,
    pub(super) advisory_disagreements: usize,
}

pub(super) struct VerifyRecPair<'a> {
    pub(super) first: &'a VerifyRec,
    pub(super) other: &'a VerifyRec,
}

#[derive(Copy, Clone)]
enum DisagreementLane {
    FalseMerge,
    LossyFingerprintCollision,
    Advisory,
}

pub(super) fn classify_verify_soundness(recs: &[VerifyRec]) -> VerifySoundness {
    let mut summary = VerifySoundness::default();
    summary.fingerprint_groups = visit_soundness_disagreements(recs, |lane, first, rec| {
        let disagreement = SoundnessDisagreement {
            a: first.loc.clone(),
            b: rec.loc.clone(),
            differing_inputs: differing_behavior_inputs(first, rec),
        };
        match lane {
            DisagreementLane::FalseMerge => summary.false_merges.push(disagreement),
            DisagreementLane::LossyFingerprintCollision => {
                summary.lossy_fingerprint_collisions.push(disagreement);
            }
            DisagreementLane::Advisory => summary.advisory_disagreements.push(disagreement),
        }
    });
    sort_disagreements(&mut summary.false_merges);
    sort_disagreements(&mut summary.lossy_fingerprint_collisions);
    sort_disagreements(&mut summary.advisory_disagreements);
    summary
}

pub(super) fn count_verify_soundness(recs: &[VerifyRec]) -> VerifySoundnessCounts {
    let mut counts = VerifySoundnessCounts::default();
    counts.fingerprint_groups = visit_soundness_disagreements(recs, |lane, _, _| match lane {
        DisagreementLane::FalseMerge => counts.false_merges += 1,
        DisagreementLane::LossyFingerprintCollision => counts.lossy_fingerprint_collisions += 1,
        DisagreementLane::Advisory => counts.advisory_disagreements += 1,
    });
    counts
}

pub(super) fn hard_gate_equal_behavior_representative_pairs(
    recs: &[VerifyRec],
) -> Vec<VerifyRecPair<'_>> {
    let mut pairs = Vec::new();
    for members in fingerprint_groups(recs) {
        if members.len() < 2 {
            continue;
        }
        let first = members[0];
        for other in &members[1..] {
            if other.beh == first.beh
                && first.claimable
                && other.claimable
                && first.domain_sig == other.domain_sig
            {
                pairs.push(VerifyRecPair { first, other });
            }
        }
    }
    pairs
}

fn visit_soundness_disagreements(
    recs: &[VerifyRec],
    mut visit: impl FnMut(DisagreementLane, &VerifyRec, &VerifyRec),
) -> usize {
    let mut group_count = 0usize;
    for members in fingerprint_groups(recs) {
        if members.len() < 2 {
            continue;
        }
        group_count += 1;
        let first = members[0];
        for rec in &members[1..] {
            if rec.beh != first.beh {
                visit(soundness_lane(first, rec), first, rec);
            }
        }
    }
    group_count
}

fn soundness_lane(first: &VerifyRec, rec: &VerifyRec) -> DisagreementLane {
    if first.beh.iter().any(nose_normalize::behavior_has_sym)
        || rec.beh.iter().any(nose_normalize::behavior_has_sym)
        || first.domain_sig != rec.domain_sig
    {
        DisagreementLane::Advisory
    } else if first.claimable && rec.claimable {
        DisagreementLane::FalseMerge
    } else {
        DisagreementLane::LossyFingerprintCollision
    }
}

fn differing_behavior_inputs(first: &VerifyRec, rec: &VerifyRec) -> usize {
    rec.beh
        .iter()
        .zip(&first.beh)
        .filter(|(a, b)| a != b)
        .count()
}

fn fingerprint_groups(recs: &[VerifyRec]) -> Vec<Vec<&VerifyRec>> {
    let mut by_fp: HashMap<&[u64], Vec<&VerifyRec>> = HashMap::new();
    for rec in recs {
        by_fp.entry(&rec.fp).or_default().push(rec);
    }
    by_fp.into_values().collect()
}

fn sort_disagreements(disagreements: &mut [SoundnessDisagreement]) {
    disagreements
        .sort_by(|a, b| (&a.a, &a.b, a.differing_inputs).cmp(&(&b.a, &b.b, b.differing_inputs)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify_collect::VerifyRec;
    use nose_il::NodeId;
    use nose_normalize::{Behavior, Value};

    fn behavior(value: Value) -> Vec<Behavior> {
        vec![Behavior {
            ret: value,
            effects: Vec::new(),
            fields: Vec::new(),
        }]
    }

    fn rec(
        loc: &str,
        fp: &[u64],
        beh: Vec<Behavior>,
        claimable: bool,
        domain_sig: u64,
    ) -> VerifyRec {
        VerifyRec {
            fp: fp.to_vec(),
            beh,
            file: format!("{loc}.rs"),
            start: 1,
            end: 1,
            tokens: 1,
            loc: loc.to_string(),
            claimable,
            product_admission: "admitted",
            canon_exposed: false,
            admission_rejection: None,
            domain_sig,
            file_idx: 0,
            core_root: NodeId(0),
        }
    }

    #[test]
    fn classifies_fingerprint_disagreements_by_gate_lane() {
        let recs = vec![
            rec("hard-a", &[1], behavior(Value::Int(1)), true, 7),
            rec("hard-b", &[1], behavior(Value::Int(2)), true, 7),
            rec("lossy-a", &[2], behavior(Value::Int(1)), false, 7),
            rec("lossy-b", &[2], behavior(Value::Int(2)), true, 7),
            rec("advisory-a", &[3], behavior(Value::Sym(1)), true, 7),
            rec("advisory-b", &[3], behavior(Value::Sym(2)), true, 7),
            rec("domain-a", &[4], behavior(Value::Int(1)), true, 7),
            rec("domain-b", &[4], behavior(Value::Int(2)), true, 8),
            rec("equal-a", &[5], behavior(Value::Int(1)), true, 7),
            rec("equal-b", &[5], behavior(Value::Int(1)), true, 7),
        ];

        let summary = classify_verify_soundness(&recs);
        let counts = count_verify_soundness(&recs);

        assert_eq!(summary.fingerprint_groups, 5);
        assert_eq!(summary.false_merges.len(), 1);
        assert_eq!(summary.lossy_fingerprint_collisions.len(), 1);
        assert_eq!(summary.advisory_disagreements.len(), 2);
        assert_eq!(counts.fingerprint_groups, summary.fingerprint_groups);
        assert_eq!(counts.false_merges, summary.false_merges.len());
        assert_eq!(
            counts.lossy_fingerprint_collisions,
            summary.lossy_fingerprint_collisions.len()
        );
        assert_eq!(
            counts.advisory_disagreements,
            summary.advisory_disagreements.len()
        );
    }

    #[test]
    fn hard_gate_equal_representative_pairs_feed_falsification_search() {
        let recs = vec![
            rec("hard-a", &[1], behavior(Value::Int(1)), true, 7),
            rec("hard-b", &[1], behavior(Value::Int(1)), true, 7),
            rec("hard-c", &[1], behavior(Value::Int(1)), true, 7),
            rec("changed", &[2], behavior(Value::Int(1)), true, 7),
            rec("changed-other", &[2], behavior(Value::Int(2)), true, 7),
            rec("lossy-a", &[3], behavior(Value::Int(1)), false, 7),
            rec("lossy-b", &[3], behavior(Value::Int(1)), true, 7),
            rec("domain-a", &[4], behavior(Value::Int(1)), true, 7),
            rec("domain-b", &[4], behavior(Value::Int(1)), true, 8),
        ];

        let pairs = hard_gate_equal_behavior_representative_pairs(&recs);

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].first.loc, "hard-a");
        assert_eq!(pairs[0].other.loc, "hard-b");
        assert_eq!(pairs[1].first.loc, "hard-a");
        assert_eq!(pairs[1].other.loc, "hard-c");
    }
}
