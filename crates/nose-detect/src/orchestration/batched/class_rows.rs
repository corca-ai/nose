//! A quotient of the candidate relation, refined by exact scoring inputs.
//! Members share every bucket, so every outside endpoint sees the entire row
//! or none of it. Location-dependent exclusions are applied after that relation.
use super::{retain_connected_seeds, DetectionStages};
use crate::{DetectOptions, Detector, UnitFeat};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod tests;

struct Row {
    class: usize,
    members: Vec<usize>,
    buckets: Vec<usize>,
    spans: FxHashMap<usize, Vec<usize>>,
    connected: bool,
}

impl Row {
    fn suffix(&self, left: usize, span: usize) -> (usize, usize) {
        let start = self.members.partition_point(|&right| right <= left);
        let same_span = self.spans.get(&span).map_or(0, |members| {
            members.len() - members.partition_point(|&right| right <= left)
        });
        (start, self.members.len() - start - same_span)
    }
}

fn rows(
    units: &[UnitFeat],
    buckets: &[Vec<u32>],
    spans: &[usize],
    classes: &[usize],
) -> (Vec<Row>, Vec<Vec<usize>>) {
    let membership = crate::lsh::membership(units.len(), buckets);
    let mut ids = FxHashMap::default();
    let mut rows: Vec<Row> = Vec::new();
    for (index, memberships) in membership.iter().enumerate() {
        let connected = units[index].connected_tokens.len()
            >= super::super::connected_pricing::MIN_PRODUCT_SEED_NODES;
        let next = rows.len();
        let row = *ids
            .entry((classes[index], memberships, connected))
            .or_insert(next);
        if row == next {
            rows.push(Row {
                class: classes[index],
                members: Vec::new(),
                buckets: memberships.clone(),
                spans: FxHashMap::default(),
                connected,
            });
        }
        rows[row].members.push(index);
        rows[row].spans.entry(spans[index]).or_default().push(index);
    }
    let mut bucket_rows = vec![Vec::new(); buckets.len()];
    for (id, row) in rows.iter().enumerate() {
        for &bucket in &row.buckets {
            bucket_rows[bucket].push(id);
        }
    }
    (rows, bucket_rows)
}

pub(super) fn score(
    units: &[UnitFeat],
    opts: &DetectOptions,
    detector: &dyn Detector,
    buckets: &[Vec<u32>],
    spans: &[usize],
    classes: &[usize],
    batch_size: usize,
) -> DetectionStages {
    let (rows, bucket_rows) = rows(units, buckets, spans, classes);
    let paths = units.iter().map(|u| u.path.as_str()).collect::<Vec<_>>();
    let weights = units
        .iter()
        .map(|u| u.connected_tokens.len())
        .collect::<Vec<_>>();
    let compact = |result: &mut DetectionStages| {
        result
            .scored
            .sort_unstable_by_key(|pair| (pair.left, pair.right));
        retain_connected_seeds(&mut result.scored, &paths, &weights, opts.threshold);
    };
    let partials = rows
        .par_chunks(rows.len().div_ceil(rayon::current_num_threads() * 4).max(1))
        .map(|chunk| {
            let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
            let mut seen = vec![usize::MAX; rows.len()];
            let mut neighbors = Vec::new();
            let mut memo = FxHashMap::default();
            let mut pending = 0;
            for row in chunk {
                neighbors.clear();
                memo.clear();
                for &bucket in &row.buckets {
                    for &right in &bucket_rows[bucket] {
                        if seen[right] != row.members[0] {
                            seen[right] = row.members[0];
                            neighbors.push(right);
                        }
                    }
                }
                for &left in &row.members {
                    for &right_row in &neighbors {
                        let right_row = &rows[right_row];
                        let (start, count) = right_row.suffix(left, spans[left]);
                        if count == 0 {
                            continue;
                        }
                        result.candidate_count += count;
                        // Classes promise interchangeable arguments. The representative
                        // supplies only a score; it is never admitted as a source edge.
                        let score = *memo.entry(right_row.class).or_insert_with(|| {
                            detector.score(&units[left], &units[right_row.members[start]])
                        });
                        let seeds =
                            opts.connected_witnesses && row.connected && right_row.connected;
                        if score < opts.threshold && !seeds {
                            continue;
                        }
                        for &right in &right_row.members[start..] {
                            if spans[left] == spans[right] {
                                continue;
                            }
                            let ordinary_score =
                                (!crate::locations::is_nested(&units[left], &units[right]))
                                    .then_some(score);
                            if let Some(score) = ordinary_score.filter(|&s| s >= opts.threshold) {
                                result.accepted.push((left, right, score));
                            }
                            if seeds && ordinary_score.is_none_or(|s| s < opts.threshold) {
                                result.scored.push(super::super::ScoredCandidate {
                                    left,
                                    right,
                                    ordinary_score,
                                });
                                pending += 1;
                                if pending == batch_size {
                                    compact(&mut result);
                                    pending = 0;
                                }
                            }
                        }
                    }
                }
            }
            compact(&mut result);
            result
        })
        .collect::<Vec<_>>();
    let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
    for partial in partials {
        result.candidate_count += partial.candidate_count;
        result.accepted.extend(partial.accepted);
        result.scored.extend(partial.scored);
        compact(&mut result);
    }
    // Floating-point aggregation and connected tie breaking see original pair order.
    result
        .accepted
        .sort_unstable_by_key(|&(left, right, _)| (left, right));
    result
}
