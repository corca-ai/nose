use super::*;
use crate::candidates::{ConnectedAccepted, ConnectedRoute};
use crate::locations::enclosing_unit_indices;
use crate::orchestration::connected_pricing::{
    connected_seed_indices, evaluate_connected_candidate, same_unit_seed_indices,
};
use crate::{DetectOptions, UnitFeat};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) fn connected(
    units: &[UnitFeat],
    prepared: &PreparedDetection,
    scored: &[ScoredCandidate],
    ordinary: &[(usize, usize, f64)],
    opts: &DetectOptions,
    stats: &mut IncrementalDetectionStats,
) -> IncrementalConnected {
    if !opts.connected_witnesses {
        return IncrementalConnected::default();
    }
    let contexts = file_contexts(units, &prepared.unit_keys);
    let current_index = prepared
        .unit_keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<HashMap<_, _>>();
    let (accepted, evaluations) = connected_pair_evaluations(
        units,
        prepared,
        scored,
        ordinary,
        opts,
        &contexts,
        &current_index,
        stats,
    );
    let (same_unit_accepted, same_unit_evaluations) =
        same_unit_evaluations(units, prepared, opts, &contexts, &current_index, stats);
    IncrementalConnected {
        accepted,
        same_unit_accepted,
        evaluations,
        same_unit_evaluations,
    }
}

#[allow(clippy::too_many_arguments)]
fn connected_pair_evaluations(
    units: &[UnitFeat],
    prepared: &PreparedDetection,
    scored: &[ScoredCandidate],
    ordinary: &[(usize, usize, f64)],
    opts: &DetectOptions,
    contexts: &[[u8; 32]],
    current_index: &HashMap<UnitKey, usize>,
    stats: &mut IncrementalDetectionStats,
) -> (Vec<ConnectedAccepted>, Vec<StoredConnectedEvaluation>) {
    let previous = prepared
        .previous_connected
        .iter()
        .map(|evaluation| (evaluation.key, &evaluation.accepted))
        .collect::<HashMap<_, _>>();
    let unit_paths = units
        .iter()
        .map(|unit| unit.path.as_str())
        .collect::<Vec<_>>();
    let unit_weights = units
        .iter()
        .map(|unit| unit.connected_tokens.len())
        .collect::<Vec<_>>();
    let selected = connected_seed_indices(
        scored,
        &unit_paths,
        &unit_weights,
        opts.threshold,
        !opts.emit_pairs,
    );
    let enclosing = enclosing_unit_indices(units);
    let mut units_by_file = HashMap::<&str, Vec<usize>>::new();
    for (index, unit) in units.iter().enumerate() {
        units_by_file
            .entry(unit.path.as_str())
            .or_default()
            .push(index);
    }
    let ordinary_pairs = ordinary
        .iter()
        .map(|&(left, right, _)| (left, right))
        .collect::<HashSet<_>>();
    let mut accepted = Vec::new();
    let mut evaluations = Vec::with_capacity(selected.len());
    for candidate_index in selected {
        let candidate = scored[candidate_index];
        let key = connected_evaluation_key(
            candidate.left,
            candidate.right,
            &prepared.unit_keys,
            contexts,
        );
        let values = if let Some(stored) = previous.get(&key) {
            stats.connected_evaluations_reused += 1;
            stored
                .iter()
                .filter_map(|value| restore_connected(value, current_index))
                .collect::<Vec<_>>()
        } else {
            stats.connected_evaluations_evaluated += 1;
            evaluate_connected_candidate(
                units,
                &enclosing,
                units_by_file
                    .get(units[candidate.left].path.as_str())
                    .map_or(&[], Vec::as_slice),
                candidate.left,
                candidate.right,
                ordinary_pairs.contains(&(candidate.left, candidate.right)),
                opts,
            )
        };
        evaluations.push(StoredConnectedEvaluation {
            key,
            accepted: values
                .iter()
                .map(|value| store_connected(value, &prepared.unit_keys))
                .collect(),
        });
        accepted.extend(values);
    }
    (accepted, evaluations)
}

