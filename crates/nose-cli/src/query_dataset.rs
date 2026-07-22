use anyhow::Result;

use crate::cli_args::QueryArgs;
use crate::detect_pipeline::{detection_engine, detection_options, validate_exclude_globs};
use crate::path_utils::{relativize, relativize_loc};
use crate::query_options::{
    validate_min_value, DetectionChannels, QueryScope, SortKey, DIVERGENCE_DEFAULT_MODES,
    QUERY_DEFAULT_MODES,
};
use crate::source_lines::{
    apply_cached_family_lines, cached_line_idf, corpus_line_idf, family_anchor, is_trivial_line,
    shared_lines_of, varying_spots_of, FileLineCache,
};
use crate::surfaces::GeneratedPathAssertions;
use crate::timing::{time_lower, time_stage};
use crate::{cache, config, ignores};
use std::collections::BTreeSet;

include!("query_dataset/annotations.rs");

mod session;
pub(super) use session::QueryAnalysisSession;

/// The ranked family dataset behind `nose query`: detect, rank,
/// filter (min-members / min-value / scope), relativize paths, weight shared lines, and
/// sort. It stops before query view selection, structured ignores, surface classification,
/// rendering, and the CI gate so each query view can apply those layers deterministically.
pub(super) struct QueryDataset {
    pub(super) families: Vec<nose_detect::RefactorFamily>,
    pub(super) scope: QueryScope,
    pub(super) settings: QuerySettings,
    pub(super) semantic_packs: nose_semantics::SemanticPackSet,
    pub(super) semantic_pack_near_report: nose_semantics::SemanticPackNearReport,
    pub(super) semantic_pack_external_exact_report: nose_semantics::SemanticPackExternalExactReport,
    pub(super) reinvented: Vec<nose_detect::ReinventedHelper>,
    pub(super) opts: nose_detect::DetectOptions,
}

pub(super) fn build_query_dataset(
    args: &QueryArgs,
    refs: &[&std::path::Path],
) -> Result<QueryDataset> {
    let (settings, semantic_packs) = resolve_query_settings(args, QUERY_DEFAULT_MODES)?;
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines);
    let detector = detection_engine(settings.channels, &opts);
    let detection = query_detect_report(QueryDetectRequest {
        args,
        refs,
        exclude: &settings.exclude,
        opts: &opts,
        detector: detector.as_ref(),
        semantic_packs: &semantic_packs,
        cache_max_bytes: settings.cache_max_bytes,
        accepted_coverage: AcceptedCoverage::Query,
    });
    finish_query_dataset(args, refs, settings, semantic_packs, opts, detection, true)
}

