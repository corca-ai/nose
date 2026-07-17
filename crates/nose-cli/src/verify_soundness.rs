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

fn same_domain_contract(left: &VerifyRec, right: &VerifyRec) -> bool {
    crate::falsify::effective_domain_contract(&left.param_domains, &left.input_projections)
        == crate::falsify::effective_domain_contract(&right.param_domains, &right.input_projections)
}

fn same_observed_behavior(left: &VerifyRec, right: &VerifyRec) -> bool {
    left.beh == right.beh && left.fragment_exits == right.fragment_exits
}

fn behavior_is_symbolic(rec: &VerifyRec) -> bool {
    rec.beh.iter().any(nose_normalize::behavior_has_sym)
}

fn domains_are_hosted(rec: &VerifyRec) -> bool {
    crate::falsify::domains_are_hosted_with_projections(
        rec.lang,
        &rec.param_domains,
        &rec.input_projections,
    )
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
        for domain_members in exact_domain_partitions(&members) {
            let mut behavior_groups: Vec<Vec<&VerifyRec>> = Vec::new();
            for rec in domain_members.into_iter().filter(|rec| {
                rec.claimable && domains_are_hosted(rec) && !behavior_is_symbolic(rec)
            }) {
                match behavior_groups
                    .iter_mut()
                    .find(|group| same_observed_behavior(group[0], rec))
                {
                    Some(group) => group.push(rec),
                    None => behavior_groups.push(vec![rec]),
                }
            }
            for group in behavior_groups {
                let Some((first, others)) = group.split_first() else {
                    continue;
                };
                for &other in others {
                    pairs.push(VerifyRecPair { first, other });
                }
            }
        }
    }
    pairs.sort_by(|a, b| (&a.first.loc, &a.other.loc).cmp(&(&b.first.loc, &b.other.loc)));
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
        for domain_members in exact_domain_partitions(&members) {
            let mut behavior_groups = behavior_partitions(&domain_members);
            behavior_groups.sort_by_key(|group| representative_rank(group));
            let Some((first_group, other_groups)) = behavior_groups.split_first() else {
                continue;
            };
            let first = preferred_representative(first_group);
            for group in other_groups {
                let rec = preferred_representative(group);
                visit(soundness_lane(first, rec), first, rec);
            }
        }
        // Preserve cross-domain disagreements as advisory diagnostics without letting the first
        // domain mask compatible hard comparisons inside later partitions.
        let first = members[0];
        for rec in &members[1..] {
            if !same_domain_contract(rec, first) && !same_observed_behavior(rec, first) {
                visit(DisagreementLane::Advisory, first, rec);
            }
        }
    }
    group_count
}

fn soundness_lane(first: &VerifyRec, rec: &VerifyRec) -> DisagreementLane {
    if behavior_is_symbolic(first)
        || behavior_is_symbolic(rec)
        || !same_domain_contract(first, rec)
        || !domains_are_hosted(first)
        || !domains_are_hosted(rec)
    {
        DisagreementLane::Advisory
    } else if first.claimable && rec.claimable {
        DisagreementLane::FalseMerge
    } else {
        DisagreementLane::LossyFingerprintCollision
    }
}

fn differing_behavior_inputs(first: &VerifyRec, rec: &VerifyRec) -> usize {
    let behavior_differences = rec
        .beh
        .iter()
        .zip(&first.beh)
        .filter(|(a, b)| a != b)
        .count();
    let exit_differences = match (&rec.fragment_exits, &first.fragment_exits) {
        (Some(left), Some(right)) => left.iter().zip(right).filter(|(a, b)| a != b).count(),
        (None, None) => 0,
        _ => rec.beh.len().max(first.beh.len()),
    };
    behavior_differences.max(exit_differences)
}

fn fingerprint_groups(recs: &[VerifyRec]) -> Vec<Vec<&VerifyRec>> {
    let mut by_fp: HashMap<&[u64], Vec<&VerifyRec>> = HashMap::new();
    for rec in recs {
        by_fp.entry(&rec.fp).or_default().push(rec);
    }
    by_fp.into_values().collect()
}

fn exact_domain_partitions<'a>(members: &[&'a VerifyRec]) -> Vec<Vec<&'a VerifyRec>> {
    let mut partitions: Vec<Vec<&VerifyRec>> = Vec::new();
    for &rec in members {
        match partitions
            .iter_mut()
            .find(|partition| same_domain_contract(partition[0], rec))
        {
            Some(partition) => partition.push(rec),
            None => partitions.push(vec![rec]),
        }
    }
    partitions
}

