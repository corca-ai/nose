use crate::{cli_args::QueryArgs, query_options::DetectionMode};

/// Replay inspection settings without gates or write operations.
pub(crate) fn words(args: &QueryArgs) -> Vec<String> {
    let mut words = vec!["nose".into(), "query".into()];
    for path in &args.paths {
        words.extend(["--root".into(), path.to_string_lossy().into_owned()]);
    }
    for mode in &args.mode {
        let name = match mode {
            DetectionMode::Syntax => "syntax".into(),
            DetectionMode::Semantic => "semantic".into(),
            DetectionMode::Near(t) => t.map_or_else(|| "near".into(), |t| format!("near:{t}")),
            DetectionMode::Abstraction(t) => {
                t.map_or_else(|| "abstraction".into(), |t| format!("abstraction:{t}"))
            }
        };
        words.extend(["--mode".into(), name]);
    }
    for (flag, value) in [
        ("--min-size", args.min_size.map(|n| n.to_string())),
        (
            "--max-candidate-pairs",
            args.max_candidate_pairs.map(|n| n.to_string()),
        ),
        ("--min-lines", args.min_lines.map(|n| n.to_string())),
        ("--min-members", args.min_members.map(|n| n.to_string())),
        ("--min-value", args.min_value.map(|n| n.to_string())),
        (
            "--cache-max-bytes",
            args.cache_max_bytes.map(|n| n.to_string()),
        ),
    ] {
        if let Some(value) = value {
            words.extend([flag.into(), value]);
        }
    }
    for (flag, value) in [
        ("--config", &args.config),
        ("--baseline", &args.baseline),
        ("--ignore-file", &args.ignore_file),
        ("--cache-dir", &args.cache_dir),
        ("--semantic-pack-lock", &args.semantic_pack_lock),
    ] {
        if let Some(value) = value {
            words.extend([flag.into(), value.to_string_lossy().into_owned()]);
        }
    }
    for path in &args.semantic_pack {
        words.extend([
            "--semantic-pack".into(),
            path.to_string_lossy().into_owned(),
        ]);
    }
    for glob in &args.exclude {
        words.extend(["--exclude".into(), glob.clone()]);
    }
    for glob in &args.generated_path {
        words.extend(["--generated-path".into(), glob.clone()]);
    }
    words
}

/// Keep the caller's root spelling while sharing the option replay contract.
pub(crate) fn path(args: &QueryArgs, root_expression: &str) -> String {
    let options = words(args)
        .into_iter()
        .skip(2 + 2 * args.paths.len())
        .map(|word| crate::path_utils::shell_quote(&word))
        .collect::<Vec<_>>();
    if options.is_empty() {
        root_expression.into()
    } else {
        format!("{root_expression} {}", options.join(" "))
    }
}
