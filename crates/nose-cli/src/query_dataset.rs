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
    let (mut report, scope, semantic_pack_near, semantic_pack_external_exact, line_context) =
        query_detect_report(
            args,
            refs,
            &settings.exclude,
            &opts,
            detector.as_ref(),
            &semantic_packs,
            settings.cache_max_bytes,
            AcceptedCoverage::Query,
        );

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
        weight_shared_lines(
            &mut families,
            refs,
            &settings.exclude,
            line_context.as_ref(),
        )
    });
    finish_cache_run(line_context);
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
pub(crate) fn build_divergence_families(
    args: &QueryArgs,
    refs: &[&std::path::Path],
) -> Result<(Vec<nose_detect::RefactorFamily>, nose_detect::DetectOptions)> {
    let (settings, semantic_packs) = resolve_query_settings(args, DIVERGENCE_DEFAULT_MODES)?;
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines);
    let detector = detection_engine(settings.channels, &opts);
    let (report, _, semantic_pack_near, semantic_pack_external_exact, line_context) =
        query_detect_report(
            args,
            refs,
            &settings.exclude,
            &opts,
            detector.as_ref(),
            &semantic_packs,
            settings.cache_max_bytes,
            AcceptedCoverage::Direct,
        );
    let mut families = nose_detect::rank_families(&report);
    annotate_semantic_pack_near(&mut families, &semantic_pack_near);
    annotate_semantic_pack_external_exact(&mut families, &semantic_pack_external_exact);
    if settings.channels.abstraction_only() {
        families.retain(|family| family.abstraction_witness.is_some());
    }
    finish_cache_run(line_context);
    Ok((families, opts))
}

fn finish_cache_run(context: Option<cache::CachedLineContext>) {
    let Some(context) = context else { return };
    if let Err(error) = context.run.commit() {
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [cache-generation] commit skipped: {error}");
        }
    } else if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        eprintln!(
            "  [cache-generation] written_bytes={}",
            context.run.written_bytes()
        );
    }
    cache::enforce_run_budget(context.run);
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

/// With --cache-dir, build units per file through the on-disk cache (skips
/// normalize/extract for unchanged files); otherwise lower the whole corpus.
type DetectionReport = (
    nose_detect::Report,
    QueryScope,
    nose_semantics::SemanticPackNearRegistry,
    nose_semantics::SemanticPackExternalExactRegistry,
    Option<cache::CachedLineContext>,
);

#[derive(Clone, Copy)]
enum AcceptedCoverage {
    Query,
    Direct,
}

