use anyhow::Result;

use crate::cache;
use crate::cli_args::QueryArgs;
use crate::detect_pipeline::{detection_engine, detection_options};
use crate::path_utils::{relativize, relativize_loc};
use crate::query_options::{QueryScope, QUERY_DEFAULT_MODES};
use crate::source_lines::{
    apply_cached_family_lines, cached_line_idf, corpus_line_idf, family_anchor, is_trivial_line,
    shared_lines_of, varying_spots_of, FileLineCache,
};
use crate::timing::{time_lower, time_stage};
use std::collections::BTreeSet;

include!("query_dataset/annotations.rs");

mod detection;
mod divergence;
mod session;
mod settings;
use detection::{
    detect_cached_or_clean, print_invalidation, try_query_detect_report_fast, DetectCachedRequest,
};
pub(crate) use divergence::{build_divergence_families, prepare_divergence_query};
pub(super) use session::QueryAnalysisSession;
pub(crate) use settings::resolve_query_settings;
pub(super) use settings::{resolve_query_semantic_packs, QuerySettings};

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
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines)?;
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
    })
    .map_err(|error| crate::query_recovery::explain(error, args, refs, &settings.exclude))?;
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
        _retained_normalized,
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

type DetectionReport = (
    nose_detect::Report,
    QueryScope,
    nose_semantics::SemanticPackNearRegistry,
    nose_semantics::SemanticPackExternalExactRegistry,
    Option<cache::CachedLineContext>,
    Option<RetainedNormalizedCorpus>,
);

/// With --cache-dir, build units per file through the on-disk cache (skips
/// normalize/extract for unchanged files); otherwise retain the already-normalized
/// base corpus for bounded divergence witnesses.
pub(crate) struct RetainedNormalizedCorpus {
    pub(crate) corpus: nose_il::Corpus,
    pub(crate) exact_safety: Vec<RetainedExactSafety>,
    pub(crate) value_contexts: Vec<(String, nose_normalize::ValueFingerprintContext)>,
}

pub(crate) struct RetainedExactSafety {
    pub(crate) path: String,
    pub(crate) kind: nose_il::UnitKind,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) exact_safe: bool,
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
    retained_normalized: Option<RetainedNormalizedCorpus>,
}

fn query_detect_report(request: QueryDetectRequest<'_>) -> Result<DetectionReport> {
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
    corpus.ensure_complete()?;
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
        retained_normalized,
    } = prepare_detection_features(
        &mut corpus,
        opts,
        args.cache_dir.is_some(),
        line_context.as_ref(),
        unit_snapshot,
        accepted_coverage,
    );
    if opts.shape_candidates && semantic_pack_near.is_active() {
        for unit in &mut units {
            unit.semantic_pack_near_protocols =
                semantic_pack_near.protocols_for_unit(&unit.path, unit.start_line, unit.end_line);
        }
    }
    drop(semantic_pack_evidence);
    drop(corpus);
    let report = detect_cached_or_clean(DetectCachedRequest {
        cache_identity_parts,
        cache_run: line_context.as_ref().map(|context| &context.run),
        detection_units: (units, unit_keys.as_deref()),
        files,
        streams: &streams,
        opts,
        detector,
        accepted_coverage,
    })?;
    Ok((
        report,
        scope,
        semantic_pack_near,
        semantic_pack_external_exact,
        line_context,
        retained_normalized,
    ))
}

fn prepare_detection_features(
    corpus: &mut nose_il::Corpus,
    opts: &nose_detect::DetectOptions,
    cached: bool,
    line_context: Option<&cache::CachedLineContext>,
    unit_snapshot: Option<cache::CachedUnitSnapshot>,
    accepted_coverage: AcceptedCoverage,
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
            retained_normalized: None,
        }
    } else if matches!(accepted_coverage, AcceptedCoverage::Direct) {
        let (features, normalized, value_contexts) = time_stage("normalize+extract", || {
            nose_detect::corpus_features_with_normalized(corpus, opts)
        });
        let exact_safety = features
            .units
            .iter()
            .map(|unit| RetainedExactSafety {
                path: unit.path.clone(),
                kind: unit.kind,
                start_line: unit.start_line,
                end_line: unit.end_line,
                exact_safe: unit.exact_safe,
            })
            .collect();
        PreparedDetectionFeatures {
            units: features.units,
            unit_keys: None,
            streams: features.streams,
            files: features.files,
            retained_normalized: Some(RetainedNormalizedCorpus {
                corpus: normalized,
                exact_safety,
                value_contexts,
            }),
        }
    } else {
        let features = time_stage("normalize+extract", || {
            nose_detect::corpus_features(corpus, opts)
        });
        PreparedDetectionFeatures {
            units: features.units,
            unit_keys: None,
            streams: features.streams,
            files: features.files,
            retained_normalized: None,
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
