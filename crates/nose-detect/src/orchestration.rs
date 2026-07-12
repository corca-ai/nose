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
use nose_il::{Corpus, Il, Interner, NodeKind};
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
    if raw_il_is_empty_module(il) || units::large_test_file(il) {
        return Vec::new();
    }
    let n = nose_normalize::normalize(il, interner, norm_opts);
    let block_units = block_units_for_file(&n, opts);
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

fn raw_il_is_empty_module(il: &Il) -> bool {
    il.units.is_empty() && il.kind(il.root) == NodeKind::Module && il.children(il.root).is_empty()
}

/// Keep whole function/method/class units for cross-file matches, but do not expand
/// every nested `if`/loop into extra block units inside dependency code or very
/// large files. The syntax channel still covers exact copy-paste spans there.
const LARGE_FILE_BLOCK_NODE_CUTOFF: usize = 5_000;

fn block_units_for_file(il: &Il, opts: &DetectOptions) -> bool {
    opts.block_units
        && !is_bulk_dependency_path(&il.meta.path)
        && il.nodes.len() <= LARGE_FILE_BLOCK_NODE_CUTOFF
}

fn is_bulk_dependency_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    [
        "vendor/",
        "third_party/",
        "third-party/",
        "/deps/",
        "node_modules/",
        "/dist/",
        "/build/",
        "/external/",
        ".min.",
        ".pb.",
        "_pb2",
        ".g.dart",
        ".d.ts",
        "generated/",
        "/gen/",
        ".generated.",
    ]
    .iter()
    .any(|m| p.contains(m))
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

    let (candidates, accepted, mut connected_accepted) = if opts.structural {
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
        (candidates, accepted, connected)
    } else {
        clk.lap("candidates");
        (Vec::new(), Vec::new(), Vec::new())
    };

    deduplicate_connected(&accepted, &mut connected_accepted, !opts.emit_pairs);

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

fn score_connected_candidates(
    units: &[UnitFeat],
    candidates: &[ScoredCandidate],
    ordinary: &[AcceptedPair],
    threshold: f64,
    bound_product_work: bool,
) -> Vec<ConnectedAccepted> {
    let ordinary_pairs = ordinary
        .iter()
        .map(|&(left, right, _)| (left, right))
        .collect::<HashSet<_>>();
    let enclosing_indices = enclosing_unit_indices(units);
    let mut units_by_file: HashMap<&str, Vec<usize>> = HashMap::new();
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
        threshold,
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
                threshold,
            )
        })
        .collect::<Vec<_>>();
    connected
}

/// The raw audit interface evaluates every seed. Product queries instead price the
/// expensive pair-local proof only for the strongest ordinary near misses, while always
/// retaining nested seeds because they are the sole route to disjoint descendants.
/// Endpoints below 18 nodes cannot meet the matcher's lowest complete-exit threshold.
fn connected_seed_indices(
    candidates: &[ScoredCandidate],
    unit_paths: &[&str],
    unit_weights: &[usize],
    threshold: f64,
    bound_product_work: bool,
) -> Vec<usize> {
    const MIN_PRODUCT_SEED_NODES: usize = 18;
    const PRODUCT_GENERAL_SEED_CAP: usize = 2_048;
    const PRODUCT_NESTED_SEED_CAP: usize = 512;
    const PRODUCT_NESTED_PER_FILE_CAP: usize = 64;
    const PRODUCT_CROSS_FILE_PER_FILE_CAP: usize = 8;

    if !bound_product_work {
        return (0..candidates.len()).collect();
    }
    let mut nested = Vec::new();
    let mut nested_per_file: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut scored = Vec::new();
    let mut cross_per_file: HashMap<&str, Vec<(usize, f64)>> = HashMap::new();
    let mut same_per_file: HashMap<&str, Vec<(usize, f64)>> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let weight = unit_weights[candidate.left].min(unit_weights[candidate.right]);
        if weight < MIN_PRODUCT_SEED_NODES {
            continue;
        }
        if let Some(score) = candidate.ordinary_score.filter(|&score| score < threshold) {
            scored.push((index, score));
            let left_path = unit_paths[candidate.left];
            let right_path = unit_paths[candidate.right];
            if left_path == right_path {
                record_scored_seed(
                    &mut same_per_file,
                    left_path,
                    index,
                    score,
                    PRODUCT_CROSS_FILE_PER_FILE_CAP,
                );
            } else {
                for path in [left_path, right_path] {
                    record_scored_seed(
                        &mut cross_per_file,
                        path,
                        index,
                        score,
                        PRODUCT_CROSS_FILE_PER_FILE_CAP,
                    );
                }
            }
        } else if candidate.ordinary_score.is_none() {
            nested.push(index);
            let per_file = nested_per_file
                .entry(unit_paths[candidate.left])
                .or_default();
            if per_file.len() < PRODUCT_NESTED_PER_FILE_CAP {
                per_file.push(index);
            }
        }
    }
    nested.truncate(PRODUCT_NESTED_SEED_CAP);
    scored.sort_unstable_by(|(left_index, left_score), (right_index, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left_index.cmp(right_index))
    });
    scored.truncate(PRODUCT_GENERAL_SEED_CAP);
    let mut selected = nested.into_iter().collect::<HashSet<_>>();
    selected.extend(nested_per_file.into_values().flatten());
    selected.extend(scored.into_iter().map(|(index, _)| index));
    selected.extend(
        cross_per_file
            .into_values()
            .flatten()
            .map(|(index, _)| index),
    );
    selected.extend(
        same_per_file
            .into_values()
            .flatten()
            .map(|(index, _)| index),
    );
    let mut selected = selected.into_iter().collect::<Vec<_>>();
    selected.sort_unstable();
    selected
}

