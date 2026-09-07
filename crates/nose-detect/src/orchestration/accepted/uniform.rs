//! Preserve an exact uniform source relation before mapping it onto report sites.
use super::{AcceptedPairs, RowPairs};
use crate::{candidates::round3, Group, WitnessEvidence};

impl AcceptedPairs {
    pub(crate) fn uniform_groups(&self, raw: &[Vec<usize>], groups: &[Group]) -> Vec<Option<f64>> {
        let Self::Rows(rows) = self else {
            return vec![None; raw.len()];
        };
        let mut counts = vec![0; rows.targets.len()];
        for &row in &rows.row_of {
            counts[row] += 1;
        }
        let scores = counts
            .into_iter()
            .enumerate()
            .map(|(row, count)| uniform_score(rows, row, count))
            .collect::<Vec<_>>();
        raw.iter()
            .zip(groups)
            .map(|(members, group)| {
                if !group.witness.as_ref().is_some_and(|witness| {
                    matches!(&witness.evidence, WitnessEvidence::ExactValueGraph { .. })
                }) {
                    return None;
                }
                let row = rows.row_of[*members.first()?];
                members
                    .iter()
                    .all(|&unit| rows.row_of[unit] == row)
                    .then_some(scores[row])
                    .flatten()
            })
            .collect()
    }
}

fn uniform_score(rows: &RowPairs, row: usize, count: usize) -> Option<f64> {
    let targets = &rows.targets[row];
    let score = targets.first()?.1;
    (targets.len() == count
        && round3(score).is_finite()
        && targets
            .iter()
            .all(|&(unit, value)| rows.row_of[unit] == row && value.to_bits() == score.to_bits()))
    .then_some(round3(score))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_group_proof_rejects_mixed_rows_nonfinite_scores_and_ineligible_values() {
        let mut units = crate::test_support::scoring_units(24);
        let paths = (0..units.len()).map(|index| index % 3).collect::<Vec<_>>();
        for (index, unit) in units.iter_mut().enumerate() {
            unit.path = format!("{}.py", paths[index]);
            unit.start_line = index as u32;
            unit.end_line = unit.start_line + 3;
        }
        let members = vec![(0..12).collect::<Vec<_>>(), (12..24).collect::<Vec<_>>()];
        let opts = crate::DetectOptions::default();
        for eligible in [true, false] {
            for unit in &mut units {
                unit.exact_safe = eligible;
            }
            for score in [-0.0_f64, 0.0, 0.875, f64::MAX, f64::INFINITY, f64::NAN] {
                for mixed in [false, true] {
                    let relations = if mixed {
                        vec![(0, 1, score), (1, 0, score)]
                    } else {
                        vec![(0, 0, score), (1, 1, score)]
                    };
                    let pairs = AcceptedPairs::rows(&units, &paths, &members, relations);
                    let (groups, _) = crate::candidates::build_groups(
                        &units,
                        &pairs,
                        &members,
                        &vec![None; units.len()],
                        &opts,
                        false,
                    );
                    let actual = pairs.uniform_groups(&members, &groups);
                    let expected = (eligible && !mixed && round3(score).is_finite())
                        .then_some(round3(score).to_bits());
                    assert_eq!(
                        actual
                            .into_iter()
                            .map(|score| score.map(f64::to_bits))
                            .collect::<Vec<_>>(),
                        vec![expected; 2]
                    );
                    let explicit = AcceptedPairs::Explicit(pairs.iter().collect());
                    assert_eq!(explicit.uniform_groups(&members, &groups), vec![None; 2]);
                }
            }
        }
    }
}