fn finish_query_dataset(
    args: &QueryArgs,
    refs: &[&std::path::Path],
    settings: QuerySettings,
    semantic_packs: nose_semantics::SemanticPackSet,
    opts: nose_detect::DetectOptions,
    detection: DetectionReport,
    finalize_cache: bool,
) -> Result<QueryDataset> {
    let (
        mut report,
        scope,
        semantic_pack_near,
        semantic_pack_external_exact,
        line_context,
        _retained_resolved,
    ) = detection;

    let mut families = time_stage("rank_families", || nose_detect::rank_families(&report));
    annotate_semantic_pack_near(&mut families, &semantic_pack_near);
    annotate_semantic_pack_external_exact(&mut families, &semantic_pack_external_exact);
    preserve_query_accepted_coverage(&mut families);
    time_stage("query_filter", || {
        if settings.channels.abstraction_only() {
            families.retain(|f| f.abstraction_witness.is_some());
        }
        families.retain(|f| f.members >= settings.min_members && f.value >= settings.min_value);
        families.retain(|f| args.scope.keeps(f));
    });
    // Show paths relative to the working directory — absolute paths are unreadable
    // in CI logs, and relative ones are clickable and portable.
    let mut reinvented = std::mem::take(&mut report.reinvented);
    if let Ok(cwd) = std::env::current_dir() {
        for f in &mut families {
            for l in &mut f.locations {
                relativize_loc(l, &cwd);
                for provenance in &mut l.semantic_pack_near {
                    provenance.occurrence_file = relativize(&provenance.occurrence_file, &cwd);
                }
                for provenance in &mut l.semantic_pack_external_exact {
                    provenance.occurrence_file = relativize(&provenance.occurrence_file, &cwd);
                }
            }
            for provenance in &mut f.semantic_pack_near {
                provenance.occurrence_file = relativize(&provenance.occurrence_file, &cwd);
            }
            for provenance in &mut f.semantic_pack_external_exact {
                provenance.occurrence_file = relativize(&provenance.occurrence_file, &cwd);
            }
            for obligation in &mut f.accepted_coverage {
                for l in &mut obligation.sites {
                    relativize_loc(l, &cwd);
                }
            }
        }
        for r in &mut reinvented {
            r.helper_file = relativize(&r.helper_file, &cwd);
            r.container_file = relativize(&r.container_file, &cwd);
        }
    }
    time_stage("shared_lines", || {
        // The persistent line dictionary is valuable for watch sessions and
        // small one-shot projects. Rebuilding and serializing it for a large
        // foreground scan costs more than the clean parallel implementation;
        // the line weighting itself remains identical in either path.
        let cached_lines = line_context.as_ref().filter(|context| {
            !finalize_cache || context.source_files.len() <= cache::MAX_FOREGROUND_PORTABLE_IL_FILES
        });
        weight_shared_lines(&mut families, refs, &settings.exclude, cached_lines)
    });
    if finalize_cache {
        time_stage("cache_commit", || cache::finish_query_run(line_context));
    }
    let sort = settings.sort;
    time_stage("query_rank_sort", || {
        families.sort_by(|a, b| {
            sort.score(b)
                .total_cmp(&sort.score(a))
                // Deterministic tie-breaks: raw value, then first site's location.
                .then(b.value.total_cmp(&a.value))
                .then_with(|| family_anchor(a).cmp(&family_anchor(b)))
        })
    });
    let semantic_pack_near_report = semantic_pack_near.report_with_influential(
        families
            .iter()
            .flat_map(|family| family.semantic_pack_near.iter()),
    );
    let semantic_pack_external_exact_report = semantic_pack_external_exact.report_with_influential(
        families
            .iter()
            .flat_map(|family| family.semantic_pack_external_exact.iter()),
    );
    Ok(QueryDataset {
        families,
        scope,
        settings,
        semantic_packs,
        semantic_pack_near_report,
        semantic_pack_external_exact_report,
        reinvented,
        opts,
    })
}

/// Run the `base=` detector through the same frontend, semantic-pack, and
/// per-file cache engine as ordinary query while retaining pair-local edges for
/// propagation targets. Divergence keeps its stricter default channel set.
pub(crate) struct DivergenceQueryPlan {
    settings: QuerySettings,
    semantic_packs: nose_semantics::SemanticPackSet,
    opts: nose_detect::DetectOptions,
}

impl DivergenceQueryPlan {
    pub(crate) fn options(&self) -> &nose_detect::DetectOptions {
        &self.opts
    }
}

pub(crate) fn prepare_divergence_query(args: &QueryArgs) -> Result<DivergenceQueryPlan> {
    let (settings, semantic_packs) = resolve_query_settings(args, DIVERGENCE_DEFAULT_MODES)?;
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines);
    Ok(DivergenceQueryPlan {
        settings,
        semantic_packs,
        opts,
    })
}

pub(crate) fn build_divergence_families(
    args: &QueryArgs,
    refs: &[&std::path::Path],
    plan: DivergenceQueryPlan,
) -> Result<(
    Vec<nose_detect::RefactorFamily>,
    nose_detect::DetectOptions,
    Option<RetainedResolvedCorpus>,
)> {
    let DivergenceQueryPlan {
        settings,
        semantic_packs,
        opts,
    } = plan;
    let detector = detection_engine(settings.channels, &opts);
    let (
        report,
        _,
        semantic_pack_near,
        semantic_pack_external_exact,
        line_context,
        retained_resolved,
    ) = query_detect_report(QueryDetectRequest {
        args,
        refs,
        exclude: &settings.exclude,
        opts: &opts,
        detector: detector.as_ref(),
        semantic_packs: &semantic_packs,
        cache_max_bytes: settings.cache_max_bytes,
        accepted_coverage: AcceptedCoverage::Direct,
    });
    let mut families = nose_detect::rank_families(&report);
    annotate_semantic_pack_near(&mut families, &semantic_pack_near);
    annotate_semantic_pack_external_exact(&mut families, &semantic_pack_external_exact);
    if settings.channels.abstraction_only() {
        families.retain(|family| family.abstraction_witness.is_some());
    }
    time_stage("cache_commit", || cache::finish_query_run(line_context));
    Ok((families, opts, retained_resolved))
}

