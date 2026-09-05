use crate::{
    cli_args::QueryArgs,
    path_utils::shell_quote,
    query_options::{DetectionMode, ReportFormat},
};
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
    let mut words = vec!["nose".into(), "query".into()];
    for path in &args.paths {
        words.extend(["--root".into(), path.to_string_lossy().into_owned()]);
    }
    words.push(format!("base={base_ref}"));
    for mode in &args.mode {
        let name = match mode {
            DetectionMode::Syntax => "syntax".into(),
            DetectionMode::Semantic => "semantic".into(),
            DetectionMode::Near(t) => t.map_or_else(|| "near".into(), |t| format!("near:{t}")),
            DetectionMode::Abstraction(t) => {
                t.map_or_else(|| "abstraction".into(), |t| format!("abstraction:{t}"))
            }
        };
        words.extend(["--mode".into(), name]);
    }
    for (flag, value) in [
        ("--min-size", args.min_size.map(|n| n.to_string())),
        ("--min-lines", args.min_lines.map(|n| n.to_string())),
        (
            "--cache-max-bytes",
            args.cache_max_bytes.map(|n| n.to_string()),
        ),
    ] {
        if let Some(value) = value {
            words.extend([flag.into(), value]);
        }
    }
    for (flag, value) in [
        ("--config", &args.config),
        ("--ignore-file", &args.ignore_file),
        ("--cache-dir", &args.cache_dir),
        ("--semantic-pack-lock", &args.semantic_pack_lock),
    ] {
        if let Some(value) = value {
            words.extend([flag.into(), value.to_string_lossy().into_owned()]);
        }
    }
    for path in &args.semantic_pack {
        words.extend([
            "--semantic-pack".into(),
            path.to_string_lossy().into_owned(),
        ]);
    }
    for glob in &args.exclude {
        words.extend(["--exclude".into(), glob.clone()]);
    }
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