fn behavior_partitions<'a>(members: &[&'a VerifyRec]) -> Vec<Vec<&'a VerifyRec>> {
    let mut partitions: Vec<Vec<&VerifyRec>> = Vec::new();
    for &rec in members {
        match partitions
            .iter_mut()
            .find(|partition| same_observed_behavior(partition[0], rec))
        {
            Some(partition) => partition.push(rec),
            None => partitions.push(vec![rec]),
        }
    }
    partitions
}

fn representative_rank(group: &[&VerifyRec]) -> u8 {
    if group.iter().any(|rec| hard_eligible(rec)) {
        0
    } else if group.iter().any(|rec| concrete_hosted(rec)) {
        1
    } else {
        2
    }
}

fn preferred_representative<'a>(group: &[&'a VerifyRec]) -> &'a VerifyRec {
    group
        .iter()
        .copied()
        .find(|rec| hard_eligible(rec))
        .or_else(|| group.iter().copied().find(|rec| concrete_hosted(rec)))
        .unwrap_or(group[0])
}

fn hard_eligible(rec: &VerifyRec) -> bool {
    rec.claimable && concrete_hosted(rec)
}

fn concrete_hosted(rec: &VerifyRec) -> bool {
    domains_are_hosted(rec) && !behavior_is_symbolic(rec)
}