/// `direct_edges` is the richer representation needed by the `base=` divergence view.
/// Ordinary query opportunity folding predates that representation and must keep treating
/// the same detector pairs as accepted-coverage obligations; otherwise adding target evidence
/// changes which ordinary families remain visible.
pub(crate) fn preserve_query_accepted_coverage(families: &mut [nose_detect::RefactorFamily]) {
    for family in families {
        if family.direct_edges.is_empty() {
            continue;
        }
        family.accepted_coverage.insert(
            0,
            nose_detect::AcceptedCoverage {
                sites: family.locations.clone(),
                edges: std::mem::take(&mut family.direct_edges),
            },
        );
    }
}

/// The query settings after layering: CLI flag wins, else config file, else built-in
/// default.
pub(super) struct QuerySettings {
    pub(super) min_members: usize,
    pub(super) min_value: f64,
    pub(super) sort: SortKey,
    pub(super) channels: DetectionChannels,
    pub(super) min_lines: u32,
    pub(super) min_tokens: usize,
    pub(super) exclude: Vec<String>,
    pub(super) generated_paths: GeneratedPathAssertions,
    pub(super) ignore_set: Option<ignores::IgnoreSet>,
    pub(super) cache_max_bytes: u64,
}

pub(super) fn resolve_query_semantic_packs(
    args: &QueryArgs,
) -> Result<nose_semantics::SemanticPackSet> {
    let cfg = config::load_query(args.config.as_deref())?;
    semantic_pack_set_from_inputs(
        cfg.semantic_packs,
        &args.semantic_pack,
        cfg.semantic_pack_lock,
        args.semantic_pack_lock.as_ref(),
    )
}

fn semantic_pack_set_from_inputs(
    mut semantic_pack_paths: Vec<std::path::PathBuf>,
    cli_semantic_pack_paths: &[std::path::PathBuf],
    config_lock: Option<std::path::PathBuf>,
    cli_lock: Option<&std::path::PathBuf>,
) -> Result<nose_semantics::SemanticPackSet> {
    semantic_pack_paths.extend(cli_semantic_pack_paths.iter().cloned());
    let lock = cli_lock.cloned().or(config_lock);
    if let Some(lock) = lock {
        if !semantic_pack_paths.is_empty() {
            anyhow::bail!(
                "a semantic-pack project lock is mutually exclusive with `--semantic-pack` and `[query].semantic-packs`; the lock owns the complete manifest set"
            );
        }
        return Ok(nose_semantics::SemanticPackSet::new_locked(&lock)?);
    }
    Ok(nose_semantics::SemanticPackSet::new_local(
        &semantic_pack_paths,
    )?)
}

