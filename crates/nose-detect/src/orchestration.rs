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
    minhash,
    model::{Dump, DupPair, EnclosingUnit, LineSpan, Metrics, Report, UnitLoc},
    options::DetectOptions,
    reinvented::reinvented_helpers,
    units::{self, UnitFeat},
};
use nose_il::{Corpus, Il, Interner};
use nose_normalize::NormalizeOptions;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Build one file's syntax-channel token stream from its (raw) IL. Exposed so the
/// CLI's `--cache-dir` can cache it per file and pass it to [`detect_from_units`] — the
/// counterpart to [`units_of_file`] for the syntax channel.
pub fn file_stream(il: &Il, interner: &Interner) -> Stream {
    contiguous::stream(il, interner)
}

pub fn detect(corpus: &Corpus, opts: &DetectOptions, detector: &dyn Detector) -> Report {
    detect_with_dump(corpus, opts, detector).0
}

/// Product-query detection with compact direct accepted-edge provenance retained
/// through ranking. Keeping this control outside [`DetectOptions`] leaves the
/// normalize/extract hot path and its option layout identical for every caller.
pub fn detect_with_accepted_coverage(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> Report {
    detect_with_dump_inner(corpus, opts, detector, true).0
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

/// Like [`detect`] but also returns the unit/candidate [`Dump`] for diagnostics.
/// Normalize one file and extract its detection units. The resulting [`UnitFeat`]s
/// are interner-independent (every feature is a content-derived hash), so a caller
/// may pass a throwaway per-file interner — which is exactly what makes caching a
/// file's units by its source-content hash sound.
pub fn units_of_file(il: &Il, interner: &Interner, opts: &DetectOptions) -> Vec<UnitFeat> {
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    extract_units_of_file(il, interner, opts, &norm_opts, &seeds)
}

/// Keep the normalization/extraction body out of the Rayon closure. This path
/// is large and hot; sharing one non-inlined implementation with the cached
/// per-file entry point avoids code-layout-sensitive copies while preserving
/// the fused normalize-then-extract lifetime.
#[inline(never)]
fn extract_units_of_file(
    il: &Il,
    interner: &Interner,
    opts: &DetectOptions,
    norm_opts: &NormalizeOptions,
    seeds: &[u64],
) -> Vec<UnitFeat> {
    if units::raw_il_is_empty_module(il) || units::large_test_file(il) {
        return Vec::new();
    }
    let n = nose_normalize::normalize(il, interner, norm_opts);
    let block_units = units::block_units_for_file(&n, opts);
    units::extract(
        &n,
        interner,
        seeds,
        opts.min_lines,
        opts.min_tokens,
        block_units,
        units::ExtractFeatures {
            shape_features: opts.shape_features,
            abstraction_witnesses: opts.abstraction_witnesses,
            connected_witnesses: opts.connected_witnesses,
        },
    )
}

pub fn detect_with_dump(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    detect_with_dump_inner(corpus, opts, detector, false)
}

fn detect_with_dump_inner(
    corpus: &Corpus,
    opts: &DetectOptions,
    detector: &dyn Detector,
    trace_accepted_coverage: bool,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    // Normalize each file and extract its units in one fused parallel pass — a file's
    // normalized IL stays hot in cache through extraction and is freed immediately,
    // rather than materializing the whole normalized corpus first.
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    // Normalize each file once; extract its units and (when enabled) its contiguous
    // token stream from the same hot normalized IL.
    let per_file: Vec<(Vec<UnitFeat>, Option<Stream>)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let units = if opts.structural {
                extract_units_of_file(il, &corpus.interner, opts, &norm_opts, &seeds)
            } else {
                Vec::new()
            };
            // Build the contiguous stream from the *raw* IL, not the normalized one:
            // alpha-renaming is function-scoped, so a copy-pasted block's variable
            // cids depend on its enclosing function and identical blocks diverge.
            // Raw tokens (names content-hashed by `node_tag`) are stable across files
            // — matching jscpd's name-based copy-paste. Renamed Type-2/3/4 is the
            // structural channel's job.
            let stream = opts
                .contiguous
                .then(|| contiguous::stream(il, &corpus.interner));
            (units, stream)
        })
        .collect();
    // `UnitFeat` is large enough that repeatedly growing the aggregate vector
    // copies a meaningful amount of memory on repositories with many files.
    // The parallel pass already owns every per-file length, so reserve the
    // exact aggregate capacities before moving the results into place.
    let unit_count = per_file.iter().map(|(units, _)| units.len()).sum();
    let stream_count = per_file
        .iter()
        .filter(|(_, stream)| stream.is_some())
        .count();
    let mut units: Vec<UnitFeat> = Vec::with_capacity(unit_count);
    let mut streams: Vec<Stream> = Vec::with_capacity(stream_count);
    for (u, s) in per_file {
        units.extend(u);
        if let Some(s) = s {
            streams.push(s);
        }
    }
    clk.lap("normalize+extract");

    // `detect_from_units` runs its own `StageTimer` for the detection sub-phases
    // (candidates/score/groups/contiguous), so no lap here — a single outer lap would
    // mislabel the whole call (group scoring dwarfs contiguous) as "contiguous".
    detect_from_units_inner(
        units,
        corpus.files.len(),
        &streams,
        opts,
        detector,
        trace_accepted_coverage,
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
    detect_from_units_inner(units, files, streams, opts, detector, false)
}

/// Cached-query counterpart to [`detect_with_accepted_coverage`].
pub fn detect_from_units_with_accepted_coverage(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
) -> (Report, Dump) {
    detect_from_units_inner(units, files, streams, opts, detector, true)
}

fn detect_from_units_inner(
    units: Vec<UnitFeat>,
    files: usize,
    streams: &[Stream],
    opts: &DetectOptions,
    detector: &dyn Detector,
    trace_accepted_coverage: bool,
) -> (Report, Dump) {
    let mut clk = StageTimer::new();

    let (candidates, accepted, mut connected_accepted, mut same_unit_accepted) = if opts.structural
    {
        // 3. LSH candidate generation. Semantic runs use the value-graph signature;
        //    near-duplicate runs also use shape signatures so Type-3 edits that
        //    change behavior-defining values still reach the scorer. When both
        //    channels run, score the union once.
        let candidates = structural_candidates(&units, opts);
        clk.lap("candidates");

        // 4. Score candidates in parallel; keep accepted pairs.
        let (scored, accepted) =
            score_ordinary_candidates(&units, &candidates, detector, opts.threshold);
        let connected = if opts.connected_witnesses {
            score_connected_candidates(&units, &scored, &accepted, opts.threshold, !opts.emit_pairs)
        } else {
            Vec::new()
        };
        let same_unit = if opts.connected_witnesses {
            score_same_unit_candidates(&units, opts.threshold, !opts.emit_pairs)
        } else {
            Vec::new()
        };
        (candidates, accepted, connected, same_unit)
    } else {
        clk.lap("candidates");
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };

    deduplicate_connected(&accepted, &mut connected_accepted, !opts.emit_pairs);
    deduplicate_same_unit(&units, &mut same_unit_accepted, !opts.emit_pairs);
    connected_accepted.extend(same_unit_accepted);

    clk.lap("score");

    // 5. Cluster.
    let mut uf = UnionFind::new(units.len());
    for &(i, j, _) in &accepted {
        uf.union(i, j);
    }
    let raw_groups = uf.groups(units.len());
    clk.lap("cluster");

    let enclosing = enclosing_units(&units);

    let duplicates = build_pair_output(
        &units,
        &enclosing,
        &accepted,
        &connected_accepted,
        opts.emit_pairs,
    );

    let (mut groups, mut accepted_group_edges) = build_groups(
        &units,
        &accepted,
        &mut uf,
        &raw_groups,
        &enclosing,
        opts,
        trace_accepted_coverage,
    );
    let (connected_groups, connected_edges) = build_connected_groups(
        &units,
        &connected_accepted,
        &enclosing,
        opts,
        trace_accepted_coverage,
    );
    groups.extend(connected_groups);
    accepted_group_edges.extend(connected_edges);
    clk.lap("groups");

    let reinvented = if opts.structural {
        reinvented_helpers(&units)
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
    append_contiguous_groups(&mut report, streams, opts, &units);
    clk.lap("contiguous");

    let dump = Dump {
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
    };

    (report, dump)
}

type AcceptedPair = (usize, usize, f64);

#[derive(Clone, Copy, Debug)]
struct ScoredCandidate {
    left: usize,
    right: usize,
    /// Nested pairs are intentionally not scored by the ordinary detector.
    ordinary_score: Option<f64>,
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

mod connected_pricing;
use connected_pricing::{
    deduplicate_connected, deduplicate_same_unit, score_connected_candidates,
    score_same_unit_candidates,
};

fn append_contiguous_groups(
    report: &mut Report,
    streams: &[Stream],
    opts: &DetectOptions,
    units: &[UnitFeat],
) {
    if !opts.contiguous {
        return;
    }
    let mut extra = contiguous::detect(
        streams,
        opts.contiguous_min_tokens,
        opts.contiguous_min_lines,
    );
    attach_enclosing_units(&mut extra, units);
    report.metrics.groups += extra.len();
    report.groups.extend(extra);
}
