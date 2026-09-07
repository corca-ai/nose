//! A certified complete relation needs one score, not one block per site pair.
use super::{AcceptedEdge, Evidence, SiteEdges};
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct Complete {
    sites: u32,
    evidence: Evidence,
}

impl Complete {
    pub(super) fn incident_sites(&self) -> usize {
        if self.sites < 2 {
            0
        } else {
            self.sites as usize
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = AcceptedEdge> + '_ {
        (0..self.sites).flat_map(move |left| {
            (left + 1..self.sites).map(move |right| AcceptedEdge {
                left,
                right,
                score: self.evidence.score,
                witness_kind: self.evidence.witness_kind,
            })
        })
    }
}

impl SiteEdges {
    pub(crate) fn complete(sites: u32, score: f64) -> Arc<Self> {
        Arc::new(Self {
            count: sites as usize * sites.saturating_sub(1) as usize / 2,
            rows: Vec::new(),
            values: Vec::new(),
            complete: Some(Complete {
                sites,
                evidence: Evidence {
                    score,
                    witness_kind: "exact-value-graph",
                },
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::edges::SiteEdgeBuilder;

    #[test]
    fn complete_relation_preserves_explicit_order_count_and_score_bits() {
        for sites in [0, 1, 7, 8, 63, 64, 65, 130, 257] {
            for score in [-0.0, 0.0, 0.875] {
                let actual = SiteEdges::complete(sites, score);
                let mut expected = SiteEdgeBuilder::new(sites as usize);
                for left in 0..sites {
                    for right in left + 1..sites {
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
                let expected = expected.into_edges();
                assert_eq!(actual.count, expected.count);
                let collect = |edges: &SiteEdges| {
                    edges
                        .iter()
                        .map(|edge| {
                            (
                                edge.left,
                                edge.right,
                                edge.score.to_bits(),
                                edge.witness_kind,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(collect(&actual), collect(&expected));
            }
        }
    }
}
