use super::{AcceptedPair, ScoredCandidate};
use crate::candidates::ConnectedAccepted;

#[derive(Default)]
pub(super) struct ConnectedStage {
    pub(super) cross_unit: Vec<ConnectedAccepted>,
    pub(super) same_unit: Vec<ConnectedAccepted>,
}

pub(super) struct ContiguousStage {
    pub(super) groups: Vec<crate::Group>,
    pub(super) accepted_edges: Vec<Vec<crate::AcceptedEdge>>,
}

pub(super) struct ResolvedDetectionStages {
    pub(super) raw_groups: Option<Vec<Vec<usize>>>,
    pub(super) connected: Option<ConnectedStage>,
    pub(super) contiguous: Option<ContiguousStage>,
}

pub(super) struct DetectionStages {
    pub(super) candidates: Vec<(usize, usize)>,
    pub(super) candidate_count: usize,
    pub(super) scored: Vec<ScoredCandidate>,
    pub(super) accepted: Vec<AcceptedPair>,
    pub(super) source: DetectionStageSource,
}

pub(super) enum DetectionStageSource {
    Fresh,
    Incremental {
        raw_groups: Vec<Vec<usize>>,
        connected: ConnectedStage,
        contiguous: Option<ContiguousStage>,
    },
}

impl DetectionStages {
    pub(super) fn fresh(
        candidates: Vec<(usize, usize)>,
        scored: Vec<ScoredCandidate>,
        accepted: Vec<AcceptedPair>,
    ) -> Self {
        Self {
            candidate_count: candidates.len(),
            candidates,
            scored,
            accepted,
            source: DetectionStageSource::Fresh,
        }
    }
}

impl DetectionStageSource {
    pub(super) fn resolve(self) -> ResolvedDetectionStages {
        match self {
            Self::Fresh => ResolvedDetectionStages {
                raw_groups: None,
                connected: None,
                contiguous: None,
            },
            Self::Incremental {
                raw_groups,
                connected,
                contiguous,
            } => ResolvedDetectionStages {
                raw_groups: Some(raw_groups),
                connected: Some(connected),
                contiguous,
            },
        }
    }
}
