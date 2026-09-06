use crate::path_utils::shell_quote;
use std::collections::BTreeSet;

/// Navigation hints from existing paths; these do not classify or suppress evidence.
pub(super) fn routes(
    families: &[&nose_detect::RefactorFamily],
    analysis: &serde_json::Value,
    path: &str,
    json: bool,
) -> Vec<(String, String)> {
    let evaluation: BTreeSet<_> = families
        .iter()
        .flat_map(|f| &f.locations)
        .flat_map(|loc| loc.file.split('/'))
        .filter(|part| {
            matches!(
                *part,
                "bench" | "benches" | "benchmarks" | "eval" | "evaluation" | "corpus" | "fixtures"
            )
        })
        .collect();
    let command = |terms: Vec<String>| {
        format!(
            "nose query {path} {}{}",
            terms
                .iter()
                .map(|term| shell_quote(term))
                .collect::<Vec<_>>()
                .join(" "),
            if json { " --format json" } else { "" }
        )
    };
    let mut product = vec!["scope=prod".into()];
    product.extend(evaluation.iter().map(|dir| format!("path!~{dir}/")));
    let mut routes = vec![
        (
            "Production outside evaluation/fixture directories".into(),
            command(product),
        ),
        ("Tests".into(), command(vec!["scope=test".into()])),
        (
            "Production/test relationships".into(),
            command(vec!["scope=mixed".into()]),
        ),
    ];
    for (directory, count) in directory_routes(families, analysis).into_iter().take(6) {
        routes.push((
            format!("{directory} · {count} families (directory hint)"),
            command(vec![format!("path~{directory}")]),
        ));
    }
    routes.extend(evaluation.into_iter().map(|dir| {
        (
            format!("{dir}/ (directory hint)"),
            command(vec![format!("path~{dir}/")]),
        )
    }));
    routes
}

/// Filters can expose a slice whose representative is outside the selection.
/// Advertised drilldown counts must use the same selection rule as the target view.
pub(super) fn witness_count(
    families: &[nose_detect::RefactorFamily],
    overrides: &crate::surfaces::SurfaceOverrides,
    opportunities: &crate::query_opportunities::OpportunityGroups,
    path: &str,
    witness: &str,
) -> usize {
    let query = crate::query_terms::parse_query(&[format!("witness={witness}")])
        .expect("dashboard evidence filter is supported");
    crate::query_output::query_selection(families, overrides, opportunities, &query, path, None)
        .expect("evidence filter selection is valid")
        .len()
}

/// Group by the first directory inside each explicit root, keeping ownership a caller judgment.
fn directory_routes(
    families: &[&nose_detect::RefactorFamily],
    analysis: &serde_json::Value,
) -> Vec<(String, usize)> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let roots: Vec<_> = analysis["roots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str())
        .map(|p| cwd.join(p))
        .collect();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for family in families {
        let mut paths = BTreeSet::new();
        for loc in &family.locations {
            let file = cwd.join(&loc.file);
            for root in &roots {
                let Ok(relative) = file.strip_prefix(root) else {
                    continue;
                };
                let mut parts = relative.components();
                let Some(first) = parts.next() else { continue };
                if parts.next().is_none() {
                    continue;
                }
                let directory = root.join(first.as_os_str());
                let display = directory.strip_prefix(&cwd).unwrap_or(&directory);
                paths.insert(format!("{}/", display.to_string_lossy()));
            }
        }
        for path in paths {
            *counts.entry(path).or_default() += 1;
        }
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}
