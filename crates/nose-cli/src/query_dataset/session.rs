use super::*;

pub(crate) struct QueryAnalysisSession {
    units: cache::FastUnitSession,
    opts: nose_detect::DetectOptions,
    detection_state: Option<nose_detect::IncrementalDetectionState>,
    initial_invalidation: Option<cache::InvalidationReport>,
}

pub(crate) struct QueryAnalysisUpdate {
    pub(crate) dataset: QueryDataset,
    pub(crate) invalidation: cache::InvalidationReport,
    pub(crate) source_set_digest: String,
}

impl QueryAnalysisSession {
    pub(crate) fn open(args: &QueryArgs, refs: &[&std::path::Path]) -> Result<Option<Self>> {
        let (settings, semantic_packs) = resolve_query_settings(args, QUERY_DEFAULT_MODES)?;
        let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines)?;
        let detector = detection_engine(settings.channels, &opts);
        let Some(dir) = args.cache_dir.as_ref() else {
            return Ok(None);
        };
        let Some(fast) = cache::try_build_units_fast(
            refs,
            &settings.exclude,
            dir,
            &semantic_packs,
            settings.cache_max_bytes,
            &opts,
        ) else {
            return Ok(None);
        };
        let mut units = cache::FastUnitSession::from_fast(fast);
        let identity = cache::DetectionCacheIdentity::new(
            units.workspace_digest(),
            units.semantic_pack_digest(),
            &opts,
            detector.as_ref(),
        );
        let detection_state = cache::load_detection_state(units.run(), &identity);
        let initial_invalidation = units.take_initial_report();
        Ok(Some(Self {
            units,
            opts,
            detection_state,
            initial_invalidation,
        }))
    }

    pub(crate) fn refresh_leaf(
        &mut self,
        args: &QueryArgs,
        refs: &[&std::path::Path],
        path: &std::path::Path,
    ) -> Result<Option<QueryAnalysisUpdate>> {
        let (settings, semantic_packs) = resolve_query_settings(args, QUERY_DEFAULT_MODES)?;
        let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines)?;
        if opts != self.opts {
            return Ok(None);
        }
        let Some(refresh) = self.units.refresh_leaf(path, &opts) else {
            return Ok(None);
        };
        let dataset = self.analyze(args, refs, settings, semantic_packs, opts)?;
        Ok(Some(QueryAnalysisUpdate {
            dataset,
            invalidation: refresh.invalidation,
            source_set_digest: refresh.source_set_digest,
        }))
    }

    pub(crate) fn current_dataset(
        &mut self,
        args: &QueryArgs,
        refs: &[&std::path::Path],
    ) -> Result<QueryDataset> {
        let (settings, semantic_packs) = resolve_query_settings(args, QUERY_DEFAULT_MODES)?;
        let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines)?;
        anyhow::ensure!(
            opts == self.opts,
            "watch analysis options changed during startup"
        );
        self.analyze(args, refs, settings, semantic_packs, opts)
    }

    fn analyze(
        &mut self,
        args: &QueryArgs,
        refs: &[&std::path::Path],
        settings: QuerySettings,
        semantic_packs: nose_semantics::SemanticPackSet,
        opts: nose_detect::DetectOptions,
    ) -> Result<QueryDataset> {
        let detector = detection_engine(settings.channels, &opts);
        crate::detect_pipeline::ensure_candidate_budget(
            self.units.units(),
            &opts,
            args.max_candidate_pairs,
        )?;
        let report = if nose_detect::prefers_batched_detection(self.units.units(), &opts) {
            self.detection_state = None;
            nose_detect::detect_from_borrowed_units_with_accepted_coverage(
                self.units.units(),
                self.units.files(),
                self.units.streams(),
                &opts,
                detector.as_ref(),
            )
        } else {
            let (report, state, stats) =
                nose_detect::detect_from_units_incremental_session_with_accepted_coverage(
                    self.units.units(),
                    self.units.files(),
                    self.units.streams(),
                    &opts,
                    detector.as_ref(),
                    self.detection_state.take(),
                    Some(self.units.unit_keys()),
                );
            self.detection_state = Some(state);
            if std::env::var_os("NOSE_CACHE_STATS").is_some() {
                eprintln!(
                    "  [detection] {}",
                    cache::incremental_detection_stats_json(&stats)
                );
            }
            report
        };
        let detection = (
            report,
            QueryScope::from_langs(self.units.langs().to_vec())
                .with_sources(&self.units.line_context().source_files),
            nose_semantics::SemanticPackNearRegistry::default(),
            nose_semantics::SemanticPackExternalExactRegistry::default(),
            Some(self.units.line_context()),
            None,
        );
        finish_query_dataset(args, refs, settings, semantic_packs, opts, detection, false)
    }

    pub(crate) fn source_set_digest(&self) -> String {
        self.units.source_set_digest()
    }

    pub(crate) fn take_initial_invalidation(&mut self) -> Option<cache::InvalidationReport> {
        self.initial_invalidation.take()
    }

    pub(crate) fn source_path_for_event(
        &self,
        path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        self.units.source_path_for_event(path)
    }
}
