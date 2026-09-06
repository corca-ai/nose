use crate::{
    candidates::{build_connected_groups, build_groups, structural_candidates},
    cluster::UnionFind,
    contiguous::Stream,
    detectors::Detector,
    locations::enclosing_units,
    model::{Dump, Metrics, Report},
    options::DetectOptions,
    reinvented::reinvented_helpers,
    units::UnitFeat,
};
use nose_il::Corpus;

mod batched;
mod features;
mod signatures;
pub use features::{
    corpus_features, corpus_features_with_normalized, file_stream, units_of_file, CorpusFeatures,
};
mod incremental_session;
mod output;
mod scoring;
mod stages;
mod timing;
pub use incremental_session::{
    detect_from_units_incremental_session_with_accepted_coverage,
    detect_from_units_incremental_with_accepted_coverage,
};
use output::{append_resolved_contiguous, build_pair_output, detection_dump};
use scoring::score_ordinary_candidates;
pub(crate) use scoring::{AcceptedPair, ScoredCandidate};
use stages::{ConnectedStage, DetectionStages, ResolvedDetectionStages};
use timing::StageTimer;

pub fn detect(corpus: &Corpus, opts: &DetectOptions, detector: &dyn Detector) -> Report {
    detect_with_dump_inner(corpus, opts, detector, DetectionOutput::REPORT).0
}

/// Product-query detection with compact direct accepted-edge provenance retained
/// through ranking. Keeping this control outside [`DetectOptions`] leaves the
/// normalize/extract hot path and its option layout identical for every caller.
pub fn detect_with_accepted_coverage(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_with_dump_inner(corpus, opts, detector, DetectionOutput::ACCEPTED_COVERAGE).0
}

/// Divergent-edit detection counterpart that also retains direct copy-paste-run
/// edges. Product query suppression intentionally keeps its historical structural-
/// coverage behavior; propagation targets need every enabled detector channel.
pub fn detect_with_direct_accepted_coverage(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_with_dump_inner(
        corpus,
        opts,
        detector,
        DetectionOutput::DIRECT_ACCEPTED_COVERAGE,
    )
    .0
}

#[derive(Clone, Copy)]
enum CoverageTrace {
    None,
    Structural,
    StructuralAndContiguous,
}

#[derive(Clone, Copy)]
enum DumpSelection {
    None,
    Candidates,
}

#[derive(Clone, Copy)]
struct DetectionOutput {
    coverage: CoverageTrace,
    dump: DumpSelection,
}

impl DetectionOutput {
    const REPORT: Self = Self {
        coverage: CoverageTrace::None,
        dump: DumpSelection::None,
    };
    const ACCEPTED_COVERAGE: Self = Self {
        coverage: CoverageTrace::Structural,
        dump: DumpSelection::None,
    };
    const DIRECT_ACCEPTED_COVERAGE: Self = Self {
        coverage: CoverageTrace::StructuralAndContiguous,
        dump: DumpSelection::None,
    };
    const DUMP: Self = Self {
        coverage: CoverageTrace::None,
        dump: DumpSelection::Candidates,
    };

    fn traces_structural_coverage(self) -> bool {
        !matches!(self.coverage, CoverageTrace::None)
    }

    fn traces_contiguous_coverage(self) -> bool {
        matches!(self.coverage, CoverageTrace::StructuralAndContiguous)
    }
}

struct DetectionRequest<'a> {
    units: &'a [UnitFeat],
    files: usize,
    streams: &'a [Stream],
    opts: &'a DetectOptions,
    detector: &'a dyn Detector,
    output: DetectionOutput,
}

fn score_fresh_connected(
    units: &[UnitFeat],
    scored: &[ScoredCandidate],
    accepted: &[AcceptedPair],
    opts: &DetectOptions,
) -> ConnectedStage {
    if !opts.connected_witnesses {
        return ConnectedStage::default();
    }
    ConnectedStage {
        cross_unit: score_connected_candidates(units, scored, accepted, opts, !opts.emit_pairs),
        same_unit: score_same_unit_candidates(units, opts, !opts.emit_pairs),
    }
}

