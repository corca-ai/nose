//! Direct evidence in raw member coordinates or the canonical reported sites.
//! Uniform 64-site blocks share a palette entry; mixed blocks retain exact values.
use super::AcceptedEdge;
use rustc_hash::FxHashMap;
use std::sync::Arc;

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
    appended: Vec<AcceptedEdge>,
}

impl From<Vec<AcceptedEdge>> for AcceptedEdges {
    fn from(appended: Vec<AcceptedEdge>) -> Self {
        Self {
            packed: None,
            appended,
        }
    }
}

impl AcceptedEdges {
    pub fn iter(&self) -> impl Iterator<Item = AcceptedEdge> + '_ {
        self.packed
            .iter()
            .flat_map(|edges| edges.iter())
            .chain(self.appended.iter().cloned())
    }

    pub fn len(&self) -> usize {
        self.packed.as_ref().map_or(0, |edges| edges.count) + self.appended.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, edge: AcceptedEdge) {
        self.appended.push(edge);
    }
}

impl PartialEq for AcceptedEdges {
    fn eq(&self, other: &Self) -> bool {
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
}

pub(crate) struct SiteEdgeBuilder {
    rows: Vec<FxHashMap<u32, Block>>,
    palette: FxHashMap<(u64, &'static str), u32>,
    values: Vec<Evidence>,
}

impl SiteEdgeBuilder {
    pub(crate) fn new(sites: usize) -> Self {
        Self {
            rows: (0..sites).map(|_| FxHashMap::default()).collect(),
            palette: FxHashMap::default(),
            values: Vec::new(),
        }
    }

    pub(crate) fn best(&self, left: u32, right: u32) -> Option<Evidence> {
        let code = self.rows[left as usize]
            .get(&(right / 64))?
            .get((right % 64) as usize)?;
        Some(self.values[code as usize])
    }

    pub(crate) fn insert(&mut self, left: u32, right: u32, evidence: Evidence) {
        let next = self.values.len() as u32;
        let code = *self
            .palette
            .entry((evidence.score.to_bits(), evidence.witness_kind))
            .or_insert(next);
        if code == next {
            self.values.push(evidence);
        }
        self.rows[left as usize]
            .entry(right / 64)
            .or_insert(Block {
                mask: 0,
                values: Values::Uniform(code),
            })
            .insert((right % 64) as usize, code);
    }

    pub(crate) fn finish(self) -> GroupEdges {
        let count = self
            .rows
            .iter()
            .flat_map(|row| row.values())
            .map(|block| block.mask.count_ones() as usize)
            .sum();
        GroupEdges::Sites(AcceptedEdges {
            packed: Some(Arc::new(SiteEdges {
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
            })),
            appended: Vec::new(),
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
        let GroupEdges::Sites(edges) = builder.finish() else {
            unreachable!()
        };
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