fn query_detect_report(
    args: &QueryArgs,
    refs: &[&std::path::Path],
    exclude: &[String],
    opts: &nose_detect::DetectOptions,
    detector: &dyn nose_detect::Detector,
    semantic_packs: &nose_semantics::SemanticPackSet,
    cache_max_bytes: u64,
    accepted_coverage: AcceptedCoverage,
) -> DetectionReport {
    let (mut corpus, invalidation_report, unit_contexts, cache_identity_parts, line_context) =
        if let Some(dir) = &args.cache_dir {
            let cached = time_lower(|| {
                cache::build_corpus_cached(refs, exclude, dir, semantic_packs, cache_max_bytes)
            });
            let run = cached.run.clone();
            let line_context = cache::CachedLineContext {
                source_files: cached.source_files,
                run,
            };
            (
                cached.corpus,
                Some(cached.report),
                Some(cached.unit_contexts),
                Some((cached.workspace_digest, cached.semantic_pack_digest)),
                Some(line_context),
            )
        } else {
            (
                time_lower(|| nose_frontend::lower_corpus_filtered(refs, exclude)),
                None,
                None,
                None,
                None,
            )
        };
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
    let (mut units, unit_keys, streams, files) = if args.cache_dir.is_some() {
        let cache::CachedUnits {
            units,
            unit_keys,
            streams,
            files,
            stats,
        } = time_stage("cache", || {
            cache::build_units_cached_with_context(
                &mut corpus,
                opts,
                &line_context
                    .as_ref()
                    .expect("cached corpus includes a cache run")
                    .run,
                unit_contexts
                    .as_deref()
                    .expect("cached corpus includes unit contexts"),
            )
        });
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!(
                "  [cache] files={} hits={} misses={} read_bytes={} written_bytes={}",
                stats.files, stats.hits, stats.misses, stats.read_bytes, stats.written_bytes
            );
        }
        (units, Some(unit_keys), streams, files)
    } else {
        let features = time_stage("normalize+extract", || {
            nose_detect::corpus_features(&corpus, opts)
        });
        (features.units, None, features.streams, features.files)
    };
    if opts.shape_candidates && semantic_pack_near.is_active() {
        for unit in &mut units {
            unit.semantic_pack_near_protocols =
                semantic_pack_near.protocols_for_unit(&unit.path, unit.start_line, unit.end_line);
        }
    }
    drop(semantic_pack_evidence);
    drop(corpus);
    let report = detect_cached_or_clean(
        cache_identity_parts,
        line_context.as_ref().map(|context| &context.run),
        (units, unit_keys.as_deref()),
        files,
        &streams,
        opts,
        detector,
        accepted_coverage,
    );
    (
        report,
        scope,
        semantic_pack_near,
        semantic_pack_external_exact,
        line_context,
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

fn detect_cached_or_clean(
    cache_identity_parts: Option<([u8; 32], [u8; 32])>,
    cache_run: Option<&cache::CacheRun>,
    detection_units: (Vec<nose_detect::UnitFeat>, Option<&[[u8; 32]]>),
    files: usize,
    streams: &[nose_detect::Stream],
    opts: &nose_detect::DetectOptions,
    detector: &dyn nose_detect::Detector,
    accepted_coverage: AcceptedCoverage,
) -> nose_detect::Report {
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

fn annotate_semantic_pack_near(
    families: &mut [nose_detect::RefactorFamily],
    registry: &nose_semantics::SemanticPackNearRegistry,
) {
    if !registry.is_active() {
        return;
    }
    for family in families.iter_mut().filter(|family| {
        family.witness.as_ref().map(|witness| witness.kind) == Some("structural-similarity")
    }) {
        let protocols = family
            .locations
            .iter()
            .map(|location| {
                registry.protocols_for_unit(&location.file, location.start_line, location.end_line)
            })
            .collect::<Vec<_>>();
        let mut aggregate = BTreeSet::new();
        for (index, location) in family.locations.iter_mut().enumerate() {
            let mut member = BTreeSet::new();
            for protocol in &protocols[index] {
                let Some(provenance) = &protocol.provenance else {
                    continue;
                };
                let supported = protocols.iter().enumerate().any(|(other_index, others)| {
                    other_index != index
                        && others
                            .iter()
                            .any(|other| other.operation == protocol.operation)
                });
                if supported {
                    member.insert(provenance.clone());
                    aggregate.insert(provenance.clone());
                }
            }
            location.semantic_pack_near = member.into_iter().collect();
        }
        family.semantic_pack_near = aggregate.into_iter().collect();
    }
}

fn annotate_semantic_pack_external_exact(
    families: &mut [nose_detect::RefactorFamily],
    registry: &nose_semantics::SemanticPackExternalExactRegistry,
) {
    if !registry.is_active() {
        return;
    }
    for family in families.iter_mut().filter(|family| {
        family.witness.as_ref().map(|witness| witness.kind) == Some("exact-value-graph")
    }) {
        let mut aggregate = BTreeSet::new();
        for location in &mut family.locations {
            let claims =
                registry.claims_for_unit(&location.file, location.start_line, location.end_line);
            aggregate.extend(claims.iter().cloned());
            location.semantic_pack_external_exact = claims;
        }
        family.semantic_pack_external_exact = aggregate.into_iter().collect();
    }
}

/// Compute the honest shared-line count for each family, before ranking. This layer has
/// source access; the detector deals only in IL.
///
/// `shared_lines` (displayed) is the count of *all* lines invariant across the family
/// — including boilerplate, so it matches what `--show proposal` shows. For *ranking*
/// (`shared_weight`) we separate signal from noise: sum the IDF weight of the
/// substantive lines (non-trivial, and rare across the corpus — a `if err != nil {`
/// that appears in most files contributes ~0), then use that as a **gate** on the
/// full block. A family whose shared lines are all boilerplate/idiom has ~0
/// substantive weight → it scores ~0 however much it "shares"; a family with real
/// shared content is credited for its whole extractable block (boilerplate included).
/// Cross-language families have no shared *source* lines to diff, so they keep
/// `shared_weight = 0` and fall back to the structural estimate in `extractability()`.
/// Only same-language families with ≥2 sites get an honest shared-line count; the
/// rest keep the detector's structural estimate. Computing the corpus line-IDF means
/// re-reading every analyzed file, so skip it entirely when no family qualifies (a
/// clean repo, or a run where `--min-value`/`--min-members` filtered everything) —
/// otherwise a quiet analysis pays a full second corpus read for nothing.
fn weight_shared_lines(
    families: &mut [nose_detect::RefactorFamily],
    refs: &[&std::path::Path],
    exclude: &[String],
    cached: Option<&cache::CachedLineContext>,
) {
    let needs_shared = |f: &nose_detect::RefactorFamily| f.languages == 1 && f.locations.len() >= 2;
    if !families.iter().any(needs_shared) {
        return;
    }
    let mut lines = FileLineCache::default();
    if let Some(context) = cached {
        let (mut idf, mut stats, mut changed_lines, mut file_count, complete) =
            cached_line_idf(context, &mut lines, false);
        let mut family_stats = apply_cached_family_lines(
            families,
            &idf,
            &mut lines,
            context,
            &changed_lines,
            file_count,
            complete,
        );
        if family_stats.is_none() {
            lines = FileLineCache::default();
            let full = cached_line_idf(context, &mut lines, true);
            idf = full.0;
            stats = full.1;
            changed_lines = full.2;
            file_count = full.3;
            family_stats = apply_cached_family_lines(
                families,
                &idf,
                &mut lines,
                context,
                &changed_lines,
                file_count,
                true,
            );
        }
        let family_stats = family_stats.expect("full line index covers every family");
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [line-index] {}", cache::line_index_stats_json(&stats));
            eprintln!(
                "  [family-lines] {}",
                serde_json::to_string(&family_stats)
                    .expect("family line stats are JSON serializable")
            );
        }
        return;
    }
    let idf = corpus_line_idf(refs, exclude, &mut lines);
    for f in families.iter_mut().filter(|f| needs_shared(f)) {
        // Difference evidence comes from the same first readable representative
        // pair the `params` count uses (locations[0] vs the first member that
        // reads), so the two fields stay mutually consistent.
        f.varying_spots = f.locations[1..]
            .iter()
            .find_map(|b| varying_spots_of(&f.locations[0], b, &mut lines))
            .unwrap_or_default();
        if let Some(s) = shared_lines_of(&f.locations, &mut lines) {
            let substantive: f64 = s
                .rank_lines
                .iter()
                .filter(|l| !is_trivial_line(l))
                .map(|l| idf.weight(l))
                .sum();
            // Gate ramps 0→1 as substantive shared content goes 0→2 lines.
            let gate = (substantive / 2.0).clamp(0.0, 1.0);
            // Display is the all-copies invariant count (#366); ranking weights the
            // majority-voted set. `shared_weight` keeps using the rank set so the
            // robust signal still drives the order, unchanged by the display basis.
            f.shared_lines = s.display;
            f.shared_weight = s.rank_lines.len() as f64 * gate;
            f.params = s.params;
            f.display_params = Some(s.display_params);
        }
    }
}
