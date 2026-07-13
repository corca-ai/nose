use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::cli_args::{Cmd, QueryArgs, ScopeFilter};
use crate::divergence;
use crate::path_utils::{paths_as_refs, require_paths_exist, warn_no_files};
use crate::query_baseline_gate::{
    apply_query_baseline, compare_since, partition_ignored, write_query_baseline,
};
use crate::query_dataset::{build_query_dataset, resolve_query_semantic_packs, QueryDataset};
use crate::query_opportunities::OpportunityGroups;
use crate::query_options::{FailOn, ReportFormat};
use crate::query_output::{enforce_query_fail_on, render_query_output, QueryOutput};
use crate::query_semantic_packs::semantic_packs_json;
use crate::query_terms::{parse_query, Query};
use crate::query_views::render_query_base;
use crate::query_witness::enrich_graded_witnesses;
use crate::source_lines::family_anchor;
use crate::surfaces::{
    classify_surface_overrides, is_default_opportunity_family, is_default_report_family,
    SurfaceOverrides,
};
use crate::timing::time_stage;
use anyhow::Result;
use std::path::PathBuf;

/// The `base=<git-ref>` view: divergent edits (a clone changed in one copy but not its
/// siblings) detected at the ref and surfaced under query. Default CI gates only on the
/// strict tier; broader divergences remain visible as review/report-only evidence.
fn run_query_base(args: &QueryArgs, base_ref: &str, q: &Query, path_arg: &str) -> Result<()> {
    validate_base_query(q, args)?;
    // `base=` gates on a diff against a ref, not a saved baseline — `--fail-on new` (which
    // needs `--baseline`) is meaningless here.
    if matches!(args.fail_on, Some(FailOn::New)) {
        anyhow::bail!(
            "`base=` gates on a diff, not a baseline — use `--fail-on any` (fires on unsuppressed strict divergences)"
        );
    }
    let semantic_packs_json = if matches!(args.format, ReportFormat::Json) {
        semantic_packs_json(&resolve_query_semantic_packs(args)?)
    } else {
        Vec::new()
    };
    let divergence_args = divergence::DivergenceArgs {
        paths: args.paths.clone(),
        base: base_ref.to_string(),
        mode: args.mode.clone(),
        min_size: args.min_size,
        min_lines: args.min_lines,
        exclude: args.exclude.clone(),
        config: args.config.clone(),
        ignore_file: args.ignore_file.clone(),
    };
    let (flagged, changed_files) =
        divergence::detect_divergences(&divergence_args)?.unwrap_or_default();
    match args.format {
        ReportFormat::Json => render_query_base(
            &flagged,
            changed_files,
            base_ref,
            path_arg,
            q.top,
            true,
            &semantic_packs_json,
        ),
        ReportFormat::Sarif => println!(
            "{}",
            divergence::divergence_sarif(&flagged, q.top, "top=0")?
        ),
        _ => render_query_base(
            &flagged,
            changed_files,
            base_ref,
            path_arg,
            q.top,
            false,
            &semantic_packs_json,
        ),
    }
    // The default gate fires on the v2 strict tier.
    if matches!(args.fail_on, Some(FailOn::Any)) && divergence::divergences_fire(&flagged) {
        std::process::exit(1);
    }
    Ok(())
}

fn validate_base_query(q: &Query, args: &QueryArgs) -> Result<()> {
    let unsupported_terms = q.reinvented
        || q.all
        || q.id_full
        || q.group.is_some()
        || q.id.is_some()
        || q.at.is_some()
        || q.since.is_some()
        || q.sort.is_some()
        || !q.filters.is_empty();
    if unsupported_terms {
        anyhow::bail!(
            "`base=` is its own divergent-edit view; combine it only with `top=N`, detection flags, `--format`, or `--fail-on any`"
        );
    }
    let mut unsupported_flags = Vec::new();
    if args.min_members.is_some() {
        unsupported_flags.push("--min-members");
    }
    if args.min_value.is_some() {
        unsupported_flags.push("--min-value");
    }
    if args.cache_dir.is_some() {
        unsupported_flags.push("--cache-dir");
    }
    let cfg = crate::config::load_query(args.config.as_deref())?;
    if !cfg.semantic_packs.is_empty() {
        unsupported_flags.push("semantic-packs config");
    }
    if !args.semantic_pack.is_empty() {
        unsupported_flags.push("--semantic-pack");
    }
    if args.baseline.is_some() {
        unsupported_flags.push("--baseline");
    }
    if args.write_baseline {
        unsupported_flags.push("--write-baseline");
    }
    if !unsupported_flags.is_empty() {
        anyhow::bail!(
            "`base=` does not support {}; combine it only with `top=N`, detection flags, `--format`, or `--fail-on any`",
            unsupported_flags.join(", ")
        );
    }
    Ok(())
}

