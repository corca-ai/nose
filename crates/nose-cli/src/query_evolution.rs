//! Query exploration over explicitly captured analysis populations.
mod capture;
mod details;
mod items;
mod navigation;
mod render;
mod reviews;
mod selection;
mod sources;
mod view;
use crate::cli_args::{Cmd, QueryArgs};
use crate::query_options::ReportFormat;
use anyhow::{ensure, Result};
pub(crate) use capture::capture;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct AnalysisArgs {
    /// Save all admitted code families before presentation/ignore filtering. Never overwrites.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["before", "after"])]
    pub(crate) save_analysis: Option<PathBuf>,
    /// Compare saved analyses offline; source reads require explicit --before-source/--after-source.
    #[arg(long, value_name = "FILE", requires = "after")]
    pub(crate) before: Option<PathBuf>,
    /// Later saved analysis to compare; accepts the same capture format as --before.
    #[arg(long, value_name = "FILE", requires = "before")]
    pub(crate) after: Option<PathBuf>,
    /// Directory representing the before capture's path base; read and verify selected source bytes.
    #[arg(long, requires = "before", value_name = "DIR")]
    pub(crate) before_source: Option<PathBuf>,
    /// Directory representing the after capture's path base; read and verify selected source bytes.
    #[arg(long, requires = "before", value_name = "DIR")]
    pub(crate) after_source: Option<PathBuf>,
    /// Explicit caller-owned review record to evaluate; repeat for several records.
    #[arg(long, requires = "before", value_name = "FILE")]
    pub(crate) reviews: Vec<PathBuf>,
    /// Record a decision for one change=ID's current family in a new file. Never overwrites.
    #[arg(long, requires_all = ["before", "decision", "reason"], value_name = "FILE")]
    pub(crate) write_review: Option<PathBuf>,
    /// Caller decision; records intent without suppressing findings or authorizing edits.
    #[arg(long, requires = "write_review", value_enum)]
    pub(crate) decision: Option<reviews::Decision>,
    /// Why the selected current family should receive this decision.
    #[arg(long, requires = "write_review")]
    pub(crate) reason: Option<String>,
    /// Total region/family candidate budget for --before/--after; top=N only limits display.
    #[arg(long, requires = "before", default_value_t = 100_000)]
    pub(crate) max_candidates: usize,
}

pub(crate) fn validate_capture(
    options: &AnalysisArgs,
    args: &QueryArgs,
    terms: &[String],
    other: bool,
) -> Result<()> {
    if options.save_analysis.is_some() {
        ensure!(terms.is_empty() && !other && args.fail_on.is_none() && args.baseline.is_none()
            && !args.write_baseline && args.ignore_file.is_none(),
            "--save-analysis captures the complete admitted population; query terms, watch, show-config, gates, baselines and explicit ignores are not supported");
        ensure!(
            matches!(args.format, ReportFormat::Human | ReportFormat::Json),
            "analysis capture supports --format human or json"
        );
    }
    Ok(())
}

pub(crate) fn try_compare(cmd: &Cmd) -> Result<bool> {
    let Cmd::Query {
        analysis,
        positionals,
        format,
        roots,
        watch,
        mode,
        limits,
        exclude,
        generated_path,
        cache_dir,
        cache_max_bytes,
        ignore_file,
        semantic_pack,
        semantic_pack_lock,
        config,
        config_root,
        show_config,
        fail_on,
        baseline,
        write_baseline,
    } = cmd
    else {
        return Ok(false);
    };
    let crate::cli_args::QueryLimits {
        max_candidate_pairs,
        min_size,
        min_lines,
        min_value,
        min_members,
    } = limits.as_ref();
    let Some(before) = &analysis.before else {
        return Ok(false);
    };
    ensure!(roots.is_empty() && !watch && mode.is_empty() && min_size.is_none()
        && min_lines.is_none() && min_value.is_none() && min_members.is_none() && exclude.is_empty()
        && max_candidate_pairs.is_none() && generated_path.is_empty() && cache_dir.is_none() && cache_max_bytes.is_none()
        && ignore_file.is_none() && semantic_pack.is_empty() && semantic_pack_lock.is_none()
        && config.is_none() && !config_root && !show_config && fail_on.is_none()
        && baseline.is_none() && !write_baseline,
        "analysis comparison uses its captured roots/profile; pass only --before, --after, --max-candidates, --format and comparison terms");
    ensure!(
        matches!(format, ReportFormat::Human | ReportFormat::Json),
        "analysis comparison supports --format human or json"
    );
    let after = analysis.after.as_ref().expect("clap requires --after");
    view::run(before, after, analysis, positionals, *format)?;
    Ok(true)
}

/// Discover the comparison grammar without scraping help text.
pub(crate) fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "capture": "nose query <path> --save-analysis FILE",
        "compare": "nose query --before FILE --after FILE [terms...]",
        "views": ["dashboard", "list", "group", "change"],
        "fields": selection::FIELDS, "reason_values": selection::REASONS,
        "correspondence_values": selection::KINDS, "evidence_values": ["retained", "recheck"],
        "witness_values": selection::WITNESSES,
        "terms": ["group=FIELD", "change=ID", "FIELD=VALUE", "FIELD!=VALUE", "path~TEXT", "path!~TEXT", "top=N", "full", "all", "review=STATUS"],
        "formats": ["human", "json"], "default_max_candidates": 100_000,
        "max_input_bytes": 128 * 1024 * 1024,
        "source_limits": {"file_bytes":16*1024*1024,"total_bytes_per_side":64*1024*1024,"region_bytes":64*1024,"diff_lines_per_side":120},
        "population": "admitted-query-families", "source_bodies": "not-stored",
        "source_options": ["--before-source DIR", "--after-source DIR"],
        "source_verification": "containing-buffer-and-selected-content-sha256",
        "review_options": ["--reviews FILE", "--write-review FILE --decision VALUE --reason TEXT"],
        "review_decisions": ["keep-separate", "refactor", "defer"],
        "review_statuses": ["applicable", "recheck", "unreviewed"],
        "review_schema": "nose.review/v1",
        "review_filter": "review=applicable|recheck|unreviewed",
        "order": "recheck-first-then-observation-id",
        "actions": ["resume-selection", "reset-filters", "increase-budget", "recheck", "group-reason", "group-evidence", "expand-view", "return-selection", "inspect-source", "review-selection", "inspect-review"],
        "member_change_statuses": ["same-content", "same-content-new-location", "candidate", "ambiguous", "unresolved", "unmatched-current", "budget-exceeded", "unavailable"],
    })
}
