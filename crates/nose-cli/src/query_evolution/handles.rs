//! Resolve recorded live handles without treating navigation as correspondence.
use anyhow::{ensure, Result};
use nose_detect::regions::evolution::{AnalysisSnapshot, Change};
use std::collections::BTreeSet;

pub(super) fn select(
    rows: &mut Vec<&Change>,
    prefix: &str,
    snapshots: [&AnalysisSnapshot; 2],
) -> Result<()> {
    ensure!(snapshots.iter().any(|s| !s.family_handles.is_empty()),
        "live family handles were not recorded in these captures; browse with path~TEXT or create a new capture");
    let handles: BTreeSet<_> = snapshots
        .iter()
        .flat_map(|s| s.family_handles.keys())
        .filter(|handle| handle.starts_with(prefix))
        .collect();
    ensure!(!handles.is_empty(),
        "no captured live family id matching `{prefix}`; use an id from these captures or browse with path~TEXT");
    ensure!(
        handles.len() == 1,
        "ambiguous live family id `{prefix}`; use a longer prefix"
    );
    let handle = *handles.first().expect("one handle");
    let targets = snapshots.map(|s| {
        s.family_handles
            .get(handle)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    });
    ensure!(targets.iter().all(|ids| ids.len() <= 1),
        "live family id `{handle}` names multiple captured observations; browse with path~TEXT and select a change=ID");
    rows.retain(|row| {
        row.before.iter().any(|id| targets[0].contains(id))
            || row.after.iter().any(|id| targets[1].contains(id))
    });
    Ok(())
}
