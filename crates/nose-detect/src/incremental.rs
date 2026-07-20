//! Persistent, delete-capable detection state for cached queries.
//!
//! Stable unit identities let the focused submodules update candidate buckets,
//! pair scores, connected witnesses, structural components, and syntax components
//! without retaining process-local vector indexes across runs.

mod candidates;
mod components;
mod connected;
#[cfg(test)]
mod tests;

pub(crate) use candidates::{prepare, score};
pub(crate) use components::components;
pub(crate) use connected::connected;

use crate::candidates::ConnectedAccepted;
use crate::contiguous::IncrementalContiguousState;
use crate::orchestration::ScoredCandidate;
use serde::{Deserialize, Serialize};

const STATE_SCHEMA: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(crate) struct UnitKey([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct UnitPairKey {
    left: UnitKey,
    right: UnitKey,
}

impl UnitPairKey {
    fn new(left: UnitKey, right: UnitKey) -> Self {
        if left <= right {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
enum BucketKey {
    ValueBand(u64),
    ShapeBand(u64),
    ExactValue([u8; 32]),
    Anchor(u64),
}

#[derive(Serialize, Deserialize)]
struct CandidateBucket {
    key: BucketKey,
    members: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
struct StoredScore {
    left: u32,
    right: u32,
    bucket_count: u16,
    ordinary_score: Option<f64>,
}

impl StoredScore {
    fn pair(&self, units: &[UnitKey]) -> Option<UnitPairKey> {
        Some(UnitPairKey::new(
            *units.get(self.left as usize)?,
            *units.get(self.right as usize)?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
struct ConnectedEvaluationKey {
    pair: UnitPairKey,
    left_context: [u8; 32],
    right_context: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
struct SameUnitEvaluationKey {
    unit: UnitKey,
    file_context: [u8; 32],
}

#[derive(Serialize, Deserialize)]
struct StoredConnectedEvaluation {
    key: ConnectedEvaluationKey,
    accepted: Vec<StoredConnected>,
}

#[derive(Serialize, Deserialize)]
struct StoredSameUnitEvaluation {
    key: SameUnitEvaluationKey,
    accepted: Option<StoredConnected>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredConnected {
    left: UnitKey,
    right: UnitKey,
    score: f64,
    left_lines: (u32, u32),
    right_lines: (u32, u32),
    mapped_nodes: u32,
    holes: u32,
    complete_exit: bool,
    route: u8,
}

/// Binary-owned state. Fields stay private so schema evolution remains an engine
/// concern; callers only serialize/deserialize the value as one artifact.
#[derive(Default, Serialize, Deserialize)]
pub struct IncrementalDetectionState {
    schema: u32,
    units: Vec<UnitKey>,
    buckets: Vec<CandidateBucket>,
    scores: Vec<StoredScore>,
    components: Vec<Vec<UnitKey>>,
    connected: Vec<StoredConnectedEvaluation>,
    same_unit: Vec<StoredSameUnitEvaluation>,
    contiguous: Option<IncrementalContiguousState>,
}

impl IncrementalDetectionState {
    fn is_valid(&self) -> bool {
        self.schema == STATE_SCHEMA
            && self.scores.iter().all(|score| {
                (score.left as usize) < self.units.len()
                    && (score.right as usize) < self.units.len()
            })
            && self
                .buckets
                .iter()
                .flat_map(|bucket| &bucket.members)
                .all(|&member| (member as usize) < self.units.len())
    }
}

#[derive(Debug, Default, Serialize)]
pub struct IncrementalDetectionStats {
    pub schema: &'static str,
    pub state_hit: bool,
    pub units_reused: usize,
    pub units_added: usize,
    pub units_removed: usize,
    pub buckets_reused: usize,
    pub buckets_rebuilt: usize,
    pub scores_reused: usize,
    pub scores_evaluated: usize,
    pub connected_evaluations_reused: usize,
    pub connected_evaluations_evaluated: usize,
    pub components_reused: usize,
    pub components_rebuilt: usize,
    pub contiguous_streams_reused: usize,
    pub contiguous_streams_rebuilt: usize,
    pub contiguous_components_reused: usize,
    pub contiguous_components_rebuilt: usize,
}

impl IncrementalDetectionStats {
    pub(crate) fn new() -> Self {
        Self {
            schema: "nose.detection-incremental/v1",
            ..Self::default()
        }
    }
}

pub(crate) struct PreparedDetection {
    pub(crate) unit_keys: Vec<UnitKey>,
    pub(crate) candidates: Vec<(usize, usize)>,
    candidate_counts: Vec<u16>,
    buckets: Vec<CandidateBucket>,
    previous_scores: Vec<StoredScore>,
    previous_unit_keys: Vec<UnitKey>,
    previous_scores_aligned: bool,
    previous_components: Vec<Vec<UnitKey>>,
    previous_connected: Vec<StoredConnectedEvaluation>,
    previous_same_unit: Vec<StoredSameUnitEvaluation>,
    pub(crate) previous_contiguous: Option<IncrementalContiguousState>,
}

#[derive(Default)]
pub(crate) struct IncrementalConnected {
    pub(crate) accepted: Vec<ConnectedAccepted>,
    pub(crate) same_unit_accepted: Vec<ConnectedAccepted>,
    evaluations: Vec<StoredConnectedEvaluation>,
    same_unit_evaluations: Vec<StoredSameUnitEvaluation>,
}

pub(crate) fn finish_state(
    prepared: PreparedDetection,
    scored: &[ScoredCandidate],
    components: &[Vec<usize>],
    connected: IncrementalConnected,
    contiguous: Option<IncrementalContiguousState>,
) -> IncrementalDetectionState {
    let scores = scored
        .iter()
        .zip(prepared.candidate_counts)
        .map(|(candidate, bucket_count)| StoredScore {
            left: candidate.left as u32,
            right: candidate.right as u32,
            bucket_count,
            ordinary_score: candidate.ordinary_score,
        })
        .collect();
    let stored_components = components
        .iter()
        .map(|members| {
            members
                .iter()
                .map(|&member| prepared.unit_keys[member])
                .collect()
        })
        .collect();
    IncrementalDetectionState {
        schema: STATE_SCHEMA,
        units: prepared.unit_keys,
        buckets: prepared.buckets,
        scores,
        components: stored_components,
        connected: connected.evaluations,
        same_unit: connected.same_unit_evaluations,
        contiguous,
    }
}
