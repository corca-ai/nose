//! Streaming form of the product's existing global and per-file top-k policies.
use super::ScoredCandidate;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub(in crate::orchestration) const MIN_PRODUCT_SEED_NODES: usize = 18;
const GENERAL: usize = 2_048;
const NESTED: usize = 512;
const NESTED_PER_FILE: usize = 64;
const SCORED_PER_FILE: usize = 8;

pub(in crate::orchestration) fn path_classes(paths: &[&str]) -> Vec<usize> {
    let mut ids = FxHashMap::default();
    paths
        .iter()
        .map(|path| {
            let next = ids.len();
            *ids.entry(path).or_insert(next)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Ranked<T> {
    score: f64,
    ordinal: (usize, usize),
    payload: T,
}
impl<T> PartialEq for Ranked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl<T> Eq for Ranked<T> {}
impl<T> PartialOrd for Ranked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Ranked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // The largest entry is the worst retained one, ready for replacement.
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| self.ordinal.cmp(&other.ordinal))
    }
}

struct Top<T> {
    cap: usize,
    entries: BinaryHeap<Ranked<T>>,
}
impl<T> Top<T> {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: BinaryHeap::new(),
        }
    }
    fn may_retain(&self, score: f64, ordinal: (usize, usize)) -> bool {
        self.entries.len() < self.cap
            || self.entries.peek().is_some_and(|worst| {
                score.total_cmp(&worst.score).is_gt()
                    || (score.total_cmp(&worst.score).is_eq() && ordinal < worst.ordinal)
            })
    }
    fn push(&mut self, entry: Ranked<T>) {
        if self.entries.len() < self.cap {
            self.entries.push(entry);
        } else if self.entries.peek().is_some_and(|worst| entry < *worst) {
            *self.entries.peek_mut().unwrap() = entry;
        }
    }
}

pub(in crate::orchestration) struct SeedSelection<'a, T> {
    paths: &'a [usize],
    weights: &'a [usize],
    threshold: f64,
    general: Top<T>,
    nested: Top<T>,
    same: Vec<Top<T>>,
    cross: Vec<Top<T>>,
    nested_by_file: Vec<Top<T>>,
}

impl<'a, T: Copy> SeedSelection<'a, T> {
    pub(in crate::orchestration) fn new(
        paths: &'a [usize],
        weights: &'a [usize],
        threshold: f64,
    ) -> Self {
        let count = paths.iter().max().map_or(0, |id| id + 1);
        let per_file = |cap| (0..count).map(|_| Top::new(cap)).collect();
        Self {
            paths,
            weights,
            threshold,
            general: Top::new(GENERAL),
            nested: Top::new(NESTED),
            same: per_file(SCORED_PER_FILE),
            cross: per_file(SCORED_PER_FILE),
            nested_by_file: per_file(NESTED_PER_FILE),
        }
    }

    /// Conservative bound for a row pair with one score and sorted file classes.
    /// Its ordinal is a lower bound for every source pair in the row product.
    /// A nested location can occur only in a file shared by both rows.
    pub(in crate::orchestration) fn may_select(
        &self,
        score: f64,
        ordinal: (usize, usize),
        left: &[usize],
        right: &[usize],
    ) -> bool {
        if self.general.may_retain(score, ordinal) {
            return true;
        }
        for &path in left {
            if right.binary_search(&path).is_ok()
                && (self.same[path].may_retain(score, ordinal)
                    || self.nested.may_retain(0.0, ordinal)
                    || self.nested_by_file[path].may_retain(0.0, ordinal))
            {
                return true;
            }
            if (right.len() > 1 || right[0] != path) && self.cross[path].may_retain(score, ordinal)
            {
                return true;
            }
        }
        right.iter().any(|&path| {
            (left.len() > 1 || left[0] != path) && self.cross[path].may_retain(score, ordinal)
        })
    }

    pub(in crate::orchestration) fn push(
        &mut self,
        candidate: ScoredCandidate,
        ordinal: (usize, usize),
        payload: T,
    ) {
        if self.weights[candidate.left].min(self.weights[candidate.right]) < MIN_PRODUCT_SEED_NODES
        {
            return;
        }
        let left = self.paths[candidate.left];
        let right = self.paths[candidate.right];
        if let Some(score) = candidate
            .ordinary_score
            .filter(|&score| score < self.threshold)
        {
            let entry = Ranked {
                score,
                ordinal,
                payload,
            };
            self.general.push(entry);
            if left == right {
                self.same[left].push(entry);
            } else {
                self.cross[left].push(entry);
                self.cross[right].push(entry);
            }
        } else if candidate.ordinary_score.is_none() {
            let entry = Ranked {
                score: 0.0,
                ordinal,
                payload,
            };
            self.nested.push(entry);
            self.nested_by_file[left].push(entry);
        }
    }

    pub(in crate::orchestration) fn finish(self) -> Vec<T> {
        let mut selected = self.general.entries.into_vec();
        selected.extend(self.nested.entries);
        for top in self
            .same
            .into_iter()
            .chain(self.cross)
            .chain(self.nested_by_file)
        {
            selected.extend(top.entries);
        }
        selected.sort_unstable_by_key(|entry| entry.ordinal);
        selected.dedup_by_key(|entry| entry.ordinal);
        selected.into_iter().map(|entry| entry.payload).collect()
    }
}

#[cfg(test)]
mod tests;
