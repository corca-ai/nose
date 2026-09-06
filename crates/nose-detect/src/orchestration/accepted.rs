//! Accepted relations retain their complete source-pair order without expanding
//! repeated left-hand rows into an array of every accepted edge.
use super::AcceptedPair;
use crate::UnitFeat;
use rustc_hash::FxHashMap;

mod score_runs;
use score_runs::ScoreRuns;

#[derive(Clone, Debug)]
pub(crate) enum AcceptedPairs {
    Explicit(Vec<AcceptedPair>),
    Rows(std::sync::Arc<RowPairs>),
}

#[derive(Debug)]
pub(crate) struct RowPairs {
    row_of: Vec<usize>,
    targets: Vec<Vec<(usize, f64)>>,
    locations: Vec<(usize, u32, u32)>,
    by_path: Vec<FxHashMap<usize, Vec<usize>>>,
    count: usize,
}

impl Default for AcceptedPairs {
    fn default() -> Self {
        Self::Explicit(Vec::new())
    }
}

impl From<Vec<AcceptedPair>> for AcceptedPairs {
    fn from(pairs: Vec<AcceptedPair>) -> Self {
        Self::Explicit(pairs)
    }
}

impl AcceptedPairs {
    pub(crate) fn rows(
        units: &[UnitFeat],
        paths: &[usize],
        members: &[Vec<usize>],
        relations: Vec<AcceptedPair>,
    ) -> Self {
        let mut row_of = vec![0; units.len()];
        for (row, members) in members.iter().enumerate() {
            for &unit in members {
                row_of[unit] = row;
            }
        }
        let mut targets = vec![Vec::new(); members.len()];
        for (left, right, score) in relations {
            targets[left].extend(members[right].iter().map(|&unit| (unit, score)));
        }
        for row in &mut targets {
            row.sort_unstable_by_key(|&(unit, _)| unit);
            debug_assert!(row.windows(2).all(|pair| pair[0].0 != pair[1].0));
        }
        let locations = units
            .iter()
            .zip(paths)
            .map(|(unit, &path)| (path, unit.start_line, unit.end_line))
            .collect();
        let by_path = targets
            .iter()
            .map(|targets| {
                let mut by_path: FxHashMap<_, Vec<_>> = FxHashMap::default();
                for (position, &(right, _)) in targets.iter().enumerate() {
                    by_path.entry(paths[right]).or_default().push(position);
                }
                by_path
            })
            .collect();
        let mut rows = RowPairs {
            row_of,
            targets,
            locations,
            by_path,
            count: 0,
        };
        rows.count = (0..units.len()).map(|left| rows.count_after(left)).sum();
        Self::Rows(std::sync::Arc::new(rows))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = AcceptedPair> + Clone + '_ {
        let (mut left, mut offset) = (0, 0);
        std::iter::from_fn(move || match self {
            Self::Explicit(pairs) => {
                let pair = pairs.get(offset).copied();
                offset += usize::from(pair.is_some());
                pair
            }
            Self::Rows(rows) => {
                while left < rows.row_of.len() {
                    let targets = &rows.targets[rows.row_of[left]];
                    if offset == 0 {
                        offset = targets.partition_point(|&(right, _)| right <= left);
                    }
                    while let Some(&(right, score)) = targets.get(offset) {
                        offset += 1;
                        if rows.admits(left, right) {
                            return Some((left, right, score));
                        }
                    }
                    left += 1;
                    offset = 0;
                }
                None
            }
        })
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Explicit(pairs) => pairs.len(),
            Self::Rows(rows) => rows.count,
        }
    }

    pub(crate) fn group_scores(
        &self,
        member_group: &[Option<usize>],
        groups: usize,
    ) -> Vec<(f64, usize)> {
        let mut scores = vec![(0.0, 0); groups];
        if let Self::Rows(rows) = self {
            let unit_scores = rows
                .targets
                .iter()
                .map(|targets| targets.iter().all(|&(_, score)| score == 1.0))
                .collect::<Vec<_>>();
            let mut repetitions = vec![0usize; rows.targets.len()];
            for &row in &rows.row_of {
                repetitions[row] += 1;
            }
            let runs = rows
                .targets
                .iter()
                .enumerate()
                .map(|(row, targets)| {
                    (repetitions[row] >= 16 && !unit_scores[row])
                        .then(|| ScoreRuns::new(targets))
                        .flatten()
                })
                .collect::<Vec<_>>();
            for (left, &group) in member_group.iter().enumerate() {
                let Some(group) = group else { continue };
                let row = rows.row_of[left];
                if unit_scores[row] && add_unit_scores(&mut scores[group], rows.count_after(left)) {
                    continue;
                }
                let targets = &rows.targets[row];
                let mut start = targets.partition_point(|&(right, _)| right <= left);
                if let Some(positions) = rows.by_path[row].get(&rows.locations[left].0) {
                    for &position in positions {
                        if position >= start && !rows.admits(left, targets[position].0) {
                            accumulate_scores(
                                &mut scores[group],
                                targets,
                                start..position,
                                runs[row].as_ref(),
                            );
                            start = position + 1;
                        }
                    }
                }
                accumulate_scores(
                    &mut scores[group],
                    targets,
                    start..targets.len(),
                    runs[row].as_ref(),
                );
            }
        } else {
            for (left, _, score) in self.iter() {
                if let Some(group) = member_group[left] {
                    scores[group].0 += score;
                    scores[group].1 += 1;
                }
            }
        }
        scores
    }

    pub(crate) fn components(&self, units: usize) -> Vec<Vec<usize>> {
        let mut union = crate::cluster::UnionFind::new(units);
        if let Self::Rows(rows) = self {
            let mut uniform = vec![false; rows.targets.len()];
            for left in 0..units {
                let row = rows.row_of[left];
                let targets = &rows.targets[row];
                let start = targets.partition_point(|&(right, _)| right <= left);
                if uniform[row] {
                    // Every remaining target was already in one component. The first
                    // admitted edge performs the same union; later unions are redundant.
                    if let Some(&(right, _)) = targets[start..]
                        .iter()
                        .find(|&&(right, _)| rows.admits(left, right))
                    {
                        union.union(left, right);
                    }
                } else {
                    for &(right, _) in &targets[start..] {
                        if rows.admits(left, right) {
                            union.union(left, right);
                        }
                    }
                    let root = union.find(left);
                    uniform[row] = targets[start..]
                        .iter()
                        .all(|&(right, _)| union.find(right) == root);
                }
            }
        } else {
            for (left, right, _) in self.iter() {
                union.union(left, right);
            }
        }
        union.groups(units)
    }

    /// Keys identify the same reported site and complete pair-witness inputs.
    /// Earlier members of an identical scoring row cover every later cross-file
    /// target. Within-file exclusions remain specific to each source occurrence.
    pub(crate) fn visit_site_evidence(
        &self,
        keys: &[Option<(usize, u32, usize)>],
        mut visit: impl FnMut(AcceptedPair),
    ) {
        let Self::Rows(rows) = self else {
            self.iter().for_each(visit);
            return;
        };
        let mut seen = rustc_hash::FxHashSet::default();
        let targets = rows.site_targets(keys);
        for (left, &key) in keys.iter().enumerate() {
            let Some(key) = key else {
                continue;
            };
            let row = rows.row_of[left];
            if seen.insert((row, key, rows.locations[left].0)) {
                let targets = &targets[row];
                let start = targets.partition_point(|&(right, _)| right <= left);
                for &(right, score) in &targets[start..] {
                    if rows.locations[left].0 != rows.locations[right].0 {
                        visit((left, right, score));
                    }
                }
            }
            if let Some(positions) = rows.by_path[row].get(&rows.locations[left].0) {
                for &position in positions {
                    let &(right, score) = &rows.targets[row][position];
                    if rows.admits(left, right) {
                        visit((left, right, score));
                    }
                }
            }
        }
    }

    pub(crate) fn extend(&mut self, pairs: impl IntoIterator<Item = AcceptedPair>) {
        match self {
            Self::Explicit(out) => out.extend(pairs),
            Self::Rows(_) => unreachable!("a completed row relation is immutable"),
        }
    }

    /// Retain only queried membership facts, rather than hashing the entire graph.
    pub(crate) fn matching(
        &self,
        queries: impl IntoIterator<Item = (usize, usize)>,
    ) -> rustc_hash::FxHashSet<(usize, usize)> {
        let mut wanted = queries.into_iter().collect::<rustc_hash::FxHashSet<_>>();
        if wanted.is_empty() {
            return wanted;
        }
        if let Self::Rows(rows) = self {
            wanted.retain(|&(left, right)| {
                rows.admits(left, right)
                    && rows.targets[rows.row_of[left]]
                        .binary_search_by_key(&right, |&(unit, _)| unit)
                        .is_ok()
            });
            return wanted;
        }
        let mut found = rustc_hash::FxHashSet::default();
        for (left, right, _) in self.iter() {
            if wanted.remove(&(left, right)) {
                found.insert((left, right));
                if wanted.is_empty() {
                    break;
                }
            }
        }
        found
    }
}

