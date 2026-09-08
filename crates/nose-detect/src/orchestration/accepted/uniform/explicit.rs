//! A unique, valid subset with the complete relation's cardinality is complete.
use super::super::AcceptedPair;
use crate::{candidates::round3, Group};
use rustc_hash::FxHashMap;

mod pair_count;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Member {
    group: usize,
    file: usize,
    start: u32,
    end: u32,
}

struct IndexedGroups {
    members: Vec<Option<Member>>,
    expected: Vec<Option<usize>>,
}

impl Member {
    fn nested(self, other: Self) -> bool {
        self.file == other.file
            && ((self.start <= other.start && self.end >= other.end)
                || (other.start <= self.start && other.end >= self.end))
    }
}

pub(super) fn uniform_groups(
    pairs: &[AcceptedPair],
    raw: &[Vec<usize>],
    groups: &[Group],
) -> Vec<Option<f64>> {
    let fallback = || vec![None; raw.len()];
    if !groups.iter().any(super::exact_group) {
        return fallback();
    }
    let Some(IndexedGroups { members, expected }) = index_members(raw, groups) else {
        return fallback();
    };
    let mut scores = vec![None::<u64>; raw.len()];
    let mut counts = vec![0usize; raw.len()];
    let mut valid = expected.iter().map(Option::is_some).collect::<Vec<_>>();
    let mut previous = None;
    for &(left, right, score) in pairs {
        if left >= right || previous.is_some_and(|pair| pair >= (left, right)) {
            return fallback();
        }
        previous = Some((left, right));
        let (Some(Some(a)), Some(Some(b))) = (members.get(left), members.get(right)) else {
            return fallback();
        };
        if a.group != b.group {
            return fallback();
        }
        let group = a.group;
        if !valid[group] {
            continue;
        }
        if a.nested(*b) {
            valid[group] = false;
            continue;
        }
        if scores[group].is_none() && !round3(score).is_finite() {
            valid[group] = false;
            continue;
        }
        let first = scores[group].get_or_insert(score.to_bits());
        valid[group] &= *first == score.to_bits();
        counts[group] += 1;
    }
    (0..raw.len())
        .map(|group| {
            (valid[group] && expected[group] == Some(counts[group]))
                .then(|| scores[group].map(|bits| round3(f64::from_bits(bits))))
                .flatten()
        })
        .collect()
}

fn index_members(raw: &[Vec<usize>], groups: &[Group]) -> Option<IndexedGroups> {
    if raw.len() != groups.len() {
        return None;
    }
    let size = raw
        .iter()
        .flatten()
        .max()
        .map_or(Some(0), |&index| index.checked_add(1))?;
    let mut indexed = vec![None; size];
    let mut files = FxHashMap::default();
    let mut expected = Vec::with_capacity(raw.len());
    for (group, (indices, report)) in raw.iter().zip(groups).enumerate() {
        if indices.len() != report.members.len() {
            return None;
        }
        let mut spans = Vec::with_capacity(indices.len());
        for (&index, location) in indices.iter().zip(&report.members) {
            let next = files.len();
            let file = *files.entry(location.file.as_str()).or_insert(next);
            if indexed[index].is_some() {
                return None;
            }
            indexed[index] = Some(Member {
                group,
                file,
                start: location.start_line,
                end: location.end_line,
            });
            spans.push((file, location.start_line, location.end_line));
        }
        let exact = super::exact_group(report);
        expected.push(exact.then(|| pair_count::non_nested_count(spans)));
    }
    Some(IndexedGroups {
        members: indexed,
        expected,
    })
}
