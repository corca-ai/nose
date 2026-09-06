//! Project direct evidence to ranking's canonical sites, preserving the winning
//! accepted score and witness. The coordinate domain is explicit in GroupEdges.
use super::{exact_claim_eligible, round3, witness_kind, AcceptedPairs, Group, UnitFeat};
use crate::{
    report::{
        edges::{Evidence, SiteEdgeBuilder},
        sites,
    },
    AcceptedEdge, GroupEdges,
};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

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
    // Keep the ordinary parallel classification path within a bounded allocation.
    let pairs = accepted.iter().collect::<Vec<_>>();
    let classified = pairs
        .par_iter()
        .filter_map(|&(left, right, score)| {
            let (Some((group, a)), Some((other, b))) = (position[left], position[right]) else {
                return None;
            };
            debug_assert_eq!(group, other);
            (group == other).then(|| {
                (
                    group,
                    AcceptedEdge {
                        left: a,
                        right: b,
                        score: round3(score),
                        witness_kind: witness_kind(&[left, right], units),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    let mut edges = vec![Vec::new(); raw_groups.len()];
    for (group, edge) in classified {
        edges[group].push(edge);
    }
    edges.into_iter().map(GroupEdges::Members).collect()
}

fn projected_edges(
    units: &[UnitFeat],
    raw_groups: &[Vec<usize>],
    groups: &[Group],
    accepted: &AcceptedPairs,
) -> Vec<GroupEdges> {
    let mut position = vec![None; units.len()];
    let mut edges = Vec::new();
    for (group_id, (members, group)) in raw_groups.iter().zip(groups).enumerate() {
        let collapsed = sites::collapsed_sites(group);
        edges.push(SiteEdgeBuilder::new(collapsed.len()));
        let sites = sites::member_sites(group, &collapsed);
        for (&unit, site) in members.iter().zip(sites) {
            position[unit] = site.map(|site| (group_id, site));
        }
    }
    let exact = exact_classes(units);
    let mut witnesses = FxHashMap::default();
    let keys = units
        .iter()
        .zip(&position)
        .map(|(unit, position)| {
            let next = witnesses.len();
            let class = *witnesses
                .entry((
                    &unit.value,
                    unit.exact_safe,
                    unit.anchors
                        .iter()
                        .map(|a| (a.hash, a.weight))
                        .collect::<Vec<_>>(),
                ))
                .or_insert(next);
            position.map(|(group, site)| (group, site, class))
        })
        .collect::<Vec<_>>();
    let mut kinds = vec![None; witnesses.len()];
    accepted.visit_site_evidence(&keys, |(left, right, score)| {
        let (Some((group, a)), Some((other, b))) = (position[left], position[right]) else {
            return;
        };
        debug_assert_eq!(group, other);
        if group != other || a == b {
            return;
        }
        let key = (a.min(b), a.max(b));
        let score = round3(score);
        let previous = edges[group].best(key.0, key.1);
        if previous.is_some_and(|edge| edge.score > score) {
            return;
        }
        let is_exact = exact[left].is_some() && exact[left] == exact[right];
        let best_kind = if is_exact {
            "exact-value-graph"
        } else {
            "shared-sub-dag"
        };
        if previous.is_some_and(|edge| edge.score == score && edge.witness_kind <= best_kind) {
            return;
        }
        let left_class = keys[left].unwrap().2;
        let right_class = keys[right].unwrap().2;
        let kind = if is_exact {
            best_kind
        } else if let Some((class, kind)) =
            kinds[right_class].filter(|&(class, _)| class == left_class)
        {
            debug_assert_eq!(class, left_class);
            kind
        } else {
            let kind =
                if super::shared_anchor_weight(&units[left].anchors, &units[right].anchors) > 0 {
                    "shared-sub-dag"
                } else {
                    "structural-similarity"
                };
            kinds[right_class] = Some((left_class, kind));
            kind
        };
        if previous.is_none_or(|edge| score > edge.score || kind < edge.witness_kind) {
            edges[group].insert(
                key.0,
                key.1,
                Evidence {
                    score,
                    witness_kind: kind,
                },
            );
        }
    });
    edges.into_iter().map(SiteEdgeBuilder::finish).collect()
}

fn exact_classes(units: &[UnitFeat]) -> Vec<Option<usize>> {
    let mut classes = FxHashMap::default();
    units
        .iter()
        .map(|unit| {
            if !exact_claim_eligible(unit) {
                return None;
            }
            let next = classes.len();
            Some(*classes.entry(&unit.value).or_insert(next))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, Interner, Lang};

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
        let units = (0..48)
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
