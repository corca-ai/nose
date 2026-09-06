use super::{
    capture, navigation,
    selection::{self, Selection},
};
use crate::query_options::ReportFormat;
use anyhow::Result;
use nose_detect::regions::evolution::{compare, Change};
use serde_json::json;
use std::{collections::BTreeMap, path::Path};

pub(super) fn run(
    before: &Path,
    after: &Path,
    options: &super::AnalysisArgs,
    terms: &[String],
    format: ReportFormat,
) -> Result<()> {
    let budget = options.max_candidates;
    let selection = Selection::parse(terms)?;
    anyhow::ensure!(
        (options.before_source.is_none()
            && options.after_source.is_none()
            && options.write_review.is_none())
            || selection.change.is_some(),
        "source inspection and review recording require change=ID; open one observation first"
    );
    let (before_path, before) = capture::input(before, "--before")?;
    let (after_path, after) = capture::input(after, "--after")?;
    let index = selection::Observations::new(&before, &after);
    let reviews = super::reviews::Reviews::load(&options.reviews, &before, &after)?;
    let comparison = compare(&before, &after, budget).map_err(anyhow::Error::msg)?;
    let details = (selection.full || selection.change.is_some())
        .then(|| super::details::Details::new(&after, &comparison.member_correspondences));
    let assessments = reviews.evaluate(&comparison, &index);
    let mut rows = selection.select(&comparison.changes, &index)?;
    if let Some(status) = &selection.review {
        rows.retain(|row| super::reviews::status(&assessments[&row.id]) == status);
    }
    let selected = rows.len();
    let selected_retained = rows.iter().filter(|r| r.unchanged_evidence).count();
    let retained = comparison
        .changes
        .iter()
        .filter(|r| r.unchanged_evidence)
        .count();
    let recheck = comparison.changes.len() - retained;
    rows.sort_by_key(|r| (r.unchanged_evidence, r.id));
    let search_complete = candidate_search_complete(&comparison);
    let navigation = navigation::Navigation::new(&before_path, &after_path, budget, terms, format)
        .with_reviews(&options.reviews)
        .with_sources(options);
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
    let mut item_view = super::items::Items {
        index: &index,
        details: details.as_ref(),
        navigation: &navigation,
        options,
        before: &before,
        after: &after,
        assessments: &assessments,
    };
    let items = item_view.rows(&rows)?;
    let group_rows: Vec<_> = groups.iter().take(if selection.top == 0 { usize::MAX } else { selection.top })
        .map(|(key,count)| json!({"key":key,"count":count,"next":[navigation.selected(vec![format!("{group_field}={}", if group_field == "path" { serde_json::to_string(key).unwrap() } else { key.clone() })])]})).collect();
    let view = selection.view(terms.is_empty());
    let mut actions = navigation.actions(&selection, selected, recheck, search_complete);
    if let Some(path) = &options.write_review {
        actions.push(json!({"kind":"inspect-review", "label":"Explore the saved caller decision", "command":format!("{} --reviews {}", navigation.selected(Vec::new()), crate::path_utils::shell_quote(&path.to_string_lossy()))}));
    }
    let next: Vec<_> = actions.iter().map(|a| a["command"].clone()).collect();
    let output = json!({
        "schema":comparison.schema,"view":view,"exploration":before_path == after_path,
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
        "reviews":{"unrelated":reviews.unrelated(),"written":options.write_review,
            "meaning":"Caller decisions with explicit applicability conditions; no findings are suppressed and no edits are authorized."},
        "order":"recheck-first-then-observation-id",
        "empty_message":if selected == 0 { Some(if recheck == 0 { "No recheck observations in this captured population; no observations match the current selection." } else { "No observations match the current filters; other observations remain in the captured population." }) } else { None },
        "notes":["Retained evidence is a fact for caller policy, never approval or ancestry.",
            "Absence means unmatched within captured admitted families, not deleted code or a completed refactor.",
            "Changed evidence facets describe observed differences; they do not identify a causal edit.",
            "Source bodies are not stored; detailed observations remain available without the workspace."],
    });
    record_review(options, &rows, &after, &index)?;
    if format == ReportFormat::Json {
        println!("{}", serde_json::to_string(&output)?);
    } else {
        super::render::render(&output, details.is_some());
    }
    Ok(())
}

fn record_review(
    options: &super::AnalysisArgs,
    rows: &[&Change],
    after: &nose_detect::regions::evolution::AnalysisSnapshot,
    index: &selection::Observations<'_>,
) -> Result<()> {
    if let Some(path) = &options.write_review {
        anyhow::ensure!(rows.len() == 1 && rows[0].after.len() == 1, "review recording requires exactly one current family; ambiguous or unresolved relations cannot choose a target");
        super::reviews::write(
            path,
            after,
            index.after[&rows[0].after[0]],
            options.decision.expect("clap requires decision"),
            options.reason.as_deref().expect("clap requires reason"),
        )?;
    }
    Ok(())
}

fn candidate_search_complete(comparison: &nose_detect::regions::evolution::Comparison) -> bool {
    !comparison
        .changes
        .iter()
        .any(|r| r.correspondence == "budget-exceeded")
        && !comparison
            .member_correspondences
            .iter()
            .any(|r| r.kind == nose_detect::regions::ChangeKind::BudgetExceeded)
}
