use super::{AcceptedPair, ScoredCandidate};
use crate::{
    candidates::{ConnectedAccepted, ConnectedRoute},
    connected,
    locations::{enclosing_unit_indices, is_nested},
    model::LineSpan,
    units::UnitFeat,
};
use rayon::prelude::*;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

pub(super) fn score_connected_candidates(
    units: &[UnitFeat],
    candidates: &[ScoredCandidate],
    ordinary: &[AcceptedPair],
    opts: &crate::DetectOptions,
    bound_product_work: bool,
) -> Vec<ConnectedAccepted> {
    let ordinary_pairs = ordinary
        .iter()
        .map(|&(left, right, _)| (left, right))
        .collect::<HashSet<_>>();
    let enclosing_indices = enclosing_unit_indices(units);
    let mut units_by_file: HashMap<&str, Vec<usize>> = HashMap::default();
    for (index, unit) in units.iter().enumerate() {
        units_by_file
            .entry(unit.path.as_str())
            .or_default()
            .push(index);
    }
    let unit_paths = units
        .iter()
        .map(|unit| unit.path.as_str())
        .collect::<Vec<_>>();
    let unit_weights = units
        .iter()
        .map(|unit| unit.connected_tokens.len())
        .collect::<Vec<_>>();
    let candidate_indices = connected_seed_indices(
        candidates,
        &unit_paths,
        &unit_weights,
        opts.threshold,
        bound_product_work,
    );
    let connected = candidate_indices
        .par_iter()
        .flat_map_iter(|&index| {
            let ScoredCandidate { left, right, .. } = candidates[index];
            evaluate_connected_candidate(
                units,
                &enclosing_indices,
                units_by_file
                    .get(units[left].path.as_str())
                    .map_or(&[], Vec::as_slice),
                left,
                right,
                ordinary_pairs.contains(&(left, right)),
                opts,
            )
        })
        .collect::<Vec<_>>();
    connected
}

pub(super) fn score_same_unit_candidates(
    units: &[UnitFeat],
    opts: &crate::DetectOptions,
    bound_product_work: bool,
) -> Vec<ConnectedAccepted> {
    same_unit_seed_indices(units, bound_product_work)
        .par_iter()
        .filter_map(|&index| {
            let witness = connected::same_unit_witness(&units[index].connected_tokens)?;
            let score = opts.scoring.anchor_score(witness.mapped_nodes);
            (score >= opts.threshold).then_some(ConnectedAccepted {
                left: index,
                right: index,
                score,
                witness,
                route: ConnectedRoute::SameUnit,
            })
        })
        .collect()
}