fn resolve_query_settings(
    args: &QueryArgs,
    default_modes: &[crate::query_options::DetectionMode],
) -> Result<(QuerySettings, nose_semantics::SemanticPackSet)> {
    let cfg = config::load_query(args.config.as_deref())?;
    let min_members = args.min_members.or(cfg.min_members).unwrap_or(2);
    let min_value = validate_min_value(args.min_value.or(cfg.min_value).unwrap_or(0.0))?;
    let sort = args.sort.or(cfg.sort).unwrap_or(SortKey::Extractability);
    let channels = DetectionChannels::resolve(args.mode.clone(), cfg.mode, default_modes)?;
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let cache_max_bytes = args
        .cache_max_bytes
        .or(cfg.cache_max_bytes)
        .unwrap_or(cache::DEFAULT_MAX_BYTES);
    let ignore_file = args.ignore_file.clone().or(cfg.ignore_file);
    let semantic_packs = semantic_pack_set_from_inputs(
        cfg.semantic_packs,
        &args.semantic_pack,
        cfg.semantic_pack_lock,
        args.semantic_pack_lock.as_ref(),
    )?;
    // Excludes are additive: config patterns plus any given on the command line.
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());
    validate_exclude_globs(&exclude)?;
    let mut generated_path_patterns = cfg.generated_paths;
    generated_path_patterns.extend(args.generated_path.iter().cloned());
    let generated_paths = GeneratedPathAssertions::new(&args.paths, generated_path_patterns)?;
    let ignore_set = ignores::load_for_query(ignore_file.as_deref())?;
    if let Some(ignore_set) = &ignore_set {
        ignore_set.warn_expired();
    }
    Ok((
        QuerySettings {
            min_members,
            min_value,
            sort,
            channels,
            min_lines,
            min_tokens,
            exclude,
            generated_paths,
            ignore_set,
            cache_max_bytes,
        },
        semantic_packs,
    ))
}

type DetectionReport = (
    nose_detect::Report,
    QueryScope,
    nose_semantics::SemanticPackNearRegistry,
    nose_semantics::SemanticPackExternalExactRegistry,
    Option<cache::CachedLineContext>,
    Option<RetainedResolvedCorpus>,
);

pub(crate) struct RetainedResolvedCorpus {
    pub(crate) corpus: nose_il::Corpus,
}

#[derive(Clone, Copy)]
enum AcceptedCoverage {
    Query,
    Direct,
}

struct QueryDetectRequest<'a> {
    args: &'a QueryArgs,
    refs: &'a [&'a std::path::Path],
    exclude: &'a [String],
    opts: &'a nose_detect::DetectOptions,
    detector: &'a dyn nose_detect::Detector,
    semantic_packs: &'a nose_semantics::SemanticPackSet,
    cache_max_bytes: u64,
    accepted_coverage: AcceptedCoverage,
}

struct PreparedDetectionFeatures {
    units: Vec<nose_detect::UnitFeat>,
    unit_keys: Option<Vec<[u8; 32]>>,
    streams: Vec<nose_detect::Stream>,
    files: usize,
}

fn query_detect_report(request: QueryDetectRequest<'_>) -> DetectionReport {
    if let Some(fast) = try_query_detect_report_fast(&request) {
        return fast;
    }
    let PreparedCorpus {
        mut corpus,
        invalidation_report,
        unit_snapshot,
        cache_identity_parts,
        line_context,
    } = prepare_query_corpus(&request);
    let QueryDetectRequest {
        args,
        refs: _,
        exclude: _,
        opts,
        detector,
        semantic_packs,
        cache_max_bytes: _,
        accepted_coverage,
    } = request;
    print_invalidation(invalidation_report.as_ref());
    let scope = QueryScope::from_corpus(&corpus);
    let semantic_pack_evidence =
        nose_semantics::SemanticPackEvidenceIndex::build(semantic_packs, &corpus);
    let semantic_pack_near = nose_semantics::SemanticPackNearRegistry::build(
        semantic_packs,
        &semantic_pack_evidence,
        &corpus,
    );
    let semantic_pack_external_exact = nose_semantics::SemanticPackExternalExactRegistry::build(
        semantic_packs,
        &semantic_pack_evidence,
        &corpus,
    );
    semantic_pack_external_exact.apply(&mut corpus);
    let PreparedDetectionFeatures {
        mut units,
        unit_keys,
        streams,
        files,
    } = prepare_detection_features(
        &mut corpus,
        opts,
        args.cache_dir.is_some(),
        line_context.as_ref(),
        unit_snapshot,
    );
    if opts.shape_candidates && semantic_pack_near.is_active() {
        for unit in &mut units {
            unit.semantic_pack_near_protocols =
                semantic_pack_near.protocols_for_unit(&unit.path, unit.start_line, unit.end_line);
        }
    }
    drop(semantic_pack_evidence);
    // Divergence witnesses need corpus-wide import/pack evidence, but retaining the
    // normalized detector corpus extends thousands of fresh arena lifetimes. Keep the
    // smaller resolved raw corpus and normalize only the bounded flagged-file set later.
    let retained_resolved = if matches!(accepted_coverage, AcceptedCoverage::Direct) {
        Some(RetainedResolvedCorpus { corpus })
    } else {
        drop(corpus);
        None
    };
    let report = detect_cached_or_clean(DetectCachedRequest {
        cache_identity_parts,
        cache_run: line_context.as_ref().map(|context| &context.run),
        detection_units: (units, unit_keys.as_deref()),
        files,
        streams: &streams,
        opts,
        detector,
        accepted_coverage,
    });
    (
        report,
        scope,
        semantic_pack_near,
        semantic_pack_external_exact,
        line_context,
        retained_resolved,
    )
}

