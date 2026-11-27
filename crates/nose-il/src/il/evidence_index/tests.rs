use super::*;
use crate::DomainEvidence;
use std::collections::{BTreeSet, VecDeque};

fn record(id: u32, dependencies: &[u32]) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        EvidenceAnchor::binding(Span::new(FileId(0), id, id + 1, 1, 1), id as u64),
        EvidenceKind::Domain(DomainEvidence::Collection),
        "test",
        "dependency-resolution",
        dependencies.iter().copied().map(EvidenceId).collect(),
        EvidenceStatus::Asserted,
    )
}

fn reference_walk(evidence: &[EvidenceRecord], dependencies: &[EvidenceId]) -> bool {
    let mut queue = VecDeque::from(dependencies.to_vec());
    let mut seen = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.0) {
            continue;
        }
        let selected = evidence
            .iter()
            .enumerate()
            .filter(|(_, record)| record.id == id)
            .min_by_key(|(position, _)| (usize::from(*position != id.0 as usize), *position));
        let Some((_, record)) = selected else {
            return false;
        };
        if record.status != EvidenceStatus::Asserted {
            return false;
        }
        queue.extend(record.dependencies.iter().copied());
    }
    true
}

#[test]
fn dependency_index_matches_linear_resolution_with_sparse_duplicate_and_cyclic_ids() {
    for ids in [
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        vec![9, 1, 1, 9, 4, 1, 6, 6],
        vec![7, 6, 5, 4, 3, 2, 1, 0],
    ] {
        let mut evidence = ids
            .iter()
            .enumerate()
            .map(|(position, &id)| record(id, &[ids[(position + 1) % ids.len()]]))
            .collect::<Vec<_>>();
        let mut index = EvidenceIndex::default();
        index.extend_from(&evidence);
        // Statuses are live: changing one must not require rebuilding the index.
        for ambiguous in 0..=evidence.len() {
            for (position, record) in evidence.iter_mut().enumerate() {
                record.status = if position == ambiguous {
                    EvidenceStatus::Ambiguous
                } else {
                    EvidenceStatus::Asserted
                };
            }
            for id in 0..12 {
                let dependencies = [EvidenceId(id)];
                assert_eq!(
                    index.deps_walk(&evidence, &dependencies),
                    reference_walk(&evidence, &dependencies),
                    "ids={ids:?}, ambiguous={ambiguous}, dependency={id}"
                );
            }
        }
    }
}

#[test]
fn appended_positional_record_takes_precedence_over_earlier_duplicate() {
    let mut evidence = vec![record(2, &[]), record(7, &[])];
    evidence[0].status = EvidenceStatus::Ambiguous;
    let mut index = EvidenceIndex::default();
    index.extend_from(&evidence);
    assert!(!index.deps_walk(&evidence, &[EvidenceId(2)]));
    evidence.push(record(2, &[7]));
    index.extend_from(&evidence);
    assert!(index.deps_walk(&evidence, &[EvidenceId(2)]));
    evidence[1].dependencies.push(EvidenceId(99));
    assert!(!index.deps_walk(&evidence, &[EvidenceId(2)]));
}
