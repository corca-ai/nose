use crate::{
    candidates::{
        build_connected_groups, build_groups, round3, structural_candidates, ConnectedAccepted,
        ConnectedRoute,
    },
    cluster::UnionFind,
    connected,
    contiguous::{self, Stream},
    detectors::{connected_witness_score, Detector},
    locations::{
        attach_enclosing_units, connected_loc_of, enclosing_unit_indices, enclosing_units,
        is_nested, loc_of,
    },
    model::{Dump, DupPair, EnclosingUnit, LineSpan, Metrics, Report, UnitLoc},
    options::DetectOptions,
    reinvented::reinvented_helpers,
    units::UnitFeat,
};
use nose_il::Corpus;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::incremental::{self, IncrementalDetectionState, IncrementalDetectionStats};

mod features;
pub use features::{
    corpus_features, corpus_features_with_normalized, file_stream, units_of_file, CorpusFeatures,
};
mod incremental_session;
pub use incremental_session::{
    detect_from_units_incremental_session_with_accepted_coverage,
    detect_from_units_incremental_with_accepted_coverage,
};

pub fn detect(corpus: &Corpus, opts: &DetectOptions, detector: &dyn Detector) -> Report {
    detect_with_dump_inner(corpus, opts, detector, false, false, false).0
}

/// Product-query detection with compact direct accepted-edge provenance retained
/// through ranking. Keeping this control outside [`DetectOptions`] leaves the
/// normalize/extract hot path and its option layout identical for every caller.
pub fn detect_with_accepted_coverage(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_with_dump_inner(corpus, opts, detector, true, false, false).0
}

/// Divergent-edit detection counterpart that also retains direct copy-paste-run
/// edges. Product query suppression intentionally keeps its historical structural-
/// coverage behavior; propagation targets need every enabled detector channel.
pub fn detect_with_direct_accepted_coverage(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_with_dump_inner(corpus, opts, detector, true, true, false).0
}

/// Per-stage wall-clock timing, printed to stderr when `NOSE_TIME` is set. A
/// zero-cost no-op otherwise (the `Instant`s are cheap; only the env check gates
/// printing).
struct StageTimer {
    on: bool,
    start: std::time::Instant,
    last: std::time::Instant,
}
impl StageTimer {
    fn new() -> Self {
        let now = std::time::Instant::now();
        StageTimer {
            on: std::env::var_os("NOSE_TIME").is_some(),
            start: now,
            last: now,
        }
    }
    fn lap(&mut self, stage: &str) {
        let now = std::time::Instant::now();
        if self.on {
            eprintln!(
                "  [time] {stage:<12} {:>7.1}ms   (total {:>7.1}ms)",
                now.duration_since(self.last).as_secs_f64() * 1e3,
                now.duration_since(self.start).as_secs_f64() * 1e3,
            );
        }
        self.last = now;
    }
}

pub fn detect_with_dump(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    detect_with_dump_inner(corpus, opts, detector, false, false, true)
}

fn detect_with_dump_inner(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
    trace_accepted_coverage: bool,
    trace_contiguous_coverage: bool,
    build_dump: bool,
) -> (Report, Dump) {
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
    detect_from_units_inner(
        units,
        files,
        &streams,
        opts,
        detector,
        trace_accepted_coverage,
        trace_contiguous_coverage,
        build_dump,
    )
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
    detect_from_units_inner(units, files, streams, opts, detector, false, false, true)
}

/// Cached-query counterpart to [`detect_with_accepted_coverage`].
pub fn detect_from_units_with_accepted_coverage(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_from_units_inner(units, files, streams, opts, detector, true, false, false).0
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
    detect_from_units_inner(units, files, streams, opts, detector, true, true, false).0
}

