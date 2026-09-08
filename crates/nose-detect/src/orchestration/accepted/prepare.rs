//! Prepare independent accepted target rows without expanding every source pair.
use super::AcceptedPair;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

type Targets = Vec<(usize, f64)>;
type ByPath = FxHashMap<usize, Vec<usize>>;

pub(super) struct PreparedRows {
    pub targets: Vec<Targets>,
    pub by_path: Vec<ByPath>,
}

pub(super) fn group(rows: usize, relations: Vec<AcceptedPair>) -> Vec<Vec<(usize, f64)>> {
    let mut grouped = vec![Vec::new(); rows];
    for (left, right, score) in relations {
        grouped[left].push((right, score));
    }
    grouped
}

pub(super) fn rows(
    members: &[Vec<usize>],
    grouped: Vec<Vec<(usize, f64)>>,
    paths: &[usize],
) -> PreparedRows {
    debug_assert_eq!(members.len(), grouped.len());
    let mut work = 0usize;
    for &(right, _) in grouped.iter().flatten() {
        work = work.saturating_add(members[right].len());
        if work >= 16_384 {
            break;
        }
    }
    let prepare = |(left, relations): (usize, Vec<(usize, f64)>)| {
        let capacity = relations
            .iter()
            .map(|&(right, _)| members[right].len())
            .sum();
        let mut targets = Vec::with_capacity(capacity);
        for (right, score) in relations {
            targets.extend(members[right].iter().map(|&unit| (unit, score)));
        }
        targets.sort_unstable_by_key(|&(unit, _)| unit);
        debug_assert!(targets.windows(2).all(|pair| pair[0].0 != pair[1].0));
        // Only files containing a left endpoint can have a same-file exclusion.
        let mut by_path: ByPath = members[left]
            .iter()
            .map(|&unit| (paths[unit], Vec::new()))
            .collect();
        for (position, &(right, _)) in targets.iter().enumerate() {
            if let Some(positions) = by_path.get_mut(&paths[right]) {
                positions.push(position);
            }
        }
        (targets, by_path)
    };
    let (targets, by_path) = if work < 16_384 {
        grouped.into_iter().enumerate().map(prepare).unzip()
    } else {
        grouped.into_par_iter().enumerate().map(prepare).unzip()
    };
    PreparedRows { targets, by_path }
}