fn ensure_query_fail_on_is_valid(args: &QueryArgs) -> Result<()> {
    if matches!(args.fail_on, Some(FailOn::New)) && args.baseline.is_none() {
        anyhow::bail!(
            "--fail-on new requires --baseline (it gates on families new vs the baseline)"
        );
    }
    Ok(())
}

fn activate_query_families(
    args: &QueryArgs,
    dataset: &mut QueryDataset,
) -> Result<Option<BaselineComparison>> {
    let baseline_comparison = apply_query_baseline(args, &mut dataset.families)?;
    let ignore_set = dataset.settings.ignore_set.take();
    dataset.families =
        partition_ignored(std::mem::take(&mut dataset.families), ignore_set.as_ref());
    Ok(baseline_comparison)
}

fn query_needs_spotclass(q: &Query) -> bool {
    q.group.as_deref() == Some("spotclass") || q.filters.iter().any(|flt| flt.field == "spotclass")
}

fn query_uses_status(q: &Query) -> bool {
    q.group.as_deref() == Some("status") || q.filters.iter().any(|flt| flt.field == "status")
}

fn query_since<'a>(
    q: &Query,
    families: &[nose_detect::RefactorFamily],
    slot: &'a mut Option<BaselineComparison>,
) -> Result<Option<&'a BaselineComparison>> {
    if query_uses_status(q) && q.since.is_none() {
        anyhow::bail!("`status` needs a snapshot — add `since=<baseline-file>` (write one with `--write-baseline`)");
    }
    *slot = match &q.since {
        Some(p) => Some(compare_since(p, families)?),
        None => None,
    };
    Ok(slot.as_ref())
}

fn sort_query_families(q: &Query, families: &mut [nose_detect::RefactorFamily]) {
    if let Some(sk) = q.sort {
        families.sort_by(|a, b| {
            sk.score(b)
                .total_cmp(&sk.score(a))
                .then(b.value.total_cmp(&a.value))
                .then_with(|| family_anchor(a).cmp(&family_anchor(b)))
        });
    }
}

fn query_opportunities(
    families: &[nose_detect::RefactorFamily],
    overrides: &SurfaceOverrides,
) -> OpportunityGroups {
    let default_fams: Vec<&nose_detect::RefactorFamily> = families
        .iter()
        .filter(|f| is_default_opportunity_family(f, overrides))
        .collect();
    let mut groups = OpportunityGroups::from_ranked(&default_fams);
    let default_ids = families
        .iter()
        .filter(|family| is_default_report_family(family, overrides))
        .map(baseline::family_id)
        .collect();
    groups.restrict_default_slices_to(&default_ids);
    groups
}

fn discard_accepted_coverage(families: &mut [nose_detect::RefactorFamily]) {
    for family in families {
        family.accepted_coverage.clear();
    }
}

fn semantic_packs_for_output(
    format: ReportFormat,
    dataset: &QueryDataset,
) -> Vec<serde_json::Value> {
    if matches!(format, ReportFormat::Json) {
        semantic_packs_json(&dataset.semantic_packs)
    } else {
        Vec::new()
    }
}

fn split_query_roots_and_terms(
    roots: Vec<PathBuf>,
    positionals: Vec<String>,
) -> Result<(Vec<PathBuf>, Vec<String>, String, bool)> {
    if roots.is_empty() {
        let Some((path, terms)) = positionals.split_first() else {
            anyhow::bail!(
                "`nose query` needs a root path — use `nose query <path>` or `nose query --root <path>`"
            );
        };
        return Ok((
            vec![PathBuf::from(path)],
            terms.to_vec(),
            path.to_string(),
            false,
        ));
    }
    let path_arg = roots
        .iter()
        .map(|root| format!("-r {}", root.display()))
        .collect::<Vec<_>>()
        .join(" ");
    Ok((roots, positionals, path_arg, true))
}

