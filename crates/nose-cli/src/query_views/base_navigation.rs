use crate::{cli_args::QueryArgs, path_utils::shell_quote, query_options::ReportFormat};
use serde_json::{json, Value};

pub(crate) struct BaseViewOptions<'a> {
    pub format: ReportFormat,
    pub actions: &'a [Value],
    pub semantic_packs: &'a [Value],
}

/// Base evidence depends on the workspace and Git. Keep that context explicit,
/// and replay only inspection options in the evidence action (never a gate).
pub(crate) fn actions(args: &QueryArgs, base_ref: &str) -> anyhow::Result<Vec<Value>> {
    let cwd = std::env::current_dir()?;
    let mut words = crate::query_navigation::words(args);
    words.push(format!("base={base_ref}"));
    let command = |suffix: &[&str]| {
        format!(
            "cd {} && {}",
            shell_quote(&cwd.to_string_lossy()),
            words
                .iter()
                .map(String::as_str)
                .chain(suffix.iter().copied())
                .map(shell_quote)
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    Ok(vec![
        json!({"kind":"inspect-evidence", "label":"Inspect all findings and source-region candidates as JSON (same workspace and analysis options)",
            "command":command(&["--format", "json", "top=0"])}),
        json!({"kind":"gate", "label":"Run the strict divergence CI gate", "command":command(&["--format", if args.format == ReportFormat::Json { "json" } else { "human" }, "--fail-on", "any"])}),
    ])
}
