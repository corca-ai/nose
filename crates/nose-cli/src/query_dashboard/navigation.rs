use crate::path_utils::shell_quote;
use std::collections::BTreeSet;

/// Navigation hints from existing paths; these do not classify or suppress evidence.
pub(super) fn routes(
    families: &[&nose_detect::RefactorFamily],
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