fn parse_query_with_path_hint(
    terms: &[String],
    roots: &[PathBuf],
    path_arg: &str,
    roots_are_explicit: bool,
) -> Result<Query> {
    match parse_query(terms) {
        Ok(q) => Ok(q),
        Err(err) => {
            if let Some(term) = terms
                .iter()
                .find(|term| std::path::Path::new(term).exists())
            {
                if roots_are_explicit {
                    anyhow::bail!(
                        "{err}\n\
                         `{term}` looks like a path. When using `--root`/`-r`, pass every analyzed path with `--root <path>` or `-r <path>`; bare arguments are query terms."
                    );
                }
                let first = roots
                    .first()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| path_arg.to_string());
                anyhow::bail!(
                    "{err}\n\
                     `{term}` looks like another path. `nose query` accepts one positional root; pass multiple roots explicitly, for example `nose query -r {first} -r {term}`."
                );
            }
            Err(err)
        }
    }
}

pub(super) fn run_query_cmd(cmd: Cmd) -> Result<()> {
    let Cmd::Query {
        roots,
        positionals,
        format,
        mode,
        min_size,
        min_lines,
        min_value,
        min_members,
        exclude,
        cache_dir,
        ignore_file,
        semantic_pack,
        config,
        fail_on,
        baseline,
        write_baseline,
    } = cmd
    else {
        unreachable!("run_query_cmd requires Cmd::Query")
    };
    let (paths, terms, path_arg, roots_are_explicit) =
        split_query_roots_and_terms(roots, positionals)?;
    require_paths_exist(&paths)?;
    let q = parse_query_with_path_hint(&terms, &paths, &path_arg, roots_are_explicit)?;
    // The path as the user typed it — every suggested next-command echoes it so the links
    // are runnable verbatim. Multi-root commands echo the explicit root flags.
    let args = QueryArgs {
        paths,
        min_members,
        min_value,
        sort: None,
        config,
        mode,
        cache_dir,
        fail_on,
        baseline,
        ignore_file,
        semantic_pack,
        write_baseline,
        format,
        exclude,
        min_size,
        min_lines,
        scope: ScopeFilter::All,
    };
    ensure_query_fail_on_is_valid(&args)?;
    if let Some(base_ref) = &q.base {
        return run_query_base(&args, base_ref, &q, &path_arg);
    }

    let refs = paths_as_refs(&args.paths);
    let mut dataset = build_query_dataset(&args, &refs)?;
    if args.write_baseline {
        return write_query_baseline(&args, &dataset.families);
    }
    let baseline_comparison = time_stage("query_activate", || {
        activate_query_families(&args, &mut dataset)
    })?;
    let overrides = time_stage("query_surface", || {
        classify_surface_overrides(&mut dataset.families)
    });
    if query_needs_spotclass(&q) {
        time_stage("query_spot", || {
            enrich_graded_witnesses(&mut dataset.families, &dataset.opts)
        });
    }
    let mut since_cmp = None;
    let since = time_stage("query_since", || {
        query_since(&q, &dataset.families, &mut since_cmp)
    })?;
    time_stage("query_sort", || {
        sort_query_families(&q, &mut dataset.families)
    });
    let opp = time_stage("query_opp", || {
        query_opportunities(&dataset.families, &overrides)
    });
    // Accepted-edge graphs are needed only to decide the fold forest. Drop the
    // potentially large internal provenance before list selection and JSON
    // rendering; it is intentionally absent from the product schema.
    discard_accepted_coverage(&mut dataset.families);
    let semantic_packs_json = semantic_packs_for_output(args.format, &dataset);
    let output = QueryOutput {
        args: &args,
        terms: &terms,
        q: &q,
        path_arg: &path_arg,
        families: &dataset.families,
        reinvented: &dataset.reinvented,
        scope: &dataset.scope,
        settings: &dataset.settings,
        semantic_packs: &semantic_packs_json,
        overrides: &overrides,
        opp: &opp,
        baseline_comparison: baseline_comparison.as_ref(),
        since,
    };
    let markdown_found = time_stage("query_render", || render_query_output(&output))?;
    if dataset.scope.files == 0 && !markdown_found {
        warn_no_files(&args.paths);
    }
    time_stage("query_gate", || enforce_query_fail_on(&output))?;
    Ok(())
}
