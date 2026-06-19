use super::query_dashboard::render_query_dashboard;
use super::query_model::*;
use super::query_open::render_query_family;
use super::query_views::*;
use crate::divergence;
use crate::legacy_prelude::*;
use crate::query_dataset::{build_query_dataset, QueryDataset, QuerySettings};

/// The flat family set a report format (`--format markdown`/`sarif`) emits for a query: the
/// single addressed family for `at=`/`id=`, otherwise the same default-surface (or `all`/
/// `surface=`-widened, slice-folded, filtered) selection the list view shows. Report formats
/// are non-interactive, so they collapse the dashboard/group views to this set.
fn query_selection<'a>(
    families: &'a [nose_detect::RefactorFamily],
    ov: &SurfaceOverrides,
    opp: &OpportunityGroups,
    q: &Query,
    path_arg: &str,
    since: Option<&BaselineComparison>,
) -> Result<Vec<&'a nose_detect::RefactorFamily>> {
    if let Some(at) = &q.at {
        let idv = baseline::family_id(family_at(families, at, path_arg)?);
        return Ok(families
            .iter()
            .filter(|f| baseline::family_id(f) == idv)
            .collect());
    }
    if let Some(idv) = &q.id {
        return Ok(families
            .iter()
            .filter(|f| baseline::family_id(f).starts_with(idv.as_str()))
            .collect());
    }
    let widen = q.all || q.filters.iter().any(|flt| flt.field == "surface");
    Ok(families
        .iter()
        .filter(|f| {
            (widen || is_default_surface(f, ov))
                && !opp.is_slice(f)
                && q.filters
                    .iter()
                    .all(|flt| family_matches(f, ov, flt, since))
        })
        .collect())
}

/// The `base=<git-ref>` view: divergent edits (a clone changed in one copy but not its
/// siblings) detected at the ref and surfaced under query. Reuses the same conservative
/// shared-logic policy measured in §BV.
fn run_query_base(args: &QueryArgs, base_ref: &str, q: &Query, path_arg: &str) -> Result<()> {
    validate_base_query(q, args)?;
    // `base=` gates on a diff against a ref, not a saved baseline — `--fail-on new` (which
    // needs `--baseline`) is meaningless here.
    if matches!(args.fail_on, Some(FailOn::New)) {
        anyhow::bail!(
            "`base=` gates on a diff, not a baseline — use `--fail-on any` (fires on a proven divergence)"
        );
    }
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
        ReportFormat::Json => {
            render_query_base(&flagged, changed_files, base_ref, path_arg, q.top, true)
        }
        ReportFormat::Sarif => println!(
            "{}",
            divergence::divergence_sarif(&flagged, q.top, "top=0")?
        ),
        _ => render_query_base(&flagged, changed_files, base_ref, path_arg, q.top, false),
    }
    // The gate fires on the §BV conservative policy: a proven shared-logic divergence.
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

struct QueryOutput<'a> {
    args: &'a QueryArgs,
    terms: &'a [String],
    q: &'a Query,
    path_arg: &'a str,
    families: &'a [nose_detect::RefactorFamily],
    reinvented: &'a [nose_detect::ReinventedHelper],
    scope: &'a QueryScope,
    settings: &'a QuerySettings,
    overrides: &'a SurfaceOverrides,
    opp: &'a OpportunityGroups,
    baseline_comparison: Option<&'a BaselineComparison>,
    since: Option<&'a BaselineComparison>,
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
        .filter(|f| is_default_surface(f, overrides))
        .collect();
    OpportunityGroups::from_ranked(&default_fams)
}

fn render_query_output(ctx: &QueryOutput<'_>) -> Result<bool> {
    match ctx.args.format {
        ReportFormat::Markdown | ReportFormat::Sarif => {
            render_query_report_format(ctx)?;
            Ok(false)
        }
        _ => render_query_exploration(ctx),
    }
}

