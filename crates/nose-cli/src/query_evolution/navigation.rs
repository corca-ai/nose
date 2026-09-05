use std::path::Path;

pub(super) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn command(before: &Path, after: &Path, budget: usize, terms: &[String]) -> String {
    let mut args = vec![
        "nose".to_string(),
        "query".into(),
        "--before".into(),
        quote(&before.to_string_lossy()),
        "--after".into(),
        quote(&after.to_string_lossy()),
        "--max-candidates".into(),
        budget.to_string(),
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
