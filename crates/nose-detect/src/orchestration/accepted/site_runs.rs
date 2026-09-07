//! Exact target runs retain the last two occurrences from distinct files per
//! site. Ordered expiry events update masks as the source index advances, so
//! suffix membership needs no repeated target or per-site scan.
use super::AcceptedPair;
use rustc_hash::FxHashMap;
use std::ops::Range;

type SiteKey = (usize, u32, usize);
type RunKey = (usize, usize, u64);

pub(crate) enum SiteEvidence {
    Pair(AcceptedPair),
    ExactMask {
        left: usize,
        block: u32,
        mask: u64,
        score: f64,
    },
}

pub(super) struct TargetRuns {
    targets: Vec<(usize, f64)>,
    runs: Vec<Run>,
}

struct Run {
    range: Range<usize>,
    exact: Option<ExactRun>,
}

struct ExactRun {
    group: usize,
    class: usize,
    blocks: Vec<SiteBlock>,
    expiry: Vec<Expiry>,
    expired: usize,
}

struct SiteBlock {
    index: u32,
    mask: u64,
    excluded: FxHashMap<usize, u64>,
}

struct Expiry {
    right: usize,
    block: usize,
    bit: u64,
    exclude: Option<usize>,
}

struct SiteSuffix {
    site: u32,
    latest: (usize, usize),
    other_file: Option<usize>,
}

impl SiteSuffix {
    fn advance(&mut self, right: usize, path: usize) {
        // Targets arrive in source order. If the latest file changes, its old
        // occurrence is also the latest alternative to the new file.
        if self.latest.1 != path {
            self.other_file = Some(self.latest.0);
        }
        self.latest = (right, path);
    }

    #[cfg(test)]
    fn admits(&self, left: usize, path: usize) -> bool {
        let last = if self.latest.1 == path {
            self.other_file
        } else {
            Some(self.latest.0)
        };
        last.is_some_and(|right| right > left)
    }
}

fn run_key(
    pair: &(usize, f64),
    keys: &[Option<SiteKey>],
    exact: &[Option<usize>],
) -> Option<RunKey> {
    let (right, score) = *pair;
    let (group, _, _) = keys[right]?;
    let class = exact[right]?;
    if !crate::candidates::round3(score).is_finite() {
        return None;
    }
    Some((group, class, score.to_bits()))
}

impl ExactRun {
    fn new(
        key: RunKey,
        targets: &[(usize, f64)],
        keys: &[Option<SiteKey>],
        locations: &[(usize, u32, u32)],
    ) -> Self {
        let mut sites = FxHashMap::<u32, SiteSuffix>::default();
        for &(right, _) in targets {
            let site = keys[right].unwrap().1;
            let path = locations[right].0;
            sites
                .entry(site)
                .and_modify(|suffix| suffix.advance(right, path))
                .or_insert(SiteSuffix {
                    site,
                    latest: (right, path),
                    other_file: None,
                });
        }
        let mut sites = sites.into_values().collect::<Vec<_>>();
        sites.sort_unstable_by_key(|suffix| suffix.site);
        let mut blocks: Vec<SiteBlock> = Vec::new();
        let mut expiry = Vec::new();
        for suffix in sites {
            let index = suffix.site / 64;
            if blocks.last().is_none_or(|block| block.index != index) {
                blocks.push(SiteBlock {
                    index,
                    mask: 0,
                    excluded: FxHashMap::default(),
                });
            }
            let block = blocks.len() - 1;
            let bit = 1 << (suffix.site % 64);
            blocks[block].mask |= bit;
            expiry.push(Expiry {
                right: suffix.latest.0,
                block,
                bit,
                exclude: None,
            });
            if let Some(right) = suffix.other_file {
                // Until this alternative expires, every excluded file still has
                // a cross-file occurrence. Afterwards only the latest file does.
                expiry.push(Expiry {
                    right,
                    block,
                    bit,
                    exclude: Some(suffix.latest.1),
                });
            } else {
                *blocks[block].excluded.entry(suffix.latest.1).or_default() |= bit;
            }
        }
        expiry.sort_unstable_by_key(|event| event.right);
        Self {
            group: key.0,
            class: key.1,
            blocks,
            expiry,
            expired: 0,
        }
    }

    fn matches(&self, key: SiteKey, class: Option<usize>) -> bool {
        key.0 == self.group && class == Some(self.class)
    }

    fn visit(
        &mut self,
        left: usize,
        path: usize,
        score: f64,
        visit: &mut impl FnMut(SiteEvidence),
    ) {
        // The enclosing source traversal is monotone. Each event is applied at
        // most once; neither exclusions nor accepted-source cutoffs are relaxed.
        while let Some(event) = self
            .expiry
            .get(self.expired)
            .filter(|event| event.right <= left)
        {
            let block = &mut self.blocks[event.block];
            if let Some(path) = event.exclude {
                *block.excluded.entry(path).or_default() |= event.bit;
            } else {
                block.mask &= !event.bit;
            }
            self.expired += 1;
        }
        for block in &self.blocks {
            let mask = block.mask & !block.excluded.get(&path).copied().unwrap_or(0);
            if mask != 0 {
                visit(SiteEvidence::ExactMask {
                    left,
                    block: block.index,
                    mask,
                    score,
                });
            }
        }
    }
}

impl TargetRuns {
    pub(super) fn new(
        targets: Vec<(usize, f64)>,
        keys: &[Option<SiteKey>],
        exact: &[Option<usize>],
        locations: &[(usize, u32, u32)],
    ) -> Self {
        let mut runs: Vec<Run> = Vec::new();
        let mut start = 0;
        for chunk in targets.chunk_by(|a, b| run_key(a, keys, exact) == run_key(b, keys, exact)) {
            let end = start + chunk.len();
            // Short runs retain scalar traversal instead of allocating per-file masks.
            let packed = if chunk.len() >= 8 {
                run_key(&chunk[0], keys, exact)
                    .map(|key| ExactRun::new(key, chunk, keys, locations))
            } else {
                None
            };
            if packed.is_none() && runs.last().is_some_and(|run| run.exact.is_none()) {
                runs.last_mut().unwrap().range.end = end;
            } else {
                runs.push(Run {
                    range: start..end,
                    exact: packed,
                });
            }
            start = end;
        }
        Self { targets, runs }
    }

    pub(super) fn visit(
        &mut self,
        left: usize,
        key: SiteKey,
        class: Option<usize>,
        locations: &[(usize, u32, u32)],
        visit: &mut impl FnMut(SiteEvidence),
    ) {
        let start = self.targets.partition_point(|&(right, _)| right <= left);
        let first = self.runs.partition_point(|run| run.range.end <= start);
        let path = locations[left].0;
        for run in &mut self.runs[first..] {
            if let Some(exact) = run.exact.as_mut().filter(|exact| exact.matches(key, class)) {
                exact.visit(left, path, self.targets[run.range.start].1, visit);
                continue;
            }
            for &(right, score) in &self.targets[run.range.start.max(start)..run.range.end] {
                if path != locations[right].0 {
                    visit(SiteEvidence::Pair((left, right, score)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