fn render_query_report_format(ctx: &QueryOutput<'_>) -> Result<()> {
    let selected = query_selection(
        ctx.families,
        ctx.overrides,
        ctx.opp,
        ctx.q,
        ctx.path_arg,
        ctx.since,
    )?;
    let top = query_row_limit(ctx.q.top);
    let shown: Vec<&nose_detect::RefactorFamily> = selected.iter().take(top).copied().collect();
    if matches!(ctx.args.format, ReportFormat::Sarif) {
        println!("{}", refactor_sarif(&shown, selected.len())?);
        return Ok(());
    }
    print_refactor_markdown(
        &selected,
        &shown,
        ctx.settings.channels,
        ctx.baseline_comparison,
        None,
        0,
        None,
    );
    // `id=<fam>` is a single-family drilldown: render the extraction skeleton
    // (and, on `full`, the representative diff) so markdown composes with
    // `id=`/`full` the way the human/JSON views do (#422). Bulk reports stay a
    // compact location list — the skeleton is paid only on drilldown.
    if ctx.q.id.is_some() {
        for f in &shown {
            if f.locations.len() >= 2 {
                markdown_member_proposal(&f.locations);
                if ctx.q.id_full {
                    markdown_member_diff(&f.locations[0], &f.locations[1]);
                }
            }
        }
    }
    Ok(())
}

fn render_query_exploration(ctx: &QueryOutput<'_>) -> Result<bool> {
    let json = matches!(ctx.args.format, ReportFormat::Json);
    if !json {
        print_query_prelude();
    }
    if ctx.q.reinvented {
        render_query_reinvented(ctx.reinvented, ctx.path_arg, ctx.q.top, json);
        return Ok(false);
    }
    if ctx.terms.is_empty() {
        let reinvented_prod = ctx
            .reinvented
            .iter()
            .filter(|r| !r.container_in_test && !r.helper_in_test)
            .count();
        let md = markdown::detect_under(&ctx.args.paths[0], &ctx.settings.exclude);
        let markdown_found = !md.is_empty();
        render_query_dashboard(
            ctx.families,
            ctx.overrides,
            ctx.opp,
            ctx.scope,
            ctx.path_arg,
            reinvented_prod,
            json,
            ctx.since,
            &md,
        );
        return Ok(markdown_found);
    }
    if let Some(at) = &ctx.q.at {
        let idv = baseline::family_id(family_at(ctx.families, at, ctx.path_arg)?);
        render_query_family_view(ctx, &idv, json);
    } else if let Some(idv) = &ctx.q.id {
        render_query_family_view(ctx, idv, json);
    } else {
        render_query_list_or_group(ctx, json)?;
    }
    Ok(false)
}

fn render_query_family_view(ctx: &QueryOutput<'_>, idv: &str, json: bool) {
    render_query_family(
        ctx.families,
        ctx.overrides,
        ctx.opp,
        idv,
        ctx.q.id_full,
        ctx.path_arg,
        json,
        ctx.since,
    );
}

fn render_query_list_or_group(ctx: &QueryOutput<'_>, json: bool) -> Result<()> {
    let widen = ctx.q.all || ctx.q.filters.iter().any(|flt| flt.field == "surface");
    let sel = query_selection(
        ctx.families,
        ctx.overrides,
        ctx.opp,
        ctx.q,
        ctx.path_arg,
        ctx.since,
    )?;
    match &ctx.q.group {
        Some(field) => render_query_group(&sel, field, ctx.terms, ctx.path_arg, json, ctx.since),
        None => render_query_list(
            &sel,
            ctx.overrides,
            ctx.opp,
            ctx.q,
            ctx.terms,
            ctx.path_arg,
            widen,
            json,
            ctx.since,
        ),
    }
    Ok(())
}

fn enforce_query_fail_on(ctx: &QueryOutput<'_>) -> Result<()> {
    let reportable = if ctx.q.reinvented {
        Vec::new()
    } else {
        query_selection(
            ctx.families,
            ctx.overrides,
            ctx.opp,
            ctx.q,
            ctx.path_arg,
            ctx.since,
        )?
        .into_iter()
        .filter(|f| is_default_report_family(f, ctx.overrides))
        .collect()
    };
    enforce_query_fail_on_selection(
        ctx.args,
        ctx.settings.channels,
        &reportable,
        ctx.baseline_comparison,
    );
    Ok(())
}

pub(super) fn run_query_cmd(cmd: Cmd) -> Result<()> {
    let Cmd::Query {
        path,
        terms,
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
    require_paths_exist(std::slice::from_ref(&path))?;
    let q = parse_query(&terms)?;
    // The path as the user typed it — every suggested next-command echoes it so the links
    // are runnable verbatim (the surface takes the path positionally).
    let path_arg = path.to_string_lossy().into_owned();
    let args = QueryArgs {
        paths: vec![path],
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
    let output = QueryOutput {
        args: &args,
        terms: &terms,
        q: &q,
        path_arg: &path_arg,
        families: &dataset.families,
        reinvented: &dataset.reinvented,
        scope: &dataset.scope,
        settings: &dataset.settings,
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
