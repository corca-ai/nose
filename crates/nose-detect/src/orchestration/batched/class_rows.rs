//! A quotient of the candidate relation, refined by exact scoring inputs.
//! Members share every bucket, so every outside endpoint sees the entire row
//! or none of it. Location-dependent exclusions are applied after that relation.
use super::super::connected_pricing::{path_classes, SeedSelection};
use super::DetectionStages;
use crate::{DetectOptions, Detector, UnitFeat};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod tests;

struct Row {
    class: usize,
    members: Vec<usize>,
    buckets: Vec<usize>,
    shared_spans: FxHashMap<usize, usize>,
    connected: bool,
    paths: Vec<usize>,
    by_path: FxHashMap<usize, Vec<usize>>,
}

struct PairSources<'a> {
    units: &'a [UnitFeat],
    paths: &'a [usize],
    spans: &'a [usize],
}

impl Row {
    fn neighbors(
        &self,
        rows: &[Row],
        buckets: &[Vec<usize>],
        seen: &mut [usize],
        out: &mut Vec<usize>,
    ) {
        out.clear();
        for &bucket in &self.buckets {
            for &right in &buckets[bucket] {
                if seen[right] != self.members[0] {
                    seen[right] = self.members[0];
                    if self.members[0] < *rows[right].members.last().unwrap() {
                        out.push(right);
                    }
                }
            }
        }
    }

    fn collect_seeds(
        &self,
        other: &Row,
        score: f64,
        threshold: f64,
        sources: &PairSources<'_>,
        seeds: &mut SeedSelection<'_, super::super::ScoredCandidate>,
    ) {
        for &left in &self.members {
            let members = if score >= threshold {
                other
                    .by_path
                    .get(&sources.paths[left])
                    .map_or(&[][..], Vec::as_slice)
            } else {
                &other.members
            };
            let start = members.partition_point(|&right| right <= left);
            for &right in &members[start..] {
                if sources.spans[left] == sources.spans[right] {
                    continue;
                }
                let ordinary_score =
                    (!crate::locations::is_nested(&sources.units[left], &sources.units[right]))
                        .then_some(score);
                if ordinary_score.is_none_or(|s| s < threshold) {
                    let candidate = super::super::ScoredCandidate {
                        left,
                        right,
                        ordinary_score,
                    };
                    seeds.push(candidate, (left, right), candidate);
                }
            }
        }
    }

    fn pair_count(&self, other: &Row, same: bool) -> usize {
        if same {
            self.members.len() * (self.members.len() - 1) / 2
                - self
                    .shared_spans
                    .values()
                    .map(|n| n * (n - 1) / 2)
                    .sum::<usize>()
        } else {
            let (a, b) = if self.shared_spans.len() < other.shared_spans.len() {
                (&self.shared_spans, &other.shared_spans)
            } else {
                (&other.shared_spans, &self.shared_spans)
            };
            self.members.len() * other.members.len()
                - a.iter()
                    .map(|(span, count)| count * b.get(span).copied().unwrap_or(0))
                    .sum::<usize>()
        }
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
    let mut span_rows: FxHashMap<usize, FxHashMap<usize, usize>> = FxHashMap::default();
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
                shared_spans: FxHashMap::default(),
                connected,
                paths: Vec::new(),
                by_path: FxHashMap::default(),
            });
        }
        rows[row].members.push(index);
        *span_rows
            .entry(spans[index])
            .or_default()
            .entry(row)
            .or_default() += 1;
    }
    for (span, members) in span_rows {
        if members.values().sum::<usize>() > 1 {
            for (row, count) in members {
                rows[row].shared_spans.insert(span, count);
            }
        }
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
) -> DetectionStages {
    let (mut rows, bucket_rows) = rows(units, buckets, spans, classes);
    let representatives = rows
        .iter()
        .map(|row| &units[row.members[0]])
        .collect::<Vec<_>>();
    let prepared = detector.prepare_scores(&representatives);
    let paths = units.iter().map(|u| u.path.as_str()).collect::<Vec<_>>();
    let weights = units
        .iter()
        .map(|u| u.connected_tokens.len())
        .collect::<Vec<_>>();
    let path_ids = path_classes(&paths);
    for row in &mut rows {
        row.paths = row.members.iter().map(|&id| path_ids[id]).collect();
        row.paths.sort_unstable();
        row.paths.dedup();
        for &unit in &row.members {
            row.by_path.entry(path_ids[unit]).or_default().push(unit);
        }
    }
    let sources = PairSources {
        units,
        paths: &path_ids,
        spans,
    };
    let chunk_size = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let partials = rows
        .par_chunks(chunk_size)
        .enumerate()
        .map(|(chunk_id, chunk)| {
            let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
            let mut relations = Vec::new();
            let mut seen = vec![usize::MAX; rows.len()];
            let mut neighbors = Vec::new();
            let mut memo = FxHashMap::default();
            let mut seeds = SeedSelection::new(&path_ids, &weights, opts.threshold);
            for (offset, row) in chunk.iter().enumerate() {
                let row_id = chunk_id * chunk_size + offset;
                row.neighbors(&rows, &bucket_rows, &mut seen, &mut neighbors);
                memo.clear();
                let row_scores = prepared.as_ref().map(|p| p.row(row_id, &neighbors));
                for (position, &right_id) in neighbors.iter().enumerate() {
                    let right_row = &rows[right_id];
                    if row_id <= right_id {
                        result.candidate_count += row.pair_count(right_row, row_id == right_id);
                    }
                    // Score classes are interchangeable, but source admission is per edge.
                    let score = row_scores.as_ref().map_or_else(
                        || {
                            *memo.entry(right_row.class).or_insert_with(|| {
                                detector.score(&units[row.members[0]], &units[right_row.members[0]])
                            })
                        },
                        |scores| scores[position],
                    );
                    let connected =
                        opts.connected_witnesses && row.connected && right_row.connected;
                    if score >= opts.threshold {
                        relations.push((row_id, right_id, score));
                        if !connected {
                            continue;
                        }
                    }
                    if score < opts.threshold
                        && (!connected
                            || !seeds.may_select(
                                score,
                                (row.members[0], right_row.members[0]),
                                &row.paths,
                                &right_row.paths,
                            ))
                    {
                        continue;
                    }
                    row.collect_seeds(right_row, score, opts.threshold, &sources, &mut seeds);
                }
            }
            result.scored = seeds.finish();
            (result, relations)
        })
        .collect::<Vec<_>>();
    let mut result = DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new());
    let mut seeds = SeedSelection::new(&path_ids, &weights, opts.threshold);
    let mut relations = Vec::new();
    for (partial, accepted) in partials {
        result.candidate_count += partial.candidate_count;
        relations.extend(accepted);
        for candidate in partial.scored {
            seeds.push(candidate, (candidate.left, candidate.right), candidate);
        }
    }
    result.scored = seeds.finish();
    result.accepted = super::super::accepted::AcceptedPairs::rows(
        units,
        &path_ids,
        &rows.into_iter().map(|row| row.members).collect::<Vec<_>>(),
        relations,
    );
    result
}
