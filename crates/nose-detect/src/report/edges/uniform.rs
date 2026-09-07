//! Resolve a uniform source relation against ranking's already selected sites.
use super::{AcceptedEdges, Evidence, SiteEdgeBuilder, SiteEdges};
use crate::{Group, Loc};

pub(crate) fn uniform_source_edges(group: &Group, sites: &[Loc], score: f64) -> AcceptedEdges {
    let spans = sites
        .iter()
        .map(|site| (site.file.as_str(), site.start_line, site.end_line))
        .collect();
    let valid_spans = sites.iter().all(|site| {
        site.start_line <= site.end_line
            && u64::from(site.span_lines)
                == u64::from(site.end_line) - u64::from(site.start_line) + 1
    });
    if valid_spans && crate::locations::all_non_nested(spans) {
        return AcceptedEdges::from_packed(SiteEdges::complete(sites.len() as u32, score));
    }
    // Public report values can carry inconsistent cached span lengths. Preserve
    // source exclusions even if such input defeats ordinary site selection.
    let mapping = crate::report::sites::member_sites(group, sites);
    let mut edges = SiteEdgeBuilder::new(sites.len());
    for (left, a) in group.members.iter().enumerate() {
        for (right, b) in group.members.iter().enumerate().skip(left + 1) {
            let (Some(x), Some(y)) = (mapping[left], mapping[right]) else {
                continue;
            };
            if x == y
                || (a.file == b.file
                    && ((a.start_line <= b.start_line && a.end_line >= b.end_line)
                        || (b.start_line <= a.start_line && b.end_line >= a.end_line)))
            {
                continue;
            }
            edges.insert(
                x.min(y),
                x.max(y),
                Evidence {
                    score,
                    witness_kind: "exact-value-graph",
                },
            );
        }
    }
    AcceptedEdges::from_packed(edges.into_edges())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_sources_match_explicit_edges_for_aliases_and_malformed_report_spans() {
        let mut units = crate::test_support::scoring_units(32);
        for (index, unit) in units.iter_mut().enumerate() {
            unit.path = format!("{}.py", index % 3);
            unit.start_line = (index % 11) as u32;
            unit.end_line = unit.start_line + (index % 7) as u32;
        }
        for variation in 0..3 {
            let mut group = Group {
                score: 1.0,
                semantic_laws: Vec::new(),
                abstraction_witness: None,
                witness: None,
                members: units
                    .iter()
                    .map(|unit| crate::locations::loc_of(unit, None))
                    .collect(),
            };
            for member in &mut group.members {
                if variation == 1 {
                    member.span_lines = 1000;
                }
                if variation == 2 {
                    member.start_line = member.end_line + 1;
                    member.span_lines = 1;
                }
            }
            let sites = crate::report::sites::collapsed_sites(&group);
            for score in [-0.0_f64, 0.0, 0.875] {
                let mut pairs = Vec::new();
                for (left, a) in group.members.iter().enumerate() {
                    for (right, b) in group.members.iter().enumerate().skip(left + 1) {
                        if a.file != b.file
                            || !((a.start_line <= b.start_line && a.end_line >= b.end_line)
                                || (b.start_line <= a.start_line && b.end_line >= a.end_line))
                        {
                            pairs.push(crate::AcceptedEdge {
                                left: left as u32,
                                right: right as u32,
                                score,
                                witness_kind: "exact-value-graph",
                            });
                        }
                    }
                }
                let expected = crate::report::collapsed_accepted_edges(&group, &sites, &pairs);
                let actual = uniform_source_edges(&group, &sites, score);
                assert_eq!(actual.len(), expected.len());
                let key = |edge: crate::AcceptedEdge| {
                    (
                        edge.left,
                        edge.right,
                        edge.score.to_bits(),
                        edge.witness_kind,
                    )
                };
                assert_eq!(
                    actual.iter().map(key).collect::<Vec<_>>(),
                    expected.into_iter().map(key).collect::<Vec<_>>()
                );
            }
        }
    }
}
