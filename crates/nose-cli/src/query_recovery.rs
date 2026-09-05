//! Recovery from an incomplete candidate search, without presenting partial findings.
use crate::{cli_args::QueryArgs, path_utils::shell_quote};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) fn explain(
    error: anyhow::Error,
    args: &QueryArgs,
    roots: &[&Path],
    exclude: &[String],
) -> anyhow::Error {
    let Some(budget) = error.downcast_ref::<nose_detect::CandidateBudgetExceeded>() else {
        return error;
    };
    let limit = budget.limit;
    let inventory = nose_frontend::discover_source_inventory(roots, exclude);
    let mut directories = BTreeMap::<PathBuf, usize>::new();
    for (file, _) in &inventory.paths {
        if let Some(parent) = Path::new(file)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            *directories.entry(parent.to_path_buf()).or_default() += 1;
        }
    }
    let mut rows: Vec<_> = directories.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut lines = vec![format!("Analysis incomplete; no clone findings were returned. Source inventory: {} supported files, {} directories, {} discovery errors.", inventory.paths.len(), rows.len(), inventory.errors.len()),
        "These are source-file counts, not candidate counts or diagnoses of the overload. Inspect a smaller root (root-relative exclusion scope changes with the root):".into()];
    let options: Vec<_> = crate::query_navigation::words(args)
        .into_iter()
        .skip(2 + 2 * args.paths.len())
        .collect();
    for (directory, count) in rows.iter().take(8) {
        let mut words = vec![
            "nose".into(),
            "query".into(),
            "--root".into(),
            directory.to_string_lossy().into_owned(),
        ];
        words.extend(options.clone());
        if args.format == crate::query_options::ReportFormat::Json {
            words.extend(["--format".into(), "json".into()]);
        }
        let command = format!(
            "nose query {}",
            words
                .iter()
                .skip(2)
                .map(|w| shell_quote(w))
                .collect::<Vec<_>>()
                .join(" ")
        );
        lines.push(format!(
            "  {count} files directly in {}\n    {command}",
            directory.display()
        ));
    }
    if rows.len() > 8 {
        lines.push(format!(
            "  {} more source directories omitted.",
            rows.len() - 8
        ));
    }
    if let Some(higher) = limit.checked_mul(2) {
        let mut retry = crate::query_navigation::words(args);
        if let Some(index) = retry
            .iter()
            .position(|word| word == "--max-candidate-pairs")
        {
            retry[index + 1] = higher.to_string();
        } else {
            retry.extend(["--max-candidate-pairs".into(), higher.to_string()]);
        }
        if args.format == crate::query_options::ReportFormat::Json {
            retry.extend(["--format".into(), "json".into()]);
        }
        lines.push(format!("To retain these roots and modes, explicitly allow more work (more time and memory; completion is not guaranteed):\n    nose query {}", retry.iter().skip(2).map(|w| shell_quote(w)).collect::<Vec<_>>().join(" ")));
    }
    lines.push("Scope and path filters run after detection; they do not reduce candidate work. No roots or detection modes were changed automatically.".into());
    error.context(lines.join("\n"))
}
