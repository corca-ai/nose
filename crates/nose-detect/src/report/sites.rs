use super::paths::{overlap_frac, span_lines};
use crate::{Group, Loc};

pub(crate) fn collapsed_sites(group: &Group) -> Vec<Loc> {
    // Collapse co-located units to one refactoring site. Block extraction yields a
    // function unit *and* inner blocks that overlap it, and near-identical spans can
    // differ by a line; all of these are one place to refactor, not several. Keep the
    // largest enclosing span per file and drop anything that substantially overlaps it.
    let mut locs = group.members.clone();
    // Largest span first (within a file) so the enclosing unit wins.
    locs.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| span_lines(b).cmp(&span_lines(a)))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    let mut kept: Vec<Loc> = Vec::with_capacity(locs.len());
    let mut kept_by_file: rustc_hash::FxHashMap<String, Vec<usize>> =
        rustc_hash::FxHashMap::default();
    for l in locs {
        let subsumed = kept_by_file
            .get(l.file.as_str())
            .is_some_and(|idxs| idxs.iter().any(|&i| overlap_frac(&kept[i], &l) >= 0.5));
        if !subsumed {
            let file = l.file.clone();
            kept.push(l);
            kept_by_file.entry(file).or_default().push(kept.len() - 1);
        }
    }
    let mut locs = kept;
    locs.sort_by_key(|b| std::cmp::Reverse(span_lines(b)));
    locs
}

pub(crate) fn member_sites(group: &Group, collapsed_sites: &[Loc]) -> Vec<Option<u32>> {
    let mut sites_by_file: rustc_hash::FxHashMap<&str, Vec<(u32, &Loc)>> =
        rustc_hash::FxHashMap::default();
    for (index, site) in collapsed_sites.iter().enumerate() {
        sites_by_file
            .entry(site.file.as_str())
            .or_default()
            .push((index as u32, site));
    }
    let site_of: Vec<Option<u32>> = group
        .members
        .iter()
        .map(|member| {
            sites_by_file
                .get(member.file.as_str())
                .into_iter()
                .flatten()
                .map(|&(index, site)| (index, overlap_frac(site, member)))
                .filter(|(_, overlap)| *overlap >= 0.5)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        })
        .collect();
    site_of
}