pub fn detect_with_dump(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    detect_with_dump_inner(corpus, opts, detector, DetectionOutput::DUMP)
}

fn detect_with_dump_inner(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
    output: DetectionOutput,
) -> (Report, Dump) {
    let plan = opts.validate().expect("invalid detection options");
    let opts = &*plan;
    let mut clk = StageTimer::new();

    // Normalize each file and extract its units in one fused parallel pass — a file's
    // normalized IL stays hot in cache through extraction and is freed immediately,
    // rather than materializing the whole normalized corpus first.
    let CorpusFeatures {
        units,
        streams,
        files,
    } = corpus_features(corpus, opts);
    clk.lap("normalize+extract");

    // `detect_from_units` runs its own `StageTimer` for the detection sub-phases
    // (candidates/score/groups/contiguous), so no lap here — a single outer lap would
    // mislabel the whole call (group scoring dwarfs contiguous) as "contiguous".
    detect_from_units_inner(DetectionRequest {
        units: &units,
        files,
        streams: &streams,
        opts,
        detector,
        output,
    })
}

/// Run candidate-generation → scoring → clustering over already-built `units` (the
/// value-graph channel) and, when `opts.contiguous`, the copy-paste channel over
/// `streams` — producing the report and diagnostic dump. Split from unit/stream
/// extraction so a caller (the CLI's cache path) can supply both, built — and cached —
/// per file. `files` is the source file count, for the report's metrics only.
pub fn detect_from_units(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    detect_from_units_inner(DetectionRequest {
        units: &units,
        files,
        streams,
        opts,
        detector,
        output: DetectionOutput::DUMP,
    })
}

/// Cached-query counterpart to [`detect_with_accepted_coverage`].
pub fn detect_from_units_with_accepted_coverage(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_from_units_inner(DetectionRequest {
        units: &units,
        files,
        streams,
        opts,
        detector,
        output: DetectionOutput::ACCEPTED_COVERAGE,
    })
    .0
}

/// Borrowed-unit product entry point for watch sessions that retain unit caches
/// while choosing bounded scoring instead of a large persistent pair index.
pub fn detect_from_borrowed_units_with_accepted_coverage(
    units: &[UnitFeat],
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_from_units_inner(DetectionRequest {
        units,
        files,
        streams,
        opts,
        detector,
        output: DetectionOutput::ACCEPTED_COVERAGE,
    })
    .0
}

/// Cached-unit counterpart to [`detect_with_direct_accepted_coverage`].
/// Divergent-edit propagation needs contiguous copy-paste edges as well as the
/// structural accepted edges retained by the ordinary query surface.
pub fn detect_from_units_with_direct_accepted_coverage(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_from_units_inner(DetectionRequest {
        units: &units,
        files,
        streams,
        opts,
        detector,
        output: DetectionOutput::DIRECT_ACCEPTED_COVERAGE,
    })
    .0
}

fn detect_from_units_inner(request: DetectionRequest<'_>) -> (Report, Dump) {
    let plan = request.opts.validate().expect("invalid detection options");
    let request = DetectionRequest {
        opts: &plan,
        ..request
    };
    let mut clk = StageTimer::new();

    let stages = if matches!(request.output.dump, DumpSelection::None)
        && crate::prefers_batched_detection(request.units, request.opts)
    {
        clk.lap("candidates");
        batched::score(request.units, request.opts, request.detector)
    } else if request.opts.structural {
        // 3. LSH candidate generation. Semantic runs use the value-graph signature;
        //    near-duplicate runs also use shape signatures so Type-3 edits that
        //    change behavior-defining values still reach the scorer. When both
        //    channels run, score the union once.
        let candidates = structural_candidates(request.units, request.opts);
        clk.lap("candidates");

        // 4. Score candidates in parallel; keep accepted pairs.
        let (scored, accepted) = score_ordinary_candidates(
            request.units,
            &candidates,
            request.detector,
            request.opts.threshold,
        );
        DetectionStages::fresh(candidates, scored, accepted)
    } else {
        clk.lap("candidates");
        DetectionStages::fresh(Vec::new(), Vec::new(), Vec::new())
    };

    finish_detection(request, stages, &mut clk)
}

