//! Project direct evidence to ranking's canonical sites, preserving the winning
//! accepted score and witness. The coordinate domain is explicit in GroupEdges.
use super::{round3, witness_kind, AcceptedPairs, Group, UnitFeat};
use crate::{
    report::{
        edges::{Evidence, SiteEdgeBuilder},
        sites,
    },
    AcceptedEdge, GroupEdges,
};
use rayon::prelude::*;

mod exact_blocks;
mod inputs;

pub(super) fn accepted_edges_by_group(
    units: &[UnitFeat],
    raw_groups: &[Vec<usize>],
    groups: &[Group],
    accepted: &AcceptedPairs,
) -> Vec<GroupEdges> {
    let uniform = accepted.uniform_groups(raw_groups, groups);
    if uniform.iter().all(Option::is_some) {
        return uniform
            .into_iter()
            .map(|score| GroupEdges::AllNonNested(score.unwrap()))
            .collect();
    }
    if accepted.len() <= 1_000_000 {
        return expanded_edges(units, raw_groups, groups, accepted, &uniform);
    }
    projected_edges(units, raw_groups, groups, accepted, &uniform)
}

fn expanded_edges(
    units: &[UnitFeat],
    raw_groups: &[Vec<usize>],
    groups: &[Group],
    accepted: &AcceptedPairs,
    uniform: &[Option<f64>],
) -> Vec<GroupEdges> {
    let mut position = vec![None; units.len()];
    for (group, members) in raw_groups.iter().enumerate() {
        if uniform[group].is_some() {
            continue;
        }
        for (local, &unit) in members.iter().enumerate() {
            position[unit] = Some((group, local as u32));
        }
    }
    let mut by_group = vec![Vec::new(); raw_groups.len()];
    for (left, right, score) in accepted.iter() {
        let (Some((group, _)), Some((other, _))) = (position[left], position[right]) else {
            continue;
        };
        debug_assert_eq!(group, other);
        if group == other {
            by_group[group].push((left, right, score));
        }
    }
    // Each group owns its source-ordered evidence directly. Avoid global pair
    // and classified arrays followed by another serial copy into every group.
    by_group
        .into_par_iter()
        .enumerate()
        .map(|(group, pairs)| {
            if let Some(score) = uniform[group] {
                return GroupEdges::AllNonNested(score);
            }
            let exact = groups[group].witness.as_ref().is_some_and(|witness| {
                matches!(
                    &witness.evidence,
                    crate::WitnessEvidence::ExactValueGraph { .. }
                )
            });
            GroupEdges::Members(
                pairs
                    .into_par_iter()
                    .with_min_len(256)
                    .map(|(left, right, score)| AcceptedEdge {
                        left: position[left].unwrap().1,
                        right: position[right].unwrap().1,
                        score: round3(score),
                        witness_kind: if exact {
                            "exact-value-graph"
                        } else {
                            witness_kind(&[left, right], units)
                        },
                    })
                    .collect(),
            )
        })
        .collect()
}

fn projected_edges(
    units: &[UnitFeat],
    raw_groups: &[Vec<usize>],
    groups: &[Group],
    accepted: &AcceptedPairs,
    uniform: &[Option<f64>],
) -> Vec<GroupEdges> {
    let projection = std::sync::Arc::new(Projection::new(
        units, raw_groups, groups, accepted, uniform,
    ));
    let large = projection
        .sizes
        .iter()
        .map(|&n| n.saturating_mul(n.saturating_sub(1)) / 2 > 1_000_000)
        .collect::<Vec<_>>();
    let mut ready = projection.materialize(|group| !large[group] && uniform[group].is_none());
    (0..groups.len())
        .map(|group| {
            if let Some(score) = uniform[group] {
                return GroupEdges::AllNonNested(score);
            }
            if let Some(edges) = ready[group].take() {
                return GroupEdges::Sites(crate::AcceptedEdges::from_packed(edges));
            }
            let projection = projection.clone();
            // A connected member graph mapped entirely onto two or more sites
            // necessarily has a cross-site edge. Unmapped members need an explicit check.
            let has_edges = projection.has_edges(group, &raw_groups[group]);
            GroupEdges::Sites(crate::AcceptedEdges::deferred(has_edges, move || {
                projection.materialize(|selected| selected == group)[group]
                    .take()
                    .unwrap()
            }))
        })
        .collect()
}

struct Projection {
    accepted: AcceptedPairs,
    keys: Vec<Option<(usize, u32, usize)>>,
    exact: Vec<Option<usize>>,
    anchors: Vec<Vec<nose_normalize::Anchor>>,
    floor: u32,
    sizes: Vec<usize>,
}

