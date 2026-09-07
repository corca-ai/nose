//! Certify actual non-nested representatives for every canonical site pair.
use super::RowPairs;

pub(super) fn size(rows: &RowPairs, sites: &[(u32, usize, usize)]) -> Option<u32> {
    // A prefix with no coordinate holes can use the complete-graph iterator.
    if sites
        .iter()
        .enumerate()
        .any(|(index, &(site, _, _))| site as usize != index)
    {
        return None;
    }
    let spans = sites
        .iter()
        .map(|&(_, unit, _)| rows.locations[unit])
        .collect::<Vec<_>>();
    // Across files every pair is admitted. Within each file, strictly increasing
    // starts AND ends prove that no representative contains another one.
    if !crate::locations::all_non_nested(spans) {
        return None;
    }
    sites.len().try_into().ok()
}