// Repeated +1 is exact up to 2^53, then ties-to-even leaves the sum there.
// Nonintegral or larger starting sums retain the ordinary ordered fold.
fn add_unit_scores(total: &mut (f64, usize), count: usize) -> bool {
    const LIMIT: f64 = 9_007_199_254_740_992.0;
    if !(0.0..=LIMIT).contains(&total.0) || total.0.fract() != 0.0 {
        return false;
    }
    if count != 0 {
        total.0 = (total.0 + (count as f64).min(LIMIT)).min(LIMIT);
        total.1 += count;
    }
    true
}

fn accumulate_scores(
    total: &mut (f64, usize),
    targets: &[(usize, f64)],
    range: std::ops::Range<usize>,
    runs: Option<&ScoreRuns>,
) {
    total.0 = runs.map_or_else(
        || {
            targets[range.clone()]
                .iter()
                .fold(total.0, |sum, &(_, score)| sum + score)
        },
        |runs| runs.sum(range.clone(), total.0),
    );
    total.1 += range.len();
}

impl RowPairs {
    fn site_targets(&self, keys: &[Option<(usize, u32, usize)>]) -> Vec<Vec<(usize, f64)>> {
        let mut needed = vec![false; self.targets.len()];
        for (left, key) in keys.iter().enumerate() {
            if key.is_some() {
                needed[self.row_of[left]] = true;
            }
        }
        self.targets
            .iter()
            .enumerate()
            .map(|(row, targets)| {
                if !needed[row] {
                    return Vec::new();
                }
                let mut latest = FxHashMap::default();
                for &(right, score) in targets {
                    if let Some(key) = keys[right] {
                        // The last equivalent occurrence covers every earlier left
                        // endpoint across files. Same-file nesting is handled separately.
                        latest.insert(
                            (key, self.locations[right].0, score.to_bits()),
                            (right, score),
                        );
                    }
                }
                let mut targets = latest.into_values().collect::<Vec<_>>();
                targets.sort_unstable_by_key(|&(right, _)| right);
                targets
            })
            .collect()
    }

