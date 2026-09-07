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

pub(super) fn rows(
    members: &[Vec<usize>],
    relations: Vec<AcceptedPair>,
    paths: &[usize],
) -> PreparedRows {
    let mut grouped = vec![Vec::new(); members.len()];
    let mut work = 0usize;
    for (left, right, score) in relations {
        work = work.saturating_add(members[right].len());
        grouped[left].push((right, score));
    }
    let prepare = |relations: Vec<(usize, f64)>| {
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
        let mut by_path: ByPath = FxHashMap::default();
        for (position, &(right, _)) in targets.iter().enumerate() {
            by_path.entry(paths[right]).or_default().push(position);
        }
        (targets, by_path)
    };
    let (targets, by_path) = if work < 16_384 {
        grouped.into_iter().map(prepare).unzip()
    } else {
        grouped.into_par_iter().map(prepare).unzip()
    };
    PreparedRows { targets, by_path }
}
