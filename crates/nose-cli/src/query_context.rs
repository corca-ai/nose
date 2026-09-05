//! Effective analysis population, distinct from filters on completed findings.
use crate::{cli_args::QueryArgs, query_dataset::QuerySettings, query_options::QueryScope};
use serde_json::{json, Value};

pub(crate) fn describe(args: &QueryArgs, settings: &QuerySettings, scope: &QueryScope) -> Value {
    let modes = settings.channels.mode_names();
    json!({"scope":"selected-roots", "complete":true,
        "roots":args.paths, "scanned_files":scope.files, "languages":scope.langs,
        "skipped_sources":scope.skipped_sources, "modes":modes, "exclude":settings.exclude,
        "gitignore":true,
        "max_candidate_pairs":crate::detect_pipeline::candidate_limit(args.max_candidate_pairs).expect("candidate limit validated during analysis"), "min_size":settings.min_tokens, "min_lines":settings.min_lines,
        "min_members":settings.min_members, "min_value":settings.min_value,
        "meaning":"Complete detection within the selected roots, modes and discovery rules; query filters select findings from this population."})
}

pub(crate) fn render(context: &Value) {
    let strings = |key: &str| {
        context[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!(
        "analysis: {} files · roots: {} · modes: {}",
        context["scanned_files"],
        strings("roots"),
        strings("modes")
    );
    println!("  discovery: gitignore respected · exclude: {} · min-size {} · min-lines {} · min-members {} · min-value {}",
        if context["exclude"].as_array().unwrap().is_empty() { "none".into() } else { strings("exclude") },
        context["min_size"], context["min_lines"], context["min_members"], context["min_value"]);
    let ceiling = context["max_candidate_pairs"]
        .as_u64()
        .map_or_else(String::new, |limit| {
            format!("candidate limit: {limit} distinct pairs · ")
        });
    println!("  {ceiling}filters select findings within this population");
}