    fn count_after(&self, left: usize) -> usize {
        let row = self.row_of[left];
        let targets = &self.targets[row];
        let start = targets.partition_point(|&(right, _)| right <= left);
        let excluded = self.by_path[row]
            .get(&self.locations[left].0)
            .into_iter()
            .flatten()
            .filter(|&&position| position >= start && !self.admits(left, targets[position].0))
            .count();
        targets.len() - start - excluded
    }
    fn admits(&self, left: usize, right: usize) -> bool {
        let (a, start, end) = self.locations[left];
        let (b, other_start, other_end) = self.locations[right];
        left < right
            && (a != b
                || !((start <= other_start && end >= other_end)
                    || (other_start <= start && other_end >= end)))
    }
}

#[cfg(test)]
impl PartialEq<Vec<AcceptedPair>> for AcceptedPairs {
    fn eq(&self, other: &Vec<AcceptedPair>) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, Interner, Lang};

    fn assert_projection_matches(
        rows: &AcceptedPairs,
        explicit: &AcceptedPairs,
        keys: &[Option<(usize, u32, usize)>],
    ) {
        let project = |pairs: Vec<AcceptedPair>| {
            let mut evidence = std::collections::BTreeSet::new();
            for (left, right, score) in pairs {
                let (Some((_, a, ac)), Some((_, b, bc))) = (keys[left], keys[right]) else {
                    continue;
                };
                if a != b {
                    // Comparing all witness-class/score facts is stronger than
                    // comparing only each site's winning projected edge.
                    let ends = if a < b {
                        ((a, ac), (b, bc))
                    } else {
                        ((b, bc), (a, ac))
                    };
                    evidence.insert((ends, score.to_bits()));
                }
            }
            evidence
        };
        let mut projected = Vec::new();
        rows.visit_site_evidence(keys, |pair| projected.push(pair));
        assert_eq!(project(projected), project(explicit.iter().collect()));
    }

    #[test]
    fn unit_score_batches_match_sequential_rounding_at_integer_boundaries() {
        let limit = 9_007_199_254_740_992.0;
        for start in [0.0, -0.0, 1.0, 123.0, limit - 3.0, limit - 1.0, limit] {
            for count in [0, 1, 2, 3, 17, 4096] {
                let expected = (0..count).fold(start, |sum, _| sum + 1.0);
                let mut total = (start, 7);
                assert!(add_unit_scores(&mut total, count));
                assert_eq!(total.0.to_bits(), expected.to_bits());
                assert_eq!(total.1, 7 + count);
            }
        }
        for start in [0.5, -1.0, limit + 2.0, f64::INFINITY, f64::NAN] {
            let mut total = (start, 0);
            assert!(!add_unit_scores(&mut total, 3));
            assert_eq!(total.0.to_bits(), start.to_bits());
            assert_eq!(total.1, 0);
        }
    }

    #[test]
    fn row_relation_preserves_order_scores_exclusions_and_membership() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    return x * x + 1\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = crate::DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            ..Default::default()
        };
        let mut units = Vec::new();
        for i in 0..128 {
            let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
            unit.path = format!("{}.py", i % 7);
            unit.start_line = (i / 14) as u32;
            unit.end_line = unit.start_line + if i % 3 == 0 { 4 } else { 1 };
            units.push(unit);
        }
        let paths = (0..units.len()).map(|i| i % 7).collect::<Vec<_>>();
        let members = (0..4)
            .map(|row| (row..units.len()).step_by(4).collect())
            .collect::<Vec<_>>();
        let score = |left: usize, right: usize| ((left * 3 + right * 7) % 17) as f64 / 17.0;
        let relations = (0..4)
            .flat_map(|a| (0..4).map(move |b| (a, b, score(a, b))))
            .collect();
        let rows = AcceptedPairs::rows(&units, &paths, &members, relations);
        let expected = (0..units.len())
            .flat_map(|left| {
                let units = &units;
                (left + 1..units.len()).filter_map(move |right| {
                    (!crate::locations::is_nested(&units[left], &units[right])).then_some((
                        left,
                        right,
                        score(left % 4, right % 4),
                    ))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), expected.len());
        assert_eq!(rows, expected);
        let queries = (0..128)
            .flat_map(|left| (left..128).step_by(9).map(move |right| (left, right)))
            .collect::<Vec<_>>();
        let explicit = AcceptedPairs::from(expected);
        assert_eq!(
            rows.components(units.len()),
            explicit.components(units.len())
        );
        assert_eq!(rows.matching(queries.clone()), explicit.matching(queries));
        assert!(rows.matching([]).is_empty());
        for variation in 0..4 {
            let keys = (0..units.len())
                .map(|unit| {
                    (variation != 3 || unit % 5 != 0).then_some((
                        0,
                        (unit / 28 * 7 + paths[unit]) as u32,
                        unit % (variation + 1),
                    ))
                })
                .collect::<Vec<_>>();
            assert_projection_matches(&rows, &explicit, &keys);
        }
        for salt in 0..10 {
            let relations = (0..4)
                .flat_map(|a| {
                    (0..4).filter_map(move |b| {
                        ((a * 3 + b + salt) % (salt + 1) == 0).then_some((
                            a,
                            b,
                            if salt >= 8 { 1.0 } else { score(a, b) },
                        ))
                    })
                })
                .collect();
            let rows = AcceptedPairs::rows(&units, &paths, &members, relations);
            let explicit = AcceptedPairs::from(rows.iter().collect::<Vec<_>>());
            assert_eq!(rows.len(), explicit.len());
            let groups = (0..units.len())
                .map(|unit| (unit % 5 != 0).then_some(unit % 3))
                .collect::<Vec<_>>();
            let bit_scores = |pairs: &AcceptedPairs| {
                pairs
                    .group_scores(&groups, 3)
                    .into_iter()
                    .map(|(sum, count)| (sum.to_bits(), count))
                    .collect::<Vec<_>>()
            };
            assert_eq!(bit_scores(&rows), bit_scores(&explicit));
            assert_eq!(
                rows.components(units.len()),
                explicit.components(units.len())
            );
        }
    }
}