impl Projection {
    fn new(
        units: &[UnitFeat],
        raw: &[Vec<usize>],
        groups: &[Group],
        accepted: &AcceptedPairs,
        uniform: &[Option<f64>],
    ) -> Self {
        let mut position = vec![None; units.len()];
        let mappings = groups
            .par_iter()
            .enumerate()
            .map(|(index, group)| {
                if uniform[index].is_some() {
                    return (0, Vec::new());
                }
                let collapsed = sites::collapsed_sites(group);
                (collapsed.len(), sites::member_sites(group, &collapsed))
            })
            .collect::<Vec<_>>();
        let mut sizes = Vec::with_capacity(groups.len());
        for (group_id, (members, (size, sites))) in raw.iter().zip(mappings).enumerate() {
            sizes.push(size);
            for (&unit, site) in members.iter().zip(sites) {
                position[unit] = site.map(|site| (group_id, site));
            }
        }
        let inputs = inputs::WitnessInputs::new(units, &position, groups);
        Self {
            accepted: accepted.clone(),
            keys: inputs.keys,
            exact: inputs.exact,
            anchors: inputs.anchors,
            floor: nose_normalize::anchor_min_weight(),
            sizes,
        }
    }

    fn has_edges(&self, group: usize, members: &[usize]) -> bool {
        if self.sizes[group] < 2 {
            return false;
        }
        if members.iter().all(|&unit| self.keys[unit].is_some()) {
            return true;
        }
        self.accepted.iter().any(|(left, right, _)| {
            matches!((self.keys[left], self.keys[right]),
                (Some((a, x, _)), Some((b, y, _))) if a == group && b == group && x != y)
        })
    }

    fn materialize(
        &self,
        selected: impl Fn(usize) -> bool,
    ) -> Vec<Option<std::sync::Arc<crate::SiteEdges>>> {
        let mut edges = self
            .sizes
            .iter()
            .enumerate()
            .map(|(group, &size)| selected(group).then(|| SiteEdgeBuilder::new(size)))
            .collect::<Vec<_>>();
        if edges.iter().all(Option::is_none) {
            return edges.into_iter().map(|_| None).collect();
        }
        let keys = self
            .keys
            .iter()
            .map(|&key| key.filter(|&(group, _, _)| selected(group)))
            .collect::<Vec<_>>();
        let mut kinds = vec![None; self.anchors.len()];
        let mut exact_blocks = exact_blocks::ExactBlocks::default();
        let mut complete = vec![None; self.sizes.len()];
        self.accepted
            .visit_projected_evidence(&keys, &self.exact, |evidence| {
                use crate::orchestration::accepted::SiteEvidence;
                let (left, right, score) = match evidence {
                    SiteEvidence::Pair(pair) => pair,
                    SiteEvidence::Complete { left, sites, score } => {
                        let group = keys[left].unwrap().0;
                        // The producer proves this group has no competing row.
                        complete[group] = Some(crate::SiteEdges::complete(sites, round3(score)));
                        edges[group] = None;
                        return;
                    }
                    SiteEvidence::ExactMask {
                        left,
                        block,
                        mask,
                        score,
                    } => {
                        let (group, site, _) = keys[left].unwrap();
                        exact_blocks.push_sites(
                            &mut edges,
                            group,
                            site,
                            block,
                            mask,
                            round3(score),
                        );
                        return;
                    }
                };
                let (Some((group, a, left_class)), Some((other, b, right_class))) =
                    (keys[left], keys[right])
                else {
                    return;
                };
                debug_assert_eq!(group, other);
                if group != other || a == b {
                    return;
                }
                let (a, b) = (a.min(b), a.max(b));
                let score = round3(score);
                let is_exact = self.exact[left].is_some() && self.exact[left] == self.exact[right];
                if is_exact && score.is_finite() {
                    exact_blocks.push(&mut edges, group, a, b, score);
                    return;
                }
                exact_blocks.flush(&mut edges);
                let builder = edges[group].as_mut().unwrap();
                let previous = builder.best(a, b);
                if previous.is_some_and(|edge| edge.score > score) {
                    return;
                }
                let best_kind = if is_exact {
                    "exact-value-graph"
                } else {
                    "shared-sub-dag"
                };
                if previous
                    .is_some_and(|edge| edge.score == score && edge.witness_kind <= best_kind)
                {
                    return;
                }
                let kind =
                    self.projected_witness_kind(is_exact, left_class, right_class, &mut kinds);
                if previous.is_none_or(|edge| score > edge.score || kind < edge.witness_kind) {
                    builder.insert(
                        a,
                        b,
                        Evidence {
                            score,
                            witness_kind: kind,
                        },
                    );
                }
            });
        exact_blocks.flush(&mut edges);
        edges
            .into_iter()
            .zip(complete)
            .map(|(builder, complete)| {
                complete.or_else(|| builder.map(SiteEdgeBuilder::into_edges))
            })
            .collect()
    }

    fn projected_witness_kind(
        &self,
        is_exact: bool,
        left_class: usize,
        right_class: usize,
        cache: &mut [Option<(usize, &'static str)>],
    ) -> &'static str {
        if is_exact {
            return "exact-value-graph";
        }
        if let Some((_, kind)) = cache[right_class].filter(|&(class, _)| class == left_class) {
            return kind;
        }
        let kind = self.anchor_witness_kind(left_class, right_class);
        cache[right_class] = Some((left_class, kind));
        kind
    }

    fn anchor_witness_kind(&self, left_class: usize, right_class: usize) -> &'static str {
        if super::shared_anchor_weight_at_floor(
            &self.anchors[left_class],
            &self.anchors[right_class],
            self.floor,
        ) > 0
        {
            "shared-sub-dag"
        } else {
            "structural-similarity"
        }
    }
}

#[cfg(test)]
mod tests;
