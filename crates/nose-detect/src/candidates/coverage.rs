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
    if accepted.len() <= 1_000_000 {
        return expanded_edges(units, raw_groups, accepted);
    }
    projected_edges(units, raw_groups, groups, accepted)
}

fn expanded_edges(
    units: &[UnitFeat],
    raw_groups: &[Vec<usize>],
    accepted: &AcceptedPairs,
) -> Vec<GroupEdges> {
    let mut position = vec![None; units.len()];
    for (group, members) in raw_groups.iter().enumerate() {
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
        .map(|pairs| {
            GroupEdges::Members(
                pairs
                    .into_par_iter()
                    .with_min_len(256)
                    .map(|(left, right, score)| AcceptedEdge {
                        left: position[left].unwrap().1,
                        right: position[right].unwrap().1,
                        score: round3(score),
                        witness_kind: witness_kind(&[left, right], units),
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
) -> Vec<GroupEdges> {
    let projection = std::sync::Arc::new(Projection::new(units, raw_groups, groups, accepted));
    let large = projection
        .sizes
        .iter()
        .map(|&n| n.saturating_mul(n.saturating_sub(1)) / 2 > 1_000_000)
        .collect::<Vec<_>>();
    let mut ready = projection.materialize(|group| !large[group]);
    (0..groups.len())
        .map(|group| {
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
    ) -> Self {
        let mut position = vec![None; units.len()];
        let mappings = groups
            .par_iter()
            .map(|group| {
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
mod tests {
    use super::*;
    use nose_il::{FileId, Interner, Lang};

    #[test]
    fn exact_group_proof_reuse_preserves_mixed_group_witnesses() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    return x * x + 7\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = crate::DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            ..Default::default()
        };
        let units = (0..9)
            .map(|i| {
                let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
                unit.path = format!("{i}.py");
                unit.exact_safe = i < 4 || i % 2 == 0;
                if i % 2 == 1 {
                    unit.anchors.clear();
                }
                unit
            })
            .collect::<Vec<_>>();
        let raw = vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]];
        let pairs = raw
            .iter()
            .flat_map(|members| {
                members.iter().enumerate().flat_map(|(index, &left)| {
                    members[index + 1..]
                        .iter()
                        .map(move |&right| (left, right, 0.75 + right as f64 / 100.0))
                })
            })
            .collect::<Vec<_>>();
        let accepted = AcceptedPairs::from(pairs.clone());
        let (groups, _) = crate::candidates::build_groups(
            &units,
            &accepted,
            &raw,
            &vec![None; units.len()],
            &opts,
            false,
        );
        assert_eq!(
            groups[0].witness.as_ref().unwrap().kind(),
            "exact-value-graph"
        );
        assert_ne!(
            groups[1].witness.as_ref().unwrap().kind(),
            "exact-value-graph"
        );
        let projection = Projection::new(&units, &raw, &groups, &accepted);
        assert!(projection.keys[..4]
            .iter()
            .all(|key| key.unwrap().2 == projection.keys[0].unwrap().2));
        assert_ne!(projection.exact[0], projection.exact[4]);
        assert_eq!(projection.keys[8], None);
        assert_eq!(projection.exact[8], None);
        let actual = projection.materialize(|_| true);
        for (group, members) in raw.iter().enumerate() {
            let expected = pairs
                .iter()
                .filter_map(|&(left, right, score)| {
                    let a = members.iter().position(|&index| index == left)?;
                    let b = members.iter().position(|&index| index == right)?;
                    Some(AcceptedEdge {
                        left: a as u32,
                        right: b as u32,
                        score: round3(score),
                        witness_kind: witness_kind(&[left, right], &units),
                    })
                })
                .collect::<Vec<_>>();
            let expected = crate::report::collapsed_accepted_edges(
                &groups[group],
                &sites::collapsed_sites(&groups[group]),
                &expected,
            );
            assert_eq!(
                actual[group].as_ref().unwrap().iter().collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn unmapped_bridge_does_not_claim_direct_site_evidence() {
        let mut projection = Projection {
            accepted: vec![(0, 1, 0.8), (1, 2, 0.8)].into(),
            keys: vec![Some((0, 0, 0)), None, Some((0, 1, 0))],
            exact: vec![None; 3],
            anchors: vec![Vec::new()],
            floor: 1,
            sizes: vec![2],
        };
        assert!(!projection.has_edges(0, &[0, 1, 2]));
        let edges = projection.materialize(|_| true)[0].take().unwrap();
        assert!(crate::AcceptedEdges::from_packed(edges).is_empty());
        projection.keys[1] = Some((0, 0, 0));
        assert!(projection.has_edges(0, &[0, 1, 2]));
        let edges = projection.materialize(|_| true)[0].take().unwrap();
        assert_eq!(crate::AcceptedEdges::from_packed(edges).len(), 1);
    }

    #[test]
    fn deferred_large_site_graph_matches_expanded_reference_after_sources_are_dropped() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    return x * x + 7\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = crate::DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            ..Default::default()
        };
        let units = (0..1500)
            .map(|i| {
                let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
                unit.path = format!("{i}.py");
                unit.exact_safe = i % 3 == 0;
                unit
            })
            .collect::<Vec<_>>();
        let pairs = (0..units.len() - 1)
            .map(|i| (i, i + 1, 0.75))
            .collect::<Vec<_>>();
        let accepted = AcceptedPairs::from(pairs.clone());
        let raw = vec![(0..units.len()).collect::<Vec<_>>()];
        let (groups, _) = crate::candidates::build_groups(
            &units,
            &accepted,
            &raw,
            &vec![None; units.len()],
            &opts,
            false,
        );
        let projected = projected_edges(&units, &raw, &groups, &accepted);
        let expanded = pairs
            .iter()
            .map(|&(left, right, score)| AcceptedEdge {
                left: left as u32,
                right: right as u32,
                score,
                witness_kind: witness_kind(&[left, right], &units),
            })
            .collect::<Vec<_>>();
        let expected = crate::report::collapsed_accepted_edges(
            &groups[0],
            &sites::collapsed_sites(&groups[0]),
            &expanded,
        );
        drop(units);
        drop(accepted);
        drop(groups);
        let GroupEdges::Sites(edges) = &projected[0] else {
            unreachable!()
        };
        assert!(!edges.is_empty());
        assert_eq!(edges.len(), expected.len());
        assert_eq!(edges.iter().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn projecting_before_materialization_keeps_the_same_direct_site_evidence() {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(0),
            "f.py",
            b"def f(x):\n    a = x * x\n    b = a + 1\n    return b * 2\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        let opts = crate::DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            ..Default::default()
        };
        let mut units = (0..48)
            .map(|i| {
                let mut unit = crate::units_of_file(&il, &interner, &opts).remove(0);
                unit.path = format!("{}.py", i % 4);
                unit.start_line = (i / 8 * 2) as u32;
                unit.end_line = unit.start_line + if i % 3 == 0 { 9 } else { 2 };
                unit.exact_safe = i % 3 == 0;
                if i % 5 == 0 {
                    unit.anchors.clear();
                }
                unit
            })
            .collect::<Vec<_>>();
        let pairs = (0..units.len())
            .flat_map(|left| {
                let units = &units;
                (left + 1..units.len()).filter_map(move |right| {
                    (!crate::locations::is_nested(&units[left], &units[right])).then_some((
                        left,
                        right,
                        0.7 + ((left + right) % 4) as f64 / 20.0,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let raw = vec![(0..units.len()).collect::<Vec<_>>()];
        let mut unrelated = crate::units_of_file(&il, &interner, &opts).remove(0);
        unrelated.path = "outside-group.py".into();
        unrelated.anchors.clear();
        unrelated.exact_safe = false;
        units.push(unrelated);
        let accepted = AcceptedPairs::from(pairs.clone());
        let (groups, _) = crate::candidates::build_groups(
            &units,
            &accepted,
            &raw,
            &vec![None; units.len()],
            &opts,
            false,
        );
        let projected = projected_edges(&units, &raw, &groups, &accepted);
        let expanded = pairs
            .iter()
            .map(|&(left, right, score)| AcceptedEdge {
                left: left as u32,
                right: right as u32,
                score: round3(score),
                witness_kind: witness_kind(&[left, right], &units),
            })
            .collect::<Vec<_>>();
        let GroupEdges::Members(actual) = &expanded_edges(&units, &raw, &accepted)[0] else {
            unreachable!()
        };
        assert_eq!(actual, &expanded);
        let sites = sites::collapsed_sites(&groups[0]);
        let collapse = |edges: &[AcceptedEdge]| {
            crate::report::collapsed_accepted_edges(&groups[0], &sites, edges)
        };
        let GroupEdges::Sites(projected) = &projected[0] else {
            unreachable!()
        };
        let projected = projected.iter().collect::<Vec<_>>();
        assert_eq!(projected, collapse(&expanded));
        assert!(projected.len() < expanded.len());
    }
}
