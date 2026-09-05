use super::{
    capture, navigation,
    selection::{self, Selection},
};
use crate::query_options::ReportFormat;
use anyhow::{ensure, Result};
use nose_detect::regions::evolution::{compare, Change};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

pub(super) fn run(
    before: &Path,
    after: &Path,
    budget: usize,
    terms: &[String],
    format: ReportFormat,
) -> Result<()> {
    let selection = Selection::parse(terms)?;
    let before_path = std::fs::canonicalize(before)?;
    let after_path = std::fs::canonicalize(after)?;
    let before = capture::read(&before_path)?;
    let after = capture::read(&after_path)?;
    let index = selection::Observations::new(&before, &after);
    let comparison = compare(&before, &after, budget).map_err(anyhow::Error::msg)?;
    let mut rows: Vec<_> = comparison
        .changes
        .iter()
        .filter(|r| selection.keeps(r, &index))
        .collect();
    if let Some(id) = &selection.change {
        rows.retain(|r| r.id.hex().starts_with(id));
        ensure!(
            !rows.is_empty(),
            "no change matching `{id}` in this selection; remove change= to browse"
        );
        ensure!(
            rows.len() == 1,
            "ambiguous change id `{id}`; use a longer prefix"
        );
    }
    let selected = rows.len();
    let base = navigation::selection_terms(terms);
    let command = |suffix: Vec<String>| {
        let mut next = base.clone();
        next.extend(suffix);
        navigation::command(&before_path, &after_path, budget, &next)
    };
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    let group_field = selection.group.as_deref().unwrap_or("reason");
    for row in &rows {
        for value in selection::values(row, group_field, &index) {
            *groups.entry(value).or_default() += 1;
        }
    }
    if selection.top != 0 {
        rows.truncate(selection.top);
    }
    let detail = selection.full || selection.change.is_some();
    let items: Vec<_> = rows
        .iter()
        .map(|row| {
            row_json(
                row,
                &index,
                detail,
                command(vec![format!("change={}", row.id.hex()), "full".into()]),
            )
        })
        .collect();
    let group_rows: Vec<_> = groups.iter().take(if selection.top == 0 { usize::MAX } else { selection.top })
        .map(|(key,count)| json!({"key":key,"count":count,"next":[command(vec![format!("{group_field}={}", if group_field == "path" { serde_json::to_string(key).unwrap() } else { key.clone() })])]})).collect();
    let view = if selection.change.is_some() {
        "change"
    } else if selection.group.is_some() {
        "group"
    } else if terms.is_empty() {
        "dashboard"
    } else {
        "list"
    };
    let mut next = vec![
        command(vec!["group=reason".into()]),
        command(vec!["group=evidence".into()]),
        command(vec!["top=0".into()]),
    ];
    if selected == 0 {
        next.insert(
            0,
            navigation::command(&before_path, &after_path, budget, &[]),
        );
    }
    let output = json!({
        "schema":comparison.schema,"view":view,
        "inputs":{"before":before_path,"after":after_path},
        "population":"admitted-query-families", "profile_matches":comparison.profile_matches,
        "profiles":{"before":before.profile,"after":after.profile},
        "roots":{"before":before.roots,"after":after.roots},
        "path_bases":{"before":before.path_base,"after":after.path_base},
        "complete":comparison.complete,"candidates_examined":comparison.candidates_examined,"max_candidates":budget,
        "summary":{"total":comparison.changes.len(),"selected":selected,"shown":if view == "group" { 0 } else { items.len() },
            "before_families":before.families.len(),"after_families":after.families.len(),
            "groups_total":groups.len(),"groups_shown":group_rows.len(),
            "retained":comparison.changes.iter().filter(|r| r.unchanged_evidence).count()},
        "group_field":group_field,"groups":group_rows,"group_counts_overlap":true,
        "items":if view == "group" { Vec::new() } else { items },"next":next,
        "notes":["Retained evidence is a fact for caller policy, never approval or ancestry.",
            "Absence means unmatched within captured admitted families, not deleted code or a completed refactor.",
            "Changed evidence facets describe observed differences; they do not identify a causal edit.",
            "Source bodies are not stored; detailed observations remain available without the workspace."],
    });
    if format == ReportFormat::Json {
        println!("{}", serde_json::to_string(&output)?);
    } else {
        super::render::render(&output);
    }
    Ok(())
}

fn row_json(
    row: &Change,
    index: &selection::Observations<'_>,
    detail: bool,
    next: String,
) -> Value {
    let mut output = serde_json::to_value(row).expect("change serializes");
    output["next"] = json!([next]);
    output["reason_details"] = json!(row
        .reasons
        .iter()
        .map(|code| json!({
            "code":code, "meaning":super::render::reason(code),
        }))
        .collect::<Vec<_>>());
    output["scope"] = json!(selection::values(row, "scope", index));
    output["paths"] = json!(selection::values(row, "path", index));
    if detail {
        output["before_observation"] = json!(row.before.and_then(|id| index.before.get(&id)));
        output["after_observations"] = json!(row
            .after
            .iter()
            .filter_map(|id| index.after.get(id))
            .collect::<Vec<_>>());
        output["source_body_status"] = json!("not-stored");
    }
    output
}
