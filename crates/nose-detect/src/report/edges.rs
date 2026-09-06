//! Direct evidence in raw member coordinates or the canonical reported sites.
//! Uniform 64-site blocks share a palette entry; mixed blocks retain exact values.
use super::AcceptedEdge;
use rustc_hash::FxHashMap;
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub enum GroupEdges {
    Members(Vec<AcceptedEdge>),
    Sites(AcceptedEdges),
}

/// Ordered direct evidence. Large immutable graphs remain shared when a family
/// becomes a coverage obligation; iteration reconstructs each exact edge lazily.
#[derive(Clone, Debug, Default)]
pub struct AcceptedEdges {
    packed: Option<Arc<SiteEdges>>,
    deferred: Option<Arc<DeferredEdges>>,
    appended: Vec<AcceptedEdge>,
}

impl From<Vec<AcceptedEdge>> for AcceptedEdges {
    fn from(appended: Vec<AcceptedEdge>) -> Self {
        Self {
            packed: None,
            deferred: None,
            appended,
        }
    }
}

struct DeferredEdges {
    has_edges: bool,
    edges: OnceLock<Arc<SiteEdges>>,
    build: Box<dyn Fn() -> Arc<SiteEdges> + Send + Sync>,
}

impl std::fmt::Debug for DeferredEdges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeferredEdges")
            .field("has_edges", &self.has_edges)
            .field("materialized", &self.edges.get().is_some())
            .finish_non_exhaustive()
    }
}

impl AcceptedEdges {
    pub(crate) fn from_packed(packed: Arc<SiteEdges>) -> Self {
        Self {
            packed: Some(packed),
            deferred: None,
            appended: Vec::new(),
        }
    }

    pub(crate) fn deferred(
        has_edges: bool,
        build: impl Fn() -> Arc<SiteEdges> + Send + Sync + 'static,
    ) -> Self {
        Self {
            packed: None,
            deferred: Some(Arc::new(DeferredEdges {
                has_edges,
                edges: OnceLock::new(),
                build: Box::new(build),
            })),
            appended: Vec::new(),
        }
    }

