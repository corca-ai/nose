use super::{AcceptedPair, ConnectedAccepted, ScoredCandidate};

pub(super) type ConnectedStage = (Vec<ConnectedAccepted>, Vec<ConnectedAccepted>);
type ContiguousStage = (Vec<crate::Group>, Vec<Vec<crate::AcceptedEdge>>);

pub(super) struct DetectionStages {
    pub(super) candidates: Vec<(usize, usize)>,
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
            candidates,
            scored,
            accepted,
            source: DetectionStageSource::Fresh,
        }
    }
}

impl DetectionStageSource {
    pub(super) fn into_cached(
        self,
    ) -> (
        Option<Vec<Vec<usize>>>,
        Option<ConnectedStage>,
        Option<ContiguousStage>,
    ) {
        match self {
            Self::Fresh => (None, None, None),
            Self::Incremental {
                raw_groups,
                connected,
                contiguous,
            } => (Some(raw_groups), Some(connected), contiguous),
        }
    }
}