#[allow(clippy::too_many_arguments)]
fn detect_from_units_inner(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    trace_accepted_coverage: bool,
    trace_contiguous_coverage: bool,
    build_dump: bool,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    let (candidates, scored, accepted) = if opts.structural {
        // 3. LSH candidate generation. Semantic runs use the value-graph signature;
        //    near-duplicate runs also use shape signatures so Type-3 edits that
        //    change behavior-defining values still reach the scorer. When both
        //    channels run, score the union once.
        let candidates = structural_candidates(&units, opts);
        clk.lap("candidates");

        // 4. Score candidates in parallel; keep accepted pairs.
        let (scored, accepted) =
            score_ordinary_candidates(&units, &candidates, detector, opts.threshold);
        (candidates, scored, accepted)
    } else {
        clk.lap("candidates");
        (Vec::new(), Vec::new(), Vec::new())
    };

    finish_detection(
        &units,
        files,
        streams,
        opts,
        detector,
        &candidates,
        &scored,
        accepted,
        None,
        None,
        None,
        trace_accepted_coverage,
        trace_contiguous_coverage,
        build_dump,
        &mut clk,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_detection(
    units: &[UnitFeat],
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    candidates: &[(usize, usize)],
    scored: &[ScoredCandidate],
    accepted: Vec<AcceptedPair>,
    raw_groups: Option<Vec<Vec<usize>>>,
    connected_override: Option<(Vec<ConnectedAccepted>, Vec<ConnectedAccepted>)>,
    contiguous_override: Option<(Vec<crate::Group>, Vec<Vec<crate::AcceptedEdge>>)>,
    trace_accepted_coverage: bool,
    trace_contiguous_coverage: bool,
    build_dump: bool,
    clk: &mut StageTimer,
) -> (Report, Dump) {
    let (mut connected_accepted, mut same_unit_accepted) = if let Some(cached) = connected_override
    {
        cached
    } else if opts.connected_witnesses {
        (
            score_connected_candidates(units, scored, &accepted, opts.threshold, !opts.emit_pairs),
            score_same_unit_candidates(units, opts.threshold, !opts.emit_pairs),
        )
    } else {
        (Vec::new(), Vec::new())
    };

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
            candidate_pairs: candidates.len(),
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
    if let Some((groups, edges)) = contiguous_override {
        append_contiguous_output(&mut report, groups, edges, units, trace_contiguous_coverage);
    } else {
        append_contiguous_groups(&mut report, streams, opts, units, trace_contiguous_coverage);
    }
    clk.lap("contiguous");

    let dump = if build_dump {
        detection_dump(units, candidates)
    } else {
        Dump::default()
    };
    (report, dump)
}

fn detection_dump(units: &[UnitFeat], candidates: &[(usize, usize)]) -> Dump {
    Dump {
        units: units
            .iter()
            .map(|u| UnitLoc {
                path: u.path.clone(),
                start_line: u.start_line,
                end_line: u.end_line,
                lang: u.lang.name().to_string(),
                name: u.name.clone(),
            })
            .collect(),
        candidates: candidates
            .iter()
            .map(|&(i, j)| (i as u32, j as u32))
            .collect(),
    }
}

pub(crate) type AcceptedPair = (usize, usize, f64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScoredCandidate {
    pub(crate) left: usize,
    pub(crate) right: usize,
    /// Nested pairs are intentionally not scored by the ordinary detector.
    pub(crate) ordinary_score: Option<f64>,
}

fn score_ordinary_candidates(
    units: &[UnitFeat],
    candidates: &[(usize, usize)],
    detector: &dyn Detector,
    threshold: f64,
) -> (Vec<ScoredCandidate>, Vec<AcceptedPair>) {
    let scored = candidates
        .par_iter()
        .map(|&(left, right)| ScoredCandidate {
            left,
            right,
            ordinary_score: (!is_nested(&units[left], &units[right]))
                .then(|| detector.score(&units[left], &units[right])),
        })
        .collect::<Vec<_>>();
    let accepted = scored
        .iter()
        .filter_map(|candidate| {
            candidate
                .ordinary_score
                .filter(|&score| score >= threshold)
                .map(|score| (candidate.left, candidate.right, score))
        })
        .collect();
    (scored, accepted)
}

fn build_pair_output(
    units: &[UnitFeat],
    enclosing: &[Option<EnclosingUnit>],
    ordinary: &[AcceptedPair],
    connected: &[ConnectedAccepted],
    emit_pairs: bool,
) -> Vec<DupPair> {
    if !emit_pairs {
        return Vec::new();
    }
    let mut output = ordinary
        .iter()
        .map(|&(left, right, score)| DupPair {
            left: loc_of(&units[left], enclosing[left].clone()),
            right: loc_of(&units[right], enclosing[right].clone()),
            score: round3(score),
            cross_language: units[left].lang != units[right].lang,
        })
        .collect::<Vec<_>>();
    output.extend(connected.iter().map(|pair| {
        let left = connected_loc_of(
            &units[pair.left],
            enclosing[pair.left].clone(),
            pair.witness.left_lines,
            pair.witness.mapped_nodes,
        );
        let right = connected_loc_of(
            &units[pair.right],
            enclosing[pair.right].clone(),
            pair.witness.right_lines,
            pair.witness.mapped_nodes,
        );
        DupPair {
            left,
            right,
            score: round3(pair.score),
            cross_language: units[pair.left].lang != units[pair.right].lang,
        }
    }));
    output.sort_by(|left, right| right.score.total_cmp(&left.score));
    output
}

pub(crate) mod connected_pricing;
use connected_pricing::{
    deduplicate_connected, deduplicate_same_unit, score_connected_candidates,
    score_same_unit_candidates,
};

fn append_contiguous_groups(
    report: &mut Report,
    streams: &[Stream],
    opts: &DetectOptions,
    units: &[UnitFeat],
    trace_accepted_coverage: bool,
) {
    if !opts.contiguous {
        return;
    }
    let (extra, accepted_edges) = contiguous::detect(
        streams,
        opts.contiguous_min_tokens,
        opts.contiguous_min_lines,
        trace_accepted_coverage,
    );
    append_contiguous_output(
        report,
        extra,
        accepted_edges,
        units,
        trace_accepted_coverage,
    );
}

fn append_contiguous_output(
    report: &mut Report,
    mut groups: Vec<crate::Group>,
    accepted_edges: Vec<Vec<crate::AcceptedEdge>>,
    units: &[UnitFeat],
    trace_accepted_coverage: bool,
) {
    attach_enclosing_units(&mut groups, units);
    report.metrics.groups += groups.len();
    report.groups.extend(groups);
    if trace_accepted_coverage {
        report.accepted_group_edges.extend(accepted_edges);
    }
}
