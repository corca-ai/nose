//! Exact source equality is a cheap Type-1 floor independent of unit size.
use super::*;

pub(crate) fn detect_whole_sources(
    streams: &[Stream],
    min_tokens: usize,
    min_lines: u32,
    existing: &[crate::Group],
    trace_edges: bool,
) -> (Vec<crate::Group>, Vec<Vec<crate::AcceptedEdge>>) {
    let mut first = FxHashMap::default();
    let mut locs = Vec::new();
    let mut pairs = Vec::new();
    for (index, stream) in streams.iter().enumerate() {
        if !stream.complete_source
            || !stream.root_is_module
            || stream.tags.len() < min_tokens
            || !stream.op.iter().any(|&op| op)
        {
            continue;
        }
        let Some(source) = &stream.source else {
            continue;
        };
        let end = source.line_count();
        if end < min_lines {
            continue;
        }
        let key = (stream.lang, source.digest());
        let (representative, representative_loc) = first.entry(key).or_insert((index, None));
        if *representative == index || streams[*representative].path == stream.path {
            continue;
        }
        let left = *representative_loc.get_or_insert_with(|| {
            let id = locs.len();
            locs.push(whole_seed(*representative, &streams[*representative]));
            id
        });
        let right = locs.len();
        locs.push(whole_seed(index, stream));
        pairs.push((left, right));
    }
    let (groups, edges) = groups_from_primitives(locs, pairs, streams, trace_edges);
    if groups.is_empty() {
        return (groups, edges);
    }
    // A whole function already representing these complete source buffers is
    // the more precise boundary. Keep it rather than add a duplicate file row.
    let mut covered = FxHashMap::<_, Vec<usize>>::default();
    for (index, group) in existing.iter().enumerate() {
        for member in &group.members {
            if let Some(region) = &member.source_region {
                covered
                    .entry((
                        member.file.as_str(),
                        region.source_digest,
                        member.start_line,
                        member.end_line,
                    ))
                    .or_default()
                    .push(index);
            }
        }
    }
    let mut kept = Vec::new();
    let mut kept_edges = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        let memberships = group
            .members
            .iter()
            .map(|member| {
                member.source_region.as_ref().and_then(|region| {
                    covered.get(&(
                        member.file.as_str(),
                        region.source_digest,
                        member.start_line,
                        member.end_line,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let redundant = memberships.first().and_then(|ids| *ids).is_some_and(|ids| {
            ids.iter().any(|id| {
                memberships
                    .iter()
                    .all(|members| members.is_some_and(|members| members.contains(id)))
            })
        });
        if !redundant {
            kept.push(group);
            if trace_edges {
                kept_edges.push(edges[index].clone());
            }
        }
    }
    (kept, kept_edges)
}

fn whole_seed(index: usize, stream: &Stream) -> LocSeed {
    let end = stream
        .source
        .as_ref()
        .map_or(0, |source| source.line_count());
    LocSeed {
        stream: index,
        start_line: 1,
        end_line: end,
        sem: stream.tags.len(),
        source_lines: (1, end),
    }
}
