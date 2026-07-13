use super::query_dashboard::render_query_dashboard;
use super::query_model::*;
use super::query_open::render_query_family;
use super::query_views::*;
use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::cli_args::QueryArgs;
use crate::markdown;
use crate::query_baseline_gate::enforce_query_fail_on_selection;
use crate::query_dataset::QuerySettings;
use crate::query_markdown;
use crate::query_opportunities::OpportunityGroups;
use crate::query_options::{QueryScope, ReportFormat};
use crate::query_sarif::refactor_sarif;
use crate::query_terms::{family_at, QOp, Query};
use crate::surfaces::{is_default_report_family, SurfaceOverrides};
use anyhow::Result;

pub(super) struct QueryOutput<'a> {
    pub(super) args: &'a QueryArgs,
    pub(super) terms: &'a [String],
    pub(super) q: &'a Query,
    pub(super) path_arg: &'a str,
    pub(super) families: &'a [nose_detect::RefactorFamily],
    pub(super) reinvented: &'a [nose_detect::ReinventedHelper],
    pub(super) scope: &'a QueryScope,
    pub(super) settings: &'a QuerySettings,
    pub(super) semantic_packs: &'a [serde_json::Value],
    pub(super) overrides: &'a SurfaceOverrides,
    pub(super) opp: &'a OpportunityGroups,
    pub(super) baseline_comparison: Option<&'a BaselineComparison>,
    pub(super) since: Option<&'a BaselineComparison>,
}

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
    let default_folds = query_uses_default_folds(q, widen);
    Ok(families
        .iter()
        .filter(|f| {
            (widen || is_default_surface(f, ov))
                && !(if default_folds {
                    opp.is_default_slice(f)
                } else {
                    opp.is_slice(f)
                })
                && q.filters
                    .iter()
                    .all(|flt| family_matches(f, ov, flt, since))
        })
        .collect())
}

fn query_uses_default_folds(q: &Query, widen: bool) -> bool {
    !widen
        || q.filters.iter().any(|filter| {
            filter.field == "surface"
                && matches!(filter.op, QOp::Eq)
                && !filter.negate
                && filter.value == "default"
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_terms::QFilter;

    #[test]
    fn explicit_default_surface_uses_default_only_folds() {
        let query = Query {
            filters: vec![QFilter {
                field: "surface".into(),
                op: QOp::Eq,
                value: "default".into(),
                negate: false,
            }],
            ..Query::default()
        };
        let widen = query.filters.iter().any(|filter| filter.field == "surface");
        let default_folds = query_uses_default_folds(&query, widen);

        assert!(widen, "surface filters still open the non-default universe");
        assert!(
            default_folds,
            "an explicit default filter must not fold under a generated primary"
        );
    }
}

pub(super) fn render_query_output(ctx: &QueryOutput<'_>) -> Result<bool> {
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
    query_markdown::print_refactor_markdown(
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
                query_markdown::markdown_member_proposal(&f.locations);
                if ctx.q.id_full {
                    query_markdown::markdown_member_diff(&f.locations[0], &f.locations[1]);
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
        render_query_reinvented(
            ctx.reinvented,
            ctx.path_arg,
            ctx.q.top,
            json,
            ctx.semantic_packs,
        );
        return Ok(false);
    }
    if ctx.terms.is_empty() {
        let reinvented_prod = ctx
            .reinvented
            .iter()
            .filter(|r| !r.container_in_test && !r.helper_in_test)
            .count();
        let markdown_report =
            markdown::QueryMarkdownReport::detect_under(&ctx.args.paths, &ctx.settings.exclude);
        let markdown_found = markdown_report.has_findings();
        render_query_dashboard(
            ctx.families,
            ctx.overrides,
            ctx.opp,
            ctx.scope,
            ctx.path_arg,
            reinvented_prod,
            json,
            ctx.baseline_comparison,
            ctx.since,
            &markdown_report,
            ctx.semantic_packs,
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
        ctx.baseline_comparison,
        ctx.since,
        ctx.semantic_packs,
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
        Some(field) => render_query_group(
            &sel,
            field,
            ctx.terms,
            ctx.path_arg,
            json,
            ctx.baseline_comparison,
            ctx.since,
            ctx.semantic_packs,
        ),
        None => render_query_list(
            &sel,
            ctx.overrides,
            ctx.opp,
            ctx.q,
            ctx.terms,
            ctx.path_arg,
            widen,
            json,
            ctx.baseline_comparison,
            ctx.since,
            ctx.semantic_packs,
        ),
    }
    Ok(())
}

pub(super) fn enforce_query_fail_on(ctx: &QueryOutput<'_>) -> Result<()> {
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
