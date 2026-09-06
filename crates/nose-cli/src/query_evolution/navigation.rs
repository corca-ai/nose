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
    reviews: Vec<String>,
    sources: String,
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
            reviews: Vec::new(),
            sources: String::new(),
        }
    }

    pub(super) fn with_reviews(mut self, paths: &[std::path::PathBuf]) -> Self {
        self.reviews = paths
            .iter()
            .map(|p| format!(" --reviews {}", quote(&p.to_string_lossy())))
            .collect();
        self
    }

    pub(super) fn with_sources(mut self, options: &super::AnalysisArgs) -> Self {
        for (flag, path) in [
            ("--before-source", &options.before_source),
            ("--after-source", &options.after_source),
        ] {
            if let Some(path) = path {
                self.sources
                    .push_str(&format!(" {flag} {}", quote(&path.to_string_lossy())));
            }
        }
        self
    }

    pub(super) fn selected(&self, suffix: Vec<String>) -> String {
        let mut terms = self.base.clone();
        terms.extend(suffix);
        self.command(self.budget, &terms)
    }

    fn command(&self, budget: usize, terms: &[String]) -> String {
        format!(
            "{}{}",
            command(self.before, self.after, budget, terms, self.format),
            self.reviews.join("")
        )
    }

    fn resume_action(&self) -> serde_json::Value {
        serde_json::json!({"kind":"resume-selection","label":"Resume this saved selection",
            "command":format!("{}{}", self.command(self.budget, self.terms), self.sources)})
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
        if !self.reviews.is_empty() {
            for (status, label) in [
                (
                    "recheck",
                    "Revisit caller decisions whose conditions need review",
                ),
                (
                    "applicable",
                    "Inspect caller decisions whose conditions still hold",
                ),
            ] {
                let mut terms: Vec<_> = self
                    .base
                    .iter()
                    .filter(|t| !t.starts_with("review="))
                    .cloned()
                    .collect();
                terms.push(format!("review={status}"));
                actions.push(action(
                    "review-selection",
                    label,
                    self.command(self.budget, &terms),
                ));
            }
        }
        if selected == 0 {
            actions.push(action(
                "reset-filters",
                "Clear filters and return to the comparison",
                self.command(self.budget, &[]),
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
                actions.push(action("increase-budget", &format!("Retry with candidate budget {higher} (more work; return from any change address)"), self.command(higher, &retry)));
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
                self.command(self.budget, &recheck_terms),
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
                self.command(self.budget, &expanded),
            ));
        }
        if selected > 0 && !self.terms.is_empty() {
            actions.push(self.resume_action());
        }
        actions
    }
}