fn record_scored_seed<'a>(
    by_file: &mut HashMap<&'a str, Vec<(usize, f64)>>,
    path: &'a str,
    index: usize,
    score: f64,
    cap: usize,
) {
    let best = by_file.entry(path).or_default();
    best.push((index, score));
    best.sort_unstable_by(|(left_index, left_score), (right_index, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left_index.cmp(right_index))
    });
    best.truncate(cap);
}

fn evaluate_connected_candidate(
    units: &[UnitFeat],
    enclosing_indices: &[Option<usize>],
    same_file: &[usize],
    raw_left: usize,
    raw_right: usize,
    raw_accepted: bool,
    threshold: f64,
) -> Vec<ConnectedAccepted> {
    if is_nested(&units[raw_left], &units[raw_right]) {
        return connected_descendant_pairs(units, raw_left, raw_right, same_file, threshold);
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
            threshold,
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
    threshold: f64,
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
    let score = connected_witness_score(witness);
    (score >= threshold).then_some(ConnectedAccepted {
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
fn deduplicate_connected(
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
    connected.sort_unstable_by(|left, right| {
        right
            .witness
            .mapped_nodes
            .cmp(&left.witness.mapped_nodes)
            .then_with(|| left.witness.holes.cmp(&right.witness.holes))
            .then_with(|| (left.left, left.right).cmp(&(right.left, right.right)))
    });
    let (mut mapped, mut exit, mut nested) = (0, 0, 0);
    connected.retain(|pair| {
        let (count, cap) = match pair.route {
            ConnectedRoute::Mapped => (&mut mapped, MAPPED_CAP),
            ConnectedRoute::CompleteExit => (&mut exit, EXIT_CAP),
            ConnectedRoute::Nested => (&mut nested, NESTED_CAP),
        };
        *count += 1;
        *count <= cap
    });
}

/// A nested raw candidate is never itself reportable. It may, however, be the only LSH
/// evidence that reaches two disjoint siblings below the same container. Search only that
/// bounded subtree, require like-kind endpoints, and keep every resulting edge pair-local.
fn connected_descendant_pairs(
    units: &[UnitFeat],
    left: usize,
    right: usize,
    same_file: &[usize],
    threshold: f64,
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
                threshold,
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

#[cfg(test)]
mod connected_seed_tests {
    use super::*;

    fn scored(score: Option<f64>) -> ScoredCandidate {
        ScoredCandidate {
            left: 0,
            right: 1,
            ordinary_score: score,
        }
    }

    fn nested(left: usize, right: usize) -> ScoredCandidate {
        ScoredCandidate {
            left,
            right,
            ordinary_score: None,
        }
    }

    #[test]
    fn raw_connected_audit_keeps_every_seed() {
        let candidates = [scored(Some(0.1)), scored(None), scored(Some(0.9))];
        assert_eq!(
            connected_seed_indices(&candidates, &["a", "b"], &[20, 20], 0.7, false),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn product_connected_work_keeps_nested_and_strongest_scored_seeds() {
        let mut candidates = vec![scored(Some(0.1)); 2_050];
        candidates[0] = scored(None);
        candidates[1] = scored(Some(0.0));
        candidates[2_049] = scored(Some(0.99));
        let selected = connected_seed_indices(&candidates, &["x/a", "y/b"], &[20, 20], 1.0, true);
        assert!(
            selected.contains(&0),
            "nested routes are never budgeted away"
        );
        assert!(
            selected.contains(&2_049),
            "the strongest scored seed is retained"
        );
        assert!(
            !selected.contains(&1),
            "the weakest overflow seed is dropped"
        );
        assert_eq!(selected.len(), 2_049);
    }

    #[test]
    fn product_connected_work_reserves_nested_seeds_per_file() {
        let mut candidates = vec![nested(0, 1); 513];
        candidates.push(nested(2, 3));
        let paths = ["dense/a.rs", "dense/a.rs", "small/b.rs", "small/b.rs"];
        let selected = connected_seed_indices(&candidates, &paths, &[20; 4], 0.7, true);
        assert!(
            selected.contains(&513),
            "a later file keeps its own nested seed after the global cap"
        );
    }
}