    fn packed(&self) -> Option<&Arc<SiteEdges>> {
        self.packed.as_ref().or_else(|| {
            self.deferred
                .as_ref()
                .map(|deferred| deferred.edges.get_or_init(|| (deferred.build)()))
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = AcceptedEdge> + '_ {
        self.packed()
            .into_iter()
            .flat_map(|edges| edges.iter())
            .chain(self.appended.iter().cloned())
    }

    pub fn len(&self) -> usize {
        self.packed().map_or(0, |edges| edges.count) + self.appended.len()
    }

    pub fn is_empty(&self) -> bool {
        self.appended.is_empty()
            && self.deferred.as_ref().map_or_else(
                || self.packed.as_ref().is_none_or(|edges| edges.count == 0),
                |deferred| !deferred.has_edges,
            )
    }

    pub fn push(&mut self, edge: AcceptedEdge) {
        self.appended.push(edge);
    }
}

impl PartialEq for AcceptedEdges {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (&self.deferred, &other.deferred) {
            if Arc::ptr_eq(a, b) {
                return self.appended == other.appended;
            }
        }
        if let (Some(a), Some(b)) = (&self.packed, &other.packed) {
            if Arc::ptr_eq(a, b) {
                return self.appended == other.appended;
            }
        }
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl From<Vec<AcceptedEdge>> for GroupEdges {
    fn from(edges: Vec<AcceptedEdge>) -> Self {
        Self::Members(edges)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Evidence {
    pub(crate) score: f64,
    pub(crate) witness_kind: &'static str,
}

#[derive(Debug)]
enum Values {
    Uniform(u32),
    Mixed(Box<[u32; 64]>),
}

#[derive(Debug)]
struct Block {
    mask: u64,
    values: Values,
}

impl Block {
    fn get(&self, offset: usize) -> Option<u32> {
        (self.mask & (1 << offset) != 0).then(|| match &self.values {
            Values::Uniform(code) => *code,
            Values::Mixed(codes) => codes[offset],
        })
    }

    fn insert(&mut self, offset: usize, code: u32) {
        match &mut self.values {
            Values::Uniform(previous) if *previous != code => {
                let mut codes = Box::new([*previous; 64]);
                codes[offset] = code;
                self.values = Values::Mixed(codes);
            }
            Values::Mixed(codes) => codes[offset] = code,
            Values::Uniform(_) => {}
        }
        self.mask |= 1 << offset;
    }

    fn merge(&mut self, mask: u64, code: u32, palette: &[Evidence]) {
        if let Values::Uniform(previous) = self.values {
            if previous == code {
                self.mask |= mask;
                return;
            }
            let changed = if evidence_wins(palette[code as usize], palette[previous as usize]) {
                mask
            } else {
                mask & !self.mask
            };
            if changed == 0 {
                return;
            }
            if self.mask & !changed == 0 {
                self.values = Values::Uniform(code);
            } else {
                let mut codes = Box::new([previous; 64]);
                write_codes(&mut codes, changed, code);
                self.values = Values::Mixed(codes);
            }
        } else if let Values::Mixed(codes) = &mut self.values {
            let mut remaining = mask;
            while remaining != 0 {
                let offset = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                if self.mask & (1 << offset) == 0
                    || evidence_wins(palette[code as usize], palette[codes[offset] as usize])
                {
                    codes[offset] = code;
                }
            }
        }
        self.mask |= mask;
    }
}

fn write_codes(codes: &mut [u32; 64], mut mask: u64, code: u32) {
    while mask != 0 {
        codes[mask.trailing_zeros() as usize] = code;
        mask &= mask - 1;
    }
}

fn evidence_wins(next: Evidence, previous: Evidence) -> bool {
    if previous.score > next.score {
        return false;
    }
    next.score > previous.score || next.witness_kind < previous.witness_kind
}

pub(crate) struct SiteEdgeBuilder {
    rows: Vec<FxHashMap<u32, Block>>,
    palette: FxHashMap<(u64, &'static str), u32>,
    values: Vec<Evidence>,
    last_code: Option<u32>,
}

impl SiteEdgeBuilder {
    pub(crate) fn new(sites: usize) -> Self {
        Self {
            rows: (0..sites).map(|_| FxHashMap::default()).collect(),
            palette: FxHashMap::default(),
            values: Vec::new(),
            last_code: None,
        }
    }

    pub(crate) fn best(&self, left: u32, right: u32) -> Option<Evidence> {
        let code = self.rows[left as usize]
            .get(&(right / 64))?
            .get((right % 64) as usize)?;
        Some(self.values[code as usize])
    }

    pub(crate) fn insert(&mut self, left: u32, right: u32, evidence: Evidence) {
        let code = self.evidence_code(evidence);
        self.rows[left as usize]
            .entry(right / 64)
            .or_insert(Block {
                mask: 0,
                values: Values::Uniform(code),
            })
            .insert((right % 64) as usize, code);
    }

    pub(crate) fn insert_exact_mask(&mut self, left: u32, block: u32, mask: u64, score: f64) {
        if mask == 0 {
            return;
        }
        let code = self.evidence_code(Evidence {
            score,
            witness_kind: "exact-value-graph",
        });
        self.rows[left as usize]
            .entry(block)
            .or_insert(Block {
                mask: 0,
                values: Values::Uniform(code),
            })
            .merge(mask, code, &self.values);
    }

    fn evidence_code(&mut self, evidence: Evidence) -> u32 {
        if let Some(code) = self.last_code {
            let previous = self.values[code as usize];
            if previous.score.to_bits() == evidence.score.to_bits()
                && previous.witness_kind == evidence.witness_kind
            {
                return code;
            }
        }
        let next = self.values.len() as u32;
        let code = *self
            .palette
            .entry((evidence.score.to_bits(), evidence.witness_kind))
            .or_insert(next);
        if code == next {
            self.values.push(evidence);
        }
        self.last_code = Some(code);
        code
    }

    pub(crate) fn into_edges(self) -> Arc<SiteEdges> {
        let count = self
            .rows
            .iter()
            .flat_map(|row| row.values())
            .map(|block| block.mask.count_ones() as usize)
            .sum();
        Arc::new(SiteEdges {
            count,
            rows: self
                .rows
                .into_iter()
                .map(|row| {
                    let mut blocks = row.into_iter().collect::<Vec<_>>();
                    blocks.sort_unstable_by_key(|&(index, _)| index);
                    blocks
                })
                .collect(),
            values: self.values,
        })
    }
}

/// Exact direct site pairs, with scores and witness categories stored once per
/// palette entry. Coordinates use `collapsed_sites(group)` from the same report.
#[derive(Debug)]
pub struct SiteEdges {
    count: usize,
    rows: Vec<Vec<(u32, Block)>>,
    values: Vec<Evidence>,
}

impl SiteEdges {
    pub fn iter(&self) -> impl Iterator<Item = AcceptedEdge> + '_ {
        self.rows
            .iter()
            .enumerate()
            .flat_map(move |(left, blocks)| {
                blocks.iter().flat_map(move |(base, block)| {
                    let mut mask = block.mask;
                    std::iter::from_fn(move || {
                        if mask == 0 {
                            return None;
                        }
                        let offset = mask.trailing_zeros();
                        mask &= mask - 1;
                        let evidence = self.values[block.get(offset as usize).unwrap() as usize];
                        Some(AcceptedEdge {
                            left: left as u32,
                            right: base * 64 + offset,
                            score: evidence.score,
                            witness_kind: evidence.witness_kind,
                        })
                    })
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_evidence_preserves_appends_and_is_shared_across_concurrent_readers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let builds = Arc::new(AtomicUsize::new(0));
        let counter = builds.clone();
        let mut edges = AcceptedEdges::deferred(true, move || {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut builder = SiteEdgeBuilder::new(3);
            builder.insert(
                0,
                2,
                Evidence {
                    score: 0.8,
                    witness_kind: "shared-sub-dag",
                },
            );
            builder.into_edges()
        });
        let original = edges.clone();
        assert!(!edges.is_empty());
        assert_eq!(edges, original);
        edges.push(AcceptedEdge {
            left: 0,
            right: 1,
            score: 1.0,
            witness_kind: "exact-value-graph",
        });
        assert_eq!(builds.load(Ordering::SeqCst), 0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let edges = &edges;
                scope.spawn(move || {
                    assert_eq!(edges.len(), 2);
                    assert_eq!(
                        edges.iter().map(|edge| edge.score).collect::<Vec<_>>(),
                        [0.8, 1.0]
                    );
                });
            }
        });
        assert_eq!(original.len(), 1);
        assert_ne!(edges, original);
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exact_masks_match_ordered_edge_updates_for_uniform_mixed_and_sparse_blocks() {
        let mut actual = SiteEdgeBuilder::new(130);
        let mut expected = SiteEdgeBuilder::new(130);
        let scores = [-0.0_f64, 0.0, 0.7, 0.812345, 1.0];
        let kinds = [
            "exact-value-graph",
            "shared-sub-dag",
            "structural-similarity",
        ];
        for left in 0..129 {
            for right in left + 1..130 {
                if (left + right) % 7 == 0 {
                    let evidence = Evidence {
                        score: scores[right as usize % 5],
                        witness_kind: kinds[left as usize % 3],
                    };
                    actual.insert(left, right, evidence);
                    expected.insert(left, right, evidence);
                }
            }
        }
        let mut seed = 0x7ade_295d_346c_120d_u64;
        for step in 0..512 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let left = (seed % 129) as u32;
            let block = ((seed >> 8) % 3) as u32;
            let mask = (0..64)
                .filter(|&offset| {
                    let right = block * 64 + offset;
                    right > left && right < 130 && seed & (1 << offset) != 0
                })
                .fold(0, |mask, offset| mask | (1 << offset));
            let score = scores[step % scores.len()];
            actual.insert_exact_mask(left, block, mask, score);
            for offset in 0..64 {
                if mask & (1 << offset) == 0 {
                    continue;
                }
                let right = block * 64 + offset;
                let previous = expected.best(left, right);
                if previous.is_some_and(|old| old.score > score) {
                    continue;
                }
                if previous
                    .is_none_or(|old| score > old.score || "exact-value-graph" < old.witness_kind)
                {
                    expected.insert(
                        left,
                        right,
                        Evidence {
                            score,
                            witness_kind: "exact-value-graph",
                        },
                    );
                }
            }
        }
        actual.insert_exact_mask(129, 2, 0, 1.0);
        let (actual, expected) = (actual.into_edges(), expected.into_edges());
        assert_eq!(actual.count, expected.count);
        assert_eq!(
            actual.iter().collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            actual.iter().map(|e| e.score.to_bits()).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|e| e.score.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn packed_blocks_keep_sparse_mixed_and_uniform_evidence_in_pair_order() {
        let mut builder = SiteEdgeBuilder::new(130);
        let mut expected = std::collections::BTreeMap::new();
        for step in 0..3 {
            for left in (0..129).rev() {
                for right in (left + 1..130).rev() {
                    if (left + right) % 5 != 1 && (step == 0 || (left + right) % 3 == 0) {
                        let evidence = Evidence {
                            score: [0.7, 0.812345, 1.0][step],
                            witness_kind: if step == 1 {
                                "shared-sub-dag"
                            } else {
                                "exact-value-graph"
                            },
                        };
                        builder.insert(left, right, evidence);
                        expected.insert((left, right), evidence);
                    }
                }
            }
        }
        assert!(builder.best(129, 129).is_none());
        let edges = AcceptedEdges::from_packed(builder.into_edges());
        let actual = edges
            .iter()
            .map(|edge| {
                (
                    (edge.left, edge.right),
                    edge.score.to_bits(),
                    edge.witness_kind,
                )
            })
            .collect::<Vec<_>>();
        let expected = expected
            .into_iter()
            .map(|(pair, e)| (pair, e.score.to_bits(), e.witness_kind))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
