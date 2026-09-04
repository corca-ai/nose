use super::*;

pub(super) fn try_query_detect_report_fast(
    request: &QueryDetectRequest<'_>,
) -> Option<Result<DetectionReport>> {
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
) -> Result<DetectionReport> {
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
    })?;
    Ok((
        report,
        QueryScope::from_langs(langs).with_sources(&source_files),
        nose_semantics::SemanticPackNearRegistry::default(),
        nose_semantics::SemanticPackExternalExactRegistry::default(),
        Some(cache::CachedLineContext {
            source_files: source_files.into(),
            run,
        }),
        None,
    ))
}

pub(super) fn print_invalidation(report: Option<&cache::InvalidationReport>) {
    if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        if let Some(report) = report {
            eprintln!(
                "  [invalidation] {}",
                cache::invalidation_report_json(report)
            );
        }
    }
}

pub(super) struct DetectCachedRequest<'a> {
    pub(super) cache_identity_parts: Option<([u8; 32], [u8; 32])>,
    pub(super) cache_run: Option<&'a cache::CacheRun>,
    pub(super) detection_units: (Vec<nose_detect::UnitFeat>, Option<&'a [[u8; 32]]>),
    pub(super) files: usize,
    pub(super) streams: &'a [nose_detect::Stream],
    pub(super) opts: &'a nose_detect::DetectOptions,
    pub(super) detector: &'a dyn nose_detect::Detector,
    pub(super) accepted_coverage: AcceptedCoverage,
}

pub(super) fn detect_cached_or_clean(
    request: DetectCachedRequest<'_>,
) -> Result<nose_detect::Report> {
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
    crate::detect_pipeline::ensure_candidate_budget(&units, opts)?;
    if matches!(accepted_coverage, AcceptedCoverage::Direct) {
        return Ok(
            nose_detect::detect_from_units_with_direct_accepted_coverage(
                units, files, streams, opts, detector,
            ),
        );
    }
    let (Some(run), Some((workspace, pack_digest))) = (cache_run, cache_identity_parts) else {
        return Ok(nose_detect::detect_from_units_with_accepted_coverage(
            units, files, streams, opts, detector,
        ));
    };
    let identity = cache::DetectionCacheIdentity::new(workspace, pack_digest, opts, detector);
    let previous = cache::load_detection_state(run, &identity);
    // Building the persistent bucket graph is deliberately more expensive than
    // the clean sort-based detector. Above this bound its first-generation cost
    // and state write exceed the savings from many no-op process invocations;
    // the unit cache still avoids normalization, while watch sessions retain
    // their in-memory incremental detector without this cap.
    if previous.is_none() && units.len() > MAX_PERSISTENT_DETECTION_UNITS {
        return Ok(nose_detect::detect_from_units_with_accepted_coverage(
            units, files, streams, opts, detector,
        ));
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
    Ok(report)
}
