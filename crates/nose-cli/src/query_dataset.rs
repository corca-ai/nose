use anyhow::Result;

use crate::cli_args::QueryArgs;
use crate::detect_pipeline::{detection_engine, detection_options, validate_exclude_globs};
use crate::path_utils::{relativize, relativize_loc};
use crate::query_options::{
    validate_min_value, DetectionChannels, QueryScope, SortKey, QUERY_DEFAULT_MODES,
};
use crate::source_lines::{
    corpus_line_idf, family_anchor, is_trivial_line, shared_lines_of, varying_spots_of,
    FileLineCache,
};
use crate::timing::{time_lower, time_stage};
use crate::{cache, config, ignores};

/// The ranked family dataset behind `nose query`: detect, rank,
/// filter (min-members / min-value / scope), relativize paths, weight shared lines, and
/// sort. It stops before query view selection, structured ignores, surface classification,
/// rendering, and the CI gate so each query view can apply those layers deterministically.
pub(super) struct QueryDataset {
    pub(super) families: Vec<nose_detect::RefactorFamily>,
    pub(super) scope: QueryScope,
    pub(super) settings: QuerySettings,
    pub(super) semantic_packs: nose_semantics::SemanticPackSet,
    pub(super) reinvented: Vec<nose_detect::ReinventedHelper>,
    pub(super) opts: nose_detect::DetectOptions,
}

pub(super) fn build_query_dataset(
    args: &QueryArgs,
    refs: &[&std::path::Path],
) -> Result<QueryDataset> {
    let (settings, semantic_packs) = resolve_query_settings(args)?;
    let opts = detection_options(settings.channels, settings.min_tokens, settings.min_lines);
    let detector = detection_engine(settings.channels, &opts);
    let (mut report, scope) =
        query_detect_report(args, refs, &settings.exclude, &opts, detector.as_ref());

    let mut families = time_stage("rank_families", || nose_detect::rank_families(&report));
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
        weight_shared_lines(&mut families, refs, &settings.exclude)
    });
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
    Ok(QueryDataset {
        families,
        scope,
        settings,
        semantic_packs,
        reinvented,
        opts,
    })
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
    pub(super) ignore_set: Option<ignores::IgnoreSet>,
}

pub(super) fn resolve_query_semantic_packs(
    args: &QueryArgs,
) -> Result<nose_semantics::SemanticPackSet> {
    let cfg = config::load_query(args.config.as_deref())?;
    semantic_pack_set_from_paths(cfg.semantic_packs, &args.semantic_pack)
}

fn semantic_pack_set_from_paths(
    mut semantic_pack_paths: Vec<std::path::PathBuf>,
    cli_semantic_pack_paths: &[std::path::PathBuf],
) -> Result<nose_semantics::SemanticPackSet> {
    semantic_pack_paths.extend(cli_semantic_pack_paths.iter().cloned());
    Ok(nose_semantics::SemanticPackSet::new_local(
        &semantic_pack_paths,
    )?)
}

fn resolve_query_settings(
    args: &QueryArgs,
) -> Result<(QuerySettings, nose_semantics::SemanticPackSet)> {
    let cfg = config::load_query(args.config.as_deref())?;
    let min_members = args.min_members.or(cfg.min_members).unwrap_or(2);
    let min_value = validate_min_value(args.min_value.or(cfg.min_value).unwrap_or(0.0))?;
    let sort = args.sort.or(cfg.sort).unwrap_or(SortKey::Extractability);
    let channels = DetectionChannels::resolve(args.mode.clone(), cfg.mode, QUERY_DEFAULT_MODES)?;
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let ignore_file = args.ignore_file.clone().or(cfg.ignore_file);
    let semantic_packs = semantic_pack_set_from_paths(cfg.semantic_packs, &args.semantic_pack)?;
    // Excludes are additive: config patterns plus any given on the command line.
    let mut exclude = cfg.exclude;
    exclude.extend(args.exclude.iter().cloned());
    validate_exclude_globs(&exclude)?;
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
            ignore_set,
        },
        semantic_packs,
    ))
}

/// With --cache-dir, build units per file through the on-disk cache (skips
/// parse/normalize/extract for unchanged files); otherwise lower the whole corpus.
fn query_detect_report(
    args: &QueryArgs,
    refs: &[&std::path::Path],
    exclude: &[String],
    opts: &nose_detect::DetectOptions,
    detector: &dyn nose_detect::Detector,
) -> (nose_detect::Report, QueryScope) {
    if let Some(dir) = &args.cache_dir {
        // Lower AND cross-file-resolve the corpus every run (the smaller half of
        // the work, §BQ), then cache only the dominant normalize+extract step
        // keyed on the post-resolve IL. This makes the cached query identical to
        // the non-cached path including imported-immutable-literal convergence
        // (#275), which the old per-file source-content cache skipped.
        let corpus = time_lower(|| nose_frontend::lower_corpus_filtered(refs, exclude));
        let scope = QueryScope::from_corpus(&corpus);
        let cache::CachedUnits {
            units,
            streams,
            files,
        } = cache::build_units_cached(&corpus, opts, dir);
        let report = nose_detect::detect_from_units_with_accepted_coverage(
            units, files, &streams, opts, detector,
        )
        .0;
        (report, scope)
    } else {
        let corpus = time_lower(|| nose_frontend::lower_corpus_filtered(refs, exclude));
        let scope = QueryScope::from_corpus(&corpus);
        (
            nose_detect::detect_with_accepted_coverage(&corpus, opts, detector),
            scope,
        )
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
) {
    let needs_shared = |f: &nose_detect::RefactorFamily| f.languages == 1 && f.locations.len() >= 2;
    if !families.iter().any(needs_shared) {
        return;
    }
    let mut lines = FileLineCache::default();
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
