use crate::query_options::ReportFormat;
use std::path::Path;

use crate::path_utils::shell_quote as quote;

pub(super) fn command(
    before: &Path,
    after: &Path,
    budget: usize,
    terms: &[String],
    format: ReportFormat,
) -> String {
    let mut args = vec![
        "nose".to_string(),
        "query".into(),
        "--before".into(),
        quote(&before.to_string_lossy()),
        "--after".into(),
        quote(&after.to_string_lossy()),
        "--max-candidates".into(),
        budget.to_string(),
        "--format".into(),
        if format == ReportFormat::Json {
            "json"
        } else {
            "human"
        }
        .into(),
    ];
    args.extend(terms.iter().map(|t| quote(t)));
    args.join(" ")
}

pub(super) fn selection_terms(terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .filter(|t| {
            !t.starts_with("group=")
                && !t.starts_with("change=")
                && !t.starts_with("top=")
                && *t != "full"
        })
        .cloned()
        .collect()
}

pub(super) struct Navigation<'a> {
    before: &'a Path,
    after: &'a Path,
    budget: usize,
    terms: &'a [String],
    base: Vec<String>,
    format: ReportFormat,
}

impl<'a> Navigation<'a> {
    pub(super) fn new(
        before: &'a Path,
        after: &'a Path,
        budget: usize,
        terms: &'a [String],
        format: ReportFormat,
    ) -> Self {
        Self {
            before,
            after,
            budget,
            terms,
            base: selection_terms(terms),
            format,
        }
    }

    pub(super) fn selected(&self, suffix: Vec<String>) -> String {
        let mut terms = self.base.clone();
        terms.extend(suffix);
        command(self.before, self.after, self.budget, &terms, self.format)
    }

    pub(super) fn actions(
        &self,
        selection: &super::selection::Selection,
        selected: usize,
        recheck: usize,
        search_complete: bool,
    ) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut actions = Vec::new();
        let action = |kind: &str, label: &str, command: String| json!({"kind":kind,"label":label,"command":command});
        if selected == 0 {
            actions.push(action(
                "reset-filters",
                "Clear filters and return to the comparison",
                command(self.before, self.after, self.budget, &[], self.format),
            ));
        }
        if !search_complete {
            if let Some(higher) = self
                .budget
                .checked_mul(2)
                .map(|n| n.max(100_000))
                .filter(|n| *n > self.budget)
            {
                let retry: Vec<_> = self
                    .terms
                    .iter()
                    .filter(|t| !t.starts_with("change="))
                    .cloned()
                    .collect();
                actions.push(action("increase-budget", &format!("Retry with candidate budget {higher} (more work; return from any change address)"), command(self.before, self.after, higher, &retry, self.format)));
            }
        }
        if recheck > 0 {
            let mut recheck_terms: Vec<_> = self
                .base
                .iter()
                .filter(|t| !t.starts_with("evidence=") && !t.starts_with("evidence!="))
                .cloned()
                .collect();
            recheck_terms.push("evidence=recheck".into());
            actions.push(action(
                "recheck",
                "Inspect recheck observations (replace the evidence filter)",
                command(
                    self.before,
                    self.after,
                    self.budget,
                    &recheck_terms,
                    self.format,
                ),
            ));
        }
        actions.push(action(
            "group-reason",
            "Group this selection by change reason",
            self.selected(vec!["group=reason".into()]),
        ));
        actions.push(action(
            "group-evidence",
            "Group this selection by retained/recheck evidence",
            self.selected(vec!["group=evidence".into()]),
        ));
        if selection.change.is_some() {
            actions.push(action(
                "return-selection",
                "Return to the selected observations",
                self.selected(Vec::new()),
            ));
        } else {
            let expanded: Vec<_> = self
                .terms
                .iter()
                .filter(|t| !t.starts_with("top="))
                .cloned()
                .chain(["top=0".into()])
                .collect();
            actions.push(action(
                "expand-view",
                "Show all entries in this view",
                command(self.before, self.after, self.budget, &expanded, self.format),
            ));
        }
        actions
    }
}