fn sort_disagreements(disagreements: &mut [SoundnessDisagreement]) {
    disagreements
        .sort_by(|a, b| (&a.a, &a.b, a.differing_inputs).cmp(&(&b.a, &b.b, b.differing_inputs)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify_collect::VerifyRec;
    use nose_il::{DomainEvidence, Lang, NodeId};
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
        param_domains: &[Option<DomainEvidence>],
    ) -> VerifyRec {
        VerifyRec {
            lang: Lang::Python,
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
            param_domains: param_domains.to_vec(),
            input_projections: vec![
                nose_detect::OracleInputProjection::Declared;
                param_domains.len()
            ],
            file_idx: 0,
            core_root: NodeId(0),
            core_fragment: None,
            fragment_exits: None,
        }
    }

    #[test]
    fn classifies_fingerprint_disagreements_by_gate_lane() {
        let recs = vec![
            rec("hard-a", &[1], behavior(Value::Int(1)), true, 7, &[]),
            rec("hard-b", &[1], behavior(Value::Int(2)), true, 7, &[]),
            rec("lossy-a", &[2], behavior(Value::Int(1)), false, 7, &[]),
            rec("lossy-b", &[2], behavior(Value::Int(2)), true, 7, &[]),
            rec("advisory-a", &[3], behavior(Value::Sym(1)), true, 7, &[]),
            rec("advisory-b", &[3], behavior(Value::Sym(2)), true, 7, &[]),
            // Same compact signature on purpose: exact domains, not hash inequality, must
            // keep this disagreement out of the hard lane.
            rec(
                "domain-a",
                &[4],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::Integer)],
            ),
            rec(
                "domain-b",
                &[4],
                behavior(Value::Int(2)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
            rec("equal-a", &[5], behavior(Value::Int(1)), true, 7, &[]),
            rec("equal-b", &[5], behavior(Value::Int(1)), true, 7, &[]),
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
    fn unread_trailing_parameters_do_not_change_the_effective_domain_contract() {
        let short = rec(
            "short",
            &[1],
            behavior(Value::Int(1)),
            true,
            7,
            &[Some(DomainEvidence::String)],
        );
        let mut long = rec(
            "long",
            &[1],
            behavior(Value::Int(1)),
            true,
            7,
            &[Some(DomainEvidence::String), Some(DomainEvidence::Integer)],
        );
        long.input_projections[1] = nose_detect::OracleInputProjection::UnusedTrailing;
        assert!(same_domain_contract(&short, &long));

        long.input_projections = vec![
            nose_detect::OracleInputProjection::UnusedTrailing,
            nose_detect::OracleInputProjection::Declared,
        ];
        assert!(!same_domain_contract(&short, &long));
        assert!(!domains_are_hosted(&long));
    }

    #[test]
    fn hard_gate_equal_representative_pairs_feed_falsification_search() {
        let recs = vec![
            rec("hard-a", &[1], behavior(Value::Int(1)), true, 7, &[]),
            rec("hard-b", &[1], behavior(Value::Int(1)), true, 7, &[]),
            rec("hard-c", &[1], behavior(Value::Int(1)), true, 7, &[]),
            rec("changed", &[2], behavior(Value::Int(1)), true, 7, &[]),
            rec("changed-other", &[2], behavior(Value::Int(2)), true, 7, &[]),
            rec("lossy-a", &[3], behavior(Value::Int(1)), false, 7, &[]),
            rec("lossy-b", &[3], behavior(Value::Int(1)), true, 7, &[]),
            rec(
                "domain-a",
                &[4],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::Integer)],
            ),
            rec(
                "domain-b",
                &[4],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
            rec("symbolic-a", &[5], behavior(Value::Sym(1)), true, 7, &[]),
            rec("symbolic-b", &[5], behavior(Value::Sym(1)), true, 7, &[]),
        ];

        let pairs = hard_gate_equal_behavior_representative_pairs(&recs);

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].first.loc, "hard-a");
        assert_eq!(pairs[0].other.loc, "hard-b");
        assert_eq!(pairs[1].first.loc, "hard-a");
        assert_eq!(pairs[1].other.loc, "hard-c");
    }

    #[test]
    fn compatible_domain_subgroups_are_not_masked_by_the_first_member() {
        let recs = vec![
            rec(
                "integer",
                &[1],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::Integer)],
            ),
            rec(
                "string-a",
                &[1],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
            rec(
                "string-b",
                &[1],
                behavior(Value::Int(2)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
        ];

        let summary = classify_verify_soundness(&recs);

        assert_eq!(summary.false_merges.len(), 1);
        assert_eq!(summary.false_merges[0].a, "string-a");
        assert_eq!(summary.false_merges[0].b, "string-b");
        assert_eq!(summary.advisory_disagreements.len(), 1);
    }

    #[test]
    fn symbolic_member_does_not_mask_a_concrete_hard_subgroup() {
        let recs = vec![
            rec("symbolic", &[1], behavior(Value::Sym(1)), true, 7, &[]),
            rec("concrete-a", &[1], behavior(Value::Int(1)), true, 7, &[]),
            rec("concrete-b", &[1], behavior(Value::Int(2)), true, 7, &[]),
        ];

        let summary = classify_verify_soundness(&recs);

        assert_eq!(summary.false_merges.len(), 1);
        assert_eq!(summary.false_merges[0].a, "concrete-a");
        assert_eq!(summary.false_merges[0].b, "concrete-b");
        assert_eq!(summary.advisory_disagreements.len(), 1);
    }

    #[test]
    fn equal_behavior_subgroups_all_feed_falsification() {
        let recs = vec![
            rec(
                "integer",
                &[1],
                behavior(Value::Int(1)),
                true,
                7,
                &[Some(DomainEvidence::Integer)],
            ),
            rec(
                "string-a",
                &[1],
                behavior(Value::Int(2)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
            rec(
                "string-b",
                &[1],
                behavior(Value::Int(2)),
                true,
                7,
                &[Some(DomainEvidence::String)],
            ),
            rec("symbolic", &[2], behavior(Value::Sym(1)), true, 7, &[]),
            rec("concrete-a", &[2], behavior(Value::Int(3)), true, 7, &[]),
            rec("concrete-b", &[2], behavior(Value::Int(3)), true, 7, &[]),
        ];

        let pairs = hard_gate_equal_behavior_representative_pairs(&recs);
        let locations: Vec<_> = pairs
            .iter()
            .map(|pair| (pair.first.loc.as_str(), pair.other.loc.as_str()))
            .collect();

        assert_eq!(
            locations,
            vec![("concrete-a", "concrete-b"), ("string-a", "string-b")]
        );
    }

    #[test]
    fn missing_static_or_unhosted_domains_fail_closed() {
        let mut rust_a = rec("rust-a", &[1], behavior(Value::Int(1)), true, 7, &[None]);
        rust_a.lang = Lang::Rust;
        let mut rust_b = rec("rust-b", &[1], behavior(Value::Int(2)), true, 7, &[None]);
        rust_b.lang = Lang::Rust;
        let map_a = rec(
            "map-a",
            &[2],
            behavior(Value::Int(1)),
            true,
            7,
            &[Some(DomainEvidence::Map)],
        );
        let map_b = rec(
            "map-b",
            &[2],
            behavior(Value::Int(2)),
            true,
            7,
            &[Some(DomainEvidence::Map)],
        );

        let recs = [rust_a, rust_b, map_a, map_b];
        let summary = classify_verify_soundness(&recs);

        assert!(summary.false_merges.is_empty());
        assert_eq!(summary.advisory_disagreements.len(), 2);
        assert!(hard_gate_equal_behavior_representative_pairs(&recs).is_empty());
    }
}