fn prepare_detection_features(
    corpus: &mut nose_il::Corpus,
    opts: &nose_detect::DetectOptions,
    cached: bool,
    line_context: Option<&cache::CachedLineContext>,
    unit_snapshot: Option<cache::CachedUnitSnapshot>,
) -> PreparedDetectionFeatures {
    if cached {
        let cache::CachedUnits {
            units,
            unit_keys,
            streams,
            files,
            stats,
        } = time_stage("cache", || {
            cache::build_units_cached_with_context(
                corpus,
                opts,
                &line_context
                    .expect("cached corpus includes a cache run")
                    .run,
                unit_snapshot.expect("cached corpus includes unit contexts"),
            )
        });
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!(
                "  [cache] files={} hits={} misses={} read_bytes={} written_bytes={}",
                stats.files, stats.hits, stats.misses, stats.read_bytes, stats.written_bytes
            );
        }
        PreparedDetectionFeatures {
            units,
            unit_keys: Some(unit_keys),
            streams,
            files,
        }
    } else {
        // Semantic-change projection is capped to the already-flagged files. Retaining
        // every normalized arena here extends a corpus-sized allocation lifetime and
        // makes the later witness path index thousands of files it will never inspect.
        // Re-project the bounded target set there instead.
        let features = time_stage("normalize+extract", || {
            nose_detect::corpus_features(corpus, opts)
        });
        PreparedDetectionFeatures {
            units: features.units,
            unit_keys: None,
            streams: features.streams,
            files: features.files,
        }
    }
}

struct PreparedCorpus {
    corpus: nose_il::Corpus,
    invalidation_report: Option<cache::InvalidationReport>,
    unit_snapshot: Option<cache::CachedUnitSnapshot>,
    cache_identity_parts: Option<([u8; 32], [u8; 32])>,
    line_context: Option<cache::CachedLineContext>,
}

fn prepare_query_corpus(request: &QueryDetectRequest<'_>) -> PreparedCorpus {
    let Some(dir) = &request.args.cache_dir else {
        return PreparedCorpus {
            corpus: time_lower(|| {
                nose_frontend::lower_corpus_filtered(request.refs, request.exclude)
            }),
            invalidation_report: None,
            unit_snapshot: None,
            cache_identity_parts: None,
            line_context: None,
        };
    };
    let cached = time_lower(|| {
        cache::build_corpus_cached(
            request.refs,
            request.exclude,
            dir,
            request.semantic_packs,
            request.cache_max_bytes,
        )
    });
    let line_context = cache::CachedLineContext {
        source_files: cached.source_files.into(),
        run: cached.run.clone(),
    };
    PreparedCorpus {
        corpus: cached.corpus,
        invalidation_report: Some(cached.report),
        unit_snapshot: Some(cached.unit_snapshot),
        cache_identity_parts: Some((cached.workspace_digest, cached.semantic_pack_digest)),
        line_context: Some(line_context),
    }
}

fn try_query_detect_report_fast(request: &QueryDetectRequest<'_>) -> Option<DetectionReport> {
    matches!(request.accepted_coverage, AcceptedCoverage::Query).then_some(())?;
    let dir = request.args.cache_dir.as_ref()?;
    let fast = time_lower(|| {
        cache::try_build_units_fast(
            request.refs,
            request.exclude,
            dir,
            request.semantic_packs,
            request.cache_max_bytes,
            request.opts,
        )
    })?;
    Some(query_detect_report_fast(
        fast,
        request.opts,
        request.detector,
    ))
}