#[allow(clippy::too_many_arguments)]
fn same_unit_evaluations(
    units: &[UnitFeat],
    prepared: &PreparedDetection,
    opts: &DetectOptions,
    contexts: &[[u8; 32]],
    current_index: &HashMap<UnitKey, usize>,
    stats: &mut IncrementalDetectionStats,
) -> (Vec<ConnectedAccepted>, Vec<StoredSameUnitEvaluation>) {
    let previous_same = prepared
        .previous_same_unit
        .iter()
        .map(|evaluation| (evaluation.key, evaluation.accepted.as_ref()))
        .collect::<HashMap<_, _>>();
    let same_seeds = same_unit_seed_indices(units, !opts.emit_pairs);
    let mut same_unit_accepted = Vec::new();
    let mut same_unit_evaluations = Vec::with_capacity(same_seeds.len());
    for index in same_seeds {
        let key = SameUnitEvaluationKey {
            unit: prepared.unit_keys[index],
            file_context: contexts[index],
        };
        let value = if let Some(stored) = previous_same.get(&key) {
            stats.connected_evaluations_reused += 1;
            stored.and_then(|value| restore_connected(value, current_index))
        } else {
            stats.connected_evaluations_evaluated += 1;
            crate::connected::same_unit_witness(&units[index].connected_tokens).and_then(
                |witness| {
                    let score = opts.scoring.anchor_score(witness.mapped_nodes);
                    (score >= opts.threshold).then_some(ConnectedAccepted {
                        left: index,
                        right: index,
                        score,
                        witness,
                        route: ConnectedRoute::SameUnit,
                    })
                },
            )
        };
        same_unit_evaluations.push(StoredSameUnitEvaluation {
            key,
            accepted: value
                .as_ref()
                .map(|value| store_connected(value, &prepared.unit_keys)),
        });
        same_unit_accepted.extend(value);
    }
    (same_unit_accepted, same_unit_evaluations)
}

fn file_contexts(units: &[UnitFeat], keys: &[UnitKey]) -> Vec<[u8; 32]> {
    let mut by_path = BTreeMap::<&str, Vec<UnitKey>>::new();
    for (unit, &key) in units.iter().zip(keys) {
        by_path.entry(unit.path.as_str()).or_default().push(key);
    }
    let digests = by_path
        .into_iter()
        .map(|(path, members)| {
            let mut bytes = Vec::with_capacity(members.len() * 32);
            for member in members {
                bytes.extend_from_slice(&member.0);
            }
            (path, digest(b"nose.incremental-file-context.v1", &bytes))
        })
        .collect::<HashMap<_, _>>();
    units
        .iter()
        .map(|unit| digests[unit.path.as_str()])
        .collect()
}

fn connected_evaluation_key(
    left: usize,
    right: usize,
    keys: &[UnitKey],
    contexts: &[[u8; 32]],
) -> ConnectedEvaluationKey {
    if keys[left] <= keys[right] {
        ConnectedEvaluationKey {
            pair: UnitPairKey::new(keys[left], keys[right]),
            left_context: contexts[left],
            right_context: contexts[right],
        }
    } else {
        ConnectedEvaluationKey {
            pair: UnitPairKey::new(keys[left], keys[right]),
            left_context: contexts[right],
            right_context: contexts[left],
        }
    }
}

fn store_connected(value: &ConnectedAccepted, keys: &[UnitKey]) -> StoredConnected {
    StoredConnected {
        left: keys[value.left],
        right: keys[value.right],
        score: value.score,
        left_lines: value.witness.left_lines,
        right_lines: value.witness.right_lines,
        mapped_nodes: value.witness.mapped_nodes,
        holes: value.witness.holes,
        complete_exit: value.witness.complete_exit,
        route: match value.route {
            ConnectedRoute::Mapped => 1,
            ConnectedRoute::CompleteExit => 2,
            ConnectedRoute::Nested => 3,
            ConnectedRoute::SameUnit => 4,
        },
    }
}

fn restore_connected(
    value: &StoredConnected,
    current_index: &HashMap<UnitKey, usize>,
) -> Option<ConnectedAccepted> {
    let route = match value.route {
        1 => ConnectedRoute::Mapped,
        2 => ConnectedRoute::CompleteExit,
        3 => ConnectedRoute::Nested,
        4 => ConnectedRoute::SameUnit,
        _ => return None,
    };
    Some(ConnectedAccepted {
        left: *current_index.get(&value.left)?,
        right: *current_index.get(&value.right)?,
        score: value.score,
        witness: crate::ConnectedWitness {
            left_lines: value.left_lines,
            right_lines: value.right_lines,
            mapped_nodes: value.mapped_nodes,
            holes: value.holes,
            complete_exit: value.complete_exit,
        },
        route,
    })
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}
