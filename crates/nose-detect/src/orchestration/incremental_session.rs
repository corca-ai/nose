use super::{
    finish_detection,
    stages::{ConnectedStage, ContiguousStage, DetectionStageSource, DetectionStages},
    timing::StageTimer,
    DetectionOutput, DetectionRequest,
};
use crate::{
    contiguous::{self, Stream},
    detectors::Detector,
    incremental::{self, IncrementalDetectionState, IncrementalDetectionStats},
    model::Report,
    options::DetectOptions,
    units::UnitFeat,
};

/// Cached-query entry point with persistent candidate membership and pair-score
/// reuse. The state is content-addressed by the CLI; this layer owns its schema.
pub fn detect_from_units_incremental_with_accepted_coverage(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    previous: Option<IncrementalDetectionState>,
    stable_unit_keys: Option<&[[u8; 32]]>,
) -> (
    Report,
    Option<IncrementalDetectionState>,
    IncrementalDetectionStats,
) {
    let (report, state, stats, state_changed) = detect_inner(
        &units,
        files,
        streams,
        opts,
        detector,
        previous,
        stable_unit_keys,
    );
    (report, state_changed.then_some(state), stats)
}

/// Long-lived-session counterpart to
/// [`detect_from_units_incremental_with_accepted_coverage`]. The caller retains
/// units and streams in memory between revisions, while this function always
/// returns the next reusable detection state even when a source edit is
/// semantically neutral.
pub fn detect_from_units_incremental_session_with_accepted_coverage(
    units: &[UnitFeat],
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    previous: Option<IncrementalDetectionState>,
    stable_unit_keys: Option<&[[u8; 32]]>,
) -> (Report, IncrementalDetectionState, IncrementalDetectionStats) {
    let (report, state, stats, _) = detect_inner(
        units,
        files,
        streams,
        opts,
        detector,
        previous,
        stable_unit_keys,
    );
    (report, state, stats)
}

fn detect_inner(
    units: &[UnitFeat],
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    previous: Option<IncrementalDetectionState>,
    stable_unit_keys: Option<&[[u8; 32]]>,
) -> (
    Report,
    IncrementalDetectionState,
    IncrementalDetectionStats,
    bool,
) {
    let mut clk = StageTimer::new();
    let mut stats = IncrementalDetectionStats::new();
    let mut prepared = incremental::prepare(units, stable_unit_keys, opts, previous, &mut stats);
    clk.lap("candidates");
    let (scored, accepted) =
        incremental::score(units, &prepared, detector, opts.threshold, &mut stats);
    let raw_groups = incremental::components(&prepared, &accepted, opts.threshold, &mut stats);
    let mut connected =
        incremental::connected(units, &prepared, &scored, &accepted, opts, &mut stats);
    let connected_stage = ConnectedStage {
        cross_unit: std::mem::take(&mut connected.accepted),
        same_unit: std::mem::take(&mut connected.same_unit_accepted),
    };
    let (contiguous_stage, contiguous_state) = if opts.contiguous {
        let (groups, edges, state, contiguous_stats) = contiguous::detect_incremental(
            streams,
            opts.contiguous_min_tokens,
            opts.contiguous_min_lines,
            false,
            prepared.previous_contiguous.take(),
        );
        stats.contiguous_streams_reused = contiguous_stats.streams_reused;
        stats.contiguous_streams_rebuilt = contiguous_stats.streams_rebuilt;
        stats.contiguous_components_reused = contiguous_stats.components_reused;
        stats.contiguous_components_rebuilt = contiguous_stats.components_rebuilt;
        (
            Some(ContiguousStage {
                groups,
                accepted_edges: edges,
            }),
            Some(state),
        )
    } else {
        (None, None)
    };
    let candidates = std::mem::take(&mut prepared.candidates);
    let state_changed = !stats.state_hit
        || stats.units_added > 0
        || stats.units_removed > 0
        || stats.buckets_rebuilt > 0
        || stats.scores_evaluated > 0
        || stats.connected_evaluations_evaluated > 0
        || stats.contiguous_streams_rebuilt > 0;
    let state =
        incremental::finish_state(prepared, &scored, &raw_groups, connected, contiguous_state);
    let report = finish_detection(
        DetectionRequest {
            units,
            files,
            streams,
            opts,
            detector,
            output: DetectionOutput::ACCEPTED_COVERAGE,
        },
        DetectionStages {
            candidates,
            scored,
            accepted,
            source: DetectionStageSource::Incremental {
                raw_groups,
                connected: connected_stage,
                contiguous: contiguous_stage,
            },
        },
        &mut clk,
    )
    .0;
    (report, state, stats, state_changed)
}