fn query_detect_report_fast(
    fast: cache::FastCachedUnits,
    opts: &nose_detect::DetectOptions,
    detector: &dyn nose_detect::Detector,
) -> DetectionReport {
    let cache::FastCachedUnits {
        cached,
        report: invalidation_report,
        workspace_digest,
        semantic_pack_digest,
        source_files,
        run,
        langs,
        ..
    } = fast;
    print_invalidation(Some(&invalidation_report));
    let cache::CachedUnits {
        units,
        unit_keys,
        streams,
        files,
        stats,
    } = cached;
    if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        eprintln!(
            "  [cache] files={} hits={} misses={} read_bytes={} written_bytes={}",
            stats.files, stats.hits, stats.misses, stats.read_bytes, stats.written_bytes
        );
    }
    let report = detect_cached_or_clean(DetectCachedRequest {
        cache_identity_parts: Some((workspace_digest, semantic_pack_digest)),
        cache_run: Some(&run),
        detection_units: (units, Some(&unit_keys)),
        files,
        streams: &streams,
        opts,
        detector,
        accepted_coverage: AcceptedCoverage::Query,
    });
    (
        report,
        QueryScope::from_langs(langs),
        nose_semantics::SemanticPackNearRegistry::default(),
        nose_semantics::SemanticPackExternalExactRegistry::default(),
        Some(cache::CachedLineContext {
            source_files: source_files.into(),
            run,
        }),
        None,
    )
}

fn print_invalidation(report: Option<&cache::InvalidationReport>) {
    if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        if let Some(report) = report {
            eprintln!(
                "  [invalidation] {}",
                cache::invalidation_report_json(report)
            );
        }
    }
}

struct DetectCachedRequest<'a> {
    cache_identity_parts: Option<([u8; 32], [u8; 32])>,
    cache_run: Option<&'a cache::CacheRun>,
    detection_units: (Vec<nose_detect::UnitFeat>, Option<&'a [[u8; 32]]>),
    files: usize,
    streams: &'a [nose_detect::Stream],
    opts: &'a nose_detect::DetectOptions,
    detector: &'a dyn nose_detect::Detector,
    accepted_coverage: AcceptedCoverage,
}

fn detect_cached_or_clean(request: DetectCachedRequest<'_>) -> nose_detect::Report {
    const MAX_PERSISTENT_DETECTION_UNITS: usize = 20_000;
    let DetectCachedRequest {
        cache_identity_parts,
        cache_run,
        detection_units,
        files,
        streams,
        opts,
        detector,
        accepted_coverage,
    } = request;
    let (units, unit_keys) = detection_units;
    if matches!(accepted_coverage, AcceptedCoverage::Direct) {
        return nose_detect::detect_from_units_with_direct_accepted_coverage(
            units, files, streams, opts, detector,
        );
    }
    let (Some(run), Some((workspace, pack_digest))) = (cache_run, cache_identity_parts) else {
        return nose_detect::detect_from_units_with_accepted_coverage(
            units, files, streams, opts, detector,
        );
    };
    let identity = cache::DetectionCacheIdentity::new(workspace, pack_digest, opts, detector);
    let previous = cache::load_detection_state(run, &identity);
    // Building the persistent bucket graph is deliberately more expensive than
    // the clean sort-based detector. Above this bound its first-generation cost
    // and state write exceed the savings from many no-op process invocations;
    // the unit cache still avoids normalization, while watch sessions retain
    // their in-memory incremental detector without this cap.
    if previous.is_none() && units.len() > MAX_PERSISTENT_DETECTION_UNITS {
        return nose_detect::detect_from_units_with_accepted_coverage(
            units, files, streams, opts, detector,
        );
    }
    let (report, state, stats) = nose_detect::detect_from_units_incremental_with_accepted_coverage(
        units, files, streams, opts, detector, previous, unit_keys,
    );
    if let Some(state) = state {
        cache::store_detection_state(run, &identity, &state);
    }
    if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        eprintln!(
            "  [detection] {}",
            cache::incremental_detection_stats_json(&stats)
        );
    }
    report
}
