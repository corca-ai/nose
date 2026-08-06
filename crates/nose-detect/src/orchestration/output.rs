use super::{stages::ContiguousStage, AcceptedPair};
use crate::{
    candidates::{round3, ConnectedAccepted},
    contiguous::{self, Stream},
    locations::{attach_enclosing_units, connected_loc_of, loc_of},
    model::{Dump, DupPair, EnclosingUnit, Report, UnitLoc},
    options::DetectOptions,
    units::UnitFeat,
};

pub(super) fn detection_dump(units: &[UnitFeat], candidates: &[(usize, usize)]) -> Dump {
    Dump {
        units: units
            .iter()
            .map(|unit| UnitLoc {
                path: unit.path.clone(),
                start_line: unit.start_line,
                end_line: unit.end_line,
                lang: unit.lang.name().to_string(),
                name: unit.name.clone(),
            })
            .collect(),
        candidates: candidates
            .iter()
            .map(|&(left, right)| (left as u32, right as u32))
            .collect(),
    }
}

pub(super) fn build_pair_output(
    units: &[UnitFeat],
    enclosing: &[Option<EnclosingUnit>],
    ordinary: &[AcceptedPair],
    connected: &[ConnectedAccepted],
    emit_pairs: bool,
) -> Vec<DupPair> {
    if !emit_pairs {
        return Vec::new();
    }
    let mut output = ordinary
        .iter()
        .map(|&(left, right, score)| DupPair {
            left: loc_of(&units[left], enclosing[left].clone()),
            right: loc_of(&units[right], enclosing[right].clone()),
            score: round3(score),
            cross_language: units[left].lang != units[right].lang,
        })
        .collect::<Vec<_>>();
    output.extend(connected.iter().map(|pair| {
        let left = connected_loc_of(
            &units[pair.left],
            enclosing[pair.left].clone(),
            pair.witness.left_lines,
            pair.witness.mapped_nodes,
        );
        let right = connected_loc_of(
            &units[pair.right],
            enclosing[pair.right].clone(),
            pair.witness.right_lines,
            pair.witness.mapped_nodes,
        );
        DupPair {
            left,
            right,
            score: round3(pair.score),
            cross_language: units[pair.left].lang != units[pair.right].lang,
        }
    }));
    output.sort_by(|left, right| right.score.total_cmp(&left.score));
    output
}

pub(super) fn append_resolved_contiguous(
    report: &mut Report,
    resolved: Option<ContiguousStage>,
    streams: &[Stream],
    opts: &DetectOptions,
    units: &[UnitFeat],
    trace_accepted_coverage: bool,
) {
    if let Some(ContiguousStage {
        groups,
        accepted_edges,
    }) = resolved
    {
        append_contiguous_output(
            report,
            groups,
            accepted_edges,
            units,
            trace_accepted_coverage,
        );
    } else if opts.contiguous {
        let (groups, accepted_edges) = contiguous::detect(
            streams,
            opts.contiguous_min_tokens,
            opts.contiguous_min_lines,
            trace_accepted_coverage,
        );
        append_contiguous_output(
            report,
            groups,
            accepted_edges,
            units,
            trace_accepted_coverage,
        );
    }
}

fn append_contiguous_output(
    report: &mut Report,
    mut groups: Vec<crate::Group>,
    accepted_edges: Vec<Vec<crate::AcceptedEdge>>,
    units: &[UnitFeat],
    trace_accepted_coverage: bool,
) {
    attach_enclosing_units(&mut groups, units);
    report.metrics.groups += groups.len();
    report.groups.extend(groups);
    if trace_accepted_coverage {
        report.accepted_group_edges.extend(accepted_edges);
    }
}