pub(crate) fn same_unit_seed_indices(units: &[UnitFeat], bound_product_work: bool) -> Vec<usize> {
    const MIN_SELF_UNIT_NODES: usize = 20;
    const PRODUCT_PER_FILE_CAP: usize = 2;
    const PRODUCT_GLOBAL_CAP: usize = 4_096;

    let eligible = units
        .iter()
        .enumerate()
        .filter(|(_, unit)| {
            unit.fragment_kind.is_none()
                && matches!(
                    unit.kind,
                    nose_il::UnitKind::Function | nose_il::UnitKind::Method
                )
                && unit.connected_tokens.len() >= MIN_SELF_UNIT_NODES
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if !bound_product_work {
        return eligible;
    }

    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::default();
    for index in eligible {
        let best = by_file.entry(units[index].path.as_str()).or_default();
        best.push(index);
        best.sort_unstable_by(|&left, &right| {
            units[right]
                .connected_tokens
                .len()
                .cmp(&units[left].connected_tokens.len())
                .then_with(|| left.cmp(&right))
        });
        best.truncate(PRODUCT_PER_FILE_CAP);
    }
    let mut selected = by_file.into_values().flatten().collect::<Vec<_>>();
    selected.sort_unstable_by(|&left, &right| {
        units[right]
            .connected_tokens
            .len()
            .cmp(&units[left].connected_tokens.len())
            .then_with(|| left.cmp(&right))
    });
    selected.truncate(PRODUCT_GLOBAL_CAP);
    selected.sort_unstable();
    selected
}

mod seed_selection;
pub(super) use seed_selection::{path_classes, SeedSelection, MIN_PRODUCT_SEED_NODES};

/// Product seed selection retains the strongest near misses and nested routes.
/// The raw audit interface evaluates every candidate instead.
pub(crate) fn connected_seed_indices(
    candidates: &[ScoredCandidate],
    unit_paths: &[&str],
    unit_weights: &[usize],
    threshold: f64,
    bound_product_work: bool,
) -> Vec<usize> {
    if !bound_product_work {
        return (0..candidates.len()).collect();
    }
    let paths = path_classes(unit_paths);
    let mut selected = SeedSelection::new(&paths, unit_weights, threshold);
    for (index, &candidate) in candidates.iter().enumerate() {
        selected.push(candidate, (index, 0), index);
    }
    selected.finish()
}

pub(crate) fn evaluate_connected_candidate(
    units: &[UnitFeat],
    enclosing_indices: &[Option<usize>],
    same_file: &[usize],
    raw_left: usize,
    raw_right: usize,
    raw_accepted: bool,
    opts: &crate::DetectOptions,
) -> Vec<ConnectedAccepted> {
    if is_nested(&units[raw_left], &units[raw_right]) {
        return connected_descendant_pairs(units, raw_left, raw_right, same_file, opts);
    }

    // A child/block candidate may seed its two distinct enclosing units. If both
    // children share one enclosing unit, keep the child endpoints as two locations.
    let mut left = enclosing_indices[raw_left].unwrap_or(raw_left);
    let mut right = enclosing_indices[raw_right].unwrap_or(raw_right);
    let mut left_constraint = LineSpan::new(units[raw_left].start_line, units[raw_left].end_line);
    let mut right_constraint =
        LineSpan::new(units[raw_right].start_line, units[raw_right].end_line);
    if left == right {
        left = raw_left;
        right = raw_right;
    }
    if left > right {
        std::mem::swap(&mut left, &mut right);
        std::mem::swap(&mut left_constraint, &mut right_constraint);
    }
    let already_accepted = (left, right) == (raw_left, raw_right) && raw_accepted;
    let connected = if already_accepted || left == right || is_nested(&units[left], &units[right]) {
        None
    } else {
        accepted_connected_pair(
            units,
            left,
            right,
            left_constraint,
            right_constraint,
            false,
            opts,
        )
    };
    connected.into_iter().collect()
}

fn accepted_connected_pair(
    units: &[UnitFeat],
    left: usize,
    right: usize,
    left_constraint: LineSpan,
    right_constraint: LineSpan,
    nested_route: bool,
    opts: &crate::DetectOptions,
) -> Option<ConnectedAccepted> {
    if units[left].lang != units[right].lang {
        return None;
    }
    let witness = connected::connected_witness(
        &units[left].connected_tokens,
        &units[right].connected_tokens,
        left_constraint,
        right_constraint,
    )?;
    let score = opts.scoring.anchor_score(witness.mapped_nodes);
    (score >= opts.threshold).then_some(ConnectedAccepted {
        left,
        right,
        score,
        witness,
        route: if nested_route {
            ConnectedRoute::Nested
        } else if witness.complete_exit && witness.holes == 0 {
            ConnectedRoute::CompleteExit
        } else {
            ConnectedRoute::Mapped
        },
    })
}

/// Several child seeds can prove the same enclosing pair. Keep one deterministic strongest
/// witness and discard pairs already accepted by ordinary scoring.
pub(super) fn deduplicate_connected(
    ordinary: &[AcceptedPair],
    connected: &mut Vec<ConnectedAccepted>,
    bound_product_output: bool,
) {
    let ordinary_pairs: HashSet<(usize, usize)> = ordinary
        .iter()
        .map(|&(left, right, _)| (left.min(right), left.max(right)))
        .collect();
    connected.retain(|pair| {
        !ordinary_pairs.contains(&(pair.left.min(pair.right), pair.left.max(pair.right)))
    });
    connected.sort_unstable_by(|left, right| {
        (left.left, left.right)
            .cmp(&(right.left, right.right))
            .then_with(|| right.witness.mapped_nodes.cmp(&left.witness.mapped_nodes))
            .then_with(|| left.witness.holes.cmp(&right.witness.holes))
            .then_with(|| left.witness.left_lines.cmp(&right.witness.left_lines))
    });
    connected.dedup_by_key(|pair| (pair.left, pair.right));
    if bound_product_output {
        retain_strongest_connected_routes(connected);
    }
}

fn retain_strongest_connected_routes(connected: &mut Vec<ConnectedAccepted>) {
    const MAPPED_CAP: usize = 32;
    const EXIT_CAP: usize = 32;
    const NESTED_CAP: usize = 32;
    const SAME_UNIT_CAP: usize = 32;
    connected.sort_unstable_by(|left, right| {
        right
            .witness
            .mapped_nodes
            .cmp(&left.witness.mapped_nodes)
            .then_with(|| left.witness.holes.cmp(&right.witness.holes))
            .then_with(|| (left.left, left.right).cmp(&(right.left, right.right)))
    });
    let (mut mapped, mut exit, mut nested, mut same_unit) = (0, 0, 0, 0);
    connected.retain(|pair| {
        let (count, cap) = match pair.route {
            ConnectedRoute::Mapped => (&mut mapped, MAPPED_CAP),
            ConnectedRoute::CompleteExit => (&mut exit, EXIT_CAP),
            ConnectedRoute::Nested => (&mut nested, NESTED_CAP),
            ConnectedRoute::SameUnit => (&mut same_unit, SAME_UNIT_CAP),
        };
        *count += 1;
        *count <= cap
    });
}

pub(super) fn deduplicate_same_unit(
    units: &[UnitFeat],
    accepted: &mut Vec<ConnectedAccepted>,
    bound_product_output: bool,
) {
    accepted.sort_unstable_by(|left, right| {
        right
            .witness
            .mapped_nodes
            .cmp(&left.witness.mapped_nodes)
            .then_with(|| left.witness.holes.cmp(&right.witness.holes))
            .then_with(|| left.left.cmp(&right.left))
            .then_with(|| left.witness.left_lines.cmp(&right.witness.left_lines))
            .then_with(|| left.witness.right_lines.cmp(&right.witness.right_lines))
    });
    accepted.dedup_by_key(|pair| (pair.left, pair.witness.left_lines, pair.witness.right_lines));
    if bound_product_output {
        let mut files = HashSet::default();
        accepted.retain(|pair| {
            units
                .get(pair.left)
                .is_none_or(|unit| files.insert(unit.path.as_str()))
        });
        accepted.truncate(32);
    }
}

/// A nested raw candidate is never itself reportable. It may, however, be the only LSH
/// evidence that reaches two disjoint siblings below the same container. Search only that
/// bounded subtree, require like-kind endpoints, and keep every resulting edge pair-local.
fn connected_descendant_pairs(
    units: &[UnitFeat],
    left: usize,
    right: usize,
    same_file: &[usize],
    opts: &crate::DetectOptions,
) -> Vec<ConnectedAccepted> {
    let (container_index, focus) = if strictly_contains(&units[left], &units[right]) {
        (left, right)
    } else if strictly_contains(&units[right], &units[left]) {
        (right, left)
    } else {
        return Vec::new();
    };
    let container = &units[container_index];
    let focus_unit = &units[focus];
    let inside = same_file
        .iter()
        .copied()
        .filter(|&index| {
            let unit = &units[index];
            index != container_index
                && contains_or_same(container, unit)
                && !unit.connected_tokens.is_empty()
        })
        .collect::<Vec<_>>();
    let mut accepted = Vec::new();
    for (offset, &i) in inside.iter().enumerate() {
        for &j in &inside[offset + 1..] {
            if units[i].lang != units[j].lang
                || units[i].kind != units[j].kind
                || is_nested(&units[i], &units[j])
                || (!contains_or_same(focus_unit, &units[i])
                    && !contains_or_same(focus_unit, &units[j]))
            {
                continue;
            }
            if let Some(pair) = accepted_connected_pair(
                units,
                i,
                j,
                LineSpan::new(units[i].start_line, units[i].end_line),
                LineSpan::new(units[j].start_line, units[j].end_line),
                true,
                opts,
            ) {
                accepted.push(pair);
            }
        }
    }
    accepted
}

fn contains_or_same(parent: &UnitFeat, child: &UnitFeat) -> bool {
    parent.path == child.path
        && parent.start_line <= child.start_line
        && parent.end_line >= child.end_line
}

fn strictly_contains(parent: &UnitFeat, child: &UnitFeat) -> bool {
    contains_or_same(parent, child)
        && (parent.start_line < child.start_line || parent.end_line > child.end_line)
}

#[cfg(test)]
mod tests;
