use super::{
    capture, navigation,
    selection::{self, Selection},
};
use crate::query_options::ReportFormat;
use anyhow::Result;
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
    let (before_path, before) = capture::input(before, "--before")?;
    let (after_path, after) = capture::input(after, "--after")?;
    let index = selection::Observations::new(&before, &after);
    let comparison = compare(&before, &after, budget).map_err(anyhow::Error::msg)?;
    let details = (selection.full || selection.change.is_some())
        .then(|| super::details::Details::new(&after, &comparison.member_correspondences));
    let mut rows = selection.select(&comparison.changes, &index)?;
    let selected = rows.len();
    let selected_retained = rows.iter().filter(|r| r.unchanged_evidence).count();
    let retained = comparison
        .changes
        .iter()
        .filter(|r| r.unchanged_evidence)
        .count();
    let recheck = comparison.changes.len() - retained;
    rows.sort_by_key(|r| (r.unchanged_evidence, r.id));
    let search_complete = !comparison
        .changes
        .iter()
        .any(|r| r.correspondence == "budget-exceeded")
        && !comparison
            .member_correspondences
            .iter()
            .any(|r| r.kind == nose_detect::regions::ChangeKind::BudgetExceeded);
    let navigation = navigation::Navigation::new(&before_path, &after_path, budget, terms, format);
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
    let items: Vec<_> = rows
        .iter()
        .map(|row| {
            row_json(
                row,
                &index,
                details.as_ref(),
                navigation.selected(vec![format!("change={}", row.id.hex()), "full".into()]),
            )
        })
        .collect();
    let group_rows: Vec<_> = groups.iter().take(if selection.top == 0 { usize::MAX } else { selection.top })
        .map(|(key,count)| json!({"key":key,"count":count,"next":[navigation.selected(vec![format!("{group_field}={}", if group_field == "path" { serde_json::to_string(key).unwrap() } else { key.clone() })])]})).collect();
    let view = if selection.change.is_some() {
        "change"
    } else if selection.group.is_some() {
        "group"
    } else if terms.is_empty() {
        "dashboard"
    } else {
        "list"
    };
    let actions = navigation.actions(&selection, selected, recheck, search_complete);
    let next: Vec<_> = actions.iter().map(|a| a["command"].clone()).collect();
    let output = json!({
        "schema":comparison.schema,"view":view,
        "inputs":{"before":before_path,"after":after_path},
        "population":"admitted-query-families", "profile_matches":comparison.profile_matches,
        "profiles":{"before":before.profile,"after":after.profile},
        "roots":{"before":before.roots,"after":after.roots},
        "path_bases":{"before":before.path_base,"after":after.path_base},
        "coverage":{"before":capture::coverage(&before),"after":capture::coverage(&after)},
        "complete":comparison.complete,"candidates_examined":comparison.candidates_examined,"max_candidates":budget,
        "summary":{"total":comparison.changes.len(),"selected":selected,"shown":if view == "group" { 0 } else { items.len() },
            "before_families":before.families.len(),"after_families":after.families.len(),
            "groups_total":groups.len(),"groups_shown":group_rows.len(),
            "retained":retained,"recheck":recheck,"selected_retained":selected_retained,"selected_recheck":selected-selected_retained},
        "group_field":group_field,"groups":group_rows,"group_counts_overlap":true,
        "items":if view == "group" { Vec::new() } else { items },"next":next,"actions":actions,
        "candidate_search_complete":search_complete,
        "order":"recheck-first-then-observation-id",
        "empty_message":if selected == 0 { Some(if recheck == 0 { "No recheck observations in this captured population; no observations match the current selection." } else { "No observations match the current filters; other observations remain in the captured population." }) } else { None },
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
    details: Option<&super::details::Details<'_>>,
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
    if let Some(details) = details {
        output["member_changes"] = details.summarize(row, index);
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
