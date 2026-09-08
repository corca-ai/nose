use super::*;
use crate::query_options::DIVERGENCE_DEFAULT_MODES;

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
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines)?;
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
    Option<RetainedNormalizedCorpus>,
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
        retained_normalized,
    ) = query_detect_report(QueryDetectRequest {
        args,
        refs,
        exclude: &settings.exclude,
        opts: &opts,
        detector: detector.as_ref(),
        semantic_packs: &semantic_packs,
        cache_max_bytes: settings.cache_max_bytes,
        accepted_coverage: AcceptedCoverage::Direct,
    })?;
    let mut families = nose_detect::rank_families(&report);
    annotate_semantic_pack_near(&mut families, &semantic_pack_near);
    annotate_semantic_pack_external_exact(&mut families, &semantic_pack_external_exact);
    if settings.channels.abstraction_only() {
        families.retain(|family| family.abstraction_witness.is_some());
    }
    time_stage("cache_commit", || cache::finish_query_run(line_context));
    Ok((families, opts, retained_normalized))
}
