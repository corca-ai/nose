use super::*;
use crate::cluster::UnionFind;
use std::collections::{BTreeSet, HashMap};

pub(crate) fn components(
    prepared: &PreparedDetection,
    accepted: &[(usize, usize, f64)],
    threshold: f64,
    stats: &mut IncrementalDetectionStats,
) -> Vec<Vec<usize>> {
    if prepared.previous_components.is_empty() {
        let groups = clean_components(prepared.unit_keys.len(), accepted);
        stats.components_rebuilt = groups.len();
        return groups;
    }
    let changes = component_changes(prepared, accepted, threshold);
    let mut groups = prepared
        .previous_components
        .iter()
        .enumerate()
        .filter(|(component, _)| !changes.dirty.contains(component))
        .map(|(_, members)| {
            members
                .iter()
                .map(|member| changes.current_index[member])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    stats.components_reused = groups.len();
    let (rebuilt, rebuilt_work) = rebuild_components(&changes, accepted);
    stats.components_rebuilt = changes.dirty.len().max(rebuilt_work);
    groups.extend(rebuilt);
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable_by_key(|group| group[0]);
    groups
}

struct ComponentChanges {
    current_index: HashMap<UnitKey, usize>,
    dirty: BTreeSet<usize>,
    affected: BTreeSet<UnitKey>,
}

fn component_changes(
    prepared: &PreparedDetection,
    accepted: &[(usize, usize, f64)],
    threshold: f64,
) -> ComponentChanges {
    let current_index = prepared
        .unit_keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<HashMap<_, _>>();
    let previous_component = prepared
        .previous_components
        .iter()
        .enumerate()
        .flat_map(|(component, members)| {
            members
                .iter()
                .copied()
                .map(move |member| (member, component))
        })
        .collect::<HashMap<_, _>>();
    let previous_edges = prepared
        .previous_scores
        .iter()
        .filter_map(|score| {
            score
                .ordinary_score
                .filter(|&value| value >= threshold)
                .and_then(|_| score.pair(&prepared.previous_unit_keys))
        })
        .collect::<BTreeSet<_>>();
    let current_edges = accepted
        .iter()
        .map(|&(left, right, _)| {
            UnitPairKey::new(prepared.unit_keys[left], prepared.unit_keys[right])
        })
        .collect::<BTreeSet<_>>();
    let mut dirty = prepared
        .previous_components
        .iter()
        .enumerate()
        .filter(|(_, members)| {
            members
                .iter()
                .any(|member| !current_index.contains_key(member))
        })
        .map(|(component, _)| component)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::new();
    for pair in previous_edges.symmetric_difference(&current_edges) {
        for key in [pair.left, pair.right] {
            affected.insert(key);
            if let Some(&component) = previous_component.get(&key) {
                dirty.insert(component);
            }
        }
    }
    for &component in &dirty {
        affected.extend(
            prepared.previous_components[component]
                .iter()
                .copied()
                .filter(|key| current_index.contains_key(key)),
        );
    }
    ComponentChanges {
        current_index,
        dirty,
        affected,
    }
}

fn rebuild_components(
    changes: &ComponentChanges,
    accepted: &[(usize, usize, f64)],
) -> (Vec<Vec<usize>>, usize) {
    let affected_indices = changes
        .affected
        .iter()
        .filter_map(|key| changes.current_index.get(key).copied())
        .collect::<Vec<_>>();
    let local_index = affected_indices
        .iter()
        .copied()
        .enumerate()
        .map(|(local, current)| (current, local))
        .collect::<HashMap<_, _>>();
    let mut union = UnionFind::new(affected_indices.len());
    for &(left, right, _) in accepted {
        if let (Some(&local_left), Some(&local_right)) =
            (local_index.get(&left), local_index.get(&right))
        {
            union.union(local_left, local_right);
        }
    }
    let rebuilt = union
        .groups(affected_indices.len())
        .into_iter()
        .map(|members| {
            members
                .into_iter()
                .map(|member| affected_indices[member])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let work = rebuilt.len();
    (rebuilt, work)
}

fn clean_components(unit_count: usize, accepted: &[(usize, usize, f64)]) -> Vec<Vec<usize>> {
    let mut union = UnionFind::new(unit_count);
    for &(left, right, _) in accepted {
        union.union(left, right);
    }
    let mut groups = union.groups(unit_count);
    for group in &mut groups {
        group.sort_unstable();
    }
    groups.sort_unstable_by_key(|group| group[0]);
    groups
}