fn finish_detection(
    request: DetectionRequest<'_>,
    stages: DetectionStages,
    clk: &mut StageTimer,
) -> (Report, Dump) {
    let DetectionRequest {
        units,
        files,
        streams,
        opts,
        detector,
        output,
    } = request;
    let DetectionStages {
        candidates,
        candidate_count,
        scored,
        accepted,
        source,
    } = stages;
    let trace_accepted_coverage = output.traces_structural_coverage();
    let trace_contiguous_coverage = output.traces_contiguous_coverage();

    let ResolvedDetectionStages {
        raw_groups,
        connected,
        contiguous,
    } = source.resolve();
    let ConnectedStage {
        cross_unit: mut connected_accepted,
        same_unit: mut same_unit_accepted,
    } = connected.unwrap_or_else(|| score_fresh_connected(units, &scored, &accepted, opts));

    deduplicate_connected(&accepted, &mut connected_accepted, !opts.emit_pairs);
    deduplicate_same_unit(units, &mut same_unit_accepted, !opts.emit_pairs);
    connected_accepted.extend(same_unit_accepted);

    clk.lap("score");

    // 5. Cluster.
    let raw_groups = raw_groups.unwrap_or_else(|| {
        let mut union = UnionFind::new(units.len());
        for &(left, right, _) in &accepted {
            union.union(left, right);
        }
        union.groups(units.len())
    });
    clk.lap("cluster");

    let enclosing = enclosing_units(units);

    let duplicates = build_pair_output(
        units,
        &enclosing,
        &accepted,
        &connected_accepted,
        opts.emit_pairs,
    );

    let (mut groups, mut accepted_group_edges) = build_groups(
        units,
        &accepted,
        &raw_groups,
        &enclosing,
        opts,
        trace_accepted_coverage,
    );
    let (connected_groups, connected_edges) = build_connected_groups(
        units,
        &connected_accepted,
        &enclosing,
        opts,
        trace_accepted_coverage,
    );
    groups.extend(connected_groups);
    accepted_group_edges.extend(connected_edges);
    clk.lap("groups");

    let reinvented = if opts.structural {
        reinvented_helpers(units)
    } else {
        Vec::new()
    };
    let mut report = Report {
        tool: "nose",
        version: env!("CARGO_PKG_VERSION"),
        detector: detector.name().to_string(),
        metrics: Metrics {
            files,
            units: units.len(),
            candidate_pairs: candidate_count,
            accepted_pairs: accepted.len() + connected_accepted.len(),
            groups: groups.len(),
        },
        duplicates,
        groups,
        reinvented,
        accepted_group_edges,
    };

    // Copy-paste channel over the (raw-IL) token streams. Runs here, after the
    // value-graph channel, so both `detect` and the CLI's `--cache-dir` path produce
    // the same families — the cache supplies cached streams, otherwise this would
    // silently omit every contiguous clone.
    append_resolved_contiguous(
        &mut report,
        contiguous,
        streams,
        opts,
        units,
        trace_contiguous_coverage,
    );
    clk.lap("contiguous");

    let dump = if matches!(output.dump, DumpSelection::Candidates) {
        detection_dump(units, &candidates)
    } else {
        Dump::default()
    };
    (report, dump)
}

pub(crate) mod connected_pricing;
use connected_pricing::{
    deduplicate_connected, deduplicate_same_unit, score_connected_candidates,
    score_same_unit_candidates,
};
