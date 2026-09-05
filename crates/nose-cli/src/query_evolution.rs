//! Query exploration over explicitly captured analysis populations.
mod capture;
mod navigation;
mod render;
mod selection;
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
    /// Compare two saved analyses without reading source, config, Git, or cache.
    #[arg(long, value_name = "FILE", requires = "after")]
    pub(crate) before: Option<PathBuf>,
    #[arg(long, value_name = "FILE", requires = "before")]
    pub(crate) after: Option<PathBuf>,
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
        min_size,
        min_lines,
        min_value,
        min_members,
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
    let Some(before) = &analysis.before else {
        return Ok(false);
    };
    ensure!(roots.is_empty() && !watch && mode.is_empty() && min_size.is_none()
        && min_lines.is_none() && min_value.is_none() && min_members.is_none() && exclude.is_empty()
        && generated_path.is_empty() && cache_dir.is_none() && cache_max_bytes.is_none()
        && ignore_file.is_none() && semantic_pack.is_empty() && semantic_pack_lock.is_none()
        && config.is_none() && !config_root && !show_config && fail_on.is_none()
        && baseline.is_none() && !write_baseline,
        "analysis comparison uses its captured roots/profile; pass only --before, --after, --max-candidates, --format and comparison terms");
    ensure!(
        matches!(format, ReportFormat::Human | ReportFormat::Json),
        "analysis comparison supports --format human or json"
    );
    let after = analysis.after.as_ref().expect("clap requires --after");
    view::run(before, after, analysis.max_candidates, positionals, *format)?;
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
        "terms": ["group=FIELD", "change=ID", "FIELD=VALUE", "FIELD!=VALUE", "path~TEXT", "path!~TEXT", "top=N", "full", "all"],
        "formats": ["human", "json"], "default_max_candidates": 100_000,
        "max_input_bytes": 128 * 1024 * 1024,
        "population": "admitted-query-families", "source_bodies": "not-stored",
    })
}
