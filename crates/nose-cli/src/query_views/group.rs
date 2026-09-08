use super::*;
use std::collections::HashMap;

pub(crate) struct QueryGroupView<'a> {
    pub(crate) analysis: &'a serde_json::Value,
    pub(crate) selection: &'a [&'a nose_detect::RefactorFamily],
    pub(crate) top: Option<usize>,
    pub(crate) field: &'a str,
    pub(crate) terms: &'a [String],
    pub(crate) path: &'a str,
    pub(crate) navigation_path: &'a str,
    pub(crate) json: bool,
    pub(crate) baseline_comparison: Option<&'a BaselineComparison>,
    pub(crate) since: Option<&'a BaselineComparison>,
    pub(crate) semantic_packs: &'a [serde_json::Value],
}

#[derive(Default)]
struct GroupAgg {
    count: usize,
    removable: u32,
    exemplar_id: String,
    exemplar_row: String,
}

pub(crate) fn render_query_group(view: QueryGroupView<'_>) {
    let rows = grouped_rows(&view);
    let total = rows.len();
    let shown = &rows[..total.min(query_row_limit(view.top))];
    if view.json {
        render_group_json(&view, shown, total);
    } else {
        render_group_human(&view, shown, total);
    }
}

fn group_key(view: &QueryGroupView<'_>, family: &nose_detect::RefactorFamily) -> String {
    match view.field {
        "scope" => family.scope.to_string(),
        "witness" => {
            witness_token(family.witness.as_ref().map(|witness| witness.kind())).to_string()
        }
        "lang" | "language" => family
            .locations
            .first()
            .map(|location| location.lang.as_str().to_string())
            .unwrap_or_default(),
        "dir" => family_dir(family),
        "file" => family
            .locations
            .first()
            .map(|location| location.file.clone())
            .unwrap_or_default(),
        "shape" | "extraction_shape" => family.extraction_shape().to_string(),
        "same_symbol" => family_same_symbol(family).to_string(),
        "spotclass" => family_spotclass(family)
            .unwrap_or("unwitnessed")
            .to_string(),
        "status" => view.since.or(view.baseline_comparison).map_or_else(
            || "?".to_string(),
            |comparison| family_status(family, comparison).to_string(),
        ),
        _ => "?".to_string(),
    }
}

fn grouped_rows(view: &QueryGroupView<'_>) -> Vec<(String, GroupAgg)> {
    let mut buckets: HashMap<String, GroupAgg> = HashMap::new();
    for family in view.selection {
        let aggregate = buckets.entry(group_key(view, family)).or_default();
        if aggregate.count == 0 {
            aggregate.exemplar_id = baseline::family_id(family);
            aggregate.exemplar_row = query_row(family);
        }
        aggregate.count += 1;
        aggregate.removable += removable_lines(family);
    }
    let mut rows: Vec<(String, GroupAgg)> = buckets.into_iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .removable
            .cmp(&left.1.removable)
            .then(right.1.count.cmp(&left.1.count))
            .then(left.0.cmp(&right.0))
    });
    rows
}

fn render_group_json(view: &QueryGroupView<'_>, rows: &[(String, GroupAgg)], total: usize) {
    let groups: Vec<_> = rows
        .iter()
        .map(|(key, aggregate)| {
            serde_json::json!({
                "key": key,
                "count": aggregate.count,
                "removable": aggregate.removable,
                "exemplar_id": aggregate.exemplar_id,
                "next": [group_command(view, key, &aggregate.exemplar_id)],
            })
        })
        .collect();
    println!(
        "{}",
        with_semantic_packs(
            serde_json::json!({
                "schema_version": schema_versions::QUERY_JSON_SCHEMA_VERSION,
                "tool": "nose",
                "view": "group",
                "analysis": view.analysis,
                "path": view.path,
                "field": view.field,
                "groups": groups,
                "summary":{"families":view.selection.len(),"groups_total":total,"groups_shown":rows.len()},
                "next":if rows.len() < total { vec![expand_command(view)] } else { vec![] },
            }),
            view.semantic_packs
        )
    );
}

fn render_group_human(view: &QueryGroupView<'_>, rows: &[(String, GroupAgg)], total: usize) {
    println!(
        "{} {} by {} (most removable first):",
        view.selection.len(),
        plural(view.selection.len(), "family", "families"),
        view.field
    );
    println!("Showing {} / {total} groups.", rows.len());
    println!("Group counts include all matching families; top=N limits displayed groups, not input families.");
    for (key, aggregate) in rows {
        let label = if view.field == "witness" && key == "subdag" {
            "shared-core"
        } else {
            key.as_str()
        };
        println!(
            "  {label:<16} ({:>3} {} · ~{} removable)  e.g. {}",
            aggregate.count,
            plural(aggregate.count, "family", "families"),
            aggregate.removable,
            aggregate.exemplar_row
        );
        println!(
            "        {}",
            group_command(view, key, &aggregate.exemplar_id)
        );
    }
    if rows.len() < total {
        println!("Show all groups: {}", expand_command(view));
    }
}

fn expand_command(view: &QueryGroupView<'_>) -> String {
    let terms = view
        .terms
        .iter()
        .filter(|t| !t.starts_with("top="))
        .cloned()
        .collect::<Vec<_>>();
    format!(
        "{} group={} top=0{}",
        base_cmd(&terms, view.navigation_path),
        view.field,
        if view.json { " --format json" } else { "" }
    )
}

fn group_command(view: &QueryGroupView<'_>, key: &str, exemplar: &str) -> String {
    let base = base_cmd(view.terms, view.navigation_path);
    let filter = format!("{}={key}", view.field);
    let term = if crate::query_terms::parse_query(std::slice::from_ref(&filter)).is_ok() {
        crate::path_utils::shell_quote(&filter)
    } else {
        format!("id={exemplar} full")
    };
    format!(
        "{base} {term}{}",
        if view.json { " --format json" } else { "" }
    )
}
